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

//! Authenticated direct-hatch admission and private received provenance.
use super::*;

pub struct FileReceiveContext<'a> {
    pub transport: &'a BinkpPolicy,
    pub artifacts: &'a dyn NetworkArtifactStore,
}

// Flattening keeps existing metadata rows readable. The raw, credential-bearing
// TIC is held only in the restricted artifact store, never in this JSON or UI.
#[derive(Serialize, Deserialize)]
pub(super) struct StoredTic {
    #[serde(flatten)]
    pub metadata: tic::Metadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received_tic: Option<TicReceipt>,
}
#[derive(Serialize, Deserialize)]
pub(super) struct TicReceipt {
    pub artifact: String,
    pub peer: Endpoint,
    pub session: String,
    pub received_at: i64,
    pub direct_hatch: Option<tic::DirectHatchHistory>,
}

pub(super) fn unambiguous_hatch(
    meta: &tic::Metadata,
    policy: &Policy,
    transport: &BinkpPolicy,
) -> bool {
    let identities = [&meta.from, &meta.origin]
        .into_iter()
        .chain(meta.to.iter())
        .chain(meta.path.iter().map(|p| &p.address))
        .chain(meta.seen.iter());
    for identity in identities {
        if policy.akas.iter().map(|a| &a.endpoint)
            .chain(policy.links.iter().map(|l| &l.remote))
            .chain(transport.links.iter().flat_map(|l| &l.remote_akas))
            .any(|e| e.address == identity.address && e.domain != identity.domain)
            || policy.routes.iter().any(|r| r.domain != identity.domain && matches!(
                r.target, RouteMatch::Exact { address } | RouteMatch::Boss { address } if address == identity.address
            ))
        {
            return false;
        }
    }
    // Empty Seenby claims nothing; actual local-address evidence contradicts a
    // direct first arrival and must not become a compatibility bypass.
    !meta.seen.iter().any(|e| policy.local(e).is_some())
}
