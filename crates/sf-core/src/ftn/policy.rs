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

//! Static FTN identity, routing and directory-source configuration.
use super::*;
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub enabled: bool,
    pub akas: Vec<Aka>,
    pub links: Vec<Link>,
    pub routes: Vec<Route>,
    pub sources: Vec<DirectorySource>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aka {
    pub id: String,
    pub endpoint: Endpoint,
    pub enabled: bool,
    pub primary: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub id: String,
    pub remote: Endpoint,
    pub aka: String,
    pub enabled: bool,
    pub inbound: bool,
    pub outbound: bool,
    pub transit: bool,
    pub profile: PacketProfile,
    pub charset: Charset,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RouteMatch {
    Exact { address: Address },
    Boss { address: Address },
    Net { zone: u16, net: u16 },
    Zone { zone: u16 },
    Default,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub domain: Domain,
    pub target: RouteMatch,
    pub link: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectorySource {
    pub id: String,
    pub domain: Domain,
    pub enabled: bool,
    pub format: wire::directory::DirectoryFormat,
    pub charset: Charset,
    pub default_zone: u16,
    pub priority: u16,
    pub cadence_days: u16,
    pub require_crc: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RouteDecision {
    pub final_destination: Endpoint,
    pub next_hop: Endpoint,
    pub link: String,
    pub aka: String,
    pub reason: String,
}
impl Policy {
    pub fn validate(&self) -> Result<(), Error> {
        if self.akas.len() > 32
            || self.links.len() > 32
            || self.routes.len() > 256
            || self.sources.len() > 16
        {
            return Err(Error::Capacity);
        }
        let mut ids = BTreeSet::new();
        let mut endpoints = BTreeSet::new();
        let mut primaries = BTreeSet::new();
        for a in &self.akas {
            if !token(&a.id)
                || !ids.insert(&a.id)
                || !endpoints.insert(&a.endpoint)
                || (a.enabled && a.primary && !primaries.insert(&a.endpoint.domain))
            {
                return Err(Error::Policy);
            }
        }
        if self
            .akas
            .iter()
            .any(|a| a.enabled && !primaries.contains(&a.endpoint.domain))
        {
            return Err(Error::Policy);
        }
        ids.clear();
        endpoints.clear();
        for l in &self.links {
            let a = self.aka(&l.aka)?;
            if !token(&l.id)
                || !ids.insert(&l.id)
                || !endpoints.insert(&l.remote)
                || a.endpoint.domain != l.remote.domain
                || self.akas.iter().any(|a| a.endpoint == l.remote)
                || (l.profile == PacketProfile::Type2
                    && (a.endpoint.address.point() != 0 || l.remote.address.point() != 0))
            {
                return Err(Error::Policy);
            }
        }
        let mut keys = BTreeSet::new();
        for r in &self.routes {
            let l = self.link(&r.link)?;
            if r.domain != l.remote.domain || !keys.insert(format!("{}:{:?}", r.domain, r.target)) {
                return Err(Error::Policy);
            }
            match r.target {
                RouteMatch::Boss { address } if address.point() != 0 => return Err(Error::Policy),
                RouteMatch::Net { zone, net } if zone == 0 || net == 0 => {
                    return Err(Error::Policy)
                }
                RouteMatch::Zone { zone: 0 } => return Err(Error::Policy),
                _ => (),
            }
        }
        ids.clear();
        for s in &self.sources {
            if !token(&s.id)
                || !ids.insert(&s.id)
                || s.default_zone == 0
                || s.cadence_days == 0
                || s.cadence_days > 366
            {
                return Err(Error::Policy);
            }
        }
        Ok(())
    }
    pub fn aka(&self, id: &str) -> Result<&Aka, Error> {
        self.akas.iter().find(|a| a.id == id).ok_or(Error::Policy)
    }
    pub fn link(&self, id: &str) -> Result<&Link, Error> {
        self.links.iter().find(|a| a.id == id).ok_or(Error::Policy)
    }
    pub fn local(&self, e: &Endpoint) -> Option<&Aka> {
        self.akas.iter().find(|a| a.enabled && &a.endpoint == e)
    }
    pub fn route(&self, destination: &Endpoint) -> Result<RouteDecision, Error> {
        if !self.enabled || self.local(destination).is_some() {
            return Err(Error::Routing);
        }
        let mut choices: Vec<(u8, &str, &str)> = vec![];
        for r in self
            .routes
            .iter()
            .filter(|r| r.domain == destination.domain)
        {
            let a = destination.address;
            let score = match r.target {
                RouteMatch::Exact { address } if address == a => Some((0, "exact")),
                RouteMatch::Boss { address } if a.point() != 0 && address == a.boss() => {
                    Some((2, "boss"))
                }
                RouteMatch::Net { zone, net } if zone == a.zone() && net == a.net() => {
                    Some((3, "net"))
                }
                RouteMatch::Zone { zone } if zone == a.zone() => Some((4, "zone")),
                RouteMatch::Default => Some((5, "default")),
                _ => None,
            };
            if let Some((n, s)) = score {
                choices.push((n, &r.link, s));
            }
        }
        for l in &self.links {
            if l.remote == *destination {
                choices.push((1, &l.id, "direct"));
            }
        }
        choices.sort_by_key(|v| v.0);
        let (_, id, reason) = choices.first().ok_or(Error::Routing)?;
        let l = self.link(id)?;
        let a = self.aka(&l.aka)?;
        if !l.enabled || !l.outbound || !a.enabled {
            return Err(Error::Routing);
        }
        Ok(RouteDecision {
            final_destination: destination.clone(),
            next_hop: l.remote.clone(),
            link: l.id.clone(),
            aka: a.id.clone(),
            reason: reason.to_string(),
        })
    }
    pub fn digest(&self) -> Result<String, Error> {
        Ok(sf_net::qwk::digest(
            &serde_json::to_vec(self).map_err(|_| Error::Policy)?,
        ))
    }
}
