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

//! Native typed configuration UI. Persistence and validation belong to sf-bbs/sf-core.
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use sf_bbs::{ConfigurationResult, ConfigurationSnapshot, OfflineConfiguration, OperatorClient};
use sf_core::{
    configuration::*, LocalOperatorCapability as Cap, LocalOperatorIdentity, LocalizationArgs,
    Localizer,
};
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn t(key: &str) -> String {
    sf_core::text(key, &LocalizationArgs::new())
}
fn issue_text(issue: &ConfigurationIssue) -> String {
    let label = issue
        .field
        .as_ref()
        .map(|field| format!("{}: ", t(field.label_key())))
        .unwrap_or_default();
    format!("{}{}", label, t(&issue.message_key))
}
fn command_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}
const SECTIONS: [&str; 8] = [
    "general",
    "nodes",
    "callers",
    "presentation",
    "security",
    "operators",
    "messages-files",
    "storage",
];
const CAPS: [Cap; 16] = Cap::ALL;

fn cap_key(cap: Cap) -> &'static str {
    match cap {
        Cap::BoardStatistics => "sfconfig-cap-statistics",
        Cap::NodeStatus => "sfconfig-cap-nodes",
        Cap::OperationalEvents => "sfconfig-cap-events",
        Cap::CallerActivity => "sfconfig-cap-callers",
        Cap::Notifications => "sfconfig-cap-notifications",
        Cap::MaintenanceStatus => "sfconfig-cap-maintenance",
        Cap::AcknowledgeNotifications => "sfconfig-cap-ack",
        Cap::AdjustSessionTime => "sfconfig-cap-time",
        Cap::ManagePageAvailability => "sfconfig-cap-availability",
        Cap::ManageCallerPages => "sfconfig-cap-pages",
        Cap::ChatWithCaller => "sfconfig-cap-chat",
        Cap::DisconnectSession => "sfconfig-cap-disconnect",
        Cap::RequestGracefulShutdown => "sfconfig-cap-shutdown",
        Cap::ReadConfiguration => "sfconfig-cap-read-config",
        Cap::ChangeOnlineConfiguration => "sfconfig-cap-config",
        Cap::ChangeSensitiveConfiguration => "sfconfig-cap-sensitive",
    }
}
fn identity_name(identity: &LocalOperatorIdentity) -> String {
    match identity {
        LocalOperatorIdentity::Unix { uid, .. } => format!("UID {uid}"),
        LocalOperatorIdentity::Windows { sid, .. } => format!("SID {sid}"),
    }
}
fn grants(identity: &mut LocalOperatorIdentity) -> &mut Vec<Cap> {
    match identity {
        LocalOperatorIdentity::Unix { capabilities, .. }
        | LocalOperatorIdentity::Windows { capabilities, .. } => capabilities,
    }
}

#[derive(Clone)]
enum Row {
    Field(ConfigurationField),
    Capability(usize, Cap),
    AddCurrent,
    Summary(String),
}
#[derive(Clone, Copy, PartialEq)]
enum Prompt {
    Quit,
    Reload,
    Cancel,
}
pub struct ConfigModel {
    snapshot: ConfigurationSnapshot,
    edits: Vec<ConfigurationEdit>,
    operators: sf_core::OperatorConfig,
    section: usize,
    selected: usize,
    input: Option<String>,
    review: bool,
    review_offset: u16,
    help: bool,
    prompt: Option<Prompt>,
    status: String,
    offline: bool,
    disconnected: bool,
    pending_command: Option<String>,
}
impl ConfigModel {
    pub fn new(snapshot: ConfigurationSnapshot, offline: bool) -> Self {
        let operators = snapshot.config.operators.clone();
        Self {
            snapshot,
            edits: vec![],
            operators,
            section: 0,
            selected: 0,
            input: None,
            review: false,
            review_offset: 0,
            help: false,
            prompt: None,
            status: String::new(),
            offline,
            disconnected: false,
            pending_command: None,
        }
    }
    fn reload(&mut self, snapshot: ConfigurationSnapshot) {
        let section = self.section;
        let selected = self.selected;
        let status = self.status.clone();
        *self = Self::new(snapshot, self.offline);
        self.section = section;
        self.selected = selected.min(self.rows().len().saturating_sub(1));
        self.status = status;
    }
    fn online_lost(&mut self, error: String) {
        self.disconnected = !self.offline;
        self.review = false;
        if self.disconnected && self.prompt == Some(Prompt::Reload) {
            self.prompt = None;
        }
        self.status = error;
    }
    fn probe(&mut self, result: Result<ConfigurationSnapshot, String>) {
        match result {
            Ok(snapshot) => {
                if snapshot.version != self.snapshot.version {
                    if self.dirty() {
                        self.status = t("sfconfig-conflict");
                        // A draft keeps its expected version, but grants are current.
                        self.snapshot.capabilities = snapshot.capabilities;
                    } else {
                        self.reload(snapshot);
                    }
                } else {
                    self.snapshot.capabilities = snapshot.capabilities;
                }
            }
            Err(error) => self.online_lost(error),
        }
    }
    fn dirty(&self) -> bool {
        !self.edits.is_empty()
            || self.operators != self.snapshot.config.operators
            || self.input.is_some()
    }
    fn candidate(&self) -> ConfigurationCandidate {
        ConfigurationCandidate {
            expected: self.snapshot.version.clone(),
            edits: self.edits.clone(),
            operators: (self.operators != self.snapshot.config.operators)
                .then(|| self.operators.clone()),
        }
    }
    fn rows(&self) -> Vec<Row> {
        let section = SECTIONS[self.section];
        match section {
            "operators" => {
                let mut rows = vec![Row::AddCurrent];
                for (index, _) in self.operators.local_identities.iter().enumerate() {
                    for cap in CAPS {
                        rows.push(Row::Capability(index, cap));
                    }
                }
                rows
            }
            "messages-files" => self
                .snapshot
                .domains
                .iter()
                .map(|domain| {
                    Row::Summary(format!(
                        "{} {}: {} | {} | {} {} / {} {}",
                        t(if domain.kind == "messages" {
                            "sfconfig-conference"
                        } else {
                            "sfconfig-file-area"
                        }),
                        domain.number,
                        domain.name,
                        t(if domain.active {
                            "sfconfig-enabled"
                        } else {
                            "sfconfig-disabled"
                        }),
                        t("sfconfig-read-security"),
                        domain.read_security,
                        t("sfconfig-write-security"),
                        domain.write_security
                    ))
                })
                .collect(),
            "storage" => vec![
                Row::Summary(t("sfconfig-storage-help")),
                Row::Summary(format!(
                    "{}: {}",
                    t("sfconfig-backup"),
                    t("sfconfig-backup-previous")
                )),
            ],
            _ => {
                let mut rows: Vec<_> = ConfigurationField::fields(&self.snapshot.config)
                    .into_iter()
                    .filter(|field| field.section() == section)
                    .map(Row::Field)
                    .collect();
                if section == "general" {
                    rows.push(Row::Summary(format!(
                        "{}: {}",
                        t("sfconfig-board-name"),
                        self.snapshot.config.board.name
                    )));
                    rows.push(Row::Summary(format!(
                        "{}: {}",
                        t("sfconfig-sysop-display"),
                        self.snapshot.config.board.sysop
                    )));
                }
                if section == "security" {
                    for (index, status) in self.snapshot.ssh_keys.iter().enumerate() {
                        rows.push(Row::Summary(format!(
                            "{} {}: {}",
                            t("sfconfig-ssh-key"),
                            index + 1,
                            t(match status {
                                sf_bbs::SecretStatus::Missing => "sfconfig-secret-missing",
                                sf_bbs::SecretStatus::Configured => "sfconfig-secret-configured",
                                sf_bbs::SecretStatus::Invalid => "sfconfig-secret-invalid",
                            })
                        )));
                    }
                }
                rows
            }
        }
    }
    fn row_text(&self, row: &Row) -> String {
        match row {
            Row::Field(field) => {
                let value = self
                    .edits
                    .iter()
                    .find(|e| &e.field == field)
                    .map(|e| e.value.clone())
                    .unwrap_or_else(|| field.value(&self.snapshot.config));
                let suffix = match field {
                    ConfigurationField::ListenerEnabled { index }
                    | ConfigurationField::ListenerAddress { index } => format!(
                        " ({})",
                        self.snapshot.config.transports[*index].effective_name(*index)
                    ),
                    _ => String::new(),
                };
                format!(
                    "{}{}: {}{}",
                    t(field.label_key()),
                    suffix,
                    value,
                    if self.edits.iter().any(|e| &e.field == field) {
                        " *"
                    } else {
                        ""
                    }
                )
            }
            Row::Capability(index, cap) => {
                let mut identity = self.operators.local_identities[*index].clone();
                let enabled = grants(&mut identity).contains(cap);
                format!(
                    "{}  [{}] {}",
                    identity_name(&identity),
                    if enabled { "x" } else { " " },
                    t(cap_key(*cap))
                )
            }
            Row::AddCurrent => t("sfconfig-enroll-current"),
            Row::Summary(value) => value.clone(),
        }
    }
    fn result(&mut self, result: ConfigurationResult) -> bool {
        self.review = false;
        match result {
            ConfigurationResult::Saved {
                version,
                restart_required,
                ..
            } => {
                if let Ok(mut config) = self.candidate().validate(&self.snapshot.config) {
                    config.revision = version.revision;
                    self.snapshot.config = config;
                    self.snapshot.version = version;
                    self.snapshot.restart_required = restart_required;
                    self.edits.clear();
                    self.operators = self.snapshot.config.operators.clone();
                    self.pending_command = None;
                }
                self.status = t(if restart_required {
                    "sfconfig-saved-restart"
                } else {
                    "sfconfig-saved"
                });
                true
            }
            ConfigurationResult::Conflict { .. } => {
                self.status = t("sfconfig-conflict");
                self.pending_command = None;
                false
            }
            ConfigurationResult::Invalid { issues } => {
                self.status = issues.iter().map(issue_text).collect::<Vec<_>>().join("; ");
                self.pending_command = None;
                false
            }
            ConfigurationResult::Denied => {
                self.status = t("sfconfig-denied");
                self.pending_command = None;
                false
            }
            ConfigurationResult::Replayed {
                result_class: Some(class),
                revision: Some(revision),
            } if class == "configuration-saved" || class == "configuration-restart-required" => {
                // The receipt proves this unchanged candidate committed, even if
                // its operator edit now denies the following snapshot read.
                if let Ok(mut config) = self.candidate().validate(&self.snapshot.config) {
                    config.revision = revision;
                    if let Ok(version) = sf_bbs::configuration_version(&config) {
                        return self.result(ConfigurationResult::Saved {
                            version,
                            effects: vec![],
                            restart_required: class == "configuration-restart-required",
                        });
                    }
                }
                self.status = t("sfconfig-recovery-required");
                false
            }
            ConfigurationResult::Replayed {
                result_class: Some(class),
                ..
            } if class == "configuration-not-committed" => {
                self.pending_command = None;
                self.status = t("sfconfig-not-committed");
                false
            }
            _ => {
                self.status = t("sfconfig-recovery-required");
                false
            }
        }
    }
    fn key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.help = false;
            self.review = false;
            if self.dirty() {
                self.prompt = Some(Prompt::Quit);
                return Action::None;
            }
            return Action::Quit;
        }
        if self.disconnected
            && matches!(key.code, KeyCode::Char('s' | 'S' | 'r' | 'R'))
            && self.input.is_none()
            && !self.help
            && self.prompt.is_none()
        {
            self.status = t("sfconfig-reopen-required");
            return Action::None;
        }
        if self.help {
            match key.code {
                KeyCode::Down | KeyCode::PageDown => {
                    self.review_offset = self.review_offset.saturating_add(5).min(2048)
                }
                KeyCode::Up | KeyCode::PageUp => {
                    self.review_offset = self.review_offset.saturating_sub(5)
                }
                KeyCode::Home => self.review_offset = 0,
                _ => {}
            }
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1)) {
                self.help = false;
            }
            return Action::None;
        }
        if let Some(prompt) = self.prompt {
            if key.code == KeyCode::Esc {
                self.prompt = None;
            }
            if key.code == KeyCode::Enter {
                self.prompt = None;
                return match prompt {
                    Prompt::Quit => Action::Quit,
                    Prompt::Reload => Action::Reload,
                    Prompt::Cancel => {
                        self.edits.clear();
                        self.operators = self.snapshot.config.operators.clone();
                        self.pending_command = None;
                        Action::None
                    }
                };
            }
            return Action::None;
        }
        if let Some(input) = &mut self.input {
            match key.code {
                KeyCode::Esc => self.input = None,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(ch) if !ch.is_control() && input.len() + ch.len_utf8() <= 256 => {
                    input.push(ch)
                }
                KeyCode::Enter => {
                    let value = self.input.take().unwrap_or_default();
                    if let Some(Row::Field(field)) = self.rows().get(self.selected) {
                        let mut config = self.snapshot.config.clone();
                        match field.apply(&mut config, &value) {
                            Ok(()) => {
                                self.edits.retain(|e| &e.field != field);
                                if field.value(&self.snapshot.config) != value {
                                    self.edits.push(ConfigurationEdit {
                                        field: field.clone(),
                                        value,
                                    });
                                }
                                self.status.clear();
                                self.pending_command = None;
                            }
                            Err(issue) => {
                                self.status = t(&issue.message_key);
                                self.input = Some(value);
                            }
                        }
                    }
                }
                _ => {}
            }
            return Action::None;
        }
        if self.review {
            match key.code {
                KeyCode::Down | KeyCode::PageDown => {
                    self.review_offset = self.review_offset.saturating_add(5).min(2048)
                }
                KeyCode::Up | KeyCode::PageUp => {
                    self.review_offset = self.review_offset.saturating_sub(5)
                }
                KeyCode::Home => self.review_offset = 0,
                KeyCode::Esc => self.review = false,
                KeyCode::Enter => return Action::Save,
                _ => {}
            }
            return Action::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
            || matches!(key.code, KeyCode::Char('q' | 'Q'))
        {
            if self.dirty() {
                self.prompt = Some(Prompt::Quit);
                return Action::None;
            }
            return Action::Quit;
        }
        let length = self.rows().len();
        match key.code {
            KeyCode::Tab | KeyCode::Right => {
                self.section = (self.section + 1) % SECTIONS.len();
                self.selected = 0;
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.section = (self.section + SECTIONS.len() - 1) % SECTIONS.len();
                self.selected = 0;
            }
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(length.saturating_sub(1)),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(10),
            KeyCode::PageDown => self.selected = (self.selected + 10).min(length.saturating_sub(1)),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = length.saturating_sub(1),
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.help = true;
                self.review_offset = 0;
            }
            KeyCode::Char('r' | 'R') => {
                if self.dirty() {
                    self.prompt = Some(Prompt::Reload);
                } else {
                    return Action::Reload;
                }
            }
            KeyCode::Char('c' | 'C') | KeyCode::Esc if self.dirty() => {
                self.prompt = Some(Prompt::Cancel)
            }
            KeyCode::Char('s' | 'S') if self.dirty() => {
                match self.candidate().validate(&self.snapshot.config) {
                    Ok(_) => {
                        self.review = true;
                        self.review_offset = 0;
                        self.status.clear();
                    }
                    Err(issues) => {
                        self.status = issues.iter().map(issue_text).collect::<Vec<_>>().join("; ")
                    }
                }
            }
            KeyCode::Char('d' | 'D') if SECTIONS[self.section] == "operators" => {
                if let Some(Row::Capability(index, _)) = self.rows().get(self.selected) {
                    self.operators.local_identities.remove(*index);
                    self.selected = self.selected.min(self.rows().len().saturating_sub(1));
                    self.pending_command = None;
                }
            }
            KeyCode::Enter => match self.rows().get(self.selected) {
                Some(Row::Field(field)) => {
                    self.input = Some(
                        self.edits
                            .iter()
                            .find(|e| &e.field == field)
                            .map(|e| e.value.clone())
                            .unwrap_or_else(|| field.value(&self.snapshot.config)),
                    )
                }
                Some(Row::Capability(index, cap)) => {
                    let caps = grants(&mut self.operators.local_identities[*index]);
                    if caps.contains(cap) {
                        caps.retain(|c| c != cap);
                    } else {
                        caps.push(*cap);
                    }
                    self.pending_command = None;
                }
                Some(Row::AddCurrent) => {
                    if let Ok(principal) = sf_bbs::current_operator_identity() {
                        let identity = if let Some(uid) = principal
                            .strip_prefix("unix-uid:")
                            .and_then(|v| v.parse().ok())
                        {
                            LocalOperatorIdentity::Unix {
                                uid,
                                label: None,
                                capabilities: Cap::READ_ONLY.to_vec(),
                            }
                        } else if let Some(sid) = principal.strip_prefix("windows-sid:") {
                            LocalOperatorIdentity::Windows {
                                sid: sid.into(),
                                label: None,
                                capabilities: Cap::READ_ONLY.to_vec(),
                            }
                        } else {
                            return Action::None;
                        };
                        if !self
                            .operators
                            .local_identities
                            .iter()
                            .any(|i| identity_name(i) == identity_name(&identity))
                        {
                            self.operators.local_identities.push(identity);
                            self.pending_command = None;
                        }
                        self.status = t("sfconfig-enrollment-help");
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Action::None
    }
}
#[derive(Debug, PartialEq)]
enum Action {
    None,
    Quit,
    Reload,
    Save,
}
fn effect_key(effect: ConfigurationEffect) -> &'static str {
    match effect {
        ConfigurationEffect::Live => "sfconfig-effect-live",
        ConfigurationEffect::NewSessions => "sfconfig-effect-new-sessions",
        ConfigurationEffect::RestartRequired => "sfconfig-effect-restart",
        ConfigurationEffect::OfflineOnly => "sfconfig-effect-offline",
    }
}
fn render(frame: &mut Frame<'_>, model: &ConfigModel) {
    let area = frame.area();
    if area.width < 60 || area.height < 20 {
        frame.render_widget(Paragraph::new(t("sfconfig-resize")), area);
        return;
    }
    let parts = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(4),
        Constraint::Length(2),
    ])
    .split(area);
    let mode = t(if model.offline {
        "sfconfig-offline"
    } else if model.disconnected {
        "sfconfig-disconnected"
    } else {
        "sfconfig-online"
    });
    let state = if model.dirty() {
        t("sfconfig-dirty")
    } else {
        t("sfconfig-clean")
    };
    frame.render_widget(
        Paragraph::new(format!(
            "sfconfig — {}\n{}\n{} {} | {}{}",
            model.snapshot.config.board.name,
            mode,
            t("sfconfig-version"),
            model.snapshot.version.revision,
            state,
            if model.snapshot.restart_required {
                format!(" | {}", t("sfconfig-effect-restart"))
            } else {
                String::new()
            }
        ))
        .block(Block::default().borders(Borders::ALL)),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "{} / {}   {}",
            model.section + 1,
            SECTIONS.len(),
            t(&format!("sfconfig-section-{}", SECTIONS[model.section]))
        ))
        .style(Style::default().add_modifier(Modifier::BOLD)),
        parts[1],
    );
    if model.help {
        let maintenance = if SECTIONS[model.section] == "storage" {
            sf_core::observability::MaintenanceService::ALL
                .into_iter()
                .map(|service| t(service.guidance_key()))
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            String::new()
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{}\n\n{}\n\n{}\n\n{}\n\nconfiguration.{}\n\n{}",
                t(&format!("sfconfig-help-{}", SECTIONS[model.section])),
                t("sfconfig-help-editing"),
                t("sfconfig-help-conflict"),
                t("sfconfig-help-mode"),
                SECTIONS[model.section],
                maintenance
            ))
            .wrap(Wrap { trim: false })
            .scroll((model.review_offset, 0))
            .block(
                Block::default()
                    .title(t("sfconfig-help"))
                    .borders(Borders::ALL),
            ),
            parts[2],
        );
    } else if model.review {
        let mut lines = vec![t("sfconfig-review-valid")];
        for edit in &model.edits {
            lines.push(format!(
                "{}: {} -> {} [{}]",
                t(edit.field.label_key()),
                edit.field.value(&model.snapshot.config),
                edit.value,
                t(effect_key(edit.field.effect()))
            ));
        }
        if model.operators != model.snapshot.config.operators {
            lines.push(t("sfconfig-review-operators"));
            for identity in &model.operators.local_identities {
                let name = identity_name(identity);
                let old = model
                    .snapshot
                    .config
                    .operators
                    .local_identities
                    .iter()
                    .find(|old| identity_name(old) == name);
                for capability in CAPS {
                    let before = old.is_some_and(|identity| {
                        grants(&mut identity.clone()).contains(&capability)
                    });
                    let after = grants(&mut identity.clone()).contains(&capability);
                    if before != after {
                        lines.push(format!(
                            "{}: {} {}",
                            name,
                            t(if after {
                                "sfconfig-cap-added"
                            } else {
                                "sfconfig-cap-removed"
                            }),
                            t(cap_key(capability))
                        ));
                    }
                }
            }
            for old in &model.snapshot.config.operators.local_identities {
                if !model
                    .operators
                    .local_identities
                    .iter()
                    .any(|new| identity_name(new) == identity_name(old))
                {
                    lines.push(format!(
                        "{}: {}",
                        identity_name(old),
                        t("sfconfig-principal-removed")
                    ));
                }
            }
        }
        lines.push(t("sfconfig-confirm-save"));
        frame.render_widget(
            Paragraph::new(lines.join("\n"))
                .scroll((model.review_offset, 0))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(t("sfconfig-review"))
                        .borders(Borders::ALL),
                ),
            parts[2],
        );
    } else {
        let rows: Vec<ListItem<'_>> = model
            .rows()
            .iter()
            .map(|row| ListItem::new(model.row_text(row)))
            .collect();
        let mut state = ListState::default().with_selected(Some(model.selected));
        frame.render_stateful_widget(
            List::new(rows)
                .block(Block::default().borders(Borders::ALL))
                .highlight_symbol("> ")
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
            parts[2],
            &mut state,
        );
    }
    let detail = if model.review {
        t("sfconfig-confirm-save")
    } else if let Some(prompt) = model.prompt {
        t(match prompt {
            Prompt::Quit => "sfconfig-discard-quit",
            Prompt::Reload => "sfconfig-discard-reload",
            Prompt::Cancel => "sfconfig-discard-cancel",
        })
    } else if let Some(input) = &model.input {
        format!(
            "{}: {}_\n{}",
            t("sfconfig-edit"),
            input,
            t("sfconfig-edit-footer")
        )
    } else if !model.status.is_empty() {
        model.status.clone()
    } else {
        match model.rows().get(model.selected) {
            Some(Row::Field(field)) => format!(
                "{} — {}",
                t(effect_key(field.effect())),
                t(&format!("sfconfig-help-{}", field.section()))
            ),
            _ => t(&format!("sfconfig-help-{}", SECTIONS[model.section])),
        }
    };
    frame.render_widget(Paragraph::new(detail).wrap(Wrap { trim: false }), parts[3]);
    frame.render_widget(
        Paragraph::new(t("sfconfig-footer")).wrap(Wrap { trim: false }),
        parts[4],
    );
}

enum Backend {
    Online {
        runtime: tokio::runtime::Runtime,
        client: Box<OperatorClient>,
    },
    Offline(Box<OfflineConfiguration>),
}
impl Backend {
    fn snapshot(&mut self) -> Result<ConfigurationSnapshot, String> {
        match self {
            Self::Online { runtime, client } => runtime
                .block_on(client.configuration_snapshot())
                .map_err(|_| t("sfconfig-connection-error")),
            Self::Offline(authority) => authority
                .snapshot()
                .map_err(|_| t("sfconfig-recovery-required")),
        }
    }
    fn save(
        &mut self,
        command: String,
        candidate: ConfigurationCandidate,
    ) -> Result<ConfigurationResult, String> {
        match self {
            Self::Online { runtime, client } => runtime
                .block_on(client.apply_configuration(command, candidate))
                .map_err(|_| t("sfconfig-save-uncertain")),
            Self::Offline(authority) => authority
                .apply(&command, &candidate)
                .map_err(|_| t("sfconfig-save-uncertain")),
        }
    }
}
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
}
pub fn run_from_env() -> Result<(), String> {
    sf_core::with_localizer(Localizer::embedded_en_us(), || {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
            println!("{}", t("sfconfig-usage"));
            return Ok(());
        }
        if args.len() == 1 && (args[0] == "--version" || args[0] == "-V") {
            println!("sfconfig {}", sf_core::PRODUCT_VERSION);
            return Ok(());
        }
        let mut board = None;
        let mut offline = false;
        let mut index = 0;
        while index < args.len() {
            match args[index].to_str() {
                Some("--board") if board.is_none() => {
                    index += 1;
                    board = args.get(index).map(PathBuf::from);
                }
                Some("--offline") if !offline => offline = true,
                _ => return Err(t("sfconfig-usage")),
            }
            index += 1;
        }
        let board = board.ok_or_else(|| t("sfconfig-usage"))?;
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(t("sfconfig-terminal-required"));
        }
        let mut backend = if offline {
            Backend::Offline(Box::new(
                OfflineConfiguration::open(&board).map_err(|_| t("sfconfig-offline-error"))?,
            ))
        } else {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| t("sfconfig-connection-error"))?;
            let client = runtime
                .block_on(OperatorClient::connect(&board))
                .map_err(|_| t("sfconfig-connection-error"))?;
            Backend::Online {
                runtime,
                client: Box::new(client),
            }
        };
        let mut model = ConfigModel::new(backend.snapshot()?, offline);
        enable_raw_mode().map_err(|e| e.to_string())?;
        let _restore = Restore;
        execute!(io::stdout(), EnterAlternateScreen).map_err(|e| e.to_string())?;
        let mut terminal =
            Terminal::new(CrosstermBackend::new(io::stdout())).map_err(|e| e.to_string())?;
        let mut last_probe = Instant::now();
        loop {
            if !offline && !model.disconnected && last_probe.elapsed() >= Duration::from_secs(5) {
                model.probe(backend.snapshot());
                last_probe = Instant::now();
            }
            terminal
                .draw(|frame| render(frame, &model))
                .map_err(|e| e.to_string())?;
            if !event::poll(Duration::from_millis(100)).map_err(|e| e.to_string())? {
                continue;
            }
            if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    continue;
                }
                let size = terminal.size().map_err(|e| e.to_string())?;
                if (size.width < 60 || size.height < 20)
                    && (model.dirty() || !matches!(key.code, KeyCode::Char('q' | 'Q')))
                {
                    continue;
                }
                match model.key(key) {
                    Action::Quit => return Ok(()),
                    Action::None => {}
                    Action::Reload => match backend.snapshot() {
                        Ok(snapshot) => {
                            model.reload(snapshot);
                            model.status.clear();
                        }
                        Err(error) => model.online_lost(error),
                    },
                    Action::Save => {
                        let command = model.pending_command.get_or_insert_with(command_id).clone();
                        match backend.save(command, model.candidate()) {
                            Ok(result) => {
                                if model.result(result) {
                                    match backend.snapshot() {
                                        Ok(snapshot) => {
                                            model.reload(snapshot);
                                        }
                                        Err(error) => {
                                            model.online_lost(error);
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                model.review = false;
                                model.status = error;
                            }
                        }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> ConfigModel {
        let config = sf_core::RuntimeConfig::synthetic_fixture();
        let version = sf_bbs::configuration_version(&config).unwrap();
        ConfigModel::new(
            ConfigurationSnapshot {
                config,
                version,
                restart_required: false,
                ssh_keys: vec![sf_bbs::SecretStatus::Configured],
                capabilities: vec![],
                domains: vec![],
            },
            false,
        )
    }
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn changed(model: &mut ConfigModel) {
        model.edits.push(ConfigurationEdit {
            field: ConfigurationField::InactivityMinutes,
            value: "7".into(),
        });
    }
    #[test]
    fn navigation_retains_edits_and_quit_prompts_only_when_dirty() {
        let mut model = model();
        assert_eq!(model.key(key(KeyCode::Char('q'))), Action::Quit);
        changed(&mut model);
        for _ in 0..SECTIONS.len() {
            model.key(key(KeyCode::Tab));
        }
        assert_eq!(model.section, 0);
        assert!(model.dirty());
        assert_eq!(model.key(key(KeyCode::Char('q'))), Action::None);
        assert!(model.prompt == Some(Prompt::Quit));
        model.key(key(KeyCode::Esc));
        assert!(model.dirty());
        model.key(key(KeyCode::Char('q')));
        assert_eq!(model.key(key(KeyCode::Enter)), Action::Quit);
    }
    #[test]
    fn field_cancel_does_not_mutate_authority_and_save_requires_review() {
        let mut model = model();
        model.key(key(KeyCode::Enter));
        model.key(key(KeyCode::Char('x')));
        model.key(key(KeyCode::Esc));
        assert!(!model.dirty());
        changed(&mut model);
        assert_eq!(model.key(key(KeyCode::Char('s'))), Action::None);
        assert!(model.review);
        model.key(key(KeyCode::Esc));
        assert!(model.dirty());
        model.key(key(KeyCode::Char('s')));
        assert_eq!(model.key(key(KeyCode::Enter)), Action::Save);
    }
    #[test]
    fn conflict_retains_local_edits_and_reload_needs_explicit_confirmation() {
        let mut model = model();
        changed(&mut model);
        assert!(!model.result(ConfigurationResult::Conflict {
            current: model.snapshot.version.clone()
        }));
        assert!(model.dirty());
        assert_eq!(model.key(key(KeyCode::Char('r'))), Action::None);
        model.key(key(KeyCode::Esc));
        assert!(model.dirty());
        model.key(key(KeyCode::Char('r')));
        assert_eq!(model.key(key(KeyCode::Enter)), Action::Reload);
    }
    #[test]
    fn confirmed_saved_edits_are_clean_even_if_following_read_is_revoked() {
        let mut model = model();
        changed(&mut model);
        let mut version = model.snapshot.version.clone();
        version.revision += 1;
        assert!(model.result(ConfigurationResult::Saved {
            version,
            effects: vec![ConfigurationEffect::NewSessions],
            restart_required: false
        }));
        assert!(!model.dirty());
        assert_eq!(model.snapshot.config.caller.inactivity_minutes, 7);
        assert_eq!(model.key(key(KeyCode::Char('q'))), Action::Quit);
    }
    #[test]
    fn refreshed_snapshot_preserves_section_and_selection() {
        let mut model = model();
        model.section = 2;
        model.selected = 4;
        changed(&mut model);
        let snapshot = model.snapshot.clone();
        model.reload(snapshot);
        assert_eq!(model.section, 2);
        assert_eq!(model.selected, 4);
        assert!(!model.dirty());
    }
    #[test]
    fn control_c_during_field_edit_prompts_without_losing_input() {
        let mut model = model();
        model.key(key(KeyCode::Enter));
        assert_eq!(
            model.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::None
        );
        assert!(model.prompt == Some(Prompt::Quit));
        assert!(model.input.is_some());
        model.key(key(KeyCode::Esc));
        assert!(model.input.is_some());
    }
    #[test]
    fn lost_online_authority_preserves_candidate_and_requires_explicit_reopen() {
        let mut model = model();
        changed(&mut model);
        model.pending_command = Some("a".repeat(32));
        model.review = true;
        model.prompt = Some(Prompt::Reload);
        let before = format!("{:?}", model.candidate());
        model.probe(Err("lost".into()));
        assert!(model.disconnected && !model.offline && !model.review);
        assert!(model.prompt.is_none());
        assert_eq!(format!("{:?}", model.candidate()), before);
        assert_eq!(
            model.pending_command.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(model.key(key(KeyCode::Char('s'))), Action::None);
        assert_eq!(model.key(key(KeyCode::Char('r'))), Action::None);
        assert!(!model.review && model.dirty());
        model.key(key(KeyCode::Char('q')));
        assert_eq!(model.key(key(KeyCode::Enter)), Action::Quit);
    }
    #[test]
    fn replayed_save_is_clean_even_when_its_permission_edit_denies_refresh() {
        let mut model = model();
        changed(&mut model);
        model.pending_command = Some("b".repeat(32));
        assert!(model.result(ConfigurationResult::Replayed {
            result_class: Some("configuration-restart-required".into()),
            revision: Some(3),
        }));
        model.online_lost("denied".into());
        assert!(!model.dirty());
        assert!(model.pending_command.is_none());
        assert_eq!(model.snapshot.version.revision, 3);
        assert!(model.snapshot.restart_required);
        assert_eq!(model.snapshot.config.caller.inactivity_minutes, 7);
    }
    #[test]
    fn control_c_from_help_or_confirmation_keeps_dirty_exit_deliberate() {
        let mut model = model();
        model.help = true;
        let interrupt = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(model.key(interrupt), Action::Quit);
        changed(&mut model);
        model.help = true;
        assert_eq!(model.key(interrupt), Action::None);
        assert!(!model.help && model.prompt == Some(Prompt::Quit));
        model.key(key(KeyCode::Esc));
        assert!(model.dirty());
    }
    #[test]
    fn offline_recovery_failure_never_claims_online_connection_loss() {
        let mut model = model();
        model.offline = true;
        changed(&mut model);
        model.online_lost("recovery required".into());
        assert!(model.offline && !model.disconnected && model.dirty());
    }
    #[test]
    fn invalid_candidate_cannot_enter_save_review() {
        let mut model = model();
        model.edits.push(ConfigurationEdit {
            field: ConfigurationField::NodeCount,
            value: "0".into(),
        });
        assert_eq!(model.key(key(KeyCode::Char('s'))), Action::None);
        assert!(!model.review);
        assert!(model.dirty());
    }
    #[test]
    fn every_section_help_and_resize_render_without_losing_selection() {
        sf_core::with_localizer(Localizer::embedded_en_us(), || {
            let mut model = model();
            changed(&mut model);
            for section in 0..SECTIONS.len() {
                model.section = section;
                for (width, height) in [(100, 30), (80, 24), (60, 20), (59, 19), (1, 1)] {
                    let backend = ratatui::backend::TestBackend::new(width, height);
                    let mut terminal = Terminal::new(backend).unwrap();
                    terminal.draw(|frame| render(frame, &model)).unwrap();
                    if width >= 80 && height >= 24 {
                        assert!(terminal.backend().to_string().contains("Unsaved changes"));
                    }
                    model.help = true;
                    terminal.draw(|frame| render(frame, &model)).unwrap();
                    model.help = false;
                    assert!(model.dirty());
                }
            }
        });
    }
    #[test]
    fn operator_grants_are_individually_toggled_and_staged() {
        let mut model = model();
        model.section = 5;
        model
            .operators
            .local_identities
            .push(LocalOperatorIdentity::Unix {
                uid: 7,
                label: None,
                capabilities: Cap::READ_ONLY.to_vec(),
            });
        model.selected = 1 + 14;
        model.key(key(KeyCode::Enter));
        assert!(grants(&mut model.operators.local_identities[0])
            .contains(&Cap::ChangeOnlineConfiguration));
        assert!(!grants(&mut model.operators.local_identities[0])
            .contains(&Cap::ChangeSensitiveConfiguration));
        assert!(model.dirty());
        assert!(model.snapshot.config.operators.local_identities.is_empty());
        model.key(key(KeyCode::Char('d')));
        assert!(model.operators.local_identities.is_empty());
    }
    #[test]
    fn secret_status_is_visible_without_any_key_value() {
        sf_core::with_localizer(Localizer::embedded_en_us(), || {
            let mut model = model();
            model.section = 4;
            let rows = model
                .rows()
                .iter()
                .map(|r| model.row_text(r))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(rows.contains("SSH private key 1: Configured"));
            assert!(!rows.contains("BEGIN OPENSSH"));
        });
    }
}
