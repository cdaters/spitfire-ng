//! Portable native runtime foundation for SPITFIRE NG.
//!
//! Historical binary parsing remains in `sf-legacy`; this crate owns modern
//! board, node, session, configuration, logical-path, and operational-store
//! behavior.

pub mod board;
pub mod caller;
pub mod caller_access;
pub mod config;
pub mod credentials;
pub mod database;
pub mod file;
pub mod file_maintenance;
mod file_session;
pub mod interaction;
pub mod localization;
pub mod message;
mod message_session;
pub mod node;
pub mod paths;
pub mod public_information;
pub mod resources;
pub mod session;
pub mod terminal;
pub mod transfer;
pub mod transfer_runtime;

/// Authoritative runtime version sourced from Cargo package metadata.
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use board::{BoardIdentity, BoardIdentityError};
pub use caller::{
    board_local_day, canonicalize_caller_name, canonicalize_login_identifier,
    daily_session_elapsed_seconds, derive_login_identifier_base, format_board_local_timestamp,
    parse_birth_date, AccessDenialReason, AuthenticatedCaller, Caller, CallerAccessDenial,
    CallerError, CallerId, CallerPreferences, CallerProfile, CallerSessionContext, CallerState,
    GraphicsPreference, PostalAddress, SecurityLevel, SessionAllowance, TimePolicy,
    TransferPreference, MAX_CALLER_NAME_BYTES, MAX_LOGIN_IDENTIFIER_BYTES, MAX_SECURITY_LEVEL,
};
pub use caller_access::{
    CallerAccessActor, JokerError, JokerPolicy, JokerRuleKind, SubscriptionEvaluation,
    MAX_JOKER_BYTES, MAX_JOKER_LINE_BYTES, MAX_JOKER_RULES,
};
pub use config::{
    BoardAccessMode, BoardConfig, CallerConfig, CallerProfilePolicy, ConfigError, LanguageConfig,
    LegacyNodeConfig, MenuPresentationMode, NetworkTerminalDefaults, NodeOverrideConfig,
    NodePoolConfig, PasswordHashConfig, PathConfig, PostLoginJourney, PresentationConfig,
    PresentationMode, ProfileFieldPolicy, RuntimeConfig, SecurityLimitConfig, StorageConfig,
    SubscriptionConfig, TransportAdapterConfig, TransportConfig, ValidatedConfig,
    CONFIG_FORMAT_VERSION,
};
pub use credentials::{CredentialError, CredentialHasher, CREDENTIAL_SCHEME};
pub use database::{
    AuthenticationResult, DatabaseError, MigrationReport, RuntimeDatabase, SCHEMA_VERSION,
};
pub use file::{
    normalize_filename, AsciiTransfer, FileAccess, FileAccessMode, FileActor, FileArea,
    FileAreaDefinition, FileAreaId, FileBackend, FileEntry, FileError, FileId, FileIntegrity,
    FileLifecycle, FileSearch, FileStatistics, FileStorage, FileTransfer, LogicalFileStorageRoot,
    NewFileEntry, StagedUpload, StorageRootAccess, TransferDirection, TransferReport,
    MAX_FILE_AREAS, MAX_FILE_DESCRIPTION_BYTES, MAX_FILE_DESCRIPTION_LINES, MAX_FILE_NAME_BYTES,
};
pub use file_maintenance::*;
pub use interaction::{
    CallerChat, InteractionError, InteractionHub, OperatorChat, PageAnswer, PageRequest, PageState,
    PageTicket, SysopAvailability,
};
pub use localization::*;
pub use message::{
    Conference, ConferenceAccessMode, ConferenceDefinition, ConferenceId, CopyRecipient, Message,
    MessageActor, MessageBackend, MessageCallerSearchDirection, MessageDeliveryRole,
    MessageDiscoveryMatch, MessageDiscoveryQuery, MessageDiscoveryResult, MessageError, MessageId,
    MessageKind, MessageLifecycle, MessageMutationCapabilities, MessageMutationStorageStats,
    MessageRecipient, MessageStats, MessageSummary, MessageVisibility, NewMessage, MAX_CONFERENCES,
    MAX_MESSAGE_BODY_BYTES, MAX_MESSAGE_CC_RECIPIENTS, MAX_MESSAGE_LINES,
    MAX_MESSAGE_SEARCH_CANDIDATES, MAX_MESSAGE_SEARCH_RESULTS, MAX_MESSAGE_SEARCH_TERMS,
    MAX_MESSAGE_SEARCH_TERM_BYTES, MAX_MESSAGE_SUBJECT_BYTES,
};
pub use node::{
    MenuRendererPath, NodeChangeHook, NodeDefinition, NodeError, NodeId, NodeLease, NodeManager,
    NodePresentationContext, NodeRuntimeState, NodeSnapshot,
};
pub use paths::{LogicalPath, LogicalPaths, PathError};
pub use public_information::*;
pub use resources::{
    render_display, render_generated_menu, visible_menu_action_count, DisplayCallerContext,
    DisplayContext, DisplayFormat, DisplayResource, DisplaySource, HelpRecord, MenuDefinition,
    MenuItem, MenuSection, ResourceError, StockResources,
};
pub use session::{
    run_stock_session, Session, SessionCloseReason, SessionError, SessionId, SessionOutcome,
    SessionState, SessionStatusObserver, StockSessionContext,
};
pub use terminal::{
    InMemoryTerminal, PagingTerminal, SuppliedCredentials, Terminal, TerminalCapabilities,
    TerminalError, TerminalInfo, TerminalSize, TransportIdentity, TransportKind,
    VerifiedCallerGrant,
};
pub use transfer::{
    receive_binary_files, send_binary_files, send_binary_streams, ProtocolFile, ProtocolStreamFile,
    ReceivedProtocolFile, TransferProtocol, TransferProtocolError,
};
pub use transfer_runtime::{
    ActiveTransferSummary, DailyTransferUsage, FileStorageLocator, LegacyDailyLimitDocument,
    LegacyDailyLimitRecord, LegacyExtendedStorageDocument, QueuedFile, QuotaReservation,
    ReservationId, StorageAvailability, StorageRoot, StorageRootDefinition, StorageRootId,
    StorageRootKind, StorageRootMode, StorageRootState, TransferCancelSource,
    TransferDirectionKind, TransferId, TransferMethod, TransferPolicy, TransferQueue,
    TransferRuntimeError, TransferRuntimeState, TransferStateChange, UploadCreditRequest,
    ALL_PROTOCOLS_MASK, MAX_BATCH_QUEUE_BYTES, MAX_BATCH_QUEUE_ITEMS, MAX_LEGACY_POLICY_BYTES,
    MAX_LEGACY_POLICY_LINES, MAX_LEGACY_STORAGE_ROOTS,
};
