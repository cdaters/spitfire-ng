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

//! Capability-gated SPITFIRE NG local operator monitor.

mod live_ui;
mod model;
mod ui;
mod worker;

use std::ffi::OsString;
use std::io::{self, IsTerminal, Stdout};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use model::{ConnectionState, MonitorModel, View};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use sf_core::{text, with_localizer, LocalizationArgs, Localizer, PRODUCT_VERSION};
use thiserror::Error;
use worker::{MonitorWorker, WorkerCommand, WorkerUpdate};

pub use model::{layout_mode, EventFilter, LayoutMode};

const INPUT_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Startup(String),
    #[error("{0}")]
    Terminal(#[from] io::Error),
}

impl MonitorError {
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Startup(_) => 3,
            Self::Terminal(_) => 4,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum LaunchAction {
    Help,
    Version,
    Monitor { board: PathBuf },
}

pub fn run_from_env() -> Result<(), MonitorError> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let action = parse_arguments(&arguments)?;
    with_localizer(Localizer::embedded_en_us(), || match action {
        LaunchAction::Help => {
            println!("{}", text("sfmonitor-usage", &LocalizationArgs::new()));
            Ok(())
        }
        LaunchAction::Version => {
            println!(
                "{}",
                text(
                    "sfmonitor-version",
                    &LocalizationArgs::new().with("version", PRODUCT_VERSION)
                )
            );
            Ok(())
        }
        LaunchAction::Monitor { board } => run_monitor(board),
    })
}

fn parse_arguments(arguments: &[OsString]) -> Result<LaunchAction, MonitorError> {
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("--help" | "-h")) {
        return Ok(LaunchAction::Help);
    }
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("--version" | "-V")) {
        return Ok(LaunchAction::Version);
    }
    let mut board = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].to_str() {
            Some("--board") => {
                index += 1;
                let value = arguments.get(index).ok_or_else(usage_error)?;
                if board.replace(PathBuf::from(value)).is_some() {
                    return Err(usage_error());
                }
            }
            Some("--locale") => {
                index += 1;
                let locale = arguments
                    .get(index)
                    .and_then(|value| value.to_str())
                    .ok_or_else(usage_error)?;
                if !locale.eq_ignore_ascii_case("en-US") {
                    return Err(MonitorError::Usage(text(
                        "sfmonitor-locale-unsupported",
                        &LocalizationArgs::new().with("locale", locale),
                    )));
                }
            }
            _ => return Err(usage_error()),
        }
        index += 1;
    }
    board
        .map(|board| LaunchAction::Monitor { board })
        .ok_or_else(usage_error)
}

fn usage_error() -> MonitorError {
    MonitorError::Usage(text("sfmonitor-usage", &LocalizationArgs::new()))
}

fn run_monitor(board: PathBuf) -> Result<(), MonitorError> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(MonitorError::Startup(text(
            "sfmonitor-terminal-required",
            &LocalizationArgs::new(),
        )));
    }
    let mut model = MonitorModel::default();
    let initial_query = model.filter.query(Utc::now().timestamp());
    let worker = MonitorWorker::start(board.clone(), initial_query);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
        let _ = disable_raw_mode();
        return Err(MonitorError::Terminal(error));
    }
    let mut restoration = TerminalRestoration::active();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => {
            restoration.restore();
            return Err(MonitorError::Terminal(error));
        }
    };
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        event_loop(&mut terminal, &worker, &mut model, &board, &mut restoration)
    }));
    drop(terminal);
    restoration.restore();
    worker.stop();
    match outcome {
        Ok(result) => result,
        Err(payload) => panic::resume_unwind(payload),
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    worker: &MonitorWorker,
    model: &mut MonitorModel,
    board: &std::path::Path,
    restoration: &mut TerminalRestoration,
) -> Result<(), MonitorError> {
    let mut dirty = true;
    loop {
        dirty |= apply_worker_updates(model, worker);
        if dirty {
            terminal.draw(|frame| ui::render(frame, model))?;
            dirty = false;
        }
        if !event::poll(INPUT_POLL)? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                match handle_key(model, worker, key) {
                    InputOutcome::Quit => return Ok(()),
                    InputOutcome::Configuration => {
                        restoration.restore();
                        let result = std::env::current_exe().and_then(|executable| {
                            let name = if cfg!(windows) {
                                "sfconfig.exe"
                            } else {
                                "sfconfig"
                            };
                            std::process::Command::new(executable.with_file_name(name))
                                .arg("--board")
                                .arg(board)
                                .status()
                        });
                        enable_raw_mode()?;
                        restoration.active = true;
                        execute!(terminal.backend_mut(), EnterAlternateScreen, Hide)?;
                        terminal.clear()?;
                        request_refresh(model, worker);
                        model.status_key = Some(if result.is_ok_and(|status| status.success()) {
                            "sfmonitor-configuration-returned"
                        } else {
                            "sfmonitor-configuration-handoff-error"
                        });
                    }
                    InputOutcome::Continue => {}
                }
                dirty = true;
            }
            Event::Resize(_, _) => dirty = true,
            _ => {}
        }
    }
}

fn apply_worker_updates(model: &mut MonitorModel, worker: &MonitorWorker) -> bool {
    let mut changed = false;
    if worker.take_transport_gap() {
        if model.live.chat.take().is_some() {
            worker.send(WorkerCommand::EndChat);
            model.action_result = Some(text(
                "sfmonitor-chat-connection-lost",
                &LocalizationArgs::new(),
            ));
        }
        model.event_gap = true;
        model.status_key = Some("sfmonitor-event-gap-short");
        changed = true;
    }
    for update in worker.drain_updates() {
        changed = true;
        match update {
            WorkerUpdate::ChatSendResult(accepted) => live_ui::apply_send_result(model, accepted),
            WorkerUpdate::Chat(frame) => {
                if live_ui::apply_chat(model, frame) {
                    request_refresh(model, worker);
                }
            }
            WorkerUpdate::ChatEnded(key) => {
                if model.live.chat.take().is_some() {
                    model.action_result = Some(text(key, &LocalizationArgs::new()));
                    request_refresh(model, worker);
                }
            }
            WorkerUpdate::Uncertain(command_id) => {
                model.live.uncertain = Some(command_id);
                model.action_result = Some(text(
                    "sfmonitor-command-uncertain",
                    &LocalizationArgs::new(),
                ));
            }
            WorkerUpdate::Connected {
                daemon_generation,
                features,
            } => {
                model.connection = ConnectionState::Connected {
                    daemon_generation,
                    features,
                };
                model.status_key = Some("sfmonitor-status-live");
            }
            WorkerUpdate::Snapshot(mut snapshot) => {
                snapshot
                    .events
                    .sort_by_key(|event| std::cmp::Reverse(event.event_id));
                model.snapshot = *snapshot;
                model.event_gap = false;
                model.status_key = Some("sfmonitor-status-live");
                model.clamp_selections();
            }
            WorkerUpdate::Events(batch) => {
                model.merge_live_events(
                    batch.events,
                    batch.gap_before_first,
                    Utc::now().timestamp(),
                );
            }
            WorkerUpdate::Disconnected { reason_key } => model.mark_disconnected(reason_key),
            WorkerUpdate::MutationDenied => {
                model.show_actions = false;
                model.snapshot.authorized_capabilities.clear();
                model.action_result =
                    Some(text("sfmonitor-action-denied", &LocalizationArgs::new()));
                request_refresh(model, worker);
            }
            WorkerUpdate::MutationResult(result) => {
                model.show_actions = false;
                model.action_result = Some(match result {
                    sf_bbs::MutationResult::Configuration(_) => live_ui::result_text("unsupported"),
                    sf_bbs::MutationResult::LiveControl { command_id, value } => {
                        let message = match &value {
                            sf_bbs::LiveControlResult::Pending { result_class } => {
                                live_ui::result_text(result_class)
                            }
                            _ => text("sfmonitor-action-ready", &LocalizationArgs::new()),
                        };
                        live_ui::apply_result(model, command_id, value);
                        message
                    }
                    sf_bbs::MutationResult::Completed { result_class, .. } => {
                        live_ui::result_text(&result_class)
                    }
                    sf_bbs::MutationResult::Replayed { result_class, .. } => text(
                        "sfmonitor-result-recovered",
                        &LocalizationArgs::new().with(
                            "result",
                            live_ui::result_text(result_class.as_deref().unwrap_or("accepted")),
                        ),
                    ),
                    sf_bbs::MutationResult::Accepted { .. } => live_ui::result_text("accepted"),
                    sf_bbs::MutationResult::Rejected { reason, .. } => {
                        live_ui::result_text(&reason)
                    }
                    sf_bbs::MutationResult::Receipt { receipt } => {
                        model.live.uncertain = None;
                        text(
                            "sfmonitor-result-recovered",
                            &LocalizationArgs::new().with(
                                "result",
                                live_ui::result_text(
                                    receipt.result_class.as_deref().unwrap_or(&receipt.state),
                                ),
                            ),
                        )
                    }
                    sf_bbs::MutationResult::Preflight { valid, .. } => {
                        if valid {
                            live_ui::result_text("preflight-ready")
                        } else {
                            live_ui::result_text("stale-target")
                        }
                    }
                });
                request_refresh(model, worker);
            }
        }
    }
    changed
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputOutcome {
    Configuration,
    Continue,
    Quit,
}

fn handle_key(model: &mut MonitorModel, worker: &MonitorWorker, key: KeyEvent) -> InputOutcome {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return InputOutcome::Quit;
    }
    if model.show_help {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1)) {
            model.show_help = false;
        }
        return InputOutcome::Continue;
    }
    if key.code == KeyCode::F(1) || (key.code == KeyCode::Char('?') && model.live.chat.is_none()) {
        model.show_help = true;
        return InputOutcome::Continue;
    }
    if live_ui::handle_key(model, worker, key) {
        return InputOutcome::Continue;
    }
    if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
        return InputOutcome::Quit;
    }
    if model.show_actions {
        match key.code {
            KeyCode::Esc => model.show_actions = false,
            KeyCode::Char('a') | KeyCode::Char('A')
                if model.view == View::Notifications
                    && model
                        .action_unavailable(
                            sf_bbs::OperatorFeature::NotificationAcknowledgement,
                            sf_core::LocalOperatorCapability::AcknowledgeNotifications,
                        )
                        .is_none() =>
            {
                if let Some(notification) = model
                    .snapshot
                    .notifications
                    .get(model.selected_notification)
                {
                    let _ = worker.send(WorkerCommand::AcknowledgeNotification {
                        command_id: new_command_id(),
                        notification_id: notification.notification_id,
                        expected_version: notification.state_version,
                    });
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=')
                if model.view == View::Nodes
                    && model
                        .action_unavailable(
                            sf_bbs::OperatorFeature::SessionTimeAdjustment,
                            sf_core::LocalOperatorCapability::AdjustSessionTime,
                        )
                        .is_none() =>
            {
                if let Some(node) = model.snapshot.nodes.get(model.selected_node) {
                    if let (Some(session_id), Some(occupancy_generation)) =
                        (node.session_id, node.occupancy_generation)
                    {
                        let _ = worker.send(WorkerCommand::AdjustSessionTime {
                            command_id: new_command_id(),
                            node_id: node.node_id,
                            session_id,
                            occupancy_generation,
                            delta_minutes: 5,
                        });
                    }
                }
            }
            KeyCode::Char('-')
                if model.view == View::Nodes
                    && model
                        .action_unavailable(
                            sf_bbs::OperatorFeature::SessionTimeAdjustment,
                            sf_core::LocalOperatorCapability::AdjustSessionTime,
                        )
                        .is_none() =>
            {
                if let Some(node) = model.snapshot.nodes.get(model.selected_node) {
                    if let (Some(session_id), Some(occupancy_generation)) =
                        (node.session_id, node.occupancy_generation)
                    {
                        let _ = worker.send(WorkerCommand::AdjustSessionTime {
                            command_id: new_command_id(),
                            node_id: node.node_id,
                            session_id,
                            occupancy_generation,
                            delta_minutes: -5,
                        });
                    }
                }
            }
            _ => {}
        }
        return InputOutcome::Continue;
    }
    if model.show_filters {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                model.show_filters = false;
                request_refresh(model, worker);
            }
            KeyCode::Char('c') | KeyCode::Char('C') => model.filter.cycle_category(),
            KeyCode::Char('s') | KeyCode::Char('S') => model.filter.cycle_severity(),
            KeyCode::Char('o') | KeyCode::Char('O') => model.filter.cycle_outcome(),
            KeyCode::Char('t') | KeyCode::Char('T') => model.filter.cycle_time(),
            KeyCode::Char('n') | KeyCode::Char('N') => cycle_node_filter(model),
            KeyCode::Char('x') | KeyCode::Char('X') => model.filter.clear(),
            _ => {}
        }
        return InputOutcome::Continue;
    }
    match key.code {
        KeyCode::Tab | KeyCode::Right => model.next_view(),
        KeyCode::BackTab | KeyCode::Left => model.previous_view(),
        KeyCode::Up => model.move_selection(-1),
        KeyCode::Down => model.move_selection(1),
        KeyCode::PageUp => model.move_selection(-10),
        KeyCode::PageDown => model.move_selection(10),
        KeyCode::Home => model.move_selection(isize::MIN),
        KeyCode::End => model.move_selection(isize::MAX),
        KeyCode::Enter if model.view == View::Nodes => {
            model.show_node_detail = !model.show_node_detail;
        }
        KeyCode::Enter if model.view == View::SystemConfiguration => {
            return InputOutcome::Configuration;
        }
        KeyCode::Esc if model.show_node_detail => model.show_node_detail = false,
        KeyCode::Char('?') | KeyCode::F(1) => model.show_help = true,
        KeyCode::F(2) => model.select_view(View::Dashboard),
        KeyCode::F(3) => model.select_view(View::Nodes),
        KeyCode::F(4) => model.select_view(View::Activity),
        KeyCode::Char('/') if model.view == View::Activity => model.show_filters = true,
        KeyCode::Char('a') | KeyCode::Char('A')
            if matches!(
                model.view,
                View::Dashboard | View::Nodes | View::Notifications
            ) =>
        {
            model.show_actions = true
        }
        KeyCode::Char('r') | KeyCode::Char('R') => request_refresh(model, worker),
        _ => {}
    }
    InputOutcome::Continue
}

fn new_command_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    format!(
        "{}-{:032x}",
        "sfmonitor",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    )
}

fn request_refresh(model: &mut MonitorModel, worker: &MonitorWorker) {
    let query = model.filter.query(Utc::now().timestamp());
    let reconnecting = matches!(model.connection, ConnectionState::Disconnected { .. });
    let command = if reconnecting {
        WorkerCommand::Reconnect(query)
    } else {
        WorkerCommand::Refresh(query)
    };
    if worker.send(command) {
        if reconnecting {
            model.connection = ConnectionState::Connecting;
        }
        model.status_key = Some("sfmonitor-status-refreshing");
    } else {
        model.status_key = Some("sfmonitor-status-busy");
    }
}

fn cycle_node_filter(model: &mut MonitorModel) {
    let mut node_ids = model
        .snapshot
        .nodes
        .iter()
        .map(|node| node.node_id)
        .collect::<Vec<_>>();
    node_ids.sort_unstable();
    node_ids.dedup();
    model.filter.node_id = match model.filter.node_id {
        None => node_ids.first().copied(),
        Some(current) => node_ids
            .iter()
            .position(|node| *node == current)
            .and_then(|position| node_ids.get(position + 1).copied()),
    };
}

struct TerminalRestoration {
    active: bool,
}

impl TerminalRestoration {
    const fn active() -> Self {
        Self { active: true }
    }

    fn restore(&mut self) {
        if !self.active {
            return;
        }
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
        self.active = false;
    }
}

impl Drop for TerminalRestoration {
    fn drop(&mut self) {
        self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_bbs::{EventWire, OperatorEventQuery};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn command_line_requires_an_explicit_board() {
        assert!(parse_arguments(&[]).is_err());
        assert_eq!(
            parse_arguments(&[OsString::from("--board"), OsString::from("board.toml")]).unwrap(),
            LaunchAction::Monitor {
                board: PathBuf::from("board.toml")
            }
        );
        assert!(parse_arguments(&[
            OsString::from("--board"),
            OsString::from("one.toml"),
            OsString::from("--board"),
            OsString::from("two.toml")
        ])
        .is_err());
    }

    #[test]
    fn configuration_doorway_hands_off_without_sending_a_daemon_command() {
        let (worker, commands) = MonitorWorker::test_channels();
        let mut model = MonitorModel {
            view: View::SystemConfiguration,
            ..MonitorModel::default()
        };
        assert!(matches!(
            handle_key(
                &mut model,
                &worker,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            InputOutcome::Configuration
        ));
        assert!(commands.try_recv().is_err());
        assert!(matches!(
            handle_key(
                &mut model,
                &worker,
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)
            ),
            InputOutcome::Quit
        ));
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn dashboard_shutdown_requires_permission_preflight_confirmation_and_preserves_q() {
        let (worker, commands) = MonitorWorker::test_channels();
        let mut model = MonitorModel {
            connection: ConnectionState::Connected {
                daemon_generation: "generation".to_owned(),
                features: vec![sf_bbs::OperatorFeature::GracefulShutdown],
            },
            ..MonitorModel::default()
        };
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        handle_key(&mut model, &worker, key(KeyCode::Char('a')));
        handle_key(&mut model, &worker, key(KeyCode::Char('s')));
        assert!(commands.try_recv().is_err());
        model
            .snapshot
            .authorized_capabilities
            .push(sf_core::LocalOperatorCapability::RequestGracefulShutdown);
        handle_key(&mut model, &worker, key(KeyCode::Char('s')));
        let WorkerCommand::LiveControl {
            command_id,
            action: sf_bbs::LiveControlAction::PrepareGracefulShutdown { .. },
        } = commands.try_recv().unwrap()
        else {
            panic!("preflight expected")
        };
        let impact = sf_bbs::ShutdownImpact {
            daemon_generation: "generation".to_owned(),
            active_callers: 2,
            active_transfers: 1,
            active_chats: 1,
            interactions: 1,
            phase: sf_bbs::ShutdownPhase::Running,
            token: "opaque-token".to_owned(),
        };
        live_ui::apply_result(
            &mut model,
            command_id.clone(),
            sf_bbs::LiveControlResult::ShutdownPreflight {
                impact: impact.clone(),
            },
        );
        handle_key(&mut model, &worker, key(KeyCode::Esc));
        assert!(commands.try_recv().is_err());
        assert!(model.live.shutdown_confirmation.is_none());
        live_ui::apply_result(
            &mut model,
            command_id.clone(),
            sf_bbs::LiveControlResult::ShutdownPreflight { impact },
        );
        handle_key(&mut model, &worker, key(KeyCode::Enter));
        assert!(
            matches!(commands.try_recv().unwrap(), WorkerCommand::LiveControl { command_id: id, action: sf_bbs::LiveControlAction::RequestGracefulShutdown { .. } } if id == command_id)
        );
        assert_eq!(
            handle_key(&mut model, &worker, key(KeyCode::Char('q'))),
            InputOutcome::Quit
        );
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn action_keys_require_both_feature_support_and_explicit_authorization() {
        let (worker, commands) = MonitorWorker::test_channels();
        let mut model = MonitorModel {
            view: View::Notifications,
            show_actions: true,
            ..MonitorModel::default()
        };
        model.connection = ConnectionState::Connected {
            daemon_generation: "test".to_owned(),
            features: model::MONITOR_FEATURES.to_vec(),
        };
        model.snapshot.notifications.push(sf_bbs::NotificationWire {
            notification_id: 7,
            source_event_id: 1,
            created_at: 1,
            category: "error".to_owned(),
            severity: "error".to_owned(),
            reason_key: "operator-notification-operational-error".to_owned(),
            remediation_key: None,
            state: "open".to_owned(),
            state_version: 1,
        });
        let feature = sf_bbs::OperatorFeature::NotificationAcknowledgement;
        let capability = sf_core::LocalOperatorCapability::AcknowledgeNotifications;
        model.snapshot.authorized_capabilities = vec![capability];
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(
            model.action_unavailable(feature, capability),
            Some("operator-feature-unsupported")
        );
        handle_key(&mut model, &worker, key);
        assert!(commands.try_recv().is_err());
        if let ConnectionState::Connected { features, .. } = &mut model.connection {
            features.push(feature);
        }
        model.snapshot.authorized_capabilities =
            sf_core::LocalOperatorCapability::READ_ONLY.to_vec();
        assert_eq!(
            model.action_unavailable(feature, capability),
            Some("sfmonitor-action-denied")
        );
        handle_key(&mut model, &worker, key);
        assert!(commands.try_recv().is_err());
        model.snapshot.authorized_capabilities.push(capability);
        handle_key(&mut model, &worker, key);
        assert!(matches!(
            commands.try_recv().unwrap(),
            WorkerCommand::AcknowledgeNotification {
                notification_id: 7,
                expected_version: 1,
                ..
            }
        ));
        model.snapshot.authorized_capabilities =
            sf_core::LocalOperatorCapability::READ_ONLY.to_vec();
        handle_key(&mut model, &worker, key);
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn command_line_has_safe_help_version_and_locale() {
        assert_eq!(
            parse_arguments(&[OsString::from("--help")]).unwrap(),
            LaunchAction::Help
        );
        assert_eq!(
            parse_arguments(&[OsString::from("--version")]).unwrap(),
            LaunchAction::Version
        );
        assert!(parse_arguments(&[
            OsString::from("--locale"),
            OsString::from("fr-FR"),
            OsString::from("--board"),
            OsString::from("board.toml")
        ])
        .is_err());
    }

    #[test]
    fn only_press_and_repeat_are_processed_by_the_loop_contract() {
        assert!(matches!(
            KeyEventKind::Press,
            KeyEventKind::Press | KeyEventKind::Repeat
        ));
        assert!(!matches!(
            KeyEventKind::Release,
            KeyEventKind::Press | KeyEventKind::Repeat
        ));
    }

    #[test]
    fn terminal_restoration_is_idempotent_without_activating_a_terminal() {
        let mut restoration = TerminalRestoration { active: false };
        restoration.restore();
        restoration.restore();
        assert!(!restoration.active);
    }

    #[test]
    fn host_shutdown_restart_and_configuration_remain_outside_monitor_protocol() {
        for forbidden in [
            "shutdown",
            "daemon-shutdown",
            "request-shutdown",
            "configuration-write",
            "host-shutdown",
            "restart",
        ] {
            assert!(
                serde_json::from_value::<sf_bbs::OperatorFeature>(serde_json::json!(forbidden))
                    .is_err()
            );
            assert!(
                serde_json::from_value::<sf_core::LocalOperatorCapability>(serde_json::json!(
                    forbidden
                ))
                .is_err()
            );
            assert!(serde_json::from_value::<sf_bbs::LiveControlAction>(
                serde_json::json!({"action": forbidden})
            )
            .is_err());
        }
        let _ = WorkerCommand::Refresh(OperatorEventQuery::default());
    }

    #[test]
    fn panic_on_a_client_thread_does_not_escape_that_process() {
        let daemon_unaffected = Arc::new(Mutex::new(true));
        let proof = Arc::clone(&daemon_unaffected);
        let handle = thread::spawn(move || {
            let _: Result<(), _> = panic::catch_unwind(|| panic!("client-only panic")).map(|_| ());
            *proof.lock().unwrap() = true;
        });
        handle.join().unwrap();
        assert!(*daemon_unaffected.lock().unwrap());
    }

    #[test]
    fn snapshot_refresh_replaces_stale_data_and_gap_state() {
        let mut model = MonitorModel {
            event_gap: true,
            ..MonitorModel::default()
        };
        model.mark_disconnected("operator-daemon-restarted");
        let (tx, rx) = mpsc::sync_channel(1);
        tx.send(WorkerUpdate::Snapshot(Box::default())).unwrap();
        drop(tx);
        for update in rx {
            if let WorkerUpdate::Snapshot(snapshot) = update {
                model.snapshot = *snapshot;
                model.event_gap = false;
            }
        }
        assert!(!model.event_gap);
    }

    #[test]
    fn source_event_payload_is_not_needed_by_input_or_navigation() {
        fn assert_send<T: Send>() {}
        assert_send::<EventWire>();
    }

    #[test]
    fn every_static_sfmonitor_key_exists_in_the_embedded_catalog() {
        let keys = sf_core::embedded_catalog_keys().unwrap();
        for source in [
            include_str!("lib.rs"),
            include_str!("model.rs"),
            include_str!("ui.rs"),
            include_str!("worker.rs"),
            include_str!("live_ui.rs"),
        ] {
            for candidate in source.split('"').skip(1).step_by(2) {
                if candidate.starts_with("sfmonitor-") && !candidate.contains('{') {
                    assert!(keys.contains(candidate), "missing catalog key {candidate}");
                }
            }
        }
    }
}
