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

//! FTS-1026/1027 framing and authentication primitives; no transport authority.
use crate::ftn::{Domain, Endpoint};
use hmac::{Hmac, Mac};
use md5::Md5;
use serde::{Deserialize, Serialize};

pub const MAX_FRAME: usize = 32767;
pub const MAX_COMMAND: usize = 4096;
pub const MAX_ADDRESSES: usize = 32;
pub const MAX_FILE: u64 = 16 * 1024 * 1024;
pub const MAX_FILES: usize = 64;
pub const MAX_SESSION_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case")]
pub enum Error {
    #[error("malformed BinkP frame or command")]
    Malformed,
    #[error("BinkP resource limit reached")]
    Limit,
    #[error("BinkP address mismatch")]
    Address,
    #[error("BinkP authentication failed")]
    Authentication,
    #[error("BinkP transfer interrupted")]
    Interrupted,
    #[error("BinkP timeout")]
    Timeout,
    #[error("BinkP link unavailable")]
    Unavailable,
    #[error("BinkP peer refused session")]
    Refused,
    #[error("BinkP session cancelled")]
    Cancelled,
    #[error("BinkP artifact custody failed")]
    Custody,
    #[error("BinkP link busy")]
    Busy,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Connecting,
    Greeting,
    Authenticating,
    Ready,
    Exchanging,
    Finishing,
    Complete,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Command {
    Nul = 0,
    Adr = 1,
    Pwd = 2,
    File = 3,
    Ok = 4,
    Eob = 5,
    Got = 6,
    Err = 7,
    Bsy = 8,
    Get = 9,
    Skip = 10,
}
impl Command {
    pub fn from_id(id: u8) -> Option<Self> {
        Some(match id {
            0 => Self::Nul,
            1 => Self::Adr,
            2 => Self::Pwd,
            3 => Self::File,
            4 => Self::Ok,
            5 => Self::Eob,
            6 => Self::Got,
            7 => Self::Err,
            8 => Self::Bsy,
            9 => Self::Get,
            10 => Self::Skip,
            _ => return None,
        })
    }
}
// Deliberately no Debug/Serialize: command arguments may contain credentials.
pub enum Frame {
    Empty,
    Data(Vec<u8>),
    Command(u8, Vec<u8>),
}
impl Frame {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 2 {
            return Err(Error::Malformed);
        }
        let header = u16::from_be_bytes([bytes[0], bytes[1]]);
        let size = usize::from(header & 0x7fff);
        if bytes.len() != size + 2 {
            return Err(Error::Malformed);
        }
        if size == 0 {
            return Ok(Self::Empty);
        }
        if header & 0x8000 == 0 {
            return Ok(Self::Data(bytes[2..].to_vec()));
        }
        if size > MAX_COMMAND {
            return Err(Error::Limit);
        }
        if bytes[2] > 127 {
            return Err(Error::Malformed);
        }
        let args = &bytes[3..];
        let args = args.strip_suffix(&[0]).unwrap_or(args);
        if args.contains(&0) {
            return Err(Error::Malformed);
        }
        Ok(Self::Command(bytes[2], args.to_vec()))
    }
    pub fn command(command: Command, args: &str) -> Result<Self, Error> {
        if args.len() + 1 > MAX_COMMAND || args.bytes().any(|b| !(32..=126).contains(&b)) {
            return Err(Error::Malformed);
        }
        Ok(Self::Command(command as u8, args.as_bytes().to_vec()))
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let (header, payload) = match self {
            Self::Empty => (0, vec![]),
            Self::Data(b) if b.len() <= MAX_FRAME => (b.len() as u16, b.clone()),
            Self::Command(c, a) if *c < 128 && a.len() < MAX_COMMAND && !a.contains(&0) => {
                let mut p = vec![*c];
                p.extend(a);
                (0x8000 | p.len() as u16, p)
            }
            _ => return Err(Error::Limit),
        };
        let mut out = header.to_be_bytes().to_vec();
        out.extend(payload);
        Ok(out)
    }
}
/// Validated protocol filename metadata. It must never be used as a local path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offer {
    pub name: String,
    pub size: u64,
    pub time: u64,
    pub offset: u64,
}
fn decimal(s: &str) -> Result<u64, Error> {
    if s.is_empty() || s.len() > 20 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Malformed);
    }
    s.parse().map_err(|_| Error::Malformed)
}
fn filename(s: &str) -> Result<String, Error> {
    if s.is_empty() || s.len() > 512 {
        return Err(Error::Limit);
    }
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let v = if b[i] == b'\\' {
            i += 1;
            if b.get(i) == Some(&b'x') {
                i += 1;
            }
            let pair = b.get(i..i + 2).ok_or(Error::Malformed)?;
            i += 2;
            let text = std::str::from_utf8(pair).map_err(|_| Error::Malformed)?;
            u8::from_str_radix(text, 16).map_err(|_| Error::Malformed)?
        } else {
            let v = b[i];
            i += 1;
            v
        };
        if !(32..=126).contains(&v) || b"/\\:".contains(&v) {
            return Err(Error::Malformed);
        }
        out.push(v);
    }
    if out.len() > 128 {
        return Err(Error::Limit);
    }
    let name = String::from_utf8(out).map_err(|_| Error::Malformed)?;
    if name == "." || name == ".." || name.starts_with('.') || name.ends_with(['.', ' ']) {
        return Err(Error::Malformed);
    }
    Ok(name)
}
fn escape(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b == b' ' || b == b'\\' {
            out.push_str(&format!("\\x{b:02x}"));
        } else {
            out.push(char::from(b));
        }
    }
    out
}
impl Offer {
    pub fn parse(args: &str, offset: bool) -> Result<Self, Error> {
        if args.len() > MAX_COMMAND {
            return Err(Error::Limit);
        }
        let v: Vec<_> = args.split_ascii_whitespace().take(6).collect();
        if v.len() != if offset { 4 } else { 3 } {
            return Err(Error::Malformed);
        }
        let result = Self {
            name: filename(v[0])?,
            size: decimal(v[1])?,
            time: decimal(v[2])?,
            offset: if offset { decimal(v[3])? } else { 0 },
        };
        if result.size == 0 || result.size > MAX_FILE || result.offset > result.size {
            return Err(Error::Limit);
        }
        Ok(result)
    }
    pub fn arguments(&self, offset: bool) -> String {
        let prefix = format!("{} {} {}", escape(&self.name), self.size, self.time);
        if offset {
            format!("{prefix} {}", self.offset)
        } else {
            prefix
        }
    }
    pub fn same_file(&self, other: &Self) -> bool {
        self.name == other.name && self.size == other.size && self.time == other.time
    }
}
/// Domainless compatibility uses an explicitly supplied link domain, never a global default.
pub fn addresses(args: &str, default_domain: Option<&Domain>) -> Result<Vec<Endpoint>, Error> {
    if args.len() > MAX_COMMAND {
        return Err(Error::Limit);
    }
    let mut result = Vec::new();
    for a in args.split_ascii_whitespace() {
        if result.len() == MAX_ADDRESSES {
            return Err(Error::Limit);
        }
        let endpoint = if a.contains('@') {
            a.parse().map_err(|_| Error::Address)?
        } else {
            Endpoint {
                address: a.parse().map_err(|_| Error::Address)?,
                domain: default_domain.ok_or(Error::Address)?.clone(),
            }
        };
        if result.contains(&endpoint) {
            return Err(Error::Address);
        }
        result.push(endpoint);
    }
    if result.is_empty() {
        return Err(Error::Address);
    }
    Ok(result)
}
pub fn challenge(option: &str) -> Result<Vec<u8>, Error> {
    let value = option.strip_prefix("CRAM-").ok_or(Error::Authentication)?;
    let (methods, data) = value.split_once('-').ok_or(Error::Authentication)?;
    if !methods.split('/').any(|m| m == "MD5") {
        return Err(Error::Authentication);
    }
    let bytes = unhex(data)?;
    if !(8..=64).contains(&bytes.len()) {
        return Err(Error::Authentication);
    }
    Ok(bytes)
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(value: &str) -> Result<Vec<u8>, Error> {
    if value.len() > 128
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::Authentication);
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| {
            u8::from_str_radix(
                std::str::from_utf8(p).map_err(|_| Error::Authentication)?,
                16,
            )
            .map_err(|_| Error::Authentication)
        })
        .collect()
}
pub fn cram(secret: &[u8], challenge: &[u8]) -> String {
    let mut mac = Hmac::<Md5>::new_from_slice(secret).expect("HMAC accepts arbitrary key length");
    mac.update(challenge);
    format!("CRAM-MD5-{}", hex(&mac.finalize().into_bytes()))
}
pub fn verify_cram(secret: &[u8], challenge: &[u8], response: &str) -> Result<(), Error> {
    let digest = unhex(
        response
            .strip_prefix("CRAM-MD5-")
            .ok_or(Error::Authentication)?,
    )?;
    let mut mac = Hmac::<Md5>::new_from_slice(secret).map_err(|_| Error::Authentication)?;
    mac.update(challenge);
    mac.verify_slice(&digest).map_err(|_| Error::Authentication)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_and_bounds() {
        assert_eq!(
            Frame::command(Command::Nul, "TEST")
                .unwrap()
                .encode()
                .unwrap(),
            b"\x80\x05\x00TEST"
        );
        for bytes in [vec![0, 0], vec![128, 0]] {
            assert!(matches!(Frame::decode(&bytes).unwrap(), Frame::Empty));
        }
        let data = Frame::Data(vec![0xff; MAX_FRAME]).encode().unwrap();
        for n in 0..data.len() {
            assert!(Frame::decode(&data[..n]).is_err());
        }
        assert!(matches!(Frame::decode(&data).unwrap(),Frame::Data(d) if d.len()==MAX_FRAME));
        assert!(Frame::Data(vec![0; MAX_FRAME + 1]).encode().is_err());
        assert!(Frame::decode(&[128, 2, 128, 1]).is_err());
        assert!(Frame::decode(&[128, 3, 2, 0, 1]).is_err());
        assert!(
            matches!(Frame::decode(&[128,2,127,0]).unwrap(),Frame::Command(127,a) if a.is_empty())
        );
    }
    #[test]
    fn filenames_offsets_and_spoofed_ack() {
        let offer = Offer::parse("a\\x20b.pkt 125 2476327846 100", true).unwrap();
        assert_eq!(offer.name, "a b.pkt");
        assert_eq!(offer.arguments(true), "a\\x20b.pkt 125 2476327846 100");
        assert_eq!(
            Offer::parse("a\\20b.pkt 125 2476327846", false)
                .unwrap()
                .name,
            "a b.pkt"
        );
        for name in [
            "../x", "/x", "C:x", "a\\x2fb", "a\\x5cb", "a\\x00b", "..", ".hidden", "a\\x0ab",
        ] {
            assert!(
                Offer::parse(&format!("{name} 1 0 0"), true).is_err(),
                "{name}"
            );
        }
        for args in [
            "x 1 0 2",
            "x -1 0 0",
            "x 18446744073709551616 0 0",
            "x 1 0 -1",
            "x 0 0 0",
            "x 1 0 0 extra",
        ] {
            assert!(Offer::parse(args, true).is_err());
        }
        assert!(!offer.same_file(&Offer {
            time: 0,
            ..offer.clone()
        }));
    }
    #[test]
    fn five_d_identity_and_point_isolation() {
        let a = addresses("10:100/1.3@TEST 10:100/1@second", None).unwrap();
        assert_eq!(a[0].address.point(), 3);
        assert_ne!(a[0].domain, a[1].domain);
        assert!(addresses("10:100/1", None).is_err());
        assert_eq!(
            addresses("10:100/1.3", Some(&a[0].domain)).unwrap()[0],
            a[0]
        );
        assert!(addresses(&"10:100/1@a ".repeat(33), None).is_err());
    }
    #[test]
    fn authoritative_cram_vector_and_constant_time_verification() {
        let data = challenge("CRAM-MD5-f0315b074d728d483d6887d0182fc328").unwrap();
        let response = cram(b"tanstaaftanstaaf", &data);
        assert_eq!(response, "CRAM-MD5-56be002162a4a15ba7a9064f0c93fd00");
        verify_cram(b"tanstaaftanstaaf", &data, &response).unwrap();
        assert!(verify_cram(b"wrong", &data, &response).is_err());
        for c in [
            "CRAM-MD5-1",
            "CRAM-MD5-xx",
            "CRAM-SHA1-0011223344556677",
            "CRAM-MD5-00",
        ] {
            assert!(challenge(c).is_err());
        }
        // RFC 2202 HMAC-MD5 long-key vector exercises the library's key normalization.
        assert_eq!(
            cram(
                &[0xaa; 80],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            ),
            "CRAM-MD5-6b1ab7fe4bd7bf8f0b62e6ce61b9d0cd"
        );
    }
}
