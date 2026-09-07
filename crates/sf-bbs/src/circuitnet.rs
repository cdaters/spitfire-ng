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

//! Explicit cold-board operator binding for development/offline CircuitNET.
use crate::{ApplicationError, OfflineConfiguration};
use sf_core::{circuitnet::*, LocalOperatorCapability};
use sf_net::circuitnet::MAX_ARTIFACT;
use std::{
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::Path,
};
fn usage() -> ApplicationError {
    ApplicationError::Usage(crate::op("circuitnet-usage"))
}
fn io_error() -> ApplicationError {
    ApplicationError::Usage(crate::op("circuitnet-file-error"))
}
fn read(path: &str) -> Result<Vec<u8>, ApplicationError> {
    let meta = fs::symlink_metadata(path).map_err(|_| io_error())?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_ARTIFACT as u64 {
        return Err(io_error());
    }
    let mut bytes = vec![];
    fs::File::open(path)
        .map_err(|_| io_error())?
        .take(MAX_ARTIFACT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io_error())?;
    if bytes.len() > MAX_ARTIFACT {
        return Err(io_error());
    }
    Ok(bytes)
}
fn write(path: &str, bytes: &[u8]) -> Result<(), ApplicationError> {
    let path = Path::new(path);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| io_error())?;
    temp.write_all(bytes).map_err(|_| io_error())?;
    temp.as_file().sync_all().map_err(|_| io_error())?;
    temp.persist_noclobber(path).map_err(|_| io_error())?;
    Ok(())
}
fn role(s: &str) -> Result<Role, ApplicationError> {
    match s.to_ascii_uppercase().as_str() {
        "END" => Ok(Role::End),
        "HOST" => Ok(Role::Host),
        "ROOT" => Ok(Role::Root),
        _ => Err(usage()),
    }
}
fn codec<T>(result: Result<T, sf_net::circuitnet::Error>) -> Result<T, ApplicationError> {
    result.map_err(|e| ApplicationError::CircuitNet(e.into()))
}
/// Arguments are named operations, never raw SQL/TOML or a trusted packet filename.
pub fn run(config: &Path, args: &[OsString]) -> Result<String, ApplicationError> {
    let args = args
        .iter()
        .map(|s| s.to_str().ok_or_else(usage))
        .collect::<Result<Vec<_>, _>>()?;
    let [action, network, rest @ ..] = args.as_slice() else {
        return Err(usage());
    };
    let network = codec(NetworkId::new(network))?;
    let authority = OfflineConfiguration::open(config)?;
    let now = chrono::Utc::now().timestamp();
    let config_cap = LocalOperatorCapability::ChangeSensitiveConfiguration;
    let run_cap = LocalOperatorCapability::NetworkRun;
    let read_cap = LocalOperatorCapability::ReadConfiguration;
    match (*action, rest) {
        ("init", [local, name, trust, nodes @ ..]) if *trust == "--trusted-offline" => {
            let topology = Topology {
                nodes: nodes
                    .iter()
                    .map(|s| {
                        let parts = s.split(':').collect::<Vec<_>>();
                        let [id, r, parent] = parts.as_slice() else {
                            return Err(usage());
                        };
                        Ok(Node {
                            id: codec(NodeId::new(id))?,
                            role: role(r)?,
                            parent: if *parent == "-" {
                                None
                            } else {
                                Some(codec(NodeId::new(parent))?)
                            },
                        })
                    })
                    .collect::<Result<_, ApplicationError>>()?,
            };
            let p = Profile {
                network: network.clone(),
                display_name: (*name).into(),
                local: codec(NodeId::new(local))?,
                enabled: true,
                trusted_offline: true,
                topology,
            };
            authority.circuitnet(config_cap, |db, _, actor| {
                db.circuitnet_configure(actor, &p, 0, now)
            })?;
        }
        ("enable", [enabled]) => {
            let enabled = match *enabled {
                "yes" => true,
                "no" => false,
                _ => return Err(usage()),
            };
            authority.circuitnet(config_cap, |db, _, actor| {
                let mut s = db.circuitnet_status(&network)?;
                s.profile.enabled = enabled;
                db.circuitnet_configure(actor, &s.profile, s.version, now)
            })?;
        }
        ("map", [number, code]) => {
            let number: u16 = number.parse().map_err(|_| usage())?;
            let codename = codec(Codename::new(code))?;
            authority.circuitnet(config_cap, |db, _, actor| {
                let conference = db
                    .all_conferences()?
                    .into_iter()
                    .find(|c| c.number == number)
                    .ok_or(Error::Policy)?;
                let m = Mapping {
                    codename,
                    conference: conference.id.get(),
                    send: true,
                    receive: true,
                };
                db.circuitnet_map(actor, &network, &m, 0, now)
            })?;
        }
        ("subscribe" | "unsubscribe", [neighbor, code]) => {
            let neighbor = codec(NodeId::new(neighbor))?;
            let codename = codec(Codename::new(code))?;
            authority.circuitnet(config_cap, |db, _, actor| {
                let s = db.circuitnet_status(&network)?;
                let version = s
                    .dossiers
                    .iter()
                    .find(|d| d.neighbor == neighbor && d.codename == codename)
                    .map_or(0, |d| d.version);
                db.circuitnet_subscribe(
                    actor,
                    &network,
                    &Dossier {
                        neighbor,
                        codename,
                        subscribed: *action == "subscribe",
                        version,
                    },
                    now,
                )
            })?;
        }
        ("status", []) => {
            return authority.circuitnet(read_cap, |db, _, _| {
                Ok(serde_json::to_string_pretty(
                    &db.circuitnet_status(&network)?,
                )?)
            })
        }
        ("queue", after) if after.len() <= 1 => {
            return authority.circuitnet(read_cap, |db, _, _| {
                Ok(serde_json::to_string_pretty(&db.circuitnet_queue(
                    &network,
                    after.first().copied().unwrap_or(""),
                )?)?)
            })
        }
        ("scan", []) => {
            authority.circuitnet(run_cap, |db, _, _| {
                let mut after = 0;
                for _ in 0..100 {
                    let (_, next) = db.circuitnet_scan(&network, after, now)?;
                    if next == after {
                        return Ok(());
                    }
                    after = next;
                }
                Ok(())
            })?;
        }
        ("export", [neighbor, file]) => {
            let neighbor = codec(NodeId::new(neighbor))?;
            let prepared = authority.circuitnet(run_cap, |db, store, _| {
                db.circuitnet_prepare(store, &network, &neighbor, now)
            })?;
            if let Err(error) = write(file, &prepared.bytes) {
                authority.circuitnet(run_cap, |db, _, _| {
                    db.circuitnet_failed(&network, &prepared.artifact, now)
                })?;
                return Err(error);
            }
        }
        ("import", [neighbor, file, receipt]) => {
            let neighbor = codec(NodeId::new(neighbor))?;
            let bytes = read(file)?;
            let result = authority.circuitnet(run_cap, |db, store, _| {
                db.circuitnet_import(store, &network, &neighbor, &bytes, now)
            })?;
            write(receipt, &result.receipt)?;
        }
        ("ack", [neighbor, file]) => {
            let neighbor = codec(NodeId::new(neighbor))?;
            let bytes = read(file)?;
            authority.circuitnet(run_cap, |db, store, _| {
                db.circuitnet_acknowledge(store, &network, &neighbor, &bytes, now)
            })?;
        }
        ("retry", [queue, version]) => {
            let version = version.parse().map_err(|_| usage())?;
            authority.circuitnet(run_cap, |db, _, _| {
                db.circuitnet_retry(&network, queue, version)
            })?;
        }
        _ => return Err(usage()),
    }
    Ok(crate::op("circuitnet-completed"))
}
