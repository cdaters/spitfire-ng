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

//! QWK classic CP437 and the QWKE long-header subset, independently implemented
//! from Lee 1.6, Herring 2.0 and Rocca 1.02. Offsets here are zero based.
use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
};
use thiserror::Error;

pub const RECORD: usize = 128;
pub const MAX_ARCHIVE: usize = 16 * 1024 * 1024;
pub const MAX_EXPANDED: usize = 64 * 1024 * 1024;
pub const MAX_MEMBERS: usize = 1024;
pub const MAX_MESSAGES: usize = 1000;
pub const MAX_BODY: usize = 64 * 1024;
pub const MAX_CONTROL: usize = 256 * 1024;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid QWK archive or record framing")]
    Malformed,
    #[error("QWK resource limit exceeded")]
    Limit,
    #[error("unsupported QWK metadata or text")]
    Unsupported,
    #[error("QWK field cannot be represented without loss")]
    Unrepresentable,
    #[error("QWK board identity does not match")]
    BoardMismatch,
    #[error("invalid QWK date or time")]
    Date,
    #[error("QWK archive codec failed")]
    Zip(#[from] zip::result::ZipError),
    #[error("QWK artifact I/O failed")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    ClassicCp437,
    ExtendedCp437,
}

/// Asserted wire information only; authenticated ingress supplies native authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub number: u32,
    pub conference: u16,
    pub reference: u32,
    pub private: bool,
    pub received: bool,
    pub to: Vec<u8>,
    pub from: Vec<u8>,
    pub subject: Vec<u8>,
    pub body: Vec<u8>,
    /// No UTC assertion: classic QWK carries minute-precision wall-clock time.
    pub wall_time: NaiveDateTime,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub ordinal: usize,
    pub offset: usize,
    pub digest: String,
    pub message: Message,
}

#[derive(Clone, Debug)]
pub struct Control {
    pub board_id: String,
    pub board_name: Vec<u8>,
    pub caller: Vec<u8>,
    pub created: NaiveDateTime,
    pub conferences: Vec<(u16, Vec<u8>)>,
}

/// Exact uncompressed evidence. Names have no filesystem interpretation.
#[derive(Debug)]
pub struct Artifact {
    pub members: BTreeMap<String, Vec<u8>>,
    pub digest: String,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn valid_board_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 8
        && id
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && safe_name(&format!("{id}.MSG"))
}

fn safe_name(name: &str) -> bool {
    let mut parts = name.split('.');
    let base = parts.next().unwrap_or_default();
    let ext = parts.next().unwrap_or_default();
    !base.is_empty()
        && base.len() <= 8
        && !ext.is_empty()
        && ext.len() <= 3
        && parts.next().is_none()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        && !matches!(base, "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        && !(base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base.as_bytes()[3].is_ascii_digit())
}
fn u16_at(b: &[u8], n: usize) -> Result<usize, Error> {
    let s = b.get(n..n + 2).ok_or(Error::Malformed)?;
    Ok(u16::from_le_bytes([s[0], s[1]]) as usize)
}
fn u32_at(b: &[u8], n: usize) -> Result<usize, Error> {
    let s = b.get(n..n + 4).ok_or(Error::Malformed)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

/// Validate the directory before ZipArchive allocates it. ZIP64/multi-disk,
/// executable prefixes and nested paths are outside this bounded ZIP32 profile.
/// At most one XMODEM block's CP/M padding may follow the EOCD.
fn preflight(bytes: &[u8]) -> Result<(usize, usize), Error> {
    if bytes.len() > MAX_ARCHIVE + 1023 {
        return Err(Error::Limit);
    }
    let start = bytes.len().saturating_sub(65_557 + 1023);
    let end = (start..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            bytes.get(i..i + 4) == Some(b"PK\x05\x06")
                && u16_at(bytes, i + 20).is_ok_and(|n| {
                    let tail = i + 22 + n;
                    tail <= bytes.len()
                        && bytes.len() - tail <= 1023
                        && bytes[tail..].iter().all(|b| *b == 0x1a)
                })
        })
        .ok_or(Error::Malformed)?;
    let length = end + 22 + u16_at(bytes, end + 20)?;
    if length > MAX_ARCHIVE {
        return Err(Error::Limit);
    }
    let count = u16_at(bytes, end + 10)?;
    if count == 0 || count > MAX_MEMBERS {
        return Err(Error::Limit);
    }
    if u16_at(bytes, end + 4)? != 0
        || u16_at(bytes, end + 6)? != 0
        || u16_at(bytes, end + 8)? != count
    {
        return Err(Error::Malformed);
    }
    let directory = u32_at(bytes, end + 16)?;
    if directory.checked_add(u32_at(bytes, end + 12)?) != Some(end) {
        return Err(Error::Malformed);
    }
    let mut pos = directory;
    let mut names = BTreeSet::new();
    let mut regions = Vec::new();
    let mut expanded = 0usize;
    for _ in 0..count {
        if bytes.get(pos..pos + 4) != Some(b"PK\x01\x02") {
            return Err(Error::Malformed);
        }
        let name_len = u16_at(bytes, pos + 28)?;
        let extra_len = u16_at(bytes, pos + 30)?;
        let comment_len = u16_at(bytes, pos + 32)?;
        if name_len > 12 || extra_len > 4096 || comment_len > 1024 {
            return Err(Error::Limit);
        }
        let raw = bytes
            .get(pos + 46..pos + 46 + name_len)
            .ok_or(Error::Malformed)?;
        let name = std::str::from_utf8(raw)
            .map_err(|_| Error::Malformed)?
            .to_ascii_uppercase();
        if !safe_name(&name) || !names.insert(name) {
            return Err(Error::Malformed);
        }
        let flags = u16_at(bytes, pos + 8)?;
        let method = u16_at(bytes, pos + 10)?;
        if flags & !0x080e != 0 || !matches!(method, 0 | 8) {
            return Err(Error::Unsupported);
        }
        if u16_at(bytes, pos + 34)? != 0 {
            return Err(Error::Unsupported);
        }
        let mode = u32_at(bytes, pos + 38)? >> 16;
        if mode & 0xf000 != 0 && mode & 0xf000 != 0x8000 {
            return Err(Error::Unsupported);
        }
        if u32_at(bytes, pos + 38)? & 0x18 != 0 {
            return Err(Error::Unsupported);
        }
        let compressed = u32_at(bytes, pos + 20)?;
        let size = u32_at(bytes, pos + 24)?;
        expanded = expanded.checked_add(size).ok_or(Error::Limit)?;
        if expanded > MAX_EXPANDED || size > compressed.saturating_mul(100) {
            return Err(Error::Limit);
        }
        let local = u32_at(bytes, pos + 42)?;
        if local >= directory {
            return Err(Error::Malformed);
        }
        if bytes.get(local..local + 4) != Some(b"PK\x03\x04")
            || u16_at(bytes, local + 6)? != flags
            || u16_at(bytes, local + 8)? != method
            || u16_at(bytes, local + 26)? != name_len
        {
            return Err(Error::Malformed);
        }
        let local_extra = u16_at(bytes, local + 28)?;
        if local_extra > 4096 {
            return Err(Error::Limit);
        }
        if bytes.get(local + 30..local + 30 + name_len) != Some(raw) {
            return Err(Error::Malformed);
        }
        if flags & 8 == 0
            && (u32_at(bytes, local + 14)? != u32_at(bytes, pos + 16)?
                || u32_at(bytes, local + 18)? != compressed
                || u32_at(bytes, local + 22)? != size)
        {
            return Err(Error::Malformed);
        }
        let data_end = local
            .checked_add(30 + name_len + local_extra)
            .and_then(|n| n.checked_add(compressed))
            .ok_or(Error::Limit)?;
        if data_end > directory {
            return Err(Error::Malformed);
        }
        let mut region_end = data_end;
        if flags & 8 != 0 {
            let d = if bytes.get(data_end..data_end + 4) == Some(b"PK\x07\x08") {
                data_end + 4
            } else {
                data_end
            };
            if u32_at(bytes, d)? != u32_at(bytes, pos + 16)?
                || u32_at(bytes, d + 4)? != compressed
                || u32_at(bytes, d + 8)? != size
            {
                return Err(Error::Malformed);
            }
            region_end = d + 12;
            if region_end > directory {
                return Err(Error::Malformed);
            }
        }
        regions.push((local, region_end));
        pos = pos
            .checked_add(46 + name_len + extra_len + comment_len)
            .ok_or(Error::Limit)?;
        if pos > end {
            return Err(Error::Malformed);
        }
    }
    if pos != end {
        return Err(Error::Malformed);
    }
    regions.sort_unstable();
    if regions[0].0 != 0 || regions.windows(2).any(|w| w[0].1 > w[1].0) {
        return Err(Error::Malformed);
    }
    Ok((length, count))
}

pub fn inspect(bytes: &[u8]) -> Result<Artifact, Error> {
    let (length, count) = preflight(bytes)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes[..length]))?;
    if archive.len() != count || archive.has_overlapping_files()? {
        return Err(Error::Malformed);
    }
    let mut members = BTreeMap::new();
    let mut total = 0usize;
    for i in 0..count {
        let mut f = archive.by_index(i)?;
        let size = usize::try_from(f.size()).map_err(|_| Error::Limit)?;
        let name = f.name().to_ascii_uppercase();
        if (name == "CONTROL.DAT" || name.ends_with(".EXT") || name.ends_with(".LMR"))
            && size > MAX_CONTROL
        {
            return Err(Error::Limit);
        }
        let mut value = Vec::with_capacity(size.min(MAX_EXPANDED));
        (&mut f).take(size as u64 + 1).read_to_end(&mut value)?;
        if value.len() != size {
            return Err(Error::Malformed);
        }
        total = total.checked_add(value.len()).ok_or(Error::Limit)?;
        if total > MAX_EXPANDED {
            return Err(Error::Limit);
        }
        members.insert(name, value);
    }
    let mut hash = Sha256::new();
    for (name, value) in &members {
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
    }
    Ok(Artifact {
        members,
        digest: format!("{:x}", hash.finalize()),
    })
}

pub fn archive(members: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, Error> {
    if members.is_empty()
        || members.len() > MAX_MEMBERS
        || members.values().map(Vec::len).sum::<usize>() > MAX_EXPANDED
    {
        return Err(Error::Limit);
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    // Stored avoids generating packets that violate our own expansion-ratio policy.
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o600);
    for (name, data) in members {
        if !safe_name(name) {
            return Err(Error::Malformed);
        }
        writer.start_file(name, options)?;
        writer.write_all(data)?;
        if writer
            .get_ref()
            .is_none_or(|c| c.get_ref().len() > MAX_ARCHIVE)
        {
            return Err(Error::Limit);
        }
    }
    let bytes = writer.finish()?.into_inner();
    if bytes.len() > MAX_ARCHIVE {
        return Err(Error::Limit);
    }
    Ok(bytes)
}
fn field(bytes: &[u8]) -> Vec<u8> {
    let n = bytes
        .iter()
        .rposition(|b| *b != b' ' && *b != 0)
        .map_or(0, |n| n + 1);
    bytes[..n].to_vec()
}
fn number(bytes: &[u8], blank: bool) -> Result<u32, Error> {
    let s = std::str::from_utf8(bytes)
        .map_err(|_| Error::Malformed)?
        .trim();
    if s.is_empty() && blank {
        return Ok(0);
    }
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Malformed);
    }
    s.parse().map_err(|_| Error::Malformed)
}
fn text_ok(bytes: &[u8]) -> bool {
    bytes.iter().all(|b| *b >= 32 && *b != 127 && *b != 0xe3)
}
fn put(dst: &mut [u8], value: &[u8]) -> Result<(), Error> {
    if value.len() > dst.len() || !text_ok(value) {
        return Err(Error::Unrepresentable);
    }
    dst.fill(b' ');
    dst[..value.len()].copy_from_slice(value);
    Ok(())
}
fn date(bytes: &[u8]) -> Result<NaiveDateTime, Error> {
    if bytes.len() != 13 || bytes[2] != b'-' || bytes[5] != b'-' || bytes[10] != b':' {
        return Err(Error::Date);
    }
    let month = number(&bytes[..2], false)?;
    let day = number(&bytes[3..5], false)?;
    let year = number(&bytes[6..8], false)?;
    let year = if year >= 80 { 1900 + year } else { 2000 + year };
    chrono::NaiveDate::from_ymd_opt(year as i32, month, day)
        .and_then(|d| {
            d.and_hms_opt(
                number(&bytes[8..10], false).ok()?,
                number(&bytes[11..13], false).ok()?,
                0,
            )
        })
        .ok_or(Error::Date)
}

pub fn decode_records(
    bytes: &[u8],
    reply_board: Option<&str>,
    profile: Profile,
) -> Result<Vec<Member>, Error> {
    if bytes.len() < RECORD || !bytes.len().is_multiple_of(RECORD) {
        return Err(Error::Malformed);
    }
    if bytes.len() > MAX_EXPANDED {
        return Err(Error::Limit);
    }
    if let Some(board) = reply_board {
        if !valid_board_id(board) || field(&bytes[..RECORD]) != board.as_bytes() {
            return Err(Error::BoardMismatch);
        }
    }
    let mut members = Vec::new();
    let mut offset = RECORD;
    while offset < bytes.len() {
        if members.len() >= MAX_MESSAGES {
            return Err(Error::Limit);
        }
        let h = bytes.get(offset..offset + RECORD).ok_or(Error::Malformed)?;
        let blocks = number(&h[116..122], false)? as usize;
        if blocks < 2 {
            return Err(Error::Malformed);
        }
        let size = blocks.checked_mul(RECORD).ok_or(Error::Limit)?;
        if size > MAX_BODY + 1024 {
            return Err(Error::Limit);
        }
        let raw = bytes.get(offset..offset + size).ok_or(Error::Malformed)?;
        if h[122] != 225 || !matches!(h[0], b' ' | b'-' | b'*' | b'+') || h[127] != b' ' {
            return Err(Error::Unsupported);
        }
        if h[96..108].iter().any(|b| *b != b' ' && *b != 0) {
            return Err(Error::Unsupported);
        }
        let conference = u16::from_le_bytes([h[123], h[124]]);
        let n = number(&h[1..8], false)?;
        if reply_board.is_some() && n != u32::from(conference) {
            return Err(Error::Malformed);
        }
        let mut msg = Message {
            number: n,
            conference,
            reference: number(&h[108..116], true)?,
            private: matches!(h[0], b'*' | b'+'),
            received: matches!(h[0], b'-' | b'+'),
            to: field(&h[21..46]),
            from: field(&h[46..71]),
            subject: field(&h[71..96]),
            body: Vec::new(),
            wall_time: date(&h[8..21])?,
        };
        if [&msg.to, &msg.from, &msg.subject]
            .iter()
            .any(|v| !text_ok(v))
        {
            return Err(Error::Unsupported);
        }
        let text = field(&raw[RECORD..]);
        let mut body = Vec::with_capacity(text.len());
        let mut last_cr = false;
        for b in text {
            match b {
                0xe3 | b'\r' => {
                    body.push(b'\n');
                    last_cr = b == b'\r';
                }
                b'\n' => {
                    if !last_cr {
                        body.push(b'\n')
                    }
                    last_cr = false;
                }
                b'\t' => {
                    body.push(b'\t');
                    last_cr = false;
                }
                b if b >= 32 && b != 127 => {
                    body.push(b);
                    last_cr = false;
                }
                _ => return Err(Error::Unsupported),
            }
        }
        if profile == Profile::ExtendedCp437 {
            let mut consumed = 0;
            let mut keys = BTreeSet::new();
            for line in body.split_inclusive(|b| *b == b'\n') {
                let line_text = line.strip_suffix(b"\n").unwrap_or(line);
                let key = [b"To:".as_slice(), b"From:", b"Subject:"]
                    .iter()
                    .position(|k| {
                        line_text
                            .get(..k.len())
                            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(k))
                    });
                let Some(key) = key else {
                    if !keys.is_empty() && line_text.is_empty() {
                        consumed += line.len()
                    }
                    break;
                };
                if !keys.insert(key) || line.len() > 1024 {
                    return Err(Error::Unsupported);
                }
                let prefix = [3, 5, 8][key];
                let value = line_text[prefix..]
                    .strip_prefix(b" ")
                    .unwrap_or(&line_text[prefix..]);
                if !text_ok(value) {
                    return Err(Error::Unsupported);
                }
                match key {
                    0 => msg.to = value.to_vec(),
                    1 => msg.from = value.to_vec(),
                    _ => msg.subject = value.to_vec(),
                };
                consumed += line.len();
            }
            body.drain(..consumed);
        }
        if body.len() > MAX_BODY
            || msg.subject.len() > 72
            || msg.to.len() > 64
            || msg.from.len() > 64
        {
            return Err(Error::Limit);
        }
        msg.body = body;
        members.push(Member {
            ordinal: members.len(),
            offset,
            digest: digest(raw),
            message: msg,
        });
        offset += size;
    }
    Ok(members)
}

pub fn encode_records(
    messages: &[Message],
    reply_board: Option<&str>,
    profile: Profile,
) -> Result<(Vec<u8>, Vec<usize>), Error> {
    if messages.len() > MAX_MESSAGES {
        return Err(Error::Limit);
    }
    let mut result = vec![b' '; RECORD];
    let banner = reply_board.unwrap_or("Produced by SPITFIRE NG");
    if reply_board.is_some_and(|id| !valid_board_id(id)) {
        return Err(Error::BoardMismatch);
    }
    put(&mut result, banner.as_bytes())?;
    let mut offsets = Vec::new();
    for (ordinal, m) in messages.iter().enumerate() {
        if m.body.len() > MAX_BODY {
            return Err(Error::Limit);
        }
        let mut body = Vec::new();
        for (label, value) in [("To", &m.to), ("From", &m.from), ("Subject", &m.subject)] {
            if !text_ok(value) {
                return Err(Error::Unrepresentable);
            }
            if value.len() > 25 {
                if profile == Profile::ClassicCp437 {
                    return Err(Error::Unrepresentable);
                }
                body.extend_from_slice(label.as_bytes());
                body.extend_from_slice(b": ");
                body.extend_from_slice(value);
                body.push(0xe3);
            }
        }
        // QWKE readers can consume leading header-like body lines even after
        // the extension separator. No agreed lossless escape exists in this
        // profile; hold export rather than misattribute native text as metadata.
        let visible_body = m.body.trim_ascii_start();
        if profile == Profile::ExtendedCp437
            && [
                b"To:".as_slice(),
                b"From:",
                b"Subject:",
                b"Title:",
                b"@Subject:",
            ]
            .iter()
            .any(|key| {
                visible_body
                    .get(..key.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key))
            })
        {
            return Err(Error::Unrepresentable);
        }
        if !body.is_empty() {
            body.push(0xe3)
        }
        let mut last_cr = false;
        for &b in &m.body {
            match b {
                0xe3 => return Err(Error::Unrepresentable),
                b'\r' => {
                    body.push(0xe3);
                    last_cr = true;
                }
                b'\n' => {
                    if !last_cr {
                        body.push(0xe3)
                    }
                    last_cr = false;
                }
                b'\t' => {
                    body.push(b);
                    last_cr = false;
                }
                b if b >= 32 && b != 127 => {
                    body.push(b);
                    last_cr = false;
                }
                _ => return Err(Error::Unrepresentable),
            }
        }
        if body.last() != Some(&0xe3) {
            body.push(0xe3)
        }
        let blocks = 1 + body.len().div_ceil(RECORD);
        let mut header = [b' '; RECORD];
        header[0] = match (m.private, m.received) {
            (false, false) => b' ',
            (false, true) => b'-',
            (true, false) => b'*',
            (true, true) => b'+',
        };
        let n = if reply_board.is_some() {
            u32::from(m.conference)
        } else {
            m.number
        };
        put(&mut header[1..8], n.to_string().as_bytes())?;
        use chrono::Datelike;
        if !(1980..=2079).contains(&m.wall_time.year()) {
            return Err(Error::Date);
        }
        put(
            &mut header[8..21],
            m.wall_time.format("%m-%d-%y%H:%M").to_string().as_bytes(),
        )?;
        for (range, value) in [(21..46, &m.to), (46..71, &m.from), (71..96, &m.subject)] {
            put(&mut header[range], &value[..value.len().min(25)])?;
        }
        put(&mut header[108..116], m.reference.to_string().as_bytes())?;
        put(&mut header[116..122], blocks.to_string().as_bytes())?;
        header[122] = 225;
        header[123..125].copy_from_slice(&m.conference.to_le_bytes());
        header[125..127].copy_from_slice(&((ordinal + 1) as u16).to_le_bytes());
        offsets.push(result.len());
        result.extend_from_slice(&header);
        result.extend_from_slice(&body);
        result.resize(result.len().div_ceil(RECORD) * RECORD, b' ');
        if result.len() > MAX_EXPANDED {
            return Err(Error::Limit);
        }
    }
    Ok((result, offsets))
}

pub fn control(c: &Control, count: usize, profile: Profile) -> Result<Vec<u8>, Error> {
    if !valid_board_id(&c.board_id) || c.conferences.is_empty() || c.conferences.len() > 784 {
        return Err(Error::Malformed);
    }
    let mut lines = vec![
        c.board_name.clone(),
        Vec::new(),
        Vec::new(),
        b"Sysop".to_vec(),
        format!("0,{}", c.board_id).into_bytes(),
        c.created
            .format("%m-%d-%Y,%H:%M:%S")
            .to_string()
            .into_bytes(),
        c.caller.clone(),
        Vec::new(),
        b"0".to_vec(),
        count.to_string().into_bytes(),
        (c.conferences.len() - 1).to_string().into_bytes(),
    ];
    let mut seen = BTreeSet::new();
    for (n, name) in &c.conferences {
        if !seen.insert(n)
            || name.len()
                > if profile == Profile::ClassicCp437 {
                    13
                } else {
                    255
                }
        {
            return Err(Error::Unrepresentable);
        }
        lines.push(n.to_string().into_bytes());
        lines.push(name.clone());
    }
    lines.extend([Vec::new(), Vec::new(), Vec::new()]);
    let mut result = Vec::new();
    for line in lines {
        if line.len() > 255 || !text_ok(&line) {
            return Err(Error::Unrepresentable);
        }
        result.extend(line);
        result.extend(b"\r\n");
    }
    if result.len() > MAX_CONTROL {
        return Err(Error::Limit);
    }
    Ok(result)
}

/// Integer record positions represented directly in Microsoft binary format.
/// The 23-bit significand is exact for this profile's bounded record numbers.
pub fn index_record(offset: usize, conference: u16) -> Result<[u8; 5], Error> {
    if !offset.is_multiple_of(RECORD) || !(RECORD..=MAX_EXPANDED).contains(&offset) {
        return Err(Error::Malformed);
    }
    let n = (offset / RECORD + 1) as u32;
    let exponent = 31 - n.leading_zeros();
    let mantissa = (n << (23 - exponent)) & 0x7fffff;
    Ok([
        mantissa as u8,
        (mantissa >> 8) as u8,
        (mantissa >> 16) as u8,
        (exponent + 129) as u8,
        conference as u8,
    ])
}

/// Check Microsoft binary pointers against the decoded header directory. The
/// fifth index byte is the low conference byte, not a native conference ID.
pub fn validate_indexes(
    files: &BTreeMap<String, Vec<u8>>,
    headers: &[(usize, u16)],
) -> Result<(), Error> {
    if headers.len() > MAX_MESSAGES {
        return Err(Error::Limit);
    }
    let mut allowed = BTreeMap::new();
    for &(offset, conference) in headers {
        let record = index_record(offset, conference)?;
        if allowed.insert(record[..4].to_vec(), conference).is_some() {
            return Err(Error::Malformed);
        }
    }
    for (name, bytes) in files.iter().filter(|(name, _)| name.ends_with(".NDX")) {
        if !bytes.len().is_multiple_of(5) || bytes.len() / 5 > MAX_MESSAGES {
            return Err(Error::Malformed);
        }
        let conference = if name == "PERSONAL.NDX" {
            None
        } else {
            Some(
                name.strip_suffix(".NDX")
                    .ok_or(Error::Malformed)?
                    .parse::<u16>()
                    .map_err(|_| Error::Malformed)?,
            )
        };
        let mut seen = BTreeSet::new();
        for record in bytes.as_chunks::<5>().0 {
            let actual = allowed.get(&record[..4]).ok_or(Error::Malformed)?;
            if record[4] != *actual as u8
                || conference.is_some_and(|c| c != *actual)
                || !seen.insert(record[..4].to_vec())
            {
                return Err(Error::Malformed);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message() -> Message {
        Message {
            number: 17,
            conference: 513,
            reference: 9,
            private: true,
            received: false,
            to: b"READER".to_vec(),
            from: b"AUTHOR".to_vec(),
            subject: b"CP437 check".to_vec(),
            body: b"Caf\x82 \xb3 \xdb\r\nSecond line\r\n".to_vec(),
            wall_time: chrono::NaiveDate::from_ymd_opt(2026, 9, 5)
                .unwrap()
                .and_hms_opt(10, 30, 0)
                .unwrap(),
        }
    }
    #[test]
    fn ambiguous_leading_body_headers_are_held_before_reader_misattribution() {
        for body in [
            b"From: Pretend Sysop\nOriginal text".as_slice(),
            b"sUbJeCt: misleading\nOriginal text",
            b"To: Wrong recipient\nOriginal text",
            b"Title: literal text",
            b"@Subject: literal text",
            b"\n\r\n  From: Pretend Sysop",
        ] {
            let mut m = message();
            m.to = b"ALL".to_vec();
            m.from = b"PUBLIC HANDLE".to_vec();
            m.subject = b"Canonical subject".to_vec();
            m.body = body.to_vec();
            assert!(matches!(
                encode_records(&[m], None, Profile::ExtendedCp437),
                Err(Error::Unrepresentable)
            ));
        }
        let mut m = message();
        m.body = b"Ordinary introduction\nFrom: literal body line\n".to_vec();
        let (records, _) =
            encode_records(std::slice::from_ref(&m), None, Profile::ExtendedCp437).unwrap();
        assert_eq!(
            decode_records(&records, None, Profile::ExtendedCp437).unwrap()[0].message,
            m
        );
    }
    #[test]
    fn index_pointers_must_name_actual_headers() {
        let valid = index_record(128, 513).unwrap().to_vec();
        let mut files = BTreeMap::from([("513.NDX".into(), valid)]);
        validate_indexes(&files, &[(128, 513)]).unwrap();
        assert!(validate_indexes(&files, &[(256, 513)]).is_err());
        files.get_mut("513.NDX").unwrap()[4] = 0;
        assert!(validate_indexes(&files, &[(128, 513)]).is_err());
        files.insert("513.NDX".into(), vec![0; 4]);
        assert!(validate_indexes(&files, &[(128, 513)]).is_err());
    }
    #[test]
    fn records_offsets_cp437_and_mbf() {
        let m = message();
        let (b, o) = encode_records(std::slice::from_ref(&m), None, Profile::ClassicCp437).unwrap();
        assert_eq!(o, [128]);
        assert_eq!(&b[128 + 123..128 + 125], &513u16.to_le_bytes());
        assert_eq!(&b[129..136], b"17     ");
        assert_eq!(index_record(128, 513).unwrap(), [0, 0, 0, 130, 1]);
        let d = decode_records(&b, None, Profile::ClassicCp437).unwrap();
        assert_eq!(d[0].message.body, b"Caf\x82 \xb3 \xdb\nSecond line\n");
        assert_eq!(d[0].message.wall_time, m.wall_time);
        assert!(d[0].message.private);
    }
    #[test]
    fn control_conference_count_and_crlf() {
        let c = Control {
            board_id: "TEST".into(),
            board_name: b"Board".to_vec(),
            caller: b"HANDLE".to_vec(),
            created: message().wall_time,
            conferences: vec![(1, b"General".to_vec()), (513, b"Second".to_vec())],
        };
        let b = control(&c, 7, Profile::ClassicCp437).unwrap();
        let s = String::from_utf8(b).unwrap();
        let l = s.split("\r\n").collect::<Vec<_>>();
        assert_eq!(l[4], "0,TEST");
        assert_eq!(l[9], "7");
        assert_eq!(l[10], "1");
        assert_eq!(&l[11..15], ["1", "General", "513", "Second"]);
    }
    #[test]
    fn extended_headers_and_classic_refusal() {
        let mut m = message();
        m.subject = b"Subject longer than twenty five bytes".to_vec();
        m.from = b"Public Handle Longer Than Twenty Five".to_vec();
        assert!(matches!(
            encode_records(std::slice::from_ref(&m), None, Profile::ClassicCp437),
            Err(Error::Unrepresentable)
        ));
        let (b, _) = encode_records(&[m.clone()], Some("TEST"), Profile::ExtendedCp437).unwrap();
        let d = decode_records(&b, Some("TEST"), Profile::ExtendedCp437).unwrap();
        assert_eq!(d[0].message.subject, m.subject);
        assert_eq!(d[0].message.from, m.from);
        assert_eq!(d[0].message.number, 513);
        assert!(decode_records(&b, Some("OTHER"), Profile::ExtendedCp437).is_err());
    }
    #[test]
    fn malformed_records_never_panic() {
        let (b, _) = encode_records(&[message()], Some("TEST"), Profile::ClassicCp437).unwrap();
        for n in 0..b.len() {
            assert!(
                decode_records(&b[..n], Some("TEST"), Profile::ClassicCp437).is_err() || n == 128
            )
        }
        for (offset, value) in [(244, b'0'), (136, b'9'), (251, 0), (250, 226)] {
            let mut bad = b.clone();
            bad[offset] = value;
            assert!(
                decode_records(&bad, Some("TEST"), Profile::ClassicCp437).is_err(),
                "{offset}"
            );
        }
    }
    #[test]
    fn pi_controls_and_size_refused() {
        for body in [
            vec![0xe3],
            vec![27, b'[', b'2', b'J'],
            vec![b'x'; MAX_BODY + 1],
        ] {
            let mut m = message();
            m.body = body;
            assert!(encode_records(&[m], None, Profile::ClassicCp437).is_err());
        }
        let mut m = message();
        m.body = vec![b'x'; MAX_BODY];
        let (b, _) = encode_records(&[m], None, Profile::ClassicCp437).unwrap();
        assert_eq!(b.len() % 128, 0);
    }
    #[test]
    fn archive_roundtrip_repacking_and_xmodem_padding() {
        let entries = BTreeMap::from([
            ("TEST.MSG".into(), vec![b'a'; 300]),
            ("TEST.LMR".into(), vec![0; 8]),
        ]);
        let b = archive(&entries).unwrap();
        let a = inspect(&b).unwrap();
        assert_eq!(a.members, entries);
        let mut padded = b.clone();
        padded.extend([0x1a; 127]);
        assert_eq!(inspect(&padded).unwrap().digest, a.digest);
        padded.push(0);
        assert!(inspect(&padded).is_err());
        for n in 0..b.len() {
            assert!(inspect(&b[..n]).is_err());
        }
    }
    #[test]
    fn archive_hostile_paths_devices_duplicates_bombs() {
        for name in ["../X.MSG", "/X.MSG", "C:\\X.MSG", "CON.MSG", "dir/X.MSG"] {
            assert!(archive(&BTreeMap::from([(name.into(), vec![1])])).is_err());
        }
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "TEST.MSG",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(&vec![0; 100000]).unwrap();
        let bomb = writer.finish().unwrap().into_inner();
        assert!(matches!(inspect(&bomb), Err(Error::Limit)));
        let b = archive(&BTreeMap::from([("TEST.MSG".into(), vec![1; 200])])).unwrap();
        let mut local = b.clone();
        local[30] = b'X';
        assert!(inspect(&local).is_err());
        let mut corrupt = b;
        corrupt[40] ^= 0x80;
        assert!(inspect(&corrupt).is_err());
    }
    #[test]
    fn calendar_and_no_timezone_guess() {
        let mut m = message();
        m.wall_time = chrono::NaiveDate::from_ymd_opt(2000, 2, 29)
            .unwrap()
            .and_hms_opt(1, 2, 0)
            .unwrap();
        let (b, _) = encode_records(std::slice::from_ref(&m), None, Profile::ClassicCp437).unwrap();
        assert_eq!(
            decode_records(&b, None, Profile::ClassicCp437).unwrap()[0]
                .message
                .wall_time,
            m.wall_time
        );
        let mut bad = b;
        bad[136..144].copy_from_slice(b"02-29-01");
        assert!(decode_records(&bad, None, Profile::ClassicCp437).is_err());
    }
}
