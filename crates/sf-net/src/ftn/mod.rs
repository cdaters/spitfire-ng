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

//! FTSC wire types. No filesystem, caller, routing, or database authority.
pub mod directory;
mod packet;
mod text;
pub use packet::*;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
pub use text::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("invalid FTN address or domain")]
    Address,
    #[error("malformed or truncated FTN packet")]
    Packet,
    #[error("unsupported FTN packet profile")]
    Profile,
    #[error("FTN resource limit reached")]
    Limit,
    #[error("invalid FTN control metadata")]
    Control,
    #[error("unsupported or invalid FTN encoding")]
    Encoding,
    #[error("invalid FTN timestamp")]
    Timestamp,
    #[error("malformed directory generation")]
    Directory,
    #[error("directory checksum mismatch")]
    Checksum,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Address {
    zone: u16,
    net: u16,
    node: u16,
    point: u16,
}
impl Address {
    pub fn new(zone: u16, net: u16, node: u16, point: u16) -> Result<Self, Error> {
        if zone == 0 || net == 0 {
            return Err(Error::Address);
        }
        Ok(Self {
            zone,
            net,
            node,
            point,
        })
    }
    pub fn zone(self) -> u16 {
        self.zone
    }
    pub fn net(self) -> u16 {
        self.net
    }
    pub fn node(self) -> u16 {
        self.node
    }
    pub fn point(self) -> u16 {
        self.point
    }
    pub fn boss(self) -> Self {
        Self { point: 0, ..self }
    }
    pub fn with_point(self, point: u16) -> Self {
        Self { point, ..self }
    }
    pub fn two_d(self) -> (u16, u16) {
        (self.net, self.node)
    }
}
fn number(s: &str) -> Result<u16, Error> {
    if s.is_empty() || s.len() > 5 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Address);
    }
    s.parse().map_err(|_| Error::Address)
}
impl FromStr for Address {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        let (z, rest) = s.split_once(':').ok_or(Error::Address)?;
        let (n, rest) = rest.split_once('/').ok_or(Error::Address)?;
        let (node, p) = rest.split_once('.').unwrap_or((rest, "0"));
        Self::new(number(z)?, number(n)?, number(node)?, number(p)?)
    }
}
impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}/{}", self.zone, self.net, self.node)?;
        if self.point != 0 {
            write!(f, ".{}", self.point)?;
        }
        Ok(())
    }
}
impl TryFrom<String> for Address {
    type Error = Error;
    fn try_from(s: String) -> Result<Self, Error> {
        s.parse()
    }
}
impl From<Address> for String {
    fn from(a: Address) -> Self {
        a.to_string()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Domain(String);
impl Domain {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl FromStr for Domain {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        if s.is_empty()
            || s.len() > 32
            || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !s.as_bytes()[0].is_ascii_alphanumeric()
            || !s.as_bytes()[s.len() - 1].is_ascii_alphanumeric()
        {
            return Err(Error::Address);
        }
        Ok(Self(s.to_ascii_lowercase()))
    }
}
impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl TryFrom<String> for Domain {
    type Error = Error;
    fn try_from(s: String) -> Result<Self, Error> {
        s.parse()
    }
}
impl From<Domain> for String {
    fn from(d: Domain) -> Self {
        d.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub domain: Domain,
    pub address: Address,
}
impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.address, self.domain)
    }
}
impl FromStr for Endpoint {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        let (a, d) = s.split_once('@').ok_or(Error::Address)?;
        Ok(Self {
            domain: d.parse()?,
            address: a.parse()?,
        })
    }
}

pub const MAX_PACKET: usize = 16 * 1024 * 1024;
pub const MAX_MESSAGES: usize = 1000;
pub const MAX_TEXT: usize = 80 * 1024;
pub const MAX_CONTROLS: usize = 128;
pub const MAX_CONTROL_BYTES: usize = 16 * 1024;
pub const MAX_CONTROL_LINE: usize = 1024;
pub const MAX_HOPS: usize = 128;

#[cfg(test)]
mod tests;
