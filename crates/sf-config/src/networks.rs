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

//! Named network forms; JSON is an in-memory typed draft, never a raw editor.
use super::*;
use serde_json::{json, Value};
use sf_bbs::{NetworkAction, NetworkResult};
use sf_core::ftn::{Mapping, NetworkQuery, NetworkSection};

#[derive(Clone)]
struct Data {
    secrets: Vec<(String, sf_bbs::SecretStatus)>,
    snapshot: ConfigurationSnapshot,
    qwk: Vec<sf_core::qwk_network::LinkStatus>,
    areas: Vec<Mapping>,
    conferences: Vec<sf_core::ftn::NetworkConference>,
}
impl Backend {
    fn network_data(&mut self) -> Result<Data, String> {
        let snapshot = self.snapshot()?;
        let mut areas = vec![];
        let mut conferences = vec![];
        let mut offset = 0;
        let mut qwk = vec![];
        let mut secrets = vec![];
        loop {
            let query = NetworkQuery {
                section: NetworkSection::Areas,
                offset,
            };
            let page = match self {
                Self::Online { runtime, client } => {
                    let s = runtime
                        .block_on(client.networks(query))
                        .map_err(|_| t("networks-access"))?;
                    secrets = s
                        .binkp
                        .links
                        .into_iter()
                        .map(|l| (l.link, l.credential))
                        .collect();
                    qwk = s.qwk;
                    s.page
                }
                Self::Offline(a) => {
                    qwk = a.qwk_partners().map_err(|_| t("networks-access"))?;
                    a.network_page(&query).map_err(|_| t("networks-access"))?
                }
            };
            areas.extend(page.areas);
            conferences.extend(
                page.conferences
                    .into_iter()
                    .filter(|c| {
                        !conferences
                            .iter()
                            .any(|old: &sf_core::ftn::NetworkConference| old.id == c.id)
                    })
                    .collect::<Vec<_>>(),
            );
            if !page.more {
                break;
            }
            offset += 100;
        }
        Ok(Data {
            secrets,
            snapshot,
            qwk,
            areas,
            conferences,
        })
    }
    fn network_action(&mut self, id: String, action: NetworkAction) -> Result<(), String> {
        match self {
            Self::Online { runtime, client } => {
                match runtime.block_on(client.qwk_network_action(id, action)) {
                    Ok(
                        NetworkResult::Configured
                        | NetworkResult::Updated
                        | NetworkResult::Ftn {
                            response: sf_bbs::ftn::Result::Updated,
                        }
                        | NetworkResult::Binkp {
                            response: sf_bbs::binkp::Result::Updated,
                        },
                    ) => Ok(()),
                    Ok(NetworkResult::Replayed { state }) if state == "completed" => Ok(()),
                    Ok(NetworkResult::Rejected { reason }) => Err(
                        if reason.contains("changed") || reason.contains("conflict") {
                            t("netconfig-conflict")
                        } else {
                            t("networks-action-rejected")
                        },
                    ),
                    Ok(_) => Err(t("networks-action-rejected")),
                    Err(_) => Err(t("sfconfig-save-uncertain")),
                }
            }
            Self::Offline(a) => a
                .configure_network(&action)
                .map_err(|_| t("netconfig-invalid-or-conflict")),
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Ftn,
    Binkp,
    Qwk,
    Area,
    Downstream,
    Subscription,
    Access,
    Rescan,
}
struct Form {
    kind: Kind,
    before: Value,
    draft: Value,
    path: Vec<String>,
    selected: usize,
    input: Option<String>,
    review: bool,
    confirm_discard: bool,
    status: String,
    command: String,
}
fn get<'a>(v: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut v = v;
    for k in path {
        v = if let Some(a) = v.as_array() {
            a.get(k.parse::<usize>().ok()?)?
        } else {
            v.get(k)?
        };
    }
    Some(v)
}
fn get_mut<'a>(v: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    let mut v = v;
    for k in path {
        v = match v {
            Value::Array(a) => a.get_mut(k.parse::<usize>().ok()?)?,
            Value::Object(o) => o.get_mut(k)?,
            _ => return None,
        };
    }
    Some(v)
}
fn label(key: &str) -> String {
    t(&format!("netconfig-field-{}", key.replace('_', "-")))
}
fn clean(s: impl ToString) -> String {
    s.to_string()
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
        .take(512)
        .collect()
}
fn value(v: &Value) -> String {
    match v {
        Value::Bool(b) => t(if *b {
            "sfconfig-enabled"
        } else {
            "sfconfig-disabled"
        }),
        Value::Null => t("netconfig-unset"),
        Value::String(s) => clean(s),
        Value::Object(o) => o
            .get("id")
            .or_else(|| o.get("link"))
            .or_else(|| o.get("area"))
            .or_else(|| o.get("domain"))
            .map(value)
            .unwrap_or_else(|| t("netconfig-details")),
        Value::Array(a) => format!("{} {}", a.len(), t("netconfig-entries")),
        _ => v.to_string(),
    }
}
impl Form {
    fn new(kind: Kind, v: Value) -> Self {
        Self {
            kind,
            before: if (kind == Kind::Qwk && v["link"]["version"].as_i64() == Some(0))
                || (matches!(
                    kind,
                    Kind::Area | Kind::Downstream | Kind::Subscription | Kind::Access
                ) && v["version"].as_i64() == Some(0))
            {
                Value::Null
            } else {
                v.clone()
            },
            draft: v,
            path: vec![],
            selected: 0,
            input: None,
            review: false,
            confirm_discard: false,
            status: String::new(),
            command: command_id(),
        }
    }
    fn keys(&self) -> Vec<String> {
        match get(&self.draft, &self.path) {
            Some(Value::Object(o)) => o
                .keys()
                .filter(|k| {
                    *k != "version"
                        && !(self.kind == Kind::Subscription
                            && ["source", "changed_at"].contains(&k.as_str()))
                })
                .cloned()
                .collect(),
            Some(Value::Array(a)) => (0..a.len()).map(|i| i.to_string()).collect(),
            _ => vec![],
        }
    }
    fn target(&self) -> Vec<String> {
        let mut p = self.path.clone();
        if let Some(k) = self.keys().get(self.selected) {
            p.push(k.clone())
        }
        p
    }
    fn template(&self, data: &Data) -> Option<Value> {
        let aka = data
            .snapshot
            .config
            .ftn
            .akas
            .first()
            .map(|a| a.id.as_str())
            .unwrap_or("local");
        let domain = data
            .snapshot
            .config
            .ftn
            .akas
            .first()
            .map(|a| a.endpoint.domain.as_str())
            .unwrap_or("isolated");
        let link = data
            .snapshot
            .config
            .ftn
            .links
            .first()
            .map(|l| l.id.as_str())
            .unwrap_or("peer");
        let conference = data.conferences.first().map(|c| c.id).unwrap_or(1);
        Some(match self.path.last()?.as_str() {
            "akas" if self.kind == Kind::Ftn => {
                json!({"id":"local","endpoint":{"domain":domain,"address":"10:100/1"},"enabled":false,"primary":false})
            }
            "akas" => json!(aka),
            "remote_akas" => json!({"domain":domain,"address":"10:100/2.1"}),
            "links" if self.kind == Kind::Ftn => {
                json!({"id":"peer","remote":{"domain":domain,"address":"10:100/2"},"aka":aka,"enabled":false,"inbound":false,"outbound":false,"transit":false,"profile":"type2-plus","charset":"cp437"})
            }
            "links" if self.kind == Kind::Binkp => {
                json!({"link":link,"enabled":false,"inbound":false,"outbound":false,"endpoint":null,"port":24554,"directory":true,"akas":[aka],"remote_akas":[],"auth":"require-cram","allow_domainless":false})
            }
            "links" => json!(link),
            "routes" => json!({"domain":domain,"target":{"kind":"default"},"link":link}),
            "sources" => {
                json!({"id":"nodes","domain":domain,"enabled":false,"format":"nodelist","charset":"ascii","default_zone":10,"priority":10,"cadence_days":7,"require_crc":true})
            }
            "mappings" => {
                json!({"wire_conference":1,"area":"general","conference_id":conference,"enabled":true,"inbound":true,"outbound":true,"version":1})
            }
            _ => return None,
        })
    }
    fn save(&mut self, backend: &mut Backend, data: &Data) -> Result<(), String> {
        match self.kind {
            Kind::Ftn | Kind::Binkp => {
                let mut candidate = ConfigurationCandidate {
                    expected: data.snapshot.version.clone(),
                    edits: vec![],
                    operators: None,
                    ftn: None,
                    binkp: None,
                };
                if self.kind == Kind::Ftn {
                    candidate.ftn = Some(
                        serde_json::from_value(self.draft.clone())
                            .map_err(|_| t("netconfig-invalid"))?,
                    )
                } else {
                    candidate.binkp = Some(
                        serde_json::from_value(self.draft.clone())
                            .map_err(|_| t("netconfig-invalid"))?,
                    )
                }
                candidate
                    .validate(&data.snapshot.config)
                    .map_err(|e| e.iter().map(issue_text).collect::<Vec<_>>().join("; "))?;
                match backend.save(self.command.clone(), candidate)? {
                    ConfigurationResult::Saved { .. } => Ok(()),
                    ConfigurationResult::Conflict { .. } => Err(t("netconfig-conflict")),
                    ConfigurationResult::Replayed {
                        result_class: Some(c),
                        ..
                    } if c == "configuration-saved" || c == "configuration-restart-required" => {
                        Ok(())
                    }
                    ConfigurationResult::Invalid { issues } => {
                        Err(issues.iter().map(issue_text).collect::<Vec<_>>().join("; "))
                    }
                    _ => Err(t("netconfig-invalid-or-conflict")),
                }
            }
            Kind::Qwk => {
                let mut link: sf_core::qwk_network::Link =
                    serde_json::from_value(self.draft["link"].clone())
                        .map_err(|_| t("netconfig-invalid"))?;
                let expected = self.before["link"]["version"].as_i64().unwrap_or(0);
                link.version = expected + 1;
                let mut mappings: Vec<sf_core::qwk_network::Mapping> =
                    serde_json::from_value(self.draft["mappings"].clone())
                        .map_err(|_| t("netconfig-invalid"))?;
                for m in &mut mappings {
                    m.version = expected + 1;
                }
                backend.network_action(
                    self.command.clone(),
                    NetworkAction::Configure {
                        link,
                        mappings,
                        expected,
                    },
                )
            }
            Kind::Downstream | Kind::Subscription | Kind::Access | Kind::Rescan => {
                let expected = self.before["version"].as_i64().unwrap_or(0);
                let mut draft = self.draft.clone();
                if self.kind != Kind::Rescan {
                    draft["version"] = json!(expected + 1);
                }
                let request = match self.kind {
                    Kind::Downstream => sf_bbs::ftn::Action::Downstream {
                        downstream: serde_json::from_value(draft)
                            .map_err(|_| t("netconfig-invalid"))?,
                        expected,
                    },
                    Kind::Subscription => sf_bbs::ftn::Action::Subscription {
                        subscription: serde_json::from_value(draft)
                            .map_err(|_| t("netconfig-invalid"))?,
                        expected,
                    },
                    Kind::Access => sf_bbs::ftn::Action::AreaAccess {
                        access: serde_json::from_value(draft)
                            .map_err(|_| t("netconfig-invalid"))?,
                        expected,
                    },
                    _ => sf_bbs::ftn::Action::Rescan {
                        link: draft["link"]
                            .as_str()
                            .ok_or_else(|| t("netconfig-invalid"))?
                            .into(),
                        areas: vec![serde_json::from_value(draft["request"].clone())
                            .map_err(|_| t("netconfig-invalid"))?],
                    },
                };
                backend.network_action(self.command.clone(), NetworkAction::Ftn { request })
            }
            Kind::Area => {
                let mut mapping: Mapping = serde_json::from_value(self.draft.clone())
                    .map_err(|_| t("netconfig-invalid"))?;
                let expected = self.before["version"].as_i64().unwrap_or(0);
                mapping.version = expected + 1;
                backend.network_action(
                    self.command.clone(),
                    NetworkAction::Ftn {
                        request: sf_bbs::ftn::Action::Mapping { mapping, expected },
                    },
                )
            }
        }
    }
    fn edit(&mut self, input: String) {
        let p = self.target();
        let old = get(&self.draft, &p).cloned().unwrap_or(Value::Null);
        let new = match old {
            Value::Number(_) => input.parse::<i64>().map(|v| json!(v)).ok(),
            Value::Bool(_) => input.parse::<bool>().ok().map(Value::Bool),
            _ => Some(
                if input.is_empty() && p.last().is_some_and(|s| s == "endpoint" || s == "boss_aka")
                {
                    Value::Null
                } else {
                    Value::String(input.clone())
                },
            ),
        };
        if let Some(new) = new {
            if p.last().is_some_and(|s| s == "kind") {
                let target = match input.as_str() {
                    "default" => json!({"kind":"default"}),
                    "zone" => json!({"kind":"zone","zone":10}),
                    "net" => json!({"kind":"net","zone":10,"net":100}),
                    "boss" | "exact" => json!({"kind":input,"address":"10:100/2"}),
                    _ => {
                        self.status = t("netconfig-route-kinds");
                        return;
                    }
                };
                if let Some(v) = get_mut(&mut self.draft, &self.path) {
                    *v = target;
                    self.selected = 0;
                }
            } else if let Some(v) = get_mut(&mut self.draft, &p) {
                *v = new;
            }
            self.command = command_id();
            self.status.clear();
        } else {
            self.status = t("netconfig-invalid");
            self.input = Some(input)
        }
    }
}
fn diff(before: &Value, after: &Value, path: &str, out: &mut Vec<String>) {
    if before == after {
        return;
    }
    match (before, after) {
        (Value::Object(_), _) | (_, Value::Object(_)) => {
            let keys: std::collections::BTreeSet<_> = before
                .as_object()
                .into_iter()
                .flat_map(|o| o.keys())
                .chain(after.as_object().into_iter().flat_map(|o| o.keys()))
                .collect();
            for k in keys {
                if k != "version" {
                    diff(
                        before.get(k).unwrap_or(&Value::Null),
                        after.get(k).unwrap_or(&Value::Null),
                        &format!("{path} / {}", label(k)),
                        out,
                    );
                }
            }
        }
        (Value::Array(_), _) | (_, Value::Array(_)) => {
            let empty = Vec::new();
            let a = before.as_array().unwrap_or(&empty);
            let b = after.as_array().unwrap_or(&empty);
            for i in 0..a.len().max(b.len()) {
                diff(
                    a.get(i).unwrap_or(&Value::Null),
                    b.get(i).unwrap_or(&Value::Null),
                    &format!("{path} / {}", i + 1),
                    out,
                )
            }
        }
        _ => out.push(format!("{path}: {} → {}", value(before), value(after))),
    }
}
fn draw_form(frame: &mut Frame<'_>, f: &Form, data: &Data) {
    let p = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(4),
        Constraint::Length(5),
        Constraint::Length(3),
    ])
    .split(frame.area());
    frame.render_widget(
        Paragraph::new(format!(
            "{} | {} {}\n{}",
            t("netconfig-title"),
            t("netconfig-revision"),
            data.snapshot.version.revision,
            f.path
                .iter()
                .map(|k| k
                    .parse::<usize>()
                    .map(|n| (n + 1).to_string())
                    .unwrap_or_else(|_| label(k)))
                .collect::<Vec<_>>()
                .join(" / ")
        )),
        p[0],
    );
    let lines = if f.review {
        let mut v = vec![t(if f.kind == Kind::Ftn || f.kind == Kind::Binkp {
            "netconfig-static-effect"
        } else {
            "netconfig-relational-effect"
        })];
        diff(&f.before, &f.draft, "", &mut v);
        v
    } else {
        f.keys()
            .iter()
            .map(|k| {
                let mut path = f.path.clone();
                path.push(k.clone());
                format!(
                    "{}: {}",
                    k.parse::<usize>()
                        .map(|n| (n + 1).to_string())
                        .unwrap_or_else(|_| label(k)),
                    value(get(&f.draft, &path).unwrap_or(&Value::Null))
                )
            })
            .collect()
    };
    let mut state =
        ListState::default().with_selected(Some(f.selected.min(lines.len().saturating_sub(1))));
    frame.render_stateful_widget(
        List::new(lines.into_iter().map(|s| ListItem::new(clean(s))))
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        p[1],
        &mut state,
    );
    let hint = if f.confirm_discard {
        t("netconfig-discard")
    } else if f.review {
        t("netconfig-save-confirm")
    } else if let Some(input) = &f.input {
        format!("{}: {}", t("sfconfig-edit"), clean(input))
    } else {
        t("netconfig-form-help")
    };
    frame.render_widget(Paragraph::new(hint).wrap(Wrap { trim: false }), p[2]);
    let status = if f.target().last().is_some_and(|k| k == "conference_id") {
        let id = f
            .input
            .as_ref()
            .and_then(|s| s.parse::<i64>().ok())
            .or_else(|| get(&f.draft, &f.target()).and_then(Value::as_i64));
        let chosen = data.conferences.iter().find(|c| Some(c.id) == id);
        format!(
            "{} {}",
            t("netconfig-conference-picker"),
            chosen
                .map(|c| format!(
                    "{} ({}) / ID {} / {}",
                    c.name,
                    c.number,
                    c.id,
                    if c.active {
                        t("sfconfig-enabled")
                    } else {
                        t("sfconfig-disabled")
                    }
                ))
                .unwrap_or_else(|| t("netconfig-unset"))
        )
    } else {
        f.status.clone()
    };
    frame.render_widget(
        Paragraph::new(clean(status)).wrap(Wrap { trim: false }),
        p[3],
    );
}
fn edit_form(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    backend: &mut Backend,
    data: &Data,
    mut f: Form,
) -> Result<(), String> {
    let mut checked = std::time::Instant::now();
    loop {
        if checked.elapsed() >= Duration::from_secs(10) {
            if let Err(e) = backend.snapshot() {
                f.status = e;
            }
            checked = std::time::Instant::now();
        }
        terminal
            .draw(|frame| draw_form(frame, &f, data))
            .map_err(|_| t("sfconfig-connection-error"))?;
        if !event::poll(Duration::from_millis(100)).map_err(|_| t("sfconfig-connection-error"))? {
            continue;
        }
        let Event::Key(k) = event::read().map_err(|_| t("sfconfig-connection-error"))? else {
            continue;
        };
        if !matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            if f.before == f.draft {
                return Ok(());
            }
            f.confirm_discard = true;
            f.input = None;
            continue;
        }
        if f.confirm_discard {
            match k.code {
                KeyCode::Enter => return Ok(()),
                KeyCode::Esc => f.confirm_discard = false,
                _ => {}
            }
            continue;
        }
        let conference_field = f.target().last().is_some_and(|k| k == "conference_id");
        if let Some(input) = &mut f.input {
            match k.code {
                KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
                    if conference_field && !data.conferences.is_empty() =>
                {
                    let id = input.parse::<i64>().ok();
                    let current = data
                        .conferences
                        .iter()
                        .position(|c| Some(c.id) == id)
                        .unwrap_or(0);
                    let next = match k.code {
                        KeyCode::Up => current.saturating_sub(1),
                        KeyCode::PageUp => current.saturating_sub(10),
                        KeyCode::PageDown => current.saturating_add(10),
                        _ => current.saturating_add(1),
                    }
                    .min(data.conferences.len() - 1);
                    *input = data.conferences[next].id.to_string();
                }
                KeyCode::Esc => f.input = None,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) if !c.is_control() && input.len() < 256 => input.push(c),
                KeyCode::Enter => {
                    let input = f.input.take().unwrap();
                    f.edit(input)
                }
                _ => {}
            }
            continue;
        }
        if f.review {
            match k.code {
                KeyCode::Enter => match f.save(backend, data) {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        f.status = e;
                        f.review = false
                    }
                },
                KeyCode::Esc => f.review = false,
                KeyCode::Down | KeyCode::PageDown => f.selected = f.selected.saturating_add(1),
                KeyCode::Up | KeyCode::PageUp => f.selected = f.selected.saturating_sub(1),
                _ => {}
            }
            continue;
        }
        match k.code {
            KeyCode::Esc if !f.path.is_empty() => {
                f.path.pop();
                f.selected = 0;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                if f.draft != f.before {
                    f.confirm_discard = true
                } else {
                    return Ok(());
                }
            }
            KeyCode::Up => f.selected = f.selected.saturating_sub(1),
            KeyCode::Down => {
                f.selected = f
                    .selected
                    .saturating_add(1)
                    .min(f.keys().len().saturating_sub(1))
            }
            KeyCode::PageUp => f.selected = f.selected.saturating_sub(10),
            KeyCode::PageDown => {
                f.selected = f
                    .selected
                    .saturating_add(10)
                    .min(f.keys().len().saturating_sub(1))
            }
            KeyCode::Char('s') => {
                f.review = true;
                f.selected = 0;
            }
            KeyCode::Char('a') => {
                if let Some(v) = f.template(data) {
                    if let Some(Value::Array(a)) = get_mut(&mut f.draft, &f.path) {
                        if a.len() < 256 {
                            a.push(v);
                            f.selected = a.len() - 1;
                            f.command = command_id();
                        }
                    }
                }
            }
            KeyCode::Char('d') => {
                let i = f.selected;
                if let Some(Value::Array(a)) = get_mut(&mut f.draft, &f.path) {
                    if i < a.len() {
                        a.remove(i);
                        f.selected = i.saturating_sub(1);
                        f.command = command_id();
                    }
                }
            }
            KeyCode::Enter => {
                let p = f.target();
                let v = get(&f.draft, &p).cloned().unwrap_or(Value::Null);
                match v {
                    Value::Object(_) | Value::Array(_) => {
                        f.path = p;
                        f.selected = 0;
                    }
                    Value::Bool(b) => {
                        if let Some(v) = get_mut(&mut f.draft, &p) {
                            *v = Value::Bool(!b);
                            f.command = command_id();
                        }
                    }
                    Value::Null if p.last().is_some_and(|k| k == "listener") => {
                        if let Some(v) = get_mut(&mut f.draft, &p) {
                            *v = json!({"enabled":false,"bind":"127.0.0.1:24554","akas":[]});
                            f.command = command_id();
                        }
                    }
                    _ => {
                        f.input = Some(if v.is_null() {
                            String::new()
                        } else if let Some(s) = v.as_str() {
                            s.into()
                        } else {
                            v.to_string()
                        })
                    }
                }
            }
            _ => {}
        }
    }
}
fn prompt(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    title: &str,
    secret: bool,
) -> Result<Option<String>, String> {
    let mut value = String::new();
    loop {
        terminal
            .draw(|f| {
                f.render_widget(
                    Paragraph::new(format!(
                        "{}\n\n{}\n\n{}",
                        t(title),
                        if secret {
                            "•".repeat(value.len())
                        } else {
                            clean(&value)
                        },
                        t("netconfig-input-help")
                    ))
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::ALL)),
                    f.area(),
                )
            })
            .map_err(|_| t("sfconfig-connection-error"))?;
        let Event::Key(k) = event::read().map_err(|_| t("sfconfig-connection-error"))? else {
            continue;
        };
        if !matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            return Ok(None);
        }
        match k.code {
            KeyCode::Esc => return Ok(None),
            KeyCode::Enter if !value.is_empty() => return Ok(Some(value)),
            KeyCode::Backspace => {
                value.pop();
            }
            KeyCode::Char(c)
                if !c.is_control() && value.len() < if secret { 128 } else { 1024 } =>
            {
                value.push(c)
            }
            _ => {}
        }
    }
}
pub(super) fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    backend: &mut Backend,
) -> Result<(), String> {
    let mut data = backend.network_data()?;
    let mut selected = 0usize;
    let mut status = String::new();
    let mut checked = std::time::Instant::now();
    loop {
        if checked.elapsed() >= Duration::from_secs(10) {
            match backend.network_data() {
                Ok(d) => data = d,
                Err(e) => status = e,
            }
            checked = std::time::Instant::now();
        }
        let mut entries = vec![
            t("netconfig-ftn"),
            t("netconfig-binkp"),
            t("netconfig-qwk-add"),
            t("netconfig-area-add"),
            t("netconfig-recover"),
        ];
        entries.extend(data.qwk.iter().map(|q| format!("QWK / {}", q.link.name)));
        entries.extend(
            data.areas
                .iter()
                .map(|a| format!("EchoMail / {}@{}", a.area, a.domain)),
        );
        entries.extend(data.snapshot.config.binkp.links.iter().map(|l| {
            format!(
                "{} / {}: {}",
                t("netconfig-credential"),
                l.link,
                data.secrets
                    .iter()
                    .find(|(id, _)| id == &l.link)
                    .map(|(_, s)| t(match s {
                        sf_bbs::SecretStatus::Configured => "sfconfig-secret-configured",
                        sf_bbs::SecretStatus::Invalid => "sfconfig-secret-invalid",
                        sf_bbs::SecretStatus::Missing => "sfconfig-secret-missing",
                    }))
                    .unwrap_or_else(|| t("netconfig-secret-online"))
            )
        }));
        entries.push(t("netconfig-hub"));
        terminal
            .draw(|f| {
                let p = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(4),
                ])
                .split(f.area());
                f.render_widget(
                    Paragraph::new(format!(
                        "{} | {}\n{}",
                        t("netconfig-title"),
                        if matches!(backend, Backend::Offline(_)) {
                            "OFFLINE"
                        } else {
                            "ONLINE"
                        },
                        t("netconfig-menu-help")
                    ))
                    .wrap(Wrap { trim: false }),
                    p[0],
                );
                let mut state = ListState::default().with_selected(Some(selected));
                f.render_stateful_widget(
                    List::new(entries.iter().map(|s| ListItem::new(clean(s))))
                        .block(Block::default().borders(Borders::ALL))
                        .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
                    p[1],
                    &mut state,
                );
                f.render_widget(
                    Paragraph::new(clean(&status)).wrap(Wrap { trim: false }),
                    p[2],
                );
            })
            .map_err(|_| t("sfconfig-connection-error"))?;
        if !event::poll(Duration::from_millis(100)).map_err(|_| t("sfconfig-connection-error"))? {
            continue;
        }
        let Event::Key(k) = event::read().map_err(|_| t("sfconfig-connection-error"))? else {
            continue;
        };
        if !matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            return Ok(());
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(entries.len().saturating_sub(1)),
            KeyCode::Char('c') if selected >= 5 + data.qwk.len() + data.areas.len() => {
                let idx = selected - 5 - data.qwk.len() - data.areas.len();
                if let Some(l) = data.snapshot.config.binkp.links.get(idx) {
                    if prompt(terminal, "netconfig-clear-prompt", false)?.as_deref()
                        == Some("CLEAR")
                    {
                        if let Backend::Online { runtime, client } = backend {
                            let current = runtime
                                .block_on(client.binkp_status())
                                .map_err(|_| t("networks-access"))?;
                            let action = NetworkAction::Binkp {
                                request: sf_bbs::binkp::Action::ClearCredential {
                                    link: l.link.clone(),
                                    expected: current.policy,
                                },
                            };
                            status = match backend.network_action(command_id(), action) {
                                Ok(()) => t("binkp-credential-updated"),
                                Err(e) => e,
                            };
                            if let Ok(d) = backend.network_data() {
                                data = d;
                            }
                        } else {
                            status = t("netconfig-secret-online")
                        }
                    }
                }
            }
            KeyCode::Char('r') => match backend.network_data() {
                Ok(d) => {
                    data = d;
                    selected = 0;
                    status.clear()
                }
                Err(e) => status = e,
            },
            KeyCode::Enter if selected == entries.len() - 1 => {
                hub_menu(terminal, backend, &data)?;
                data = backend.network_data()?;
            }
            KeyCode::Enter => {
                let conf = data.conferences.first().map(|c| c.id).unwrap_or(1);
                let aka = data
                    .snapshot
                    .config
                    .ftn
                    .akas
                    .first()
                    .map(|a| a.id.as_str())
                    .unwrap_or("local");
                let domain = data
                    .snapshot
                    .config
                    .ftn
                    .akas
                    .first()
                    .map(|a| a.endpoint.domain.as_str())
                    .unwrap_or("isolated");
                let form = match selected {
                    0 => Some(Form::new(
                        Kind::Ftn,
                        serde_json::to_value(&data.snapshot.config.ftn)
                            .map_err(|_| t("netconfig-invalid"))?,
                    )),
                    1 => Some(Form::new(
                        Kind::Binkp,
                        serde_json::to_value(&data.snapshot.config.binkp)
                            .map_err(|_| t("netconfig-invalid"))?,
                    )),
                    2 => Some(Form::new(
                        Kind::Qwk,
                        json!({"link":{"id":"partner","network":"isolated","local_id":"MYBBS","remote_id":"PEER","name":"QWK partner","profile":"qwk-headers","role":"hub","enabled":false,"inbound":true,"outbound":true,"version":0},"mappings":[{"wire_conference":1,"area":"general","conference_id":conf,"enabled":true,"inbound":true,"outbound":true,"version":1}]}),
                    )),
                    3 => Some(Form::new(
                        Kind::Area,
                        json!({"domain":domain,"area":"GENERAL","conference_id":conf,"aka":aka,"receive":false,"send":false,"origin":"SPITFIRE NG","links":[],"version":0}),
                    )),
                    4 => {
                        if let Backend::Offline(authority) = backend {
                            if let Some(path) = prompt(terminal, "netconfig-recover-prompt", false)?
                            {
                                if prompt(terminal, "netconfig-transfer-prompt", false)?.as_deref()
                                    != Some("TRANSFER")
                                {
                                    continue;
                                }
                                status = match authority.recover_networks_from(&PathBuf::from(path))
                                {
                                    Ok(r) => format!(
                                        "{}: {} / {}",
                                        t("netconfig-recovered"),
                                        r.origins,
                                        r.accepted
                                    ),
                                    Err(_) => t("netconfig-recovery-failed"),
                                };
                            }
                        } else {
                            status = t("netconfig-recovery-offline")
                        }
                        None
                    }
                    i if i < 5 + data.qwk.len() => {
                        let q = &data.qwk[i - 5];
                        Some(Form::new(
                            Kind::Qwk,
                            json!({"link":q.link,"mappings":q.mappings}),
                        ))
                    }
                    i if i < 5 + data.qwk.len() + data.areas.len() => Some(Form::new(
                        Kind::Area,
                        serde_json::to_value(&data.areas[i - 5 - data.qwk.len()])
                            .map_err(|_| t("netconfig-invalid"))?,
                    )),
                    i => {
                        let l = &data.snapshot.config.binkp.links
                            [i - 5 - data.qwk.len() - data.areas.len()];
                        if let Backend::Online { runtime, client } = backend {
                            let current = runtime
                                .block_on(client.binkp_status())
                                .map_err(|_| t("networks-access"))?;
                            let state = current
                                .links
                                .iter()
                                .find(|s| s.link == l.link)
                                .map(|s| s.credential);
                            let title = format!(
                                "{}: {}",
                                t("netconfig-credential"),
                                match state {
                                    Some(sf_bbs::SecretStatus::Configured) =>
                                        t("sfconfig-secret-configured"),
                                    Some(sf_bbs::SecretStatus::Invalid) =>
                                        t("sfconfig-secret-invalid"),
                                    _ => t("sfconfig-secret-missing"),
                                }
                            );
                            status = title;
                            if let Some(secret) = prompt(terminal, "netconfig-secret-prompt", true)?
                            {
                                let request = sf_bbs::binkp::Action::Credential {
                                    link: l.link.clone(),
                                    expected: current.policy,
                                    secret,
                                };
                                status = match backend
                                    .network_action(command_id(), NetworkAction::Binkp { request })
                                {
                                    Ok(()) => t("binkp-credential-updated"),
                                    Err(e) => e,
                                };
                                if let Ok(d) = backend.network_data() {
                                    data = d;
                                }
                            }
                        } else {
                            status = t("netconfig-secret-online")
                        }
                        None
                    }
                };
                if let Some(f) = form {
                    edit_form(terminal, backend, &data, f)?;
                    match backend.network_data() {
                        Ok(d) => data = d,
                        Err(e) => status = e,
                    };
                    selected = selected.min(entries.len().saturating_sub(1));
                }
            }
            _ => {}
        }
    }
}

fn hub_page(backend: &mut Backend) -> Result<sf_core::ftn::NetworkPage, String> {
    let mut page = sf_core::ftn::NetworkPage::default();
    let mut offset = 0;
    loop {
        let query = NetworkQuery {
            section: NetworkSection::Hub,
            offset,
        };
        let part = match backend {
            Backend::Online { runtime, client } => {
                runtime
                    .block_on(client.networks(query))
                    .map_err(|_| t("networks-access"))?
                    .page
            }
            Backend::Offline(a) => a.network_page(&query).map_err(|_| t("networks-access"))?,
        };
        page.downstreams = part.downstreams;
        page.subscriptions.extend(part.subscriptions);
        page.area_access.extend(part.area_access);
        if !part.more {
            break;
        }
        offset += 100;
        if offset > 10000 {
            return Err(t("networks-action-rejected"));
        }
    }
    Ok(page)
}
fn hub_menu(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    backend: &mut Backend,
    data: &Data,
) -> Result<(), String> {
    let mut selected = 0usize;
    let mut status = String::new();
    let mut page = hub_page(backend)?;
    loop {
        let first = data.snapshot.config.ftn.links.first();
        let domain = first
            .map(|l| l.remote.domain.as_str())
            .unwrap_or("isolated");
        let link = first.map(|l| l.id.as_str()).unwrap_or("downstream");
        let area = data
            .areas
            .first()
            .map(|a| a.area.as_str())
            .unwrap_or("GENERAL");
        let boss = data
            .snapshot
            .config
            .ftn
            .akas
            .iter()
            .find(|a| a.endpoint.address.point() == 0)
            .map(|a| a.id.as_str())
            .unwrap_or("local");
        let mut forms = vec![
            (
                t("netconfig-downstream-add"),
                Kind::Downstream,
                json!({"link":link,"enabled":false,"held":false,"boss_aka":null,"areafix":false,"rescan":false,"max_area":50,"max_total":100,"cooldown":300,"version":0}),
            ),
            (
                t("netconfig-point-add"),
                Kind::Downstream,
                json!({"link":data.snapshot.config.ftn.links.iter().find(|l|l.remote.address.point()>0).map(|l|l.id.as_str()).unwrap_or("point1"),"enabled":false,"held":false,"boss_aka":boss,"areafix":false,"rescan":false,"max_area":50,"max_total":100,"cooldown":300,"version":0}),
            ),
            (
                t("netconfig-subscription-add"),
                Kind::Subscription,
                json!({"link":page.downstreams.first().map(|d|d.link.as_str()).unwrap_or(link),"domain":domain,"area":area,"subscribed":true,"source":"manual","version":0,"changed_at":0}),
            ),
        ];
        for d in &page.downstreams {
            forms.push((
                format!("{} / {}", t("networks-downstream"), d.link),
                Kind::Downstream,
                serde_json::to_value(d).map_err(|_| t("netconfig-invalid"))?,
            ));
        }
        for a in &page.area_access {
            forms.push((
                format!("{} / {}@{}", t("netconfig-area-access"), a.area, a.domain),
                Kind::Access,
                serde_json::to_value(a).map_err(|_| t("netconfig-invalid"))?,
            ));
        }
        for s in &page.subscriptions {
            let mut v = serde_json::to_value(s).map_err(|_| t("netconfig-invalid"))?;
            v["source"] = json!("manual");
            forms.push((
                format!(
                    "{} / {} / {} / {}",
                    t("networks-subscription"),
                    s.link,
                    s.area,
                    t(if s.subscribed {
                        "networks-subscribed"
                    } else {
                        "networks-unsubscribed"
                    })
                ),
                Kind::Subscription,
                v,
            ));
        }
        for d in &page.downstreams {
            forms.push((
                format!("Rescan / {}", d.link),
                Kind::Rescan,
                json!({"link":d.link,"request":{"area":area,"count":d.max_area}}),
            ));
        }
        let mut entries: Vec<_> = forms.iter().map(|f| f.0.clone()).collect();
        entries.extend(
            page.downstreams
                .iter()
                .map(|d| format!("{} / {}", t("netconfig-areafix-secret"), d.link)),
        );
        terminal
            .draw(|f| {
                let p = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(4),
                ])
                .split(f.area());
                f.render_widget(Paragraph::new(t("netconfig-hub")), p[0]);
                let mut state = ListState::default().with_selected(Some(selected));
                f.render_stateful_widget(
                    List::new(entries.iter().map(|s| ListItem::new(clean(s))))
                        .block(Block::default().borders(Borders::ALL))
                        .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
                    p[1],
                    &mut state,
                );
                f.render_widget(
                    Paragraph::new(format!("{}\n{}", t("netconfig-hub-help"), status))
                        .wrap(Wrap { trim: false }),
                    p[2],
                );
            })
            .map_err(|_| t("sfconfig-connection-error"))?;
        let Event::Key(k) = event::read().map_err(|_| t("sfconfig-connection-error"))? else {
            continue;
        };
        if !matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            return Ok(());
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(entries.len().saturating_sub(1)),
            KeyCode::PageUp => selected = selected.saturating_sub(10),
            KeyCode::PageDown => selected = (selected + 10).min(entries.len().saturating_sub(1)),
            KeyCode::Char('f') => page = hub_page(backend)?,
            KeyCode::Enter if selected < forms.len() => {
                let (_, kind, value) = &forms[selected];
                edit_form(terminal, backend, data, Form::new(*kind, value.clone()))?;
                page = hub_page(backend)?;
            }
            KeyCode::Enter | KeyCode::Char('c') if selected >= forms.len() => {
                let d = &page.downstreams[selected - forms.len()];
                if let Backend::Online { runtime, client } = backend {
                    let current = runtime
                        .block_on(client.binkp_status())
                        .map_err(|_| t("networks-access"))?;
                    let credential = current
                        .links
                        .iter()
                        .find(|l| l.link == d.link)
                        .map(|l| &l.areafix_credential);
                    status = t(match credential {
                        Some(sf_bbs::SecretStatus::Configured) => "sfconfig-secret-configured",
                        Some(sf_bbs::SecretStatus::Invalid) => "sfconfig-secret-invalid",
                        _ => "sfconfig-secret-missing",
                    });
                    let request = if k.code == KeyCode::Char('c') {
                        if prompt(terminal, "netconfig-clear-prompt", false)?.as_deref()
                            != Some("CLEAR")
                        {
                            continue;
                        }
                        sf_bbs::binkp::Action::ClearAreaFixCredential {
                            link: d.link.clone(),
                            expected: current.policy,
                        }
                    } else {
                        let Some(secret) =
                            prompt(terminal, "netconfig-areafix-secret-prompt", true)?
                        else {
                            continue;
                        };
                        sf_bbs::binkp::Action::AreaFixCredential {
                            link: d.link.clone(),
                            expected: current.policy,
                            secret,
                        }
                    };
                    status = match backend
                        .network_action(command_id(), NetworkAction::Binkp { request })
                    {
                        Ok(()) => t("binkp-credential-updated"),
                        Err(e) => e,
                    };
                } else {
                    status = t("netconfig-secret-online")
                }
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data() -> Data {
        let config = sf_core::RuntimeConfig::synthetic_fixture();
        Data {
            snapshot: ConfigurationSnapshot {
                version: sf_bbs::configuration_version(&config).unwrap(),
                config,
                restart_required: false,
                ssh_keys: vec![],
                capabilities: vec![],
                domains: vec![],
            },
            qwk: vec![],
            areas: vec![],
            conferences: vec![],
            secrets: vec![],
        }
    }
    #[test]
    fn creation_review_includes_every_added_field_and_removal() {
        let f = Form::new(
            Kind::Qwk,
            json!({"link":{"id":"peer","version":0,"enabled":false},"mappings":[]}),
        );
        assert!(f.before.is_null());
        let mut rows = vec![];
        diff(&f.before, &f.draft, "", &mut rows);
        assert!(rows.iter().any(|r| r.contains("peer")));
        assert!(rows.iter().any(|r| r.contains("Enabled")));
        let mut removed = vec![];
        diff(
            &json!({"routes":[{"domain":"retired","target":{"kind":"zone","zone":10}}]}),
            &json!({"routes":[]}),
            "",
            &mut removed,
        );
        assert!(removed.iter().any(|r| r.contains("retired")));
        assert!(removed.iter().any(|r| r.contains("zone")));
        assert!(removed.iter().any(|r| r.contains("10")));
    }
    #[test]
    fn named_route_form_replaces_variant_fields_and_validates_numeric_input() {
        let mut f = Form::new(
            Kind::Ftn,
            json!({"routes":[{"domain":"synthetic","target":{"kind":"default"},"link":"peer"}]}),
        );
        f.path = vec!["routes".into(), "0".into(), "target".into()];
        f.edit("boss".into());
        assert_eq!(
            f.draft["routes"][0]["target"],
            json!({"kind":"boss","address":"10:100/2"})
        );
        f.selected = 1;
        f.edit("zone".into());
        assert_eq!(
            f.draft["routes"][0]["target"],
            json!({"kind":"zone","zone":10})
        );
        f.selected = 1;
        f.edit("not-a-number".into());
        assert_eq!(f.draft["routes"][0]["target"]["zone"], 10);
        assert!(f.input.is_some());
    }
    #[test]
    fn point_domain_drafts_and_review_preserve_typed_identity() {
        let mut f = Form::new(Kind::Ftn, json!({"akas":[]}));
        f.path = vec!["akas".into()];
        let mut a = f.template(&data()).unwrap();
        a["endpoint"]["address"] = json!("10:100/1.3");
        a["endpoint"]["domain"] = json!("othernet");
        a["primary"] = json!(true);
        a["enabled"] = json!(true);
        let typed: sf_core::ftn::Aka = serde_json::from_value(a.clone()).unwrap();
        assert_eq!(typed.endpoint.address.point(), 3);
        f.draft["akas"].as_array_mut().unwrap().push(a);
        let mut changes = vec![];
        diff(&f.before, &f.draft, "", &mut changes);
        assert!(!changes.is_empty());
        assert!(!changes.join(" ").contains("password"));
        assert_eq!(f.before["akas"].as_array().unwrap().len(), 0);
    }
    #[test]
    fn forms_render_at_supported_terminal_sizes_without_raw_json() {
        let d = data();
        let f = Form::new(
            Kind::Ftn,
            serde_json::to_value(&d.snapshot.config.ftn).unwrap(),
        );
        for (w, h) in [(60, 20), (80, 24), (100, 30)] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw_form(frame, &f, &d)).unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("Networks configuration"));
            assert!(!text.contains("netconfig-field"));
            assert!(!text.contains("\"enabled\":"));
        }
    }
}
