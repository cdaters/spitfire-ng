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

//! QWK network metadata profile layered on the shared QWK record/archive codec.
//! HEADERS.DAT is interchange evidence, never native identity or routing authority.
use crate::qwk::{self, Error, Message, Profile};
use std::collections::{BTreeMap, BTreeSet};

type HeaderFields = Vec<(String, Vec<u8>)>;
type HeaderSections = BTreeMap<usize, HeaderFields>;

pub const MAX_PATH: usize = 32;
pub const MAX_HEADER_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub id: String,
    pub reply: Option<String>,
    /// Reverse route excluding the sending partner, as in SenderNetAddr.
    pub path: Vec<String>,
    pub utf8: bool,
    pub written: Option<String>,
    /// Ordered raw fields, including unknown/unsupported evidence. Never relayed blindly.
    pub fields: Vec<(String, Vec<u8>)>,
}
#[derive(Clone, Debug)]
pub struct NetworkMessage {
    pub message: Message,
    pub metadata: Metadata,
    pub offset: usize,
    pub digest: String,
}

impl Metadata {
    /// QWK system path only; never an FTN or Internet address.
    pub fn destination_path(&self) -> Result<Vec<String>, Error> {
        get(&self.fields, "recipientnetaddr")?
            .map(ascii)
            .transpose()?
            .map_or(Ok(Vec::new()), |s| parse_path(&s))
    }
}

/// Content evidence detects conflicting reuse of an identity; never a duplicate identity by itself.
pub fn content_digest(member: &NetworkMessage) -> String {
    let mut bytes = Vec::new();
    for value in [
        &member.message.from,
        &member.message.to,
        &member.message.subject,
        &member.message.body,
    ] {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    }
    bytes.push(u8::from(member.metadata.utf8));
    qwk::digest(&bytes)
}

pub fn valid_native_text(message: &Message, utf8: bool) -> bool {
    for (bytes, body) in [
        (&message.from, false),
        (&message.to, false),
        (&message.subject, false),
        (&message.body, true),
    ] {
        if bytes
            .iter()
            .any(|b| (*b < 32 && !(body && matches!(*b, b'\r' | b'\n' | b'\t'))) || *b == 127)
        {
            return false;
        }
        if utf8
            && std::str::from_utf8(bytes).map_or(true, |s| {
                s.chars()
                    .any(|c| c.is_control() && !(body && matches!(c, '\r' | '\n' | '\t')))
            })
        {
            return false;
        }
    }
    true
}

/// Recognized Synchronet ISO timestamp spellings; the trailing legacy zone
/// token is retained as evidence, while the explicit ISO offset supplies UTC.
pub fn written_timestamp(value: &str) -> Option<i64> {
    let mut parts = value.split_ascii_whitespace();
    let stamp = parts.next()?;
    if let Some(zone) = parts.next() {
        if zone.len() != 4 || !zone.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
    }
    if parts.next().is_some() {
        return None;
    }
    let stamp = stamp
        .strip_suffix('Z')
        .map_or_else(|| stamp.to_owned(), |s| format!("{s}+0000"));
    for format in ["%Y%m%dT%H%M%S%z", "%Y%m%d%H%M%S%z"] {
        if let Ok(t) = chrono::DateTime::parse_from_str(&stamp, format) {
            return (t.timestamp() >= 0).then_some(t.timestamp());
        }
    }
    None
}

pub fn valid_system_id(id: &str) -> bool {
    qwk::valid_board_id(id)
        && id.len() >= 2
        && id.as_bytes()[0].is_ascii_alphabetic()
        && !matches!(id, "SYSOP" | "NETMAIL")
}
pub fn valid_message_id(id: &str) -> bool {
    id.len() >= 5
        && id.len() <= 255
        && id.starts_with('<')
        && id.ends_with('>')
        && id[1..id.len() - 1].contains('@')
        && id.bytes().all(|b| (33..=126).contains(&b))
}
pub fn parse_path(value: &str) -> Result<Vec<String>, Error> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let path: Vec<_> = value.split('/').map(str::to_ascii_uppercase).collect();
    let unique: BTreeSet<_> = path.iter().collect();
    if path.len() > MAX_PATH
        || unique.len() != path.len()
        || path.iter().any(|id| !valid_system_id(id))
    {
        return Err(Error::Malformed);
    }
    Ok(path)
}
fn ascii(value: &[u8]) -> Result<String, Error> {
    if !value.is_ascii() {
        return Err(Error::Unsupported);
    }
    String::from_utf8(value.to_vec()).map_err(|_| Error::Malformed)
}
fn sections(bytes: &[u8]) -> Result<HeaderSections, Error> {
    if bytes.len() > qwk::MAX_MESSAGES * MAX_HEADER_BYTES {
        return Err(Error::Limit);
    }
    let mut result = BTreeMap::new();
    let mut current = None;
    let mut size = 0;
    for raw in bytes.split(|b| *b == b'\n') {
        if raw.len() > 1024 {
            return Err(Error::Limit);
        }
        let line = raw.trim_ascii();
        if line.is_empty() || line.starts_with(b";") {
            continue;
        }
        if line.starts_with(b"[") && line.ends_with(b"]") {
            let token = ascii(&line[1..line.len() - 1])?;
            if token.is_empty() || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::Malformed);
            }
            let offset = usize::from_str_radix(&token, 16).map_err(|_| Error::Malformed)?;
            if offset < qwk::RECORD
                || !offset.is_multiple_of(qwk::RECORD)
                || result.insert(offset, Vec::new()).is_some()
            {
                return Err(Error::Malformed);
            }
            current = Some(offset);
            size = 0;
            continue;
        }
        let fields = result
            .get_mut(&current.ok_or(Error::Malformed)?)
            .ok_or(Error::Malformed)?;
        size += raw.len();
        if size > MAX_HEADER_BYTES || fields.len() >= 128 {
            return Err(Error::Limit);
        }
        let split = line
            .iter()
            .position(|b| matches!(*b, b':' | b'='))
            .ok_or(Error::Malformed)?;
        let key = ascii(line[..split].trim_ascii())?.to_ascii_lowercase();
        let value = line[split + 1..].trim_ascii();
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.iter().any(|b| *b < 32 || *b == 127)
        {
            return Err(Error::Malformed);
        }
        fields.push((key, value.to_vec()));
    }
    Ok(result)
}
fn get<'a>(fields: &'a [(String, Vec<u8>)], key: &str) -> Result<Option<&'a [u8]>, Error> {
    let mut found = fields.iter().filter(|(k, _)| k == key);
    let value = found.next().map(|(_, v)| v.as_slice());
    if found.next().is_some() {
        return Err(Error::Malformed);
    }
    Ok(value)
}

/// Framing and all offset associations are checked before any caller can import.
pub fn decode(
    artifact: &qwk::Artifact,
    reply_board: Option<&str>,
) -> Result<Vec<NetworkMessage>, Error> {
    let name = reply_board.map_or_else(|| "MESSAGES.DAT".into(), |id| format!("{id}.MSG"));
    let records = artifact.members.get(&name).ok_or(Error::Malformed)?;
    let mut headers = sections(
        artifact
            .members
            .get("HEADERS.DAT")
            .ok_or(Error::Unsupported)?,
    )?;
    if artifact.members.keys().any(|file| {
        file != &name
            && !matches!(
                file.as_str(),
                "HEADERS.DAT" | "CONTROL.DAT" | "DOOR.ID" | "NETFLAGS.DAT" | "PERSONAL.NDX"
            )
            && !file
                .strip_suffix(".NDX")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    }) {
        return Err(Error::Unsupported);
    }
    if artifact.members.contains_key("VOTING.DAT") {
        return Err(Error::Unsupported);
    }
    let mut utf8 = BTreeSet::new();
    for (offset, fields) in &headers {
        match get(fields, "utf8")? {
            Some(b"true") => {
                utf8.insert(*offset);
            }
            Some(b"false") | None => (),
            _ => return Err(Error::Unsupported),
        }
    }
    let members =
        qwk::decode_records_profiled(records, reply_board, Profile::ClassicCp437, &utf8, true)?;
    let mut result = Vec::new();
    for member in members {
        let mut fields = headers.remove(&member.offset).ok_or(Error::Malformed)?;
        let legacy_recipient = member.message.to.clone();
        let mut message = member.message;
        let conference: u16 = ascii(get(&fields, "conference")?.ok_or(Error::Malformed)?)?
            .parse()
            .map_err(|_| Error::Malformed)?;
        if conference != message.conference {
            return Err(Error::Malformed);
        }
        let id = ascii(get(&fields, "message-id")?.ok_or(Error::Unsupported)?)?;
        let reply = get(&fields, "in-reply-to")?.map(ascii).transpose()?;
        if !valid_message_id(&id) || reply.as_ref().is_some_and(|id| !valid_message_id(id)) {
            return Err(Error::Malformed);
        }
        let mut path = parse_path(
            &get(&fields, "sendernetaddr")?
                .map(ascii)
                .transpose()?
                .unwrap_or_default(),
        )?;
        for (key, target, max) in [
            ("sender", &mut message.from, 60),
            ("to", &mut message.to, 60),
            ("subject", &mut message.subject, 72),
        ] {
            let primary = get(&fields, key)?;
            let alias = if key == "to" {
                get(&fields, "recipient")?
            } else {
                None
            };
            if primary.zip(alias).is_some_and(|(a, b)| a != b) {
                return Err(Error::Malformed);
            }
            if let Some(value) = primary.or(alias) {
                *target = value.to_vec();
            }
            if target.is_empty() || target.len() > max {
                return Err(Error::Limit);
            }
            if utf8.contains(&member.offset) {
                std::str::from_utf8(target).map_err(|_| Error::Unsupported)?;
            }
        }
        if message.conference == 0 {
            let declared = get(&fields, "recipientnetaddr")?
                .map(ascii)
                .transpose()?
                .map(|s| parse_path(&s))
                .transpose()?;
            // Legacy transit marker carries one address line, not message content.
            let legacy = legacy_recipient.eq_ignore_ascii_case(b"NETMAIL");
            let address = if legacy {
                let end = message
                    .body
                    .iter()
                    .position(|b| *b == b'\n')
                    .ok_or(Error::Malformed)?;
                let value = message.body[..end].trim_ascii().to_vec();
                message.body = message.body[end + 1..].to_vec();
                Some(value)
            } else if message.to.contains(&b'@') {
                Some(message.to.clone())
            } else {
                None
            };
            if let Some(address) = address {
                let at = address
                    .iter()
                    .position(|b| *b == b'@')
                    .ok_or(Error::Malformed)?;
                let route = parse_path(&ascii(&address[at + 1..])?)?;
                if route.is_empty() || declared.as_ref().is_some_and(|d| !d.ends_with(&route)) {
                    return Err(Error::Malformed);
                }
                let claimed_recipient = message
                    .to
                    .split(|b| *b == b'@')
                    .next()
                    .ok_or(Error::Malformed)?;
                if legacy
                    && !claimed_recipient.eq_ignore_ascii_case(b"NETMAIL")
                    && !claimed_recipient.eq_ignore_ascii_case(&address[..at])
                {
                    return Err(Error::Malformed);
                }
                message.to = address[..at].to_vec();
                if declared.is_none() {
                    fields.push(("recipientnetaddr".into(), route.join("/").into_bytes()));
                }
            }
            if message.body.starts_with(b"@VIA: ") {
                let end = message
                    .body
                    .iter()
                    .position(|b| *b == b'\n')
                    .ok_or(Error::Malformed)?;
                let via = parse_path(&ascii(message.body[6..end].trim_ascii())?)?;
                if !path.is_empty() && path != via {
                    return Err(Error::Malformed);
                }
                path = via;
                message.body = message.body[end + 1..].to_vec();
            }
        }
        // Network controls are not ordinary text or local commands in this profile.
        if message.body.trim_ascii_start().starts_with(b"@")
            || (message.conference != 0 && get(&fields, "recipientnetaddr")?.is_some())
            || get(&fields, "content-type")?.is_some()
            || get(&fields, "content-transfer-encoding")?.is_some()
            || get(&fields, "fileattach")?.is_some()
        {
            return Err(Error::Unsupported);
        }
        if get(&fields, "whenwritten")?
            .is_some_and(|v| ascii(v).ok().and_then(|v| written_timestamp(&v)).is_none())
        {
            return Err(Error::Unsupported);
        }
        if !valid_native_text(&message, utf8.contains(&member.offset)) {
            return Err(Error::Unsupported);
        }
        result.push(NetworkMessage {
            metadata: Metadata {
                id,
                reply,
                path,
                utf8: utf8.contains(&member.offset),
                written: get(&fields, "whenwritten")?.map(ascii).transpose()?,
                fields,
            },
            message,
            offset: member.offset,
            digest: member.digest,
        });
    }
    if !headers.is_empty() {
        return Err(Error::Malformed);
    }
    Ok(result)
}

/// Returns shared records plus a privacy-minimal metadata sidecar. Unknown/private
/// trace fields are retained at ingress, never automatically republished.
pub fn encode(
    messages: &[NetworkMessage],
    reply_board: Option<&str>,
) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let mut wire = Vec::new();
    let mut utf8 = BTreeSet::new();
    for (ordinal, m) in messages.iter().enumerate() {
        if !valid_message_id(&m.metadata.id)
            || m.metadata
                .reply
                .as_ref()
                .is_some_and(|s| !valid_message_id(s))
            || parse_path(&m.metadata.path.join("/"))? != m.metadata.path
        {
            return Err(Error::Malformed);
        }
        if m.metadata.utf8 {
            utf8.insert(ordinal);
        }
        let mut record = m.message.clone();
        let destination = m.metadata.destination_path()?;
        if !destination.is_empty() {
            if record.conference != 0 || !record.private {
                return Err(Error::Malformed);
            }
            record
                .to
                .extend_from_slice(format!("@{}", destination.join("/")).as_bytes());
        }
        // Metadata profile is mandatory: a safe ASCII placeholder avoids broken
        // UTF-8 prefixes and prevents truncated fallback names from becoming identity.
        for field in [&mut record.to, &mut record.from, &mut record.subject] {
            if field.len() > 25 || (m.metadata.utf8 && !field.is_ascii()) {
                *field = b"See HEADERS.DAT".to_vec();
            }
        }
        wire.push(record);
    }
    let (records, offsets) =
        qwk::encode_records_profiled(&wire, reply_board, Profile::ClassicCp437, &utf8)?;
    let mut headers = Vec::new();
    for (m, offset) in messages.iter().zip(offsets) {
        headers.extend_from_slice(
            format!(
                "[{offset:x}]\nUtf8 = {}\nFormat = fixed\nConference: {}\nMessage-ID: {}\n",
                m.metadata.utf8, m.message.conference, m.metadata.id
            )
            .as_bytes(),
        );
        let mut values = vec![
            ("Sender", m.message.from.clone()),
            ("To", m.message.to.clone()),
            ("Subject", m.message.subject.clone()),
        ];
        let destination = m.metadata.destination_path()?;
        if !destination.is_empty() {
            values[1]
                .1
                .extend_from_slice(format!("@{}", destination.join("/")).as_bytes());
            values.push(("RecipientNetAddr", destination.join("/").into_bytes()));
        }
        if let Some(reply) = &m.metadata.reply {
            values.push(("In-Reply-To", reply.as_bytes().to_vec()));
        }
        if !m.metadata.path.is_empty() {
            values.push(("SenderNetAddr", m.metadata.path.join("/").into_bytes()));
        }
        if let Some(written) = &m.metadata.written {
            values.push(("WhenWritten", written.as_bytes().to_vec()));
        }
        for (key, value) in values {
            if value.len() > 1000 || value.iter().any(|b| *b < 32 || *b == 127) {
                return Err(Error::Unrepresentable);
            }
            headers.extend_from_slice(key.as_bytes());
            headers.extend_from_slice(b": ");
            headers.extend_from_slice(&value);
            headers.push(b'\n');
        }
        headers.push(b'\n');
    }
    Ok((records, headers))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(utf8: bool) -> NetworkMessage {
        NetworkMessage {
            message: Message {
                number: 42,
                conference: 2001,
                reference: 0,
                private: false,
                received: false,
                to: b"All".to_vec(),
                from: b"Peer".to_vec(),
                subject: b"Network subject beyond classic width".to_vec(),
                body: if utf8 {
                    "Unicode π 界\n".as_bytes().to_vec()
                } else {
                    b"Caf\x82 \xdb\n".to_vec()
                },
                wall_time: chrono::NaiveDate::from_ymd_opt(2026, 9, 5)
                    .unwrap()
                    .and_hms_opt(12, 34, 0)
                    .unwrap(),
            },
            metadata: Metadata {
                id: "<test@ORIGIN.qwk>".into(),
                reply: Some("<parent@ORIGIN.qwk>".into()),
                path: vec!["ORIGIN".into()],
                utf8,
                written: Some("20260905T123400Z 0000".into()),
                fields: Vec::new(),
            },
            offset: 0,
            digest: String::new(),
        }
    }
    fn packet(messages: &[NetworkMessage]) -> qwk::Artifact {
        let (records, headers) = encode(messages, Some("HUB")).unwrap();
        qwk::inspect(
            &qwk::archive(&BTreeMap::from([
                ("HUB.MSG".into(), records),
                ("HEADERS.DAT".into(), headers),
            ]))
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn mixed_encoding_long_metadata_and_offset_round_trip() {
        let messages = vec![message(false), message(true)];
        let packet = packet(&messages);
        let decoded = decode(&packet, Some("HUB")).unwrap();
        for (a, b) in messages.iter().zip(decoded) {
            assert_eq!(a.message.body, b.message.body);
            assert_eq!(a.message.subject, b.message.subject);
            assert_eq!(a.metadata.id, b.metadata.id);
            assert_eq!(a.metadata.path, b.metadata.path);
            assert_eq!(a.metadata.utf8, b.metadata.utf8);
        }
    }
    #[test]
    fn exact_offsets_conflicts_paths_and_identity_fail_closed() {
        assert!(parse_path("AA/BB/aa").is_err());
        assert!(parse_path("../PEER").is_err());
        assert!(!valid_system_id("SYSOP"));
        for changed in [
            "[81]\nConference: 2001\nMessage-ID: <test@peer>\n",
            "[80]\nConference: 2002\nMessage-ID: <test@peer>\n",
            "[80]\nConference: 2001\nMessage-ID: <test@peer>\nmessage-id: <other@peer>\n",
        ] {
            let mut p = packet(&[message(false)]);
            p.members
                .insert("HEADERS.DAT".into(), changed.as_bytes().to_vec());
            assert!(decode(&p, Some("HUB")).is_err());
        }
    }
    #[test]
    fn metadata_resource_bounds_and_unknown_evidence() {
        let mut p = packet(&[message(false)]);
        p.members
            .get_mut("HEADERS.DAT")
            .unwrap()
            .extend_from_slice(b"X-Unknown: retained bytes\n");
        let d = decode(&p, Some("HUB")).unwrap();
        assert!(d[0]
            .metadata
            .fields
            .iter()
            .any(|(k, v)| k == "x-unknown" && v == b"retained bytes"));
        p.members
            .get_mut("HEADERS.DAT")
            .unwrap()
            .extend_from_slice(&vec![b'x'; 1025]);
        assert!(decode(&p, Some("HUB")).is_err());
    }
    #[test]
    fn synchronet_network_marker_timestamp_and_advisory_files_are_profile_scoped() {
        let mut p = packet(&[message(false)]);
        p.members.get_mut("HUB.MSG").unwrap()[128 + 127] = b'*';
        assert!(
            qwk::decode_records(&p.members["HUB.MSG"], Some("HUB"), Profile::ClassicCp437).is_err()
        );
        assert_eq!(decode(&p, Some("HUB")).unwrap().len(), 1);
        assert_eq!(
            written_timestamp("20260905170500Z      0000"),
            Some(1788627900)
        );
        assert_eq!(
            written_timestamp("20260905T100500-0700 0000"),
            Some(1788627900)
        );
        assert_eq!(written_timestamp("not-a-time"), None);
        p.members
            .insert("PAYLOAD.EXE".into(), b"untrusted extra member".to_vec());
        assert!(decode(&p, Some("HUB")).is_err());
    }
    #[test]
    fn unicode_controls_conflicting_recipient_and_private_metadata_fail_closed() {
        let mut p = packet(&[message(true)]);
        p.members
            .get_mut("HEADERS.DAT")
            .unwrap()
            .extend_from_slice(b"Recipient: Different\n");
        assert!(decode(&p, Some("HUB")).is_err());
        let mut m = message(true);
        m.message.body = "Text\u{009b} control".as_bytes().to_vec();
        assert!(decode(&packet(&[m]), Some("HUB")).is_err());
        let mut p = packet(&[message(false)]);
        p.members
            .get_mut("HEADERS.DAT")
            .unwrap()
            .extend_from_slice(b"RecipientNetAddr: SOME/REMOTE\n");
        assert!(decode(&p, Some("HUB")).is_err());
    }
    #[test]
    fn private_destination_roundtrip_is_bounded_and_public_profile_rejects_it() {
        let mut m = message(false);
        m.message.private = true;
        m.message.conference = 0;
        m.message.to = b"Mailbox Recipient".to_vec();
        m.metadata.fields = vec![("recipientnetaddr".into(), b"NEXT/DEST".to_vec())];
        let (records, headers) = encode(&[m.clone()], Some("PEER")).unwrap();
        let bytes = qwk::archive(&BTreeMap::from([
            ("PEER.MSG".into(), records),
            ("HEADERS.DAT".into(), headers),
        ]))
        .unwrap();
        let result = decode(&qwk::inspect(&bytes).unwrap(), Some("PEER")).unwrap();
        assert_eq!(result[0].message.to, m.message.to);
        assert_eq!(
            result[0].metadata.destination_path().unwrap(),
            vec!["NEXT", "DEST"]
        );
        let mut legacy = m.clone();
        legacy.message.body = b"Other Recipient@NEXT/DEST\nPrivate body\n".to_vec();
        let (mut records, headers) = encode(&[legacy], Some("PEER")).unwrap();
        records[qwk::RECORD + 21..qwk::RECORD + 46].fill(b' ');
        records[qwk::RECORD + 21..qwk::RECORD + 28].copy_from_slice(b"NETMAIL");
        let bytes = qwk::archive(&BTreeMap::from([
            ("PEER.MSG".into(), records),
            ("HEADERS.DAT".into(), headers),
        ]))
        .unwrap();
        assert!(decode(&qwk::inspect(&bytes).unwrap(), Some("PEER")).is_err());
        m.message.conference = 1;
        assert!(encode(&[m.clone()], Some("PEER")).is_err());
        m.message.conference = 0;
        m.metadata.fields = vec![("recipientnetaddr".into(), b"NEXT/NEXT".to_vec())];
        assert!(encode(&[m.clone()], Some("PEER")).is_err());
        m.metadata.fields = vec![("recipientnetaddr".into(), b"2:123/456".to_vec())];
        assert!(encode(&[m], Some("PEER")).is_err());
    }
}
