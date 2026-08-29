use std::num::NonZeroI64;

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use thiserror::Error;

use crate::{Caller, CallerError, CallerId, CallerState, RuntimeDatabase, SecurityLevel};

pub const MAX_CONFERENCES: u16 = 784;
pub const MAX_MESSAGE_SUBJECT_BYTES: usize = 72;
pub const MAX_MESSAGE_BODY_BYTES: usize = 64 * 1024;
pub const MAX_MESSAGE_LINES: usize = 99;
pub const MAX_MESSAGE_SEARCH_TERMS: usize = 6;
pub const MAX_MESSAGE_SEARCH_TERM_BYTES: usize = 64;
pub const MAX_MESSAGE_SEARCH_RESULTS: usize = 100;
pub const MAX_MESSAGE_SEARCH_CANDIDATES: usize = 10_000;
pub const MAX_MESSAGE_CC_RECIPIENTS: usize = 9;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConferenceId(NonZeroI64);

impl ConferenceId {
    pub fn new(value: i64) -> Result<Self, MessageError> {
        NonZeroI64::new(value)
            .filter(|value| value.get() > 0)
            .map(Self)
            .ok_or(MessageError::InvalidConferenceId(value))
    }

    pub const fn get(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId(NonZeroI64);

impl MessageId {
    pub fn new(value: i64) -> Result<Self, MessageError> {
        NonZeroI64::new(value)
            .filter(|value| value.get() > 0)
            .map(Self)
            .ok_or(MessageError::InvalidMessageId(value))
    }

    pub const fn get(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConferenceAccessMode {
    AtLeast,
    Exact,
}

impl ConferenceAccessMode {
    pub const fn as_database_value(self) -> &'static str {
        match self {
            Self::AtLeast => "at-least",
            Self::Exact => "exact",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, MessageError> {
        match value {
            "at-least" => Ok(Self::AtLeast),
            "exact" => Ok(Self::Exact),
            _ => Err(MessageError::InvalidStoredAccessMode(value.to_owned())),
        }
    }

    fn allows(self, caller: SecurityLevel, required: SecurityLevel) -> bool {
        match self {
            Self::AtLeast => caller.allows(required),
            Self::Exact => caller == required,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conference {
    pub id: ConferenceId,
    pub number: u16,
    pub name: String,
    pub description: String,
    pub access_mode: ConferenceAccessMode,
    pub read_security: SecurityLevel,
    pub post_security: SecurityLevel,
    pub public_only: bool,
    pub caller_deletion_enabled: bool,
    pub maximum_lines: u16,
    pub privileged_security_levels: Vec<SecurityLevel>,
    pub active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConferenceDefinition {
    pub number: u16,
    pub name: String,
    pub description: String,
    pub access_mode: ConferenceAccessMode,
    pub read_security: SecurityLevel,
    pub post_security: SecurityLevel,
    pub public_only: bool,
    pub caller_deletion_enabled: bool,
    pub maximum_lines: u16,
    pub privileged_security_levels: Vec<SecurityLevel>,
}

impl ConferenceDefinition {
    pub fn validate(&self) -> Result<(), MessageError> {
        validate_conference_definition(self)
    }
}

impl Conference {
    fn allows_read(&self, caller: &Caller, sysop_security: SecurityLevel) -> bool {
        caller.security_level.is_sysop(sysop_security)
            || self
                .privileged_security_levels
                .contains(&caller.security_level)
            || self
                .access_mode
                .allows(caller.security_level, self.read_security)
    }

    fn allows_post(&self, caller: &Caller, sysop_security: SecurityLevel) -> bool {
        caller.security_level.is_sysop(sysop_security)
            || self
                .privileged_security_levels
                .contains(&caller.security_level)
            || caller.security_level.allows(self.post_security)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageVisibility {
    Public,
    Private,
}

impl MessageVisibility {
    fn as_database_value(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, MessageError> {
        match value {
            "public" => Ok(Self::Public),
            "private" => Ok(Self::Private),
            _ => Err(MessageError::InvalidStoredVisibility(value.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageKind {
    Standard,
    SysopComment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageDeliveryRole {
    Single,
    Primary,
    CarbonCopy,
}

impl MessageDeliveryRole {
    fn as_database_value(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Primary => "primary",
            Self::CarbonCopy => "cc",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, MessageError> {
        match value {
            "single" => Ok(Self::Single),
            "primary" => Ok(Self::Primary),
            "cc" => Ok(Self::CarbonCopy),
            _ => Err(MessageError::InvalidStoredDeliveryRole(value.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageLifecycle {
    Active,
    Deleted,
}

impl MessageLifecycle {
    fn as_database_value(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, MessageError> {
        match value {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            _ => Err(MessageError::InvalidStoredLifecycle(value.to_owned())),
        }
    }
}

impl MessageKind {
    fn as_database_value(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::SysopComment => "sysop-comment",
        }
    }

    fn from_database_value(value: &str) -> Result<Self, MessageError> {
        match value {
            "standard" => Ok(Self::Standard),
            "sysop-comment" => Ok(Self::SysopComment),
            _ => Err(MessageError::InvalidStoredKind(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub id: MessageId,
    pub conference_id: ConferenceId,
    pub number: u64,
    pub author_caller_id: Option<CallerId>,
    pub author_name: String,
    pub recipient_caller_id: Option<CallerId>,
    pub recipient_name: String,
    pub subject: Vec<u8>,
    pub body: Vec<u8>,
    pub created_at: i64,
    pub parent_message_id: Option<MessageId>,
    pub visibility: MessageVisibility,
    pub kind: MessageKind,
    pub lifecycle: MessageLifecycle,
    pub state_version: u64,
    pub delivery_role: MessageDeliveryRole,
    pub delivery_ordinal: u8,
    pub primary_recipient_name: Option<String>,
    pub received: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageSummary {
    pub id: MessageId,
    pub number: u64,
    pub author_caller_id: Option<CallerId>,
    pub author_name: String,
    pub recipient_name: String,
    pub recipient_caller_id: Option<CallerId>,
    pub subject: Vec<u8>,
    pub created_at: i64,
    pub visibility: MessageVisibility,
    pub kind: MessageKind,
    pub lifecycle: MessageLifecycle,
    pub state_version: u64,
    pub delivery_role: MessageDeliveryRole,
    pub received: bool,
}

impl From<&Message> for MessageSummary {
    fn from(message: &Message) -> Self {
        Self {
            id: message.id,
            number: message.number,
            author_caller_id: message.author_caller_id,
            author_name: message.author_name.clone(),
            recipient_name: message.recipient_name.clone(),
            recipient_caller_id: message.recipient_caller_id,
            subject: message.subject.clone(),
            created_at: message.created_at,
            visibility: message.visibility,
            kind: message.kind,
            lifecycle: message.lifecycle,
            state_version: message.state_version,
            delivery_role: message.delivery_role,
            received: message.received,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMessage {
    pub conference_id: ConferenceId,
    pub recipient_caller_id: Option<CallerId>,
    pub recipient_name: String,
    pub subject: Vec<u8>,
    pub body: Vec<u8>,
    pub created_at: i64,
    pub parent_message_id: Option<MessageId>,
    pub visibility: MessageVisibility,
    pub kind: MessageKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageActor {
    caller_id: CallerId,
    sysop_security: SecurityLevel,
}

impl MessageActor {
    pub const fn new(caller_id: CallerId, sysop_security: SecurityLevel) -> Self {
        Self {
            caller_id,
            sysop_security,
        }
    }

    pub const fn caller_id(self) -> CallerId {
        self.caller_id
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MessageStats {
    pub new_waiting: u64,
    pub already_received: u64,
    pub sent: u64,
    pub total_available: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageRecipient {
    pub caller_id: CallerId,
    pub display_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CopyRecipient {
    Preserve,
    AllCallers,
    Caller(MessageRecipient),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MessageMutationCapabilities {
    pub delete: bool,
    pub undelete: bool,
    pub toggle_visibility: bool,
    pub copy: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MessageMutationStorageStats {
    pub payloads: u64,
    pub fanouts: u64,
    pub deliveries: u64,
    pub recipient_relations: u64,
    pub tombstones: u64,
    pub receipts: u64,
    pub lineage_relations: u64,
    pub audit_events: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageCallerSearchDirection {
    From,
    To,
    Both,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessageDiscoveryQuery {
    SpecificCaller {
        caller_id: CallerId,
        direction: MessageCallerSearchDirection,
    },
    /// Terms are matched as ASCII-case-insensitive substrings in the body.
    /// Every supplied term must occur. Bytes outside ASCII remain exact so
    /// CP437 content is never decoded or normalized.
    Text { terms: Vec<Vec<u8>> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageDiscoveryMatch {
    pub conference_id: ConferenceId,
    pub conference_number: u16,
    pub message_number: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessageDiscoveryResult {
    pub matches: Vec<MessageDiscoveryMatch>,
    pub candidates_examined: usize,
    pub truncated: bool,
}

/// Narrow storage boundary exercised by the stock-core message session.
/// Future SPITFIRE-file and SMB adapters must enforce the same authorization
/// contract instead of exposing storage-specific operations to the session.
pub trait MessageBackend {
    fn recipient(&self, caller_name: &[u8]) -> Result<MessageRecipient, MessageError>;
    fn conferences(&self, actor: MessageActor) -> Result<Vec<Conference>, MessageError>;
    fn conference(
        &self,
        actor: MessageActor,
        conference_number: u16,
    ) -> Result<Conference, MessageError>;
    fn queued_conferences(&self, actor: MessageActor) -> Result<Vec<Conference>, MessageError>;
    fn replace_queue(
        &mut self,
        actor: MessageActor,
        conference_numbers: &[u16],
    ) -> Result<Vec<Conference>, MessageError>;
    fn messages(
        &self,
        actor: MessageActor,
        conference: ConferenceId,
    ) -> Result<Vec<MessageSummary>, MessageError>;
    fn message(
        &self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
    ) -> Result<Message, MessageError>;
    fn discover_messages(
        &self,
        actor: MessageActor,
        conferences: &[ConferenceId],
        query: &MessageDiscoveryQuery,
    ) -> Result<MessageDiscoveryResult, MessageError>;
    fn post(&mut self, actor: MessageActor, message: NewMessage) -> Result<Message, MessageError>;
    fn post_with_cc(
        &mut self,
        actor: MessageActor,
        message: NewMessage,
        cc_recipients: &[MessageRecipient],
    ) -> Result<Vec<Message>, MessageError>;
    fn mutation_capabilities(
        &self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
    ) -> Result<MessageMutationCapabilities, MessageError>;
    fn delete_message(
        &mut self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
        expected_version: u64,
    ) -> Result<Message, MessageError>;
    fn undelete_message(
        &mut self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
        expected_version: u64,
    ) -> Result<Message, MessageError>;
    fn toggle_message_visibility(
        &mut self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
        expected_version: u64,
        address_all_callers: bool,
    ) -> Result<Message, MessageError>;
    #[allow(clippy::too_many_arguments)]
    fn copy_message(
        &mut self,
        actor: MessageActor,
        source_conference: ConferenceId,
        message_number: u64,
        expected_version: u64,
        destination_conference_number: u16,
        recipient: CopyRecipient,
        placed_at: i64,
    ) -> Result<Message, MessageError>;
    fn mark_read(
        &mut self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
    ) -> Result<(), MessageError>;
    fn last_read(&self, actor: MessageActor, conference: ConferenceId)
        -> Result<u64, MessageError>;
    fn received(
        &self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
    ) -> Result<bool, MessageError>;
    fn stats(&self, actor: MessageActor) -> Result<MessageStats, MessageError>;
}

impl RuntimeDatabase {
    pub fn message_mutation_storage_stats(
        &self,
    ) -> Result<MessageMutationStorageStats, MessageError> {
        self.connection
            .query_row(
                r#"
                SELECT
                    (SELECT COUNT(*) FROM message_payloads),
                    (SELECT COUNT(*) FROM message_fanouts),
                    (SELECT COUNT(*) FROM messages),
                    (SELECT COUNT(*) FROM message_delivery_recipients),
                    (SELECT COUNT(*) FROM messages WHERE lifecycle_state = 'deleted'),
                    (SELECT COUNT(*) FROM caller_message_receipts),
                    (SELECT COUNT(*) FROM message_lineage),
                    (SELECT COUNT(*) FROM message_mutation_events)
                "#,
                [],
                |row| {
                    Ok(MessageMutationStorageStats {
                        payloads: sqlite_u64(row.get(0)?).map_err(to_sql_conversion_error)?,
                        fanouts: sqlite_u64(row.get(1)?).map_err(to_sql_conversion_error)?,
                        deliveries: sqlite_u64(row.get(2)?).map_err(to_sql_conversion_error)?,
                        recipient_relations: sqlite_u64(row.get(3)?)
                            .map_err(to_sql_conversion_error)?,
                        tombstones: sqlite_u64(row.get(4)?).map_err(to_sql_conversion_error)?,
                        receipts: sqlite_u64(row.get(5)?).map_err(to_sql_conversion_error)?,
                        lineage_relations: sqlite_u64(row.get(6)?)
                            .map_err(to_sql_conversion_error)?,
                        audit_events: sqlite_u64(row.get(7)?).map_err(to_sql_conversion_error)?,
                    })
                },
            )
            .map_err(MessageError::Sqlite)
    }

    pub fn ensure_conference(
        &mut self,
        definition: &ConferenceDefinition,
    ) -> Result<Conference, MessageError> {
        definition.validate()?;
        let transaction = self
            .connection
            .transaction()
            .map_err(MessageError::Sqlite)?;
        transaction
            .execute(
                r#"
                INSERT INTO message_conferences (
                    conference_number, name, description, access_mode,
                    read_security, post_security, public_only, caller_deletion_enabled,
                    maximum_lines
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(conference_number) DO NOTHING
                "#,
                params![
                    definition.number,
                    definition.name,
                    definition.description,
                    definition.access_mode.as_database_value(),
                    definition.read_security.get(),
                    definition.post_security.get(),
                    definition.public_only,
                    definition.caller_deletion_enabled,
                    definition.maximum_lines
                ],
            )
            .map_err(MessageError::Sqlite)?;
        let conference_id: i64 = transaction
            .query_row(
                "SELECT conference_id FROM message_conferences WHERE conference_number = ?1",
                params![definition.number],
                |row| row.get(0),
            )
            .map_err(MessageError::Sqlite)?;
        if definition.privileged_security_levels.len() > 5 {
            return Err(MessageError::TooManyPrivilegedSecurityLevels);
        }
        for level in &definition.privileged_security_levels {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO conference_privileged_security (conference_id, security_level) VALUES (?1, ?2)",
                    params![conference_id, level.get()],
                )
                .map_err(MessageError::Sqlite)?;
        }
        transaction.commit().map_err(MessageError::Sqlite)?;
        self.load_conference_by_number(definition.number)?
            .ok_or(MessageError::ConferenceNotFound(definition.number))
    }

    pub fn create_conference(
        &mut self,
        definition: &ConferenceDefinition,
    ) -> Result<Conference, MessageError> {
        definition.validate()?;
        if self
            .load_conference_by_number_including_disabled(definition.number)?
            .is_some()
        {
            return Err(MessageError::ConferenceAlreadyExists(definition.number));
        }
        self.ensure_conference(definition)
    }

    pub fn update_conference(
        &mut self,
        conference_number: u16,
        definition: &ConferenceDefinition,
    ) -> Result<Conference, MessageError> {
        if conference_number != definition.number {
            return Err(MessageError::ConferenceRenumberNotSupported);
        }
        definition.validate()?;
        let transaction = self
            .connection
            .transaction()
            .map_err(MessageError::Sqlite)?;
        let changed = transaction
            .execute(
                r#"
                UPDATE message_conferences SET
                    name = ?2, description = ?3, access_mode = ?4,
                    read_security = ?5, post_security = ?6,
                    public_only = ?7, caller_deletion_enabled = ?8, maximum_lines = ?9,
                    updated_at = CURRENT_TIMESTAMP
                WHERE conference_number = ?1
                "#,
                params![
                    definition.number,
                    definition.name,
                    definition.description,
                    definition.access_mode.as_database_value(),
                    definition.read_security.get(),
                    definition.post_security.get(),
                    definition.public_only,
                    definition.caller_deletion_enabled,
                    definition.maximum_lines,
                ],
            )
            .map_err(MessageError::Sqlite)?;
        if changed == 0 {
            return Err(MessageError::ConferenceNotFound(conference_number));
        }
        let conference_id: i64 = transaction
            .query_row(
                "SELECT conference_id FROM message_conferences WHERE conference_number = ?1",
                params![conference_number],
                |row| row.get(0),
            )
            .map_err(MessageError::Sqlite)?;
        transaction
            .execute(
                "DELETE FROM conference_privileged_security WHERE conference_id = ?1",
                params![conference_id],
            )
            .map_err(MessageError::Sqlite)?;
        for level in &definition.privileged_security_levels {
            transaction
                .execute(
                    "INSERT INTO conference_privileged_security (conference_id, security_level) VALUES (?1, ?2)",
                    params![conference_id, level.get()],
                )
                .map_err(MessageError::Sqlite)?;
        }
        transaction.commit().map_err(MessageError::Sqlite)?;
        self.load_conference_by_number_including_disabled(conference_number)?
            .ok_or(MessageError::ConferenceNotFound(conference_number))
    }

    pub fn set_conference_enabled(
        &self,
        conference_number: u16,
        enabled: bool,
    ) -> Result<(), MessageError> {
        if conference_number == 1 && !enabled {
            return Err(MessageError::RequiredConferenceCannotBeDisabled);
        }
        let changed = self
            .connection
            .execute(
                "UPDATE message_conferences SET active = ?2, updated_at = CURRENT_TIMESTAMP WHERE conference_number = ?1",
                params![conference_number, enabled],
            )
            .map_err(MessageError::Sqlite)?;
        if changed == 0 {
            return Err(MessageError::ConferenceNotFound(conference_number));
        }
        Ok(())
    }

    pub fn all_conferences(&self) -> Result<Vec<Conference>, MessageError> {
        let mut statement = self
            .connection
            .prepare(
                r#"
                SELECT conference_id, conference_number, name, description,
                       access_mode, read_security, post_security, public_only,
                       caller_deletion_enabled, maximum_lines, active
                FROM message_conferences ORDER BY conference_number
                "#,
            )
            .map_err(MessageError::Sqlite)?;
        let rows = statement
            .query_map([], conference_from_row)
            .map_err(MessageError::Sqlite)?;
        let mut conferences = Vec::new();
        for row in rows {
            let mut conference = row.map_err(MessageError::Sqlite)?;
            conference.privileged_security_levels = self.privileged_levels(conference.id)?;
            conferences.push(conference);
        }
        Ok(conferences)
    }

    pub fn ensure_system_message(
        &mut self,
        conference_number: u16,
        subject: &[u8],
        body: &[u8],
        created_at: i64,
    ) -> Result<Message, MessageError> {
        validate_message_contents(subject, body, MAX_MESSAGE_LINES as u16)?;
        let conference = self
            .load_conference_by_number(conference_number)?
            .ok_or(MessageError::ConferenceNotFound(conference_number))?;
        if let Some(message) = self.load_message_by_system_subject(conference.id, subject)? {
            return Ok(message);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(MessageError::Sqlite)?;
        let number = next_message_number(&transaction, conference.id)?;
        transaction
            .execute(
                "INSERT INTO message_payloads (subject, body, content_kind) VALUES (?1, ?2, 'standard')",
                params![subject, body],
            )
            .map_err(MessageError::Sqlite)?;
        let payload_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO message_fanouts (payload_id, created_by_caller_id, created_at) VALUES (?1, NULL, ?2)",
                params![payload_id, created_at],
            )
            .map_err(MessageError::Sqlite)?;
        let fanout_id = transaction.last_insert_rowid();
        let id = next_message_id(&transaction)?;
        transaction
            .execute(
                r#"
                INSERT INTO messages (
                    message_id, fanout_id, conference_id, message_number,
                    author_caller_id, author_name, created_at, placed_at,
                    parent_message_id, audience_kind, visibility, lifecycle_state,
                    state_version, delivery_role, delivery_ordinal, primary_delivery_id
                ) VALUES (?1, ?2, ?3, ?4, NULL, 'SPITFIRE NG', ?5, ?5,
                          NULL, 'all-callers', 'public', 'active', 1, 'single', 0, NULL)
                "#,
                params![
                    id.get(),
                    fanout_id,
                    conference.id.get(),
                    sqlite_i64(number)?,
                    created_at
                ],
            )
            .map_err(MessageError::Sqlite)?;
        transaction.commit().map_err(MessageError::Sqlite)?;
        self.load_message_by_id(id)?
            .ok_or(MessageError::MessageNotFound {
                conference: conference_number,
                number,
            })
    }

    fn load_conference_by_number(
        &self,
        conference_number: u16,
    ) -> Result<Option<Conference>, MessageError> {
        let mut conference = self
            .connection
            .query_row(
                r#"
                SELECT conference_id, conference_number, name, description,
                       access_mode, read_security, post_security, public_only,
                       caller_deletion_enabled, maximum_lines, active
                FROM message_conferences WHERE conference_number = ?1 AND active = 1
                "#,
                params![conference_number],
                conference_from_row,
            )
            .optional()
            .map_err(MessageError::Sqlite)?;
        if let Some(value) = conference.as_mut() {
            value.privileged_security_levels = self.privileged_levels(value.id)?;
        }
        Ok(conference)
    }

    fn load_conference_by_number_including_disabled(
        &self,
        conference_number: u16,
    ) -> Result<Option<Conference>, MessageError> {
        let mut conference = self
            .connection
            .query_row(
                r#"
                SELECT conference_id, conference_number, name, description,
                       access_mode, read_security, post_security, public_only,
                       caller_deletion_enabled, maximum_lines, active
                FROM message_conferences WHERE conference_number = ?1
                "#,
                params![conference_number],
                conference_from_row,
            )
            .optional()
            .map_err(MessageError::Sqlite)?;
        if let Some(value) = conference.as_mut() {
            value.privileged_security_levels = self.privileged_levels(value.id)?;
        }
        Ok(conference)
    }

    fn load_conference_by_id(
        &self,
        conference_id: ConferenceId,
    ) -> Result<Option<Conference>, MessageError> {
        let mut conference = self
            .connection
            .query_row(
                r#"
                SELECT conference_id, conference_number, name, description,
                       access_mode, read_security, post_security, public_only,
                       caller_deletion_enabled, maximum_lines, active
                FROM message_conferences WHERE conference_id = ?1 AND active = 1
                "#,
                params![conference_id.get()],
                conference_from_row,
            )
            .optional()
            .map_err(MessageError::Sqlite)?;
        if let Some(value) = conference.as_mut() {
            value.privileged_security_levels = self.privileged_levels(value.id)?;
        }
        Ok(conference)
    }

    fn privileged_levels(
        &self,
        conference_id: ConferenceId,
    ) -> Result<Vec<SecurityLevel>, MessageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT security_level FROM conference_privileged_security WHERE conference_id = ?1 ORDER BY security_level",
            )
            .map_err(MessageError::Sqlite)?;
        let values = statement
            .query_map(params![conference_id.get()], |row| row.get::<_, u16>(0))
            .map_err(MessageError::Sqlite)?;
        values
            .map(|value| {
                SecurityLevel::new(value.map_err(MessageError::Sqlite)?)
                    .map_err(MessageError::InvalidCaller)
            })
            .collect()
    }

    fn active_actor(&self, actor: MessageActor) -> Result<Caller, MessageError> {
        let caller = self
            .caller_by_id(actor.caller_id)
            .map_err(MessageError::Database)?
            .ok_or(MessageError::CallerUnavailable)?;
        if caller.state != CallerState::Active {
            return Err(MessageError::CallerUnavailable);
        }
        Ok(caller)
    }

    fn authorized_conference(
        &self,
        actor: MessageActor,
        conference_id: ConferenceId,
        posting: bool,
    ) -> Result<(Caller, Conference), MessageError> {
        let caller = self.active_actor(actor)?;
        let conference = self
            .load_conference_by_id(conference_id)?
            .ok_or(MessageError::ConferenceIdNotFound(conference_id.get()))?;
        let allowed = if posting {
            conference.allows_post(&caller, actor.sysop_security)
        } else {
            conference.allows_read(&caller, actor.sysop_security)
        };
        if !allowed {
            return Err(MessageError::ConferenceAccessDenied(conference.number));
        }
        Ok((caller, conference))
    }

    fn load_message_by_id(&self, id: MessageId) -> Result<Option<Message>, MessageError> {
        self.connection
            .query_row(MESSAGE_SELECT_BY_ID, params![id.get()], message_from_row)
            .optional()
            .map_err(MessageError::Sqlite)
    }

    fn load_message_by_system_subject(
        &self,
        conference_id: ConferenceId,
        subject: &[u8],
    ) -> Result<Option<Message>, MessageError> {
        self.connection
            .query_row(
                &format!("{MESSAGE_SELECT} WHERE m.conference_id = ?1 AND m.author_caller_id IS NULL AND p.subject = ?2 LIMIT 1"),
                params![conference_id.get(), subject],
                message_from_row,
            )
            .optional()
            .map_err(MessageError::Sqlite)
    }
}

impl MessageBackend for RuntimeDatabase {
    fn recipient(&self, caller_name: &[u8]) -> Result<MessageRecipient, MessageError> {
        let caller = self
            .caller_by_name(caller_name)
            .map_err(MessageError::Database)?
            .ok_or(MessageError::RecipientNotFound)?;
        if caller.state != CallerState::Active {
            return Err(MessageError::RecipientNotFound);
        }
        Ok(MessageRecipient {
            caller_id: caller.id,
            display_name: caller.display_name,
        })
    }
    fn conferences(&self, actor: MessageActor) -> Result<Vec<Conference>, MessageError> {
        let caller = self.active_actor(actor)?;
        let mut statement = self
            .connection
            .prepare(
                r#"
                SELECT conference_id, conference_number, name, description,
                       access_mode, read_security, post_security, public_only,
                       caller_deletion_enabled, maximum_lines, active
                FROM message_conferences WHERE active = 1 ORDER BY conference_number
                "#,
            )
            .map_err(MessageError::Sqlite)?;
        let rows = statement
            .query_map([], conference_from_row)
            .map_err(MessageError::Sqlite)?;
        let mut result = Vec::new();
        for row in rows {
            let mut conference = row.map_err(MessageError::Sqlite)?;
            conference.privileged_security_levels = self.privileged_levels(conference.id)?;
            if conference.allows_read(&caller, actor.sysop_security) {
                result.push(conference);
            }
        }
        Ok(result)
    }

    fn conference(
        &self,
        actor: MessageActor,
        conference_number: u16,
    ) -> Result<Conference, MessageError> {
        let caller = self.active_actor(actor)?;
        let conference = self
            .load_conference_by_number(conference_number)?
            .ok_or(MessageError::ConferenceNotFound(conference_number))?;
        if !conference.allows_read(&caller, actor.sysop_security) {
            return Err(MessageError::ConferenceAccessDenied(conference_number));
        }
        Ok(conference)
    }

    fn queued_conferences(&self, actor: MessageActor) -> Result<Vec<Conference>, MessageError> {
        let accessible = self.conferences(actor)?;
        let mut statement = self
            .connection
            .prepare("SELECT conference_id FROM caller_message_queue WHERE caller_id = ?1")
            .map_err(MessageError::Sqlite)?;
        let rows = statement
            .query_map(params![actor.caller_id.get()], |row| row.get::<_, i64>(0))
            .map_err(MessageError::Sqlite)?;
        let queued = rows
            .collect::<Result<std::collections::HashSet<_>, _>>()
            .map_err(MessageError::Sqlite)?;
        Ok(accessible
            .into_iter()
            .filter(|conference| conference.number == 1 || queued.contains(&conference.id.get()))
            .collect())
    }

    fn replace_queue(
        &mut self,
        actor: MessageActor,
        conference_numbers: &[u16],
    ) -> Result<Vec<Conference>, MessageError> {
        let accessible = self.conferences(actor)?;
        let accessible_by_number = accessible
            .iter()
            .map(|conference| (conference.number, conference.id))
            .collect::<std::collections::HashMap<_, _>>();
        let mut selected = std::collections::BTreeSet::from([1_u16]);
        for number in conference_numbers {
            if !accessible_by_number.contains_key(number) {
                return Err(MessageError::ConferenceNotFound(*number));
            }
            selected.insert(*number);
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(MessageError::Sqlite)?;
        transaction
            .execute(
                "DELETE FROM caller_message_queue WHERE caller_id = ?1",
                params![actor.caller_id.get()],
            )
            .map_err(MessageError::Sqlite)?;
        for number in selected {
            if let Some(id) = accessible_by_number.get(&number) {
                transaction
                    .execute(
                        "INSERT INTO caller_message_queue (caller_id, conference_id) VALUES (?1, ?2)",
                        params![actor.caller_id.get(), id.get()],
                    )
                    .map_err(MessageError::Sqlite)?;
            }
        }
        transaction.commit().map_err(MessageError::Sqlite)?;
        self.queued_conferences(actor)
    }

    fn messages(
        &self,
        actor: MessageActor,
        conference_id: ConferenceId,
    ) -> Result<Vec<MessageSummary>, MessageError> {
        let (caller, _) = self.authorized_conference(actor, conference_id, false)?;
        let lifecycle_filter = if caller.security_level.is_sysop(actor.sysop_security) {
            ""
        } else {
            " AND m.lifecycle_state = 'active'"
        };
        let mut statement = self
            .connection
            .prepare(&format!(
                "{MESSAGE_SELECT} WHERE m.conference_id = ?1{lifecycle_filter} ORDER BY m.message_number"
            ))
            .map_err(MessageError::Sqlite)?;
        let rows = statement
            .query_map(params![conference_id.get()], message_from_row)
            .map_err(MessageError::Sqlite)?;
        let mut result = Vec::new();
        for row in rows {
            let message = row.map_err(MessageError::Sqlite)?;
            if message_visible(&message, &caller, actor.sysop_security) {
                result.push(MessageSummary::from(&message));
            }
        }
        Ok(result)
    }

    fn message(
        &self,
        actor: MessageActor,
        conference_id: ConferenceId,
        message_number: u64,
    ) -> Result<Message, MessageError> {
        let (caller, conference) = self.authorized_conference(actor, conference_id, false)?;
        let message = self
            .connection
            .query_row(
                &format!("{MESSAGE_SELECT} WHERE m.conference_id = ?1 AND m.message_number = ?2"),
                params![conference_id.get(), sqlite_i64(message_number)?],
                message_from_row,
            )
            .optional()
            .map_err(MessageError::Sqlite)?
            .ok_or(MessageError::MessageNotFound {
                conference: conference.number,
                number: message_number,
            })?;
        if message.lifecycle == MessageLifecycle::Deleted
            && !caller.security_level.is_sysop(actor.sysop_security)
        {
            return Err(MessageError::MessageNotFound {
                conference: conference.number,
                number: message_number,
            });
        }
        if !message_visible(&message, &caller, actor.sysop_security) {
            return Err(MessageError::MessageAccessDenied);
        }
        Ok(message)
    }

    fn discover_messages(
        &self,
        actor: MessageActor,
        conference_ids: &[ConferenceId],
        query: &MessageDiscoveryQuery,
    ) -> Result<MessageDiscoveryResult, MessageError> {
        validate_discovery_query(query)?;
        if conference_ids.is_empty() || conference_ids.len() > usize::from(MAX_CONFERENCES) {
            return Err(MessageError::InvalidDiscoveryConferenceCount(
                conference_ids.len(),
            ));
        }

        let mut seen = std::collections::HashSet::new();
        let mut conferences = Vec::new();
        for conference_id in conference_ids {
            if seen.insert(*conference_id) {
                let (caller, conference) =
                    self.authorized_conference(actor, *conference_id, false)?;
                conferences.push((caller, conference));
            }
        }
        conferences.sort_by_key(|(_, conference)| conference.number);

        let mut result = MessageDiscoveryResult::default();
        'conference: for (caller, conference) in conferences {
            let remaining = MAX_MESSAGE_SEARCH_CANDIDATES - result.candidates_examined;
            if remaining == 0 {
                result.truncated = true;
                break;
            }
            let query_limit =
                i64::try_from(remaining + 1).map_err(|_| MessageError::InvalidDiscoveryQuery)?;
            let mut statement = self
                .connection
                .prepare(&format!(
                    "{MESSAGE_SELECT} WHERE m.conference_id = ?1 AND m.lifecycle_state = 'active' ORDER BY m.message_number LIMIT ?2"
                ))
                .map_err(MessageError::Sqlite)?;
            let rows = statement
                .query_map(params![conference.id.get(), query_limit], message_from_row)
                .map_err(MessageError::Sqlite)?;
            for row in rows {
                if result.candidates_examined == MAX_MESSAGE_SEARCH_CANDIDATES {
                    result.truncated = true;
                    break 'conference;
                }
                let message = row.map_err(MessageError::Sqlite)?;
                result.candidates_examined += 1;
                if !message_visible(&message, &caller, actor.sysop_security)
                    || !discovery_query_matches(query, &message, &caller, actor.sysop_security)
                {
                    continue;
                }
                if result.matches.len() == MAX_MESSAGE_SEARCH_RESULTS {
                    result.truncated = true;
                    break 'conference;
                }
                result.matches.push(MessageDiscoveryMatch {
                    conference_id: conference.id,
                    conference_number: conference.number,
                    message_number: message.number,
                });
            }
        }
        Ok(result)
    }

    fn post(&mut self, actor: MessageActor, message: NewMessage) -> Result<Message, MessageError> {
        self.post_with_cc(actor, message, &[])?
            .into_iter()
            .next()
            .ok_or(MessageError::MutationInvariant)
    }

    fn post_with_cc(
        &mut self,
        actor: MessageActor,
        message: NewMessage,
        cc_recipients: &[MessageRecipient],
    ) -> Result<Vec<Message>, MessageError> {
        self.post_message_fanout(actor, message, cc_recipients)
    }

    fn mutation_capabilities(
        &self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
    ) -> Result<MessageMutationCapabilities, MessageError> {
        self.message_mutation_capabilities(actor, conference, message_number)
    }

    fn delete_message(
        &mut self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
        expected_version: u64,
    ) -> Result<Message, MessageError> {
        self.set_message_lifecycle(
            actor,
            conference,
            message_number,
            expected_version,
            MessageLifecycle::Deleted,
        )
    }

    fn undelete_message(
        &mut self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
        expected_version: u64,
    ) -> Result<Message, MessageError> {
        self.set_message_lifecycle(
            actor,
            conference,
            message_number,
            expected_version,
            MessageLifecycle::Active,
        )
    }

    fn toggle_message_visibility(
        &mut self,
        actor: MessageActor,
        conference: ConferenceId,
        message_number: u64,
        expected_version: u64,
        address_all_callers: bool,
    ) -> Result<Message, MessageError> {
        self.toggle_visibility(
            actor,
            conference,
            message_number,
            expected_version,
            address_all_callers,
        )
    }

    fn copy_message(
        &mut self,
        actor: MessageActor,
        source_conference: ConferenceId,
        message_number: u64,
        expected_version: u64,
        destination_conference_number: u16,
        recipient: CopyRecipient,
        placed_at: i64,
    ) -> Result<Message, MessageError> {
        self.copy_delivery(
            actor,
            source_conference,
            message_number,
            expected_version,
            destination_conference_number,
            recipient,
            placed_at,
        )
    }

    fn mark_read(
        &mut self,
        actor: MessageActor,
        conference_id: ConferenceId,
        message_number: u64,
    ) -> Result<(), MessageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(MessageError::Sqlite)?;
        let (_, security) = active_actor_snapshot(&transaction, actor.caller_id)?;
        let conference = conference_policy_snapshot(&transaction, conference_id)?;
        ensure_read_authority(&transaction, &conference, security, actor.sysop_security)?;
        let message =
            load_message_by_number_connection(&transaction, conference_id, message_number)?.ok_or(
                MessageError::MessageNotFound {
                    conference: conference.number,
                    number: message_number,
                },
            )?;
        if message.lifecycle == MessageLifecycle::Deleted {
            return Ok(());
        }
        if message.visibility == MessageVisibility::Private
            && !security.is_sysop(actor.sysop_security)
            && message.author_caller_id != Some(actor.caller_id)
            && message.recipient_caller_id != Some(actor.caller_id)
        {
            return Err(MessageError::MessageAccessDenied);
        }
        transaction
            .execute(
                r#"
                INSERT INTO caller_last_read (caller_id, conference_id, last_message_number)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(caller_id, conference_id) DO UPDATE SET
                    last_message_number = max(last_message_number, excluded.last_message_number),
                    updated_at = CURRENT_TIMESTAMP
                "#,
                params![
                    actor.caller_id.get(),
                    conference_id.get(),
                    sqlite_i64(message_number)?
                ],
            )
            .map_err(MessageError::Sqlite)?;
        if message.recipient_caller_id == Some(actor.caller_id) {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO caller_message_receipts (caller_id, message_id) VALUES (?1, ?2)",
                    params![actor.caller_id.get(), message.id.get()],
                )
                .map_err(MessageError::Sqlite)?;
        }
        transaction.commit().map_err(MessageError::Sqlite)?;
        Ok(())
    }

    fn last_read(
        &self,
        actor: MessageActor,
        conference_id: ConferenceId,
    ) -> Result<u64, MessageError> {
        self.authorized_conference(actor, conference_id, false)?;
        let value: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE((SELECT last_message_number FROM caller_last_read WHERE caller_id = ?1 AND conference_id = ?2), 0)",
                params![actor.caller_id.get(), conference_id.get()],
                |row| row.get(0),
            )
            .map_err(MessageError::Sqlite)?;
        sqlite_u64(value)
    }

    fn received(
        &self,
        actor: MessageActor,
        conference_id: ConferenceId,
        message_number: u64,
    ) -> Result<bool, MessageError> {
        let message = self.message(actor, conference_id, message_number)?;
        if message.recipient_caller_id != Some(actor.caller_id) {
            return Ok(false);
        }
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM caller_message_receipts WHERE caller_id = ?1 AND message_id = ?2)",
                params![actor.caller_id.get(), message.id.get()],
                |row| row.get(0),
            )
            .map_err(MessageError::Sqlite)
    }

    fn stats(&self, actor: MessageActor) -> Result<MessageStats, MessageError> {
        self.active_actor(actor)?;
        let mut stats = MessageStats::default();
        for conference in self.conferences(actor)? {
            for message in self.messages(actor, conference.id)? {
                if message.lifecycle == MessageLifecycle::Deleted {
                    continue;
                }
                stats.total_available += 1;
                if message.author_caller_id == Some(actor.caller_id) {
                    stats.sent += 1;
                }
                if message.recipient_caller_id == Some(actor.caller_id) {
                    if self.received(actor, conference.id, message.number)? {
                        stats.already_received += 1;
                    } else {
                        stats.new_waiting += 1;
                    }
                }
            }
        }
        Ok(stats)
    }
}

impl RuntimeDatabase {
    pub fn set_conference_caller_deletion(
        &mut self,
        conference_number: u16,
        enabled: bool,
    ) -> Result<(), MessageError> {
        let changed = self
            .connection
            .execute(
                "UPDATE message_conferences SET caller_deletion_enabled = ?2, updated_at = CURRENT_TIMESTAMP WHERE conference_number = ?1",
                params![conference_number, enabled],
            )
            .map_err(MessageError::Sqlite)?;
        if changed == 0 {
            return Err(MessageError::ConferenceNotFound(conference_number));
        }
        Ok(())
    }

    fn post_message_fanout(
        &mut self,
        actor: MessageActor,
        message: NewMessage,
        cc_recipients: &[MessageRecipient],
    ) -> Result<Vec<Message>, MessageError> {
        let (caller, conference) =
            self.authorized_conference(actor, message.conference_id, true)?;
        validate_message_contents(&message.subject, &message.body, conference.maximum_lines)?;
        if cc_recipients.len() > MAX_MESSAGE_CC_RECIPIENTS {
            return Err(MessageError::TooManyCarbonCopies);
        }
        if conference.public_only && message.visibility == MessageVisibility::Private {
            return Err(MessageError::PrivateMessagesNotAllowed(conference.number));
        }
        if message.visibility == MessageVisibility::Private && message.recipient_caller_id.is_none()
        {
            return Err(MessageError::PrivateMessageNeedsRecipient);
        }
        if !cc_recipients.is_empty() && message.recipient_caller_id.is_none() {
            return Err(MessageError::CarbonCopyNeedsPrimary);
        }
        let mut recipients = Vec::new();
        if let Some(id) = message.recipient_caller_id {
            recipients.push(MessageRecipient {
                caller_id: id,
                display_name: message.recipient_name.clone(),
            });
        } else if message.recipient_name != "All Callers" {
            return Err(MessageError::RecipientNotFound);
        }
        recipients.extend_from_slice(cc_recipients);

        let mut unique = std::collections::HashSet::new();
        for recipient in &recipients {
            if recipient.caller_id == caller.id {
                return Err(MessageError::SelfRecipient);
            }
            if !unique.insert(recipient.caller_id) {
                return Err(MessageError::DuplicateRecipient);
            }
            validate_recipient_connection(
                &self.connection,
                recipient,
                conference.id,
                conference.number,
            )?;
        }
        if let Some(parent) = message.parent_message_id {
            let stored = self
                .load_message_by_id(parent)?
                .ok_or(MessageError::ParentMessageNotFound(parent.get()))?;
            if stored.conference_id != message.conference_id
                || stored.lifecycle != MessageLifecycle::Active
                || !message_visible(&stored, &caller, actor.sysop_security)
            {
                return Err(MessageError::ParentMessageNotFound(parent.get()));
            }
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(MessageError::Sqlite)?;
        let (actor_name, actor_security) = active_actor_snapshot(&transaction, actor.caller_id)?;
        ensure_post_authority(
            &transaction,
            message.conference_id,
            actor_security,
            actor.sysop_security,
        )?;
        for recipient in &recipients {
            validate_recipient_connection(
                &transaction,
                recipient,
                conference.id,
                conference.number,
            )?;
        }

        transaction
            .execute(
                "INSERT INTO message_payloads (subject, body, content_kind) VALUES (?1, ?2, ?3)",
                params![
                    message.subject,
                    message.body,
                    message.kind.as_database_value()
                ],
            )
            .map_err(MessageError::Sqlite)?;
        let payload_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO message_fanouts (payload_id, created_by_caller_id, created_at) VALUES (?1, ?2, ?3)",
                params![payload_id, actor.caller_id.get(), message.created_at],
            )
            .map_err(MessageError::Sqlite)?;
        let fanout_id = transaction.last_insert_rowid();
        let first_number = next_message_number(&transaction, conference.id)?;
        let first_id = next_message_id(&transaction)?;
        let delivery_count = recipients.len().max(1);
        let mut ids = Vec::with_capacity(delivery_count);

        for ordinal in 0..delivery_count {
            let id = MessageId::new(
                first_id.get()
                    + i64::try_from(ordinal).map_err(|_| MessageError::MessageNumberOverflow)?,
            )?;
            let number = first_number
                .checked_add(
                    u64::try_from(ordinal).map_err(|_| MessageError::MessageNumberOverflow)?,
                )
                .ok_or(MessageError::MessageNumberOverflow)?;
            let (audience, role, primary_id) = if recipients.is_empty() {
                ("all-callers", MessageDeliveryRole::Single, None)
            } else if ordinal == 0 {
                (
                    "local-recipient",
                    MessageDeliveryRole::Primary,
                    Some(first_id),
                )
            } else {
                (
                    "local-recipient",
                    MessageDeliveryRole::CarbonCopy,
                    Some(first_id),
                )
            };
            transaction
                .execute(
                    r#"
                    INSERT INTO messages (
                        message_id, fanout_id, conference_id, message_number,
                        author_caller_id, author_name, created_at, placed_at,
                        parent_message_id, audience_kind, visibility, lifecycle_state,
                        state_version, delivery_role, delivery_ordinal, primary_delivery_id
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9, ?10,
                              'active', 1, ?11, ?12, ?13)
                    "#,
                    params![
                        id.get(),
                        fanout_id,
                        conference.id.get(),
                        sqlite_i64(number)?,
                        caller.id.get(),
                        caller.display_name,
                        message.created_at,
                        message.parent_message_id.map(MessageId::get),
                        audience,
                        message.visibility.as_database_value(),
                        role.as_database_value(),
                        i64::try_from(ordinal).map_err(|_| MessageError::MessageNumberOverflow)?,
                        primary_id.map(MessageId::get),
                    ],
                )
                .map_err(MessageError::Sqlite)?;
            if let Some(recipient) = recipients.get(ordinal) {
                transaction
                    .execute(
                        "INSERT INTO message_delivery_recipients (message_id, fanout_id, caller_id, display_name_snapshot, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![id.get(), fanout_id, recipient.caller_id.get(), recipient.display_name, message.created_at],
                    )
                    .map_err(MessageError::Sqlite)?;
            }
            ids.push(id);
        }
        if cc_recipients.is_empty() {
            // A normal post is not a mutation event. The immutable fan-out is
            // still the authoritative posting identity.
        } else {
            transaction
                .execute(
                    r#"
                    INSERT INTO message_mutation_events (
                        occurred_at, operation, actor_caller_id, actor_name_snapshot,
                        message_id, new_state_version, destination_conference_id,
                        destination_message_number, recipient_count
                    ) VALUES (?1, 'cc-created', ?2, ?3, ?4, 1, ?5, ?6, ?7)
                    "#,
                    params![
                        message.created_at,
                        actor.caller_id.get(),
                        actor_name,
                        first_id.get(),
                        conference.id.get(),
                        sqlite_i64(first_number)?,
                        i64::try_from(delivery_count)
                            .map_err(|_| MessageError::MessageNumberOverflow)?
                    ],
                )
                .map_err(MessageError::Sqlite)?;
        }
        transaction
            .execute(
                "UPDATE callers SET messages_posted = messages_posted + ?2, updated_at = CURRENT_TIMESTAMP WHERE caller_id = ?1 AND account_state = 'active'",
                params![caller.id.get(), i64::try_from(delivery_count).map_err(|_| MessageError::MessageNumberOverflow)?],
            )
            .map_err(MessageError::Sqlite)?;
        validate_fanout(&transaction, fanout_id)?;
        transaction.commit().map_err(MessageError::Sqlite)?;

        ids.into_iter()
            .map(|id| {
                self.load_message_by_id(id)?
                    .ok_or(MessageError::MutationInvariant)
            })
            .collect()
    }

    fn message_mutation_capabilities(
        &self,
        actor: MessageActor,
        conference_id: ConferenceId,
        message_number: u64,
    ) -> Result<MessageMutationCapabilities, MessageError> {
        let (caller, conference) = self.authorized_conference(actor, conference_id, false)?;
        let message =
            load_message_by_number_connection(&self.connection, conference_id, message_number)?
                .ok_or(MessageError::MessageNotFound {
                    conference: conference.number,
                    number: message_number,
                })?;
        let threshold = caller.security_level.is_sysop(actor.sysop_security);
        if message.lifecycle == MessageLifecycle::Deleted && !threshold {
            return Err(MessageError::MessageNotFound {
                conference: conference.number,
                number: message_number,
            });
        }
        if !message_visible(&message, &caller, actor.sysop_security) {
            return Err(MessageError::MessageAccessDenied);
        }
        let ordinary_delete = conference.caller_deletion_enabled
            && (message.author_caller_id == Some(caller.id)
                || message.recipient_caller_id == Some(caller.id));
        Ok(MessageMutationCapabilities {
            delete: message.lifecycle == MessageLifecycle::Active && (threshold || ordinary_delete),
            undelete: message.lifecycle == MessageLifecycle::Deleted && threshold,
            toggle_visibility: message.lifecycle == MessageLifecycle::Active && threshold,
            copy: message.lifecycle == MessageLifecycle::Active && threshold,
        })
    }

    fn set_message_lifecycle(
        &mut self,
        actor: MessageActor,
        conference_id: ConferenceId,
        message_number: u64,
        expected_version: u64,
        target: MessageLifecycle,
    ) -> Result<Message, MessageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(MessageError::Sqlite)?;
        let (actor_name, security) = active_actor_snapshot(&transaction, actor.caller_id)?;
        let conference = conference_policy_snapshot(&transaction, conference_id)?;
        ensure_read_authority(&transaction, &conference, security, actor.sysop_security)?;
        let message =
            load_message_by_number_connection(&transaction, conference_id, message_number)?.ok_or(
                MessageError::MessageNotFound {
                    conference: conference.number,
                    number: message_number,
                },
            )?;
        let threshold = security.is_sysop(actor.sysop_security);
        if message.state_version != expected_version {
            return Err(MessageError::MutationConflict);
        }
        if message.lifecycle == target {
            return Err(if target == MessageLifecycle::Deleted {
                MessageError::AlreadyDeleted
            } else {
                MessageError::AlreadyActive
            });
        }
        if target == MessageLifecycle::Active && !threshold {
            return Err(MessageError::MutationDenied);
        }
        if target == MessageLifecycle::Deleted
            && !threshold
            && !(conference.caller_deletion_enabled
                && (message.author_caller_id == Some(actor.caller_id)
                    || message.recipient_caller_id == Some(actor.caller_id)))
        {
            return Err(MessageError::MutationDenied);
        }
        let new_version = expected_version
            .checked_add(1)
            .ok_or(MessageError::StateVersionOverflow)?;
        let changed = transaction
            .execute(
                "UPDATE messages SET lifecycle_state = ?2, state_version = ?3 WHERE message_id = ?1 AND state_version = ?4 AND lifecycle_state = ?5",
                params![message.id.get(), target.as_database_value(), sqlite_i64(new_version)?, sqlite_i64(expected_version)?, message.lifecycle.as_database_value()],
            )
            .map_err(MessageError::Sqlite)?;
        if changed != 1 {
            return Err(MessageError::MutationConflict);
        }
        transaction
            .execute(
                r#"
                INSERT INTO message_mutation_events (
                    occurred_at, operation, actor_caller_id, actor_name_snapshot,
                    message_id, prior_state_version, new_state_version,
                    source_conference_id, source_message_number,
                    prior_lifecycle, new_lifecycle
                ) VALUES (unixepoch(), ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
                params![
                    if target == MessageLifecycle::Deleted {
                        "deleted"
                    } else {
                        "undeleted"
                    },
                    actor.caller_id.get(),
                    actor_name,
                    message.id.get(),
                    sqlite_i64(expected_version)?,
                    sqlite_i64(new_version)?,
                    conference_id.get(),
                    sqlite_i64(message_number)?,
                    message.lifecycle.as_database_value(),
                    target.as_database_value(),
                ],
            )
            .map_err(MessageError::Sqlite)?;
        let id = message.id;
        transaction.commit().map_err(MessageError::Sqlite)?;
        self.load_message_by_id(id)?
            .ok_or(MessageError::MutationInvariant)
    }

    fn toggle_visibility(
        &mut self,
        actor: MessageActor,
        conference_id: ConferenceId,
        message_number: u64,
        expected_version: u64,
        address_all_callers: bool,
    ) -> Result<Message, MessageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(MessageError::Sqlite)?;
        let (actor_name, security) = active_actor_snapshot(&transaction, actor.caller_id)?;
        let conference = conference_policy_snapshot(&transaction, conference_id)?;
        ensure_read_authority(&transaction, &conference, security, actor.sysop_security)?;
        if !security.is_sysop(actor.sysop_security) {
            return Err(MessageError::MutationDenied);
        }
        let message =
            load_message_by_number_connection(&transaction, conference_id, message_number)?.ok_or(
                MessageError::MessageNotFound {
                    conference: conference.number,
                    number: message_number,
                },
            )?;
        if message.lifecycle != MessageLifecycle::Active {
            return Err(MessageError::MutationDenied);
        }
        if message.state_version != expected_version {
            return Err(MessageError::MutationConflict);
        }
        let (new_visibility, new_audience) = match message.visibility {
            MessageVisibility::Public => {
                if message.recipient_caller_id.is_none() {
                    return Err(MessageError::PrivateMessageNeedsRecipient);
                }
                (MessageVisibility::Private, "local-recipient")
            }
            MessageVisibility::Private if address_all_callers => {
                transaction
                    .execute(
                        "DELETE FROM message_delivery_recipients WHERE message_id = ?1",
                        params![message.id.get()],
                    )
                    .map_err(MessageError::Sqlite)?;
                (MessageVisibility::Public, "all-callers")
            }
            MessageVisibility::Private => (MessageVisibility::Public, "local-recipient"),
        };
        let prior_audience = if message.recipient_caller_id.is_some() {
            "local-recipient"
        } else {
            "all-callers"
        };
        let new_version = expected_version
            .checked_add(1)
            .ok_or(MessageError::StateVersionOverflow)?;
        let changed = transaction
            .execute(
                "UPDATE messages SET visibility = ?2, audience_kind = ?3, state_version = ?4 WHERE message_id = ?1 AND state_version = ?5 AND lifecycle_state = 'active'",
                params![message.id.get(), new_visibility.as_database_value(), new_audience, sqlite_i64(new_version)?, sqlite_i64(expected_version)?],
            )
            .map_err(MessageError::Sqlite)?;
        if changed != 1 {
            return Err(MessageError::MutationConflict);
        }
        transaction
            .execute(
                r#"
                INSERT INTO message_mutation_events (
                    occurred_at, operation, actor_caller_id, actor_name_snapshot,
                    message_id, prior_state_version, new_state_version,
                    source_conference_id, source_message_number,
                    prior_visibility, new_visibility, prior_audience, new_audience
                ) VALUES (unixepoch(), 'visibility-changed', ?1, ?2, ?3, ?4, ?5,
                          ?6, ?7, ?8, ?9, ?10, ?11)
                "#,
                params![
                    actor.caller_id.get(),
                    actor_name,
                    message.id.get(),
                    sqlite_i64(expected_version)?,
                    sqlite_i64(new_version)?,
                    conference_id.get(),
                    sqlite_i64(message_number)?,
                    message.visibility.as_database_value(),
                    new_visibility.as_database_value(),
                    prior_audience,
                    new_audience
                ],
            )
            .map_err(MessageError::Sqlite)?;
        let id = message.id;
        transaction.commit().map_err(MessageError::Sqlite)?;
        self.load_message_by_id(id)?
            .ok_or(MessageError::MutationInvariant)
    }

    #[allow(clippy::too_many_arguments)]
    fn copy_delivery(
        &mut self,
        actor: MessageActor,
        source_conference_id: ConferenceId,
        message_number: u64,
        expected_version: u64,
        destination_conference_number: u16,
        recipient: CopyRecipient,
        placed_at: i64,
    ) -> Result<Message, MessageError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(MessageError::Sqlite)?;
        let (actor_name, security) = active_actor_snapshot(&transaction, actor.caller_id)?;
        if !security.is_sysop(actor.sysop_security) {
            return Err(MessageError::MutationDenied);
        }
        let source_conference = conference_policy_snapshot(&transaction, source_conference_id)?;
        ensure_read_authority(
            &transaction,
            &source_conference,
            security,
            actor.sysop_security,
        )?;
        let destination =
            conference_snapshot_by_number(&transaction, destination_conference_number)?;
        ensure_post_authority(&transaction, destination.id, security, actor.sysop_security)?;
        let source =
            load_message_by_number_connection(&transaction, source_conference_id, message_number)?
                .ok_or(MessageError::MessageNotFound {
                    conference: source_conference.number,
                    number: message_number,
                })?;
        if source.lifecycle != MessageLifecycle::Active {
            return Err(MessageError::MutationDenied);
        }
        if source.state_version != expected_version {
            return Err(MessageError::MutationConflict);
        }
        let (selected, forwarded) = match recipient {
            CopyRecipient::Preserve => (
                source
                    .recipient_caller_id
                    .map(|caller_id| MessageRecipient {
                        caller_id,
                        display_name: source.recipient_name.clone(),
                    }),
                false,
            ),
            CopyRecipient::AllCallers => (None, true),
            CopyRecipient::Caller(value) => (Some(value), true),
        };
        if let Some(value) = selected.as_ref() {
            if actor.caller_id == value.caller_id {
                return Err(MessageError::SelfRecipient);
            }
            validate_recipient_connection(&transaction, value, destination.id, destination.number)?;
        } else if source.visibility == MessageVisibility::Private {
            return Err(MessageError::PrivateMessageNeedsRecipient);
        }
        if destination.public_only && source.visibility == MessageVisibility::Private {
            return Err(MessageError::PrivateMessagesNotAllowed(destination.number));
        }
        let payload_id: i64 = transaction
            .query_row(
                "SELECT f.payload_id FROM messages AS m JOIN message_fanouts AS f ON f.fanout_id = m.fanout_id WHERE m.message_id = ?1",
                params![source.id.get()],
                |row| row.get(0),
            )
            .map_err(MessageError::Sqlite)?;
        transaction
            .execute(
                "INSERT INTO message_fanouts (payload_id, created_by_caller_id, created_at) VALUES (?1, ?2, ?3)",
                params![payload_id, actor.caller_id.get(), placed_at],
            )
            .map_err(MessageError::Sqlite)?;
        let fanout_id = transaction.last_insert_rowid();
        let id = next_message_id(&transaction)?;
        let number = next_message_number(&transaction, destination.id)?;
        let (audience, role, primary_id) = if selected.is_some() {
            ("local-recipient", MessageDeliveryRole::Primary, Some(id))
        } else {
            ("all-callers", MessageDeliveryRole::Single, None)
        };
        transaction
            .execute(
                r#"
                INSERT INTO messages (
                    message_id, fanout_id, conference_id, message_number,
                    author_caller_id, author_name, created_at, placed_at,
                    parent_message_id, audience_kind, visibility, lifecycle_state,
                    state_version, delivery_role, delivery_ordinal, primary_delivery_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                          'active', 1, ?12, 0, ?13)
                "#,
                params![
                    id.get(),
                    fanout_id,
                    destination.id.get(),
                    sqlite_i64(number)?,
                    source.author_caller_id.map(CallerId::get),
                    source.author_name,
                    source.created_at,
                    placed_at,
                    source.parent_message_id.map(MessageId::get),
                    audience,
                    source.visibility.as_database_value(),
                    role.as_database_value(),
                    primary_id.map(MessageId::get)
                ],
            )
            .map_err(MessageError::Sqlite)?;
        if let Some(value) = selected.as_ref() {
            transaction
                .execute(
                    "INSERT INTO message_delivery_recipients (message_id, fanout_id, caller_id, display_name_snapshot, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id.get(), fanout_id, value.caller_id.get(), value.display_name, placed_at],
                )
                .map_err(MessageError::Sqlite)?;
        }
        transaction
            .execute(
                r#"
                INSERT INTO message_mutation_events (
                    occurred_at, operation, actor_caller_id, actor_name_snapshot,
                    message_id, derived_message_id, prior_state_version,
                    new_state_version, source_conference_id, source_message_number,
                    destination_conference_id, destination_message_number, recipient_count
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?10, ?11, ?12)
                "#,
                params![
                    placed_at,
                    if forwarded { "forwarded" } else { "copied" },
                    actor.caller_id.get(),
                    actor_name,
                    source.id.get(),
                    id.get(),
                    sqlite_i64(expected_version)?,
                    source_conference_id.get(),
                    sqlite_i64(message_number)?,
                    destination.id.get(),
                    sqlite_i64(number)?,
                    if selected.is_some() { 1 } else { 0 }
                ],
            )
            .map_err(MessageError::Sqlite)?;
        let event_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO message_lineage (derived_message_id, source_message_id, relation, mutation_event_id) VALUES (?1, ?2, ?3, ?4)",
                params![id.get(), source.id.get(), if forwarded { "forward" } else { "copy" }, event_id],
            )
            .map_err(MessageError::Sqlite)?;
        validate_fanout(&transaction, fanout_id)?;
        transaction.commit().map_err(MessageError::Sqlite)?;
        self.load_message_by_id(id)?
            .ok_or(MessageError::MutationInvariant)
    }
}

const MESSAGE_SELECT: &str = r#"
    SELECT m.message_id, m.conference_id, m.message_number, m.author_caller_id,
           m.author_name, r.caller_id, COALESCE(r.display_name_snapshot, 'All Callers'),
           p.subject, p.body, m.created_at, m.parent_message_id, m.visibility,
           p.content_kind, m.lifecycle_state, m.state_version, m.delivery_role,
           m.delivery_ordinal, pr.display_name_snapshot,
           EXISTS(SELECT 1 FROM caller_message_receipts AS rr WHERE rr.message_id = m.message_id)
      FROM messages AS m
      JOIN message_fanouts AS f ON f.fanout_id = m.fanout_id
      JOIN message_payloads AS p ON p.payload_id = f.payload_id
      LEFT JOIN message_delivery_recipients AS r ON r.message_id = m.message_id
      LEFT JOIN message_delivery_recipients AS pr ON pr.message_id = m.primary_delivery_id
"#;
const MESSAGE_SELECT_BY_ID: &str = r#"
    SELECT m.message_id, m.conference_id, m.message_number, m.author_caller_id,
           m.author_name, r.caller_id, COALESCE(r.display_name_snapshot, 'All Callers'),
           p.subject, p.body, m.created_at, m.parent_message_id, m.visibility,
           p.content_kind, m.lifecycle_state, m.state_version, m.delivery_role,
           m.delivery_ordinal, pr.display_name_snapshot,
           EXISTS(SELECT 1 FROM caller_message_receipts AS rr WHERE rr.message_id = m.message_id)
      FROM messages AS m
      JOIN message_fanouts AS f ON f.fanout_id = m.fanout_id
      JOIN message_payloads AS p ON p.payload_id = f.payload_id
      LEFT JOIN message_delivery_recipients AS r ON r.message_id = m.message_id
      LEFT JOIN message_delivery_recipients AS pr ON pr.message_id = m.primary_delivery_id
     WHERE m.message_id = ?1
"#;

fn conference_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conference> {
    let id = row.get::<_, i64>(0)?;
    let mode = row.get::<_, String>(4)?;
    let read_security = row.get::<_, u16>(5)?;
    let post_security = row.get::<_, u16>(6)?;
    Ok(Conference {
        id: ConferenceId::new(id).map_err(to_sql_conversion_error)?,
        number: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        access_mode: ConferenceAccessMode::from_database_value(&mode)
            .map_err(to_sql_conversion_error)?,
        read_security: SecurityLevel::new(read_security).map_err(to_sql_conversion_error)?,
        post_security: SecurityLevel::new(post_security).map_err(to_sql_conversion_error)?,
        public_only: row.get(7)?,
        caller_deletion_enabled: row.get(8)?,
        maximum_lines: row.get(9)?,
        privileged_security_levels: Vec::new(),
        active: row.get(10)?,
    })
}

fn message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let author_id = row.get::<_, Option<i64>>(3)?;
    let recipient_id = row.get::<_, Option<i64>>(5)?;
    let parent_id = row.get::<_, Option<i64>>(10)?;
    let visibility = row.get::<_, String>(11)?;
    let kind = row.get::<_, String>(12)?;
    let lifecycle = row.get::<_, String>(13)?;
    let delivery_role = row.get::<_, String>(15)?;
    let subject = row.get::<_, Vec<u8>>(7)?;
    let body = row.get::<_, Vec<u8>>(8)?;
    validate_message_contents(&subject, &body, MAX_MESSAGE_LINES as u16)
        .map_err(to_sql_conversion_error)?;
    Ok(Message {
        id: MessageId::new(row.get(0)?).map_err(to_sql_conversion_error)?,
        conference_id: ConferenceId::new(row.get(1)?).map_err(to_sql_conversion_error)?,
        number: sqlite_u64(row.get(2)?).map_err(to_sql_conversion_error)?,
        author_caller_id: author_id
            .map(CallerId::new)
            .transpose()
            .map_err(to_sql_conversion_error)?,
        author_name: row.get(4)?,
        recipient_caller_id: recipient_id
            .map(CallerId::new)
            .transpose()
            .map_err(to_sql_conversion_error)?,
        recipient_name: row.get(6)?,
        subject,
        body,
        created_at: row.get(9)?,
        parent_message_id: parent_id
            .map(MessageId::new)
            .transpose()
            .map_err(to_sql_conversion_error)?,
        visibility: MessageVisibility::from_database_value(&visibility)
            .map_err(to_sql_conversion_error)?,
        kind: MessageKind::from_database_value(&kind).map_err(to_sql_conversion_error)?,
        lifecycle: MessageLifecycle::from_database_value(&lifecycle)
            .map_err(to_sql_conversion_error)?,
        state_version: sqlite_u64(row.get(14)?).map_err(to_sql_conversion_error)?,
        delivery_role: MessageDeliveryRole::from_database_value(&delivery_role)
            .map_err(to_sql_conversion_error)?,
        delivery_ordinal: row.get(16)?,
        primary_recipient_name: row.get(17)?,
        received: row.get(18)?,
    })
}

fn message_visible(message: &Message, caller: &Caller, sysop_security: SecurityLevel) -> bool {
    message.visibility == MessageVisibility::Public
        || caller.security_level.is_sysop(sysop_security)
        || message.author_caller_id == Some(caller.id)
        || message.recipient_caller_id == Some(caller.id)
}

fn validate_discovery_query(query: &MessageDiscoveryQuery) -> Result<(), MessageError> {
    let MessageDiscoveryQuery::Text { terms } = query else {
        return Ok(());
    };
    if terms.is_empty()
        || terms.len() > MAX_MESSAGE_SEARCH_TERMS
        || terms.iter().any(|term| {
            term.is_empty()
                || term.len() > MAX_MESSAGE_SEARCH_TERM_BYTES
                || term
                    .iter()
                    .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        })
    {
        return Err(MessageError::InvalidDiscoveryQuery);
    }
    Ok(())
}

fn discovery_query_matches(
    query: &MessageDiscoveryQuery,
    message: &Message,
    caller: &Caller,
    sysop_security: SecurityLevel,
) -> bool {
    match query {
        MessageDiscoveryQuery::SpecificCaller {
            caller_id,
            direction,
        } => {
            // The stock Specific Caller command limits ordinary callers to
            // public messages. Threshold Sysops still pass the common message
            // visibility authority; deleted rows remain unavailable under the
            // current message-domain policy.
            if !caller.security_level.is_sysop(sysop_security)
                && message.visibility != MessageVisibility::Public
            {
                return false;
            }
            let from = message.author_caller_id == Some(*caller_id);
            let to = message.recipient_caller_id == Some(*caller_id);
            match direction {
                MessageCallerSearchDirection::From => from,
                MessageCallerSearchDirection::To => to,
                MessageCallerSearchDirection::Both => from || to,
            }
        }
        MessageDiscoveryQuery::Text { terms } => terms
            .iter()
            .all(|term| contains_bytes_ascii_case_insensitive(&message.body, term)),
    }
}

fn contains_bytes_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn validate_conference_definition(definition: &ConferenceDefinition) -> Result<(), MessageError> {
    if definition.number == 0 || definition.number > MAX_CONFERENCES {
        return Err(MessageError::InvalidConferenceNumber(definition.number));
    }
    if definition.name.trim().is_empty() || definition.name.len() > 60 {
        return Err(MessageError::InvalidConferenceName);
    }
    if definition.description.len() > 255 {
        return Err(MessageError::InvalidConferenceDescription);
    }
    if !(25..=99).contains(&definition.maximum_lines) {
        return Err(MessageError::InvalidMaximumLines(definition.maximum_lines));
    }
    if definition.privileged_security_levels.len() > 5 {
        return Err(MessageError::TooManyPrivilegedSecurityLevels);
    }
    let mut levels = std::collections::HashSet::new();
    if definition
        .privileged_security_levels
        .iter()
        .any(|level| !levels.insert(*level))
    {
        return Err(MessageError::DuplicatePrivilegedSecurityLevel);
    }
    Ok(())
}

fn validate_message_contents(
    subject: &[u8],
    body: &[u8],
    maximum_lines: u16,
) -> Result<(), MessageError> {
    if subject.is_empty()
        || subject.len() > MAX_MESSAGE_SUBJECT_BYTES
        || subject.iter().any(|byte| *byte < b' ' || *byte == 0x7f)
    {
        return Err(MessageError::InvalidSubject);
    }
    if body.is_empty()
        || body.len() > MAX_MESSAGE_BODY_BYTES
        || body
            .iter()
            .any(|byte| *byte < b' ' && !matches!(*byte, b'\r' | b'\n' | b'\t'))
    {
        return Err(MessageError::InvalidBody);
    }
    let lines =
        body.iter().filter(|byte| **byte == b'\n').count() + usize::from(!body.ends_with(b"\n"));
    if lines > usize::from(maximum_lines) || lines > MAX_MESSAGE_LINES {
        return Err(MessageError::TooManyLines {
            actual: lines,
            maximum: usize::from(maximum_lines),
        });
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ConferencePolicySnapshot {
    id: ConferenceId,
    number: u16,
    access_mode: ConferenceAccessMode,
    read_security: SecurityLevel,
    post_security: SecurityLevel,
    public_only: bool,
    caller_deletion_enabled: bool,
}

fn active_actor_snapshot(
    connection: &rusqlite::Connection,
    caller_id: CallerId,
) -> Result<(String, SecurityLevel), MessageError> {
    let value = connection
        .query_row(
            "SELECT c.display_name, MIN(c.security_level, COALESCE((SELECT MIN(target_security_level) FROM caller_security_adjustments WHERE caller_id=c.caller_id AND status='active'), c.security_level)) FROM callers c WHERE c.caller_id = ?1 AND c.account_state = 'active'",
            params![caller_id.get()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, u16>(1)?)),
        )
        .optional()
        .map_err(MessageError::Sqlite)?
        .ok_or(MessageError::CallerUnavailable)?;
    Ok((
        value.0,
        SecurityLevel::new(value.1).map_err(MessageError::InvalidCaller)?,
    ))
}

fn conference_policy_snapshot(
    connection: &rusqlite::Connection,
    conference_id: ConferenceId,
) -> Result<ConferencePolicySnapshot, MessageError> {
    connection
        .query_row(
            r#"
            SELECT conference_id, conference_number, access_mode, read_security,
                   post_security, public_only, caller_deletion_enabled
              FROM message_conferences
             WHERE conference_id = ?1 AND active = 1
            "#,
            params![conference_id.get()],
            |row| {
                let mode = row.get::<_, String>(2)?;
                Ok(ConferencePolicySnapshot {
                    id: ConferenceId::new(row.get(0)?).map_err(to_sql_conversion_error)?,
                    number: row.get(1)?,
                    access_mode: ConferenceAccessMode::from_database_value(&mode)
                        .map_err(to_sql_conversion_error)?,
                    read_security: SecurityLevel::new(row.get(3)?)
                        .map_err(to_sql_conversion_error)?,
                    post_security: SecurityLevel::new(row.get(4)?)
                        .map_err(to_sql_conversion_error)?,
                    public_only: row.get(5)?,
                    caller_deletion_enabled: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(MessageError::Sqlite)?
        .ok_or(MessageError::ConferenceIdNotFound(conference_id.get()))
}

fn conference_snapshot_by_number(
    connection: &rusqlite::Connection,
    conference_number: u16,
) -> Result<ConferencePolicySnapshot, MessageError> {
    let id = connection
        .query_row(
            "SELECT conference_id FROM message_conferences WHERE conference_number = ?1 AND active = 1",
            params![conference_number],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(MessageError::Sqlite)?
        .ok_or(MessageError::ConferenceNotFound(conference_number))?;
    conference_policy_snapshot(connection, ConferenceId::new(id)?)
}

fn privileged_security(
    connection: &rusqlite::Connection,
    conference_id: ConferenceId,
    security: SecurityLevel,
) -> Result<bool, MessageError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conference_privileged_security WHERE conference_id = ?1 AND security_level = ?2)",
            params![conference_id.get(), security.get()],
            |row| row.get(0),
        )
        .map_err(MessageError::Sqlite)
}

fn ensure_read_authority(
    connection: &rusqlite::Connection,
    conference: &ConferencePolicySnapshot,
    security: SecurityLevel,
    sysop_security: SecurityLevel,
) -> Result<(), MessageError> {
    if security.is_sysop(sysop_security)
        || privileged_security(connection, conference.id, security)?
        || conference
            .access_mode
            .allows(security, conference.read_security)
    {
        Ok(())
    } else {
        Err(MessageError::ConferenceAccessDenied(conference.number))
    }
}

fn ensure_post_authority(
    connection: &rusqlite::Connection,
    conference_id: ConferenceId,
    security: SecurityLevel,
    sysop_security: SecurityLevel,
) -> Result<(), MessageError> {
    let conference = conference_policy_snapshot(connection, conference_id)?;
    if security.is_sysop(sysop_security)
        || privileged_security(connection, conference_id, security)?
        || security.allows(conference.post_security)
    {
        Ok(())
    } else {
        Err(MessageError::ConferenceAccessDenied(conference.number))
    }
}

fn validate_recipient_connection(
    connection: &rusqlite::Connection,
    recipient: &MessageRecipient,
    conference_id: ConferenceId,
    conference_number: u16,
) -> Result<(), MessageError> {
    let valid: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM callers WHERE caller_id = ?1 AND display_name = ?2 AND account_state = 'active')",
            params![recipient.caller_id.get(), recipient.display_name],
            |row| row.get(0),
        )
        .map_err(MessageError::Sqlite)?;
    if !valid {
        return Err(MessageError::RecipientNotFound);
    }
    if conference_number != 1 {
        let queued: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM caller_message_queue WHERE caller_id = ?1 AND conference_id = ?2)",
                params![recipient.caller_id.get(), conference_id.get()],
                |row| row.get(0),
            )
            .map_err(MessageError::Sqlite)?;
        if !queued {
            return Err(MessageError::RecipientConferenceNotQueued(
                conference_number,
            ));
        }
    }
    Ok(())
}

fn load_message_by_number_connection(
    connection: &rusqlite::Connection,
    conference_id: ConferenceId,
    message_number: u64,
) -> Result<Option<Message>, MessageError> {
    connection
        .query_row(
            &format!("{MESSAGE_SELECT} WHERE m.conference_id = ?1 AND m.message_number = ?2"),
            params![conference_id.get(), sqlite_i64(message_number)?],
            message_from_row,
        )
        .optional()
        .map_err(MessageError::Sqlite)
}

fn validate_fanout(connection: &rusqlite::Connection, fanout_id: i64) -> Result<(), MessageError> {
    let (deliveries, recipients, distinct_recipients, minimum, maximum): (
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = connection
        .query_row(
            r#"
            SELECT
                (SELECT COUNT(*) FROM messages WHERE fanout_id = ?1),
                (SELECT COUNT(*) FROM message_delivery_recipients WHERE fanout_id = ?1),
                (SELECT COUNT(DISTINCT caller_id) FROM message_delivery_recipients WHERE fanout_id = ?1),
                (SELECT COALESCE(MIN(delivery_ordinal), 0) FROM messages WHERE fanout_id = ?1),
                (SELECT COALESCE(MAX(delivery_ordinal), 0) FROM messages WHERE fanout_id = ?1)
            "#,
            params![fanout_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .map_err(MessageError::Sqlite)?;
    if !(1..=10).contains(&deliveries)
        || recipients != distinct_recipients
        || minimum != 0
        || maximum != deliveries - 1
        || !(recipients == deliveries || (deliveries == 1 && recipients == 0))
    {
        return Err(MessageError::MutationInvariant);
    }
    Ok(())
}

fn next_message_number(
    transaction: &rusqlite::Transaction<'_>,
    conference_id: ConferenceId,
) -> Result<u64, MessageError> {
    let current: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(message_number), 0) FROM messages WHERE conference_id = ?1",
            params![conference_id.get()],
            |row| row.get(0),
        )
        .map_err(MessageError::Sqlite)?;
    sqlite_u64(current)?
        .checked_add(1)
        .ok_or(MessageError::MessageNumberOverflow)
}

fn next_message_id(transaction: &rusqlite::Transaction<'_>) -> Result<MessageId, MessageError> {
    let current: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(message_id), 0) FROM messages",
            [],
            |row| row.get(0),
        )
        .map_err(MessageError::Sqlite)?;
    MessageId::new(
        current
            .checked_add(1)
            .ok_or(MessageError::MessageNumberOverflow)?,
    )
}

fn sqlite_i64(value: u64) -> Result<i64, MessageError> {
    i64::try_from(value).map_err(|_| MessageError::MessageNumberOverflow)
}

fn sqlite_u64(value: i64) -> Result<u64, MessageError> {
    u64::try_from(value).map_err(|_| MessageError::InvalidStoredMessageNumber(value))
}

fn to_sql_conversion_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Null, Box::new(error))
}

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("conference identifier must be positive, got {0}")]
    InvalidConferenceId(i64),
    #[error("message identifier must be positive, got {0}")]
    InvalidMessageId(i64),
    #[error("conference number must be in 1..={MAX_CONFERENCES}, got {0}")]
    InvalidConferenceNumber(u16),
    #[error("conference name must contain 1..=60 bytes")]
    InvalidConferenceName,
    #[error("conference description must contain at most 255 bytes")]
    InvalidConferenceDescription,
    #[error("conference maximum message lines must be in 25..=99, got {0}")]
    InvalidMaximumLines(u16),
    #[error("a conference may contain at most five privileged security levels")]
    TooManyPrivilegedSecurityLevels,
    #[error("conference privileged security levels must be unique")]
    DuplicatePrivilegedSecurityLevel,
    #[error("conference {0} already exists")]
    ConferenceAlreadyExists(u16),
    #[error("conference renumbering is not supported; preserve stable identity")]
    ConferenceRenumberNotSupported,
    #[error("conference 1 is required for stock SPITFIRE operation and cannot be disabled")]
    RequiredConferenceCannotBeDisabled,
    #[error("message subject must contain 1..={MAX_MESSAGE_SUBJECT_BYTES} display-safe bytes")]
    InvalidSubject,
    #[error("message body must contain 1..={MAX_MESSAGE_BODY_BYTES} display-safe bytes")]
    InvalidBody,
    #[error("message has {actual} lines; conference maximum is {maximum}")]
    TooManyLines { actual: usize, maximum: usize },
    #[error("conference {0} does not exist")]
    ConferenceNotFound(u16),
    #[error("conference identifier {0} does not exist")]
    ConferenceIdNotFound(i64),
    #[error("access to conference {0} is denied")]
    ConferenceAccessDenied(u16),
    #[error("message {number} does not exist in conference {conference}")]
    MessageNotFound { conference: u16, number: u64 },
    #[error("message access is denied")]
    MessageAccessDenied,
    #[error("message mutation is not authorized")]
    MutationDenied,
    #[error("message changed after it was displayed; reopen it and try again")]
    MutationConflict,
    #[error("message is already deleted")]
    AlreadyDeleted,
    #[error("message is already active")]
    AlreadyActive,
    #[error("message state version overflow")]
    StateVersionOverflow,
    #[error("message mutation violated a persistent invariant")]
    MutationInvariant,
    #[error("message discovery requires 1..={MAX_CONFERENCES} conferences, got {0}")]
    InvalidDiscoveryConferenceCount(usize),
    #[error("message discovery query is empty, oversized, or malformed")]
    InvalidDiscoveryQuery,
    #[error("caller account is unavailable")]
    CallerUnavailable,
    #[error("private messages are not allowed in conference {0}")]
    PrivateMessagesNotAllowed(u16),
    #[error("a private message requires a recipient")]
    PrivateMessageNeedsRecipient,
    #[error("a carbon-copy fan-out requires a primary recipient")]
    CarbonCopyNeedsPrimary,
    #[error("a message may contain at most {MAX_MESSAGE_CC_RECIPIENTS} carbon copies")]
    TooManyCarbonCopies,
    #[error("a caller cannot send a message to themselves")]
    SelfRecipient,
    #[error("a primary or carbon-copy recipient was entered more than once")]
    DuplicateRecipient,
    #[error("message recipient does not exist or is unavailable")]
    RecipientNotFound,
    #[error("message recipient does not have conference {0} in their queue")]
    RecipientConferenceNotQueued(u16),
    #[error("reply parent message {0} does not exist or is not visible")]
    ParentMessageNotFound(i64),
    #[error("message number overflow")]
    MessageNumberOverflow,
    #[error("database contains invalid message number {0}")]
    InvalidStoredMessageNumber(i64),
    #[error("database contains unknown conference access mode {0:?}")]
    InvalidStoredAccessMode(String),
    #[error("database contains unknown message visibility {0:?}")]
    InvalidStoredVisibility(String),
    #[error("database contains unknown message kind {0:?}")]
    InvalidStoredKind(String),
    #[error("database contains unknown message delivery role {0:?}")]
    InvalidStoredDeliveryRole(String),
    #[error("database contains unknown message lifecycle {0:?}")]
    InvalidStoredLifecycle(String),
    #[error(transparent)]
    InvalidCaller(#[from] CallerError),
    #[error(transparent)]
    Database(#[from] crate::DatabaseError),
    #[error("SQLite message operation failed: {0}")]
    Sqlite(#[source] rusqlite::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoardIdentity, CallerState, CredentialHasher, PasswordHashConfig};
    use tempfile::TempDir;

    fn database() -> (
        TempDir,
        RuntimeDatabase,
        MessageActor,
        MessageActor,
        MessageActor,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("runtime.sqlite3");
        let mut database = RuntimeDatabase::open(&path).unwrap();
        database.migrate().unwrap();
        database
            .ensure_board_identity(&BoardIdentity::new("Message Test Board", "Test Sysop").unwrap())
            .unwrap();
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher.hash(b"synthetic message test password").unwrap();
        let alice = database
            .create_caller(
                b"Alice Caller",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        let bob = database
            .create_caller(
                b"Bob Caller",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        let stranger = database
            .create_caller(
                b"Other Caller",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        database
            .ensure_conference(&ConferenceDefinition {
                number: 1,
                name: "General".to_owned(),
                description: "General messages".to_owned(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: false,
                caller_deletion_enabled: true,
                maximum_lines: 50,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        let sysop = SecurityLevel::new(100).unwrap();
        (
            temp,
            database,
            MessageActor::new(alice.id, sysop),
            MessageActor::new(bob.id, sysop),
            MessageActor::new(stranger.id, sysop),
        )
    }

    fn public_message(conference_id: ConferenceId) -> NewMessage {
        NewMessage {
            conference_id,
            recipient_caller_id: None,
            recipient_name: "All Callers".to_owned(),
            subject: b"Synthetic subject".to_vec(),
            body: b"Synthetic body\r\n".to_vec(),
            created_at: 2,
            parent_message_id: None,
            visibility: MessageVisibility::Public,
            kind: MessageKind::Standard,
        }
    }

    fn message_with_body(conference_id: ConferenceId, body: &[u8]) -> NewMessage {
        let mut message = public_message(conference_id);
        message.body = body.to_vec();
        message
    }

    fn create_test_caller(
        database: &mut RuntimeDatabase,
        name: &str,
        security: u16,
        sysop_security: u16,
    ) -> MessageActor {
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher.hash(b"synthetic mutation password").unwrap();
        let caller = database
            .create_caller(
                name.as_bytes(),
                &hash,
                SecurityLevel::new(security).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        MessageActor::new(caller.id, SecurityLevel::new(sysop_security).unwrap())
    }

    #[test]
    fn conferences_posts_replies_and_last_read_are_persistent() {
        let (_temp, mut database, alice, _bob, _stranger) = database();
        let conference = database.conference(alice, 1).unwrap();
        let original = database.post(alice, public_message(conference.id)).unwrap();
        assert_eq!(original.author_caller_id, Some(alice.caller_id()));
        assert_eq!(original.author_name, "Alice Caller");
        let mut reply = public_message(conference.id);
        reply.parent_message_id = Some(original.id);
        reply.subject = b"Re: Synthetic subject".to_vec();
        let reply = database.post(alice, reply).unwrap();
        assert_eq!(reply.parent_message_id, Some(original.id));
        database
            .mark_read(alice, conference.id, reply.number)
            .unwrap();
        assert_eq!(
            database.last_read(alice, conference.id).unwrap(),
            reply.number
        );
        assert_eq!(database.stats(alice).unwrap().sent, 2);
    }

    #[test]
    fn private_message_is_visible_only_to_author_recipient_and_sysop() {
        let (_temp, mut database, alice, bob, stranger) = database();
        let conference = database.conference(alice, 1).unwrap();
        let bob_caller = database.caller_by_id(bob.caller_id()).unwrap().unwrap();
        let mut private = public_message(conference.id);
        private.recipient_caller_id = Some(bob.caller_id());
        private.recipient_name = bob_caller.display_name;
        private.visibility = MessageVisibility::Private;
        let stored = database.post(alice, private).unwrap();
        assert_eq!(
            database.message(bob, conference.id, stored.number).unwrap(),
            stored
        );
        assert!(matches!(
            database.message(stranger, conference.id, stored.number),
            Err(MessageError::MessageAccessDenied)
        ));
        assert!(matches!(
            database.received(stranger, conference.id, stored.number),
            Err(MessageError::MessageAccessDenied)
        ));
        database
            .connection
            .execute(
                "UPDATE callers SET security_level = 100 WHERE caller_id = ?1",
                params![stranger.caller_id().get()],
            )
            .unwrap();
        assert_eq!(
            database
                .message(stranger, conference.id, stored.number)
                .unwrap(),
            stored
        );
    }

    #[test]
    fn queues_and_receipts_persist_and_enforce_recipient_conference_selection() {
        let (temp, mut database, alice, bob, _stranger) = database();
        database
            .ensure_conference(&ConferenceDefinition {
                number: 2,
                name: "Queued".to_owned(),
                description: "Queued messages".to_owned(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: false,
                caller_deletion_enabled: true,
                maximum_lines: 50,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        let conference = database.conference(alice, 2).unwrap();
        let bob_caller = database.caller_by_id(bob.caller_id()).unwrap().unwrap();
        let mut private = public_message(conference.id);
        private.recipient_caller_id = Some(bob.caller_id());
        private.recipient_name = bob_caller.display_name;
        private.visibility = MessageVisibility::Private;
        assert!(matches!(
            database.post(alice, private.clone()),
            Err(MessageError::RecipientConferenceNotQueued(2))
        ));

        assert_eq!(
            database
                .replace_queue(bob, &[2])
                .unwrap()
                .iter()
                .map(|conference| conference.number)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            database
                .queued_conferences(alice)
                .unwrap()
                .iter()
                .map(|conference| conference.number)
                .collect::<Vec<_>>(),
            vec![1]
        );
        let stored = database.post(alice, private).unwrap();
        assert!(!database
            .received(alice, conference.id, stored.number)
            .unwrap());
        assert!(!database
            .received(bob, conference.id, stored.number)
            .unwrap());
        assert_eq!(database.stats(bob).unwrap().new_waiting, 1);
        database
            .mark_read(bob, conference.id, stored.number)
            .unwrap();
        database
            .mark_read(bob, conference.id, stored.number)
            .unwrap();
        assert!(database
            .received(bob, conference.id, stored.number)
            .unwrap());
        assert_eq!(database.stats(bob).unwrap().already_received, 1);
        assert_eq!(database.stats(bob).unwrap().new_waiting, 0);
        drop(database);

        let reopened = RuntimeDatabase::open(&temp.path().join("runtime.sqlite3")).unwrap();
        assert_eq!(
            reopened
                .queued_conferences(bob)
                .unwrap()
                .iter()
                .map(|conference| conference.number)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(reopened
            .received(bob, conference.id, stored.number)
            .unwrap());
        assert_eq!(reopened.stats(alice).unwrap().sent, 1);
        assert!(!reopened
            .received(alice, conference.id, stored.number)
            .unwrap());
    }

    #[test]
    fn disabled_stale_actor_cannot_access_messages_or_last_read() {
        let (_temp, mut database, alice, _bob, _stranger) = database();
        let conference = database.conference(alice, 1).unwrap();
        let message = database.post(alice, public_message(conference.id)).unwrap();
        database
            .set_caller_state(alice.caller_id(), CallerState::Disabled)
            .unwrap();
        assert!(matches!(
            database.message(alice, conference.id, message.number),
            Err(MessageError::CallerUnavailable)
        ));
        assert!(matches!(
            database.mark_read(alice, conference.id, message.number),
            Err(MessageError::CallerUnavailable)
        ));
    }

    #[test]
    fn conference_security_and_invalid_inputs_are_enforced() {
        let (_temp, mut database, alice, _bob, _stranger) = database();
        database
            .ensure_conference(&ConferenceDefinition {
                number: 2,
                name: "Restricted".to_owned(),
                description: "Restricted messages".to_owned(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(50).unwrap(),
                post_security: SecurityLevel::new(50).unwrap(),
                public_only: true,
                caller_deletion_enabled: true,
                maximum_lines: 25,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        assert!(matches!(
            database.conference(alice, 2),
            Err(MessageError::ConferenceAccessDenied(2))
        ));
        let conference = database.conference(alice, 1).unwrap();
        let mut invalid = public_message(conference.id);
        invalid.body.clear();
        assert!(matches!(
            database.post(alice, invalid),
            Err(MessageError::InvalidBody)
        ));
        let mut terminal_escape = public_message(conference.id);
        terminal_escape.body = b"unsafe \x1b[2J body\r\n".to_vec();
        assert!(matches!(
            database.post(alice, terminal_escape),
            Err(MessageError::InvalidBody)
        ));
        assert!(matches!(
            database.recipient(b"Missing Caller"),
            Err(MessageError::RecipientNotFound)
        ));
    }

    #[test]
    fn exact_access_mode_and_privileged_security_exception_are_enforced() {
        let (_temp, database, alice, _bob, _stranger) = database();
        let conference = database.load_conference_by_number(1).unwrap().unwrap();
        database
            .connection
            .execute(
                "UPDATE message_conferences SET access_mode = 'exact', read_security = 20 WHERE conference_id = ?1",
                params![conference.id.get()],
            )
            .unwrap();
        assert!(matches!(
            database.conference(alice, 1),
            Err(MessageError::ConferenceAccessDenied(1))
        ));
        assert!(matches!(
            database.discover_messages(
                alice,
                &[conference.id],
                &MessageDiscoveryQuery::Text {
                    terms: vec![b"hidden".to_vec()]
                }
            ),
            Err(MessageError::ConferenceAccessDenied(1))
        ));
        database
            .connection
            .execute(
                "INSERT INTO conference_privileged_security (conference_id, security_level) VALUES (?1, 10)",
                params![conference.id.get()],
            )
            .unwrap();
        assert_eq!(database.conference(alice, 1).unwrap().number, 1);
    }

    #[test]
    fn text_discovery_matches_sf37_case_substring_body_count_and_cp437_evidence() {
        let (_temp, mut database, alice, bob, _stranger) = database();
        database
            .ensure_conference(&ConferenceDefinition {
                number: 2,
                name: "Second".to_owned(),
                description: "Second messages".to_owned(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: false,
                caller_deletion_enabled: true,
                maximum_lines: 50,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        let first = database.conference(alice, 1).unwrap();
        let second = database.conference(alice, 2).unwrap();
        database
            .post(alice, message_with_body(first.id, b"Alpha beta \xDB\r\n"))
            .unwrap();
        database
            .post(alice, message_with_body(first.id, b"alpha BETA\r\n"))
            .unwrap();
        database
            .post(alice, message_with_body(first.id, b"alphabet only\r\n"))
            .unwrap();
        let mut subject_only = message_with_body(first.id, b"No body match\r\n");
        subject_only.subject = b"Alpha beta".to_vec();
        database.post(alice, subject_only).unwrap();
        database
            .post(alice, message_with_body(second.id, b"Alpha beta later\r\n"))
            .unwrap();
        let mut private = message_with_body(first.id, b"Alpha beta private\r\n");
        private.recipient_caller_id = Some(bob.caller_id());
        private.recipient_name = "Bob Caller".to_owned();
        private.visibility = MessageVisibility::Private;
        let private = database.post(alice, private).unwrap();

        let result = database
            .discover_messages(
                bob,
                &[second.id, first.id, first.id],
                &MessageDiscoveryQuery::Text {
                    terms: vec![b"Alpha".to_vec(), b"beta".to_vec()],
                },
            )
            .unwrap();
        assert_eq!(
            result
                .matches
                .iter()
                .map(|found| (found.conference_number, found.message_number))
                .collect::<Vec<_>>(),
            vec![(1, 1), (1, 2), (1, private.number), (2, 1)]
        );
        assert!(!result.truncated);
        assert_eq!(database.last_read(bob, first.id).unwrap(), 0);
        assert!(!database.received(bob, first.id, private.number).unwrap());

        let lowercase = database
            .discover_messages(
                bob,
                &[first.id],
                &MessageDiscoveryQuery::Text {
                    terms: vec![b"alpha".to_vec()],
                },
            )
            .unwrap();
        assert_eq!(lowercase.matches.len(), 4);
        let repeated = database
            .post(alice, message_with_body(first.id, b"Zeta Zeta\r\n"))
            .unwrap();
        let repeated_result = database
            .discover_messages(
                bob,
                &[first.id],
                &MessageDiscoveryQuery::Text {
                    terms: vec![b"zeta".to_vec()],
                },
            )
            .unwrap();
        assert_eq!(repeated_result.matches.len(), 1);
        assert_eq!(repeated_result.matches[0].message_number, repeated.number);
        let cp437 = database
            .discover_messages(
                bob,
                &[first.id],
                &MessageDiscoveryQuery::Text {
                    terms: vec![vec![0xDB]],
                },
            )
            .unwrap();
        assert_eq!(cp437.matches.len(), 1);

        for invalid in [
            Vec::new(),
            vec![Vec::new()],
            vec![vec![b'x'; MAX_MESSAGE_SEARCH_TERM_BYTES + 1]],
            vec![b"one".to_vec(); MAX_MESSAGE_SEARCH_TERMS + 1],
            vec![b"two words".to_vec()],
        ] {
            assert!(matches!(
                database.discover_messages(
                    bob,
                    &[first.id],
                    &MessageDiscoveryQuery::Text { terms: invalid }
                ),
                Err(MessageError::InvalidDiscoveryQuery)
            ));
        }
    }

    #[test]
    fn caller_discovery_obeys_direction_visibility_deletion_and_current_authority() {
        let (_temp, mut database, alice, bob, stranger) = database();
        let conference = database.conference(alice, 1).unwrap();
        let mut to_bob = public_message(conference.id);
        to_bob.recipient_caller_id = Some(bob.caller_id());
        to_bob.recipient_name = "Bob Caller".to_owned();
        let to_bob = database.post(alice, to_bob).unwrap();
        let from_bob = database.post(bob, public_message(conference.id)).unwrap();
        let mut private = message_with_body(conference.id, b"Private discovery body\r\n");
        private.recipient_caller_id = Some(alice.caller_id());
        private.recipient_name = "Alice Caller".to_owned();
        private.visibility = MessageVisibility::Private;
        let private = database.post(bob, private).unwrap();
        database
            .connection
            .execute(
                "UPDATE messages SET lifecycle_state = 'deleted' WHERE message_id = ?1",
                params![from_bob.id.get()],
            )
            .unwrap();

        let from_query = MessageDiscoveryQuery::SpecificCaller {
            caller_id: bob.caller_id(),
            direction: MessageCallerSearchDirection::From,
        };
        assert!(database
            .discover_messages(alice, &[conference.id], &from_query)
            .unwrap()
            .matches
            .is_empty());
        let both_query = MessageDiscoveryQuery::SpecificCaller {
            caller_id: bob.caller_id(),
            direction: MessageCallerSearchDirection::Both,
        };
        assert_eq!(
            database
                .discover_messages(alice, &[conference.id], &both_query)
                .unwrap()
                .matches
                .iter()
                .map(|found| found.message_number)
                .collect::<Vec<_>>(),
            vec![to_bob.number]
        );
        assert!(database
            .discover_messages(stranger, &[conference.id], &both_query)
            .unwrap()
            .matches
            .iter()
            .all(|found| found.message_number != private.number));

        database
            .connection
            .execute(
                "UPDATE callers SET security_level = 100 WHERE caller_id = ?1",
                params![stranger.caller_id().get()],
            )
            .unwrap();
        assert_eq!(
            database
                .discover_messages(stranger, &[conference.id], &both_query)
                .unwrap()
                .matches
                .iter()
                .map(|found| found.message_number)
                .collect::<Vec<_>>(),
            vec![to_bob.number, private.number]
        );

        let found = database
            .discover_messages(stranger, &[conference.id], &both_query)
            .unwrap()
            .matches[0];
        database
            .set_caller_state(stranger.caller_id(), CallerState::Disabled)
            .unwrap();
        assert!(matches!(
            database.message(stranger, found.conference_id, found.message_number),
            Err(MessageError::CallerUnavailable)
        ));
        assert!(matches!(
            database.discover_messages(stranger, &[conference.id], &both_query),
            Err(MessageError::CallerUnavailable)
        ));
    }

    #[test]
    fn discovery_caps_results_without_unbounded_collection() {
        let (_temp, mut database, alice, _bob, _stranger) = database();
        let conference = database.conference(alice, 1).unwrap();
        for index in 0..=MAX_MESSAGE_SEARCH_RESULTS {
            database
                .post(
                    alice,
                    message_with_body(
                        conference.id,
                        format!("bounded needle {index}\r\n").as_bytes(),
                    ),
                )
                .unwrap();
        }
        let result = database
            .discover_messages(
                alice,
                &[conference.id],
                &MessageDiscoveryQuery::Text {
                    terms: vec![b"needle".to_vec()],
                },
            )
            .unwrap();
        assert_eq!(result.matches.len(), MAX_MESSAGE_SEARCH_RESULTS);
        assert!(result.truncated);
        assert!(result.candidates_examined <= MAX_MESSAGE_SEARCH_CANDIDATES);
        assert!(result
            .matches
            .windows(2)
            .all(|pair| pair[0].message_number < pair[1].message_number));
    }

    #[test]
    fn concurrent_post_and_discovery_connections_remain_consistent() {
        let (temp, database, alice, _bob, _stranger) = database();
        let path = temp.path().join("runtime.sqlite3");
        let conference = database.conference(alice, 1).unwrap();
        drop(database);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

        let reader_barrier = barrier.clone();
        let reader_path = path.clone();
        let reader = std::thread::spawn(move || {
            let database = RuntimeDatabase::open(&reader_path).unwrap();
            reader_barrier.wait();
            database
                .discover_messages(
                    alice,
                    &[conference.id],
                    &MessageDiscoveryQuery::Text {
                        terms: vec![b"concurrent".to_vec()],
                    },
                )
                .unwrap()
        });

        let writer_barrier = barrier.clone();
        let writer_path = path.clone();
        let writer = std::thread::spawn(move || {
            let mut database = RuntimeDatabase::open(&writer_path).unwrap();
            writer_barrier.wait();
            database
                .post(
                    alice,
                    message_with_body(conference.id, b"concurrent message\r\n"),
                )
                .unwrap()
        });

        barrier.wait();
        let during = reader.join().unwrap();
        writer.join().unwrap();
        assert!(during.matches.len() <= 1);
        let database = RuntimeDatabase::open(&path).unwrap();
        let after = database
            .discover_messages(
                alice,
                &[conference.id],
                &MessageDiscoveryQuery::Text {
                    terms: vec![b"concurrent".to_vec()],
                },
            )
            .unwrap();
        assert_eq!(after.matches.len(), 1);
    }

    #[test]
    fn fixture_seed_is_idempotent_and_not_attributed_to_a_caller() {
        let (_temp, mut database, alice, _bob, _stranger) = database();
        let first = database
            .ensure_system_message(1, b"Welcome", b"Synthetic welcome\r\n", 1)
            .unwrap();
        let second = database
            .ensure_system_message(1, b"Welcome", b"Different body is ignored\r\n", 2)
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(first.author_caller_id, None);
        assert_eq!(
            database.messages(alice, first.conference_id).unwrap().len(),
            1
        );
    }

    #[test]
    fn b009_cc_fanout_enforces_nine_unique_recipients_and_independent_delivery_state() {
        let (_temp, mut database, alice, bob, stranger) = database();
        let conference = database.conference(alice, 1).unwrap();
        let mut message = public_message(conference.id);
        message.recipient_caller_id = Some(bob.caller_id());
        message.recipient_name = "Bob Caller".to_owned();
        message.subject = b"CP437 \xDB fanout".to_vec();
        message.body = b"Payload \xB3 remains byte exact\r\n".to_vec();
        message.visibility = MessageVisibility::Private;

        let mut cc_actors = vec![stranger];
        for ordinal in 2..=9 {
            cc_actors.push(create_test_caller(
                &mut database,
                &format!("CC Caller {ordinal}"),
                10,
                100,
            ));
        }
        let cc = cc_actors
            .iter()
            .map(|actor| {
                let caller = database.caller_by_id(actor.caller_id()).unwrap().unwrap();
                MessageRecipient {
                    caller_id: caller.id,
                    display_name: caller.display_name,
                }
            })
            .collect::<Vec<_>>();
        let deliveries = database.post_with_cc(alice, message.clone(), &cc).unwrap();
        assert_eq!(deliveries.len(), 10);
        assert_eq!(
            deliveries
                .iter()
                .map(|item| item.number)
                .collect::<Vec<_>>(),
            (1..=10).collect::<Vec<_>>()
        );
        assert_eq!(deliveries[0].delivery_role, MessageDeliveryRole::Primary);
        assert!(deliveries[1..]
            .iter()
            .all(|item| item.delivery_role == MessageDeliveryRole::CarbonCopy));
        assert_eq!(deliveries[9].delivery_ordinal, 9);
        assert_eq!(
            deliveries[1].primary_recipient_name.as_deref(),
            Some("Bob Caller")
        );
        assert!(deliveries
            .iter()
            .all(|item| item.subject == b"CP437 \xDB fanout"
                && item.body == b"Payload \xB3 remains byte exact\r\n"));
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM message_payloads", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        database
            .mark_read(bob, conference.id, deliveries[0].number)
            .unwrap();
        database
            .mark_read(stranger, conference.id, deliveries[1].number)
            .unwrap();
        assert!(database
            .received(bob, conference.id, deliveries[0].number)
            .unwrap());
        assert!(database
            .received(stranger, conference.id, deliveries[1].number)
            .unwrap());
        assert!(matches!(
            database.received(bob, conference.id, deliveries[1].number),
            Err(MessageError::MessageAccessDenied)
        ));

        let threshold = create_test_caller(&mut database, "CC Threshold", 100, 100);
        let all_callers_primary = database
            .toggle_message_visibility(
                threshold,
                conference.id,
                deliveries[0].number,
                deliveries[0].state_version,
                true,
            )
            .unwrap();
        assert_eq!(all_callers_primary.recipient_caller_id, None);
        assert_eq!(all_callers_primary.recipient_name, "All Callers");
        database.validate_current_snapshot().unwrap();

        let deleted = database
            .delete_message(
                stranger,
                conference.id,
                deliveries[1].number,
                deliveries[1].state_version,
            )
            .unwrap();
        assert_eq!(deleted.lifecycle, MessageLifecycle::Deleted);
        assert_eq!(
            database
                .message(alice, conference.id, deliveries[0].number)
                .unwrap()
                .lifecycle,
            MessageLifecycle::Active
        );

        let mut too_many = cc.clone();
        too_many.push(MessageRecipient {
            caller_id: bob.caller_id(),
            display_name: "Bob Caller".to_owned(),
        });
        assert!(matches!(
            database.post_with_cc(alice, message.clone(), &too_many),
            Err(MessageError::TooManyCarbonCopies)
        ));
        assert!(matches!(
            database.post_with_cc(
                alice,
                message.clone(),
                &[MessageRecipient {
                    caller_id: bob.caller_id(),
                    display_name: "Bob Caller".to_owned(),
                }]
            ),
            Err(MessageError::DuplicateRecipient)
        ));
        assert!(matches!(
            database.post_with_cc(
                alice,
                message.clone(),
                &[MessageRecipient {
                    caller_id: CallerId::new(9_999).unwrap(),
                    display_name: "Missing Caller".to_owned(),
                }]
            ),
            Err(MessageError::RecipientNotFound)
        ));
        message.recipient_caller_id = Some(alice.caller_id());
        message.recipient_name = "Alice Caller".to_owned();
        assert!(matches!(
            database.post_with_cc(alice, message, &[]),
            Err(MessageError::SelfRecipient)
        ));
    }

    #[test]
    fn b009_delete_toggle_copy_forward_authorization_lineage_and_audit_are_durable() {
        let (_temp, mut database, alice, bob, stranger) = database();
        let threshold = create_test_caller(&mut database, "Threshold Caller", 100, 100);
        let first = database.conference(alice, 1).unwrap();
        database
            .ensure_conference(&ConferenceDefinition {
                number: 2,
                name: "Destination".to_owned(),
                description: "Copy destination".to_owned(),
                access_mode: ConferenceAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                post_security: SecurityLevel::new(5).unwrap(),
                public_only: false,
                caller_deletion_enabled: true,
                maximum_lines: 50,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        for actor in [alice, bob, stranger, threshold] {
            database.replace_queue(actor, &[1, 2]).unwrap();
        }
        let second = database.conference(threshold, 2).unwrap();

        let mut private = public_message(first.id);
        private.recipient_caller_id = Some(bob.caller_id());
        private.recipient_name = "Bob Caller".to_owned();
        private.visibility = MessageVisibility::Private;
        private.subject = b"Mutation \xDB".to_vec();
        private.body = b"Mutation \xB3 body\r\n".to_vec();
        let private = database.post(alice, private).unwrap();
        database.mark_read(bob, first.id, private.number).unwrap();
        assert!(matches!(
            database.delete_message(stranger, first.id, private.number, private.state_version),
            Err(MessageError::MutationDenied)
        ));
        let deleted = database
            .delete_message(alice, first.id, private.number, private.state_version)
            .unwrap();
        assert_eq!(deleted.lifecycle, MessageLifecycle::Deleted);
        assert!(matches!(
            database.message(bob, first.id, private.number),
            Err(MessageError::MessageNotFound { .. })
        ));
        assert_eq!(
            database
                .message(threshold, first.id, private.number)
                .unwrap()
                .lifecycle,
            MessageLifecycle::Deleted
        );
        let restored = database
            .undelete_message(threshold, first.id, private.number, deleted.state_version)
            .unwrap();
        assert_eq!(restored.id, private.id);
        assert!(restored.received);

        let all = database
            .toggle_message_visibility(
                threshold,
                first.id,
                private.number,
                restored.state_version,
                true,
            )
            .unwrap();
        assert_eq!(all.visibility, MessageVisibility::Public);
        assert_eq!(all.recipient_caller_id, None);
        assert_eq!(all.recipient_name, "All Callers");
        assert!(all.received);
        assert!(matches!(
            database.toggle_message_visibility(
                threshold,
                first.id,
                private.number,
                restored.state_version,
                false
            ),
            Err(MessageError::MutationConflict)
        ));

        let mut named = public_message(first.id);
        named.recipient_caller_id = Some(bob.caller_id());
        named.recipient_name = "Bob Caller".to_owned();
        named.parent_message_id = Some(private.id);
        let named = database.post(alice, named).unwrap();
        assert!(matches!(
            database.toggle_message_visibility(
                alice,
                first.id,
                named.number,
                named.state_version,
                false
            ),
            Err(MessageError::MutationDenied)
        ));
        assert!(matches!(
            database.copy_message(
                alice,
                first.id,
                named.number,
                named.state_version,
                1,
                CopyRecipient::Preserve,
                999,
            ),
            Err(MessageError::MutationDenied)
        ));
        let non_public = database
            .toggle_message_visibility(
                threshold,
                first.id,
                named.number,
                named.state_version,
                false,
            )
            .unwrap();
        assert_eq!(non_public.visibility, MessageVisibility::Private);
        assert_eq!(non_public.recipient_caller_id, Some(bob.caller_id()));
        let public_named = database
            .toggle_message_visibility(
                threshold,
                first.id,
                named.number,
                non_public.state_version,
                false,
            )
            .unwrap();
        assert_eq!(public_named.visibility, MessageVisibility::Public);
        assert_eq!(public_named.recipient_caller_id, Some(bob.caller_id()));

        let mut recipient_owned = public_message(first.id);
        recipient_owned.recipient_caller_id = Some(bob.caller_id());
        recipient_owned.recipient_name = "Bob Caller".to_owned();
        recipient_owned.visibility = MessageVisibility::Private;
        let recipient_owned = database.post(alice, recipient_owned).unwrap();
        assert!(database
            .delete_message(
                bob,
                first.id,
                recipient_owned.number,
                recipient_owned.state_version,
            )
            .is_ok());

        let same = database
            .copy_message(
                threshold,
                first.id,
                named.number,
                public_named.state_version,
                1,
                CopyRecipient::Preserve,
                1000,
            )
            .unwrap();
        assert_ne!(same.id, named.id);
        assert_eq!(same.author_caller_id, Some(alice.caller_id()));
        assert_eq!(same.recipient_caller_id, Some(bob.caller_id()));
        assert_eq!(same.parent_message_id, Some(private.id));
        assert!(!same.received);
        assert_eq!(same.subject, b"Synthetic subject");
        let cross = database
            .copy_message(
                threshold,
                first.id,
                named.number,
                public_named.state_version,
                2,
                CopyRecipient::Preserve,
                1001,
            )
            .unwrap();
        assert_eq!(cross.conference_id, second.id);
        assert_eq!(cross.number, 1);
        let forwarded = database
            .copy_message(
                threshold,
                first.id,
                named.number,
                public_named.state_version,
                1,
                CopyRecipient::Caller(MessageRecipient {
                    caller_id: stranger.caller_id(),
                    display_name: "Other Caller".to_owned(),
                }),
                1002,
            )
            .unwrap();
        assert_eq!(forwarded.author_caller_id, Some(alice.caller_id()));
        assert_eq!(forwarded.recipient_caller_id, Some(stranger.caller_id()));
        assert!(matches!(
            database.copy_message(
                threshold,
                first.id,
                named.number,
                public_named.state_version,
                1,
                CopyRecipient::Caller(MessageRecipient {
                    caller_id: threshold.caller_id(),
                    display_name: "Threshold Caller".to_owned(),
                }),
                1003,
            ),
            Err(MessageError::SelfRecipient)
        ));
        assert_eq!(
            database
                .message(threshold, first.id, named.number)
                .unwrap()
                .lifecycle,
            MessageLifecycle::Active
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT relation FROM message_lineage WHERE derived_message_id = ?1",
                    params![forwarded.id.get()],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "forward"
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM message_mutation_events WHERE operation IN ('deleted', 'undeleted', 'visibility-changed', 'copied', 'forwarded')",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            9
        );
        let audit_columns = database
            .connection
            .prepare("PRAGMA table_info(message_mutation_events)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!audit_columns.iter().any(|name| matches!(
            name.as_str(),
            "subject" | "body" | "recipient_name" | "recipient_list"
        )));
        assert!(database
            .connection
            .execute(
                "UPDATE message_mutation_events SET outcome = 'applied' WHERE event_id = 1",
                [],
            )
            .is_err());
        assert!(database
            .connection
            .execute(
                "UPDATE message_payloads SET body = body WHERE payload_id = 1",
                []
            )
            .is_err());

        database.set_conference_caller_deletion(1, false).unwrap();
        let policy = database.post(alice, public_message(first.id)).unwrap();
        assert!(matches!(
            database.delete_message(alice, first.id, policy.number, policy.state_version),
            Err(MessageError::MutationDenied)
        ));
        assert!(database
            .delete_message(threshold, first.id, policy.number, policy.state_version)
            .is_ok());
    }

    #[test]
    fn b009_two_connections_commit_only_one_stale_delete() {
        let (temp, mut first_database, alice, _bob, _stranger) = database();
        let conference = first_database.conference(alice, 1).unwrap();
        let stored = first_database
            .post(alice, public_message(conference.id))
            .unwrap();
        let path = temp.path().join("runtime.sqlite3");
        let mut second_database = RuntimeDatabase::open(&path).unwrap();
        first_database
            .delete_message(alice, conference.id, stored.number, stored.state_version)
            .unwrap();
        assert!(matches!(
            second_database.delete_message(
                alice,
                conference.id,
                stored.number,
                stored.state_version
            ),
            Err(MessageError::MutationConflict)
        ));
        assert_eq!(
            first_database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM message_mutation_events WHERE operation = 'deleted'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn malformed_stored_terminal_controls_are_rejected_before_rendering() {
        let (_temp, mut database, alice, _bob, _stranger) = database();
        let conference = database.conference(alice, 1).unwrap();
        let stored = database.post(alice, public_message(conference.id)).unwrap();
        database
            .connection
            .execute_batch("DROP TRIGGER message_payloads_immutable_update")
            .unwrap();
        database
            .connection
            .execute(
                "UPDATE message_payloads SET body = ?2 WHERE payload_id = (SELECT f.payload_id FROM messages AS m JOIN message_fanouts AS f ON f.fanout_id = m.fanout_id WHERE m.message_id = ?1)",
                params![stored.id.get(), b"unsafe \x1b[2J body\r\n".as_slice()],
            )
            .unwrap();
        assert!(matches!(
            database.messages(alice, conference.id),
            Err(MessageError::Sqlite(_))
        ));
    }
}
