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

//! Private account components and resolved posting identity (M061).
//!
//! A supplied name is not verified identity. Neither these components nor a
//! resolved sender grants authentication, mailbox enrollment or routing authority.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// In-memory binding to canonical TOML authority; never a second database default.
#[derive(Clone, Debug, Default)]
pub(crate) struct IdentityContext {
    pub board: PostingIdentityPolicy,
    pub revision: u64,
    pub ftn: crate::ftn::Policy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct IdentityDestination {
    pub scope: String,
    pub requirement: PostingIdentityPolicy,
    pub revision: String,
    pub maximum_bytes: usize,
    pub authority: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct IdentityEvidence {
    pub resolver_version: u32,
    pub account_version: u64,
    pub conference: Option<i64>,
    pub conference_version: u64,
    pub board_revision: u64,
    pub policy: PostingIdentityPolicy,
    pub mode: PostingIdentityMode,
    pub source: String,
    pub destinations: Vec<IdentityDestination>,
}

/// Caller-private preview, minted by the core and compared again in the posting
/// transaction. No deserialization lets a wire client assert its own evidence.
#[derive(Clone, Eq, PartialEq)]
pub struct PostingIdentityPreview {
    pub(crate) caller: crate::CallerId,
    pub(crate) posted_as: String,
    pub(crate) evidence: IdentityEvidence,
}

impl std::fmt::Debug for PostingIdentityPreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PostingIdentityPreview([private])")
    }
}

impl PostingIdentityPreview {
    pub fn posted_as(&self) -> &str {
        &self.posted_as
    }
    pub fn policy(&self) -> PostingIdentityPolicy {
        self.evidence.policy
    }
    pub fn source(&self) -> &str {
        &self.evidence.source
    }
    pub(crate) fn proof(&self) -> Result<String, IdentityError> {
        serde_json::to_string(&self.evidence).map_err(|_| IdentityError::InvalidPolicy)
    }
}

impl crate::RuntimeDatabase {
    /// Host authority binds the configuration captured for this operation.
    pub fn bind_posting_identity_configuration(&mut self, config: &crate::RuntimeConfig) {
        self.identity_context = IdentityContext {
            board: config.caller.posting_identity,
            revision: config.revision,
            ftn: config.ftn.clone(),
        };
    }

    pub fn preview_posting_identity(
        &self,
        actor: crate::MessageActor,
        conference: crate::ConferenceId,
    ) -> Result<PostingIdentityPreview, crate::MessageError> {
        // This also checks posting/access authority; names are returned only to
        // the authenticated caller's own compose operation.
        self.authorized_conference(actor, conference, true)?;
        resolve_conference(
            &self.connection,
            &self.identity_context,
            actor.caller_id(),
            conference.get(),
        )
        .map_err(crate::MessageError::Identity)
    }
}

pub(crate) fn resolve_conference(
    conn: &Connection,
    context: &IdentityContext,
    caller: crate::CallerId,
    conference: i64,
) -> Result<PostingIdentityPreview, IdentityError> {
    let (policy, source, destinations, version) = conference_policy(conn, context, conference)?;
    let (handle, first, last, account_version): (String,Option<String>,Option<String>,i64) = conn.query_row(
        "SELECT display_name,first_name,last_name,state_version FROM callers WHERE caller_id=?1 AND account_state='active'",
        [caller.get()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let (posted_as, mode) = match policy {
        PostingIdentityPolicy::HandleAllowed => (handle, PostingIdentityMode::Handle),
        PostingIdentityPolicy::RealNameRequired => (
            PrivateIdentity::new(first, last)?
                .real_name()
                .ok_or(IdentityError::MissingRealName)?,
            PostingIdentityMode::RealName,
        ),
    };
    for destination in &destinations {
        let encoded = crate::encode_text(&posted_as, crate::TerminalTextEncoding::Cp437)
            .ok_or(IdentityError::Unrepresentable)?;
        if encoded.len() > destination.maximum_bytes {
            return Err(IdentityError::Unrepresentable);
        }
    }
    Ok(PostingIdentityPreview {
        caller,
        posted_as,
        evidence: IdentityEvidence {
            resolver_version: 1,
            account_version: u64::try_from(account_version)
                .map_err(|_| IdentityError::InvalidPolicy)?,
            conference: Some(conference),
            conference_version: u64::try_from(version).map_err(|_| IdentityError::InvalidPolicy)?,
            board_revision: context.revision,
            policy,
            mode,
            source,
            destinations,
        },
    })
}

pub(crate) fn conference_policy(
    conn: &Connection,
    context: &IdentityContext,
    conference: i64,
) -> Result<(PostingIdentityPolicy, String, Vec<IdentityDestination>, i64), IdentityError> {
    let (override_policy, version): (Option<String>, i64) = conn.query_row(
        "SELECT posting_identity,identity_policy_version FROM message_conferences WHERE conference_id=?1 AND active=1",
        [conference], |r| Ok((r.get(0)?,r.get(1)?)))?;
    let mut policy = override_policy
        .as_deref()
        .map(PostingIdentityPolicy::parse)
        .transpose()?
        .unwrap_or(context.board);
    let mut source = if override_policy.is_some() {
        "conference"
    } else {
        "board-default"
    }
    .to_owned();
    let mut destinations = crate::ftn::identity_destinations(conn, &context.ftn, conference)
        .map_err(|_| IdentityError::InvalidPolicy)?;
    let rows = conn.prepare("SELECT l.link_id,m.wire_conference,m.posting_identity,l.posting_identity,m.version,l.version
        FROM qwk_link_mappings m JOIN qwk_links l USING(link_id)
        WHERE m.conference_id=?1 AND m.enabled=1 AND m.outbound=1 AND l.enabled=1 AND l.outbound=1
        ORDER BY l.link_id,m.wire_conference")?.query_map([conference], |r| Ok((
            r.get::<_,String>(0)?,r.get::<_,u16>(1)?,r.get::<_,String>(2)?,
            r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?)))?
        .collect::<Result<Vec<_>,_>>()?;
    for (link, area, mapping_policy, hard_policy, mv, lv) in rows {
        let requirement = PostingIdentityPolicy::parse(&mapping_policy)?
            .join(PostingIdentityPolicy::parse(&hard_policy)?);
        destinations.push(IdentityDestination {
            scope: format!("qwk:{link}:{area}"),
            requirement,
            revision: format!("{mv}:{lv}"),
            maximum_bytes: 128,
            authority: if hard_policy == "real-name-required" {
                3
            } else {
                2
            },
        });
    }
    destinations.sort_by(|a, b| a.scope.cmp(&b.scope));
    let mut source_authority = 0;
    for destination in &destinations {
        if destination.requirement == PostingIdentityPolicy::RealNameRequired
            && destination.authority > source_authority
        {
            source = destination.scope.clone();
            source_authority = destination.authority;
        }
        policy = policy.join(destination.requirement);
    }
    Ok((policy, source, destinations, version))
}

pub(crate) fn validate_destination(
    conn: &Connection,
    context: &IdentityContext,
    mid: i64,
    requirement: PostingIdentityPolicy,
    scope: &str,
) -> Result<(), IdentityError> {
    let conference: Option<i64> = conn.query_row(
        "SELECT conference_id FROM messages WHERE message_id=?1",
        [mid],
        |r| r.get(0),
    )?;
    let base = if let Some(conference) = conference {
        conference_policy(conn, context, conference)?.0
    } else {
        context.board
    };
    validate_stored(conn, mid, base.join(requirement), Some(scope))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_private(
    conn: &Connection,
    context: &IdentityContext,
    caller: crate::CallerId,
    scope: String,
    requirement: PostingIdentityPolicy,
    revision: String,
    alias: String,
    maximum: usize,
    charset: sf_net::ftn::Charset,
) -> Result<PostingIdentityPreview, IdentityError> {
    let policy = context.board.join(requirement);
    let (first,last,version):(Option<String>,Option<String>,i64) = conn.query_row(
        "SELECT first_name,last_name,state_version FROM callers WHERE caller_id=?1 AND account_state='active'",[caller.get()],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let (posted_as, mode) = if policy == PostingIdentityPolicy::RealNameRequired {
        let name = PrivateIdentity::new(first, last)?
            .real_name()
            .ok_or(IdentityError::MissingRealName)?;
        // Real-name publication does not enroll or rename a network mailbox.
        if name != alias {
            return Err(IdentityError::EnrollmentMismatch);
        }
        (name, PostingIdentityMode::RealName)
    } else {
        (alias, PostingIdentityMode::EnrolledNetworkAlias)
    };
    if charset
        .encode(&posted_as)
        .map_err(|_| IdentityError::Unrepresentable)?
        .len()
        > maximum
    {
        return Err(IdentityError::Unrepresentable);
    }
    Ok(PostingIdentityPreview {
        caller,
        posted_as,
        evidence: IdentityEvidence {
            resolver_version: 1,
            account_version: u64::try_from(version).map_err(|_| IdentityError::InvalidPolicy)?,
            conference: None,
            conference_version: 0,
            board_revision: context.revision,
            policy,
            mode,
            source: scope.clone(),
            destinations: vec![IdentityDestination {
                scope,
                requirement,
                revision,
                maximum_bytes: maximum,
                authority: 3,
            }],
        },
    })
}

pub(crate) fn check_preview(
    current: &PostingIdentityPreview,
    supplied: Option<&PostingIdentityPreview>,
) -> Result<(), IdentityError> {
    if let Some(supplied) = supplied {
        if supplied != current {
            return Err(IdentityError::PreviewChanged);
        }
    } else if current.policy() == PostingIdentityPolicy::RealNameRequired {
        return Err(IdentityError::PreviewRequired);
    }
    Ok(())
}

/// Revalidate stored evidence without reading mutable private profile values.
pub(crate) fn validate_stored(
    conn: &Connection,
    mid: i64,
    requirement: PostingIdentityPolicy,
    scope: Option<&str>,
) -> Result<(), IdentityError> {
    let (author,mode,proof,external): (Option<i64>,String,Option<String>,bool) = conn.query_row(
        "SELECT author_caller_id,identity_mode,identity_proof,origin_kind='external-network' FROM messages WHERE message_id=?1",
        [mid], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    if external && author.is_none() {
        return Ok(());
    }
    if let Some(author) = author {
        let active: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM callers WHERE caller_id=?1 AND account_state='active')",
            [author],
            |r| r.get(0),
        )?;
        if !active {
            return Err(IdentityError::PolicyMismatch);
        }
    }
    if requirement == PostingIdentityPolicy::RealNameRequired && mode != "real-name" {
        return Err(IdentityError::PolicyMismatch);
    }
    if let Some(proof) = proof {
        let evidence: IdentityEvidence =
            serde_json::from_str(&proof).map_err(|_| IdentityError::PolicyMismatch)?;
        if evidence.resolver_version != 1
            || (requirement == PostingIdentityPolicy::RealNameRequired
                && evidence.mode != PostingIdentityMode::RealName)
        {
            return Err(IdentityError::PolicyMismatch);
        }
        if scope.is_some_and(|scope| !evidence.destinations.iter().any(|d| d.scope == scope)) {
            return Err(IdentityError::PolicyMismatch);
        }
    } else if requirement == PostingIdentityPolicy::RealNameRequired {
        return Err(IdentityError::PolicyMismatch);
    }
    Ok(())
}

pub(crate) fn freeze_sender(
    conn: &Connection,
    queue: &str,
    mid: i64,
    encoded: &[u8],
    profile: &str,
) -> Result<(), IdentityError> {
    let prior: Option<(i64,Vec<u8>,String)> = conn.query_row(
        "SELECT message_id,wire_sender,wire_profile FROM network_sender_snapshots WHERE queue_id=?1", [queue],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    if let Some((stored_mid, bytes, stored_profile)) = prior {
        if stored_mid != mid || bytes != encoded || stored_profile != profile {
            return Err(IdentityError::PolicyMismatch);
        }
    } else {
        conn.execute("INSERT INTO network_sender_snapshots(queue_id,message_id,sender,wire_sender,wire_profile,identity_mode,identity_proof)
            SELECT ?1,message_id,author_name,?3,?4,identity_mode,identity_proof FROM messages WHERE message_id=?2",
            params![queue,mid,encoded,profile])?;
    }
    Ok(())
}

pub(crate) fn freeze_qwk_sender(
    conn: &Connection,
    queue: &str,
    mid: i64,
) -> Result<(), IdentityError> {
    let (sender,encoding):(String,String)=conn.query_row("SELECT m.author_name,p.encoding FROM messages m JOIN message_fanouts f USING(fanout_id) JOIN message_payloads p USING(payload_id) WHERE message_id=?1",[mid],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let utf8 = encoding == "utf8";
    let bytes = crate::encode_text(
        &sender,
        if utf8 {
            crate::TerminalTextEncoding::Utf8
        } else {
            crate::TerminalTextEncoding::Cp437
        },
    )
    .ok_or(IdentityError::Unrepresentable)?;
    freeze_sender(
        conn,
        queue,
        mid,
        &bytes,
        if utf8 {
            "qwk-headers-utf8"
        } else {
            "qwk-headers-cp437"
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityEditActor {
    Caller,
    LocalOperator,
}

impl IdentityEditActor {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Caller => "caller",
            Self::LocalOperator => "local-operator",
        }
    }
}

/// Private profile data. Deliberately not serializable and redacted in Debug.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct PrivateIdentity {
    first_name: Option<String>,
    last_name: Option<String>,
}

impl std::fmt::Debug for PrivateIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateIdentity([private])")
    }
}

impl PrivateIdentity {
    pub fn new(first: Option<String>, last: Option<String>) -> Result<Self, IdentityError> {
        let value = Self {
            first_name: normalize_component(first)?,
            last_name: normalize_component(last)?,
        };
        if value
            .real_name()
            .is_some_and(|name| name.len() > 120 || name.chars().count() > 60)
        {
            return Err(IdentityError::InvalidName);
        }
        Ok(value)
    }

    pub fn first_name(&self) -> Option<&str> {
        self.first_name.as_deref()
    }

    pub fn last_name(&self) -> Option<&str> {
        self.last_name.as_deref()
    }

    /// Only complete, explicitly supplied components form a posting name.
    pub fn real_name(&self) -> Option<String> {
        Some(format!("{} {}", self.first_name()?, self.last_name()?))
    }
}

fn normalize_component(value: Option<String>) -> Result<Option<String>, IdentityError> {
    let Some(value) = value else { return Ok(None) };
    if value.chars().any(|c| {
        c.is_control()
            || matches!(c,
        '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    }) {
        return Err(IdentityError::InvalidName);
    }
    // Unicode Zs only; do not silently accept controls through whitespace folding.
    let name = value
        .split(|c| {
            matches!(
                c,
                ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
                    ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
            )
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if name.len() > 60 {
        return Err(IdentityError::InvalidName);
    }
    Ok((!name.is_empty()).then_some(name))
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostingIdentityPolicy {
    #[default]
    HandleAllowed,
    RealNameRequired,
}

impl PostingIdentityPolicy {
    pub const fn key(self) -> &'static str {
        match self {
            Self::HandleAllowed => "handle-allowed",
            Self::RealNameRequired => "real-name-required",
        }
    }
    pub fn parse(value: &str) -> Result<Self, IdentityError> {
        match value {
            "handle-allowed" => Ok(Self::HandleAllowed),
            "real-name-required" => Ok(Self::RealNameRequired),
            _ => Err(IdentityError::InvalidPolicy),
        }
    }
    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::HandleAllowed, Self::HandleAllowed) => Self::HandleAllowed,
            _ => Self::RealNameRequired,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostingIdentityMode {
    Handle,
    RealName,
    EnrolledNetworkAlias,
    ExternalAsserted,
    System,
    LegacyUnclassified,
}

impl PostingIdentityMode {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Handle => "handle",
            Self::RealName => "real-name",
            Self::EnrolledNetworkAlias => "enrolled-network-alias",
            Self::ExternalAsserted => "external-asserted",
            Self::System => "system",
            Self::LegacyUnclassified => "legacy-unclassified",
        }
    }
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("Posting identity storage is unavailable.")]
    Storage(#[from] rusqlite::Error),
    #[error("Enter first and last names of at most 60 UTF-8 bytes each, together at most 120 bytes and 60 characters, without control characters.")]
    InvalidName,
    #[error("This conference requires your real name. Add your first and last name to your profile before posting.")]
    MissingRealName,
    #[error(
        "Posting identity or policy changed. Review the posting identity again before submitting."
    )]
    PreviewChanged,
    #[error("Review the posting identity before submitting this message.")]
    PreviewRequired,
    #[error("The stored posting identity does not satisfy the destination policy.")]
    PolicyMismatch,
    #[error("The posting name cannot be represented exactly by this destination.")]
    Unrepresentable,
    #[error("The required posting name must also have an explicitly enrolled network mailbox. Ask the Sysop to review your enrollment.")]
    EnrollmentMismatch,
    #[error("Invalid posting identity policy.")]
    InvalidPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_components_normalize_spaces_preserve_names_and_redact() {
        let name = PrivateIdentity::new(
            Some("  María\u{a0}del  Carmen ".into()),
            Some("O’Neil".into()),
        )
        .unwrap();
        assert_eq!(name.real_name().as_deref(), Some("María del Carmen O’Neil"));
        assert!(!format!("{name:?}").contains("María"));
        assert_eq!(
            PrivateIdentity::new(Some("Craig".into()), None)
                .unwrap()
                .real_name(),
            None
        );
        assert_eq!(
            PrivateIdentity::new(None, Some("Daters".into()))
                .unwrap()
                .real_name(),
            None
        );
        for bad in ["A\tB", "A\nB", "A\u{2028}B", "A\u{202e}B", "A\u{2066}B"] {
            assert!(PrivateIdentity::new(Some(bad.into()), None).is_err());
        }
        assert!(PrivateIdentity::new(Some("a".repeat(60)), Some("b".into())).is_err());
        assert!(PrivateIdentity::new(Some("é".repeat(31)), None).is_err());
        let decomposed = "e\u{301}";
        assert_eq!(
            PrivateIdentity::new(Some(decomposed.into()), None)
                .unwrap()
                .first_name(),
            Some(decomposed)
        );
    }

    #[test]
    fn restrictive_join_never_weakens_a_requirement() {
        use PostingIdentityPolicy::*;
        assert_eq!(HandleAllowed.join(HandleAllowed), HandleAllowed);
        assert_eq!(HandleAllowed.join(RealNameRequired), RealNameRequired);
        assert_eq!(RealNameRequired.join(HandleAllowed), RealNameRequired);
    }
    fn board() -> (
        tempfile::TempDir,
        crate::RuntimeDatabase,
        crate::MessageActor,
        crate::ConferenceDefinition,
    ) {
        use crate::*;
        let temp = tempfile::tempdir().unwrap();
        let mut db = RuntimeDatabase::open(&temp.path().join("identity.sqlite3")).unwrap();
        db.migrate().unwrap();
        db.ensure_board_identity(&BoardIdentity::new("Identity Fixture", "Sysop").unwrap())
            .unwrap();
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher.hash(b"synthetic identity password").unwrap();
        let caller = db
            .create_caller(
                b"PixelWizard",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        let definition = ConferenceDefinition {
            posting_identity: None,
            number: 1,
            name: "General".into(),
            description: "Synthetic identity acceptance".into(),
            access_mode: ConferenceAccessMode::AtLeast,
            read_security: SecurityLevel::new(0).unwrap(),
            post_security: SecurityLevel::new(0).unwrap(),
            public_only: true,
            caller_deletion_enabled: true,
            maximum_lines: 99,
            privileged_security_levels: vec![],
        };
        db.ensure_conference(&definition).unwrap();
        (
            temp,
            db,
            MessageActor::new(caller.id, SecurityLevel::new(9999).unwrap()),
            definition,
        )
    }

    fn message(
        id: crate::ConferenceId,
        preview: Option<PostingIdentityPreview>,
    ) -> crate::NewMessage {
        crate::NewMessage {
            identity_preview: preview,
            conference_id: id,
            recipient_caller_id: None,
            recipient_name: "All Callers".into(),
            subject: b"Identity fixture".to_vec(),
            body: b"No private profile components in this body.\r\n".to_vec(),
            created_at: 2,
            parent_message_id: None,
            visibility: crate::MessageVisibility::Public,
            kind: crate::MessageKind::Standard,
        }
    }

    fn names(
        db: &crate::RuntimeDatabase,
        actor: crate::MessageActor,
        first: Option<&str>,
        last: Option<&str>,
    ) {
        let caller = db.caller_by_id(actor.caller_id()).unwrap().unwrap();
        let mut profile = caller.profile;
        profile.identity =
            PrivateIdentity::new(first.map(str::to_owned), last.map(str::to_owned)).unwrap();
        db.update_caller_profile_versioned(
            actor.caller_id(),
            caller.state_version,
            profile,
            &crate::CallerProfilePolicy::default(),
            IdentityEditActor::Caller,
            3,
        )
        .unwrap();
    }

    #[test]
    fn registration_and_handle_post_do_not_claim_or_publish_real_identity() {
        use crate::MessageBackend;
        let (_temp, mut db, actor, _) = board();
        let caller = db.caller_by_id(actor.caller_id()).unwrap().unwrap();
        assert_eq!(caller.real_name, None);
        assert_eq!(caller.profile.identity.real_name(), None);
        names(
            &db,
            actor,
            Some("PrivateCanaryFirst"),
            Some("PrivateCanaryLast"),
        );
        let id = crate::ConferenceId::new(1).unwrap();
        let preview = db.preview_posting_identity(actor, id).unwrap();
        assert_eq!(preview.posted_as(), "PixelWizard");
        let post = db.post(actor, message(id, Some(preview))).unwrap();
        assert_eq!(post.author_name, "PixelWizard");
        let caller = db.caller_by_id(actor.caller_id()).unwrap().unwrap();
        assert!(!format!("{caller:?}").contains("PrivateCanary"));
        let proof: String = db
            .connection
            .query_row(
                "SELECT identity_proof FROM messages WHERE message_id=?1",
                [post.id.get()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!proof.contains("PrivateCanary"));
        let columns = db
            .connection
            .prepare("PRAGMA table_info(caller_name_events)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns
            .iter()
            .any(|c| c.contains("name") || c.contains("digest")));
    }

    #[test]
    fn required_names_fail_closed_and_preview_is_not_forgeable_through_message_input() {
        use crate::MessageBackend;
        let (_temp, mut db, actor, mut definition) = board();
        definition.posting_identity = Some(PostingIdentityPolicy::RealNameRequired);
        let conference = db.update_conference(1, &definition).unwrap();
        for (first, last) in [(None, None), (Some("Craig"), None), (None, Some("Daters"))] {
            names(&db, actor, first, last);
            assert!(matches!(
                db.preview_posting_identity(actor, conference.id),
                Err(crate::MessageError::Identity(
                    IdentityError::MissingRealName
                ))
            ));
            assert!(db.post(actor, message(conference.id, None)).is_err());
        }
        names(&db, actor, Some("Craig"), Some("Daters"));
        assert!(matches!(
            db.post(actor, message(conference.id, None)),
            Err(crate::MessageError::Identity(
                IdentityError::PreviewRequired
            ))
        ));
        let preview = db.preview_posting_identity(actor, conference.id).unwrap();
        assert_eq!(preview.posted_as(), "Craig Daters");
        assert_eq!(
            db.post(actor, message(conference.id, Some(preview)))
                .unwrap()
                .author_name,
            "Craig Daters"
        );
        assert_eq!(
            db.connection
                .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn posted_authors_survive_handle_and_component_changes_deactivation_restart_and_direct_mutation(
    ) {
        use crate::MessageBackend;
        let (temp, mut db, actor, mut definition) = board();
        let id = crate::ConferenceId::new(1).unwrap();
        let handle = db.post(actor, message(id, None)).unwrap();
        names(&db, actor, Some("Craig"), Some("Daters"));
        definition.posting_identity = Some(PostingIdentityPolicy::RealNameRequired);
        db.update_conference(1, &definition).unwrap();
        let real = db
            .post(
                actor,
                message(id, Some(db.preview_posting_identity(actor, id).unwrap())),
            )
            .unwrap();
        let caller = db.caller_by_id(actor.caller_id()).unwrap().unwrap();
        let login = caller.login_identifier.clone();
        db.update_caller_login_handle(
            caller.id,
            caller.state_version,
            login.as_bytes(),
            b"Darkmage",
            &crate::CallerConfig::default(),
            4,
        )
        .unwrap();
        for (first, last) in [("Chris", "Daters"), ("Chris", "D.")] {
            names(&db, actor, Some(first), Some(last));
            assert_eq!(
                db.message(actor, id, handle.number).unwrap().author_name,
                "PixelWizard"
            );
            assert_eq!(
                db.message(actor, id, real.number).unwrap().author_name,
                "Craig Daters"
            );
        }
        assert_eq!(
            db.caller_by_login_identifier(login.as_bytes())
                .unwrap()
                .unwrap()
                .id,
            caller.id
        );
        for sql in [
            "UPDATE messages SET author_name='Mutated'",
            "UPDATE messages SET identity_mode='handle'",
            "UPDATE messages SET identity_proof=NULL",
            "DELETE FROM callers",
        ] {
            assert!(db.connection.execute(sql, []).is_err(), "{sql}");
        }
        db.connection
            .execute(
                "UPDATE callers SET account_state='deleted' WHERE caller_id=?1",
                [caller.id.get()],
            )
            .unwrap();
        drop(db);
        let db = crate::RuntimeDatabase::open(&temp.path().join("identity.sqlite3")).unwrap();
        let authors = db
            .connection
            .prepare("SELECT author_name FROM messages ORDER BY message_id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(authors, ["PixelWizard", "Craig Daters"]);
    }

    #[test]
    fn concurrent_profile_or_conference_change_rejects_stale_preview_atomically() {
        use crate::MessageBackend;
        let (_temp, mut db, actor, mut definition) = board();
        let id = crate::ConferenceId::new(1).unwrap();
        let old = db.preview_posting_identity(actor, id).unwrap();
        names(&db, actor, Some("Craig"), Some("Daters"));
        assert!(matches!(
            db.post(actor, message(id, Some(old))),
            Err(crate::MessageError::Identity(IdentityError::PreviewChanged))
        ));
        let old = db.preview_posting_identity(actor, id).unwrap();
        definition.posting_identity = Some(PostingIdentityPolicy::RealNameRequired);
        db.update_conference(1, &definition).unwrap();
        assert!(matches!(
            db.post(actor, message(id, Some(old))),
            Err(crate::MessageError::Identity(IdentityError::PreviewChanged))
        ));
        assert_eq!(
            db.connection
                .query_row("SELECT COUNT(*) FROM message_payloads", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn board_default_is_fallback_and_collection_does_not_publish() {
        let (_temp, mut db, actor, mut definition) = board();
        names(&db, actor, Some("Craig"), Some("Daters"));
        db.identity_context.board = PostingIdentityPolicy::RealNameRequired;
        let id = crate::ConferenceId::new(1).unwrap();
        assert_eq!(
            db.preview_posting_identity(actor, id).unwrap().posted_as(),
            "Craig Daters"
        );
        definition.posting_identity = Some(PostingIdentityPolicy::HandleAllowed);
        db.update_conference(1, &definition).unwrap();
        assert_eq!(
            db.preview_posting_identity(actor, id).unwrap().posted_as(),
            "PixelWizard"
        );
        let profile = crate::CallerProfile::default();
        let policy = crate::CallerProfilePolicy {
            require_names: true,
            ..Default::default()
        };
        assert!(profile.validate_for_policy(&policy).is_err());
    }
}
