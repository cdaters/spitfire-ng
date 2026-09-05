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

use crate::{
    model::{ConnectionState, MonitorModel, View},
    worker::{MonitorWorker, WorkerCommand},
};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use sf_bbs::{
    DisconnectPreflight, LiveControlAction as Action, LiveControlResult, LiveSessionTarget,
    OperatorFeature,
};
use sf_core::{LocalOperatorCapability as Capability, LocalizationArgs};
use std::collections::VecDeque;

#[derive(Clone, Default, Debug)]
pub struct LiveUi {
    pub page_menu: bool,
    pub disconnect_choice: bool,
    pub confirmation: Option<(String, DisconnectPreflight)>,
    pub shutdown_confirmation: Option<(String, sf_bbs::ShutdownImpact)>,
    pub chat: Option<ChatPane>,
    pub uncertain: Option<String>,
}

#[derive(Clone)]
pub struct ChatPane {
    target: LiveSessionTarget,
    handle: String,
    state: String,
    lines: VecDeque<String>,
    input: String,
    pending: Option<String>,
}

impl std::fmt::Debug for ChatPane {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChatPane")
            .field("node", &self.target.node_id)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

fn text(key: &str) -> String {
    sf_core::text(key, &LocalizationArgs::new())
}

fn target(model: &MonitorModel) -> Option<LiveSessionTarget> {
    let ConnectionState::Connected {
        daemon_generation, ..
    } = &model.connection
    else {
        return None;
    };
    let node = model.snapshot.nodes.get(model.selected_node)?;
    Some(LiveSessionTarget {
        daemon_generation: daemon_generation.clone(),
        node_id: node.node_id,
        session_id: node.session_id?,
        occupancy_generation: node.occupancy_generation?,
    })
}

fn send(model: &mut MonitorModel, worker: &MonitorWorker, command_id: String, action: Action) {
    if let Some(reason) = model.action_unavailable(action.feature(), action.capability()) {
        model.action_result = Some(text(reason));
        model.live.page_menu = false;
        model.live.disconnect_choice = false;
        return;
    }
    if matches!(action, Action::AnswerCallerPage { .. })
        && model
            .action_unavailable(OperatorFeature::CallerChat, Capability::ChatWithCaller)
            .is_some()
    {
        model.action_result = Some(text("sfmonitor-action-denied"));
        return;
    }
    if worker.send(WorkerCommand::LiveControl { command_id, action }) {
        model.show_actions = false;
        model.live.page_menu = false;
        model.live.disconnect_choice = false;
    }
}

pub fn apply_result(model: &mut MonitorModel, command_id: String, result: LiveControlResult) {
    model.show_actions = false;
    match result {
        LiveControlResult::ShutdownPreflight { impact } => {
            model.live.shutdown_confirmation = Some((command_id, impact));
        }
        LiveControlResult::DisconnectPreflight { impact } => {
            model.live.confirmation = Some((command_id, impact))
        }
        LiveControlResult::ChatReady {
            target, invited, ..
        } => {
            let handle = model
                .snapshot
                .nodes
                .iter()
                .find(|node| {
                    node.node_id == target.node_id && node.session_id == Some(target.session_id)
                })
                .and_then(|node| node.public_handle.clone())
                .unwrap_or_default();
            model.live.chat = Some(ChatPane {
                target,
                handle,
                state: if invited {
                    "chat-invited"
                } else {
                    "chat-started"
                }
                .to_owned(),
                lines: VecDeque::new(),
                input: String::new(),
                pending: None,
            });
        }
        LiveControlResult::Pending { result_class } => {
            model.action_result = Some(result_text(&result_class))
        }
    }
}

pub fn apply_chat(model: &mut MonitorModel, frame: sf_bbs::ChatServerFrame) -> bool {
    let changed = model
        .live
        .chat
        .as_ref()
        .is_some_and(|chat| chat.state != frame.state);
    if !matches!(
        frame.state.as_str(),
        "chat-invited" | "chat-started" | "chat-busy"
    ) {
        model.live.chat = None;
        model.action_result = Some(result_text(&frame.state));
        return true;
    }
    if let Some(chat) = model.live.chat.as_mut() {
        chat.state = frame.state;
        for line in frame.lines {
            if chat.lines.len() == 100 {
                chat.lines.pop_front();
            }
            chat.lines.push_back(line);
        }
    }
    changed
}

pub fn result_text(class: &str) -> String {
    text(&format!("sfmonitor-result-{class}"))
}

pub fn apply_send_result(model: &mut MonitorModel, accepted: bool) {
    if let Some(chat) = model.live.chat.as_mut() {
        if let Some(line) = chat.pending.take() {
            if accepted {
                if chat.lines.len() == 100 {
                    chat.lines.pop_front();
                }
                chat.lines.push_back(line);
            } else {
                chat.input = line;
            }
        }
    }
}

/// Returns true only when a B2 focused control consumed the key. Esc drops
/// the active chat buffer; Q outside chat still quits this monitor only.
pub fn handle_key(model: &mut MonitorModel, worker: &MonitorWorker, key: KeyEvent) -> bool {
    if model.live.shutdown_confirmation.is_some() {
        match key.code {
            KeyCode::Esc => model.live.shutdown_confirmation = None,
            KeyCode::Enter => {
                if let Some((id, impact)) = model.live.shutdown_confirmation.take() {
                    send(
                        model,
                        worker,
                        id,
                        Action::RequestGracefulShutdown {
                            daemon_generation: impact.daemon_generation,
                            preflight_token: impact.token,
                        },
                    );
                }
            }
            KeyCode::Char('q' | 'Q') => return false,
            _ => {}
        }
        return true;
    }
    if model.show_actions
        && model.view == View::Dashboard
        && matches!(key.code, KeyCode::Char('s' | 'S'))
    {
        if let ConnectionState::Connected {
            daemon_generation, ..
        } = &model.connection
        {
            send(
                model,
                worker,
                crate::new_command_id(),
                Action::PrepareGracefulShutdown {
                    daemon_generation: daemon_generation.clone(),
                },
            );
        } else {
            model.action_result = Some(text("sfmonitor-action-stale"));
        }
        return true;
    }
    if let Some(chat) = model.live.chat.as_mut() {
        match key.code {
            KeyCode::Esc => {
                worker.send(WorkerCommand::EndChat);
                model.live.chat = None;
            }
            KeyCode::Enter
                if chat.state == "chat-started"
                    && !chat.input.is_empty()
                    && chat.pending.is_none() =>
            {
                if worker.send(WorkerCommand::ChatLine(chat.input.clone())) {
                    chat.pending = Some(std::mem::take(&mut chat.input));
                }
            }
            KeyCode::Backspace if chat.pending.is_none() => {
                chat.input.pop();
            }
            KeyCode::Char(ch)
                if !ch.is_control()
                    && chat.pending.is_none()
                    && chat.input.len() + ch.len_utf8() <= sf_core::MAX_CHAT_LINE_BYTES =>
            {
                chat.input.push(ch)
            }
            _ => {}
        }
        return true;
    }
    if model.live.confirmation.is_some() {
        match key.code {
            KeyCode::Esc => model.live.confirmation = None,
            KeyCode::Enter => {
                if let Some((id, impact)) = model.live.confirmation.take() {
                    send(
                        model,
                        worker,
                        id,
                        Action::DisconnectSession {
                            target: impact.target,
                            notice: impact.notice,
                            preflight_token: impact.token,
                        },
                    );
                }
            }
            _ => {}
        }
        return true;
    }
    if model.live.disconnect_choice {
        match key.code {
            KeyCode::Esc => model.live.disconnect_choice = false,
            KeyCode::Char('1' | '2') => {
                if let Some(target) = target(model) {
                    send(
                        model,
                        worker,
                        crate::new_command_id(),
                        Action::PrepareDisconnect {
                            target,
                            notice: key.code == KeyCode::Char('1'),
                        },
                    );
                } else {
                    model.live.disconnect_choice = false;
                    model.live.page_menu = false;
                    model.action_result = Some(text("sfmonitor-action-stale"));
                }
            }
            _ => {}
        }
        return true;
    }
    if model.live.page_menu {
        match key.code {
            KeyCode::Esc => model.live.page_menu = false,
            KeyCode::Char('o' | 'O') => {
                let available = model
                    .snapshot
                    .interactions
                    .as_ref()
                    .is_none_or(|snapshot| !snapshot.available);
                send(
                    model,
                    worker,
                    crate::new_command_id(),
                    Action::SetPageAvailability { available },
                );
            }
            KeyCode::Char('i' | 'I') => {
                if let Some(target) = target(model) {
                    send(
                        model,
                        worker,
                        crate::new_command_id(),
                        Action::InviteOperatorChat { target },
                    );
                }
            }
            KeyCode::Char('a' | 'A' | 'd' | 'D') => {
                if let Some(target) = target(model) {
                    let page = model.snapshot.interactions.as_ref().and_then(|snapshot| {
                        snapshot
                            .pages
                            .iter()
                            .find(|page| page.target == target && page.state == "pending")
                    });
                    if let Some(page) = page {
                        let action = if matches!(key.code, KeyCode::Char('a' | 'A')) {
                            Action::AnswerCallerPage {
                                target,
                                interaction_id: page.interaction_id,
                            }
                        } else {
                            Action::DeclineCallerPage {
                                target,
                                interaction_id: page.interaction_id,
                            }
                        };
                        send(model, worker, crate::new_command_id(), action);
                    } else {
                        model.action_result = Some(text("sfmonitor-page-none"));
                    }
                }
            }
            _ => {}
        }
        return true;
    }
    if model.show_actions && model.view == View::Nodes {
        match key.code {
            KeyCode::Char('p' | 'P') => {
                model.show_actions = false;
                model.live.page_menu = true;
                return true;
            }
            KeyCode::Char('d' | 'D') => {
                if let Some(reason) = model.action_unavailable(
                    OperatorFeature::SessionDisconnect,
                    Capability::DisconnectSession,
                ) {
                    model.action_result = Some(text(reason));
                } else {
                    model.show_actions = false;
                    model.live.disconnect_choice = true;
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

pub fn render(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let popup = Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(4),
    };
    if let Some(chat) = &model.live.chat {
        frame.render_widget(Clear, popup);
        let border = Block::default()
            .borders(Borders::ALL)
            .title(text("sfmonitor-page-chat"));
        let inner = border.inner(popup);
        frame.render_widget(border, popup);
        let regions = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
                Constraint::Length(2),
            ])
            .split(inner);
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(format!("{} — {}", chat.target.node_id, chat.handle)),
                Line::raw(result_text(&chat.state)),
            ])
            .wrap(Wrap { trim: false }),
            regions[0],
        );
        let mut rows = Vec::new();
        for line in &chat.lines {
            rows.extend(wrapped_rows(line, regions[1].width));
        }
        let skip = rows.len().saturating_sub(usize::from(regions[1].height));
        frame.render_widget(
            Paragraph::new(
                rows.into_iter()
                    .skip(skip)
                    .map(Line::raw)
                    .collect::<Vec<_>>(),
            ),
            regions[1],
        );
        let input = wrapped_rows(
            &format!("> {}", chat.pending.as_deref().unwrap_or(&chat.input)),
            regions[2].width,
        );
        let skip = input.len().saturating_sub(usize::from(regions[2].height));
        frame.render_widget(
            Paragraph::new(
                input
                    .into_iter()
                    .skip(skip)
                    .map(Line::raw)
                    .collect::<Vec<_>>(),
            ),
            regions[2],
        );
        frame.render_widget(
            Paragraph::new(text("sfmonitor-chat-keys")).wrap(Wrap { trim: false }),
            regions[3],
        );
        return;
    }
    let mut lines = Vec::new();
    let title;
    if let Some((_, impact)) = &model.live.shutdown_confirmation {
        title = text("sfmonitor-shutdown");
        lines.push(Line::raw(sf_core::text(
            "sfmonitor-shutdown-impact",
            &LocalizationArgs::new()
                .with("callers", impact.active_callers.to_string())
                .with("transfers", impact.active_transfers.to_string())
                .with("chats", impact.active_chats.to_string())
                .with("interactions", impact.interactions.to_string()),
        )));
        lines.push(Line::raw(text("sfmonitor-shutdown-warning")));
        lines.push(Line::raw(text("sfmonitor-shutdown-confirm")));
        lines.push(Line::raw(text("sfmonitor-disconnect-confirm-keys")));
    } else if let Some((_, impact)) = &model.live.confirmation {
        title = text("sfmonitor-disconnect-confirm");
        lines.push(Line::raw(format!(
            "{} — {}",
            impact.target.node_id, impact.public_handle
        )));
        lines.push(Line::raw(text(if impact.notice {
            "sfmonitor-disconnect-notice"
        } else {
            "sfmonitor-disconnect-no-notice"
        })));
        if impact.transfer_active {
            lines.push(Line::raw(text("sfmonitor-disconnect-transfer-warning")));
        }
        if impact.interaction_active {
            lines.push(Line::raw(text("sfmonitor-disconnect-chat-warning")));
        }
        lines.push(Line::raw(text("sfmonitor-disconnect-confirm-keys")));
    } else if model.live.disconnect_choice {
        title = text("sfmonitor-disconnect");
        lines.push(Line::raw(text("sfmonitor-disconnect-choices")));
    } else if model.live.page_menu {
        title = text("sfmonitor-page-chat");
        if let Some(node) = model.snapshot.nodes.get(model.selected_node) {
            lines.push(Line::raw(format!(
                "{} — {}",
                node.node_id,
                node.public_handle.as_deref().unwrap_or("—")
            )));
        }
        if target(model).is_some_and(|target| {
            model
                .snapshot
                .interactions
                .as_ref()
                .is_some_and(|snapshot| {
                    snapshot
                        .pages
                        .iter()
                        .any(|page| page.target == target && page.state == "pending")
                })
        }) {
            lines.push(Line::raw(text("sfmonitor-page-pending")));
        }
        lines.push(Line::raw(text("sfmonitor-page-chat-choices")));
        if let Some(snapshot) = &model.snapshot.interactions {
            lines.push(Line::raw(text(if snapshot.available {
                "sfmonitor-page-available"
            } else {
                "sfmonitor-page-unavailable"
            })));
        }
        for (label, feature, capability) in [
            (
                "sfmonitor-page-availability-action",
                OperatorFeature::PageAvailability,
                Capability::ManagePageAvailability,
            ),
            (
                "sfmonitor-page-manage-action",
                OperatorFeature::CallerPages,
                Capability::ManageCallerPages,
            ),
            (
                "sfmonitor-chat-invite-action",
                OperatorFeature::CallerChat,
                Capability::ChatWithCaller,
            ),
        ] {
            if let Some(reason) = model.action_unavailable(feature, capability) {
                lines.push(Line::raw(format!("{} — {}", text(label), text(reason))));
            }
        }
    } else {
        return;
    }
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn wrapped_rows(text: &str, width: u16) -> Vec<String> {
    let width = usize::from(width).max(1);
    let mut rows = vec![String::new()];
    let mut used = 0;
    for ch in text.chars() {
        let cells = Line::raw(ch.to_string()).width();
        if used + cells > width && used > 0 {
            rows.push(String::new());
            used = 0;
        }
        rows.last_mut().expect("one row").push(ch);
        used += cells;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active_model() -> MonitorModel {
        let mut model = MonitorModel::default();
        apply_result(
            &mut model,
            "test-chat-command-001".to_owned(),
            LiveControlResult::ChatReady {
                join_token: "opaque".to_owned(),
                target: LiveSessionTarget {
                    daemon_generation: "test".to_owned(),
                    node_id: 1,
                    session_id: 2,
                    occupancy_generation: 3,
                },
                invited: true,
            },
        );
        model
    }
    #[test]
    fn vanished_disconnect_target_closes_choice_and_exposes_refresh_guidance() {
        let (worker, commands) = MonitorWorker::test_channels();
        let mut model = MonitorModel::default();
        model.live.disconnect_choice = true;
        assert!(handle_key(
            &mut model,
            &worker,
            KeyEvent::new(KeyCode::Char('2'), crossterm::event::KeyModifiers::NONE)
        ));
        assert!(!model.live.disconnect_choice);
        assert!(model.action_result.is_some());
        assert!(commands.try_recv().is_err());
    }
    #[test]
    fn ephemeral_transcript_is_bounded_redacted_and_discarded_on_every_end() {
        for state in [
            "chat-ended",
            "caller-gone",
            "authorization-denied",
            "chat-timeout",
            "chat-declined",
        ] {
            let mut model = active_model();
            apply_chat(
                &mut model,
                sf_bbs::ChatServerFrame {
                    sequence: 1,
                    state: "chat-started".to_owned(),
                    lines: vec!["ephemeral regression payload".to_owned(); 120],
                },
            );
            assert_eq!(model.live.chat.as_ref().unwrap().lines.len(), 100);
            assert!(!format!("{:?}", model.live).contains("ephemeral regression payload"));
            apply_chat(
                &mut model,
                sf_bbs::ChatServerFrame {
                    sequence: 2,
                    state: state.to_owned(),
                    lines: vec![],
                },
            );
            assert!(model.live.chat.is_none());
        }
    }
    #[test]
    fn long_chat_keeps_input_and_safe_exit_visible() {
        let mut model = active_model();
        let chat = model.live.chat.as_mut().unwrap();
        chat.lines = vec!["界".repeat(170); 100].into();
        chat.input = "INPUT REMAINS VISIBLE".to_owned();
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &model))
            .unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(screen.contains("INPUT REMAINS VISIBLE"));
        assert!(screen.contains("Esc End chat"));
        assert!(wrapped_rows("界界", 2)
            .iter()
            .all(|row| Line::raw(row.clone()).width() <= 2));
    }

    #[test]
    fn rejected_chat_send_is_restored_without_false_transcript_delivery() {
        let mut model = active_model();
        model.live.chat.as_mut().unwrap().pending = Some("synthetic line".to_owned());
        apply_send_result(&mut model, false);
        let chat = model.live.chat.as_mut().unwrap();
        assert_eq!(chat.input, "synthetic line");
        assert!(chat.lines.is_empty());
        chat.pending = Some(std::mem::take(&mut chat.input));
        apply_send_result(&mut model, true);
        assert_eq!(model.live.chat.as_ref().unwrap().lines.len(), 1);
    }
}
