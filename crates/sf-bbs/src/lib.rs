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

//! Application orchestration for the native SPITFIRE NG runtime.

mod admin;
mod backup;
mod board_lock;
mod configuration;
pub use configuration::{
    configuration_version, current_operator_identity, ConfigurationDomainSummary,
    ConfigurationResult, ConfigurationSnapshot, OfflineConfiguration, SecretStatus,
};
mod error;
mod fixture;
mod live_control;
mod network_artifacts;
mod operator;
mod operator_control;
mod presentation;
mod resources;
pub use network_artifacts::DiskArtifactStore;
mod runtime;
mod setup;
mod shutdown;
mod status;
mod transports;

pub use admin::{interactive_config, BoardAdmin};
pub use backup::{
    backup_board, restore_board, BackupReport, BoardBackupError, RestoreReport,
    BACKUP_MANIFEST_FILE,
};
pub use error::ApplicationError;
pub use fixture::{initialize_fixture_board, FixtureReport, FIXTURE_CONFIG_FILE};
pub use live_control::{
    DisconnectPreflight, InteractionSnapshot, LiveControlAction, LiveControlResult,
    LiveSessionTarget, PendingCallerPage,
};
pub use operator::{run_operator_console, OperatorService};
pub use operator_control::{
    BoardStatusWire, ChatServerFrame, CommandReceiptWire, EventBatchWire, EventCursorWire,
    EventWire, MaintenanceWire, MutationResult, NodeStatusWire, NotificationWire,
    OperatorChatClient, OperatorClient, OperatorControlDescriptor, OperatorControlError,
    OperatorControlsWire, OperatorEventQuery, OperatorFeature, RecentCallerWire, StatisticsWire,
};
pub use presentation::{
    EngineCompatibility, FallbackPolicy, PresentationResolver, PresentationStatus,
    ProfileDescriptor, ProfileFormat, ProfileResourceKind, ProfileResourceRecord, ProvenanceKind,
    ProvenanceRecord, Redistribution, CLASSIC_PROFILE_ID, CLASSIC_PROFILE_VERSION,
    MINIMAL_PROFILE_ID, MINIMAL_PROFILE_VERSION, MODERN_PROFILE_ID, MODERN_PROFILE_VERSION,
    PROFILE_DESCRIPTOR, PROFILE_DIRECTORY, PROFILE_FORMAT_VERSION, RESOURCE_API_VERSION,
};
pub use runtime::{
    run_board, serve_board, serve_board_console, BoardRuntime, ConnectionReport, RunReport,
    ServeReport,
};
pub use setup::{interactive_setup, setup_board, SetupPlan, SetupReport, BOARD_CONFIG_FILE};
pub use shutdown::{ShutdownImpact, ShutdownPhase};
pub use status::{board_status, RuntimeStatusDocument, RUNTIME_STATUS_FILE};

use std::ffi::OsString;
use std::path::PathBuf;

use sf_core::InMemoryTerminal;

#[cfg(unix)]
use crate::transports::StdioTerminal;

/// Executes CLI behavior without exiting the process, keeping argument and
/// expected-error behavior integration-testable.
pub fn run_cli<I, S>(arguments: I) -> Result<String, ApplicationError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut arguments: Vec<OsString> = arguments.into_iter().map(Into::into).collect();
    let explicit_locale = if arguments.first().is_some_and(|value| value == "--locale") {
        if arguments.len() < 3 {
            return Err(ApplicationError::Usage(op("operator-usage")));
        }
        let locale = arguments[1]
            .to_str()
            .ok_or_else(|| ApplicationError::Usage(op("operator-usage")))?
            .to_owned();
        arguments.drain(0..2);
        Some(locale)
    } else {
        None
    };
    sf_core::bootstrap_locale(explicit_locale.as_deref())?;
    sf_core::with_localizer(sf_core::Localizer::embedded_en_us(), || {
        run_cli_inner(arguments)
    })
}

fn run_cli_inner(arguments: Vec<OsString>) -> Result<String, ApplicationError> {
    match arguments.as_slice() {
        [command] if command == "--version" || command == "-V" => Ok(op_args(
            "operator-version",
            sf_core::LocalizationArgs::new().with("version", sf_core::PRODUCT_VERSION),
        )),
        [command, package] if command == "language-validate" => {
            let package = sf_core::validate_language_package(&PathBuf::from(package))?;
            Ok(op_args(
                "operator-language-valid",
                sf_core::LocalizationArgs::new()
                    .with("locale", package.locale)
                    .with("version", package.package_version),
            ))
        }
        [command, config, package] if command == "language-install" => {
            let config_path = PathBuf::from(config).canonicalize().map_err(|source| {
                ApplicationError::ResolveConfiguration {
                    path: PathBuf::from(config),
                    source,
                }
            })?;
            let root = config_path
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .ok_or_else(|| ApplicationError::MissingBoardRoot(config_path.clone()))?;
            let _lock = board_lock::BoardOperationLock::acquire(root)?;
            let config = sf_core::RuntimeConfig::load(&config_path)?;
            let paths = sf_core::LogicalPaths::resolve(root, &config.validate()?)?;
            let package = sf_core::install_language_package(
                paths.get(sf_core::LogicalPath::System),
                &PathBuf::from(package),
            )?;
            Ok(op_args(
                "operator-language-installed",
                sf_core::LocalizationArgs::new()
                    .with("locale", package.locale)
                    .with("version", package.package_version)
                    .with("config", config_path.display().to_string()),
            ))
        }
        [command, output] if command == "setup" => {
            let report = interactive_setup(&PathBuf::from(output))?;
            Ok(op_args(
                "operator-setup-complete",
                sf_core::LocalizationArgs::new()
                    .with("config", report.config_path.display().to_string())
                    .with("database", report.database_path.display().to_string())
                    .with("schema", report.schema_version)
                    .with("nodes", report.node_count as u64)
                    .with("conferences", report.conference_count as u64)
                    .with("areas", report.file_area_count as u64)
                    .with("sysop", format!("{:?}", report.sysop_caller_name)),
            ))
        }
        [command, config] if command == "config" => interactive_config(&PathBuf::from(config)),
        [command, config] if command == "status" => board_status(&PathBuf::from(config)),
        [command, action, config] if command == "operator" => {
            run_operator_attach_cli(action.to_string_lossy().as_ref(), &PathBuf::from(config))
        }
        [command, config, destination] if command == "backup" => {
            let report = backup_board(&PathBuf::from(config), &PathBuf::from(destination))?;
            Ok(op_args(
                "operator-backup-complete",
                sf_core::LocalizationArgs::new()
                    .with("board", report.board_name)
                    .with("destination", report.destination.display().to_string())
                    .with("schema", report.schema_version)
                    .with("resources", report.resource_files as u64)
                    .with("files", report.cataloged_files as u64)
                    .with("bytes", report.total_bytes),
            ))
        }
        [command, backup, target] if command == "restore" => {
            let report = restore_board(&PathBuf::from(backup), &PathBuf::from(target), false)?;
            Ok(op_args(
                "operator-restore-complete",
                sf_core::LocalizationArgs::new()
                    .with("board", report.board_name)
                    .with("root", report.root.display().to_string())
                    .with("config", report.config_path.display().to_string())
                    .with("schema", report.schema_version)
                    .with("resources", report.resource_files as u64)
                    .with("files", report.cataloged_files as u64)
                    .with("replaced", "no"),
            ))
        }
        [command, backup, target, flag] if command == "restore" && flag == "--replace" => {
            let report = restore_board(&PathBuf::from(backup), &PathBuf::from(target), true)?;
            Ok(op_args(
                "operator-restore-complete",
                sf_core::LocalizationArgs::new()
                    .with("board", report.board_name)
                    .with("root", report.root.display().to_string())
                    .with("config", report.config_path.display().to_string())
                    .with("schema", report.schema_version)
                    .with("resources", report.resource_files as u64)
                    .with("files", report.cataloged_files as u64)
                    .with("replaced", "yes"),
            ))
        }
        [command, output] if command == "init-fixture" => {
            let output = PathBuf::from(output);
            let report = initialize_fixture_board(&output)?;
            Ok(op_args(
                "operator-fixture-complete",
                sf_core::LocalizationArgs::new()
                    .with("config", report.config_path.display().to_string())
                    .with("database", report.database_path.display().to_string())
                    .with("schema", report.schema_version),
            ))
        }
        [command, config] if command == "demo" => {
            let config = PathBuf::from(config);
            let runtime = BoardRuntime::load(&config)?;
            if !runtime.is_synthetic_fixture() {
                return Err(ApplicationError::DemoRequiresFixture);
            }
            let mut login = if runtime.caller_exists(b"Increment Two Demo")? {
                vec![
                    b"N".to_vec(),
                    b"Increment Two Demo".to_vec(),
                    b"test-only-demo-password".to_vec(),
                ]
            } else {
                vec![
                    b"Y".to_vec(),
                    b"Increment Two Demo".to_vec(),
                    b"test-only-demo-password".to_vec(),
                    b"test-only-demo-password".to_vec(),
                ]
            };
            login.extend([
                b"M".to_vec(),
                b"F".to_vec(),
                b"Q".to_vec(),
                b"?".to_vec(),
                b"M".to_vec(),
                b"G".to_vec(),
            ]);
            let mut terminal = InMemoryTerminal::with_lines(login);
            let report = match runtime.run_connection(&mut terminal)? {
                ConnectionReport::Completed(report) => report,
                ConnectionReport::NodeBusy => return Err(ApplicationError::Coordination(
                    "newly loaded demo runtime unexpectedly reported every configured node busy",
                )),
            };
            let transcript = terminal
                .output_text()
                .map_err(ApplicationError::InvalidTranscript)?;
            Ok(format!(
                "{transcript}\n{}",
                op_args(
                    "operator-demo-summary",
                    sf_core::LocalizationArgs::new()
                        .with("board", format!("{:?}", report.board_name))
                        .with("node", report.node_id)
                        .with("schema", report.schema_version)
                        .with("session", report.session_id)
                        .with("caller", format!("{:?}", report.caller_name))
                        .with("transport", format!("{:?}", report.transport))
                        .with("commands", report.commands_processed as u64)
                )
            ))
        }
        [command, config] if command == "init-sysop" => {
            let runtime = BoardRuntime::load(&PathBuf::from(config))?;
            let mut password = rpassword::prompt_password(op("operator-init-sysop-password"))
                .map_err(ApplicationError::PasswordPrompt)?
                .into_bytes();
            let mut confirmation =
                match rpassword::prompt_password(op("operator-setup-password-confirm")) {
                    Ok(confirmation) => confirmation.into_bytes(),
                    Err(error) => {
                        password.fill(0);
                        return Err(ApplicationError::PasswordPrompt(error));
                    }
                };
            if password != confirmation {
                password.fill(0);
                confirmation.fill(0);
                return Err(ApplicationError::PasswordConfirmation);
            }
            let caller = runtime.initialize_sysop(&password);
            password.fill(0);
            confirmation.fill(0);
            let caller = caller?;
            Ok(op_args(
                "operator-init-sysop-complete",
                sf_core::LocalizationArgs::new()
                    .with("caller", format!("{:?}", caller.display_name))
                    .with("security", caller.security_level.get()),
            ))
        }
        [command, config] if command == "shell" => {
            #[cfg(unix)]
            {
                let mut terminal = StdioTerminal::open();
                let report = run_board(&PathBuf::from(config), &mut terminal)?;
                Ok(op_args(
                    "operator-shell-ended",
                    sf_core::LocalizationArgs::new()
                        .with("session", report.session_id)
                        .with("reason", format!("{:?}", report.close_reason)),
                ))
            }
            #[cfg(not(unix))]
            {
                let _ = config;
                Err(ApplicationError::Transport(
                    "the shell adapter is available only on Unix-like hosts".to_owned(),
                ))
            }
        }
        [command, config] if command == "console" => {
            let report = serve_board_console(&PathBuf::from(config))?;
            Ok(op_args(
                "operator-console-ended",
                sf_core::LocalizationArgs::new().with("sessions", report.completed_sessions as u64),
            ))
        }
        [command, config] if command == "run" => {
            let report = serve_board(&PathBuf::from(config), None)?;
            Ok(op_args(
                "operator-listeners-ended",
                sf_core::LocalizationArgs::new().with("sessions", report.completed_sessions as u64),
            ))
        }
        [command, config, flag, count] if command == "run" && flag == "--max-sessions" => {
            let count = count
                .to_str()
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| ApplicationError::Usage(op("operator-usage")))?;
            let report = serve_board(&PathBuf::from(config), Some(count))?;
            Ok(op_args(
                "operator-listeners-ended",
                sf_core::LocalizationArgs::new().with("sessions", report.completed_sessions as u64),
            ))
        }
        _ => Err(ApplicationError::Usage(op("operator-usage"))),
    }
}

fn run_operator_attach_cli(
    action: &str,
    config: &std::path::Path,
) -> Result<String, ApplicationError> {
    if !matches!(
        action,
        "status"
            | "nodes"
            | "events"
            | "watch-events"
            | "notifications"
            | "statistics"
            | "callers"
            | "maintenance"
    ) {
        return Err(ApplicationError::Usage(op("operator-usage")));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ApplicationError::Transport(op("operator-client-start-failed")))?;
    runtime
        .block_on(async {
            let mut client = OperatorClient::connect(config).await?;
            let negotiated = format!("{}", client.features().len());
            let body = match action {
                "status" => {
                    let value = client.board_status().await?;
                    op_args(
                        "operator-attach-status",
                        sf_core::LocalizationArgs::new()
                            .with("board", value.board_name)
                            .with("schema", value.schema_version)
                            .with("uptime", value.uptime_seconds)
                            .with("nodes", value.active_nodes as u64)
                            .with("callers", value.callers_online as u64)
                            .with("features", negotiated),
                    )
                }
                "nodes" => {
                    let rows = client.nodes().await?;
                    if rows.is_empty() {
                        op("operator-attach-nodes-empty")
                    } else {
                        rows.into_iter()
                            .map(|node| {
                                op_args(
                                    "operator-attach-node-row",
                                    sf_core::LocalizationArgs::new()
                                        .with("node", node.node_id)
                                        .with("state", node.lifecycle)
                                        .with(
                                            "caller",
                                            node.public_handle.unwrap_or_else(|| "-".to_owned()),
                                        ),
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                }
                "events" | "watch-events" => {
                    let batch = if action == "watch-events" {
                        client.subscribe_events(4_000).await?
                    } else {
                        client.recent_events(100).await?
                    };
                    if batch.events.is_empty() {
                        op("operator-attach-events-empty")
                    } else {
                        let mut lines = batch
                            .events
                            .into_iter()
                            .map(|event| {
                                op_args(
                                    "operator-attach-event-row",
                                    sf_core::LocalizationArgs::new()
                                        .with("id", event.event_id)
                                        .with("severity", event.severity)
                                        .with("code", event.event_code),
                                )
                            })
                            .collect::<Vec<_>>();
                        if batch.gap_before_first {
                            lines.insert(0, op("operator-attach-event-gap"));
                        }
                        lines.join("\n")
                    }
                }
                "notifications" => {
                    let rows = client.notifications(false, 100).await?;
                    if rows.is_empty() {
                        op("operator-notifications-empty")
                    } else {
                        rows.into_iter()
                            .map(|item| {
                                op_args(
                                    "operator-attach-notification-row",
                                    sf_core::LocalizationArgs::new()
                                        .with("id", item.notification_id)
                                        .with("severity", item.severity)
                                        .with("reason", item.reason_key),
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                }
                "statistics" => {
                    let value = client.statistics().await?;
                    op_args(
                        "operator-attach-statistics",
                        sf_core::LocalizationArgs::new()
                            .with("day", i64::from(value.board_day))
                            .with("calls", value.calls_completed_today)
                            .with("messages", value.messages_posted_today)
                            .with("uploads", value.successful_uploads_today)
                            .with("downloads", value.successful_downloads_today)
                            .with("lifetime", value.lifetime_calls),
                    )
                }
                "callers" => {
                    let rows = client.recent_callers(100).await?;
                    if rows.is_empty() {
                        op("operator-recent-callers-empty")
                    } else {
                        rows.into_iter()
                            .map(|item| {
                                op_args(
                                    "operator-attach-caller-row",
                                    sf_core::LocalizationArgs::new()
                                        .with("caller", item.public_handle)
                                        .with("time", item.occurred_at_utc)
                                        .with("node", item.node_id.unwrap_or(0)),
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                }
                "maintenance" => {
                    let value = client.maintenance_status().await?;
                    op_args(
                        "operator-attach-maintenance",
                        sf_core::LocalizationArgs::new()
                            .with("notifications", value.open_notifications)
                            .with("warnings", value.recent_warning_events)
                            .with("errors", value.recent_error_events)
                            .with("storage", value.unavailable_storage_roots)
                            .with("review", value.pending_review_files)
                            .with("transfers", value.nonterminal_transfers),
                    )
                }
                _ => unreachable!("operator action was validated before client startup"),
            };
            Ok::<_, OperatorControlError>(body)
        })
        .map_err(localized_operator_error)
}

fn localized_operator_error(error: OperatorControlError) -> ApplicationError {
    let key = match error {
        OperatorControlError::EndpointUnavailable | OperatorControlError::Io(_) => {
            "operator-endpoint-unavailable"
        }
        OperatorControlError::UnsafeEndpoint(_) => "operator-endpoint-unsafe",
        OperatorControlError::AuthenticationFailed => "operator-authentication-failed",
        OperatorControlError::AuthorizationDenied => "operator-authorization-failed",
        OperatorControlError::ProtocolMismatch
        | OperatorControlError::MalformedFrame
        | OperatorControlError::OversizedFrame
        | OperatorControlError::Serialization(_) => "operator-protocol-mismatch",
        OperatorControlError::UnsupportedFeature | OperatorControlError::PlatformUnavailable => {
            "operator-feature-unsupported"
        }
        OperatorControlError::PeerIdentityUnavailable => "operator-peer-identity-unavailable",
        OperatorControlError::InvalidWindowsSid => "operator-windows-sid-invalid",
        OperatorControlError::PipeSecurityUnavailable => "operator-pipe-security-failed",
        OperatorControlError::Timeout => "operator-request-timeout",
        OperatorControlError::StaleDaemonGeneration => "operator-daemon-restarted",
        OperatorControlError::Conflict | OperatorControlError::InvalidCommand => {
            "operator-request-failed"
        }
        OperatorControlError::Service(_) => "operator-request-failed",
    };
    ApplicationError::Transport(op(key))
}

fn op(key: &str) -> String {
    sf_core::text(key, &sf_core::LocalizationArgs::new())
}

fn op_args(key: &str, args: sf_core::LocalizationArgs) -> String {
    sf_core::text(key, &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_reports_usage_without_touching_files() {
        assert!(matches!(
            run_cli(["unknown"]),
            Err(ApplicationError::Usage(_))
        ));
    }

    #[test]
    fn cli_and_runtime_share_the_cargo_package_version() {
        assert_eq!(
            run_cli(["--version"]).unwrap(),
            format!(
                "SPITFIRE NG Bulletin Board System {}",
                sf_core::PRODUCT_VERSION
            )
        );
    }

    #[test]
    fn cli_initializes_and_runs_the_real_fixture_path() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        let init = run_cli([OsString::from("init-fixture"), root.as_os_str().to_owned()]).unwrap();
        assert!(init.contains("Initialized synthetic fixture board"));

        let run = run_cli([
            OsString::from("demo"),
            root.join(FIXTURE_CONFIG_FILE).into_os_string(),
        ])
        .unwrap();
        assert!(run.contains("MAIN MENU"));
        assert!(run.contains("shutdown=clean"));

        let rerun = run_cli([
            OsString::from("demo"),
            root.join(FIXTURE_CONFIG_FILE).into_os_string(),
        ])
        .unwrap();
        assert!(rerun.contains("Welcome, Increment Two Demo"));
        assert!(rerun.contains("Times On: 2"));
    }
}
