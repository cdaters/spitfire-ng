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

use std::io::{self, BufRead, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use sf_core::{
    BoardAccessMode, Conference, ConferenceAccessMode, ConferenceDefinition, FileAccessMode,
    FileArea, FileAreaDefinition, FileStorage, LogicalPaths, MenuPresentationMode, NodePoolConfig,
    PostLoginJourney, PresentationMode, ProfileFieldPolicy, RuntimeConfig, RuntimeDatabase,
    SecurityLevel, TransportAdapterConfig,
};

use crate::board_lock::BoardOperationLock;
use crate::ApplicationError;

pub struct BoardAdmin {
    _operation_lock: BoardOperationLock,
    config_path: PathBuf,
    root: PathBuf,
    config: RuntimeConfig,
    paths: LogicalPaths,
}

impl BoardAdmin {
    pub fn load(config_path: &Path) -> Result<Self, ApplicationError> {
        let config_path = config_path.canonicalize().map_err(|source| {
            ApplicationError::ResolveConfiguration {
                path: config_path.to_path_buf(),
                source,
            }
        })?;
        let root = config_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| ApplicationError::MissingBoardRoot(config_path.clone()))?
            .to_path_buf();
        let operation_lock = BoardOperationLock::acquire(&root)?;
        let config = RuntimeConfig::load(&config_path)?;
        crate::configuration::ConfigurationAuthority::new(config_path.clone(), config.clone())?;
        let validated = config.validate()?;
        let paths = LogicalPaths::resolve(&root, &validated)?;
        Ok(Self {
            _operation_lock: operation_lock,
            config_path,
            root,
            config,
            paths,
        })
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn save_static(&mut self, mut replacement: RuntimeConfig) -> Result<(), ApplicationError> {
        let disk = RuntimeConfig::load(&self.config_path)?;
        if crate::configuration_version(&disk)? != crate::configuration_version(&self.config)? {
            return Err(crate::OperatorControlError::Conflict.into());
        }
        replacement.revision = self
            .config
            .revision
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(ApplicationError::Coordination(
                "configuration revision exhausted",
            ))?;
        replacement.configuration_commit = None;
        let previous = self.config.validate()?;
        let replacement_validated = replacement.validate()?;
        if previous.paths != replacement_validated.paths
            || previous.database_file != replacement_validated.database_file
        {
            return Err(ApplicationError::ConfigurationRelocationUnsupported);
        }
        let replacement_paths = LogicalPaths::resolve(&self.root, &replacement_validated)?;
        if replacement_paths.database() != self.paths.database() {
            return Err(ApplicationError::ConfigurationRelocationUnsupported);
        }
        let database = RuntimeDatabase::open(self.paths.database())?;
        self.config
            .save_atomic(&self.config_path.with_extension("toml.previous"))?;
        let identity_changed = previous.identity != replacement_validated.identity;
        if identity_changed {
            database.replace_board_identity(&previous.identity, &replacement_validated.identity)?;
        }
        if let Err(error) = replacement.save_atomic(&self.config_path) {
            if identity_changed {
                let _ = database
                    .replace_board_identity(&replacement_validated.identity, &previous.identity);
            }
            return Err(error.into());
        }
        self.config = replacement;
        Ok(())
    }

    pub fn conferences(&self) -> Result<Vec<Conference>, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .all_conferences()
            .map_err(Into::into)
    }

    pub fn create_conference(
        &self,
        definition: &ConferenceDefinition,
    ) -> Result<Conference, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .create_conference(definition)
            .map_err(Into::into)
    }

    pub fn update_conference(
        &self,
        number: u16,
        definition: &ConferenceDefinition,
    ) -> Result<Conference, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .update_conference(number, definition)
            .map_err(Into::into)
    }

    pub fn set_conference_enabled(
        &self,
        number: u16,
        enabled: bool,
    ) -> Result<(), ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .set_conference_enabled(number, enabled)
            .map_err(Into::into)
    }

    pub fn file_areas(&self) -> Result<Vec<FileArea>, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .all_file_areas()
            .map_err(Into::into)
    }

    pub fn create_file_area(
        &self,
        definition: &FileAreaDefinition,
    ) -> Result<FileArea, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        let area = database.create_file_area(definition)?;
        FileStorage::new(&self.paths)?.ensure_area(&area)?;
        Ok(area)
    }

    pub fn update_file_area(
        &self,
        number: u16,
        definition: &FileAreaDefinition,
    ) -> Result<FileArea, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .update_file_area(number, definition)
            .map_err(Into::into)
    }

    pub fn set_file_area_enabled(
        &self,
        number: u16,
        enabled: bool,
    ) -> Result<(), ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .set_file_area_enabled(number, enabled)
            .map_err(Into::into)
    }

    pub fn file_count(&self, area: sf_core::FileAreaId) -> Result<u64, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .file_count(area)
            .map_err(Into::into)
    }
}

pub fn interactive_config(config_path: &Path) -> Result<String, ApplicationError> {
    let mut admin = BoardAdmin::load(config_path)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout();
    let mut working = admin.config().clone();
    loop {
        writeln!(output, "\n{}", op("operator-config-title")).map_err(ApplicationError::SetupIo)?;
        match prompt(
            &mut input,
            &mut output,
            &op("operator-config-selection"),
            "Q",
        )?
        .to_ascii_uppercase()
        .as_str()
        {
            "1" => {
                working.board.name = prompt(
                    &mut input,
                    &mut output,
                    &op("operator-setup-board-name"),
                    &working.board.name,
                )?;
                working.board.sysop = prompt(
                    &mut input,
                    &mut output,
                    &op("operator-setup-sysop-display-name"),
                    &working.board.sysop,
                )?;
                working.board.timezone = prompt(
                    &mut input,
                    &mut output,
                    &op("operator-setup-timezone"),
                    &working.board.timezone,
                )?;
                let access = prompt(
                    &mut input,
                    &mut output,
                    &op("operator-setup-board-access"),
                    if working.board.access.is_private() {
                        "private"
                    } else {
                        "public"
                    },
                )?;
                working.board.access = if access.eq_ignore_ascii_case("private") {
                    BoardAccessMode::Private
                } else {
                    BoardAccessMode::Public
                };
                working.board.private_security_level = prompt_u16(
                    &mut input,
                    &mut output,
                    &op("operator-setup-private-security"),
                    working.board.private_security_level,
                )?;
                working.caller.sysop_caller_name = prompt(
                    &mut input,
                    &mut output,
                    &op("operator-setup-sysop-caller-name"),
                    &working.caller.sysop_caller_name,
                )?;
            }
            "2" => {
                let current = working.nodes.as_ref().map_or(1, |nodes| nodes.count);
                let count = prompt_u32(
                    &mut input,
                    &mut output,
                    &op("operator-config-node-count"),
                    current,
                )?;
                working.node = None;
                working.nodes = Some(NodePoolConfig {
                    count,
                    overrides: working.nodes.take().map_or_else(Vec::new, |nodes| {
                        nodes
                            .overrides
                            .into_iter()
                            .filter(|value| value.number <= count)
                            .collect()
                    }),
                });
            }
            "3" => edit_terminal_services(&mut input, &mut output, &mut working)?,
            "4" => edit_caller_defaults(&mut input, &mut output, &mut working)?,
            "5" => edit_conferences(&mut input, &mut output, &admin)?,
            "6" => edit_file_areas(&mut input, &mut output, &admin)?,
            "7" => {
                let current_mode = match working.presentation.mode {
                    PresentationMode::LegacyResources => "legacy-resources",
                    PresentationMode::Profile => "profile",
                };
                let mode = prompt(
                    &mut input,
                    &mut output,
                    &op("operator-config-presentation-mode"),
                    current_mode,
                )?;
                if mode.eq_ignore_ascii_case("legacy-resources") {
                    working.presentation.mode = PresentationMode::LegacyResources;
                    working.presentation.active_profile = None;
                    working.presentation.base_profile = None;
                } else if mode.eq_ignore_ascii_case("profile") {
                    working.presentation.mode = PresentationMode::Profile;
                    let active = working
                        .presentation
                        .active_profile
                        .as_deref()
                        .unwrap_or("modern-ng");
                    let base = working
                        .presentation
                        .base_profile
                        .as_deref()
                        .unwrap_or("modern-ng");
                    working.presentation.active_profile = Some(prompt(
                        &mut input,
                        &mut output,
                        &op("operator-config-active-profile"),
                        active,
                    )?);
                    working.presentation.base_profile = Some(prompt(
                        &mut input,
                        &mut output,
                        &op("operator-config-base-profile"),
                        base,
                    )?);
                } else {
                    writeln!(output, "{}", op("operator-config-presentation-invalid"))
                        .map_err(ApplicationError::SetupIo)?;
                }
                let menu_mode = prompt(
                    &mut input,
                    &mut output,
                    &op("operator-setup-menu-presentation"),
                    match working.presentation.menu_mode {
                        MenuPresentationMode::DisplayOverrides => "display-overrides",
                        MenuPresentationMode::Generated => "generated",
                    },
                )?;
                working.presentation.menu_mode = match menu_mode.to_ascii_lowercase().as_str() {
                    "display-overrides" => MenuPresentationMode::DisplayOverrides,
                    "generated" => MenuPresentationMode::Generated,
                    _ => {
                        return Err(ApplicationError::InvalidSetupValue(
                            "menu presentation must be display-overrides or generated",
                        ));
                    }
                };
            }
            "8" => {
                working.language.default_locale = prompt(
                    &mut input,
                    &mut output,
                    &sf_core::text(
                        "operator-setup-default-locale",
                        &sf_core::LocalizationArgs::new(),
                    ),
                    &working.language.default_locale,
                )?;
            }
            "S" => {
                working.format_version = sf_core::CONFIG_FORMAT_VERSION;
                admin.save_static(working.clone())?;
                writeln!(output, "{}", op("operator-config-saved"))
                    .map_err(ApplicationError::SetupIo)?;
            }
            "Q" => return Ok(op("operator-config-ended")),
            _ => writeln!(output, "{}", op("operator-config-unknown"))
                .map_err(ApplicationError::SetupIo)?,
        }
    }
}

fn edit_file_areas(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    admin: &BoardAdmin,
) -> Result<(), ApplicationError> {
    let areas = admin.file_areas()?;
    writeln!(output, "\n{}", op("operator-config-file-areas-title"))
        .map_err(ApplicationError::SetupIo)?;
    for area in &areas {
        writeln!(
            output,
            "{}",
            op_args(
                "operator-config-file-area-row",
                sf_core::LocalizationArgs::new()
                    .with("number", area.number)
                    .with("name", area.name.as_str())
                    .with("active", area.active.to_string())
                    .with("files", admin.file_count(area.id)?)
                    .with("read", area.read_security.get())
                    .with("upload", area.upload_security.get())
                    .with("storage", area.storage_key.as_str())
            )
        )
        .map_err(ApplicationError::SetupIo)?;
    }
    match prompt(input, output, &op("operator-config-edit-actions"), "")?
        .to_ascii_uppercase()
        .as_str()
    {
        "A" => {
            let definition = prompt_file_area(input, output, None)?;
            admin.create_file_area(&definition)?;
        }
        "E" => {
            let number = prompt_u16(input, output, &op("operator-config-file-area-number"), 1)?;
            let current = areas
                .iter()
                .find(|area| area.number == number)
                .ok_or(ApplicationError::InvalidSetupValue("unknown file area"))?;
            let definition = prompt_file_area(input, output, Some(current))?;
            admin.update_file_area(number, &definition)?;
        }
        "T" => {
            let number = prompt_u16(input, output, &op("operator-config-file-area-number"), 1)?;
            let current = areas
                .iter()
                .find(|area| area.number == number)
                .ok_or(ApplicationError::InvalidSetupValue("unknown file area"))?;
            admin.set_file_area_enabled(number, !current.active)?;
        }
        _ => {}
    }
    Ok(())
}

fn prompt_file_area(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    current: Option<&FileArea>,
) -> Result<FileAreaDefinition, ApplicationError> {
    let number = prompt_u16(
        input,
        output,
        &op("operator-config-file-area-number"),
        current.map_or(1, |area| area.number),
    )?;
    let name = prompt(
        input,
        output,
        &op("operator-config-file-area-name"),
        current.map_or("Untitled File Area", |area| &area.name),
    )?;
    let description = prompt(
        input,
        output,
        &op("operator-config-file-area-description"),
        current.map_or("File area", |area| &area.description),
    )?;
    let storage_key = prompt(
        input,
        output,
        &op("operator-config-file-area-storage"),
        current.map_or("files", |area| &area.storage_key),
    )?;
    let read_security = prompt_u16(
        input,
        output,
        &op("operator-config-file-area-security"),
        current.map_or(5, |area| area.read_security.get()),
    )?;
    let upload_security = prompt_u16(
        input,
        output,
        &op("operator-config-file-area-upload-security"),
        current.map_or(5, |area| area.upload_security.get()),
    )?;
    let exact = prompt_yes_no(
        input,
        output,
        &op("operator-config-file-area-exact"),
        current.is_some_and(|area| area.access_mode == FileAccessMode::Exact),
    )?;
    let preview = prompt_yes_no(
        input,
        output,
        &op("operator-config-file-area-preview"),
        current.is_some_and(|area| area.preview),
    )?;
    let no_charge = prompt_yes_no(
        input,
        output,
        &op("operator-config-file-area-free"),
        current.is_some_and(|area| area.no_charge),
    )?;
    let max_mib = prompt_u32(
        input,
        output,
        &op("operator-config-file-area-max-mib"),
        current.map_or(10, |area| {
            u32::try_from(area.maximum_upload_bytes / (1024 * 1024)).unwrap_or(1024)
        }),
    )?;
    let privileged_default = current.map_or_else(String::new, |area| {
        area.privileged_security_levels
            .iter()
            .map(|level| level.get().to_string())
            .collect::<Vec<_>>()
            .join(",")
    });
    let privileged_security_levels = parse_security_levels(&prompt(
        input,
        output,
        &op("operator-config-privileged-levels"),
        &privileged_default,
    )?)?;
    Ok(FileAreaDefinition {
        number,
        name,
        description,
        storage_key,
        access_mode: if exact {
            FileAccessMode::Exact
        } else {
            FileAccessMode::AtLeast
        },
        read_security: SecurityLevel::new(read_security).map_err(sf_core::DatabaseError::from)?,
        upload_security: SecurityLevel::new(upload_security)
            .map_err(sf_core::DatabaseError::from)?,
        preview,
        no_charge,
        maximum_upload_bytes: u64::from(max_mib) * 1024 * 1024,
        privileged_security_levels,
    })
}

fn parse_security_levels(value: &str) -> Result<Vec<SecurityLevel>, ApplicationError> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<u16>()
                .map_err(|_| {
                    ApplicationError::InvalidSetupValue(
                        "privileged security levels must be comma-separated unsigned integers",
                    )
                })
                .and_then(|level| {
                    SecurityLevel::new(level)
                        .map_err(sf_core::DatabaseError::from)
                        .map_err(ApplicationError::from)
                })
        })
        .collect()
}

fn edit_terminal_services(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    config: &mut RuntimeConfig,
) -> Result<(), ApplicationError> {
    writeln!(output, "\n{}", op("operator-config-services-title"))
        .map_err(ApplicationError::SetupIo)?;
    for (index, transport) in config.transports.iter().enumerate() {
        writeln!(
            output,
            "{}",
            op_args(
                "operator-config-service-row",
                sf_core::LocalizationArgs::new()
                    .with("number", (index + 1) as u64)
                    .with("name", transport.effective_name(index))
                    .with("enabled", transport.enabled.to_string())
                    .with(
                        "endpoint",
                        transport
                            .network_listener()
                            .map_or_else(|| "serial/device".to_owned(), |value| value.to_string())
                    )
            )
        )
        .map_err(ApplicationError::SetupIo)?;
    }
    let selection = prompt(input, output, &op("operator-config-service-selection"), "")?;
    if selection.is_empty() {
        return Ok(());
    }
    let index = selection
        .parse::<usize>()
        .ok()
        .and_then(|value| value.checked_sub(1))
        .filter(|value| *value < config.transports.len())
        .ok_or(ApplicationError::InvalidSetupValue(
            "invalid service number",
        ))?;
    let transport = &mut config.transports[index];
    transport.enabled = prompt_yes_no(
        input,
        output,
        &op("operator-config-enabled"),
        transport.enabled,
    )?;
    if transport.enabled {
        if let Some(current) = transport.network_listener() {
            let address = prompt(
                input,
                output,
                &op("operator-config-bind"),
                &current.to_string(),
            )?
            .parse::<SocketAddr>()
            .map_err(|_| ApplicationError::InvalidSetupValue("invalid listener address"))?;
            match &mut transport.adapter {
                TransportAdapterConfig::Telnet { listen, .. }
                | TransportAdapterConfig::Raw { listen, .. }
                | TransportAdapterConfig::Rlogin { listen, .. }
                | TransportAdapterConfig::Ssh { listen, .. } => *listen = address,
                _ => {}
            }
        }
    }
    Ok(())
}

fn edit_caller_defaults(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    config: &mut RuntimeConfig,
) -> Result<(), ApplicationError> {
    config.caller.new_caller_security = prompt_u16(
        input,
        output,
        &op("operator-setup-new-caller-security"),
        config.caller.new_caller_security,
    )?;
    config.caller.sysop_security = prompt_u16(
        input,
        output,
        &op("operator-setup-sysop-threshold"),
        config.caller.sysop_security,
    )?;
    config.caller.minutes_per_call = prompt_u32(
        input,
        output,
        &op("operator-setup-minutes-call"),
        config.caller.minutes_per_call,
    )?;
    config.caller.minutes_per_day = prompt_u32(
        input,
        output,
        &op("operator-setup-minutes-day"),
        config.caller.minutes_per_day,
    )?;
    config.caller.new_caller_first_day_minutes = prompt_u32(
        input,
        output,
        &op("operator-setup-new-caller-minutes"),
        config.caller.new_caller_first_day_minutes,
    )?;
    config.caller.maximum_daily_calls = prompt_u32(
        input,
        output,
        &op("operator-setup-calls-day"),
        config.caller.maximum_daily_calls,
    )?;
    config.caller.inactivity_minutes = prompt_u32(
        input,
        output,
        &op("operator-setup-inactivity"),
        config.caller.inactivity_minutes,
    )?;
    let journey = prompt(
        input,
        output,
        &op("operator-setup-post-login"),
        match config.caller.post_login_journey {
            PostLoginJourney::None => "none",
            PostLoginJourney::Stock => "stock",
        },
    )?;
    config.caller.post_login_journey = match journey.to_ascii_lowercase().as_str() {
        "none" => PostLoginJourney::None,
        "stock" => PostLoginJourney::Stock,
        _ => {
            return Err(ApplicationError::InvalidSetupValue(
                "post-login journey must be none or stock",
            ));
        }
    };
    config.caller.profile.address = prompt_profile_policy(
        input,
        output,
        &op("operator-setup-address-policy"),
        config.caller.profile.address,
    )?;
    config.caller.profile.phone = prompt_profile_policy(
        input,
        output,
        &op("operator-setup-phone-policy"),
        config.caller.profile.phone,
    )?;
    config.caller.profile.email = prompt_profile_policy(
        input,
        output,
        &op("operator-setup-email-policy"),
        config.caller.profile.email,
    )?;
    config.caller.profile.birthday = prompt_profile_policy(
        input,
        output,
        &op("operator-setup-birthday-policy"),
        config.caller.profile.birthday,
    )?;
    Ok(())
}

fn prompt_profile_policy(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
    current: ProfileFieldPolicy,
) -> Result<ProfileFieldPolicy, ApplicationError> {
    let default = match current {
        ProfileFieldPolicy::Disabled => "disabled",
        ProfileFieldPolicy::Optional => "optional",
        ProfileFieldPolicy::Required => "required",
    };
    match prompt(
        input,
        output,
        &op_args(
            "operator-setup-policy-options",
            sf_core::LocalizationArgs::new().with("label", label),
        ),
        default,
    )?
    .to_ascii_lowercase()
    .as_str()
    {
        "disabled" | "d" => Ok(ProfileFieldPolicy::Disabled),
        "optional" | "o" => Ok(ProfileFieldPolicy::Optional),
        "required" | "r" => Ok(ProfileFieldPolicy::Required),
        _ => Err(ApplicationError::InvalidSetupValue(
            "profile policy must be disabled, optional, or required",
        )),
    }
}

fn edit_conferences(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    admin: &BoardAdmin,
) -> Result<(), ApplicationError> {
    let conferences = admin.conferences()?;
    writeln!(output, "\n{}", op("operator-config-conferences-title"))
        .map_err(ApplicationError::SetupIo)?;
    for conference in &conferences {
        writeln!(
            output,
            "{}",
            op_args(
                "operator-config-conference-row",
                sf_core::LocalizationArgs::new()
                    .with("number", conference.number)
                    .with("name", conference.name.as_str())
                    .with("active", conference.active.to_string())
                    .with("read", conference.read_security.get())
                    .with("post", conference.post_security.get())
                    .with("mode", format!("{:?}", conference.access_mode))
            )
        )
        .map_err(ApplicationError::SetupIo)?;
    }
    match prompt(input, output, &op("operator-config-edit-actions"), "")?
        .to_ascii_uppercase()
        .as_str()
    {
        "A" => {
            let definition = prompt_conference(input, output, None)?;
            admin.create_conference(&definition)?;
        }
        "E" => {
            let number = prompt_u16(input, output, &op("operator-config-conference-number"), 1)?;
            let current = conferences
                .iter()
                .find(|conference| conference.number == number)
                .ok_or(ApplicationError::InvalidSetupValue("unknown conference"))?;
            let definition = prompt_conference(input, output, Some(current))?;
            admin.update_conference(number, &definition)?;
        }
        "T" => {
            let number = prompt_u16(input, output, &op("operator-config-conference-number"), 1)?;
            let current = conferences
                .iter()
                .find(|conference| conference.number == number)
                .ok_or(ApplicationError::InvalidSetupValue("unknown conference"))?;
            admin.set_conference_enabled(number, !current.active)?;
        }
        _ => {}
    }
    Ok(())
}

fn prompt_conference(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    current: Option<&Conference>,
) -> Result<ConferenceDefinition, ApplicationError> {
    let number = current.map_or(1, |value| value.number);
    let number = prompt_u16(
        input,
        output,
        &op("operator-config-conference-number"),
        number,
    )?;
    let name = prompt(
        input,
        output,
        &op("operator-config-conference-name"),
        current.map_or("Untitled Message Conference", |value| &value.name),
    )?;
    let description = prompt(
        input,
        output,
        &op("operator-config-description"),
        current.map_or("Message conference", |value| &value.description),
    )?;
    let read = prompt_u16(
        input,
        output,
        &op("operator-config-read-security"),
        current.map_or(5, |value| value.read_security.get()),
    )?;
    let post = prompt_u16(
        input,
        output,
        &op("operator-config-post-security"),
        current.map_or(5, |value| value.post_security.get()),
    )?;
    let exact = prompt_yes_no(
        input,
        output,
        &op("operator-config-exact-read"),
        current.is_some_and(|value| value.access_mode == ConferenceAccessMode::Exact),
    )?;
    let public_only = prompt_yes_no(
        input,
        output,
        &op("operator-config-public-only"),
        current.is_some_and(|value| value.public_only),
    )?;
    let caller_deletion_enabled = prompt_yes_no(
        input,
        output,
        &op("operator-config-caller-message-deletion"),
        current.is_none_or(|value| value.caller_deletion_enabled),
    )?;
    let maximum_lines = prompt_u16(
        input,
        output,
        &op("operator-config-maximum-lines"),
        current.map_or(50, |value| value.maximum_lines),
    )?;
    Ok(ConferenceDefinition {
        number,
        name,
        description,
        access_mode: if exact {
            ConferenceAccessMode::Exact
        } else {
            ConferenceAccessMode::AtLeast
        },
        read_security: SecurityLevel::new(read).map_err(sf_core::DatabaseError::from)?,
        post_security: SecurityLevel::new(post).map_err(sf_core::DatabaseError::from)?,
        public_only,
        caller_deletion_enabled,
        maximum_lines,
        privileged_security_levels: current
            .map_or_else(Vec::new, |value| value.privileged_security_levels.clone()),
    })
}

fn prompt(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
    default: &str,
) -> Result<String, ApplicationError> {
    let return_label = op("operator-prompt-return");
    write!(
        output,
        "{label} [{}]: ",
        if default.is_empty() {
            return_label.as_str()
        } else {
            default
        }
    )
    .map_err(ApplicationError::SetupIo)?;
    output.flush().map_err(ApplicationError::SetupIo)?;
    let mut line = String::new();
    input
        .read_line(&mut line)
        .map_err(ApplicationError::SetupIo)?;
    let value = line.trim();
    Ok(if value.is_empty() {
        default.to_owned()
    } else {
        value.to_owned()
    })
}

fn prompt_yes_no(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
    default: bool,
) -> Result<bool, ApplicationError> {
    loop {
        let value = prompt(input, output, label, if default { "yes" } else { "no" })?;
        match value.to_ascii_lowercase().as_str() {
            "yes" | "y" => return Ok(true),
            "no" | "n" => return Ok(false),
            _ => {
                writeln!(output, "{}", op("operator-setup-yes-no-invalid"))
                    .map_err(ApplicationError::SetupIo)?;
            }
        }
    }
}

fn op(key: &str) -> String {
    sf_core::text(key, &sf_core::LocalizationArgs::new())
}

fn op_args(key: &str, args: sf_core::LocalizationArgs) -> String {
    sf_core::text(key, &args)
}

fn prompt_u32(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
    default: u32,
) -> Result<u32, ApplicationError> {
    prompt(input, output, label, &default.to_string())?
        .parse()
        .map_err(|_| ApplicationError::InvalidSetupValue("expected an unsigned integer"))
}

fn prompt_u16(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
    default: u16,
) -> Result<u16, ApplicationError> {
    prompt(input, output, label, &default.to_string())?
        .parse()
        .map_err(|_| ApplicationError::InvalidSetupValue("expected a 16-bit unsigned integer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{setup_board, SetupPlan};
    use sf_core::{NetworkTerminalDefaults, TransportConfig};

    fn installed_board() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        let mut plan = SetupPlan::stock_defaults("Admin Board", "Admin Sysop", "Sysop", 2);
        plan.config.caller.password.memory_kib = 8;
        plan.config.caller.password.iterations = 1;
        setup_board(&root, &plan, b"test-only setup password").unwrap();
        (temp, root.join("spitfire.toml"))
    }

    #[test]
    fn static_configuration_round_trip_edits_nodes_listeners_and_caller_defaults() {
        let (_temp, config_path) = installed_board();
        let mut admin = BoardAdmin::load(&config_path).unwrap();
        let mut replacement = admin.config().clone();
        replacement.board.name = "Edited Board".to_owned();
        replacement.nodes.as_mut().unwrap().count = 4;
        replacement.caller.new_caller_security = 20;
        if let TransportAdapterConfig::Telnet { listen, .. } =
            &mut replacement.transports[0].adapter
        {
            *listen = "127.0.0.1:4323".parse().unwrap();
        }
        admin.save_static(replacement.clone()).unwrap();
        replacement.revision += 1;
        assert_eq!(RuntimeConfig::load(&config_path).unwrap(), replacement);
        drop(admin);
        assert_eq!(
            BoardAdmin::load(&config_path).unwrap().config().board.name,
            "Edited Board"
        );
    }

    #[test]
    fn configuration_rejects_listener_conflicts_and_allows_same_type_on_distinct_ports() {
        let (_temp, config_path) = installed_board();
        let mut admin = BoardAdmin::load(&config_path).unwrap();
        let mut valid = admin.config().clone();
        valid.transports.push(TransportConfig {
            name: Some("telnet-secondary".to_owned()),
            enabled: true,
            adapter: TransportAdapterConfig::Telnet {
                listen: "127.0.0.1:3323".parse().unwrap(),
                terminal: NetworkTerminalDefaults::default(),
            },
        });
        admin.save_static(valid.clone()).unwrap();
        let mut invalid = valid;
        if let TransportAdapterConfig::Telnet { listen, .. } =
            &mut invalid.transports.last_mut().unwrap().adapter
        {
            *listen = "127.0.0.1:2323".parse().unwrap();
        }
        assert!(admin.save_static(invalid).is_err());
    }

    #[test]
    fn operator_boolean_prompts_reprompt_and_disabled_services_skip_bind() {
        let mut config = SetupPlan::stock_defaults("Prompt", "Sysop", "Sysop", 1).config;
        let mut input = std::io::Cursor::new(b"1\nmaybe\nN\n".to_vec());
        let mut output = Vec::new();
        edit_terminal_services(&mut input, &mut output, &mut config).unwrap();
        assert!(!config.transports[0].enabled);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Please enter yes/y or no/n."));
        assert!(!output.contains("Bind address and port"));
    }

    #[test]
    fn conference_edits_preserve_identity_and_messages_and_disable_safely() {
        let (_temp, config_path) = installed_board();
        let admin = BoardAdmin::load(&config_path).unwrap();
        let original = admin.conferences().unwrap().remove(0);
        let definition = ConferenceDefinition {
            number: original.number,
            name: "General Discussion".to_owned(),
            description: "Edited without deleting messages".to_owned(),
            access_mode: ConferenceAccessMode::Exact,
            read_security: SecurityLevel::new(5).unwrap(),
            post_security: SecurityLevel::new(10).unwrap(),
            public_only: false,
            caller_deletion_enabled: false,
            maximum_lines: 75,
            privileged_security_levels: vec![SecurityLevel::new(20).unwrap()],
        };
        let edited = admin.update_conference(1, &definition).unwrap();
        assert_eq!(edited.id, original.id);
        assert!(!edited.caller_deletion_enabled);
        assert!(admin.set_conference_enabled(1, false).is_err());
        let second = admin.conferences().unwrap().remove(1);
        admin.set_conference_enabled(2, false).unwrap();
        let disabled = admin.conferences().unwrap().remove(1);
        assert_eq!(disabled.id, second.id);
        assert!(!disabled.active);

        let created = admin
            .create_conference(&ConferenceDefinition {
                number: 3,
                name: "Verification Conference".to_owned(),
                description: "Clean-board conference acceptance".to_owned(),
                access_mode: ConferenceAccessMode::Exact,
                read_security: SecurityLevel::new(30).unwrap(),
                post_security: SecurityLevel::new(40).unwrap(),
                public_only: true,
                caller_deletion_enabled: true,
                maximum_lines: 99,
                privileged_security_levels: vec![SecurityLevel::new(20).unwrap()],
            })
            .unwrap();
        drop(admin);

        let reopened = BoardAdmin::load(&config_path).unwrap();
        let persisted = reopened
            .conferences()
            .unwrap()
            .into_iter()
            .find(|conference| conference.number == 3)
            .unwrap();
        assert_eq!(persisted.id, created.id);
        assert_eq!(persisted.name, "Verification Conference");
        assert_eq!(persisted.access_mode, ConferenceAccessMode::Exact);
        assert_eq!(persisted.read_security.get(), 30);
        assert_eq!(persisted.post_security.get(), 40);
        assert!(persisted.public_only);
        assert_eq!(persisted.maximum_lines, 99);
        assert_eq!(
            persisted
                .privileged_security_levels
                .iter()
                .map(|level| level.get())
                .collect::<Vec<_>>(),
            vec![20]
        );
    }

    #[test]
    fn file_area_administration_preserves_identity_files_and_storage() {
        let (_temp, config_path) = installed_board();
        let admin = BoardAdmin::load(&config_path).unwrap();
        let original = admin.file_areas().unwrap().remove(0);
        assert_eq!(admin.file_count(original.id).unwrap(), 1);
        let definition = FileAreaDefinition {
            number: original.number,
            name: "General Downloads".to_owned(),
            description: "Edited without deleting catalog entries".to_owned(),
            storage_key: original.storage_key.clone(),
            access_mode: FileAccessMode::Exact,
            read_security: SecurityLevel::new(10).unwrap(),
            upload_security: SecurityLevel::new(20).unwrap(),
            preview: true,
            no_charge: true,
            maximum_upload_bytes: 2 * 1024 * 1024,
            privileged_security_levels: vec![SecurityLevel::new(50).unwrap()],
        };
        let edited = admin.update_file_area(1, &definition).unwrap();
        assert_eq!(edited.id, original.id);
        assert_eq!(admin.file_count(edited.id).unwrap(), 1);
        admin.set_file_area_enabled(1, false).unwrap();
        let disabled = admin.file_areas().unwrap().remove(0);
        assert_eq!(disabled.id, original.id);
        assert!(!disabled.active);
        assert_eq!(admin.file_count(disabled.id).unwrap(), 1);

        let created = admin
            .create_file_area(&FileAreaDefinition {
                number: 3,
                name: "Uploads".to_owned(),
                description: "Caller upload area".to_owned(),
                storage_key: "uploads".to_owned(),
                access_mode: FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                upload_security: SecurityLevel::new(10).unwrap(),
                preview: false,
                no_charge: false,
                maximum_upload_bytes: 1024 * 1024,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        assert_eq!(created.number, 3);
        assert!(admin
            .paths
            .get(sf_core::LogicalPath::External)
            .join("files/uploads")
            .is_dir());
    }
}
