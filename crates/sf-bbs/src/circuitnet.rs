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
    if matches!(
        *action,
        "test-link"
            | "poll"
            | "live-status"
            | "hold"
            | "release"
            | "live-retry"
            | "live-subscribe"
            | "live-unsubscribe"
    ) {
        return live_command(config, action, &network, rest);
    }
    let authority = OfflineConfiguration::open(config)?;
    let now = chrono::Utc::now().timestamp();
    let config_cap = LocalOperatorCapability::ChangeSensitiveConfiguration;
    let run_cap = LocalOperatorCapability::NetworkRun;
    let read_cap = LocalOperatorCapability::ReadConfiguration;
    match (*action, rest) {
        ("identity", [certificate, key]) => {
            let certificate = read(certificate)?;
            let secret = read(key)?;
            let cfg = sf_core::RuntimeConfig::load(config)?;
            let paths = sf_core::LogicalPaths::resolve(
                config.parent().ok_or_else(usage)?,
                &cfg.validate()?,
            )?;
            authority.circuitnet(config_cap, |db, _, actor| {
                let (mut c, v) = db.circuitnet_live(&network)?;
                crate::circuitnet_live::install_identity(
                    paths.get(sf_core::LogicalPath::System),
                    &certificate,
                    secret,
                )
                .map_err(|_| Error::Policy)?;
                c.certificate = certificate;
                db.circuitnet_configure_live(actor, &network, &c, v, now)
            })?;
        }
        ("listener", [address]) => {
            let address = if *address == "off" {
                None
            } else {
                Some(address.parse().map_err(|_| usage())?)
            };
            authority.circuitnet(config_cap, |db, _, actor| {
                let (mut c, v) = db.circuitnet_live(&network)?;
                c.listener = address;
                db.circuitnet_configure_live(actor, &network, &c, v, now)
            })?;
        }
        ("peer", [node, host, port, server_name, certificate, inbound, outbound]) => {
            let node = codec(NodeId::new(node))?;
            let port = port.parse().map_err(|_| usage())?;
            let certificate = read(certificate)?;
            let flag = |s: &str| match s {
                "yes" => Ok(true),
                "no" => Ok(false),
                _ => Err(usage()),
            };
            let inbound = flag(inbound)?;
            let outbound = flag(outbound)?;
            authority.circuitnet(config_cap, |db, _, actor| {
                let (mut c, v) = db.circuitnet_live(&network)?;
                c.peers.retain(|p| p.node != node);
                c.peers.push(live::Peer {
                    node,
                    host: (*host).into(),
                    port,
                    server_name: (*server_name).into(),
                    certificate,
                    enabled: true,
                    inbound,
                    outbound,
                    held: false,
                });
                db.circuitnet_configure_live(actor, &network, &c, v, now)
            })?;
        }
        ("peer-enabled", [node, enabled]) => {
            let node = codec(NodeId::new(node))?;
            let enabled = match *enabled {
                "yes" => true,
                "no" => false,
                _ => return Err(usage()),
            };
            authority.circuitnet(config_cap, |db, _, actor| {
                let (mut c, v) = db.circuitnet_live(&network)?;
                c.peers
                    .iter_mut()
                    .find(|p| p.node == node)
                    .ok_or(Error::Policy)?
                    .enabled = enabled;
                db.circuitnet_configure_live(actor, &network, &c, v, now)
            })?;
        }
        ("init", [local, name, trust, nodes @ ..])
            if matches!(*trust, "--trusted-offline" | "--live") =>
        {
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
                trusted_offline: *trust == "--trusted-offline",
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

fn live_command(
    config: &Path,
    action: &str,
    network: &NetworkId,
    rest: &[&str],
) -> Result<String, ApplicationError> {
    use crate::circuitnet_live::Action;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| usage())?;
    rt.block_on(async {
        let mut client = crate::OperatorClient::connect(config).await?;
        client.describe_operator_controls().await?;
        let snapshot = client
            .networks(sf_core::ftn::NetworkQuery {
                section: sf_core::ftn::NetworkSection::Circuitnet,
                offset: 0,
            })
            .await?;
        let status = snapshot
            .circuitnet
            .iter()
            .find(|n| &n.network == network)
            .ok_or_else(usage)?;
        if action == "live-status" && rest.is_empty() {
            return serde_json::to_string_pretty(status).map_err(|_| usage());
        }
        let request = match (action, rest) {
            ("test-link", [node]) => Action::Test {
                network: network.clone(),
                node: codec(NodeId::new(node))?,
            },
            ("poll", [node]) => Action::Poll {
                network: network.clone(),
                node: codec(NodeId::new(node))?,
            },
            ("hold" | "release", [node]) => Action::Hold {
                network: network.clone(),
                node: codec(NodeId::new(node))?,
                held: action == "hold",
                expected: status.version,
            },
            ("live-retry", [queue, version]) => Action::Retry {
                network: network.clone(),
                queue: (*queue).into(),
                expected: version.parse().map_err(|_| usage())?,
            },
            ("live-subscribe" | "live-unsubscribe", [node, code, version]) => Action::Subscribe {
                network: network.clone(),
                node: codec(NodeId::new(node))?,
                codename: codec(Codename::new(code))?,
                subscribed: action == "live-subscribe",
                expected: version.parse().map_err(|_| usage())?,
            },
            _ => return Err(usage()),
        };
        let result = client
            .qwk_network_action(
                crate::operator_control::random_token(),
                crate::NetworkAction::Circuitnet { request },
            )
            .await?;
        if matches!(result, crate::NetworkResult::Rejected { .. }) {
            return Err(ApplicationError::Transport(
                "CircuitNET operator action rejected".into(),
            ));
        }
        serde_json::to_string_pretty(&result).map_err(|_| usage())
    })
}
