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

//! Networks cockpit: safe projections and typed daemon requests only.
use crate::{
    model::{ConnectionState, MonitorModel},
    worker::{MonitorWorker, WorkerCommand},
};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use sf_bbs::{NetworkAction, NetworkResult};
use sf_core::{
    ftn::{NetworkQuery, NetworkSection as Section},
    LocalizationArgs,
};
fn t(k: &str) -> String {
    sf_core::text(k, &LocalizationArgs::new())
}
fn event_policy(policy: sf_core::events::ExchangePolicy) -> String {
    use sf_core::events::ExchangePolicy as P;
    t(match policy {
        P::Immediate => "events-immediate",
        P::Scheduled => "events-scheduled",
        P::Manual => "events-manual",
        P::Hybrid => "events-hybrid",
    })
}
fn event_action(action: &sf_core::events::Action) -> String {
    match action {
        sf_core::events::Action::ConferenceHealth => t("health-title"),
        sf_core::events::Action::Circuitnet { network, node } => format!(
            "CircuitNET / {network} / {}",
            node.as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| t("events-all-peers"))
        ),
        sf_core::events::Action::Binkp { link } => format!("BinkP / {link}"),
    }
}
fn event_schedule(schedule: &sf_core::events::Schedule) -> String {
    use sf_core::events::Schedule as S;
    match schedule {
        S::Manual => t("events-manual"),
        S::Interval { seconds } => {
            format!("{} {seconds} {}", t("events-every"), t("events-seconds"))
        }
        S::Daily { hour, minute, days } => format!(
            "{} {hour:02}:{minute:02} / {} {days:?}",
            t("events-daily"),
            t("events-days")
        ),
    }
}
fn safe(s: impl ToString) -> String {
    s.to_string()
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
        .take(512)
        .collect()
}
fn time(v: Option<i64>) -> String {
    v.and_then(|n| chrono::DateTime::from_timestamp(n, 0))
        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "—".into())
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub query: NetworkQuery,
    pub selected: usize,
    pub detail: bool,
    pub pending: Option<NetworkAction>,
    pub input: Option<String>,
    pub status: String,
    pub lookup: Option<sf_core::ftn::DirectoryLookup>,
}
impl Model {
    pub fn result(&mut self, result: &NetworkResult) {
        self.status = match result {
            NetworkResult::Binkp {
                response: sf_bbs::binkp::Result::Started { .. },
            } => t("networks-started"),
            NetworkResult::Rejected { reason } => t(match reason.as_str() {
                "ftn-held" => "networks-held-help",
                "ftn-conflict" => "sfconfig-conflict",
                "ftn-routing" => "networks-connect-help",
                "ftn-policy" => "netconfig-invalid",
                "ftn-denied" => "sfmonitor-action-denied",
                _ => "networks-action-rejected",
            }),
            NetworkResult::Replayed { .. } => t("networks-replayed"),
            _ => t("networks-updated"),
        };
    }
}
pub fn guidance(code: Option<&str>) -> String {
    t(match code {
        Some("authentication") => "networks-auth-help",
        Some("address") => "networks-address-help",
        Some("connect" | "unavailable") => "networks-connect-help",
        Some("timeout") => "networks-timeout-help",
        Some("interrupted" | "cancelled") => "networks-interrupted-help",
        Some("malformed" | "limit") => "networks-protocol-help",
        Some("custody") => "networks-custody-help",
        Some(_) => "networks-action-rejected",
        None => "networks-no-error",
    })
}
fn request(model: &mut MonitorModel, worker: &MonitorWorker) {
    model.networks.detail = false;
    model.networks.lookup = None;
    model.networks.selected = 0;
    if !worker.send(WorkerCommand::Networks(model.networks.query.clone())) {
        model.networks.status = t("networks-busy")
    }
}
pub fn key(model: &mut MonitorModel, worker: &MonitorWorker, key: KeyEvent) -> bool {
    if !matches!(model.connection, ConnectionState::Connected { .. })
        && matches!(key.code, KeyCode::Char('r' | 'R' | 'f' | 'F'))
    {
        crate::request_refresh(model, worker);
        return true;
    }
    if let Some(input) = &mut model.networks.input {
        match key.code {
            KeyCode::Esc => model.networks.input = None,
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) if !c.is_control() && input.len() < 96 => input.push(c),
            KeyCode::Enter => {
                let input = model.networks.input.take().unwrap_or_default();
                match input.parse() {
                    Ok(e) => {
                        worker.send(WorkerCommand::NetworkLookup(e));
                    }
                    Err(_) => model.networks.status = t("networks-invalid-address"),
                }
            }
            _ => {}
        }
        return true;
    }
    if model.networks.pending.is_some() {
        match key.code {
            KeyCode::Esc => model.networks.pending = None,
            KeyCode::Enter => {
                let action = model.networks.pending.take().unwrap();
                if matches!(model.connection, ConnectionState::Connected { .. })
                    && model
                        .snapshot
                        .authorized_capabilities
                        .contains(&action.capability())
                {
                    if worker.send(WorkerCommand::NetworkAction {
                        command_id: crate::new_command_id(),
                        action,
                    }) {
                        model.networks.status = t("networks-requested")
                    } else {
                        model.networks.status = t("networks-busy")
                    }
                } else {
                    model.networks.status = t("sfmonitor-action-denied")
                }
            }
            _ => {}
        }
        return true;
    }
    match key.code {
        KeyCode::Char('e') => {
            model.networks.query = NetworkQuery {
                section: Section::Events,
                offset: 0,
            };
            request(model, worker);
        }
        KeyCode::Char('r') if model.networks.query.section == Section::Events => {
            if let Some(event) = model
                .snapshot
                .networks
                .as_ref()
                .and_then(|s| s.events.get(model.networks.selected))
            {
                model.networks.pending = Some(NetworkAction::Event {
                    request: sf_bbs::events::Command::Run {
                        id: event.definition.id.clone(),
                        expected: event.version,
                    },
                });
            }
        }
        KeyCode::Char('f') => request(model, worker),
        KeyCode::Char(c @ '1'..='9') => {
            model.networks.query = NetworkQuery {
                section: Section::ALL[c as usize - '1' as usize],
                offset: 0,
            };
            request(model, worker);
        }
        KeyCode::Char(']') => {
            if model
                .snapshot
                .networks
                .as_ref()
                .is_some_and(|s| s.page.more)
            {
                model.networks.query.offset +=
                    if model.networks.query.section == Section::Circuitnet {
                        16
                    } else {
                        100
                    };
                request(model, worker)
            }
        }
        KeyCode::Char('[') => {
            model.networks.query.offset = model.networks.query.offset.saturating_sub(
                if model.networks.query.section == Section::Circuitnet {
                    16
                } else {
                    100
                },
            );
            request(model, worker)
        }
        KeyCode::Up => model.networks.selected = model.networks.selected.saturating_sub(1),
        KeyCode::Down => {
            model.networks.selected = model
                .networks
                .selected
                .saturating_add(1)
                .min(rows(model).len().saturating_sub(1))
        }
        KeyCode::PageDown => {
            model.networks.selected = model
                .networks
                .selected
                .saturating_add(10)
                .min(rows(model).len().saturating_sub(1))
        }
        KeyCode::PageUp => model.networks.selected = model.networks.selected.saturating_sub(10),
        KeyCode::Home => model.networks.selected = 0,
        KeyCode::End => model.networks.selected = rows(model).len().saturating_sub(1),
        KeyCode::Enter => model.networks.detail = !model.networks.detail,
        KeyCode::Esc => {
            model.networks.detail = false;
            model.networks.lookup = None;
        }
        KeyCode::Char('l') if model.networks.query.section == Section::Directory => {
            model.networks.input = Some(String::new())
        }
        KeyCode::Char('a') if model.networks.query.section == Section::Directory => {
            if let Some(s) = &model.snapshot.networks {
                if s.page.query == model.networks.query {
                    if let Some(g) = s.page.directory.get(model.networks.selected) {
                        if g.state == "validated" && !g.active {
                            model.networks.pending = Some(NetworkAction::Ftn {
                                request: sf_bbs::ftn::Action::DirectoryActivate {
                                    generation: g.generation.clone(),
                                    expected: g.version,
                                },
                            });
                        }
                    }
                }
            }
        }
        KeyCode::Char('s') if model.networks.query.section != Section::Circuitnet => {
            model.networks.pending = Some(NetworkAction::Ftn {
                request: sf_bbs::ftn::Action::Scan,
            })
        }
        KeyCode::Char(c @ ('a' | 'd')) if model.networks.query.section == Section::Circuitnet => {
            if let Some(s) = &model.snapshot.networks {
                let mut index = model.networks.selected;
                for n in &s.circuitnet {
                    if index == 0 {
                        break;
                    }
                    index -= 1;
                    if index < n.peers.len() {
                        break;
                    }
                    index -= n.peers.len();
                    if let Some(e) = n.controls.get(index) {
                        if e.direction == "incoming"
                            && e.result.outcome
                                == sf_core::circuitnet::control::Outcome::PendingApproval
                        {
                            let request = sf_bbs::circuitnet_live::Action::DecideControl {
                                network: n.network.clone(),
                                id: e.request.id.clone(),
                                approve: c == 'a',
                            };
                            if model
                                .snapshot
                                .authorized_capabilities
                                .contains(&request.capability())
                            {
                                model.networks.pending =
                                    Some(NetworkAction::Circuitnet { request });
                            } else {
                                model.networks.status = t("sfmonitor-action-denied");
                            }
                        }
                        break;
                    }
                    index = index.saturating_sub(n.controls.len());
                }
            }
        }
        KeyCode::Char(c @ ('t' | 'p' | 'h' | 'r'))
            if model.networks.query.section == Section::Circuitnet =>
        {
            if let Some(s) = &model.snapshot.networks {
                let mut index = model.networks.selected;
                for n in &s.circuitnet {
                    if index == 0 {
                        break;
                    }
                    index -= 1;
                    if let Some(p) = n.peers.get(index) {
                        use sf_bbs::circuitnet_live::Action;
                        let request = match c {
                            't' => Action::Test {
                                network: n.network.clone(),
                                node: p.node.clone(),
                            },
                            'p' => Action::Poll {
                                network: n.network.clone(),
                                node: p.node.clone(),
                            },
                            _ => Action::Hold {
                                network: n.network.clone(),
                                node: p.node.clone(),
                                held: c == 'h',
                                expected: n.version,
                            },
                        };
                        if model
                            .snapshot
                            .authorized_capabilities
                            .contains(&request.capability())
                        {
                            model.networks.pending = Some(NetworkAction::Circuitnet { request });
                        } else {
                            model.networks.status = t("sfmonitor-action-denied");
                        }
                        break;
                    }
                    index = index.saturating_sub(n.peers.len() + n.controls.len());
                }
            }
        }
        KeyCode::Char(c @ ('t' | 'p' | 'h' | 'r')) => {
            if let Some(s) = &model.snapshot.networks {
                if s.page.query != model.networks.query {
                    return true;
                }
                let idx = model.networks.selected;
                let action = match model.networks.query.section {
                    Section::Links => s
                        .ftn
                        .links
                        .get(idx)
                        .and_then(|l| {
                            if !s.transport.links.iter().any(|t| t.link == l.id) {
                                return None;
                            }
                            match c {
                                't' => Some(sf_bbs::binkp::Action::Test {
                                    link: l.id.clone(),
                                    expected: s.binkp.policy.clone(),
                                }),
                                'p' => Some(sf_bbs::binkp::Action::Poll {
                                    link: l.id.clone(),
                                    expected: s.binkp.policy.clone(),
                                }),
                                _ => None,
                            }
                        })
                        .map(|request| NetworkAction::Binkp { request }),
                    Section::Queues => s.page.queues.get(idx).and_then(|q| {
                        if q.adapter == "ftn" {
                            match c {
                                'h' => Some(NetworkAction::Binkp {
                                    request: sf_bbs::binkp::Action::Hold {
                                        queue: q.id.clone(),
                                        expected: q.version,
                                    },
                                }),
                                'r' => Some(NetworkAction::Binkp {
                                    request: sf_bbs::binkp::Action::Release {
                                        queue: q.id.clone(),
                                        expected: q.version,
                                    },
                                }),
                                _ => None,
                            }
                        } else if c == 'r' {
                            Some(NetworkAction::Retry {
                                queue: q.id.clone(),
                                expected: q.version,
                            })
                        } else {
                            None
                        }
                    }),
                    _ => None,
                };
                if let Some(a) = action {
                    if model
                        .snapshot
                        .authorized_capabilities
                        .contains(&a.capability())
                    {
                        model.networks.pending = Some(a)
                    } else {
                        model.networks.status = t("sfmonitor-action-denied")
                    }
                }
            }
        }
        _ => return false,
    }
    true
}
fn control_outcome(outcome: sf_core::circuitnet::control::Outcome) -> String {
    use sf_core::circuitnet::control::Outcome;
    t(match outcome {
        Outcome::Accepted => "circuitnet-accepted",
        Outcome::PendingApproval => "circuitnet-pending-approval",
        Outcome::Applied => "circuitnet-applied",
        Outcome::AlreadySubscribed => "circuitnet-already-subscribed",
        Outcome::AlreadyUnsubscribed => "circuitnet-already-unsubscribed",
        Outcome::Denied => "circuitnet-denied",
        Outcome::UnknownCodename => "circuitnet-unknown-conference",
        Outcome::Unauthorized => "circuitnet-unauthorized",
        Outcome::Malformed => "circuitnet-malformed",
        Outcome::ReplayConflict => "circuitnet-replay-conflict",
    })
}
fn rows(model: &MonitorModel) -> Vec<String> {
    let Some(s) = &model.snapshot.networks else {
        return vec![t("networks-access")];
    };
    if s.page.query != model.networks.query {
        return vec![t("sfmonitor-loading")];
    }
    match model.networks.query.section {
        Section::Events => s.events.iter().map(|e|format!("{} / {} / {}: {} / {}: {}", e.definition.name,
            if e.running {t("events-running")}else if !e.definition.enabled{t("events-disabled")}else{e.last_result.as_ref().map(|r|t(&format!("events-result-{r}"))).unwrap_or_else(||"—".into())},
            t("events-next"),time(e.next_due),t("events-last"),time(e.last_completed))).collect(),
        Section::Circuitnet => s.circuitnet.iter().flat_map(|n| {
            let mut lines=vec![format!("{} / {} / {:?} | {}: {} | {}: {}",n.network,n.local,n.role,t("circuitnet-listener"),n.listening,t("circuitnet-authentication"),n.credential)];
            lines.push(format!("{}: {} / {} / {}: {} / {}: {} / {}: {} / {}", t("catalog-revision"), n.catalog.revision, n.catalog.authority.as_ref().map_or("—",|a|a.publisher.as_str()), t("catalog-pending"), n.catalog.pending_mapping, t("catalog-required"), n.catalog.required_unmapped, t("catalog-rejected"), n.catalog.rejected, n.catalog.last_error.as_deref().unwrap_or("—")));
            if n.catalog.pending_sync {lines.push(t("catalog-sync-pending"));}
            lines.push(format!("{}: {} / {}: {} / {}: {}",t("files-distribution"),n.files.pending,t("files-area-codename"),n.files.mappings.len(),t("files-subscription"),n.files.dossiers.iter().filter(|d|d.subscribed).count()));
            if let Some(files)=&s.native_files {lines.push(format!("{}: {} / {}: {} / {}: {} / {}: {}",t("files-native"),files.published,t("files-pending-approval"),files.pending_approval,t("files-quarantine"),files.quarantined,t("files-scanner-unavailable"),files.scanner_unavailable));}
            lines.extend(n.peers.iter().map(|p|format!("{} {:?} / {}:{} | {}: {} | {}: {} | {}: {} | {}: {} | {}: {} | {}",
                p.node,p.role,p.host,p.port,t("circuitnet-held"),p.held,t("networks-queues"),p.queued,
                t("circuitnet-last-attempt"),time(p.health.as_ref().map(|h|h.last_attempt)),t("circuitnet-last-contact"),time(p.health.as_ref().and_then(|h|h.last_success)),
                t("circuitnet-active"),p.active,p.health.as_ref().map_or("—",|h|h.result.as_str()))));
            lines.extend(n.controls.iter().map(|e|format!("{} / {} / {:?} / {} / {}", t("circuitnet-remote-request"),e.request.id,e.request.operation,e.request.codename.as_ref().map_or("—",sf_core::circuitnet::Codename::as_str),control_outcome(e.result.outcome))));
            lines
        }).collect(),
        Section::Overview => vec![
            format!(
                "QWK: {} | FTN: {} ({}) | BinkP {}: {}",
                s.qwk.len(),
                s.ftn.links.len(),
                t(if s.ftn.enabled {
                    "sfconfig-enabled"
                } else {
                    "sfconfig-disabled"
                }),
                t("binkp-listener"),
                s.binkp.listener_enabled
            ),
            format!(
                "{}: {}",
                t("networks-queues"),
                s.page
                    .queue_counts
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("  ")
            ),
            format!(
                "{}: {} | {}: {}",
                t("networks-quarantine"),
                s.page.counters.get("quarantine").copied().unwrap_or(0),
                t("networks-directory"),
                s.page.counters.get("directory").copied().unwrap_or(0)
            ),
            format!(
                "{}: {}",
                t("networks-statistics"),
                s.page
                    .counters
                    .iter()
                    .filter(|(k, _)| *k != "directory" && *k != "quarantine")
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("  ")
            ),
            t("networks-authority"),
            t("networks-safety"),
        ],
        Section::Links => s
            .ftn
            .links
            .iter()
            .map(|l| {
                let health = s
                    .binkp
                    .links
                    .iter()
                    .find(|h| h.link == l.id)
                    .and_then(|h| h.health.as_ref());
                format!(
                    "FTN {} | {} | {} {} | {} | {}",
                    l.id,
                    l.remote,
                    t("ftn-aka"),
                    l.aka,
                    if !s.ftn.enabled
                        || !l.enabled
                        || s.transport
                            .links
                            .iter()
                            .any(|t| t.link == l.id && !t.enabled)
                    {
                        "disabled"
                    } else if health.is_some_and(|h| h.active) {
                        "active"
                    } else if health.is_some_and(|h| h.held) {
                        "held"
                    } else {
                        "idle"
                    },
                    time(health.and_then(|h| h.last_success))
                )
            })
            .chain(s.qwk.iter().map(|q| {
                format!(
                    "QWK {} | {} / {} | {} | {}",
                    q.link.name,
                    q.link.network,
                    q.link.remote_id,
                    q.last_result,
                    time(q.last_contact)
                )
            }))
            .collect(),
        Section::Queues => s
            .page
            .queues
            .iter()
            .map(|q| {
                format!(
                    "{} {} | {} | {} | {} | {} {}",
                    q.adapter,
                    &q.id[..q.id.len().min(12)],
                    q.link,
                    q.kind,
                    q.state,
                    t("networks-attempts"),
                    q.attempts
                )
            })
            .collect(),
        Section::Areas => s
            .page
            .areas
            .iter()
            .map(|a| {
                format!(
                    "FTN {}@{} → {} | {} | RX={} TX={}",
                    a.area, a.domain, a.conference_id, a.aka, a.receive, a.send
                )
            })
            .chain(s.qwk.iter().flat_map(|q| {
                q.mappings.iter().map(|m| {
                    format!(
                        "QWK {} / {} → {} | RX={} TX={}",
                        q.link.id, m.wire_conference, m.conference_id, m.inbound, m.outbound
                    )
                })
            }))
            .collect(),
        Section::Directory => s
            .page
            .directory
            .iter()
            .map(|g| {
                format!(
                    "{}@{} | {} | {} | {} {} | {}",
                    g.source,
                    g.domain,
                    g.date,
                    if g.active { "active" } else { &g.state },
                    g.records,
                    t("networks-records"),
                    if chrono::NaiveDate::parse_from_str(&g.date, "%Y-%m-%d")
                        .ok()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                        .is_some_and(|d| s.now - d.and_utc().timestamp()
                            > i64::from(g.cadence_days) * 2 * 86400)
                    {
                        t("networks-stale")
                    } else {
                        t("networks-current")
                    }
                )
            })
            .chain(s.ftn.sources.iter().map(|src| {
                format!(
                    "{}: {}@{} | {:?} | {}={} | {}",
                    t("networks-source"),
                    src.id,
                    src.domain,
                    src.format,
                    t("networks-priority"),
                    src.priority,
                    if src.enabled { "enabled" } else { "disabled" }
                )
            }))
            .collect(),
        Section::Quarantine => s
            .page
            .quarantine
            .iter()
            .map(|q| {
                format!(
                    "{} | {} | {} | {} | {} bytes",
                    q.id,
                    q.link.as_deref().unwrap_or("—"),
                    q.reason,
                    time(Some(q.received)),
                    q.bytes.unwrap_or(0)
                )
            })
            .chain(s.files.iter().flat_map(|f|f.activity.iter()).filter(|a|a.result.contains("rejected") || a.result.contains("denied") || a.result.contains("mismatch") || a.result.contains("unsafe") || a.result.contains("expired") || a.result.contains("conflict") || a.result.contains("exceeded")).map(|a|format!("{} | {} | {} | {} bytes",a.link.as_deref().unwrap_or("—"),a.result,time(Some(a.time)),a.bytes)))
            .collect(),
        Section::Files => s.files.as_ref().map(|files| {
            let mut rows=vec![format!("{}: {} | {} bytes | FREQ {}",t("netfiles-staging"),files.staged,files.staged_bytes,files.policy.freq)];
            rows.extend(files.areas.iter().map(|a|format!("FileEcho {}@{} / {} {} / in {} / out {} / v{}",a.tag,a.domain,t("netfiles-native-area"),a.native_area,a.inbound,a.outbound,a.version)));
            rows.extend(files.subscriptions.iter().map(|s|format!("{} / {} / in {} / out {} / hold {}",s.link,s.tag,s.inbound,s.subscribed,s.held)));
            rows.extend(files.queue.iter().map(|q|format!("{} / {} / {} / {} bytes / SHA256 {} / payload {} / TIC {} / hold {} / attempts {} / {}",q.filename,q.link,format_args!("{} / {} / {} / {}",q.kind,q.area.as_deref().unwrap_or("—"),q.origin.as_deref().unwrap_or("—"),q.provenance.as_deref().unwrap_or("—")),q.size,q.sha256,q.payload_accepted,q.tic_accepted,q.held,q.attempts,q.last_error.as_deref().unwrap_or("—"))));
            rows.extend(files.activity.iter().map(|a|format!("{} / {} / {} / {} / {} files / {} bytes",time(Some(a.time)),a.link.as_deref().unwrap_or("—"),a.filename.as_deref().unwrap_or("—"),a.result,a.files,a.bytes)));
            rows
        }).unwrap_or_else(||vec![t("networks-access")]),
        Section::Hub => s
            .page
            .downstreams
            .iter()
            .map(|d| {
                format!(
                    "{} | {} | {} | AreaFix {} | Rescan {} | {} {}",
                    d.link,
                    t(if d.held {
                        "networks-downstream-held"
                    } else if d.enabled {
                        "sfconfig-enabled"
                    } else {
                        "sfconfig-disabled"
                    }),
                    d.boss_aka
                        .as_deref()
                        .map(|a| format!("{} {a}", t("networks-boss")))
                        .unwrap_or_else(|| t("networks-downstream")),
                    d.areafix,
                    d.rescan,
                    t("networks-queue-count"),
                    s.page.ftn_queue_counts.get(&d.link).unwrap_or(&0)
                )
            })
            .chain(s.page.subscriptions.iter().map(|a| {
                format!(
                    "{} | {}@{} | {} | {:?} | v{}",
                    a.link,
                    a.area,
                    a.domain,
                    t(if a.subscribed {
                        "networks-subscribed"
                    } else {
                        "networks-unsubscribed"
                    }),
                    a.source,
                    a.version
                )
            }))
            .chain(s.page.areafix.iter().map(|a| {
                format!(
                    "AreaFix | {} | {} | {} | {} {} | {} {}",
                    a.link,
                    time(Some(a.received_at)),
                    a.result,
                    t("networks-commands"),
                    a.commands,
                    t("networks-changes"),
                    a.changes
                )
            }))
            .chain(s.page.rescans.iter().map(|r| {
                format!(
                    "Rescan | {} | {} | {} {} | {} {} | {} {}",
                    r.link,
                    r.area,
                    t("networks-requested-count"),
                    r.requested,
                    t("networks-queued-count"),
                    r.queued,
                    t("networks-accepted-count"),
                    r.accepted
                )
            }))
            .collect(),
        Section::Recovery => s
            .page
            .origins
            .iter()
            .map(|o| {
                format!(
                    "{} | {} {:08x} | {}",
                    o.endpoint,
                    t("networks-next-serial"),
                    o.next_serial,
                    if o.held {
                        t("networks-recovery-required")
                    } else {
                        t("networks-current")
                    }
                )
            })
            .chain([t("networks-recovery-help")])
            .collect(),
    }
}
fn details(model: &MonitorModel) -> Vec<String> {
    let Some(s) = &model.snapshot.networks else {
        return vec![];
    };
    let i = model.networks.selected;
    if let Some(l) = &model.networks.lookup {
        return vec![
            format!("{}: {}", t("networks-local"), l.endpoint),
            format!("{} | {} | {}", l.system, l.location, l.sysop),
            format!("{} / {} / {}", l.source, l.generation, l.date),
            format!(
                "{} | {}",
                l.flags.join(" "),
                l.services
                    .iter()
                    .map(|s| format!(
                        "{} {}:{}",
                        s.protocol,
                        s.host.as_deref().unwrap_or("—"),
                        s.port.unwrap_or(0)
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        ];
    }
    match model.networks.query.section {
        Section::Events => s
            .events
            .get(i)
            .map(|e| {
                vec![
                    e.definition.name.clone(),
                    format!(
                        "{}: {}",
                        t("events-action"),
                        event_action(&e.definition.action)
                    ),
                    format!(
                        "{}: {} / {}",
                        t("events-schedule"),
                        event_schedule(&e.definition.schedule),
                        e.definition.timezone
                    ),
                    format!(
                        "{}: {}",
                        t("events-policy"),
                        event_policy(e.definition.policy)
                    ),
                    format!("{}: {}", t("events-failures"), e.failures),
                    t("events-keys"),
                ]
            })
            .unwrap_or_default(),
        Section::Circuitnet => {
            let mut index = i;
            for n in &s.circuitnet {
                if index == 0 {
                    return vec![
                        format!("{} / {} / {:?}", n.network, n.local, n.role),
                        format!(
                            "{}: {} / {}: {} / {}: {}",
                            t("circuitnet-pending-approval"),
                            n.pending_approvals,
                            t("circuitnet-directed"),
                            n.directed_pending,
                            t("circuitnet-route-failures"),
                            n.directed_failures
                        ),
                        format!(
                            "{}: {:?}",
                            t("circuitnet-control-policy"),
                            n.control_policy.0
                        ),
                        t("circuitnet-visibility-help"),
                    ];
                }
                index -= 1;
                if let Some(p) = n.peers.get(index) {
                    return vec![
                        format!("{} / {} / {:?}", n.network, p.node, p.role),
                        format!("{}:{} / {}", p.host, p.port, p.server_name),
                        format!(
                            "{} / {}: {}",
                            t("circuitnet-tls"),
                            t("circuitnet-protocol-version"),
                            p.health
                                .as_ref()
                                .and_then(|h| h.protocol_minor)
                                .map_or_else(|| "—".into(), |v| format!("1.{v}"))
                        ),
                        format!(
                            "{}: {} / {}: {} / {}: {}",
                            t("networks-queues"),
                            p.queued,
                            t("circuitnet-dossiers"),
                            p.dossiers,
                            t("circuitnet-held"),
                            p.held
                        ),
                        format!(
                            "{}: {}",
                            t("circuitnet-last-contact"),
                            time(p.health.as_ref().and_then(|h| h.last_success))
                        ),
                        format!("{}: {}", t("events-next"), time(p.next_exchange)),
                    ];
                }
                index = index.saturating_sub(n.peers.len());
                if let Some(e) = n.controls.get(index) {
                    return vec![
                        format!("{} / {}", n.network, e.request.id),
                        format!(
                            "{} -> {} / {:?} / {:?}",
                            e.request.requester,
                            e.request.target,
                            e.request.operation,
                            e.request.codename
                        ),
                        format!(
                            "{} / {}: {}",
                            control_outcome(e.result.outcome),
                            t("circuitnet-authentication"),
                            e.authenticated_source
                        ),
                        format!(
                            "{} / {}",
                            time(Some(e.created_at)),
                            time(Some(e.updated_at))
                        ),
                        t("circuitnet-control-help"),
                    ];
                }
                index = index.saturating_sub(n.controls.len());
            }
            vec![]
        }
        Section::Links => {
            if let Some(l) = s.ftn.links.get(i) {
                let tr = s.transport.links.iter().find(|t| t.link == l.id);
                let status = s.binkp.links.iter().find(|h| h.link == l.id);
                let h = status.and_then(|s| s.health.as_ref());
                vec![
                    format!(
                        "{} | {} | AKA {} | RX={} TX={}",
                        l.id, l.remote, l.aka, l.inbound, l.outbound
                    ),
                    format!(
                        "{}: {} | {}",
                        t("networks-endpoint"),
                        tr.and_then(|t| t.endpoint.as_deref())
                            .unwrap_or("local directory"),
                        tr.map(|t| t.port).unwrap_or(0)
                    ),
                    format!(
                        "{}: {}",
                        t("netconfig-credential"),
                        match status.map(|s| s.credential) {
                            Some(sf_bbs::SecretStatus::Configured) =>
                                t("sfconfig-secret-configured"),
                            Some(sf_bbs::SecretStatus::Invalid) => t("sfconfig-secret-invalid"),
                            _ => t("sfconfig-secret-missing"),
                        }
                    ),
                    format!(
                        "{}: {} / {}: {}",
                        t("networks-last-attempt"),
                        time(h.and_then(|h| h.last_attempt)),
                        t("networks-last-success"),
                        time(h.and_then(|h| h.last_success))
                    ),
                    format!(
                        "{}: {} ms | {}: {}",
                        t("networks-latency"),
                        h.and_then(|h| h.latency_ms).unwrap_or(0),
                        t("binkp-queued"),
                        s.page.ftn_queue_counts.get(&l.id).copied().unwrap_or(0)
                    ),
                    guidance(h.and_then(|h| h.last_error.as_deref())),
                    format!(
                        "{}: {}",
                        t("networks-last-remote"),
                        h.map(|h| h
                            .addresses
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" "))
                            .unwrap_or_default()
                    ),
                    format!(
                        "{}: {}",
                        t("binkp-capabilities"),
                        h.map(|h| h.capabilities.join(" ")).unwrap_or_default()
                    ),
                    t("networks-test-help"),
                ]
            } else {
                s.qwk
                    .get(i.saturating_sub(s.ftn.links.len()))
                    .map(|q| {
                        vec![
                            format!(
                                "{} | {} | {} → {}",
                                q.link.name, q.link.network, q.link.local_id, q.link.remote_id
                            ),
                            format!(
                                "{:?} / {:?} | RX={} TX={}",
                                q.link.profile, q.link.role, q.link.inbound, q.link.outbound
                            ),
                            format!("{}: {}", t("networks-last-success"), time(q.last_contact)),
                            format!("{}: {}", t("networks-areas"), q.mappings.len()),
                            t("networks-qwk-help"),
                        ]
                    })
                    .unwrap_or_default()
            }
        }
        Section::Queues => s
            .page
            .queues
            .get(i)
            .map(|q| {
                vec![
                    q.id.clone(),
                    format!(
                        "{}: {} | {}: {}",
                        t("networks-final"),
                        q.final_destination
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| q
                                .qwk_destination
                                .clone()
                                .unwrap_or_else(|| q.network.clone())),
                        t("ftn-next-hop"),
                        q.next_hop
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| q.link.clone())
                    ),
                    format!("{}: {} | {}", t("networks-route"), q.route_reason, q.reason),
                    format!(
                        "{}: {} | {}: {}",
                        t("networks-created"),
                        time(Some(q.created_at)),
                        t("networks-last-attempt"),
                        time(q.last_attempt)
                    ),
                    format!(
                        "{}: {} | {}: {}",
                        t("binkp-retry"),
                        time(q.next_attempt),
                        t("networks-attempts"),
                        q.attempts
                    ),
                    format!(
                        "{}: {} | {} bytes",
                        t("networks-artifact"),
                        q.artifact.as_deref().unwrap_or("—"),
                        q.bytes.unwrap_or(0)
                    ),
                    format!("{}: {}", t("networks-publication"), q.publication),
                    t("networks-queue-help"),
                ]
            })
            .unwrap_or_default(),
        Section::Directory => s
            .page
            .directory
            .get(i)
            .map(|g| {
                vec![
                    g.generation.clone(),
                    format!("{}@{} / {}", g.source, g.domain, g.date),
                    format!(
                        "{}: {} | {}: {}",
                        t("networks-records"),
                        g.records,
                        t("networks-issues"),
                        g.issues
                    ),
                    format!("{}: {}", t("networks-activated"), time(g.activated_at)),
                    t("networks-directory-help"),
                ]
            })
            .unwrap_or_default(),
        Section::Files => s
            .files
            .as_ref()
            .and_then(|f| {
                i.checked_sub(1 + f.areas.len() + f.subscriptions.len())
                    .and_then(|n| f.queue.get(n))
            })
            .map(|q| {
                vec![
                    format!(
                        "{} / {} / {}",
                        q.filename,
                        q.area.as_deref().unwrap_or("FREQ"),
                        q.link
                    ),
                    format!(
                        "{} / {} / {} bytes",
                        q.provenance.as_deref().unwrap_or(&q.kind),
                        q.origin.as_deref().unwrap_or("—"),
                        q.size
                    ),
                    format!("SHA256 {}", q.sha256),
                    format!(
                        "{} {} / TIC {} / {} {}",
                        t("netfiles-payload-accepted"),
                        q.payload_accepted,
                        q.tic_accepted,
                        t("networks-attempts"),
                        q.attempts
                    ),
                    format!(
                        "{} {} / {}",
                        t("netfiles-held"),
                        q.held,
                        q.last_error.as_deref().unwrap_or("—")
                    ),
                    format!(
                        "{}: {}",
                        t("networks-route"),
                        if q.kind == "fileecho" {
                            t("netfiles-subscription")
                        } else {
                            t("netfiles-freq")
                        }
                    ),
                ]
            })
            .unwrap_or_else(|| vec![t("networks-files")]),
        Section::Quarantine => vec![t("networks-quarantine-help")],
        _ => vec![t("networks-authority")],
    }
}
pub fn render(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let regions = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(if model.networks.detail { 9 } else { 2 }),
        Constraint::Length(3),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(t("networks-tabs")).wrap(Wrap { trim: false }),
        regions[0],
    );
    let list = rows(model);
    let mut state = ListState::default().with_selected(Some(
        model.networks.selected.min(list.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(
        List::new(list.into_iter().map(|r| ListItem::new(safe(r))))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(t(model.networks.query.section.key())),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        regions[1],
        &mut state,
    );
    let detail = if model.networks.detail {
        details(model)
            .into_iter()
            .map(safe)
            .collect::<Vec<_>>()
            .join("\n")
    } else if model.networks.query.section == Section::Events {
        t("events-keys")
    } else if model.networks.query.section == Section::Circuitnet {
        t("circuitnet-keys")
    } else {
        t("networks-keys")
    };
    frame.render_widget(
        Paragraph::new(detail).wrap(Wrap { trim: false }),
        regions[2],
    );
    let status = if let Some(action) = &model.networks.pending {
        let circuitnet_label = if let NetworkAction::Circuitnet { request } = action {
            use sf_bbs::circuitnet_live::Action;
            Some(t(match request {
                Action::DecideControl { approve: true, .. } => "circuitnet-approve",
                Action::DecideControl { approve: false, .. } => "circuitnet-deny",
                Action::Test { .. } => "circuitnet-test-link",
                Action::Poll { .. } => "circuitnet-poll",
                Action::Hold { held: true, .. } => "circuitnet-hold",
                Action::Hold { held: false, .. } => "circuitnet-release",
                _ => "circuitnet-networks",
            }))
        } else {
            None
        };
        let (operation, target) = match action {
            NetworkAction::Circuitnet { request } => {
                use sf_bbs::circuitnet_live::Action;
                let target = match request {
                    Action::Test { node, .. }
                    | Action::Poll { node, .. }
                    | Action::Hold { node, .. }
                    | Action::Subscribe { node, .. } => node.as_str(),
                    Action::Retry { queue, .. } => queue.as_str(),
                    Action::DecideControl { id, .. } | Action::RetryControl { id, .. } => {
                        id.as_str()
                    }
                    Action::Direct { destination, .. } => destination.as_str(),
                    Action::ControlPolicy { network, .. }
                    | Action::RequestSubscription { network, .. } => network.as_str(),
                };
                (circuitnet_label.as_deref().unwrap_or(""), target)
            }
            NetworkAction::Binkp { request } => match request {
                sf_bbs::binkp::Action::Test { link, .. } => ("Test Link", link.as_str()),
                sf_bbs::binkp::Action::Poll { link, .. } => ("Poll Link", link.as_str()),
                sf_bbs::binkp::Action::Hold { queue, .. } => ("Hold", queue.as_str()),
                sf_bbs::binkp::Action::Release { queue, .. } => ("Release", queue.as_str()),
                _ => ("", ""),
            },
            NetworkAction::Hold { queue, .. } => ("Hold", queue.as_str()),
            NetworkAction::Retry { queue, .. } => ("Retry", queue.as_str()),
            NetworkAction::Ftn {
                request: sf_bbs::ftn::Action::DirectoryActivate { generation, .. },
            } => ("Activate", generation.as_str()),
            NetworkAction::Ftn {
                request: sf_bbs::ftn::Action::Scan,
            } => ("Scan", "FTN"),
            _ => ("", ""),
        };
        format!(
            "{}: {} — {}",
            operation,
            safe(target),
            t("networks-confirm")
        )
    } else if let Some(input) = &model.networks.input {
        format!("{}: {}", t("networks-lookup-prompt"), safe(input))
    } else {
        model.networks.status.clone()
    };
    frame.render_widget(
        Paragraph::new(status).wrap(Wrap { trim: false }),
        regions[3],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use sf_core::LocalOperatorCapability as Cap;
    #[test]
    fn network_confirmation_cannot_dispatch_after_daemon_loss() {
        let (worker, commands) = MonitorWorker::test_channels();
        let mut m = MonitorModel {
            connection: ConnectionState::Connected {
                daemon_generation: "synthetic".into(),
                features: vec![sf_bbs::OperatorFeature::Networks],
            },
            ..Default::default()
        };
        m.snapshot.authorized_capabilities = vec![Cap::NetworkRun];
        m.networks.pending = Some(NetworkAction::Ftn {
            request: sf_bbs::ftn::Action::Scan,
        });
        m.mark_disconnected("sfmonitor-disconnected");
        assert!(m.networks.pending.is_none());
        key(
            &mut m,
            &worker,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(commands.try_recv().is_err());
        key(
            &mut m,
            &worker,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        );
        assert!(matches!(
            commands.try_recv(),
            Ok(WorkerCommand::Reconnect(_))
        ));
    }
    #[test]
    fn section_navigation_requests_bounded_projection_and_no_mutation() {
        let (worker, commands) = MonitorWorker::test_channels();
        let mut m = MonitorModel::default();
        key(
            &mut m,
            &worker,
            KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE),
        );
        assert!(matches!(
            commands.try_recv().unwrap(),
            WorkerCommand::Networks(NetworkQuery {
                section: Section::Directory,
                offset: 0
            })
        ));
        assert!(commands.try_recv().is_err());
        m.networks.query = NetworkQuery {
            section: Section::Circuitnet,
            offset: 32,
        };
        key(
            &mut m,
            &worker,
            KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
        );
        assert!(matches!(
            commands.try_recv().unwrap(),
            WorkerCommand::Networks(NetworkQuery {
                section: Section::Circuitnet,
                offset: 16
            })
        ));
    }
    #[test]
    fn cockpit_sizes_and_hostile_text_are_safe() {
        assert_eq!(safe("safe\x1b\u{202e}text"), "safe[text".replace('[', ""));
        assert_eq!(safe("x".repeat(5000)).len(), 512);
        let m = MonitorModel::default();
        for (w, h) in [(60, 20), (80, 24), (100, 30)] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| render(f, f.area(), &m)).unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("Overview"));
            assert!(!text.contains("networks-access"));
        }
    }
}
