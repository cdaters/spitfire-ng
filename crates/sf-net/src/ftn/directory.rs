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

//! FTS-5000 nodelists and FTS-5002 Boss/combined pointlists. No endpoint probing.
use super::*;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DirectoryFormat {
    Nodelist,
    Boss,
    Combined,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryProfile {
    pub format: DirectoryFormat,
    pub charset: Charset,
    pub date: NaiveDate,
    pub default_zone: u16,
    pub require_crc: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InternetService {
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
}
#[derive(Clone, Eq, PartialEq)]
pub struct DirectoryEntry {
    pub address: Address,
    pub keyword: String,
    pub system: String,
    pub location: String,
    pub sysop: String,
    pub phone: String,
    pub speed: u32,
    pub flags: Vec<String>,
    pub services: Vec<InternetService>,
    pub line: u32,
    pub raw: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DirectoryIssue {
    pub line: u32,
    pub reason: String,
}
pub struct DirectoryCandidate {
    pub entries: Vec<DirectoryEntry>,
    pub issues: Vec<DirectoryIssue>,
    pub checksum: Option<u16>,
}
pub fn crc16(b: &[u8]) -> u16 {
    let mut crc = 0u16;
    for b in b {
        crc ^= u16::from(*b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}
fn service(value: &str, protocol: &str) -> Result<InternetService, Error> {
    let default = match protocol {
        "IBN" => Some(24554),
        "IFC" => Some(60179),
        "ITN" => Some(23),
        "IFT" => Some(21),
        "IVM" => Some(3141),
        _ => None,
    };
    if value.is_empty() {
        return Ok(InternetService {
            protocol: protocol.into(),
            host: None,
            port: default,
        });
    }
    let (host, port) = if value.starts_with('[') {
        let end = value.find(']').ok_or(Error::Directory)?;
        let host = &value[..=end];
        let rest = &value[end + 1..];
        (
            host,
            if rest.is_empty() {
                None
            } else {
                Some(rest.strip_prefix(':').ok_or(Error::Directory)?)
            },
        )
    } else {
        match value.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None if value.bytes().all(|b| b.is_ascii_digit()) && protocol != "INA" => {
                ("", Some(value))
            }
            None => (value, None),
        }
    };
    if host.len() > 255
        || host
            .bytes()
            .any(|b| b <= 32 || b >= 127 || b"/\\@".contains(&b))
    {
        return Err(Error::Directory);
    }
    let port = port.map(number).transpose()?.or(default);
    if port == Some(0) {
        return Err(Error::Directory);
    }
    Ok(InternetService {
        protocol: protocol.into(),
        host: (!host.is_empty()).then(|| host.into()),
        port,
    })
}
pub fn parse(bytes: &[u8], profile: &DirectoryProfile) -> Result<DirectoryCandidate, Error> {
    if bytes.len() > 16 * 1024 * 1024 || profile.default_zone == 0 {
        return Err(Error::Limit);
    }
    let bytes = bytes.strip_suffix(&[26]).unwrap_or(bytes);
    if bytes.contains(&26) {
        return Err(Error::Directory);
    }
    let mut lines: Vec<&[u8]> = bytes.split(|b| *b == 10).take(100_002).collect();
    if lines.last() == Some(&&b""[..]) {
        lines.pop();
    }
    if lines.is_empty() || lines.len() > 100_000 {
        return Err(Error::Limit);
    }
    let lines: Vec<&[u8]> = lines
        .into_iter()
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .collect();
    if lines
        .iter()
        .any(|l| l.len() > 2048 || l.iter().any(|b| *b < 32 || *b == 127))
    {
        return Err(Error::Directory);
    }
    let first = profile.charset.decode(lines[0])?;
    if !first.starts_with(';') {
        return Err(Error::Directory);
    }
    let checksum = if let Some((head, crc)) = first.rsplit_once(':') {
        let day = head
            .split(|c: char| !c.is_ascii_digit())
            .rfind(|s| !s.is_empty())
            .ok_or(Error::Directory)?;
        if u32::from(number(day)?) != profile.date.ordinal() {
            return Err(Error::Directory);
        }
        let expected = number(crc.trim())?;
        let mut raw = vec![];
        for line in lines.iter().skip(1) {
            raw.extend_from_slice(line);
            raw.extend(b"\r\n");
        }
        if crc16(&raw) != expected {
            return Err(Error::Checksum);
        }
        Some(expected)
    } else {
        if profile.require_crc {
            return Err(Error::Checksum);
        }
        None
    };
    let mut out = DirectoryCandidate {
        entries: vec![],
        issues: vec![],
        checksum,
    };
    let mut zone = profile.default_zone;
    let mut net = None;
    let mut boss = None;
    let mut seen = BTreeMap::new();
    for (i, raw) in lines.into_iter().enumerate().skip(1) {
        if raw.is_empty() || raw.starts_with(b";") {
            continue;
        }
        let line = profile.charset.decode(raw)?;
        let f: Vec<&str> = line.split(',').collect();
        let result = (|| -> Result<DirectoryEntry, Error> {
            if f.first() == Some(&"Boss") {
                if profile.format != DirectoryFormat::Boss || f.len() < 2 {
                    return Err(Error::Directory);
                }
                let a: Address = f[1].parse()?;
                if a.point() != 0 {
                    return Err(Error::Directory);
                }
                boss = Some(a);
                return Err(Error::Profile);
            }
            if f.len() < 7 || f.iter().any(|f| f.len() > 255 || f.contains(' ')) {
                return Err(Error::Directory);
            }
            let n = number(f[1])?;
            if n == 0 {
                return Err(Error::Address);
            }
            let a = if profile.format == DirectoryFormat::Boss {
                if !f[0].is_empty() {
                    return Err(Error::Directory);
                }
                boss.ok_or(Error::Directory)?.with_point(n)
            } else {
                match f[0] {
                    "Zone" => {
                        zone = n;
                        net = Some(n);
                        boss = None;
                        Address::new(zone, n, 0, 0)?
                    }
                    "Region" | "Host" => {
                        net = Some(n);
                        boss = None;
                        Address::new(zone, n, 0, 0)?
                    }
                    "Point" if profile.format == DirectoryFormat::Combined => {
                        boss.ok_or(Error::Directory)?.with_point(n)
                    }
                    "" | "Hub" | "Pvt" | "Hold" | "Down" => {
                        let a = Address::new(zone, net.ok_or(Error::Directory)?, n, 0)?;
                        boss = Some(a);
                        a
                    }
                    _ => return Err(Error::Directory),
                }
            };
            let speed = f[6].parse::<u32>().map_err(|_| Error::Directory)?;
            let flags: Vec<String> = f[7..]
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            let mut services = vec![];
            for flag in &flags {
                let (name, v) = flag.split_once(':').unwrap_or((flag, ""));
                if matches!(name, "INA" | "IBN" | "IFC" | "ITN" | "IFT" | "IVM") {
                    services.push(service(v, name)?);
                }
            }
            Ok(DirectoryEntry {
                address: a,
                keyword: f[0].into(),
                system: f[2].replace('_', " "),
                location: f[3].replace('_', " "),
                sysop: f[4].replace('_', " "),
                phone: f[5].into(),
                speed,
                flags,
                services,
                line: (i + 1) as u32,
                raw: raw.to_vec(),
            })
        })();
        match result {
            Ok(e) => {
                if let Some(old) = seen.insert(e.address, e.raw.clone()) {
                    out.issues.push(DirectoryIssue {
                        line: e.line,
                        reason: if old == e.raw {
                            "duplicate-address"
                        } else {
                            "conflicting-address"
                        }
                        .into(),
                    });
                } else {
                    out.entries.push(e);
                }
            }
            Err(Error::Profile) => (),
            Err(_) => {
                out.issues.push(DirectoryIssue {
                    line: (i + 1) as u32,
                    reason: "malformed-record".into(),
                });
                boss = None;
                if profile.format != DirectoryFormat::Boss {
                    net = None;
                }
            }
        }
    }
    if out.entries.is_empty() {
        return Err(Error::Directory);
    }
    Ok(out)
}
