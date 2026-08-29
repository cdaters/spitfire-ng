use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("{0}")]
    Usage(String),
    #[error("fixture output directory already exists: {0}")]
    FixtureExists(PathBuf),
    #[error("board setup output directory already exists: {0}")]
    SetupExists(PathBuf),
    #[error("the demo command is restricted to a synthetic fixture board")]
    DemoRequiresFixture,
    #[error("could not create fixture directory {path}: {source}")]
    CreateFixtureDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not create board setup directory {path}: {source}")]
    CreateSetupDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not create fixture file {path}: {source}")]
    CreateFixtureFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write fixture file {path}: {source}")]
    WriteFixtureFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not read board resource {path}: {source}")]
    ReadResource {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("board resource is too large: {path} is {actual} bytes; maximum is {maximum}")]
    ResourceTooLarge {
        path: PathBuf,
        actual: usize,
        maximum: usize,
    },
    #[error("required board resource was not found: {0}")]
    MissingResource(PathBuf),
    #[error("invalid SPITFIRE menu resource {path}: {source}")]
    MenuResource {
        path: PathBuf,
        #[source]
        source: sf_legacy::MenuError,
    },
    #[error("invalid SPITFIRE help resource {path}: {source}")]
    HelpResource {
        path: PathBuf,
        #[source]
        source: sf_legacy::HelpError,
    },
    #[error("configuration path has no board root: {0}")]
    MissingBoardRoot(PathBuf),
    #[error("board is running or another cold-board operation is active: {0}")]
    BoardInUse(PathBuf),
    #[error("could not coordinate board operation through {path}: {source}")]
    BoardLockIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not resolve configuration path {path}: {source}")]
    ResolveConfiguration {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Config(#[from] sf_core::ConfigError),
    #[error(transparent)]
    Paths(#[from] sf_core::PathError),
    #[error(transparent)]
    Language(#[from] sf_core::LanguageError),
    #[error(transparent)]
    Joker(#[from] sf_core::JokerError),
    #[error(transparent)]
    Backup(#[from] crate::BoardBackupError),
    #[error(transparent)]
    Database(#[from] sf_core::DatabaseError),
    #[error(transparent)]
    Message(#[from] sf_core::MessageError),
    #[error(transparent)]
    File(#[from] sf_core::FileError),
    #[error(transparent)]
    Credential(#[from] sf_core::CredentialError),
    #[error(transparent)]
    Node(#[from] sf_core::NodeError),
    #[error(transparent)]
    Interaction(#[from] sf_core::InteractionError),
    #[error(transparent)]
    Session(#[from] sf_core::SessionError),
    #[error(transparent)]
    Terminal(#[from] sf_core::TerminalError),
    #[error("runtime coordination failed: {0}")]
    Coordination(&'static str),
    #[error("transport failed: {0}")]
    Transport(String),
    #[error("in-memory runtime transcript is not valid UTF-8: {0}")]
    InvalidTranscript(#[source] std::str::Utf8Error),
    #[error("could not read a password from the controlling terminal: {0}")]
    PasswordPrompt(#[source] std::io::Error),
    #[error("the two entered passwords did not match")]
    PasswordConfirmation,
    #[error("Sysop password must contain {minimum}..={maximum} bytes")]
    InvalidSysopPasswordLength { minimum: usize, maximum: usize },
    #[error("setup input/output failed: {0}")]
    SetupIo(#[source] std::io::Error),
    #[error("invalid setup value: {0}")]
    InvalidSetupValue(&'static str),
    #[error("setup conference number is duplicated: {0}")]
    DuplicateSetupConference(u16),
    #[error("setup file-area number is duplicated: {0}")]
    DuplicateSetupFileArea(u16),
    #[error("setup file-area storage key is duplicated: {0:?}")]
    DuplicateSetupStorageKey(String),
    #[error(
        "changing logical paths or the database location requires a dedicated relocation workflow"
    )]
    ConfigurationRelocationUnsupported,
    #[error("could not write runtime status {path}: {source}")]
    WriteStatus {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not read runtime status {path}: {source}")]
    ReadStatus {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize runtime status: {0}")]
    StatusSerialize(String),
    #[error("runtime status is malformed: {0}")]
    StatusParse(String),
}
