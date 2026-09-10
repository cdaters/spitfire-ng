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

//! Offline installation transactions. Network authority remains in sf-core.

mod control;
mod io;
mod manager;
mod model;
mod release;
mod snapshot;
#[cfg(test)]
mod tests;
mod worker;

use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use thiserror::Error;

use manager::Manager;
pub use model::{
    BoardState, PayloadPolicy, Range, ReleaseManifest, RuntimeDescriptor, Snapshot, StoragePolicy,
};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Rejected(String),
    #[error("deployment filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("deployment database operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid deployment JSON; inspect the document privately")]
    Json(#[from] serde_json::Error),
    #[error("invalid deployment TOML; inspect the document privately")]
    Toml(#[from] toml::de::Error),
    #[error("could not encode deployment TOML")]
    TomlEncode(#[from] toml::ser::Error),
    #[error("board configuration failed validation; inspect configuration privately")]
    Config(#[from] sf_core::ConfigError),
    #[error(transparent)]
    Paths(#[from] sf_core::PathError),
    #[error(transparent)]
    Database(#[from] sf_core::DatabaseError),
    #[error(transparent)]
    Files(#[from] sf_core::files::FilesError),
    #[error(transparent)]
    File(#[from] sf_core::FileError),
    #[error(transparent)]
    Transfer(#[from] sf_core::TransferRuntimeError),
    #[error(transparent)]
    Backup(#[from] crate::BoardBackupError),
    #[error(transparent)]
    Application(Box<crate::ApplicationError>),
    #[cfg(test)]
    #[error("simulated process interruption")]
    Interrupted,
}
impl Error {
    fn application(error: crate::ApplicationError) -> Self {
        match error {
            crate::ApplicationError::Config(config) => Self::Config(config),
            other => Self::Application(Box::new(other)),
        }
    }
}
fn ensure(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Rejected(message.into()))
    }
}

/// All board writers use the existing OS lock. Managed boards additionally
/// require a committed transaction and the selected runtime, even when someone
/// invokes a retained executable directly instead of the launcher.
pub(crate) fn check_board_access(board: &Path) -> Result<()> {
    let Some(locator) = control::locator(board)? else {
        return Ok(());
    };
    let (installation, state) = control::load(&locator.installation)?;
    ensure(
        locator.id == installation.id && installation.config.parent() == Some(board),
        "managed board locator identity/path mismatch",
    )?;
    ensure(
        state.transaction.is_none(),
        "interrupted deployment blocks board startup; run the managed launcher update --recover",
    )?;
    let record = state
        .releases
        .get(&state.active)
        .ok_or_else(|| Error::Rejected("active runtime missing".into()))?;
    ensure(
        io::hash(&std::env::current_exe()?)?
            == (record.manifest.size_bytes, record.manifest.sha256.clone()),
        "this is not the active managed runtime; use the installation launcher",
    )
}
pub(crate) fn preserve_locator(target: &Path, stage: &Path) -> Result<()> {
    control::preserve_locator(target, stage)
}

/// Special offline entry points also work without a managed installation.
pub fn special_cli(arguments: &[OsString]) -> Result<Option<String>> {
    match arguments {
        [command] if command == "deployment-describe" => {
            Ok(Some(serde_json::to_string(&RuntimeDescriptor::current())?))
        }
        [command] if command == "deployment-worker" => Ok(Some(worker::from_stdin()?)),
        [command, subcommand, config, install, source, pin]
            if command == "deployment" && subcommand == "adopt" =>
        {
            Ok(Some(manager::adopt(
                Path::new(config),
                Path::new(install),
                Path::new(source),
                Path::new(pin),
                &std::env::current_exe()?,
                &worker::ProcessRunner,
            )?))
        }
        [command, subcommand, root] if command == "deployment" && subcommand == "recover" => {
            let root = io::real(Path::new(root), true)?;
            let _lock = control::InstallLock::acquire(&root)?;
            // Explicit bootstrap recovery can let SQLite recover its own hot
            // journal before any runtime pointer is read. Normal dry-run never
            // opens the control database writable.
            let db = rusqlite::Connection::open_with_flags(
                root.join("deployment.sqlite3"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
            )?;
            db.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))?;
            drop(db);
            drop(_lock);
            let mut manager = Manager::open(&root, &worker::ProcessRunner, &io::HostCapacity)?;
            Ok(Some(manager.recover()?))
        }
        _ => Ok(None),
    }
}

/// Stable launcher and explicit --installation selection. Returns None for
/// ordinary unmanaged CLI handling. Child stdio is inherited without a shell.
pub fn entry(mut arguments: Vec<OsString>) -> Result<Option<ExitCode>> {
    if arguments.first().is_some_and(|a| {
        a == "deployment-describe" || a == "deployment-worker" || a == "deployment"
    }) {
        return Ok(None);
    }
    let explicit = arguments.first().is_some_and(|a| a == "--installation");
    let root = if explicit {
        ensure(
            arguments.len() >= 3,
            "usage: spitfire --installation INSTALL COMMAND",
        )?;
        let root = PathBuf::from(&arguments[1]);
        arguments.drain(0..2);
        Some(root)
    } else {
        let exe = std::env::current_exe()?;
        let launcher = exe
            .parent()
            .filter(|p| p.file_name().is_some_and(|n| n == "bin"))
            .and_then(Path::parent)
            .filter(|p| p.join("installation.toml").is_file())
            .map(Path::to_path_buf);
        launcher.or(control::locator(&std::env::current_dir()?)?.map(|l| l.installation))
    };
    let Some(root) = root else {
        return Ok(None);
    };
    let mut manager = Manager::open(&root, &worker::ProcessRunner, &io::HostCapacity)?;
    let active = manager.executable(&manager.state.active)?;
    // The immutable launcher is only a format-1 selector. All update logic is
    // executed by the active runtime and therefore updates with the product.
    if io::real(&std::env::current_exe()?, false)? != io::real(&active, false)? {
        manager.verify_runtime(&manager.state.active)?;
        let status = Command::new(active)
            .arg("--installation")
            .arg(&root)
            .args(arguments)
            .status()?;
        return Ok(Some(ExitCode::from(
            u8::try_from(status.code().unwrap_or(1)).unwrap_or(1),
        )));
    }
    let read_only = matches!(arguments.as_slice(),[c] if c=="version"||c=="--version"||c=="-V")
        || matches!(arguments.as_slice(),[c,f] if (c=="update"&&(f=="--check"||f=="--dry-run"))||(c=="rollback"&&f=="--list")||(c=="backup"&&f=="list"));
    let recovery = matches!(arguments.as_slice(),[c,f] if c=="update"&&f=="--recover");
    if manager.state.transaction.is_some() && !read_only && !recovery {
        println!("{}", manager.recover()?);
    }
    let output=match arguments.as_slice() {
        [c] if c=="version"||c=="--version"||c=="-V"=>manager.version()?,
        [c,f] if c=="update"&&f=="--check"=>manager.check(false)?,
        [c,f] if c=="update"&&f=="--dry-run"=>manager.check(true)?,
        [c,f] if c=="update"&&f=="--recover"=>manager.recover()?,
        [c] if c=="update"=>manager.update()?,
        [c] if c=="backup"=>manager.backup()?,
        [c,f] if c=="backup"&&f=="list"=>manager.backups()?,
        [c,f] if c=="rollback"&&f=="--list"=>manager.rollback_list()?,
        [c,f] if c=="rollback"&&f=="--yes"=>manager.rollback(true)?,
        [c] if c=="rollback"=>{
            let preview=manager.rollback(false)?;
            if !std::io::stdin().is_terminal(){return Err(Error::Rejected(format!("{preview}\nConfirmation required; no changes made.")));}
            println!("{preview}");print!("Proceed? [y/N] ");std::io::stdout().flush()?;
            let mut answer=String::new();std::io::stdin().read_line(&mut answer)?;
            if answer.trim().eq_ignore_ascii_case("y")||answer.trim().eq_ignore_ascii_case("yes"){manager.rollback(true)?}else{"Runtime rollback cancelled. No changes made.".into()}
        },
        [c,id,f] if c=="restore"&&f=="--replace"=>manager.restore(id.to_str().ok_or_else(||Error::Rejected("invalid backup id".into()))?)?,
        [c,..] if c=="update"||c=="rollback"||c=="backup"||c=="restore"=>return Err(Error::Rejected("Managed commands: update [--check|--dry-run|--recover], backup [list], rollback [--list|--yes], restore ID --replace (discards later data; native network protections apply)".into())),
        _=>{
            if matches!(arguments.as_slice(),[c] if c=="run"||c=="console"||c=="shell"||c=="status"||c=="config") {arguments.push(manager.installation.config.as_os_str().to_os_string());}
            crate::run_cli(arguments).map_err(Error::application)?
        },
    };
    println!("{output}");
    Ok(Some(ExitCode::SUCCESS))
}
