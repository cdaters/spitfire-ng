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

//! Typed administrative namespace. Authentication is supplied by the host.
use super::{Codename, Error, MessageId, NetworkId, NodeId};
use serde::{Deserialize, Serialize};
pub const MAX_CONTROLS: usize = 16;
pub const MAX_CONTROL_BYTES: usize = 65536;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Subscribe,
    Unsubscribe,
    QuerySubscriptions,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub network: NetworkId,
    pub id: MessageId,
    pub requester: NodeId,
    pub target: NodeId,
    pub operation: Operation,
    pub codename: Option<Codename>,
}
impl Request {
    pub fn validate(&self) -> Result<(), Error> {
        if self.id.origin() != self.requester
            || self.target == self.requester
            || (self.operation == Operation::QuerySubscriptions) != self.codename.is_none()
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn fingerprint(&self) -> Result<String, Error> {
        Ok(super::digest(&serde_json::to_vec(self)?))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Accepted,
    PendingApproval,
    Applied,
    AlreadySubscribed,
    AlreadyUnsubscribed,
    Denied,
    UnknownCodename,
    Unauthorized,
    Malformed,
    ReplayConflict,
}
impl Outcome {
    pub fn terminal(self) -> bool {
        !matches!(self, Self::Accepted | Self::PendingApproval)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriptionResult {
    pub network: NetworkId,
    pub id: MessageId,
    pub fingerprint: String,
    pub requester: NodeId,
    pub target: NodeId,
    pub outcome: Outcome,
    pub subscriptions: Vec<Codename>,
}
impl SubscriptionResult {
    pub fn for_request(r: &Request, outcome: Outcome) -> Result<Self, Error> {
        Ok(Self {
            network: r.network.clone(),
            id: r.id.clone(),
            fingerprint: r.fingerprint()?,
            requester: r.requester.clone(),
            target: r.target.clone(),
            outcome,
            subscriptions: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_namespace_strict_shapes_and_identity() {
        let mut r = Request {
            network: NetworkId::new("test").unwrap(),
            id: MessageId::new("END1:00000000000000000000000000000001").unwrap(),
            requester: NodeId::new("END1").unwrap(),
            target: NodeId::new("HOST1").unwrap(),
            operation: Operation::Subscribe,
            codename: Some(Codename::new("CNTEST").unwrap()),
        };
        r.validate().unwrap();
        let bytes = serde_json::to_vec(&r).unwrap();
        assert_eq!(serde_json::from_slice::<Request>(&bytes).unwrap(), r);
        r.codename = None;
        assert!(r.validate().is_err());
        r.operation = Operation::QuerySubscriptions;
        r.validate().unwrap();
        r.requester = NodeId::new("END2").unwrap();
        assert!(r.validate().is_err());
        let mut value = serde_json::to_value(&r).unwrap();
        value["operation"] = "create-conference".into();
        assert!(serde_json::from_value::<Request>(value).is_err());
    }
}
