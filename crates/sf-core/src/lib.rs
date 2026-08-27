//! Portable native runtime foundation for SPITFIRE NG.
//!
//! Historical binary parsing remains in `sf-legacy`; this crate owns modern
//! board, node, session, configuration, logical-path, and operational-store
//! behavior.

pub mod board;
pub mod caller;
pub mod config;
pub mod credentials;
pub mod database;
pub mod file;
mod file_session;
pub mod interaction;
pub mod localization;
pub mod message;
mod message_session;
pub mod node;
pub mod paths;
pub mod resources;
pub mod session;
pub mod terminal;
pub mod transfer;

/// Authoritative runtime version sourced from Cargo package metadata.
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use board::{BoardIdentity, BoardIdentityError};
pub use caller::{
    board_local_day, canonicalize_caller_name, daily_session_elapsed_seconds,
    format_board_local_timestamp, parse_birth_date, AccessDenialReason, AuthenticatedCaller,
    Caller, CallerAccessDenial, CallerError, CallerId, CallerPreferences, CallerProfile,
    CallerSessionContext, CallerState, GraphicsPreference, PostalAddress, SecurityLevel,
    SessionAllowance, TimePolicy, TransferPreference, MAX_CALLER_NAME_BYTES, MAX_SECURITY_LEVEL,
};
pub use config::{
    BoardAccessMode, BoardConfig, CallerConfig, CallerProfilePolicy, ConfigError, LanguageConfig,
    LegacyNodeConfig, MenuPresentationMode, NetworkTerminalDefaults, NodeOverrideConfig,
    NodePoolConfig, PasswordHashConfig, PathConfig, PostLoginJourney, PresentationConfig,
    PresentationMode, ProfileFieldPolicy, RuntimeConfig, SecurityLimitConfig, StorageConfig,
    TransportAdapterConfig, TransportConfig, ValidatedConfig, CONFIG_FORMAT_VERSION,
};
pub use credentials::{CredentialError, CredentialHasher, CREDENTIAL_SCHEME};
pub use database::{
    AuthenticationResult, DatabaseError, MigrationReport, RuntimeDatabase, SCHEMA_VERSION,
};
pub use file::{
    normalize_filename, AsciiTransfer, FileAccess, FileAccessMode, FileActor, FileArea,
    FileAreaDefinition, FileAreaId, FileBackend, FileEntry, FileError, FileId, FileSearch,
    FileStatistics, FileStorage, FileTransfer, NewFileEntry, StagedUpload, TransferDirection,
    TransferReport, MAX_FILE_AREAS, MAX_FILE_DESCRIPTION_BYTES, MAX_FILE_NAME_BYTES,
};
pub use interaction::{
    CallerChat, InteractionError, InteractionHub, OperatorChat, PageAnswer, PageRequest, PageState,
    PageTicket, SysopAvailability,
};
pub use localization::*;
pub use message::{
    Conference, ConferenceAccessMode, ConferenceDefinition, ConferenceId, Message, MessageActor,
    MessageBackend, MessageError, MessageId, MessageKind, MessageRecipient, MessageStats,
    MessageSummary, MessageVisibility, NewMessage, MAX_CONFERENCES, MAX_MESSAGE_BODY_BYTES,
    MAX_MESSAGE_LINES, MAX_MESSAGE_SUBJECT_BYTES,
};
pub use node::{
    MenuRendererPath, NodeChangeHook, NodeDefinition, NodeError, NodeId, NodeLease, NodeManager,
    NodePresentationContext, NodeRuntimeState, NodeSnapshot,
};
pub use paths::{LogicalPath, LogicalPaths, PathError};
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
};
pub use transfer::{
    receive_binary_files, send_binary_files, ProtocolFile, ReceivedProtocolFile, TransferProtocol,
    TransferProtocolError,
};
