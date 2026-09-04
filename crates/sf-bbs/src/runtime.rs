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

use std::io::{self, BufReader};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use sf_core::{
    run_stock_session, BoardIdentity, Caller, CallerAccessActor, CallerActivity, CallerConfig,
    CallerState, CredentialHasher, EventCategory, EventOutcome, EventPage, EventQuery,
    EventSeverity, FileStorage, InteractionHub, JokerPolicy, LogicalPath, LogicalPaths,
    MaintenanceStatus, MessageActivityPage, NewOperationalEvent, NodeError, NodeManager,
    NodeRuntimeState, ObservabilityService, OperatorNotification, OperatorPrincipal, RecentCaller,
    RuntimeConfig, RuntimeDatabase, SecurityLevel, SessionCloseReason, SessionId, SessionState,
    StockSessionContext, SystemStatistics, Terminal, TransferActivityPage, TransportAdapterConfig,
    TransportConfig, TransportKind, VerifiedCallerGrant,
};
use tracing::{info, warn};

use crate::board_lock::BoardOperationLock;
use crate::fixture::{seed_fixture_file_areas, seed_fixture_messages, seed_starter_files};
use crate::operator::{run_operator_console, OperatorService};
use crate::resources::load_stock_resources;
use crate::status::{publish_runtime_status, remove_runtime_status, RUNTIME_STATUS_FILE};
use crate::transports::{
    load_or_generate_host_key, serve_ssh_listener, ModemTerminal, RawTcpTerminal, RloginTerminal,
    SerialTerminal, SshListenerOptions, TelnetTerminal,
};
use crate::ApplicationError;
use crate::PresentationResolver;

const NODE_BUSY_MESSAGE: &[u8] =
    b"\r\nAll SPITFIRE nodes are currently busy. Please call again later.\r\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunReport {
    pub board_name: String,
    pub node_id: u32,
    pub database_path: PathBuf,
    pub schema_version: u32,
    pub session_id: u64,
    pub commands_processed: usize,
    pub close_reason: SessionCloseReason,
    pub transport: TransportKind,
    pub caller_id: Option<i64>,
    pub caller_name: Option<String>,
    pub node_idle_at_shutdown: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionReport {
    Completed(RunReport),
    NodeBusy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServeReport {
    pub listeners: Vec<ListenerReport>,
    pub completed_sessions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListenerReport {
    pub name: String,
    pub transport: TransportKind,
    pub address: SocketAddr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservabilityCapabilities {
    pub view_board_statistics: bool,
    pub view_node_status: bool,
    pub view_operational_events: bool,
    pub view_caller_activity: bool,
    pub view_notifications: bool,
    pub view_maintenance_status: bool,
    pub acknowledge_notifications: bool,
}

impl ObservabilityCapabilities {
    pub const fn host_operator() -> Self {
        Self {
            view_board_statistics: true,
            view_node_status: true,
            view_operational_events: true,
            view_caller_activity: true,
            view_notifications: true,
            view_maintenance_status: true,
            acknowledge_notifications: true,
        }
    }

    pub const fn named_sysop() -> Self {
        Self {
            view_board_statistics: true,
            view_node_status: true,
            view_operational_events: true,
            view_caller_activity: true,
            view_notifications: true,
            view_maintenance_status: true,
            acknowledge_notifications: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorObservabilityContext {
    pub principal: OperatorPrincipal,
    pub capabilities: ObservabilityCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardStatus {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveNodeStatus {
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

pub struct BoardRuntime {
    _operation_lock: BoardOperationLock,
    identity: BoardIdentity,
    timezone: chrono_tz::Tz,
    board_access: sf_core::BoardAccessMode,
    private_security_level: SecurityLevel,
    paths: LogicalPaths,
    nodes: NodeManager,
    next_session: AtomicU64,
    schema_version: u32,
    transports: Vec<TransportConfig>,
    caller_config: CallerConfig,
    credential_hasher: CredentialHasher,
    file_storage: FileStorage,
    interaction: InteractionHub,
    presentation: PresentationResolver,
    language: sf_core::LanguageResolver,
    joker_policy: JokerPolicy,
    status_path: PathBuf,
    started_at: i64,
    observability: ObservabilityService,
    daemon_generation: String,
}

impl BoardRuntime {
    pub fn load(config_path: &Path) -> Result<Self, ApplicationError> {
        info!(
            version = sf_core::PRODUCT_VERSION,
            "loading SPITFIRE NG runtime configuration"
        );
        let canonical_config = config_path.canonicalize().map_err(|source| {
            ApplicationError::ResolveConfiguration {
                path: config_path.to_path_buf(),
                source,
            }
        })?;
        let root = canonical_config
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| ApplicationError::MissingBoardRoot(canonical_config.clone()))?;
        let operation_lock = BoardOperationLock::acquire(root)?;
        let config = RuntimeConfig::load(&canonical_config)?;
        let validated = config.validate()?;
        let paths = LogicalPaths::resolve(root, &validated)?;
        paths.create_directories()?;
        let presentation = PresentationResolver::load(&paths, &validated.presentation);
        let language = sf_core::LanguageResolver::load(&paths, &validated.language.default_locale);
        let joker_policy = JokerPolicy::load(
            &paths.get(LogicalPath::System).join("JOKER.DAT"),
            1,
            &validated.caller.sysop_caller_name,
        )?;
        info!(
            mode = ?presentation.status().mode,
            source = presentation.status().effective_source,
            degraded = presentation.status().degraded,
            "presentation resolver initialized"
        );

        info!("opening and migrating the operational SQLite database");
        let mut database = RuntimeDatabase::open(paths.database())?;
        let migration = database.migrate()?;
        let identity = database.ensure_board_identity(&validated.identity)?;
        let file_storage = FileStorage::new(&paths)?;
        if identity.name() == "SPITFIRE NG Fixture Board"
            && identity.sysop_name() == "Fixture Sysop"
        {
            seed_fixture_messages(&mut database)?;
            let areas = seed_fixture_file_areas(&mut database, &file_storage)?;
            seed_starter_files(&mut database, &file_storage, &areas)?;
        }
        let resource_observed_at = current_unix_seconds()?;
        for (kind, digest) in crate::resources::public_resource_digests(&paths)? {
            database.observe_public_resource(kind, &digest, resource_observed_at)?;
        }
        let schema_version = database.schema_version()?;
        let started_at = current_unix_seconds()?;
        if schema_version >= 18 {
            database.synchronize_transfer_timezone(validated.timezone, started_at)?;
        }
        if schema_version >= 16 {
            let recovered = database.reconcile_interrupted_transfers(current_unix_seconds()?)?;
            if recovered != 0 {
                warn!(
                    recovered,
                    "released nonterminal transfer reservations after daemon restart"
                );
            }
        }
        let credential_hasher = CredentialHasher::new(&validated.caller.password)?;
        let timezone = validated.timezone;
        let board_access = validated.board_access;
        let private_security_level = SecurityLevel::new(validated.private_security_level)
            .map_err(sf_core::DatabaseError::from)?;
        let configured_nodes = validated.nodes.len();
        let status_path = paths
            .get(sf_core::LogicalPath::Work)
            .join(RUNTIME_STATUS_FILE);
        let status_board = identity.name().to_owned();
        let status_transports = validated.transports.clone();
        let status_output = status_path.clone();
        let status_database = paths.database().to_path_buf();
        let observability = ObservabilityService::new(paths.database(), started_at);
        let started = NewOperationalEvent::new(
            started_at,
            EventCategory::System,
            EventSeverity::Notice,
            "system.started",
            EventOutcome::Succeeded,
        );
        observability.record(&started)?;
        let nodes = NodeManager::with_change_hook(
            validated.nodes,
            Some(Arc::new(move |snapshots| {
                if let Err(error) = publish_runtime_status(
                    &status_output,
                    &status_board,
                    started_at,
                    &status_transports,
                    &status_database,
                    snapshots,
                ) {
                    warn!(error = %error, "could not publish transient node status");
                }
            })),
        )?;
        info!(
            board = identity.name(),
            configured_nodes,
            schema_version,
            migrations_applied = migration.applied,
            "board runtime initialized"
        );

        Ok(Self {
            _operation_lock: operation_lock,
            identity,
            timezone,
            board_access,
            private_security_level,
            paths,
            nodes,
            next_session: AtomicU64::new(1),
            schema_version,
            transports: validated.transports,
            caller_config: validated.caller,
            credential_hasher,
            file_storage,
            interaction: InteractionHub::new(),
            presentation,
            language,
            joker_policy,
            status_path,
            started_at,
            observability,
            daemon_generation: crate::operator_control::random_token(),
        })
    }

    pub fn transports(&self) -> &[TransportConfig] {
        &self.transports
    }

    pub fn node_snapshots(&self) -> Result<Vec<sf_core::NodeSnapshot>, ApplicationError> {
        self.nodes.snapshots().map_err(Into::into)
    }

    pub fn board_status(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<BoardStatus, ApplicationError> {
        require(context.capabilities.view_board_statistics)?;
        let now = current_unix_seconds()?;
        let nodes = self.nodes.snapshots()?;
        let maintenance = RuntimeDatabase::open(self.paths.database())?.maintenance_status(now)?;
        Ok(BoardStatus {
            board_name: self.identity.name().to_owned(),
            running_since_utc: self.started_at,
            uptime_seconds: u64::try_from(now.saturating_sub(self.started_at)).unwrap_or(0),
            schema_version: self.schema_version,
            configured_nodes: nodes.len(),
            active_nodes: nodes
                .iter()
                .filter(|node| {
                    !matches!(
                        node.state,
                        NodeRuntimeState::Waiting | NodeRuntimeState::Disabled
                    )
                })
                .count(),
            callers_online: nodes
                .iter()
                .filter(|node| node.caller_name.is_some())
                .count(),
            active_transfers: maintenance.nonterminal_transfers,
            storage_warnings: maintenance.unavailable_storage_roots,
            recent_errors: maintenance.recent_error_events,
            open_notifications: maintenance.open_notifications,
        })
    }

    pub fn live_node_statuses(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<Vec<LiveNodeStatus>, ApplicationError> {
        require(context.capabilities.view_node_status)?;
        let now = current_unix_seconds()?;
        Ok(self
            .nodes
            .snapshots()?
            .into_iter()
            .map(|node| {
                let presentation = node.presentation.as_ref();
                LiveNodeStatus {
                    node_id: node.id.get(),
                    lifecycle: format!("{:?}", node.state).to_ascii_lowercase(),
                    session_id: node.session_id.map(SessionId::get),
                    public_handle: node.caller_name,
                    transport: node
                        .transport
                        .map(|transport| transport.as_str().to_owned()),
                    online_seconds: node
                        .connected_at
                        .map(|started| u64::try_from(now.saturating_sub(started)).unwrap_or(0)),
                    current_section: presentation.and_then(|value| value.menu_context.clone()),
                    terminal_type: presentation.and_then(|value| value.terminal_type.clone()),
                    encoding: presentation.map(|value| value.encoding.clone()),
                    columns: presentation.and_then(|value| value.columns),
                    rows: presentation.and_then(|value| value.rows),
                    presentation_profile: presentation
                        .map(|value| value.presentation_profile.clone()),
                    security_context: presentation.and_then(|value| {
                        value.caller_security.map(|level| {
                            if level >= value.sysop_threshold {
                                "sysop-threshold"
                            } else {
                                "caller"
                            }
                            .to_owned()
                        })
                    }),
                    transfer_state: matches!(
                        node.state,
                        NodeRuntimeState::Downloading | NodeRuntimeState::Uploading
                    )
                    .then(|| format!("{:?}", node.state).to_ascii_lowercase()),
                }
            })
            .collect())
    }

    pub fn recent_operational_events(
        &self,
        context: &OperatorObservabilityContext,
        query: &EventQuery,
    ) -> Result<EventPage, ApplicationError> {
        require(context.capabilities.view_operational_events)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .query_operational_events(query)
            .map_err(Into::into)
    }

    pub fn operator_notifications(
        &self,
        context: &OperatorObservabilityContext,
        include_closed: bool,
        limit: usize,
    ) -> Result<Vec<OperatorNotification>, ApplicationError> {
        require(context.capabilities.view_notifications)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .notifications(include_closed, limit)
            .map_err(Into::into)
    }

    pub fn acknowledge_operator_notification(
        &self,
        context: &OperatorObservabilityContext,
        notification_id: sf_core::NotificationId,
        expected_version: u64,
    ) -> Result<bool, ApplicationError> {
        require(context.capabilities.acknowledge_notifications)?;
        RuntimeDatabase::open(self.paths.database())?
            .acknowledge_notification(
                notification_id,
                expected_version,
                &context.principal,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn system_statistics(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<SystemStatistics, ApplicationError> {
        require(context.capabilities.view_board_statistics)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .system_statistics(current_unix_seconds()?)
            .map_err(Into::into)
    }

    pub fn recent_callers(
        &self,
        context: &OperatorObservabilityContext,
        limit: usize,
    ) -> Result<Vec<RecentCaller>, ApplicationError> {
        require(context.capabilities.view_caller_activity)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .recent_callers(limit)
            .map_err(Into::into)
    }

    pub fn caller_activity(
        &self,
        context: &OperatorObservabilityContext,
        caller_id: sf_core::CallerId,
        query: &EventQuery,
    ) -> Result<Option<CallerActivity>, ApplicationError> {
        require(context.capabilities.view_caller_activity)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .caller_activity(caller_id, query)
            .map_err(Into::into)
    }

    pub fn message_activity(
        &self,
        context: &OperatorObservabilityContext,
        from_utc: i64,
        through_utc: i64,
    ) -> Result<MessageActivityPage, ApplicationError> {
        require(context.capabilities.view_board_statistics)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .message_activity(from_utc, through_utc)
            .map_err(Into::into)
    }

    pub fn transfer_activity(
        &self,
        context: &OperatorObservabilityContext,
        from_utc: i64,
        through_utc: i64,
    ) -> Result<TransferActivityPage, ApplicationError> {
        require(context.capabilities.view_board_statistics)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .transfer_activity(from_utc, through_utc)
            .map_err(Into::into)
    }

    pub fn file_activity(
        &self,
        context: &OperatorObservabilityContext,
        query: &EventQuery,
    ) -> Result<EventPage, ApplicationError> {
        require(context.capabilities.view_board_statistics)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .file_activity(query)
            .map_err(Into::into)
    }

    pub fn recent_errors(
        &self,
        context: &OperatorObservabilityContext,
        query: &EventQuery,
    ) -> Result<EventPage, ApplicationError> {
        require(context.capabilities.view_maintenance_status)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .recent_errors(query)
            .map_err(Into::into)
    }

    pub fn maintenance_status(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<MaintenanceStatus, ApplicationError> {
        require(context.capabilities.view_maintenance_status)?;
        RuntimeDatabase::open_read_only(self.paths.database())?
            .maintenance_status(current_unix_seconds()?)
            .map_err(Into::into)
    }

    pub fn live_operational_events(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<Vec<sf_core::OperationalEvent>, ApplicationError> {
        require(context.capabilities.view_operational_events)?;
        self.observability
            .refresh_live(current_unix_seconds()?)
            .map_err(Into::into)
    }

    pub fn live_operational_event_batch(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<sf_core::LiveEventBatch, ApplicationError> {
        require(context.capabilities.view_operational_events)?;
        self.observability
            .refresh_live_batch(current_unix_seconds()?)
            .map_err(Into::into)
    }

    pub fn subscribe_operational_events(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<sf_core::LiveEventSubscription, ApplicationError> {
        require(context.capabilities.view_operational_events)?;
        self.observability.subscribe_live().map_err(Into::into)
    }

    pub fn poll_operational_event_subscription(
        &self,
        context: &OperatorObservabilityContext,
        subscription: &sf_core::LiveEventSubscription,
    ) -> Result<sf_core::LiveEventBatch, ApplicationError> {
        require(context.capabilities.view_operational_events)?;
        self.observability
            .poll_live_subscription(subscription, current_unix_seconds()?)
            .map_err(Into::into)
    }

    pub fn interaction(&self) -> InteractionHub {
        self.interaction.clone()
    }

    pub fn board_name(&self) -> &str {
        self.identity.name()
    }

    pub fn database_path(&self) -> &Path {
        self.paths.database()
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn daemon_generation(&self) -> &str {
        &self.daemon_generation
    }

    pub(crate) fn system_path(&self) -> &Path {
        self.paths.get(LogicalPath::System)
    }

    pub(crate) fn authenticate_ssh_password(
        &self,
        login_identifier: &str,
        password: &str,
    ) -> Result<Option<VerifiedCallerGrant>, ApplicationError> {
        if password.len() > self.caller_config.maximum_password_length
            || sf_core::canonicalize_login_identifier(login_identifier.as_bytes()).is_err()
        {
            warn!("SSH password authentication rejected");
            return Ok(None);
        }
        let database = RuntimeDatabase::open(self.paths.database())?;
        match database.authenticate_login_identifier(
            login_identifier.as_bytes(),
            password.as_bytes(),
            &self.credential_hasher,
        )? {
            sf_core::AuthenticationResult::Valid(caller) => {
                if self
                    .joker_policy
                    .denial_for(caller.display_name.as_bytes())?
                    .is_some()
                {
                    database.record_joker_denial(
                        Some(caller.id),
                        self.joker_policy.generation(),
                        current_unix_seconds()?,
                    )?;
                    warn!(
                        caller_id = caller.id.get(),
                        "SSH authentication denied by caller access policy"
                    );
                    return Ok(None);
                }
                info!(
                    caller_id = caller.id.get(),
                    "SSH password authentication accepted"
                );
                Ok(Some(VerifiedCallerGrant {
                    caller_id: caller.id,
                    authenticated_state_version: caller.state_version,
                }))
            }
            sf_core::AuthenticationResult::Unavailable(caller) => {
                database.record_caller_access_denial(
                    caller.id,
                    current_unix_seconds()?,
                    sf_core::AccessDenialReason::AccountUnavailable,
                )?;
                warn!(
                    caller_id = caller.id.get(),
                    "SSH authentication rejected for unavailable caller"
                );
                Ok(None)
            }
            sf_core::AuthenticationResult::Invalid => {
                let at = current_unix_seconds()?;
                let mut event = NewOperationalEvent::new(
                    at,
                    EventCategory::Authentication,
                    EventSeverity::Notice,
                    "authentication.failed",
                    EventOutcome::Denied,
                );
                event.attributes = sf_core::EventAttributes::Session {
                    public_handle: None,
                    transport: Some("ssh".to_owned()),
                    duration_seconds: None,
                    close_reason: Some("invalid-credentials".to_owned()),
                };
                self.observability.record(&event)?;
                warn!("SSH password authentication rejected");
                Ok(None)
            }
        }
    }

    pub fn callers(&self) -> Result<Vec<Caller>, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .all_callers()
            .map_err(Into::into)
    }

    pub fn public_information_policy(
        &self,
    ) -> Result<sf_core::PublicDirectoryPolicy, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .public_directory_policy()
            .map_err(Into::into)
    }

    pub fn update_public_information_policy(
        &self,
        expected_version: u64,
        enabled: bool,
        show_last_call: bool,
        show_location: bool,
        caller_additions: bool,
    ) -> Result<sf_core::PublicDirectoryPolicy, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        database
            .update_public_directory_policy(
                sf_core::PublicInformationActor::LocalOperator,
                expected_version,
                enabled,
                show_last_call,
                show_location,
                caller_additions,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn other_bbs_entries(&self) -> Result<Vec<sf_core::OtherBbsEntry>, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .other_bbs_entries(true)
            .map_err(Into::into)
    }

    pub fn add_other_bbs(
        &self,
        entry: sf_core::NewOtherBbsEntry,
    ) -> Result<sf_core::OtherBbsEntry, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        database
            .add_other_bbs(
                sf_core::PublicInformationActor::LocalOperator,
                entry,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn edit_other_bbs(
        &self,
        id: sf_core::OtherBbsId,
        expected_version: u64,
        entry: sf_core::NewOtherBbsEntry,
    ) -> Result<sf_core::OtherBbsEntry, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        database
            .edit_other_bbs(
                sf_core::PublicInformationActor::LocalOperator,
                id,
                expected_version,
                entry,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn reorder_other_bbs(
        &self,
        id: sf_core::OtherBbsId,
        expected_version: u64,
        order: usize,
    ) -> Result<sf_core::OtherBbsEntry, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        database
            .reorder_other_bbs(
                sf_core::PublicInformationActor::LocalOperator,
                id,
                expected_version,
                order,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn set_other_bbs_lifecycle(
        &self,
        id: sf_core::OtherBbsId,
        expected_version: u64,
        lifecycle: sf_core::OtherBbsLifecycle,
    ) -> Result<sf_core::OtherBbsEntry, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        database
            .set_other_bbs_lifecycle(
                sf_core::PublicInformationActor::LocalOperator,
                id,
                expected_version,
                lifecycle,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn caller(&self, name: &[u8]) -> Result<Caller, ApplicationError> {
        RuntimeDatabase::open(self.paths.database())?
            .caller_by_name(name)?
            .ok_or(ApplicationError::InvalidSetupValue("unknown caller"))
    }

    pub fn set_caller_profile(
        &self,
        name: &[u8],
        profile: sf_core::CallerProfile,
    ) -> Result<Caller, ApplicationError> {
        let database = RuntimeDatabase::open(self.paths.database())?;
        let caller = database
            .caller_by_name(name)?
            .ok_or(ApplicationError::InvalidSetupValue("unknown caller"))?;
        database
            .update_caller_profile(caller.id, profile, &self.caller_config.profile)
            .map_err(Into::into)
    }

    pub fn set_caller_identity(
        &self,
        name: &[u8],
        login_identifier: &[u8],
        display_handle: &[u8],
        real_name: Option<String>,
    ) -> Result<Caller, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        let caller = database
            .caller_by_name(name)?
            .ok_or(ApplicationError::InvalidSetupValue("unknown caller"))?;
        database
            .update_caller_identity(
                caller.id,
                caller.state_version,
                login_identifier,
                display_handle,
                real_name,
                &self.caller_config,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn set_caller_state(
        &self,
        name: &[u8],
        state: CallerState,
    ) -> Result<Caller, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        let caller = database
            .caller_by_name(name)?
            .ok_or(ApplicationError::InvalidSetupValue("unknown caller"))?;
        let updated = database.mutate_caller_lifecycle(
            caller.id,
            caller.state_version,
            state,
            CallerAccessActor::LocalOperator,
            &self.caller_config,
            current_unix_seconds()?,
        )?;
        if matches!(state, CallerState::Disabled | CallerState::Deleted) {
            self.invalidate_caller_sessions(caller.id)?;
        }
        Ok(updated)
    }

    pub fn set_caller_security(
        &self,
        name: &[u8],
        security: SecurityLevel,
    ) -> Result<Caller, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        let caller = database
            .caller_by_name(name)?
            .ok_or(ApplicationError::InvalidSetupValue("unknown caller"))?;
        database
            .change_caller_base_security(
                caller.id,
                caller.state_version,
                security,
                CallerAccessActor::LocalOperator,
                &self.caller_config,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn set_caller_purge_protection(
        &self,
        name: &[u8],
        protected: bool,
    ) -> Result<Caller, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        let caller = database
            .caller_by_name(name)?
            .ok_or(ApplicationError::InvalidSetupValue("unknown caller"))?;
        database
            .set_caller_purge_protection(
                caller.id,
                caller.state_version,
                protected,
                CallerAccessActor::LocalOperator,
                &self.caller_config,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn update_caller_subscription(
        &self,
        name: &[u8],
        expires_on: Option<chrono::NaiveDate>,
    ) -> Result<Caller, ApplicationError> {
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        let caller = database
            .caller_by_name(name)?
            .ok_or(ApplicationError::InvalidSetupValue("unknown caller"))?;
        database
            .update_caller_subscription(
                caller.id,
                caller.state_version,
                expires_on,
                CallerAccessActor::LocalOperator,
                &self.caller_config,
                current_unix_seconds()?,
                self.timezone,
            )
            .map_err(Into::into)
    }

    fn invalidate_caller_sessions(
        &self,
        caller_id: sf_core::CallerId,
    ) -> Result<(), ApplicationError> {
        for node in self.nodes.snapshots()? {
            if node.caller_id == Some(caller_id) {
                if let Some(session) = node.session_id {
                    self.interaction.request_disconnect(session)?;
                }
            }
        }
        Ok(())
    }

    pub fn is_synthetic_fixture(&self) -> bool {
        self.identity.name() == "SPITFIRE NG Fixture Board"
            && self.identity.sysop_name() == "Fixture Sysop"
    }

    pub fn caller_exists(&self, name: &[u8]) -> Result<bool, ApplicationError> {
        let database = RuntimeDatabase::open(self.paths.database())?;
        Ok(database.caller_by_name(name)?.is_some())
    }

    pub fn initialize_sysop(&self, password: &[u8]) -> Result<Caller, ApplicationError> {
        if password.len() < self.caller_config.minimum_password_length
            || password.len() > self.caller_config.maximum_password_length
        {
            return Err(ApplicationError::InvalidSysopPasswordLength {
                minimum: self.caller_config.minimum_password_length,
                maximum: self.caller_config.maximum_password_length,
            });
        }
        let encoded = self.credential_hasher.hash(password)?;
        let mut database = RuntimeDatabase::open(self.paths.database())?;
        database
            .create_caller(
                self.caller_config.sysop_caller_name.as_bytes(),
                &encoded,
                SecurityLevel::new(self.caller_config.sysop_security)
                    .map_err(sf_core::DatabaseError::from)?,
                CallerState::Active,
                false,
                current_unix_seconds()?,
            )
            .map_err(Into::into)
    }

    pub fn run_connection(
        &self,
        terminal: &mut dyn Terminal,
    ) -> Result<ConnectionReport, ApplicationError> {
        let terminal_info = terminal.info();
        let resources = load_stock_resources(&self.paths, &terminal_info, &self.presentation)?;
        let mut text_info = terminal_info.clone();
        text_info.capabilities.ansi = false;
        let text_resources = load_stock_resources(&self.paths, &text_info, &self.presentation)?;
        let session_id =
            self.next_session
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    current.checked_add(1)
                });
        let session_id = match session_id {
            Ok(value) => SessionId::new(value)?,
            Err(_) => {
                return Err(ApplicationError::Coordination(
                    "session identifier exhausted",
                ))
            }
        };
        // Open the per-connection database before claiming a node so an
        // operational database failure cannot strand that node as busy.
        let mut database = RuntimeDatabase::open(self.paths.database())?;

        let connected_at = terminal_info
            .connected_at
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_secs()).ok())
            .unwrap_or(current_unix_seconds()?);
        let lease = match self
            .nodes
            .acquire(session_id, terminal_info.transport, connected_at)
        {
            Ok(lease) => lease,
            Err(NodeError::AllNodesBusy) => {
                terminal.write_all(NODE_BUSY_MESSAGE)?;
                terminal.disconnect()?;
                info!(transport = ?terminal_info.transport, "connection declined because all configured nodes are busy");
                return Ok(ConnectionReport::NodeBusy);
            }
            Err(error) => return Err(error.into()),
        };
        let mut session = lease.start_session();

        info!(
            transport = ?terminal_info.transport,
            node = session.node_id().get(),
            session = session.id().get(),
            "SPITFIRE session started"
        );
        let outcome = sf_core::with_localizer(self.language.localizer(), || {
            let presentation_profile = self
                .presentation
                .status()
                .configured_active
                .as_deref()
                .unwrap_or("legacy-resources");
            let menu_mode = match self.presentation.menu_mode() {
                sf_core::MenuPresentationMode::DisplayOverrides => "display-overrides",
                sf_core::MenuPresentationMode::Generated => "generated",
            };
            run_stock_session(
                &mut session,
                terminal,
                &mut database,
                &self.caller_config,
                &self.credential_hasher,
                StockSessionContext {
                    board: &self.identity,
                    timezone: self.timezone,
                    board_access: self.board_access,
                    private_security_level: self.private_security_level,
                    resources: &resources,
                    text_resources: &text_resources,
                    status: &lease,
                    file_storage: &self.file_storage,
                    interaction: &self.interaction,
                    page_timeout: Duration::from_secs(30),
                    chat_timeout: Duration::from_secs(300),
                    presentation_profile,
                    menu_mode,
                    locale: self.language.status().effective_locale.as_str(),
                    joker_policy: &self.joker_policy,
                },
            )
        });
        if let Err(error) = self.interaction.session_ended(session.id()) {
            warn!(error = %error, session = session.id().get(), "could not clear page/chat state");
        }
        if outcome.is_err() && session.state() == SessionState::Active {
            let _ = session.close(SessionCloseReason::TransportLost);
        }
        let accounting_result = session.accounting(self.timezone)?.map_or(
            Ok(()),
            |(caller, elapsed, daily_elapsed, day)| {
                database.finish_caller_session_observed(
                    caller,
                    elapsed,
                    daily_elapsed,
                    day,
                    session.close_reason().map(|reason| {
                        (
                            session.node_id().get(),
                            session.id().get(),
                            terminal_info.transport.as_str(),
                            reason.as_str(),
                        )
                    }),
                    current_unix_seconds().unwrap_or(connected_at),
                )
            },
        );
        lease.mark_disconnecting()?;
        let completed_node = session.node_id();
        lease.release(&session)?;
        let node_idle_at_shutdown = self
            .nodes
            .snapshots()?
            .iter()
            .any(|node| node.id == completed_node && node.state == NodeRuntimeState::Waiting);
        accounting_result?;
        let outcome = outcome?;
        info!(
            transport = ?terminal_info.transport,
            node = outcome.node_id.get(),
            session = outcome.session_id.get(),
            close_reason = ?outcome.close_reason,
            "SPITFIRE session ended"
        );

        Ok(ConnectionReport::Completed(RunReport {
            board_name: self.identity.name().to_owned(),
            node_id: outcome.node_id.get(),
            database_path: self.paths.database().to_path_buf(),
            schema_version: self.schema_version,
            session_id: outcome.session_id.get(),
            commands_processed: outcome.commands_processed,
            close_reason: outcome.close_reason,
            transport: terminal_info.transport,
            caller_id: outcome.caller_id.map(|id| id.get()),
            caller_name: outcome.caller_name,
            node_idle_at_shutdown,
        }))
    }
}

impl Drop for BoardRuntime {
    fn drop(&mut self) {
        remove_runtime_status(&self.status_path);
    }
}

fn current_unix_seconds() -> Result<i64, ApplicationError> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApplicationError::Coordination("system clock is before the Unix epoch"))?
        .as_secs();
    i64::try_from(seconds)
        .map_err(|_| ApplicationError::Coordination("system clock value is too large"))
}

fn require(allowed: bool) -> Result<(), ApplicationError> {
    if allowed {
        Ok(())
    } else {
        Err(ApplicationError::Coordination(
            "operator observability capability is required",
        ))
    }
}

pub fn run_board(
    config_path: &Path,
    terminal: &mut dyn Terminal,
) -> Result<RunReport, ApplicationError> {
    let runtime = BoardRuntime::load(config_path)?;
    match runtime.run_connection(terminal)? {
        ConnectionReport::Completed(report) => Ok(report),
        ConnectionReport::NodeBusy => Err(ApplicationError::Coordination(
            "newly loaded runtime unexpectedly reported every configured node busy",
        )),
    }
}

pub fn serve_board(
    config_path: &Path,
    maximum_sessions: Option<usize>,
) -> Result<ServeReport, ApplicationError> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let signal_shutdown = Arc::clone(&shutdown);
    ctrlc::set_handler(move || signal_shutdown.store(true, Ordering::SeqCst)).map_err(|error| {
        ApplicationError::Transport(format!("could not install Ctrl-C handler: {error}"))
    })?;
    serve_with_shutdown(config_path, maximum_sessions, shutdown)
}

pub fn serve_board_console(config_path: &Path) -> Result<ServeReport, ApplicationError> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let signal_shutdown = Arc::clone(&shutdown);
    ctrlc::set_handler(move || signal_shutdown.store(true, Ordering::SeqCst)).map_err(|error| {
        ApplicationError::Transport(format!("could not install Ctrl-C handler: {error}"))
    })?;
    serve_with_operator(
        config_path,
        None,
        shutdown,
        Some(Box::new(|runtime, shutdown| {
            let service = OperatorService::new(runtime);
            let mut input = BufReader::new(io::stdin());
            let mut output = io::stdout();
            let result = run_operator_console(&service, &mut input, &mut output);
            shutdown.store(true, Ordering::SeqCst);
            result
        })),
    )
}

pub(crate) fn serve_with_shutdown(
    config_path: &Path,
    maximum_sessions: Option<usize>,
    shutdown: Arc<AtomicBool>,
) -> Result<ServeReport, ApplicationError> {
    serve_with_operator(config_path, maximum_sessions, shutdown, None)
}

type OperatorRunner =
    Box<dyn FnOnce(Arc<BoardRuntime>, Arc<AtomicBool>) -> Result<(), ApplicationError> + Send>;

fn serve_with_operator(
    config_path: &Path,
    maximum_sessions: Option<usize>,
    shutdown: Arc<AtomicBool>,
    operator: Option<OperatorRunner>,
) -> Result<ServeReport, ApplicationError> {
    if maximum_sessions == Some(0) {
        return Err(ApplicationError::Transport(
            "maximum session count must be greater than zero".to_owned(),
        ));
    }
    let runtime = Arc::new(BoardRuntime::load(config_path)?);
    let completed = Arc::new(AtomicUsize::new(0));
    let mut listeners = Vec::new();
    let mut network = Vec::new();
    let mut ssh_network = Vec::new();
    let mut devices = Vec::new();

    for (index, transport) in runtime.transports().iter().enumerate() {
        if !transport.enabled {
            continue;
        }
        let name = transport.effective_name(index);
        match &transport.adapter {
            TransportAdapterConfig::Telnet { listen, terminal } => {
                let listener = bind_listener(*listen)?;
                listeners.push(ListenerReport {
                    name: name.clone(),
                    transport: TransportKind::Telnet,
                    address: listener_address(&listener)?,
                });
                network.push(NetworkListener {
                    name,
                    kind: TransportKind::Telnet,
                    listener,
                    defaults: terminal.clone(),
                    rlogin_auto_login: false,
                });
            }
            TransportAdapterConfig::Raw { listen, terminal } => {
                let listener = bind_listener(*listen)?;
                listeners.push(ListenerReport {
                    name: name.clone(),
                    transport: TransportKind::RawTcp,
                    address: listener_address(&listener)?,
                });
                network.push(NetworkListener {
                    name,
                    kind: TransportKind::RawTcp,
                    listener,
                    defaults: terminal.clone(),
                    rlogin_auto_login: false,
                });
            }
            TransportAdapterConfig::Rlogin {
                listen,
                auto_login,
                terminal,
            } => {
                let listener = bind_listener(*listen)?;
                listeners.push(ListenerReport {
                    name: name.clone(),
                    transport: TransportKind::Rlogin,
                    address: listener_address(&listener)?,
                });
                network.push(NetworkListener {
                    name,
                    kind: TransportKind::Rlogin,
                    listener,
                    defaults: terminal.clone(),
                    rlogin_auto_login: *auto_login,
                });
            }
            TransportAdapterConfig::Serial {
                device,
                baud,
                terminal,
            } => devices.push(DeviceTransport::Serial {
                device: device.clone(),
                baud: *baud,
                terminal: terminal.clone(),
            }),
            TransportAdapterConfig::Modem {
                device,
                baud,
                initialization,
                answer,
                terminal,
            } => devices.push(DeviceTransport::Modem {
                device: device.clone(),
                baud: *baud,
                initialization: initialization.clone(),
                answer: answer.clone(),
                terminal: terminal.clone(),
            }),
            TransportAdapterConfig::Ssh {
                listen,
                host_key,
                terminal,
                maximum_unauthenticated_connections,
                maximum_authentication_attempts,
                handshake_timeout_seconds,
            } => {
                let listener = bind_listener(*listen)?;
                listeners.push(ListenerReport {
                    name: name.clone(),
                    transport: TransportKind::Ssh,
                    address: listener_address(&listener)?,
                });
                let key = load_or_generate_host_key(runtime.system_path(), host_key)?;
                ssh_network.push((
                    listener,
                    key,
                    SshListenerOptions {
                        defaults: terminal.clone(),
                        maximum_unauthenticated_connections: *maximum_unauthenticated_connections,
                        maximum_authentication_attempts: *maximum_authentication_attempts,
                        handshake_timeout: Duration::from_secs(*handshake_timeout_seconds),
                    },
                ));
            }
        }
    }
    if network.is_empty() && ssh_network.is_empty() && devices.is_empty() {
        return Err(ApplicationError::Transport(
            "no listener or device transports are configured".to_owned(),
        ));
    }
    let operator_server = crate::operator_control::start_operator_server(
        Arc::clone(&runtime),
        config_path
            .canonicalize()
            .map_err(|source| ApplicationError::ResolveConfiguration {
                path: config_path.to_path_buf(),
                source,
            })?,
        Arc::clone(&shutdown),
    )?;

    for listener in &listeners {
        info!(name = listener.name, transport = ?listener.transport, listen = %listener.address, "transport listener ready");
    }
    let mut handles = Vec::new();
    for network_listener in network {
        let runtime = Arc::clone(&runtime);
        let shutdown = Arc::clone(&shutdown);
        let completed = Arc::clone(&completed);
        handles.push(thread::spawn(move || {
            listener_loop(
                network_listener,
                runtime,
                completed,
                maximum_sessions,
                shutdown,
            )
        }));
    }
    for (listener, host_key, options) in ssh_network {
        let runtime = Arc::clone(&runtime);
        let shutdown = Arc::clone(&shutdown);
        let completed = Arc::clone(&completed);
        handles.push(thread::spawn(move || {
            serve_ssh_listener(
                listener,
                runtime,
                host_key,
                options,
                completed,
                maximum_sessions,
                shutdown,
            )
        }));
    }
    for device in devices {
        let runtime = Arc::clone(&runtime);
        let shutdown = Arc::clone(&shutdown);
        let completed = Arc::clone(&completed);
        handles.push(thread::spawn(move || {
            if shutdown.load(Ordering::SeqCst) {
                return;
            }
            let result = match device {
                DeviceTransport::Serial {
                    device,
                    baud,
                    terminal,
                } => SerialTerminal::open(&device, baud, &terminal)
                    .map(|mut terminal| runtime.run_connection(&mut terminal)),
                DeviceTransport::Modem {
                    device,
                    baud,
                    initialization,
                    answer,
                    terminal,
                } => ModemTerminal::answer(&device, baud, &initialization, &answer, &terminal)
                    .map(|mut terminal| runtime.run_connection(&mut terminal)),
            };
            match result {
                Ok(Ok(ConnectionReport::Completed(_))) => {
                    record_completion(&completed, maximum_sessions, &shutdown);
                }
                Ok(Ok(ConnectionReport::NodeBusy)) => {}
                Ok(Err(error)) => warn!(error = %error, "device transport ended"),
                Err(error) => warn!(error = %error, "device transport ended"),
            }
        }));
    }

    let operator_result = if let Some(operator) = operator {
        let result = operator(Arc::clone(&runtime), Arc::clone(&shutdown));
        shutdown.store(true, Ordering::SeqCst);
        Some(result)
    } else {
        None
    };

    for handle in handles {
        handle
            .join()
            .map_err(|_| ApplicationError::Coordination("transport listener thread panicked"))?;
    }
    operator_server.join()?;
    if let Some(result) = operator_result {
        result?;
    }
    info!("SPITFIRE NG listeners shut down cleanly");
    Ok(ServeReport {
        listeners,
        completed_sessions: completed.load(Ordering::SeqCst),
    })
}

fn bind_listener(address: SocketAddr) -> Result<TcpListener, ApplicationError> {
    let listener = TcpListener::bind(address).map_err(|error| {
        ApplicationError::Transport(format!("could not bind listener {address}: {error}"))
    })?;
    listener.set_nonblocking(true).map_err(|error| {
        ApplicationError::Transport(format!("could not configure listener {address}: {error}"))
    })?;
    Ok(listener)
}

fn listener_address(listener: &TcpListener) -> Result<SocketAddr, ApplicationError> {
    listener.local_addr().map_err(|error| {
        ApplicationError::Transport(format!("could not inspect listener: {error}"))
    })
}

fn listener_loop(
    network: NetworkListener,
    runtime: Arc<BoardRuntime>,
    completed: Arc<AtomicUsize>,
    maximum_sessions: Option<usize>,
    shutdown: Arc<AtomicBool>,
) {
    let NetworkListener {
        name,
        kind,
        listener,
        defaults,
        rlogin_auto_login,
    } = network;
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, remote)) => {
                let runtime = Arc::clone(&runtime);
                let completed = Arc::clone(&completed);
                let shutdown = Arc::clone(&shutdown);
                let defaults = defaults.clone();
                let listener_name = name.clone();
                thread::spawn(move || {
                    let result = match kind {
                        TransportKind::Telnet => TelnetTerminal::accept(stream, remote, &defaults)
                            .map(|mut terminal| runtime.run_connection(&mut terminal)),
                        TransportKind::RawTcp => RawTcpTerminal::new(stream, remote, &defaults)
                            .map(|mut terminal| runtime.run_connection(&mut terminal)),
                        TransportKind::Rlogin => {
                            RloginTerminal::accept(stream, remote, &defaults, rlogin_auto_login)
                                .map(|mut terminal| runtime.run_connection(&mut terminal))
                        }
                        _ => return,
                    };
                    match result {
                        Ok(Ok(ConnectionReport::Completed(_))) => {
                            record_completion(&completed, maximum_sessions, &shutdown);
                        }
                        Ok(Ok(ConnectionReport::NodeBusy)) => {}
                        Ok(Err(error)) => {
                            warn!(listener = listener_name, transport = ?kind, error = %error, "connection ended with an error");
                        }
                        Err(error) => {
                            warn!(listener = listener_name, transport = ?kind, error = %error, "connection ended with an error");
                        }
                    }
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                warn!(transport = ?kind, error = %error, "listener accept failed");
                shutdown.store(true, Ordering::SeqCst);
            }
        }
    }
}

pub(crate) fn record_completion(
    completed: &AtomicUsize,
    maximum_sessions: Option<usize>,
    shutdown: &AtomicBool,
) {
    let count = completed.fetch_add(1, Ordering::SeqCst) + 1;
    if maximum_sessions.is_some_and(|maximum| count >= maximum) {
        shutdown.store(true, Ordering::SeqCst);
    }
}

enum DeviceTransport {
    Serial {
        device: String,
        baud: u32,
        terminal: sf_core::NetworkTerminalDefaults,
    },
    Modem {
        device: String,
        baud: u32,
        initialization: String,
        answer: String,
        terminal: sf_core::NetworkTerminalDefaults,
    },
}

struct NetworkListener {
    name: String,
    kind: TransportKind,
    listener: TcpListener,
    defaults: sf_core::NetworkTerminalDefaults,
    rlogin_auto_login: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use sf_core::{
        CallerState, CredentialHasher, FileActor, FileBackend, InMemoryTerminal, MessageActor,
        MessageBackend, MessageKind, NetworkTerminalDefaults, NodePoolConfig, PasswordHashConfig,
        PostLoginJourney, PresentationMode, SecurityLevel, TerminalCapabilities, TerminalInfo,
        TerminalSize,
    };

    use crate::{initialize_fixture_board, FIXTURE_CONFIG_FILE};

    struct AcceptTestSshServerKey;

    impl russh::client::Handler for AcceptTestSshServerKey {
        type Error = russh::Error;

        async fn check_server_key(
            &mut self,
            _server_public_key: &russh::keys::PublicKeyOrCertificate,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    async fn connect_ssh_test_client(
        address: SocketAddr,
    ) -> russh::client::Handle<AcceptTestSshServerKey> {
        let started = Instant::now();
        loop {
            match russh::client::connect(
                Arc::new(russh::client::Config {
                    inactivity_timeout: Some(Duration::from_secs(10)),
                    ..Default::default()
                }),
                address,
                AcceptTestSshServerKey,
            )
            .await
            {
                Ok(client) => return client,
                Err(error) if started.elapsed() < Duration::from_secs(5) => {
                    let _ = error;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(error) => panic!("could not connect SSH test client: {error}"),
            }
        }
    }

    #[test]
    fn complete_stock_runtime_traverses_menus_and_shuts_down() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        let config = root.join(FIXTURE_CONFIG_FILE);
        let mut terminal = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"Runtime Test Caller".to_vec(),
            b"test-only-runtime-password".to_vec(),
            b"test-only-runtime-password".to_vec(),
            b"M".to_vec(),
            b"F".to_vec(),
            b"Q".to_vec(),
            b"?".to_vec(),
            b"M".to_vec(),
            b"G".to_vec(),
        ]);

        let report = run_board(&config, &mut terminal).unwrap();
        assert_eq!(report.board_name, "SPITFIRE NG Fixture Board");
        assert_eq!(report.node_id, 1);
        assert_eq!(report.schema_version, sf_core::SCHEMA_VERSION);
        assert_eq!(report.close_reason, SessionCloseReason::Goodbye);
        assert_eq!(report.transport, TransportKind::InMemory);
        assert_eq!(report.caller_name.as_deref(), Some("Runtime Test Caller"));
        assert!(report.node_idle_at_shutdown);
        let output = terminal.output();
        assert!(contains(output, b"SPITFIRE NG connection established"));
        assert!(contains(output, b"Welcome, new caller"));
        assert!(contains(output, b"Welcome back, Runtime"));
        assert!(contains(output, b"MAIN MENU"));
        assert!(contains(output, b"Entering the SPITFIRE Message Section"));
        assert!(contains(output, b"MESSAGE MENU"));
        assert!(contains(output, b"Entering the SPITFIRE File Section"));
        assert!(contains(output, b"FILE MENU"));
        assert!(contains(output, b"Moves from Main to the Message Menu."));
        assert!(terminal.disconnected());
    }

    #[test]
    fn ssh_authentication_uses_login_identifier_and_rejects_unavailable_callers() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("ssh-auth-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Available SSH Caller",
            b"test-only available ssh password",
            CallerState::Active,
        );
        seed_caller(
            &root,
            b"Disabled SSH Caller",
            b"test-only disabled ssh password",
            CallerState::Disabled,
        );
        seed_caller(
            &root,
            b"Deleted SSH Caller",
            b"test-only deleted ssh password",
            CallerState::Deleted,
        );
        seed_caller(
            &root,
            b"Joker SSH Caller",
            b"test-only joker ssh password",
            CallerState::Active,
        );
        fs::write(
            root.join("system").join("JOKER.DAT"),
            b"Joker SSH Caller\r\n",
        )
        .unwrap();
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        assert!(runtime
            .authenticate_ssh_password("available-ssh-caller", "test-only available ssh password")
            .unwrap()
            .is_some());
        for (login, password) in [
            ("available-ssh-caller", "wrong password"),
            ("missing-ssh-caller", "test-only missing password"),
            ("disabled-ssh-caller", "test-only disabled ssh password"),
            ("deleted-ssh-caller", "test-only deleted ssh password"),
            ("joker-ssh-caller", "test-only joker ssh password"),
        ] {
            assert!(runtime
                .authenticate_ssh_password(login, password)
                .unwrap()
                .is_none());
        }
        assert!(runtime
            .authenticate_ssh_password(&"x".repeat(33), "irrelevant")
            .unwrap()
            .is_none());
    }

    #[test]
    fn ssh_listener_enters_the_common_bbs_session_and_exposes_no_os_shell() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("ssh-listener-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        let password = "test-only ssh listener password";
        seed_caller(
            &root,
            b"Legacy SSH Public Name",
            password.as_bytes(),
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let runtime = BoardRuntime::load(&config_path).unwrap();
        let identity = runtime
            .set_caller_identity(
                b"Legacy SSH Public Name",
                b"pixelwizard",
                b"PixelWizard",
                Some("SSH Acceptance Real Name".to_owned()),
            )
            .unwrap();
        assert_eq!(identity.login_identifier, "pixelwizard");
        assert_eq!(identity.display_name, "PixelWizard");
        assert_eq!(
            identity.real_name.as_deref(),
            Some("SSH Acceptance Real Name")
        );
        drop(runtime);

        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let mut public_database = RuntimeDatabase::open(paths.database()).unwrap();
        public_database
            .update_public_directory_policy(
                sf_core::PublicInformationActor::LocalOperator,
                1,
                true,
                true,
                false,
                false,
                1_700_000_020,
            )
            .unwrap();
        public_database
            .update_caller_publicity(
                sf_core::PublicInformationActor::Caller(identity.id),
                identity.id,
                identity.publicity_state_version,
                true,
                1_700_000_021,
            )
            .unwrap();
        public_database
            .add_other_bbs(
                sf_core::PublicInformationActor::LocalOperator,
                sf_core::NewOtherBbsEntry {
                    name: "SSH Fixture BBS".to_owned(),
                    speed: "SSH".to_owned(),
                    dial_string: "ssh-fixture.example:2222".to_owned(),
                },
                1_700_000_022,
            )
            .unwrap();
        drop(public_database);

        let address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![listener_config(
            "ssh-acceptance",
            TransportAdapterConfig::Ssh {
                listen: address,
                host_key: PathBuf::from("ssh/host-ed25519"),
                terminal: NetworkTerminalDefaults {
                    cp437: false,
                    ..NetworkTerminalDefaults::default()
                },
                maximum_unauthenticated_connections: 4,
                maximum_authentication_attempts: 3,
                handshake_timeout_seconds: 5,
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let server =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(1), shutdown).unwrap());

        let async_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let transcript = async_runtime.block_on(async {
            let mut rejected = connect_ssh_test_client(address).await;
            assert!(!rejected
                .authenticate_password("pixelwizard", "wrong password")
                .await
                .unwrap()
                .success());
            let _ = rejected
                .disconnect(russh::Disconnect::ByApplication, "", "")
                .await;

            let mut missing = connect_ssh_test_client(address).await;
            assert!(!missing
                .authenticate_password("no-such-login", "irrelevant")
                .await
                .unwrap()
                .success());
            let _ = missing
                .disconnect(russh::Disconnect::ByApplication, "", "")
                .await;

            let mut command_client = connect_ssh_test_client(address).await;
            assert!(command_client
                .authenticate_password("pixelwizard", password)
                .await
                .unwrap()
                .success());
            let mut command_channel = command_client.channel_open_session().await.unwrap();
            command_channel.exec(true, b"id".as_slice()).await.unwrap();
            assert!(matches!(
                tokio::time::timeout(Duration::from_secs(2), command_channel.wait())
                    .await
                    .unwrap(),
                Some(russh::ChannelMsg::Failure)
            ));
            command_client
                .disconnect(russh::Disconnect::ByApplication, "", "")
                .await
                .unwrap();

            let mut client = connect_ssh_test_client(address).await;
            assert!(client
                .authenticate_password("PIXELWIZARD", password)
                .await
                .unwrap()
                .success());
            let mut channel = client.channel_open_session().await.unwrap();
            channel
                .request_pty(true, "xterm-256color", 100, 40, 0, 0, &[])
                .await
                .unwrap();
            channel.request_shell(true).await.unwrap();
            channel.window_change(120, 50, 0, 0).await.unwrap();
            channel
                .data(b"#\rN\rL\rPixel\rY\rO\rA\rB\r1\rN\rT\rM\r".as_slice())
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
            let status = crate::board_status(&config_path).unwrap();
            assert!(
                status.contains("caller=pixelwizard (PixelWizard) lifecycle=active transport=ssh")
            );
            assert!(
                status.contains("terminal=xterm-256color ansi=true encoding=utf-8 size=120x50"),
                "unexpected SSH status after resize:\n{status}"
            );
            channel.data(b"B\rF\rQ\rG\r".as_slice()).await.unwrap();
            let mut transcript = Vec::new();
            loop {
                let message = tokio::time::timeout(Duration::from_secs(10), channel.wait())
                    .await
                    .expect("timed out waiting for SSH BBS transcript");
                match message {
                    Some(russh::ChannelMsg::Data { data }) => transcript.extend_from_slice(&data),
                    Some(russh::ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
            transcript
        });
        let report = server.join().unwrap();
        assert_eq!(report.completed_sessions, 1);
        assert_eq!(report.listeners[0].transport, TransportKind::Ssh);
        assert!(contains(&transcript, b"Welcome back, PixelWizard"));
        assert!(contains(&transcript, b"MAIN MENU"));
        assert!(contains(&transcript, b"MESSAGE MENU"));
        assert!(contains(&transcript, b"FILE MENU"));
        assert!(contains(&transcript, b"SPITFIRE CALLER DIRECTORY"));
        assert!(contains(
            &transcript,
            b"Is PixelWizard the caller you want?"
        ));
        assert!(contains(&transcript, b"SSH Fixture BBS"));
        assert!(contains(
            &transcript,
            b"Caller additions to Other BBS information are disabled."
        ));
        assert!(contains(&transcript, b"SPITFIRE BULLETINS"));
        assert!(contains(&transcript, b"SPITFIRE NG Newsletter"));
        assert!(contains(&transcript, b"SPITFIRE SYSTEM INFORMATION"));
        assert!(contains(&transcript, b"Thank you for calling"));
        assert!(!contains(&transcript, b"Caller name"));
        assert!(!contains(&transcript, password.as_bytes()));
        assert!(!contains(&transcript, b"SSH Acceptance Real Name"));
        assert!(!contains(&transcript, b"pixelwizard"));
        assert!(root.join("system/ssh/host-ed25519").is_file());
    }

    #[test]
    fn classic_profile_uses_the_engine_owned_stock_post_login_journey() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("classic-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Classic Caller",
            b"test-only classic password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.presentation.mode = PresentationMode::Profile;
        config.presentation.active_profile = Some("classic-spitfire".to_owned());
        config.presentation.base_profile = Some("modern-ng".to_owned());
        config.caller.post_login_journey = PostLoginJourney::Stock;
        config.save_atomic(&config_path).unwrap();

        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Classic Caller".to_vec(),
            b"test-only classic password".to_vec(),
            b"N".to_vec(),
            b"G".to_vec(),
        ]);
        let report = run_board(&config_path, &mut terminal).unwrap();
        assert_eq!(report.close_reason, SessionCloseReason::Goodbye);
        let output = terminal.output();
        assert!(contains(
            output,
            b"SPITFIRE NG Bulletin Board System - Version"
        ));
        assert!(contains(output, b"SPITFIRE MESSAGE SUMMARY"));
        assert!(contains(output, b"Messages Waiting: 0"));
        assert!(contains(output, b"YOUR SPITFIRE STATISTICS"));
        assert!(contains(output, b"Times On: 1"));
        assert!(contains(output, b"SPITFIRE NEW-FILE CHECK"));
        assert!(contains(output, b"New Files Since Last Check: 2"));
        assert!(contains(output, b"List new files now? (Y/N):"));
        assert!(contains(output, b"SPITFIRE MAIN MENU"));
        assert!(contains(output, b"It is presently"));
        assert!(contains(output, b"Caller #"));
        assert!(contains(output, b"call minute(s) left"));
        assert!(contains(output, b"THANK YOU FOR CALLING"));
        assert!(output.contains(&0x1b));
        assert!(output.contains(&0xc9));
    }

    #[test]
    fn failed_login_context_is_caller_owned_presented_once_and_board_local() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("caller-context-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        let password = b"test-only context password";
        seed_caller(&root, b"Context Caller", password, CallerState::Active);
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.board.timezone = "America/Phoenix".to_owned();
        config.save_atomic(&config_path).unwrap();

        let mut failed_lines = Vec::new();
        for _ in 0..config.caller.maximum_login_attempts {
            failed_lines.push(b"Context Caller".to_vec());
            failed_lines.push(b"wrong test-only password".to_vec());
        }
        failed_lines.insert(0, b"N".to_vec());
        let mut failed = InMemoryTerminal::with_lines(failed_lines);
        let failed_report = run_board(&config_path, &mut failed).unwrap();
        assert_eq!(
            failed_report.close_reason,
            SessionCloseReason::AuthenticationFailed
        );

        let mut successful = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Context Caller".to_vec(),
            password.to_vec(),
            b"G".to_vec(),
        ]);
        run_board(&config_path, &mut successful).unwrap();
        let output = successful.output();
        assert!(contains(
            output,
            b"Security notice: an earlier access attempt"
        ));
        assert!(contains(output, b"invalid credentials"));
        assert!(contains(output, b"MST (board local)"));
        assert!(contains(output, b"Caller #"));
        assert!(contains(output, b"daily minute(s) left"));
        assert!(!contains(output, password));
        assert!(!contains(output, b"wrong test-only password"));

        let mut reconnect = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Context Caller".to_vec(),
            password.to_vec(),
            b"G".to_vec(),
        ]);
        run_board(&config_path, &mut reconnect).unwrap();
        assert!(!contains(
            reconnect.output(),
            b"Security notice: an earlier access attempt"
        ));
        assert!(contains(reconnect.output(), b"call 2 today"));
    }

    #[test]
    fn exact_security_art_and_generated_fallback_never_grant_sysop_authority() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("security-menu-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        for (name, password, security) in [
            (
                b"Exact One Hundred".as_slice(),
                b"test-only exact 100".as_slice(),
                100,
            ),
            (
                b"Arbitrary Seven Seven Seven".as_slice(),
                b"test-only arbitrary 777".as_slice(),
                777,
            ),
            (
                b"Exact Nine Nine Nine".as_slice(),
                b"test-only exact 999".as_slice(),
                999,
            ),
        ] {
            seed_caller(&root, name, password, CallerState::Active);
            let config = RuntimeConfig::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
            let validated = config.validate().unwrap();
            let paths = LogicalPaths::resolve(&root, &validated).unwrap();
            let mut database = RuntimeDatabase::open(paths.database()).unwrap();
            let caller = database.caller_by_name(name).unwrap().unwrap();
            database
                .change_caller_base_security(
                    caller.id,
                    caller.state_version,
                    SecurityLevel::new(security).unwrap(),
                    sf_core::CallerAccessActor::LocalOperator,
                    &validated.caller,
                    1_700_000_000,
                )
                .unwrap();
        }
        fs::write(
            root.join("display/MAIN100.CLR"),
            b"\x1b[36mEXACT MAIN 100 <@>\x1b[0m\r\n",
        )
        .unwrap();
        fs::write(
            root.join("display/SOP100.CLR"),
            b"\x1b[36mEXACT SOP 100\x1b[0m\r\n",
        )
        .unwrap();
        fs::write(
            root.join("display/MAIN999.CLR"),
            b"\x1b[36mEXACT MAIN 999 <@>\x1b[0m\r\n",
        )
        .unwrap();
        fs::write(
            root.join("display/SOP999.CLR"),
            b"\x1b[36mEXACT SOP 999\x1b[0m\r\n",
        )
        .unwrap();
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.caller.sysop_security = 500;
        config.save_atomic(&config_path).unwrap();

        let mut exact_100 = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Exact One Hundred".to_vec(),
            b"test-only exact 100".to_vec(),
            b"@".to_vec(),
            b"G".to_vec(),
        ]);
        run_board(&config_path, &mut exact_100).unwrap();
        assert!(contains(exact_100.output(), b"EXACT MAIN 100"));
        assert!(contains(
            exact_100.output(),
            b"Sysop Utilities require the configured Sysop security threshold."
        ));
        assert!(!contains(exact_100.output(), b"EXACT SOP 100"));

        let mut arbitrary = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Arbitrary Seven Seven Seven".to_vec(),
            b"test-only arbitrary 777".to_vec(),
            b"@".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        run_board(&config_path, &mut arbitrary).unwrap();
        assert!(contains(arbitrary.output(), b">>>>>>>>>> MAIN MENU"));
        assert!(contains(arbitrary.output(), b">>>>>>>>> SYSOP MENU"));
        assert!(contains(arbitrary.output(), b"Security: 777"));

        let mut exact_999 = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Exact Nine Nine Nine".to_vec(),
            b"test-only exact 999".to_vec(),
            b"@".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        run_board(&config_path, &mut exact_999).unwrap();
        assert!(contains(exact_999.output(), b"EXACT MAIN 999"));
        assert!(contains(exact_999.output(), b"EXACT SOP 999"));
    }

    #[test]
    fn generated_mode_uses_one_geometry_while_override_mode_reports_distinct_sources() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("renderer-source-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        for (name, password, security) in [
            (
                b"Normal Geometry".as_slice(),
                b"test-only normal geometry".as_slice(),
                10,
            ),
            (
                b"Sysop Geometry".as_slice(),
                b"test-only sysop geometry".as_slice(),
                50,
            ),
        ] {
            seed_caller(&root, name, password, CallerState::Active);
            let config = RuntimeConfig::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
            let validated = config.validate().unwrap();
            let paths = LogicalPaths::resolve(&root, &validated).unwrap();
            let mut database = RuntimeDatabase::open(paths.database()).unwrap();
            let caller = database.caller_by_name(name).unwrap().unwrap();
            database
                .change_caller_base_security(
                    caller.id,
                    caller.state_version,
                    SecurityLevel::new(security).unwrap(),
                    sf_core::CallerAccessActor::LocalOperator,
                    &validated.caller,
                    1_700_000_000,
                )
                .unwrap();
        }
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let session_info = || {
            let mut info = TerminalInfo::in_memory();
            info.capabilities.terminal_type = Some("ANSI".to_owned());
            info.capabilities.ansi = true;
            info.capabilities.cp437 = true;
            info.capabilities.size = Some(TerminalSize {
                width: 80,
                height: 25,
            });
            info
        };
        let call = |name: &[u8], password: &[u8]| {
            let mut terminal = InMemoryTerminal::with_info(
                [
                    b"N".to_vec(),
                    name.to_vec(),
                    password.to_vec(),
                    b"G".to_vec(),
                ],
                session_info(),
            );
            run_board(&config_path, &mut terminal).unwrap();
            terminal.output().to_vec()
        };

        // The packaged Modern MAIN10 override is intentionally distinguishable
        // from the generated fallback used when exact MAIN50 art is absent.
        let override_normal = call(b"Normal Geometry", b"test-only normal geometry");
        let override_sysop = call(b"Sysop Geometry", b"test-only sysop geometry");
        assert!(contains(&override_normal, b">>>>>>>> MAIN MENU <<<<<<<<"));
        assert!(contains(
            &override_normal,
            b"<M> Messages <F> Files <C> Comment <P> Page"
        ));
        assert!(contains(&override_sysop, b">>>>>>>>>> MAIN MENU <<<<<<<<<"));
        assert!(contains(&override_sysop, b"<M>........... Message Section"));

        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.presentation.menu_mode = sf_core::MenuPresentationMode::Generated;
        config.save_atomic(&config_path).unwrap();
        let generated_normal = call(b"Normal Geometry", b"test-only normal geometry");
        let generated_sysop = call(b"Sysop Geometry", b"test-only sysop geometry");
        for output in [&generated_normal, &generated_sysop] {
            assert!(contains(output, b">>>>>>>>>> MAIN MENU <<<<<<<<<"));
            assert!(contains(output, b"<M>........... Message Section"));
            assert!(!contains(output, b"<M> Messages  <F> Files"));
        }
        assert!(!contains(
            &generated_normal,
            b"<@>........... Sysop Utilities"
        ));
        assert!(contains(
            &generated_sysop,
            b"<@>........... Sysop Utilities"
        ));
    }

    #[test]
    fn stock_post_login_new_file_yes_path_uses_and_updates_live_checkpoint() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("classic-new-files-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Classic Files Caller",
            b"test-only classic files password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.caller.post_login_journey = PostLoginJourney::Stock;
        config.save_atomic(&config_path).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database
            .caller_by_name(b"Classic Files Caller")
            .unwrap()
            .unwrap();
        let actor = FileActor::new(
            caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        assert_eq!(database.new_file_checkpoint(actor).unwrap(), None);
        drop(database);

        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Classic Files Caller".to_vec(),
            b"test-only classic files password".to_vec(),
            b"Y".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"G".to_vec(),
        ]);
        run_board(&config_path, &mut terminal).unwrap();
        assert!(contains(terminal.output(), b"WELCOME.TXT"));
        assert!(contains(terminal.output(), b"SFNGINFO.TXT"));
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        assert!(database.new_file_checkpoint(actor).unwrap().is_some());
    }

    #[test]
    fn customized_command_letter_preserves_stock_identifier_action_and_help() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Mapped Caller",
            b"test-only mapped password",
            CallerState::Active,
        );
        let menu_path = root.join("system/SFMAIN.MNU");
        let menu = fs::read_to_string(&menu_path).unwrap().replacen(
            "M,<M>.......... Message Section,,5,E",
            "Z,<Z>.......... Message Section,,5,E",
            1,
        );
        fs::write(&menu_path, menu).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Mapped Caller".to_vec(),
            b"test-only mapped password".to_vec(),
            b"?".to_vec(),
            b"Z".to_vec(),
            b"Z".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        run_board(&root.join(FIXTURE_CONFIG_FILE), &mut terminal).unwrap();
        assert!(contains(
            terminal.output(),
            b"Moves from Main to the Message Menu."
        ));
        assert!(contains(terminal.output(), b"MESSAGE MENU"));
    }

    #[test]
    fn caller_page_chat_preferences_and_operator_admin_share_the_live_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Interaction Caller",
            b"test-only interaction password",
            CallerState::Active,
        );
        let runtime = Arc::new(BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap());
        let service = crate::OperatorService::new(Arc::clone(&runtime));
        let connection = Arc::clone(&runtime);
        let caller = thread::spawn(move || {
            let mut terminal = InMemoryTerminal::with_lines([
                b"N".to_vec(),
                b"Interaction Caller".to_vec(),
                b"test-only interaction password".to_vec(),
                b"P".to_vec(),
                b"Hello from caller".to_vec(),
                b"/Q".to_vec(),
                b"U".to_vec(),
                b"M".to_vec(),
                b"U".to_vec(),
                b"G".to_vec(),
                b"T".to_vec(),
                b"U".to_vec(),
                b"W".to_vec(),
                b"132".to_vec(),
                b"U".to_vec(),
                b"L".to_vec(),
                b"20".to_vec(),
                b"U".to_vec(),
                b"H".to_vec(),
                b"G".to_vec(),
            ]);
            let report = connection.run_connection(&mut terminal).unwrap();
            (report, terminal)
        });

        let started = Instant::now();
        let page = loop {
            let pages = service.pages().unwrap();
            if let Some(page) = pages.into_iter().next() {
                break page;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(5));
        };
        let state_started = Instant::now();
        while runtime.node_snapshots().unwrap()[0].state != NodeRuntimeState::PagePending {
            assert!(state_started.elapsed() < Duration::from_secs(1));
            thread::sleep(Duration::from_millis(2));
        }
        let operator = service.answer(page.session_id).unwrap();
        assert_eq!(
            operator
                .receive_line(Duration::from_secs(2))
                .unwrap()
                .as_deref(),
            Some("Hello from caller")
        );
        operator.send_line("Hello from the Sysop").unwrap();
        operator.end();

        let (report, terminal) = caller.join().unwrap();
        assert!(matches!(report, ConnectionReport::Completed(_)));
        assert!(contains(terminal.output(), b"Interactive chat is active"));
        assert!(contains(terminal.output(), b"Sysop> Hello from the Sysop"));
        assert!(contains(terminal.output(), b"Terminal preferences saved"));
        assert_eq!(
            runtime.node_snapshots().unwrap()[0].state,
            NodeRuntimeState::Waiting
        );

        let stored = runtime
            .callers()
            .unwrap()
            .into_iter()
            .find(|caller| caller.display_name == "Interaction Caller")
            .unwrap();
        assert_eq!(
            stored.preferences.graphics,
            sf_core::GraphicsPreference::Text
        );
        assert_eq!(stored.preferences.screen_width, Some(132));
        assert_eq!(stored.preferences.page_length, Some(20));
        assert!(!stored.preferences.more_prompt);
        assert!(stored.preferences.hot_keys);

        let changed = service
            .set_caller_security("Interaction Caller", 25)
            .unwrap();
        assert_eq!(changed.security_level.get(), 25);
        let disabled = service
            .set_caller_state("Interaction Caller", CallerState::Disabled)
            .unwrap();
        assert_eq!(disabled.state, CallerState::Disabled);
        let enabled = service
            .set_caller_state("Interaction Caller", CallerState::Active)
            .unwrap();
        assert_eq!(enabled.state, CallerState::Active);

        let mut reconnect = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Interaction Caller".to_vec(),
            b"test-only interaction password".to_vec(),
            b"Y".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut reconnect).unwrap();
        assert!(contains(reconnect.output(), b"Terminal: Text"));
        assert!(!contains(reconnect.output(), b"\x1B[1;33m"));
    }

    #[test]
    fn disabling_an_active_caller_requests_disconnect_and_enable_does_not_restore_session() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("active-lifecycle-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Active Lifecycle Caller",
            b"test-only active lifecycle password",
            CallerState::Active,
        );
        let runtime = Arc::new(BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap());
        let service = crate::OperatorService::new(Arc::clone(&runtime));
        service
            .set_availability(sf_core::SysopAvailability::Available)
            .unwrap();
        let connection = Arc::clone(&runtime);
        let caller = thread::spawn(move || {
            let mut terminal = InMemoryTerminal::with_lines([
                b"N".to_vec(),
                b"Active Lifecycle Caller".to_vec(),
                b"test-only active lifecycle password".to_vec(),
                b"P".to_vec(),
                b"Lifecycle test".to_vec(),
                b"G".to_vec(),
            ]);
            let report = connection.run_connection(&mut terminal).unwrap();
            (report, terminal)
        });
        let started = Instant::now();
        let page = loop {
            if let Some(page) = service.pages().unwrap().into_iter().next() {
                break page;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(5));
        };
        let disabled = service
            .set_caller_state("Active Lifecycle Caller", CallerState::Disabled)
            .unwrap();
        assert_eq!(disabled.state, CallerState::Disabled);
        service.decline(page.session_id).unwrap();
        let (report, terminal) = caller.join().unwrap();
        let ConnectionReport::Completed(report) = report else {
            panic!("all nodes busy");
        };
        assert_eq!(report.close_reason, SessionCloseReason::OperatorDisconnect);
        assert!(contains(terminal.output(), b"Sysop has disconnected"));
        let enabled = service
            .set_caller_state("Active Lifecycle Caller", CallerState::Active)
            .unwrap();
        assert_eq!(enabled.state, CallerState::Active);
        assert_eq!(
            runtime.node_snapshots().unwrap()[0].state,
            NodeRuntimeState::Waiting
        );
    }

    #[test]
    fn clean_setup_board_runs_one_persistent_caller_message_file_and_chat_journey() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("clean-board");
        let mut plan = crate::SetupPlan::stock_defaults(
            "Clean Acceptance Board",
            "Acceptance Sysop",
            "Sysop",
            4,
        );
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        crate::setup_board(&root, &plan, b"test-only clean sysop password").unwrap();
        let runtime = Arc::new(BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap());

        let mut sysop = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Sysop".to_vec(),
            b"test-only clean sysop password".to_vec(),
            b"M".to_vec(),
            b"E".to_vec(),
            b"".to_vec(),
            b"Clean board welcome".to_vec(),
            b"A message created through the normal board session".to_vec(),
            b"/S".to_vec(),
            b"Y".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut sysop).unwrap();
        assert!(contains(sysop.output(), b"Message 1 was saved"));

        let service = crate::OperatorService::new(Arc::clone(&runtime));
        let connection = Arc::clone(&runtime);
        let caller = thread::spawn(move || {
            let password = b"test-only clean caller password";
            let mut terminal = InMemoryTerminal::with_lines([
                b"Y".to_vec(),
                b"Clean Caller".to_vec(),
                password.to_vec(),
                password.to_vec(),
                b"U".to_vec(),
                b"M".to_vec(),
                b"M".to_vec(),
                b"R".to_vec(),
                b"T".to_vec(),
                b"R".to_vec(),
                b"N".to_vec(),
                b"".to_vec(),
                b"".to_vec(),
                b"Clean caller reply".to_vec(),
                b"/S".to_vec(),
                b"Y".to_vec(),
                b"Q".to_vec(),
                b"Q".to_vec(),
                b"C".to_vec(),
                b"Clean board comment".to_vec(),
                b"A persistent comment to the Sysop".to_vec(),
                b"/S".to_vec(),
                b"Y".to_vec(),
                b"F".to_vec(),
                b"L".to_vec(),
                Vec::new(),
                b"D".to_vec(),
                b"WELCOME.TXT".to_vec(),
                b"1".to_vec(),
                b"U".to_vec(),
                b"CLEAN.TXT".to_vec(),
                b"Clean-board acceptance upload".to_vec(),
                b"1".to_vec(),
                b"persistent clean-board upload".to_vec(),
                b"/S".to_vec(),
                b"Q".to_vec(),
                b"P".to_vec(),
                b"Clean caller checking in".to_vec(),
                b"/Q".to_vec(),
                b"G".to_vec(),
            ]);
            let report = connection.run_connection(&mut terminal).unwrap();
            (report, terminal)
        });
        let started = Instant::now();
        let page = loop {
            if let Some(page) = service.pages().unwrap().into_iter().next() {
                break page;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(5));
        };
        let operator = service.answer(page.session_id).unwrap();
        assert_eq!(
            operator
                .receive_line(Duration::from_secs(2))
                .unwrap()
                .as_deref(),
            Some("Clean caller checking in")
        );
        operator.send_line("Clean board is online").unwrap();
        operator.end();
        let (report, terminal) = caller.join().unwrap();
        assert!(matches!(report, ConnectionReport::Completed(_)));
        assert!(contains(terminal.output(), b"Clean board welcome"));
        assert!(contains(terminal.output(), b"Message 2 was saved"));
        assert!(contains(terminal.output(), b"ASCII download complete"));
        assert!(contains(terminal.output(), b"Upload complete: CLEAN.TXT"));
        assert!(contains(terminal.output(), b"Sysop> Clean board is online"));

        let mut reconnect = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Clean Caller".to_vec(),
            b"test-only clean caller password".to_vec(),
            b"M".to_vec(),
            b"B".to_vec(),
            b"Q".to_vec(),
            b"F".to_vec(),
            b"F".to_vec(),
            b"CLEAN".to_vec(),
            Vec::new(),
            b"Q".to_vec(),
            b"Y".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut reconnect).unwrap();
        assert!(
            contains(reconnect.output(), b"Clean board welcome"),
            "{}",
            String::from_utf8_lossy(reconnect.output())
        );
        assert!(contains(reconnect.output(), b"CLEAN.TXT"));
        assert!(contains(reconnect.output(), b"Files Uploaded: 1"));
        assert!(runtime
            .node_snapshots()
            .unwrap()
            .iter()
            .all(|node| node.state == NodeRuntimeState::Waiting));
    }

    #[test]
    fn clean_minimal_profile_board_accepts_registration_messages_files_and_transfers() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("minimal-board");
        let mut plan = crate::SetupPlan::stock_defaults(
            "Minimal Acceptance Board",
            "Acceptance Sysop",
            "Sysop",
            2,
        );
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        plan.config.presentation = sf_core::PresentationConfig {
            mode: sf_core::PresentationMode::Profile,
            menu_mode: sf_core::MenuPresentationMode::DisplayOverrides,
            active_profile: Some(crate::MINIMAL_PROFILE_ID.to_owned()),
            base_profile: Some(crate::MODERN_PROFILE_ID.to_owned()),
        };
        crate::setup_board(&root, &plan, b"test-only minimal sysop password").unwrap();
        let runtime = BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap();
        let password = b"test-only minimal caller password";
        let mut terminal = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"Minimal Caller".to_vec(),
            password.to_vec(),
            password.to_vec(),
            b"?".to_vec(),
            b"M".to_vec(),
            b"A".to_vec(),
            b"".to_vec(),
            b"M".to_vec(),
            b"E".to_vec(),
            b"".to_vec(),
            b"Minimal profile message".to_vec(),
            b"Message storage remains engine-owned".to_vec(),
            b"/S".to_vec(),
            b"Y".to_vec(),
            b"Y".to_vec(),
            b"S".to_vec(),
            b"".to_vec(),
            b"Q".to_vec(),
            b"B".to_vec(),
            b"Q".to_vec(),
            b"F".to_vec(),
            b"L".to_vec(),
            Vec::new(),
            b"D".to_vec(),
            b"WELCOME.TXT".to_vec(),
            b"1".to_vec(),
            b"U".to_vec(),
            b"MINIMAL.TXT".to_vec(),
            b"Minimal profile acceptance upload".to_vec(),
            b"1".to_vec(),
            b"plain terminal upload".to_vec(),
            b"/S".to_vec(),
            b"Q".to_vec(),
            b"Y".to_vec(),
            b"@".to_vec(),
            b"G".to_vec(),
        ]);

        let report = runtime.run_connection(&mut terminal).unwrap();
        assert!(matches!(
            report,
            ConnectionReport::Completed(RunReport {
                close_reason: SessionCloseReason::Goodbye,
                caller_name: Some(ref name),
                ..
            }) if name == "Minimal Caller"
        ));
        let output = terminal.output();
        assert!(contains(output, b"Minimal Terminal profile"));
        assert!(contains(output, b"SPITFIRE NG - MAIN MENU"));
        assert!(contains(output, b"SPITFIRE NG - MESSAGE MENU"));
        assert!(contains(output, b"Message 1 was saved"));
        assert!(contains(output, b"--- YOUR MESSAGES SENT"));
        assert!(contains(output, b"SPITFIRE NG - FILE MENU"));
        assert!(contains(output, b"ASCII download complete"));
        assert!(contains(output, b"Upload complete: MINIMAL.TXT"));
        assert!(contains(output, b"Files Uploaded: 1"));
        assert!(contains(output, b"Invalid or unavailable selection."));
        assert!(!contains(output, b"SPITFIRE NG - SYSOP MENU"));
        assert!(contains(
            output,
            b"Thank you for calling Minimal Acceptance Board"
        ));
        assert!(!output.contains(&0x1b));
        assert!(output.is_ascii());
    }

    #[test]
    fn clean_classic_profile_board_accepts_core_journeys_and_ascii_transfers() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("classic-acceptance-board");
        let mut plan = crate::SetupPlan::stock_defaults(
            "Classic Acceptance Board",
            "Acceptance Sysop",
            "Sysop",
            2,
        );
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        plan.config.caller.post_login_journey = PostLoginJourney::Stock;
        plan.config.presentation = sf_core::PresentationConfig {
            mode: sf_core::PresentationMode::Profile,
            menu_mode: sf_core::MenuPresentationMode::DisplayOverrides,
            active_profile: Some(crate::CLASSIC_PROFILE_ID.to_owned()),
            base_profile: Some(crate::MODERN_PROFILE_ID.to_owned()),
        };
        crate::setup_board(&root, &plan, b"test-only classic sysop password").unwrap();
        let runtime = BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap();
        let password = b"test-only classic caller password";
        let mut terminal = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"Classic Acceptance Caller".to_vec(),
            password.to_vec(),
            password.to_vec(),
            b"N".to_vec(),
            b"?".to_vec(),
            b"M".to_vec(),
            b"A".to_vec(),
            b"".to_vec(),
            b"M".to_vec(),
            b"E".to_vec(),
            b"".to_vec(),
            b"Classic presentation message".to_vec(),
            b"Live message state remains engine-owned".to_vec(),
            b"/S".to_vec(),
            b"Y".to_vec(),
            b"Y".to_vec(),
            b"S".to_vec(),
            b"".to_vec(),
            b"Q".to_vec(),
            b"B".to_vec(),
            b"Q".to_vec(),
            b"F".to_vec(),
            b"L".to_vec(),
            Vec::new(),
            b"D".to_vec(),
            b"WELCOME.TXT".to_vec(),
            b"1".to_vec(),
            b"U".to_vec(),
            b"CLASSIC.TXT".to_vec(),
            b"Classic profile acceptance upload".to_vec(),
            b"1".to_vec(),
            b"independent classic upload".to_vec(),
            b"/S".to_vec(),
            b"Q".to_vec(),
            b"Y".to_vec(),
            b"@".to_vec(),
            b"G".to_vec(),
        ]);

        let report = runtime.run_connection(&mut terminal).unwrap();
        assert!(matches!(
            report,
            ConnectionReport::Completed(RunReport {
                close_reason: SessionCloseReason::Goodbye,
                caller_name: Some(ref name),
                ..
            }) if name == "Classic Acceptance Caller"
        ));
        let output = terminal.output();
        for expected in [
            b"SPITFIRE MESSAGE SUMMARY".as_slice(),
            b"YOUR SPITFIRE STATISTICS".as_slice(),
            b"SPITFIRE NEW-FILE CHECK".as_slice(),
            b"SPITFIRE MAIN MENU".as_slice(),
            b"SPITFIRE MESSAGE MENU".as_slice(),
            b"Message 1 was saved".as_slice(),
            b"--- YOUR MESSAGES SENT".as_slice(),
            b"SPITFIRE FILE MENU".as_slice(),
            b"ASCII download complete".as_slice(),
            b"Upload complete: CLASSIC.TXT".as_slice(),
            b"Files Uploaded: 1".as_slice(),
            b"THANK YOU FOR CALLING".as_slice(),
        ] {
            assert!(
                contains(output, expected),
                "Classic transcript omitted {:?}",
                String::from_utf8_lossy(expected)
            );
        }
        assert!(contains(output, b"Invalid or unavailable selection."));
        assert!(!contains(output, b"SPITFIRE SYSOP MENU"));
        assert!(output.contains(&0x1b));
        assert!(output.contains(&0xc9));
        assert!(!contains(output, password));
    }

    #[test]
    fn clean_board_profile_policy_product_identity_and_operator_privacy_are_integrated() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("caller-policy-board");
        let mut plan =
            crate::SetupPlan::stock_defaults("Mystical Realm BBS", "Realm Sysop", "Sysop", 2);
        plan.config.board.timezone = "America/Phoenix".to_owned();
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        plan.config.caller.profile = sf_core::CallerProfilePolicy {
            address: sf_core::ProfileFieldPolicy::Required,
            phone: sf_core::ProfileFieldPolicy::Optional,
            email: sf_core::ProfileFieldPolicy::Required,
            birthday: sf_core::ProfileFieldPolicy::Optional,
        };
        crate::setup_board(&root, &plan, b"test-only profile sysop password").unwrap();
        let runtime = Arc::new(BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap());
        let password = b"test-only profile caller password";
        let mut registration = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"Profile Caller".to_vec(),
            password.to_vec(),
            password.to_vec(),
            b"001 Desert Road".to_vec(),
            b"".to_vec(),
            b"Phoenix".to_vec(),
            b"Arizona".to_vec(),
            b"00850".to_vec(),
            b"United States".to_vec(),
            b"".to_vec(),
            b"profile@example.test".to_vec(),
            b"".to_vec(),
            b"R".to_vec(),
            b"".to_vec(),
            b"-".to_vec(),
            b"Tempe".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"+1 602 555 0100".to_vec(),
            b"profile+edit@example.test".to_vec(),
            b"1990-01-02".to_vec(),
            b"V".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut registration).unwrap();
        let transcript = registration.output();
        assert!(contains(transcript, b"Welcome to Mystical Realm BBS"));
        assert!(contains(transcript, b"Your Sysop is Realm Sysop - Node 1"));
        assert!(contains(
            transcript,
            format!(
                "SPITFIRE NG Bulletin Board System - Version {}",
                sf_core::PRODUCT_VERSION
            )
            .as_bytes()
        ));
        assert!(contains(transcript, b"Copyright (C) 2026 Craig Daters"));
        assert!(contains(transcript, b"1987-2010 by Mike Woltz"));
        assert!(!contains(transcript, b"native SPITFIRE-style"));
        assert!(!contains(transcript, b"preservation-created"));
        assert!(!contains(transcript, password));

        let stored = runtime.caller(b"Profile Caller").unwrap();
        assert_eq!(stored.profile.address.city.as_deref(), Some("Tempe"));
        assert_eq!(stored.profile.address.postal_code.as_deref(), Some("00850"));
        assert_eq!(stored.profile.phone.as_deref(), Some("+1 602 555 0100"));
        assert_eq!(
            stored.profile.email.as_deref(),
            Some("profile+edit@example.test")
        );
        assert_eq!(stored.profile.birthday_iso().as_deref(), Some("1990-01-02"));

        let service = crate::OperatorService::new(Arc::clone(&runtime));
        let mut changed = service.caller("Profile Caller").unwrap().profile;
        changed.phone = Some("+1 480 555 0101".to_owned());
        let changed = service
            .set_caller_profile("Profile Caller", changed)
            .unwrap();
        assert_eq!(changed.profile.phone.as_deref(), Some("+1 480 555 0101"));

        let callers = service.callers().unwrap();
        assert_eq!(callers.len(), 2);
        let public_summary = callers
            .iter()
            .map(|caller| format!("{} {}", caller.display_name, caller.call_count))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!public_summary.contains("00850"));
        assert!(!public_summary.contains("profile+edit@example.test"));

        let mut reconnect = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Profile Caller".to_vec(),
            password.to_vec(),
            b"R".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"".to_vec(),
            b"/Q".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut reconnect).unwrap();
        assert!(contains(reconnect.output(), b"[+1 480 555 0101]"));
        assert!(contains(reconnect.output(), b"[1990-01-02]"));
    }

    #[test]
    fn registration_reprompts_email_before_a_later_required_birth_date() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("registration-validation-board");
        let mut plan = crate::SetupPlan::stock_defaults("Registration Board", "Sysop", "Sysop", 1);
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        plan.config.caller.profile = sf_core::CallerProfilePolicy {
            address: sf_core::ProfileFieldPolicy::Optional,
            phone: sf_core::ProfileFieldPolicy::Optional,
            email: sf_core::ProfileFieldPolicy::Optional,
            birthday: sf_core::ProfileFieldPolicy::Required,
        };
        crate::setup_board(&root, &plan, b"test-only registration sysop password").unwrap();
        let runtime = BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap();
        let password = b"test-only registration caller password";
        let mut registration = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"Registration Caller".to_vec(),
            password.to_vec(),
            password.to_vec(),
            b"123 Test Street".to_vec(),
            b"".to_vec(),
            b"Phoenix".to_vec(),
            b"Arizona".to_vec(),
            b"85001".to_vec(),
            b"United States".to_vec(),
            b"+1 602 555 0100".to_vec(),
            b"not-an-email".to_vec(),
            b"caller@example.test".to_vec(),
            b"1990-01-02".to_vec(),
            b"G".to_vec(),
        ]);

        runtime.run_connection(&mut registration).unwrap();

        let transcript = registration.output();
        assert!(contains(
            transcript,
            b"Please enter a valid email address or leave it blank if optional."
        ));
        assert!(contains(transcript, b"Birth Date (YYYY-MM-DD):"));
        assert!(contains(transcript, b"New caller registration complete."));
        let stored = runtime.caller(b"Registration Caller").unwrap();
        assert_eq!(stored.profile.email.as_deref(), Some("caller@example.test"));
        assert_eq!(stored.profile.birthday_iso().as_deref(), Some("1990-01-02"));
    }

    #[test]
    fn registration_profile_collection_follows_every_policy_combination() {
        let disabled = sf_core::ProfileFieldPolicy::Disabled;
        let optional = sf_core::ProfileFieldPolicy::Optional;
        let required = sf_core::ProfileFieldPolicy::Required;

        let (birthday_required, caller) = run_registration_policy_case(
            sf_core::CallerProfilePolicy {
                address: disabled,
                phone: disabled,
                email: optional,
                birthday: required,
            },
            b"Birthday Required",
            [b"".as_slice(), b"".as_slice(), b"not-a-date", b"1991-03-04"],
        );
        assert!(contains(
            &birthday_required,
            b"This information is required by the Sysop."
        ));
        assert!(contains(
            &birthday_required,
            b"Please enter a real date as YYYY-MM-DD."
        ));
        assert!(
            find_bytes(&birthday_required, b"Email:")
                < find_bytes(&birthday_required, b"Birth Date (YYYY-MM-DD):")
        );
        assert!(!contains(&birthday_required, b"Address Line 1:"));
        assert!(!contains(&birthday_required, b"Phone:"));
        assert_eq!(caller.profile.email, None);
        assert_eq!(caller.profile.birthday_iso().as_deref(), Some("1991-03-04"));

        let (email_required, caller) = run_registration_policy_case(
            sf_core::CallerProfilePolicy {
                address: disabled,
                phone: disabled,
                email: required,
                birthday: optional,
            },
            b"Email Required",
            [
                b"".as_slice(),
                b"not-an-email",
                b"required@example.test",
                b"",
            ],
        );
        assert!(contains(
            &email_required,
            b"This information is required by the Sysop."
        ));
        assert!(contains(
            &email_required,
            b"Please enter a valid email address"
        ));
        assert_eq!(
            caller.profile.email.as_deref(),
            Some("required@example.test")
        );
        assert!(!contains(&email_required, b"Address Line 1:"));
        assert!(!contains(&email_required, b"Phone:"));
        assert_eq!(caller.profile.birthday, None);

        let (birthday_disabled, caller) = run_registration_policy_case(
            sf_core::CallerProfilePolicy {
                address: disabled,
                phone: disabled,
                email: optional,
                birthday: disabled,
            },
            b"Birthday Disabled",
            [b"".as_slice()],
        );
        assert!(!contains(&birthday_disabled, b"Address Line 1:"));
        assert!(!contains(&birthday_disabled, b"Phone:"));
        assert!(!contains(&birthday_disabled, b"Birth Date (YYYY-MM-DD):"));
        assert_eq!(caller.profile.email, None);
        assert_eq!(caller.profile.birthday, None);

        let (multiple_required, caller) = run_registration_policy_case(
            sf_core::CallerProfilePolicy {
                address: required,
                phone: required,
                email: required,
                birthday: required,
            },
            b"Multiple Required",
            [
                b"".as_slice(),
                b"10 Policy Way",
                b"",
                b"",
                b"Phoenix",
                b"",
                b"",
                b"",
                b"United States",
                b"",
                b"+1 602 555 0199",
                b"",
                b"multiple@example.test",
                b"",
                b"1992-05-06",
            ],
        );
        assert!(contains(
            &multiple_required,
            b"New caller registration complete."
        ));
        assert!(
            find_bytes(&multiple_required, b"Address Line 1:")
                < find_bytes(&multiple_required, b"Phone:")
        );
        assert!(
            find_bytes(&multiple_required, b"Phone:") < find_bytes(&multiple_required, b"Email:")
        );
        assert!(
            find_bytes(&multiple_required, b"Email:")
                < find_bytes(&multiple_required, b"Birth Date (YYYY-MM-DD):")
        );
        assert_eq!(
            caller.profile.address.line_1.as_deref(),
            Some("10 Policy Way")
        );
        assert_eq!(caller.profile.address.city.as_deref(), Some("Phoenix"));
        assert_eq!(
            caller.profile.address.country.as_deref(),
            Some("United States")
        );
        assert_eq!(caller.profile.phone.as_deref(), Some("+1 602 555 0199"));
        assert_eq!(
            caller.profile.email.as_deref(),
            Some("multiple@example.test")
        );
        assert_eq!(caller.profile.birthday_iso().as_deref(), Some("1992-05-06"));
    }

    #[test]
    fn canceled_required_profile_collection_returns_to_login_without_a_partial_caller() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("registration-cancel-board");
        let mut plan = crate::SetupPlan::stock_defaults("Cancel Board", "Sysop", "Sysop", 1);
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        plan.config.caller.profile = sf_core::CallerProfilePolicy {
            address: sf_core::ProfileFieldPolicy::Disabled,
            phone: sf_core::ProfileFieldPolicy::Disabled,
            email: sf_core::ProfileFieldPolicy::Optional,
            birthday: sf_core::ProfileFieldPolicy::Required,
        };
        let sysop_password = b"test-only cancellation sysop password";
        crate::setup_board(&root, &plan, sysop_password).unwrap();
        let runtime = BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap();
        let new_password = b"test-only canceled caller password";
        let mut terminal = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"Canceled Caller".to_vec(),
            new_password.to_vec(),
            new_password.to_vec(),
            b"".to_vec(),
            b"/Q".to_vec(),
            b"Sysop".to_vec(),
            sysop_password.to_vec(),
            b"G".to_vec(),
        ]);

        let report = runtime.run_connection(&mut terminal).unwrap();

        assert!(matches!(
            report,
            ConnectionReport::Completed(RunReport {
                close_reason: SessionCloseReason::Goodbye,
                caller_name: Some(ref name),
                ..
            }) if name == "Sysop"
        ));
        assert!(contains(
            terminal.output(),
            b"New caller registration canceled. Returning to caller login."
        ));
        assert!(runtime.caller(b"Canceled Caller").is_err());
    }

    #[test]
    fn about_waits_for_acknowledgement_before_main_menu_redraw() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("about-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"About Caller",
            b"test-only about password",
            CallerState::Active,
        );
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut info = TerminalInfo::in_memory();
        info.capabilities.size = Some(TerminalSize {
            width: 80,
            height: 10,
        });
        let mut terminal = InMemoryTerminal::with_info(
            [
                b"N".to_vec(),
                b"About Caller".to_vec(),
                b"test-only about password".to_vec(),
                b"V".to_vec(),
                b"".to_vec(),
                b"".to_vec(),
                b"G".to_vec(),
            ],
            info,
        );

        let report = runtime.run_connection(&mut terminal).unwrap();

        assert!(matches!(
            report,
            ConnectionReport::Completed(RunReport {
                close_reason: SessionCloseReason::Goodbye,
                ..
            })
        ));
        assert!(
            contains(terminal.output(), b"Press ENTER to return to Main:"),
            "{}",
            String::from_utf8_lossy(terminal.output())
        );
        assert!(contains(
            terminal.output(),
            b"MORE: <S>top, <N>onstop, < ENTER > to continue?"
        ));
        assert!(contains(
            terminal.output(),
            b"SPITFIRE NG is not an official Buffalo Creek Software release."
        ));
    }

    #[test]
    fn minimal_about_uses_existing_pager_at_small_terminal_sizes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("minimal-paging-board");
        let mut plan = crate::SetupPlan::stock_defaults(
            "Minimal Paging Board",
            "Acceptance Sysop",
            "Sysop",
            1,
        );
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        plan.config.presentation = sf_core::PresentationConfig {
            mode: sf_core::PresentationMode::Profile,
            menu_mode: sf_core::MenuPresentationMode::DisplayOverrides,
            active_profile: Some(crate::MINIMAL_PROFILE_ID.to_owned()),
            base_profile: Some(crate::MODERN_PROFILE_ID.to_owned()),
        };
        crate::setup_board(&root, &plan, b"test-only minimal paging password").unwrap();
        let runtime = BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap();
        let mut info = TerminalInfo::in_memory();
        info.capabilities.size = Some(TerminalSize {
            width: 48,
            height: 10,
        });
        let mut terminal = InMemoryTerminal::with_info(
            [
                b"N".to_vec(),
                b"Sysop".to_vec(),
                b"test-only minimal paging password".to_vec(),
                b"".to_vec(),
                b"V".to_vec(),
                b"".to_vec(),
                b"".to_vec(),
                b"".to_vec(),
                b"G".to_vec(),
            ],
            info,
        );

        let report = runtime.run_connection(&mut terminal).unwrap();
        assert!(matches!(report, ConnectionReport::Completed(_)));
        assert!(contains(
            terminal.output(),
            b"MORE: <S>top, <N>onstop, < ENTER > to continue?"
        ));
        assert!(
            contains(terminal.output(), b"Press ENTER to return to Main:"),
            "{}",
            String::from_utf8_lossy(terminal.output())
        );
        assert!(contains(terminal.output(), b"SPITFIRE NG - MAIN MENU"));
        assert!(!terminal.output().contains(&0x1b));
    }

    #[test]
    fn classic_about_pages_and_returns_safely_at_48_by_10() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("classic-paging-board");
        let mut plan = crate::SetupPlan::stock_defaults(
            "Classic Paging Board",
            "Acceptance Sysop",
            "Sysop",
            1,
        );
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        plan.config.presentation = sf_core::PresentationConfig {
            mode: sf_core::PresentationMode::Profile,
            menu_mode: sf_core::MenuPresentationMode::DisplayOverrides,
            active_profile: Some(crate::CLASSIC_PROFILE_ID.to_owned()),
            base_profile: Some(crate::MODERN_PROFILE_ID.to_owned()),
        };
        crate::setup_board(&root, &plan, b"test-only classic paging password").unwrap();
        let runtime = BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap();
        let mut info = TerminalInfo::in_memory();
        info.capabilities.size = Some(TerminalSize {
            width: 48,
            height: 10,
        });
        let mut terminal = InMemoryTerminal::with_info(
            [
                b"N".to_vec(),
                b"Sysop".to_vec(),
                b"test-only classic paging password".to_vec(),
                b"V".to_vec(),
                b"N".to_vec(),
                b"".to_vec(),
                b"G".to_vec(),
            ],
            info,
        );

        let report = runtime.run_connection(&mut terminal).unwrap();
        assert!(matches!(report, ConnectionReport::Completed(_)));
        assert!(contains(
            terminal.output(),
            b"MORE: <S>top, <N>onstop, < ENTER > to continue?"
        ));
        assert!(
            contains(terminal.output(), b"Press ENTER to return to Main:"),
            "{}",
            String::from_utf8_lossy(terminal.output())
        );
        assert!(contains(terminal.output(), b"SPITFIRE NG SYSOP MAIN"));
        assert!(terminal.output().contains(&0x1b));
    }

    #[test]
    fn classic_art_makes_safe_progress_at_constrained_and_wide_sizes() {
        for size in [
            TerminalSize {
                width: 48,
                height: 10,
            },
            TerminalSize {
                width: 132,
                height: 40,
            },
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp
                .path()
                .join(format!("classic-{}x{}", size.width, size.height));
            initialize_fixture_board(&root).unwrap();
            use_fast_test_hashing(&root);
            seed_caller(
                &root,
                b"Classic Size Caller",
                b"test-only classic size password",
                CallerState::Active,
            );
            let config_path = root.join(FIXTURE_CONFIG_FILE);
            let mut config = RuntimeConfig::load(&config_path).unwrap();
            config.presentation = sf_core::PresentationConfig {
                mode: sf_core::PresentationMode::Profile,
                menu_mode: sf_core::MenuPresentationMode::DisplayOverrides,
                active_profile: Some(crate::CLASSIC_PROFILE_ID.to_owned()),
                base_profile: Some(crate::MODERN_PROFILE_ID.to_owned()),
            };
            config.caller.post_login_journey = PostLoginJourney::Stock;
            config.save_atomic(&config_path).unwrap();
            let validated = config.validate().unwrap();
            let paths = LogicalPaths::resolve(&root, &validated).unwrap();
            let database = RuntimeDatabase::open(paths.database()).unwrap();
            let caller = database
                .caller_by_name(b"Classic Size Caller")
                .unwrap()
                .unwrap();
            let mut preferences = caller.preferences;
            preferences.more_prompt = false;
            database
                .update_caller_preferences(caller.id, preferences)
                .unwrap();
            drop(database);

            let runtime = BoardRuntime::load(&config_path).unwrap();
            let mut info = TerminalInfo::in_memory();
            info.capabilities.size = Some(size);
            let mut terminal = InMemoryTerminal::with_info(
                [
                    b"N".to_vec(),
                    b"Classic Size Caller".to_vec(),
                    b"test-only classic size password".to_vec(),
                    b"N".to_vec(),
                    b"G".to_vec(),
                ],
                info,
            );
            let report = runtime.run_connection(&mut terminal).unwrap();
            assert!(matches!(report, ConnectionReport::Completed(_)));
            assert!(contains(terminal.output(), b"SPITFIRE MAIN MENU"));
            assert!(contains(terminal.output(), b"THANK YOU FOR CALLING"));
        }
    }

    #[test]
    fn private_board_requires_pre_authorized_security_and_idle_timeout_is_graceful() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("private-board");
        let mut plan =
            crate::SetupPlan::stock_defaults("Private Board", "Private Sysop", "Sysop", 1);
        plan.config.board.access = sf_core::BoardAccessMode::Private;
        plan.config.board.private_security_level = 50;
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        crate::setup_board(&root, &plan, b"test-only private sysop password").unwrap();
        seed_caller(
            &root,
            b"Low Security Caller",
            b"test-only low security password",
            CallerState::Active,
        );
        let runtime = BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap();
        let mut denied = InMemoryTerminal::with_lines([
            b"Low Security Caller".to_vec(),
            b"test-only low security password".to_vec(),
        ]);
        let ConnectionReport::Completed(denied_report) =
            runtime.run_connection(&mut denied).unwrap()
        else {
            panic!("private-board denial did not complete")
        };
        assert_eq!(
            denied_report.close_reason,
            SessionCloseReason::AccountUnavailable
        );
        assert!(contains(denied.output(), b"private SPITFIRE board"));
        assert!(!contains(denied.output(), b"Are you a New Caller"));
        assert!(!contains(denied.output(), b"MAIN MENU"));

        let mut idle = InMemoryTerminal::default();
        idle.set_supplied_credentials(sf_core::SuppliedCredentials::new(
            b"Sysop".to_vec(),
            b"test-only private sysop password".to_vec(),
        ));
        idle.timeout_next_input();
        let ConnectionReport::Completed(idle_report) = runtime.run_connection(&mut idle).unwrap()
        else {
            panic!("idle session did not complete")
        };
        assert_eq!(idle_report.close_reason, SessionCloseReason::Inactivity);
        assert!(contains(idle.output(), b"No activity time limit exceeded"));
        assert!(idle_report.node_idle_at_shutdown);
    }

    #[test]
    fn file_menu_rejects_empty_and_oversized_commands_without_disconnect() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"File Input Caller",
            b"test-only file input password",
            CallerState::Active,
        );
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"File Input Caller".to_vec(),
            b"test-only file input password".to_vec(),
            b"F".to_vec(),
            Vec::new(),
            b"\x1b[13;2uXY".to_vec(),
            b"L".to_vec(),
            Vec::new(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);

        let ConnectionReport::Completed(report) = runtime.run_connection(&mut terminal).unwrap()
        else {
            panic!("file-menu input-hardening session did not complete")
        };
        assert_eq!(report.close_reason, SessionCloseReason::Goodbye);
        assert!(contains(
            terminal.output(),
            b"Invalid or unavailable selection."
        ));
        assert!(contains(terminal.output(), b"WELCOME.TXT"));
        assert!(contains(
            terminal.output(),
            b"Press ENTER to return to the File Menu:"
        ));
    }

    #[test]
    fn complete_file_flow_lists_searches_downloads_uploads_and_persists_statistics() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"File Caller",
            b"test-only file password",
            CallerState::Active,
        );
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut first = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"File Caller".to_vec(),
            b"test-only file password".to_vec(),
            b"F".to_vec(),
            b"R".to_vec(),
            b"WELCOME.TXT".to_vec(),
            Vec::new(),
            b"L".to_vec(),
            Vec::new(),
            b"F".to_vec(),
            b"WELCOME".to_vec(),
            Vec::new(),
            b"T".to_vec(),
            b"synthetic file library".to_vec(),
            Vec::new(),
            b"D".to_vec(),
            b"WELCOME.TXT".to_vec(),
            b"C".to_vec(),
            b"2".to_vec(),
            b"L".to_vec(),
            Vec::new(),
            b"C".to_vec(),
            b"1".to_vec(),
            b"U".to_vec(),
            b"CALLER.TXT".to_vec(),
            b"Synthetic caller upload".to_vec(),
            b"hello from the caller".to_vec(),
            b"/S".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut first).unwrap();
        assert!(contains(first.output(), b"General Files"));
        assert!(contains(first.output(), b"SPITFIRE Files"));
        assert!(contains(first.output(), b"WELCOME.TXT"));
        assert!(contains(
            first.output(),
            b"Welcome to the SPITFIRE NG file library"
        ));
        assert!(contains(first.output(), b"Upload complete: CALLER.TXT"));

        let mut reconnect = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"File Caller".to_vec(),
            b"test-only file password".to_vec(),
            b"F".to_vec(),
            b"F".to_vec(),
            b"CALLER".to_vec(),
            Vec::new(),
            b"D".to_vec(),
            b"CALLER.TXT".to_vec(),
            b"Q".to_vec(),
            b"Y".to_vec(),
            b"G".to_vec(),
        ]);
        if let Err(error) = runtime.run_connection(&mut reconnect) {
            panic!(
                "reconnect file flow failed: {error:?}\n{}",
                String::from_utf8_lossy(reconnect.output())
            );
        }
        assert!(contains(reconnect.output(), b"hello from the caller"));
        assert!(contains(
            reconnect.output(),
            b"Files Uploaded: 1 (23 bytes)"
        ));
        assert!(contains(reconnect.output(), b"Files Downloaded: 2"));

        let config = RuntimeConfig::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database.caller_by_name(b"File Caller").unwrap().unwrap();
        let actor = FileActor::new(
            caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        let area = database.file_area(actor, 1).unwrap().0;
        assert!(database
            .files(actor, area.id)
            .unwrap()
            .iter()
            .any(|file| file.filename == "CALLER.TXT"));
        assert_eq!(caller.files_uploaded, 1);
        assert_eq!(caller.files_downloaded, 2);
        let file_events = database.file_activity(&EventQuery::default()).unwrap();
        assert!(file_events.events.iter().any(|event| {
            event.event_code == "file.added"
                && event.caller_id == Some(caller.id)
                && event.object_kind.as_deref() == Some("file")
        }));
    }

    #[test]
    fn unavailable_file_download_creates_one_private_durable_request() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Request Caller",
            b"test-only request password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database.caller_by_name(b"Request Caller").unwrap().unwrap();
        let actor = FileActor::new(
            caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        let area = database.file_area(actor, 1).unwrap().0;
        let storage = FileStorage::new(&paths).unwrap();
        let file = storage
            .write_seed_file(
                &mut database,
                &area,
                "OFFLINE.TXT",
                "Offline request fixture",
                b"not currently available",
                1_777_000_000,
            )
            .unwrap();
        database
            .set_file_lifecycle(
                sf_core::FileAdminActor::LocalOperator,
                file.id,
                file.state_version,
                sf_core::FileLifecycle::Offline,
            )
            .unwrap();
        drop(database);

        let runtime = BoardRuntime::load(&config_path).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Request Caller".to_vec(),
            b"test-only request password".to_vec(),
            b"F".to_vec(),
            b"D".to_vec(),
            b"OFFLINE.TXT".to_vec(),
            b"Y".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut terminal).unwrap();
        assert!(
            contains(terminal.output(), b"private file request was recorded"),
            "{}",
            String::from_utf8_lossy(terminal.output())
        );
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let requests = database
            .pending_file_requests(sf_core::FileAdminActor::LocalOperator)
            .unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].file_id, file.id);
        assert_eq!(requests[0].requesting_caller_id, caller.id);
    }

    #[test]
    fn preview_area_session_inspects_but_never_prompts_for_transfer_or_request() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Preview Journey Caller",
            b"test-only preview journey password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let area = database
            .create_file_area(&sf_core::FileAreaDefinition {
                number: 3,
                name: "Preview Files".to_owned(),
                description: "Synthetic inspect-only area".to_owned(),
                storage_key: "preview".to_owned(),
                access_mode: sf_core::FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(50).unwrap(),
                upload_security: SecurityLevel::new(50).unwrap(),
                preview: true,
                no_charge: false,
                maximum_upload_bytes: 1024 * 1024,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        let storage = FileStorage::new(&paths).unwrap();
        storage.ensure_area(&area).unwrap();
        storage
            .write_seed_file(
                &mut database,
                &area,
                "PREVIEW.TXT",
                "Preview session fixture",
                b"Preview-safe text.\r\n",
                1_777_000_010,
            )
            .unwrap();
        drop(database);

        let runtime = BoardRuntime::load(&config_path).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Preview Journey Caller".to_vec(),
            b"test-only preview journey password".to_vec(),
            b"F".to_vec(),
            b"C".to_vec(),
            b"3".to_vec(),
            b"R".to_vec(),
            b"PREVIEW.TXT".to_vec(),
            Vec::new(),
            b"D".to_vec(),
            b"U".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut terminal).unwrap();
        assert!(contains(terminal.output(), b"Preview-safe text."));
        assert!(contains(
            terminal.output(),
            b"This is a preview area; downloads are not permitted."
        ));
        assert!(contains(
            terminal.output(),
            b"This is a preview area; uploads are not permitted."
        ));
        assert!(!contains(terminal.output(), b"File to download"));
        assert!(!contains(terminal.output(), b"private request"));
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        assert!(database
            .pending_file_requests(sf_core::FileAdminActor::LocalOperator)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn tranche_5_inspection_semantics_are_identical_across_packaged_profiles() {
        use std::io::Cursor;

        for profile in [
            crate::MODERN_PROFILE_ID,
            crate::MINIMAL_PROFILE_ID,
            crate::CLASSIC_PROFILE_ID,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join(format!("{profile}-inspection-board"));
            initialize_fixture_board(&root).unwrap();
            use_fast_test_hashing(&root);
            seed_caller(
                &root,
                b"Profile Inspection Caller",
                b"test-only profile inspection password",
                CallerState::Active,
            );
            let config_path = root.join(FIXTURE_CONFIG_FILE);
            let mut config = RuntimeConfig::load(&config_path).unwrap();
            config.presentation.active_profile = Some(profile.to_owned());
            config.presentation.base_profile = Some(crate::MODERN_PROFILE_ID.to_owned());
            config.save_atomic(&config_path).unwrap();
            let paths = LogicalPaths::resolve(&root, &config.validate().unwrap()).unwrap();
            let mut database = RuntimeDatabase::open(paths.database()).unwrap();
            let area = database
                .create_file_area(&sf_core::FileAreaDefinition {
                    number: 3,
                    name: "Profile Preview".to_owned(),
                    description: "Synthetic profile parity area".to_owned(),
                    storage_key: "profile-preview".to_owned(),
                    access_mode: sf_core::FileAccessMode::AtLeast,
                    read_security: SecurityLevel::new(50).unwrap(),
                    upload_security: SecurityLevel::new(50).unwrap(),
                    preview: true,
                    no_charge: false,
                    maximum_upload_bytes: 1024 * 1024,
                    privileged_security_levels: Vec::new(),
                })
                .unwrap();
            let storage = FileStorage::new(&paths).unwrap();
            storage.ensure_area(&area).unwrap();
            storage
                .write_seed_file(
                    &mut database,
                    &area,
                    "PROFILE.TXT",
                    "Profile semantic text",
                    b"Identical safe inspection result.\r\n",
                    1_777_000_020,
                )
                .unwrap();
            let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
            writer
                .start_file(
                    "profile-member.txt",
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(b"metadata only").unwrap();
            let archive = writer.finish().unwrap().into_inner();
            storage
                .write_seed_file(
                    &mut database,
                    &area,
                    "PROFILE.ZIP",
                    "Profile semantic archive",
                    &archive,
                    1_777_000_021,
                )
                .unwrap();
            drop(database);

            let runtime = BoardRuntime::load(&config_path).unwrap();
            let mut terminal = InMemoryTerminal::with_lines([
                b"N".to_vec(),
                b"Profile Inspection Caller".to_vec(),
                b"test-only profile inspection password".to_vec(),
                b"F".to_vec(),
                b"C".to_vec(),
                b"3".to_vec(),
                b"R".to_vec(),
                b"PROFILE.TXT".to_vec(),
                Vec::new(),
                b"V".to_vec(),
                b"PROFILE.ZIP".to_vec(),
                Vec::new(),
                b"D".to_vec(),
                b"Q".to_vec(),
                b"G".to_vec(),
            ]);
            runtime.run_connection(&mut terminal).unwrap();
            assert!(
                contains(terminal.output(), b"Identical safe inspection result."),
                "{profile} text inspection transcript:\n{}",
                String::from_utf8_lossy(terminal.output())
            );
            assert!(
                contains(terminal.output(), b"profile-member.txt"),
                "{profile} ZIP inspection transcript:\n{}",
                String::from_utf8_lossy(terminal.output())
            );
            assert!(contains(
                terminal.output(),
                b"This is a preview area; downloads are not permitted."
            ));
        }
    }

    #[test]
    fn two_nodes_download_the_same_file_concurrently_over_raw_tcp() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        for (name, password) in [
            (
                b"Download One".as_slice(),
                b"test-only download one".as_slice(),
            ),
            (
                b"Download Two".as_slice(),
                b"test-only download two".as_slice(),
            ),
        ] {
            seed_caller(&root, name, password, CallerState::Active);
        }
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.node = None;
        config.nodes = Some(NodePoolConfig {
            count: 2,
            overrides: Vec::new(),
        });
        config.transports = vec![listener_config(
            "raw-files",
            TransportAdapterConfig::Raw {
                listen: address,
                terminal: NetworkTerminalDefaults::default(),
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(2), shutdown).unwrap());

        let mut first = connect_retry(address);
        let mut second = connect_retry(address);
        first
            .write_all(b"N\rDownload One\rtest-only download one\rF\rD\rWELCOME.TXT\rG\r")
            .unwrap();
        second
            .write_all(b"N\rDownload Two\rtest-only download two\rF\rD\rWELCOME.TXT\rG\r")
            .unwrap();
        let _ = first.shutdown(Shutdown::Write);
        let _ = second.shutdown(Shutdown::Write);
        first
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        second
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let first_reader = thread::spawn(move || {
            let mut output = Vec::new();
            let result = first.read_to_end(&mut output);
            (result, output)
        });
        let second_reader = thread::spawn(move || {
            let mut output = Vec::new();
            let result = second.read_to_end(&mut output);
            (result, output)
        });
        let (first_read, first_output) = first_reader.join().unwrap();
        let (second_read, second_output) = second_reader.join().unwrap();
        assert!(
            first_read.is_ok()
                || first_read
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionReset)
        );
        assert!(
            second_read.is_ok()
                || second_read
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionReset)
        );
        assert_eq!(handle.join().unwrap().completed_sessions, 2);
        assert!(contains(&first_output, b"ASCII download complete"));
        assert!(contains(&second_output, b"ASCII download complete"));

        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let first_caller = database.caller_by_name(b"Download One").unwrap().unwrap();
        let second_caller = database.caller_by_name(b"Download Two").unwrap().unwrap();
        assert_eq!(first_caller.files_downloaded, 1);
        assert_eq!(second_caller.files_downloaded, 1);
        let actor = FileActor::new(
            first_caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        let area = database.file_area(actor, 1).unwrap().0;
        let welcome = database.file(actor, area.id, "WELCOME.TXT", true).unwrap();
        assert_eq!(welcome.download_count, 2);
        let events = database
            .query_operational_events(&sf_core::EventQuery {
                limit: Some(100),
                ..sf_core::EventQuery::default()
            })
            .unwrap()
            .events;
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_code == "session.completed")
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_code == "transfer.download.completed")
                .count(),
            2
        );
        assert!(events
            .iter()
            .filter(|event| {
                event.event_code == "session.completed"
                    || event.event_code == "transfer.download.completed"
            })
            .all(|event| event.node_id.is_some()));
        let latest = events.first().unwrap();
        let summary = database
            .daily_operational_summary(latest.board_day, latest.timezone_policy_version)
            .unwrap()
            .unwrap();
        assert_eq!(summary.calls_started, 2);
        assert_eq!(summary.calls_completed, 2);
        assert_eq!(summary.successful_downloads, 2);
        assert_eq!(summary.download_bytes, welcome.size_bytes * 2);
        let minimum_time = events
            .iter()
            .filter(|event| event.category == EventCategory::Transfer)
            .map(|event| event.occurred_at_utc)
            .min()
            .unwrap();
        let maximum_time = events
            .iter()
            .filter(|event| event.category == EventCategory::Transfer)
            .map(|event| event.occurred_at_utc)
            .max()
            .unwrap();
        let transfer_activity = database
            .transfer_activity(minimum_time, maximum_time)
            .unwrap();
        assert!(transfer_activity.rows.iter().any(|row| {
            row.direction.as_deref() == Some("download")
                && row.outcome == EventOutcome::Succeeded
                && row.transfers == 2
                && row.bytes == welcome.size_bytes * 2
        }));
        let caller_activity = database
            .caller_activity(first_caller.id, &EventQuery::default())
            .unwrap()
            .unwrap();
        assert_eq!(caller_activity.public_handle, "Download One");
        assert_eq!(caller_activity.lifetime_files_downloaded, 1);
        let raw = format!("{events:?}");
        for private in [
            "test-only download one",
            "test-only download two",
            "/Users/",
            "fixture-board",
        ] {
            assert!(!raw.contains(private));
        }
        drop(database);
        let runtime = BoardRuntime::load(&config_path).unwrap();
        let context = OperatorObservabilityContext {
            principal: sf_core::OperatorPrincipal {
                kind: sf_core::OperatorPrincipalKind::HostOperator,
                stable_id: Some("test-host-operator".to_owned()),
            },
            capabilities: ObservabilityCapabilities::host_operator(),
        };
        assert_eq!(runtime.board_status(&context).unwrap().configured_nodes, 2);
        assert_eq!(runtime.live_node_statuses(&context).unwrap().len(), 2);
        assert_eq!(runtime.recent_callers(&context, 10).unwrap().len(), 2);
        assert_eq!(
            runtime
                .system_statistics(&context)
                .unwrap()
                .today
                .successful_downloads,
            2
        );
        assert!(
            runtime
                .maintenance_status(&context)
                .unwrap()
                .open_notifications
                == 0
        );
    }

    #[test]
    fn observability_projections_require_explicit_operator_capabilities() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let denied = OperatorObservabilityContext {
            principal: sf_core::OperatorPrincipal {
                kind: sf_core::OperatorPrincipalKind::NamedSysop,
                stable_id: Some("named-sysop".to_owned()),
            },
            capabilities: ObservabilityCapabilities {
                view_board_statistics: false,
                view_node_status: false,
                view_operational_events: false,
                view_caller_activity: false,
                view_notifications: false,
                view_maintenance_status: false,
                acknowledge_notifications: false,
            },
        };
        assert!(runtime.board_status(&denied).is_err());
        assert!(runtime.live_node_statuses(&denied).is_err());
        assert!(runtime
            .recent_operational_events(&denied, &EventQuery::default())
            .is_err());

        let mut database = RuntimeDatabase::open(runtime.database_path()).unwrap();
        let mut warning = NewOperationalEvent::new(
            current_unix_seconds().unwrap(),
            EventCategory::Storage,
            EventSeverity::Warning,
            "storage.unavailable",
            EventOutcome::Unavailable,
        );
        warning.attributes = sf_core::EventAttributes::Storage {
            state: "unavailable".to_owned(),
        };
        database.record_operational_event(&warning).unwrap();
        drop(database);

        let named = OperatorObservabilityContext {
            principal: sf_core::OperatorPrincipal {
                kind: sf_core::OperatorPrincipalKind::NamedSysop,
                stable_id: Some("named-sysop".to_owned()),
            },
            capabilities: ObservabilityCapabilities::named_sysop(),
        };
        let notification = runtime.operator_notifications(&named, false, 10).unwrap()[0].clone();
        assert!(runtime
            .acknowledge_operator_notification(&named, notification.id, notification.state_version)
            .is_err());
        let host = OperatorObservabilityContext {
            principal: sf_core::OperatorPrincipal {
                kind: sf_core::OperatorPrincipalKind::HostOperator,
                stable_id: Some("host-operator".to_owned()),
            },
            capabilities: ObservabilityCapabilities::host_operator(),
        };
        assert!(runtime
            .acknowledge_operator_notification(&host, notification.id, notification.state_version,)
            .unwrap());
    }

    #[test]
    fn telnet_caller_browses_searches_downloads_and_uploads_end_to_end() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Telnet File Caller",
            b"test-only telnet file password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![listener_config(
            "telnet-files",
            TransportAdapterConfig::Telnet {
                listen: address,
                terminal: NetworkTerminalDefaults::default(),
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(1), shutdown).unwrap());

        let mut stream = connect_retry(address);
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(&[
                255, 251, 24, 255, 250, 24, 0, b'A', b'N', b'S', b'I', 255, 240, 255, 250, 31, 0,
                80, 0, 25, 255, 240,
            ])
            .unwrap();
        stream
            .write_all(
                b"N\rTelnet File Caller\rtest-only telnet file password\rF\rL\r\rF\rWELCOME\r\rD\rWELCOME.TXT\rU\rNETFILE.TXT\rNetwork acceptance upload\rnetwork upload body\r/S\rF\rNETFILE\r\rD\rNETFILE.TXT\rQ\rY\rG\r",
            )
            .unwrap();
        let _ = stream.shutdown(Shutdown::Write);
        let mut transcript = Vec::new();
        stream.read_to_end(&mut transcript).unwrap();
        assert_eq!(handle.join().unwrap().completed_sessions, 1);
        for expected in [
            b"General Files".as_slice(),
            b"WELCOME.TXT".as_slice(),
            b"Welcome to the SPITFIRE NG file library".as_slice(),
            b"Upload complete: NETFILE.TXT".as_slice(),
            b"network upload body".as_slice(),
            b"Files Uploaded: 1".as_slice(),
            b"Files Downloaded: 2".as_slice(),
        ] {
            assert!(contains(&transcript, expected), "missing {expected:?}");
        }

        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database
            .caller_by_name(b"Telnet File Caller")
            .unwrap()
            .unwrap();
        assert_eq!(caller.files_uploaded, 1);
        assert_eq!(caller.files_downloaded, 2);
    }

    #[test]
    fn end_of_input_releases_node_gracefully() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        let mut terminal = InMemoryTerminal::default();
        let report = run_board(&root.join(FIXTURE_CONFIG_FILE), &mut terminal).unwrap();
        assert_eq!(report.close_reason, SessionCloseReason::EndOfInput);
        assert!(report.node_idle_at_shutdown);
    }

    #[test]
    fn new_caller_reconnects_with_persistent_identity_and_call_count() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let password = b"test-only reconnect password";
        let mut registration = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"Persistent Caller".to_vec(),
            password.to_vec(),
            password.to_vec(),
            b"G".to_vec(),
        ]);
        let first = runtime.run_connection(&mut registration).unwrap();
        assert!(matches!(first, ConnectionReport::Completed(_)));

        let mut reconnect = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"PERSISTENT CALLER".to_vec(),
            password.to_vec(),
            b"Y".to_vec(),
            b"G".to_vec(),
        ]);
        let second = runtime.run_connection(&mut reconnect).unwrap();
        let ConnectionReport::Completed(second) = second else {
            panic!("all configured nodes were unexpectedly busy");
        };
        assert_eq!(second.caller_name.as_deref(), Some("Persistent Caller"));
        assert!(contains(reconnect.output(), b"Times On: 2"));
        assert!(contains(reconnect.output(), b"YOUR SPITFIRE STATISTICS"));
        assert!(!contains(reconnect.output(), password));
    }

    #[test]
    fn failed_authentication_releases_node_one() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Unknown Caller".to_vec(),
            b"wrong-one".to_vec(),
            b"Unknown Caller".to_vec(),
            b"wrong-two".to_vec(),
            b"Unknown Caller".to_vec(),
            b"wrong-three".to_vec(),
        ]);
        let report = runtime.run_connection(&mut terminal).unwrap();
        let ConnectionReport::Completed(report) = report else {
            panic!("all configured nodes were unexpectedly busy");
        };
        assert_eq!(
            report.close_reason,
            SessionCloseReason::AuthenticationFailed
        );
        assert!(report.node_idle_at_shutdown);
        assert!(!contains(terminal.output(), b"wrong-one"));
    }

    #[test]
    fn disabled_caller_is_rejected_before_the_main_menu() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Disabled Caller",
            b"test-only disabled password",
            CallerState::Disabled,
        );
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Disabled Caller".to_vec(),
            b"test-only disabled password".to_vec(),
        ]);
        let report = runtime.run_connection(&mut terminal).unwrap();
        let ConnectionReport::Completed(report) = report else {
            panic!("all configured nodes were unexpectedly busy");
        };
        assert_eq!(report.close_reason, SessionCloseReason::AccountUnavailable);
        assert!(!contains(terminal.output(), b"MAIN MENU"));
        assert!(!contains(terminal.output(), b"test-only disabled password"));
    }

    #[test]
    fn joker_full_and_substring_rules_deny_returning_and_new_names_privately() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("joker-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Blocked Caller",
            b"test-only blocked password",
            CallerState::Active,
        );
        fs::write(
            root.join("system/JOKER.DAT"),
            b"Blocked Caller\r\n@fragment\r\n",
        )
        .unwrap();
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut returning =
            InMemoryTerminal::with_lines([b"N".to_vec(), b"blocked caller".to_vec()]);
        let ConnectionReport::Completed(report) = runtime.run_connection(&mut returning).unwrap()
        else {
            panic!("all nodes unexpectedly busy");
        };
        assert_eq!(report.close_reason, SessionCloseReason::AccountUnavailable);
        assert!(contains(
            returning.output(),
            b"caller account is not available"
        ));
        assert!(!contains(returning.output(), b"Blocked Caller"));
        assert!(!contains(returning.output(), b"fragment"));

        let mut new_caller =
            InMemoryTerminal::with_lines([b"Y".to_vec(), b"Synthetic Fragment Name".to_vec()]);
        let ConnectionReport::Completed(report) = runtime.run_connection(&mut new_caller).unwrap()
        else {
            panic!("all nodes unexpectedly busy");
        };
        assert_eq!(report.close_reason, SessionCloseReason::AccountUnavailable);
        assert!(!runtime.caller_exists(b"Synthetic Fragment Name").unwrap());
    }

    #[test]
    fn subscription_warning_expiry_and_renewal_preserve_base_security() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("subscription-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Subscription Caller",
            b"test-only subscription password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.caller.subscription.enabled = true;
        config.caller.subscription.warning_days = 7;
        config.caller.subscription.expired_security = 5;
        config.save_atomic(&config_path).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let now = current_unix_seconds().unwrap();
        let today = chrono::DateTime::<chrono::Utc>::from_timestamp(now, 0)
            .unwrap()
            .with_timezone(&validated.timezone)
            .date_naive();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database
            .caller_by_name(b"Subscription Caller")
            .unwrap()
            .unwrap();
        database
            .update_caller_subscription(
                caller.id,
                caller.state_version,
                Some(today),
                CallerAccessActor::LocalOperator,
                &validated.caller,
                now,
                validated.timezone,
            )
            .unwrap();
        drop(database);
        let runtime = BoardRuntime::load(&config_path).unwrap();
        let mut warning = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Subscription Caller".to_vec(),
            b"test-only subscription password".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut warning).unwrap();
        assert!(contains(warning.output(), b"subscription will expire soon"));
        drop(runtime);

        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database
            .caller_by_name(b"Subscription Caller")
            .unwrap()
            .unwrap();
        database
            .update_caller_subscription(
                caller.id,
                caller.state_version,
                Some(today.pred_opt().unwrap()),
                CallerAccessActor::LocalOperator,
                &validated.caller,
                now,
                validated.timezone,
            )
            .unwrap();
        drop(database);
        let runtime = Arc::new(BoardRuntime::load(&config_path).unwrap());
        let mut expired = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Subscription Caller".to_vec(),
            b"test-only subscription password".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut expired).unwrap();
        assert!(contains(expired.output(), b"access level changed"));
        let restricted = runtime.caller(b"Subscription Caller").unwrap();
        assert_eq!(restricted.base_security_level.get(), 10);
        assert_eq!(restricted.security_level.get(), 5);
        let service = crate::OperatorService::new(Arc::clone(&runtime));
        let renewed = service
            .update_caller_subscription("Subscription Caller", Some(today.succ_opt().unwrap()))
            .unwrap();
        assert_eq!(renewed.base_security_level.get(), 10);
        assert_eq!(renewed.security_level.get(), 10);
    }

    #[test]
    fn duplicate_new_caller_name_returns_to_the_normal_login_path() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Existing Caller",
            b"test-only existing password",
            CallerState::Active,
        );
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"existing caller".to_vec(),
            b"Existing Caller".to_vec(),
            b"test-only existing password".to_vec(),
            b"G".to_vec(),
        ]);
        let report = runtime.run_connection(&mut terminal).unwrap();
        assert!(matches!(report, ConnectionReport::Completed(_)));
        assert!(contains(
            terminal.output(),
            b"Continuing with returning-caller login"
        ));
        assert!(contains(terminal.output(), b"Welcome, Existing Caller"));
        assert!(!contains(terminal.output(), b"test-only existing password"));
    }

    #[test]
    fn fixture_sysop_requires_explicit_secret_initialization() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let sysop = runtime
            .initialize_sysop(b"test-only explicit sysop password")
            .unwrap();
        assert_eq!(sysop.display_name, "Sysop");
        assert!(sysop
            .security_level
            .is_sysop(SecurityLevel::new(50).unwrap()));
    }

    #[test]
    fn all_nodes_busy_is_polite_and_does_not_corrupt_state() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let active_id = SessionId::new(99).unwrap();
        let lease = runtime
            .nodes
            .acquire(
                active_id,
                TransportKind::InMemory,
                current_unix_seconds().unwrap(),
            )
            .unwrap();
        let mut terminal = InMemoryTerminal::with_lines([b"G".to_vec()]);
        assert_eq!(
            runtime.run_connection(&mut terminal).unwrap(),
            ConnectionReport::NodeBusy
        );
        assert!(contains(terminal.output(), b"currently busy"));
        assert!(terminal.disconnected());
        let occupied = runtime.node_snapshots().unwrap();
        assert_eq!(occupied[0].state, NodeRuntimeState::Connecting);
        assert_eq!(occupied[0].session_id, Some(active_id));
        drop(lease);
        assert_eq!(
            runtime.node_snapshots().unwrap()[0].state,
            NodeRuntimeState::Waiting
        );
    }

    #[test]
    fn missing_configuration_is_reported_without_panicking() {
        let temp = tempfile::tempdir().unwrap();
        let mut terminal = InMemoryTerminal::default();
        assert!(matches!(
            run_board(&temp.path().join("missing.toml"), &mut terminal),
            Err(ApplicationError::ResolveConfiguration { .. })
        ));
    }

    #[test]
    fn raw_telnet_and_rlogin_reach_the_same_menu_engine() {
        for kind in [
            TransportKind::RawTcp,
            TransportKind::Telnet,
            TransportKind::Rlogin,
        ] {
            let transcript = run_loopback(kind);
            for expected in [
                b"MAIN MENU".as_slice(),
                b"MESSAGE MENU".as_slice(),
                b"Welcome to the fixture board".as_slice(),
                b"FILE MENU".as_slice(),
                b"SPITFIRE CALLER DIRECTORY".as_slice(),
                b"Is Loopback Caller the caller you want?".as_slice(),
                b"Loopback Fixture BBS".as_slice(),
                b"Caller additions to Other BBS information are disabled.".as_slice(),
                b"SPITFIRE BULLETINS".as_slice(),
                b"SPITFIRE NG Newsletter".as_slice(),
                b"SPITFIRE SYSTEM INFORMATION".as_slice(),
                b"Thank you for calling".as_slice(),
            ] {
                assert!(
                    contains(&transcript, expected),
                    "{kind:?} transcript did not contain {:?}",
                    String::from_utf8_lossy(expected)
                );
            }
        }
    }

    #[test]
    fn public_information_journey_never_releases_private_caller_fields() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("public-information-privacy");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Legacy Public Name",
            b"test-only public information password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let runtime = BoardRuntime::load(&config_path).unwrap();
        let identity = runtime
            .set_caller_identity(
                b"Legacy Public Name",
                b"private-login-id",
                b"PublicHandle",
                Some("Sensitive Real Name".to_owned()),
            )
            .unwrap();
        drop(runtime);
        let config = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &config).unwrap();
        let connection = rusqlite::Connection::open(paths.database()).unwrap();
        connection.execute("UPDATE callers SET email='private@example.test',phone='555-0199',address_line_1='99 Private Road',birthday='1990-01-02',city='Public City',region='AZ',subscription_expires_on='2030-01-01' WHERE caller_id=?1", [identity.id.get()]).unwrap();
        drop(connection);
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        database
            .update_public_directory_policy(
                sf_core::PublicInformationActor::LocalOperator,
                1,
                true,
                true,
                true,
                false,
                1_700_000_030,
            )
            .unwrap();
        database
            .update_caller_publicity(
                sf_core::PublicInformationActor::Caller(identity.id),
                identity.id,
                identity.publicity_state_version,
                true,
                1_700_000_031,
            )
            .unwrap();
        drop(database);
        let runtime = BoardRuntime::load(&config_path).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"PublicHandle".to_vec(),
            b"test-only public information password".to_vec(),
            b"#".to_vec(),
            b"N".to_vec(),
            b"L".to_vec(),
            b"handle".to_vec(),
            b"Y".to_vec(),
            b"T".to_vec(),
            b"O".to_vec(),
            b"B".to_vec(),
            b"1".to_vec(),
            b"N".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut terminal).unwrap();
        let output = terminal.output();
        assert!(contains(output, b"PublicHandle"));
        assert!(contains(output, b"Public City, AZ"));
        for private in [
            b"private-login-id".as_slice(),
            b"Sensitive Real Name".as_slice(),
            b"private@example.test".as_slice(),
            b"555-0199".as_slice(),
            b"99 Private Road".as_slice(),
            b"1990-01-02".as_slice(),
            b"2030-01-01".as_slice(),
        ] {
            assert!(
                !contains(output, private),
                "private value leaked: {}",
                String::from_utf8_lossy(private)
            );
        }
        assert!(!contains(
            output,
            paths.database().display().to_string().as_bytes()
        ));
        assert!(!contains(output, b"host-ed25519"));
    }

    #[test]
    fn newsletter_digest_change_notifies_a_returning_caller_and_survives_reload() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("newsletter-notification");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Newsletter Caller",
            b"test-only newsletter password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let runtime = BoardRuntime::load(&config_path).unwrap();
        let mut first = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Newsletter Caller".to_vec(),
            b"test-only newsletter password".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut first).unwrap();
        drop(runtime);
        std::thread::sleep(Duration::from_secs(1));
        fs::write(
            root.join("display/SFNWSLTR.BBS"),
            b"Updated synthetic newsletter\r\n",
        )
        .unwrap();
        let runtime = BoardRuntime::load(&config_path).unwrap();
        let mut second = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Newsletter Caller".to_vec(),
            b"test-only newsletter password".to_vec(),
            b"N".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut second).unwrap();
        assert!(contains(
            second.output(),
            b"newsletter has changed since your previous call"
        ));
        assert!(contains(second.output(), b"Updated synthetic newsletter"));
    }

    #[test]
    fn missing_publication_resources_fail_safely_without_releasing_paths() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("missing-publications");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Missing Resource Caller",
            b"test-only missing resource password",
            CallerState::Active,
        );
        for name in ["BULLETIN.BBS", "BULLET1.BBS", "SFNWSLTR.BBS", "THOUGHTS.NG"] {
            fs::remove_file(root.join("display").join(name)).unwrap();
        }
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Missing Resource Caller".to_vec(),
            b"test-only missing resource password".to_vec(),
            b"B".to_vec(),
            b"N".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut terminal).unwrap();
        assert!(contains(terminal.output(), b"No bulletins are available"));
        assert!(contains(terminal.output(), b"newsletter is unavailable"));
        assert!(!contains(
            terminal.output(),
            root.display().to_string().as_bytes()
        ));
        assert!(!contains(terminal.output(), b"SFNWSLTR.BBS"));
    }

    #[test]
    fn minimal_profile_is_plain_text_over_raw_tcp_and_telnet() {
        for kind in [TransportKind::RawTcp, TransportKind::Telnet] {
            let transcript = run_loopback_with_selection(
                kind,
                Some(sf_core::PresentationConfig {
                    mode: sf_core::PresentationMode::Profile,
                    menu_mode: sf_core::MenuPresentationMode::DisplayOverrides,
                    active_profile: Some(crate::MINIMAL_PROFILE_ID.to_owned()),
                    base_profile: Some(crate::MODERN_PROFILE_ID.to_owned()),
                }),
            );
            for expected in [
                b"Minimal Terminal profile".as_slice(),
                b"SPITFIRE NG - MAIN MENU".as_slice(),
                b"SPITFIRE NG - MESSAGE MENU".as_slice(),
                b"SPITFIRE NG - FILE MENU".as_slice(),
                b"Thank you for calling".as_slice(),
            ] {
                assert!(
                    contains(&transcript, expected),
                    "{kind:?} Minimal transcript did not contain {:?}",
                    String::from_utf8_lossy(expected)
                );
            }
            assert!(!transcript.contains(&0x1b));
            if kind == TransportKind::RawTcp {
                assert!(transcript.is_ascii());
            }
        }
    }

    #[test]
    fn classic_profile_uses_clr_over_telnet_and_bbs_over_raw_tcp() {
        for kind in [TransportKind::RawTcp, TransportKind::Telnet] {
            let transcript = run_loopback_with_selection_and_journey(
                kind,
                Some(sf_core::PresentationConfig {
                    mode: sf_core::PresentationMode::Profile,
                    menu_mode: sf_core::MenuPresentationMode::DisplayOverrides,
                    active_profile: Some(crate::CLASSIC_PROFILE_ID.to_owned()),
                    base_profile: Some(crate::MODERN_PROFILE_ID.to_owned()),
                }),
                true,
            );
            for expected in [
                b"SPITFIRE MESSAGE SUMMARY".as_slice(),
                b"SPITFIRE NEW-FILE CHECK".as_slice(),
                b"SPITFIRE MAIN MENU".as_slice(),
                b"SPITFIRE MESSAGE MENU".as_slice(),
                b"SPITFIRE FILE MENU".as_slice(),
                b"THANK YOU FOR CALLING".as_slice(),
            ] {
                assert!(contains(&transcript, expected));
            }
            assert!(transcript.contains(&0xc9));
            if kind == TransportKind::RawTcp {
                assert!(!transcript.contains(&0x1b));
            } else {
                assert!(transcript.contains(&0x1b));
            }
        }
    }

    #[test]
    fn shell_serial_and_modem_metadata_use_the_same_caller_engine() {
        for kind in [
            TransportKind::UnixShell,
            TransportKind::DirectSerial,
            TransportKind::HayesModem,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("fixture-board");
            initialize_fixture_board(&root).unwrap();
            use_fast_test_hashing(&root);
            seed_caller(
                &root,
                b"Adapter Caller",
                b"test-only adapter password",
                CallerState::Active,
            );
            let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
            let info = TerminalInfo {
                transport: kind,
                local: kind == TransportKind::UnixShell,
                capabilities: TerminalCapabilities {
                    terminal_type: Some("synthetic-ansi".to_owned()),
                    ansi: true,
                    cp437: true,
                    size: Some(TerminalSize {
                        width: 80,
                        height: 25,
                    }),
                },
                remote_address: None,
                connected_at: std::time::SystemTime::now(),
                connection_speed: (kind != TransportKind::UnixShell).then_some(38_400),
                carrier: (kind == TransportKind::HayesModem).then_some(true),
                declared_identity: None,
            };
            let mut terminal = InMemoryTerminal::with_info(
                [
                    b"N".to_vec(),
                    b"Adapter Caller".to_vec(),
                    b"test-only adapter password".to_vec(),
                    b"M".to_vec(),
                    b"B".to_vec(),
                    b"Q".to_vec(),
                    b"G".to_vec(),
                ],
                info,
            );
            let report = runtime.run_connection(&mut terminal).unwrap();
            assert!(matches!(report, ConnectionReport::Completed(_)));
            assert!(contains(terminal.output(), b"Welcome, Adapter Caller"));
            assert!(contains(terminal.output(), b"Welcome to the fixture board"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn serial_and_hayes_adapters_enter_the_authenticated_session_engine() {
        use serialport::SerialPort;

        for modem in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("fixture-board");
            initialize_fixture_board(&root).unwrap();
            use_fast_test_hashing(&root);
            seed_caller(
                &root,
                b"Serial Caller",
                b"test-only serial password",
                CallerState::Active,
            );
            let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
            let (mut master, slave) = serialport::TTYPort::pair().unwrap();
            master.set_timeout(Duration::from_secs(2)).unwrap();
            let defaults = NetworkTerminalDefaults::default();
            let handle = thread::spawn(move || {
                if modem {
                    let mut terminal = ModemTerminal::answer_port(
                        Box::new(slave),
                        "AT&F",
                        "ATA",
                        &defaults,
                        Duration::from_secs(2),
                    )
                    .unwrap();
                    runtime.run_connection(&mut terminal).unwrap()
                } else {
                    let mut terminal = SerialTerminal::from_port(
                        Box::new(slave),
                        TransportKind::DirectSerial,
                        38_400,
                        None,
                        &defaults,
                    );
                    runtime.run_connection(&mut terminal).unwrap()
                }
            });

            if modem {
                assert!(contains(&read_until(&mut master, b"AT&F\r"), b"AT&F\r"));
                master.write_all(b"OK\rRING\r").unwrap();
                assert!(contains(&read_until(&mut master, b"ATA\r"), b"ATA\r"));
                master.write_all(b"CONNECT 14400\r").unwrap();
            }
            master
                .write_all(b"N\rSerial Caller\rtest-only serial password\rM\rB\rQ\rG\r")
                .unwrap();
            master.flush().unwrap();
            let transcript = read_until(&mut master, b"Goodbye!");
            let report = handle.join().unwrap();
            assert!(matches!(report, ConnectionReport::Completed(_)));
            assert!(contains(&transcript, b"Welcome, Serial Caller"));
            assert!(contains(&transcript, b"Welcome to the fixture board"));
            assert!(!contains(&transcript, b"test-only serial password"));
        }
    }

    #[test]
    fn supplied_rlogin_credentials_use_the_same_verifier_and_invalid_data_falls_back() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"RLogin Caller",
            b"test-only rlogin password",
            CallerState::Active,
        );
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let mut valid = InMemoryTerminal::with_lines([b"G".to_vec()]);
        valid.set_supplied_credentials(sf_core::SuppliedCredentials::new(
            b"RLogin Caller".to_vec(),
            b"test-only rlogin password".to_vec(),
        ));
        let valid_report = runtime.run_connection(&mut valid).unwrap();
        assert!(matches!(valid_report, ConnectionReport::Completed(_)));
        assert!(!contains(valid.output(), b"test-only rlogin password"));

        let mut invalid = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"RLogin Caller".to_vec(),
            b"test-only rlogin password".to_vec(),
            b"G".to_vec(),
        ]);
        invalid.set_supplied_credentials(sf_core::SuppliedCredentials::new(
            b"RLogin Caller".to_vec(),
            b"incorrect supplied secret".to_vec(),
        ));
        let captured_logs = Arc::new(Mutex::new(Vec::new()));
        let log_destination = Arc::clone(&captured_logs);
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || CapturedLogWriter(Arc::clone(&log_destination)))
            .finish();
        let invalid_report = tracing::subscriber::with_default(subscriber, || {
            runtime.run_connection(&mut invalid).unwrap()
        });
        assert!(matches!(invalid_report, ConnectionReport::Completed(_)));
        assert!(contains(invalid.output(), b"continuing with normal login"));
        assert!(!contains(invalid.output(), b"incorrect supplied secret"));
        assert!(!contains(
            &captured_logs.lock().unwrap(),
            b"incorrect supplied secret"
        ));

        let mut overlong = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"RLogin Caller".to_vec(),
            b"test-only rlogin password".to_vec(),
            b"G".to_vec(),
        ]);
        overlong.set_supplied_credentials(sf_core::SuppliedCredentials::new(
            b"RLogin Caller".to_vec(),
            vec![b'x'; 129],
        ));
        let overlong_report = runtime.run_connection(&mut overlong).unwrap();
        assert!(matches!(overlong_report, ConnectionReport::Completed(_)));
        assert!(contains(overlong.output(), b"continuing with normal login"));
    }

    #[test]
    fn complete_message_flow_persists_replies_private_mail_sysop_comments_and_last_read() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Recipient Caller",
            b"test-only recipient password",
            CallerState::Active,
        );
        seed_caller(
            &root,
            b"Unrelated Caller",
            b"test-only unrelated password",
            CallerState::Active,
        );
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        runtime
            .initialize_sysop(b"test-only explicit sysop password")
            .unwrap();
        {
            let mut database = RuntimeDatabase::open(runtime.database_path()).unwrap();
            let recipient = database
                .caller_by_name(b"Recipient Caller")
                .unwrap()
                .unwrap();
            let actor = MessageActor::new(recipient.id, SecurityLevel::new(100).unwrap());
            database.replace_queue(actor, &[2]).unwrap();
        }

        let password = b"test-only message password";
        let mut posting = InMemoryTerminal::with_lines([
            b"Y".to_vec(),
            b"Message Caller".to_vec(),
            password.to_vec(),
            password.to_vec(),
            b"M".to_vec(),
            b"C".to_vec(),
            b"1".to_vec(),
            b"R".to_vec(),
            b"T".to_vec(),
            b"R".to_vec(),
            b"N".to_vec(),
            b"".to_vec(),
            b"A synthetic reply".to_vec(),
            b"/S".to_vec(),
            b"Y".to_vec(),
            b"Q".to_vec(),
            b"E".to_vec(),
            b"".to_vec(),
            b"Hello from SPITFIRE NG".to_vec(),
            b"A persistent public post".to_vec(),
            b"/S".to_vec(),
            b"Y".to_vec(),
            b"C".to_vec(),
            b"2".to_vec(),
            b"R".to_vec(),
            b"T".to_vec(),
            b"Q".to_vec(),
            b"E".to_vec(),
            b"Recipient Caller".to_vec(),
            b"".to_vec(),
            b"Y".to_vec(),
            b"Private fixture greeting".to_vec(),
            b"Private message content".to_vec(),
            b"/S".to_vec(),
            b"Y".to_vec(),
            b"Q".to_vec(),
            b"C".to_vec(),
            b"Fixture Sysop comment".to_vec(),
            b"Please review this synthetic board".to_vec(),
            b"/S".to_vec(),
            b"Y".to_vec(),
            b"G".to_vec(),
        ]);
        let report = runtime
            .run_connection(&mut posting)
            .unwrap_or_else(|error| {
                panic!(
                    "message-flow session failed: {error:?}\n{}",
                    String::from_utf8_lossy(posting.output())
                )
            });
        assert!(matches!(report, ConnectionReport::Completed(_)));
        assert!(contains(posting.output(), b"Conference 1: General"));
        assert!(contains(posting.output(), b"Conference 2: SPITFIRE"));
        assert!(contains(posting.output(), b"Message 2 was saved"));
        assert!(!contains(posting.output(), password));

        let mut reconnect = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"MESSAGE CALLER".to_vec(),
            password.to_vec(),
            b"M".to_vec(),
            b"B".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut reconnect).unwrap();
        assert!(contains(reconnect.output(), b"Hello from SPITFIRE NG"));
        assert!(contains(
            reconnect.output(),
            b"Welcome to the fixture board"
        ));

        let mut recipient = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Recipient Caller".to_vec(),
            b"test-only recipient password".to_vec(),
            b"M".to_vec(),
            b"C".to_vec(),
            b"2".to_vec(),
            b"B".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut recipient).unwrap();
        assert!(contains(recipient.output(), b"Private fixture greeting"));

        let mut unrelated = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Unrelated Caller".to_vec(),
            b"test-only unrelated password".to_vec(),
            b"M".to_vec(),
            b"C".to_vec(),
            b"2".to_vec(),
            b"B".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        runtime.run_connection(&mut unrelated).unwrap();
        assert!(!contains(unrelated.output(), b"Private fixture greeting"));

        let config = RuntimeConfig::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database.caller_by_name(b"Message Caller").unwrap().unwrap();
        let actor = MessageActor::new(
            caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        let general = database.conference(actor, 1).unwrap();
        assert_eq!(database.last_read(actor, general.id).unwrap(), 1);
        assert_eq!(database.stats(actor).unwrap().sent, 4);
        let sysop = database.caller_by_name(b"Sysop").unwrap().unwrap();
        let sysop_actor = MessageActor::new(
            sysop.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        assert!(database
            .messages(sysop_actor, general.id)
            .unwrap()
            .iter()
            .any(|message| message.kind == MessageKind::SysopComment));
        let message_events = database
            .query_operational_events(&EventQuery {
                category: Some(EventCategory::Message),
                limit: Some(100),
                ..EventQuery::default()
            })
            .unwrap()
            .events;
        assert_eq!(
            message_events
                .iter()
                .filter(|event| event.event_code == "message.posted")
                .count(),
            4
        );
        let minimum_time = message_events
            .iter()
            .map(|event| event.occurred_at_utc)
            .min()
            .unwrap();
        let maximum_time = message_events
            .iter()
            .map(|event| event.occurred_at_utc)
            .max()
            .unwrap();
        let activity = database
            .message_activity(minimum_time, maximum_time)
            .unwrap();
        assert_eq!(
            activity
                .rows
                .iter()
                .map(|row| row.messages_posted)
                .sum::<u64>(),
            4
        );
        assert!(activity.rows.iter().any(|row| row.visibility == "private"));
        let projected = format!("{message_events:?}");
        for private in [
            "Private fixture greeting",
            "Private message content",
            "Recipient Caller",
            "test-only message password",
        ] {
            assert!(!projected.contains(private));
        }
    }

    #[test]
    fn canceled_invalid_and_interrupted_composition_never_leave_partial_messages() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Composer Caller",
            b"test-only composer password",
            CallerState::Active,
        );
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        for lines in [
            vec![
                b"N".to_vec(),
                b"Composer Caller".to_vec(),
                b"test-only composer password".to_vec(),
                b"M".to_vec(),
                b"E".to_vec(),
                b"/A".to_vec(),
                b"Q".to_vec(),
                b"G".to_vec(),
            ],
            vec![
                b"N".to_vec(),
                b"Composer Caller".to_vec(),
                b"test-only composer password".to_vec(),
                b"M".to_vec(),
                b"E".to_vec(),
                b"Missing Recipient".to_vec(),
                b"Q".to_vec(),
                b"G".to_vec(),
            ],
        ] {
            let mut terminal = InMemoryTerminal::with_lines(lines);
            runtime.run_connection(&mut terminal).unwrap();
            assert!(
                contains(terminal.output(), b"nothing was saved")
                    || contains(terminal.output(), b"does not exist")
            );
        }

        let mut interrupted = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Composer Caller".to_vec(),
            b"test-only composer password".to_vec(),
            b"M".to_vec(),
            b"E".to_vec(),
            b"".to_vec(),
            b"Interrupted post".to_vec(),
            b"unsaved body".to_vec(),
        ]);
        let report = runtime.run_connection(&mut interrupted).unwrap();
        assert!(matches!(
            report,
            ConnectionReport::Completed(RunReport {
                close_reason: SessionCloseReason::EndOfInput,
                ..
            })
        ));

        let config = RuntimeConfig::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database
            .caller_by_name(b"Composer Caller")
            .unwrap()
            .unwrap();
        let actor = MessageActor::new(
            caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        assert_eq!(database.stats(actor).unwrap().sent, 0);
    }

    #[test]
    fn syncterm_rlogin_handshake_can_auto_login_only_when_enabled() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"SyncTERM Caller",
            b"test-only syncterm password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![TransportConfig {
            name: Some("rlogin-auto-login".to_owned()),
            enabled: true,
            adapter: TransportAdapterConfig::Rlogin {
                listen: address,
                auto_login: true,
                terminal: NetworkTerminalDefaults::default(),
            },
        }];
        fs::write(&config_path, config.to_toml().unwrap()).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(1), shutdown).unwrap());

        let mut stream = connect_retry(address);
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(b"\0test-only syncterm password\0SyncTERM Caller\0ansi/38400\0M\rB\rQ\rG\r")
            .unwrap();
        let _ = stream.shutdown(Shutdown::Write);
        let mut transcript = Vec::new();
        stream.read_to_end(&mut transcript).unwrap();
        assert_eq!(handle.join().unwrap().completed_sessions, 1);
        assert!(contains(&transcript, b"Welcome, SyncTERM Caller"));
        assert!(contains(&transcript, b"Welcome to the fixture board"));
        assert!(!contains(&transcript, b"Are you a New Caller"));
        assert!(!contains(&transcript, b"test-only syncterm password"));
    }

    #[test]
    fn concurrent_transports_share_four_nodes_report_busy_and_reuse_release() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let telnet = available_address();
        let raw_primary = available_address();
        let rlogin = available_address();
        let raw_secondary = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.node = None;
        config.nodes = Some(NodePoolConfig {
            count: 4,
            overrides: Vec::new(),
        });
        let defaults = NetworkTerminalDefaults::default();
        config.transports = vec![
            listener_config(
                "telnet-primary",
                TransportAdapterConfig::Telnet {
                    listen: telnet,
                    terminal: defaults.clone(),
                },
            ),
            listener_config(
                "raw-primary",
                TransportAdapterConfig::Raw {
                    listen: raw_primary,
                    terminal: defaults.clone(),
                },
            ),
            listener_config(
                "rlogin-primary",
                TransportAdapterConfig::Rlogin {
                    listen: rlogin,
                    auto_login: false,
                    terminal: defaults.clone(),
                },
            ),
            listener_config(
                "raw-secondary",
                TransportAdapterConfig::Raw {
                    listen: raw_secondary,
                    terminal: defaults,
                },
            ),
        ];
        config.save_atomic(&config_path).unwrap();

        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(5), shutdown).unwrap());

        let mut clients = [
            connect_retry(telnet),
            connect_retry(raw_primary),
            connect_retry(rlogin),
            connect_retry(raw_secondary),
        ];
        clients[0]
            .write_all(&[
                255, 251, 24, 255, 250, 24, 0, b'A', b'N', b'S', b'I', 255, 240,
            ])
            .unwrap();
        clients[2]
            .write_all(b"\0untrusted\0untrusted\0ansi/9600\0")
            .unwrap();
        wait_for_node_count(&root, "login", 4);

        let mut busy = connect_retry(raw_primary);
        busy.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let _ = busy.shutdown(Shutdown::Write);
        let mut busy_output = Vec::new();
        busy.read_to_end(&mut busy_output).unwrap();
        assert!(contains(&busy_output, b"currently busy"));

        close_stream(&mut clients[0]);
        wait_for_node_count(&root, "waiting", 1);
        let mut replacement = connect_retry(raw_primary);
        wait_for_node_count(&root, "login", 4);

        for client in &mut clients[1..] {
            close_stream(client);
        }
        close_stream(&mut replacement);
        let report = handle.join().unwrap();
        assert_eq!(report.completed_sessions, 5);
        assert_eq!(report.listeners.len(), 4);
        assert_eq!(
            report
                .listeners
                .iter()
                .filter(|listener| listener.transport == TransportKind::RawTcp)
                .count(),
            2
        );
    }

    #[test]
    fn clean_setup_board_message_closure_persists_over_telnet_and_raw() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("message-closure-board");
        let mut plan = crate::SetupPlan::stock_defaults(
            "Message Closure Board",
            "Acceptance Sysop",
            "Sysop",
            2,
        );
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        crate::setup_board(&root, &plan, b"test-only closure sysop password").unwrap();
        seed_caller(
            &root,
            b"Closure Caller",
            b"test-only closure caller password",
            CallerState::Active,
        );
        let config_path = root.join(crate::BOARD_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        {
            let mut database = RuntimeDatabase::open(paths.database()).unwrap();
            database
                .ensure_conference(&sf_core::ConferenceDefinition {
                    number: 2,
                    name: "SPITFIRE".to_owned(),
                    description: "Message closure acceptance".to_owned(),
                    access_mode: sf_core::ConferenceAccessMode::AtLeast,
                    read_security: SecurityLevel::new(5).unwrap(),
                    post_security: SecurityLevel::new(5).unwrap(),
                    public_only: false,
                    caller_deletion_enabled: true,
                    maximum_lines: 50,
                    privileged_security_levels: Vec::new(),
                })
                .unwrap();
            let caller = database.caller_by_name(b"Closure Caller").unwrap().unwrap();
            let sysop = database.caller_by_name(b"Sysop").unwrap().unwrap();
            let caller_actor = MessageActor::new(
                caller.id,
                SecurityLevel::new(validated.caller.sysop_security).unwrap(),
            );
            let sysop_actor = MessageActor::new(
                sysop.id,
                SecurityLevel::new(validated.caller.sysop_security).unwrap(),
            );
            database.replace_queue(caller_actor, &[2]).unwrap();
            database.replace_queue(sysop_actor, &[2]).unwrap();
            let conference = database.conference(sysop_actor, 2).unwrap();
            database
                .post(
                    sysop_actor,
                    sf_core::NewMessage {
                        conference_id: conference.id,
                        recipient_caller_id: Some(caller.id),
                        recipient_name: caller.display_name,
                        subject: b"Closure Thread".to_vec(),
                        body: b"Stock original line\r\n".to_vec(),
                        created_at: 1,
                        parent_message_id: None,
                        visibility: sf_core::MessageVisibility::Private,
                        kind: MessageKind::Standard,
                    },
                )
                .unwrap();
        }

        let telnet_address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![listener_config(
            "message-closure-telnet",
            TransportAdapterConfig::Telnet {
                listen: telnet_address,
                terminal: NetworkTerminalDefaults::default(),
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(2), shutdown).unwrap());

        let mut telnet = connect_retry(telnet_address);
        telnet
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut script = vec![
            255, 251, 24, 255, 250, 24, 0, b'A', b'N', b'S', b'I', 255, 240, 255, 250, 31, 0, 80,
            0, 25, 255, 240,
        ];
        script.extend_from_slice(
            b"N\rClosure Caller\rtest-only closure caller password\rM\rC\r2\rR\rT\rR\rN\r\r\r",
        );
        script.extend_from_slice(&[0x11, b'\r']);
        script.extend_from_slice(
            b"1-1\rReply through Telnet\r/S\rY\rF\rS\rF\rE\rQ\rR\rA\rQ\rR\rO\rQ\rY\rR\r2/1\r\rS\r\rQ\rQ\rG\r",
        );
        telnet.write_all(&script).unwrap();
        let _ = telnet.shutdown(Shutdown::Write);
        let mut telnet_transcript = Vec::new();
        telnet.read_to_end(&mut telnet_transcript).unwrap();
        for expected in [
            b">>>> CONFERENCE SCAN MENU <<<<".as_slice(),
            b"Stock original line".as_slice(),
            b"S> Stock original line".as_slice(),
            b">>>> MESSAGE THREAD MENU <<<<".as_slice(),
            b"--- YOUR MESSAGES RECEIVED ---".as_slice(),
            b"--- YOUR MESSAGES SENT ---".as_slice(),
            b"Reply through Telnet".as_slice(),
        ] {
            assert!(
                contains(&telnet_transcript, expected),
                "missing {expected:?} in {}",
                String::from_utf8_lossy(&telnet_transcript)
            );
        }

        let mut reconnect = connect_retry(telnet_address);
        reconnect
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        reconnect
            .write_all(b"N\rClosure Caller\rtest-only closure caller password\rM\rY\rQ\rQ\rG\r")
            .unwrap();
        let _ = reconnect.shutdown(Shutdown::Write);
        let mut reconnect_transcript = Vec::new();
        reconnect.read_to_end(&mut reconnect_transcript).unwrap();
        assert_eq!(handle.join().unwrap().completed_sessions, 2);
        assert!(contains(
            &reconnect_transcript,
            b"Messages Already Received: 1"
        ));
        assert!(contains(&reconnect_transcript, b"Messages Sent: 1"));

        let raw_address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![listener_config(
            "message-closure-raw",
            TransportAdapterConfig::Raw {
                listen: raw_address,
                terminal: NetworkTerminalDefaults::default(),
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(1), shutdown).unwrap());
        let mut raw = connect_retry(raw_address);
        raw.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        raw.write_all(
            b"N\rClosure Caller\rtest-only closure caller password\rM\rY\rS\r2/2\r\rQ\rQ\rG\r",
        )
        .unwrap();
        let _ = raw.shutdown(Shutdown::Write);
        let mut raw_transcript = Vec::new();
        raw.read_to_end(&mut raw_transcript).unwrap();
        assert_eq!(handle.join().unwrap().completed_sessions, 1);
        assert!(contains(&raw_transcript, b"--- YOUR MESSAGES SENT ---"));
        assert!(contains(&raw_transcript, b"Reply through Telnet"));
    }

    #[test]
    fn clean_setup_board_file_presentation_persists_over_telnet_and_raw() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("file-presentation-board");
        let mut plan = crate::SetupPlan::stock_defaults(
            "File Presentation Board",
            "Acceptance Sysop",
            "Sysop",
            2,
        );
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        crate::setup_board(&root, &plan, b"test-only file presentation sysop password").unwrap();
        seed_caller(
            &root,
            b"Presentation Caller",
            b"test-only presentation caller password",
            CallerState::Active,
        );
        let config_path = root.join(crate::BOARD_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        // 2026-08-21 12:00:00 in the setup board's America/Phoenix timezone.
        let uploaded_at = 1_787_338_800;
        {
            let mut database = RuntimeDatabase::open(paths.database()).unwrap();
            let caller = database
                .caller_by_name(b"Presentation Caller")
                .unwrap()
                .unwrap();
            let actor = FileActor::new(
                caller.id,
                SecurityLevel::new(validated.caller.sysop_security).unwrap(),
            );
            let area = database.file_area(actor, 1).unwrap().0;
            sf_core::FileStorage::new(&paths)
                .unwrap()
                .write_seed_file(
                    &mut database,
                    &area,
                    "PRESENT.TXT",
                    "Stock brief description\r\nExtended presentation detail",
                    b"file presentation acceptance\r\n",
                    uploaded_at,
                )
                .unwrap();
        }

        let telnet_address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![listener_config(
            "file-presentation-telnet",
            TransportAdapterConfig::Telnet {
                listen: telnet_address,
                terminal: NetworkTerminalDefaults::default(),
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(2), shutdown).unwrap());

        let mut telnet = connect_retry(telnet_address);
        telnet
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut script = vec![
            255, 251, 24, 255, 250, 24, 0, b'A', b'N', b'S', b'I', 255, 240, 255, 250, 31, 0, 80,
            0, 25, 255, 240,
        ];
        script.extend_from_slice(
            b"N\rPresentation Caller\rtest-only presentation caller password\rF\rL\r\rN\r\r08-21-26\r\rQ\rG\r",
        );
        telnet.write_all(&script).unwrap();
        let _ = telnet.shutdown(Shutdown::Write);
        let mut telnet_transcript = Vec::new();
        telnet.read_to_end(&mut telnet_transcript).unwrap();
        for expected in [
            b"PRESENT.TXT".as_slice(),
            b"08-21-26".as_slice(),
            b"Extended presentation detail".as_slice(),
            b"New files since last checked:".as_slice(),
            b"Total downloadable files:".as_slice(),
            b"Total downloadable bytes:".as_slice(),
        ] {
            assert!(
                contains(&telnet_transcript, expected),
                "missing {expected:?} in {}",
                String::from_utf8_lossy(&telnet_transcript)
            );
        }

        let mut reconnect = connect_retry(telnet_address);
        reconnect
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        reconnect
            .write_all(
                b"N\rPresentation Caller\rtest-only presentation caller password\rF\rN\r\rL\r\rQ\rG\r",
            )
            .unwrap();
        let _ = reconnect.shutdown(Shutdown::Write);
        let mut reconnect_transcript = Vec::new();
        reconnect.read_to_end(&mut reconnect_transcript).unwrap();
        assert_eq!(handle.join().unwrap().completed_sessions, 2);
        assert!(contains(
            &reconnect_transcript,
            b"No matching files are available."
        ));
        let first_checkpoint = {
            let database = RuntimeDatabase::open(paths.database()).unwrap();
            let caller = database
                .caller_by_name(b"Presentation Caller")
                .unwrap()
                .unwrap();
            let actor = FileActor::new(
                caller.id,
                SecurityLevel::new(validated.caller.sysop_security).unwrap(),
            );
            database.new_file_checkpoint(actor).unwrap().unwrap()
        };

        let raw_address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![listener_config(
            "file-presentation-raw",
            TransportAdapterConfig::Raw {
                listen: raw_address,
                terminal: NetworkTerminalDefaults::default(),
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(1), shutdown).unwrap());
        let mut raw = connect_retry(raw_address);
        raw.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        raw.write_all(
            b"N\rPresentation Caller\rtest-only presentation caller password\rF\rL\r\rN\rC\r08-21-2026\r\rQ\rG\r",
        )
        .unwrap();
        let _ = raw.shutdown(Shutdown::Write);
        let mut raw_transcript = Vec::new();
        raw.read_to_end(&mut raw_transcript).unwrap();
        assert_eq!(handle.join().unwrap().completed_sessions, 1);
        assert!(contains(&raw_transcript, b"PRESENT.TXT"));
        assert!(contains(&raw_transcript, b"08-21-26"));
        assert!(contains(&raw_transcript, b"Extended presentation detail"));

        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let caller = database
            .caller_by_name(b"Presentation Caller")
            .unwrap()
            .unwrap();
        let actor = FileActor::new(
            caller.id,
            SecurityLevel::new(validated.caller.sysop_security).unwrap(),
        );
        assert!(database.new_file_checkpoint(actor).unwrap().unwrap() >= first_checkpoint);
    }

    #[test]
    fn clean_setup_board_sysop_boundary_is_shared_by_telnet_and_raw() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("resource-navigation-board");
        let mut plan = crate::SetupPlan::stock_defaults(
            "Resource Navigation Board",
            "Acceptance Sysop",
            "Sysop",
            2,
        );
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        crate::setup_board(&root, &plan, b"test-only navigation sysop password").unwrap();
        fs::write(
            root.join("system/SFSYSOP.MNU"),
            b"V,<V>........... View Log Files,,50,G\r\nQ,<Q>........ Quit To Main Menu,,50,C\r\nX,<X>........ Xpert Mode Toggle,,50,B\r\nG,<G>........ Goodbye & Log Off,,50,A\r\n",
        )
        .unwrap();
        let config_path = root.join(crate::BOARD_CONFIG_FILE);

        let telnet_address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![listener_config(
            "navigation-telnet",
            TransportAdapterConfig::Telnet {
                listen: telnet_address,
                terminal: NetworkTerminalDefaults::default(),
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(1), shutdown).unwrap());
        let mut telnet = connect_retry(telnet_address);
        telnet
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        telnet
            .write_all(&[
                255, 251, 24, 255, 250, 24, 0, b'A', b'N', b'S', b'I', 255, 240, 255, 250, 31, 0,
                80, 0, 25, 255, 240,
            ])
            .unwrap();
        telnet
            .write_all(
                b"N\rSysop\rtest-only navigation sysop password\r?\r@\r@\rV\rQ\rM\r@\rQ\rF\r@\rX\rQ\rG\r",
            )
            .unwrap();
        let _ = telnet.shutdown(Shutdown::Write);
        let mut telnet_transcript = Vec::new();
        telnet.read_to_end(&mut telnet_transcript).unwrap();
        assert_eq!(handle.join().unwrap().completed_sessions, 1);
        assert!(contains(&telnet_transcript, b">>>>>>>> SYSOP MENU"));
        assert!(contains(
            &telnet_transcript,
            b"Enters the security-controlled Sysop Utilities menu."
        ));
        assert!(contains(
            &telnet_transcript,
            b"View Log Files is not available in this SPITFIRE NG capability set."
        ));
        assert!(contains(&telnet_transcript, b"Xpert command mode is ON."));
        assert!(contains(&telnet_transcript, b"Thank you for calling"));

        let raw_address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.transports = vec![listener_config(
            "navigation-raw",
            TransportAdapterConfig::Raw {
                listen: raw_address,
                terminal: NetworkTerminalDefaults {
                    ansi: false,
                    ..NetworkTerminalDefaults::default()
                },
            },
        )];
        config.save_atomic(&config_path).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(1), shutdown).unwrap());
        let mut raw = connect_retry(raw_address);
        raw.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        raw.write_all(b"N\rSysop\rtest-only navigation sysop password\r@\rQ\rG\r")
            .unwrap();
        let _ = raw.shutdown(Shutdown::Write);
        let mut raw_transcript = Vec::new();
        raw.read_to_end(&mut raw_transcript).unwrap();
        assert_eq!(handle.join().unwrap().completed_sessions, 1);
        assert!(
            contains(&raw_transcript, b">>>>>>>> SYSOP MENU"),
            "{}",
            String::from_utf8_lossy(&raw_transcript)
        );
        assert!(!contains(&raw_transcript, b"\x1b["));
        assert!(
            contains(&raw_transcript, b"MAIN MENU - Selection?"),
            "{}",
            String::from_utf8_lossy(&raw_transcript)
        );
    }

    #[test]
    fn sysop_navigation_is_security_filtered_and_session_local() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Ordinary Caller",
            b"test-only ordinary password",
            CallerState::Active,
        );
        fs::remove_file(
            root.join("system/presentation-profiles/modern-ng/resources/display/SOP50.BBS"),
        )
        .unwrap();
        fs::remove_file(
            root.join("system/presentation-profiles/modern-ng/resources/display/SOP50.CLR"),
        )
        .unwrap();
        let runtime = BoardRuntime::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        runtime
            .initialize_sysop(b"test-only navigation sysop password")
            .unwrap();

        let mut sysop = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Sysop".to_vec(),
            b"test-only navigation sysop password".to_vec(),
            b"@".to_vec(),
            b"X".to_vec(),
            b"Q".to_vec(),
            b"G".to_vec(),
        ]);
        assert!(matches!(
            runtime.run_connection(&mut sysop).unwrap(),
            ConnectionReport::Completed(_)
        ));
        assert!(contains(sysop.output(), b"SYSOP MENU"));
        assert!(contains(sysop.output(), b"<Q>......... Quit to Main Menu"));

        let mut caller = InMemoryTerminal::with_lines([
            b"N".to_vec(),
            b"Ordinary Caller".to_vec(),
            b"test-only ordinary password".to_vec(),
            b"@".to_vec(),
            b"G".to_vec(),
        ]);
        assert!(matches!(
            runtime.run_connection(&mut caller).unwrap(),
            ConnectionReport::Completed(_)
        ));
        assert!(contains(
            caller.output(),
            b"Invalid or unavailable selection."
        ));
        assert!(!contains(caller.output(), b"SYSOP MENU"));
        assert!(contains(caller.output(), b">>>>>>>> MAIN MENU"));
    }

    fn run_loopback(kind: TransportKind) -> Vec<u8> {
        run_loopback_with_selection(kind, None)
    }

    fn run_loopback_with_selection(
        kind: TransportKind,
        presentation: Option<sf_core::PresentationConfig>,
    ) -> Vec<u8> {
        run_loopback_with_selection_and_journey(kind, presentation, false)
    }

    fn run_loopback_with_selection_and_journey(
        kind: TransportKind,
        presentation: Option<sf_core::PresentationConfig>,
        stock_post_login: bool,
    ) -> Vec<u8> {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        seed_caller(
            &root,
            b"Loopback Caller",
            b"test-only-loopback-password",
            CallerState::Active,
        );
        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let mut public_database = RuntimeDatabase::open(paths.database()).unwrap();
        public_database
            .update_public_directory_policy(
                sf_core::PublicInformationActor::LocalOperator,
                1,
                true,
                true,
                false,
                false,
                1_700_000_010,
            )
            .unwrap();
        public_database
            .add_other_bbs(
                sf_core::PublicInformationActor::LocalOperator,
                sf_core::NewOtherBbsEntry {
                    name: "Loopback Fixture BBS".to_owned(),
                    speed: "SSH".to_owned(),
                    dial_string: "loopback.example:2222".to_owned(),
                },
                1_700_000_011,
            )
            .unwrap();
        drop(public_database);
        let address = available_address();
        let mut config = RuntimeConfig::load(&config_path).unwrap();
        if let Some(presentation) = presentation {
            config.presentation = presentation;
        }
        if stock_post_login {
            config.caller.post_login_journey = PostLoginJourney::Stock;
        }
        let mut defaults = NetworkTerminalDefaults::default();
        if kind == TransportKind::RawTcp {
            defaults.ansi = false;
        }
        config.transports = vec![TransportConfig {
            name: Some(format!("{kind:?}-loopback")),
            enabled: true,
            adapter: match kind {
                TransportKind::RawTcp => TransportAdapterConfig::Raw {
                    listen: address,
                    terminal: defaults,
                },
                TransportKind::Telnet => TransportAdapterConfig::Telnet {
                    listen: address,
                    terminal: defaults,
                },
                TransportKind::Rlogin => TransportAdapterConfig::Rlogin {
                    listen: address,
                    auto_login: false,
                    terminal: defaults,
                },
                _ => unreachable!(),
            },
        }];
        fs::write(&config_path, config.to_toml().unwrap()).unwrap();
        let thread_config = config_path.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle =
            thread::spawn(move || serve_with_shutdown(&thread_config, Some(1), shutdown).unwrap());

        let mut stream = connect_retry(address);
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        match kind {
            TransportKind::Telnet => stream
                .write_all(&[
                    255, 251, 24, 255, 250, 24, 0, b'A', b'N', b'S', b'I', 255, 240, 255, 250, 31,
                    0, 80, 0, 25, 255, 240,
                ])
                .unwrap(),
            TransportKind::Rlogin => stream
                .write_all(b"\0untrusted-user\0ignored-local-user\0ansi/9600\0")
                .unwrap(),
            _ => {}
        }
        let input = if stock_post_login {
            b"N\rLoopback Caller\rtest-only-loopback-password\rN\r#\rY\rY\rL\rloop\rY\rO\rA\rB\r1\rN\rT\rM\rB\rF\rQ\rG\r".as_slice()
        } else {
            b"N\rLoopback Caller\rtest-only-loopback-password\r#\rY\rY\rL\rloop\rY\rO\rA\rB\r1\rN\rT\rM\rB\rF\rQ\rG\r".as_slice()
        };
        stream.write_all(input).unwrap();
        let _ = stream.shutdown(Shutdown::Write);
        let mut transcript = Vec::new();
        stream.read_to_end(&mut transcript).unwrap();
        let report = handle.join().unwrap();
        assert_eq!(report.completed_sessions, 1);
        transcript
    }

    #[test]
    #[ignore = "manual Qodem, SyncTERM, and macOS OpenSSH Tranche 5 acceptance server"]
    fn tranche_5_real_client_acceptance_server() {
        use std::io::Cursor;

        let root = PathBuf::from(
            std::env::var("SPITFIRE_TRANCHE5_ACCEPTANCE_ROOT")
                .expect("set SPITFIRE_TRANCHE5_ACCEPTANCE_ROOT to a new disposable directory"),
        );
        let telnet_address: SocketAddr = std::env::var("SPITFIRE_TRANCHE5_TELNET")
            .unwrap_or_else(|_| "127.0.0.1:24231".to_owned())
            .parse()
            .unwrap();
        let ssh_address: SocketAddr = std::env::var("SPITFIRE_TRANCHE5_SSH")
            .unwrap_or_else(|_| "127.0.0.1:24232".to_owned())
            .parse()
            .unwrap();
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        let password = b"test-only tranche-five password";
        for caller in [
            b"Qodem Acceptance".as_slice(),
            b"SyncTERM Acceptance".as_slice(),
            b"OpenSSH Acceptance".as_slice(),
        ] {
            seed_caller(&root, caller, password, CallerState::Active);
        }

        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let storage = FileStorage::new(&paths).unwrap();
        let operator = database
            .caller_by_name(b"Qodem Acceptance")
            .unwrap()
            .unwrap();
        let actor = FileActor::new(operator.id, SecurityLevel::new(10).unwrap());
        let area = database.file_area(actor, 1).unwrap().0;

        let mut zip_writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip_writer
            .start_file(
                "docs/readme.txt",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        zip_writer
            .write_all(b"Synthetic real-client archive member")
            .unwrap();
        let zip_bytes = zip_writer.finish().unwrap().into_inner();
        storage
            .write_seed_file(
                &mut database,
                &area,
                "CLIENT.ZIP",
                "Synthetic real-client ZIP",
                &zip_bytes,
                1_777_000_000,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &area,
                "BINARY.DAT",
                "Synthetic binary rejection",
                b"binary\0fixture",
                1_777_000_001,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &area,
                "CONTROL.TXT",
                "Synthetic terminal-control rejection",
                b"safe prefix\x1b[2Junsafe suffix",
                1_777_000_002,
            )
            .unwrap();
        let offline = storage
            .write_seed_file(
                &mut database,
                &area,
                "OFFLINE.ZIP",
                "Synthetic unavailable request",
                &zip_bytes,
                1_777_000_003,
            )
            .unwrap();
        database
            .set_file_lifecycle(
                sf_core::FileAdminActor::LocalOperator,
                offline.id,
                offline.state_version,
                sf_core::FileLifecycle::Offline,
            )
            .unwrap();
        let preview_area = database
            .create_file_area(&sf_core::FileAreaDefinition {
                number: 3,
                name: "Preview Files".to_owned(),
                description: "Synthetic inspect-only acceptance area".to_owned(),
                storage_key: "preview".to_owned(),
                access_mode: sf_core::FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(50).unwrap(),
                upload_security: SecurityLevel::new(50).unwrap(),
                preview: true,
                no_charge: false,
                maximum_upload_bytes: 1024 * 1024,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        storage.ensure_area(&preview_area).unwrap();
        storage
            .write_seed_file(
                &mut database,
                &preview_area,
                "PREVIEW.TXT",
                "Synthetic Preview-area text",
                b"Preview inspection succeeds without transfer authority.\r\n",
                1_777_000_004,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &preview_area,
                "PREVIEW.ZIP",
                "Synthetic Preview-area archive",
                &zip_bytes,
                1_777_000_005,
            )
            .unwrap();
        drop(database);

        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.node = None;
        config.nodes = Some(NodePoolConfig {
            count: 3,
            overrides: Vec::new(),
        });
        config.transports = vec![
            listener_config(
                "tranche5-telnet",
                TransportAdapterConfig::Telnet {
                    listen: telnet_address,
                    terminal: NetworkTerminalDefaults::default(),
                },
            ),
            listener_config(
                "tranche5-ssh",
                TransportAdapterConfig::Ssh {
                    listen: ssh_address,
                    host_key: PathBuf::from("ssh/host-ed25519"),
                    terminal: NetworkTerminalDefaults::default(),
                    maximum_unauthenticated_connections: 4,
                    maximum_authentication_attempts: 3,
                    handshake_timeout_seconds: 10,
                },
            ),
        ];
        config.save_atomic(&config_path).unwrap();
        println!("TRANCHE5_ACCEPTANCE_READY root={}", root.display());
        println!("TELNET={telnet_address} SSH={ssh_address}");
        println!("PASSWORD=test-only tranche-five password");
        let report =
            serve_with_shutdown(&config_path, Some(3), Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(report.completed_sessions, 3);

        let database = RuntimeDatabase::open(paths.database()).unwrap();
        let requests = database
            .pending_file_requests(sf_core::FileAdminActor::LocalOperator)
            .unwrap();
        assert_eq!(requests.len(), 3);
        println!("TRANCHE5_ACCEPTANCE_COMPLETE requests={}", requests.len());
    }

    #[test]
    #[ignore = "manual Qodem, SyncTERM, and macOS OpenSSH Tranche 6 interoperability server"]
    fn tranche_6_real_client_acceptance_server() {
        let root = PathBuf::from(
            std::env::var("SPITFIRE_TRANCHE6_ACCEPTANCE_ROOT")
                .expect("set SPITFIRE_TRANCHE6_ACCEPTANCE_ROOT to a new disposable directory"),
        );
        let telnet_address: SocketAddr = std::env::var("SPITFIRE_TRANCHE6_TELNET")
            .unwrap_or_else(|_| "127.0.0.1:24241".to_owned())
            .parse()
            .unwrap();
        let ssh_address: SocketAddr = std::env::var("SPITFIRE_TRANCHE6_SSH")
            .unwrap_or_else(|_| "127.0.0.1:24242".to_owned())
            .parse()
            .unwrap();
        let raw_address: SocketAddr = std::env::var("SPITFIRE_TRANCHE6_RAW")
            .unwrap_or_else(|_| "127.0.0.1:24243".to_owned())
            .parse()
            .unwrap();
        let maximum_sessions = std::env::var("SPITFIRE_TRANCHE6_MAX_SESSIONS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(3);
        initialize_fixture_board(&root).unwrap();
        use_fast_test_hashing(&root);
        let password = b"test-only tranche-six password";
        for caller in [
            b"Qodem Transfer".as_slice(),
            b"SyncTERM Transfer".as_slice(),
            b"OpenSSH Transfer".as_slice(),
            b"Binkley TeLink".as_slice(),
        ] {
            seed_caller(&root, caller, password, CallerState::Active);
        }
        seed_caller(&root, b"BT", b"TELINKTEST", CallerState::Active);

        let config_path = root.join(FIXTURE_CONFIG_FILE);
        let validated = RuntimeConfig::load(&config_path)
            .unwrap()
            .validate()
            .unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        let storage = FileStorage::new(&paths).unwrap();
        let operator = database.caller_by_name(b"Qodem Transfer").unwrap().unwrap();
        let actor = FileActor::new(operator.id, SecurityLevel::new(10).unwrap());
        let area = database.file_area(actor, 1).unwrap().0;
        for (name, description, bytes, timestamp) in [
            (
                "ONE.BIN",
                "Synthetic first batch member",
                (0_u8..=255).collect::<Vec<_>>(),
                1_777_100_000,
            ),
            (
                "TWO.BIN",
                "Synthetic second batch member",
                (0_u8..=255).rev().cycle().take(2049).collect::<Vec<_>>(),
                1_777_100_001,
            ),
        ] {
            storage
                .write_seed_file(&mut database, &area, name, description, &bytes, timestamp)
                .unwrap();
        }
        let external_bytes = (0_u8..=255).cycle().take(128 * 1024).collect::<Vec<_>>();
        let external_file = storage
            .write_seed_file(
                &mut database,
                &area,
                "EXTERNAL.BIN",
                "Synthetic read-only external member",
                &external_bytes,
                1_777_100_003,
            )
            .unwrap();
        let external_root_path = root.join("test-external-read-only");
        fs::create_dir(&external_root_path).unwrap();
        fs::write(external_root_path.join("EXTERNAL.BIN"), &external_bytes).unwrap();
        let external_root = database
            .add_storage_root(
                actor,
                sf_core::StorageRootDefinition {
                    area_id: area.id,
                    stable_key: "acceptance-read-only",
                    label: "Acceptance Read Only",
                    configured_locator: external_root_path.to_str().unwrap(),
                    priority: 1,
                    mode: sf_core::StorageRootMode::ReadOnly,
                    occurred_at: 1_777_100_004,
                },
            )
            .unwrap();
        database
            .set_storage_availability(
                actor,
                external_root.id,
                external_root.state_version,
                sf_core::StorageAvailability::Available,
                1_777_100_005,
            )
            .unwrap();
        database
            .set_file_storage_locator(
                actor,
                external_file.id,
                external_root.id,
                "EXTERNAL.BIN",
                external_file.state_version,
                1,
                1_777_100_006,
            )
            .unwrap();
        drop(database);

        let upload_directory = root.join("client-upload");
        fs::create_dir(&upload_directory).unwrap();
        fs::write(
            upload_directory.join("UPLOAD1.BIN"),
            (0_u8..=255).cycle().take(1025).collect::<Vec<_>>(),
        )
        .unwrap();
        fs::write(
            upload_directory.join("UPLOAD2.BIN"),
            b"synthetic second upload member",
        )
        .unwrap();

        let mut config = RuntimeConfig::load(&config_path).unwrap();
        config.node = None;
        config.nodes = Some(NodePoolConfig {
            count: 4,
            overrides: Vec::new(),
        });
        config.transports = vec![
            listener_config(
                "tranche6-telnet",
                TransportAdapterConfig::Telnet {
                    listen: telnet_address,
                    terminal: NetworkTerminalDefaults::default(),
                },
            ),
            listener_config(
                "tranche6-ssh",
                TransportAdapterConfig::Ssh {
                    listen: ssh_address,
                    host_key: PathBuf::from("ssh/host-ed25519"),
                    terminal: NetworkTerminalDefaults::default(),
                    maximum_unauthenticated_connections: 4,
                    maximum_authentication_attempts: 3,
                    handshake_timeout_seconds: 10,
                },
            ),
            listener_config(
                "tranche6-raw",
                TransportAdapterConfig::Raw {
                    listen: raw_address,
                    terminal: NetworkTerminalDefaults::default(),
                },
            ),
        ];
        config.save_atomic(&config_path).unwrap();
        println!("TRANCHE6_ACCEPTANCE_READY root={}", root.display());
        println!("TELNET={telnet_address} SSH={ssh_address} RAW={raw_address}");
        println!("PASSWORD=test-only tranche-six password");
        println!("UPLOAD_DIRECTORY={}", upload_directory.display());
        let report = serve_with_shutdown(
            &config_path,
            Some(maximum_sessions),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(report.completed_sessions, maximum_sessions);
        println!(
            "TRANCHE6_ACCEPTANCE_COMPLETE sessions={}",
            report.completed_sessions
        );
    }

    fn available_address() -> SocketAddr {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.local_addr().unwrap()
    }

    fn listener_config(name: &str, adapter: TransportAdapterConfig) -> TransportConfig {
        TransportConfig {
            name: Some(name.to_owned()),
            enabled: true,
            adapter,
        }
    }

    fn close_stream(stream: &mut TcpStream) {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let _ = stream.shutdown(Shutdown::Write);
        let mut output = Vec::new();
        stream.read_to_end(&mut output).unwrap();
    }

    fn wait_for_node_count(root: &Path, expected_state: &str, expected_count: usize) {
        let path = root.join("work").join(RUNTIME_STATUS_FILE);
        let started = Instant::now();
        loop {
            if let Ok(input) = fs::read_to_string(&path) {
                if let Ok(status) = toml::from_str::<crate::RuntimeStatusDocument>(&input) {
                    if status
                        .nodes
                        .iter()
                        .filter(|node| node.state == expected_state)
                        .count()
                        >= expected_count
                    {
                        return;
                    }
                }
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "timed out waiting for {expected_count} nodes in {expected_state}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn connect_retry(address: SocketAddr) -> TcpStream {
        let started = Instant::now();
        loop {
            match TcpStream::connect(address) {
                Ok(stream) => return stream,
                Err(error) if started.elapsed() < Duration::from_secs(5) => {
                    let _ = error;
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("could not connect to loopback listener: {error}"),
            }
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> usize {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
            .unwrap_or_else(|| panic!("could not find {:?}", String::from_utf8_lossy(needle)))
    }

    struct CapturedLogWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedLogWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn use_fast_test_hashing(root: &Path) {
        let path = root.join(FIXTURE_CONFIG_FILE);
        let mut config = RuntimeConfig::load(&path).unwrap();
        config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        fs::write(path, config.to_toml().unwrap()).unwrap();
    }

    fn seed_caller(root: &Path, name: &[u8], password: &[u8], state: CallerState) {
        let config = RuntimeConfig::load(&root.join(FIXTURE_CONFIG_FILE)).unwrap();
        let validated = config.validate().unwrap();
        let paths = LogicalPaths::resolve(root, &validated).unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        database.migrate().unwrap();
        let hasher = CredentialHasher::new(&validated.caller.password).unwrap();
        let encoded = hasher.hash(password).unwrap();
        let caller = database
            .create_caller(
                name,
                &encoded,
                SecurityLevel::new(10).unwrap(),
                state,
                false,
                1,
            )
            .unwrap();
        let mut preferences = caller.preferences;
        preferences.transfer_protocol = sf_core::TransferPreference::Ascii;
        database
            .update_caller_preferences(caller.id, preferences)
            .unwrap();
    }

    fn run_registration_policy_case<const N: usize>(
        policy: sf_core::CallerProfilePolicy,
        name: &[u8],
        profile_inputs: [&[u8]; N],
    ) -> (Vec<u8>, Caller) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("registration-policy-board");
        let mut plan = crate::SetupPlan::stock_defaults("Policy Board", "Sysop", "Sysop", 1);
        plan.config.caller.password = PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        plan.config.caller.profile = policy;
        crate::setup_board(&root, &plan, b"test-only policy sysop password").unwrap();
        let runtime = BoardRuntime::load(&root.join(crate::BOARD_CONFIG_FILE)).unwrap();
        let password = b"test-only policy caller password";
        let mut lines = vec![
            b"Y".to_vec(),
            name.to_vec(),
            password.to_vec(),
            password.to_vec(),
        ];
        lines.extend(profile_inputs.into_iter().map(<[u8]>::to_vec));
        lines.push(b"G".to_vec());
        let mut terminal = InMemoryTerminal::with_lines(lines);

        let report = runtime.run_connection(&mut terminal).unwrap();

        let ConnectionReport::Completed(report) = report else {
            panic!("registration unexpectedly found every node busy");
        };
        assert_eq!(report.close_reason, SessionCloseReason::Goodbye);
        assert_eq!(
            report.caller_name.as_deref(),
            Some(String::from_utf8_lossy(name).as_ref())
        );
        assert!(contains(
            terminal.output(),
            b"New caller registration complete."
        ));
        let caller = runtime.caller(name).unwrap();
        (terminal.output().to_vec(), caller)
    }

    #[cfg(unix)]
    fn read_until(port: &mut serialport::TTYPort, marker: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !contains(&output, marker) {
            match port.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => output.extend_from_slice(&buffer[..count]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::UnexpectedEof
                    ) =>
                {
                    break;
                }
                Err(error) => panic!("could not read synthetic serial transcript: {error}"),
            }
        }
        output
    }
}
