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

//! Protected local, read-only operator attachment protocol.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sf_core::{
    CallerId, EventId, EventQuery, LocalOperatorCapability, LocalOperatorIdentity,
    OperatorPrincipal, OperatorPrincipalKind, RuntimeConfig,
};
use sf_core::{
    EventCategory, EventCursor, EventOutcome, EventSeverity, MaintenanceStatus,
    OperatorNotification, SystemStatistics,
};
#[cfg(unix)]
use sf_core::{LogicalPath, LogicalPaths};
#[cfg(any(unix, windows))]
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::runtime::{BoardRuntime, BoardStatus, LiveNodeStatus};
use crate::runtime::{ObservabilityCapabilities, OperatorObservabilityContext};
use crate::OperatorService;

pub const OPERATOR_PROTOCOL_MAJOR: u16 = 1;
pub const OPERATOR_PROTOCOL_MINOR: u16 = 0;
pub const MAX_OPERATOR_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_OPERATOR_FEATURES: usize = 32;
pub const DEFAULT_REQUEST_DEADLINE_MS: u64 = 5_000;
#[cfg(unix)]
const CONTROL_ENDPOINT: &str = "sfop.sock";
#[cfg(windows)]
const WINDOWS_PIPE_PREFIX: &str = r"\\.\pipe\spitfire-ng-operator-";
#[cfg(unix)]
const PORTABLE_UNIX_SOCKET_PATH_LIMIT: usize = 100;

#[derive(Debug, Error)]
pub enum OperatorControlError {
    #[error("operator endpoint is unavailable")]
    EndpointUnavailable,
    #[error("operator endpoint is unsafe: {0}")]
    UnsafeEndpoint(&'static str),
    #[error("local operator authentication failed")]
    AuthenticationFailed,
    #[error("operator authorization was denied")]
    AuthorizationDenied,
    #[error("operator protocol major version is incompatible")]
    ProtocolMismatch,
    #[error("operator protocol feature is unsupported")]
    UnsupportedFeature,
    #[error("operator protocol frame is malformed")]
    MalformedFrame,
    #[error("operator protocol frame exceeds {MAX_OPERATOR_FRAME_BYTES} bytes")]
    OversizedFrame,
    #[error("operator request timed out")]
    Timeout,
    #[error("operator daemon generation changed")]
    StaleDaemonGeneration,
    #[error("operator control I/O failed: {0}")]
    Io(#[source] std::io::Error),
    #[error("operator control serialization failed: {0}")]
    Serialization(#[source] serde_json::Error),
    #[error("operator control is not yet available on this platform")]
    PlatformUnavailable,
    #[error("Windows operator identity is unavailable")]
    PeerIdentityUnavailable,
    #[error("Windows operator SID is invalid")]
    InvalidWindowsSid,
    #[error("Windows operator pipe security could not be established")]
    PipeSecurityUnavailable,
    #[error("operator service failed safely: {0}")]
    Service(String),
}

impl From<std::io::Error> for OperatorControlError {
    fn from(error: std::io::Error) -> Self {
        if matches!(
            error.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ) {
            Self::Timeout
        } else {
            Self::Io(error)
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperatorFeature {
    BoardStatus,
    NodeList,
    NodeStatus,
    RecentEvents,
    LiveEvents,
    Notifications,
    Statistics,
    RecentCallers,
    MaintenanceStatus,
}

impl OperatorFeature {
    const ALL: [Self; 9] = [
        Self::BoardStatus,
        Self::NodeList,
        Self::NodeStatus,
        Self::RecentEvents,
        Self::LiveEvents,
        Self::Notifications,
        Self::Statistics,
        Self::RecentCallers,
        Self::MaintenanceStatus,
    ];
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ClientMessage {
    Hello {
        major: u16,
        minor: u16,
        features: Vec<OperatorFeature>,
    },
    Authenticate {
        session_id: String,
        challenge: String,
        daemon_generation: String,
    },
    Request {
        session_id: String,
        daemon_generation: String,
        request_id: u64,
        deadline_ms: u64,
        operation: ReadOperation,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case")]
enum ReadOperation {
    BoardStatus,
    ListNodes,
    NodeStatus { node_id: u32 },
    RecentEvents { query: OperatorEventQuery },
    SubscribeEvents { wait_ms: u64 },
    CancelEventSubscription,
    Notifications { include_closed: bool, limit: usize },
    Statistics,
    RecentCallers { limit: usize },
    MaintenanceStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ServerMessage {
    Hello {
        major: u16,
        minor: u16,
        daemon_generation: String,
        session_id: String,
        challenge: String,
        negotiated_features: Vec<OperatorFeature>,
        schema_version: u32,
    },
    Authenticated {
        principal: String,
    },
    Response {
        request_id: u64,
        result: ReadResult,
    },
    Error {
        request_id: Option<u64>,
        code: ErrorCode,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ErrorCode {
    AuthenticationFailed,
    AuthorizationDenied,
    ProtocolMismatch,
    UnsupportedFeature,
    MalformedRequest,
    OversizedRequest,
    Timeout,
    StaleDaemonGeneration,
    InternalFailure,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "result", content = "value", rename_all = "kebab-case")]
enum ReadResult {
    BoardStatus(BoardStatusWire),
    Nodes(Vec<NodeStatusWire>),
    Node(Option<NodeStatusWire>),
    Events(EventBatchWire),
    SubscriptionCancelled,
    Notifications(Vec<NotificationWire>),
    Statistics(StatisticsWire),
    RecentCallers(Vec<RecentCallerWire>),
    Maintenance(MaintenanceWire),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BoardStatusWire {
    pub board_name: String,
    pub running_since_utc: i64,
    pub uptime_seconds: u64,
    pub schema_version: u32,
    pub configured_nodes: usize,
    pub active_nodes: usize,
    pub callers_online: usize,
    pub active_transfers: u64,
    pub storage_warnings: u64,
    pub recent_errors: u64,
    pub open_notifications: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeStatusWire {
    pub node_id: u32,
    pub lifecycle: String,
    pub session_id: Option<u64>,
    pub public_handle: Option<String>,
    pub transport: Option<String>,
    pub online_seconds: Option<u64>,
    pub current_section: Option<String>,
    pub terminal_type: Option<String>,
    pub encoding: Option<String>,
    pub columns: Option<u16>,
    pub rows: Option<u16>,
    pub presentation_profile: Option<String>,
    pub security_context: Option<String>,
    pub transfer_state: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventWire {
    pub event_id: u64,
    pub occurred_at_utc: i64,
    pub board_day: i32,
    pub category: String,
    pub severity: String,
    pub event_code: String,
    pub outcome: String,
    pub node_id: Option<u32>,
    pub session_id: Option<u64>,
    pub correlation_id: Option<String>,
    pub object_kind: Option<String>,
    pub object_id: Option<String>,
    pub attributes: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventBatchWire {
    pub events: Vec<EventWire>,
    pub gap_before_first: bool,
    pub has_more: bool,
    pub next_cursor: Option<EventCursorWire>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct EventCursorWire {
    pub occurred_at_utc: i64,
    pub event_id: u64,
    pub snapshot_event_id: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct OperatorEventQuery {
    pub from_utc: Option<i64>,
    pub through_utc: Option<i64>,
    pub category: Option<EventCategory>,
    pub minimum_severity: Option<EventSeverity>,
    pub outcome: Option<EventOutcome>,
    pub node_id: Option<u32>,
    pub caller_id: Option<u64>,
    pub cursor: Option<EventCursorWire>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NotificationWire {
    pub notification_id: u64,
    pub source_event_id: u64,
    pub created_at: i64,
    pub category: String,
    pub severity: String,
    pub reason_key: String,
    pub remediation_key: Option<String>,
    pub state: String,
    pub state_version: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatisticsWire {
    pub observability_activated_at: i64,
    pub board_day: i32,
    pub calls_started_today: u64,
    pub calls_completed_today: u64,
    pub messages_posted_today: u64,
    pub successful_uploads_today: u64,
    pub successful_downloads_today: u64,
    pub lifetime_calls: u64,
    pub lifetime_messages_posted: u64,
    pub lifetime_files_uploaded: u64,
    pub lifetime_files_downloaded: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RecentCallerWire {
    pub event_id: u64,
    pub public_handle: String,
    pub occurred_at_utc: i64,
    pub board_day: i32,
    pub transport: Option<String>,
    pub duration_seconds: u64,
    pub close_reason: Option<String>,
    pub node_id: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MaintenanceWire {
    pub open_notifications: u64,
    pub recent_warning_events: u64,
    pub recent_error_events: u64,
    pub unavailable_storage_roots: u64,
    pub pending_review_files: u64,
    pub nonterminal_transfers: u64,
    pub detail_retention_days: u16,
    pub summary_retention_days: u16,
}

pub(crate) struct OperatorServerHandle {
    handle: Option<thread::JoinHandle<()>>,
}

impl OperatorServerHandle {
    pub(crate) fn join(mut self) -> Result<(), OperatorControlError> {
        match self.handle.take() {
            Some(handle) => handle.join().map_err(|_| {
                OperatorControlError::Service("operator endpoint thread panicked".to_owned())
            }),
            None => Ok(()),
        }
    }
}

pub(crate) fn start_operator_server(
    runtime: Arc<BoardRuntime>,
    config_path: PathBuf,
    shutdown: Arc<AtomicBool>,
) -> Result<OperatorServerHandle, OperatorControlError> {
    #[cfg(unix)]
    {
        server::start_unix(runtime, config_path, shutdown)
    }
    #[cfg(windows)]
    {
        windows::start(runtime, config_path, shutdown)
    }
}

trait OperatorIo: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}

impl<T> OperatorIo for T where T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}

pub struct OperatorClient {
    stream: Box<dyn OperatorIo>,
    session_id: String,
    daemon_generation: String,
    features: Vec<OperatorFeature>,
    next_request_id: u64,
}

impl OperatorClient {
    pub async fn connect(config_path: &Path) -> Result<Self, OperatorControlError> {
        let mut stream = connect_transport(config_path).await?;
        write_frame(
            &mut stream,
            &ClientMessage::Hello {
                major: OPERATOR_PROTOCOL_MAJOR,
                minor: OPERATOR_PROTOCOL_MINOR,
                features: OperatorFeature::ALL.to_vec(),
            },
        )
        .await?;
        let hello: ServerMessage = read_frame(&mut stream).await?;
        let (daemon_generation, session_id, challenge, features) = match hello {
            ServerMessage::Hello {
                major,
                daemon_generation,
                session_id,
                challenge,
                negotiated_features,
                ..
            } if major == OPERATOR_PROTOCOL_MAJOR => (
                daemon_generation,
                session_id,
                challenge,
                negotiated_features,
            ),
            ServerMessage::Error {
                code: ErrorCode::ProtocolMismatch,
                ..
            } => return Err(OperatorControlError::ProtocolMismatch),
            ServerMessage::Error {
                code: ErrorCode::AuthenticationFailed,
                ..
            } => return Err(OperatorControlError::AuthenticationFailed),
            _ => return Err(OperatorControlError::MalformedFrame),
        };
        write_frame(
            &mut stream,
            &ClientMessage::Authenticate {
                session_id: session_id.clone(),
                challenge,
                daemon_generation: daemon_generation.clone(),
            },
        )
        .await?;
        match read_frame::<ServerMessage>(&mut stream).await? {
            ServerMessage::Authenticated { .. } => Ok(Self {
                stream,
                session_id,
                daemon_generation,
                features,
                next_request_id: 1,
            }),
            ServerMessage::Error {
                code: ErrorCode::AuthenticationFailed,
                ..
            } => Err(OperatorControlError::AuthenticationFailed),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }

    pub fn daemon_generation(&self) -> &str {
        &self.daemon_generation
    }
    pub fn features(&self) -> &[OperatorFeature] {
        &self.features
    }

    async fn request(
        &mut self,
        operation: ReadOperation,
    ) -> Result<ReadResult, OperatorControlError> {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        write_frame(
            &mut self.stream,
            &ClientMessage::Request {
                session_id: self.session_id.clone(),
                daemon_generation: self.daemon_generation.clone(),
                request_id,
                deadline_ms: DEFAULT_REQUEST_DEADLINE_MS,
                operation,
            },
        )
        .await?;
        let response = tokio::time::timeout(
            Duration::from_millis(DEFAULT_REQUEST_DEADLINE_MS + 250),
            read_frame(&mut self.stream),
        )
        .await
        .map_err(|_| OperatorControlError::Timeout)??;
        match response {
            ServerMessage::Response {
                request_id: returned,
                result,
            } if returned == request_id => Ok(result),
            ServerMessage::Error { code, .. } => Err(map_error(code)),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }

    pub async fn board_status(&mut self) -> Result<BoardStatusWire, OperatorControlError> {
        match self.request(ReadOperation::BoardStatus).await? {
            ReadResult::BoardStatus(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn nodes(&mut self) -> Result<Vec<NodeStatusWire>, OperatorControlError> {
        match self.request(ReadOperation::ListNodes).await? {
            ReadResult::Nodes(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn node_status(
        &mut self,
        node_id: u32,
    ) -> Result<Option<NodeStatusWire>, OperatorControlError> {
        match self.request(ReadOperation::NodeStatus { node_id }).await? {
            ReadResult::Node(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn recent_events(
        &mut self,
        limit: usize,
    ) -> Result<EventBatchWire, OperatorControlError> {
        self.query_events(OperatorEventQuery {
            limit: Some(limit),
            ..OperatorEventQuery::default()
        })
        .await
    }
    pub async fn query_events(
        &mut self,
        query: OperatorEventQuery,
    ) -> Result<EventBatchWire, OperatorControlError> {
        match self.request(ReadOperation::RecentEvents { query }).await? {
            ReadResult::Events(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn subscribe_events(
        &mut self,
        wait_ms: u64,
    ) -> Result<EventBatchWire, OperatorControlError> {
        match self
            .request(ReadOperation::SubscribeEvents { wait_ms })
            .await?
        {
            ReadResult::Events(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn cancel_event_subscription(&mut self) -> Result<(), OperatorControlError> {
        match self.request(ReadOperation::CancelEventSubscription).await? {
            ReadResult::SubscriptionCancelled => Ok(()),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn notifications(
        &mut self,
        include_closed: bool,
        limit: usize,
    ) -> Result<Vec<NotificationWire>, OperatorControlError> {
        match self
            .request(ReadOperation::Notifications {
                include_closed,
                limit,
            })
            .await?
        {
            ReadResult::Notifications(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn statistics(&mut self) -> Result<StatisticsWire, OperatorControlError> {
        match self.request(ReadOperation::Statistics).await? {
            ReadResult::Statistics(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn recent_callers(
        &mut self,
        limit: usize,
    ) -> Result<Vec<RecentCallerWire>, OperatorControlError> {
        match self.request(ReadOperation::RecentCallers { limit }).await? {
            ReadResult::RecentCallers(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
    pub async fn maintenance_status(&mut self) -> Result<MaintenanceWire, OperatorControlError> {
        match self.request(ReadOperation::MaintenanceStatus).await? {
            ReadResult::Maintenance(value) => Ok(value),
            _ => Err(OperatorControlError::MalformedFrame),
        }
    }
}

fn map_error(code: ErrorCode) -> OperatorControlError {
    match code {
        ErrorCode::AuthenticationFailed => OperatorControlError::AuthenticationFailed,
        ErrorCode::AuthorizationDenied => OperatorControlError::AuthorizationDenied,
        ErrorCode::ProtocolMismatch => OperatorControlError::ProtocolMismatch,
        ErrorCode::UnsupportedFeature => OperatorControlError::UnsupportedFeature,
        ErrorCode::OversizedRequest => OperatorControlError::OversizedFrame,
        ErrorCode::Timeout => OperatorControlError::Timeout,
        ErrorCode::StaleDaemonGeneration => OperatorControlError::StaleDaemonGeneration,
        ErrorCode::MalformedRequest => OperatorControlError::MalformedFrame,
        ErrorCode::InternalFailure => {
            OperatorControlError::Service("internal safe failure".to_owned())
        }
    }
}

pub(crate) fn random_token() -> String {
    let mut bytes = [0_u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(unix)]
async fn connect_transport(
    config_path: &Path,
) -> Result<Box<dyn OperatorIo>, OperatorControlError> {
    let endpoint = endpoint_for_config(config_path)?;
    let stream = tokio::time::timeout(
        Duration::from_millis(DEFAULT_REQUEST_DEADLINE_MS),
        tokio::net::UnixStream::connect(endpoint),
    )
    .await
    .map_err(|_| OperatorControlError::Timeout)?
    .map_err(|_| OperatorControlError::EndpointUnavailable)?;
    Ok(Box::new(stream))
}

#[cfg(windows)]
async fn connect_transport(
    config_path: &Path,
) -> Result<Box<dyn OperatorIo>, OperatorControlError> {
    windows::connect(config_path).await
}

#[cfg(unix)]
fn endpoint_for_config(config_path: &Path) -> Result<PathBuf, OperatorControlError> {
    let canonical = config_path
        .canonicalize()
        .map_err(|_| OperatorControlError::EndpointUnavailable)?;
    let root = canonical
        .parent()
        .ok_or(OperatorControlError::EndpointUnavailable)?;
    let config = RuntimeConfig::load(&canonical)
        .map_err(|error| OperatorControlError::Service(error.to_string()))?;
    let paths = LogicalPaths::resolve(
        root,
        &config
            .validate()
            .map_err(|error| OperatorControlError::Service(error.to_string()))?,
    )
    .map_err(|error| OperatorControlError::Service(error.to_string()))?;
    let board_local = paths.get(LogicalPath::Work).join(CONTROL_ENDPOINT);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if board_local.as_os_str().as_bytes().len() >= PORTABLE_UNIX_SOCKET_PATH_LIMIT {
            let mut digest = Sha256::new();
            digest.update(root.as_os_str().as_bytes());
            let board_id = format!("{:x}", digest.finalize());
            let uid = std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(root)?);
            return Ok(PathBuf::from("/tmp")
                .join(format!("spitfire-ng-operator-{uid}"))
                .join(format!("{}.sock", &board_id[..24])));
        }
    }
    Ok(board_local)
}

async fn write_frame<T: Serialize>(
    stream: &mut (impl tokio::io::AsyncWrite + Unpin),
    value: &T,
) -> Result<(), OperatorControlError> {
    use tokio::io::AsyncWriteExt;
    let body = serde_json::to_vec(value).map_err(OperatorControlError::Serialization)?;
    if body.len() > MAX_OPERATOR_FRAME_BYTES {
        return Err(OperatorControlError::OversizedFrame);
    }
    tokio::time::timeout(Duration::from_millis(DEFAULT_REQUEST_DEADLINE_MS), async {
        stream.write_all(&(body.len() as u32).to_be_bytes()).await?;
        stream.write_all(&body).await?;
        stream.flush().await
    })
    .await
    .map_err(|_| OperatorControlError::Timeout)??;
    Ok(())
}

async fn read_frame<T: for<'de> Deserialize<'de>>(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<T, OperatorControlError> {
    use tokio::io::AsyncReadExt;
    let mut length = [0_u8; 4];
    stream.read_exact(&mut length).await?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 {
        return Err(OperatorControlError::MalformedFrame);
    }
    if length > MAX_OPERATOR_FRAME_BYTES {
        return Err(OperatorControlError::OversizedFrame);
    }
    let mut body = vec![0_u8; length];
    stream.read_exact(&mut body).await?;
    serde_json::from_slice(&body).map_err(OperatorControlError::Serialization)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PeerIdentity {
    #[cfg(unix)]
    Unix(u32),
    #[cfg(windows)]
    Windows(String),
}

impl PeerIdentity {
    fn stable_id(&self) -> String {
        match self {
            #[cfg(unix)]
            Self::Unix(uid) => format!("unix-uid:{uid}"),
            #[cfg(windows)]
            Self::Windows(sid) => format!("windows-sid:{sid}"),
        }
    }
}

mod server {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};

    #[cfg(unix)]
    pub(super) fn start_unix(
        runtime: Arc<BoardRuntime>,
        config_path: PathBuf,
        shutdown: Arc<AtomicBool>,
    ) -> Result<OperatorServerHandle, OperatorControlError> {
        let endpoint = endpoint_for_config(&config_path)?;
        let directory = endpoint
            .parent()
            .ok_or(OperatorControlError::UnsafeEndpoint(
                "missing endpoint parent",
            ))?;
        let root_uid = std::fs::metadata(
            config_path
                .parent()
                .ok_or(OperatorControlError::UnsafeEndpoint("missing board root"))?,
        )?
        .uid();
        if directory.exists() {
            let metadata = std::fs::symlink_metadata(directory)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.uid() != root_uid
            {
                return Err(OperatorControlError::UnsafeEndpoint(
                    "control directory ownership is unsafe",
                ));
            }
        } else {
            std::fs::DirBuilder::new().mode(0o700).create(directory)?;
        }
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
        if endpoint.exists() {
            let metadata = std::fs::symlink_metadata(&endpoint)?;
            if metadata.file_type().is_symlink() || !metadata.file_type().is_socket() {
                return Err(OperatorControlError::UnsafeEndpoint(
                    "pre-existing endpoint is not a socket",
                ));
            }
            if std::os::unix::net::UnixStream::connect(&endpoint).is_ok() {
                return Err(OperatorControlError::UnsafeEndpoint(
                    "operator endpoint is already active",
                ));
            }
            std::fs::remove_file(&endpoint)?;
        }
        let listener = std::os::unix::net::UnixListener::bind(&endpoint)?;
        listener.set_nonblocking(true)?;
        std::fs::set_permissions(&endpoint, std::fs::Permissions::from_mode(0o600))?;
        let handle = thread::spawn(move || {
            let tokio = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("operator runtime");
            tokio.block_on(async move {
                let listener = tokio::net::UnixListener::from_std(listener)
                    .expect("validated nonblocking operator listener");
                while !shutdown.load(Ordering::SeqCst) {
                    match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await
                    {
                        Ok(Ok((stream, _))) => {
                            let runtime = Arc::clone(&runtime);
                            let config_path = config_path.clone();
                            tokio::spawn(async move {
                                let peer = match stream.peer_cred() {
                                    Ok(credentials) => PeerIdentity::Unix(credentials.uid()),
                                    Err(_) => return,
                                };
                                let _ = handle_connection(
                                    stream,
                                    runtime,
                                    config_path,
                                    peer,
                                    Some(root_uid),
                                    None,
                                )
                                .await;
                            });
                        }
                        Ok(Err(_)) => break,
                        Err(_) => {}
                    }
                }
                let _ = std::fs::remove_file(&endpoint);
            });
        });
        Ok(OperatorServerHandle {
            handle: Some(handle),
        })
    }

    pub(super) async fn handle_connection<S>(
        mut stream: S,
        runtime: Arc<BoardRuntime>,
        config_path: PathBuf,
        peer: PeerIdentity,
        bootstrap_unix_uid: Option<u32>,
        initial_hello: Option<ClientMessage>,
    ) -> Result<(), OperatorControlError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let hello = match initial_hello {
            Some(hello) => Ok(Ok(hello)),
            None => tokio::time::timeout(
                Duration::from_millis(DEFAULT_REQUEST_DEADLINE_MS),
                read_frame::<ClientMessage>(&mut stream),
            )
            .await
            .map_err(|_| OperatorControlError::Timeout),
        };
        let hello = match hello {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => {
                audit(
                    &runtime,
                    &peer,
                    false,
                    "operator.protocol",
                    "not-applicable",
                    "rejected",
                    Some(match error {
                        OperatorControlError::OversizedFrame => "oversized-frame",
                        _ => "malformed-frame",
                    }),
                );
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        let requested = match hello {
            ClientMessage::Hello {
                major,
                minor: _,
                features,
            } if major == OPERATOR_PROTOCOL_MAJOR && features.len() <= MAX_OPERATOR_FEATURES => {
                features
            }
            ClientMessage::Hello { .. } => {
                audit(
                    &runtime,
                    &peer,
                    false,
                    "operator.negotiate",
                    "not-applicable",
                    "incompatible",
                    Some("protocol-major"),
                );
                write_frame(
                    &mut stream,
                    &ServerMessage::Error {
                        request_id: None,
                        code: ErrorCode::ProtocolMismatch,
                    },
                )
                .await?;
                return Err(OperatorControlError::ProtocolMismatch);
            }
            _ => return Err(OperatorControlError::MalformedFrame),
        };
        let _capabilities = match authorize(&config_path, bootstrap_unix_uid, &peer) {
            Ok(value) => value,
            Err(error) => {
                audit(
                    &runtime,
                    &peer,
                    false,
                    "operator.authenticate",
                    "denied",
                    "denied",
                    Some("peer-not-allowed"),
                );
                write_frame(
                    &mut stream,
                    &ServerMessage::Error {
                        request_id: None,
                        code: ErrorCode::AuthenticationFailed,
                    },
                )
                .await?;
                return Err(error);
            }
        };
        let session_id = random_token();
        let challenge = random_token();
        let negotiated_features = OperatorFeature::ALL
            .into_iter()
            .filter(|item| requested.contains(item))
            .collect::<Vec<_>>();
        write_frame(
            &mut stream,
            &ServerMessage::Hello {
                major: OPERATOR_PROTOCOL_MAJOR,
                minor: OPERATOR_PROTOCOL_MINOR,
                daemon_generation: runtime.daemon_generation().to_owned(),
                session_id: session_id.clone(),
                challenge: challenge.clone(),
                negotiated_features: negotiated_features.clone(),
                schema_version: runtime.schema_version(),
            },
        )
        .await?;
        let auth = read_frame::<ClientMessage>(&mut stream).await?;
        if !matches!(auth, ClientMessage::Authenticate { session_id: ref supplied_session, challenge: ref supplied_challenge, daemon_generation: ref supplied_generation } if supplied_session == &session_id && supplied_challenge == &challenge && supplied_generation == runtime.daemon_generation())
        {
            audit(
                &runtime,
                &peer,
                false,
                "operator.authenticate",
                "denied",
                "denied",
                Some("challenge-mismatch"),
            );
            write_frame(
                &mut stream,
                &ServerMessage::Error {
                    request_id: None,
                    code: ErrorCode::AuthenticationFailed,
                },
            )
            .await?;
            return Err(OperatorControlError::AuthenticationFailed);
        }
        audit(
            &runtime,
            &peer,
            true,
            "operator.authenticate",
            "allowed",
            "succeeded",
            None,
        );
        write_frame(
            &mut stream,
            &ServerMessage::Authenticated {
                principal: peer.stable_id(),
            },
        )
        .await?;
        let mut live_subscription = None;
        loop {
            let request = match tokio::time::timeout(
                Duration::from_secs(60),
                read_frame::<ClientMessage>(&mut stream),
            )
            .await
            {
                Ok(Ok(value)) => value,
                Ok(Err(error)) => {
                    audit(
                        &runtime,
                        &peer,
                        true,
                        "operator.protocol",
                        "not-applicable",
                        "rejected",
                        Some(match error {
                            OperatorControlError::OversizedFrame => "oversized-frame",
                            _ => "malformed-frame",
                        }),
                    );
                    return Err(error);
                }
                Err(_) => return Err(OperatorControlError::Timeout),
            };
            let ClientMessage::Request {
                session_id: supplied_session,
                daemon_generation,
                request_id,
                deadline_ms,
                operation,
            } = request
            else {
                audit(
                    &runtime,
                    &peer,
                    true,
                    "operator.protocol",
                    "not-applicable",
                    "rejected",
                    Some("unexpected-message"),
                );
                return Err(OperatorControlError::MalformedFrame);
            };
            if supplied_session != session_id || daemon_generation != runtime.daemon_generation() {
                write_frame(
                    &mut stream,
                    &ServerMessage::Error {
                        request_id: Some(request_id),
                        code: ErrorCode::StaleDaemonGeneration,
                    },
                )
                .await?;
                continue;
            }
            if deadline_ms == 0 || deadline_ms > 30_000 {
                write_frame(
                    &mut stream,
                    &ServerMessage::Error {
                        request_id: Some(request_id),
                        code: ErrorCode::Timeout,
                    },
                )
                .await?;
                continue;
            }
            let current = match authorize(&config_path, bootstrap_unix_uid, &peer) {
                Ok(value) => value,
                Err(_) => {
                    audit(
                        &runtime,
                        &peer,
                        true,
                        "operator.read",
                        "denied",
                        "denied",
                        Some("policy-revoked"),
                    );
                    write_frame(
                        &mut stream,
                        &ServerMessage::Error {
                            request_id: Some(request_id),
                            code: ErrorCode::AuthorizationDenied,
                        },
                    )
                    .await?;
                    continue;
                }
            };
            if !negotiated_features.contains(&operation.feature()) {
                write_frame(
                    &mut stream,
                    &ServerMessage::Error {
                        request_id: Some(request_id),
                        code: ErrorCode::UnsupportedFeature,
                    },
                )
                .await?;
                continue;
            }
            if !permitted(operation.feature(), &current) {
                audit(
                    &runtime,
                    &peer,
                    true,
                    "operator.read",
                    "denied",
                    "denied",
                    Some("capability-denied"),
                );
                write_frame(
                    &mut stream,
                    &ServerMessage::Error {
                        request_id: Some(request_id),
                        code: ErrorCode::AuthorizationDenied,
                    },
                )
                .await?;
                continue;
            }
            let context = context(&peer, &current);
            let service = OperatorService::new(Arc::clone(&runtime));
            let result = tokio::time::timeout(
                Duration::from_millis(deadline_ms),
                dispatch(&service, &context, operation, &mut live_subscription),
            )
            .await;
            let message = match result {
                Ok(Ok(result)) => ServerMessage::Response { request_id, result },
                Ok(Err(_)) => ServerMessage::Error {
                    request_id: Some(request_id),
                    code: ErrorCode::InternalFailure,
                },
                Err(_) => ServerMessage::Error {
                    request_id: Some(request_id),
                    code: ErrorCode::Timeout,
                },
            };
            write_frame(&mut stream, &message).await?;
        }
    }

    fn authorize(
        config_path: &Path,
        bootstrap_unix_uid: Option<u32>,
        peer: &PeerIdentity,
    ) -> Result<Vec<LocalOperatorCapability>, OperatorControlError> {
        let config = RuntimeConfig::load(config_path)
            .map_err(|error| OperatorControlError::Service(error.to_string()))?;
        let validated = config
            .validate()
            .map_err(|error| OperatorControlError::Service(error.to_string()))?;
        #[cfg(windows)]
        let _ = bootstrap_unix_uid;
        if validated.operators.local_identities.is_empty() {
            return match peer {
                #[cfg(unix)]
                PeerIdentity::Unix(uid) if bootstrap_unix_uid == Some(*uid) => {
                    Ok(super::all_read_capabilities())
                }
                _ => Err(OperatorControlError::AuthenticationFailed),
            };
        }
        validated
            .operators
            .local_identities
            .into_iter()
            .find_map(|identity| match (identity, peer) {
                #[cfg(unix)]
                (
                    LocalOperatorIdentity::Unix {
                        uid: configured,
                        capabilities,
                        ..
                    },
                    PeerIdentity::Unix(uid),
                ) if configured == *uid => Some(capabilities),
                #[cfg(windows)]
                (
                    LocalOperatorIdentity::Windows {
                        sid: configured,
                        capabilities,
                        ..
                    },
                    PeerIdentity::Windows(sid),
                ) if configured.eq_ignore_ascii_case(sid) => Some(capabilities),
                _ => None,
            })
            .ok_or(OperatorControlError::AuthenticationFailed)
    }

    fn audit(
        runtime: &BoardRuntime,
        peer: &PeerIdentity,
        authenticated: bool,
        operation: &str,
        authorization: &str,
        outcome: &str,
        detail: Option<&str>,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|value| i64::try_from(value.as_secs()).ok())
            .unwrap_or(0);
        if let Ok(mut database) = sf_core::RuntimeDatabase::open(runtime.database_path()) {
            let _ = database.record_operator_control_audit(&sf_core::NewOperatorControlAudit {
                occurred_at: now,
                operator_kind: if authenticated {
                    "host-operator"
                } else {
                    "unknown-peer"
                }
                .to_owned(),
                operator_id: authenticated.then(|| peer.stable_id()),
                operation: operation.to_owned(),
                authorization_result: authorization.to_owned(),
                target_kind: None,
                target_id: None,
                command_id: None,
                correlation_id: None,
                outcome: outcome.to_owned(),
                detail_code: detail.map(str::to_owned),
            });
        }
    }
}

#[cfg(windows)]
pub(crate) fn windows_current_process_sid() -> Result<String, OperatorControlError> {
    windows::current_process_sid()
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use std::ptr;

    use tokio::net::windows::named_pipe::{NamedPipeClient, NamedPipeServer, ServerOptions};
    use windows_sys::Win32::Foundation::{
        CloseHandle, LocalFree, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, HANDLE,
        INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        ConvertStringSidToSidW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, IsValidSid, RevertToSelf, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_CREATE_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ,
        FILE_GENERIC_WRITE, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    };
    use windows_sys::Win32::System::Pipes::ImpersonateNamedPipeClient;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken,
    };

    const MAX_PIPE_INSTANCES: usize = 32;
    // Tokio's standard client builder requests GENERIC_WRITE, whose pipe
    // mapping includes FILE_CREATE_PIPE_INSTANCE. Open with the equivalent
    // precise client rights minus that server-only bit so an authorized
    // operator cannot create a competing server instance.
    const CLIENT_PIPE_ACCESS: u32 =
        FILE_GENERIC_READ | (FILE_GENERIC_WRITE & !FILE_CREATE_PIPE_INSTANCE);

    pub(super) async fn connect(
        config_path: &Path,
    ) -> Result<Box<dyn OperatorIo>, OperatorControlError> {
        let pipe_name = pipe_name(config_path)?;
        let deadline =
            tokio::time::Instant::now() + Duration::from_millis(DEFAULT_REQUEST_DEADLINE_MS);
        loop {
            match open_pipe(&pipe_name) {
                Ok(stream) => return Ok(Box::new(stream)),
                Err(error) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) => {
                    return Err(OperatorControlError::AuthenticationFailed);
                }
                Err(error)
                    if matches!(
                        error.raw_os_error(),
                        Some(code)
                            if code == ERROR_PIPE_BUSY as i32
                                || code == ERROR_FILE_NOT_FOUND as i32
                    ) && tokio::time::Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                Err(_) => return Err(OperatorControlError::EndpointUnavailable),
            }
        }
    }

    pub(super) fn open_pipe(pipe_name: &str) -> std::io::Result<NamedPipeClient> {
        let wide = wide_null(pipe_name);
        // SAFETY: the pipe name is NUL-terminated, the desired access exactly
        // matches the client ACE, and the returned handle is transferred once
        // to Tokio. SECURITY_IDENTIFICATION permits peer identity checks but
        // prevents a server from using this client for broader impersonation.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                CLIENT_PIPE_ACCESS,
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | SECURITY_IDENTIFICATION | SECURITY_SQOS_PRESENT,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: CreateFileW returned one owned, overlapped named-pipe handle.
        unsafe { NamedPipeClient::from_raw_handle(handle as _) }
    }

    pub(super) fn start(
        runtime: Arc<BoardRuntime>,
        config_path: PathBuf,
        shutdown: Arc<AtomicBool>,
    ) -> Result<OperatorServerHandle, OperatorControlError> {
        let pipe_name = pipe_name(&config_path)?;
        let config = RuntimeConfig::load(&config_path)
            .map_err(|error| OperatorControlError::Service(error.to_string()))?
            .validate()
            .map_err(|error| OperatorControlError::Service(error.to_string()))?;
        let daemon_sid = current_process_sid()?;
        let operator_sids = configured_windows_sids(&config.operators.local_identities)?;
        let security = PipeSecurity::new(&daemon_sid, &operator_sids)?;
        let tokio = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(OperatorControlError::Io)?;
        let first = {
            let _entered = tokio.enter();
            create_pipe(&pipe_name, true, &security)?
        };
        let handle = thread::spawn(move || {
            tokio.block_on(async move {
                let mut pending = first;
                while !shutdown.load(Ordering::SeqCst) {
                    match tokio::time::timeout(Duration::from_millis(100), pending.connect()).await
                    {
                        Ok(Ok(())) => {
                            let connected = pending;
                            let runtime = Arc::clone(&runtime);
                            let config_path = config_path.clone();
                            tokio::spawn(async move {
                                let mut connected = connected;
                                let initial_hello = match tokio::time::timeout(
                                    Duration::from_millis(DEFAULT_REQUEST_DEADLINE_MS),
                                    read_frame::<ClientMessage>(&mut connected),
                                )
                                .await
                                {
                                    Ok(Ok(hello)) => hello,
                                    _ => return,
                                };
                                let peer = match peer_sid(&connected) {
                                    Ok(sid) => PeerIdentity::Windows(sid),
                                    Err(_) => return,
                                };
                                let _ = server::handle_connection(
                                    connected,
                                    runtime,
                                    config_path,
                                    peer,
                                    None,
                                    Some(initial_hello),
                                )
                                .await;
                            });
                            loop {
                                match create_pipe(&pipe_name, false, &security) {
                                    Ok(next) => {
                                        pending = next;
                                        break;
                                    }
                                    Err(OperatorControlError::Io(error))
                                        if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                                            && !shutdown.load(Ordering::SeqCst) =>
                                    {
                                        tokio::time::sleep(Duration::from_millis(25)).await;
                                    }
                                    Err(_) => return,
                                }
                            }
                        }
                        Ok(Err(_)) => return,
                        Err(_) => {}
                    }
                }
            });
        });
        Ok(OperatorServerHandle {
            handle: Some(handle),
        })
    }

    pub(super) fn pipe_name(config_path: &Path) -> Result<String, OperatorControlError> {
        let canonical = config_path
            .canonicalize()
            .map_err(|_| OperatorControlError::EndpointUnavailable)?;
        let normalized = canonical
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let mut digest = Sha256::new();
        digest.update(b"spitfire-ng-operator-pipe-v1\0");
        digest.update(normalized);
        let board_hash = format!("{:x}", digest.finalize());
        Ok(format!("{WINDOWS_PIPE_PREFIX}{}", &board_hash[..32]))
    }

    fn configured_windows_sids(
        identities: &[LocalOperatorIdentity],
    ) -> Result<Vec<String>, OperatorControlError> {
        let mut sids = Vec::new();
        for identity in identities {
            if let LocalOperatorIdentity::Windows { sid, .. } = identity {
                let canonical = canonical_sid(sid)?;
                if canonical != *sid || sids.contains(&canonical) {
                    return Err(OperatorControlError::InvalidWindowsSid);
                }
                sids.push(canonical);
            }
        }
        Ok(sids)
    }

    fn create_pipe(
        pipe_name: &str,
        first: bool,
        security: &PipeSecurity,
    ) -> Result<NamedPipeServer, OperatorControlError> {
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .max_instances(MAX_PIPE_INSTANCES)
            .in_buffer_size(MAX_OPERATOR_FRAME_BYTES as u32)
            .out_buffer_size(MAX_OPERATOR_FRAME_BYTES as u32);
        // SAFETY: `security.attributes` points to a live self-relative descriptor
        // owned by `security`. Tokio passes it synchronously to CreateNamedPipeW,
        // and the descriptor outlives this call.
        unsafe {
            options
                .create_with_security_attributes_raw(
                    pipe_name,
                    ptr::from_ref(&security.attributes)
                        .cast_mut()
                        .cast::<c_void>(),
                )
                .map_err(OperatorControlError::Io)
        }
    }

    struct PipeSecurity {
        descriptor: *mut c_void,
        attributes: SECURITY_ATTRIBUTES,
    }

    // The descriptor is process-local heap memory with no thread affinity and
    // is moved, not shared, into the single operator runtime thread.
    unsafe impl Send for PipeSecurity {}

    impl PipeSecurity {
        fn new(daemon_sid: &str, operator_sids: &[String]) -> Result<Self, OperatorControlError> {
            let sddl = security_sddl(daemon_sid, operator_sids);
            let wide = wide_null(&sddl);
            let mut descriptor = ptr::null_mut();
            // SAFETY: `wide` is NUL-terminated and the output pointer is valid.
            // Windows allocates a self-relative descriptor released by LocalFree.
            let converted = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    wide.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    ptr::null_mut(),
                )
            };
            if converted == 0 || descriptor.is_null() {
                return Err(OperatorControlError::PipeSecurityUnavailable);
            }
            Ok(Self {
                descriptor,
                attributes: SECURITY_ATTRIBUTES {
                    nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                    lpSecurityDescriptor: descriptor,
                    bInheritHandle: 0,
                },
            })
        }
    }

    pub(super) fn security_sddl(daemon_sid: &str, operator_sids: &[String]) -> String {
        let mut sddl = format!("D:P(A;;GA;;;{daemon_sid})");
        for sid in operator_sids {
            if !sid.eq_ignore_ascii_case(daemon_sid) {
                sddl.push_str(&format!("(A;;0x{CLIENT_PIPE_ACCESS:x};;;{sid})"));
            }
        }
        sddl
    }

    impl Drop for PipeSecurity {
        fn drop(&mut self) {
            // SAFETY: the pointer was allocated by the SDDL conversion API and
            // is released exactly once after the last pipe creation call.
            unsafe {
                LocalFree(self.descriptor);
            }
        }
    }

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: this wrapper owns one valid token handle.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    struct ImpersonationGuard;

    impl Drop for ImpersonationGuard {
        fn drop(&mut self) {
            // SAFETY: reverting an impersonating thread is always valid. The
            // guard exists before any fallible token operation begins.
            unsafe {
                RevertToSelf();
            }
        }
    }

    fn peer_sid(pipe: &NamedPipeServer) -> Result<String, OperatorControlError> {
        // This synchronous scope contains no await: the current-thread Tokio
        // runtime cannot switch tasks while this thread is impersonating.
        // SAFETY: the handle is a connected named-pipe server handle.
        if unsafe { ImpersonateNamedPipeClient(pipe.as_raw_handle() as HANDLE) } == 0 {
            return Err(OperatorControlError::PeerIdentityUnavailable);
        }
        let _guard = ImpersonationGuard;
        let mut token = ptr::null_mut();
        // SAFETY: the current thread is impersonating the pipe client and the
        // output handle pointer is valid.
        if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) } == 0 {
            return Err(OperatorControlError::PeerIdentityUnavailable);
        }
        token_sid(OwnedHandle(token))
    }

    pub(super) fn current_process_sid() -> Result<String, OperatorControlError> {
        let mut token = ptr::null_mut();
        // SAFETY: GetCurrentProcess returns a valid pseudo-handle and the token
        // output pointer is valid.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(OperatorControlError::PeerIdentityUnavailable);
        }
        token_sid(OwnedHandle(token))
    }

    fn token_sid(token: OwnedHandle) -> Result<String, OperatorControlError> {
        let mut required = 0_u32;
        // SAFETY: a null information buffer is the documented size query.
        unsafe {
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut required);
        }
        if required < std::mem::size_of::<TOKEN_USER>() as u32 {
            return Err(OperatorControlError::PeerIdentityUnavailable);
        }
        let words = (required as usize).div_ceil(std::mem::size_of::<usize>());
        let mut buffer = vec![0_usize; words];
        // SAFETY: the aligned buffer is at least `required` bytes and all
        // pointers remain valid for the duration of the call.
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast::<c_void>(),
                required,
                &mut required,
            )
        } == 0
        {
            return Err(OperatorControlError::PeerIdentityUnavailable);
        }
        // SAFETY: successful TokenUser output starts with a TOKEN_USER whose
        // SID pointer remains valid while `buffer` is alive.
        let sid = unsafe { (*(buffer.as_ptr().cast::<TOKEN_USER>())).User.Sid };
        sid_to_string(sid)
    }

    pub(super) fn canonical_sid(value: &str) -> Result<String, OperatorControlError> {
        let wide = wide_null(value);
        let mut sid = ptr::null_mut();
        // SAFETY: the input is NUL-terminated and the returned SID is owned by
        // LocalAlloc and released below.
        if unsafe { ConvertStringSidToSidW(wide.as_ptr(), &mut sid) } == 0 || sid.is_null() {
            return Err(OperatorControlError::InvalidWindowsSid);
        }
        let result = sid_to_string(sid);
        // SAFETY: ConvertStringSidToSidW allocated this pointer.
        unsafe {
            LocalFree(sid);
        }
        result
    }

    fn sid_to_string(
        sid: windows_sys::Win32::Security::PSID,
    ) -> Result<String, OperatorControlError> {
        // SAFETY: callers supply a token SID or a SID returned by the Windows
        // SID parser. IsValidSid validates it before conversion.
        if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
            return Err(OperatorControlError::InvalidWindowsSid);
        }
        let mut text = ptr::null_mut();
        // SAFETY: the validated SID and output pointer are valid. Windows
        // allocates the returned NUL-terminated UTF-16 string.
        if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 || text.is_null() {
            return Err(OperatorControlError::PeerIdentityUnavailable);
        }
        let mut length = 0;
        // SAFETY: ConvertSidToStringSidW guarantees a NUL-terminated string.
        unsafe {
            while *text.add(length) != 0 {
                length += 1;
            }
        }
        // SAFETY: `length` was found within the API-owned NUL-terminated buffer.
        let value = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) })
            .map_err(|_| OperatorControlError::PeerIdentityUnavailable);
        // SAFETY: ConvertSidToStringSidW allocated this pointer.
        unsafe {
            LocalFree(text.cast::<c_void>());
        }
        value
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use crate::{
        initialize_fixture_board, setup_board, BoardRuntime, SetupPlan, BOARD_CONFIG_FILE,
        FIXTURE_CONFIG_FILE,
    };
    use tokio::io::AsyncWriteExt;

    #[test]
    fn windows_pipe_name_acl_and_sid_are_board_specific_and_restrictive() {
        let temp = tempfile::tempdir().unwrap();
        let setup_root = temp.path().join("setup-board");
        let setup = SetupPlan::stock_defaults("Windows Setup Board", "Setup Sysop", "Sysop", 1);
        setup_board(&setup_root, &setup, b"test-only Windows setup password").unwrap();
        let setup_config = RuntimeConfig::load(&setup_root.join(BOARD_CONFIG_FILE)).unwrap();
        let current_sid = windows_current_process_sid().unwrap();
        assert!(setup_config
            .operators
            .local_identities
            .iter()
            .any(|identity| matches!(identity, LocalOperatorIdentity::Windows { sid, .. } if sid == &current_sid)));

        let first_root = temp.path().join("board-a");
        let second_root = temp.path().join("board-b");
        initialize_fixture_board(&first_root).unwrap();
        initialize_fixture_board(&second_root).unwrap();
        let first = windows::pipe_name(&first_root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let second = windows::pipe_name(&second_root.join(FIXTURE_CONFIG_FILE)).unwrap();
        assert!(first.starts_with(WINDOWS_PIPE_PREFIX));
        assert!(first.len() < 128);
        assert_ne!(first, second);

        let daemon_sid = windows_current_process_sid().unwrap();
        assert_eq!(windows::canonical_sid(&daemon_sid).unwrap(), daemon_sid);
        let sddl = windows::security_sddl(&daemon_sid, &["S-1-5-18".to_owned()]);
        assert!(sddl.starts_with("D:P"));
        assert!(sddl.contains(&daemon_sid));
        assert!(sddl.contains("S-1-5-18"));
        assert!(!sddl.contains(";;;WD"));
        assert!(!sddl.contains(";;;AN"));
        assert!(!sddl.contains(";;;AU"));
        assert!(matches!(
            windows::canonical_sid("not-a-sid"),
            Err(OperatorControlError::InvalidWindowsSid)
        ));
    }

    #[test]
    fn windows_clients_share_the_protocol_and_observe_policy_revocation() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        initialize_fixture_board(&root).unwrap();
        let config = root.join(FIXTURE_CONFIG_FILE);
        let stored = RuntimeConfig::load(&config).unwrap();
        let current_sid = windows_current_process_sid().unwrap();
        assert!(stored
            .operators
            .local_identities
            .iter()
            .any(|identity| matches!(identity, LocalOperatorIdentity::Windows { sid, .. } if sid == &current_sid)));

        let shutdown = Arc::new(AtomicBool::new(false));
        let runtime = Arc::new(BoardRuntime::load(&config).unwrap());
        let handle =
            start_operator_server(Arc::clone(&runtime), config.clone(), Arc::clone(&shutdown))
                .unwrap();
        let async_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        async_runtime.block_on(async {
            let mut first = OperatorClient::connect(&config).await.unwrap();
            let mut second = OperatorClient::connect(&config).await.unwrap();
            assert_eq!(first.features(), OperatorFeature::ALL);
            let board = first.board_status().await.unwrap();
            assert_eq!(board.schema_version, 19);
            let nodes = first.nodes().await.unwrap();
            assert!(!nodes.is_empty());
            assert!(first.node_status(nodes[0].node_id).await.unwrap().is_some());
            let _ = second.subscribe_events(0).await.unwrap();
            let events = first.recent_events(100).await.unwrap();
            let notifications = first.notifications(false, 100).await.unwrap();
            let statistics = first.statistics().await.unwrap();
            let callers = first.recent_callers(100).await.unwrap();
            let maintenance = first.maintenance_status().await.unwrap();
            let encoded = serde_json::to_string(&(
                board,
                nodes,
                events,
                notifications,
                statistics,
                callers,
                maintenance,
            ))
            .unwrap();
            for forbidden in [
                "password_hash",
                "login_identifier",
                "real_name",
                "message_body",
                "private_key",
                "host_path",
            ] {
                assert!(!encoded.contains(forbidden));
            }

            let mut database = sf_core::RuntimeDatabase::open(runtime.database_path()).unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            for sequence in 0..300_u16 {
                let mut event = sf_core::NewOperationalEvent::new(
                    now,
                    sf_core::EventCategory::System,
                    sf_core::EventSeverity::Info,
                    "system.windows-operator-flow-test",
                    sf_core::EventOutcome::Observed,
                );
                event.idempotency_key = Some(format!("windows-operator-flow-test-{sequence}"));
                database.record_operational_event(&event).unwrap();
                // A deliberately slow subscriber may remain connected while
                // using other read projections. Keep the authenticated
                // session active without draining its bounded event queue;
                // heavily parallel Windows suites can otherwise spend longer
                // than the intentional 60-second idle-connection timeout on
                // these durable inserts.
                if sequence % 16 == 15 {
                    first.board_status().await.unwrap();
                    second.board_status().await.unwrap();
                }
            }
            drop(database);
            let overflowed = second.subscribe_events(0).await.unwrap();
            assert!(overflowed.gap_before_first);
            assert!(overflowed.events.len() <= sf_core::MAX_LIVE_SUBSCRIBER_EVENTS);

            let mut revoked = RuntimeConfig::load(&config).unwrap();
            revoked.operators.local_identities.clear();
            revoked
                .operators
                .local_identities
                .push(LocalOperatorIdentity::Windows {
                    sid: "S-1-5-18".to_owned(),
                    // Display labels never substitute for the verified SID.
                    label: Some("fixture creator".to_owned()),
                    capabilities: all_read_capabilities(),
                });
            revoked.save_atomic(&config).unwrap();
            assert!(matches!(
                first.board_status().await,
                Err(OperatorControlError::AuthorizationDenied)
            ));
            assert!(matches!(
                OperatorClient::connect(&config).await,
                Err(OperatorControlError::AuthenticationFailed)
            ));
            drop(second);
        });
        shutdown.store(true, Ordering::SeqCst);
        handle.join().unwrap();
    }

    #[test]
    fn windows_transport_rejects_bad_frames_replay_and_stale_generation() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        initialize_fixture_board(&root).unwrap();
        let config = root.join(FIXTURE_CONFIG_FILE);
        let shutdown = Arc::new(AtomicBool::new(false));
        let runtime = Arc::new(BoardRuntime::load(&config).unwrap());
        let handle =
            start_operator_server(Arc::clone(&runtime), config.clone(), Arc::clone(&shutdown))
                .unwrap();
        let async_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        async_runtime.block_on(async {
            let pipe_name = windows::pipe_name(&config).unwrap();
            let mut mismatch = windows::open_pipe(&pipe_name).unwrap();
            write_frame(
                &mut mismatch,
                &ClientMessage::Hello {
                    major: 99,
                    minor: 0,
                    features: vec![],
                },
            )
            .await
            .unwrap();
            assert!(matches!(
                read_frame::<ServerMessage>(&mut mismatch).await.unwrap(),
                ServerMessage::Error {
                    code: ErrorCode::ProtocolMismatch,
                    ..
                }
            ));

            let mut oversized = windows::open_pipe(&pipe_name).unwrap();
            oversized
                .write_all(&((MAX_OPERATOR_FRAME_BYTES as u32) + 1).to_be_bytes())
                .await
                .unwrap();
            let oversized_result = tokio::time::timeout(
                Duration::from_secs(1),
                read_frame::<ServerMessage>(&mut oversized),
            )
            .await;
            assert!(matches!(oversized_result, Err(_) | Ok(Err(_))));

            let mut malformed = windows::open_pipe(&pipe_name).unwrap();
            malformed.write_all(&1_u32.to_be_bytes()).await.unwrap();
            malformed.write_all(b"{").await.unwrap();
            let malformed_result = tokio::time::timeout(
                Duration::from_secs(1),
                read_frame::<ServerMessage>(&mut malformed),
            )
            .await;
            assert!(matches!(malformed_result, Err(_) | Ok(Err(_))));

            let mut first = windows::open_pipe(&pipe_name).unwrap();
            write_frame(
                &mut first,
                &ClientMessage::Hello {
                    major: 1,
                    minor: 0,
                    features: OperatorFeature::ALL.to_vec(),
                },
            )
            .await
            .unwrap();
            let ServerMessage::Hello {
                session_id,
                challenge,
                daemon_generation,
                ..
            } = read_frame::<ServerMessage>(&mut first).await.unwrap()
            else {
                panic!("hello expected")
            };
            drop(first);

            let mut replay = windows::open_pipe(&pipe_name).unwrap();
            write_frame(
                &mut replay,
                &ClientMessage::Hello {
                    major: 1,
                    minor: 0,
                    features: OperatorFeature::ALL.to_vec(),
                },
            )
            .await
            .unwrap();
            let _ = read_frame::<ServerMessage>(&mut replay).await.unwrap();
            write_frame(
                &mut replay,
                &ClientMessage::Authenticate {
                    session_id,
                    challenge,
                    daemon_generation,
                },
            )
            .await
            .unwrap();
            assert!(matches!(
                read_frame::<ServerMessage>(&mut replay).await.unwrap(),
                ServerMessage::Error {
                    code: ErrorCode::AuthenticationFailed,
                    ..
                }
            ));

            let mut limited = windows::open_pipe(&pipe_name).unwrap();
            write_frame(
                &mut limited,
                &ClientMessage::Hello {
                    major: 1,
                    minor: 0,
                    features: vec![OperatorFeature::BoardStatus],
                },
            )
            .await
            .unwrap();
            let ServerMessage::Hello {
                session_id,
                challenge,
                daemon_generation,
                ..
            } = read_frame::<ServerMessage>(&mut limited).await.unwrap()
            else {
                panic!("hello expected")
            };
            write_frame(
                &mut limited,
                &ClientMessage::Authenticate {
                    session_id: session_id.clone(),
                    challenge,
                    daemon_generation: daemon_generation.clone(),
                },
            )
            .await
            .unwrap();
            assert!(matches!(
                read_frame::<ServerMessage>(&mut limited).await.unwrap(),
                ServerMessage::Authenticated { .. }
            ));
            write_frame(
                &mut limited,
                &ClientMessage::Request {
                    session_id,
                    daemon_generation,
                    request_id: 9,
                    deadline_ms: 1_000,
                    operation: ReadOperation::ListNodes,
                },
            )
            .await
            .unwrap();
            assert!(matches!(
                read_frame::<ServerMessage>(&mut limited).await.unwrap(),
                ServerMessage::Error {
                    code: ErrorCode::UnsupportedFeature,
                    ..
                }
            ));

            let mut stale = OperatorClient::connect(&config).await.unwrap();
            stale.daemon_generation = "0".repeat(32);
            assert!(matches!(
                stale.board_status().await,
                Err(OperatorControlError::StaleDaemonGeneration)
            ));

            drop(windows::open_pipe(&pipe_name).unwrap());
            assert!(OperatorClient::connect(&config).await.is_ok());
        });
        shutdown.store(true, Ordering::SeqCst);
        handle.join().unwrap();
    }
}

impl ReadOperation {
    fn feature(&self) -> OperatorFeature {
        match self {
            Self::BoardStatus => OperatorFeature::BoardStatus,
            Self::ListNodes => OperatorFeature::NodeList,
            Self::NodeStatus { .. } => OperatorFeature::NodeStatus,
            Self::RecentEvents { .. } => OperatorFeature::RecentEvents,
            Self::SubscribeEvents { .. } | Self::CancelEventSubscription => {
                OperatorFeature::LiveEvents
            }
            Self::Notifications { .. } => OperatorFeature::Notifications,
            Self::Statistics => OperatorFeature::Statistics,
            Self::RecentCallers { .. } => OperatorFeature::RecentCallers,
            Self::MaintenanceStatus => OperatorFeature::MaintenanceStatus,
        }
    }
}

#[cfg(any(unix, test))]
fn all_read_capabilities() -> Vec<LocalOperatorCapability> {
    vec![
        LocalOperatorCapability::BoardStatistics,
        LocalOperatorCapability::NodeStatus,
        LocalOperatorCapability::OperationalEvents,
        LocalOperatorCapability::CallerActivity,
        LocalOperatorCapability::Notifications,
        LocalOperatorCapability::MaintenanceStatus,
    ]
}

fn permitted(feature: OperatorFeature, capabilities: &[LocalOperatorCapability]) -> bool {
    let required = match feature {
        OperatorFeature::BoardStatus | OperatorFeature::Statistics => {
            LocalOperatorCapability::BoardStatistics
        }
        OperatorFeature::NodeList | OperatorFeature::NodeStatus => {
            LocalOperatorCapability::NodeStatus
        }
        OperatorFeature::RecentEvents | OperatorFeature::LiveEvents => {
            LocalOperatorCapability::OperationalEvents
        }
        OperatorFeature::Notifications => LocalOperatorCapability::Notifications,
        OperatorFeature::RecentCallers => LocalOperatorCapability::CallerActivity,
        OperatorFeature::MaintenanceStatus => LocalOperatorCapability::MaintenanceStatus,
    };
    capabilities.contains(&required)
}

fn context(
    peer: &PeerIdentity,
    capabilities: &[LocalOperatorCapability],
) -> OperatorObservabilityContext {
    let has = |capability| capabilities.contains(&capability);
    OperatorObservabilityContext {
        principal: OperatorPrincipal {
            kind: OperatorPrincipalKind::HostOperator,
            stable_id: Some(peer.stable_id()),
        },
        capabilities: ObservabilityCapabilities {
            view_board_statistics: has(LocalOperatorCapability::BoardStatistics),
            view_node_status: has(LocalOperatorCapability::NodeStatus),
            view_operational_events: has(LocalOperatorCapability::OperationalEvents),
            view_caller_activity: has(LocalOperatorCapability::CallerActivity),
            view_notifications: has(LocalOperatorCapability::Notifications),
            view_maintenance_status: has(LocalOperatorCapability::MaintenanceStatus),
            acknowledge_notifications: false,
        },
    }
}

async fn dispatch(
    service: &OperatorService,
    context: &OperatorObservabilityContext,
    operation: ReadOperation,
    live_subscription: &mut Option<sf_core::LiveEventSubscription>,
) -> Result<ReadResult, crate::ApplicationError> {
    Ok(match operation {
        ReadOperation::BoardStatus => {
            ReadResult::BoardStatus(service.board_status(context)?.into())
        }
        ReadOperation::ListNodes => ReadResult::Nodes(
            service
                .live_nodes(context)?
                .into_iter()
                .map(Into::into)
                .collect(),
        ),
        ReadOperation::NodeStatus { node_id } => ReadResult::Node(
            service
                .live_nodes(context)?
                .into_iter()
                .find(|item| item.node_id == node_id)
                .map(Into::into),
        ),
        ReadOperation::RecentEvents { query } => {
            let caller_id = query
                .caller_id
                .map(|value| {
                    i64::try_from(value)
                        .ok()
                        .and_then(|value| CallerId::new(value).ok())
                        .ok_or(OperatorControlError::MalformedFrame)
                })
                .transpose()?;
            let cursor = query
                .cursor
                .map(|cursor| {
                    Ok::<_, OperatorControlError>(EventCursor {
                        occurred_at_utc: cursor.occurred_at_utc,
                        event_id: EventId::new(cursor.event_id)
                            .ok_or(OperatorControlError::MalformedFrame)?,
                        snapshot_event_id: EventId::new(cursor.snapshot_event_id)
                            .ok_or(OperatorControlError::MalformedFrame)?,
                    })
                })
                .transpose()?;
            let query = EventQuery {
                from_utc: query.from_utc,
                through_utc: query.through_utc,
                category: query.category,
                minimum_severity: query.minimum_severity,
                outcome: query.outcome,
                node_id: query.node_id,
                caller_id,
                cursor,
                limit: query.limit,
            };
            let page = service.recent_events(context, &query)?;
            ReadResult::Events(EventBatchWire {
                events: page.events.into_iter().map(event_wire).collect(),
                gap_before_first: false,
                has_more: page.next_cursor.is_some(),
                next_cursor: page.next_cursor.map(Into::into),
            })
        }
        ReadOperation::SubscribeEvents { wait_ms } => {
            let wait_ms = wait_ms.min(4_500);
            let deadline = tokio::time::Instant::now() + Duration::from_millis(wait_ms);
            if live_subscription.is_none() {
                *live_subscription = Some(service.subscribe_events(context)?);
            }
            let subscription = live_subscription
                .as_ref()
                .expect("subscription was initialized");
            loop {
                let batch = service.poll_events(context, subscription)?;
                if !batch.events.is_empty()
                    || batch.gap_before_first
                    || tokio::time::Instant::now() >= deadline
                {
                    break ReadResult::Events(EventBatchWire {
                        events: batch.events.into_iter().map(event_wire).collect(),
                        gap_before_first: batch.gap_before_first,
                        has_more: false,
                        next_cursor: None,
                    });
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        ReadOperation::CancelEventSubscription => {
            *live_subscription = None;
            ReadResult::SubscriptionCancelled
        }
        ReadOperation::Notifications {
            include_closed,
            limit,
        } => ReadResult::Notifications(
            service
                .notifications(context, include_closed, limit.min(500))?
                .into_iter()
                .map(Into::into)
                .collect(),
        ),
        ReadOperation::Statistics => ReadResult::Statistics(service.statistics(context)?.into()),
        ReadOperation::RecentCallers { limit } => ReadResult::RecentCallers(
            service
                .recent_callers(context, limit.min(500))?
                .into_iter()
                .map(|value| RecentCallerWire {
                    event_id: value.event_id.get(),
                    public_handle: value.public_handle,
                    occurred_at_utc: value.occurred_at_utc,
                    board_day: value.board_day,
                    transport: value.transport,
                    duration_seconds: value.duration_seconds,
                    close_reason: value.close_reason,
                    node_id: value.node_id,
                })
                .collect(),
        ),
        ReadOperation::MaintenanceStatus => {
            ReadResult::Maintenance(service.maintenance_status(context)?.into())
        }
    })
}

fn event_wire(value: sf_core::OperationalEvent) -> EventWire {
    EventWire {
        event_id: value.id.get(),
        occurred_at_utc: value.occurred_at_utc,
        board_day: value.board_day,
        category: value.category.as_str().to_owned(),
        severity: value.severity.as_str().to_owned(),
        event_code: value.event_code,
        outcome: value.outcome.as_str().to_owned(),
        node_id: value.node_id,
        session_id: value.session_id,
        correlation_id: value.correlation_id,
        object_kind: value.object_kind,
        object_id: value.object_id,
        attributes: serde_json::to_value(value.attributes).unwrap_or(Value::Null),
    }
}

impl From<BoardStatus> for BoardStatusWire {
    fn from(v: BoardStatus) -> Self {
        Self {
            board_name: v.board_name,
            running_since_utc: v.running_since_utc,
            uptime_seconds: v.uptime_seconds,
            schema_version: v.schema_version,
            configured_nodes: v.configured_nodes,
            active_nodes: v.active_nodes,
            callers_online: v.callers_online,
            active_transfers: v.active_transfers,
            storage_warnings: v.storage_warnings,
            recent_errors: v.recent_errors,
            open_notifications: v.open_notifications,
        }
    }
}
impl From<LiveNodeStatus> for NodeStatusWire {
    fn from(v: LiveNodeStatus) -> Self {
        Self {
            node_id: v.node_id,
            lifecycle: v.lifecycle,
            session_id: v.session_id,
            public_handle: v.public_handle,
            transport: v.transport,
            online_seconds: v.online_seconds,
            current_section: v.current_section,
            terminal_type: v.terminal_type,
            encoding: v.encoding,
            columns: v.columns,
            rows: v.rows,
            presentation_profile: v.presentation_profile,
            security_context: v.security_context,
            transfer_state: v.transfer_state,
        }
    }
}
impl From<OperatorNotification> for NotificationWire {
    fn from(v: OperatorNotification) -> Self {
        Self {
            notification_id: v.id.get(),
            source_event_id: v.source_event_id.get(),
            created_at: v.created_at,
            category: v.category.as_str().to_owned(),
            severity: v.severity.as_str().to_owned(),
            reason_key: v.reason_key,
            remediation_key: v.remediation_key,
            state: format!("{:?}", v.state).to_ascii_lowercase(),
            state_version: v.state_version,
        }
    }
}
impl From<SystemStatistics> for StatisticsWire {
    fn from(v: SystemStatistics) -> Self {
        Self {
            observability_activated_at: v.observability_activated_at,
            board_day: v.today.board_day,
            calls_started_today: v.today.calls_started,
            calls_completed_today: v.today.calls_completed,
            messages_posted_today: v.today.messages_posted,
            successful_uploads_today: v.today.successful_uploads,
            successful_downloads_today: v.today.successful_downloads,
            lifetime_calls: v.lifetime_calls,
            lifetime_messages_posted: v.lifetime_messages_posted,
            lifetime_files_uploaded: v.lifetime_files_uploaded,
            lifetime_files_downloaded: v.lifetime_files_downloaded,
        }
    }
}
impl From<EventCursor> for EventCursorWire {
    fn from(v: EventCursor) -> Self {
        Self {
            occurred_at_utc: v.occurred_at_utc,
            event_id: v.event_id.get(),
            snapshot_event_id: v.snapshot_event_id.get(),
        }
    }
}
impl From<MaintenanceStatus> for MaintenanceWire {
    fn from(v: MaintenanceStatus) -> Self {
        Self {
            open_notifications: v.open_notifications,
            recent_warning_events: v.recent_warning_events,
            recent_error_events: v.recent_error_events,
            unavailable_storage_roots: v.unavailable_storage_roots,
            pending_review_files: v.pending_review_files,
            nonterminal_transfers: v.nonterminal_transfers,
            detail_retention_days: v.retention.detail_days,
            summary_retention_days: v.retention.summary_days,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{initialize_fixture_board, BoardRuntime, FIXTURE_CONFIG_FILE};

    #[test]
    fn authorized_clients_attach_concurrently_and_restart_changes_generation() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        initialize_fixture_board(&root).unwrap();
        let config = root.join(FIXTURE_CONFIG_FILE);
        let shutdown = Arc::new(AtomicBool::new(false));
        let runtime = Arc::new(BoardRuntime::load(&config).unwrap());
        let handle =
            start_operator_server(Arc::clone(&runtime), config.clone(), Arc::clone(&shutdown))
                .unwrap();
        let generation = runtime.daemon_generation().to_owned();
        let async_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        async_runtime.block_on(async {
            let mut first = OperatorClient::connect(&config).await.unwrap();
            let mut second = OperatorClient::connect(&config).await.unwrap();
            assert_eq!(first.daemon_generation(), generation);
            assert_eq!(first.board_status().await.unwrap().schema_version, 19);
            assert!(!first.nodes().await.unwrap().is_empty());
            assert!(second
                .recent_events(100)
                .await
                .unwrap()
                .events
                .iter()
                .all(|event| event.attributes.to_string().len() <= 1024));
            assert!(first.notifications(false, 100).await.unwrap().is_empty());
            let _ = second.statistics().await.unwrap();
            let _ = first.maintenance_status().await.unwrap();
            let database_path = runtime.database_path().to_path_buf();
            let producer = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                let mut database = sf_core::RuntimeDatabase::open(&database_path).unwrap();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                database
                    .record_operational_event(&sf_core::NewOperationalEvent::new(
                        now,
                        sf_core::EventCategory::System,
                        sf_core::EventSeverity::Notice,
                        "system.operator-attach-test",
                        sf_core::EventOutcome::Observed,
                    ))
                    .unwrap();
            });
            let live = second.subscribe_events(1_000).await.unwrap();
            producer.join().unwrap();
            assert!(live
                .events
                .iter()
                .any(|event| event.event_code == "system.operator-attach-test"));
            let first_page = first
                .query_events(OperatorEventQuery {
                    limit: Some(1),
                    ..OperatorEventQuery::default()
                })
                .await
                .unwrap();
            let cursor = first_page.next_cursor.expect("a second event remains");
            let second_page = first
                .query_events(OperatorEventQuery {
                    cursor: Some(cursor),
                    limit: Some(1),
                    ..OperatorEventQuery::default()
                })
                .await
                .unwrap();
            assert_eq!(second_page.events.len(), 1);
            assert_ne!(
                first_page.events[0].event_id,
                second_page.events[0].event_id
            );
            second.cancel_event_subscription().await.unwrap();
            let mut slow = OperatorClient::connect(&config).await.unwrap();
            let _ = slow.subscribe_events(0).await.unwrap();
            let mut database = sf_core::RuntimeDatabase::open(runtime.database_path()).unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            for sequence in 0..300_u16 {
                let mut event = sf_core::NewOperationalEvent::new(
                    now,
                    sf_core::EventCategory::System,
                    sf_core::EventSeverity::Info,
                    "system.operator-flow-test",
                    sf_core::EventOutcome::Observed,
                );
                event.idempotency_key = Some(format!("operator-flow-test-{sequence}"));
                database.record_operational_event(&event).unwrap();
            }
            drop(database);
            let overflowed = slow.subscribe_events(0).await.unwrap();
            assert!(overflowed.gap_before_first);
            assert!(overflowed.events.len() <= sf_core::MAX_LIVE_SUBSCRIBER_EVENTS);
            let encoded = serde_json::to_string(&live).unwrap();
            for forbidden in [
                "password_hash",
                "login_identifier",
                "real_name",
                "message_body",
                "private_key",
                "/Users/",
            ] {
                assert!(!encoded.contains(forbidden));
            }
        });
        shutdown.store(true, Ordering::SeqCst);
        handle.join().unwrap();
        drop(runtime);

        let shutdown = Arc::new(AtomicBool::new(false));
        let runtime = Arc::new(BoardRuntime::load(&config).unwrap());
        assert_ne!(generation, runtime.daemon_generation());
        let handle =
            start_operator_server(Arc::clone(&runtime), config.clone(), Arc::clone(&shutdown))
                .unwrap();
        async_runtime.block_on(async {
            assert_ne!(
                OperatorClient::connect(&config)
                    .await
                    .unwrap()
                    .daemon_generation(),
                generation
            );
        });
        shutdown.store(true, Ordering::SeqCst);
        handle.join().unwrap();
    }

    #[test]
    fn unsafe_endpoint_and_oversized_frames_fail_closed() {
        use std::io::Write;
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixStream as StdUnixStream;

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        initialize_fixture_board(&root).unwrap();
        let config = root.join(FIXTURE_CONFIG_FILE);
        let endpoint = endpoint_for_config(&config).unwrap();
        std::fs::create_dir_all(endpoint.parent().unwrap()).unwrap();
        let target = root.join("do-not-touch");
        std::fs::write(&target, b"safe").unwrap();
        symlink(&target, &endpoint).unwrap();
        let runtime = Arc::new(BoardRuntime::load(&config).unwrap());
        let shutdown = Arc::new(AtomicBool::new(false));
        assert!(matches!(
            start_operator_server(Arc::clone(&runtime), config.clone(), shutdown),
            Err(OperatorControlError::UnsafeEndpoint(_))
        ));
        std::fs::remove_file(&endpoint).unwrap();
        let stale = std::os::unix::net::UnixListener::bind(&endpoint).unwrap();
        drop(stale);
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            start_operator_server(Arc::clone(&runtime), config.clone(), Arc::clone(&shutdown))
                .unwrap();
        assert!(matches!(
            start_operator_server(runtime, config.clone(), Arc::new(AtomicBool::new(false))),
            Err(OperatorControlError::UnsafeEndpoint(_))
        ));
        let mut stream = StdUnixStream::connect(&endpoint).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        stream
            .write_all(&((MAX_OPERATOR_FRAME_BYTES as u32) + 1).to_be_bytes())
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let mut one = [0_u8; 1];
        assert!(std::io::Read::read(&mut stream, &mut one).is_err() || one == [0]);
        let mut malformed = StdUnixStream::connect(&endpoint).unwrap();
        malformed
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        malformed.write_all(&3_u32.to_be_bytes()).unwrap();
        malformed.write_all(b"bad").unwrap();
        assert!(std::io::Read::read(&mut malformed, &mut one).is_err() || one == [0]);
        shutdown.store(true, Ordering::SeqCst);
        handle.join().unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"safe");
    }

    #[test]
    fn protocol_mismatch_challenge_replay_and_policy_revocation_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        initialize_fixture_board(&root).unwrap();
        let config = root.join(FIXTURE_CONFIG_FILE);
        let shutdown = Arc::new(AtomicBool::new(false));
        let runtime = Arc::new(BoardRuntime::load(&config).unwrap());
        let handle =
            start_operator_server(Arc::clone(&runtime), config.clone(), Arc::clone(&shutdown))
                .unwrap();
        let tokio = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio.block_on(async {
            let endpoint = endpoint_for_config(&config).unwrap();
            let mut mismatch = tokio::net::UnixStream::connect(&endpoint).await.unwrap();
            write_frame(
                &mut mismatch,
                &ClientMessage::Hello {
                    major: 99,
                    minor: 0,
                    features: vec![],
                },
            )
            .await
            .unwrap();
            assert!(matches!(
                read_frame::<ServerMessage>(&mut mismatch).await.unwrap(),
                ServerMessage::Error {
                    code: ErrorCode::ProtocolMismatch,
                    ..
                }
            ));

            let mut first = tokio::net::UnixStream::connect(&endpoint).await.unwrap();
            write_frame(
                &mut first,
                &ClientMessage::Hello {
                    major: 1,
                    minor: 0,
                    features: OperatorFeature::ALL.to_vec(),
                },
            )
            .await
            .unwrap();
            let ServerMessage::Hello {
                session_id,
                challenge,
                daemon_generation,
                ..
            } = read_frame::<ServerMessage>(&mut first).await.unwrap()
            else {
                panic!("hello expected")
            };
            drop(first);
            let mut replay = tokio::net::UnixStream::connect(&endpoint).await.unwrap();
            write_frame(
                &mut replay,
                &ClientMessage::Hello {
                    major: 1,
                    minor: 0,
                    features: OperatorFeature::ALL.to_vec(),
                },
            )
            .await
            .unwrap();
            let _ = read_frame::<ServerMessage>(&mut replay).await.unwrap();
            write_frame(
                &mut replay,
                &ClientMessage::Authenticate {
                    session_id,
                    challenge,
                    daemon_generation,
                },
            )
            .await
            .unwrap();
            assert!(matches!(
                read_frame::<ServerMessage>(&mut replay).await.unwrap(),
                ServerMessage::Error {
                    code: ErrorCode::AuthenticationFailed,
                    ..
                }
            ));

            let mut limited = tokio::net::UnixStream::connect(&endpoint).await.unwrap();
            write_frame(
                &mut limited,
                &ClientMessage::Hello {
                    major: 1,
                    minor: 0,
                    features: vec![OperatorFeature::BoardStatus],
                },
            )
            .await
            .unwrap();
            let ServerMessage::Hello {
                session_id,
                challenge,
                daemon_generation,
                ..
            } = read_frame::<ServerMessage>(&mut limited).await.unwrap()
            else {
                panic!("hello expected")
            };
            write_frame(
                &mut limited,
                &ClientMessage::Authenticate {
                    session_id: session_id.clone(),
                    challenge,
                    daemon_generation: daemon_generation.clone(),
                },
            )
            .await
            .unwrap();
            assert!(matches!(
                read_frame::<ServerMessage>(&mut limited).await.unwrap(),
                ServerMessage::Authenticated { .. }
            ));
            write_frame(
                &mut limited,
                &ClientMessage::Request {
                    session_id,
                    daemon_generation,
                    request_id: 9,
                    deadline_ms: 1_000,
                    operation: ReadOperation::ListNodes,
                },
            )
            .await
            .unwrap();
            assert!(matches!(
                read_frame::<ServerMessage>(&mut limited).await.unwrap(),
                ServerMessage::Error {
                    code: ErrorCode::UnsupportedFeature,
                    ..
                }
            ));
        });

        let mut attached = tokio.block_on(OperatorClient::connect(&config)).unwrap();
        let mut stale = tokio.block_on(OperatorClient::connect(&config)).unwrap();
        stale.daemon_generation = "0".repeat(32);
        assert!(matches!(
            tokio.block_on(stale.board_status()),
            Err(OperatorControlError::StaleDaemonGeneration)
        ));

        let mut stored = RuntimeConfig::load(&config).unwrap();
        if let LocalOperatorIdentity::Unix { uid, .. } = &mut stored.operators.local_identities[0] {
            *uid = uid.saturating_add(1);
        }
        stored.save_atomic(&config).unwrap();
        tokio.block_on(async {
            assert!(matches!(
                attached.board_status().await,
                Err(OperatorControlError::AuthorizationDenied)
            ));
            assert!(matches!(
                OperatorClient::connect(&config).await,
                Err(OperatorControlError::AuthenticationFailed)
            ));
        });
        shutdown.store(true, Ordering::SeqCst);
        handle.join().unwrap();
        let database = rusqlite::Connection::open(runtime.database_path()).unwrap();
        assert_eq!(
            database
                .query_row(
                    "SELECT COUNT(*) FROM operator_control_audit WHERE operator_kind='unknown-peer' AND operator_id IS NOT NULL",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }
}
