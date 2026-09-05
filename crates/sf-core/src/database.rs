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

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use thiserror::Error;

use crate::{
    board_local_day, canonicalize_caller_name, canonicalize_login_identifier,
    derive_login_identifier_base, parse_birth_date, AccessDenialReason, AuthenticatedCaller,
    Caller, CallerAccessDenial, CallerConfig, CallerError, CallerId, CallerPreferences,
    CallerProfile, CallerProfilePolicy, CallerState, CredentialError, CredentialHasher,
    GraphicsPreference, PostalAddress, PublicInformationError, SecurityLevel, TimePolicy,
    TransferPreference, CREDENTIAL_SCHEME, MAX_LOGIN_IDENTIFIER_BYTES,
};
use crate::{
    insert_operational_event_tx, EventAttributes, EventCategory, EventOutcome, EventSeverity,
    NewOperationalEvent, RetentionClass,
};
use crate::{BoardIdentity, BoardIdentityError};

pub const SCHEMA_VERSION: u32 = 19;

const CALLER_SELECT: &str = r#"
SELECT c.caller_id, c.login_identifier, c.display_name, c.normalized_name, c.real_name,
       MIN(c.security_level, COALESCE((
           SELECT MIN(a.target_security_level)
             FROM caller_security_adjustments AS a
            WHERE a.caller_id = c.caller_id AND a.status = 'active'
       ), c.security_level)),
       c.account_state, c.first_call_at, c.last_call_at, c.call_count,
       c.total_time_seconds, c.messages_posted, c.files_uploaded, c.upload_bytes,
       c.files_downloaded, c.download_bytes, c.graphics_preference,
       c.screen_width, c.page_length, c.more_prompt, c.scroll_prompt, c.hot_keys,
       c.transfer_protocol, c.address_line_1, c.address_line_2, c.city, c.region,
       c.postal_code, c.country, c.phone, c.email, c.birthday, c.is_new_caller,
       c.security_level, c.state_version, c.subscription_expires_on,
       c.purge_protected, c.lifecycle_prior_state,
       c.public_directory_listed, c.publicity_state_version
  FROM callers AS c
"#;

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: [Migration; 19] = [
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
    Migration {
        version: 11,
        name: "auditable_message_mutation",
        sql: r#"
        CREATE TABLE migration_11_validation (
            message_count INTEGER NOT NULL,
            receipt_count INTEGER NOT NULL,
            last_read_count INTEGER NOT NULL,
            deleted_count INTEGER NOT NULL,
            private_count INTEGER NOT NULL,
            parent_count INTEGER NOT NULL,
            message_id_sum INTEGER NOT NULL,
            message_number_sum INTEGER NOT NULL,
            subject_bytes INTEGER NOT NULL,
            body_bytes INTEGER NOT NULL
        );

        INSERT INTO migration_11_validation
        SELECT
            COUNT(*),
            (SELECT COUNT(*) FROM caller_message_receipts),
            (SELECT COUNT(*) FROM caller_last_read),
            COALESCE(SUM(deleted), 0),
            COALESCE(SUM(visibility = 'private'), 0),
            COALESCE(SUM(parent_message_id IS NOT NULL), 0),
            COALESCE(SUM(message_id), 0),
            COALESCE(SUM(message_number), 0),
            COALESCE(SUM(length(subject)), 0),
            COALESCE(SUM(length(body)), 0)
        FROM messages;

        ALTER TABLE message_conferences ADD COLUMN caller_deletion_enabled INTEGER
            NOT NULL DEFAULT 1 CHECK (caller_deletion_enabled IN (0, 1));

        ALTER TABLE caller_message_receipts RENAME TO caller_message_receipts_v10;
        ALTER TABLE messages RENAME TO messages_v10;

        CREATE TABLE message_payloads (
            payload_id INTEGER PRIMARY KEY,
            subject BLOB NOT NULL CHECK (length(subject) BETWEEN 1 AND 72),
            body BLOB NOT NULL CHECK (length(body) BETWEEN 1 AND 65536),
            content_kind TEXT NOT NULL CHECK (content_kind IN ('standard', 'sysop-comment'))
        );

        CREATE TABLE message_fanouts (
            fanout_id INTEGER PRIMARY KEY,
            payload_id INTEGER NOT NULL REFERENCES message_payloads(payload_id) ON DELETE RESTRICT,
            created_by_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE messages (
            message_id INTEGER PRIMARY KEY,
            fanout_id INTEGER NOT NULL REFERENCES message_fanouts(fanout_id) ON DELETE RESTRICT,
            conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id) ON DELETE RESTRICT,
            message_number INTEGER NOT NULL CHECK (message_number > 0),
            author_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            author_name TEXT NOT NULL CHECK (length(author_name) BETWEEN 1 AND 60),
            created_at INTEGER NOT NULL,
            placed_at INTEGER NOT NULL,
            parent_message_id INTEGER REFERENCES messages(message_id) ON DELETE RESTRICT,
            audience_kind TEXT NOT NULL CHECK (audience_kind IN ('all-callers', 'local-recipient')),
            visibility TEXT NOT NULL CHECK (visibility IN ('public', 'private')),
            lifecycle_state TEXT NOT NULL CHECK (lifecycle_state IN ('active', 'deleted')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            delivery_role TEXT NOT NULL CHECK (delivery_role IN ('single', 'primary', 'cc')),
            delivery_ordinal INTEGER NOT NULL CHECK (delivery_ordinal BETWEEN 0 AND 9),
            primary_delivery_id INTEGER REFERENCES messages(message_id) ON DELETE RESTRICT,
            UNIQUE (conference_id, message_number),
            UNIQUE (fanout_id, delivery_ordinal),
            UNIQUE (message_id, fanout_id),
            CHECK (
                (delivery_role = 'single' AND delivery_ordinal = 0 AND primary_delivery_id IS NULL)
                OR (delivery_role = 'primary' AND delivery_ordinal = 0 AND primary_delivery_id = message_id)
                OR (delivery_role = 'cc' AND delivery_ordinal BETWEEN 1 AND 9 AND primary_delivery_id IS NOT NULL)
            ),
            CHECK (visibility = 'public' OR audience_kind = 'local-recipient')
        );

        CREATE TABLE message_delivery_recipients (
            message_id INTEGER PRIMARY KEY,
            fanout_id INTEGER NOT NULL,
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            display_name_snapshot TEXT NOT NULL CHECK (length(display_name_snapshot) BETWEEN 1 AND 60),
            added_at INTEGER NOT NULL,
            FOREIGN KEY (message_id, fanout_id)
                REFERENCES messages(message_id, fanout_id) ON DELETE CASCADE,
            UNIQUE (fanout_id, caller_id)
        );

        CREATE TABLE message_mutation_events (
            event_id INTEGER PRIMARY KEY,
            occurred_at INTEGER NOT NULL,
            operation TEXT NOT NULL CHECK (operation IN (
                'cc-created', 'deleted', 'undeleted', 'visibility-changed',
                'copied', 'forwarded'
            )),
            actor_caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            actor_name_snapshot TEXT NOT NULL CHECK (length(actor_name_snapshot) BETWEEN 1 AND 60),
            message_id INTEGER NOT NULL REFERENCES messages(message_id) ON DELETE RESTRICT,
            derived_message_id INTEGER REFERENCES messages(message_id) ON DELETE RESTRICT,
            prior_state_version INTEGER,
            new_state_version INTEGER NOT NULL CHECK (new_state_version > 0),
            source_conference_id INTEGER REFERENCES message_conferences(conference_id) ON DELETE RESTRICT,
            source_message_number INTEGER,
            destination_conference_id INTEGER REFERENCES message_conferences(conference_id) ON DELETE RESTRICT,
            destination_message_number INTEGER,
            prior_lifecycle TEXT CHECK (prior_lifecycle IS NULL OR prior_lifecycle IN ('active', 'deleted')),
            new_lifecycle TEXT CHECK (new_lifecycle IS NULL OR new_lifecycle IN ('active', 'deleted')),
            prior_visibility TEXT CHECK (prior_visibility IS NULL OR prior_visibility IN ('public', 'private')),
            new_visibility TEXT CHECK (new_visibility IS NULL OR new_visibility IN ('public', 'private')),
            prior_audience TEXT CHECK (prior_audience IS NULL OR prior_audience IN ('all-callers', 'local-recipient')),
            new_audience TEXT CHECK (new_audience IS NULL OR new_audience IN ('all-callers', 'local-recipient')),
            recipient_count INTEGER CHECK (recipient_count IS NULL OR recipient_count BETWEEN 0 AND 10),
            outcome TEXT NOT NULL DEFAULT 'applied' CHECK (outcome = 'applied')
        );

        CREATE TABLE message_lineage (
            derived_message_id INTEGER PRIMARY KEY REFERENCES messages(message_id) ON DELETE RESTRICT,
            source_message_id INTEGER NOT NULL REFERENCES messages(message_id) ON DELETE RESTRICT,
            relation TEXT NOT NULL CHECK (relation IN ('copy', 'forward')),
            mutation_event_id INTEGER NOT NULL UNIQUE REFERENCES message_mutation_events(event_id) ON DELETE RESTRICT,
            CHECK (derived_message_id <> source_message_id)
        );

        CREATE TABLE caller_message_receipts (
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE CASCADE,
            message_id INTEGER NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
            received_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (caller_id, message_id)
        );

        INSERT INTO message_payloads (payload_id, subject, body, content_kind)
        SELECT message_id, subject, body, kind FROM messages_v10 ORDER BY message_id;

        INSERT INTO message_fanouts (fanout_id, payload_id, created_by_caller_id, created_at)
        SELECT message_id, message_id, author_caller_id, created_at
          FROM messages_v10 ORDER BY message_id;

        INSERT INTO messages (
            message_id, fanout_id, conference_id, message_number,
            author_caller_id, author_name, created_at, placed_at,
            parent_message_id, audience_kind, visibility, lifecycle_state,
            state_version, delivery_role, delivery_ordinal, primary_delivery_id
        )
        SELECT
            message_id, message_id, conference_id, message_number,
            author_caller_id, author_name, created_at, created_at,
            parent_message_id,
            CASE WHEN recipient_caller_id IS NULL THEN 'all-callers' ELSE 'local-recipient' END,
            visibility,
            CASE WHEN deleted = 1 THEN 'deleted' ELSE 'active' END,
            1,
            CASE WHEN recipient_caller_id IS NULL THEN 'single' ELSE 'primary' END,
            0,
            CASE WHEN recipient_caller_id IS NULL THEN NULL ELSE message_id END
        FROM messages_v10 ORDER BY message_id;

        INSERT INTO message_delivery_recipients (
            message_id, fanout_id, caller_id, display_name_snapshot, added_at
        )
        SELECT message_id, message_id, recipient_caller_id, recipient_name, created_at
          FROM messages_v10
         WHERE recipient_caller_id IS NOT NULL
         ORDER BY message_id;

        INSERT INTO caller_message_receipts (caller_id, message_id, received_at)
        SELECT caller_id, message_id, received_at
          FROM caller_message_receipts_v10;

        DROP TABLE caller_message_receipts_v10;
        DROP TABLE messages_v10;

        CREATE INDEX messages_conference_scan
            ON messages (conference_id, message_number, lifecycle_state);
        CREATE INDEX messages_author_scan
            ON messages (author_caller_id, visibility, lifecycle_state);
        CREATE INDEX message_recipients_caller_scan
            ON message_delivery_recipients (caller_id, message_id);
        CREATE INDEX caller_message_receipts_message
            ON caller_message_receipts (message_id, caller_id);
        CREATE INDEX message_mutation_events_message
            ON message_mutation_events (message_id, event_id);
        CREATE INDEX message_mutation_events_actor
            ON message_mutation_events (actor_caller_id, event_id);

        CREATE TRIGGER message_payloads_immutable_update
        BEFORE UPDATE ON message_payloads
        BEGIN
            SELECT RAISE(ABORT, 'message payloads are immutable');
        END;

        CREATE TRIGGER message_payloads_immutable_delete
        BEFORE DELETE ON message_payloads
        BEGIN
            SELECT RAISE(ABORT, 'message payloads are immutable');
        END;

        CREATE TRIGGER message_mutation_events_append_only_update
        BEFORE UPDATE ON message_mutation_events
        BEGIN
            SELECT RAISE(ABORT, 'message mutation events are append-only');
        END;

        CREATE TRIGGER message_mutation_events_append_only_delete
        BEFORE DELETE ON message_mutation_events
        BEGIN
            SELECT RAISE(ABORT, 'message mutation events are append-only');
        END;
    "#,
    },
    Migration {
        version: 12,
        name: "auditable_caller_access_lifecycle",
        sql: r#"
        CREATE TABLE migration_12_validation (
            caller_count INTEGER NOT NULL,
            caller_id_sum INTEGER NOT NULL,
            security_sum INTEGER NOT NULL,
            active_count INTEGER NOT NULL,
            disabled_count INTEGER NOT NULL,
            deleted_count INTEGER NOT NULL,
            credential_count INTEGER NOT NULL,
            profile_bytes INTEGER NOT NULL
        );
        INSERT INTO migration_12_validation
        SELECT COUNT(*), COALESCE(SUM(caller_id), 0), COALESCE(SUM(security_level), 0),
               COALESCE(SUM(account_state='active'), 0),
               COALESCE(SUM(account_state='disabled'), 0),
               COALESCE(SUM(account_state='deleted'), 0),
               (SELECT COUNT(*) FROM caller_credentials),
               COALESCE(SUM(length(COALESCE(address_line_1,'')) + length(COALESCE(address_line_2,''))
                 + length(COALESCE(city,'')) + length(COALESCE(region,''))
                 + length(COALESCE(postal_code,'')) + length(COALESCE(country,''))
                 + length(COALESCE(phone,'')) + length(COALESCE(email,''))
                 + length(COALESCE(birthday,''))), 0)
          FROM callers;

        ALTER TABLE callers ADD COLUMN state_version INTEGER NOT NULL DEFAULT 0
            CHECK (state_version >= 0);
        ALTER TABLE callers ADD COLUMN subscription_expires_on TEXT
            CHECK (subscription_expires_on IS NULL OR (
                length(subscription_expires_on) = 10
                AND substr(subscription_expires_on, 5, 1) = '-'
                AND substr(subscription_expires_on, 8, 1) = '-'
            ));
        ALTER TABLE callers ADD COLUMN purge_protected INTEGER NOT NULL DEFAULT 1
            CHECK (purge_protected IN (0, 1));
        ALTER TABLE callers ADD COLUMN lifecycle_prior_state TEXT
            CHECK (lifecycle_prior_state IS NULL OR lifecycle_prior_state IN ('active', 'disabled'));

        CREATE TABLE caller_access_events (
            event_id INTEGER PRIMARY KEY,
            occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
            operation TEXT NOT NULL CHECK (operation IN (
                'caller-created', 'security-changed', 'disabled', 'enabled',
                'tombstoned', 'restored', 'purge-protection-changed',
                'subscription-updated', 'subscription-expired',
                'subscription-adjustment-resolved', 'subscription-warning',
                'joker-denied'
            )),
            outcome TEXT NOT NULL DEFAULT 'committed' CHECK (outcome IN ('committed', 'denied')),
            subject_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            actor_kind TEXT NOT NULL CHECK (actor_kind IN ('caller', 'threshold-sysop', 'local-operator', 'system-policy')),
            actor_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            prior_lifecycle TEXT CHECK (prior_lifecycle IS NULL OR prior_lifecycle IN ('active', 'disabled', 'deleted')),
            new_lifecycle TEXT CHECK (new_lifecycle IS NULL OR new_lifecycle IN ('active', 'disabled', 'deleted')),
            prior_state_version INTEGER CHECK (prior_state_version IS NULL OR prior_state_version >= 0),
            new_state_version INTEGER CHECK (new_state_version IS NULL OR new_state_version >= 0),
            prior_base_security INTEGER CHECK (prior_base_security IS NULL OR prior_base_security BETWEEN 0 AND 9999),
            new_base_security INTEGER CHECK (new_base_security IS NULL OR new_base_security BETWEEN 0 AND 9999),
            adjustment_kind TEXT CHECK (adjustment_kind IS NULL OR adjustment_kind = 'subscription-expired'),
            policy_generation INTEGER CHECK (policy_generation IS NULL OR policy_generation > 0)
        );

        CREATE TABLE caller_security_adjustments (
            adjustment_id INTEGER PRIMARY KEY,
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            kind TEXT NOT NULL CHECK (kind = 'subscription-expired'),
            target_security_level INTEGER NOT NULL CHECK (target_security_level BETWEEN 0 AND 9999),
            status TEXT NOT NULL CHECK (status IN ('active', 'resolved')),
            applied_at INTEGER NOT NULL CHECK (applied_at >= 0),
            resolved_at INTEGER CHECK (
                (status = 'active' AND resolved_at IS NULL)
                OR (status = 'resolved' AND resolved_at IS NOT NULL AND resolved_at >= applied_at)
            ),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            applied_event_id INTEGER NOT NULL REFERENCES caller_access_events(event_id) ON DELETE RESTRICT,
            resolved_event_id INTEGER REFERENCES caller_access_events(event_id) ON DELETE RESTRICT
        );
        CREATE UNIQUE INDEX caller_security_adjustments_one_active_kind
            ON caller_security_adjustments (caller_id, kind) WHERE status = 'active';
        CREATE INDEX caller_access_events_subject
            ON caller_access_events (subject_caller_id, event_id);

        CREATE TRIGGER caller_access_events_no_update
        BEFORE UPDATE ON caller_access_events BEGIN
            SELECT RAISE(ABORT, 'caller access events are append-only');
        END;
        CREATE TRIGGER caller_access_events_no_delete
        BEFORE DELETE ON caller_access_events BEGIN
            SELECT RAISE(ABORT, 'caller access events are append-only');
        END;
        "#,
    },
    Migration {
        version: 13,
        name: "durable_caller_identity",
        sql: r#"
        ALTER TABLE callers ADD COLUMN login_identifier TEXT
            CHECK (login_identifier IS NULL OR length(login_identifier) BETWEEN 1 AND 32);
        ALTER TABLE callers ADD COLUMN real_name TEXT
            CHECK (real_name IS NULL OR length(real_name) BETWEEN 1 AND 120);

        CREATE TABLE caller_identity_events (
            event_id INTEGER PRIMARY KEY,
            occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            prior_state_version INTEGER NOT NULL CHECK (prior_state_version >= 0),
            new_state_version INTEGER NOT NULL CHECK (new_state_version = prior_state_version + 1),
            actor_kind TEXT NOT NULL CHECK (actor_kind = 'local-operator')
        );
        CREATE INDEX caller_identity_events_caller
            ON caller_identity_events (caller_id, event_id);
        CREATE TRIGGER caller_identity_events_no_update
        BEFORE UPDATE ON caller_identity_events BEGIN
            SELECT RAISE(ABORT, 'caller identity events are append-only');
        END;
        CREATE TRIGGER caller_identity_events_no_delete
        BEFORE DELETE ON caller_identity_events BEGIN
            SELECT RAISE(ABORT, 'caller identity events are append-only');
        END;
        "#,
    },
    Migration {
        version: 14,
        name: "privacy_bounded_public_information",
        sql: r#"
        CREATE TABLE migration_14_validation AS
        SELECT COUNT(*) AS caller_count,
               COALESCE(SUM(caller_id),0) AS caller_id_sum,
               (SELECT COUNT(*) FROM caller_credentials) AS credential_count,
               (SELECT COUNT(*) FROM messages) AS message_count,
               (SELECT COUNT(*) FROM files) AS file_count,
               COALESCE(SUM(length(login_identifier)),0) AS login_bytes,
               COALESCE(SUM(length(display_name)),0) AS handle_bytes,
               COALESCE(SUM(length(COALESCE(real_name,''))),0) AS real_name_bytes,
               COALESCE(SUM(state_version),0) AS caller_state_sum
          FROM callers;

        ALTER TABLE callers ADD COLUMN public_directory_listed INTEGER NOT NULL DEFAULT 0
            CHECK (public_directory_listed IN (0,1));
        ALTER TABLE callers ADD COLUMN publicity_state_version INTEGER NOT NULL DEFAULT 0
            CHECK (publicity_state_version >= 0);

        CREATE TABLE public_information_policy (
            singleton INTEGER PRIMARY KEY CHECK (singleton=1),
            directory_enabled INTEGER NOT NULL DEFAULT 0 CHECK (directory_enabled IN (0,1)),
            show_last_call_date INTEGER NOT NULL DEFAULT 0 CHECK (show_last_call_date IN (0,1)),
            show_city_region INTEGER NOT NULL DEFAULT 0 CHECK (show_city_region IN (0,1)),
            caller_bbs_additions_enabled INTEGER NOT NULL DEFAULT 0 CHECK (caller_bbs_additions_enabled IN (0,1)),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        INSERT INTO public_information_policy (singleton) VALUES (1);

        CREATE TABLE other_bbs_entries (
            entry_id INTEGER PRIMARY KEY,
            bbs_name TEXT NOT NULL CHECK (length(CAST(bbs_name AS BLOB)) BETWEEN 1 AND 60),
            speed_label TEXT NOT NULL CHECK (length(CAST(speed_label AS BLOB)) BETWEEN 1 AND 32),
            dial_string TEXT NOT NULL CHECK (length(CAST(dial_string AS BLOB)) BETWEEN 1 AND 64),
            display_order INTEGER NOT NULL CHECK (display_order BETWEEN 1 AND 512),
            lifecycle TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('active','disabled')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            contributor_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX other_bbs_entries_order ON other_bbs_entries(display_order,entry_id);

        CREATE TABLE public_information_resource_state (
            resource_kind TEXT PRIMARY KEY CHECK (resource_kind IN ('bulletins','newsletter','thoughts')),
            generation INTEGER NOT NULL CHECK (generation > 0),
            sha256 TEXT NOT NULL CHECK (length(sha256)=64),
            published_at INTEGER NOT NULL CHECK (published_at >= 0)
        );

        CREATE TABLE public_information_events (
            event_id INTEGER PRIMARY KEY,
            occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
            operation TEXT NOT NULL CHECK (operation IN (
                'policy-changed','caller-listed','caller-unlisted',
                'other-bbs-added','other-bbs-edited','other-bbs-reordered',
                'other-bbs-disabled','other-bbs-restored','resource-published'
            )),
            actor_kind TEXT NOT NULL CHECK (actor_kind IN ('caller','threshold-sysop','local-operator','system-policy')),
            actor_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            subject_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            other_bbs_entry_id INTEGER REFERENCES other_bbs_entries(entry_id) ON DELETE RESTRICT,
            resource_kind TEXT CHECK (resource_kind IS NULL OR resource_kind IN ('bulletins','newsletter','thoughts')),
            resource_digest TEXT CHECK (resource_digest IS NULL OR (
                length(resource_digest)=64 AND resource_digest NOT GLOB '*[^0-9a-f]*'
            )),
            semantic_detail TEXT CHECK (semantic_detail IS NULL OR length(semantic_detail) BETWEEN 1 AND 128),
            prior_state_version INTEGER CHECK (prior_state_version IS NULL OR prior_state_version >= 0),
            new_state_version INTEGER CHECK (new_state_version IS NULL OR new_state_version >= 0)
        );
        CREATE INDEX public_information_events_subject ON public_information_events(subject_caller_id,event_id);
        CREATE INDEX public_information_events_other_bbs ON public_information_events(other_bbs_entry_id,event_id);
        CREATE TRIGGER public_information_events_no_update BEFORE UPDATE ON public_information_events
        BEGIN SELECT RAISE(ABORT, 'public information events are append-only'); END;
        CREATE TRIGGER public_information_events_no_delete BEFORE DELETE ON public_information_events
        BEGIN SELECT RAISE(ABORT, 'public information events are append-only'); END;
        "#,
    },
    Migration {
        version: 15,
        name: "safe_file_inspection_request_maintenance",
        sql: r#"
        CREATE TABLE migration_15_validation AS
        SELECT COUNT(*) AS caller_count,
               COALESCE(SUM(caller_id),0) AS caller_id_sum,
               (SELECT COUNT(*) FROM caller_credentials) AS credential_count,
               (SELECT COUNT(*) FROM messages) AS message_count,
               (SELECT COUNT(*) FROM file_areas) AS area_count,
               (SELECT COALESCE(SUM(area_id),0) FROM file_areas) AS area_id_sum,
               (SELECT COUNT(*) FROM files) AS file_count,
               (SELECT COALESCE(SUM(file_id),0) FROM files) AS file_id_sum,
               (SELECT COALESCE(SUM(size_bytes),0) FROM files) AS file_size_sum,
               (SELECT COALESCE(SUM(download_count),0) FROM files) AS download_count_sum,
               (SELECT COUNT(*) FROM public_information_policy) AS public_policy_count,
               (SELECT COUNT(*) FROM other_bbs_entries) AS other_bbs_count
          FROM callers;

        ALTER TABLE file_areas ADD COLUMN state_version INTEGER NOT NULL DEFAULT 1
            CHECK (state_version > 0);

        DROP INDEX files_area_listing;
        DROP INDEX files_upload_time;
        ALTER TABLE files RENAME TO files_schema_14;

        CREATE TABLE files (
            file_id INTEGER PRIMARY KEY,
            area_id INTEGER NOT NULL REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            filename TEXT NOT NULL CHECK (length(filename) BETWEEN 1 AND 64),
            normalized_filename TEXT NOT NULL CHECK (length(normalized_filename) BETWEEN 1 AND 64),
            description TEXT NOT NULL CHECK (length(CAST(description AS BLOB)) BETWEEN 1 AND 4096),
            size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
            sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
            uploaded_at INTEGER NOT NULL,
            uploader_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            uploader_name TEXT NOT NULL CHECK (length(uploader_name) BETWEEN 1 AND 60),
            download_count INTEGER NOT NULL DEFAULT 0 CHECK (download_count >= 0),
            lifecycle TEXT NOT NULL DEFAULT 'active'
                CHECK (lifecycle IN ('active','offline','pending-review','disabled','tombstoned')),
            integrity_state TEXT NOT NULL DEFAULT 'unknown'
                CHECK (integrity_state IN ('unknown','present','missing','digest-mismatch')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            description_source TEXT NOT NULL DEFAULT 'legacy-import'
                CHECK (description_source IN ('caller','operator','file-id-diz','legacy-import','system')),
            description_source_digest TEXT CHECK (description_source_digest IS NULL OR
                (length(description_source_digest)=64 AND description_source_digest NOT GLOB '*[^0-9a-f]*')),
            review_submitted_at INTEGER CHECK (review_submitted_at IS NULL OR review_submitted_at >= 0),
            reviewed_by_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            reviewed_at INTEGER CHECK (reviewed_at IS NULL OR reviewed_at >= 0),
            tombstoned_at INTEGER CHECK (tombstoned_at IS NULL OR tombstoned_at >= 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        INSERT INTO files (
            file_id, area_id, filename, normalized_filename, description,
            size_bytes, sha256, uploaded_at, uploader_caller_id, uploader_name,
            download_count, lifecycle, integrity_state, state_version,
            description_source, created_at, updated_at
        )
        SELECT file_id, area_id, filename, normalized_filename, description,
               size_bytes, lower(sha256), uploaded_at, uploader_caller_id,
               uploader_name, download_count,
               CASE state WHEN 'available' THEN 'active' ELSE 'disabled' END,
               'unknown', 1, 'legacy-import', created_at, updated_at
          FROM files_schema_14;
        DROP TABLE files_schema_14;

        CREATE UNIQUE INDEX files_area_filename_live
            ON files(area_id, normalized_filename)
            WHERE lifecycle <> 'tombstoned';
        CREATE INDEX files_area_listing
            ON files(area_id, normalized_filename, lifecycle, integrity_state);
        CREATE INDEX files_upload_time
            ON files(uploaded_at, area_id, lifecycle);

        CREATE TABLE file_policy (
            singleton INTEGER PRIMARY KEY CHECK (singleton=1),
            comprehensive_upload_search INTEGER NOT NULL DEFAULT 0
                CHECK (comprehensive_upload_search IN (0,1)),
            description_normalization TEXT NOT NULL DEFAULT 'preserve'
                CHECK (description_normalization IN ('preserve','historical-uppercase')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        INSERT INTO file_policy(singleton) VALUES(1);

        CREATE TABLE file_upload_denials (
            rule_id INTEGER PRIMARY KEY,
            pattern TEXT NOT NULL CHECK (length(CAST(pattern AS BLOB)) BETWEEN 1 AND 64),
            normalized_pattern TEXT NOT NULL UNIQUE
                CHECK (length(normalized_pattern) BETWEEN 1 AND 64),
            lifecycle TEXT NOT NULL DEFAULT 'active'
                CHECK (lifecycle IN ('active','disabled')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE file_uppercase_terms (
            term_id INTEGER PRIMARY KEY,
            term TEXT NOT NULL CHECK (length(CAST(term AS BLOB)) BETWEEN 1 AND 32),
            normalized_term TEXT NOT NULL UNIQUE
                CHECK (length(normalized_term) BETWEEN 1 AND 32),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0)
        );

        CREATE TABLE file_requests (
            request_id INTEGER PRIMARY KEY,
            file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE RESTRICT,
            requesting_caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            created_board_day TEXT NOT NULL CHECK (length(created_board_day)=10),
            reason TEXT NOT NULL CHECK (reason IN ('offline','missing')),
            reason_detail TEXT CHECK (reason_detail IS NULL OR
                length(CAST(reason_detail AS BLOB)) BETWEEN 1 AND 255),
            status TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending','fulfilled','rejected','cancelled','stale')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            resolved_at TEXT,
            resolved_by_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE UNIQUE INDEX file_requests_one_pending
            ON file_requests(file_id, requesting_caller_id) WHERE status='pending';
        CREATE INDEX file_requests_status ON file_requests(status, request_id);

        CREATE TABLE file_operations (
            operation_id TEXT PRIMARY KEY CHECK (length(operation_id) BETWEEN 1 AND 96),
            kind TEXT NOT NULL CHECK (kind IN
                ('add','upload','import','move','remove','review-accept','review-reject',
                 'reconcile','publish-listing')),
            file_id INTEGER REFERENCES files(file_id) ON DELETE RESTRICT,
            source_area_id INTEGER REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            destination_area_id INTEGER REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            expected_file_version INTEGER CHECK (expected_file_version IS NULL OR expected_file_version > 0),
            expected_area_version INTEGER CHECK (expected_area_version IS NULL OR expected_area_version > 0),
            phase TEXT NOT NULL CHECK (phase IN
                ('planned','staged','catalog-prepared','bytes-published','catalog-committed',
                 'committed','rolled-back','orphaned','needs-review')),
            staging_path TEXT CHECK (staging_path IS NULL OR length(staging_path) BETWEEN 1 AND 255),
            digest TEXT CHECK (digest IS NULL OR
                (length(digest)=64 AND digest NOT GLOB '*[^0-9a-f]*')),
            actor_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            error_code TEXT CHECK (error_code IS NULL OR length(error_code) BETWEEN 1 AND 64),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT
        );
        CREATE INDEX file_operations_phase ON file_operations(phase, operation_id);

        CREATE TABLE file_operation_leases (
            lease_id INTEGER PRIMARY KEY,
            operation_id TEXT NOT NULL REFERENCES file_operations(operation_id) ON DELETE CASCADE,
            lease_kind TEXT NOT NULL CHECK (lease_kind IN ('file','area','name')),
            file_id INTEGER REFERENCES files(file_id) ON DELETE RESTRICT,
            area_id INTEGER REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            normalized_filename TEXT,
            expires_at TEXT NOT NULL,
            CHECK ((lease_kind='file')=(file_id IS NOT NULL)),
            CHECK ((lease_kind IN ('area','name'))=(area_id IS NOT NULL)),
            CHECK ((lease_kind='name')=(normalized_filename IS NOT NULL))
        );
        CREATE UNIQUE INDEX file_operation_file_lease
            ON file_operation_leases(file_id) WHERE lease_kind='file';
        CREATE UNIQUE INDEX file_operation_name_lease
            ON file_operation_leases(area_id, normalized_filename) WHERE lease_kind='name';
        CREATE UNIQUE INDEX file_operation_area_lease
            ON file_operation_leases(area_id) WHERE lease_kind='area';

        CREATE TABLE file_active_uses (
            use_id TEXT PRIMARY KEY CHECK (length(use_id) BETWEEN 1 AND 96),
            file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE RESTRICT,
            session_id INTEGER NOT NULL CHECK (session_id > 0),
            use_kind TEXT NOT NULL CHECK (use_kind IN ('inspect','download')),
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX file_active_uses_file ON file_active_uses(file_id,expires_at);

        CREATE TABLE file_legacy_publications (
            area_id INTEGER PRIMARY KEY REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            generation INTEGER NOT NULL CHECK (generation > 0),
            sha256 TEXT NOT NULL CHECK (length(sha256)=64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            published_at INTEGER NOT NULL CHECK (published_at >= 0)
        );

        CREATE TABLE file_events (
            event_id INTEGER PRIMARY KEY,
            occurred_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            operation TEXT NOT NULL CHECK (operation IN
                ('file-added','file-imported','metadata-edited','file-moved',
                 'file-tombstoned','request-created','request-cancelled',
                 'request-resolved','review-accepted','review-rejected',
                 'integrity-updated','reconciled','listing-published',
                 'denial-policy-changed','description-policy-changed',
                 'lifecycle-changed','file-id-diz-applied')),
            actor_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            file_id INTEGER REFERENCES files(file_id) ON DELETE RESTRICT,
            area_id INTEGER REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            request_id INTEGER REFERENCES file_requests(request_id) ON DELETE RESTRICT,
            operation_id TEXT REFERENCES file_operations(operation_id) ON DELETE RESTRICT,
            prior_version INTEGER CHECK (prior_version IS NULL OR prior_version >= 0),
            next_version INTEGER CHECK (next_version IS NULL OR next_version >= 0),
            digest TEXT CHECK (digest IS NULL OR
                (length(digest)=64 AND digest NOT GLOB '*[^0-9a-f]*')),
            detail TEXT CHECK (detail IS NULL OR
                length(CAST(detail AS BLOB)) BETWEEN 1 AND 96)
        );
        CREATE INDEX file_events_file ON file_events(file_id,event_id);
        CREATE INDEX file_events_request ON file_events(request_id,event_id);
        CREATE TRIGGER file_events_no_update BEFORE UPDATE ON file_events
        BEGIN SELECT RAISE(ABORT, 'file events are append-only'); END;
        CREATE TRIGGER file_events_no_delete BEFORE DELETE ON file_events
        BEGIN SELECT RAISE(ABORT, 'file events are append-only'); END;
        "#,
    },
    Migration {
        version: 16,
        name: "batch_transfer_policy_extended_storage",
        sql: r#"
        CREATE TABLE migration_16_validation AS
        SELECT COUNT(*) AS caller_count,
               COALESCE(SUM(caller_id),0) AS caller_id_sum,
               (SELECT COUNT(*) FROM caller_credentials) AS credential_count,
               (SELECT COUNT(*) FROM messages) AS message_count,
               (SELECT COUNT(*) FROM file_areas) AS area_count,
               (SELECT COALESCE(SUM(area_id),0) FROM file_areas) AS area_id_sum,
               (SELECT COUNT(*) FROM files) AS file_count,
               (SELECT COALESCE(SUM(file_id),0) FROM files) AS file_id_sum,
               (SELECT COALESCE(SUM(size_bytes),0) FROM files) AS file_size_sum,
               (SELECT COALESCE(SUM(download_count),0) FROM files) AS download_count_sum,
               (SELECT COUNT(*) FROM file_requests) AS request_count,
               (SELECT COUNT(*) FROM file_operations) AS operation_count,
               (SELECT COUNT(*) FROM public_information_policy) AS public_policy_count,
               (SELECT COUNT(*) FROM other_bbs_entries) AS other_bbs_count,
               (SELECT COUNT(*) FROM caller_access_events) AS access_event_count,
               (SELECT COUNT(*) FROM caller_security_adjustments) AS adjustment_count
          FROM callers;

        CREATE TEMP TABLE migration_16_caller_access_events AS
        SELECT * FROM caller_access_events;
        CREATE TEMP TABLE migration_16_caller_security_adjustments AS
        SELECT * FROM caller_security_adjustments;
        DROP TRIGGER caller_access_events_no_update;
        DROP TRIGGER caller_access_events_no_delete;
        DROP INDEX caller_security_adjustments_one_active_kind;
        DROP TABLE caller_security_adjustments;
        DROP TABLE caller_access_events;

        CREATE TABLE caller_access_events (
            event_id INTEGER PRIMARY KEY,
            occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
            operation TEXT NOT NULL CHECK (operation IN (
                'caller-created', 'security-changed', 'disabled', 'enabled',
                'tombstoned', 'restored', 'purge-protection-changed',
                'subscription-updated', 'subscription-expired',
                'subscription-adjustment-resolved', 'subscription-warning',
                'joker-denied', 'ratio-adjustment-applied',
                'ratio-adjustment-resolved'
            )),
            outcome TEXT NOT NULL DEFAULT 'committed' CHECK (outcome IN ('committed', 'denied')),
            subject_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            actor_kind TEXT NOT NULL CHECK (actor_kind IN ('caller', 'threshold-sysop', 'local-operator', 'system-policy')),
            actor_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            prior_lifecycle TEXT CHECK (prior_lifecycle IS NULL OR prior_lifecycle IN ('active', 'disabled', 'deleted')),
            new_lifecycle TEXT CHECK (new_lifecycle IS NULL OR new_lifecycle IN ('active', 'disabled', 'deleted')),
            prior_state_version INTEGER CHECK (prior_state_version IS NULL OR prior_state_version >= 0),
            new_state_version INTEGER CHECK (new_state_version IS NULL OR new_state_version >= 0),
            prior_base_security INTEGER CHECK (prior_base_security IS NULL OR prior_base_security BETWEEN 0 AND 9999),
            new_base_security INTEGER CHECK (new_base_security IS NULL OR new_base_security BETWEEN 0 AND 9999),
            adjustment_kind TEXT CHECK (adjustment_kind IS NULL OR adjustment_kind IN ('subscription-expired','ratio-violation')),
            policy_generation INTEGER CHECK (policy_generation IS NULL OR policy_generation > 0)
        );
        INSERT INTO caller_access_events SELECT * FROM migration_16_caller_access_events;

        CREATE TABLE caller_security_adjustments (
            adjustment_id INTEGER PRIMARY KEY,
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            kind TEXT NOT NULL CHECK (kind IN ('subscription-expired','ratio-violation')),
            target_security_level INTEGER NOT NULL CHECK (target_security_level BETWEEN 0 AND 9999),
            status TEXT NOT NULL CHECK (status IN ('active', 'resolved')),
            applied_at INTEGER NOT NULL CHECK (applied_at >= 0),
            resolved_at INTEGER CHECK (
                (status = 'active' AND resolved_at IS NULL)
                OR (status = 'resolved' AND resolved_at IS NOT NULL AND resolved_at >= applied_at)
            ),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            applied_event_id INTEGER NOT NULL REFERENCES caller_access_events(event_id) ON DELETE RESTRICT,
            resolved_event_id INTEGER REFERENCES caller_access_events(event_id) ON DELETE RESTRICT
        );
        INSERT INTO caller_security_adjustments SELECT * FROM migration_16_caller_security_adjustments;
        CREATE UNIQUE INDEX caller_security_adjustments_one_active_kind
            ON caller_security_adjustments (caller_id, kind) WHERE status = 'active';
        CREATE TRIGGER caller_access_events_no_update
        BEFORE UPDATE ON caller_access_events BEGIN
            SELECT RAISE(ABORT, 'caller access events are append-only');
        END;
        CREATE TRIGGER caller_access_events_no_delete
        BEFORE DELETE ON caller_access_events BEGIN
            SELECT RAISE(ABORT, 'caller access events are append-only');
        END;
        DROP TABLE migration_16_caller_security_adjustments;
        DROP TABLE migration_16_caller_access_events;

        CREATE TABLE transfer_timezone_policy (
            singleton INTEGER PRIMARY KEY CHECK (singleton=1),
            timezone_name TEXT NOT NULL CHECK (length(CAST(timezone_name AS BLOB)) BETWEEN 1 AND 64),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        INSERT INTO transfer_timezone_policy(singleton,timezone_name) VALUES(1,'UTC');

        CREATE TABLE transfer_policies (
            security_level INTEGER PRIMARY KEY CHECK (security_level BETWEEN 0 AND 9999),
            daily_file_limit INTEGER CHECK (daily_file_limit IS NULL OR daily_file_limit > 0),
            daily_byte_limit INTEGER CHECK (daily_byte_limit IS NULL OR daily_byte_limit > 0),
            ratio_warning_thousandths INTEGER CHECK (ratio_warning_thousandths IS NULL OR ratio_warning_thousandths > 0),
            ratio_enforcement_thousandths INTEGER CHECK (ratio_enforcement_thousandths IS NULL OR ratio_enforcement_thousandths > 0),
            ratio_violation_security INTEGER CHECK (ratio_violation_security IS NULL OR ratio_violation_security BETWEEN 0 AND 9999),
            upload_credit_thousandths INTEGER NOT NULL DEFAULT 0 CHECK (upload_credit_thousandths BETWEEN 0 AND 100000),
            upload_credit_file_cap_seconds INTEGER NOT NULL DEFAULT 0 CHECK (upload_credit_file_cap_seconds >= 0),
            upload_credit_day_cap_seconds INTEGER NOT NULL DEFAULT 0 CHECK (upload_credit_day_cap_seconds >= 0),
            protocol_mask INTEGER NOT NULL DEFAULT 511 CHECK (protocol_mask BETWEEN 0 AND 511),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            CHECK (ratio_warning_thousandths IS NULL OR ratio_enforcement_thousandths IS NULL OR ratio_warning_thousandths <= ratio_enforcement_thousandths)
        );

        CREATE TABLE transfer_daily_usage (
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            board_day TEXT NOT NULL CHECK (length(board_day)=10),
            timezone_policy_version INTEGER NOT NULL CHECK (timezone_policy_version > 0),
            chargeable_download_files INTEGER NOT NULL DEFAULT 0 CHECK (chargeable_download_files >= 0),
            chargeable_download_bytes INTEGER NOT NULL DEFAULT 0 CHECK (chargeable_download_bytes >= 0),
            upload_credit_seconds INTEGER NOT NULL DEFAULT 0 CHECK (upload_credit_seconds >= 0),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(caller_id,board_day,timezone_policy_version)
        );

        CREATE TABLE transfer_records (
            transfer_id TEXT PRIMARY KEY CHECK (length(transfer_id) BETWEEN 1 AND 96),
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            node_id INTEGER NOT NULL CHECK (node_id > 0),
            direction TEXT NOT NULL CHECK (direction IN ('download','upload')),
            protocol TEXT NOT NULL CHECK (protocol IN ('ascii','xmodem-checksum','xmodem-crc','xmodem-1k','xmodem-1k-g','ymodem-batch','ymodem-g-batch','zmodem-batch','telink')),
            state TEXT NOT NULL CHECK (state IN ('planned','authorized','reserved','negotiating','transferring','settling','completed','cancelled','failed','needs-review')),
            bytes_expected INTEGER NOT NULL DEFAULT 0 CHECK (bytes_expected >= 0),
            bytes_transferred INTEGER NOT NULL DEFAULT 0 CHECK (bytes_transferred >= 0),
            cancel_source TEXT CHECK (cancel_source IS NULL OR cancel_source IN ('caller','operator','disconnect','timeout','daemon-shutdown')),
            error_class TEXT CHECK (error_class IS NULL OR length(error_class) BETWEEN 1 AND 64),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            started_at INTEGER NOT NULL CHECK (started_at >= 0),
            updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
            completed_at INTEGER CHECK (completed_at IS NULL OR completed_at >= started_at)
        );
        CREATE INDEX transfer_records_state ON transfer_records(state,transfer_id);
        CREATE INDEX transfer_records_caller ON transfer_records(caller_id,started_at);

        CREATE TABLE transfer_quota_reservations (
            reservation_id TEXT PRIMARY KEY CHECK (length(reservation_id) BETWEEN 1 AND 96),
            transfer_id TEXT NOT NULL UNIQUE REFERENCES transfer_records(transfer_id) ON DELETE RESTRICT,
            caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
            board_day TEXT NOT NULL CHECK (length(board_day)=10),
            timezone_policy_version INTEGER NOT NULL CHECK (timezone_policy_version > 0),
            policy_security_level INTEGER NOT NULL CHECK (policy_security_level BETWEEN 0 AND 9999),
            policy_state_version INTEGER NOT NULL CHECK (policy_state_version >= 0),
            reserved_file_count INTEGER NOT NULL CHECK (reserved_file_count >= 0),
            reserved_bytes INTEGER NOT NULL CHECK (reserved_bytes >= 0),
            state TEXT NOT NULL CHECK (state IN ('active','settled','released','needs-review')),
            expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            created_at INTEGER NOT NULL CHECK (created_at >= 0),
            updated_at INTEGER NOT NULL CHECK (updated_at >= created_at)
        );
        CREATE INDEX transfer_quota_reservations_active
            ON transfer_quota_reservations(caller_id,board_day,state,expires_at);

        CREATE TABLE transfer_quota_reservation_items (
            reservation_id TEXT NOT NULL REFERENCES transfer_quota_reservations(reservation_id) ON DELETE CASCADE,
            item_id TEXT NOT NULL CHECK (length(item_id) BETWEEN 1 AND 96),
            file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE RESTRICT,
            expected_file_version INTEGER NOT NULL CHECK (expected_file_version > 0),
            reserved_bytes INTEGER NOT NULL CHECK (reserved_bytes >= 0),
            no_charge INTEGER NOT NULL CHECK (no_charge IN (0,1)),
            state TEXT NOT NULL DEFAULT 'reserved' CHECK (state IN ('reserved','settled','released')),
            settled_bytes INTEGER CHECK (settled_bytes IS NULL OR settled_bytes >= 0),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            PRIMARY KEY(reservation_id,item_id)
        );
        CREATE INDEX transfer_reservation_items_file
            ON transfer_quota_reservation_items(file_id,state);

        CREATE TABLE transfer_settlements (
            transfer_id TEXT NOT NULL REFERENCES transfer_records(transfer_id) ON DELETE RESTRICT,
            item_id TEXT NOT NULL CHECK (length(item_id) BETWEEN 1 AND 96),
            file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE RESTRICT,
            direction TEXT NOT NULL CHECK (direction IN ('download','upload')),
            completed_bytes INTEGER NOT NULL CHECK (completed_bytes >= 0),
            no_charge INTEGER NOT NULL CHECK (no_charge IN (0,1)),
            upload_credit_seconds INTEGER NOT NULL DEFAULT 0 CHECK (upload_credit_seconds >= 0),
            settled_at INTEGER NOT NULL CHECK (settled_at >= 0),
            PRIMARY KEY(transfer_id,item_id)
        );

        CREATE TABLE file_storage_roots (
            storage_root_id INTEGER PRIMARY KEY,
            area_id INTEGER NOT NULL REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            stable_key TEXT NOT NULL UNIQUE CHECK (length(CAST(stable_key AS BLOB)) BETWEEN 1 AND 96),
            label TEXT NOT NULL CHECK (length(CAST(label AS BLOB)) BETWEEN 1 AND 96),
            root_kind TEXT NOT NULL CHECK (root_kind IN ('managed','external')),
            access_mode TEXT NOT NULL CHECK (access_mode IN ('read-write','read-only')),
            priority INTEGER NOT NULL CHECK (priority BETWEEN 0 AND 255),
            configured_locator TEXT NOT NULL CHECK (length(CAST(configured_locator AS BLOB)) BETWEEN 1 AND 255),
            configured_state TEXT NOT NULL DEFAULT 'enabled' CHECK (configured_state IN ('enabled','maintenance','disabled')),
            availability TEXT NOT NULL DEFAULT 'unknown' CHECK (availability IN ('unknown','available','unavailable')),
            media_expectation TEXT CHECK (media_expectation IS NULL OR length(CAST(media_expectation AS BLOB)) BETWEEN 1 AND 128),
            staging_policy TEXT NOT NULL DEFAULT 'direct-if-safe' CHECK (staging_policy IN ('direct-if-safe','always-stage')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(area_id,priority)
        );
        CREATE INDEX file_storage_roots_area ON file_storage_roots(area_id,priority);

        INSERT INTO file_storage_roots(
            storage_root_id,area_id,stable_key,label,root_kind,access_mode,
            priority,configured_locator,configured_state,availability,staging_policy
        )
        SELECT area_id,area_id,'area-'||area_id||'-primary',name||' primary',
               'managed','read-write',0,storage_key,'enabled','available','direct-if-safe'
          FROM file_areas;

        CREATE TABLE file_storage_locators (
            file_id INTEGER PRIMARY KEY REFERENCES files(file_id) ON DELETE RESTRICT,
            storage_root_id INTEGER NOT NULL REFERENCES file_storage_roots(storage_root_id) ON DELETE RESTRICT,
            relative_path TEXT NOT NULL CHECK (length(CAST(relative_path AS BLOB)) BETWEEN 1 AND 255),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX file_storage_locators_root ON file_storage_locators(storage_root_id,file_id);
        INSERT INTO file_storage_locators(file_id,storage_root_id,relative_path)
        SELECT file_id,area_id,filename FROM files;

        CREATE TABLE transfer_events (
            event_id INTEGER PRIMARY KEY,
            occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
            operation TEXT NOT NULL CHECK (operation IN (
                'transfer-started','reservation-created','transfer-state-changed',
                'transfer-completed','transfer-cancelled','transfer-failed',
                'quota-settled','quota-released','queue-item-skipped',
                'storage-unavailable','upload-credit-applied',
                'transfer-policy-changed','timezone-policy-changed',
                'storage-root-added','storage-root-updated','storage-root-reordered',
                'storage-root-disabled','storage-root-probed','storage-locator-updated'
            )),
            actor_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            transfer_id TEXT REFERENCES transfer_records(transfer_id) ON DELETE RESTRICT,
            reservation_id TEXT REFERENCES transfer_quota_reservations(reservation_id) ON DELETE RESTRICT,
            file_id INTEGER REFERENCES files(file_id) ON DELETE RESTRICT,
            storage_root_id INTEGER REFERENCES file_storage_roots(storage_root_id) ON DELETE RESTRICT,
            protocol TEXT CHECK (protocol IS NULL OR length(protocol) BETWEEN 1 AND 32),
            direction TEXT CHECK (direction IS NULL OR direction IN ('download','upload')),
            outcome TEXT CHECK (outcome IS NULL OR length(outcome) BETWEEN 1 AND 64),
            byte_count INTEGER CHECK (byte_count IS NULL OR byte_count >= 0),
            prior_version INTEGER CHECK (prior_version IS NULL OR prior_version >= 0),
            next_version INTEGER CHECK (next_version IS NULL OR next_version >= 0),
            detail TEXT CHECK (detail IS NULL OR length(CAST(detail AS BLOB)) BETWEEN 1 AND 96)
        );
        CREATE INDEX transfer_events_transfer ON transfer_events(transfer_id,event_id);
        CREATE INDEX transfer_events_caller ON transfer_events(actor_caller_id,event_id);
        CREATE TRIGGER transfer_events_no_update BEFORE UPDATE ON transfer_events
        BEGIN SELECT RAISE(ABORT, 'transfer events are append-only'); END;
        CREATE TRIGGER transfer_events_no_delete BEFORE DELETE ON transfer_events
        BEGIN SELECT RAISE(ABORT, 'transfer events are append-only'); END;
        "#,
    },
    Migration {
        version: 17,
        name: "zero_byte_file_authority",
        sql: r#"
        CREATE TABLE migration_17_validation AS
        SELECT COUNT(*) AS file_count,
               COALESCE(SUM(file_id),0) AS file_id_sum,
               COALESCE(SUM(size_bytes),0) AS file_size_sum,
               COALESCE(SUM(download_count),0) AS download_count_sum,
               COALESCE(SUM(state_version),0) AS file_version_sum,
               COALESCE(SUM(length(filename)+length(normalized_filename)+
                   length(CAST(description AS BLOB))+length(sha256)+
                   length(uploader_name)+length(lifecycle)+length(integrity_state)+
                   length(description_source)),0) AS semantic_bytes
          FROM files;

        DROP INDEX files_area_filename_live;
        DROP INDEX files_area_listing;
        DROP INDEX files_upload_time;
        ALTER TABLE files RENAME TO files_schema_16;

        CREATE TABLE files (
            file_id INTEGER PRIMARY KEY,
            area_id INTEGER NOT NULL REFERENCES file_areas(area_id) ON DELETE RESTRICT,
            filename TEXT NOT NULL CHECK (length(filename) BETWEEN 1 AND 64),
            normalized_filename TEXT NOT NULL CHECK (length(normalized_filename) BETWEEN 1 AND 64),
            description TEXT NOT NULL CHECK (length(CAST(description AS BLOB)) BETWEEN 1 AND 4096),
            size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
            sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
            uploaded_at INTEGER NOT NULL,
            uploader_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            uploader_name TEXT NOT NULL CHECK (length(uploader_name) BETWEEN 1 AND 60),
            download_count INTEGER NOT NULL DEFAULT 0 CHECK (download_count >= 0),
            lifecycle TEXT NOT NULL DEFAULT 'active'
                CHECK (lifecycle IN ('active','offline','pending-review','disabled','tombstoned')),
            integrity_state TEXT NOT NULL DEFAULT 'unknown'
                CHECK (integrity_state IN ('unknown','present','missing','digest-mismatch')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            description_source TEXT NOT NULL DEFAULT 'legacy-import'
                CHECK (description_source IN ('caller','operator','file-id-diz','legacy-import','system')),
            description_source_digest TEXT CHECK (description_source_digest IS NULL OR
                (length(description_source_digest)=64 AND description_source_digest NOT GLOB '*[^0-9a-f]*')),
            review_submitted_at INTEGER CHECK (review_submitted_at IS NULL OR review_submitted_at >= 0),
            reviewed_by_caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            reviewed_at INTEGER CHECK (reviewed_at IS NULL OR reviewed_at >= 0),
            tombstoned_at INTEGER CHECK (tombstoned_at IS NULL OR tombstoned_at >= 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        INSERT INTO files (
            file_id,area_id,filename,normalized_filename,description,size_bytes,
            sha256,uploaded_at,uploader_caller_id,uploader_name,download_count,
            lifecycle,integrity_state,state_version,description_source,
            description_source_digest,review_submitted_at,reviewed_by_caller_id,
            reviewed_at,tombstoned_at,created_at,updated_at
        )
        SELECT file_id,area_id,filename,normalized_filename,description,size_bytes,
               sha256,uploaded_at,uploader_caller_id,uploader_name,download_count,
               lifecycle,integrity_state,state_version,description_source,
               description_source_digest,review_submitted_at,reviewed_by_caller_id,
               reviewed_at,tombstoned_at,created_at,updated_at
          FROM files_schema_16;
        DROP TABLE files_schema_16;

        CREATE UNIQUE INDEX files_area_filename_live
            ON files(area_id, normalized_filename)
            WHERE lifecycle <> 'tombstoned';
        CREATE INDEX files_area_listing
            ON files(area_id, normalized_filename, lifecycle, integrity_state);
        CREATE INDEX files_upload_time
            ON files(uploaded_at, area_id, lifecycle);
        "#,
    },
    Migration {
        version: 18,
        name: "operational_observability",
        sql: r#"
        CREATE TABLE migration_18_validation AS
        SELECT
            (SELECT COUNT(*) FROM callers) AS caller_count,
            (SELECT COALESCE(SUM(caller_id),0) FROM callers) AS caller_id_sum,
            (SELECT COUNT(*) FROM messages) AS message_count,
            (SELECT COALESCE(SUM(message_id),0) FROM messages) AS message_id_sum,
            (SELECT COUNT(*) FROM files) AS file_count,
            (SELECT COALESCE(SUM(file_id),0) FROM files) AS file_id_sum,
            (SELECT COUNT(*) FROM transfer_records) AS transfer_count,
            (SELECT COUNT(*) FROM file_storage_roots) AS storage_root_count,
            (SELECT COUNT(*) FROM message_mutation_events) +
            (SELECT COUNT(*) FROM caller_access_events) +
            (SELECT COUNT(*) FROM caller_identity_events) +
            (SELECT COUNT(*) FROM public_information_events) +
            (SELECT COUNT(*) FROM file_events) +
            (SELECT COUNT(*) FROM transfer_events) AS audit_count;

        CREATE TABLE operational_events (
            event_id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at_utc INTEGER NOT NULL CHECK (occurred_at_utc >= 0),
            board_day INTEGER NOT NULL CHECK (board_day BETWEEN 19700101 AND 99991231),
            timezone_policy_version INTEGER NOT NULL CHECK (timezone_policy_version > 0),
            category TEXT NOT NULL CHECK (category IN (
                'system','node','session','caller','authentication','message',
                'file','transfer','storage','backup','operator','error'
            )),
            severity TEXT NOT NULL CHECK (severity IN ('info','notice','warning','error','critical')),
            event_code TEXT NOT NULL CHECK (
                length(CAST(event_code AS BLOB)) BETWEEN 3 AND 64 AND
                event_code NOT GLOB '*[^a-z0-9.-]*'
            ),
            outcome TEXT NOT NULL CHECK (outcome IN ('succeeded','failed','cancelled','denied','unavailable','observed')),
            node_id INTEGER CHECK (node_id IS NULL OR node_id > 0),
            session_id INTEGER CHECK (session_id IS NULL OR session_id > 0),
            caller_id INTEGER REFERENCES callers(caller_id) ON DELETE RESTRICT,
            correlation_id TEXT CHECK (correlation_id IS NULL OR length(CAST(correlation_id AS BLOB)) BETWEEN 1 AND 64),
            idempotency_key TEXT UNIQUE CHECK (idempotency_key IS NULL OR length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 96),
            object_kind TEXT CHECK (object_kind IS NULL OR length(CAST(object_kind AS BLOB)) BETWEEN 1 AND 32),
            object_id TEXT CHECK (object_id IS NULL OR length(CAST(object_id AS BLOB)) BETWEEN 1 AND 64),
            retention_class TEXT NOT NULL CHECK (retention_class IN ('operational','summary-source')),
            attribute_kind TEXT NOT NULL CHECK (attribute_kind IN (
                'none','session','transfer','message','file','storage','backup','error','operator'
            )),
            attribute_version INTEGER NOT NULL DEFAULT 1 CHECK (attribute_version = 1),
            text_value_1 TEXT CHECK (text_value_1 IS NULL OR length(CAST(text_value_1 AS BLOB)) <= 256),
            text_value_2 TEXT CHECK (text_value_2 IS NULL OR length(CAST(text_value_2 AS BLOB)) <= 256),
            text_value_3 TEXT CHECK (text_value_3 IS NULL OR length(CAST(text_value_3 AS BLOB)) <= 256),
            number_value_1 INTEGER CHECK (number_value_1 IS NULL OR number_value_1 >= 0),
            number_value_2 INTEGER CHECK (number_value_2 IS NULL OR number_value_2 >= 0),
            CHECK (
                length(CAST(COALESCE(text_value_1,'') AS BLOB)) +
                length(CAST(COALESCE(text_value_2,'') AS BLOB)) +
                length(CAST(COALESCE(text_value_3,'') AS BLOB)) <= 768
            )
        );
        CREATE INDEX operational_events_time ON operational_events(occurred_at_utc,event_id);
        CREATE INDEX operational_events_category ON operational_events(category,severity,occurred_at_utc,event_id);
        CREATE INDEX operational_events_node ON operational_events(node_id,occurred_at_utc,event_id);
        CREATE INDEX operational_events_session ON operational_events(session_id,occurred_at_utc,event_id);
        CREATE INDEX operational_events_caller ON operational_events(caller_id,occurred_at_utc,event_id);
        CREATE INDEX operational_events_correlation ON operational_events(correlation_id,event_id);
        CREATE TRIGGER operational_events_no_update BEFORE UPDATE ON operational_events
        BEGIN SELECT RAISE(ABORT, 'operational events are append-only'); END;

        CREATE TABLE operational_daily_summaries (
            board_day INTEGER NOT NULL CHECK (board_day BETWEEN 19700101 AND 99991231),
            timezone_policy_version INTEGER NOT NULL CHECK (timezone_policy_version > 0),
            high_water_event_id INTEGER NOT NULL DEFAULT 0 CHECK (high_water_event_id >= 0),
            calls_started INTEGER NOT NULL DEFAULT 0 CHECK (calls_started >= 0),
            calls_completed INTEGER NOT NULL DEFAULT 0 CHECK (calls_completed >= 0),
            new_callers INTEGER NOT NULL DEFAULT 0 CHECK (new_callers >= 0),
            messages_posted INTEGER NOT NULL DEFAULT 0 CHECK (messages_posted >= 0),
            successful_uploads INTEGER NOT NULL DEFAULT 0 CHECK (successful_uploads >= 0),
            upload_bytes INTEGER NOT NULL DEFAULT 0 CHECK (upload_bytes >= 0),
            successful_downloads INTEGER NOT NULL DEFAULT 0 CHECK (successful_downloads >= 0),
            download_bytes INTEGER NOT NULL DEFAULT 0 CHECK (download_bytes >= 0),
            failed_transfers INTEGER NOT NULL DEFAULT 0 CHECK (failed_transfers >= 0),
            cancelled_transfers INTEGER NOT NULL DEFAULT 0 CHECK (cancelled_transfers >= 0),
            warning_events INTEGER NOT NULL DEFAULT 0 CHECK (warning_events >= 0),
            error_events INTEGER NOT NULL DEFAULT 0 CHECK (error_events >= 0),
            critical_events INTEGER NOT NULL DEFAULT 0 CHECK (critical_events >= 0),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
            PRIMARY KEY(board_day,timezone_policy_version)
        );

        CREATE TABLE operational_retention_policy (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            detail_days INTEGER NOT NULL CHECK (detail_days BETWEEN 1 AND 365),
            summary_days INTEGER NOT NULL CHECK (summary_days BETWEEN 31 AND 3650),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            activated_at INTEGER NOT NULL CHECK (activated_at >= 0),
            last_cleanup_at INTEGER CHECK (last_cleanup_at IS NULL OR last_cleanup_at >= 0)
        );
        INSERT INTO operational_retention_policy(
            singleton,detail_days,summary_days,activated_at
        ) VALUES(1,30,400,CAST(strftime('%s','now') AS INTEGER));

        CREATE TABLE operator_notifications (
            notification_id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_event_id INTEGER NOT NULL UNIQUE REFERENCES operational_events(event_id) ON DELETE RESTRICT,
            created_at INTEGER NOT NULL CHECK (created_at >= 0),
            category TEXT NOT NULL CHECK (category IN ('system','node','session','caller','authentication','message','file','transfer','storage','backup','operator','error')),
            severity TEXT NOT NULL CHECK (severity IN ('warning','error','critical')),
            reason_key TEXT NOT NULL CHECK (length(CAST(reason_key AS BLOB)) BETWEEN 3 AND 96),
            remediation_key TEXT CHECK (remediation_key IS NULL OR length(CAST(remediation_key AS BLOB)) BETWEEN 3 AND 96),
            state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open','acknowledged','resolved')),
            state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
            acknowledged_at INTEGER CHECK (acknowledged_at IS NULL OR acknowledged_at >= 0),
            acknowledged_by TEXT CHECK (acknowledged_by IS NULL OR length(CAST(acknowledged_by AS BLOB)) BETWEEN 1 AND 64),
            resolved_at INTEGER CHECK (resolved_at IS NULL OR resolved_at >= 0)
        );
        CREATE INDEX operator_notifications_state ON operator_notifications(state,severity,created_at,notification_id);

        CREATE TABLE operator_observability_audit (
            audit_id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
            action TEXT NOT NULL CHECK (action IN ('notification-acknowledged','notification-resolved','retention-policy-changed','retention-cleanup')),
            actor_kind TEXT NOT NULL CHECK (actor_kind IN ('host-operator','named-sysop','system')),
            actor_id TEXT CHECK (actor_id IS NULL OR length(CAST(actor_id AS BLOB)) BETWEEN 1 AND 64),
            target_kind TEXT NOT NULL CHECK (length(CAST(target_kind AS BLOB)) BETWEEN 1 AND 32),
            target_id TEXT NOT NULL CHECK (length(CAST(target_id AS BLOB)) BETWEEN 1 AND 64),
            outcome TEXT NOT NULL CHECK (outcome IN ('succeeded','conflict','failed')),
            prior_version INTEGER CHECK (prior_version IS NULL OR prior_version > 0),
            next_version INTEGER CHECK (next_version IS NULL OR next_version > 0),
            correlation_id TEXT CHECK (correlation_id IS NULL OR length(CAST(correlation_id AS BLOB)) BETWEEN 1 AND 64)
        );
        CREATE TRIGGER operator_observability_audit_no_update BEFORE UPDATE ON operator_observability_audit
        BEGIN SELECT RAISE(ABORT, 'operator observability audit is append-only'); END;
        CREATE TRIGGER operator_observability_audit_no_delete BEFORE DELETE ON operator_observability_audit
        BEGIN SELECT RAISE(ABORT, 'operator observability audit is append-only'); END;
        "#,
    },
    Migration {
        version: 19,
        name: "operator_control_foundation",
        sql: r#"
        CREATE TABLE migration_19_validation AS
        SELECT (SELECT COUNT(*) FROM callers) AS caller_count,
               (SELECT COUNT(*) FROM messages) AS message_count,
               (SELECT COUNT(*) FROM files) AS file_count,
               (SELECT COUNT(*) FROM transfer_records) AS transfer_count,
               (SELECT COUNT(*) FROM operational_events) AS event_count,
               (SELECT COUNT(*) FROM operational_daily_summaries) AS summary_count,
               (SELECT COUNT(*) FROM operator_notifications) AS notification_count,
               (SELECT COUNT(*) FROM operator_observability_audit) AS audit_count;

        CREATE TABLE operator_command_journal (
            command_id TEXT PRIMARY KEY CHECK (length(CAST(command_id AS BLOB)) BETWEEN 16 AND 64),
            daemon_generation TEXT NOT NULL CHECK (length(daemon_generation)=32 AND daemon_generation NOT GLOB '*[^0-9a-f]*'),
            operator_kind TEXT NOT NULL CHECK (operator_kind IN ('host-operator','named-sysop','system')),
            operator_id TEXT NOT NULL CHECK (length(CAST(operator_id AS BLOB)) BETWEEN 1 AND 64),
            command_family TEXT NOT NULL CHECK (length(CAST(command_family AS BLOB)) BETWEEN 3 AND 48 AND command_family NOT GLOB '*[^a-z0-9.-]*'),
            command_type TEXT NOT NULL CHECK (length(CAST(command_type AS BLOB)) BETWEEN 3 AND 64 AND command_type NOT GLOB '*[^a-z0-9.-]*'),
            request_fingerprint TEXT NOT NULL CHECK (length(request_fingerprint)=64 AND request_fingerprint NOT GLOB '*[^0-9a-f]*'),
            target_kind TEXT CHECK (target_kind IS NULL OR length(CAST(target_kind AS BLOB)) BETWEEN 1 AND 32),
            target_id TEXT CHECK (target_id IS NULL OR length(CAST(target_id AS BLOB)) BETWEEN 1 AND 64),
            target_generation TEXT CHECK (target_generation IS NULL OR length(CAST(target_generation AS BLOB)) BETWEEN 1 AND 64),
            state TEXT NOT NULL CHECK (state IN ('accepted','rejected','completed')),
            result_class TEXT CHECK (result_class IS NULL OR length(CAST(result_class AS BLOB)) BETWEEN 3 AND 64),
            result_version INTEGER CHECK (result_version IS NULL OR result_version > 0),
            received_at INTEGER NOT NULL CHECK (received_at >= 0),
            completed_at INTEGER CHECK (completed_at IS NULL OR completed_at >= received_at),
            expires_at INTEGER NOT NULL CHECK (expires_at > received_at)
        );
        CREATE INDEX operator_command_journal_expiry ON operator_command_journal(expires_at,command_id);

        CREATE TABLE operator_control_audit (
            audit_id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at INTEGER NOT NULL CHECK (occurred_at >= 0),
            operator_kind TEXT NOT NULL CHECK (operator_kind IN ('host-operator','named-sysop','system','unknown-peer')),
            operator_id TEXT CHECK (operator_id IS NULL OR length(CAST(operator_id AS BLOB)) BETWEEN 1 AND 64),
            operation TEXT NOT NULL CHECK (length(CAST(operation AS BLOB)) BETWEEN 3 AND 64 AND operation NOT GLOB '*[^a-z0-9.-]*'),
            authorization_result TEXT NOT NULL CHECK (authorization_result IN ('allowed','denied','not-applicable')),
            target_kind TEXT CHECK (target_kind IS NULL OR length(CAST(target_kind AS BLOB)) BETWEEN 1 AND 32),
            target_id TEXT CHECK (target_id IS NULL OR length(CAST(target_id AS BLOB)) BETWEEN 1 AND 64),
            command_id TEXT CHECK (command_id IS NULL OR length(CAST(command_id AS BLOB)) BETWEEN 16 AND 64),
            correlation_id TEXT CHECK (correlation_id IS NULL OR length(CAST(correlation_id AS BLOB)) BETWEEN 1 AND 64),
            outcome TEXT NOT NULL CHECK (outcome IN ('succeeded','denied','failed','rejected','incompatible')),
            detail_code TEXT CHECK (detail_code IS NULL OR length(CAST(detail_code AS BLOB)) BETWEEN 3 AND 64)
        );
        CREATE INDEX operator_control_audit_time ON operator_control_audit(occurred_at,audit_id);
        CREATE INDEX operator_control_audit_command ON operator_control_audit(command_id,audit_id);
        CREATE TRIGGER operator_control_audit_no_update BEFORE UPDATE ON operator_control_audit
        BEGIN SELECT RAISE(ABORT, 'operator control audit is append-only'); END;
        CREATE TRIGGER operator_control_audit_no_delete BEFORE DELETE ON operator_control_audit
        BEGIN SELECT RAISE(ABORT, 'operator control audit is append-only'); END;
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
        self.validate_snapshot_at_version(SCHEMA_VERSION)
    }

    /// Validates an exact supported snapshot without applying migrations.
    /// Native restore uses this for schema-10 rollback backups as well as the
    /// current schema; normal writable startup remains the only migration
    /// boundary.
    pub fn validate_snapshot_at_version(
        &self,
        required: u32,
    ) -> Result<BoardIdentity, DatabaseError> {
        let found = schema_version_from(&self.connection)?;
        if found != required {
            return Err(DatabaseError::SnapshotSchema { found, required });
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
        if required >= 11 {
            validate_schema_11_snapshot(&self.connection)?;
        }
        if required >= 12 {
            validate_schema_12_snapshot(&self.connection)?;
        }
        if required >= 13 {
            validate_schema_13_snapshot(&self.connection)?;
        }
        if required >= 14 {
            validate_schema_14_snapshot(&self.connection)?;
        }
        if required >= 15 {
            validate_schema_15_snapshot(&self.connection)?;
        }
        if required >= 16 {
            validate_schema_16_snapshot(&self.connection)?;
        }
        if required >= 17 {
            validate_schema_17_snapshot(&self.connection)?;
        }
        if required >= 18 {
            validate_schema_18_snapshot(&self.connection)?;
        }
        if required >= 19 {
            validate_schema_19_snapshot(&self.connection)?;
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
        // Windows requires a write-capable handle for FlushFileBuffers;
        // opening the completed SQLite snapshot read-only makes sync_all fail
        // with ACCESS_DENIED even though the backup itself succeeded.
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(destination)
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
        let login_identifier = self.available_login_identifier(&normalized_name)?;
        let real_name = Some(display_name.clone());
        let transaction = self
            .connection
            .transaction()
            .map_err(DatabaseError::Sqlite)?;
        transaction
            .execute(
                r#"
                INSERT INTO callers (
                    login_identifier, display_name, normalized_name, real_name,
                    security_level, account_state,
                    is_new_caller, first_call_at, address_line_1, address_line_2,
                    city, region, postal_code, country, phone, email, birthday
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                "#,
                params![
                    login_identifier,
                    display_name,
                    normalized_name,
                    real_name,
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
        let mut event = NewOperationalEvent::new(
            now,
            EventCategory::Caller,
            EventSeverity::Notice,
            "caller.created",
            EventOutcome::Succeeded,
        );
        event.caller_id = Some(caller_id);
        event.object_kind = Some("caller".to_owned());
        event.object_id = Some(caller_id.get().to_string());
        event.idempotency_key = Some(format!("caller-created-{}", caller_id.get()));
        event.retention_class = RetentionClass::SummarySource;
        insert_operational_event_tx(&transaction, &event)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
        self.caller_by_id(caller_id)?
            .ok_or(DatabaseError::MissingCaller(caller_id.get()))
    }

    pub fn caller_by_name(&self, caller_name: &[u8]) -> Result<Option<Caller>, DatabaseError> {
        let (_, normalized) = canonicalize_caller_name(caller_name)?;
        self.caller_by_normalized_name(&normalized)
    }

    pub fn caller_by_login_identifier(
        &self,
        login_identifier: &[u8],
    ) -> Result<Option<Caller>, DatabaseError> {
        let normalized = canonicalize_login_identifier(login_identifier)?;
        self.query_caller(
            &format!("{CALLER_SELECT} WHERE c.login_identifier = ?1"),
            params![normalized],
        )
    }

    fn available_login_identifier(&self, normalized_name: &str) -> Result<String, DatabaseError> {
        let base = derive_login_identifier_base(normalized_name);
        if self.caller_by_login_identifier(base.as_bytes())?.is_none() {
            return Ok(base);
        }
        let next_id: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(caller_id), 0) + 1 FROM callers",
                [],
                |row| row.get(0),
            )
            .map_err(DatabaseError::Sqlite)?;
        let suffix = format!("-{next_id}");
        let keep = MAX_LOGIN_IDENTIFIER_BYTES.saturating_sub(suffix.len());
        let mut resolved = base;
        resolved.truncate(keep);
        while resolved.ends_with(['-', '_', '.']) {
            resolved.pop();
        }
        resolved.push_str(&suffix);
        Ok(resolved)
    }

    pub fn caller_by_id(&self, caller_id: CallerId) -> Result<Option<Caller>, DatabaseError> {
        self.query_caller(
            &format!("{CALLER_SELECT} WHERE c.caller_id = ?1"),
            rusqlite::params![caller_id.get()],
        )
    }

    fn caller_by_normalized_name(
        &self,
        normalized_name: &str,
    ) -> Result<Option<Caller>, DatabaseError> {
        self.query_caller(
            &format!("{CALLER_SELECT} WHERE c.normalized_name = ?1"),
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
            String,
            Option<String>,
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
            u16,
            i64,
            Option<String>,
            bool,
            Option<String>,
            bool,
            i64,
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
                    row.get(31)?,
                    row.get(32)?,
                    row.get(33)?,
                    row.get(34)?,
                    row.get(35)?,
                    row.get(36)?,
                    row.get(37)?,
                    row.get(38)?,
                    row.get(39)?,
                ))
            })
            .optional()
            .map_err(DatabaseError::Sqlite)?;
        stored
            .map(
                |(
                    caller_id,
                    login_identifier,
                    display_name,
                    normalized_name,
                    real_name,
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
                    base_security,
                    state_version,
                    subscription_expires_on,
                    purge_protected,
                    lifecycle_prior_state,
                    public_directory_listed,
                    publicity_state_version,
                )| {
                    Ok(Caller {
                        id: CallerId::new(caller_id).map_err(DatabaseError::InvalidStoredCaller)?,
                        login_identifier,
                        display_name,
                        normalized_name,
                        real_name,
                        security_level: SecurityLevel::new(security)
                            .map_err(DatabaseError::InvalidStoredCaller)?,
                        base_security_level: SecurityLevel::new(base_security)
                            .map_err(DatabaseError::InvalidStoredCaller)?,
                        state: CallerState::from_database_value(&state)
                            .map_err(DatabaseError::InvalidStoredCaller)?,
                        state_version: nonnegative_u64(state_version)?,
                        subscription_expires_on: subscription_expires_on
                            .as_deref()
                            .map(parse_subscription_date)
                            .transpose()?,
                        purge_protected,
                        lifecycle_prior_state: lifecycle_prior_state
                            .as_deref()
                            .map(CallerState::from_database_value)
                            .transpose()
                            .map_err(DatabaseError::InvalidStoredCaller)?,
                        public_directory_listed,
                        publicity_state_version: nonnegative_u64(publicity_state_version)?,
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

    pub fn authenticate_login_identifier(
        &self,
        login_identifier: &[u8],
        password: &[u8],
        hasher: &CredentialHasher,
    ) -> Result<AuthenticationResult, DatabaseError> {
        let Some(caller) = self.caller_by_login_identifier(login_identifier)? else {
            return Ok(AuthenticationResult::Invalid);
        };
        self.authenticate_caller_password(caller, password, hasher)
    }

    fn authenticate_caller_password(
        &self,
        caller: Caller,
        password: &[u8],
        hasher: &CredentialHasher,
    ) -> Result<AuthenticationResult, DatabaseError> {
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
        self.begin_caller_session_observed(caller, config, now, timezone, None)
    }

    pub fn begin_caller_session_observed(
        &mut self,
        caller: &Caller,
        config: &CallerConfig,
        now: i64,
        timezone: chrono_tz::Tz,
        observation: Option<(u32, u64, &str)>,
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
        let mut event = NewOperationalEvent::new(
            now,
            EventCategory::Session,
            EventSeverity::Info,
            "session.started",
            EventOutcome::Succeeded,
        );
        event.caller_id = Some(caller.id);
        event.retention_class = RetentionClass::SummarySource;
        event.attributes = EventAttributes::Session {
            public_handle: Some(caller.display_name.clone()),
            transport: observation.map(|item| item.2.to_owned()),
            duration_seconds: None,
            close_reason: None,
        };
        if let Some((node_id, session_id, _)) = observation {
            event.node_id = Some(node_id);
            event.session_id = Some(session_id);
            event.correlation_id = Some(format!("session-{session_id}"));
        }
        insert_operational_event_tx(&transaction, &event)?;
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
            base_allowance: allowance,
        })
    }

    pub fn finish_caller_session(
        &mut self,
        caller_id: CallerId,
        elapsed_seconds: u64,
        daily_elapsed_seconds: u64,
        day: i64,
    ) -> Result<(), DatabaseError> {
        self.finish_caller_session_observed(
            caller_id,
            elapsed_seconds,
            daily_elapsed_seconds,
            day,
            None,
            0,
        )
    }

    pub fn finish_caller_session_observed(
        &mut self,
        caller_id: CallerId,
        elapsed_seconds: u64,
        daily_elapsed_seconds: u64,
        day: i64,
        observation: Option<(u32, u64, &str, &str)>,
        occurred_at: i64,
    ) -> Result<(), DatabaseError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(DatabaseError::Sqlite)?;
        let changed = transaction
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
        if let Some((node_id, session_id, transport, close_reason)) = observation {
            let public_handle: String = transaction
                .query_row(
                    "SELECT display_name FROM callers WHERE caller_id=?1",
                    params![caller_id.get()],
                    |row| row.get(0),
                )
                .map_err(DatabaseError::Sqlite)?;
            let mut event = NewOperationalEvent::new(
                occurred_at,
                EventCategory::Session,
                EventSeverity::Info,
                "session.completed",
                EventOutcome::Succeeded,
            );
            event.node_id = Some(node_id);
            event.session_id = Some(session_id);
            event.caller_id = Some(caller_id);
            event.correlation_id = Some(format!("session-{session_id}"));
            event.retention_class = RetentionClass::SummarySource;
            event.attributes = EventAttributes::Session {
                public_handle: Some(public_handle),
                transport: Some(transport.to_owned()),
                duration_seconds: Some(elapsed_seconds),
                close_reason: Some(close_reason.to_owned()),
            };
            insert_operational_event_tx(&transaction, &event)?;
        }
        transaction.commit().map_err(DatabaseError::Sqlite)
    }

    #[cfg(test)]
    pub(crate) fn set_caller_state(
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

    #[allow(clippy::too_many_arguments)]
    pub fn update_caller_identity(
        &mut self,
        caller_id: CallerId,
        expected_state_version: u64,
        login_identifier: &[u8],
        display_handle: &[u8],
        real_name: Option<String>,
        caller_config: &CallerConfig,
        now: i64,
    ) -> Result<Caller, DatabaseError> {
        let login_identifier = canonicalize_login_identifier(login_identifier)?;
        let (display_name, normalized_name) = canonicalize_caller_name(display_handle)?;
        let real_name = real_name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if real_name
            .as_ref()
            .is_some_and(|value| value.len() > 120 || value.chars().any(char::is_control))
        {
            return Err(DatabaseError::InvalidStoredCaller(
                CallerError::InvalidRealName,
            ));
        }
        let (_, normalized_sysop) =
            canonicalize_caller_name(caller_config.sysop_caller_name.as_bytes())?;
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(DatabaseError::Sqlite)?;
        let current: Option<(String, i64)> = transaction
            .query_row(
                "SELECT normalized_name, state_version FROM callers WHERE caller_id=?1",
                params![caller_id.get()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(DatabaseError::Sqlite)?;
        let Some((current_name, current_version)) = current else {
            return Err(DatabaseError::MissingCaller(caller_id.get()));
        };
        let current_version = nonnegative_u64(current_version)?;
        if current_version != expected_state_version {
            return Err(DatabaseError::CallerStateConflict {
                expected: expected_state_version,
                actual: current_version,
            });
        }
        if current_name == normalized_sysop && normalized_name != normalized_sysop {
            return Err(DatabaseError::ProtectedNamedSysop);
        }
        let conflicts: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM callers WHERE caller_id<>?1 AND (login_identifier=?2 OR normalized_name=?3)",
                params![caller_id.get(), login_identifier, normalized_name],
                |row| row.get(0),
            )
            .map_err(DatabaseError::Sqlite)?;
        if conflicts != 0 {
            return Err(DatabaseError::DuplicateCallerIdentity);
        }
        let new_version = current_version
            .checked_add(1)
            .ok_or(DatabaseError::CounterOverflow(current_version))?;
        transaction
            .execute(
                "UPDATE callers SET login_identifier=?2, display_name=?3, normalized_name=?4, real_name=?5, state_version=?6, updated_at=CURRENT_TIMESTAMP WHERE caller_id=?1 AND state_version=?7",
                params![caller_id.get(), login_identifier, display_name, normalized_name, real_name, sqlite_i64(new_version)?, sqlite_i64(expected_state_version)?],
            )
            .map_err(DatabaseError::Sqlite)?;
        transaction
            .execute(
                "INSERT INTO caller_identity_events (occurred_at, caller_id, prior_state_version, new_state_version, actor_kind) VALUES (?1, ?2, ?3, ?4, 'local-operator')",
                params![now, caller_id.get(), sqlite_i64(current_version)?, sqlite_i64(new_version)?],
            )
            .map_err(DatabaseError::Sqlite)?;
        transaction.commit().map_err(DatabaseError::Sqlite)?;
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

fn validate_schema_11_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let recipient_mismatch: i64 = connection
        .query_row(
            r#"
            SELECT COUNT(*)
              FROM messages AS m
              LEFT JOIN message_delivery_recipients AS r ON r.message_id = m.message_id
             WHERE (m.audience_kind = 'local-recipient' AND r.message_id IS NULL)
                OR (m.audience_kind = 'all-callers' AND r.message_id IS NOT NULL)
                OR (m.delivery_role = 'cc' AND NOT EXISTS (
                    SELECT 1 FROM messages AS primary_message
                     WHERE primary_message.message_id = m.primary_delivery_id
                       AND primary_message.fanout_id = m.fanout_id
                       AND primary_message.delivery_role = 'primary'
                ))
            "#,
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if recipient_mismatch != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "message delivery recipient or primary linkage is invalid".to_owned(),
        ));
    }
    let invalid_fanouts: i64 = connection
        .query_row(
            r#"
            SELECT COUNT(*) FROM (
                SELECT m.fanout_id,
                       COUNT(*) AS delivery_count,
                       COUNT(r.message_id) AS recipient_count,
                       COUNT(DISTINCT r.caller_id) AS distinct_recipient_count,
                       MIN(m.delivery_ordinal) AS minimum_ordinal,
                       MAX(m.delivery_ordinal) AS maximum_ordinal
                  FROM messages AS m
                  LEFT JOIN message_delivery_recipients AS r ON r.message_id = m.message_id
                 GROUP BY m.fanout_id
                HAVING delivery_count NOT BETWEEN 1 AND 10
                    OR recipient_count <> distinct_recipient_count
                    OR minimum_ordinal <> 0
                    OR maximum_ordinal <> delivery_count - 1
            )
            "#,
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if invalid_fanouts != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "message fan-out cardinality or ordinal integrity is invalid".to_owned(),
        ));
    }
    let lineage_mismatch: i64 = connection
        .query_row(
            r#"
            SELECT COUNT(*)
              FROM message_lineage AS l
              JOIN message_mutation_events AS e ON e.event_id = l.mutation_event_id
             WHERE e.derived_message_id <> l.derived_message_id
                OR (l.relation = 'copy' AND e.operation <> 'copied')
                OR (l.relation = 'forward' AND e.operation <> 'forwarded')
            "#,
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if lineage_mismatch != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "message lineage and mutation audit disagree".to_owned(),
        ));
    }
    Ok(())
}

fn validate_schema_12_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let mut dates = connection
        .prepare(
            "SELECT subscription_expires_on FROM callers WHERE subscription_expires_on IS NOT NULL",
        )
        .map_err(DatabaseError::Sqlite)?;
    let dates = dates
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(DatabaseError::Sqlite)?;
    for date in dates {
        parse_subscription_date(&date.map_err(DatabaseError::Sqlite)?)?;
    }
    let invalid_lifecycle_state: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM callers WHERE account_state<>'deleted' AND lifecycle_prior_state IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if invalid_lifecycle_state != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "caller tombstone recovery state is invalid".to_owned(),
        ));
    }
    let invalid_adjustment_state: i64 = connection
        .query_row(
            r#"
            SELECT COUNT(*)
              FROM caller_security_adjustments AS a
              JOIN caller_access_events AS applied ON applied.event_id = a.applied_event_id
              LEFT JOIN caller_access_events AS resolved ON resolved.event_id = a.resolved_event_id
             WHERE applied.operation <> 'subscription-expired'
                OR applied.outcome <> 'committed'
                OR applied.subject_caller_id <> a.caller_id
                OR applied.adjustment_kind <> a.kind
                OR (a.status = 'active' AND (a.resolved_at IS NOT NULL OR a.resolved_event_id IS NOT NULL))
                OR (a.status = 'resolved' AND (
                    a.resolved_at IS NULL
                    OR a.resolved_event_id IS NULL
                    OR resolved.operation <> 'subscription-adjustment-resolved'
                    OR resolved.outcome <> 'committed'
                    OR resolved.subject_caller_id <> a.caller_id
                    OR resolved.adjustment_kind <> a.kind
                ))
            "#,
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if invalid_adjustment_state != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "caller security adjustment and audit state disagree".to_owned(),
        ));
    }
    Ok(())
}

fn parse_subscription_date(value: &str) -> Result<chrono::NaiveDate, DatabaseError> {
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || value
            .bytes()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err(DatabaseError::InvalidSubscriptionDate(value.to_owned()));
    }
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| DatabaseError::InvalidSubscriptionDate(value.to_owned()))
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
    let rebuilds_referenced_file_table = migration.version == 17;
    if rebuilds_referenced_file_table {
        connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA legacy_alter_table=ON;")
            .map_err(DatabaseError::Sqlite)?;
    }
    let result = (|| {
        let transaction = connection.transaction().map_err(DatabaseError::Sqlite)?;
        run_migration(&transaction, migration)?;
        transaction.commit().map_err(DatabaseError::Sqlite)
    })();
    if rebuilds_referenced_file_table {
        let restore = connection
            .execute_batch("PRAGMA legacy_alter_table=OFF; PRAGMA foreign_keys=ON;")
            .map_err(DatabaseError::Sqlite);
        if result.is_ok() {
            restore?;
        }
    }
    result
}

fn run_migration(
    transaction: &Transaction<'_>,
    migration: &Migration,
) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(migration.sql)
        .map_err(DatabaseError::Sqlite)?;
    if migration.version == 11 {
        validate_message_mutation_migration(transaction)?;
        transaction
            .execute("DROP TABLE migration_11_validation", [])
            .map_err(DatabaseError::Sqlite)?;
    }
    if migration.version == 12 {
        validate_caller_access_migration(transaction)?;
        transaction
            .execute("DROP TABLE migration_12_validation", [])
            .map_err(DatabaseError::Sqlite)?;
    }
    if migration.version == 13 {
        migrate_caller_identity(transaction)?;
    }
    if migration.version == 14 {
        validate_public_information_migration(transaction)?;
        transaction
            .execute("DROP TABLE migration_14_validation", [])
            .map_err(DatabaseError::Sqlite)?;
    }
    if migration.version == 15 {
        validate_file_maintenance_migration(transaction)?;
        transaction
            .execute("DROP TABLE migration_15_validation", [])
            .map_err(DatabaseError::Sqlite)?;
    }
    if migration.version == 16 {
        validate_transfer_storage_migration(transaction)?;
        transaction
            .execute("DROP TABLE migration_16_validation", [])
            .map_err(DatabaseError::Sqlite)?;
    }
    if migration.version == 17 {
        validate_zero_byte_file_migration(transaction)?;
        transaction
            .execute("DROP TABLE migration_17_validation", [])
            .map_err(DatabaseError::Sqlite)?;
    }
    if migration.version == 18 {
        validate_observability_migration(transaction)?;
        transaction
            .execute("DROP TABLE migration_18_validation", [])
            .map_err(DatabaseError::Sqlite)?;
    }
    if migration.version == 19 {
        validate_operator_control_migration(transaction)?;
        transaction
            .execute("DROP TABLE migration_19_validation", [])
            .map_err(DatabaseError::Sqlite)?;
    }
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            params![migration.version, migration.name],
        )
        .map_err(DatabaseError::Sqlite)?;
    Ok(())
}

fn migrate_caller_identity(transaction: &Transaction<'_>) -> Result<(), DatabaseError> {
    let callers = {
        let mut statement = transaction
            .prepare(
                "SELECT caller_id, display_name, normalized_name FROM callers ORDER BY caller_id",
            )
            .map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(DatabaseError::Sqlite)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?
    };
    let mut used = HashSet::new();
    for (caller_id, display_name, normalized_name) in callers {
        let base = derive_login_identifier_base(&normalized_name);
        let login_identifier = if used.insert(base.clone()) {
            base
        } else {
            let suffix = format!("-{caller_id}");
            let keep = MAX_LOGIN_IDENTIFIER_BYTES.saturating_sub(suffix.len());
            let mut resolved = base;
            resolved.truncate(keep);
            while resolved.ends_with(['-', '_', '.']) {
                resolved.pop();
            }
            resolved.push_str(&suffix);
            if !used.insert(resolved.clone()) {
                return Err(DatabaseError::MigrationValidation(
                    "schema-13 login identifier collision could not be resolved".to_owned(),
                ));
            }
            resolved
        };
        canonicalize_login_identifier(login_identifier.as_bytes())
            .map_err(DatabaseError::InvalidStoredCaller)?;
        transaction
            .execute(
                "UPDATE callers SET login_identifier=?2, real_name=?3 WHERE caller_id=?1",
                params![caller_id, login_identifier, display_name],
            )
            .map_err(DatabaseError::Sqlite)?;
    }
    transaction
        .execute_batch(
            r#"
            CREATE UNIQUE INDEX callers_login_identifier_unique
                ON callers (login_identifier);
            CREATE TRIGGER callers_login_identifier_insert
            BEFORE INSERT ON callers
            WHEN NEW.login_identifier IS NULL
              OR length(NEW.login_identifier) NOT BETWEEN 1 AND 32
              OR NEW.login_identifier <> lower(NEW.login_identifier)
              OR substr(NEW.login_identifier, 1, 1) NOT GLOB '[a-z0-9]'
              OR NEW.login_identifier GLOB '*[^a-z0-9._-]*'
            BEGIN SELECT RAISE(ABORT, 'invalid login identifier'); END;
            CREATE TRIGGER callers_login_identifier_update
            BEFORE UPDATE OF login_identifier ON callers
            WHEN NEW.login_identifier IS NULL
              OR length(NEW.login_identifier) NOT BETWEEN 1 AND 32
              OR NEW.login_identifier <> lower(NEW.login_identifier)
              OR substr(NEW.login_identifier, 1, 1) NOT GLOB '[a-z0-9]'
              OR NEW.login_identifier GLOB '*[^a-z0-9._-]*'
            BEGIN SELECT RAISE(ABORT, 'invalid login identifier'); END;
            "#,
        )
        .map_err(DatabaseError::Sqlite)?;
    validate_schema_13_snapshot(transaction)
}

fn validate_schema_13_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let identities = {
        let mut statement = connection
            .prepare("SELECT login_identifier, real_name FROM callers ORDER BY caller_id")
            .map_err(DatabaseError::Sqlite)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            })
            .map_err(DatabaseError::Sqlite)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Sqlite)?
    };
    let mut unique = HashSet::new();
    for (login_identifier, real_name) in identities {
        let login_identifier = login_identifier.ok_or_else(|| {
            DatabaseError::MigrationValidation(
                "schema-13 caller is missing a login identifier".to_owned(),
            )
        })?;
        canonicalize_login_identifier(login_identifier.as_bytes())
            .map_err(DatabaseError::InvalidStoredCaller)?;
        if !unique.insert(login_identifier) {
            return Err(DatabaseError::MigrationValidation(
                "schema-13 caller login identifiers are not unique".to_owned(),
            ));
        }
        if real_name.as_ref().is_some_and(|value| {
            value.is_empty() || value.len() > 120 || value.chars().any(char::is_control)
        }) {
            return Err(DatabaseError::MigrationValidation(
                "schema-13 caller real name is invalid".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_public_information_migration(
    transaction: &Transaction<'_>,
) -> Result<(), DatabaseError> {
    let expected: (i64,i64,i64,i64,i64,i64,i64,i64,i64) = transaction.query_row("SELECT caller_count,caller_id_sum,credential_count,message_count,file_count,login_bytes,handle_bytes,real_name_bytes,caller_state_sum FROM migration_14_validation", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?))).map_err(DatabaseError::Sqlite)?;
    let actual: (i64,i64,i64,i64,i64,i64,i64,i64,i64) = transaction.query_row("SELECT COUNT(*),COALESCE(SUM(caller_id),0),(SELECT COUNT(*) FROM caller_credentials),(SELECT COUNT(*) FROM messages),(SELECT COUNT(*) FROM files),COALESCE(SUM(length(login_identifier)),0),COALESCE(SUM(length(display_name)),0),COALESCE(SUM(length(COALESCE(real_name,''))),0),COALESCE(SUM(state_version),0) FROM callers", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?))).map_err(DatabaseError::Sqlite)?;
    if expected != actual {
        return Err(DatabaseError::MigrationValidation(
            "schema-14 migration changed schema-13 caller, credential, message, or file authority"
                .to_owned(),
        ));
    }
    let invalid_defaults: i64 = transaction.query_row("SELECT COUNT(*) FROM callers WHERE public_directory_listed<>0 OR publicity_state_version<>0", [], |r| r.get(0)).map_err(DatabaseError::Sqlite)?;
    let policy: (bool,bool,bool,bool,i64) = transaction.query_row("SELECT directory_enabled,show_last_call_date,show_city_region,caller_bbs_additions_enabled,state_version FROM public_information_policy WHERE singleton=1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(DatabaseError::Sqlite)?;
    let entries: i64 = transaction
        .query_row("SELECT COUNT(*) FROM other_bbs_entries", [], |r| r.get(0))
        .map_err(DatabaseError::Sqlite)?;
    let resources: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM public_information_resource_state",
            [],
            |r| r.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if invalid_defaults != 0
        || policy != (false, false, false, false, 1)
        || entries != 0
        || resources != 0
    {
        return Err(DatabaseError::MigrationValidation(
            "schema-14 privacy-safe defaults are invalid".to_owned(),
        ));
    }
    validate_schema_14_snapshot(transaction)
}

fn validate_schema_14_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let policy_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM public_information_policy WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if policy_rows != 1 {
        return Err(DatabaseError::IntegrityCheck(
            "schema-14 public-information policy singleton is missing".to_owned(),
        ));
    }
    let invalid_callers: i64 = connection.query_row("SELECT COUNT(*) FROM callers WHERE public_directory_listed NOT IN (0,1) OR publicity_state_version<0", [], |r| r.get(0)).map_err(DatabaseError::Sqlite)?;
    if invalid_callers != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "schema-14 caller publicity state is invalid".to_owned(),
        ));
    }
    let (count,min_order,max_order,distinct_order): (i64,Option<i64>,Option<i64>,i64) = connection.query_row("SELECT COUNT(*),MIN(display_order),MAX(display_order),COUNT(DISTINCT display_order) FROM other_bbs_entries", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(DatabaseError::Sqlite)?;
    if count > 512
        || (count > 0
            && (min_order != Some(1) || max_order != Some(count) || distinct_order != count))
    {
        return Err(DatabaseError::IntegrityCheck(
            "schema-14 Other BBS ordering is not dense and unique".to_owned(),
        ));
    }
    let invalid_events: i64 = connection.query_row("SELECT COUNT(*) FROM public_information_events WHERE (actor_kind IN ('caller','threshold-sysop'))<>(actor_caller_id IS NOT NULL) OR (operation IN ('caller-listed','caller-unlisted') AND subject_caller_id IS NULL) OR (operation LIKE 'other-bbs-%' AND other_bbs_entry_id IS NULL) OR (operation='resource-published' AND (resource_kind IS NULL OR resource_digest IS NULL)) OR (operation<>'resource-published' AND (resource_kind IS NOT NULL OR resource_digest IS NOT NULL))", [], |r| r.get(0)).map_err(DatabaseError::Sqlite)?;
    if invalid_events != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "schema-14 public-information audit semantics are invalid".to_owned(),
        ));
    }
    let invalid_resources: i64 = connection.query_row("SELECT COUNT(*) FROM public_information_resource_state WHERE generation<=0 OR published_at<0 OR length(sha256)<>64 OR sha256 GLOB '*[^0-9a-f]*'", [], |r| r.get(0)).map_err(DatabaseError::Sqlite)?;
    if invalid_resources != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "schema-14 public-resource generation state is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_file_maintenance_migration(transaction: &Transaction<'_>) -> Result<(), DatabaseError> {
    let expected: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) =
        transaction
            .query_row(
                "SELECT caller_count,caller_id_sum,credential_count,message_count,area_count,area_id_sum,file_count,file_id_sum,file_size_sum,download_count_sum,public_policy_count,other_bbs_count FROM migration_15_validation",
                [],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?)),
            )
            .map_err(DatabaseError::Sqlite)?;
    let actual: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) =
        transaction
            .query_row(
                "SELECT COUNT(*),COALESCE(SUM(caller_id),0),(SELECT COUNT(*) FROM caller_credentials),(SELECT COUNT(*) FROM messages),(SELECT COUNT(*) FROM file_areas),(SELECT COALESCE(SUM(area_id),0) FROM file_areas),(SELECT COUNT(*) FROM files),(SELECT COALESCE(SUM(file_id),0) FROM files),(SELECT COALESCE(SUM(size_bytes),0) FROM files),(SELECT COALESCE(SUM(download_count),0) FROM files),(SELECT COUNT(*) FROM public_information_policy),(SELECT COUNT(*) FROM other_bbs_entries) FROM callers",
                [],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?)),
            )
            .map_err(DatabaseError::Sqlite)?;
    if expected != actual {
        return Err(DatabaseError::MigrationValidation(
            "schema-15 migration changed schema-14 caller, credential, message, public-information, area, or file authority".to_owned(),
        ));
    }
    let migrated_files: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM files WHERE state_version<>1 OR integrity_state<>'unknown' OR description_source<>'legacy-import' OR lifecycle NOT IN ('active','disabled') OR review_submitted_at IS NOT NULL OR reviewed_at IS NOT NULL OR tombstoned_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let fabricated: i64 = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM file_requests)+(SELECT COUNT(*) FROM file_operations)+(SELECT COUNT(*) FROM file_operation_leases)+(SELECT COUNT(*) FROM file_active_uses)+(SELECT COUNT(*) FROM file_upload_denials)+(SELECT COUNT(*) FROM file_uppercase_terms)+(SELECT COUNT(*) FROM file_legacy_publications)+(SELECT COUNT(*) FROM file_events)",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let policy: (bool, String, i64) = transaction
        .query_row(
            "SELECT comprehensive_upload_search,description_normalization,state_version FROM file_policy WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    if migrated_files != 0 || fabricated != 0 || policy != (false, "preserve".to_owned(), 1) {
        return Err(DatabaseError::MigrationValidation(
            "schema-15 file-domain defaults are invalid".to_owned(),
        ));
    }
    validate_schema_15_snapshot(transaction)
}

fn validate_schema_15_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let policy_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_policy WHERE singleton=1 AND state_version>0",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_areas: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_areas WHERE state_version<=0",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_files: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM files WHERE state_version<=0 OR lifecycle NOT IN ('active','offline','pending-review','disabled','tombstoned') OR integrity_state NOT IN ('unknown','present','missing','digest-mismatch') OR (lifecycle='pending-review')<>(review_submitted_at IS NOT NULL AND reviewed_at IS NULL) OR (lifecycle='tombstoned')<>(tombstoned_at IS NOT NULL)",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_requests: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_requests WHERE state_version<=0 OR (status='pending')<>(resolved_at IS NULL AND resolved_by_caller_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_operations: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_operations WHERE updated_at<created_at OR (phase IN ('committed','rolled-back')) AND error_code IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_events WHERE request_id IS NOT NULL AND file_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if policy_rows != 1
        || invalid_areas != 0
        || invalid_files != 0
        || invalid_requests != 0
        || invalid_operations != 0
        || invalid_events != 0
    {
        return Err(DatabaseError::IntegrityCheck(
            "schema-15 file-domain state is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_transfer_storage_migration(transaction: &Transaction<'_>) -> Result<(), DatabaseError> {
    let expected: [i64; 16] =
        transaction
            .query_row(
                "SELECT caller_count,caller_id_sum,credential_count,message_count,area_count,area_id_sum,file_count,file_id_sum,file_size_sum,download_count_sum,request_count,operation_count,public_policy_count,other_bbs_count,access_event_count,adjustment_count FROM migration_16_validation",
                [],
                |row| Ok([row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?,row.get(13)?,row.get(14)?,row.get(15)?]),
            )
            .map_err(DatabaseError::Sqlite)?;
    let actual: [i64; 16] =
        transaction
            .query_row(
                "SELECT COUNT(*),COALESCE(SUM(caller_id),0),(SELECT COUNT(*) FROM caller_credentials),(SELECT COUNT(*) FROM messages),(SELECT COUNT(*) FROM file_areas),(SELECT COALESCE(SUM(area_id),0) FROM file_areas),(SELECT COUNT(*) FROM files),(SELECT COALESCE(SUM(file_id),0) FROM files),(SELECT COALESCE(SUM(size_bytes),0) FROM files),(SELECT COALESCE(SUM(download_count),0) FROM files),(SELECT COUNT(*) FROM file_requests),(SELECT COUNT(*) FROM file_operations),(SELECT COUNT(*) FROM public_information_policy),(SELECT COUNT(*) FROM other_bbs_entries),(SELECT COUNT(*) FROM caller_access_events),(SELECT COUNT(*) FROM caller_security_adjustments) FROM callers",
                [],
                |row| Ok([row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?,row.get(13)?,row.get(14)?,row.get(15)?]),
            )
            .map_err(DatabaseError::Sqlite)?;
    if expected != actual {
        return Err(DatabaseError::MigrationValidation(
            "schema-16 migration changed schema-15 caller, credential, message, file, request, operation, or public-information authority".to_owned(),
        ));
    }
    let fabricated_runtime: i64 = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM transfer_policies)+(SELECT COUNT(*) FROM transfer_daily_usage)+(SELECT COUNT(*) FROM transfer_records)+(SELECT COUNT(*) FROM transfer_quota_reservations)+(SELECT COUNT(*) FROM transfer_quota_reservation_items)+(SELECT COUNT(*) FROM transfer_settlements)+(SELECT COUNT(*) FROM transfer_events)",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let policy: (String, i64) = transaction
        .query_row(
            "SELECT timezone_name,state_version FROM transfer_timezone_policy WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let mapping: (i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM file_storage_roots),(SELECT COUNT(*) FROM file_areas),(SELECT COUNT(*) FROM file_storage_locators),(SELECT COUNT(*) FROM files)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    if fabricated_runtime != 0
        || policy != ("UTC".to_owned(), 1)
        || mapping.0 != mapping.1
        || mapping.2 != mapping.3
    {
        return Err(DatabaseError::MigrationValidation(
            "schema-16 transfer/storage defaults are invalid".to_owned(),
        ));
    }
    validate_schema_16_snapshot(transaction)
}

fn validate_schema_16_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let timezone_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM transfer_timezone_policy WHERE singleton=1 AND state_version>0",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_policies: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM transfer_policies WHERE state_version<=0 OR protocol_mask<0 OR protocol_mask>511",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_usage: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM transfer_daily_usage WHERE state_version<=0 OR length(board_day)<>10 OR timezone_policy_version<=0",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_reservations: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM transfer_quota_reservations WHERE state_version<=0 OR updated_at<created_at OR state NOT IN ('active','settled','released','needs-review')",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_roots: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_storage_roots WHERE state_version<=0 OR priority<0 OR priority>255 OR (priority=0 AND (root_kind<>'managed' OR access_mode<>'read-write'))",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let unmapped_files: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM files f LEFT JOIN file_storage_locators l ON l.file_id=f.file_id WHERE l.file_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    let invalid_locators: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_storage_locators l JOIN files f ON f.file_id=l.file_id JOIN file_storage_roots r ON r.storage_root_id=l.storage_root_id WHERE l.state_version<=0 OR f.area_id<>r.area_id",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if timezone_rows != 1
        || invalid_policies != 0
        || invalid_usage != 0
        || invalid_reservations != 0
        || invalid_roots != 0
        || unmapped_files != 0
        || invalid_locators != 0
    {
        return Err(DatabaseError::IntegrityCheck(
            "schema-16 transfer/accounting/storage state is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_zero_byte_file_migration(transaction: &Transaction<'_>) -> Result<(), DatabaseError> {
    let expected: (i64, i64, i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT file_count,file_id_sum,file_size_sum,download_count_sum,file_version_sum,semantic_bytes FROM migration_17_validation",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let actual: (i64, i64, i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT COUNT(*),COALESCE(SUM(file_id),0),COALESCE(SUM(size_bytes),0),COALESCE(SUM(download_count),0),COALESCE(SUM(state_version),0),COALESCE(SUM(length(filename)+length(normalized_filename)+length(CAST(description AS BLOB))+length(sha256)+length(uploader_name)+length(lifecycle)+length(integrity_state)+length(description_source)),0) FROM files",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let foreign_key_failures: i64 = transaction
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(DatabaseError::Sqlite)?;
    if expected != actual || foreign_key_failures != 0 {
        return Err(DatabaseError::MigrationValidation(
            "schema-17 migration changed file authority or broke a file reference".to_owned(),
        ));
    }
    validate_schema_17_snapshot(transaction)
}

fn validate_schema_17_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let invalid_files: i64 = connection
        .query_row("SELECT COUNT(*) FROM files WHERE size_bytes<0", [], |row| {
            row.get(0)
        })
        .map_err(DatabaseError::Sqlite)?;
    if invalid_files != 0 {
        return Err(DatabaseError::IntegrityCheck(
            "schema-17 file size authority is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_observability_migration(transaction: &Transaction<'_>) -> Result<(), DatabaseError> {
    let expected: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT caller_count,caller_id_sum,message_count,message_id_sum,file_count,file_id_sum,transfer_count,storage_root_count,audit_count FROM migration_18_validation",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let actual: (i64, i64, i64, i64, i64, i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM callers),(SELECT COALESCE(SUM(caller_id),0) FROM callers),(SELECT COUNT(*) FROM messages),(SELECT COALESCE(SUM(message_id),0) FROM messages),(SELECT COUNT(*) FROM files),(SELECT COALESCE(SUM(file_id),0) FROM files),(SELECT COUNT(*) FROM transfer_records),(SELECT COUNT(*) FROM file_storage_roots),(SELECT COUNT(*) FROM message_mutation_events)+(SELECT COUNT(*) FROM caller_access_events)+(SELECT COUNT(*) FROM caller_identity_events)+(SELECT COUNT(*) FROM public_information_events)+(SELECT COUNT(*) FROM file_events)+(SELECT COUNT(*) FROM transfer_events)",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let fabricated: (i64, i64, i64) = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM operational_events),(SELECT COUNT(*) FROM operational_daily_summaries),(SELECT COUNT(*) FROM operator_notifications)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    if expected != actual || fabricated != (0, 0, 0) {
        return Err(DatabaseError::MigrationValidation(
            "schema-18 migration changed existing authority or fabricated observability history"
                .to_owned(),
        ));
    }
    validate_schema_18_snapshot(transaction)
}

fn validate_schema_18_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let invalid: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM operational_events WHERE length(CAST(COALESCE(text_value_1,'') AS BLOB))+length(CAST(COALESCE(text_value_2,'') AS BLOB))+length(CAST(COALESCE(text_value_3,'') AS BLOB))>768),(SELECT COUNT(*) FROM operational_daily_summaries WHERE high_water_event_id<0 OR state_version<=0),(SELECT COUNT(*) FROM operational_retention_policy WHERE singleton<>1 OR detail_days NOT BETWEEN 1 AND 365 OR summary_days NOT BETWEEN 31 AND 3650 OR state_version<=0),(SELECT COUNT(*) FROM operator_notifications WHERE state_version<=0)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let policy_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM operational_retention_policy",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if invalid != (0, 0, 0, 0) || policy_count != 1 {
        return Err(DatabaseError::IntegrityCheck(
            "schema-18 observability authority is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_operator_control_migration(transaction: &Transaction<'_>) -> Result<(), DatabaseError> {
    let expected: (i64, i64, i64, i64, i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT caller_count,message_count,file_count,transfer_count,event_count,summary_count,notification_count,audit_count FROM migration_19_validation",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let actual: (i64, i64, i64, i64, i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM callers),(SELECT COUNT(*) FROM messages),(SELECT COUNT(*) FROM files),(SELECT COUNT(*) FROM transfer_records),(SELECT COUNT(*) FROM operational_events),(SELECT COUNT(*) FROM operational_daily_summaries),(SELECT COUNT(*) FROM operator_notifications),(SELECT COUNT(*) FROM operator_observability_audit)",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let fabricated: (i64, i64) = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM operator_command_journal),(SELECT COUNT(*) FROM operator_control_audit)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    if expected != actual || fabricated != (0, 0) {
        return Err(DatabaseError::MigrationValidation(
            "schema-19 migration changed existing authority or fabricated operator history"
                .to_owned(),
        ));
    }
    validate_schema_19_snapshot(transaction)
}

fn validate_schema_19_snapshot(connection: &Connection) -> Result<(), DatabaseError> {
    let invalid: (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM operator_command_journal WHERE expires_at<=received_at OR length(request_fingerprint)<>64),(SELECT COUNT(*) FROM operator_control_audit WHERE authorization_result NOT IN ('allowed','denied','not-applicable'))",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    if invalid != (0, 0) {
        return Err(DatabaseError::IntegrityCheck(
            "schema-19 operator-control authority is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_caller_access_migration(transaction: &Transaction<'_>) -> Result<(), DatabaseError> {
    let expected: (i64, i64, i64, i64, i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT caller_count, caller_id_sum, security_sum, active_count, disabled_count, deleted_count, credential_count, profile_bytes FROM migration_12_validation",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    let actual: (i64, i64, i64, i64, i64, i64, i64, i64) = transaction
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(caller_id),0), COALESCE(SUM(security_level),0), COALESCE(SUM(account_state='active'),0), COALESCE(SUM(account_state='disabled'),0), COALESCE(SUM(account_state='deleted'),0), (SELECT COUNT(*) FROM caller_credentials), COALESCE(SUM(length(COALESCE(address_line_1,'')) + length(COALESCE(address_line_2,'')) + length(COALESCE(city,'')) + length(COALESCE(region,'')) + length(COALESCE(postal_code,'')) + length(COALESCE(country,'')) + length(COALESCE(phone,'')) + length(COALESCE(email,'')) + length(COALESCE(birthday,''))),0) FROM callers",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
        )
        .map_err(DatabaseError::Sqlite)?;
    if expected != actual {
        return Err(DatabaseError::MigrationValidation(
            "schema-11 caller identity, credentials, profile, lifecycle, or security changed"
                .to_owned(),
        ));
    }
    let fabricated: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM callers WHERE state_version<>0 OR subscription_expires_on IS NOT NULL OR purge_protected<>1 OR lifecycle_prior_state IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if fabricated != 0 {
        return Err(DatabaseError::MigrationValidation(
            "schema-12 migration fabricated caller lifecycle or subscription state".to_owned(),
        ));
    }
    validate_schema_12_snapshot(transaction)
}

fn validate_message_mutation_migration(transaction: &Transaction<'_>) -> Result<(), DatabaseError> {
    let expected = transaction
        .query_row(
            r#"
            SELECT message_count, receipt_count, last_read_count, deleted_count,
                   private_count, parent_count, message_id_sum,
                   message_number_sum, subject_bytes, body_bytes
              FROM migration_11_validation
            "#,
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )
        .map_err(DatabaseError::Sqlite)?;
    let actual = transaction
        .query_row(
            r#"
            SELECT
                (SELECT COUNT(*) FROM messages),
                (SELECT COUNT(*) FROM caller_message_receipts),
                (SELECT COUNT(*) FROM caller_last_read),
                (SELECT COUNT(*) FROM messages WHERE lifecycle_state = 'deleted'),
                (SELECT COUNT(*) FROM messages WHERE visibility = 'private'),
                (SELECT COUNT(*) FROM messages WHERE parent_message_id IS NOT NULL),
                (SELECT COALESCE(SUM(message_id), 0) FROM messages),
                (SELECT COALESCE(SUM(message_number), 0) FROM messages),
                (SELECT COALESCE(SUM(length(subject)), 0) FROM message_payloads),
                (SELECT COALESCE(SUM(length(body)), 0) FROM message_payloads)
            "#,
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )
        .map_err(DatabaseError::Sqlite)?;
    if expected != actual {
        return Err(DatabaseError::MigrationValidation(
            "schema-10 message, receipt, pointer, state, identity, or payload counts changed"
                .to_owned(),
        ));
    }

    let recipient_mismatch: i64 = transaction
        .query_row(
            r#"
            SELECT COUNT(*)
              FROM messages AS m
              LEFT JOIN message_delivery_recipients AS r
                ON r.message_id = m.message_id
             WHERE (m.audience_kind = 'local-recipient' AND r.message_id IS NULL)
                OR (m.audience_kind = 'all-callers' AND r.message_id IS NOT NULL)
            "#,
            [],
            |row| row.get(0),
        )
        .map_err(DatabaseError::Sqlite)?;
    if recipient_mismatch != 0 {
        return Err(DatabaseError::MigrationValidation(
            "schema-11 recipient cardinality validation failed".to_owned(),
        ));
    }

    let mut statement = transaction
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
    #[error("schema migration validation failed: {0}")]
    MigrationValidation(String),
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
    #[error("stored subscription date is not strict ISO YYYY-MM-DD: {0:?}")]
    InvalidSubscriptionDate(String),
    #[error("caller access mutation conflicts with state version {actual}; expected {expected}")]
    CallerStateConflict { expected: u64, actual: u64 },
    #[error("configured named Sysop is protected from this caller mutation")]
    ProtectedNamedSysop,
    #[error("caller access mutation is not authorized")]
    CallerAccessUnauthorized,
    #[error(transparent)]
    PublicInformation(#[from] PublicInformationError),
    #[error(transparent)]
    Credential(#[from] CredentialError),
    #[error("caller name is already registered: {0:?}")]
    DuplicateCaller(String),
    #[error("caller login identifier or display handle is already registered")]
    DuplicateCallerIdentity,
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
                applied: 19,
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
                applied: 10,
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
    fn schema_ten_to_eleven_preserves_message_bytes_identity_receipts_pointers_and_tombstones() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(10) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute_batch("PRAGMA foreign_keys = OFF;")
            .unwrap();
        connection
            .execute_batch(
                r#"
                INSERT INTO callers (
                    caller_id, display_name, normalized_name, security_level,
                    account_state, is_new_caller, first_call_at
                ) VALUES
                    (1, 'Schema Ten Author', 'schema ten author', 10, 'active', 0, 1),
                    (2, 'Schema Ten Recipient', 'schema ten recipient', 10, 'active', 0, 1);
                INSERT INTO message_conferences (
                    conference_id, conference_number, name, description,
                    access_mode, read_security, post_security, public_only,
                    maximum_lines
                ) VALUES (1, 1, 'General', 'Migration fixture', 'at-least', 5, 5, 0, 50);
                INSERT INTO messages (
                    message_id, conference_id, message_number, author_caller_id,
                    author_name, recipient_caller_id, recipient_name, subject,
                    body, created_at, visibility, kind, deleted
                ) VALUES (
                    41, 1, 17, 1, 'Schema Ten Author', 2,
                    'Schema Ten Recipient', X'4D303431DB', X'426F6479B30D0A',
                    1234, 'private', 'standard', 1
                );
                INSERT INTO caller_message_receipts (caller_id, message_id, received_at)
                VALUES (2, 41, '2026-08-28 12:00:00');
                INSERT INTO caller_last_read (caller_id, conference_id, last_message_number)
                VALUES (2, 1, 17);
                "#,
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 10,
                ending_version: SCHEMA_VERSION,
                applied: 9,
            }
        );
        let preserved = database
            .connection
            .query_row(
                r#"
                SELECT m.message_id, m.message_number, p.subject, p.body,
                       m.visibility, m.lifecycle_state, m.state_version,
                       r.caller_id, r.display_name_snapshot
                  FROM messages AS m
                  JOIN message_fanouts AS f ON f.fanout_id = m.fanout_id
                  JOIN message_payloads AS p ON p.payload_id = f.payload_id
                  JOIN message_delivery_recipients AS r ON r.message_id = m.message_id
                 WHERE m.message_id = 41
                "#,
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            preserved,
            (
                41,
                17,
                b"M041\xDB".to_vec(),
                b"Body\xB3\r\n".to_vec(),
                "private".to_owned(),
                "deleted".to_owned(),
                1,
                2,
                "Schema Ten Recipient".to_owned(),
            )
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM caller_message_receipts", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT last_message_number FROM caller_last_read",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            17
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
    }

    #[test]
    fn failed_schema_eleven_validation_rolls_back_to_unchanged_schema_ten() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
            )
            .unwrap();
        for migration in MIGRATIONS.iter().take(10) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute_batch("PRAGMA foreign_keys = OFF;")
            .unwrap();
        connection
            .execute_batch(
                r#"
                INSERT INTO callers (
                    caller_id, display_name, normalized_name, security_level,
                    account_state, is_new_caller, first_call_at
                ) VALUES (1, 'Schema Ten Caller', 'schema ten caller', 10, 'active', 0, 1);
                INSERT INTO message_conferences (
                    conference_id, conference_number, name, description,
                    access_mode, read_security, post_security, public_only,
                    maximum_lines
                ) VALUES (1, 1, 'General', 'Rollback fixture', 'at-least', 5, 5, 0, 50);
                INSERT INTO messages (
                    message_id, conference_id, message_number, author_caller_id,
                    author_name, recipient_name, subject, body, created_at,
                    visibility, kind
                ) VALUES (1, 1, 1, 1, 'Schema Ten Caller', 'All Callers',
                          X'5375626A656374', X'426F64790D0A', 1, 'public', 'standard');
                INSERT INTO caller_message_receipts (caller_id, message_id)
                VALUES (1, 999);
                "#,
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert!(database.migrate().is_err());
        assert_eq!(database.schema_version().unwrap(), 10);
        let legacy_subject_column: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'subject'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_subject_column, 1);
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
    fn schema_eleven_to_twelve_preserves_callers_without_fabricated_access_state() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
        ).unwrap();
        for migration in MIGRATIONS.iter().take(11) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute(
            "INSERT INTO callers (caller_id, display_name, normalized_name, security_level, account_state, is_new_caller, first_call_at, phone) VALUES (7, 'Schema Eleven Caller', 'schema eleven caller', 37, 'disabled', 0, 1, 'private-test-value')",
            [],
        ).unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 11,
                ending_version: SCHEMA_VERSION,
                applied: 8,
            }
        );
        let caller = database
            .caller_by_id(CallerId::new(7).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(caller.id.get(), 7);
        assert_eq!(caller.state, CallerState::Disabled);
        assert_eq!(caller.base_security_level.get(), 37);
        assert_eq!(caller.security_level.get(), 37);
        assert_eq!(caller.state_version, 0);
        assert_eq!(caller.subscription_expires_on, None);
        assert!(caller.purge_protected);
        assert_eq!(caller.profile.phone.as_deref(), Some("private-test-value"));
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM caller_access_events", [], |row| row
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            0
        );
    }

    #[test]
    fn failed_schema_twelve_validation_rolls_back_to_unchanged_schema_eleven() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
        ).unwrap();
        for migration in MIGRATIONS.iter().take(11) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute(
            "INSERT INTO callers (caller_id, display_name, normalized_name, security_level, account_state, is_new_caller, first_call_at) VALUES (9, 'Rollback Caller', 'rollback caller', 25, 'active', 0, 1)",
            [],
        ).unwrap();
        let broken_sql = MIGRATIONS[11].sql.replace(
            "ALTER TABLE callers ADD COLUMN state_version",
            "UPDATE callers SET security_level = security_level + 1;\nALTER TABLE callers ADD COLUMN state_version",
        );
        let broken = Migration {
            version: 12,
            name: "auditable_caller_access_lifecycle",
            sql: Box::leak(broken_sql.into_boxed_str()),
        };
        assert!(apply_migration(&mut connection, &broken).is_err());
        assert_eq!(schema_version_from(&connection).unwrap(), 11);
        assert_eq!(
            connection
                .query_row(
                    "SELECT security_level FROM callers WHERE caller_id=9",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            25
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('callers') WHERE name='state_version'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='caller_access_events'", [], |row| row.get::<_, i64>(0)
        ).unwrap(), 0);
    }

    #[test]
    fn schema_twelve_to_thirteen_preserves_identity_and_resolves_login_collisions() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK (version > 0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
        ).unwrap();
        for migration in MIGRATIONS.iter().take(12) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute_batch(
                r#"
            INSERT INTO callers (
                caller_id, display_name, normalized_name, security_level,
                account_state, is_new_caller, first_call_at, state_version,
                purge_protected
            ) VALUES
                (1, 'Pixel Wizard', 'pixel wizard', 20, 'active', 0, 1, 4, 1),
                (2, 'Pixel-Wizard', 'pixel-wizard', 30, 'disabled', 0, 1, 7, 0);
            "#,
            )
            .unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 12,
                ending_version: SCHEMA_VERSION,
                applied: 7,
            }
        );
        let first = database
            .caller_by_id(CallerId::new(1).unwrap())
            .unwrap()
            .unwrap();
        let second = database
            .caller_by_id(CallerId::new(2).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(first.login_identifier, "pixel-wizard");
        assert_eq!(second.login_identifier, "pixel-wizard-2");
        assert_eq!(first.display_name, "Pixel Wizard");
        assert_eq!(first.real_name.as_deref(), Some("Pixel Wizard"));
        assert_eq!(first.state_version, 4);
        assert_eq!(second.state, CallerState::Disabled);
        assert!(!second.purge_protected);
        assert_eq!(
            database
                .caller_by_login_identifier(b"PIXEL-WIZARD-2")
                .unwrap()
                .unwrap()
                .id,
            second.id
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM caller_identity_events", [], |row| row
                    .get::<_, i64>(0),)
                .unwrap(),
            0
        );
    }

    #[test]
    fn schema_thirteen_to_fourteen_preserves_identity_and_defaults_private() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(13) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Migration Board','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,real_name,security_level,account_state,is_new_caller,first_call_at,state_version) VALUES(42,'pixelwizard','PixelWizard','PIXELWIZARD','Private Fixture Name',25,'active',0,1700000000,7)", []).unwrap();
        connection.execute("INSERT INTO caller_credentials(caller_id,scheme,password_hash) VALUES(42,?1,'synthetic-hash')", [CREDENTIAL_SCHEME]).unwrap();
        connection.execute("INSERT INTO message_conferences(conference_id,conference_number,name,description,access_mode,read_security,post_security,public_only,maximum_lines) VALUES(1,1,'Migration Messages','Preserved','at-least',0,0,0,50)", []).unwrap();
        connection.execute("INSERT INTO message_payloads(payload_id,subject,body,content_kind) VALUES(8,x'507265736572766564',x'4D657373616765204279746573','standard')", []).unwrap();
        connection.execute("INSERT INTO message_fanouts(fanout_id,payload_id,created_by_caller_id,created_at) VALUES(8,8,42,1700000001)", []).unwrap();
        connection.execute("INSERT INTO messages(message_id,fanout_id,conference_id,message_number,author_caller_id,author_name,created_at,placed_at,audience_kind,visibility,lifecycle_state,delivery_role,delivery_ordinal) VALUES(8,8,1,1,42,'PixelWizard',1700000001,1700000001,'all-callers','public','active','single',0)", []).unwrap();
        connection.execute("INSERT INTO file_areas(area_id,area_number,name,description,storage_key,access_mode,read_security,upload_security,maximum_upload_bytes) VALUES(1,1,'Migration Files','Preserved','migration-files','at-least',0,0,1048576)", []).unwrap();
        connection.execute("INSERT INTO files(file_id,area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_caller_id,uploader_name) VALUES(9,1,'PRESERVE.TXT','PRESERVE.TXT','Preserved file',4,?1,1700000002,42,'PixelWizard')", ["11".repeat(32)]).unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 13,
                ending_version: SCHEMA_VERSION,
                applied: 6
            }
        );
        let caller = database
            .caller_by_id(CallerId::new(42).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(caller.login_identifier, "pixelwizard");
        assert_eq!(caller.display_name, "PixelWizard");
        assert_eq!(caller.real_name.as_deref(), Some("Private Fixture Name"));
        assert_eq!(caller.state_version, 7);
        let preserved_credential: (String, String) = database
            .connection
            .query_row(
                "SELECT scheme,password_hash FROM caller_credentials WHERE caller_id=42",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            preserved_credential,
            (CREDENTIAL_SCHEME.to_owned(), "synthetic-hash".to_owned())
        );
        let preserved_message: (Vec<u8>, Vec<u8>, i64) = database.connection.query_row("SELECT p.subject,p.body,m.author_caller_id FROM messages m JOIN message_fanouts f ON f.fanout_id=m.fanout_id JOIN message_payloads p ON p.payload_id=f.payload_id WHERE m.message_id=8", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert_eq!(
            preserved_message,
            (b"Preserved".to_vec(), b"Message Bytes".to_vec(), 42)
        );
        let preserved_file: (String, String, i64) = database
            .connection
            .query_row(
                "SELECT filename,sha256,uploader_caller_id FROM files WHERE file_id=9",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            preserved_file,
            ("PRESERVE.TXT".to_owned(), "11".repeat(32), 42)
        );
        assert!(!caller.public_directory_listed);
        assert_eq!(caller.publicity_state_version, 0);
        assert!(!database.public_directory_policy().unwrap().enabled);
        assert!(database.other_bbs_entries(true).unwrap().is_empty());
        database.validate_current_snapshot().unwrap();
    }

    #[test]
    fn failed_schema_fourteen_validation_rolls_back_to_unchanged_schema_thirteen() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(13) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Rollback Board','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,real_name,security_level,account_state,is_new_caller,first_call_at) VALUES(7,'rollback','Rollback Caller','ROLLBACK CALLER','Private Rollback Name',10,'active',0,1700000000)", []).unwrap();
        let broken_sql = MIGRATIONS[13].sql.replace(
            "INSERT INTO public_information_policy (singleton) VALUES (1);",
            "INSERT INTO public_information_policy (singleton, directory_enabled) VALUES (1, 1);",
        );
        let broken = Migration {
            version: 14,
            name: MIGRATIONS[13].name,
            sql: Box::leak(broken_sql.into_boxed_str()),
        };
        let error = apply_migration(&mut connection, &broken).unwrap_err();
        assert!(matches!(error, DatabaseError::MigrationValidation(_)));
        assert_eq!(schema_version_from(&connection).unwrap(), 13);
        let publicity_column: i64 = connection.query_row("SELECT COUNT(*) FROM pragma_table_info('callers') WHERE name='public_directory_listed'", [], |row| row.get(0)).unwrap();
        assert_eq!(publicity_column, 0);
        let preserved: (String, String, Option<String>) = connection
            .query_row(
                "SELECT login_identifier,display_name,real_name FROM callers WHERE caller_id=7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            preserved,
            (
                "rollback".to_owned(),
                "Rollback Caller".to_owned(),
                Some("Private Rollback Name".to_owned())
            )
        );
    }

    #[test]
    fn schema_fourteen_to_fifteen_preserves_authority_and_creates_no_work() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(14) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Schema Fifteen Board','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,real_name,security_level,account_state,is_new_caller,first_call_at,state_version) VALUES(42,'filecaller','FileCaller','FILECALLER','Private Fixture Name',25,'active',0,1700000000,7)", []).unwrap();
        connection.execute("INSERT INTO caller_credentials(caller_id,scheme,password_hash) VALUES(42,?1,'synthetic-hash')", [CREDENTIAL_SCHEME]).unwrap();
        connection.execute("INSERT INTO file_areas(area_id,area_number,name,description,storage_key,access_mode,read_security,upload_security,maximum_upload_bytes) VALUES(8,1,'Migration Files','Preserved','migration-files','at-least',0,0,1048576)", []).unwrap();
        connection.execute("INSERT INTO files(file_id,area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_caller_id,uploader_name,download_count,state) VALUES(9,8,'PRESERVE.TXT','PRESERVE.TXT','Preserved file',4,?1,1700000002,42,'FileCaller',3,'available')", ["11".repeat(32)]).unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 14,
                ending_version: SCHEMA_VERSION,
                applied: 5,
            }
        );
        let preserved: (i64, String, String, i64, String, String, i64) = database.connection.query_row(
            "SELECT area_id,filename,sha256,download_count,lifecycle,integrity_state,state_version FROM files WHERE file_id=9",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)),
        ).unwrap();
        assert_eq!(
            preserved,
            (
                8,
                "PRESERVE.TXT".to_owned(),
                "11".repeat(32),
                3,
                "active".to_owned(),
                "unknown".to_owned(),
                1
            )
        );
        let fabricated: i64 = database.connection.query_row(
            "SELECT (SELECT COUNT(*) FROM file_requests)+(SELECT COUNT(*) FROM file_operations)+(SELECT COUNT(*) FROM file_events)",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(fabricated, 0);
        assert_eq!(
            database
                .caller_by_id(CallerId::new(42).unwrap())
                .unwrap()
                .unwrap()
                .real_name
                .as_deref(),
            Some("Private Fixture Name")
        );
        database.validate_current_snapshot().unwrap();
    }

    #[test]
    fn failed_schema_fifteen_validation_rolls_back_to_unchanged_schema_fourteen() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(14) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Rollback Fifteen','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,real_name,security_level,account_state,is_new_caller,first_call_at) VALUES(7,'rollback-file','Rollback File','ROLLBACK FILE','Private Rollback Name',10,'active',0,1700000000)", []).unwrap();
        let broken_sql = MIGRATIONS[14].sql.replace(
            "INSERT INTO file_policy(singleton) VALUES(1);",
            "INSERT INTO file_policy(singleton,comprehensive_upload_search) VALUES(1,1);",
        );
        let broken = Migration {
            version: 15,
            name: MIGRATIONS[14].name,
            sql: Box::leak(broken_sql.into_boxed_str()),
        };
        let error = apply_migration(&mut connection, &broken).unwrap_err();
        assert!(matches!(error, DatabaseError::MigrationValidation(_)));
        assert_eq!(schema_version_from(&connection).unwrap(), 14);
        let lifecycle_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name='lifecycle'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lifecycle_column, 0);
        let preserved: (String, String, Option<String>) = connection
            .query_row(
                "SELECT login_identifier,display_name,real_name FROM callers WHERE caller_id=7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            preserved,
            (
                "rollback-file".to_owned(),
                "Rollback File".to_owned(),
                Some("Private Rollback Name".to_owned())
            )
        );
    }

    #[test]
    fn schema_fifteen_to_sixteen_preserves_authority_and_maps_storage() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(15) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Schema Sixteen Board','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,real_name,security_level,account_state,is_new_caller,first_call_at,state_version) VALUES(42,'transfercaller','TransferCaller','TRANSFERCALLER','Private Fixture Name',25,'active',0,1700000000,7)", []).unwrap();
        connection.execute("INSERT INTO caller_credentials(caller_id,scheme,password_hash) VALUES(42,?1,'synthetic-hash')", [CREDENTIAL_SCHEME]).unwrap();
        connection.execute("INSERT INTO file_areas(area_id,area_number,name,description,storage_key,access_mode,read_security,upload_security,maximum_upload_bytes,state_version) VALUES(8,1,'Migration Files','Preserved','migration-files','at-least',0,0,1048576,4)", []).unwrap();
        connection.execute("INSERT INTO files(file_id,area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_caller_id,uploader_name,download_count,lifecycle,integrity_state,state_version,description_source) VALUES(9,8,'PRESERVE.TXT','PRESERVE.TXT','Preserved file',4,?1,1700000002,42,'TransferCaller',3,'active','present',6,'caller')", ["11".repeat(32)]).unwrap();
        drop(connection);

        let mut database = RuntimeDatabase::open(&path).unwrap();
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                starting_version: 15,
                ending_version: SCHEMA_VERSION,
                applied: 4,
            }
        );
        let preserved: (i64, String, String, i64, i64, String) = database
            .connection
            .query_row(
                "SELECT f.area_id,f.filename,f.sha256,f.download_count,f.state_version,c.real_name FROM files f JOIN callers c ON c.caller_id=f.uploader_caller_id WHERE f.file_id=9",
                [],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
            )
            .unwrap();
        assert_eq!(
            preserved,
            (
                8,
                "PRESERVE.TXT".to_owned(),
                "11".repeat(32),
                3,
                6,
                "Private Fixture Name".to_owned()
            )
        );
        let mapping: (String, String, String, i64) = database.connection.query_row(
            "SELECT r.stable_key,r.configured_locator,l.relative_path,l.state_version FROM file_storage_roots r JOIN file_storage_locators l ON l.storage_root_id=r.storage_root_id WHERE l.file_id=9",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
        ).unwrap();
        assert_eq!(
            mapping,
            (
                "area-8-primary".to_owned(),
                "migration-files".to_owned(),
                "PRESERVE.TXT".to_owned(),
                1
            )
        );
        let fabricated: i64 = database.connection.query_row(
            "SELECT (SELECT COUNT(*) FROM transfer_policies)+(SELECT COUNT(*) FROM transfer_daily_usage)+(SELECT COUNT(*) FROM transfer_records)+(SELECT COUNT(*) FROM transfer_quota_reservations)+(SELECT COUNT(*) FROM transfer_settlements)+(SELECT COUNT(*) FROM transfer_events)",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(fabricated, 0);
        database.validate_current_snapshot().unwrap();
    }

    #[test]
    fn failed_schema_sixteen_validation_rolls_back_to_unchanged_schema_fifteen() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(15) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Rollback Sixteen','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,real_name,security_level,account_state,is_new_caller,first_call_at) VALUES(7,'rollback-transfer','Rollback Transfer','ROLLBACK TRANSFER','Private Rollback Name',10,'active',0,1700000000)", []).unwrap();
        connection.execute("INSERT INTO file_areas(area_id,area_number,name,description,storage_key,access_mode,read_security,upload_security,maximum_upload_bytes) VALUES(3,1,'Rollback Files','Preserved','rollback-files','at-least',0,0,1048576)", []).unwrap();
        let broken_sql = MIGRATIONS[15].sql.replace(
            "INSERT INTO transfer_timezone_policy(singleton,timezone_name) VALUES(1,'UTC');",
            "INSERT INTO transfer_timezone_policy(singleton,timezone_name) VALUES(1,'Etc/Broken');",
        );
        let broken = Migration {
            version: 16,
            name: MIGRATIONS[15].name,
            sql: Box::leak(broken_sql.into_boxed_str()),
        };
        let error = apply_migration(&mut connection, &broken).unwrap_err();
        assert!(matches!(error, DatabaseError::MigrationValidation(_)));
        assert_eq!(schema_version_from(&connection).unwrap(), 15);
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='transfer_timezone_policy'", [], |row| row.get::<_,i64>(0)).unwrap(), 0);
        assert_eq!(
            connection
                .query_row(
                    "SELECT storage_key FROM file_areas WHERE area_id=3",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "rollback-files"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT real_name FROM callers WHERE caller_id=7",
                    [],
                    |row| row.get::<_, Option<String>>(0)
                )
                .unwrap()
                .as_deref(),
            Some("Private Rollback Name")
        );
    }

    #[test]
    fn schema_sixteen_to_seventeen_preserves_file_references_and_accepts_zero_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(16) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Schema Seventeen Board','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,real_name,security_level,account_state,is_new_caller,first_call_at) VALUES(42,'zerocaller','ZeroCaller','ZEROCALLER','Private Fixture Name',25,'active',0,1700000000)", []).unwrap();
        connection.execute("INSERT INTO file_areas(area_id,area_number,name,description,storage_key,access_mode,read_security,upload_security,maximum_upload_bytes,state_version) VALUES(8,1,'Migration Files','Preserved','migration-files','at-least',0,0,1048576,4)", []).unwrap();
        connection.execute("INSERT INTO file_storage_roots(storage_root_id,area_id,stable_key,label,root_kind,access_mode,priority,configured_locator,configured_state,availability,staging_policy) VALUES(8,8,'area-8-primary','Migration primary','managed','read-write',0,'migration-files','enabled','available','direct-if-safe')", []).unwrap();
        connection.execute("INSERT INTO files(file_id,area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_caller_id,uploader_name,download_count,lifecycle,integrity_state,state_version,description_source) VALUES(9,8,'PRESERVE.TXT','PRESERVE.TXT','Preserved file',4,?1,1700000002,42,'ZeroCaller',3,'offline','present',6,'caller')", ["11".repeat(32)]).unwrap();
        connection.execute("INSERT INTO file_storage_locators(file_id,storage_root_id,relative_path,state_version) VALUES(9,8,'PRESERVE.TXT',3)", []).unwrap();
        connection.execute("INSERT INTO file_requests(request_id,file_id,requesting_caller_id,created_board_day,reason,status,state_version) VALUES(5,9,42,'2026-08-30','offline','pending',2)", []).unwrap();

        apply_migration(&mut connection, &MIGRATIONS[16]).unwrap();
        assert_eq!(schema_version_from(&connection).unwrap(), 17);
        assert_eq!(
            connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        let preserved: (String, i64, i64, i64, i64) = connection.query_row(
            "SELECT f.filename,f.size_bytes,f.state_version,l.state_version,r.state_version FROM files f JOIN file_storage_locators l ON l.file_id=f.file_id JOIN file_requests r ON r.file_id=f.file_id WHERE f.file_id=9",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
        ).unwrap();
        assert_eq!(preserved, ("PRESERVE.TXT".to_owned(), 4, 6, 3, 2));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );

        connection.execute("INSERT INTO files(file_id,area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_caller_id,uploader_name,lifecycle,integrity_state,state_version,description_source) VALUES(10,8,'EMPTY.BIN','EMPTY.BIN','Valid empty file',0,'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',1700000003,42,'ZeroCaller','active','present',1,'caller')", []).unwrap();
        connection.execute("INSERT INTO file_storage_locators(file_id,storage_root_id,relative_path) VALUES(10,8,'EMPTY.BIN')", []).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT size_bytes FROM files WHERE file_id=10", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
    }

    #[test]
    fn failed_schema_seventeen_rebuild_rolls_back_and_restores_foreign_keys() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(16) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO file_areas(area_id,area_number,name,description,storage_key,access_mode,read_security,upload_security,maximum_upload_bytes,state_version) VALUES(8,1,'Rollback Files','Preserved','rollback-files','at-least',0,0,1048576,1)", []).unwrap();
        connection.execute("INSERT INTO file_storage_roots(storage_root_id,area_id,stable_key,label,root_kind,access_mode,priority,configured_locator,configured_state,availability,staging_policy) VALUES(8,8,'area-8-primary','Rollback primary','managed','read-write',0,'rollback-files','enabled','available','direct-if-safe')", []).unwrap();
        connection.execute("INSERT INTO files(file_id,area_id,filename,normalized_filename,description,size_bytes,sha256,uploaded_at,uploader_name,lifecycle,integrity_state,state_version,description_source) VALUES(9,8,'PRESERVE.TXT','PRESERVE.TXT','Preserved file',4,?1,1700000002,'Fixture', 'active','present',6,'caller')", ["11".repeat(32)]).unwrap();
        connection.execute("INSERT INTO file_storage_locators(file_id,storage_root_id,relative_path) VALUES(9,8,'PRESERVE.TXT')", []).unwrap();
        let broken_sql = MIGRATIONS[16].sql.replace(
            "DROP TABLE files_schema_16;",
            "DROP TABLE missing_schema_16;",
        );
        let broken = Migration {
            version: 17,
            name: MIGRATIONS[16].name,
            sql: Box::leak(broken_sql.into_boxed_str()),
        };
        assert!(matches!(
            apply_migration(&mut connection, &broken),
            Err(DatabaseError::Sqlite(_))
        ));
        assert_eq!(schema_version_from(&connection).unwrap(), 16);
        assert_eq!(
            connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='files_schema_16'", [], |row| row.get::<_,i64>(0)).unwrap(), 0);
        assert_eq!(
            connection
                .query_row("SELECT size_bytes FROM files WHERE file_id=9", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            4
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
    }

    #[test]
    fn schema_seventeen_to_eighteen_preserves_authority_without_fabricated_history() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(17) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Schema Eighteen Board','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,security_level,account_state,is_new_caller,first_call_at,call_count,messages_posted) VALUES(42,'private-login','Public Handle','PUBLIC HANDLE',25,'active',0,1700000000,8,3)", []).unwrap();
        apply_migration(&mut connection, &MIGRATIONS[17]).unwrap();
        assert_eq!(schema_version_from(&connection).unwrap(), 18);
        assert_eq!(
            connection
                .query_row(
                    "SELECT call_count FROM callers WHERE caller_id=42",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            8
        );
        let empty: (i64, i64, i64) = connection.query_row("SELECT (SELECT COUNT(*) FROM operational_events),(SELECT COUNT(*) FROM operational_daily_summaries),(SELECT COUNT(*) FROM operator_notifications)", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert_eq!(empty, (0, 0, 0));
        assert_eq!(
            connection
                .query_row(
                    "SELECT detail_days FROM operational_retention_policy WHERE singleton=1",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            30
        );
    }

    #[test]
    fn failed_schema_eighteen_validation_rolls_back_to_unchanged_schema_seventeen() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(17) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Rollback Eighteen','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO callers(caller_id,login_identifier,display_name,normalized_name,security_level,account_state,is_new_caller,first_call_at,call_count) VALUES(7,'rollback-observe','Rollback Observe','ROLLBACK OBSERVE',10,'active',0,1700000000,9)", []).unwrap();
        let broken_sql = MIGRATIONS[17]
            .sql
            .replace("VALUES(1,30,400", "VALUES(1,0,400");
        let broken = Migration {
            version: 18,
            name: MIGRATIONS[17].name,
            sql: Box::leak(broken_sql.into_boxed_str()),
        };
        assert!(apply_migration(&mut connection, &broken).is_err());
        assert_eq!(schema_version_from(&connection).unwrap(), 17);
        assert_eq!(
            connection
                .query_row(
                    "SELECT call_count FROM callers WHERE caller_id=7",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            9
        );
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='operational_events'", [], |row| row.get::<_,i64>(0)).unwrap(), 0);
    }

    #[test]
    fn schema_eighteen_to_nineteen_preserves_authority_without_fabricated_control_history() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(18) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Schema Nineteen Board','Fixture Sysop')", []).unwrap();
        connection.execute("INSERT INTO operational_events(occurred_at_utc,board_day,timezone_policy_version,category,severity,event_code,outcome,retention_class,attribute_kind) VALUES(1700000000,20231114,1,'system','notice','system.test','observed','operational','none')", []).unwrap();
        apply_migration(&mut connection, &MIGRATIONS[18]).unwrap();
        assert_eq!(schema_version_from(&connection).unwrap(), 19);
        let counts:(i64,i64,i64)=connection.query_row("SELECT (SELECT COUNT(*) FROM operational_events),(SELECT COUNT(*) FROM operator_command_journal),(SELECT COUNT(*) FROM operator_control_audit)", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert_eq!(counts, (1, 0, 0));
    }

    #[test]
    fn failed_schema_nineteen_migration_rolls_back_to_unchanged_schema_eighteen() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY CHECK(version>0), name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        for migration in MIGRATIONS.iter().take(18) {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO board_identity(singleton,board_name,sysop_name) VALUES(1,'Rollback Nineteen','Fixture Sysop')", []).unwrap();
        let broken_sql = MIGRATIONS[18].sql.replace(
            "CREATE TABLE operator_control_audit",
            "CREATE TABLE operator_command_journal",
        );
        let broken = Migration {
            version: 19,
            name: MIGRATIONS[18].name,
            sql: Box::leak(broken_sql.into_boxed_str()),
        };
        assert!(apply_migration(&mut connection, &broken).is_err());
        assert_eq!(schema_version_from(&connection).unwrap(), 18);
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='operator_command_journal'", [], |row| row.get::<_,i64>(0)).unwrap(),0);
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
    fn schema_12_snapshot_rejects_an_impossible_subscription_date() {
        let temp = tempfile::tempdir().unwrap();
        let path = database_path(&temp);
        let mut database = RuntimeDatabase::open(&path).unwrap();
        database.migrate().unwrap();
        let identity = BoardIdentity::new("Date Validation Board", "Date Sysop").unwrap();
        database.ensure_board_identity(&identity).unwrap();
        let encoded = test_hasher().hash(b"test-only date password").unwrap();
        let caller = database
            .create_caller(
                b"Date Validation Caller",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        database
            .connection
            .execute(
                "UPDATE callers SET subscription_expires_on='2026-02-30' WHERE caller_id=?1",
                [caller.id.get()],
            )
            .unwrap();

        assert!(matches!(
            database.validate_current_snapshot(),
            Err(DatabaseError::InvalidSubscriptionDate(value)) if value == "2026-02-30"
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
                applied: 18,
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
                applied: 17,
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
                applied: 16,
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
                applied: 15,
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
                applied: 14,
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
                applied: 13,
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
                applied: 12,
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
                applied: 11,
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
                b"Craig   Test",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                true,
                100,
            )
            .unwrap();
        assert_eq!(caller.display_name, "Craig Test");
        assert!(matches!(
            database.authenticate(b"craig test", password, &hasher).unwrap(),
            AuthenticationResult::Valid(found) if found.id == caller.id
        ));
        assert_eq!(
            database
                .authenticate(b"CRAIG TEST", b"wrong", &hasher)
                .unwrap(),
            AuthenticationResult::Invalid
        );
        assert!(matches!(
            database.create_caller(
                b"CRAIG TEST",
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
    fn caller_identity_is_independent_versioned_unique_and_privacy_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&database_path(&temp)).unwrap();
        database.migrate().unwrap();
        let hasher = test_hasher();
        let encoded = hasher.hash(b"test-only identity password").unwrap();
        let caller = database
            .create_caller(
                b"Legacy Public Name",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        let other = database
            .create_caller(
                b"Another Caller",
                &encoded,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        let config = CallerConfig {
            sysop_caller_name: "Sysop".to_owned(),
            ..CallerConfig::default()
        };
        let updated = database
            .update_caller_identity(
                caller.id,
                caller.state_version,
                b"pixelwizard",
                b"PixelWizard",
                Some("Craig Identity Fixture".to_owned()),
                &config,
                200,
            )
            .unwrap();
        assert_eq!(updated.login_identifier, "pixelwizard");
        assert_eq!(updated.display_name, "PixelWizard");
        assert_eq!(updated.real_name.as_deref(), Some("Craig Identity Fixture"));
        assert_eq!(updated.state_version, caller.state_version + 1);
        assert!(matches!(
            database.authenticate_login_identifier(
                b"PIXELWIZARD",
                b"test-only identity password",
                &hasher,
            ).unwrap(),
            AuthenticationResult::Valid(found) if found.id == caller.id
        ));
        assert!(matches!(
            database.update_caller_identity(
                other.id,
                other.state_version,
                b"pixelwizard",
                b"Different Handle",
                None,
                &config,
                201,
            ),
            Err(DatabaseError::DuplicateCallerIdentity)
        ));
        assert!(matches!(
            database.update_caller_identity(
                caller.id,
                caller.state_version,
                b"renamed-login",
                b"Renamed Handle",
                None,
                &config,
                202,
            ),
            Err(DatabaseError::CallerStateConflict { .. })
        ));
        let event = database.connection.query_row(
            "SELECT caller_id, prior_state_version, new_state_version, actor_kind FROM caller_identity_events",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?)),
        ).unwrap();
        assert_eq!(event, (caller.id.get(), 0, 1, "local-operator".to_owned()));
        let event_columns: Vec<String> = database
            .connection
            .prepare("SELECT name FROM pragma_table_info('caller_identity_events') ORDER BY cid")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!event_columns.iter().any(|name| {
            matches!(
                name.as_str(),
                "login_identifier" | "display_name" | "real_name"
            )
        }));
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
