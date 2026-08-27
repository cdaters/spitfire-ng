//! Application orchestration for the native SPITFIRE NG runtime.

mod admin;
mod backup;
mod board_lock;
mod error;
mod fixture;
mod operator;
mod presentation;
mod resources;
mod runtime;
mod setup;
mod status;
mod transports;

pub use admin::{interactive_config, BoardAdmin};
pub use backup::{
    backup_board, restore_board, BackupReport, BoardBackupError, RestoreReport,
    BACKUP_MANIFEST_FILE,
};
pub use error::ApplicationError;
pub use fixture::{initialize_fixture_board, FixtureReport, FIXTURE_CONFIG_FILE};
pub use operator::{run_operator_console, OperatorService};
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
pub use status::{board_status, RuntimeStatusDocument, RUNTIME_STATUS_FILE};

use std::ffi::OsString;
use std::path::PathBuf;

use sf_core::InMemoryTerminal;

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
