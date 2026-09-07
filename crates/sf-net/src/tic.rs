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

//! Bounded FTS-5006.001 TIC and conservative request-file codecs.
//! Parsing grants no trust and never selects a filesystem path.
use crate::ftn::{Charset, Domain, Endpoint};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_TIC: usize = 32 * 1024;
pub const MAX_LINES: usize = 256;
pub const MAX_LINE: usize = 512;
pub const MAX_PAYLOAD: u64 = crate::binkp::MAX_FILE;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("malformed TIC metadata")]
    Malformed,
    #[error("file-network resource bound exceeded")]
    Limit,
    #[error("unsafe transfer filename")]
    Filename,
    #[error("unsupported file-network encoding")]
    Encoding,
    #[error("file size mismatch")]
    Size,
    #[error("file checksum mismatch")]
    Checksum,
}

/// Secret-bearing parsed envelope. Deliberately not serializable or clonable.
pub struct Envelope {
    pub metadata: Metadata,
    pub direct_hatch: Option<DirectHatchHistory>,
    password: String,
}
/// Received omissions, never invented remote history or an authentication grant.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DirectHatchHistory {
    pub untimed_path: Option<Endpoint>,
    pub empty_seenby: bool,
}
impl std::fmt::Debug for Envelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TIC envelope (private)")
    }
}
impl Envelope {
    pub fn authenticates(&self, verify: impl FnOnce(&str) -> bool) -> bool {
        !self.password.is_empty() && verify(&self.password)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hop {
    pub address: Endpoint,
    pub time: u64,
    pub detail: String,
}
/// Private provenance: opaque fields/descriptions must not enter diagnostics.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub area: String,
    pub file: String,
    pub long_name: Option<String>,
    pub origin: Endpoint,
    pub from: Endpoint,
    pub to: Option<Endpoint>,
    pub size: u64,
    pub crc: u32,
    pub descriptions: Vec<String>,
    pub long_descriptions: Vec<String>,
    pub path: Vec<Hop>,
    pub seen: Vec<Endpoint>,
    pub opaque: Vec<String>,
}
impl std::fmt::Debug for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TIC metadata (private provenance)")
    }
}

/// Portable, deliberately conservative DOS 8.3 subset, with Windows devices
/// excluded on every platform. This result is protocol data, never a host path.
pub fn filename(name: &str) -> Result<String, Error> {
    if name.is_empty()
        || name.len() > 12
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
    {
        return Err(Error::Filename);
    }
    let upper = name.to_ascii_uppercase();
    let parts: Vec<_> = upper.split('.').collect();
    let stem = parts[0];
    if parts.len() > 2
        || stem.is_empty()
        || stem.len() > 8
        || parts.get(1).is_some_and(|p| p.is_empty() || p.len() > 3)
        || matches!(stem, "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(Error::Filename);
    }
    Ok(upper)
}
pub fn area(tag: &str) -> Result<String, Error> {
    if tag.is_empty()
        || tag.len() > 64
        || !tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(Error::Malformed);
    }
    Ok(tag.to_ascii_uppercase())
}
fn endpoint(value: &str, domain: &Domain) -> Result<Endpoint, Error> {
    let qualified = if value.contains('@') {
        value.to_owned()
    } else {
        format!("{value}@{domain}")
    };
    let result: Endpoint = qualified.parse().map_err(|_| Error::Malformed)?;
    if &result.domain != domain {
        return Err(Error::Malformed);
    }
    Ok(result)
}
fn decimal(value: &str) -> Result<u64, Error> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Malformed);
    }
    value.parse().map_err(|_| Error::Malformed)
}
fn safe_text(value: &str) -> bool {
    !value.chars().any(char::is_control) && value.len() <= MAX_LINE
}
fn long_name(value: &str) -> Result<String, Error> {
    // Metadata only, but never preserve path-looking names in this field.
    if value.is_empty()
        || value.len() > 64
        || !safe_text(value)
        || value.starts_with('.')
        || value.ends_with(['.', ' '])
        || value.chars().any(|c| "/\\:*?<>|\"$`".contains(c))
    {
        return Err(Error::Filename);
    }
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(Error::Filename);
    }
    Ok(value.to_owned())
}
fn lines(bytes: &[u8], max: usize) -> Result<Vec<String>, Error> {
    if bytes.is_empty() || bytes.len() > max {
        return Err(Error::Limit);
    }
    let text = Charset::Cp437.decode(bytes).map_err(|_| Error::Encoding)?;
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<_> = normalized
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    if lines.len() > MAX_LINES || lines.iter().any(|l| !safe_text(l)) {
        return Err(Error::Limit);
    }
    Ok(lines)
}

pub fn parse(bytes: &[u8], domain: &Domain) -> Result<Envelope, Error> {
    parse_history(bytes, domain, false)
}

/// Narrow direct-hatch syntax. The caller must supply authenticated configured
/// identities, check ambiguity, verify the TIC credential and retain raw custody.
/// Generic parsing and encoding deliberately do not use this profile.
pub fn parse_direct_hatch(
    bytes: &[u8],
    peer: &Endpoint,
    local: &Endpoint,
) -> Result<Envelope, Error> {
    let envelope = parse_history(bytes, &peer.domain, true)?;
    let meta = &envelope.metadata;
    let history = envelope.direct_hatch.as_ref().ok_or(Error::Malformed)?;
    let paths = meta.path.len() + usize::from(history.untimed_path.is_some());
    if peer.domain != local.domain
        || meta.from != *peer
        || meta.origin != *peer
        || meta.to.as_ref() != Some(local)
        || paths != 1
        || meta.path.iter().any(|p| p.address != *peer)
        || history.untimed_path.as_ref().is_some_and(|p| p != peer)
    {
        return Err(Error::Malformed);
    }
    Ok(envelope)
}

fn parse_history(bytes: &[u8], domain: &Domain, direct_hatch: bool) -> Result<Envelope, Error> {
    let mut singleton = std::collections::BTreeMap::new();
    let mut descriptions = vec![];
    let mut long_descriptions = vec![];
    let mut path = vec![];
    let mut seen = vec![];
    let mut opaque = vec![];
    let mut untimed_path = None;
    for line in lines(bytes, MAX_TIC)? {
        let (key, value) = line.split_once(' ').ok_or(Error::Malformed)?;
        let key = key.to_ascii_lowercase();
        let value = value.trim_start_matches(' ');
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric()) || value.is_empty() {
            return Err(Error::Malformed);
        }
        match key.as_str() {
            "desc" => descriptions.push(value.to_owned()),
            "ldesc" => long_descriptions.push(value.to_owned()),
            "path" => {
                let mut fields = value.splitn(3, ' ');
                let address = endpoint(fields.next().ok_or(Error::Malformed)?, domain)?;
                let time = match fields.next() {
                    Some(value) => decimal(value)?,
                    None if direct_hatch && untimed_path.is_none() => {
                        untimed_path = Some(address);
                        continue;
                    }
                    None => return Err(Error::Malformed),
                };
                path.push(Hop {
                    address,
                    time,
                    detail: fields.next().unwrap_or_default().to_owned(),
                });
            }
            "seenby" => seen.push(endpoint(value, domain)?),
            "area" | "file" | "origin" | "from" | "to" | "size" | "crc" | "pw" | "lfile"
            | "fullname" => {
                let canonical = if key == "fullname" { "lfile" } else { &key };
                if singleton
                    .insert(canonical.to_owned(), value.to_owned())
                    .is_some()
                {
                    return Err(Error::Malformed);
                }
            }
            _ => opaque.push(line),
        }
    }
    let get = |key: &str| {
        singleton
            .get(key)
            .map(String::as_str)
            .ok_or(Error::Malformed)
    };
    let crc = get("crc")?;
    if crc.len() != 8 || !crc.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Malformed);
    }
    let size = decimal(get("size")?)?;
    if size == 0
        || size > MAX_PAYLOAD
        || (path.is_empty() && untimed_path.is_none())
        || (!direct_hatch && seen.is_empty())
        || descriptions.len() + long_descriptions.len() > 20
        || descriptions
            .iter()
            .chain(&long_descriptions)
            .map(String::len)
            .sum::<usize>()
            > 4096
    {
        return Err(Error::Limit);
    }
    let history = direct_hatch.then_some(DirectHatchHistory {
        untimed_path,
        empty_seenby: seen.is_empty(),
    });
    let metadata = Metadata {
        area: area(get("area")?)?,
        file: filename(get("file")?)?,
        long_name: singleton.get("lfile").map(|s| long_name(s)).transpose()?,
        origin: endpoint(get("origin")?, domain)?,
        from: endpoint(get("from")?, domain)?,
        to: singleton
            .get("to")
            .map(|s| endpoint(s, domain))
            .transpose()?,
        size,
        crc: u32::from_str_radix(crc, 16).map_err(|_| Error::Malformed)?,
        descriptions,
        long_descriptions,
        path,
        seen,
        opaque,
    };
    Ok(Envelope {
        metadata,
        direct_hatch: history,
        password: singleton.remove("pw").unwrap_or_default(),
    })
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}
impl Metadata {
    pub fn validate_payload(&self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() as u64 != self.size {
            return Err(Error::Size);
        }
        if crc32(bytes) != self.crc {
            return Err(Error::Checksum);
        }
        Ok(())
    }
    pub fn description(&self) -> String {
        if self.long_descriptions.is_empty() {
            self.descriptions.join("\n")
        } else {
            self.long_descriptions.join("\n")
        }
    }
    pub fn encode(&self, password: &str) -> Result<Vec<u8>, Error> {
        if password.is_empty()
            || password.len() > 71
            || !password.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Error::Malformed);
        }
        let mut lines = vec![
            format!("Area {}", self.area),
            format!("Origin {}", self.origin),
            format!("From {}", self.from),
            format!("File {}", self.file),
            format!("Size {}", self.size),
            format!("Crc {:08X}", self.crc),
        ];
        if let Some(to) = &self.to {
            lines.push(format!("To {to}"));
        }
        if let Some(name) = &self.long_name {
            lines.push(format!("Lfile {name}"));
        }
        for desc in &self.descriptions {
            lines.push(format!("Desc {desc}"));
        }
        for desc in &self.long_descriptions {
            lines.push(format!("Ldesc {desc}"));
        }
        // Prevent manually constructed opaque metadata from impersonating trust fields.
        for line in &self.opaque {
            let key = line
                .split_once(' ')
                .ok_or(Error::Malformed)?
                .0
                .to_ascii_lowercase();
            if matches!(
                key.as_str(),
                "area"
                    | "origin"
                    | "from"
                    | "to"
                    | "file"
                    | "size"
                    | "crc"
                    | "pw"
                    | "lfile"
                    | "fullname"
                    | "desc"
                    | "ldesc"
                    | "path"
                    | "seenby"
            ) {
                return Err(Error::Malformed);
            }
            lines.push(line.clone());
        }
        for hop in &self.path {
            lines.push(format!(
                "Path {} {}{}{}",
                hop.address,
                hop.time,
                if hop.detail.is_empty() { "" } else { " " },
                hop.detail
            ));
        }
        for seen in &self.seen {
            lines.push(format!("Seenby {seen}"));
        }
        lines.push(format!("Pw {password}"));
        if lines.iter().any(|l| !safe_text(l)) {
            return Err(Error::Malformed);
        }
        let bytes = Charset::Cp437
            .encode(&(lines.join("\r\n") + "\r\n"))
            .map_err(|_| Error::Encoding)?;
        let parsed = parse(&bytes, &self.from.domain)?;
        if parsed.metadata != *self {
            return Err(Error::Malformed);
        }
        Ok(bytes)
    }
}

/// Derive a reproducible 8.3 alias without altering the native filename.
pub fn transfer_name(native: &str, hash: &str) -> Result<String, Error> {
    if let Ok(name) = filename(native) {
        return Ok(name);
    }
    long_name(native)?;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Malformed);
    }
    let extension = native
        .rsplit_once('.')
        .map(|(_, s)| s)
        .filter(|s| !s.is_empty() && s.len() <= 3 && s.bytes().all(|b| b.is_ascii_alphanumeric()))
        .unwrap_or("DAT");
    filename(&format!("{}.{}", &hash[..8], extension))
}

/// Explicit filenames only. A caller cannot smuggle paths, globs, passwords,
/// update requests or executable magic into the selected request profile.
pub fn parse_request(bytes: &[u8], max_files: usize) -> Result<Vec<String>, Error> {
    let mut names = BTreeSet::new();
    let lines = lines(bytes, 4096)?;
    if lines.is_empty() || lines.len() > max_files || max_files > 32 {
        return Err(Error::Limit);
    }
    for line in lines {
        if !names.insert(filename(&line)?) {
            return Err(Error::Malformed);
        }
    }
    Ok(names.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Vec<u8> {
        b"Area TEST\r\nFile TEST.ZIP\r\nOrigin 10:1/1\r\nFrom 10:1/2\r\nSize 9\r\nCrc CBF43926\r\nDesc caf\x82\r\nPath 10:1/1 1\r\nSeenby 10:1/1\r\nPw SECRET\r\n".to_vec()
    }
    fn hatch(path: &str, seen: &str) -> Vec<u8> {
        format!("Area INTEROP.FILE\r\nFile HATCH.TXT\r\nOrigin 90:100/1\r\nFrom 90:100/1\r\nTo 90:100/2@interop\r\nSize 9\r\nCrc CBF43926\r\n{path}{seen}Pw SECRET\r\n").into_bytes()
    }
    #[test]
    fn direct_hatch_omissions_are_independent_and_generic_codec_stays_strict() {
        let peer: Endpoint = "90:100/1@interop".parse().unwrap();
        let local = "90:100/2@interop".parse().unwrap();
        for untimed in [false, true] {
            for empty in [false, true] {
                let bytes = hatch(
                    if untimed {
                        "Path 90:100/1\r\n"
                    } else {
                        "Path 90:100/1 123\r\n"
                    },
                    if empty { "" } else { "Seenby 90:100/1\r\n" },
                );
                assert_eq!(parse(&bytes, &peer.domain).is_ok(), !untimed && !empty);
                let parsed = parse_direct_hatch(&bytes, &peer, &local).unwrap();
                let history = parsed.direct_hatch.unwrap();
                assert_eq!(history.untimed_path.as_ref(), untimed.then_some(&peer));
                assert_eq!(history.empty_seenby, empty);
                assert_eq!(parsed.metadata.path.len(), usize::from(!untimed));
                assert_eq!(parsed.metadata.seen.len(), usize::from(!empty));
                assert_eq!(
                    parsed.metadata.encode("NEWSECRET").is_ok(),
                    !untimed && !empty
                );
                parsed.metadata.validate_payload(b"123456789").unwrap();
            }
        }
    }
    #[test]
    fn direct_hatch_path_syntax_and_exact_provenance_fail_closed() {
        let peer = "90:100/1@interop".parse().unwrap();
        let local = "90:100/2@interop".parse().unwrap();
        for path in [
            "",
            "Path garbage\r\n",
            "Path 90:100/1 \r\n",
            "Path 90:100/1 NaN\r\n",
            "Path 90:100/1 -1\r\n",
            "Path 90:100/1 18446744073709551616\r\n",
            "Path 90:100/9\r\n",
            "Path 90:100/1@fidonet\r\n",
            "Path 1:123/4@fidonet\r\n",
            "Path 20:200/7@othernet\r\n",
            "Path 90:100/1@unknown\r\n",
            "Path 90:100/1\r\nPath 90:100/1\r\n",
            "Path 90:100/1\r\nPath 90:100/1 2\r\n",
        ] {
            assert!(
                parse_direct_hatch(&hatch(path, ""), &peer, &local).is_err(),
                "{path}"
            );
        }
        let text = String::from_utf8(hatch("Path 90:100/1\r\n", "")).unwrap();
        for (before, after) in [
            ("Origin 90:100/1", "Origin 90:100/3"),
            ("From 90:100/1", "From 90:100/3"),
            ("To 90:100/2@interop", "To 90:100/3@interop"),
            ("To 90:100/2@interop\r\n", ""),
            ("Origin 90:100/1", "Origin 90:100/1@unknown"),
        ] {
            assert!(
                parse_direct_hatch(text.replace(before, after).as_bytes(), &peer, &local).is_err()
            );
        }
        assert!(
            parse_direct_hatch(text.as_bytes(), &peer, &"90:100/2@other".parse().unwrap()).is_err()
        );
    }
    #[test]
    fn round_trip_encoding_crc_and_redaction() {
        let domain = "synthetic".parse().unwrap();
        let parsed = parse(&sample(), &domain).unwrap();
        assert!(parsed.authenticates(|s| s == "SECRET"));
        assert!(!format!("{parsed:?}").contains("SECRET"));
        assert!(!format!("{:?}", parsed.metadata).contains("caf"));
        assert_eq!(parsed.metadata.description(), "café");
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
        assert_eq!(crc32(b""), 0);
        parsed.metadata.validate_payload(b"123456789").unwrap();
        assert_eq!(parsed.metadata.validate_payload(b"123"), Err(Error::Size));
        assert_eq!(
            parsed.metadata.validate_payload(b"123456780"),
            Err(Error::Checksum)
        );
        let encoded = parsed.metadata.encode("NEWPASS").unwrap();
        assert_eq!(parse(&encoded, &domain).unwrap().metadata, parsed.metadata);
        for delimiter in ["\r", "\n"] {
            let text = Charset::Cp437
                .decode(&sample())
                .unwrap()
                .replace("\r\n", delimiter);
            assert_eq!(
                parse(&Charset::Cp437.encode(&text).unwrap(), &domain)
                    .unwrap()
                    .metadata,
                parsed.metadata
            );
        }
    }
    #[test]
    fn traversal_devices_shell_and_ambiguous_names_reject() {
        for name in [
            "../secret",
            "/SECRET",
            "C:SECRET",
            "A/B.ZIP",
            "A\\B.ZIP",
            "CON",
            "NUL.ZIP",
            "com1.zip",
            "LPT9",
            ".HIDDEN",
            "A..ZIP",
            "A.",
            "A.ZIP ",
            "$(ID)",
            "`ID`",
            "A*.ZIP",
            "A?.ZIP",
            "A:ADS",
            "é.ZIP",
        ] {
            assert!(filename(name).is_err(), "{name}");
            assert!(parse_request(name.as_bytes(), 4).is_err(), "{name}");
        }
        assert_eq!(filename("hello.zip").unwrap(), "HELLO.ZIP");
    }
    #[test]
    fn missing_duplicate_bounds_and_unknown_provenance() {
        let domain = "synthetic".parse().unwrap();
        for key in [
            "Area", "File", "Origin", "From", "Size", "Crc", "Path", "Seenby",
        ] {
            let data = sample()
                .split(|b| *b == b'\n')
                .filter(|l| !l.starts_with(key.as_bytes()))
                .flat_map(|l| [l, b"\n"].concat())
                .collect::<Vec<_>>();
            assert!(parse(&data, &domain).is_err(), "{key}");
        }
        for field in [
            "From 10:1/3",
            "Pw OTHER",
            "File OTHER.ZIP",
            "Size 0",
            "Crc BAD",
        ] {
            assert!(parse(
                &[sample(), format!("{field}\r\n").into_bytes()].concat(),
                &domain
            )
            .is_err());
        }
        assert!(parse(&vec![b'A'; MAX_TIC + 1], &domain).is_err());
        let data = [
            sample(),
            b"Future Opaque Value\r\nReplaces *.ZIP\r\nMagic PRIVATE\r\n".to_vec(),
        ]
        .concat();
        let parsed = parse(&data, &domain).unwrap();
        assert_eq!(parsed.metadata.opaque.len(), 3);
        assert_eq!(
            parse(&parsed.metadata.encode("PASS").unwrap(), &domain)
                .unwrap()
                .metadata,
            parsed.metadata
        );
    }
    #[test]
    fn explicit_requests_and_stable_long_filename_mapping() {
        assert_eq!(
            parse_request(b"ONE.ZIP\r\nTWO.ZIP\r\n", 2).unwrap(),
            vec!["ONE.ZIP", "TWO.ZIP"]
        );
        assert!(parse_request(b"ONE.ZIP\nTWO.ZIP", 1).is_err());
        assert!(parse_request(b"ONE.ZIP\none.zip", 2).is_err());
        let hash = crate::qwk::digest(b"synthetic");
        assert_eq!(
            transfer_name("LongNativeName.zip", &hash).unwrap(),
            format!("{}.ZIP", hash[..8].to_ascii_uppercase())
        );
        assert!(transfer_name("../secret.zip", &hash).is_err());
        assert!(transfer_name("NUL.zip", &hash).is_err());
    }
}
