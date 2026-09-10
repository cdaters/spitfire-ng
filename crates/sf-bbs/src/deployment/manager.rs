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

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::control::{self, InstallLock};
use super::io::{self, Capacity};
use super::model::*;
use super::release;
use super::snapshot;
use super::worker::{self, Runner};
use super::{ensure, Error, Result};

pub struct Manager<'a> {
    pub root: PathBuf,
    pub installation: Installation,
    pub state: Control,
    pub runner: &'a dyn Runner,
    pub capacity: &'a dyn Capacity,
    #[cfg(test)]
    pub fault: Option<(Phase, bool)>,
}
impl<'a> Manager<'a> {
    pub fn open(root: &Path, runner: &'a dyn Runner, capacity: &'a dyn Capacity) -> Result<Self> {
        let root = io::real(root, true)?;
        let (installation, state) = control::load(&root)?;
        control::check_locator(&root, &installation)?;
        Ok(Self {
            root,
            installation,
            state,
            runner,
            capacity,
            #[cfg(test)]
            fault: None,
        })
    }
    fn active(&self) -> Result<&ReleaseRecord> {
        self.state
            .releases
            .get(&self.state.active)
            .ok_or_else(|| Error::Rejected("active runtime is not registered".into()))
    }
    pub fn executable(&self, version: &str) -> Result<PathBuf> {
        Ok(control::release_root(&self.root, version)?.join(release::binary_name()))
    }
    pub fn verify_runtime(&self, version: &str) -> Result<()> {
        let expected = self
            .state
            .releases
            .get(version)
            .ok_or_else(|| Error::Rejected("runtime is not registered".into()))?;
        let root = control::release_root(&self.root, version)?;
        ensure(
            release::verify(&root, &self.installation.release_public_key)? == *expected,
            "retained runtime no longer matches its registered release",
        )?;
        release::probe(&root, expected, self.runner)
    }
    fn board_root(&self) -> Result<&Path> {
        self.installation
            .config
            .parent()
            .ok_or_else(|| Error::Rejected("missing board root".into()))
    }
    fn locks(&self) -> Result<(InstallLock, crate::board_lock::BoardOperationLock)> {
        let install = InstallLock::acquire(&self.root)?;
        let board = crate::board_lock::BoardOperationLock::acquire_maintenance(self.board_root()?)
            .map_err(Error::application)?;
        Ok((install, board))
    }
    fn reload(&mut self) -> Result<()> {
        let (installation, state) = control::load(&self.root)?;
        control::check_locator(&self.root, &installation)?;
        self.installation = installation;
        self.state = state;
        Ok(())
    }
    fn no_transaction(&self) -> Result<()> {
        ensure(
            self.state.transaction.is_none(),
            "an interrupted deployment is pending; run spitfire update --recover before new work",
        )
    }
    fn live_state(&self) -> Result<BoardState> {
        let mut state = worker::inspect(&self.installation.config, self.board_root()?, false)?;
        for (name, required) in &self.state.required_features {
            state
                .required_features
                .entry(name.clone())
                .and_modify(|v| *v = (*v).max(*required))
                .or_insert(*required);
        }
        Ok(state)
    }
    fn inspect_runtime(
        &self,
        version: &str,
        config: &Path,
        payload_root: &Path,
        operation: &str,
    ) -> Result<BoardState> {
        self.runner.worker(
            &self.executable(version)?,
            &WorkerRequest {
                format: FORMAT,
                operation: operation.into(),
                config: config.into(),
                payload_root: payload_root.into(),
            },
        )
    }
    fn phase(&mut self, phase: Phase) -> Result<()> {
        self.state
            .transaction
            .as_mut()
            .ok_or_else(|| Error::Rejected("no update transaction".into()))?
            .phase = phase;
        control::save(&self.root, &self.state)?;
        #[cfg(test)]
        if phase != Phase::ApplyRuntime && self.fault == Some((phase, true)) {
            return Err(Error::Interrupted);
        }
        #[cfg(test)]
        if !matches!(phase, Phase::ApplyRuntime | Phase::VerifyBackup)
            && self.fault == Some((phase, false))
        {
            return Err(Error::Rejected(format!("injected failure at {phase:?}")));
        }
        Ok(())
    }
    pub fn version(&self) -> Result<String> {
        let current = self.active()?;
        let tx = self
            .state
            .transaction
            .as_ref()
            .map(|t| format!("\nRecovery pending: {:?}", t.phase))
            .unwrap_or_default();
        Ok(format!(
            "SPITFIRE NG {}\nManaged runtime: {}\nDeployment protocol: {}{}",
            current.manifest.runtime.version, self.state.active, FORMAT, tx
        ))
    }
    pub fn check(&mut self, dry_run: bool) -> Result<String> {
        let _locks = self.locks()?;
        self.reload()?;
        self.no_transaction()?;
        self.verify_runtime(&self.state.active)?;
        let Some((source, candidate)) = release::discover(
            &self.installation.source,
            &self.installation.release_public_key,
            &self.state.active,
        )?
        else {
            return Ok(format!(
                "SPITFIRE NG {}\nNo newer authenticated release is available.",
                self.state.active
            ));
        };
        let current = self.live_state()?;
        release::compatible_upgrade(&self.active()?.manifest.runtime, &candidate, &current)?;
        let plan = snapshot::plan(
            &self.installation,
            &self.active()?.manifest.runtime,
            "pre-upgrade",
        )?;
        let space = snapshot::space(
            &plan,
            &self.installation.storage,
            &self.root,
            candidate.manifest.size_bytes,
            true,
            self.capacity,
        )?;
        // Executing a signed probe is unnecessary for check/dry-run. It is
        // deferred until the verified package is copied into owned staging.
        let _ = source;
        Ok(format!("SPITFIRE NG {}\nUpdate available: {}\nCompatibility/configuration/database/integrity: OK\nBackup budget: {} bytes; installation workspace: {} bytes; board workspace: {} bytes\nManaged payloads: {:?}; external payloads referenced: {}\n{}{}",
            self.state.active,candidate.manifest.runtime.version,space.backup_bytes,space.installation_required,space.board_required,
            self.installation.storage.file_payloads,plan.snapshot.references.iter().filter(|r|r.class==StateClass::ExternalReference).count(),
            if space.warning {"Low-space warning: configured warning threshold would be crossed.\n"} else {""},
            if dry_run {"Dry-run complete. No installation or board files changed. Runtime execution/migrations are deferred until update."} else {"Check complete. No update was applied."}))
    }
    pub fn backup(&mut self) -> Result<String> {
        let _locks = self.locks()?;
        self.reload()?;
        self.no_transaction()?;
        self.verify_runtime(&self.state.active)?;
        let plan = snapshot::plan(
            &self.installation,
            &self.active()?.manifest.runtime,
            "manual",
        )?;
        snapshot::space(
            &plan,
            &self.installation.storage,
            &self.root,
            0,
            false,
            self.capacity,
        )?;
        let id = plan.snapshot.id.clone();
        let (snapshot, hash) = snapshot::create(&plan, &self.root.join("backups").join(&id))?;
        self.state.backups.insert(id.clone(), hash);
        self.state.last_operation = format!("manual backup {id} verified");
        control::save(&self.root, &self.state)?;
        Ok(backup_report(&snapshot))
    }
    pub fn backups(&self) -> Result<String> {
        let mut output = String::from("Verified backup history (no automatic pruning):\n");
        for (id, hash) in &self.state.backups {
            let backup = snapshot::verify(&self.root.join("backups").join(id), Some(hash))?;
            output.push_str(&format!(
                "{id}  {}  version {}  schema {}  {}  {} stored content bytes\n",
                backup.created_at,
                backup.source_runtime.version,
                backup.state.schema,
                backup.backup_type,
                backup.stored_content_bytes
            ));
        }
        Ok(output)
    }
    pub fn update(&mut self) -> Result<String> {
        let _locks = self.locks()?;
        self.reload()?;
        self.no_transaction()?;
        self.verify_runtime(&self.state.active)?;
        let Some((source, candidate)) = release::discover(
            &self.installation.source,
            &self.installation.release_public_key,
            &self.state.active,
        )?
        else {
            return Ok("No newer authenticated release is available.".into());
        };
        let before = self.live_state()?;
        release::compatible_upgrade(&self.active()?.manifest.runtime, &candidate, &before)?;
        let plan = snapshot::plan(
            &self.installation,
            &self.active()?.manifest.runtime,
            "pre-upgrade",
        )?;
        snapshot::space(
            &plan,
            &self.installation.storage,
            &self.root,
            candidate.manifest.size_bytes,
            true,
            self.capacity,
        )?;
        let old = self.state.active.clone();
        let target = candidate.manifest.runtime.version.clone();
        let id = io::id();
        self.state.transaction = Some(Transaction {
            id: id.clone(),
            phase: Phase::Backup,
            old: old.clone(),
            target: target.clone(),
            checkpoint: None,
            checkpoint_sha256: None,
            stage_sha256: None,
        });
        control::save(&self.root, &self.state)?;
        let result = self.apply(&plan, &source, &candidate, &before);
        if let Err(error) = result {
            #[cfg(test)]
            if matches!(error, Error::Interrupted) {
                return Err(error);
            }
            // Read the persisted commit authority, never an in-memory phase
            // left over from a failed fsync/save.
            self.reload()?;
            if self
                .state
                .transaction
                .as_ref()
                .is_some_and(|t| matches!(t.phase, Phase::Commit | Phase::Cleanup))
            {
                return Err(Error::Rejected(format!("Update committed; cleanup requires recovery. Current board data will not be restored. {error}")));
            }
            self.recover_locked()?;
            return Err(Error::Rejected(format!("Update was not committed: {error}\nPre-upgrade recovery and validation: OK\nSPITFIRE NG {old} remains active.")));
        }
        self.finish_committed()?;
        Ok(format!("Preflight ........ OK\nBackup ........... VERIFIED\nRelease .......... VERIFIED\nMigration ........ OK\nValidation ....... OK\nCommit ........... OK\n\nUpgrade complete.\nSPITFIRE NG {old} -> {target}\nPrevious runtime retained. Current board remains stopped; start it when ready."))
    }
    fn apply(
        &mut self,
        plan: &snapshot::Plan,
        source: &Path,
        candidate: &ReleaseRecord,
        before: &BoardState,
    ) -> Result<()> {
        self.phase(Phase::Backup)?;
        let id = plan.snapshot.id.clone();
        let checkpoint = self.root.join("backups").join(&id);
        let (snapshot, hash) = snapshot::create(plan, &checkpoint)?;
        self.state.backups.insert(id.clone(), hash.clone());
        let tx = self
            .state
            .transaction
            .as_mut()
            .ok_or_else(|| Error::Rejected("missing update transaction".into()))?;
        tx.checkpoint = Some(id);
        tx.checkpoint_sha256 = Some(hash.clone());
        self.phase(Phase::VerifyBackup)?;
        #[cfg(test)]
        if self.fault == Some((Phase::VerifyBackup, false)) {
            fs::write(
                checkpoint.join("board").join(&snapshot.config_name),
                b"injected checkpoint corruption",
            )?;
        }
        snapshot::verify(&checkpoint, Some(&hash))?;
        self.phase(Phase::StageRelease)?;
        let target = &candidate.manifest.runtime.version;
        let release_root = control::release_root(&self.root, target)?;
        release::stage(
            source,
            &release_root,
            &self.installation.release_public_key,
            candidate,
        )?;
        self.state
            .releases
            .insert(target.clone(), candidate.clone());
        self.phase(Phase::VerifyRelease)?;
        self.verify_runtime(target)?;
        let transaction_id = self
            .state
            .transaction
            .as_ref()
            .ok_or_else(|| Error::Rejected("missing transaction".into()))?
            .id
            .clone();
        let transaction_root = self.root.join("transactions").join(transaction_id);
        io::private_dir(&transaction_root)?;
        let stage = transaction_root.join("stage");
        snapshot::stage(&checkpoint, &snapshot, &stage)?;
        self.phase(Phase::Migrate)?;
        let after = self.inspect_runtime(
            target,
            &stage.join(&snapshot.config_name),
            self.board_root()?,
            "migrate",
        )?;
        self.phase(Phase::Validate)?;
        candidate.manifest.runtime.compatible(&after)?;
        ensure(
            after.schema == candidate.manifest.runtime.target_schema,
            "candidate migration did not reach signed target schema",
        )?;
        worker::preserved(before, &after)?;
        let old_config = sf_core::RuntimeConfig::load(&self.installation.config)?;
        let new_config = sf_core::RuntimeConfig::load(&stage.join(&snapshot.config_name))?;
        ensure(
            old_config.paths == new_config.paths && old_config.storage == new_config.storage,
            "D1 does not migrate board paths/storage layout",
        )?;
        snapshot::unchanged_resources(&snapshot, &stage)?;
        snapshot::unchanged_resources(&snapshot, self.board_root()?)?;
        self.inspect_runtime(
            target,
            &stage.join(&snapshot.config_name),
            self.board_root()?,
            "inspect",
        )?;
        let (_, stage_paths) = worker::paths(&stage.join(&snapshot.config_name))?;
        worker::clean_journals(stage_paths.database())?;
        let staged_hash = io::digest(&serde_json::to_vec(&(
            io::hash(stage_paths.database())?,
            io::hash(&stage.join(&snapshot.config_name))?,
        ))?);
        self.state
            .transaction
            .as_mut()
            .ok_or_else(|| Error::Rejected("missing transaction".into()))?
            .stage_sha256 = Some(staged_hash);
        // A forward migration can grow state beyond its source size. Recheck
        // actual staged growth and enough headroom to replace and recover before
        // touching either live file.
        let grown = io::add(
            io::hash(stage_paths.database())?.0,
            io::hash(&stage.join(&snapshot.config_name))?.0,
        )?;
        let remaining_write = io::add(
            io::multiply(grown, 2)?,
            io::multiply(plan.critical_bytes, 2)?,
        )?;
        for path in [&self.root, self.board_root()?] {
            ensure(
                self.capacity
                    .available(path)?
                    .checked_sub(remaining_write)
                    .is_some_and(|n| {
                        n >= self
                            .installation
                            .storage
                            .reserve_free_bytes
                            .max(self.installation.storage.hard_stop_free_bytes)
                    }),
                "migration growth leaves insufficient activation/recovery reserve",
            )?;
        }
        // The pre-COMMIT journal is durable before either live file changes.
        self.phase(Phase::ApplyRuntime)?;
        let (_, live_paths) = worker::paths(&self.installation.config)?;
        snapshot::clean_sidecars(live_paths.database())?;
        io::replace(stage_paths.database(), live_paths.database())?;
        #[cfg(test)]
        if self
            .fault
            .is_some_and(|(phase, _)| phase == Phase::ApplyRuntime)
        {
            // Model a partially adopted database containing newer staged data.
            // Recovery must actually replace its contents, not merely report
            // success because the source and target happened to share a schema.
            let db = rusqlite::Connection::open(live_paths.database())?;
            db.execute_batch("CREATE TABLE d1_interrupted_activation(value TEXT); INSERT INTO d1_interrupted_activation VALUES('uncommitted synthetic state');")?;
        }
        #[cfg(test)]
        if self.fault == Some((Phase::ApplyRuntime, true)) {
            return Err(Error::Interrupted);
        }
        #[cfg(test)]
        if self.fault == Some((Phase::ApplyRuntime, false)) {
            return Err(Error::Rejected(
                "injected activation failure after database replacement".into(),
            ));
        }
        io::replace(
            &stage.join(&snapshot.config_name),
            &self.installation.config,
        )?;
        self.phase(Phase::ValidateActive)?;
        // Validate adopted bytes offline before recording the activation. The
        // board lock and pending journal still prohibit normal operation.
        let live = self.inspect_runtime(
            target,
            &self.installation.config,
            self.board_root()?,
            "inspect",
        )?;
        ensure(
            live == after,
            "adopted state differs from validated staged state",
        )?;
        let old = self.state.active.clone();
        self.state.previous.retain(|v| v != &old && v != target);
        self.state.previous.insert(0, old);
        self.state.active = target.clone();
        for (name, generation) in after.required_features {
            self.state
                .required_features
                .entry(name)
                .and_modify(|v| *v = (*v).max(generation))
                .or_insert(generation);
        }
        self.state.last_operation = format!("update to {target} committed");
        // Active runtime, required feature generations and COMMIT share the
        // management SQLite transaction performed by phase().
        self.phase(Phase::Commit)
    }
    fn finish_committed(&mut self) -> Result<()> {
        self.phase(Phase::Cleanup)?;
        self.verify_runtime(&self.state.active)?;
        // Retain checkpoint and transaction staging conservatively. Cleanup
        // only closes journal authority; no known-good bytes are deleted.
        self.state.transaction = None;
        control::save(&self.root, &self.state)
    }
    pub fn recover(&mut self) -> Result<String> {
        let _locks = self.locks()?;
        self.reload()?;
        if self.state.transaction.is_none() {
            return Ok("No interrupted update requires recovery.".into());
        }
        self.recover_locked()?;
        Ok(format!("Recovery complete. SPITFIRE NG {} is active.\nCommitted updates never rewind current board data.",self.state.active))
    }
    fn recover_locked(&mut self) -> Result<()> {
        let tx = self
            .state
            .transaction
            .clone()
            .ok_or_else(|| Error::Rejected("no recovery transaction".into()))?;
        if matches!(tx.phase, Phase::Commit | Phase::Cleanup) {
            return self.finish_committed();
        }
        ensure(tx.phase!=Phase::ManualRestore,"manual restore was interrupted; inspect the native restore rollback directory before operator recovery")?;
        self.verify_runtime(&tx.old)?;
        if matches!(tx.phase, Phase::ApplyRuntime | Phase::ValidateActive) {
            let id = tx
                .checkpoint
                .as_deref()
                .ok_or_else(|| Error::Rejected("missing recovery checkpoint id".into()))?;
            let hash = tx
                .checkpoint_sha256
                .as_deref()
                .ok_or_else(|| Error::Rejected("missing recovery checkpoint integrity".into()))?;
            let root = self.root.join("backups").join(id);
            let snapshot = snapshot::verify(&root, Some(hash))?;
            ensure(
                snapshot.installation_id == self.installation.id
                    && snapshot.source_runtime.version == tx.old,
                "recovery checkpoint belongs to another transaction/runtime",
            )?;
            snapshot::restore_precommit(&root, &snapshot, &self.installation.config)?;
            let actual = self.inspect_runtime(
                &tx.old,
                &self.installation.config,
                self.board_root()?,
                "inspect",
            )?;
            worker::preserved(&snapshot.state, &actual)?;
            ensure(
                actual.schema == snapshot.state.schema
                    && actual.config_format == snapshot.state.config_format,
                "recovered board compatibility differs from checkpoint",
            )?;
        } else {
            // Every earlier phase has a staged-only write set. Validate the
            // authoritative old board, then close the abandoned transaction.
            self.inspect_runtime(
                &tx.old,
                &self.installation.config,
                self.board_root()?,
                "inspect",
            )?;
        }
        self.state.active = tx.old;
        self.state.last_operation = format!("pre-commit update {} recovered", tx.id);
        self.state.transaction = None;
        control::save(&self.root, &self.state)
    }
    pub fn rollback_list(&mut self) -> Result<String> {
        let _locks = self.locks()?;
        self.reload()?;
        self.no_transaction()?;
        let current = self.live_state()?;
        let mut output = format!(
            "Current runtime: {}\nCurrent board schema: {}\nPrevious runtime candidates:\n",
            self.state.active, current.schema
        );
        for version in &self.state.previous {
            let result = self.state.releases[version]
                .manifest
                .runtime
                .compatible(&current)
                .and_then(|()| self.verify_runtime(version));
            output.push_str(&format!(
                "{version}: {}\n",
                match result {
                    Ok(()) => "compatible".into(),
                    Err(e) => format!("unavailable: {e}"),
                }
            ));
        }
        Ok(output)
    }
    pub fn restore(&mut self, id: &str) -> Result<String> {
        io::safe_id(id)?;
        let _locks = self.locks()?;
        self.reload()?;
        self.no_transaction()?;
        let hash = self.state.backups.get(id).ok_or_else(|| {
            Error::Rejected("backup id is not in this installation's verified history".into())
        })?;
        let checkpoint = self.root.join("backups").join(id);
        let backup = snapshot::verify(&checkpoint, Some(hash))?;
        ensure(
            backup.installation_id == self.installation.id
                && self
                    .installation
                    .config
                    .file_name()
                    .is_some_and(|n| n == backup.config_name.as_str()),
            "backup belongs to a different installation/configuration",
        )?;
        self.active()?.manifest.runtime.compatible(&backup.state)?;
        self.verify_runtime(&self.state.active)?;
        let before = snapshot::plan(
            &self.installation,
            &self.active()?.manifest.runtime,
            "restore-validation",
        )?;
        let (_, current_paths) = worker::paths(&self.installation.config)?;
        let (_, backed_paths) = worker::paths(&checkpoint.join("board").join(&backup.config_name))?;
        let current_db = sf_core::RuntimeDatabase::open_read_only(current_paths.database())?;
        let backed_db = sf_core::RuntimeDatabase::open_read_only(backed_paths.database())?;
        current_db.circuitnet_catalog_check_restore(&backed_db).map_err(|_|Error::Rejected("restore refused: backup would discard or conflict with newer CircuitNET catalog authority/evidence; current data was not modified".into()))?;
        drop(backed_db);
        drop(current_db);
        let mut payloads = BTreeMap::new();
        for reference in backup
            .references
            .iter()
            .filter(|r| r.class == StateClass::ManagedPayload)
        {
            let source = if reference.copied {
                checkpoint.join("board").join(&reference.path)
            } else {
                self.board_root()?.join(&reference.path)
            };
            ensure(io::hash(&source)?==(reference.size_bytes,reference.sha256.clone()),"restore requires a referenced managed payload that is missing or changed; no board data was modified")?;
            payloads.insert(reference.path.clone(), (source, reference.size_bytes));
        }
        let payload_bytes = payloads
            .values()
            .try_fold(0, |sum, (_, n)| io::add(sum, *n))?;
        let required = io::add(
            io::multiply(io::add(backup.stored_content_bytes, payload_bytes)?, 4)?,
            MAX_DOCUMENT,
        )?;
        for path in [&self.root, self.board_root()?] {
            let available = self.capacity.available(path)?;
            ensure(
                available.checked_sub(required).is_some_and(|n| {
                    n >= self
                        .installation
                        .storage
                        .reserve_free_bytes
                        .max(self.installation.storage.hard_stop_free_bytes)
                }),
                "insufficient temporary space for validated disaster restore",
            )?;
        }
        let temp = tempfile::Builder::new()
            .prefix(".manual-restore-")
            .tempdir_in(self.root.join("transactions"))?;
        let materialized = temp.path().join("board");
        snapshot::stage(&checkpoint, &backup, &materialized)?;
        for (relative, (source, _)) in payloads {
            io::copy(&source, &io::parents(&materialized, &relative)?)?;
        }
        let native = temp.path().join("native");
        io::private_dir(&native)?;
        crate::backup::deployment_restore_input(
            &materialized.join(&backup.config_name),
            &native,
            &backup.source_runtime.version,
        )
        .map_err(Error::application)?;
        // The existing native restore retains its catalog check, FTN evidence
        // reconciliation, queue holds, Events recovery and external normalization.
        self.state.transaction = Some(Transaction {
            id: io::id(),
            phase: Phase::ManualRestore,
            old: self.state.active.clone(),
            target: self.state.active.clone(),
            checkpoint: Some(id.into()),
            checkpoint_sha256: Some(hash.clone()),
            stage_sha256: None,
        });
        control::save(&self.root, &self.state)?;
        let result = crate::backup::restore_board_locked(&native, self.board_root()?);
        if let Err(error) = result {
            let unchanged = before.snapshot.entries.iter().all(|e| {
                io::hash(&before.source_root.join(&e.path))
                    .is_ok_and(|found| found == (e.size_bytes, e.sha256.clone()))
            });
            if unchanged {
                self.state.transaction = None;
                control::save(&self.root, &self.state)?;
            }
            return Err(Error::application(error));
        }
        self.inspect_runtime(
            &self.state.active,
            &self.installation.config,
            self.board_root()?,
            "inspect",
        )?;
        self.state.transaction = None;
        self.state.last_operation = format!("explicit disaster restore {id} complete");
        control::save(&self.root, &self.state)?;
        Ok(format!("Disaster restore complete: backup {id}, source runtime {}, schema {}.\nPost-snapshot ordinary board changes were replaced. Native network safety/recovery rules were applied.\nExternal payload references: {}; external bytes were not copied.\nRuntime rollback remains a separate data-preserving operation.",backup.source_runtime.version,backup.state.schema,backup.references.iter().filter(|r|r.class==StateClass::ExternalReference).count()))
    }
    pub fn rollback(&mut self, confirmed: bool) -> Result<String> {
        let _locks = self.locks()?;
        self.reload()?;
        self.no_transaction()?;
        let current = self.live_state()?;
        self.verify_runtime(&self.state.active)?;
        let mut selected = None;
        let mut reasons = Vec::new();
        for version in &self.state.previous {
            let result = self.state.releases[version]
                .manifest
                .runtime
                .compatible(&current)
                .and_then(|()| self.verify_runtime(version));
            match result {
                Ok(()) => {
                    selected = Some(version.clone());
                    break;
                }
                Err(e) => reasons.push(format!("{version}: {e}")),
            }
        }
        let target=selected.ok_or_else(||Error::Rejected(format!("Runtime rollback refused. No compatible previous runtime.\n{}\nCurrent board data was not modified.",reasons.join("\n"))))?;
        if !confirmed {
            return Ok(format!("Current runtime: {}\nPrevious compatible runtime: {target}\nCurrent board schema: {}\nCurrent board data and network evidence will be preserved. Only the runtime will change.\nContinue with --yes, or confirm in an interactive terminal.",self.state.active,current.schema));
        }
        // Validate an independent critical copy, so an older worker cannot
        // accidentally write the current database during compatibility checks.
        let plan = snapshot::plan(
            &self.installation,
            &self.active()?.manifest.runtime,
            "rollback-validation",
        )?;
        let before =
            worker::database_fingerprints(worker::paths(&self.installation.config)?.1.database())?;
        let needed = io::add(plan.critical_bytes, MAX_DOCUMENT)?;
        ensure(
            self.capacity
                .available(&self.root)?
                .checked_sub(needed)
                .is_some_and(|n| {
                    n >= self
                        .installation
                        .storage
                        .reserve_free_bytes
                        .max(self.installation.storage.hard_stop_free_bytes)
                }),
            "insufficient space for isolated runtime rollback validation",
        )?;
        let temp = tempfile::Builder::new()
            .prefix(".rollback-validation-")
            .tempdir_in(self.root.join("transactions"))?;
        let stage = temp.path().join("board");
        io::private_dir(&stage)?;
        for e in plan
            .snapshot
            .entries
            .iter()
            .filter(|e| e.class == StateClass::Critical)
        {
            io::copy(
                &plan.source_root.join(&e.path),
                &io::parents(&stage, &e.path)?,
            )?;
        }
        let config = sf_core::RuntimeConfig::load(&stage.join(&plan.snapshot.config_name))?;
        sf_core::LogicalPaths::resolve(&stage, &config.validate()?)?.create_directories()?;
        let validation = self.inspect_runtime(
            &target,
            &stage.join(&plan.snapshot.config_name),
            self.board_root()?,
            "inspect",
        )?;
        worker::preserved(&current, &validation)?;
        ensure(
            before
                == worker::database_fingerprints(
                    worker::paths(&self.installation.config)?.1.database(),
                )?,
            "runtime rollback validation changed current durable state",
        )?;
        let old = self.state.active.clone();
        self.state.previous.retain(|v| v != &target && v != &old);
        self.state.previous.insert(0, old.clone());
        self.state.active = target.clone();
        self.state.last_operation = format!("runtime-only rollback {old} to {target}");
        control::save(&self.root, &self.state)?;
        Ok(format!("Runtime rollback complete: {old} -> {target}\nCurrent database, configuration and network evidence were preserved. Board remains stopped."))
    }
}

pub fn backup_report(snapshot: &Snapshot) -> String {
    format!("Backup {} verified.\nType: {}; source runtime {}; schema {}; configuration {}\nStored content: {} bytes; logical state/references: {} bytes\nManaged payloads copied: {}; managed payloads referenced: {}; external payloads referenced (not copied): {}\nKeep excluded media separately; restore is distinct from runtime rollback.",
        snapshot.id,snapshot.backup_type,snapshot.source_runtime.version,snapshot.state.schema,snapshot.state.config_format,snapshot.stored_content_bytes,snapshot.total_logical_bytes,
        snapshot.references.iter().filter(|r|r.class==StateClass::ManagedPayload && r.copied).count(),snapshot.references.iter().filter(|r|r.class==StateClass::ManagedPayload && !r.copied).count(),snapshot.references.iter().filter(|r|r.class==StateClass::ExternalReference).count())
}

pub fn adopt(
    config: &Path,
    destination: &Path,
    source: &Path,
    pin_path: &Path,
    adopter: &Path,
    runner: &dyn Runner,
) -> Result<String> {
    let config = io::real(config, false)?;
    let source = io::real(source, true)?;
    let board = config
        .parent()
        .ok_or_else(|| Error::Rejected("missing board root".into()))?;
    let parent = io::real(
        destination
            .parent()
            .ok_or_else(|| Error::Rejected("installation parent must exist".into()))?,
        true,
    )?;
    let name = destination
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Rejected("invalid installation name".into()))?;
    io::relative(name)?;
    let destination = parent.join(name);
    ensure(
        !destination.starts_with(board)
            && !board.starts_with(&destination)
            && !source.starts_with(&destination),
        "installation, board and source must have separate ownership",
    )?;
    let pin_bytes = io::read(pin_path, 65)?;
    let pin = std::str::from_utf8(&pin_bytes)
        .map_err(|_| Error::Rejected("invalid release public key".into()))?
        .trim()
        .to_owned();
    io::unhex(&pin, 32)?;
    let descriptor = runner.describe(adopter)?;
    descriptor.validate()?;
    let package = source.join(&descriptor.version);
    let record = release::verify(&package, &pin)?;
    ensure(
        record.manifest.runtime == descriptor
            && io::hash(adopter)? == (record.manifest.size_bytes, record.manifest.sha256.clone()),
        "adoption package does not match the invoking runtime",
    )?;
    let _lock = crate::board_lock::BoardOperationLock::acquire_maintenance(board)
        .map_err(Error::application)?;
    descriptor.compatible(&worker::inspect(&config, board, true)?)?;
    let (_, paths) = worker::paths(&config)?;
    let db = sf_core::RuntimeDatabase::open_read_only(paths.database())?;
    for network in db
        .circuitnet_profiles()
        .map_err(|_| Error::Rejected("invalid CircuitNET trust configuration".into()))?
    {
        let status = db
            .circuitnet_catalog_status(&network)
            .map_err(|_| Error::Rejected("invalid CircuitNET catalog authority".into()))?;
        ensure(
            status
                .authority
                .is_none_or(|authority| authority.public_key != pin),
            "release authority must be separate from CircuitNET catalog authority",
        )?;
    }
    drop(db);
    if destination.exists() {
        let (installation, mut state) = control::load(&destination)?;
        ensure(
            state.last_operation == "adoption pending"
                && installation.config == config
                && installation.source == source
                && installation.release_public_key == pin
                && state.active == descriptor.version,
            "installation already exists; refusing replacement",
        )?;
        if control::locator(board)?.is_none() {
            control::write_locator(board, &destination, &installation.id)?;
        }
        control::check_locator(&destination, &installation)?;
        state.last_operation = "adoption complete".into();
        control::save(&destination, &state)?;
        return Ok("Interrupted adoption completed.".into());
    }
    ensure(
        control::locator(board)?.is_none(),
        "board already belongs to a managed installation",
    )?;
    let temp = tempfile::Builder::new()
        .prefix(".spitfire-adopt-")
        .tempdir_in(&parent)?;
    for directory in ["bin", "releases", "backups", "transactions"] {
        io::private_dir(&temp.path().join(directory))?;
    }
    let installation = Installation {
        format: FORMAT,
        id: io::id(),
        config: config.clone(),
        source,
        release_public_key: pin,
        storage: StoragePolicy::default(),
    };
    release::stage(
        &package,
        &temp.path().join("releases").join(&descriptor.version),
        &installation.release_public_key,
        &record,
    )?;
    io::copy(
        adopter,
        &temp.path().join("bin").join(release::binary_name()),
    )?;
    io::executable(&temp.path().join("bin").join(release::binary_name()))?;
    io::write_new(
        &temp.path().join("installation.toml"),
        toml::to_string_pretty(&installation)?.as_bytes(),
    )?;
    io::write_new(&temp.path().join("maintenance.lock"), b"")?;
    let state = Control {
        format: FORMAT,
        installation_id: installation.id.clone(),
        active: descriptor.version.clone(),
        previous: vec![],
        releases: BTreeMap::from([(descriptor.version, record)]),
        required_features: durable_features(),
        backups: BTreeMap::new(),
        transaction: None,
        last_operation: "adoption pending".into(),
    };
    control::initialize(temp.path(), &state)?;
    io::sync_dir(temp.path())?;
    let staging = temp.keep();
    fs::rename(staging, &destination)?;
    io::sync_dir(&parent)?;
    control::write_locator(board, &destination, &installation.id)?;
    let mut state = state;
    state.last_operation = "adoption complete".into();
    control::save(&destination, &state)?;
    Ok(format!("Managed installation adopted.\nUse {}\nBoard data and external file libraries were not relocated.",destination.join("bin").join(release::binary_name()).display()))
}
