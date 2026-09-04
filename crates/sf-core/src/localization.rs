// SPITFIRE NG
// Preservation-driven modern cross-platform reimplementation of
// Buffalo Creek Software's SPITFIRE Bulletin Board System
//
// Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
// Licensed under MIT OR Apache-2.0
//
// This file is part of the SPITFIRE NG project.
// See the repository documentation for architecture, provenance,
// compatibility research, security, and contribution guidelines.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};

use chrono::{Datelike, Timelike, Utc};
use chrono_tz::Tz;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use fluent_syntax::ast;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use unic_langid::LanguageIdentifier;

use crate::{LogicalPath, LogicalPaths, TerminalInfo, PRODUCT_VERSION};

pub const LANGUAGE_DIRECTORY: &str = "language-packs";
pub const LANGUAGE_MANIFEST: &str = "language.toml";
pub const LANGUAGE_FORMAT_VERSION: u32 = 1;
pub const CATALOG_API_VERSION: u32 = 1;
pub const EMBEDDED_LOCALE: &str = "en-US";
pub const EMBEDDED_PACKAGE_VERSION: &str = "1.12.0";

const MAX_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_LANGUAGE_FILES: usize = 64;
const MAX_LANGUAGE_FILE_BYTES: usize = 512 * 1024;
const MAX_LANGUAGE_TOTAL_BYTES: usize = 2 * 1024 * 1024;
const MAX_REPORTED_ISSUES: usize = 32;

const SHARED_FTL: &str = include_str!("../i18n/en-US/messages/shared.ftl");
const CALLER_FTL: &str = include_str!("../i18n/en-US/messages/caller.ftl");
const OPERATOR_FTL: &str = include_str!("../i18n/en-US/messages/operator.ftl");
const PACKAGE_README: &str = "# SPITFIRE NG en-US language package\n\nCanonical project-authored English engine catalog. Presentation profile assets remain separate and are not translated by this package.\n";
const PACKAGE_LICENSE: &str = "SPDX-License-Identifier: MIT OR Apache-2.0\n\nCopyright (C) 2026 Craig Daters and SPITFIRE NG contributors.\nThis project-authored language package is available under the SPITFIRE NG repository's LICENSE-MIT or LICENSE-APACHE terms, at your option.\nNo Buffalo Creek Software resource bytes are included or relicensed.\n";

type ConcurrentBundle = FluentBundle<FluentResource>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageManifest {
    pub format_version: u32,
    pub locale: String,
    pub package_version: String,
    pub catalog_api_version: u32,
    pub engine: LanguageEngineCompatibility,
    pub fallback_locale: String,
    pub supported_terminal_encodings: Vec<TerminalTextEncoding>,
    pub provenance: Vec<LanguageProvenance>,
    pub files: Vec<LanguageFileRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageEngineCompatibility {
    pub minimum: String,
    pub maximum_exclusive: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalTextEncoding {
    Utf8,
    Cp437,
    Ascii,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageFileKind {
    Fluent,
    Documentation,
    License,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageProvenance {
    pub id: String,
    pub creator: String,
    pub rightsholder: String,
    pub source: String,
    pub classification: String,
    pub license: String,
    pub redistribution: String,
    pub modifications: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageFileRecord {
    pub path: PathBuf,
    pub kind: LanguageFileKind,
    pub bytes: u64,
    pub sha256: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageStatus {
    pub default_locale: String,
    pub effective_locale: String,
    pub package_version: String,
    pub degraded: bool,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedLanguagePackage {
    pub locale: String,
    pub package_version: String,
}

#[derive(Clone)]
pub struct Localizer {
    requested_locale: String,
    effective_locale: String,
    bundles: Vec<Arc<ConcurrentBundle>>,
    supported_terminal_encodings: BTreeSet<TerminalTextEncoding>,
    pseudo: bool,
}

impl std::fmt::Debug for Localizer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Localizer")
            .field("requested_locale", &self.requested_locale)
            .field("effective_locale", &self.effective_locale)
            .field(
                "supported_terminal_encodings",
                &self.supported_terminal_encodings,
            )
            .field("pseudo", &self.pseudo)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub enum LocalizationValue {
    Text(String),
    Number(i64),
    Unsigned(u64),
    Timestamp { unix_seconds: i64, timezone: Tz },
}

impl From<&str> for LocalizationValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for LocalizationValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i64> for LocalizationValue {
    fn from(value: i64) -> Self {
        Self::Number(value)
    }
}

impl From<u64> for LocalizationValue {
    fn from(value: u64) -> Self {
        Self::Unsigned(value)
    }
}

impl From<u32> for LocalizationValue {
    fn from(value: u32) -> Self {
        Self::Unsigned(u64::from(value))
    }
}

impl From<u16> for LocalizationValue {
    fn from(value: u16) -> Self {
        Self::Unsigned(u64::from(value))
    }
}

#[derive(Clone, Debug, Default)]
pub struct LocalizationArgs(BTreeMap<&'static str, LocalizationValue>);

impl LocalizationArgs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, name: &'static str, value: impl Into<LocalizationValue>) -> Self {
        self.0.insert(name, value.into());
        self
    }

    pub fn timestamp(mut self, name: &'static str, unix_seconds: i64, timezone: Tz) -> Self {
        self.0.insert(
            name,
            LocalizationValue::Timestamp {
                unix_seconds,
                timezone,
            },
        );
        self
    }
}

impl Localizer {
    pub fn embedded_en_us() -> Arc<Self> {
        static EMBEDDED: OnceLock<Arc<Localizer>> = OnceLock::new();
        EMBEDDED
            .get_or_init(|| {
                let locale = normalize_locale(EMBEDDED_LOCALE)
                    .expect("the embedded locale identifier is valid");
                let bundle = build_bundle(&locale, [SHARED_FTL, CALLER_FTL, OPERATOR_FTL])
                    .expect("the embedded en-US Fluent catalog is valid");
                Arc::new(Self {
                    requested_locale: EMBEDDED_LOCALE.to_owned(),
                    effective_locale: EMBEDDED_LOCALE.to_owned(),
                    bundles: vec![Arc::new(bundle)],
                    supported_terminal_encodings: [
                        TerminalTextEncoding::Utf8,
                        TerminalTextEncoding::Cp437,
                        TerminalTextEncoding::Ascii,
                    ]
                    .into_iter()
                    .collect(),
                    pseudo: false,
                })
            })
            .clone()
    }

    pub fn pseudo_for_tests() -> Arc<Self> {
        let embedded = Self::embedded_en_us();
        Arc::new(Self {
            requested_locale: "en-XA".to_owned(),
            effective_locale: "en-XA".to_owned(),
            bundles: embedded.bundles.clone(),
            supported_terminal_encodings: embedded.supported_terminal_encodings.clone(),
            pseudo: true,
        })
    }

    pub fn requested_locale(&self) -> &str {
        &self.requested_locale
    }

    pub fn effective_locale(&self) -> &str {
        &self.effective_locale
    }

    pub fn text(&self, key: &str, arguments: &LocalizationArgs) -> String {
        let rendered = self
            .format_from_bundles(key, arguments, &self.bundles)
            .or_else(|| {
                let embedded = Self::embedded_en_us();
                self.format_from_bundles(key, arguments, &embedded.bundles)
            })
            .unwrap_or_else(|| emergency_ascii(key).to_owned());
        if self.pseudo {
            pseudo_expand(&rendered)
        } else {
            rendered
        }
    }

    pub fn bytes_for_terminal(
        &self,
        terminal: &TerminalInfo,
        key: &str,
        arguments: &LocalizationArgs,
    ) -> Vec<u8> {
        let encoding = terminal_text_encoding(terminal);
        let rendered = if self.supported_terminal_encodings.contains(&encoding) {
            self.text(key, arguments)
        } else {
            Self::embedded_en_us().text(key, arguments)
        };
        encode_text(&rendered, encoding).unwrap_or_else(|| {
            let english = Self::embedded_en_us().text(key, arguments);
            encode_text(&english, encoding)
                .unwrap_or_else(|| emergency_ascii(key).as_bytes().to_vec())
        })
    }

    fn format_from_bundles(
        &self,
        key: &str,
        arguments: &LocalizationArgs,
        bundles: &[Arc<ConcurrentBundle>],
    ) -> Option<String> {
        for bundle in bundles {
            let Some(message) = bundle.get_message(key) else {
                continue;
            };
            let Some(pattern) = message.value() else {
                continue;
            };
            let owned = arguments
                .0
                .iter()
                .map(|(name, value)| (*name, self.format_value(value)))
                .collect::<Vec<_>>();
            let mut fluent_args = FluentArgs::new();
            for (name, value) in &owned {
                match value {
                    FormattedValue::Text(value) => {
                        fluent_args.set(*name, FluentValue::from(value.as_str()));
                    }
                    FormattedValue::Number(value) => {
                        fluent_args.set(*name, FluentValue::from(*value));
                    }
                }
            }
            let mut errors = Vec::new();
            let rendered = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
            if errors.is_empty() {
                return Some(rendered.into_owned());
            }
        }
        None
    }

    fn format_value(&self, value: &LocalizationValue) -> FormattedValue {
        match value {
            LocalizationValue::Text(value) => FormattedValue::Text(value.clone()),
            LocalizationValue::Number(value) => FormattedValue::Number(*value),
            LocalizationValue::Unsigned(value) => {
                FormattedValue::Number(i64::try_from(*value).unwrap_or(i64::MAX))
            }
            LocalizationValue::Timestamp {
                unix_seconds,
                timezone,
            } => FormattedValue::Text(format_local_timestamp(
                *unix_seconds,
                *timezone,
                &self.effective_locale,
            )),
        }
    }
}

enum FormattedValue {
    Text(String),
    Number(i64),
}

#[derive(Clone, Debug)]
pub struct LanguageResolver {
    localizer: Arc<Localizer>,
    status: LanguageStatus,
}

impl LanguageResolver {
    pub fn load(paths: &LogicalPaths, default_locale: &str) -> Self {
        Self::load_for_locale(paths, default_locale, default_locale)
    }

    pub fn load_for_locale(paths: &LogicalPaths, requested: &str, board_default: &str) -> Self {
        let mut issues = Vec::new();
        let requested_id = match normalize_locale(requested) {
            Ok(locale) => locale,
            Err(error) => {
                push_issue(&mut issues, format!("requested locale is invalid: {error}"));
                normalize_locale(EMBEDDED_LOCALE).expect("embedded locale is valid")
            }
        };
        let board_id = match normalize_locale(board_default) {
            Ok(locale) => locale,
            Err(error) => {
                push_issue(&mut issues, format!("board locale is invalid: {error}"));
                normalize_locale(EMBEDDED_LOCALE).expect("embedded locale is valid")
            }
        };
        let mut candidates = Vec::new();
        add_locale_candidate(&mut candidates, requested_id.clone());
        if let Some(parent) = parent_locale(&requested_id) {
            add_locale_candidate(&mut candidates, parent);
        }
        add_locale_candidate(&mut candidates, board_id.clone());
        if let Some(parent) = parent_locale(&board_id) {
            add_locale_candidate(&mut candidates, parent);
        }

        let root = paths.get(LogicalPath::System).join(LANGUAGE_DIRECTORY);
        let mut loaded = Vec::new();
        let mut required_fallbacks = BTreeSet::new();
        let mut index = 0;
        while index < candidates.len() {
            let locale = candidates[index].clone();
            index += 1;
            match load_language_package(&root, &locale) {
                Ok(package) => {
                    if package.fallback_locale != package.locale {
                        required_fallbacks.insert(package.fallback_locale.clone());
                        add_locale_candidate(&mut candidates, package.fallback_locale.clone());
                    }
                    loaded.push(package);
                }
                Err(error) if loaded.is_empty() || required_fallbacks.contains(&locale) => {
                    push_issue(
                        &mut issues,
                        format!("language package {} unavailable: {error}", locale),
                    )
                }
                Err(_) => {}
            }
        }
        let embedded = Localizer::embedded_en_us();
        let (effective_locale, package_version, supported) = loaded.first().map_or_else(
            || {
                (
                    EMBEDDED_LOCALE.to_owned(),
                    EMBEDDED_PACKAGE_VERSION.to_owned(),
                    embedded.supported_terminal_encodings.clone(),
                )
            },
            |package| {
                (
                    package.locale.to_string(),
                    package.version.clone(),
                    package.supported_terminal_encodings.clone(),
                )
            },
        );
        let mut bundles = loaded
            .iter()
            .map(|package| package.bundle.clone())
            .collect::<Vec<_>>();
        bundles.push(embedded.bundles[0].clone());
        let localizer = Arc::new(Localizer {
            requested_locale: requested_id.to_string(),
            effective_locale: effective_locale.clone(),
            bundles,
            supported_terminal_encodings: supported,
            pseudo: false,
        });
        let status = LanguageStatus {
            default_locale: board_id.to_string(),
            effective_locale,
            package_version,
            degraded: !issues.is_empty(),
            issues,
        };
        Self { localizer, status }
    }

    pub fn localizer(&self) -> Arc<Localizer> {
        self.localizer.clone()
    }

    pub fn status(&self) -> &LanguageStatus {
        &self.status
    }
}

struct LoadedLanguage {
    locale: LanguageIdentifier,
    version: String,
    fallback_locale: LanguageIdentifier,
    supported_terminal_encodings: BTreeSet<TerminalTextEncoding>,
    bundle: Arc<ConcurrentBundle>,
}

pub fn normalize_locale(value: &str) -> Result<LanguageIdentifier, LanguageError> {
    if value.is_empty() || value.len() > 64 || value.contains('_') {
        return Err(LanguageError::InvalidLocale(value.to_owned()));
    }
    value
        .parse::<LanguageIdentifier>()
        .map_err(|_| LanguageError::InvalidLocale(value.to_owned()))
}

pub fn normalize_host_locale(value: &str) -> Option<LanguageIdentifier> {
    let base = value.split(['.', '@']).next()?.replace('_', "-");
    normalize_locale(&base).ok()
}

pub fn bootstrap_locale(explicit: Option<&str>) -> Result<LanguageIdentifier, LanguageError> {
    if let Some(explicit) = explicit {
        let locale = normalize_locale(explicit)?;
        if locale != EMBEDDED_LOCALE {
            return Err(LanguageError::UnsupportedBootstrapLocale(
                locale.to_string(),
            ));
        }
        return Ok(locale);
    }
    for variable in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(variable) {
            if let Some(locale) = normalize_host_locale(&value) {
                if locale.language.as_str() == "en" {
                    return normalize_locale(EMBEDDED_LOCALE);
                }
            }
        }
    }
    normalize_locale(EMBEDDED_LOCALE)
}

pub fn install_embedded_en_us(system: &Path) -> Result<PathBuf, LanguageError> {
    let root = system.join(LANGUAGE_DIRECTORY).join(EMBEDDED_LOCALE);
    fs::create_dir_all(root.join("messages")).map_err(LanguageError::Io)?;
    fs::create_dir_all(root.join("LICENSES")).map_err(LanguageError::Io)?;
    let inputs = [
        ("messages/shared.ftl", LanguageFileKind::Fluent, SHARED_FTL),
        ("messages/caller.ftl", LanguageFileKind::Fluent, CALLER_FTL),
        (
            "messages/operator.ftl",
            LanguageFileKind::Fluent,
            OPERATOR_FTL,
        ),
        ("README.md", LanguageFileKind::Documentation, PACKAGE_README),
        (
            "LICENSES/ASSET-LICENSE.txt",
            LanguageFileKind::License,
            PACKAGE_LICENSE,
        ),
    ];
    let mut files = Vec::new();
    for (path, kind, body) in inputs {
        fs::write(root.join(path), body.as_bytes()).map_err(LanguageError::Io)?;
        files.push(LanguageFileRecord {
            path: PathBuf::from(path),
            kind,
            bytes: body.len() as u64,
            sha256: sha256(body.as_bytes()),
            provenance: "en-us-project".to_owned(),
        });
    }
    let manifest = LanguageManifest {
        format_version: LANGUAGE_FORMAT_VERSION,
        locale: EMBEDDED_LOCALE.to_owned(),
        package_version: EMBEDDED_PACKAGE_VERSION.to_owned(),
        catalog_api_version: CATALOG_API_VERSION,
        engine: LanguageEngineCompatibility {
            minimum: "0.1.0".to_owned(),
            maximum_exclusive: "0.2.0".to_owned(),
        },
        fallback_locale: EMBEDDED_LOCALE.to_owned(),
        supported_terminal_encodings: vec![
            TerminalTextEncoding::Utf8,
            TerminalTextEncoding::Cp437,
            TerminalTextEncoding::Ascii,
        ],
        provenance: vec![LanguageProvenance {
            id: "en-us-project".to_owned(),
            creator: "SPITFIRE NG contributors".to_owned(),
            rightsholder: "Craig Daters and SPITFIRE NG contributors".to_owned(),
            source: "M037.2 independently authored SPITFIRE NG UI extraction".to_owned(),
            classification: "project-authored".to_owned(),
            license: "MIT OR Apache-2.0".to_owned(),
            redistribution: "allowed".to_owned(),
            modifications: "Canonical en-US catalog baseline".to_owned(),
        }],
        files,
    };
    let encoded = toml::to_string_pretty(&manifest)
        .map_err(|error| LanguageError::InvalidManifest(error.to_string()))?;
    fs::write(root.join(LANGUAGE_MANIFEST), encoded).map_err(LanguageError::Io)?;
    load_language_package(
        &system.join(LANGUAGE_DIRECTORY),
        &normalize_locale(EMBEDDED_LOCALE)?,
    )?;
    Ok(root)
}

pub fn validate_language_package(
    package_directory: &Path,
) -> Result<ValidatedLanguagePackage, LanguageError> {
    let name = package_directory
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(LanguageError::UnsafePackage)?;
    let locale = normalize_locale(name)?;
    if locale != name {
        return Err(LanguageError::InvalidLocale(name.to_owned()));
    }
    let parent = package_directory
        .parent()
        .ok_or(LanguageError::UnsafePackage)?;
    let package = load_language_package(parent, &locale)?;
    Ok(ValidatedLanguagePackage {
        locale: package.locale.to_string(),
        package_version: package.version,
    })
}

/// Validates an untrusted package in place, copies only its declared regular
/// files into a board-local staging directory, validates the copy again, and
/// atomically installs it without replacing an existing locale.
pub fn install_language_package(
    system: &Path,
    package_directory: &Path,
) -> Result<ValidatedLanguagePackage, LanguageError> {
    let validated = validate_language_package(package_directory)?;
    let root = system.join(LANGUAGE_DIRECTORY);
    fs::create_dir_all(&root).map_err(LanguageError::Io)?;
    let destination = root.join(&validated.locale);
    if destination.exists() {
        return Err(LanguageError::AlreadyInstalled(validated.locale));
    }
    let manifest_input =
        fs::read_to_string(package_directory.join(LANGUAGE_MANIFEST)).map_err(LanguageError::Io)?;
    let manifest: LanguageManifest = toml::from_str(&manifest_input)
        .map_err(|error| LanguageError::InvalidManifest(error.to_string()))?;
    let staging = tempfile::Builder::new()
        .prefix(".language-install-")
        .tempdir_in(&root)
        .map_err(LanguageError::Io)?;
    let staged_package = staging.path().join(&validated.locale);
    fs::create_dir(&staged_package).map_err(LanguageError::Io)?;
    for record in &manifest.files {
        validate_relative_path(&record.path)?;
        let target = staged_package.join(&record.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(LanguageError::Io)?;
        }
        fs::copy(package_directory.join(&record.path), target).map_err(LanguageError::Io)?;
    }
    fs::write(staged_package.join(LANGUAGE_MANIFEST), manifest_input).map_err(LanguageError::Io)?;
    validate_language_package(&staged_package)?;
    fs::rename(&staged_package, &destination).map_err(LanguageError::Io)?;
    Ok(validated)
}

thread_local! {
    static LOCALIZER_STACK: RefCell<Vec<Arc<Localizer>>> = const { RefCell::new(Vec::new()) };
}

pub fn with_localizer<T>(localizer: Arc<Localizer>, operation: impl FnOnce() -> T) -> T {
    LOCALIZER_STACK.with(|stack| stack.borrow_mut().push(localizer));
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            LOCALIZER_STACK.with(|stack| {
                stack.borrow_mut().pop();
            });
        }
    }
    let scope = Scope;
    let result = operation();
    drop(scope);
    result
}

pub fn text(key: &str, arguments: &LocalizationArgs) -> String {
    LOCALIZER_STACK
        .with(|stack| stack.borrow().last().cloned())
        .unwrap_or_else(Localizer::embedded_en_us)
        .text(key, arguments)
}

pub fn localized_bytes(
    terminal: &TerminalInfo,
    key: &str,
    arguments: &LocalizationArgs,
) -> Vec<u8> {
    LOCALIZER_STACK
        .with(|stack| stack.borrow().last().cloned())
        .unwrap_or_else(Localizer::embedded_en_us)
        .bytes_for_terminal(terminal, key, arguments)
}

pub fn embedded_catalog_keys() -> Result<BTreeSet<String>, LanguageError> {
    let mut keys = BTreeSet::new();
    for source in [SHARED_FTL, CALLER_FTL, OPERATOR_FTL] {
        keys.extend(catalog_keys(source)?);
    }
    Ok(keys)
}

fn load_language_package(
    root: &Path,
    requested: &LanguageIdentifier,
) -> Result<LoadedLanguage, LanguageError> {
    let directory = root.join(requested.to_string());
    let metadata = fs::symlink_metadata(&directory).map_err(LanguageError::Io)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(LanguageError::UnsafePackage);
    }
    let manifest_path = directory.join(LANGUAGE_MANIFEST);
    let metadata = fs::symlink_metadata(&manifest_path).map_err(LanguageError::Io)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(LanguageError::UnsafePackage);
    }
    if metadata.len() as usize > MAX_MANIFEST_BYTES {
        return Err(LanguageError::PackageTooLarge);
    }
    let input = fs::read_to_string(&manifest_path).map_err(LanguageError::Io)?;
    let manifest: LanguageManifest = toml::from_str(&input)
        .map_err(|error| LanguageError::InvalidManifest(error.to_string()))?;
    validate_manifest(&manifest, requested)?;
    let provenance = manifest
        .provenance
        .iter()
        .map(|record| record.id.as_str())
        .collect::<BTreeSet<_>>();
    if provenance.len() != manifest.provenance.len()
        || manifest.provenance.iter().any(|record| {
            record.id.is_empty()
                || record.creator.is_empty()
                || record.rightsholder.is_empty()
                || record.source.is_empty()
                || record.classification.is_empty()
                || record.license.is_empty()
                || record.redistribution != "allowed"
                || record.modifications.is_empty()
        })
    {
        return Err(LanguageError::InvalidProvenance);
    }
    let mut declared = BTreeSet::new();
    let mut catalog_sources = BTreeMap::new();
    let mut total = 0usize;
    for record in &manifest.files {
        validate_relative_path(&record.path)?;
        if !declared.insert(inventory_key(&record.path)?)
            || !provenance.contains(record.provenance.as_str())
        {
            return Err(LanguageError::InvalidInventory);
        }
        let path = directory.join(&record.path);
        let metadata = fs::symlink_metadata(&path).map_err(LanguageError::Io)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(LanguageError::UnsafePackage);
        }
        let actual = usize::try_from(metadata.len()).map_err(|_| LanguageError::PackageTooLarge)?;
        total = total
            .checked_add(actual)
            .ok_or(LanguageError::PackageTooLarge)?;
        if actual > MAX_LANGUAGE_FILE_BYTES
            || total > MAX_LANGUAGE_TOTAL_BYTES
            || record.bytes != metadata.len()
        {
            return Err(LanguageError::PackageTooLarge);
        }
        let bytes = fs::read(&path).map_err(LanguageError::Io)?;
        if sha256(&bytes) != record.sha256 {
            return Err(LanguageError::HashMismatch(record.path.clone()));
        }
        if record.kind == LanguageFileKind::Fluent {
            let source = String::from_utf8(bytes)
                .map_err(|_| LanguageError::InvalidCatalog(record.path.clone()))?;
            catalog_sources.insert(record.path.clone(), source);
        }
    }
    let actual = recursive_inventory(&directory)?;
    let mut expected = manifest
        .files
        .iter()
        .map(|record| inventory_key(&record.path))
        .collect::<Result<BTreeSet<_>, _>>()?;
    expected.insert(LANGUAGE_MANIFEST.to_owned());
    if actual != expected {
        return Err(LanguageError::InvalidInventory);
    }
    for required in [
        "messages/shared.ftl",
        "messages/caller.ftl",
        "messages/operator.ftl",
    ] {
        if !catalog_sources.contains_key(Path::new(required)) {
            return Err(LanguageError::MissingRequiredCatalog(required));
        }
    }
    let locale = normalize_locale(&manifest.locale)?;
    let bundle = build_bundle(&locale, catalog_sources.values().map(String::as_str))?;
    if locale == EMBEDDED_LOCALE {
        let found = catalog_sources
            .values()
            .map(|source| catalog_keys(source))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<BTreeSet<_>>();
        if found != embedded_catalog_keys()? {
            return Err(LanguageError::IncompleteEnglishCatalog);
        }
    }
    Ok(LoadedLanguage {
        locale,
        version: manifest.package_version,
        fallback_locale: normalize_locale(&manifest.fallback_locale)?,
        supported_terminal_encodings: manifest.supported_terminal_encodings.into_iter().collect(),
        bundle: Arc::new(bundle),
    })
}

fn validate_manifest(
    manifest: &LanguageManifest,
    requested: &LanguageIdentifier,
) -> Result<(), LanguageError> {
    if manifest.format_version != LANGUAGE_FORMAT_VERSION
        || manifest.catalog_api_version != CATALOG_API_VERSION
        || normalize_locale(&manifest.locale)? != *requested
        || parse_semver(&manifest.package_version).is_none()
        || manifest.supported_terminal_encodings.is_empty()
        || manifest
            .supported_terminal_encodings
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != manifest.supported_terminal_encodings.len()
        || manifest.files.is_empty()
        || manifest.files.len() > MAX_LANGUAGE_FILES
    {
        return Err(LanguageError::InvalidManifest(
            "unsupported identity, version, or empty inventory".to_owned(),
        ));
    }
    let fallback = normalize_locale(&manifest.fallback_locale)?;
    if fallback.to_string() != manifest.fallback_locale {
        return Err(LanguageError::InvalidManifest(
            "fallback_locale must use canonical BCP 47 spelling".to_owned(),
        ));
    }
    if manifest.locale == EMBEDDED_LOCALE && manifest.fallback_locale != EMBEDDED_LOCALE {
        return Err(LanguageError::InvalidManifest(
            "en-US must fall back to the embedded en-US baseline".to_owned(),
        ));
    }
    let current = parse_semver(PRODUCT_VERSION).ok_or_else(|| {
        LanguageError::InvalidManifest("engine version is not core semantic version".to_owned())
    })?;
    let minimum = parse_semver(&manifest.engine.minimum)
        .ok_or_else(|| LanguageError::InvalidManifest("invalid engine minimum".to_owned()))?;
    let maximum = parse_semver(&manifest.engine.maximum_exclusive).ok_or_else(|| {
        LanguageError::InvalidManifest("invalid engine maximum_exclusive".to_owned())
    })?;
    if minimum >= maximum || current < minimum || current >= maximum {
        return Err(LanguageError::IncompatibleEngine);
    }
    Ok(())
}

fn build_bundle<'a>(
    locale: &LanguageIdentifier,
    sources: impl IntoIterator<Item = &'a str>,
) -> Result<ConcurrentBundle, LanguageError> {
    let mut bundle = ConcurrentBundle::new_concurrent(vec![locale.clone()]);
    bundle.set_use_isolating(false);
    for source in sources {
        let resource = FluentResource::try_new(source.to_owned())
            .map_err(|(_, errors)| LanguageError::Fluent(format!("{errors:?}")))?;
        bundle
            .add_resource(resource)
            .map_err(|errors| LanguageError::Fluent(format!("{errors:?}")))?;
    }
    Ok(bundle)
}

fn catalog_keys(source: &str) -> Result<BTreeSet<String>, LanguageError> {
    let resource = fluent_syntax::parser::parse(source)
        .map_err(|(_, errors)| LanguageError::Fluent(format!("{errors:?}")))?;
    let mut keys = BTreeSet::new();
    for entry in resource.body {
        if let ast::Entry::Message(message) = entry {
            if message.value.is_none() || !message.attributes.is_empty() {
                return Err(LanguageError::Fluent(
                    "messages must have one value and no attributes".to_owned(),
                ));
            }
            if !keys.insert(message.id.name.to_owned()) {
                return Err(LanguageError::Fluent("duplicate message key".to_owned()));
            }
        } else if matches!(entry, ast::Entry::Term(_)) {
            return Err(LanguageError::Fluent(
                "terms are not part of catalog API 1".to_owned(),
            ));
        }
    }
    Ok(keys)
}

fn recursive_inventory(root: &Path) -> Result<BTreeSet<String>, LanguageError> {
    fn visit(
        base: &Path,
        directory: &Path,
        output: &mut BTreeSet<String>,
    ) -> Result<(), LanguageError> {
        for entry in fs::read_dir(directory).map_err(LanguageError::Io)? {
            let entry = entry.map_err(LanguageError::Io)?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(LanguageError::Io)?;
            if metadata.file_type().is_symlink() {
                return Err(LanguageError::UnsafePackage);
            }
            if metadata.is_dir() {
                visit(base, &path, output)?;
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(base)
                    .map_err(|_| LanguageError::UnsafePackage)?;
                if !output.insert(inventory_key(relative)?) {
                    return Err(LanguageError::InvalidInventory);
                }
            } else {
                return Err(LanguageError::UnsafePackage);
            }
        }
        Ok(())
    }
    let mut output = BTreeSet::new();
    visit(root, root, &mut output)?;
    Ok(output)
}

fn inventory_key(path: &Path) -> Result<String, LanguageError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(LanguageError::UnsafePackage);
        };
        parts.push(part.to_string_lossy().to_ascii_lowercase());
    }
    if parts.is_empty() {
        return Err(LanguageError::UnsafePackage);
    }
    Ok(parts.join("/"))
}

fn validate_relative_path(path: &Path) -> Result<(), LanguageError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(LanguageError::UnsafePackage);
    }
    Ok(())
}

fn add_locale_candidate(candidates: &mut Vec<LanguageIdentifier>, locale: LanguageIdentifier) {
    if !candidates.contains(&locale) {
        candidates.push(locale);
    }
}

fn parent_locale(locale: &LanguageIdentifier) -> Option<LanguageIdentifier> {
    if locale.script.is_none() && locale.region.is_none() && locale.variants().next().is_none() {
        return None;
    }
    locale.language.as_str().parse().ok()
}

fn push_issue(issues: &mut Vec<String>, issue: String) {
    if issues.len() < MAX_REPORTED_ISSUES {
        issues.push(issue);
    }
}

fn parse_semver(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split('.');
    let version = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(version)
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn format_local_timestamp(timestamp: i64, timezone: Tz, locale: &str) -> String {
    let Some(utc) = chrono::DateTime::<Utc>::from_timestamp(timestamp, 0) else {
        return "Invalid date/time".to_owned();
    };
    let local = utc.with_timezone(&timezone);
    if locale == "en-US" {
        let (hour, suffix) = match local.hour() {
            0 => (12, "AM"),
            1..=11 => (local.hour(), "AM"),
            12 => (12, "PM"),
            hour => (hour - 12, "PM"),
        };
        format!(
            "{:02}/{:02}/{:04} {}:{:02} {} {}",
            local.month(),
            local.day(),
            local.year(),
            hour,
            local.minute(),
            suffix,
            local.format("%Z")
        )
    } else {
        local.format("%Y-%m-%d %H:%M %Z").to_string()
    }
}

pub fn terminal_text_encoding(terminal: &TerminalInfo) -> TerminalTextEncoding {
    if terminal.capabilities.cp437 {
        TerminalTextEncoding::Cp437
    } else {
        TerminalTextEncoding::Utf8
    }
}

pub fn encode_text(text: &str, encoding: TerminalTextEncoding) -> Option<Vec<u8>> {
    match encoding {
        TerminalTextEncoding::Utf8 => Some(text.as_bytes().to_vec()),
        TerminalTextEncoding::Ascii => text.is_ascii().then(|| text.as_bytes().to_vec()),
        TerminalTextEncoding::Cp437 => {
            const EXTENDED: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■ ";
            let table = EXTENDED.chars().collect::<Vec<_>>();
            if table.len() != 128 {
                return None;
            }
            let mut output = Vec::with_capacity(text.len());
            for character in text.chars() {
                if character.is_ascii() {
                    output.push(character as u8);
                } else {
                    let index = table.iter().position(|entry| *entry == character)?;
                    output.push(0x80 + index as u8);
                }
            }
            Some(output)
        }
    }
}

fn pseudo_expand(value: &str) -> String {
    let mut output = String::with_capacity(value.len() * 2 + 4);
    output.push('⟦');
    for character in value.chars() {
        let replacement: Cow<'_, str> = match character {
            'a' => Cow::Borrowed("áá"),
            'e' => Cow::Borrowed("éé"),
            'i' => Cow::Borrowed("íí"),
            'o' => Cow::Borrowed("óó"),
            'u' => Cow::Borrowed("úú"),
            'A' => Cow::Borrowed("ÁÁ"),
            'E' => Cow::Borrowed("ÉÉ"),
            'I' => Cow::Borrowed("ÍÍ"),
            'O' => Cow::Borrowed("ÓÓ"),
            'U' => Cow::Borrowed("ÚÚ"),
            _ => Cow::Owned(character.to_string()),
        };
        output.push_str(&replacement);
    }
    output.push_str("⟧界");
    output
}

fn emergency_ascii(key: &str) -> &'static str {
    match key {
        "emergency-localization-failure" => {
            "Localization failure. Limited English recovery is active."
        }
        _ => "The requested text is unavailable.",
    }
}

#[derive(Debug, Error)]
pub enum LanguageError {
    #[error("invalid BCP 47 locale identifier {0:?}")]
    InvalidLocale(String),
    #[error("bootstrap locale {0:?} is not installed; use en-US")]
    UnsupportedBootstrapLocale(String),
    #[error("invalid language manifest: {0}")]
    InvalidManifest(String),
    #[error("language package is incompatible with this engine")]
    IncompatibleEngine,
    #[error("language package inventory is invalid")]
    InvalidInventory,
    #[error("language package provenance is incomplete or not redistributable")]
    InvalidProvenance,
    #[error("language package contains an unsafe path, symlink, or special file")]
    UnsafePackage,
    #[error("language package exceeds a size or file-count bound")]
    PackageTooLarge,
    #[error("language package hash does not match for {0}")]
    HashMismatch(PathBuf),
    #[error("language catalog {0} is not valid UTF-8 or Fluent")]
    InvalidCatalog(PathBuf),
    #[error("language package is missing required catalog {0}")]
    MissingRequiredCatalog(&'static str),
    #[error("en-US catalog is not the complete canonical baseline")]
    IncompleteEnglishCatalog,
    #[error("language locale {0} is already installed")]
    AlreadyInstalled(String),
    #[error("invalid Fluent catalog: {0}")]
    Fluent(String),
    #[error("language package I/O failed: {0}")]
    Io(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LogicalPaths, RuntimeConfig};

    #[test]
    fn locale_normalization_parent_and_bootstrap_are_bcp47_bounded() {
        assert_eq!(normalize_locale("EN-us").unwrap().to_string(), "en-US");
        assert!(normalize_locale("es_MX").is_err());
        assert_eq!(
            parent_locale(&normalize_locale("zh-Hant-TW").unwrap())
                .unwrap()
                .to_string(),
            "zh"
        );
        assert!(matches!(
            bootstrap_locale(Some("es-ES")),
            Err(LanguageError::UnsupportedBootstrapLocale(_))
        ));
    }

    #[test]
    fn typed_timestamp_and_pseudo_expansion_are_supported() {
        let english = Localizer::embedded_en_us();
        let output = english.text(
            "caller-context-call-count",
            &LocalizationArgs::new()
                .with("caller_number", 2u64)
                .with("calls_today", 1u64)
                .with("total_calls", 4u64),
        );
        assert!(output.contains("caller #2"));
        assert!(output.contains("call 1 today"));
        let time = english.format_value(&LocalizationValue::Timestamp {
            unix_seconds: 1_735_689_600,
            timezone: chrono_tz::America::Phoenix,
        });
        assert!(matches!(time, FormattedValue::Text(value) if value == "12/31/2024 5:00 PM MST"));
        let pseudo = Localizer::pseudo_for_tests().text(
            "caller-welcome",
            &LocalizationArgs::new().with("caller", "Craig"),
        );
        assert!(pseudo.starts_with('⟦'));
        assert!(pseudo.len() > "Welcome, Craig.".len());
        assert!(pseudo.contains('界'));
    }

    #[test]
    fn terminal_encoding_is_strict_and_falls_back_without_mojibake() {
        assert_eq!(
            encode_text("Café", TerminalTextEncoding::Cp437).unwrap(),
            b"Caf\x82"
        );
        assert!(encode_text("界", TerminalTextEncoding::Cp437).is_none());
        assert!(encode_text("é", TerminalTextEncoding::Ascii).is_none());
    }

    #[test]
    fn installed_en_us_package_is_strict_complete_and_hash_validated() {
        let temp = tempfile::tempdir().unwrap();
        let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(temp.path(), &validated).unwrap();
        paths.create_directories().unwrap();
        install_embedded_en_us(paths.get(LogicalPath::System)).unwrap();
        let resolver = LanguageResolver::load(&paths, "en-US");
        assert!(!resolver.status().degraded, "{:?}", resolver.status());
        assert_eq!(resolver.status().effective_locale, "en-US");

        let caller = paths
            .get(LogicalPath::System)
            .join("language-packs/en-US/messages/caller.ftl");
        fs::write(caller, "caller-welcome = changed").unwrap();
        let degraded = LanguageResolver::load(&paths, "en-US");
        assert!(degraded.status().degraded);
        assert_eq!(
            degraded.localizer().text(
                "caller-welcome",
                &LocalizationArgs::new().with("caller", "Craig")
            ),
            "Welcome, Craig."
        );
    }

    #[test]
    fn en_us_is_complete_for_semantic_callsites_and_emergency_never_leaks_keys() {
        let keys = embedded_catalog_keys().unwrap();
        assert!(
            keys.len() >= 200,
            "unexpectedly small baseline: {}",
            keys.len()
        );
        for source in [
            include_str!("session.rs"),
            include_str!("message_session.rs"),
            include_str!("file_session.rs"),
            include_str!("terminal.rs"),
            include_str!("resources.rs"),
            include_str!("../../sf-bbs/src/lib.rs"),
            include_str!("../../sf-bbs/src/setup.rs"),
            include_str!("../../sf-bbs/src/admin.rs"),
            include_str!("../../sf-bbs/src/status.rs"),
            include_str!("../../sf-bbs/src/operator.rs"),
        ] {
            for marker in [
                "write_key(",
                "write_key_line(",
                "op(",
                "op_args(",
                "operator_text(",
                "sf_core::text(",
            ] {
                let mut remaining = source;
                while let Some(offset) = remaining.find(marker) {
                    remaining = &remaining[offset + marker.len()..];
                    let Some(first_quote) = remaining.find('"') else {
                        continue;
                    };
                    let tail = &remaining[first_quote + 1..];
                    let Some(last_quote) = tail.find('"') else {
                        continue;
                    };
                    let candidate = &tail[..last_quote];
                    if [
                        "caller-",
                        "file-",
                        "help-",
                        "menu-",
                        "message-",
                        "operator-",
                        "session-",
                        "shared-",
                        "transfer-",
                    ]
                    .iter()
                    .any(|prefix| candidate.starts_with(prefix))
                    {
                        assert!(keys.contains(candidate), "missing catalog key {candidate}");
                    }
                    remaining = &tail[last_quote + 1..];
                }
            }
        }
        let missing = Localizer::embedded_en_us()
            .text("caller-deliberately-missing", &LocalizationArgs::new());
        assert_eq!(missing, "The requested text is unavailable.");
        assert!(!missing.contains("caller-deliberately-missing"));
    }

    #[test]
    fn caller_ui_sources_reject_new_direct_english_writes() {
        for (name, source) in [
            ("session.rs", include_str!("session.rs")),
            ("message_session.rs", include_str!("message_session.rs")),
            ("file_session.rs", include_str!("file_session.rs")),
        ] {
            assert!(
                !source.contains("write_line(terminal, \""),
                "{name} contains direct caller-facing prose; add a semantic catalog key"
            );
            for line in source.lines() {
                let direct_bytes = line.contains("write_all(b\"")
                    && line.split("write_all(b\"").nth(1).is_some_and(|tail| {
                        tail.replace("\\r", "")
                            .replace("\\n", "")
                            .replace("\\t", "")
                            .bytes()
                            .any(|byte| byte.is_ascii_alphabetic())
                    });
                assert!(
                    !direct_bytes,
                    "{name} contains direct caller-facing byte prose: {line}"
                );
            }
        }
        for (name, source) in [
            ("lib.rs", include_str!("../../sf-bbs/src/lib.rs")),
            ("setup.rs", include_str!("../../sf-bbs/src/setup.rs")),
            ("admin.rs", include_str!("../../sf-bbs/src/admin.rs")),
            ("status.rs", include_str!("../../sf-bbs/src/status.rs")),
            ("operator.rs", include_str!("../../sf-bbs/src/operator.rs")),
        ] {
            for direct in [
                "writeln!(output, \"A",
                "writeln!(output, \"B",
                "writeln!(output, \"C",
                "writeln!(output, \"D",
                "writeln!(output, \"E",
                "writeln!(output, \"F",
                "writeln!(output, \"G",
                "writeln!(output, \"H",
                "writeln!(output, \"I",
                "writeln!(output, \"J",
                "writeln!(output, \"K",
                "writeln!(output, \"L",
                "writeln!(output, \"M",
                "writeln!(output, \"N",
                "writeln!(output, \"O",
                "writeln!(output, \"P",
                "writeln!(output, \"Q",
                "writeln!(output, \"R",
                "writeln!(output, \"S",
                "writeln!(output, \"T",
                "writeln!(output, \"U",
                "writeln!(output, \"V",
                "writeln!(output, \"W",
                "writeln!(output, \"X",
                "writeln!(output, \"Y",
                "writeln!(output, \"Z",
                "write!(output, \"Operator",
                "write!(output, \"Sysop",
                "prompt_password(\"",
            ] {
                assert!(
                    !source.contains(direct),
                    "{name} contains direct interactive operator prose; add a semantic catalog key"
                );
            }
        }
    }

    #[test]
    fn requested_parent_board_and_embedded_fallback_are_deterministic() {
        let temp = tempfile::tempdir().unwrap();
        let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(temp.path(), &validated).unwrap();
        paths.create_directories().unwrap();
        install_embedded_en_us(paths.get(LogicalPath::System)).unwrap();
        let resolver = LanguageResolver::load_for_locale(&paths, "es-MX", "en-US");
        assert_eq!(resolver.localizer().requested_locale(), "es-MX");
        assert_eq!(resolver.localizer().effective_locale(), "en-US");
        assert!(resolver.status().degraded);
        assert!(resolver.status().issues[0].contains("es-MX"));
        assert_eq!(
            resolver
                .localizer()
                .text("caller-login-failed", &LocalizationArgs::new()),
            "Invalid caller name or password."
        );
    }

    #[test]
    fn thread_local_session_localizers_do_not_leak_between_nodes() {
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let first_barrier = barrier.clone();
        let first = std::thread::spawn(move || {
            with_localizer(Localizer::pseudo_for_tests(), || {
                first_barrier.wait();
                text("caller-login-failed", &LocalizationArgs::new())
            })
        });
        let second = std::thread::spawn(move || {
            with_localizer(Localizer::embedded_en_us(), || {
                barrier.wait();
                text("caller-login-failed", &LocalizationArgs::new())
            })
        });
        assert!(first.join().unwrap().starts_with('⟦'));
        assert_eq!(second.join().unwrap(), "Invalid caller name or password.");
        assert_eq!(
            text("caller-login-failed", &LocalizationArgs::new()),
            "Invalid caller name or password."
        );
    }
}
