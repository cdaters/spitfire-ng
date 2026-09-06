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

//! Daemon-owned BinkP workers, credentials and finite operator results.
mod session;
use crate::{ApplicationError, BoardRuntime, SecretStatus};
use serde::{Deserialize, Serialize};
use sf_core::{
    ftn::{self, BinkpMode, BinkpPolicy, BinkpWork},
    LogicalPath, RuntimeDatabase,
};
use sf_net::{
    binkp::{self as wire, Error},
    ftn::Endpoint,
};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

// System getaddrinfo cannot be cancelled. Keep its outstanding calls bounded even
// after a session's deadline, instead of abandoning unbounded runtime workers.
static RESOLVERS: AtomicUsize = AtomicUsize::new(0);
struct ResolverSlot;
impl Drop for ResolverSlot {
    fn drop(&mut self) {
        RESOLVERS.fetch_sub(1, Ordering::AcqRel);
    }
}
fn resolve(
    host: String,
    port: u16,
    started: Instant,
    cancelled: impl Fn() -> bool,
) -> std::result::Result<Vec<SocketAddr>, Error> {
    if cancelled() {
        return Err(Error::Cancelled);
    }
    if let Ok(ip) = host.parse() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    RESOLVERS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < 2).then_some(n + 1)
        })
        .map_err(|_| Error::Busy)?;
    let slot = ResolverSlot;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    thread::Builder::new()
        .name("binkp-resolver".into())
        .spawn(move || {
            let _slot = slot;
            let result = (host.as_str(), port)
                .to_socket_addrs()
                .map(|a| a.take(17).collect::<Vec<_>>())
                .map_err(|_| Error::Unavailable);
            let _ = sender.send(result);
        })
        .map_err(|_| Error::Unavailable)?;
    loop {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        if started.elapsed() > Duration::from_secs(10) {
            return Err(Error::Timeout);
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(addresses)) if addresses.len() <= 16 => return Ok(addresses),
            Ok(Ok(_)) => return Err(Error::Limit),
            Ok(Err(e)) => return Err(e),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Err(Error::Unavailable),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
        }
    }
}

struct Permit(Arc<AtomicUsize>);
impl Permit {
    fn acquire(runtime: &BoardRuntime) -> std::result::Result<Self, Error> {
        let count = runtime.binkp_sessions.clone();
        count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| Error::Busy)?;
        Ok(Self(count))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub const BINKP_MINOR: u16 = 8;
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Action {
    Poll {
        link: String,
        expected: String,
    },
    Test {
        link: String,
        expected: String,
    },
    Credential {
        link: String,
        expected: String,
        secret: String,
    },
    AreaFixCredential {
        link: String,
        expected: String,
        secret: String,
    },
    ClearAreaFixCredential {
        link: String,
        expected: String,
    },
    ClearCredential {
        link: String,
        expected: String,
    },
    Hold {
        queue: String,
        expected: i64,
    },
    Release {
        queue: String,
        expected: i64,
    },
}
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.operation())
    }
}
impl Action {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::AreaFixCredential { .. } => "ftn.areafix-credential",
            Self::ClearAreaFixCredential { .. } => "ftn.areafix-clear-credential",
            Self::Poll { .. } => "binkp.poll",
            Self::Test { .. } => "binkp.test",
            Self::Credential { .. } => "binkp.credential",
            Self::ClearCredential { .. } => "binkp.clear-credential",
            Self::Release { .. } => "binkp.release",
            Self::Hold { .. } => "binkp.hold",
        }
    }
    pub fn capability(&self) -> sf_core::LocalOperatorCapability {
        use sf_core::LocalOperatorCapability as C;
        match self {
            Self::Credential { .. }
            | Self::ClearCredential { .. }
            | Self::AreaFixCredential { .. }
            | Self::ClearAreaFixCredential { .. } => C::ChangeSensitiveConfiguration,
            Self::Release { .. } | Self::Hold { .. } => C::NetworkQueue,
            Self::Test { .. } => C::NetworkTest,
            _ => C::NetworkRun,
        }
    }
    /// Credential command receipts identify the operation, not a password verifier.
    pub(crate) fn fingerprint(&self) -> std::result::Result<Vec<u8>, serde_json::Error> {
        let mut a = self.clone();
        if let Self::Credential { secret, .. } | Self::AreaFixCredential { secret, .. } = &mut a {
            *secret = "write-only-update".into();
        }
        serde_json::to_vec(&a)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum Result {
    Started { session: String },
    Updated,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub policy: String,
    pub links: Vec<LinkStatus>,
    pub listener_enabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkStatus {
    pub link: String,
    pub credential: SecretStatus,
    pub areafix_credential: SecretStatus,
    pub health: Option<ftn::BinkpHealth>,
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
fn custody<T>(r: std::result::Result<T, impl std::fmt::Display>) -> std::result::Result<T, Error> {
    r.map_err(|_| Error::Custody)
}
fn credential_root(runtime: &BoardRuntime, directory: &str) -> std::result::Result<PathBuf, Error> {
    let root = runtime.paths.get(LogicalPath::System).join(directory);
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
fn private(path: &Path, directory: bool) -> std::result::Result<(), Error> {
    let meta = custody(fs::symlink_metadata(path))?;
    if meta.file_type().is_symlink()
        || (directory && !meta.is_dir())
        || (!directory && !meta.is_file())
    {
        return Err(Error::Custody);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(Error::Custody);
        }
    }
    Ok(())
}
fn valid_secret(secret: &[u8]) -> bool {
    !secret.is_empty()
        && secret.len() <= 128
        && secret != b"-"
        && !secret.starts_with(b"CRAM-")
        && secret.iter().all(|b| (33..=126).contains(b))
}
fn read_secret(runtime: &BoardRuntime, link: &str) -> std::result::Result<Vec<u8>, Error> {
    read_credential(runtime, link, "binkp-credentials")
}
fn read_credential(
    runtime: &BoardRuntime,
    link: &str,
    directory: &str,
) -> std::result::Result<Vec<u8>, Error> {
    let path = credential_root(runtime, directory)?.join(link);
    private(&path, false)?;
    let mut secret = Vec::new();
    custody(
        custody(fs::File::open(path))?
            .take(129)
            .read_to_end(&mut secret),
    )?;
    if !valid_secret(&secret) {
        return Err(Error::Authentication);
    }
    Ok(secret)
}
fn secret_status(runtime: &BoardRuntime, link: &str) -> SecretStatus {
    credential_status(runtime, link, "binkp-credentials")
}
fn credential_status(runtime: &BoardRuntime, link: &str, directory: &str) -> SecretStatus {
    let path = runtime
        .paths
        .get(LogicalPath::System)
        .join(directory)
        .join(link);
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => SecretStatus::Missing,
        _ => {
            if read_credential(runtime, link, directory).is_ok() {
                SecretStatus::Configured
            } else {
                SecretStatus::Invalid
            }
        }
    }
}
pub(crate) fn status(runtime: &BoardRuntime) -> std::result::Result<Status, ApplicationError> {
    let c = runtime.configuration.current()?;
    let health = RuntimeDatabase::open_read_only(runtime.database_path())?.binkp_health()?;
    Ok(Status {
        policy: c.binkp.digest(&c.ftn)?,
        listener_enabled: c.binkp.listener.as_ref().is_some_and(|l| l.enabled),
        links: c
            .binkp
            .links
            .iter()
            .map(|l| LinkStatus {
                link: l.link.clone(),
                credential: secret_status(runtime, &l.link),
                areafix_credential: credential_status(runtime, &l.link, "areafix-credentials"),
                health: health.iter().find(|h| h.link == l.link).cloned(),
            })
            .collect(),
    })
}
pub(crate) fn dispatch(
    runtime: &Arc<BoardRuntime>,
    principal: &str,
    action: &Action,
) -> std::result::Result<Result, ApplicationError> {
    let c = runtime.configuration.current()?;
    if let Action::Hold { queue, expected } = action {
        RuntimeDatabase::open(runtime.database_path())?.hold_binkp(
            &c.ftn,
            principal,
            queue,
            *expected,
            now(),
        )?;
        return Ok(Result::Updated);
    }
    if let Action::Release { queue, expected } = action {
        RuntimeDatabase::open(runtime.database_path())?.release_binkp(
            &c.ftn,
            principal,
            queue,
            *expected,
            now(),
        )?;
        return Ok(Result::Updated);
    }
    let (link, expected) = match action {
        Action::Poll { link, expected }
        | Action::Test { link, expected }
        | Action::Credential { link, expected, .. }
        | Action::ClearCredential { link, expected }
        | Action::AreaFixCredential { link, expected, .. }
        | Action::ClearAreaFixCredential { link, expected } => (link, expected),
        _ => unreachable!(),
    };
    if &c.binkp.digest(&c.ftn)? != expected {
        return Err(ftn::Error::Conflict.into());
    }
    c.ftn.link(link)?;
    c.binkp.link(link)?;
    let area_secret = matches!(
        action,
        Action::AreaFixCredential { .. } | Action::ClearAreaFixCredential { .. }
    );
    if area_secret
        && !RuntimeDatabase::open_read_only(runtime.database_path())?
            .ftn_downstreams()?
            .iter()
            .any(|d| d.link == *link)
    {
        return Err(ftn::Error::Denied.into());
    }
    let directory = if area_secret {
        "areafix-credentials"
    } else {
        "binkp-credentials"
    };
    match action {
        Action::Credential { secret, .. } | Action::AreaFixCredential { secret, .. } => {
            if !valid_secret(secret.as_bytes()) || (area_secret && secret.len() > 71) {
                return Err(ftn::Error::Policy.into());
            }
            let root = credential_root(runtime, directory).map_err(|_| ftn::Error::Denied)?;
            let mut file =
                tempfile::NamedTempFile::new_in(&root).map_err(|_| ftn::Error::Denied)?;
            file.write_all(secret.as_bytes())
                .map_err(|_| ftn::Error::Denied)?;
            file.as_file().sync_all().map_err(|_| ftn::Error::Denied)?;
            file.persist(root.join(link))
                .map_err(|_| ftn::Error::Denied)?;
            runtime
                .binkp_credentials_generation
                .fetch_add(1, Ordering::AcqRel);
            Ok(Result::Updated)
        }
        Action::ClearCredential { .. } | Action::ClearAreaFixCredential { .. } => {
            let root = credential_root(runtime, directory).map_err(|_| ftn::Error::Denied)?;
            match fs::remove_file(root.join(link)) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err(ftn::Error::Denied.into()),
            };
            runtime
                .binkp_credentials_generation
                .fetch_add(1, Ordering::AcqRel);
            Ok(Result::Updated)
        }
        _ => {
            let mode = if matches!(action, Action::Test { .. }) {
                BinkpMode::Test
            } else {
                BinkpMode::Poll
            };
            let mut backend = NativeBackend::new(runtime.clone(), c.ftn, c.binkp, mode);
            let permit = Permit::acquire(runtime).map_err(|_| ftn::Error::Conflict)?;
            let plan = backend
                .prepare(link, false)
                .map_err(|_| ftn::Error::Denied)?;
            let session = backend.session.clone().ok_or(ftn::Error::Conflict)?;
            let work = backend.runtime.live_controls.track();
            thread::spawn(move || {
                let _permit = permit;
                let _work = work;
                let outcome = backend.connect().and_then(|socket| {
                    session::run(socket, Some(plan), &mut backend, session::Limits::default())
                });
                backend.finish(outcome.err());
            });
            Ok(Result::Started { session })
        }
    }
}
struct NativeBackend {
    runtime: Arc<BoardRuntime>,
    ftn: ftn::Policy,
    policy: BinkpPolicy,
    mode: BinkpMode,
    credentials_generation: u64,
    session: Option<String>,
    link: Option<String>,
    work: BTreeMap<String, BinkpWork>,
}
impl NativeBackend {
    fn new(
        runtime: Arc<BoardRuntime>,
        ftn: ftn::Policy,
        policy: BinkpPolicy,
        mode: BinkpMode,
    ) -> Self {
        let credentials_generation = runtime.binkp_credentials_generation.load(Ordering::Acquire);
        Self {
            credentials_generation,
            runtime,
            ftn,
            policy,
            mode,
            session: None,
            link: None,
            work: BTreeMap::new(),
        }
    }
    fn db(&self) -> std::result::Result<RuntimeDatabase, Error> {
        custody(RuntimeDatabase::open(self.runtime.database_path()))
    }
    fn session(&self) -> std::result::Result<&str, Error> {
        self.session.as_deref().ok_or(Error::Authentication)
    }
    fn prepare(&mut self, link: &str, inbound: bool) -> std::result::Result<session::Plan, Error> {
        let t = custody(self.policy.link(link))?;
        let l = custody(self.ftn.link(link))?;
        if if inbound {
            !t.inbound || !l.inbound
        } else {
            !t.outbound || !l.outbound
        } {
            return Err(Error::Address);
        }
        let secret = read_secret(&self.runtime, link)?;
        let local = t
            .akas
            .iter()
            .map(|id| custody(self.ftn.aka(id)).map(|a| a.endpoint.clone()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let plan = session::Plan {
            local,
            remote: l.remote.clone(),
            secret,
            auth: t.auth,
            domainless: t.allow_domainless,
            mode: self.mode,
        };
        let session = self
            .db()?
            .begin_binkp(
                &self.ftn,
                &self.policy,
                link,
                self.runtime.daemon_generation(),
                self.mode,
                now(),
            )
            .map_err(|e| match e {
                ftn::Error::Conflict => Error::Busy,
                _ => Error::Refused,
            })?;
        self.link = Some(link.into());
        self.session = Some(session);
        Ok(plan)
    }
    fn connect(&self) -> std::result::Result<TcpStream, Error> {
        let endpoint = custody(self.db()?.resolve_binkp(
            &self.ftn,
            &self.policy,
            self.link.as_deref().ok_or(Error::Address)?,
            now(),
        ))?;
        let started = Instant::now();
        let addresses = resolve(endpoint.host, endpoint.port, started, || {
            session::Backend::cancelled(self)
        })?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?;
        let result = runtime.block_on(async {
            for address in addresses {
                let mut connection = Box::pin(tokio::net::TcpStream::connect(address));
                loop {
                    if session::Backend::cancelled(self) {
                        return Err(Error::Cancelled);
                    }
                    if started.elapsed() > Duration::from_secs(10) {
                        return Err(Error::Timeout);
                    }
                    match tokio::time::timeout(Duration::from_millis(100), &mut connection).await {
                        Ok(Ok(socket)) => return socket.into_std().map_err(|_| Error::Unavailable),
                        Ok(Err(_)) => break,
                        Err(_) => (),
                    }
                }
            }
            Err(Error::Unavailable)
        });
        runtime.shutdown_timeout(Duration::from_millis(100));
        result
    }
    fn finish(&self, error: Option<Error>) {
        if let Some(reason) = error {
            tracing::warn!(reason=%reason,"BinkP session failed");
        }
        if self.session.is_none() {
            if let Ok(mut db) = self.db() {
                let code = if error == Some(Error::Address) {
                    "message.ftn.binkp-address-rejected"
                } else {
                    "message.ftn.binkp-session-rejected"
                };
                if db
                    .record_operational_event(&sf_core::NewOperationalEvent::new(
                        now(),
                        sf_core::EventCategory::Message,
                        sf_core::EventSeverity::Warning,
                        code,
                        sf_core::EventOutcome::Failed,
                    ))
                    .is_err()
                {
                    tracing::error!("BinkP admission event persistence failed");
                }
            }
        }
        if let Some(session) = &self.session {
            if self
                .db()
                .and_then(|mut db| custody(db.finish_binkp(session, error, now())))
                .is_err()
            {
                tracing::error!(
                    "BinkP result persistence failed; durable claim retained for recovery"
                );
            }
        }
    }
}
impl session::Backend for NativeBackend {
    fn identify(&mut self, args: &str) -> std::result::Result<session::Plan, Error> {
        let mut matches = vec![];
        for t in self.policy.links.iter().filter(|l| l.enabled && l.inbound) {
            let l = custody(self.ftn.link(&t.link))?;
            if wire::addresses(args, t.allow_domainless.then_some(&l.remote.domain))
                .is_ok_and(|a| a.contains(&l.remote))
            {
                matches.push(t.link.clone());
            }
        }
        if matches.len() != 1 {
            return Err(Error::Address);
        }
        self.prepare(&matches[0], true)
    }
    fn ready(
        &mut self,
        addresses: &[Endpoint],
        capabilities: &[String],
        latency: u64,
    ) -> std::result::Result<Vec<session::Outgoing>, Error> {
        let session = self.session()?.to_owned();
        let mut db = self.db()?;
        let link = self.link.as_deref().ok_or(Error::Address)?;
        let transport = custody(self.policy.link(link))?;
        let remote = &custody(self.ftn.link(link))?.remote;
        let admitted: Vec<_> = addresses
            .iter()
            .filter(|a| *a == remote || transport.remote_akas.contains(a))
            .cloned()
            .collect();
        custody(db.observe_binkp(&session, &admitted, capabilities, latency, now()))?;
        if self.mode == BinkpMode::Test {
            return Ok(vec![]);
        }
        let work = custody(db.claim_binkp(
            &self.ftn,
            &self.policy,
            &self.runtime.network_artifacts,
            &session,
            now(),
        ))?;
        let mut result = vec![];
        for item in work {
            let name = format!("{}.pkt", &item.queue[..16]);
            result.push(session::Outgoing {
                key: item.queue.clone(),
                offer: wire::Offer {
                    name,
                    size: item.size,
                    time: item.time,
                    offset: 0,
                },
            });
            self.work.insert(item.queue.clone(), item);
        }
        Ok(result)
    }
    fn load(&mut self, key: &str) -> std::result::Result<Vec<u8>, Error> {
        let item = self.work.get(key).ok_or(Error::Custody)?;
        custody(self.runtime.network_artifacts.read_binkp(&item.artifact))
    }
    fn offered(&mut self, key: &str) -> std::result::Result<(), Error> {
        custody(self.db()?.offered_binkp(self.session()?, key))
    }
    fn accepted(&mut self, key: &str) -> std::result::Result<(), Error> {
        custody(self.db()?.accepted_binkp(self.session()?, key, now()))
    }
    fn receive(&mut self, bytes: &[u8]) -> std::result::Result<(), Error> {
        if self.mode == BinkpMode::Test {
            return Err(Error::Custody);
        }
        custody(self.db()?.receive_binkp_with_areafix(
            &self.runtime.network_artifacts,
            &self.ftn,
            &self.policy,
            self.session()?,
            bytes,
            now(),
            Some(&|link, supplied| {
                read_credential(&self.runtime, link, "areafix-credentials").is_ok_and(|stored| {
                    // HMAC verification avoids a password-dependent early-exit comparison.
                    let response =
                        wire::cram(supplied.as_bytes(), b"SPITFIRE AreaFix credential check");
                    wire::verify_cram(&stored, b"SPITFIRE AreaFix credential check", &response)
                        .is_ok()
                })
            }),
        ))?;
        Ok(())
    }
    fn cancelled(&self) -> bool {
        self.runtime.shutdown_in_progress().unwrap_or(true)
            || self
                .runtime
                .binkp_credentials_generation
                .load(Ordering::Acquire)
                != self.credentials_generation
            || !self
                .runtime
                .configuration
                .current()
                .is_ok_and(|c| c.ftn == self.ftn && c.binkp == self.policy)
    }
}
pub(crate) fn listener(
    runtime: Arc<BoardRuntime>,
) -> std::result::Result<Option<thread::JoinHandle<()>>, ApplicationError> {
    let config = runtime.configuration.current()?;
    let Some(listener) = config.binkp.listener.as_ref().filter(|l| l.enabled) else {
        return Ok(None);
    };
    let socket = TcpListener::bind(listener.bind).map_err(|_| ftn::Error::Denied)?;
    socket
        .set_nonblocking(true)
        .map_err(|_| ftn::Error::Denied)?;
    let active = runtime.binkp_sessions.clone();
    Ok(Some(thread::spawn(move || {
        let mut workers = vec![];
        while !runtime.shutdown_in_progress().unwrap_or(true)
            && runtime
                .configuration
                .current()
                .is_ok_and(|c| c.binkp.listener.as_ref().is_some_and(|l| l.enabled))
        {
            workers.retain(|h: &thread::JoinHandle<()>| !h.is_finished());
            match socket.accept() {
                Ok((mut stream, _)) => {
                    if active
                        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                            (n < 2).then_some(n + 1)
                        })
                        .is_err()
                    {
                        // A finite refusal; no worker or remotely sized allocation is admitted.
                        if stream.set_nonblocking(true).is_ok() {
                            if let Ok(bytes) =
                                wire::Frame::Command(wire::Command::Bsy as u8, b"busy".to_vec())
                                    .encode()
                            {
                                let _ = stream.write(&bytes);
                            }
                        }
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        continue;
                    }
                    let r = runtime.clone();
                    let permit = Permit(active.clone());
                    let work = r.live_controls.track();
                    workers.push(thread::spawn(move || {
                        let _permit = permit;
                        let _work = work;
                        if let Ok(c) = r.configuration.current() {
                            let mut backend =
                                NativeBackend::new(r.clone(), c.ftn, c.binkp, BinkpMode::Poll);
                            let outcome = session::run(
                                stream,
                                None,
                                &mut backend,
                                session::Limits::default(),
                            );
                            backend.finish(outcome.err());
                        }
                    }));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(_) => break,
            }
        }
        for worker in workers {
            let _ = worker.join();
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_have_no_debug_or_receipt_verifier_and_explicit_privilege() {
        let first = Action::Credential {
            link: "peer".into(),
            expected: "policy".into(),
            secret: "private-one".into(),
        };
        let second = Action::Credential {
            link: "peer".into(),
            expected: "policy".into(),
            secret: "private-two".into(),
        };
        assert!(!format!("{first:?}").contains("private-one"));
        assert_eq!(first.fingerprint().unwrap(), second.fingerprint().unwrap());
        assert!(!String::from_utf8(first.fingerprint().unwrap())
            .unwrap()
            .contains("private-one"));
        assert_eq!(
            first.capability(),
            sf_core::LocalOperatorCapability::ChangeSensitiveConfiguration
        );
        assert_eq!(
            Action::Test {
                link: "peer".into(),
                expected: "policy".into()
            }
            .capability(),
            sf_core::LocalOperatorCapability::NetworkTest
        );
        let area = Action::AreaFixCredential {
            link: "peer".into(),
            expected: "policy".into(),
            secret: "synthetic-area-secret".into(),
        };
        assert!(!format!("{area:?}").contains("synthetic-area-secret"));
        assert!(!String::from_utf8(area.fingerprint().unwrap())
            .unwrap()
            .contains("synthetic-area-secret"));
        assert_eq!(
            area.capability(),
            sf_core::LocalOperatorCapability::ChangeSensitiveConfiguration
        );
        for secret in [
            b"".as_slice(),
            b"-",
            b"CRAM-MD5-bypass",
            b"white space",
            &[42; 129],
        ] {
            assert!(!valid_secret(secret));
        }
    }
    #[cfg(unix)]
    #[test]
    fn secret_files_reject_symlinks_and_public_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("secret");
        fs::write(&path, b"synthetic").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(private(&path, false).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        private(&path, false).unwrap();
        let link = temp.path().join("alias");
        symlink(&path, &link).unwrap();
        assert!(private(&link, false).is_err());
    }
}
