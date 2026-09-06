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

//! FTS-0001.016 Type 2 and explicitly selected FSC-0048.002 Type-2+.
use super::*;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PacketProfile {
    Type2,
    Type2Plus,
}
/// Header evidence includes the legacy password; deliberately lacks Debug/serialization.
#[derive(Clone, Eq, PartialEq)]
pub struct PacketHeader {
    pub profile: PacketProfile,
    pub origin: Address,
    pub destination: Address,
    pub created: NaiveDateTime,
    pub password: [u8; 8],
    pub product: [u8; 4],
    pub product_data: [u8; 4],
    pub baud: u16,
    /// Retained Type 2 spare bytes, never interpreted as point data.
    pub spare: [u8; 20],
}
#[derive(Clone, Eq, PartialEq)]
pub struct PackedMessage {
    pub origin: (u16, u16),
    pub destination: (u16, u16),
    pub attributes: u16,
    pub cost: u16,
    pub date: [u8; 20],
    pub to: Vec<u8>,
    pub from: Vec<u8>,
    pub subject: Vec<u8>,
    pub text: Vec<u8>,
}
#[derive(Clone, Eq, PartialEq)]
pub struct Packet {
    pub header: PacketHeader,
    pub messages: Vec<PackedMessage>,
}
fn word(b: &[u8], i: usize) -> Result<u16, Error> {
    let x = b.get(i..i + 2).ok_or(Error::Packet)?;
    Ok(u16::from_le_bytes([x[0], x[1]]))
}
fn set(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
fn cstring(b: &[u8], at: &mut usize, max: usize) -> Result<Vec<u8>, Error> {
    let tail = b.get(*at..).ok_or(Error::Packet)?;
    let len = tail
        .iter()
        .take(max + 1)
        .position(|b| *b == 0)
        .ok_or(Error::Packet)?;
    let out = tail[..len].to_vec();
    *at += len + 1;
    Ok(out)
}
impl Packet {
    pub fn decode(bytes: &[u8], zone_context: (u16, u16)) -> Result<Self, Error> {
        if bytes.len() > MAX_PACKET {
            return Err(Error::Limit);
        }
        if bytes.len() < 60 || word(bytes, 18)? != 2 {
            return Err(Error::Packet);
        }
        // FSC-0045 Type 2.2 uses the baud slot as subversion; do not misparse it.
        if word(bytes, 16)? == 2 {
            return Err(Error::Profile);
        }
        let cw = word(bytes, 44)?;
        let plus = cw & 1 != 0 && cw.swap_bytes() == word(bytes, 40)?;
        let profile = if plus {
            PacketProfile::Type2Plus
        } else {
            PacketProfile::Type2
        };
        let mut zn = [word(bytes, 34)?, word(bytes, 36)?];
        if plus {
            for (i, z) in zn.iter_mut().enumerate() {
                let x = word(bytes, 46 + 2 * i)?;
                if x != 0 {
                    if *z != 0 && *z != x {
                        return Err(Error::Address);
                    }
                    *z = x;
                }
            }
        }
        if zn[0] == 0 {
            zn[0] = zone_context.0;
        }
        if zn[1] == 0 {
            zn[1] = zone_context.1;
        }
        let op = if plus { word(bytes, 50)? } else { 0 };
        let dp = if plus { word(bytes, 52)? } else { 0 };
        let mut net = word(bytes, 20)?;
        if plus && net == 65535 && op != 0 {
            net = word(bytes, 38)?;
        }
        let origin = Address::new(zn[0], net, word(bytes, 0)?, op)?;
        let destination = Address::new(zn[1], word(bytes, 22)?, word(bytes, 2)?, dp)?;
        let created = NaiveDate::from_ymd_opt(
            i32::from(word(bytes, 4)?),
            u32::from(word(bytes, 6)?) + 1,
            u32::from(word(bytes, 8)?),
        )
        .and_then(|d| {
            d.and_hms_opt(
                u32::from(word(bytes, 10).ok()?),
                u32::from(word(bytes, 12).ok()?),
                u32::from(word(bytes, 14).ok()?),
            )
        })
        .ok_or(Error::Timestamp)?;
        let header = PacketHeader {
            profile,
            origin,
            destination,
            created,
            password: bytes[26..34].try_into().map_err(|_| Error::Packet)?,
            product: [bytes[24], bytes[25], bytes[42], bytes[43]],
            product_data: bytes[54..58].try_into().map_err(|_| Error::Packet)?,
            baud: word(bytes, 16)?,
            spare: bytes[38..58].try_into().map_err(|_| Error::Packet)?,
        };
        let mut at = 58;
        let mut messages = vec![];
        loop {
            let kind = word(bytes, at)?;
            at += 2;
            if kind == 0 {
                if at != bytes.len() {
                    return Err(Error::Packet);
                }
                break;
            }
            if kind != 2 {
                return Err(Error::Packet);
            }
            if messages.len() >= MAX_MESSAGES {
                return Err(Error::Limit);
            }
            let h = bytes.get(at..at + 32).ok_or(Error::Packet)?;
            let mut m = PackedMessage {
                origin: (word(h, 4)?, word(h, 0)?),
                destination: (word(h, 6)?, word(h, 2)?),
                attributes: word(h, 8)?,
                cost: word(h, 10)?,
                date: h[12..32].try_into().map_err(|_| Error::Packet)?,
                to: vec![],
                from: vec![],
                subject: vec![],
                text: vec![],
            };
            at += 32;
            if m.origin.0 == 0 || m.destination.0 == 0 || m.date[19] != 0 {
                return Err(Error::Packet);
            }
            m.to = cstring(bytes, &mut at, 35)?;
            m.from = cstring(bytes, &mut at, 35)?;
            m.subject = cstring(bytes, &mut at, 71)?;
            m.text = cstring(bytes, &mut at, MAX_TEXT)?;
            messages.push(m);
        }
        Ok(Self { header, messages })
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        if self.messages.len() > MAX_MESSAGES {
            return Err(Error::Limit);
        }
        let h = &self.header;
        let mut b = vec![0; 58];
        if !(1..=9999).contains(&h.created.year()) {
            return Err(Error::Timestamp);
        }
        if h.profile == PacketProfile::Type2
            && (h.origin.point() != 0 || h.destination.point() != 0)
        {
            return Err(Error::Profile);
        }
        for (i, v) in [
            (0, h.origin.node()),
            (2, h.destination.node()),
            (4, h.created.year() as u16),
            (6, h.created.month0() as u16),
            (8, h.created.day() as u16),
            (10, h.created.hour() as u16),
            (12, h.created.minute() as u16),
            (14, h.created.second() as u16),
            (16, h.baud),
            (18, 2),
            (20, h.origin.net()),
            (22, h.destination.net()),
            (34, h.origin.zone()),
            (36, h.destination.zone()),
        ] {
            set(&mut b, i, v);
        }
        b[24] = h.product[0];
        b[25] = h.product[1];
        b[26..34].copy_from_slice(&h.password);
        if h.profile == PacketProfile::Type2Plus {
            if h.origin.point() != 0 {
                set(&mut b, 20, 65535);
                set(&mut b, 38, h.origin.net());
            }
            for (i, v) in [
                (40, 256),
                (44, 1),
                (46, h.origin.zone()),
                (48, h.destination.zone()),
                (50, h.origin.point()),
                (52, h.destination.point()),
            ] {
                set(&mut b, i, v);
            }
            b[42] = h.product[2];
            b[43] = h.product[3];
            b[54..58].copy_from_slice(&h.product_data);
        } else {
            b[38..58].copy_from_slice(&h.spare);
        }
        for m in &self.messages {
            if m.date[19] != 0 || m.origin.0 == 0 || m.destination.0 == 0 {
                return Err(Error::Packet);
            }
            for v in [
                2,
                m.origin.1,
                m.destination.1,
                m.origin.0,
                m.destination.0,
                m.attributes,
                m.cost,
            ] {
                b.extend(v.to_le_bytes());
            }
            b.extend(m.date);
            for (s, max) in [
                (&m.to, 35),
                (&m.from, 35),
                (&m.subject, 71),
                (&m.text, MAX_TEXT),
            ] {
                if s.len() > max || s.contains(&0) {
                    return Err(Error::Limit);
                }
                b.extend(s);
                b.push(0);
            }
            if b.len() + 2 > MAX_PACKET {
                return Err(Error::Limit);
            }
        }
        b.extend([0, 0]);
        Ok(b)
    }
}
