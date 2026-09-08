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

//! Daemon-owned native CircuitNET links. No FTN protocol or message authority.
mod tls;
use crate::{ApplicationError, BoardRuntime};
use serde::{Deserialize, Serialize};
use sf_core::{
    circuitnet::{
        self as core,
        live::{Config, Health, Peer},
        NetworkId, NodeId, Profile,
    },
    LogicalPath, RuntimeDatabase,
};
use sf_net::circuitnet::{
    self as envelope,
    transport::{self as wire, Error, Frame, Hello, Mode},
};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct State {
    count: AtomicUsize,
    active: Mutex<BTreeSet<(String, String)>>,
    listeners: Mutex<BTreeSet<String>>,
}
struct Permit {
    runtime: Arc<BoardRuntime>,
    identity: Option<(String, String)>,
}
impl Permit {
    fn acquire(runtime: Arc<BoardRuntime>) -> Result<Self, Error> {
        runtime
            .circuitnet_live
            .count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                (v < 4).then_some(v + 1)
            })
            .map_err(|_| Error::Busy)?;
        Ok(Self {
            runtime,
            identity: None,
        })
    }
    fn bind(&mut self, net: &NetworkId, node: &NodeId) -> Result<(), Error> {
        let key = (net.to_string(), node.to_string());
        if !self
            .runtime
            .circuitnet_live
            .active
            .lock()
            .map_err(|_| Error::Custody)?
            .insert(key.clone())
        {
            return Err(Error::Busy);
        }
        self.identity = Some(key);
        Ok(())
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        if let Some(key) = &self.identity {
            if let Ok(mut active) = self.runtime.circuitnet_live.active.lock() {
                active.remove(key);
            }
        }
        self.runtime
            .circuitnet_live
            .count
            .fetch_sub(1, Ordering::AcqRel);
    }
}
fn custody<T, E>(r: Result<T, E>) -> Result<T, Error> {
    r.map_err(|_| Error::Custody)
}
// Safe operation/error classes only: underlying errors can contain local paths.
fn file_custody<T>(r: Result<T, core::Error>, operation: &'static str) -> Result<T, Error> {
    r.map_err(|error| {
        let (class, sqlite_code) = match &error {
            core::Error::Sqlite(error)
            | core::Error::Files(sf_core::files::FilesError::Sql(error)) => (
                "database",
                error.sqlite_error().map_or(0, |code| code.extended_code),
            ),
            core::Error::Files(_) => ("native-files", 0),
            core::Error::Policy => ("policy", 0),
            core::Error::Conflict => ("conflict", 0),
            _ => ("custody", 0),
        };
        tracing::warn!(
            operation,
            error_class = class,
            sqlite_code,
            "CircuitNET file operation failed"
        );
        Error::Custody
    })
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
fn db(runtime: &BoardRuntime) -> Result<RuntimeDatabase, Error> {
    let mut d = custody(RuntimeDatabase::open(runtime.database_path()))?;
    d.bind_posting_identity_configuration(&custody(runtime.configuration.current())?);
    Ok(d)
}
fn private(path: &Path, directory: bool) -> Result<(), Error> {
    let m = custody(fs::symlink_metadata(path))?;
    if m.file_type().is_symlink() || (directory && !m.is_dir()) || (!directory && !m.is_file()) {
        return Err(Error::Custody);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if m.permissions().mode() & 0o077 != 0 {
            return Err(Error::Custody);
        }
    }
    Ok(())
}
fn key_root(system: &Path) -> Result<std::path::PathBuf, Error> {
    let root = system.join("circuitnet-credentials");
    if !root.exists() {
        let mut b = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        custody(b.create(&root))?;
    }
    private(&root, true)?;
    Ok(root)
}
pub(crate) fn install_identity(
    system: &Path,
    certificate: &[u8],
    key: Vec<u8>,
) -> Result<(), Error> {
    if certificate.is_empty() || certificate.len() > 16384 || key.len() > 16384 {
        return Err(Error::Oversized);
    }
    tls::validate_identity(certificate, key.clone())?;
    let root = key_root(system)?;
    let path = root.join(envelope::digest(certificate));
    if path.exists() {
        private(&path, false)?;
        if custody(fs::read(path))? == key {
            return Ok(());
        }
        return Err(Error::Custody);
    }
    let mut temp = custody(tempfile::NamedTempFile::new_in(&root))?;
    custody(temp.write_all(&key))?;
    custody(temp.as_file().sync_all())?;
    custody(temp.persist_noclobber(path))?;
    Ok(())
}
fn key(runtime: &BoardRuntime, c: &Config) -> Result<Vec<u8>, Error> {
    let path =
        key_root(runtime.paths.get(LogicalPath::System))?.join(envelope::digest(&c.certificate));
    private(&path, false)?;
    let mut bytes = vec![];
    custody(
        custody(fs::File::open(path))?
            .take(16385)
            .read_to_end(&mut bytes),
    )?;
    if bytes.len() > 16384 {
        return Err(Error::Oversized);
    }
    Ok(bytes)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Action {
    ControlPolicy {
        network: NetworkId,
        policy: core::control::Policy,
        expected: i64,
    },
    RequestSubscription {
        network: NetworkId,
        change: core::control::Operation,
        codename: Option<core::Codename>,
    },
    DecideControl {
        network: NetworkId,
        id: envelope::MessageId,
        approve: bool,
    },
    RetryControl {
        network: NetworkId,
        id: envelope::MessageId,
    },
    Direct {
        network: NetworkId,
        message: i64,
        destination: NodeId,
    },
    Test {
        network: NetworkId,
        node: NodeId,
    },
    Poll {
        network: NetworkId,
        node: NodeId,
    },
    Hold {
        network: NetworkId,
        node: NodeId,
        held: bool,
        expected: i64,
    },
    Retry {
        network: NetworkId,
        queue: String,
        expected: i64,
    },
    Subscribe {
        network: NetworkId,
        node: NodeId,
        codename: core::Codename,
        subscribed: bool,
        expected: i64,
    },
}
impl Action {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::ControlPolicy { .. } => "circuitnet.control-policy",
            Self::RequestSubscription { .. } => "circuitnet.control-request",
            Self::DecideControl { .. } => "circuitnet.control-decision",
            Self::RetryControl { .. } => "circuitnet.control-retry",
            Self::Direct { .. } => "circuitnet.direct",
            Self::Test { .. } => "circuitnet.test",
            Self::Poll { .. } => "circuitnet.poll",
            Self::Hold { .. } => "circuitnet.hold",
            Self::Retry { .. } => "circuitnet.retry",
            Self::Subscribe { .. } => "circuitnet.dossier",
        }
    }
    pub fn capability(&self) -> sf_core::LocalOperatorCapability {
        use sf_core::LocalOperatorCapability as C;
        match self {
            Self::Test { .. } => C::NetworkTest,
            Self::Poll { .. } => C::NetworkRun,
            Self::Hold { .. } | Self::Retry { .. } => C::NetworkQueue,
            Self::Subscribe { .. }
            | Self::ControlPolicy { .. }
            | Self::RequestSubscription { .. }
            | Self::DecideControl { .. }
            | Self::RetryControl { .. }
            | Self::Direct { .. } => C::ChangeSensitiveConfiguration,
        }
    }
}
pub(crate) fn dispatch(
    runtime: &Arc<BoardRuntime>,
    actor: &str,
    action: &Action,
) -> Result<crate::NetworkResult, ApplicationError> {
    let mut d = db(runtime).map_err(application)?;
    match action {
        Action::ControlPolicy {
            network,
            policy,
            expected,
        } => d.circuitnet_set_control_policy(actor, network, *policy, *expected, now())?,
        Action::RequestSubscription {
            network,
            change,
            codename,
        } => {
            d.circuitnet_request(network, *change, codename.clone(), now())?;
        }
        Action::DecideControl {
            network,
            id,
            approve,
        } => {
            d.circuitnet_decide_control(actor, network, id, *approve, now())?;
        }
        Action::RetryControl { network, id } => d.circuitnet_retry_control(network, id)?,
        Action::Direct {
            network,
            message,
            destination,
        } => d.circuitnet_direct(network, *message, destination, now())?,
        Action::Hold {
            network,
            node,
            held,
            expected,
        } => {
            let (mut c, _) = d.circuitnet_live(network)?;
            c.peers
                .iter_mut()
                .find(|p| &p.node == node)
                .ok_or(core::Error::Policy)?
                .held = *held;
            d.circuitnet_configure_live(actor, network, &c, *expected, now())?;
        }
        Action::Retry {
            network,
            queue,
            expected,
        } => d.circuitnet_retry(network, queue, *expected)?,
        Action::Subscribe {
            network,
            node,
            codename,
            subscribed,
            expected,
        } => d.circuitnet_subscribe(
            actor,
            network,
            &core::Dossier {
                neighbor: node.clone(),
                codename: codename.clone(),
                subscribed: *subscribed,
                version: *expected,
            },
            now(),
        )?,
        Action::Test { network, node } | Action::Poll { network, node } => {
            let mode = if matches!(action, Action::Test { .. }) {
                Mode::Test
            } else {
                Mode::Poll
            };
            let (c, _) = d.circuitnet_live(network)?;
            c.peer(node, false)?;
            let mut permit = Permit::acquire(runtime.clone()).map_err(application)?;
            permit.bind(network, node).map_err(application)?;
            let work = runtime.live_controls.track();
            let runtime = runtime.clone();
            let network = network.clone();
            let node = node.clone();
            thread::spawn(move || {
                let _work = work;
                let _permit = permit;
                for retry in 0..3 {
                    if runtime.shutdown_in_progress().unwrap_or(true) {
                        break;
                    }
                    let result = outbound(&runtime, &network, &node, mode, retry);
                    if !matches!(
                        result,
                        Err(Error::Connect | Error::Interrupted | Error::Timeout)
                    ) {
                        break;
                    }
                    if retry < 2 {
                        thread::sleep(Duration::from_secs(1 << retry));
                    }
                }
            });
        }
    }
    Ok(crate::NetworkResult::Updated)
}
fn application(e: Error) -> ApplicationError {
    ApplicationError::Transport(e.to_string())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkStatus {
    #[serde(default)]
    pub next_exchange: Option<i64>,
    pub node: NodeId,
    pub role: core::Role,
    pub host: String,
    pub port: u16,
    pub server_name: String,
    pub enabled: bool,
    pub inbound: bool,
    pub outbound: bool,
    pub held: bool,
    pub active: bool,
    pub queued: u32,
    pub dossiers: usize,
    pub health: Option<Health>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlStatus {
    #[serde(flatten)]
    pub entry: core::control::Entry,
    pub subscription_count: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub files: core::files::Status,
    pub network: NetworkId,
    pub local: NodeId,
    pub role: core::Role,
    pub enabled: bool,
    pub listener: Option<std::net::SocketAddr>,
    pub listening: bool,
    pub credential: bool,
    pub version: i64,
    pub peers: Vec<LinkStatus>,
    pub topology: core::Topology,
    pub control_policy: (core::control::Policy, i64),
    pub controls: Vec<ControlStatus>,
    pub pending_approvals: u32,
    pub directed_pending: u32,
    pub directed_failures: u32,
    pub subscriptions: Vec<core::Dossier>,
    pub more: bool,
}
impl std::ops::Deref for ControlStatus {
    type Target = core::control::Entry;
    fn deref(&self) -> &Self::Target {
        &self.entry
    }
}
pub(crate) fn status(runtime: &BoardRuntime, offset: u32) -> Result<Vec<Status>, ApplicationError> {
    let d = db(runtime).map_err(application)?;
    let events = d.events()?;
    let mut result = vec![];
    for network in d.circuitnet_profiles()? {
        let s = d.circuitnet_status(&network)?;
        let (c, version) = d.circuitnet_live(&network)?;
        let peers = c
            .peers
            .iter()
            .map(|p| {
                Ok(LinkStatus {
                    next_exchange: events.iter().filter(|e|e.definition.enabled && matches!(&e.definition.action,sf_core::events::Action::Circuitnet{network:n,node} if n==&network && node.as_ref().is_none_or(|n|n==&p.node))).filter_map(|e|e.next_due).min(),
                    node: p.node.clone(),
                    role: s
                        .profile
                        .topology
                        .node(&p.node)
                        .map_err(core::Error::from)?
                        .role,
                    host: p.host.clone(),
                    port: p.port,
                    server_name: p.server_name.clone(),
                    enabled: p.enabled,
                    inbound: p.inbound,
                    outbound: p.outbound,
                    held: p.held,
                    active: runtime
                        .circuitnet_live
                        .active
                        .lock()
                        .is_ok_and(|a| a.contains(&(network.to_string(), p.node.to_string()))),
                    queued: d.circuitnet_neighbor_pending(&network, &p.node)?,
                    dossiers: s
                        .dossiers
                        .iter()
                        .filter(|v| v.neighbor == p.node && v.subscribed)
                        .count(),
                    health: d.circuitnet_link_health(&network, &p.node)?,
                })
            })
            .collect::<Result<Vec<_>, core::Error>>()?;
        let controls = d.circuitnet_controls_page(&network, offset)?;
        let more = controls.len() > 16 || s.dossiers.len() > offset as usize + 16;
        result.push(Status {
            files: d.circuitnet_file_status(&network)?,
            local: s.profile.local.clone(),
            role: s
                .profile
                .topology
                .node(&s.profile.local)
                .map_err(core::Error::from)?
                .role,
            enabled: s.profile.enabled,
            listener: c.listener,
            listening: runtime
                .circuitnet_live
                .listeners
                .lock()
                .is_ok_and(|a| a.contains(network.as_str())),
            credential: key(runtime, &c)
                .and_then(|key| tls::validate_identity(&c.certificate, key))
                .is_ok(),
            version,
            topology: s.profile.topology.clone(),
            control_policy: d.circuitnet_control_policy(&network)?,
            more,
            controls: controls
                .into_iter()
                .take(16)
                .map(|mut entry| {
                    let subscription_count = entry.result.subscriptions.len();
                    entry.result.subscriptions.truncate(16);
                    ControlStatus {
                        entry,
                        subscription_count,
                    }
                })
                .collect(),
            pending_approvals: d.circuitnet_control_counts(&network)?.0,
            directed_pending: d.circuitnet_control_counts(&network)?.1,
            directed_failures: d.circuitnet_control_counts(&network)?.2,
            subscriptions: s
                .dossiers
                .into_iter()
                .skip(offset as usize)
                .take(16)
                .collect(),
            network,
            peers,
        });
    }
    Ok(result)
}
struct Session<'a> {
    runtime: &'a BoardRuntime,
    profile: Profile,
    peer: Peer,
    inbound: bool,
    health: Health,
    pending: Option<String>,
    started: Instant,
    directed: bool,
    controls: bool,
    files: bool,
    hash_have: bool,
}
impl<'a> Session<'a> {
    fn new(
        runtime: &'a BoardRuntime,
        profile: Profile,
        peer: Peer,
        inbound: bool,
        retry: u32,
    ) -> Self {
        Self {
            runtime,
            profile,
            peer,
            inbound,
            health: Health {
                last_attempt: now(),
                retry,
                ..Health::default()
            },
            pending: None,
            started: Instant::now(),
            directed: false,
            controls: false,
            files: false,
            hash_have: false,
        }
    }
    fn admitted(&self) -> Result<RuntimeDatabase, Error> {
        if self.runtime.shutdown_in_progress().unwrap_or(true) {
            return Err(Error::Interrupted);
        }
        let d = db(self.runtime)?;
        let current = custody(d.circuitnet_status(&self.profile.network))?.profile;
        let (c, _) = custody(d.circuitnet_live(&self.profile.network))?;
        let peer = c
            .peer(&self.peer.node, self.inbound)
            .map_err(|_| Error::Held)?;
        if !current.enabled
            || current.local != self.profile.local
            || current.topology != self.profile.topology
            || peer.certificate != self.peer.certificate
        {
            return Err(Error::AuthFailed);
        }
        Ok(d)
    }
    fn hello(&mut self, ch: &mut tls::Channel, remote: Hello, mode: Mode) -> Result<Hello, Error> {
        let local = Hello::new(
            self.profile.network.clone(),
            self.profile.local.clone(),
            custody(self.profile.topology.node(&self.profile.local))?.role,
            mode,
        );
        let minor = local.negotiate(&remote)?;
        if remote.node != self.peer.node {
            return Err(Error::UnknownNode);
        }
        if remote.role != custody(self.profile.topology.node(&self.peer.node))?.role {
            return Err(Error::TopologyMismatch);
        }
        if ch.certificate()? != self.peer.certificate {
            return Err(Error::AuthFailed);
        }
        self.admitted()?;
        (self.directed, self.controls) = local.c4_capabilities(&remote)?;
        (self.files, self.hash_have) = local.file_capabilities(&remote)?;
        self.health.protocol_minor = Some(minor);
        Ok(local)
    }
    fn send_files(&mut self, ch: &mut tls::Channel) -> Result<(), Error> {
        if !self.files {
            return Ok(());
        }
        use envelope::files::{self as files, Want};
        let storage = custody(sf_core::FileStorage::open_existing(&self.runtime.paths))?;
        let offers = {
            let _guard = self
                .runtime
                .network_lock
                .lock()
                .map_err(|_| Error::Custody)?;
            file_custody(
                self.admitted()?.circuitnet_file_offers(
                    &self.profile.network,
                    &self.peer.node,
                    now(),
                ),
                "file-offers",
            )?
        };
        for publication in offers {
            custody(self.admitted()?.circuitnet_file_attempt(
                &self.profile.network,
                &self.peer.node,
                &publication.id,
                None,
                now(),
            ))?;
            let mut payload = custody(storage.open_content(&publication.sha256, publication.size))?;
            ch.send(&Frame::FileOffer {
                publication: Some(publication.clone()),
            })?;
            let Frame::FileWant { want } = ch.receive_file()? else {
                return Err(Error::MalformedFrame);
            };
            let receipt = match want {
                Want::Complete { receipt } => receipt,
                Want::Send | Want::Have => {
                    if matches!(want, Want::Have) && !self.hash_have {
                        return Err(Error::MalformedFrame);
                    }
                    if matches!(want, Want::Send) {
                        ch.file_deadline();
                        self.health.file_bytes += files::send(&mut payload, ch, publication.size)?;
                    }
                    let Frame::FileReceipt { receipt } = ch.receive_file()? else {
                        return Err(Error::MalformedFrame);
                    };
                    receipt
                }
            };
            custody(self.admitted()?.circuitnet_file_attempt(
                &self.profile.network,
                &self.peer.node,
                &publication.id,
                Some(&receipt),
                now(),
            ))?;
            self.health.files_sent += 1;
        }
        ch.send(&Frame::FileOffer { publication: None })?;
        Ok(())
    }
    fn receive_files(&mut self, ch: &mut tls::Channel) -> Result<(), Error> {
        if !self.files {
            return Ok(());
        }
        use envelope::files::{self as files, Want};
        use std::io::Seek;
        let storage = custody(sf_core::FileStorage::open_existing(&self.runtime.paths))?;
        for count in 0..=files::MAX_PUBLICATIONS {
            let Frame::FileOffer { publication } = ch.receive_file()? else {
                return Err(Error::MalformedFrame);
            };
            let Some(publication) = publication else {
                return Ok(());
            };
            if count == files::MAX_PUBLICATIONS {
                return Err(Error::Oversized);
            }
            let mut want = custody(self.admitted()?.circuitnet_file_offer(
                &storage,
                &self.profile.network,
                &self.peer.node,
                &publication,
            ))?;
            if matches!(want, Want::Have) && !self.hash_have {
                want = Want::Send;
            }
            ch.send(&Frame::FileWant { want: want.clone() })?;
            let mut payload = match want {
                Want::Complete { .. } => continue,
                Want::Have => custody(storage.open_content(&publication.sha256, publication.size))?,
                Want::Send => {
                    let mut temp = custody(tempfile::tempfile())?;
                    ch.file_deadline();
                    match files::receive(ch, &mut temp, publication.size, &publication.sha256) {
                        Ok(bytes) => self.health.file_bytes += bytes,
                        Err(Error::ConflictingMessage) => {
                            let receipt = custody(self.admitted()?.circuitnet_file_reject_hash(
                                &storage,
                                &self.profile.network,
                                &self.peer.node,
                                &publication,
                                now(),
                            ))?;
                            ch.send(&Frame::FileReceipt { receipt })?;
                            continue;
                        }
                        Err(error) => return Err(error),
                    }
                    custody(temp.sync_all())?;
                    custody(temp.rewind())?;
                    temp
                }
            };
            let receipt = {
                let _guard = self
                    .runtime
                    .network_lock
                    .lock()
                    .map_err(|_| Error::Custody)?;
                file_custody(
                    self.admitted()?.circuitnet_file_receive(
                        &storage,
                        &self.profile.network,
                        &self.peer.node,
                        &publication,
                        &mut payload,
                        now(),
                    ),
                    "file-admission",
                )?
            };
            ch.send(&Frame::FileReceipt { receipt })?;
            self.health.files_received += 1;
        }
        Err(Error::Oversized)
    }
    fn send_controls(&mut self, ch: &mut tls::Channel) -> Result<(), Error> {
        if !self.controls {
            return Ok(());
        }
        let requests = custody(
            self.admitted()?
                .circuitnet_pending_controls(&self.profile.network, &self.peer.node),
        )?;
        ch.send(&Frame::Controls {
            requests: requests.clone(),
        })?;
        let Frame::ControlResults { results } = ch.receive(1024 * 1024)? else {
            return Err(Error::MalformedFrame);
        };
        if results.len() != requests.len()
            || results.iter().zip(&requests).any(|(s, r)| s.id != r.id)
        {
            return Err(Error::MalformedFrame);
        }
        let mut d = self.admitted()?;
        for result in results {
            custody(d.circuitnet_control_result(
                &self.profile.network,
                &self.peer.node,
                &result,
                now(),
            ))?;
        }
        Ok(())
    }
    fn receive_controls(&mut self, ch: &mut tls::Channel) -> Result<(), Error> {
        if !self.controls {
            return Ok(());
        }
        let Frame::Controls { requests } = ch.receive(envelope::control::MAX_CONTROL_BYTES)? else {
            return Err(Error::MalformedFrame);
        };
        if requests.len() > envelope::control::MAX_CONTROLS {
            return Err(Error::Oversized);
        }
        let mut results = vec![];
        let mut d = self.admitted()?;
        for r in requests {
            results.push(custody(d.circuitnet_receive_control(
                &self.profile.network,
                &self.peer.node,
                &r,
                now(),
            ))?);
        }
        ch.send(&Frame::ControlResults { results })?;
        Ok(())
    }
    fn send(&mut self, ch: &mut tls::Channel) -> Result<(), Error> {
        self.send_controls(ch)?;
        let prepared = {
            let _guard = custody(self.runtime.network_lock.lock())?;
            let mut d = self.admitted()?;
            let mut cursor = 0;
            for _ in 0..100 {
                let (_, next) = custody(d.circuitnet_scan(&self.profile.network, cursor, now()))?;
                if next == cursor {
                    break;
                }
                cursor = next;
            }
            match d.circuitnet_prepare_capable_neighbor(
                &self.runtime.network_artifacts,
                &self.profile.network,
                &self.peer.node,
                self.directed,
                now(),
            ) {
                Ok(p) => Some(p),
                Err(core::Error::Empty) => None,
                Err(e) => return Err(core_error(e)),
            }
        };
        let Some(p) = prepared else {
            self.health.bytes += ch.send(&Frame::Offer { batch: None })? as u64;
            return Ok(());
        };
        self.pending = Some(p.artifact.clone());
        self.health.bytes += ch.send(&Frame::Offer {
            batch: Some(custody(envelope::Batch::decode(&p.bytes))?),
        })? as u64;
        let Frame::Ack {
            receipt,
            imported,
            duplicates,
        } = ch.receive(wire::CONTROL_FRAME)?
        else {
            return Err(Error::MalformedFrame);
        };
        if receipt.artifact != p.artifact || imported as usize + duplicates as usize != p.messages {
            return Err(Error::MalformedFrame);
        }
        {
            let _guard = custody(self.runtime.network_lock.lock())?;
            let mut d = db(self.runtime)?;
            custody(d.circuitnet_acknowledge_neighbor(
                &self.runtime.network_artifacts,
                &self.profile.network,
                &self.peer.node,
                &custody(receipt.encode())?,
                now(),
            ))?;
        }
        self.pending = None;
        self.health.sent += p.messages as u32;
        Ok(())
    }
    fn receive(&mut self, ch: &mut tls::Channel) -> Result<(), Error> {
        self.receive_controls(ch)?;
        let Frame::Offer { batch } = ch.receive(wire::MAX_FRAME)? else {
            return Err(Error::MalformedFrame);
        };
        let Some(batch) = batch else {
            return Ok(());
        };
        if !self.directed && batch.messages.iter().any(|m| m.destination.is_some()) {
            return Err(Error::UnsupportedVersion);
        }
        let bytes = batch.encode().map_err(|_| Error::MalformedFrame)?;
        self.health.bytes += bytes.len() as u64;
        let result = {
            let _guard = custody(self.runtime.network_lock.lock())?;
            let mut d = self.admitted()?;
            match d.circuitnet_import_neighbor(
                &self.runtime.network_artifacts,
                &self.profile.network,
                &self.peer.node,
                &bytes,
                now(),
            ) {
                Ok(result) => result,
                Err(error) => {
                    self.health.rejected = batch.messages.len() as u32;
                    return Err(core_error(error));
                }
            }
        };
        self.health.accepted += result.imported;
        self.health.duplicates += result.duplicates;
        ch.send(&Frame::Ack {
            receipt: custody(envelope::Receipt::decode(&result.receipt))?,
            imported: result.imported,
            duplicates: result.duplicates,
        })?;
        Ok(())
    }
    fn finish(mut self, result: Result<(), Error>) -> Result<(), Error> {
        let mut d = db(self.runtime)?;
        if let Some(artifact) = self.pending.take() {
            custody(d.circuitnet_failed(&self.profile.network, &artifact, now()))?;
        }
        self.health.last_success =
            custody(d.circuitnet_link_health(&self.profile.network, &self.peer.node))?
                .and_then(|h| h.last_success);
        self.health.result = result.err().map_or_else(|| "ok".into(), |e| e.to_string());
        if result.is_ok() {
            self.health.last_success = Some(now());
        }
        self.health.duration_ms = self.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        custody(d.circuitnet_record_link(&self.profile.network, &self.peer.node, &self.health))?;
        tracing::info!(profile=%self.profile.network,neighbor=%self.peer.node,result=%self.health.result,sent=self.health.sent,accepted=self.health.accepted,duplicates=self.health.duplicates,bytes=self.health.bytes,duration_ms=self.health.duration_ms,retry=self.health.retry,"CircuitNET link finished");
        result
    }
}
fn core_error(e: core::Error) -> Error {
    match e {
        core::Error::Conflict => Error::ConflictingMessage,
        core::Error::Policy => Error::UnauthorizedCodename,
        core::Error::Codec(_) => Error::MalformedFrame,
        _ => Error::Custody,
    }
}
// DNS calls are OS-blocking: retain a global bound even when their caller times out.
static RESOLVERS: AtomicUsize = AtomicUsize::new(0);
fn connect(peer: &Peer) -> Result<TcpStream, Error> {
    let started = Instant::now();
    let addresses = if let Ok(ip) = peer.host.parse() {
        vec![std::net::SocketAddr::new(ip, peer.port)]
    } else {
        RESOLVERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| Error::Busy)?;
        let host = peer.host.clone();
        let port = peer.port;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        thread::spawn(move || {
            let result = (host.as_str(), port)
                .to_socket_addrs()
                .map(|a| a.take(17).collect::<Vec<_>>());
            let _ = tx.send(result);
            RESOLVERS.fetch_sub(1, Ordering::AcqRel);
        });
        let a = rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::Connect)?;
        if a.len() > 16 {
            return Err(Error::Oversized);
        }
        a
    };
    for a in addresses {
        let remaining = Duration::from_secs(10)
            .checked_sub(started.elapsed())
            .ok_or(Error::Timeout)?;
        if let Ok(s) = TcpStream::connect_timeout(&a, remaining) {
            return Ok(s);
        }
    }
    Err(Error::Connect)
}
fn outbound(
    runtime: &BoardRuntime,
    network: &NetworkId,
    node: &NodeId,
    mode: Mode,
    retry: u32,
) -> Result<(), Error> {
    let d = db(runtime)?;
    let profile = custody(d.circuitnet_status(network))?.profile;
    let (c, _) = custody(d.circuitnet_live(network))?;
    let peer = c.peer(node, false).map_err(|_| Error::Held)?.clone();
    let mut session = Session::new(runtime, profile, peer.clone(), false, retry);
    let result = (|| {
        session.admitted()?;
        let mut ch = tls::Channel::open(connect(&peer)?, &c, Some(&peer), key(runtime, &c)?)?;
        let local = Hello::new(
            network.clone(),
            session.profile.local.clone(),
            custody(session.profile.topology.node(&session.profile.local))?.role,
            mode,
        );
        ch.send(&Frame::Hello { hello: local })?;
        let Frame::Hello { hello } = ch.receive(wire::CONTROL_FRAME)? else {
            return Err(Error::MalformedFrame);
        };
        session.hello(&mut ch, hello, mode)?;
        let exchange = (|| {
            if mode == Mode::Poll {
                session.send(&mut ch)?;
                session.send_files(&mut ch)?;
                session.receive(&mut ch)?;
                session.receive_files(&mut ch)?;
            }
            ch.send(&Frame::Close {})?;
            if !matches!(ch.receive(wire::CONTROL_FRAME)?, Frame::Close {}) {
                return Err(Error::MalformedFrame);
            }
            Ok(())
        })();
        if let Err(code) = exchange {
            let _ = ch.send(&Frame::Error { code });
        }
        ch.close();
        exchange
    })();
    session.finish(result)
}
fn inbound(
    runtime: &BoardRuntime,
    network: &NetworkId,
    stream: TcpStream,
    permit: &mut Permit,
) -> Result<(), Error> {
    let d = db(runtime)?;
    let profile = custody(d.circuitnet_status(network))?.profile;
    if !profile.enabled {
        return Err(Error::Held);
    }
    let (c, _) = custody(d.circuitnet_live(network))?;
    let mut ch = tls::Channel::open(stream, &c, None, key(runtime, &c)?)?;
    let peer = c
        .peers
        .iter()
        .find(|p| ch.certificate().is_ok_and(|cert| cert == p.certificate))
        .ok_or(Error::UnknownNode)?
        .clone();
    permit.bind(network, &peer.node)?;
    let mut session = Session::new(runtime, profile, peer, true, 0);
    let result = (|| {
        let Frame::Hello { hello } = ch.receive(wire::CONTROL_FRAME)? else {
            return Err(Error::MalformedFrame);
        };
        let mode = hello.mode;
        let local = session.hello(&mut ch, hello, mode)?;
        ch.send(&Frame::Hello { hello: local })?;
        if mode == Mode::Poll {
            session.receive(&mut ch)?;
            session.receive_files(&mut ch)?;
            session.send(&mut ch)?;
            session.send_files(&mut ch)?;
        }
        if !matches!(ch.receive(wire::CONTROL_FRAME)?, Frame::Close {}) {
            return Err(Error::MalformedFrame);
        }
        ch.send(&Frame::Close {})?;
        Ok(())
    })();
    if let Err(code) = result {
        let _ = ch.send(&Frame::Error { code });
    }
    ch.close();
    session.finish(result)
}
pub(crate) fn configured(runtime: &BoardRuntime) -> Result<bool, ApplicationError> {
    let d = db(runtime).map_err(application)?;
    for n in d.circuitnet_profiles()? {
        let c = d.circuitnet_live(&n)?.0;
        if c.listener.is_some() || c.peers.iter().any(|p| p.enabled && p.outbound) {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(crate) fn listeners(
    runtime: Arc<BoardRuntime>,
) -> Result<Vec<thread::JoinHandle<()>>, ApplicationError> {
    let d = db(&runtime).map_err(application)?;
    let mut bound = vec![];
    for n in d.circuitnet_profiles()? {
        let (c, _) = d.circuitnet_live(&n)?;
        if let Some(address) = c
            .listener
            .filter(|_| d.circuitnet_status(&n).is_ok_and(|s| s.profile.enabled))
        {
            key(&runtime, &c).map_err(application)?;
            let listener = TcpListener::bind(address).map_err(|_| application(Error::Connect))?;
            listener
                .set_nonblocking(true)
                .map_err(|_| application(Error::Connect))?;
            bound.push((n, listener));
        }
    }
    let mut handles = vec![];
    for (network, listener) in bound {
        runtime
            .circuitnet_live
            .listeners
            .lock()
            .map_err(|_| application(Error::Custody))?
            .insert(network.to_string());
        let runtime = runtime.clone();
        handles.push(thread::spawn(move||{
            while !runtime.shutdown_in_progress().unwrap_or(true){
                match listener.accept(){Ok((stream,_))=>{
                    if let Ok(mut permit)=Permit::acquire(runtime.clone()){
                        let work=runtime.live_controls.track();let runtime=runtime.clone();let network=network.clone();
                        thread::spawn(move||{let _work=work;let result=inbound(&runtime,&network,stream,&mut permit);
                            if let Err(code)=result{tracing::info!(profile=%network,error=%code,"CircuitNET inbound rejected");}});
                    }
                },Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>thread::sleep(Duration::from_millis(25)),Err(_)=>break}
            }
            if let Ok(mut l)=runtime.circuitnet_live.listeners.lock(){l.remove(network.as_str());}
        }));
    }
    Ok(handles)
}

/// Synchronous Event boundary: completion is the actual finite Poll outcome.
pub(crate) fn event_poll(
    runtime: &Arc<BoardRuntime>,
    network: &NetworkId,
    node: &NodeId,
) -> sf_core::events::Outcome {
    use sf_core::events::Outcome;
    let Ok(d) = db(runtime) else {
        return Outcome::Failed;
    };
    let Ok((c, _)) = d.circuitnet_live(network) else {
        return Outcome::Failed;
    };
    if c.peer(node, false).is_err() {
        return Outcome::Held;
    }
    let Ok(mut permit) = Permit::acquire(runtime.clone()) else {
        return Outcome::Busy;
    };
    if permit.bind(network, node).is_err() {
        return Outcome::Busy;
    }
    for retry in 0..3 {
        if runtime.shutdown_in_progress().unwrap_or(true) {
            return Outcome::Interrupted;
        }
        match outbound(runtime, network, node, Mode::Poll, retry) {
            Ok(_) => {
                let Ok(d) = db(runtime) else {
                    return Outcome::Failed;
                };
                let action = sf_core::events::Action::Circuitnet {
                    network: network.clone(),
                    node: Some(node.clone()),
                };
                // An older peer may correctly withhold unsupported directed work.
                // A successful empty session must not rearm an endless drain loop.
                if d.event_has_outbound_work(&action, now()).unwrap_or(false)
                    && d.circuitnet_link_health(network, node)
                        .ok()
                        .flatten()
                        .is_some_and(|h| h.sent == 0 && h.files_sent == 0)
                {
                    return Outcome::Held;
                }
                return Outcome::Succeeded;
            }
            Err(Error::Connect | Error::Interrupted | Error::Timeout) if retry < 2 => {
                thread::sleep(Duration::from_secs(1 << retry))
            }
            Err(_) => return Outcome::Failed,
        }
    }
    Outcome::Failed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_and_per_neighbor_admission_release_on_drop() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        crate::initialize_fixture_board(&root).unwrap();
        let runtime = Arc::new(BoardRuntime::load(&root.join(crate::FIXTURE_CONFIG_FILE)).unwrap());
        let mut permits = (0..4)
            .map(|_| Permit::acquire(runtime.clone()).unwrap())
            .collect::<Vec<_>>();
        assert!(matches!(Permit::acquire(runtime.clone()), Err(Error::Busy)));
        let net = NetworkId::new("synthetic").unwrap();
        let node = NodeId::new("END1").unwrap();
        permits[0].bind(&net, &node).unwrap();
        assert!(matches!(permits[1].bind(&net, &node), Err(Error::Busy)));
        permits.remove(0);
        permits[0].bind(&net, &node).unwrap();
        assert!(Permit::acquire(runtime.clone()).is_ok());
        drop(permits);
        assert_eq!(runtime.circuitnet_live.count.load(Ordering::Acquire), 0);
        assert!(runtime.circuitnet_live.active.lock().unwrap().is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn credential_custody_rejects_symlinks_public_permissions_and_unsafe_errors() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let key = temp.path().join("key");
        fs::write(&key, b"synthetic-private-material").unwrap();
        fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
        private(&key, false).unwrap();
        let alias = temp.path().join("alias");
        symlink(&key, &alias).unwrap();
        assert!(private(&alias, false).is_err());
        fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(private(&key, false).is_err());
        let error = install_identity(
            temp.path(),
            b"invalid-public-certificate",
            b"synthetic-private-material".to_vec(),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "tls");
        assert!(!format!("{error:?}").contains("synthetic"));
    }
}
