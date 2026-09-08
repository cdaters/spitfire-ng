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

//! Typed cold-board native Files operator commands, using the shared admission service.
use crate::{ApplicationError, OfflineConfiguration};
use sf_core::{
    files::{SafetyPolicy, ScanPolicy},
    FileAccessMode, FileAdminActor, FileAreaDefinition, FileId, LocalOperatorCapability as Cap,
    SecurityLevel,
};
use std::{ffi::OsString, fs::File, path::Path};
fn usage() -> ApplicationError {
    ApplicationError::Usage(crate::op("files-custody-usage"))
}
pub fn run(config: &Path, args: &[OsString]) -> Result<String, ApplicationError> {
    let args = args
        .iter()
        .map(|s| s.to_str().ok_or_else(usage))
        .collect::<Result<Vec<_>, _>>()?;
    let authority = OfflineConfiguration::open(config)?;
    let actor = FileAdminActor::LocalOperator;
    let action = *args.first().ok_or_else(usage)?;
    let cap = if matches!(action, "areas" | "list" | "status" | "integrity") {
        Cap::ReadConfiguration
    } else {
        Cap::ChangeSensitiveConfiguration
    };
    let output = authority.files(cap, |db, store| {
        use sf_core::files::{FilesError, Result};
        let invalid = || FilesError::Policy("invalid-files-command".into());
        let id =
            |s: &str| -> Result<FileId> { Ok(FileId::new(s.parse().map_err(|_| invalid())?)?) };
        let number = |s: &str| -> Result<u16> { s.parse().map_err(|_| invalid()) };
        let area = |db: &sf_core::RuntimeDatabase, s: &str| -> Result<sf_core::FileArea> {
            db.all_file_areas()?
                .into_iter()
                .find(|a| Some(a.number) == s.parse().ok())
                .ok_or_else(invalid)
        };
        match args.as_slice() {
            ["areas"] => Ok(db
                .all_file_areas()?
                .iter()
                .map(|a| format!("{} | {} | {}", a.number, a.name, a.active))
                .collect::<Vec<_>>()
                .join("\n")),
            ["area", n, key, name] => {
                let a = db.create_file_area(&FileAreaDefinition {
                    number: number(n)?,
                    name: (*name).into(),
                    description: (*name).into(),
                    storage_key: (*key).into(),
                    access_mode: FileAccessMode::AtLeast,
                    read_security: SecurityLevel::new(10).map_err(|_| invalid())?,
                    upload_security: SecurityLevel::new(10).map_err(|_| invalid())?,
                    preview: false,
                    no_charge: false,
                    maximum_upload_bytes: 64 * 1024 * 1024,
                    privileged_security_levels: vec![],
                })?;
                db.set_file_safety_policy(actor, a.id, &SafetyPolicy::default())?;
                Ok(format!("{}", a.number))
            }
            ["edit-area", n, name, description, read, upload] => {
                let a = area(db, n)?;
                db.update_file_area(
                    a.number,
                    &FileAreaDefinition {
                        number: a.number,
                        name: (*name).into(),
                        description: (*description).into(),
                        storage_key: a.storage_key,
                        access_mode: a.access_mode,
                        read_security: SecurityLevel::new(number(read)?).map_err(|_| invalid())?,
                        upload_security: SecurityLevel::new(number(upload)?)
                            .map_err(|_| invalid())?,
                        preview: a.preview,
                        no_charge: a.no_charge,
                        maximum_upload_bytes: a.maximum_upload_bytes,
                        privileged_security_levels: a.privileged_security_levels,
                    },
                )?;
                Ok("updated".into())
            }
            ["enable-area", n, enabled] => {
                db.set_file_area_enabled(
                    number(n)?,
                    match *enabled {
                        "yes" => true,
                        "no" => false,
                        _ => return Err(invalid()),
                    },
                )?;
                Ok("updated".into())
            }
            ["limits", n, source, expanded, member, members, nesting, ratio] => {
                let a = area(db, n)?;
                let mut policy = db.file_safety_policy(a.id)?;
                policy.archives.source_bytes = source.parse().map_err(|_| invalid())?;
                policy.archives.expanded_bytes = expanded.parse().map_err(|_| invalid())?;
                policy.archives.member_bytes = member.parse().map_err(|_| invalid())?;
                policy.archives.members = members.parse().map_err(|_| invalid())?;
                policy.archives.nesting = nesting.parse().map_err(|_| invalid())?;
                policy.archives.ratio = ratio.parse().map_err(|_| invalid())?;
                db.set_file_safety_policy(actor, a.id, &policy)?;
                Ok("updated".into())
            }
            ["policy", n, scan, approval] => {
                let a = area(db, n)?;
                let mut policy = db.file_safety_policy(a.id)?;
                policy.scanning = match *scan {
                    "required" => ScanPolicy::Required,
                    "optional" => ScanPolicy::Optional,
                    "disabled" => ScanPolicy::Disabled,
                    _ => return Err(invalid()),
                };
                policy.approval_required = match *approval {
                    "approval" => true,
                    "automatic" => false,
                    _ => return Err(invalid()),
                };
                db.set_file_safety_policy(actor, a.id, &policy)?;
                Ok("updated".into())
            }
            ["scanner", n, address] => {
                let a = area(db, n)?;
                let mut policy = db.file_safety_policy(a.id)?;
                policy.scanner = if *address == "none" {
                    None
                } else {
                    Some(sf_core::files::scanner::Clamd {
                        address: address.parse().map_err(|_| invalid())?,
                    })
                };
                db.set_file_safety_policy(actor, a.id, &policy)?;
                Ok("updated".into())
            }
            ["import", n, path, filename, description] => {
                let a = area(db, n)?;
                let metadata = std::fs::symlink_metadata(path)?;
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(invalid());
                }
                let mut input = File::open(path)?;
                let imported = store.import_file(
                    db,
                    actor,
                    &a,
                    filename,
                    description,
                    &mut input,
                    "operator",
                    None,
                )?;
                Ok(format!(
                    "{} | {} | duplicate={} | duplicate-description={} | existing={}",
                    imported.file.id.get(),
                    imported.file.sha256,
                    imported.duplicate_content,
                    imported.duplicate_description,
                    imported.existing
                ))
            }
            ["list", n] => {
                let a = area(db, n)?;
                Ok(db
                    .all_cataloged_files()?
                    .iter()
                    .filter(|(_, f)| f.area_id == a.id)
                    .map(|(_, f)| {
                        format!(
                            "{} | {} | {} | {:?}",
                            f.id.get(),
                            f.filename,
                            f.size_bytes,
                            f.lifecycle
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            ["status"] => Ok(serde_json::to_string_pretty(&db.file_admissions(actor)?)?),
            ["approve", f] | ["reject", f] => Ok(serde_json::to_string_pretty(
                &db.review_file_admission(actor, id(f)?, action == "approve")?,
            )?),
            ["rescan", f] => Ok(serde_json::to_string_pretty(&store.rescan_file(
                db,
                actor,
                id(f)?,
                None,
            )?)?),
            ["description", f, "use"] => {
                db.use_file_description(actor, id(f)?, None)?;
                Ok("updated".into())
            }
            ["description", f, "edit", text] => {
                db.use_file_description(actor, id(f)?, Some(text))?;
                Ok("updated".into())
            }
            ["description", _, "ignore"] => Ok("unchanged".into()),
            ["integrity"] => Ok(serde_json::to_string_pretty(&store.check_content(db)?)?),
            _ => Err(invalid()),
        }
    })?;
    Ok(output)
}
