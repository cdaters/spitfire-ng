use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sf_core::{
    DisplayFormat, DisplayResource, DisplaySource, LogicalPath, LogicalPaths, MenuPresentationMode,
    PresentationConfig, PresentationMode, TerminalInfo,
};
use sf_legacy::HelpFile;
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::resources::{find_case_insensitive, load_display_layer, load_help_file};

pub const PROFILE_FORMAT_VERSION: u32 = 1;
pub const RESOURCE_API_VERSION: u32 = 1;
pub const PROFILE_DIRECTORY: &str = "presentation-profiles";
pub const PROFILE_DESCRIPTOR: &str = "profile.toml";
pub const MODERN_PROFILE_ID: &str = "modern-ng";
pub const MODERN_PROFILE_VERSION: &str = "1.0.1";
pub const MINIMAL_PROFILE_ID: &str = "minimal-terminal";
pub const MINIMAL_PROFILE_VERSION: &str = "1.0.1";
pub const CLASSIC_PROFILE_ID: &str = "classic-spitfire";
pub const CLASSIC_PROFILE_VERSION: &str = "1.1.1";
const MAX_DESCRIPTOR_BYTES: usize = 256 * 1024;
const MAX_PROFILE_ASSETS: usize = 4096;
const MAX_ASSET_BYTES: usize = 1024 * 1024;
const MAX_REPORTED_ISSUES: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDescriptor {
    pub format_version: u32,
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub resource_api_version: u32,
    pub engine: EngineCompatibility,
    pub compatibility_target: String,
    pub supported_formats: Vec<ProfileFormat>,
    pub fallback_policy: FallbackPolicy,
    pub provenance: Vec<ProvenanceRecord>,
    pub resources: Vec<ProfileResourceRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EngineCompatibility {
    pub minimum: String,
    pub maximum_exclusive: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileFormat {
    Bbs,
    Clr,
    SpitfireHelp,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FallbackPolicy {
    BaseThenBuiltIn,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceRecord {
    pub id: String,
    pub kind: ProvenanceKind,
    pub creator: String,
    pub rightsholder: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    pub license: String,
    pub redistribution: Redistribution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifications: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceKind {
    HistoricalOriginal,
    HistoricalInspired,
    ProjectAuthored,
    ThirdParty,
    Generated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Redistribution {
    Allowed,
    LocalOnly,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileResourceKind {
    Display,
    MenuArtwork,
    Help,
    Prompt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileResourceRecord {
    pub key: String,
    pub kind: ProfileResourceKind,
    pub format: ProfileFormat,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationStatus {
    pub mode: PresentationMode,
    pub configured_active: Option<String>,
    pub active_version: Option<String>,
    pub configured_base: Option<String>,
    pub base_version: Option<String>,
    pub effective_source: String,
    pub degraded: bool,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct PresentationResolver {
    status: PresentationStatus,
    menu_mode: MenuPresentationMode,
    active: Option<LoadedProfile>,
    base: Option<LoadedProfile>,
}

#[derive(Clone, Debug)]
struct LoadedProfile {
    descriptor: ProfileDescriptor,
    displays: BTreeMap<(String, ProfileFormat), Vec<u8>>,
    help: Option<Vec<u8>>,
}

impl PresentationResolver {
    pub fn load(paths: &LogicalPaths, selection: &PresentationConfig) -> Self {
        if selection.mode == PresentationMode::LegacyResources {
            return Self {
                status: PresentationStatus {
                    mode: PresentationMode::LegacyResources,
                    configured_active: None,
                    active_version: None,
                    configured_base: None,
                    base_version: None,
                    effective_source: "legacy SYSTEM/DISPLAY resources".to_owned(),
                    degraded: false,
                    issues: Vec::new(),
                },
                menu_mode: selection.menu_mode,
                active: None,
                base: None,
            };
        }

        let root = paths.get(LogicalPath::System).join(PROFILE_DIRECTORY);
        let active_id = selection.active_profile.clone().unwrap_or_default();
        let base_id = selection.base_profile.clone().unwrap_or_default();
        let mut issues = Vec::new();
        let same_profile = active_id == base_id;
        let active = requested_profile(&root, &active_id, "active", &mut issues);
        let base = if same_profile {
            None
        } else {
            requested_profile(&root, &base_id, "base", &mut issues)
        };
        let effective_source = if active.is_some() {
            format!("active profile {active_id}")
        } else if base.is_some() {
            format!("base profile {base_id}")
        } else {
            "engine built-in fallback".to_owned()
        };
        let status = PresentationStatus {
            mode: PresentationMode::Profile,
            configured_active: Some(active_id),
            active_version: active
                .as_ref()
                .map(|profile| profile.descriptor.version.clone()),
            configured_base: Some(base_id),
            base_version: if same_profile {
                active
                    .as_ref()
                    .map(|profile| profile.descriptor.version.clone())
            } else {
                base.as_ref()
                    .map(|profile| profile.descriptor.version.clone())
            },
            effective_source,
            degraded: !issues.is_empty(),
            issues,
        };
        for issue in &status.issues {
            warn!(
                issue,
                "presentation profile degraded; lower-precedence fallback remains available"
            );
        }
        Self {
            status,
            menu_mode: selection.menu_mode,
            active,
            base,
        }
    }

    pub fn status(&self) -> &PresentationStatus {
        &self.status
    }

    pub(crate) const fn menu_mode(&self) -> MenuPresentationMode {
        self.menu_mode
    }

    pub(crate) fn displays(
        &self,
        board_override: &Path,
        terminal: &TerminalInfo,
    ) -> BTreeMap<String, DisplayResource> {
        if self.status.mode == PresentationMode::LegacyResources {
            let mut displays = load_display_layer(board_override, terminal).unwrap_or_else(|error| {
                warn!(error = %error, "legacy DISPLAY resources unavailable; using built-in fallbacks");
                BTreeMap::new()
            });
            if self.menu_mode == MenuPresentationMode::Generated {
                displays.retain(|key, _| !is_exact_menu_key(key));
            }
            return displays;
        }
        let mut resolved = load_display_layer(board_override, terminal).unwrap_or_else(|error| {
            warn!(error = %error, "board display override layer unavailable; trying configured profiles");
            BTreeMap::new()
        });
        let board_menu_claims = declared_menu_keys(board_override);
        if self.menu_mode == MenuPresentationMode::Generated {
            resolved.retain(|key, _| !is_exact_menu_key(key));
        }
        if let Some(profile) = &self.active {
            for (key, resource) in profile.selected_displays(terminal, DisplaySource::ActiveProfile)
            {
                if self.menu_mode == MenuPresentationMode::Generated && is_exact_menu_key(&key) {
                    continue;
                }
                if is_exact_menu_key(&key) && board_menu_claims.contains(&key) {
                    continue;
                }
                resolved.entry(key).or_insert(resource);
            }
        }
        // Exact-security menu artwork is a leaf override, not a thematic
        // inheritance resource. If the board/active package lacks the exact
        // key, the engine-generated `.MNU` menu must remain reachable.
        if let Some(profile) = &self.base {
            for (key, resource) in profile.selected_displays(terminal, DisplaySource::BaseProfile) {
                if is_exact_menu_key(&key) {
                    continue;
                }
                resolved.entry(key).or_insert(resource);
            }
        }
        resolved
    }

    pub(crate) fn help(&self, system: &Path) -> Vec<sf_core::HelpRecord> {
        if self.status.mode == PresentationMode::LegacyResources {
            return load_help_file(system).unwrap_or_default();
        }
        if find_case_insensitive(system, "SPITFIRE.HLP").is_some() {
            if let Some(help) = load_help_file(system) {
                return help;
            }
        }
        for profile in [&self.active, &self.base].into_iter().flatten() {
            if let Some(bytes) = &profile.help {
                if let Ok(help) = HelpFile::parse(bytes) {
                    return help_records(&help);
                }
            }
        }
        Vec::new()
    }
}

fn declared_menu_keys(directory: &Path) -> BTreeSet<String> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let supported = path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("BBS") || extension.eq_ignore_ascii_case("CLR")
                });
            if !supported {
                return None;
            }
            let key = path.file_stem()?.to_str()?.to_ascii_uppercase();
            is_exact_menu_key(&key).then_some(key)
        })
        .collect()
}

fn is_exact_menu_key(key: &str) -> bool {
    ["MAIN", "MSG", "FILE", "SOP"].iter().any(|prefix| {
        key.strip_prefix(prefix).is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix.len() <= 4
                && suffix.bytes().all(|byte| byte.is_ascii_digit())
                && suffix.parse::<u16>().is_ok_and(|level| level <= 9_999)
        })
    })
}

fn requested_profile(
    root: &Path,
    id: &str,
    role: &str,
    issues: &mut Vec<String>,
) -> Option<LoadedProfile> {
    let result = if valid_id(id) {
        load_profile(root, id)
    } else {
        Err("configured profile ID is invalid".to_owned())
    };
    match result {
        Ok((profile, profile_issues)) => {
            for issue in profile_issues {
                record_issue(issues, issue);
            }
            Some(profile)
        }
        Err(reason) => {
            record_issue(issues, format!("{role} profile {id}: {reason}"));
            None
        }
    }
}

impl LoadedProfile {
    fn selected_displays(
        &self,
        terminal: &TerminalInfo,
        source: DisplaySource,
    ) -> BTreeMap<String, DisplayResource> {
        let mut resources = BTreeMap::new();
        for ((key, format), bytes) in &self.displays {
            if *format == ProfileFormat::Bbs {
                resources.insert(
                    key.clone(),
                    DisplayResource {
                        format: DisplayFormat::Bbs,
                        source,
                        bytes: bytes.clone(),
                    },
                );
            }
        }
        if terminal.capabilities.ansi {
            for ((key, format), bytes) in &self.displays {
                if *format == ProfileFormat::Clr {
                    resources.insert(
                        key.clone(),
                        DisplayResource {
                            format: DisplayFormat::Clr,
                            source,
                            bytes: bytes.clone(),
                        },
                    );
                }
            }
        }
        resources
    }
}

fn load_profile(root: &Path, requested_id: &str) -> Result<(LoadedProfile, Vec<String>), String> {
    reject_symlink(root, "profile collection")?;
    let package = root.join(requested_id);
    reject_symlink(&package, "profile directory")?;
    if !package.is_dir() {
        return Err("package is missing".to_owned());
    }
    let descriptor_path = package.join(PROFILE_DESCRIPTOR);
    reject_symlink(&descriptor_path, "profile descriptor")?;
    let descriptor_bytes = read_bounded(&descriptor_path, MAX_DESCRIPTOR_BYTES)
        .map_err(|reason| format!("descriptor {reason}"))?;
    let descriptor_text =
        std::str::from_utf8(&descriptor_bytes).map_err(|_| "descriptor is not UTF-8".to_owned())?;
    let descriptor: ProfileDescriptor =
        toml::from_str(descriptor_text).map_err(|error| format!("invalid descriptor: {error}"))?;
    validate_descriptor(&descriptor, requested_id)?;

    let canonical_root = root
        .canonicalize()
        .map_err(|_| "profile collection cannot be canonicalized".to_owned())?;
    let canonical_package = package
        .canonicalize()
        .map_err(|_| "package cannot be canonicalized".to_owned())?;
    if !canonical_package.starts_with(&canonical_root) {
        return Err("package resolves outside the profile collection".to_owned());
    }
    validate_package_inventory(&canonical_package, &descriptor)?;
    let provenance_ids = descriptor
        .provenance
        .iter()
        .map(|record| record.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut displays = BTreeMap::new();
    let mut help = None;
    let mut issues = Vec::new();
    for resource in &descriptor.resources {
        let problem = validate_resource_record(resource, &provenance_ids)
            .and_then(|()| load_profile_asset(&canonical_package, resource));
        let bytes = match problem {
            Ok(bytes) => bytes,
            Err(reason) => {
                record_issue(
                    &mut issues,
                    format!(
                        "profile {requested_id} resource {} rejected: {reason}",
                        resource.key
                    ),
                );
                continue;
            }
        };
        match resource.kind {
            ProfileResourceKind::Display | ProfileResourceKind::MenuArtwork => {
                if let Err(reason) =
                    crate::resources::validate_profile_display(resource.format, &bytes)
                {
                    record_issue(
                        &mut issues,
                        format!(
                            "profile {requested_id} resource {} rejected: {reason}",
                            resource.key
                        ),
                    );
                    continue;
                }
                displays.insert((resource.key.to_ascii_uppercase(), resource.format), bytes);
            }
            ProfileResourceKind::Help => match HelpFile::parse(&bytes) {
                Ok(_) => help = Some(bytes),
                Err(_) => record_issue(
                    &mut issues,
                    format!(
                        "profile {requested_id} resource {} rejected: malformed help file",
                        resource.key
                    ),
                ),
            },
            ProfileResourceKind::Prompt => {
                record_issue(&mut issues, format!(
                    "profile {requested_id} resource {} rejected: prompt assets are reserved for a later resource API",
                    resource.key
                ));
            }
        }
    }
    Ok((
        LoadedProfile {
            descriptor,
            displays,
            help,
        },
        issues,
    ))
}

fn record_issue(issues: &mut Vec<String>, issue: String) {
    if issues.len() < MAX_REPORTED_ISSUES {
        issues.push(issue);
    } else if issues.len() == MAX_REPORTED_ISSUES {
        issues.push("additional presentation issues omitted".to_owned());
    }
}

fn validate_descriptor(descriptor: &ProfileDescriptor, requested_id: &str) -> Result<(), String> {
    if descriptor.format_version != PROFILE_FORMAT_VERSION {
        return Err(format!(
            "format version {} is unsupported",
            descriptor.format_version
        ));
    }
    if descriptor.resource_api_version != RESOURCE_API_VERSION {
        return Err(format!(
            "resource API version {} is unsupported",
            descriptor.resource_api_version
        ));
    }
    if descriptor.id != requested_id || !valid_id(&descriptor.id) {
        return Err("descriptor ID is invalid or does not match its directory".to_owned());
    }
    if !valid_semver(&descriptor.version) {
        return Err("profile version is not core MAJOR.MINOR.PATCH SemVer".to_owned());
    }
    for (label, value) in [
        ("display name", descriptor.display_name.as_str()),
        ("description", descriptor.description.as_str()),
        (
            "compatibility target",
            descriptor.compatibility_target.as_str(),
        ),
    ] {
        if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
            return Err(format!("{label} is empty, too long, or contains controls"));
        }
    }
    let minimum = parse_version(&descriptor.engine.minimum)
        .ok_or_else(|| "engine.minimum is not core SemVer".to_owned())?;
    let maximum = parse_version(&descriptor.engine.maximum_exclusive)
        .ok_or_else(|| "engine.maximum_exclusive is not core SemVer".to_owned())?;
    let current = parse_version(sf_core::PRODUCT_VERSION)
        .ok_or_else(|| "running engine version is not core SemVer".to_owned())?;
    if minimum >= maximum || current < minimum || current >= maximum {
        return Err(format!(
            "profile is incompatible with engine {}",
            sf_core::PRODUCT_VERSION
        ));
    }
    if descriptor.supported_formats.is_empty()
        || descriptor.supported_formats.len() > 3
        || descriptor.resources.len() > MAX_PROFILE_ASSETS
    {
        return Err("supported format/resource inventory bounds are invalid".to_owned());
    }
    let format_set = descriptor
        .supported_formats
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if format_set.len() != descriptor.supported_formats.len() {
        return Err("supported formats are duplicated".to_owned());
    }
    let mut provenance = BTreeSet::new();
    for record in &descriptor.provenance {
        if !valid_id(&record.id)
            || !provenance.insert(record.id.as_str())
            || record.source.is_empty()
            || record.source.len() > 512
            || record.creator.is_empty()
            || record.creator.len() > 256
            || record.rightsholder.is_empty()
            || record.rightsholder.len() > 256
            || record.license.is_empty()
            || record.license.len() > 256
            || record.source_hash.as_ref().is_some_and(|hash| {
                hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return Err("provenance metadata is invalid or duplicated".to_owned());
        }
    }
    let mut resources = BTreeSet::new();
    for resource in &descriptor.resources {
        if !format_set.contains(&resource.format) {
            return Err("resource uses a format not declared by the profile".to_owned());
        }
        if !resources.insert((resource.key.to_ascii_uppercase(), resource.format)) {
            return Err("resource inventory contains a duplicate key/format".to_owned());
        }
    }
    Ok(())
}

fn validate_resource_record(
    resource: &ProfileResourceRecord,
    provenance: &BTreeSet<&str>,
) -> Result<(), String> {
    if resource.key.is_empty()
        || resource.key.len() > 64
        || !resource
            .key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("invalid logical resource key".to_owned());
    }
    if !safe_relative_path(&resource.path) {
        return Err("unsafe resource path".to_owned());
    }
    let extension = resource
        .path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let extension_matches = match resource.format {
        ProfileFormat::Bbs => extension.eq_ignore_ascii_case("BBS"),
        ProfileFormat::Clr => extension.eq_ignore_ascii_case("CLR"),
        ProfileFormat::SpitfireHelp => extension.eq_ignore_ascii_case("HLP"),
    };
    if !extension_matches {
        return Err("resource extension does not match its declared format".to_owned());
    }
    if resource.bytes > MAX_ASSET_BYTES as u64 {
        return Err("resource exceeds the one-megabyte limit".to_owned());
    }
    if resource.sha256.len() != 64 || !resource.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("invalid SHA-256 field".to_owned());
    }
    if !provenance.contains(resource.provenance.as_str()) {
        return Err("resource references unknown provenance".to_owned());
    }
    match (resource.kind, resource.format) {
        (
            ProfileResourceKind::Display | ProfileResourceKind::MenuArtwork,
            ProfileFormat::Bbs | ProfileFormat::Clr,
        )
        | (ProfileResourceKind::Help, ProfileFormat::SpitfireHelp) => Ok(()),
        (ProfileResourceKind::Prompt, ProfileFormat::Bbs) => Ok(()),
        _ => Err("resource kind and format are incompatible".to_owned()),
    }
}

fn load_profile_asset(package: &Path, resource: &ProfileResourceRecord) -> Result<Vec<u8>, String> {
    let path = package.join(&resource.path);
    reject_symlink(&path, "resource")?;
    let canonical = path
        .canonicalize()
        .map_err(|_| "resource is missing".to_owned())?;
    if !canonical.starts_with(package) {
        return Err("resource resolves outside its profile".to_owned());
    }
    let bytes = read_bounded(&canonical, MAX_ASSET_BYTES)?;
    if bytes.len() as u64 != resource.bytes {
        return Err("length mismatch".to_owned());
    }
    let actual = hex_digest(&bytes);
    if !actual.eq_ignore_ascii_case(&resource.sha256) {
        return Err("SHA-256 mismatch".to_owned());
    }
    Ok(bytes)
}

fn reject_symlink(path: &Path, description: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(format!("{description} must not be a symbolic link"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(format!("{description} metadata is unavailable")),
    }
}

fn validate_package_inventory(
    package: &Path,
    descriptor: &ProfileDescriptor,
) -> Result<(), String> {
    let declared = descriptor
        .resources
        .iter()
        .map(|resource| resource.path.clone())
        .collect::<BTreeSet<_>>();
    let mut pending = vec![package.to_path_buf()];
    let mut file_count = 0usize;
    let mut case_folded_paths = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        let entries =
            fs::read_dir(&directory).map_err(|_| "package directory cannot be read".to_owned())?;
        let mut paths = entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "package directory entry cannot be read".to_owned())?;
        paths.sort();
        for path in paths {
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| "package entry metadata is unavailable".to_owned())?;
            if metadata.file_type().is_symlink() {
                return Err("package contains a symbolic link".to_owned());
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err("package contains a non-regular object".to_owned());
            }
            file_count += 1;
            if file_count > MAX_PROFILE_ASSETS + 64 {
                return Err("package contains too many files".to_owned());
            }
            let relative = path
                .strip_prefix(package)
                .map_err(|_| "package entry escaped its root".to_owned())?;
            if !case_folded_paths.insert(relative.to_string_lossy().to_ascii_lowercase()) {
                return Err("package contains case-fold-colliding paths".to_owned());
            }
            let allowed_document = relative == Path::new(PROFILE_DESCRIPTOR)
                || relative == Path::new("README.md")
                || relative == Path::new("GENERATED-RESOURCES.txt")
                || relative.starts_with("LICENSES");
            if !declared.contains(relative) && !allowed_document {
                return Err(format!(
                    "package contains unlisted file {}",
                    relative.to_string_lossy()
                ));
            }
        }
    }
    Ok(())
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|_| "is missing or unreadable".to_owned())?;
    if !metadata.is_file() {
        return Err("is not a regular file".to_owned());
    }
    if metadata.len() > maximum as u64 {
        return Err(format!("exceeds {maximum} bytes"));
    }
    fs::read(path).map_err(|_| "cannot be read".to_owned())
}

fn safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.as_os_str().len() <= 512
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn valid_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn valid_semver(value: &str) -> bool {
    parse_version(value).is_some()
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut fields = value.split('.');
    let major = parse_version_field(fields.next()?)?;
    let minor = parse_version_field(fields.next()?)?;
    let patch = parse_version_field(fields.next()?)?;
    if fields.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn parse_version_field(value: &str) -> Option<u64> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return None;
    }
    value.parse().ok()
}

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn help_records(help: &HelpFile) -> Vec<sf_core::HelpRecord> {
    help.records()
        .iter()
        .map(|record| sf_core::HelpRecord {
            lines: std::array::from_fn(|index| record.line(index).unwrap_or_default().to_vec()),
        })
        .collect()
}

pub fn profile_root(paths: &LogicalPaths, id: &str) -> PathBuf {
    paths
        .get(LogicalPath::System)
        .join(PROFILE_DIRECTORY)
        .join(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::initialize_fixture_board;
    use crate::resources::load_stock_resources;
    use sf_core::{MenuSection, RuntimeConfig, TerminalCapabilities, TerminalInfo};

    fn board() -> (tempfile::TempDir, PathBuf, LogicalPaths, PresentationConfig) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        initialize_fixture_board(&root).unwrap();
        let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        (temp, root, paths, validated.presentation)
    }

    fn descriptor_path(root: &Path) -> PathBuf {
        root.join("system/presentation-profiles/modern-ng/profile.toml")
    }

    fn minimal_descriptor_path(root: &Path) -> PathBuf {
        root.join("system/presentation-profiles/minimal-terminal/profile.toml")
    }

    fn classic_descriptor_path(root: &Path) -> PathBuf {
        root.join("system/presentation-profiles/classic-spitfire/profile.toml")
    }

    fn minimal_selection() -> PresentationConfig {
        PresentationConfig {
            mode: PresentationMode::Profile,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: Some(MINIMAL_PROFILE_ID.to_owned()),
            base_profile: Some(MODERN_PROFILE_ID.to_owned()),
        }
    }

    fn classic_selection() -> PresentationConfig {
        PresentationConfig {
            mode: PresentationMode::Profile,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: Some(CLASSIC_PROFILE_ID.to_owned()),
            base_profile: Some(MODERN_PROFILE_ID.to_owned()),
        }
    }

    fn rewrite_descriptor(root: &Path, change: impl FnOnce(&mut ProfileDescriptor)) {
        let path = descriptor_path(root);
        let input = fs::read_to_string(&path).unwrap();
        let mut descriptor: ProfileDescriptor = toml::from_str(&input).unwrap();
        change(&mut descriptor);
        fs::write(path, toml::to_string_pretty(&descriptor).unwrap()).unwrap();
    }

    #[test]
    fn valid_modern_descriptor_loads_and_preserves_ansi_and_text_bytes() {
        let (_temp, _root, paths, selection) = board();
        let resolver = PresentationResolver::load(&paths, &selection);
        assert!(!resolver.status().degraded);
        assert_eq!(
            resolver.status().active_version.as_deref(),
            Some(MODERN_PROFILE_VERSION)
        );
        assert_eq!(
            resolver.status().effective_source,
            "active profile modern-ng"
        );

        let ansi = load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        assert_eq!(ansi.welcome.format, DisplayFormat::Clr);
        assert_eq!(ansi.welcome.source, DisplaySource::ActiveProfile);
        assert_eq!(
            ansi.menu_display(MenuSection::Main, 10).unwrap().source,
            DisplaySource::ActiveProfile
        );
        assert_eq!(
            ansi.welcome.bytes,
            b"@CLS@\x1B[1;36mWelcome to @BOARD@\x1B[0m\r\n"
        );
        let mut text = TerminalInfo::in_memory();
        text.capabilities = TerminalCapabilities {
            ansi: false,
            ..text.capabilities
        };
        let text = load_stock_resources(&paths, &text, &resolver).unwrap();
        assert_eq!(text.welcome.format, DisplayFormat::Bbs);
        assert_eq!(
            text.welcome.bytes,
            b"Welcome to @BOARD@\r\nPlease identify yourself to enter the board.\r\n"
        );
        assert!(text.help_record(21).is_some());
    }

    #[test]
    fn minimal_profile_is_plain_text_for_ansi_and_text_callers() {
        let (_temp, root, paths, _) = board();
        let resolver = PresentationResolver::load(&paths, &minimal_selection());
        assert!(!resolver.status().degraded);
        assert_eq!(
            resolver.status().active_version.as_deref(),
            Some(MINIMAL_PROFILE_VERSION)
        );
        assert_eq!(
            resolver.status().effective_source,
            "active profile minimal-terminal"
        );

        let ansi = load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        assert_eq!(ansi.welcome.format, DisplayFormat::Bbs);
        assert!(ansi.welcome.bytes.starts_with(b"Welcome to @BOARD@"));
        assert!(ansi
            .menu_display(MenuSection::Main, 10)
            .unwrap()
            .bytes
            .starts_with(b"\r\nSPITFIRE NG - MAIN MENU"));
        let mut text_terminal = TerminalInfo::in_memory();
        text_terminal.capabilities.ansi = false;
        let text = load_stock_resources(&paths, &text_terminal, &resolver).unwrap();
        assert_eq!(text.welcome, ansi.welcome);
        assert_eq!(text.displays, ansi.displays);
        assert_ne!(text.help_record(21).unwrap().lines[0], b"Message Section");

        let descriptor: ProfileDescriptor =
            toml::from_str(&fs::read_to_string(minimal_descriptor_path(&root)).unwrap()).unwrap();
        assert_eq!(
            descriptor.supported_formats,
            vec![ProfileFormat::Bbs, ProfileFormat::SpitfireHelp]
        );
        assert!(descriptor
            .provenance
            .iter()
            .all(|record| record.redistribution == Redistribution::Allowed));
        for resource in descriptor
            .resources
            .iter()
            .filter(|resource| resource.format == ProfileFormat::Bbs)
        {
            let bytes = fs::read(
                minimal_descriptor_path(&root)
                    .parent()
                    .unwrap()
                    .join(&resource.path),
            )
            .unwrap();
            assert!(
                !bytes.contains(&0x1b),
                "{} contains ESC",
                resource.path.display()
            );
            assert!(bytes.is_ascii(), "{} is not ASCII", resource.path.display());
            assert!(bytes.split(|byte| *byte == b'\n').all(|line| line
                .strip_suffix(b"\r")
                .unwrap_or(line)
                .len()
                <= 48));
        }
    }

    #[test]
    fn minimal_profile_changes_presentation_but_not_menu_authority() {
        let (_temp, _root, paths, modern_selection) = board();
        let modern_resolver = PresentationResolver::load(&paths, &modern_selection);
        let minimal_resolver = PresentationResolver::load(&paths, &minimal_selection());
        let terminal = TerminalInfo::in_memory();
        let modern = load_stock_resources(&paths, &terminal, &modern_resolver).unwrap();
        let minimal = load_stock_resources(&paths, &terminal, &minimal_resolver).unwrap();

        assert_ne!(modern.welcome.bytes, minimal.welcome.bytes);
        assert_ne!(
            modern.menu_display(MenuSection::Main, 10).unwrap().bytes,
            minimal.menu_display(MenuSection::Main, 10).unwrap().bytes
        );
        assert_eq!(modern.menus, minimal.menus);
    }

    #[test]
    fn exact_menu_art_is_active_or_board_local_and_never_inherited_from_base() {
        let (_temp, root, paths, _) = board();
        let descriptor_path = classic_descriptor_path(&root);
        let mut descriptor: ProfileDescriptor =
            toml::from_str(&fs::read_to_string(&descriptor_path).unwrap()).unwrap();
        let removed = descriptor
            .resources
            .iter()
            .filter(|record| record.key == "MAIN10")
            .map(|record| record.path.clone())
            .collect::<Vec<_>>();
        descriptor.resources.retain(|record| record.key != "MAIN10");
        fs::write(
            &descriptor_path,
            toml::to_string_pretty(&descriptor).unwrap(),
        )
        .unwrap();
        for path in removed {
            fs::remove_file(descriptor_path.parent().unwrap().join(path)).unwrap();
        }

        let resolver = PresentationResolver::load(&paths, &classic_selection());
        assert!(!resolver.status().degraded);
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        assert!(resources.menu_display(MenuSection::Main, 10).is_none());

        fs::write(root.join("display/MAIN10.BBS"), b"Board exact menu\r\n").unwrap();
        let resolver = PresentationResolver::load(&paths, &classic_selection());
        let mut text = TerminalInfo::in_memory();
        text.capabilities.ansi = false;
        let resources = load_stock_resources(&paths, &text, &resolver).unwrap();
        assert_eq!(
            resources.menu_display(MenuSection::Main, 10).unwrap().bytes,
            b"Board exact menu\r\n"
        );
        assert_eq!(
            resources
                .menu_display(MenuSection::Main, 10)
                .unwrap()
                .source,
            DisplaySource::BoardOverride
        );
        assert!(resources.menu_display(MenuSection::Main, 999).is_none());
    }

    #[test]
    fn generated_menu_mode_ignores_exact_art_without_changing_other_resources() {
        let (_temp, _root, paths, _) = board();
        let mut selection = classic_selection();
        selection.menu_mode = MenuPresentationMode::Generated;
        let resolver = PresentationResolver::load(&paths, &selection);
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        assert!(resources.menu_display(MenuSection::Main, 10).is_none());
        assert!(resources.menu_display(MenuSection::Sysop, 50).is_none());
        assert_eq!(resources.welcome.format, DisplayFormat::Clr);
        assert!(resources
            .menu(MenuSection::Main)
            .unwrap()
            .find(b'@', 10)
            .is_none());
        assert!(resources
            .menu(MenuSection::Main)
            .unwrap()
            .find(b'@', 50)
            .is_some());
    }

    #[test]
    fn malformed_or_terminal_unsupported_exact_board_override_uses_generated_menu() {
        let (_temp, root, paths, _) = board();
        fs::write(root.join("display/MAIN10.CLR"), b"broken\x1b[").unwrap();
        let resolver = PresentationResolver::load(&paths, &classic_selection());
        let ansi = load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        assert!(ansi.menu_display(MenuSection::Main, 10).is_none());

        fs::write(
            root.join("display/MAIN10.CLR"),
            b"\x1b[36mANSI-only exact menu\x1b[0m\r\n",
        )
        .unwrap();
        let resolver = PresentationResolver::load(&paths, &classic_selection());
        let mut text = TerminalInfo::in_memory();
        text.capabilities.ansi = false;
        let text = load_stock_resources(&paths, &text, &resolver).unwrap();
        assert!(text.menu_display(MenuSection::Main, 10).is_none());
    }

    #[test]
    fn classic_profile_selects_clr_or_bbs_without_changing_menu_authority() {
        let (_temp, root, paths, modern_selection) = board();
        let modern_resolver = PresentationResolver::load(&paths, &modern_selection);
        let classic_resolver = PresentationResolver::load(&paths, &classic_selection());
        assert!(!classic_resolver.status().degraded);
        assert_eq!(
            classic_resolver.status().effective_source,
            "active profile classic-spitfire"
        );

        let terminal = TerminalInfo::in_memory();
        let modern = load_stock_resources(&paths, &terminal, &modern_resolver).unwrap();
        let classic = load_stock_resources(&paths, &terminal, &classic_resolver).unwrap();
        assert_eq!(classic.welcome.format, DisplayFormat::Clr);
        assert!(classic.welcome.bytes.contains(&0x1b));
        assert!(classic.welcome.bytes.iter().any(|byte| *byte >= 0x80));
        assert_eq!(modern.menus, classic.menus);

        let mut text_terminal = TerminalInfo::in_memory();
        text_terminal.capabilities.ansi = false;
        let text = load_stock_resources(&paths, &text_terminal, &classic_resolver).unwrap();
        assert_eq!(text.welcome.format, DisplayFormat::Bbs);
        assert!(!text.welcome.bytes.contains(&0x1b));
        assert!(text.welcome.bytes.iter().any(|byte| *byte >= 0x80));
        assert!(text.help_record(21).is_some());

        let descriptor: ProfileDescriptor =
            toml::from_str(&fs::read_to_string(classic_descriptor_path(&root)).unwrap()).unwrap();
        for resource in descriptor
            .resources
            .iter()
            .filter(|resource| resource.format == ProfileFormat::Bbs)
        {
            let bytes = fs::read(
                classic_descriptor_path(&root)
                    .parent()
                    .unwrap()
                    .join(&resource.path),
            )
            .unwrap();
            assert!(
                !bytes.contains(&0x1b),
                "{} contains ESC",
                resource.path.display()
            );
            assert!(bytes.split(|byte| *byte == b'\n').all(|line| {
                let line = line.strip_suffix(b"\r").unwrap_or(line);
                let line = line.strip_prefix(b"@PROMPTOFF@").unwrap_or(line);
                let line = line.strip_prefix(b"@CLS@").unwrap_or(line);
                line.len() <= 80
            }));
        }
    }

    #[test]
    fn classic_menu_art_matches_setup_command_letters() {
        let (_temp, _root, paths, _) = board();
        let resolver = PresentationResolver::load(&paths, &classic_selection());
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        for (section, security, expected) in [
            (MenuSection::Main, 10, b"MCFPYRUAXG?".as_slice()),
            (MenuSection::Main, 50, b"MCFPYRU A@XG?".as_slice()),
            (MenuSection::Message, 10, b"CRBEYAFQXG?".as_slice()),
            (MenuSection::Message, 50, b"CRBEYAFQ@XG?".as_slice()),
            (MenuSection::File, 10, b"CLDUNTFMQXG?".as_slice()),
            (MenuSection::File, 50, b"CLDUNTFMQ@XG?".as_slice()),
            (MenuSection::Sysop, 50, b"QXG".as_slice()),
        ] {
            let authority = resources
                .menu(section)
                .unwrap()
                .items
                .iter()
                .filter(|item| security >= item.required_security)
                .map(|item| item.command)
                .collect::<Vec<_>>();
            let expected = expected
                .iter()
                .copied()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect::<Vec<_>>();
            assert_eq!(authority, expected, "{section:?} security {security}");
            let art = &resources.menu_display(section, security).unwrap().bytes;
            for key in expected {
                assert!(
                    art.windows(3).any(|window| window == [b'<', key, b'>']),
                    "{section:?} security {security} omitted {}",
                    char::from(key)
                );
            }
        }
    }

    #[test]
    fn classic_fidelity_uses_distinct_section_grammar_without_engine_prose() {
        let (_temp, root, paths, _) = board();
        let resolver = PresentationResolver::load(&paths, &classic_selection());
        assert_eq!(
            resolver.status().active_version.as_deref(),
            Some(CLASSIC_PROFILE_VERSION)
        );
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();

        let main_caller = &resources.menu_display(MenuSection::Main, 10).unwrap().bytes;
        let main_sysop = &resources.menu_display(MenuSection::Main, 50).unwrap().bytes;
        let messages = &resources
            .menu_display(MenuSection::Message, 10)
            .unwrap()
            .bytes;
        let files = &resources.menu_display(MenuSection::File, 10).unwrap().bytes;
        let sysop = &resources
            .menu_display(MenuSection::Sysop, 50)
            .unwrap()
            .bytes;

        assert!(main_caller
            .windows(10)
            .any(|value| value == b"\x1B[1;37;45m"));
        assert!(main_sysop
            .windows(10)
            .any(|value| value == b"\x1B[1;37;44m"));
        assert!(messages.windows(10).any(|value| value == b"\x1B[1;37;41m"));
        assert!(files.windows(10).any(|value| value == b"\x1B[1;37;44m"));
        assert!(sysop.windows(10).any(|value| value == b"\x1B[1;30;42m"));
        assert_ne!(main_caller, main_sysop);

        assert!(!resources
            .welcome
            .bytes
            .windows(13)
            .any(|value| value == b"Modern engine"));
        assert!(!sysop.windows(17).any(|value| value == b"Command authority"));
        assert!(resources
            .goodbye
            .bytes
            .windows(21)
            .any(|value| value == b"THANK YOU FOR CALLING"));

        let descriptor: ProfileDescriptor =
            toml::from_str(&fs::read_to_string(classic_descriptor_path(&root)).unwrap()).unwrap();
        assert_eq!(descriptor.version, CLASSIC_PROFILE_VERSION);
        assert!(descriptor.provenance.iter().all(|record| {
            record.redistribution == Redistribution::Allowed
                && record.source_hash.is_none()
                && record.modifications.is_some()
        }));
        assert!(descriptor.provenance.iter().any(|record| {
            record.evidence.as_deref() == Some("docs/research/m036-classic-fidelity-review.md")
        }));
    }

    #[test]
    fn classic_missing_clr_falls_back_to_classic_bbs_then_modern() {
        let (_temp, root, paths, _) = board();
        let classic = root.join("system/presentation-profiles/classic-spitfire");
        fs::remove_file(classic.join("resources/display/WELCOME1.CLR")).unwrap();
        let resolver = PresentationResolver::load(&paths, &classic_selection());
        assert!(resolver.status().degraded);
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        assert_eq!(resources.welcome.format, DisplayFormat::Bbs);
        assert!(resources.welcome.bytes.iter().any(|byte| *byte >= 0x80));

        fs::remove_file(classic.join("resources/display/WELCOME1.BBS")).unwrap();
        let resolver = PresentationResolver::load(&paths, &classic_selection());
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        assert_eq!(resources.welcome.format, DisplayFormat::Clr);
        assert!(resources
            .welcome
            .bytes
            .starts_with(b"@CLS@\x1B[1;36mWelcome"));
    }

    #[test]
    fn invalid_minimal_profile_falls_back_to_modern_by_capability() {
        let (_temp, root, paths, _) = board();
        fs::remove_file(
            root.join(
                "system/presentation-profiles/minimal-terminal/resources/display/WELCOME1.BBS",
            ),
        )
        .unwrap();
        let resolver = PresentationResolver::load(&paths, &minimal_selection());
        assert!(resolver.status().degraded);
        assert_eq!(
            resolver.status().effective_source,
            "active profile minimal-terminal"
        );

        let ansi = load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
        assert_eq!(ansi.welcome.format, DisplayFormat::Clr);
        let mut terminal = TerminalInfo::in_memory();
        terminal.capabilities.ansi = false;
        let text = load_stock_resources(&paths, &terminal, &resolver).unwrap();
        assert_eq!(text.welcome.format, DisplayFormat::Bbs);
        assert!(text.welcome.bytes.starts_with(b"Welcome to @BOARD@"));
    }

    #[test]
    fn strict_descriptor_and_engine_compatibility_fail_to_built_in() {
        for incompatible in [false, true] {
            let (_temp, root, paths, selection) = board();
            if incompatible {
                rewrite_descriptor(&root, |descriptor| {
                    descriptor.engine.maximum_exclusive = "0.1.0".to_owned();
                });
            } else {
                let path = descriptor_path(&root);
                let mut input = fs::read_to_string(&path).unwrap();
                input.push_str("\nunknown_field = true\n");
                fs::write(path, input).unwrap();
            }
            let resolver = PresentationResolver::load(&paths, &selection);
            assert!(resolver.status().degraded);
            assert_eq!(
                resolver.status().effective_source,
                "engine built-in fallback"
            );
            let resources =
                load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
            assert_eq!(resources.welcome.format, DisplayFormat::Bbs);
            assert!(resources.welcome.bytes.starts_with(b"Welcome to @BOARD@"));
        }
    }

    #[test]
    fn missing_hash_mismatched_and_malformed_assets_degrade_without_crashing() {
        for failure in ["missing", "hash", "malformed"] {
            let (_temp, root, paths, selection) = board();
            let asset =
                root.join("system/presentation-profiles/modern-ng/resources/display/WELCOME1.CLR");
            match failure {
                "missing" => fs::remove_file(&asset).unwrap(),
                "hash" => fs::write(&asset, b"changed").unwrap(),
                "malformed" => {
                    let bytes = b"broken\x1b[";
                    fs::write(&asset, bytes).unwrap();
                    rewrite_descriptor(&root, |descriptor| {
                        let record = descriptor
                            .resources
                            .iter_mut()
                            .find(|record| {
                                record.key == "WELCOME1" && record.format == ProfileFormat::Clr
                            })
                            .unwrap();
                        record.bytes = bytes.len() as u64;
                        record.sha256 = hex_digest(bytes);
                    });
                }
                _ => unreachable!(),
            }
            let resolver = PresentationResolver::load(&paths, &selection);
            assert!(resolver.status().degraded);
            let resources =
                load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver).unwrap();
            assert_eq!(resources.welcome.format, DisplayFormat::Bbs);
            assert_eq!(
                resources.welcome.bytes,
                b"Welcome to @BOARD@\r\nPlease identify yourself to enter the board.\r\n"
            );
        }
    }

    #[test]
    fn board_override_then_active_then_base_then_built_in_is_deterministic() {
        let (_temp, root, paths, selection) = board();
        fs::write(root.join("display/WELCOME1.BBS"), b"Board override\r\n").unwrap();
        let resolver = PresentationResolver::load(&paths, &selection);
        let mut text = TerminalInfo::in_memory();
        text.capabilities.ansi = false;
        let resources = load_stock_resources(&paths, &text, &resolver).unwrap();
        assert_eq!(resources.welcome.bytes, b"Board override\r\n");

        let fallback = PresentationConfig {
            mode: PresentationMode::Profile,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: Some("missing-active".to_owned()),
            base_profile: Some(MODERN_PROFILE_ID.to_owned()),
        };
        let resolver = PresentationResolver::load(&paths, &fallback);
        assert!(resolver.status().degraded);
        assert_eq!(resolver.status().effective_source, "base profile modern-ng");

        let built_in = PresentationConfig {
            mode: PresentationMode::Profile,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: Some("missing-active".to_owned()),
            base_profile: Some("missing-base".to_owned()),
        };
        fs::remove_file(root.join("display/WELCOME1.BBS")).unwrap();
        let resolver = PresentationResolver::load(&paths, &built_in);
        assert_eq!(
            resolver.status().effective_source,
            "engine built-in fallback"
        );
        let resources = load_stock_resources(&paths, &text, &resolver).unwrap();
        assert!(resources.welcome.bytes.starts_with(b"Welcome to @BOARD@"));
    }

    #[test]
    fn unsafe_paths_unlisted_files_and_symlinks_reject_the_profile() {
        let (_temp, _root, paths, _selection) = board();
        let invalid_selection = PresentationConfig {
            mode: PresentationMode::Profile,
            menu_mode: MenuPresentationMode::DisplayOverrides,
            active_profile: Some("../outside".to_owned()),
            base_profile: Some("../outside".to_owned()),
        };
        let resolver = PresentationResolver::load(&paths, &invalid_selection);
        assert!(resolver.status().degraded);
        assert_eq!(
            resolver.status().effective_source,
            "engine built-in fallback"
        );

        for failure in ["traversal", "unlisted"] {
            let (_temp, root, paths, selection) = board();
            if failure == "traversal" {
                rewrite_descriptor(&root, |descriptor| {
                    descriptor.resources[0].path = PathBuf::from("../escape.BBS");
                });
            } else {
                fs::write(
                    root.join("system/presentation-profiles/modern-ng/resources/unlisted.bin"),
                    b"no",
                )
                .unwrap();
            }
            let resolver = PresentationResolver::load(&paths, &selection);
            assert!(resolver.status().degraded);
            assert_eq!(
                resolver.status().effective_source,
                "engine built-in fallback"
            );
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let (_temp, root, paths, selection) = board();
            let asset =
                root.join("system/presentation-profiles/modern-ng/resources/display/WELCOME1.CLR");
            fs::remove_file(&asset).unwrap();
            symlink("WELCOME1.BBS", &asset).unwrap();
            let resolver = PresentationResolver::load(&paths, &selection);
            assert!(resolver.status().degraded);
            assert_eq!(
                resolver.status().effective_source,
                "engine built-in fallback"
            );
        }
    }

    #[test]
    fn omitted_presentation_configuration_retains_legacy_resource_mode() {
        let (_temp, root, paths, _selection) = board();
        fs::write(root.join("display/WELCOME1.BBS"), b"Legacy display\r\n").unwrap();
        let resolver = PresentationResolver::load(&paths, &PresentationConfig::default());
        assert_eq!(resolver.status().mode, PresentationMode::LegacyResources);
        let mut text = TerminalInfo::in_memory();
        text.capabilities.ansi = false;
        let resources = load_stock_resources(&paths, &text, &resolver).unwrap();
        assert_eq!(resources.welcome.bytes, b"Legacy display\r\n");
    }
}
