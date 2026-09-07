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

//! Bounded FTS control paragraphs, explicit text encoding and timestamp evidence.
use super::*;
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use std::collections::BTreeSet;
/// RESCANNED wire syntax only. A numeric marker is not a routable identity;
/// the importer must resolve it using authenticated, unambiguous peer context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RescanSource {
    Qualified(Endpoint),
    Domainless(Address),
}
impl FromStr for RescanSource {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Error> {
        if value.contains('@') {
            value.parse().map(Self::Qualified)
        } else {
            value.parse().map(Self::Domainless)
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Charset {
    Ascii,
    Cp437,
    Cp850,
    Cp866,
    Utf8,
}
impl Charset {
    pub fn identifier(self) -> &'static str {
        match self {
            Self::Ascii => "ASCII",
            Self::Cp437 => "CP437",
            Self::Cp850 => "CP850",
            Self::Cp866 => "CP866",
            Self::Utf8 => "UTF-8",
        }
    }
    pub fn parse(s: &str) -> Result<Self, Error> {
        match s.to_ascii_uppercase().as_str() {
            "ASCII" => Ok(Self::Ascii),
            "CP437" => Ok(Self::Cp437),
            "CP850" => Ok(Self::Cp850),
            "CP866" | "+7_FIDO" => Ok(Self::Cp866),
            "UTF-8" => Ok(Self::Utf8),
            _ => Err(Error::Encoding),
        }
    }
    pub fn decode(self, b: &[u8]) -> Result<String, Error> {
        if self == Self::Utf8 {
            return String::from_utf8(b.to_vec()).map_err(|_| Error::Encoding);
        }
        let table = match self {
            Self::Cp437 => CP437,
            Self::Cp850 => CP850,
            Self::Cp866 => CP866,
            _ => "",
        };
        let high: Vec<char> = table.chars().collect();
        b.iter()
            .map(|b| {
                if *b < 128 {
                    Ok(char::from(*b))
                } else {
                    high.get(usize::from(*b) - 128)
                        .copied()
                        .ok_or(Error::Encoding)
                }
            })
            .collect()
    }
    pub fn encode(self, s: &str) -> Result<Vec<u8>, Error> {
        if self == Self::Utf8 {
            return Ok(s.as_bytes().to_vec());
        }
        let table = match self {
            Self::Cp437 => CP437,
            Self::Cp850 => CP850,
            Self::Cp866 => CP866,
            _ => "",
        };
        s.chars()
            .map(|c| {
                if c.is_ascii() {
                    Ok(c as u8)
                } else {
                    table
                        .chars()
                        .position(|x| x == c)
                        .map(|n| (n + 128) as u8)
                        .ok_or(Error::Encoding)
                }
            })
            .collect()
    }
}
/// Original unknown and understood controls retain byte spelling and paragraph position.
/// No Debug implementation: retained control data is not an operational diagnostic.
#[derive(Clone, Eq, PartialEq)]
pub struct Control {
    pub paragraph: usize,
    pub raw: Vec<u8>,
}
#[derive(Clone, Eq, PartialEq)]
pub struct Text {
    pub body: String,
    pub charset: Charset,
    pub controls: Vec<Control>,
    pub area: Option<String>,
    pub msgid: Option<String>,
    pub reply: Option<String>,
    pub intl: Option<(Address, Address)>,
    pub fmpt: u16,
    pub topt: u16,
    pub offset_minutes: Option<i32>,
    pub seen_by: BTreeSet<(u16, u16)>,
    pub path: Vec<(u16, u16)>,
    pub via: Vec<String>,
    pub origin: Option<String>,
    pub tear: Option<String>,
    pub flags: Vec<String>,
}
fn single(slot: &mut Option<String>, value: &str) -> Result<(), Error> {
    if slot.is_some() || value.is_empty() {
        return Err(Error::Control);
    }
    *slot = Some(value.to_owned());
    Ok(())
}
fn identity(s: &str) -> Result<(), Error> {
    let (origin, serial) = s.rsplit_once(' ').ok_or(Error::Control)?;
    if origin.is_empty()
        || origin.len() > 240
        || serial.len() != 8
        || !serial.bytes().all(|b| b.is_ascii_hexdigit())
        || s.chars().any(char::is_control)
    {
        return Err(Error::Control);
    }
    if origin.starts_with('"') {
        if !origin.ends_with('"') || origin.len() < 3 {
            return Err(Error::Control);
        }
    } else if origin.contains(' ') {
        return Err(Error::Control);
    }
    Ok(())
}
fn two_d(s: &str, net: &mut Option<u16>) -> Result<(u16, u16), Error> {
    let node = if let Some((n, d)) = s.split_once('/') {
        *net = Some(number(n)?);
        number(d)?
    } else {
        number(s)?
    };
    let net = net.ok_or(Error::Control)?;
    if net == 0 {
        return Err(Error::Control);
    }
    Ok((net, node))
}
pub fn valid_area(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 35
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b"._-".contains(&b))
}
pub fn utc_offset(s: &str) -> Result<i32, Error> {
    let (sign, s) = if let Some(s) = s.strip_prefix('-') {
        (-1, s)
    } else {
        (1, s.strip_prefix('+').unwrap_or(s))
    };
    if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Timestamp);
    }
    let h = number(&s[..2])?;
    let m = number(&s[2..])?;
    if h > 23 || m > 59 {
        return Err(Error::Timestamp);
    }
    Ok(sign * (i32::from(h) * 60 + i32::from(m)))
}
impl Text {
    pub fn plain(body: &str, charset: Charset) -> Result<Self, Error> {
        if body.is_empty()
            || body.len() > 65536
            || body
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            return Err(Error::Encoding);
        }
        charset.encode(body)?;
        let mut text = Self::parse(b"", charset)?;
        text.body = body.into();
        Ok(text)
    }

    pub fn parse(bytes: &[u8], fallback: Charset) -> Result<Self, Error> {
        if bytes.len() > MAX_TEXT || bytes.contains(&0) {
            return Err(Error::Limit);
        }
        let mut out = Self {
            body: String::new(),
            charset: fallback,
            controls: vec![],
            area: None,
            msgid: None,
            reply: None,
            intl: None,
            fmpt: 0,
            topt: 0,
            offset_minutes: None,
            seen_by: BTreeSet::new(),
            path: vec![],
            via: vec![],
            origin: None,
            tear: None,
            flags: vec![],
        };
        let mut paragraphs: Vec<Vec<u8>> = vec![];
        let mut singleton = BTreeSet::new();
        let mut size = 0;
        let mut declared = None;
        for (index, line) in bytes.split(|b| *b == 13).enumerate() {
            let line = line.strip_prefix(b"\n").unwrap_or(line);
            if index == 0 && line.starts_with(b"AREA:") {
                let area = std::str::from_utf8(&line[5..]).map_err(|_| Error::Control)?;
                if !valid_area(area) {
                    return Err(Error::Control);
                }
                out.area = Some(area.into());
                continue;
            }
            if let Some(raw) = line.strip_prefix(&[1]) {
                size += line.len();
                if line.len() > MAX_CONTROL_LINE
                    || size > MAX_CONTROL_BYTES
                    || out.controls.len() >= MAX_CONTROLS
                {
                    return Err(Error::Limit);
                }
                let logical: Vec<u8> = raw
                    .iter()
                    .copied()
                    .filter(|b| !matches!(b, 10 | 141))
                    .collect();
                let split = logical
                    .iter()
                    .position(|b| *b == b':' || *b == b' ')
                    .unwrap_or(logical.len());
                if split == 0
                    || split > 32
                    || !logical[..split]
                        .iter()
                        .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
                {
                    return Err(Error::Control);
                }
                let tag = std::str::from_utf8(&logical[..split]).map_err(|_| Error::Control)?;
                let rest = &logical[split..];
                let value = rest.strip_prefix(b":").unwrap_or(rest);
                let value = value.strip_prefix(b" ").unwrap_or(value);
                let known = matches!(
                    tag,
                    "MSGID"
                        | "REPLY"
                        | "INTL"
                        | "FMPT"
                        | "TOPT"
                        | "CHRS"
                        | "CHARSET"
                        | "TZUTC"
                        | "TZUTCINFO"
                        | "PATH"
                        | "Via"
                        | "FLAGS"
                        | "PID"
                        | "TID"
                        | "RESCANNED"
                );
                if known {
                    let delimiter = if matches!(
                        tag,
                        "INTL" | "FMPT" | "TOPT" | "Via" | "FLAGS" | "RESCANNED"
                    ) {
                        b' '
                    } else {
                        b':'
                    };
                    if rest.first() != Some(&delimiter) {
                        return Err(Error::Control);
                    }
                    let value = std::str::from_utf8(value).map_err(|_| Error::Control)?;
                    if !matches!(tag, "PATH" | "Via") && !singleton.insert(tag.to_string()) {
                        return Err(Error::Control);
                    }
                    match tag {
                        "MSGID" => {
                            identity(value)?;
                            single(&mut out.msgid, value)?;
                        }
                        "REPLY" => {
                            identity(value)?;
                            single(&mut out.reply, value)?;
                        }
                        "INTL" => {
                            let (d, o) = value.split_once(' ').ok_or(Error::Control)?;
                            let d: Address = d.parse()?;
                            let o: Address = o.parse()?;
                            if d.point() != 0 || o.point() != 0 {
                                return Err(Error::Control);
                            }
                            out.intl = Some((d, o));
                        }
                        "FMPT" => {
                            out.fmpt = number(value)?;
                            if out.fmpt == 0 {
                                return Err(Error::Control);
                            }
                        }
                        "TOPT" => {
                            out.topt = number(value)?;
                            if out.topt == 0 {
                                return Err(Error::Control);
                            }
                        }
                        "CHRS" | "CHARSET" => {
                            let name = value
                                .split_ascii_whitespace()
                                .next()
                                .ok_or(Error::Encoding)?;
                            let c = Charset::parse(name)?;
                            if declared.is_some_and(|d| d != c) {
                                return Err(Error::Encoding);
                            }
                            declared = Some(c);
                            out.charset = c;
                        }
                        "TZUTC" | "TZUTCINFO" => {
                            let n = utc_offset(value)?;
                            if out.offset_minutes.is_some_and(|v| v != n) {
                                return Err(Error::Timestamp);
                            }
                            out.offset_minutes = Some(n);
                        }
                        "PATH" => {
                            let mut net = None;
                            for p in value.split_ascii_whitespace() {
                                out.path.push(two_d(p, &mut net)?);
                            }
                            if out.path.len() > MAX_HOPS {
                                return Err(Error::Limit);
                            }
                        }
                        "RESCANNED" => {
                            value.parse::<RescanSource>()?;
                        }
                        "Via" => {
                            if out.via.len() >= MAX_HOPS {
                                return Err(Error::Limit);
                            }
                            out.via.push(value.into());
                        }
                        "FLAGS" => {
                            out.flags = value.split_ascii_whitespace().map(String::from).collect();
                        }
                        _ => (),
                    }
                } else if !rest.starts_with(b":") {
                    return Err(Error::Control);
                }
                out.controls.push(Control {
                    paragraph: paragraphs.len(),
                    raw: line.to_vec(),
                });
            } else {
                paragraphs.push(line.to_vec());
            }
        }
        // Routing footer is recognized only as a trailing block, never in quoted body text.
        while paragraphs.last().is_some_and(Vec::is_empty) {
            paragraphs.pop();
        }
        if out.area.is_some() {
            let mut footer = vec![];
            while paragraphs
                .last()
                .is_some_and(|p| p.starts_with(b"SEEN-BY:"))
            {
                footer.push(paragraphs.pop().ok_or(Error::Control)?);
            }
            footer.reverse();
            let mut net = None;
            for line in footer {
                let s = std::str::from_utf8(&line[8..]).map_err(|_| Error::Control)?;
                for p in s.split_ascii_whitespace() {
                    out.seen_by.insert(two_d(p, &mut net)?);
                }
                if out.seen_by.len() > 1024 {
                    return Err(Error::Limit);
                }
            }
            if paragraphs
                .last()
                .is_some_and(|p| p.starts_with(b" * Origin: "))
            {
                out.origin = Some(
                    out.charset
                        .decode(&paragraphs.pop().ok_or(Error::Control)?)?,
                );
            }
            if paragraphs
                .last()
                .is_some_and(|p| p == b"---" || p.starts_with(b"--- "))
            {
                out.tear = Some(
                    out.charset
                        .decode(&paragraphs.pop().ok_or(Error::Control)?)?,
                );
            }
        }
        let body = paragraphs
            .iter()
            .map(|p| out.charset.decode(p))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        if body
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            return Err(Error::Encoding);
        }
        out.body = body;
        Ok(out)
    }
    pub fn addresses(
        &self,
        m: &PackedMessage,
        h: &PacketHeader,
    ) -> Result<(Address, Address), Error> {
        if self.area.is_some() {
            let origin = self
                .origin
                .as_deref()
                .and_then(|s| s.rsplit_once('('))
                .and_then(|(_, a)| a.strip_suffix(')'))
                .and_then(|a| a.split('@').next())
                .ok_or(Error::Address)?
                .parse()?;
            return Ok((
                origin,
                Address::new(h.destination.zone(), m.destination.0, m.destination.1, 0)?,
            ));
        }
        let (d, o) = self.intl.unwrap_or((
            Address::new(h.destination.zone(), m.destination.0, m.destination.1, 0)?,
            Address::new(h.origin.zone(), m.origin.0, m.origin.1, 0)?,
        ));
        if d.two_d() != m.destination || o.two_d() != m.origin {
            return Err(Error::Address);
        }
        Ok((o.with_point(self.fmpt), d.with_point(self.topt)))
    }
    /// Same-charset forwarding preserves unknown control bytes at their original body paragraph.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut lines: Vec<Vec<u8>> = vec![];
        if let Some(a) = &self.area {
            if !valid_area(a) {
                return Err(Error::Control);
            }
            lines.push(format!("AREA:{a}").into_bytes());
        }
        let body: Vec<&str> = self.body.split('\n').collect();
        for i in 0..=body.len() {
            for c in self
                .controls
                .iter()
                .filter(|c| c.paragraph.min(body.len()) == i)
            {
                let tag = c.raw.get(1..).unwrap_or_default();
                if tag.starts_with(b"PATH:")
                    || (self.area.is_some()
                        && [b"INTL ".as_slice(), b"FMPT ", b"TOPT "]
                            .iter()
                            .any(|p| tag.starts_with(p)))
                {
                    continue;
                }
                if c.raw.len() > MAX_CONTROL_LINE
                    || c.raw.first() != Some(&1)
                    || c.raw.contains(&13)
                    || c.raw.contains(&0)
                {
                    return Err(Error::Control);
                }
                lines.push(c.raw.clone());
            }
            if let Some(s) = body.get(i) {
                if s.starts_with('\u{1}') || s.contains('\r') || s.contains('\0') {
                    return Err(Error::Control);
                }
                lines.push(self.charset.encode(s)?);
            }
        }
        if self.area.is_some() {
            if let Some(t) = &self.tear {
                lines.push(self.charset.encode(t)?)
            }
            if let Some(o) = &self.origin {
                lines.push(self.charset.encode(o)?)
            }
            if self.seen_by.is_empty() {
                return Err(Error::Control);
            }
            for chunk in self.seen_by.iter().copied().collect::<Vec<_>>().chunks(6) {
                lines.push(format!("SEEN-BY: {}", format_2d(chunk)).into_bytes());
            }
            for chunk in self.path.chunks(6) {
                lines.push(format!("\u{1}PATH: {}", format_2d(chunk)).into_bytes());
            }
        }
        let controls: Vec<_> = lines.iter().filter(|l| l.first() == Some(&1)).collect();
        if controls.len() > MAX_CONTROLS
            || controls.iter().map(|l| l.len()).sum::<usize>() > MAX_CONTROL_BYTES
            || controls.iter().any(|l| l.len() > MAX_CONTROL_LINE)
            || self.seen_by.len() > 1024
            || self.path.len() > 1024
        {
            return Err(Error::Limit);
        }
        let mut b = vec![];
        for l in lines {
            b.extend(l);
            b.push(13)
        }
        if b.len() > MAX_TEXT {
            return Err(Error::Limit);
        }
        Ok(b)
    }
    pub fn add_control(&mut self, s: &str) {
        self.controls.push(Control {
            paragraph: 0,
            raw: format!("\u{1}{s}").into_bytes(),
        });
    }
}
fn format_2d(p: &[(u16, u16)]) -> String {
    p.iter()
        .map(|(n, d)| format!("{n}/{d}"))
        .collect::<Vec<_>>()
        .join(" ")
}
/// Explicit 1980..2079 policy resolves the two-digit wire year; no current-year guessing.
pub fn parse_date(raw: &[u8; 20]) -> Result<NaiveDateTime, Error> {
    if raw[19] != 0 {
        return Err(Error::Timestamp);
    }
    let s = std::str::from_utf8(&raw[..19]).map_err(|_| Error::Timestamp)?;
    let parts: Vec<&str> = s.split_ascii_whitespace().collect();
    if parts.len() != 4 || parts[2].len() != 2 {
        return Err(Error::Timestamp);
    }
    let y = number(parts[2])?;
    let year = if y >= 80 {
        1900 + i32::from(y)
    } else {
        2000 + i32::from(y)
    };
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == parts[1])
    .ok_or(Error::Timestamp)?
        + 1;
    let t: Vec<&str> = parts[3].split(':').collect();
    if t.len() != 3 {
        return Err(Error::Timestamp);
    }
    NaiveDate::from_ymd_opt(year, month as u32, u32::from(number(parts[0])?))
        .and_then(|d| {
            d.and_hms_opt(
                u32::from(number(t[0]).ok()?),
                u32::from(number(t[1]).ok()?),
                u32::from(number(t[2]).ok()?),
            )
        })
        .ok_or(Error::Timestamp)
}
pub fn format_date(t: NaiveDateTime) -> Result<[u8; 20], Error> {
    if !(1980..=2079).contains(&t.year()) {
        return Err(Error::Timestamp);
    }
    let mut b = [0; 20];
    let s = t.format("%d %b %y  %H:%M:%S").to_string();
    b[..19].copy_from_slice(s.as_bytes());
    Ok(b)
}

// Fixed Unicode mappings from the named IBM code pages (Python standard codecs).
const CP437: &str = "\u{c7}\u{fc}\u{e9}\u{e2}\u{e4}\u{e0}\u{e5}\u{e7}\u{ea}\u{eb}\u{e8}\u{ef}\u{ee}\u{ec}\u{c4}\u{c5}\u{c9}\u{e6}\u{c6}\u{f4}\u{f6}\u{f2}\u{fb}\u{f9}\u{ff}\u{d6}\u{dc}\u{a2}\u{a3}\u{a5}\u{20a7}\u{192}\u{e1}\u{ed}\u{f3}\u{fa}\u{f1}\u{d1}\u{aa}\u{ba}\u{bf}\u{2310}\u{ac}\u{bd}\u{bc}\u{a1}\u{ab}\u{bb}\u{2591}\u{2592}\u{2593}\u{2502}\u{2524}\u{2561}\u{2562}\u{2556}\u{2555}\u{2563}\u{2551}\u{2557}\u{255d}\u{255c}\u{255b}\u{2510}\u{2514}\u{2534}\u{252c}\u{251c}\u{2500}\u{253c}\u{255e}\u{255f}\u{255a}\u{2554}\u{2569}\u{2566}\u{2560}\u{2550}\u{256c}\u{2567}\u{2568}\u{2564}\u{2565}\u{2559}\u{2558}\u{2552}\u{2553}\u{256b}\u{256a}\u{2518}\u{250c}\u{2588}\u{2584}\u{258c}\u{2590}\u{2580}\u{3b1}\u{df}\u{393}\u{3c0}\u{3a3}\u{3c3}\u{b5}\u{3c4}\u{3a6}\u{398}\u{3a9}\u{3b4}\u{221e}\u{3c6}\u{3b5}\u{2229}\u{2261}\u{b1}\u{2265}\u{2264}\u{2320}\u{2321}\u{f7}\u{2248}\u{b0}\u{2219}\u{b7}\u{221a}\u{207f}\u{b2}\u{25a0}\u{a0}";
const CP850: &str = "\u{c7}\u{fc}\u{e9}\u{e2}\u{e4}\u{e0}\u{e5}\u{e7}\u{ea}\u{eb}\u{e8}\u{ef}\u{ee}\u{ec}\u{c4}\u{c5}\u{c9}\u{e6}\u{c6}\u{f4}\u{f6}\u{f2}\u{fb}\u{f9}\u{ff}\u{d6}\u{dc}\u{f8}\u{a3}\u{d8}\u{d7}\u{192}\u{e1}\u{ed}\u{f3}\u{fa}\u{f1}\u{d1}\u{aa}\u{ba}\u{bf}\u{ae}\u{ac}\u{bd}\u{bc}\u{a1}\u{ab}\u{bb}\u{2591}\u{2592}\u{2593}\u{2502}\u{2524}\u{c1}\u{c2}\u{c0}\u{a9}\u{2563}\u{2551}\u{2557}\u{255d}\u{a2}\u{a5}\u{2510}\u{2514}\u{2534}\u{252c}\u{251c}\u{2500}\u{253c}\u{e3}\u{c3}\u{255a}\u{2554}\u{2569}\u{2566}\u{2560}\u{2550}\u{256c}\u{a4}\u{f0}\u{d0}\u{ca}\u{cb}\u{c8}\u{131}\u{cd}\u{ce}\u{cf}\u{2518}\u{250c}\u{2588}\u{2584}\u{a6}\u{cc}\u{2580}\u{d3}\u{df}\u{d4}\u{d2}\u{f5}\u{d5}\u{b5}\u{fe}\u{de}\u{da}\u{db}\u{d9}\u{fd}\u{dd}\u{af}\u{b4}\u{ad}\u{b1}\u{2017}\u{be}\u{b6}\u{a7}\u{f7}\u{b8}\u{b0}\u{a8}\u{b7}\u{b9}\u{b3}\u{b2}\u{25a0}\u{a0}";
const CP866: &str = "\u{410}\u{411}\u{412}\u{413}\u{414}\u{415}\u{416}\u{417}\u{418}\u{419}\u{41a}\u{41b}\u{41c}\u{41d}\u{41e}\u{41f}\u{420}\u{421}\u{422}\u{423}\u{424}\u{425}\u{426}\u{427}\u{428}\u{429}\u{42a}\u{42b}\u{42c}\u{42d}\u{42e}\u{42f}\u{430}\u{431}\u{432}\u{433}\u{434}\u{435}\u{436}\u{437}\u{438}\u{439}\u{43a}\u{43b}\u{43c}\u{43d}\u{43e}\u{43f}\u{2591}\u{2592}\u{2593}\u{2502}\u{2524}\u{2561}\u{2562}\u{2556}\u{2555}\u{2563}\u{2551}\u{2557}\u{255d}\u{255c}\u{255b}\u{2510}\u{2514}\u{2534}\u{252c}\u{251c}\u{2500}\u{253c}\u{255e}\u{255f}\u{255a}\u{2554}\u{2569}\u{2566}\u{2560}\u{2550}\u{256c}\u{2567}\u{2568}\u{2564}\u{2565}\u{2559}\u{2558}\u{2552}\u{2553}\u{256b}\u{256a}\u{2518}\u{250c}\u{2588}\u{2584}\u{258c}\u{2590}\u{2580}\u{440}\u{441}\u{442}\u{443}\u{444}\u{445}\u{446}\u{447}\u{448}\u{449}\u{44a}\u{44b}\u{44c}\u{44d}\u{44e}\u{44f}\u{401}\u{451}\u{404}\u{454}\u{407}\u{457}\u{40e}\u{45e}\u{b0}\u{2219}\u{b7}\u{221a}\u{2116}\u{a4}\u{25a0}\u{a0}";
