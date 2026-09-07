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

//! Native AreaFix response and explicit per-link historical delivery services.
use super::*;

pub(super) fn rescan_marker(text: &Text) -> Result<Option<wire::RescanSource>, Error> {
    let mut result = None;
    for c in &text.controls {
        // Match the codec's logical control spelling without changing custody.
        let logical: Vec<u8> = c
            .raw
            .iter()
            .copied()
            .filter(|b| !matches!(b, 10 | 141))
            .collect();
        if let Some(raw) = logical.strip_prefix(b"\x01RESCANNED ") {
            if result.is_some() {
                return Err(Error::Rejected);
            }
            result = Some(
                std::str::from_utf8(raw)
                    .map_err(|_| Error::Rejected)?
                    .parse()?,
            );
        }
    }
    Ok(result)
}

/// Only this metadata boundary may supply a missing domain. Offline toss has
/// no authenticated transport and cannot use the numeric compatibility form.
pub(super) fn resolve_rescan_source(
    marker: wire::RescanSource,
    policy: &Policy,
    authenticated: Option<&BinkpPolicy>,
    link: &Link,
    message_domain: Option<&Domain>,
    packet_origin: Address,
) -> Result<Endpoint, Error> {
    let domain = message_domain.ok_or(Error::Denied)?;
    if *domain != link.remote.domain {
        return Err(Error::Denied);
    }
    let resolved = match marker {
        wire::RescanSource::Qualified(endpoint) => endpoint,
        wire::RescanSource::Domainless(address) => {
            let transport = authenticated.ok_or(Error::Denied)?;
            transport.validate(policy)?;
            let peer = transport.link(&link.id)?;
            if !peer.enabled || address != link.remote.address || packet_origin != address {
                return Err(Error::Denied);
            }
            // Include disabled identities and other links' aliases: configuration
            // overlap must not become a default-domain guess or directory lookup.
            let ambiguous = policy.akas.iter().map(|a| &a.endpoint)
                .chain(policy.links.iter().map(|l| &l.remote))
                .chain(transport.links.iter().flat_map(|l| &l.remote_akas))
                .any(|e| e.address == address && e.domain != *domain)
                || policy.routes.iter().any(|r| r.domain != *domain && matches!(
                    r.target, RouteMatch::Exact { address: a } | RouteMatch::Boss { address: a } if a == address
                ));
            if ambiguous {
                return Err(Error::Denied);
            }
            Endpoint {
                address,
                domain: domain.clone(),
            }
        }
    };
    if resolved.domain != *domain || resolved.address != packet_origin {
        return Err(Error::Denied);
    }
    Ok(resolved)
}

struct RescanSelection {
    area: RescanArea,
    mapping: Mapping,
    publications: Vec<String>,
}
fn select_rescan(
    tx: &Transaction<'_>,
    policy: &Policy,
    d: &Downstream,
    areas: &[RescanArea],
    states: &BTreeMap<String, bool>,
    now: i64,
) -> Result<Vec<RescanSelection>, Error> {
    hub::downstream_valid(policy, d)?;
    let link = policy.link(&d.link)?;
    if !d.enabled
        || !d.rescan
        || !link.enabled
        || !link.outbound
        || areas.is_empty()
        || areas.len() > 32
    {
        return Err(Error::Denied);
    }
    let busy:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ftn_rescans r WHERE r.link_id=?1 AND (r.created_at>?2 OR EXISTS(SELECT 1 FROM ftn_routing_decisions d JOIN network_outbound_queue q USING(queue_id) WHERE d.delivery_key=r.request_id AND q.state NOT IN('accepted','cancelled'))))",params![d.link,now-i64::from(d.cooldown)],|r|r.get(0))?;
    if busy {
        return Err(Error::Held);
    }
    let mut total = 0u32;
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for area in areas {
        if !seen.insert(&area.area) || area.count == 0 || area.count > d.max_area {
            return Err(Error::Capacity);
        }
        total = total.checked_add(area.count).ok_or(Error::Capacity)?;
        if total > d.max_total {
            return Err(Error::Capacity);
        }
        let m = mapping(tx, &link.remote.domain, &area.area)?;
        let allowed = states
            .get(&area.area)
            .copied()
            .unwrap_or(hub::subscribed(tx, &d.link, &m.domain, &m.area)?);
        if !allowed
            || !m.send
            || !hub::access(tx, &m.domain, &m.area)?.rescan
            || policy.aka(&m.aka)?.endpoint.address.zone() != link.remote.address.zone()
        {
            return Err(Error::Denied);
        }
        let publications=tx.prepare("SELECT f.publication_id FROM ftn_messages f JOIN messages m USING(message_id) JOIN message_conferences c ON c.conference_id=m.conference_id WHERE f.domain=?1 AND f.area=?2 AND m.conference_id=?3 AND c.active=1 AND m.container_kind='conference' AND m.visibility='public' AND m.audience_kind='all-callers' AND m.lifecycle_state='active' ORDER BY m.message_id DESC LIMIT ?4")?.query_map(params![m.domain.as_str(),m.area,m.conference_id,area.count],|r|r.get(0))?.collect::<Result<_,_>>()?;
        result.push(RescanSelection {
            area: area.clone(),
            mapping: m,
            publications,
        });
    }
    Ok(result)
}
fn enqueue_rescan(
    tx: &Transaction<'_>,
    policy: &Policy,
    link: &str,
    request: &str,
    source: &str,
    selected: &[RescanSelection],
    now: i64,
) -> Result<u32, Error> {
    tx.execute(
        "INSERT INTO ftn_rescans VALUES(?1,?2,?3,?4)",
        params![request, link, source, now],
    )?;
    let l = policy.link(link)?;
    let mut count = 0;
    for selection in selected {
        for pid in &selection.publications {
            let (e, mid, _) = load(tx, pid)?;
            queue_target(
                tx,
                policy,
                pid,
                &e,
                mid,
                &RouteDecision {
                    final_destination: l.remote.clone(),
                    next_hop: l.remote.clone(),
                    link: link.into(),
                    aka: l.aka.clone(),
                    reason: "authorized-rescan".into(),
                },
                selection.mapping.version,
                request,
                now,
            )?;
            count += 1;
        }
        tx.execute(
            "INSERT INTO ftn_rescan_areas VALUES(?1,?2,?3,?4,?5)",
            params![
                request,
                selection.mapping.domain.as_str(),
                selection.area.area,
                selection.area.count,
                selection.publications.len() as u32
            ],
        )?;
    }
    event(tx, "rescan-queued", now)?;
    Ok(count)
}
impl RuntimeDatabase {
    pub fn rescan_ftn(
        &mut self,
        policy: &Policy,
        principal: &str,
        link: &str,
        areas: &[RescanArea],
        now: i64,
    ) -> Result<String, Error> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        bind(&tx, policy)?;
        let d = hub::downstream(&tx, link)?.ok_or(Error::Denied)?;
        let selected = select_rescan(&tx, policy, &d, areas, &BTreeMap::new(), now)?;
        let request = id();
        enqueue_rescan(&tx, policy, link, &request, "manual", &selected, now)?;
        audit(&tx, principal, "ftn.rescan-requested", now)?;
        tx.commit()?;
        Ok(request)
    }
}

struct Plan {
    commands: u32,
    states: BTreeMap<String, bool>,
    selected: Vec<RescanSelection>,
    body: String,
}
fn plan(
    tx: &Transaction<'_>,
    policy: &Policy,
    d: &Downstream,
    e: &Envelope,
    now: i64,
) -> Result<Plan, Error> {
    let commands = parse_areafix(&e.text.body, d.max_area)?;
    let mut states = BTreeMap::new();
    let mut rescans = Vec::new();
    let mut body = String::from("AreaFix request accepted.\n");
    for c in &commands {
        match c {
            AreaFixCommand::Subscribe(area)|AreaFixCommand::Unsubscribe(area)=>{
                let m=mapping(tx,&e.source.domain,area)?;
                if !m.send || policy.aka(&m.aka)?.endpoint.address.zone()!=e.source.address.zone(){return Err(Error::Denied)}
                let state=matches!(c,AreaFixCommand::Subscribe(_));
                if state && !hub::access(tx,&m.domain,area)?.remote_subscribe{return Err(Error::Denied)}
                // One deterministic final state per area; reject contradictory edits.
                if states.insert(area.clone(),state).is_some(){return Err(Error::Rejected)}
                body.push_str(&format!("{} {}\n",if state{"Subscribed:"}else{"Unsubscribed:"},area));
            },
            AreaFixCommand::Rescan(a)=>rescans.push(a.clone()),
            AreaFixCommand::Help=>body.push_str("%HELP  %LIST  %QUERY  +AREA  -AREA\n%RESCAN AREA [R=count]\nChanges are atomic. Rescans require permission and a subscription.\n"),
            _=>(),
        }
    }
    let selected = if rescans.is_empty() {
        Vec::new()
    } else {
        select_rescan(tx, policy, d, &rescans, &states, now)?
    };
    let areas = tx
        .prepare("SELECT area FROM ftn_area_mappings WHERE domain=?1 ORDER BY area LIMIT 257")?
        .query_map([e.source.domain.as_str()], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if areas.len() > 256 {
        return Err(Error::Capacity);
    }
    body.push_str("Current subscriptions:\n");
    for area in areas {
        let subscribed = states.get(&area).copied().unwrap_or(hub::subscribed(
            tx,
            &d.link,
            &e.source.domain,
            &area,
        )?);
        if subscribed {
            body.push_str(&format!("+{area}\n"));
        } else if commands.contains(&AreaFixCommand::List)
            && hub::access(tx, &e.source.domain, &area)?.remote_subscribe
        {
            body.push_str(&format!("Available: {area}\n"));
        }
    }
    Ok(Plan {
        commands: commands.len() as u32,
        states,
        selected,
        body,
    })
}
/// Runs inside the native member transaction. Neither credentials nor command text
/// enter request projections, native payloads, audit, errors or response subjects.
pub(super) fn areafix(
    tx: &Transaction<'_>,
    policy: &Policy,
    link: &str,
    e: &Envelope,
    verify: Option<&AreaFixVerifier<'_>>,
    now: i64,
) -> Result<bool, Error> {
    let identity = e.text.msgid.as_deref().ok_or(Error::Denied)?;
    let fingerprint = sf_net::qwk::digest(
        &serde_json::to_vec(&(&e.source, &e.destination, &e.from, &e.text.body))
            .map_err(|_| Error::Rejected)?,
    );
    let prior: Option<String> = tx
        .query_row(
            "SELECT fingerprint FROM ftn_areafix_requests WHERE link_id=?1 AND identity=?2",
            params![link, identity],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(prior) = prior {
        if prior != fingerprint {
            return Err(Error::Conflict);
        }
        event(tx, "areafix-replay-suppressed", now)?;
        return Ok(false);
    }
    let d = hub::downstream(tx, link)?;
    let l = policy.link(link)?;
    let authenticated = e.source == l.remote
        && d.as_ref().is_some_and(|d| d.enabled && d.areafix)
        && verify.is_some_and(|v| v(link, &e.subject));
    let request = id();
    let mut changes = 0;
    let mut rescans = 0;
    let mut commands = 0;
    let (result, body) = if authenticated {
        let d = d.as_ref().ok_or(Error::Denied)?;
        match plan(tx,policy,d,e,now) {
            Ok(mut p)=>{
                commands=p.commands;
                for (area,state) in &p.states {changes+=u32::from(hub::set_subscription(tx,link,&e.source.domain,area,*state,SubscriptionSource::Areafix,now)?);}
                if !p.selected.is_empty(){rescans=p.selected.len() as u32;let queued=enqueue_rescan(tx,policy,link,&request,"areafix",&p.selected,now)?;p.body.push_str(&format!("Rescan queued: {queued} messages.\n"));}
                ("accepted",p.body)
            },
            Err(Error::Policy)=>("unknown-area","Request rejected: unknown area. No changes applied.\n".into()),
            Err(Error::Denied)=>("unauthorized-area","Request rejected: area or rescan is not authorized. No changes applied.\n".into()),
            Err(Error::Capacity|Error::Held)=>("bounded-request-rejected","Request rejected: request limit, cooldown or pending rescan. No changes applied.\n".into()),
            Err(Error::Rejected)=>("malformed-request","Request rejected: malformed command. No changes applied.\n".into()),
            Err(e)=>return Err(e),
        }
    } else {
        (
            "authentication-rejected",
            "AreaFix authorization failed. No changes applied.\n".into(),
        )
    };
    // Never route a rejection to a spoofed source. A configured authenticated
    // transport with an exact source may receive a safe password rejection.
    let response = if verify.is_some() && e.source == l.remote && l.outbound {
        let aka = policy.local(&e.destination).ok_or(Error::Routing)?;
        let text = new_text(
            &body,
            e.text.charset,
            &aka.endpoint,
            serial(tx, &aka.endpoint)?,
            Some(identity),
            Some(&l.remote),
        )?;
        let response = Envelope {
            source: aka.endpoint.clone(),
            destination: l.remote.clone(),
            from: "AreaFix".into(),
            to: e.from.clone(),
            subject: "AreaFix result".into(),
            text,
            date: wire::format_date(
                chrono::DateTime::from_timestamp(now, 0)
                    .ok_or(Error::Rejected)?
                    .naive_utc(),
            )?,
            attributes: 1,
            cost: 0,
        };
        let mid = insert_native(
            tx,
            &response,
            None,
            None,
            None,
            None,
            now,
            super::mail::NativeIdentity::System,
        )?;
        let pid = publish(tx, &response, mid, None, now)?;
        queue_target(
            tx,
            policy,
            &pid,
            &response,
            mid,
            &RouteDecision {
                final_destination: l.remote.clone(),
                next_hop: l.remote.clone(),
                link: link.into(),
                aka: aka.id.clone(),
                reason: "areafix-response".into(),
            },
            0,
            "normal",
            now,
        )?;
        Some(mid)
    } else {
        None
    };
    tx.execute(
        "INSERT INTO ftn_areafix_requests VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            request,
            link,
            identity,
            fingerprint,
            authenticated,
            commands,
            changes,
            rescans,
            result,
            response,
            now
        ],
    )?;
    event(
        tx,
        if authenticated && result == "accepted" {
            "areafix-accepted"
        } else {
            "areafix-rejected"
        },
        now,
    )?;
    Ok(true)
}

#[cfg(test)]
mod rescan_context_tests {
    use super::*;

    #[test]
    fn missing_or_inconsistent_message_domain_is_never_guessed() {
        let link = Link {
            posting_identity: Default::default(),
            id: "peer".into(),
            remote: "90:100/1@interop".parse().unwrap(),
            aka: "local".into(),
            enabled: true,
            inbound: true,
            outbound: true,
            transit: false,
            profile: PacketProfile::Type2Plus,
            charset: Charset::Utf8,
        };
        for marker in ["90:100/1", "90:100/1@interop"] {
            for domain in [
                None,
                Some("fidonet".parse::<Domain>().unwrap()),
                Some("unknown".parse().unwrap()),
            ] {
                assert!(matches!(
                    resolve_rescan_source(
                        marker.parse().unwrap(),
                        &Policy::default(),
                        Some(&BinkpPolicy::default()),
                        &link,
                        domain.as_ref(),
                        link.remote.address,
                    ),
                    Err(Error::Denied)
                ));
            }
        }
    }
}
