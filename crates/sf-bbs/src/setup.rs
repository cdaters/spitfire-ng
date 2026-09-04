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

use std::fs;
use std::io::{self, BufRead, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use sf_core::{
    BoardAccessMode, BoardConfig, CallerConfig, CallerState, ConferenceAccessMode,
    ConferenceDefinition, CredentialHasher, FileAccessMode, FileAreaDefinition, FileStorage,
    LanguageConfig, LocalOperatorIdentity, LogicalPaths, MenuPresentationMode,
    NetworkTerminalDefaults, NodePoolConfig, OperatorConfig, PathConfig, PostLoginJourney,
    PresentationConfig, ProfileFieldPolicy, RuntimeConfig, RuntimeDatabase, SecurityLevel,
    StorageConfig, TransportAdapterConfig, TransportConfig, CONFIG_FORMAT_VERSION,
};

use crate::fixture::write_default_resources;
use crate::ApplicationError;

pub const BOARD_CONFIG_FILE: &str = "spitfire.toml";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupPlan {
    pub config: RuntimeConfig,
    /// Security assigned to the initial caller record. This remains separate
    /// from the configured threshold that grants Sysop authority.
    pub initial_sysop_security: u16,
    pub conferences: Vec<ConferenceDefinition>,
    pub file_areas: Vec<FileAreaDefinition>,
}

impl SetupPlan {
    pub fn stock_defaults(
        board_name: impl Into<String>,
        sysop_display_name: impl Into<String>,
        sysop_caller_name: impl Into<String>,
        node_count: u32,
    ) -> Self {
        let caller = CallerConfig {
            sysop_caller_name: sysop_caller_name.into(),
            ..CallerConfig::default()
        };
        let initial_sysop_security = caller.sysop_security;
        let config = RuntimeConfig {
            format_version: CONFIG_FORMAT_VERSION,
            board: BoardConfig {
                name: board_name.into(),
                sysop: sysop_display_name.into(),
                timezone: "UTC".to_owned(),
                access: BoardAccessMode::Public,
                private_security_level: 0,
            },
            node: None,
            nodes: Some(NodePoolConfig {
                count: node_count,
                overrides: Vec::new(),
            }),
            paths: PathConfig {
                system: PathBuf::from("system"),
                work: PathBuf::from("work"),
                display: PathBuf::from("display"),
                message: PathBuf::from("message"),
                external: PathBuf::from("external"),
            },
            storage: StorageConfig {
                database_file: PathBuf::from("spitfire-ng.sqlite3"),
            },
            presentation: PresentationConfig::modern_default(),
            language: LanguageConfig::default(),
            caller,
            transports: default_network_transports(),
            operators: OperatorConfig::default(),
        };
        let read = SecurityLevel::new(5).expect("stock setup security is valid");
        let post = SecurityLevel::new(5).expect("stock setup security is valid");
        Self {
            initial_sysop_security,
            config,
            conferences: vec![
                ConferenceDefinition {
                    number: 1,
                    name: "General".to_owned(),
                    description: "General board discussion".to_owned(),
                    access_mode: ConferenceAccessMode::AtLeast,
                    read_security: read,
                    post_security: post,
                    public_only: false,
                    caller_deletion_enabled: true,
                    maximum_lines: 50,
                    privileged_security_levels: Vec::new(),
                },
                ConferenceDefinition {
                    number: 2,
                    name: "SPITFIRE".to_owned(),
                    description: "SPITFIRE board discussion".to_owned(),
                    access_mode: ConferenceAccessMode::AtLeast,
                    read_security: read,
                    post_security: post,
                    public_only: false,
                    caller_deletion_enabled: true,
                    maximum_lines: 50,
                    privileged_security_levels: Vec::new(),
                },
            ],
            file_areas: vec![
                FileAreaDefinition {
                    number: 1,
                    name: "General Files".to_owned(),
                    description: "General files for this SPITFIRE board".to_owned(),
                    storage_key: "general".to_owned(),
                    access_mode: FileAccessMode::AtLeast,
                    read_security: read,
                    upload_security: post,
                    preview: false,
                    no_charge: false,
                    maximum_upload_bytes: 10 * 1024 * 1024,
                    privileged_security_levels: Vec::new(),
                },
                FileAreaDefinition {
                    number: 2,
                    name: "SPITFIRE Files".to_owned(),
                    description: "SPITFIRE NG board information".to_owned(),
                    storage_key: "spitfire".to_owned(),
                    access_mode: FileAccessMode::AtLeast,
                    read_security: read,
                    upload_security: post,
                    preview: false,
                    no_charge: false,
                    maximum_upload_bytes: 10 * 1024 * 1024,
                    privileged_security_levels: Vec::new(),
                },
            ],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupReport {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub database_path: PathBuf,
    pub schema_version: u32,
    pub node_count: usize,
    pub conference_count: usize,
    pub file_area_count: usize,
    pub sysop_caller_name: String,
}

pub fn setup_board(
    root: &Path,
    plan: &SetupPlan,
    sysop_password: &[u8],
) -> Result<SetupReport, ApplicationError> {
    if root.exists() {
        return Err(ApplicationError::SetupExists(root.to_path_buf()));
    }
    let validated = plan.config.validate()?;
    if plan.initial_sysop_security > 9_999
        || plan.initial_sysop_security < validated.caller.sysop_security
    {
        return Err(ApplicationError::InvalidSetupValue(
            "initial Sysop security must be in 0..=9999 and at least the configured Sysop threshold",
        ));
    }
    validate_sysop_password_pair(sysop_password, sysop_password, &validated.caller)?;
    let mut numbers = std::collections::HashSet::new();
    for conference in &plan.conferences {
        conference.validate()?;
        if !numbers.insert(conference.number) {
            return Err(ApplicationError::DuplicateSetupConference(
                conference.number,
            ));
        }
    }
    let mut area_numbers = std::collections::HashSet::new();
    let mut storage_keys = std::collections::HashSet::new();
    for area in &plan.file_areas {
        area.validate()?;
        if !area_numbers.insert(area.number) {
            return Err(ApplicationError::DuplicateSetupFileArea(area.number));
        }
        if !storage_keys.insert(area.storage_key.to_ascii_lowercase()) {
            return Err(ApplicationError::DuplicateSetupStorageKey(
                area.storage_key.clone(),
            ));
        }
    }
    let hasher = CredentialHasher::new(&validated.caller.password)?;
    let encoded_sysop_password = hasher.hash(sysop_password)?;

    if let Some(parent) = root.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|source| ApplicationError::CreateSetupDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir(root).map_err(|source| ApplicationError::CreateSetupDirectory {
        path: root.to_path_buf(),
        source,
    })?;

    let paths = LogicalPaths::resolve(root, &validated)?;
    paths.create_directories()?;
    let config_path = root.join(BOARD_CONFIG_FILE);
    let mut stored_config = plan.config.clone();
    #[cfg(unix)]
    if stored_config.operators.local_identities.is_empty() {
        use std::os::unix::fs::MetadataExt;
        stored_config
            .operators
            .local_identities
            .push(LocalOperatorIdentity::Unix {
                uid: fs::metadata(root)
                    .map_err(|source| ApplicationError::CreateSetupDirectory {
                        path: root.to_path_buf(),
                        source,
                    })?
                    .uid(),
                label: Some("board creator".to_owned()),
                capabilities: vec![
                    sf_core::LocalOperatorCapability::BoardStatistics,
                    sf_core::LocalOperatorCapability::NodeStatus,
                    sf_core::LocalOperatorCapability::OperationalEvents,
                    sf_core::LocalOperatorCapability::CallerActivity,
                    sf_core::LocalOperatorCapability::Notifications,
                    sf_core::LocalOperatorCapability::MaintenanceStatus,
                ],
            });
    }
    #[cfg(windows)]
    if stored_config.operators.local_identities.is_empty() {
        stored_config
            .operators
            .local_identities
            .push(LocalOperatorIdentity::Windows {
                sid: crate::operator_control::windows_current_process_sid()?,
                label: Some("board creator".to_owned()),
                capabilities: vec![
                    sf_core::LocalOperatorCapability::BoardStatistics,
                    sf_core::LocalOperatorCapability::NodeStatus,
                    sf_core::LocalOperatorCapability::OperationalEvents,
                    sf_core::LocalOperatorCapability::CallerActivity,
                    sf_core::LocalOperatorCapability::Notifications,
                    sf_core::LocalOperatorCapability::MaintenanceStatus,
                ],
            });
    }
    stored_config.save_atomic(&config_path)?;
    write_default_resources(&paths, false)?;

    let mut database = RuntimeDatabase::open(paths.database())?;
    database.migrate()?;
    database.ensure_board_identity(&validated.identity)?;
    for conference in &plan.conferences {
        database.create_conference(conference)?;
    }
    let storage = FileStorage::new(&paths)?;
    let mut areas = Vec::new();
    for area in &plan.file_areas {
        let stored = database.create_file_area(area)?;
        storage.ensure_area(&stored)?;
        areas.push(stored);
    }
    crate::fixture::seed_starter_files(&mut database, &storage, &areas)?;
    database.create_caller(
        validated.caller.sysop_caller_name.as_bytes(),
        &encoded_sysop_password,
        SecurityLevel::new(plan.initial_sysop_security).map_err(sf_core::DatabaseError::from)?,
        CallerState::Active,
        false,
        current_unix_seconds()?,
    )?;

    Ok(SetupReport {
        root: root.to_path_buf(),
        config_path,
        database_path: paths.database().to_path_buf(),
        schema_version: database.schema_version()?,
        node_count: validated.nodes.len(),
        conference_count: plan.conferences.len(),
        file_area_count: plan.file_areas.len(),
        sysop_caller_name: validated.caller.sysop_caller_name,
    })
}

pub fn interactive_setup(root: &Path) -> Result<SetupReport, ApplicationError> {
    if root.exists() {
        return Err(ApplicationError::SetupExists(root.to_path_buf()));
    }
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout();
    writeln!(
        output,
        "{}\n{}",
        operator_text("operator-setup-title"),
        operator_text("operator-setup-profile-summary")
    )
    .map_err(ApplicationError::SetupIo)?;
    let board = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-board-name"),
        "My SPITFIRE BBS",
    )?;
    let sysop = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-sysop-display-name"),
        "Sysop",
    )?;
    let sysop_caller = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-sysop-caller-name"),
        "Sysop",
    )?;
    let node_count = prompt_u32(
        &mut input,
        &mut output,
        &operator_text("operator-setup-node-count"),
        4,
    )?;
    let mut plan = SetupPlan::stock_defaults(board, sysop, sysop_caller, node_count);
    writeln!(output, "{}", operator_text("operator-setup-timezone-help"))
        .map_err(ApplicationError::SetupIo)?;
    plan.config.board.timezone = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-timezone"),
        &plan.config.board.timezone,
    )?;
    plan.config.language.default_locale = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-default-locale"),
        &plan.config.language.default_locale,
    )?;
    let access = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-board-access"),
        "public",
    )?;
    plan.config.board.access = if access.eq_ignore_ascii_case("private") {
        BoardAccessMode::Private
    } else {
        BoardAccessMode::Public
    };
    if plan.config.board.access.is_private() {
        plan.config.board.private_security_level = prompt_u16(
            &mut input,
            &mut output,
            &operator_text("operator-setup-private-security"),
            plan.config.caller.new_caller_security,
        )?;
    }
    configure_default_listener(
        &mut input,
        &mut output,
        &mut plan.config.transports[0],
        "Telnet",
    )?;
    configure_default_listener(
        &mut input,
        &mut output,
        &mut plan.config.transports[1],
        "RAW TCP",
    )?;
    configure_default_listener(
        &mut input,
        &mut output,
        &mut plan.config.transports[2],
        "RLogin",
    )?;
    configure_default_listener(
        &mut input,
        &mut output,
        &mut plan.config.transports[3],
        "SSH caller access",
    )?;
    writeln!(
        output,
        "{}",
        operator_text("operator-setup-presentation-help")
    )
    .map_err(ApplicationError::SetupIo)?;
    let preset = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-experience-preset"),
        "modern",
    )?;
    apply_caller_experience_preset(&mut plan.config, &preset)?;
    plan.config.presentation.active_profile = Some(prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-active-profile"),
        plan.config
            .presentation
            .active_profile
            .as_deref()
            .unwrap_or("modern-ng"),
    )?);
    plan.config.presentation.base_profile = Some(prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-base-profile"),
        plan.config
            .presentation
            .base_profile
            .as_deref()
            .unwrap_or("modern-ng"),
    )?);
    let menu_mode = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-menu-presentation"),
        match plan.config.presentation.menu_mode {
            MenuPresentationMode::DisplayOverrides => "display-overrides",
            MenuPresentationMode::Generated => "generated",
        },
    )?;
    plan.config.presentation.menu_mode = parse_menu_mode(&menu_mode)?;
    let journey = prompt(
        &mut input,
        &mut output,
        &operator_text("operator-setup-post-login"),
        match plan.config.caller.post_login_journey {
            PostLoginJourney::None => "none",
            PostLoginJourney::Stock => "stock",
        },
    )?;
    plan.config.caller.post_login_journey = parse_post_login_journey(&journey)?;
    plan.config.caller.new_caller_security = prompt_u16(
        &mut input,
        &mut output,
        &operator_text("operator-setup-new-caller-security"),
        plan.config.caller.new_caller_security,
    )?;
    plan.config.caller.sysop_security = prompt_u16(
        &mut input,
        &mut output,
        &operator_text("operator-setup-sysop-threshold"),
        plan.config.caller.sysop_security,
    )?;
    plan.initial_sysop_security = prompt_u16(
        &mut input,
        &mut output,
        &operator_text("operator-setup-initial-sysop-security"),
        plan.config.caller.sysop_security,
    )?;
    plan.config.caller.minutes_per_call = prompt_u32(
        &mut input,
        &mut output,
        &operator_text("operator-setup-minutes-call"),
        plan.config.caller.minutes_per_call,
    )?;
    plan.config.caller.minutes_per_day = prompt_u32(
        &mut input,
        &mut output,
        &operator_text("operator-setup-minutes-day"),
        plan.config.caller.minutes_per_day,
    )?;
    plan.config.caller.new_caller_first_day_minutes = prompt_u32(
        &mut input,
        &mut output,
        &operator_text("operator-setup-new-caller-minutes"),
        plan.config.caller.new_caller_first_day_minutes,
    )?;
    plan.config.caller.maximum_daily_calls = prompt_u32(
        &mut input,
        &mut output,
        &operator_text("operator-setup-calls-day"),
        plan.config.caller.maximum_daily_calls,
    )?;
    plan.config.caller.inactivity_minutes = prompt_u32(
        &mut input,
        &mut output,
        &operator_text("operator-setup-inactivity"),
        plan.config.caller.inactivity_minutes,
    )?;
    plan.config.caller.profile.address = prompt_profile_policy(
        &mut input,
        &mut output,
        &operator_text("operator-setup-address-policy"),
        plan.config.caller.profile.address,
    )?;
    plan.config.caller.profile.phone = prompt_profile_policy(
        &mut input,
        &mut output,
        &operator_text("operator-setup-phone-policy"),
        plan.config.caller.profile.phone,
    )?;
    plan.config.caller.profile.email = prompt_profile_policy(
        &mut input,
        &mut output,
        &operator_text("operator-setup-email-policy"),
        plan.config.caller.profile.email,
    )?;
    plan.config.caller.profile.birthday = prompt_profile_policy(
        &mut input,
        &mut output,
        &operator_text("operator-setup-birthday-policy"),
        plan.config.caller.profile.birthday,
    )?;
    plan.config.validate()?;

    let mut password = loop {
        let mut password = rpassword::prompt_password(operator_text("operator-setup-password"))
            .map_err(ApplicationError::PasswordPrompt)?
            .into_bytes();
        let mut confirmation =
            match rpassword::prompt_password(operator_text("operator-setup-password-confirm")) {
                Ok(value) => value.into_bytes(),
                Err(error) => {
                    password.fill(0);
                    return Err(ApplicationError::PasswordPrompt(error));
                }
            };
        match validate_sysop_password_pair(&password, &confirmation, &plan.config.caller) {
            Ok(()) => {
                confirmation.fill(0);
                break password;
            }
            Err(ApplicationError::InvalidSysopPasswordLength { minimum, maximum }) => {
                password.fill(0);
                confirmation.fill(0);
                writeln!(
                    output,
                    "{}",
                    sf_core::text(
                        "operator-setup-password-length",
                        &sf_core::LocalizationArgs::new()
                            .with("minimum", minimum as u64)
                            .with("maximum", maximum as u64),
                    )
                )
                .map_err(ApplicationError::SetupIo)?;
            }
            Err(ApplicationError::PasswordConfirmation) => {
                password.fill(0);
                confirmation.fill(0);
                writeln!(
                    output,
                    "{}",
                    operator_text("operator-setup-password-mismatch")
                )
                .map_err(ApplicationError::SetupIo)?;
            }
            Err(error) => {
                password.fill(0);
                confirmation.fill(0);
                return Err(error);
            }
        }
    };
    let report = setup_board(root, &plan, &password);
    password.fill(0);
    report
}

fn operator_text(key: &str) -> String {
    sf_core::text(key, &sf_core::LocalizationArgs::new())
}

fn apply_caller_experience_preset(
    config: &mut RuntimeConfig,
    value: &str,
) -> Result<(), ApplicationError> {
    config.presentation.mode = sf_core::PresentationMode::Profile;
    config.presentation.base_profile = Some("modern-ng".to_owned());
    config.presentation.menu_mode = MenuPresentationMode::DisplayOverrides;
    match value.to_ascii_lowercase().as_str() {
        "modern" => {
            config.presentation.active_profile = Some("modern-ng".to_owned());
            config.caller.post_login_journey = PostLoginJourney::None;
        }
        "classic" => {
            config.presentation.active_profile = Some("classic-spitfire".to_owned());
            config.caller.post_login_journey = PostLoginJourney::Stock;
        }
        "minimal" => {
            config.presentation.active_profile = Some("minimal-terminal".to_owned());
            config.caller.post_login_journey = PostLoginJourney::None;
        }
        "custom" => {}
        _ => {
            return Err(ApplicationError::InvalidSetupValue(
                "caller experience preset must be modern, classic, minimal, or custom",
            ))
        }
    }
    Ok(())
}

fn parse_menu_mode(value: &str) -> Result<MenuPresentationMode, ApplicationError> {
    match value.to_ascii_lowercase().as_str() {
        "display-overrides" => Ok(MenuPresentationMode::DisplayOverrides),
        "generated" => Ok(MenuPresentationMode::Generated),
        _ => Err(ApplicationError::InvalidSetupValue(
            "menu presentation must be display-overrides or generated",
        )),
    }
}

fn parse_post_login_journey(value: &str) -> Result<PostLoginJourney, ApplicationError> {
    match value.to_ascii_lowercase().as_str() {
        "none" => Ok(PostLoginJourney::None),
        "stock" => Ok(PostLoginJourney::Stock),
        _ => Err(ApplicationError::InvalidSetupValue(
            "post-login journey must be none or stock",
        )),
    }
}

fn validate_sysop_password_pair(
    password: &[u8],
    confirmation: &[u8],
    caller: &CallerConfig,
) -> Result<(), ApplicationError> {
    if password.len() < caller.minimum_password_length
        || password.len() > caller.maximum_password_length
    {
        return Err(ApplicationError::InvalidSysopPasswordLength {
            minimum: caller.minimum_password_length,
            maximum: caller.maximum_password_length,
        });
    }
    if password != confirmation {
        return Err(ApplicationError::PasswordConfirmation);
    }
    Ok(())
}

fn prompt_profile_policy(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
    current: ProfileFieldPolicy,
) -> Result<ProfileFieldPolicy, ApplicationError> {
    let current = match current {
        ProfileFieldPolicy::Disabled => "disabled",
        ProfileFieldPolicy::Optional => "optional",
        ProfileFieldPolicy::Required => "required",
    };
    match prompt(
        input,
        output,
        &sf_core::text(
            "operator-setup-policy-options",
            &sf_core::LocalizationArgs::new().with("label", label),
        ),
        current,
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

fn default_network_transports() -> Vec<TransportConfig> {
    let terminal = NetworkTerminalDefaults::default();
    vec![
        TransportConfig {
            name: Some("telnet".to_owned()),
            enabled: true,
            adapter: TransportAdapterConfig::Telnet {
                listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2323),
                terminal: terminal.clone(),
            },
        },
        TransportConfig {
            name: Some("raw".to_owned()),
            enabled: true,
            adapter: TransportAdapterConfig::Raw {
                listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2324),
                terminal: terminal.clone(),
            },
        },
        TransportConfig {
            name: Some("rlogin".to_owned()),
            enabled: true,
            adapter: TransportAdapterConfig::Rlogin {
                listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2513),
                auto_login: false,
                terminal,
            },
        },
        TransportConfig {
            name: Some("ssh".to_owned()),
            enabled: false,
            adapter: TransportAdapterConfig::Ssh {
                listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2222),
                host_key: PathBuf::from("ssh/host-ed25519"),
                terminal: NetworkTerminalDefaults {
                    ansi: true,
                    cp437: false,
                    width: 80,
                    height: 25,
                },
                maximum_unauthenticated_connections: 32,
                maximum_authentication_attempts: 3,
                handshake_timeout_seconds: 30,
            },
        },
    ]
}

fn configure_default_listener(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    transport: &mut TransportConfig,
    label: &str,
) -> Result<(), ApplicationError> {
    transport.enabled = prompt_yes_no(
        input,
        output,
        &sf_core::text(
            "operator-setup-enable-listener",
            &sf_core::LocalizationArgs::new().with("listener", label),
        ),
        transport.enabled,
    )?;
    if !transport.enabled {
        return Ok(());
    }
    let current = transport
        .network_listener()
        .expect("default setup transport is a listener");
    let bind = prompt(
        input,
        output,
        &sf_core::text(
            "operator-setup-listener-bind",
            &sf_core::LocalizationArgs::new().with("listener", label),
        ),
        &current.ip().to_string(),
    )?;
    let port = prompt_u16(
        input,
        output,
        &sf_core::text(
            "operator-setup-listener-port",
            &sf_core::LocalizationArgs::new().with("listener", label),
        ),
        current.port(),
    )?;
    let address = SocketAddr::new(
        bind.parse()
            .map_err(|_| ApplicationError::InvalidSetupValue("invalid bind address"))?,
        port,
    );
    match &mut transport.adapter {
        TransportAdapterConfig::Telnet { listen, .. }
        | TransportAdapterConfig::Raw { listen, .. }
        | TransportAdapterConfig::Rlogin { listen, .. }
        | TransportAdapterConfig::Ssh { listen, .. } => *listen = address,
        _ => {
            return Err(ApplicationError::InvalidSetupValue(
                "not a network listener",
            ))
        }
    }
    Ok(())
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
                writeln!(output, "{}", operator_text("operator-setup-yes-no-invalid"))
                    .map_err(ApplicationError::SetupIo)?;
            }
        }
    }
}

fn prompt(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
    default: &str,
) -> Result<String, ApplicationError> {
    write!(output, "{label} [{default}]: ").map_err(ApplicationError::SetupIo)?;
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

fn current_unix_seconds() -> Result<i64, ApplicationError> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApplicationError::Coordination("system clock is before the Unix epoch"))?
        .as_secs();
    i64::try_from(seconds)
        .map_err(|_| ApplicationError::Coordination("system clock value is too large"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::{MessageActor, MessageBackend};

    #[test]
    fn setup_creates_a_startable_nonproprietary_board() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        let mut plan = SetupPlan::stock_defaults("Setup Board", "Setup Sysop", "Sysop", 4);
        plan.config.caller.password.memory_kib = 8;
        plan.config.caller.password.iterations = 1;
        let report = setup_board(&root, &plan, b"test-only setup password").unwrap();
        assert_eq!(report.node_count, 4);
        assert_eq!(report.conference_count, 2);
        assert_eq!(report.file_area_count, 2);
        assert!(report.config_path.is_file());
        assert!(root
            .join("system/presentation-profiles/modern-ng/profile.toml")
            .is_file());
        assert!(root
            .join("system/presentation-profiles/modern-ng/GENERATED-RESOURCES.txt")
            .is_file());
        assert!(root
            .join("system/presentation-profiles/minimal-terminal/profile.toml")
            .is_file());
        assert!(root
            .join("system/presentation-profiles/minimal-terminal/GENERATED-RESOURCES.txt")
            .is_file());
        assert!(root
            .join("system/presentation-profiles/classic-spitfire/profile.toml")
            .is_file());
        assert!(root
            .join("system/presentation-profiles/classic-spitfire/LICENSES/ASSET-LICENSE.txt")
            .is_file());
        assert!(root
            .join("system/language-packs/en-US/language.toml")
            .is_file());

        let config = RuntimeConfig::load(&report.config_path).unwrap();
        let validated = config.validate().unwrap();
        assert_eq!(validated.nodes.len(), 4);
        assert_eq!(validated.language.default_locale, "en-US");
        assert_eq!(
            validated.presentation.active_profile.as_deref(),
            Some("modern-ng")
        );
        #[cfg(windows)]
        assert!(validated.operators.local_identities.iter().any(|identity| {
            matches!(
                identity,
                sf_core::LocalOperatorIdentity::Windows { sid, .. }
                    if sid == &crate::operator_control::windows_current_process_sid().unwrap()
            )
        }));
        let database = RuntimeDatabase::open(&report.database_path).unwrap();
        let sysop = database.caller_by_name(b"Sysop").unwrap().unwrap();
        let actor = MessageActor::new(
            sysop.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        assert_eq!(database.conferences(actor).unwrap().len(), 2);
        assert_eq!(database.all_file_areas().unwrap().len(), 2);
    }

    #[test]
    fn setup_uses_operator_selected_initial_sysop_security_above_threshold() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        let mut plan = SetupPlan::stock_defaults("Security Board", "Sysop", "Initial Sysop", 1);
        plan.config.caller.password.memory_kib = 8;
        plan.config.caller.password.iterations = 1;
        plan.config.caller.sysop_security = 100;
        plan.initial_sysop_security = 999;
        let report = setup_board(&root, &plan, b"test-only setup password").unwrap();
        let config = RuntimeConfig::load(&report.config_path).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database.caller_by_name(b"Initial Sysop").unwrap().unwrap();
        assert_eq!(validated.caller.sysop_security, 100);
        assert_eq!(caller.security_level.get(), 999);
    }

    #[test]
    fn setup_rejects_initial_sysop_below_configured_threshold_before_creation() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        let mut plan = SetupPlan::stock_defaults("Security Board", "Sysop", "Initial Sysop", 1);
        plan.config.caller.sysop_security = 100;
        plan.initial_sysop_security = 50;
        assert!(setup_board(&root, &plan, b"test-only setup password").is_err());
        assert!(!root.exists());
    }

    #[test]
    fn setup_presets_keep_presentation_and_journey_separate() {
        let mut config = SetupPlan::stock_defaults("Preset", "Sysop", "Sysop", 1).config;
        apply_caller_experience_preset(&mut config, "classic").unwrap();
        assert_eq!(
            config.presentation.active_profile.as_deref(),
            Some("classic-spitfire")
        );
        assert_eq!(
            config.presentation.base_profile.as_deref(),
            Some("modern-ng")
        );
        assert_eq!(config.caller.post_login_journey, PostLoginJourney::Stock);
        config.presentation.menu_mode = MenuPresentationMode::Generated;
        config.caller.post_login_journey = PostLoginJourney::None;
        assert_eq!(
            config.presentation.menu_mode,
            MenuPresentationMode::Generated
        );
        assert_eq!(config.caller.post_login_journey, PostLoginJourney::None);
    }

    #[test]
    fn disabled_listener_skips_endpoint_questions_and_yes_no_is_bounded() {
        let mut transport = default_network_transports().remove(0);
        let mut input = std::io::Cursor::new(b"N\n".to_vec());
        let mut output = Vec::new();
        configure_default_listener(&mut input, &mut output, &mut transport, "Telnet").unwrap();
        assert!(!transport.enabled);
        assert!(!String::from_utf8(output).unwrap().contains("bind address"));

        let mut transport = default_network_transports().remove(0);
        let mut input = std::io::Cursor::new(b"maybe\nY\n127.0.0.1\n2323\n".to_vec());
        let mut output = Vec::new();
        configure_default_listener(&mut input, &mut output, &mut transport, "Telnet").unwrap();
        assert!(transport.enabled);
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("Please enter yes/y or no/n."));
    }

    #[test]
    fn setup_validates_before_creating_and_never_overwrites() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        let mut invalid = SetupPlan::stock_defaults("", "Sysop", "Sysop", 4);
        invalid.config.caller.password.memory_kib = 8;
        assert!(setup_board(&root, &invalid, b"test-only setup password").is_err());
        assert!(!root.exists());

        let mut valid = SetupPlan::stock_defaults("Board", "Sysop", "Sysop", 4);
        valid.config.caller.password.memory_kib = 8;
        valid.config.caller.password.iterations = 1;
        setup_board(&root, &valid, b"test-only setup password").unwrap();
        assert!(matches!(
            setup_board(&root, &valid, b"test-only setup password"),
            Err(ApplicationError::SetupExists(_))
        ));
    }

    #[test]
    fn setup_password_validation_is_available_before_board_creation() {
        let caller = CallerConfig::default();
        assert!(matches!(
            validate_sysop_password_pair(b"short", b"short", &caller),
            Err(ApplicationError::InvalidSysopPasswordLength { .. })
        ));
        assert!(matches!(
            validate_sysop_password_pair(
                b"test-only valid setup password",
                b"test-only different password",
                &caller
            ),
            Err(ApplicationError::PasswordConfirmation)
        ));
        validate_sysop_password_pair(
            b"test-only valid setup password",
            b"test-only valid setup password",
            &caller,
        )
        .unwrap();
    }
}
