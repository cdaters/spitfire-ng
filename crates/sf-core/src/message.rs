use std::num::NonZeroI64;

use rusqlite::{params, OptionalExtension};
use thiserror::Error;

use crate::{Caller, CallerError, CallerId, CallerState, RuntimeDatabase, SecurityLevel};

pub const MAX_CONFERENCES: u16 = 784;
pub const MAX_MESSAGE_SUBJECT_BYTES: usize = 72;
pub const MAX_MESSAGE_BODY_BYTES: usize = 64 * 1024;
pub const MAX_MESSAGE_LINES: usize = 99;

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
    fn post(&mut self, actor: MessageActor, message: NewMessage) -> Result<Message, MessageError>;
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
                    read_security, post_security, public_only, maximum_lines
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
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
                    public_only = ?7, maximum_lines = ?8,
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
                       maximum_lines, active
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
            .transaction()
            .map_err(MessageError::Sqlite)?;
        let number = next_message_number(&transaction, conference.id)?;
        transaction
            .execute(
                r#"
                INSERT INTO messages (
                    conference_id, message_number, author_caller_id, author_name,
                    recipient_caller_id, recipient_name, subject, body, created_at,
                    parent_message_id, visibility, kind
                ) VALUES (?1, ?2, NULL, 'SPITFIRE NG', NULL, 'All Callers', ?3, ?4, ?5, NULL, 'public', 'standard')
                "#,
                params![conference.id.get(), sqlite_i64(number)?, subject, body, created_at],
            )
            .map_err(MessageError::Sqlite)?;
        let id = MessageId::new(transaction.last_insert_rowid())?;
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
                       maximum_lines, active
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
                       maximum_lines, active
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
                       maximum_lines, active
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
                &format!("{MESSAGE_SELECT} WHERE conference_id = ?1 AND author_caller_id IS NULL AND subject = ?2 LIMIT 1"),
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
                       maximum_lines, active
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
        let mut statement = self
            .connection
            .prepare(&format!(
                "{MESSAGE_SELECT} WHERE conference_id = ?1 AND deleted = 0 ORDER BY message_number"
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
                &format!("{MESSAGE_SELECT} WHERE conference_id = ?1 AND message_number = ?2 AND deleted = 0"),
                params![conference_id.get(), sqlite_i64(message_number)?],
                message_from_row,
            )
            .optional()
            .map_err(MessageError::Sqlite)?
            .ok_or(MessageError::MessageNotFound {
                conference: conference.number,
                number: message_number,
            })?;
        if !message_visible(&message, &caller, actor.sysop_security) {
            return Err(MessageError::MessageAccessDenied);
        }
        Ok(message)
    }

    fn post(&mut self, actor: MessageActor, message: NewMessage) -> Result<Message, MessageError> {
        let (caller, conference) =
            self.authorized_conference(actor, message.conference_id, true)?;
        validate_message_contents(&message.subject, &message.body, conference.maximum_lines)?;
        if conference.public_only && message.visibility == MessageVisibility::Private {
            return Err(MessageError::PrivateMessagesNotAllowed(conference.number));
        }
        if message.visibility == MessageVisibility::Private && message.recipient_caller_id.is_none()
        {
            return Err(MessageError::PrivateMessageNeedsRecipient);
        }
        if let Some(recipient) = message.recipient_caller_id {
            let stored = self
                .caller_by_id(recipient)
                .map_err(MessageError::Database)?
                .ok_or(MessageError::RecipientNotFound)?;
            if stored.state != CallerState::Active || stored.display_name != message.recipient_name
            {
                return Err(MessageError::RecipientNotFound);
            }
            if conference.number != 1 {
                let queued: bool = self
                    .connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM caller_message_queue WHERE caller_id = ?1 AND conference_id = ?2)",
                        params![recipient.get(), conference.id.get()],
                        |row| row.get(0),
                    )
                    .map_err(MessageError::Sqlite)?;
                if !queued {
                    return Err(MessageError::RecipientConferenceNotQueued(
                        conference.number,
                    ));
                }
            }
        } else if message.recipient_name != "All Callers" {
            return Err(MessageError::RecipientNotFound);
        }
        if let Some(parent) = message.parent_message_id {
            let stored = self
                .load_message_by_id(parent)?
                .ok_or(MessageError::ParentMessageNotFound(parent.get()))?;
            if stored.conference_id != message.conference_id
                || !message_visible(&stored, &caller, actor.sysop_security)
            {
                return Err(MessageError::ParentMessageNotFound(parent.get()));
            }
        }

        let transaction = self
            .connection
            .transaction()
            .map_err(MessageError::Sqlite)?;
        let number = next_message_number(&transaction, conference.id)?;
        transaction
            .execute(
                r#"
                INSERT INTO messages (
                    conference_id, message_number, author_caller_id, author_name,
                    recipient_caller_id, recipient_name, subject, body, created_at,
                    parent_message_id, visibility, kind
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
                params![
                    conference.id.get(),
                    sqlite_i64(number)?,
                    caller.id.get(),
                    caller.display_name,
                    message.recipient_caller_id.map(CallerId::get),
                    message.recipient_name,
                    message.subject,
                    message.body,
                    message.created_at,
                    message.parent_message_id.map(MessageId::get),
                    message.visibility.as_database_value(),
                    message.kind.as_database_value(),
                ],
            )
            .map_err(MessageError::Sqlite)?;
        let id = MessageId::new(transaction.last_insert_rowid())?;
        transaction
            .execute(
                "UPDATE callers SET messages_posted = messages_posted + 1, updated_at = CURRENT_TIMESTAMP WHERE caller_id = ?1 AND account_state = 'active'",
                params![caller.id.get()],
            )
            .map_err(MessageError::Sqlite)?;
        transaction.commit().map_err(MessageError::Sqlite)?;
        self.load_message_by_id(id)?
            .ok_or(MessageError::MessageNotFound {
                conference: conference.number,
                number,
            })
    }

    fn mark_read(
        &mut self,
        actor: MessageActor,
        conference_id: ConferenceId,
        message_number: u64,
    ) -> Result<(), MessageError> {
        let message = self.message(actor, conference_id, message_number)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(MessageError::Sqlite)?;
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

const MESSAGE_SELECT: &str = r#"
    SELECT message_id, conference_id, message_number, author_caller_id,
           author_name, recipient_caller_id, recipient_name, subject, body,
           created_at, parent_message_id, visibility, kind
    FROM messages
"#;
const MESSAGE_SELECT_BY_ID: &str = r#"
    SELECT message_id, conference_id, message_number, author_caller_id,
           author_name, recipient_caller_id, recipient_name, subject, body,
           created_at, parent_message_id, visibility, kind
    FROM messages WHERE message_id = ?1 AND deleted = 0
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
        maximum_lines: row.get(8)?,
        privileged_security_levels: Vec::new(),
        active: row.get(9)?,
    })
}

fn message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let author_id = row.get::<_, Option<i64>>(3)?;
    let recipient_id = row.get::<_, Option<i64>>(5)?;
    let parent_id = row.get::<_, Option<i64>>(10)?;
    let visibility = row.get::<_, String>(11)?;
    let kind = row.get::<_, String>(12)?;
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
    })
}

fn message_visible(message: &Message, caller: &Caller, sysop_security: SecurityLevel) -> bool {
    message.visibility == MessageVisibility::Public
        || caller.security_level.is_sysop(sysop_security)
        || message.author_caller_id == Some(caller.id)
        || message.recipient_caller_id == Some(caller.id)
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
    #[error("caller account is unavailable")]
    CallerUnavailable,
    #[error("private messages are not allowed in conference {0}")]
    PrivateMessagesNotAllowed(u16),
    #[error("a private message requires a recipient")]
    PrivateMessageNeedsRecipient,
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
    use crate::{CallerState, CredentialHasher, PasswordHashConfig};
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
    fn malformed_stored_terminal_controls_are_rejected_before_rendering() {
        let (_temp, database, alice, _bob, _stranger) = database();
        let conference = database.conference(alice, 1).unwrap();
        database
            .connection
            .execute(
                r#"
                INSERT INTO messages (
                    conference_id, message_number, author_caller_id, author_name,
                    recipient_name, subject, body, created_at, visibility, kind
                ) VALUES (?1, 1, ?2, 'Alice Caller', 'All Callers', ?3, ?4, 1, 'public', 'standard')
                "#,
                params![
                    conference.id.get(),
                    alice.caller_id().get(),
                    b"Safe subject".as_slice(),
                    b"unsafe \x1b[2J body\r\n".as_slice()
                ],
            )
            .unwrap();
        assert!(matches!(
            database.messages(alice, conference.id),
            Err(MessageError::Sqlite(_))
        ));
    }
}
