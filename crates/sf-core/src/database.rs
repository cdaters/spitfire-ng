use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use thiserror::Error;

use crate::{
    board_local_day, canonicalize_caller_name, parse_birth_date, AccessDenialReason,
    AuthenticatedCaller, Caller, CallerAccessDenial, CallerConfig, CallerError, CallerId,
    CallerPreferences, CallerProfile, CallerProfilePolicy, CallerState, CredentialError,
    CredentialHasher, GraphicsPreference, PostalAddress, SecurityLevel, TimePolicy,
    TransferPreference, CREDENTIAL_SCHEME,
};
use crate::{BoardIdentity, BoardIdentityError};

pub const SCHEMA_VERSION: u32 = 10;

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: [Migration; 10] = [
    Migration {
        version: 1,
        name: "board_identity",
        sql: r#"
        CREATE TABLE board_identity (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            board_name TEXT NOT NULL CHECK (length(trim(board_name)) > 0),
            sysop_name TEXT NOT NULL CHECK (length(trim(sysop_name)) > 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
    "#,
    },
    Migration {
        version: 2,
        name: "native_callers",
        sql: r#"
        CREATE TABLE callers (
            caller_id INTEGER PRIMARY KEY,
            display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 30),
            normalized_name TEXT NOT NULL UNIQUE CHECK (length(normalized_name) BETWEEN 1 AND 30),
            security_level INTEGER NOT NULL CHECK (security_level BETWEEN 0 AND 9999),
            account_state TEXT NOT NULL CHECK (account_state IN ('active', 'disabled', 'deleted')),
            is_new_caller INTEGER NOT NULL CHECK (is_new_caller IN (0, 1)),
            first_call_at INTEGER NOT NULL,
            last_call_at INTEGER,
            call_count INTEGER NOT NULL DEFAULT 0 CHECK (call_count >= 0),
            total_time_seconds INTEGER NOT NULL DEFAULT 0 CHECK (total_time_seconds >= 0),
            daily_usage_day INTEGER,
            daily_time_seconds INTEGER NOT NULL DEFAULT 0 CHECK (daily_time_seconds >= 0),
            daily_call_count INTEGER NOT NULL DEFAULT 0 CHECK (daily_call_count >= 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE caller_credentials (
            caller_id INTEGER PRIMARY KEY REFERENCES callers(caller_id) ON DELETE CASCADE,
            scheme TEXT NOT NULL CHECK (scheme = 'argon2id-phc-v1'),
            password_hash TEXT NOT NULL CHECK (length(password_hash) > 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
    "#,
    },
    Migration {
        version: 3,
        name: "native_message_conferences",
        sql: r#"
        ALTER TABLE callers ADD COLUMN messages_posted INTEGER NOT NULL DEFAULT 0
            CHECK (messages_posted >= 0);

        CREATE TABLE message_conferences (
            conference_id INTEGER PRIMARY KEY,
            conference_number INTEGER NOT NULL UNIQUE CHECK (conference_number BETWEEN 1 AND 784),
            name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
            description TEXT NOT NULL,
            access_mode TEXT NOT NULL CHECK (access_mode IN ('at-least', 'exact')),
            read_security INTEGER NOT NULL CHECK (read_security BETWEEN 0 AND 9999),
            post_security INTEGER NOT NULL CHECK (post_security BETWEEN 0 AND 9999),
            public_only INTEGER NOT NULL CHECK (public_only IN (0, 1)),
            maximum_lines INTEGER NOT NULL CHECK (maximum_lines BETWEEN 25 AND 99),
            active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE conference_privileged_security (
            conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id) ON DELETE CASCADE,
            security_level INTEGER NOT NULL CHECK (security_level BETWEEN 0 AND 9999),
            PRIMARY KEY (conference_id, security_level)
        );

        CREATE TABLE messages (
            message_id INTEGER PRIMARY KEY,
            conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id) ON DELETE RESTRICT,
            message_number INTEGER NOT NULL CHECK (message_number > 0),
            author_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            author_name TEXT NOT NULL CHECK (length(author_name) BETWEEN 1 AND 60),
            recipient_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            recipient_name TEXT NOT NULL CHECK (length(recipient_name) BETWEEN 1 AND 60),
            subject BLOB NOT NULL CHECK (length(subject) BETWEEN 1 AND 72),
            body BLOB NOT NULL CHECK (length(body) BETWEEN 1 AND 65536),
            created_at INTEGER NOT NULL,
            parent_message_id INTEGER REFERENCES messages(message_id) ON DELETE RESTRICT,
            visibility TEXT NOT NULL CHECK (visibility IN ('public', 'private')),
            kind TEXT NOT NULL CHECK (kind IN ('standard', 'sysop-comment')),
            deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
            UNIQUE (conference_id, message_number)
        );

        CREATE INDEX messages_conference_scan
            ON messages (conference_id, message_number, deleted);
        CREATE INDEX messages_recipient_scan
            ON messages (recipient_caller_id, visibility, deleted);
        CREATE INDEX messages_author_scan
            ON messages (author_caller_id, visibility, deleted);

        CREATE TABLE caller_last_read (
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE CASCADE,
            conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id) ON DELETE CASCADE,
            last_message_number INTEGER NOT NULL DEFAULT 0 CHECK (last_message_number >= 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (caller_id, conference_id)
        );
    "#,
    },
    Migration {
        version: 4,
        name: "native_file_areas",
        sql: r#"
        ALTER TABLE callers ADD COLUMN files_uploaded INTEGER NOT NULL DEFAULT 0
            CHECK (files_uploaded >= 0);
        ALTER TABLE callers ADD COLUMN upload_bytes INTEGER NOT NULL DEFAULT 0
            CHECK (upload_bytes >= 0);
        ALTER TABLE callers ADD COLUMN files_downloaded INTEGER NOT NULL DEFAULT 0
            CHECK (files_downloaded >= 0);
        ALTER TABLE callers ADD COLUMN download_bytes INTEGER NOT NULL DEFAULT 0
            CHECK (download_bytes >= 0);

        CREATE TABLE file_areas (
            area_id INTEGER PRIMARY KEY,
            area_number INTEGER NOT NULL UNIQUE CHECK (area_number BETWEEN 1 AND 65535),
            name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
            description TEXT NOT NULL CHECK (length(description) <= 255),
            storage_key TEXT NOT NULL UNIQUE CHECK (length(storage_key) BETWEEN 1 AND 64),
            access_mode TEXT NOT NULL CHECK (access_mode IN ('at-least', 'exact')),
            read_security INTEGER NOT NULL CHECK (read_security BETWEEN 0 AND 9999),
            upload_security INTEGER NOT NULL CHECK (upload_security BETWEEN 0 AND 9999),
            preview INTEGER NOT NULL DEFAULT 0 CHECK (preview IN (0, 1)),
            no_charge INTEGER NOT NULL DEFAULT 0 CHECK (no_charge IN (0, 1)),
            maximum_upload_bytes INTEGER NOT NULL CHECK (maximum_upload_bytes BETWEEN 1 AND 1073741824),
            active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE file_area_privileged_security (
            area_id INTEGER NOT NULL REFERENCES file_areas(area_id) ON DELETE CASCADE,
            security_level INTEGER NOT NULL CHECK (security_level BETWEEN 0 AND 9999),
            PRIMARY KEY (area_id, security_level)
        );

        CREATE TABLE files (
            file_id INTEGER PRIMARY KEY,
            area_id INTEGER NOT NULL REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            filename TEXT NOT NULL CHECK (length(filename) BETWEEN 1 AND 64),
            normalized_filename TEXT NOT NULL CHECK (length(normalized_filename) BETWEEN 1 AND 64),
            description TEXT NOT NULL CHECK (length(description) BETWEEN 1 AND 4096),
            size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
            sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
            uploaded_at INTEGER NOT NULL,
            uploader_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            uploader_name TEXT NOT NULL CHECK (length(uploader_name) BETWEEN 1 AND 60),
            download_count INTEGER NOT NULL DEFAULT 0 CHECK (download_count >= 0),
            state TEXT NOT NULL DEFAULT 'available' CHECK (state IN ('available', 'disabled')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (area_id, normalized_filename)
        );

        CREATE INDEX files_area_listing
            ON files (area_id, normalized_filename, state);
        CREATE INDEX files_upload_time
            ON files (uploaded_at, area_id, state);
    "#,
    },
    Migration {
        version: 5,
        name: "caller_terminal_preferences",
        sql: r#"
        ALTER TABLE callers ADD COLUMN graphics_preference TEXT NOT NULL DEFAULT 'auto'
            CHECK (graphics_preference IN ('auto', 'ansi', 'text'));
        ALTER TABLE callers ADD COLUMN screen_width INTEGER
            CHECK (screen_width IS NULL OR screen_width BETWEEN 40 AND 144);
        ALTER TABLE callers ADD COLUMN page_length INTEGER
            CHECK (page_length IS NULL OR page_length BETWEEN 10 AND 24);
        ALTER TABLE callers ADD COLUMN more_prompt INTEGER NOT NULL DEFAULT 1
            CHECK (more_prompt IN (0, 1));
        ALTER TABLE callers ADD COLUMN scroll_prompt INTEGER NOT NULL DEFAULT 0
            CHECK (scroll_prompt IN (0, 1));
        ALTER TABLE callers ADD COLUMN hot_keys INTEGER NOT NULL DEFAULT 0
            CHECK (hot_keys IN (0, 1));
    "#,
    },
    Migration {
        version: 6,
        name: "caller_transfer_preference",
        sql: r#"
        ALTER TABLE callers ADD COLUMN transfer_protocol TEXT NOT NULL DEFAULT 'select'
            CHECK (transfer_protocol IN (
                'select', 'ascii', 'xmodem-checksum', 'xmodem-crc',
                'xmodem-1k', 'ymodem', 'zmodem', 'telink'
            ));
    "#,
    },
    Migration {
        version: 7,
        name: "private_caller_profiles",
        sql: r#"
        ALTER TABLE callers ADD COLUMN address_line_1 TEXT;
        ALTER TABLE callers ADD COLUMN address_line_2 TEXT;
        ALTER TABLE callers ADD COLUMN city TEXT;
        ALTER TABLE callers ADD COLUMN region TEXT;
        ALTER TABLE callers ADD COLUMN postal_code TEXT;
        ALTER TABLE callers ADD COLUMN country TEXT;
        ALTER TABLE callers ADD COLUMN phone TEXT;
        ALTER TABLE callers ADD COLUMN email TEXT;
        ALTER TABLE callers ADD COLUMN birthday TEXT
            CHECK (birthday IS NULL OR length(birthday) = 10);
    "#,
    },
    Migration {
        version: 8,
        name: "message_scan_queues_and_receipts",
        sql: r#"
        CREATE TABLE caller_message_queue (
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE CASCADE,
            conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id) ON DELETE CASCADE,
            queued_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (caller_id, conference_id)
        );

        CREATE TABLE caller_message_receipts (
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE CASCADE,
            message_id INTEGER NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
            received_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (caller_id, message_id)
        );

        CREATE INDEX caller_message_receipts_message
            ON caller_message_receipts (message_id, caller_id);

        INSERT OR IGNORE INTO caller_message_queue (caller_id, conference_id)
        SELECT callers.caller_id, message_conferences.conference_id
          FROM callers CROSS JOIN message_conferences
         WHERE callers.account_state = 'active'
           AND message_conferences.conference_number = 1
           AND message_conferences.active = 1;
    "#,
    },
    Migration {
        version: 9,
        name: "caller_new_file_checkpoint",
        sql: r#"
        ALTER TABLE callers ADD COLUMN last_files_checked_at INTEGER
            CHECK (last_files_checked_at IS NULL OR last_files_checked_at >= 0);
    "#,
    },
    Migration {
        version: 10,
        name: "caller_access_denial_context",
        sql: r#"
        CREATE TABLE caller_access_denial (
            caller_id INTEGER PRIMARY KEY REFERENCES callers(caller_id) ON DELETE CASCADE,
            occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
            reason TEXT NOT NULL CHECK (reason IN (
                'invalid-credentials',
                'account-unavailable',
                'private-board-policy',
                'daily-call-limit',
                'daily-time-limit'
            )),
            generation INTEGER NOT NULL CHECK (generation > 0),
            acknowledged_generation INTEGER NOT NULL DEFAULT 0
                CHECK (acknowledged_generation >= 0 AND acknowledged_generation <= generation)
        );
    "#,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationReport {
    pub starting_version: u32,
    pub ending_version: u32,
    pub applied: usize,
}

pub struct RuntimeDatabase {
    pub(crate) connection: Connection,
    path: PathBuf,
}

impl RuntimeDatabase {
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| DatabaseError::MissingParent(path.to_path_buf()))?;
        if !parent.is_dir() {
            return Err(DatabaseError::MissingDirectory(parent.to_path_buf()));
        }

        let connection = Connection::open(path).map_err(|source| DatabaseError::Open {
            path: path.to_path_buf(),
            source,
        })?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(DatabaseError::Sqlite)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(DatabaseError::Sqlite)?;

        Ok(Self {
            connection,
            path: path.to_path_buf(),
        })
    }

    /// Opens an existing operational database without granting mutation
    /// authority. Backup validation uses this path so an invalid snapshot can
    /// never be migrated as a side effect of inspection.
    pub fn open_read_only(path: &Path) -> Result<Self, DatabaseError> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| DatabaseError::MissingParent(path.to_path_buf()))?;
        if !parent.is_dir() {
            return Err(DatabaseError::MissingDirectory(parent.to_path_buf()));
        }
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|source| DatabaseError::Open {
                path: path.to_path_buf(),
                source,
            })?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(DatabaseError::Sqlite)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(DatabaseError::Sqlite)?;
        Ok(Self {
            connection,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn migrate(&mut self) -> Result<MigrationReport, DatabaseError> {
        self.connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY CHECK (version > 0),
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                "#,
            )
            .map_err(DatabaseError::Sqlite)?;

        let starting_version = schema_version_from(&self.connection)?;
        if starting_version > SCHEMA_VERSION {
            return Err(DatabaseError::NewerSchema {
                found: starting_version,
                supported: SCHEMA_VERSION,
            });
        }
        validate_applied_migrations(&self.connection)?;

        let mut applied = 0;
        for migration in MIGRATIONS
            .iter()
            .filter(|item| item.version > starting_version)
        {
            apply_migration(&mut self.connection, migration)?;
            applied += 1;
        }

        Ok(MigrationReport {
            starting_version,
            ending_version: schema_version_from(&self.connection)?,
            applied,
        })
    }

    pub fn schema_version(&self) -> Result<u32, DatabaseError> {
        schema_version_from(&self.connection)
    }

    /// Verifies the complete schema identity plus SQLite structural and
    /// relational integrity needed before a board snapshot is accepted.
    pub fn validate_current_snapshot(&self) -> Result<BoardIdentity, DatabaseError> {
        let found = schema_version_from(&self.connection)?;
        if found != SCHEMA_VERSION {
            return Err(DatabaseError::SnapshotSchema {
                found,
                required: SCHEMA_VERSION,
            });
        }
        validate_applied_migrations(&self.connection)?;

        let mut statement = self
            .connection
            .prepare("PRAGMA quick_check")
            .map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(DatabaseError::Sqlite)?;
        let checks = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?;
        if checks.as_slice() != ["ok"] {
            return Err(DatabaseError::IntegrityCheck(checks.join("; ")));
        }

        let mut statement = self
            .connection
            .prepare("PRAGMA foreign_key_check")
            .map_err(DatabaseError::Sqlite)?;
        if statement
            .query([])
            .map_err(DatabaseError::Sqlite)?
            .next()
            .map_err(DatabaseError::Sqlite)?
            .is_some()
        {
            return Err(DatabaseError::ForeignKeyCheck);
        }

        self.load_board_identity()?
            .ok_or(DatabaseError::MissingBoardIdentity)
    }

    /// Writes a transactionally consistent SQLite snapshot to a new file.
    /// Callers coordinate the separate cataloged byte store independently.
    pub fn backup_to(&self, destination: &Path) -> Result<(), DatabaseError> {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| DatabaseError::MissingParent(destination.to_path_buf()))?;
        if !parent.is_dir() {
            return Err(DatabaseError::MissingDirectory(parent.to_path_buf()));
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
        {
            Ok(file) => drop(file),
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(DatabaseError::SnapshotExists(destination.to_path_buf()));
            }
            Err(source) => {
                return Err(DatabaseError::SnapshotCreate {
                    path: destination.to_path_buf(),
                    source,
                });
            }
        }

        let result = (|| {
            let mut output =
                Connection::open_with_flags(destination, OpenFlags::SQLITE_OPEN_READ_WRITE)
                    .map_err(|source| DatabaseError::Open {
                        path: destination.to_path_buf(),
                        source,
                    })?;
            let backup = rusqlite::backup::Backup::new(&self.connection, &mut output)
                .map_err(DatabaseError::Sqlite)?;
            backup
                .run_to_completion(128, Duration::from_millis(1), None)
                .map_err(DatabaseError::Sqlite)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(destination);
            return result;
        }
        std::fs::File::open(destination)
            .and_then(|file| file.sync_all())
            .map_err(|source| DatabaseError::SnapshotSync {
                path: destination.to_path_buf(),
                source,
            })
    }

    pub fn ensure_board_identity(
        &self,
        expected: &BoardIdentity,
    ) -> Result<BoardIdentity, DatabaseError> {
        self.connection
            .execute(
                r#"
                INSERT INTO board_identity (singleton, board_name, sysop_name)
                VALUES (1, ?1, ?2)
                ON CONFLICT(singleton) DO NOTHING
                "#,
                params![expected.name(), expected.sysop_name()],
            )
            .map_err(DatabaseError::Sqlite)?;

        let stored = self
            .load_board_identity()?
            .ok_or(DatabaseError::MissingBoardIdentity)?;
        if stored != *expected {
            return Err(DatabaseError::BoardIdentityMismatch {
                configured_name: expected.name().to_owned(),
                configured_sysop: expected.sysop_name().to_owned(),
                stored_name: stored.name().to_owned(),
                stored_sysop: stored.sysop_name().to_owned(),
            });
        }
        Ok(stored)
    }

    pub fn load_board_identity(&self) -> Result<Option<BoardIdentity>, DatabaseError> {
        let values = self
            .connection
            .query_row(
                "SELECT board_name, sysop_name FROM board_identity WHERE singleton = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(DatabaseError::Sqlite)?;
        values
            .map(|(name, sysop)| {
                BoardIdentity::new(name, sysop).map_err(DatabaseError::InvalidStoredIdentity)
            })
            .transpose()
    }

    /// Updates the persisted identity only when it still matches the expected
    /// prior configuration, preventing one administration session from
    /// silently overwriting another.
    pub fn replace_board_identity(
        &self,
        expected: &BoardIdentity,
        replacement: &BoardIdentity,
    ) -> Result<(), DatabaseError> {
        let changed = self
            .connection
            .execute(
                r#"
                UPDATE board_identity SET board_name = ?3, sysop_name = ?4,
                    updated_at = CURRENT_TIMESTAMP
                WHERE singleton = 1 AND board_name = ?1 AND sysop_name = ?2
                "#,
                params![
                    expected.name(),
                    expected.sysop_name(),
                    replacement.name(),
                    replacement.sysop_name()
                ],
            )
            .map_err(DatabaseError::Sqlite)?;
        if changed == 0 {
            let stored = self
                .load_board_identity()?
                .ok_or(DatabaseError::MissingBoardIdentity)?;
            return Err(DatabaseError::BoardIdentityMismatch {
                configured_name: expected.name().to_owned(),
                configured_sysop: expected.sysop_name().to_owned(),
                stored_name: stored.name().to_owned(),
                stored_sysop: stored.sysop_name().to_owned(),
            });
        }
        Ok(())
    }

    pub fn create_caller(
        &mut self,
        caller_name: &[u8],
        password_hash: &str,
        security_level: SecurityLevel,
        state: CallerState,
        is_new_caller: bool,
        now: i64,
    ) -> Result<Caller, DatabaseError> {
        self.create_caller_with_profile(
            caller_name,
            password_hash,
            security_level,
            state,
            is_new_caller,
            now,
            CallerProfile::default(),
            &CallerProfilePolicy::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_caller_with_profile(
        &mut self,
        caller_name: &[u8],
        password_hash: &str,
        security_level: SecurityLevel,
        state: CallerState,
        is_new_caller: bool,
        now: i64,
        profile: CallerProfile,
        policy: &CallerProfilePolicy,
    ) -> Result<Caller, DatabaseError> {
        let profile = profile
            .validate_for_policy(policy)
            .map_err(DatabaseError::InvalidStoredCaller)?;
        let (display_name, normalized_name) = canonicalize_caller_name(caller_name)?;
        if self.caller_by_normalized_name(&normalized_name)?.is_some() {
            return Err(DatabaseError::DuplicateCaller(display_name));
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(DatabaseError::Sqlite)?;
        transaction
            .execute(
                r#"
                INSERT INTO callers (
                    display_name, normalized_name, security_level, account_state,
                    is_new_caller, first_call_at, address_line_1, address_line_2,
                    city, region, postal_code, country, phone, email, birthday
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                "#,
                params![
                    display_name,
                    normalized_name,
                    security_level.get(),
                    state.as_database_value(),
                    is_new_caller,
                    now,
                    profile.address.line_1,
                    profile.address.line_2,
                    profile.address.city,
                    profile.address.region,
                    profile.address.postal_code,
                    profile.address.country,
                    profile.phone,
                    profile.email,
                    profile.birthday_iso(),
                ],
            )
            .map_err(DatabaseError::Sqlite)?;
        let caller_id = CallerId::new(transaction.last_insert_rowid())?;
        transaction
            .execute(
                r#"
                INSERT INTO caller_credentials (caller_id, scheme, password_hash)
                VALUES (?1, ?2, ?3)
                "#,
                params![caller_id.get(), CREDENTIAL_SCHEME, password_hash],
            )
            .map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    pub fn caller_by_name(&self, caller_name: &[u8]) -> Result<Option<Caller>, DatabaseError> {
        let (_, normalized) = canonicalize_caller_name(caller_name)?;
        self.caller_by_normalized_name(&normalized)
    }

    pub fn caller_by_id(&self, caller_id: CallerId) -> Result<Option<Caller>, DatabaseError> {
        self.query_caller(
            "SELECT caller_id, display_name, normalized_name, security_level, account_state, first_call_at, last_call_at, call_count, total_time_seconds, messages_posted, files_uploaded, upload_bytes, files_downloaded, download_bytes, graphics_preference, screen_width, page_length, more_prompt, scroll_prompt, hot_keys, transfer_protocol, address_line_1, address_line_2, city, region, postal_code, country, phone, email, birthday, is_new_caller FROM callers WHERE caller_id = ?1",
            rusqlite::params![caller_id.get()],
        )
    }

    fn caller_by_normalized_name(
        &self,
        normalized_name: &str,
    ) -> Result<Option<Caller>, DatabaseError> {
        self.query_caller(
            "SELECT caller_id, display_name, normalized_name, security_level, account_state, first_call_at, last_call_at, call_count, total_time_seconds, messages_posted, files_uploaded, upload_bytes, files_downloaded, download_bytes, graphics_preference, screen_width, page_length, more_prompt, scroll_prompt, hot_keys, transfer_protocol, address_line_1, address_line_2, city, region, postal_code, country, phone, email, birthday, is_new_caller FROM callers WHERE normalized_name = ?1",
            rusqlite::params![normalized_name],
        )
    }

    fn query_caller<P>(&self, sql: &str, parameters: P) -> Result<Option<Caller>, DatabaseError>
    where
        P: rusqlite::Params,
    {
        type StoredCaller = (
            i64,
            String,
            String,
            u16,
            String,
            i64,
            Option<i64>,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            String,
            Option<u16>,
            Option<u16>,
            bool,
            bool,
            bool,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            bool,
        );
        let stored: Option<StoredCaller> = self
            .connection
            .query_row(sql, parameters, |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                    row.get(16)?,
                    row.get(17)?,
                    row.get(18)?,
                    row.get(19)?,
                    row.get(20)?,
                    row.get(21)?,
                    row.get(22)?,
                    row.get(23)?,
                    row.get(24)?,
                    row.get(25)?,
                    row.get(26)?,
                    row.get(27)?,
                    row.get(28)?,
                    row.get(29)?,
                    row.get(30)?,
                ))
            })
            .optional()
            .map_err(DatabaseError::Sqlite)?;
        stored
            .map(
                |(
                    caller_id,
                    display_name,
                    normalized_name,
                    security,
                    state,
                    first_call_at,
                    last_call_at,
                    call_count,
                    total_time,
                    messages_posted,
                    files_uploaded,
                    upload_bytes,
                    files_downloaded,
                    download_bytes,
                    graphics_preference,
                    screen_width,
                    page_length,
                    more_prompt,
                    scroll_prompt,
                    hot_keys,
                    transfer_protocol,
                    address_line_1,
                    address_line_2,
                    city,
                    region,
                    postal_code,
                    country,
                    phone,
                    email,
                    birthday,
                    is_new_caller,
                )| {
                    Ok(Caller {
                        id: CallerId::new(caller_id).map_err(DatabaseError::InvalidStoredCaller)?,
                        display_name,
                        normalized_name,
                        security_level: SecurityLevel::new(security)
                            .map_err(DatabaseError::InvalidStoredCaller)?,
                        state: CallerState::from_database_value(&state)
                            .map_err(DatabaseError::InvalidStoredCaller)?,
                        first_call_at,
                        last_call_at,
                        call_count: nonnegative_u64(call_count)?,
                        total_time_seconds: nonnegative_u64(total_time)?,
                        messages_posted: nonnegative_u64(messages_posted)?,
                        files_uploaded: nonnegative_u64(files_uploaded)?,
                        upload_bytes: nonnegative_u64(upload_bytes)?,
                        files_downloaded: nonnegative_u64(files_downloaded)?,
                        download_bytes: nonnegative_u64(download_bytes)?,
                        preferences: CallerPreferences {
                            graphics: GraphicsPreference::from_database_value(&graphics_preference)
                                .map_err(DatabaseError::InvalidStoredCaller)?,
                            screen_width,
                            page_length,
                            more_prompt,
                            scroll_prompt,
                            hot_keys,
                            transfer_protocol: TransferPreference::from_database_value(
                                &transfer_protocol,
                            )
                            .map_err(DatabaseError::InvalidStoredCaller)?,
                        }
                        .validate()
                        .map_err(DatabaseError::InvalidStoredCaller)?,
                        profile: CallerProfile {
                            address: PostalAddress {
                                line_1: address_line_1,
                                line_2: address_line_2,
                                city,
                                region,
                                postal_code,
                                country,
                            },
                            phone,
                            email,
                            birthday: birthday
                                .as_deref()
                                .map(parse_birth_date)
                                .transpose()
                                .map_err(DatabaseError::InvalidStoredCaller)?
                                .flatten(),
                        },
                        is_new_caller,
                    })
                },
            )
            .transpose()
    }

    pub fn authenticate(
        &self,
        caller_name: &[u8],
        password: &[u8],
        hasher: &CredentialHasher,
    ) -> Result<AuthenticationResult, DatabaseError> {
        let (_, normalized_name) = canonicalize_caller_name(caller_name)?;
        let Some(caller) = self.caller_by_normalized_name(&normalized_name)? else {
            return Ok(AuthenticationResult::Invalid);
        };
        if caller.state != CallerState::Active {
            return Ok(AuthenticationResult::Unavailable(caller));
        }
        let stored: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT scheme, password_hash FROM caller_credentials WHERE caller_id = ?1",
                params![caller.id.get()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(DatabaseError::Sqlite)?;
        let Some((scheme, password_hash)) = stored else {
            return Err(DatabaseError::MissingCredential(caller.id.get()));
        };
        if scheme != CREDENTIAL_SCHEME {
            return Err(DatabaseError::UnsupportedCredentialScheme(scheme));
        }
        if hasher.verify(password, &password_hash)? {
            Ok(AuthenticationResult::Valid(caller))
        } else {
            Ok(AuthenticationResult::Invalid)
        }
    }

    /// Records only the latest privacy-safe denial category for a known
    /// caller. Unknown supplied identities deliberately produce no row.
    pub fn record_caller_access_denial(
        &self,
        caller_id: CallerId,
        occurred_at: i64,
        reason: AccessDenialReason,
    ) -> Result<(), DatabaseError> {
        let changed = self
            .connection
            .execute(
                r#"
                INSERT INTO caller_access_denial (
                    caller_id, occurred_at, reason, generation, acknowledged_generation
                ) VALUES (?1, ?2, ?3, 1, 0)
                ON CONFLICT(caller_id) DO UPDATE SET
                    occurred_at = excluded.occurred_at,
                    reason = excluded.reason,
                    generation = caller_access_denial.generation + 1
                "#,
                params![caller_id.get(), occurred_at, reason.as_database_value()],
            )
            .map_err(DatabaseError::Sqlite)?;
        if changed != 1 {
            return Err(DatabaseError::MissingCaller(caller_id.get()));
        }
        Ok(())
    }

    /// Acknowledges exactly the denial that was presented. A concurrent newer
    /// denial remains pending because its generation will not match.
    pub fn acknowledge_caller_access_denial(
        &self,
        caller_id: CallerId,
        generation: u64,
    ) -> Result<(), DatabaseError> {
        self.connection
            .execute(
                r#"
                UPDATE caller_access_denial
                   SET acknowledged_generation = ?3
                 WHERE caller_id = ?1
                   AND generation = ?2
                   AND acknowledged_generation < ?2
                "#,
                params![
                    caller_id.get(),
                    sqlite_i64(generation)?,
                    sqlite_i64(generation)?
                ],
            )
            .map_err(DatabaseError::Sqlite)?;
        Ok(())
    }

    pub fn begin_caller_session(
        &mut self,
        caller: &Caller,
        config: &CallerConfig,
        now: i64,
        timezone: chrono_tz::Tz,
    ) -> Result<AuthenticatedCaller, DatabaseError> {
        if caller.state != CallerState::Active {
            return Err(DatabaseError::CallerUnavailable);
        }
        let day = i64::from(board_local_day(now, timezone)?);
        let transaction = self
            .connection
            .transaction()
            .map_err(DatabaseError::Sqlite)?;
        let (stored_day, stored_time, stored_calls): (Option<i64>, i64, i64) = transaction
            .query_row(
                "SELECT daily_usage_day, daily_time_seconds, daily_call_count FROM callers WHERE caller_id = ?1",
                params![caller.id.get()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(DatabaseError::Sqlite)?;
        let (used_seconds, calls_today) = if stored_day == Some(day) {
            (
                nonnegative_u64(stored_time)?,
                nonnegative_u32(stored_calls)?,
            )
        } else {
            (0, 0)
        };
        let policy = TimePolicy::for_security(config, caller.security_level);
        if calls_today >= policy.maximum_daily_calls {
            return Err(DatabaseError::DailyCallLimitReached);
        }
        let first_day = (i64::from(board_local_day(caller.first_call_at, timezone)?) == day)
            .then_some(config.new_caller_first_day_minutes);
        let daily_limit_seconds = policy.daily_limit_seconds(first_day);
        let allowance = policy.allowance(used_seconds, first_day);
        if allowance.limit_seconds() == 0 {
            return Err(DatabaseError::DailyTimeLimitReached);
        }
        transaction
            .execute(
                r#"
                UPDATE callers
                SET last_call_at = ?2,
                    call_count = call_count + 1,
                    daily_usage_day = ?3,
                    daily_time_seconds = ?4,
                    daily_call_count = ?5,
                    is_new_caller = 0,
                    updated_at = CURRENT_TIMESTAMP
                WHERE caller_id = ?1
                "#,
                params![
                    caller.id.get(),
                    now,
                    day,
                    sqlite_i64(used_seconds)?,
                    i64::from(calls_today) + 1
                ],
            )
            .map_err(DatabaseError::Sqlite)?;
        let pending_access_denial = transaction
            .query_row(
                r#"
                SELECT generation, occurred_at, reason
                  FROM caller_access_denial
                 WHERE caller_id = ?1
                   AND generation > acknowledged_generation
                "#,
                params![caller.id.get()],
                |row| {
                    let generation: i64 = row.get(0)?;
                    let occurred_at: i64 = row.get(1)?;
                    let reason: String = row.get(2)?;
                    Ok((generation, occurred_at, reason))
                },
            )
            .optional()
            .map_err(DatabaseError::Sqlite)?
            .map(|(generation, occurred_at, reason)| {
                Ok::<CallerAccessDenial, DatabaseError>(CallerAccessDenial::new(
                    nonnegative_u64(generation)?,
                    occurred_at,
                    AccessDenialReason::from_database_value(&reason)?,
                ))
            })
            .transpose()?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        let updated = self
            .caller_by_id(caller.id)?
            .ok_or(DatabaseError::MissingCaller(caller.id.get()))?;
        Ok(AuthenticatedCaller {
            caller: updated,
            first_session: caller.is_new_caller,
            previous_call_at: caller.last_call_at,
            calls_today: calls_today + 1,
            time_used_today_seconds: used_seconds,
            daily_limit_seconds,
            session_started_at: now,
            pending_access_denial,
            allowance,
        })
    }

    pub fn finish_caller_session(
        &self,
        caller_id: CallerId,
        elapsed_seconds: u64,
        daily_elapsed_seconds: u64,
        day: i64,
    ) -> Result<(), DatabaseError> {
        let changed = self
            .connection
            .execute(
                r#"
                UPDATE callers
                SET total_time_seconds = total_time_seconds + ?2,
                    daily_time_seconds = CASE
                        WHEN daily_usage_day = ?4 THEN daily_time_seconds + ?3
                        ELSE ?3
                    END,
                    daily_call_count = CASE
                        WHEN daily_usage_day = ?4 THEN daily_call_count
                        ELSE 0
                    END,
                    daily_usage_day = ?4,
                    updated_at = CURRENT_TIMESTAMP
                WHERE caller_id = ?1
                "#,
                params![
                    caller_id.get(),
                    sqlite_i64(elapsed_seconds)?,
                    sqlite_i64(daily_elapsed_seconds)?,
                    day
                ],
            )
            .map_err(DatabaseError::Sqlite)?;
        if changed != 1 {
            return Err(DatabaseError::MissingCaller(caller_id.get()));
        }
        Ok(())
    }

    pub fn set_caller_state(
        &self,
        caller_id: CallerId,
        state: CallerState,
    ) -> Result<(), DatabaseError> {
        let changed = self
            .connection
            .execute(
                "UPDATE callers SET account_state = ?2, updated_at = CURRENT_TIMESTAMP WHERE caller_id = ?1",
                params![caller_id.get(), state.as_database_value()],
            )
            .map_err(DatabaseError::Sqlite)?;
        if changed != 1 {
            return Err(DatabaseError::MissingCaller(caller_id.get()));
        }
        Ok(())
    }

    pub fn set_caller_security(
        &self,
        caller_id: CallerId,
        security: SecurityLevel,
    ) -> Result<(), DatabaseError> {
        let changed = self
            .connection
            .execute(
                "UPDATE callers SET security_level = ?2, updated_at = CURRENT_TIMESTAMP WHERE caller_id = ?1",
                params![caller_id.get(), security.get()],
            )
            .map_err(DatabaseError::Sqlite)?;
        if changed != 1 {
            return Err(DatabaseError::MissingCaller(caller_id.get()));
        }
        Ok(())
    }

    pub fn update_caller_preferences(
        &self,
        caller_id: CallerId,
        preferences: CallerPreferences,
    ) -> Result<Caller, DatabaseError> {
        let preferences = preferences
            .validate()
            .map_err(DatabaseError::InvalidStoredCaller)?;
        let changed = self
            .connection
            .execute(
                r#"
                UPDATE callers
                SET graphics_preference = ?2, screen_width = ?3,
                    page_length = ?4, more_prompt = ?5, scroll_prompt = ?6,
                    hot_keys = ?7, transfer_protocol = ?8,
                    updated_at = CURRENT_TIMESTAMP
                WHERE caller_id = ?1
                "#,
                params![
                    caller_id.get(),
                    preferences.graphics.as_database_value(),
                    preferences.screen_width,
                    preferences.page_length,
                    preferences.more_prompt,
                    preferences.scroll_prompt,
                    preferences.hot_keys,
                    preferences.transfer_protocol.as_database_value()
                ],
            )
            .map_err(DatabaseError::Sqlite)?;
        if changed != 1 {
            return Err(DatabaseError::MissingCaller(caller_id.get()));
        }
        self.caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    pub fn update_caller_profile(
        &self,
        caller_id: CallerId,
        profile: CallerProfile,
        policy: &CallerProfilePolicy,
    ) -> Result<Caller, DatabaseError> {
        let existing = self
            .caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))?;
        let profile = profile
            .validate_update_for_policy(&existing.profile, policy)
            .map_err(DatabaseError::InvalidStoredCaller)?;
        let changed = self
            .connection
            .execute(
                r#"
                UPDATE callers SET
                    address_line_1 = ?2, address_line_2 = ?3, city = ?4,
                    region = ?5, postal_code = ?6, country = ?7, phone = ?8,
                    email = ?9, birthday = ?10, updated_at = CURRENT_TIMESTAMP
                WHERE caller_id = ?1
                "#,
                params![
                    caller_id.get(),
                    profile.address.line_1,
                    profile.address.line_2,
                    profile.address.city,
                    profile.address.region,
                    profile.address.postal_code,
                    profile.address.country,
                    profile.phone,
                    profile.email,
                    profile.birthday_iso(),
                ],
            )
            .map_err(DatabaseError::Sqlite)?;
        if changed != 1 {
            return Err(DatabaseError::MissingCaller(caller_id.get()));
        }
        self.caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    pub fn all_callers(&self) -> Result<Vec<Caller>, DatabaseError> {
        let mut statement = self
            .connection
            .prepare("SELECT caller_id FROM callers ORDER BY normalized_name")
            .map_err(DatabaseError::Sqlite)?;
        let ids = statement
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(DatabaseError::Sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?;
        ids.into_iter()
            .map(|value| {
                let id = CallerId::new(value).map_err(DatabaseError::InvalidStoredCaller)?;
                self.caller_by_id(id)?
                    .ok_or(DatabaseError::MissingCaller(value))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthenticationResult {
    Valid(Caller),
    Invalid,
    Unavailable(Caller),
}

fn nonnegative_u64(value: i64) -> Result<u64, DatabaseError> {
    u64::try_from(value).map_err(|_| DatabaseError::InvalidStoredCounter(value))
}

fn nonnegative_u32(value: i64) -> Result<u32, DatabaseError> {
    u32::try_from(value).map_err(|_| DatabaseError::InvalidStoredCounter(value))
}

fn sqlite_i64(value: u64) -> Result<i64, DatabaseError> {
    i64::try_from(value).map_err(|_| DatabaseError::CounterOverflow(value))
}

fn schema_version_from(connection: &Connection) -> Result<u32, DatabaseError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if !exists {
        return Ok(0);
    }
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)
}

fn validate_applied_migrations(connection: &Connection) -> Result<(), DatabaseError> {
    let mut statement = connection
        .prepare("SELECT version, name FROM schema_migrations ORDER BY version")
        .map_err(DatabaseError::Sqlite)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(DatabaseError::Sqlite)?;

    for row in rows {
        let (version, stored_name) = row.map_err(DatabaseError::Sqlite)?;
        let Some(expected) = MIGRATIONS.iter().find(|item| item.version == version) else {
            return Err(DatabaseError::UnknownMigration(version));
        };
        if stored_name != expected.name {
            return Err(DatabaseError::MigrationNameMismatch {
                version,
                stored: stored_name,
                expected: expected.name,
            });
        }
    }
    Ok(())
}

fn apply_migration(
    connection: &mut Connection,
    migration: &Migration,
) -> Result<(), DatabaseError> {
    let transaction = connection.transaction().map_err(DatabaseError::Sqlite)?;
    run_migration(&transaction, migration)?;
    transaction.commit().map_err(DatabaseError::Sqlite)
}

fn run_migration(
    transaction: &Transaction<'_>,
    migration: &Migration,
) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(migration.sql)
        .map_err(DatabaseError::Sqlite)?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            params![migration.version, migration.name],
        )
        .map_err(DatabaseError::Sqlite)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database path has no parent directory: {0}")]
    MissingParent(PathBuf),
    #[error("database directory does not exist: {0}")]
    MissingDirectory(PathBuf),
    #[error("could not open SQLite database {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[source] rusqlite::Error),
    #[error("database snapshot schema is {found}; current schema {required} is required")]
    SnapshotSchema { found: u32, required: u32 },
    #[error("SQLite integrity check failed: {0}")]
    IntegrityCheck(String),
    #[error("SQLite foreign-key validation failed")]
    ForeignKeyCheck,
    #[error("database snapshot destination already exists: {0}")]
    SnapshotExists(PathBuf),
    #[error("could not create database snapshot {path}: {source}")]
    SnapshotCreate {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not synchronize database snapshot {path}: {source}")]
    SnapshotSync {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("database schema version {found} is newer than supported version {supported}")]
    NewerSchema { found: u32, supported: u32 },
    #[error("database contains unknown migration version {0}")]
    UnknownMigration(u32),
    #[error("migration {version} is named {stored:?}; expected {expected:?}")]
    MigrationNameMismatch {
        version: u32,
        stored: String,
        expected: &'static str,
    },
    #[error("database does not contain its required board identity")]
    MissingBoardIdentity,
    #[error("stored board identity is invalid: {0}")]
    InvalidStoredIdentity(#[source] BoardIdentityError),
    #[error(
        "configuration identifies {configured_name:?} / {configured_sysop:?}, but the database belongs to {stored_name:?} / {stored_sysop:?}"
    )]
    BoardIdentityMismatch {
        configured_name: String,
        configured_sysop: String,
        stored_name: String,
        stored_sysop: String,
    },
    #[error(transparent)]
    InvalidCaller(#[from] CallerError),
    #[error("stored caller record is invalid: {0}")]
    InvalidStoredCaller(CallerError),
    #[error(transparent)]
    Credential(#[from] CredentialError),
    #[error("caller name is already registered: {0:?}")]
    DuplicateCaller(String),
    #[error("caller {0} does not exist")]
    MissingCaller(i64),
    #[error("caller {0} has no password credential")]
    MissingCredential(i64),
    #[error("unsupported stored credential scheme {0:?}")]
    UnsupportedCredentialScheme(String),
    #[error("caller account is unavailable")]
    CallerUnavailable,
    #[error("maximum daily caller access count has been reached")]
    DailyCallLimitReached,
    #[error("daily caller time allowance has been exhausted")]
    DailyTimeLimitReached,
    #[error("database contains an invalid negative or oversized counter {0}")]
    InvalidStoredCounter(i64),
    #[error("counter {0} is too large for SQLite")]
    CounterOverflow(u64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn database_path(temp: &tempfile::TempDir) -> PathBuf {
        temp.path().join("runtime.sqlite3")
    }

    #[test]
    fn initializes_and_migrates_an_empty_database() {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        assert_eq!(database.schema_version().unwrap(), 0);
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 0,
                ending_version: SCHEMA_VERSION,
                applied: 10,
            }
        );
        assert_eq!(database.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn migrates_schema_nine_board_to_privacy_bounded_context() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(9) {
            apply_migration(&mut connection, migration).unwrap();
        }
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 9,
                ending_version: SCHEMA_VERSION,
                applied: 1,
            }
        );
        let table_count: i64 = database
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'caller_access_denial'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn consistent_snapshot_is_read_only_validated_and_excludes_later_writes() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut database = RuntimeDatabase::open(&path).unwrap();
        database.migrate().unwrap();
        let identity = BoardIdentity::new("Snapshot Board", "Snapshot Sysop").unwrap();
        database.ensure_board_identity(&identity).unwrap();
        let encoded = test_hasher().hash(b"test-only snapshot password").unwrap();
        database
            .create_caller(
                b"Before Snapshot",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();

        assert_eq!(database.validate_current_snapshot().unwrap(), identity);
        let snapshot_path = temp.path().join("snapshot.sqlite3");
        database.backup_to(&snapshot_path).unwrap();
        database
            .create_caller(
                b"After Snapshot",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                200,
            )
            .unwrap();

        let snapshot = RuntimeDatabase::open_read_only(&snapshot_path).unwrap();
        assert_eq!(snapshot.validate_current_snapshot().unwrap(), identity);
        assert!(snapshot
            .caller_by_name(b"Before Snapshot")
            .unwrap()
            .is_some());
        assert!(snapshot
            .caller_by_name(b"After Snapshot")
            .unwrap()
            .is_none());
        assert!(matches!(
            database.backup_to(&snapshot_path),
            Err(DatabaseError::SnapshotExists(path)) if path == snapshot_path
        ));
    }

    #[test]
    fn snapshot_validation_requires_the_exact_current_schema() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(8) {
            apply_migration(&mut connection, migration).unwrap();
        }
        drop(connection);
        let database = RuntimeDatabase::open_read_only(&path).unwrap();
        assert!(matches!(
            database.validate_current_snapshot(),
            Err(DatabaseError::SnapshotSchema {
                found: 8,
                required: SCHEMA_VERSION
            })
        ));
    }

    #[test]
    fn migrations_are_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        database.migrate().unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: SCHEMA_VERSION,
                ending_version: SCHEMA_VERSION,
                applied: 0,
            }
        );
    }

    #[test]
    fn migrates_an_increment_zero_database_without_replacing_board_identity() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY CHECK (version > 0),
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE board_identity (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    board_name TEXT NOT NULL CHECK (length(trim(board_name)) > 0),
                    sysop_name TEXT NOT NULL CHECK (length(trim(sysop_name)) > 0),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO schema_migrations (version, name) VALUES (1, 'board_identity');
                INSERT INTO board_identity (singleton, board_name, sysop_name)
                VALUES (1, 'Existing Board', 'Existing Sysop');
                "#,
            )
            .unwrap();
        drop(connection);
        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 1,
                ending_version: SCHEMA_VERSION,
                applied: 9,
            }
        );
        assert_eq!(
            database.load_board_identity().unwrap().unwrap().name(),
            "Existing Board"
        );
    }

    #[test]
    fn migrates_an_increment_two_database_without_losing_callers() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(2) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute(
                "INSERT INTO callers (display_name, normalized_name, security_level, account_state, is_new_caller, first_call_at) VALUES ('Existing Caller', 'existing caller', 10, 'active', 0, 100)",
                [],
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 2,
                ending_version: SCHEMA_VERSION,
                applied: 8,
            }
        );
        let caller = database
            .caller_by_name(b"Existing Caller")
            .unwrap()
            .unwrap();
        assert_eq!(caller.messages_posted, 0);
        assert_eq!(caller.files_uploaded, 0);
        assert_eq!(caller.upload_bytes, 0);
        assert_eq!(caller.files_downloaded, 0);
        assert_eq!(caller.download_bytes, 0);
    }

    #[test]
    fn migrates_an_increment_three_database_without_losing_messages() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(3) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute(
                "INSERT INTO message_conferences (conference_number, name, description, access_mode, read_security, post_security, public_only, maximum_lines) VALUES (1, 'Existing', 'Existing messages', 'at-least', 0, 0, 0, 99)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO messages (conference_id, message_number, author_name, recipient_name, subject, body, created_at, visibility, kind) VALUES (1, 1, 'Fixture', 'All', 'Preserved', 'Still present', 100, 'public', 'standard')",
                [],
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 3,
                ending_version: SCHEMA_VERSION,
                applied: 7,
            }
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM messages", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn migrates_an_increment_five_board_from_schema_four_with_safe_preferences() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(4) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute(
                "INSERT INTO callers (display_name, normalized_name, security_level, account_state, is_new_caller, first_call_at) VALUES ('Existing Caller', 'existing caller', 10, 'active', 0, 100)",
                [],
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 4,
                ending_version: SCHEMA_VERSION,
                applied: 6,
            }
        );
        let caller = database
            .caller_by_name(b"Existing Caller")
            .unwrap()
            .unwrap();
        assert_eq!(caller.preferences, CallerPreferences::default());
        let updated = database
            .update_caller_preferences(
                caller.id,
                CallerPreferences {
                    graphics: GraphicsPreference::Text,
                    screen_width: Some(132),
                    page_length: Some(20),
                    more_prompt: false,
                    scroll_prompt: true,
                    hot_keys: true,
                    transfer_protocol: TransferPreference::Zmodem,
                },
            )
            .unwrap();
        assert_eq!(updated.preferences.screen_width, Some(132));
        assert_eq!(
            database
                .caller_by_id(caller.id)
                .unwrap()
                .unwrap()
                .preferences,
            updated.preferences
        );
    }

    #[test]
    fn migrates_schema_five_transfer_preference_without_resetting_callers() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(5) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute(
                "INSERT INTO callers (display_name, normalized_name, security_level, account_state, is_new_caller, first_call_at, graphics_preference) VALUES ('Transfer Caller', 'transfer caller', 10, 'active', 0, 100, 'ansi')",
                [],
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 5,
                ending_version: SCHEMA_VERSION,
                applied: 5,
            }
        );
        let caller = database
            .caller_by_name(b"Transfer Caller")
            .unwrap()
            .unwrap();
        assert_eq!(
            caller.preferences.transfer_protocol,
            TransferPreference::Select
        );
    }

    #[test]
    fn migrates_schema_six_and_persists_private_profile_without_losing_caller_state() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(6) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute(
                "INSERT INTO callers (display_name, normalized_name, security_level, account_state, is_new_caller, first_call_at, call_count) VALUES ('Profile Caller', 'profile caller', 10, 'active', 0, 100, 7)",
                [],
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 6,
                ending_version: SCHEMA_VERSION,
                applied: 4,
            }
        );
        let caller = database.caller_by_name(b"Profile Caller").unwrap().unwrap();
        assert_eq!(caller.call_count, 7);
        assert_eq!(caller.profile, CallerProfile::default());
        let policy = CallerProfilePolicy {
            address: crate::ProfileFieldPolicy::Optional,
            phone: crate::ProfileFieldPolicy::Optional,
            email: crate::ProfileFieldPolicy::Optional,
            birthday: crate::ProfileFieldPolicy::Optional,
        };
        let updated = database
            .update_caller_profile(
                caller.id,
                CallerProfile {
                    address: PostalAddress {
                        city: Some("Phoenix".to_owned()),
                        region: Some("Arizona".to_owned()),
                        ..PostalAddress::default()
                    },
                    phone: Some("+1 602 555 0100".to_owned()),
                    email: Some("profile@example.test".to_owned()),
                    birthday: parse_birth_date("1980-03-04").unwrap(),
                },
                &policy,
            )
            .unwrap();
        assert_eq!(updated.profile.address.city.as_deref(), Some("Phoenix"));
        assert_eq!(
            updated.profile.birthday_iso().as_deref(),
            Some("1980-03-04")
        );
    }

    #[test]
    fn migrates_schema_seven_with_messages_profiles_and_last_read_intact() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(7) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute_batch(
                r#"
                INSERT INTO callers (
                    caller_id, display_name, normalized_name, security_level,
                    account_state, is_new_caller, first_call_at, messages_posted,
                    email
                ) VALUES (1, 'Schema Seven Caller', 'schema seven caller', 10,
                          'active', 0, 100, 1, 'schema7@example.test');
                INSERT INTO message_conferences (
                    conference_id, conference_number, name, description,
                    access_mode, read_security, post_security, public_only,
                    maximum_lines
                ) VALUES (1, 1, 'General', 'General messages', 'at-least', 5, 5, 0, 50);
                INSERT INTO messages (
                    message_id, conference_id, message_number, author_caller_id,
                    author_name, recipient_caller_id, recipient_name, subject,
                    body, created_at, visibility, kind
                ) VALUES (1, 1, 1, 1, 'Schema Seven Caller', NULL,
                          'All Callers', X'5375626A656374', X'426F64790D0A', 101,
                          'public', 'standard');
                INSERT INTO caller_last_read (
                    caller_id, conference_id, last_message_number
                ) VALUES (1, 1, 1);
                "#,
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 7,
                ending_version: SCHEMA_VERSION,
                applied: 3,
            }
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM messages", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT last_message_number FROM caller_last_read",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .caller_by_name(b"Schema Seven Caller")
                .unwrap()
                .unwrap()
                .profile
                .email
                .as_deref(),
            Some("schema7@example.test")
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM caller_message_queue", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM caller_message_receipts", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
        assert_eq!(database.migrate().unwrap().applied, 0);
    }

    #[test]
    fn migrates_schema_eight_with_file_catalog_and_caller_state_intact() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(8) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute_batch(
                r#"
                INSERT INTO callers (
                    caller_id, display_name, normalized_name, security_level,
                    account_state, is_new_caller, first_call_at, files_uploaded,
                    upload_bytes, files_downloaded, download_bytes
                ) VALUES (1, 'Schema Eight Caller', 'schema eight caller', 10,
                          'active', 0, 100, 2, 4096, 3, 8192);
                INSERT INTO file_areas (
                    area_id, area_number, name, description, storage_key,
                    access_mode, read_security, upload_security, preview,
                    no_charge, maximum_upload_bytes
                ) VALUES (1, 1, 'Existing Files', 'Preserved file area',
                          'existing', 'at-least', 5, 5, 0, 0, 1048576);
                INSERT INTO files (
                    file_id, area_id, filename, normalized_filename,
                    description, size_bytes, sha256, uploaded_at,
                    uploader_caller_id, uploader_name, download_count, state
                ) VALUES (1, 1, 'PRESERVE.ZIP', 'PRESERVE.ZIP',
                          'Preserved catalog entry', 1234,
                          'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                          200, 1, 'Schema Eight Caller', 4, 'available');
                "#,
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 8,
                ending_version: SCHEMA_VERSION,
                applied: 2,
            }
        );
        let caller = database
            .caller_by_name(b"Schema Eight Caller")
            .unwrap()
            .unwrap();
        assert_eq!(caller.files_uploaded, 2);
        assert_eq!(caller.upload_bytes, 4096);
        assert_eq!(caller.files_downloaded, 3);
        assert_eq!(caller.download_bytes, 8192);
        assert_eq!(
            database
                .connection
                .query_row("SELECT filename FROM files WHERE file_id = 1", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "PRESERVE.ZIP"
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT last_files_checked_at FROM callers WHERE caller_id = 1",
                    [],
                    |row| row.get::<_, Option<i64>>(0)
                )
                .unwrap(),
            None
        );
        assert_eq!(database.migrate().unwrap().applied, 0);
    }

    #[test]
    fn board_identity_persists_and_cannot_silently_change() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let expected = BoardIdentity::new("Fixture Board", "Fixture Sysop").unwrap();
        {
            let mut database = RuntimeDatabase::open(&path).unwrap();
            database.migrate().unwrap();
            assert_eq!(database.ensure_board_identity(&expected).unwrap(), expected);
        }
        {
            let mut database = RuntimeDatabase::open(&path).unwrap();
            database.migrate().unwrap();
            assert_eq!(database.load_board_identity().unwrap(), Some(expected));
            let changed = BoardIdentity::new("Different Board", "Other Sysop").unwrap();
            assert!(matches!(
                database.ensure_board_identity(&changed),
                Err(DatabaseError::BoardIdentityMismatch { .. })
            ));
        }
    }

    #[test]
    fn missing_database_directory_is_a_useful_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing/runtime.sqlite3");
        assert!(matches!(
            RuntimeDatabase::open(&path),
            Err(DatabaseError::MissingDirectory(_))
        ));
    }

    fn test_hasher() -> CredentialHasher {
        CredentialHasher::new(&crate::PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap()
    }

    #[test]
    fn caller_creation_authentication_and_duplicate_prevention_are_persistent() {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        database.migrate().unwrap();
        let hasher = test_hasher();
        let password = b"test-only database password";
        let encoded = hasher.hash(password).unwrap();
        let caller = database
            .create_caller(
                b"Alex   Test",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                true,
                100,
            )
            .unwrap();
        assert_eq!(caller.display_name, "Alex Test");
        assert!(matches!(
            database.authenticate(b"alex test", password, &hasher).unwrap(),
            AuthenticationResult::Valid(found) if found.id == caller.id
        ));
        assert_eq!(
            database
                .authenticate(b"ALEX TEST", b"wrong", &hasher)
                .unwrap(),
            AuthenticationResult::Invalid
        );
        assert!(matches!(
            database.create_caller(
                b"ALEX TEST",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            ),
            Err(DatabaseError::DuplicateCaller(_))
        ));
        let stored_hash: String = database
            .connection
            .query_row(
                "SELECT password_hash FROM caller_credentials WHERE caller_id = ?1",
                params![caller.id.get()],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(stored_hash.as_bytes(), password);
        assert!(!stored_hash.contains("database password"));
    }

    #[test]
    fn disabled_caller_is_not_authenticated() {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        database.migrate().unwrap();
        let hasher = test_hasher();
        let encoded = hasher.hash(b"test-only disabled password").unwrap();
        let caller = database
            .create_caller(
                b"Disabled Caller",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        database
            .set_caller_state(caller.id, CallerState::Disabled)
            .unwrap();
        assert!(matches!(
            database
                .authenticate(b"Disabled Caller", b"test-only disabled password", &hasher)
                .unwrap(),
            AuthenticationResult::Unavailable(_)
        ));
    }

    #[test]
    fn latest_access_denial_is_privacy_bounded_and_acknowledged_by_generation() {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        database.migrate().unwrap();
        let hasher = test_hasher();
        let encoded = hasher.hash(b"test-only denial password").unwrap();
        let caller = database
            .create_caller(
                b"Denied Caller",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        database
            .record_caller_access_denial(caller.id, 200, AccessDenialReason::InvalidCredentials)
            .unwrap();
        let authenticated = database
            .begin_caller_session(&caller, &CallerConfig::default(), 300, chrono_tz::UTC)
            .unwrap();
        let denial = authenticated.pending_access_denial.unwrap();
        assert_eq!(denial.occurred_at(), 200);
        assert_eq!(denial.reason(), AccessDenialReason::InvalidCredentials);
        database
            .record_caller_access_denial(caller.id, 400, AccessDenialReason::PrivateBoardPolicy)
            .unwrap();
        database
            .acknowledge_caller_access_denial(caller.id, denial.generation())
            .unwrap();
        let stored = database.caller_by_id(caller.id).unwrap().unwrap();
        let next = database
            .begin_caller_session(&stored, &CallerConfig::default(), 500, chrono_tz::UTC)
            .unwrap();
        assert_eq!(
            next.pending_access_denial.unwrap().reason(),
            AccessDenialReason::PrivateBoardPolicy
        );
        let columns = database
            .connection
            .query_row(
                "SELECT count(*) FROM pragma_table_info('caller_access_denial')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(columns, 5);
    }

    #[test]
    fn call_statistics_first_last_and_time_limits_persist() {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        database.migrate().unwrap();
        let hasher = test_hasher();
        let encoded = hasher.hash(b"test-only stats password").unwrap();
        let caller = database
            .create_caller(
                b"Stats Caller",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                true,
                100,
            )
            .unwrap();
        let config = CallerConfig::default();
        let first = database
            .begin_caller_session(&caller, &config, 200, chrono_tz::UTC)
            .unwrap();
        assert_eq!(first.previous_call_at, None);
        assert_eq!(first.caller.call_count, 1);
        database
            .finish_caller_session(caller.id, 90, 90, 19_700_101)
            .unwrap();
        let stored = database.caller_by_id(caller.id).unwrap().unwrap();
        let second = database
            .begin_caller_session(&stored, &config, 400, chrono_tz::UTC)
            .unwrap();
        assert_eq!(second.previous_call_at, Some(200));
        assert_eq!(second.caller.call_count, 2);
        assert_eq!(second.caller.total_time_seconds, 90);

        let restricted = CallerConfig {
            maximum_daily_calls: 2,
            ..CallerConfig::default()
        };
        let stored = database.caller_by_id(caller.id).unwrap().unwrap();
        assert!(matches!(
            database.begin_caller_session(&stored, &restricted, 500, chrono_tz::UTC),
            Err(DatabaseError::DailyCallLimitReached)
        ));
    }

    #[test]
    fn daily_call_and_time_accounting_reset_at_board_local_midnight() {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        database.migrate().unwrap();
        let hasher = test_hasher();
        let encoded = hasher.hash(b"test-only daily boundary password").unwrap();
        let timezone = chrono_tz::America::Phoenix;
        let before = timezone
            .with_ymd_and_hms(2026, 8, 20, 23, 59, 0)
            .single()
            .unwrap()
            .timestamp();
        let after = timezone
            .with_ymd_and_hms(2026, 8, 21, 0, 1, 0)
            .single()
            .unwrap()
            .timestamp();
        let caller = database
            .create_caller(
                b"Daily Boundary Caller",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                before - 86_400,
            )
            .unwrap();
        let config = CallerConfig {
            maximum_daily_calls: 1,
            minutes_per_day: 5,
            minutes_per_call: 5,
            ..CallerConfig::default()
        };
        let first = database
            .begin_caller_session(&caller, &config, before, timezone)
            .unwrap();
        assert_eq!(first.calls_today, 1);
        database
            .finish_caller_session(
                caller.id,
                240,
                240,
                i64::from(board_local_day(before, timezone).unwrap()),
            )
            .unwrap();
        let stored = database.caller_by_id(caller.id).unwrap().unwrap();
        assert!(matches!(
            database.begin_caller_session(&stored, &config, before + 30, timezone),
            Err(DatabaseError::DailyCallLimitReached)
        ));
        let next_day = database
            .begin_caller_session(&stored, &config, after, timezone)
            .unwrap();
        assert_eq!(next_day.calls_today, 1);
        assert_eq!(next_day.time_used_today_seconds, 0);
        assert_eq!(next_day.allowance.limit_seconds(), 300);
    }
}
