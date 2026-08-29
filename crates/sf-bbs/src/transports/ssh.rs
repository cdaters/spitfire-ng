use std::borrow::Cow;
use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime};

use rand::rng;
use russh::keys::ssh_key::LineEnding;
use russh::server::{Auth, ChannelOpenHandle, Handler, Msg, Session};
use russh::{Channel, ChannelId, MethodKind, MethodSet, SshId};
use sf_core::{
    NetworkTerminalDefaults, Terminal, TerminalCapabilities, TerminalError, TerminalInfo,
    TerminalSize, TransportIdentity, TransportKind, VerifiedCallerGrant,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{info, warn};

use crate::runtime::{record_completion, BoardRuntime, ConnectionReport};
use crate::ApplicationError;

const INPUT_QUEUE_MESSAGES: usize = 64;
const MAX_PTY_WIDTH: u32 = 144;
const MAX_PTY_HEIGHT: u32 = 200;

#[derive(Clone)]
pub(crate) struct SshListenerOptions {
    pub defaults: NetworkTerminalDefaults,
    pub maximum_unauthenticated_connections: u16,
    pub maximum_authentication_attempts: u8,
    pub handshake_timeout: Duration,
}

pub(crate) fn load_or_generate_host_key(
    system: &Path,
    relative: &Path,
) -> Result<russh::keys::PrivateKey, ApplicationError> {
    let path = system.join(relative);
    if path.is_file() {
        return russh::keys::load_secret_key(&path, None).map_err(|error| {
            ApplicationError::Transport(format!(
                "could not load SSH Ed25519 host key {}: {error}",
                path.display()
            ))
        });
    }
    let parent = path
        .parent()
        .ok_or_else(|| ApplicationError::Transport("SSH host-key path has no parent".to_owned()))?;
    fs::create_dir_all(parent).map_err(|error| {
        ApplicationError::Transport(format!(
            "could not create SSH host-key directory {}: {error}",
            parent.display()
        ))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|error| {
            ApplicationError::Transport(format!(
                "could not restrict SSH host-key directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let key = russh::keys::PrivateKey::random(&mut rng(), russh::keys::Algorithm::Ed25519)
        .map_err(|error| {
            ApplicationError::Transport(format!("could not generate SSH Ed25519 host key: {error}"))
        })?;
    let encoded = key.to_openssh(LineEnding::LF).map_err(|error| {
        ApplicationError::Transport(format!("could not encode SSH Ed25519 host key: {error}"))
    })?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|error| {
        ApplicationError::Transport(format!(
            "could not create SSH host key {}: {error}",
            path.display()
        ))
    })?;
    file.write_all(encoded.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            ApplicationError::Transport(format!(
                "could not persist SSH host key {}: {error}",
                path.display()
            ))
        })?;
    info!(
        path = %path.display(),
        fingerprint = %key.public_key().fingerprint(Default::default()),
        "generated board-local SSH Ed25519 host key"
    );
    Ok(key)
}

pub(crate) fn host_key_fingerprint(
    system: &Path,
    relative: &Path,
) -> Result<Option<String>, ApplicationError> {
    let path = system.join(relative);
    if !path.is_file() {
        return Ok(None);
    }
    let key = russh::keys::load_secret_key(&path, None).map_err(|error| {
        ApplicationError::Transport(format!(
            "could not load SSH Ed25519 host key {}: {error}",
            path.display()
        ))
    })?;
    Ok(Some(
        key.public_key().fingerprint(Default::default()).to_string(),
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn serve_ssh_listener(
    listener: TcpListener,
    runtime: Arc<BoardRuntime>,
    host_key: russh::keys::PrivateKey,
    options: SshListenerOptions,
    completed: Arc<AtomicUsize>,
    maximum_sessions: Option<usize>,
    shutdown: Arc<AtomicBool>,
) {
    let tokio_runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("spitfire-ssh")
        .build()
    {
        Ok(value) => value,
        Err(error) => {
            warn!(error = %error, "could not start SSH async runtime");
            return;
        }
    };
    tokio_runtime.block_on(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, "could not adopt SSH listener");
                return;
            }
        };
        let methods = MethodSet::from(&[MethodKind::Password][..]);
        let config = Arc::new(russh::server::Config {
            server_id: SshId::Standard(Cow::Owned(format!(
                "SSH-2.0-SPITFIRE_NG_{}",
                sf_core::PRODUCT_VERSION
            ))),
            methods,
            auth_rejection_time: Duration::from_secs(1),
            auth_rejection_time_initial: Some(Duration::ZERO),
            keys: vec![host_key],
            max_auth_attempts: usize::from(options.maximum_authentication_attempts),
            inactivity_timeout: None,
            nodelay: true,
            maximum_packet_size: 32_768,
            channel_buffer_size: 64,
            event_buffer_size: 64,
            ..Default::default()
        });
        let permits = Arc::new(Semaphore::new(usize::from(
            options.maximum_unauthenticated_connections,
        )));
        info!("SSH listener started");
        while !shutdown.load(Ordering::SeqCst) {
            let accepted = tokio::select! {
                result = listener.accept() => Some(result),
                () = tokio::time::sleep(Duration::from_millis(50)) => None,
            };
            let Some(accepted) = accepted else { continue };
            let (stream, remote) = match accepted {
                Ok(value) => value,
                Err(error) => {
                    warn!(error = %error, "SSH listener accept failed");
                    continue;
                }
            };
            let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                warn!("SSH connection rejected at unauthenticated connection limit");
                drop(stream);
                continue;
            };
            info!(remote = %remote, "SSH connection accepted");
            let handler = SshHandler::new(
                Arc::clone(&runtime),
                remote,
                options.defaults.clone(),
                permit,
                Arc::clone(&completed),
                maximum_sessions,
                Arc::clone(&shutdown),
            );
            let authenticated = Arc::clone(&handler.authenticated);
            let config = Arc::clone(&config);
            let handshake_timeout = options.handshake_timeout;
            tokio::spawn(async move {
                let running = match russh::server::run_stream(config, stream, handler).await {
                    Ok(value) => value,
                    Err(error) => {
                        warn!(error = %error, "SSH handshake failed");
                        return;
                    }
                };
                let handle = running.handle();
                tokio::pin!(running);
                tokio::select! {
                    result = &mut running => {
                        if let Err(error) = result {
                            warn!(error = %error, "SSH connection ended");
                        }
                    }
                    () = authenticated.notified() => {
                        if let Err(error) = (&mut running).await {
                            warn!(error = %error, "SSH authenticated connection ended");
                        }
                    }
                    () = tokio::time::sleep(handshake_timeout) => {
                        warn!("SSH unauthenticated handshake timed out");
                        let _ = handle.disconnect(
                            russh::Disconnect::ByApplication,
                            "Authentication timeout".to_owned(),
                            String::new(),
                        ).await;
                    }
                }
            });
        }
        info!("SSH listener stopped");
    });
}

enum InputEvent {
    Data(Vec<u8>),
    Eof,
}

struct SshHandler {
    runtime: Arc<BoardRuntime>,
    remote: SocketAddr,
    defaults: NetworkTerminalDefaults,
    unauthenticated_permit: Option<OwnedSemaphorePermit>,
    verified: Option<VerifiedCallerGrant>,
    authenticated: Arc<tokio::sync::Notify>,
    channel: Option<ChannelId>,
    input_tx: mpsc::SyncSender<InputEvent>,
    input_rx: Option<mpsc::Receiver<InputEvent>>,
    info: Arc<Mutex<TerminalInfo>>,
    shell_started: bool,
    completed: Arc<AtomicUsize>,
    maximum_sessions: Option<usize>,
    shutdown: Arc<AtomicBool>,
}

impl SshHandler {
    #[allow(clippy::too_many_arguments)]
    fn new(
        runtime: Arc<BoardRuntime>,
        remote: SocketAddr,
        defaults: NetworkTerminalDefaults,
        permit: OwnedSemaphorePermit,
        completed: Arc<AtomicUsize>,
        maximum_sessions: Option<usize>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        let (input_tx, input_rx) = mpsc::sync_channel(INPUT_QUEUE_MESSAGES);
        let info = TerminalInfo {
            transport: TransportKind::Ssh,
            local: false,
            capabilities: TerminalCapabilities {
                terminal_type: None,
                ansi: defaults.ansi,
                cp437: defaults.cp437,
                size: Some(TerminalSize {
                    width: defaults.width,
                    height: defaults.height,
                }),
            },
            remote_address: Some(remote),
            connected_at: SystemTime::now(),
            connection_speed: None,
            carrier: None,
            declared_identity: None,
        };
        Self {
            runtime,
            remote,
            defaults,
            unauthenticated_permit: Some(permit),
            verified: None,
            authenticated: Arc::new(tokio::sync::Notify::new()),
            channel: None,
            input_tx,
            input_rx: Some(input_rx),
            info: Arc::new(Mutex::new(info)),
            shell_started: false,
            completed,
            maximum_sessions,
            shutdown,
        }
    }

    fn fail_request(session: &mut Session, channel: ChannelId) {
        let _ = session.channel_failure(channel);
    }
}

impl Handler for SshHandler {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        match self.runtime.authenticate_ssh_password(user, password) {
            Ok(Some(grant)) => {
                self.verified = Some(grant);
                if let Ok(mut info) = self.info.lock() {
                    info.declared_identity = Some(TransportIdentity {
                        name: user.to_owned(),
                        transport_authenticated: true,
                    });
                }
                self.unauthenticated_permit.take();
                self.authenticated.notify_one();
                Ok(Auth::Accept)
            }
            Ok(None) => Ok(Auth::reject()),
            Err(error) => {
                warn!(error = %error, "SSH authentication authority failed closed");
                Ok(Auth::reject())
            }
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel.is_some() || self.verified.is_none() {
            warn!(
                caller_id = self.verified.map(|grant| grant.caller_id.get()),
                "SSH extra or unauthenticated session channel rejected"
            );
            return Ok(());
        }
        self.channel = Some(channel.id());
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel != Some(channel) || term.len() > 64 || !term.is_ascii() {
            Self::fail_request(session, channel);
            return Ok(());
        }
        let width = col_width.clamp(40, MAX_PTY_WIDTH) as u16;
        let height = row_height.clamp(10, MAX_PTY_HEIGHT) as u16;
        let normalized = term.to_ascii_lowercase();
        let ansi = self.defaults.ansi
            && (normalized.contains("xterm")
                || normalized.contains("ansi")
                || normalized.contains("syncterm"));
        let cp437 =
            self.defaults.cp437 || normalized.contains("ansi") || normalized.contains("syncterm");
        if let Ok(mut info) = self.info.lock() {
            info.capabilities.terminal_type = Some(term.to_owned());
            info.capabilities.ansi = ansi;
            info.capabilities.cp437 = cp437;
            info.capabilities.size = Some(TerminalSize { width, height });
        }
        info!(
            caller_id = self.verified.map(|grant| grant.caller_id.get()),
            term, width, height, cp437, "SSH PTY negotiated"
        );
        session.channel_success(channel)?;
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel == Some(channel) {
            let width = col_width.clamp(40, MAX_PTY_WIDTH) as u16;
            let height = row_height.clamp(10, MAX_PTY_HEIGHT) as u16;
            if let Ok(mut info) = self.info.lock() {
                info.capabilities.size = Some(TerminalSize { width, height });
            }
            info!(
                caller_id = self.verified.map(|grant| grant.caller_id.get()),
                width, height, "SSH terminal resized"
            );
        }
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel != Some(channel) || self.shell_started {
            Self::fail_request(session, channel);
            return Ok(());
        }
        let Some(grant) = self.verified.take() else {
            Self::fail_request(session, channel);
            return Ok(());
        };
        let Some(input) = self.input_rx.take() else {
            Self::fail_request(session, channel);
            return Ok(());
        };
        self.shell_started = true;
        session.channel_success(channel)?;
        let mut terminal = SshTerminal {
            input,
            pending: VecDeque::new(),
            info: Arc::clone(&self.info),
            grant: Some(grant),
            handle: session.handle(),
            tokio: tokio::runtime::Handle::current(),
            channel,
            idle_timeout: Duration::from_secs(300),
            disconnected: false,
            skip_lf: false,
        };
        let runtime = Arc::clone(&self.runtime);
        let completed = Arc::clone(&self.completed);
        let maximum_sessions = self.maximum_sessions;
        let shutdown = Arc::clone(&self.shutdown);
        info!(caller_id = grant.caller_id.get(), remote = %self.remote, "SSH caller session established");
        tokio::task::spawn_blocking(move || match runtime.run_connection(&mut terminal) {
            Ok(ConnectionReport::Completed(_)) => {
                record_completion(&completed, maximum_sessions, &shutdown);
            }
            Ok(ConnectionReport::NodeBusy) => {}
            Err(error) => {
                warn!(error = %error, caller_id = grant.caller_id.get(), "SSH caller session ended with error")
            }
        });
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel != Some(channel)
            || data.len() > 32_768
            || self
                .input_tx
                .try_send(InputEvent::Data(data.to_vec()))
                .is_err()
        {
            warn!(
                caller_id = self.verified.map(|grant| grant.caller_id.get()),
                "SSH input rejected at resource boundary"
            );
            session.close(channel)?;
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel == Some(channel) {
            let _ = self.input_tx.try_send(InputEvent::Eof);
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel == Some(channel) {
            let _ = self.input_tx.try_send(InputEvent::Eof);
        }
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        _data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        Self::fail_request(session, channel);
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        _name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        Self::fail_request(session, channel);
        Ok(())
    }

    async fn env_request(
        &mut self,
        channel: ChannelId,
        _variable_name: &str,
        _variable_value: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        Self::fail_request(session, channel);
        Ok(())
    }

    async fn agent_request(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }

    async fn tcpip_forward(
        &mut self,
        _address: &str,
        _port: &mut u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }

    async fn streamlocal_forward(
        &mut self,
        _socket_path: &str,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

struct SshTerminal {
    input: mpsc::Receiver<InputEvent>,
    pending: VecDeque<u8>,
    info: Arc<Mutex<TerminalInfo>>,
    grant: Option<VerifiedCallerGrant>,
    handle: russh::server::Handle,
    tokio: tokio::runtime::Handle,
    channel: ChannelId,
    idle_timeout: Duration,
    disconnected: bool,
    skip_lf: bool,
}

impl SshTerminal {
    fn receive_byte(&mut self, timeout: Duration) -> Result<Option<u8>, TerminalError> {
        loop {
            if let Some(byte) = self.pending.pop_front() {
                return Ok(Some(byte));
            }
            match self.input.recv_timeout(timeout) {
                Ok(InputEvent::Data(bytes)) => self.pending.extend(bytes),
                Ok(InputEvent::Eof) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(TerminalError::TimedOut),
            }
        }
    }

    fn read_line_inner(
        &mut self,
        maximum: usize,
        echo: bool,
    ) -> Result<Option<Vec<u8>>, TerminalError> {
        let mut line = Vec::new();
        let mut overlong = false;
        let mut actual = 0usize;
        loop {
            let Some(byte) = self.receive_byte(self.idle_timeout)? else {
                return Ok((!line.is_empty()).then_some(line));
            };
            if self.skip_lf && byte == b'\n' {
                self.skip_lf = false;
                continue;
            }
            self.skip_lf = false;
            match byte {
                b'\r' | b'\n' => {
                    self.skip_lf = byte == b'\r';
                    if echo {
                        self.write_all(b"\r\n")?;
                    }
                    if overlong {
                        return Err(TerminalError::InputTooLong { actual, maximum });
                    }
                    return Ok(Some(line));
                }
                0x08 | 0x7f if !overlong => {
                    if line.pop().is_some() && echo {
                        self.write_all(b"\x08 \x08")?;
                    }
                }
                value => {
                    actual = actual.saturating_add(1);
                    if line.len() < maximum && !overlong {
                        line.push(value);
                        if echo {
                            self.write_all(&[value])?;
                        }
                    } else {
                        overlong = true;
                    }
                }
            }
        }
    }
}

impl Terminal for SshTerminal {
    fn info(&self) -> TerminalInfo {
        self.info
            .lock()
            .map(|value| value.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        if self.disconnected {
            return Err(TerminalError::Disconnected);
        }
        self.tokio
            .block_on(self.handle.data(self.channel, bytes.to_vec()))
            .map_err(|_| TerminalError::Disconnected)
    }

    fn read_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.read_line_inner(maximum_bytes, true)
    }

    fn read_secret_line(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
        self.read_line_inner(maximum_bytes, false)
    }

    fn set_idle_timeout(&mut self, timeout: Duration) -> Result<(), TerminalError> {
        self.idle_timeout = timeout;
        Ok(())
    }

    fn read_key(&mut self) -> Result<Option<u8>, TerminalError> {
        loop {
            match self.receive_byte(self.idle_timeout)? {
                Some(b'\r' | b'\n') => continue,
                value => return Ok(value),
            }
        }
    }

    fn take_verified_caller_grant(&mut self) -> Option<VerifiedCallerGrant> {
        self.grant.take()
    }

    fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
        self.skip_lf = false;
        Ok(())
    }

    fn read_binary(
        &mut self,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, TerminalError> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let Some(first) = self.receive_byte(timeout)? else {
            return Ok(0);
        };
        buffer[0] = first;
        let mut count = 1;
        while count < buffer.len() {
            let Some(byte) = self.pending.pop_front() else {
                break;
            };
            buffer[count] = byte;
            count += 1;
        }
        Ok(count)
    }

    fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.write_all(bytes)
    }

    fn disconnect(&mut self) -> Result<(), TerminalError> {
        if !self.disconnected {
            self.disconnected = true;
            let _ = self.tokio.block_on(self.handle.eof(self.channel));
            let _ = self.tokio.block_on(self.handle.close(self.channel));
            info!("SSH caller channel disconnected");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_key_is_ed25519_and_stable_with_private_permissions() {
        let directory = tempfile::tempdir().unwrap();
        let first =
            load_or_generate_host_key(directory.path(), Path::new("ssh/host-ed25519")).unwrap();
        let second =
            load_or_generate_host_key(directory.path(), Path::new("ssh/host-ed25519")).unwrap();
        assert_eq!(first.public_key(), second.public_key());
        assert_eq!(first.algorithm(), russh::keys::Algorithm::Ed25519);
        assert_eq!(
            host_key_fingerprint(directory.path(), Path::new("ssh/host-ed25519")).unwrap(),
            Some(
                first
                    .public_key()
                    .fingerprint(Default::default())
                    .to_string()
            )
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(directory.path().join("ssh/host-ed25519"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
