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

use chrono::{DateTime, Utc};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use sf_bbs::{EventWire, NotificationWire, RecentCallerWire};
use sf_core::{text, LocalizationArgs};

use crate::model::{layout_mode, ConnectionState, LayoutMode, MonitorModel, View};

const BRAND: Color = Color::Red;
const ACCENT: Color = Color::Yellow;
const INFORMATION: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;

pub fn render(frame: &mut Frame<'_>, model: &MonitorModel) {
    let area = frame.area();
    if layout_mode(area.width, area.height) == LayoutMode::MinimumNotice {
        render_minimum_notice(frame, area);
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area);
    render_header(frame, rows[0], model);
    if layout_mode(area.width, area.height) == LayoutMode::Wide {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(23), Constraint::Min(40)])
            .split(rows[1]);
        render_navigation(frame, columns[0], model);
        render_view(frame, columns[1], model, true);
    } else {
        render_view(frame, rows[1], model, false);
    }
    render_footer(frame, rows[2], model);
    if model.show_help {
        render_help(frame, area, model);
    } else if model.show_filters {
        render_filters(frame, area, model);
    } else if model.show_actions {
        render_actions(frame, area, model);
    }
    if !model.show_help {
        crate::live_ui::render(frame, area, model);
    }
}

fn render_actions(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let mut lines = Vec::new();
    match model.view {
        View::Dashboard => lines.push(Line::raw(text(
            "sfmonitor-action-shutdown",
            &LocalizationArgs::new(),
        ))),
        View::Notifications => lines.push(Line::raw(text(
            "sfmonitor-action-acknowledge",
            &LocalizationArgs::new(),
        ))),
        View::Nodes => {
            lines.push(Line::raw(text(
                "sfmonitor-action-page-chat",
                &LocalizationArgs::new(),
            )));
            lines.push(Line::raw(text(
                "sfmonitor-action-disconnect",
                &LocalizationArgs::new(),
            )));
            lines.push(Line::raw(text(
                "sfmonitor-action-adjust-time-add",
                &LocalizationArgs::new(),
            )));
            lines.push(Line::raw(text(
                "sfmonitor-action-adjust-time-remove",
                &LocalizationArgs::new(),
            )));
        }
        _ => lines.push(Line::raw("No actions are available in this view.")),
    }
    let unavailable = match model.view {
        View::Dashboard => model.action_unavailable(
            sf_bbs::OperatorFeature::GracefulShutdown,
            sf_core::LocalOperatorCapability::RequestGracefulShutdown,
        ),
        View::Nodes => model.action_unavailable(
            sf_bbs::OperatorFeature::SessionTimeAdjustment,
            sf_core::LocalOperatorCapability::AdjustSessionTime,
        ),
        View::Notifications => model.action_unavailable(
            sf_bbs::OperatorFeature::NotificationAcknowledgement,
            sf_core::LocalOperatorCapability::AcknowledgeNotifications,
        ),
        _ => None,
    };
    if let Some(reason) = unavailable {
        lines.push(Line::raw(text(reason, &LocalizationArgs::new())));
    }
    lines.push(Line::raw(text(
        "sfmonitor-action-cancel",
        &LocalizationArgs::new(),
    )));
    let popup = Rect {
        x: area.width / 4,
        y: area.height / 4,
        width: area.width / 2,
        height: 10.min(area.height.saturating_sub(2)),
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::default().title(" Actions ").borders(Borders::ALL)),
        popup,
    );
}

fn render_minimum_notice(frame: &mut Frame<'_>, area: Rect) {
    let message = text(
        "sfmonitor-minimum-size",
        &LocalizationArgs::new()
            .with("width", u64::from(area.width))
            .with("height", u64::from(area.height)),
    );
    frame.render_widget(
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(" SPITFIRE NG sfmonitor ")
                    .title_style(Style::default().fg(BRAND).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL),
            ),
        area,
    );
}

fn render_header(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let board = model
        .snapshot
        .board
        .as_ref()
        .map(|status| status.board_name.clone())
        .unwrap_or_else(|| text("sfmonitor-board-unknown", &LocalizationArgs::new()));
    let (connection, connection_style) = match &model.connection {
        ConnectionState::Connecting => (
            text("sfmonitor-connection-connecting", &LocalizationArgs::new()),
            Style::default().fg(ACCENT),
        ),
        ConnectionState::Connected { .. } => (
            text("sfmonitor-connection-connected", &LocalizationArgs::new()),
            Style::default().fg(Color::Green),
        ),
        ConnectionState::Disconnected { .. } => (
            text(
                "sfmonitor-connection-disconnected",
                &LocalizationArgs::new(),
            ),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    };
    let header = Line::from(vec![
        Span::styled(
            "SPITFIRE NG sfmonitor",
            Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(board, Style::default().fg(Color::White)),
        Span::raw("  "),
        Span::styled(connection, connection_style),
    ]);
    let detail = match &model.connection {
        ConnectionState::Disconnected { reason_key } => text(reason_key, &LocalizationArgs::new()),
        ConnectionState::Connected { .. } => text("sfmonitor-read-only", &LocalizationArgs::new()),
        ConnectionState::Connecting => text("sfmonitor-loading", &LocalizationArgs::new()),
    };
    frame.render_widget(
        Paragraph::new(vec![
            header,
            Line::styled(detail, Style::default().fg(MUTED)),
        ])
        .block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

fn render_navigation(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let items = View::ALL
        .iter()
        .map(|view| ListItem::new(text(view.localization_key(), &LocalizationArgs::new())))
        .collect::<Vec<_>>();
    let selected = View::ALL.iter().position(|view| *view == model.view);
    let mut state = ListState::default().with_selected(selected);
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .title(format!(
                        " {} ",
                        text("sfmonitor-navigation", &LocalizationArgs::new())
                    ))
                    .borders(Borders::RIGHT),
            )
            .highlight_style(
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            )
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

fn render_view(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel, wide: bool) {
    match model.view {
        View::Dashboard => render_dashboard(frame, area, model),
        View::Nodes => render_nodes(frame, area, model, wide),
        View::Callers => render_callers(frame, area, model),
        View::Activity => render_activity(frame, area, model),
        View::Statistics => render_statistics(frame, area, model),
        View::Notifications => render_notifications(frame, area, model),
        View::Maintenance => render_maintenance(frame, area, model),
        View::SystemConfiguration => render_system_configuration(frame, area),
    }
}

fn view_block(view: View) -> Block<'static> {
    Block::default()
        .title(format!(
            " {} ",
            text(view.localization_key(), &LocalizationArgs::new())
        ))
        .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let Some(board) = model.snapshot.board.as_ref() else {
        render_empty(frame, area, View::Dashboard, "sfmonitor-loading");
        return;
    };
    let stats = model.snapshot.statistics.as_ref();
    let maintenance = model.snapshot.maintenance.as_ref();
    let mut lines = vec![
        metric_line("sfmonitor-board-name", &board.board_name),
        metric_line("sfmonitor-uptime", &duration(board.uptime_seconds)),
        Line::raw(""),
        metric_number("sfmonitor-active-nodes", board.active_nodes as u64),
        metric_number("sfmonitor-callers-online", board.callers_online as u64),
        metric_number("sfmonitor-active-transfers", board.active_transfers),
        Line::raw(""),
        metric_number("sfmonitor-open-notifications", board.open_notifications),
        metric_number("sfmonitor-storage-warnings", board.storage_warnings),
        metric_number("sfmonitor-recent-errors", board.recent_errors),
    ];
    if let Some(stats) = stats {
        lines.extend([
            Line::raw(""),
            section_line("sfmonitor-today"),
            metric_number("sfmonitor-completed-calls", stats.calls_completed_today),
            metric_number("sfmonitor-messages-posted", stats.messages_posted_today),
            metric_number("sfmonitor-uploads", stats.successful_uploads_today),
            metric_number("sfmonitor-downloads", stats.successful_downloads_today),
        ]);
    }
    if let Some(maintenance) = maintenance {
        lines.extend([
            Line::raw(""),
            metric_number(
                "sfmonitor-maintenance-warnings",
                maintenance.recent_warning_events,
            ),
        ]);
    }
    if let Some(event) = model.snapshot.events.first() {
        lines.extend([
            Line::raw(""),
            section_line("sfmonitor-latest-activity"),
            Line::raw(event_summary(event)),
        ]);
    }
    if let Some(state) = &model.snapshot.shutdown {
        let code = match state.phase {
            sf_bbs::ShutdownPhase::Running => None,
            sf_bbs::ShutdownPhase::Requested => Some("shutdown-requested"),
            sf_bbs::ShutdownPhase::Draining => Some("shutdown-draining"),
            sf_bbs::ShutdownPhase::Complete => Some("shutdown-complete"),
            sf_bbs::ShutdownPhase::Failed => Some("shutdown-finalization-failed"),
        };
        if let Some(code) = code {
            lines.push(Line::raw(crate::live_ui::result_text(code)));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(view_block(View::Dashboard)),
        area,
    );
}

fn render_nodes(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel, wide: bool) {
    if model.snapshot.nodes.is_empty() {
        render_empty(frame, area, View::Nodes, "sfmonitor-nodes-empty");
        return;
    }
    if wide {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(area);
        render_node_list(frame, columns[0], model);
        render_node_detail(frame, columns[1], model);
    } else if model.show_node_detail {
        render_node_detail(frame, area, model);
    } else {
        render_node_list(frame, area, model);
    }
}

fn render_node_list(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let items = model
        .snapshot
        .nodes
        .iter()
        .map(|node| {
            ListItem::new(format!(
                "{:>2}  {:<16} {:<10} {}",
                node.node_id,
                node.public_handle.as_deref().unwrap_or("—"),
                node.transport.as_deref().unwrap_or("—"),
                node.lifecycle
            ))
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(Some(model.selected_node));
    frame.render_stateful_widget(
        List::new(items)
            .block(view_block(View::Nodes))
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

fn render_node_detail(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let Some(node) = model.snapshot.nodes.get(model.selected_node) else {
        render_empty(frame, area, View::Nodes, "sfmonitor-nodes-empty");
        return;
    };
    let terminal = match (&node.terminal_type, &node.encoding, node.columns, node.rows) {
        (None, None, None, None) => "—".to_owned(),
        _ => format!(
            "{} / {} / {}x{}",
            node.terminal_type.as_deref().unwrap_or("—"),
            node.encoding.as_deref().unwrap_or("—"),
            node.columns
                .map_or_else(|| "—".to_owned(), |value| value.to_string()),
            node.rows
                .map_or_else(|| "—".to_owned(), |value| value.to_string())
        ),
    };
    let lines = vec![
        metric_line("sfmonitor-node", &node.node_id.to_string()),
        metric_line("sfmonitor-state", &node.lifecycle),
        metric_line(
            "sfmonitor-caller",
            node.public_handle.as_deref().unwrap_or("—"),
        ),
        metric_line(
            "sfmonitor-transport",
            node.transport.as_deref().unwrap_or("—"),
        ),
        metric_line(
            "sfmonitor-online-for",
            &node
                .online_seconds
                .map(duration)
                .unwrap_or_else(|| "—".to_owned()),
        ),
        metric_line(
            "sfmonitor-current-area",
            node.current_section.as_deref().unwrap_or("—"),
        ),
        metric_line("sfmonitor-terminal", &terminal),
        metric_line(
            "sfmonitor-presentation",
            node.presentation_profile.as_deref().unwrap_or("—"),
        ),
        metric_line(
            "sfmonitor-security-context",
            node.security_context.as_deref().unwrap_or("—"),
        ),
        metric_line(
            "sfmonitor-transfer-state",
            node.transfer_state.as_deref().unwrap_or("—"),
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::default()
                .title(format!(
                    " {} ",
                    text("sfmonitor-node-detail", &LocalizationArgs::new())
                ))
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_callers(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let items = model
        .snapshot
        .callers
        .iter()
        .map(caller_line)
        .collect::<Vec<_>>();
    if items.is_empty() {
        render_empty(frame, area, View::Callers, "sfmonitor-callers-empty");
        return;
    }
    let mut state = ListState::default().with_selected(Some(model.selected_caller));
    frame.render_stateful_widget(
        List::new(items)
            .block(view_block(View::Callers))
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

fn render_activity(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(4)])
        .split(area);
    let filter_status = if model.filter.active() {
        text("sfmonitor-filter-active", &LocalizationArgs::new())
    } else {
        text("sfmonitor-filter-none", &LocalizationArgs::new())
    };
    let gap = if model.event_gap {
        format!(
            "  {}",
            text("sfmonitor-event-gap-short", &LocalizationArgs::new())
        )
    } else {
        String::new()
    };
    frame.render_widget(
        Paragraph::new(format!("{filter_status}{gap}"))
            .style(Style::default().fg(if model.event_gap { ACCENT } else { MUTED })),
        vertical[0],
    );
    if model.snapshot.events.is_empty() {
        render_empty(
            frame,
            vertical[1],
            View::Activity,
            "operator-activity-empty",
        );
        return;
    }
    let items = model
        .snapshot
        .events
        .iter()
        .map(|event| {
            ListItem::new(format!(
                "{} {:<8} {:<14} N{:<2} {}",
                timestamp(event.occurred_at_utc),
                event_severity(&event.severity),
                event_category(&event.category),
                event
                    .node_id
                    .map_or_else(|| "—".to_owned(), |id| id.to_string()),
                event_summary(event)
            ))
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(Some(model.selected_event));
    frame.render_stateful_widget(
        List::new(items)
            .block(view_block(View::Activity))
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .highlight_symbol("> "),
        vertical[1],
        &mut state,
    );
}

fn render_statistics(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let Some(stats) = model.snapshot.statistics.as_ref() else {
        render_empty(frame, area, View::Statistics, "sfmonitor-loading");
        return;
    };
    let board = model.snapshot.board.as_ref();
    let lines = vec![
        section_line("sfmonitor-live-now"),
        metric_number(
            "sfmonitor-callers-online",
            board.map_or(0, |status| status.callers_online as u64),
        ),
        metric_number(
            "sfmonitor-active-nodes",
            board.map_or(0, |status| status.active_nodes as u64),
        ),
        metric_number(
            "sfmonitor-active-transfers",
            board.map_or(0, |status| status.active_transfers),
        ),
        Line::raw(""),
        section_line("sfmonitor-today"),
        metric_number("sfmonitor-calls-started", stats.calls_started_today),
        metric_number("sfmonitor-completed-calls", stats.calls_completed_today),
        metric_number("sfmonitor-messages-posted", stats.messages_posted_today),
        metric_number("sfmonitor-uploads", stats.successful_uploads_today),
        metric_number("sfmonitor-downloads", stats.successful_downloads_today),
        Line::raw(""),
        section_line("sfmonitor-lifetime"),
        metric_number("sfmonitor-calls", stats.lifetime_calls),
        metric_number("sfmonitor-messages-posted", stats.lifetime_messages_posted),
        metric_number("sfmonitor-uploads", stats.lifetime_files_uploaded),
        metric_number("sfmonitor-downloads", stats.lifetime_files_downloaded),
        Line::raw(""),
        Line::styled(
            text(
                "sfmonitor-history-activation-note",
                &LocalizationArgs::new(),
            ),
            Style::default().fg(MUTED),
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(view_block(View::Statistics)),
        area,
    );
}

fn render_notifications(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    if model.snapshot.notifications.is_empty() {
        render_empty(
            frame,
            area,
            View::Notifications,
            "operator-notifications-empty",
        );
        return;
    }
    let items = model
        .snapshot
        .notifications
        .iter()
        .map(notification_line)
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(Some(model.selected_notification));
    frame.render_stateful_widget(
        List::new(items)
            .block(view_block(View::Notifications))
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

fn render_maintenance(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let Some(status) = model.snapshot.maintenance.as_ref() else {
        render_empty(frame, area, View::Maintenance, "sfmonitor-loading");
        return;
    };
    let lines = vec![
        section_line("sfmonitor-errors-and-warnings"),
        metric_number("sfmonitor-open-notifications", status.open_notifications),
        metric_number(
            "sfmonitor-maintenance-warnings",
            status.recent_warning_events,
        ),
        metric_number("sfmonitor-maintenance-errors", status.recent_error_events),
        Line::raw(""),
        section_line("sfmonitor-maintenance-state"),
        metric_number(
            "sfmonitor-unavailable-storage",
            status.unavailable_storage_roots,
        ),
        metric_number("sfmonitor-pending-review", status.pending_review_files),
        metric_number(
            "sfmonitor-incomplete-transfers",
            status.nonterminal_transfers,
        ),
        Line::raw(""),
        section_line("sfmonitor-retention"),
        metric_number(
            "sfmonitor-detail-retention-days",
            u64::from(status.detail_retention_days),
        ),
        metric_number(
            "sfmonitor-summary-retention-days",
            u64::from(status.summary_retention_days),
        ),
        Line::raw(""),
        Line::styled(
            text("sfmonitor-read-only-maintenance", &LocalizationArgs::new()),
            Style::default().fg(MUTED),
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(view_block(View::Maintenance)),
        area,
    );
}

fn render_system_configuration(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                text(
                    "sfmonitor-configuration-unavailable",
                    &LocalizationArgs::new(),
                ),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::raw(text(
                "sfmonitor-configuration-future",
                &LocalizationArgs::new(),
            )),
            Line::raw(""),
            Line::styled(
                text("sfmonitor-read-only", &LocalizationArgs::new()),
                Style::default().fg(MUTED),
            ),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(view_block(View::SystemConfiguration)),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let view = text(model.view.localization_key(), &LocalizationArgs::new());
    let stale = model
        .status_key
        .map(|key| format!(" | {}", text(key, &LocalizationArgs::new())))
        .unwrap_or_default();
    let footer = format!(
        "{} | {}{}{}",
        text("sfmonitor-key-hints", &LocalizationArgs::new()),
        view,
        stale,
        model
            .action_result
            .as_ref()
            .map(|value| format!(" | {value}"))
            .unwrap_or_default()
    );
    frame.render_widget(
        Paragraph::new(footer)
            .style(Style::default().fg(INFORMATION))
            .block(Block::default().borders(Borders::TOP)),
        area,
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let popup = centered(area, 76, 80);
    frame.render_widget(Clear, popup);
    let topic = text(model.view.localization_key(), &LocalizationArgs::new());
    let key = if model.live.shutdown_confirmation.is_some()
        || (model.show_actions && model.view == View::Dashboard)
    {
        "sfmonitor-help-operator-shutdown"
    } else if model.live.confirmation.is_some() || model.live.disconnect_choice {
        "sfmonitor-help-operator-disconnect"
    } else if model.live.page_menu || model.live.chat.is_some() {
        "sfmonitor-help-operator-page-chat"
    } else if model.show_actions {
        "sfmonitor-help-operator-actions"
    } else {
        model.view.help_key()
    };
    let meaning = text(key, &LocalizationArgs::new());
    let body = text(
        "sfmonitor-help-body",
        &LocalizationArgs::new()
            .with("view", topic)
            .with("meaning", meaning),
    );
    frame.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: true }).block(
            Block::default()
                .title(format!(
                    " {} ",
                    text("sfmonitor-help-title", &LocalizationArgs::new())
                ))
                .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL),
        ),
        popup,
    );
}

fn render_filters(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let popup = centered(area, 72, 70);
    frame.render_widget(Clear, popup);
    let filter = &model.filter;
    let lines = vec![
        filter_line(
            "sfmonitor-filter-category",
            &filter
                .category
                .map(|value| event_category(value.as_str()))
                .unwrap_or_else(filter_all),
        ),
        filter_line(
            "sfmonitor-filter-severity",
            &filter
                .minimum_severity
                .map(|value| event_severity(value.as_str()))
                .unwrap_or_else(filter_all),
        ),
        filter_line(
            "sfmonitor-filter-outcome",
            &filter
                .outcome
                .map(|value| event_outcome(value.as_str()))
                .unwrap_or_else(filter_all),
        ),
        filter_line(
            "sfmonitor-filter-node",
            &filter
                .node_id
                .map_or_else(filter_all, |value| value.to_string()),
        ),
        filter_line(
            "sfmonitor-filter-time",
            &filter.recent_minutes.map_or_else(filter_all, |value| {
                text(
                    "sfmonitor-filter-minutes",
                    &LocalizationArgs::new().with("minutes", value as u64),
                )
            }),
        ),
        Line::raw(""),
        Line::raw(text("sfmonitor-filter-key-hints", &LocalizationArgs::new())),
    ];
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::default()
                .title(format!(
                    " {} ",
                    text("sfmonitor-filter-title", &LocalizationArgs::new())
                ))
                .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL),
        ),
        popup,
    );
}

fn filter_all() -> String {
    text("sfmonitor-filter-all", &LocalizationArgs::new())
}

fn render_empty(frame: &mut Frame<'_>, area: Rect, view: View, key: &str) {
    frame.render_widget(
        Paragraph::new(text(key, &LocalizationArgs::new()))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(view_block(view)),
        area,
    );
}

fn metric_line(label_key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{}: ", text(label_key, &LocalizationArgs::new())),
            Style::default().fg(INFORMATION),
        ),
        Span::raw(value.to_owned()),
    ])
}

fn metric_number(label_key: &str, value: u64) -> Line<'static> {
    metric_line(label_key, &value.to_string())
}

fn section_line(key: &str) -> Line<'static> {
    Line::styled(
        text(key, &LocalizationArgs::new()),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )
}

fn caller_line(caller: &RecentCallerWire) -> ListItem<'static> {
    ListItem::new(format!(
        "{}  {:<18} N{:<2} {:<9} {}",
        timestamp(caller.occurred_at_utc),
        caller.public_handle,
        caller
            .node_id
            .map_or_else(|| "—".to_owned(), |id| id.to_string()),
        caller.transport.as_deref().unwrap_or("—"),
        duration(caller.duration_seconds)
    ))
}

fn notification_line(notification: &NotificationWire) -> ListItem<'static> {
    let reason = text(&notification.reason_key, &LocalizationArgs::new());
    ListItem::new(format!(
        "{} {:<8} {:<12} {} [{}]",
        timestamp(notification.created_at),
        event_severity(&notification.severity),
        event_category(&notification.category),
        reason,
        notification_state(&notification.state)
    ))
}

fn event_summary(event: &EventWire) -> String {
    let action = event_action(&event.event_code);
    text(
        "sfmonitor-event-summary",
        &LocalizationArgs::new()
            .with("action", action)
            .with("outcome", event_outcome(&event.outcome)),
    )
}

fn event_category(value: &str) -> String {
    let key = match value {
        "system" => "operator-event-category-system",
        "node" => "operator-event-category-node",
        "session" => "operator-event-category-session",
        "caller" => "operator-event-category-caller",
        "authentication" => "operator-event-category-authentication",
        "message" => "operator-event-category-message",
        "file" => "operator-event-category-file",
        "transfer" => "operator-event-category-transfer",
        "storage" => "operator-event-category-storage",
        "backup" => "operator-event-category-backup",
        "operator" => "operator-event-category-operator",
        "error" => "operator-event-category-error",
        _ => return value.replace(['-', '_'], " "),
    };
    text(key, &LocalizationArgs::new())
}

fn event_severity(value: &str) -> String {
    let key = match value {
        "info" => "operator-event-severity-info",
        "notice" => "operator-event-severity-notice",
        "warning" => "operator-event-severity-warning",
        "error" => "operator-event-severity-error",
        "critical" => "operator-event-severity-critical",
        _ => return value.replace(['-', '_'], " "),
    };
    text(key, &LocalizationArgs::new())
}

fn event_outcome(value: &str) -> String {
    let key = match value {
        "succeeded" => "operator-event-outcome-succeeded",
        "failed" => "operator-event-outcome-failed",
        "cancelled" => "operator-event-outcome-cancelled",
        "denied" => "operator-event-outcome-denied",
        "unavailable" => "operator-event-outcome-unavailable",
        "observed" => "operator-event-outcome-observed",
        _ => return value.replace(['-', '_'], " "),
    };
    text(key, &LocalizationArgs::new())
}

fn notification_state(value: &str) -> String {
    let key = match value {
        "open" => "sfmonitor-notification-open",
        "acknowledged" => "sfmonitor-notification-acknowledged",
        "resolved" => "sfmonitor-notification-resolved",
        _ => return value.replace(['-', '_'], " "),
    };
    text(key, &LocalizationArgs::new())
}

fn event_action(value: &str) -> String {
    let key = match value {
        "authentication.failed" => "sfmonitor-event-authentication-failed",
        "backup.completed" => "sfmonitor-event-backup-completed",
        "backup.failed" => "sfmonitor-event-backup-failed",
        "backup.started" => "sfmonitor-event-backup-started",
        "caller.created" => "sfmonitor-event-caller-created",
        "file.added" => "sfmonitor-event-file-added",
        "file.changed" => "sfmonitor-event-file-changed",
        "file.moved" => "sfmonitor-event-file-moved",
        "file.removed" => "sfmonitor-event-file-removed",
        "message.posted" => "sfmonitor-event-message-posted",
        "node.disconnect" => "sfmonitor-event-node-disconnected",
        "node.fault" => "sfmonitor-event-node-fault",
        "operator.authenticate" => "sfmonitor-event-operator-authenticated",
        "operator.negotiate" => "sfmonitor-event-operator-negotiated",
        "operator.protocol" => "sfmonitor-event-operator-protocol",
        "operator.read" => "sfmonitor-event-operator-read",
        "session.completed" => "sfmonitor-event-session-completed",
        "session.started" => "sfmonitor-event-session-started",
        "storage.unavailable" => "sfmonitor-event-storage-unavailable",
        "system.restore-verified" => "sfmonitor-event-restore-verified",
        "system.started" => "sfmonitor-event-system-started",
        "transfer.cancelled" => "sfmonitor-event-transfer-cancelled",
        "transfer.completed" => "sfmonitor-event-transfer-completed",
        "transfer.download.completed" => "sfmonitor-event-download-completed",
        "transfer.failed" => "sfmonitor-event-transfer-failed",
        "transfer.started" => "sfmonitor-event-transfer-started",
        "transfer.upload.completed" => "sfmonitor-event-upload-completed",
        _ => return value.replace(['.', '-', '_'], " "),
    };
    text(key, &LocalizationArgs::new())
}

fn timestamp(seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .map(|value| value.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".to_owned())
}

fn duration(seconds: u64) -> String {
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m {seconds:02}s")
    }
}

fn filter_line(label_key: &str, value: &str) -> Line<'static> {
    metric_line(label_key, value)
}

fn centered(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use serde_json::json;

    fn rendered(model: &MonitorModel, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, model)).unwrap();
        terminal.backend().to_string()
    }

    #[test]
    fn preferred_compact_and_minimum_layouts_render() {
        let model = MonitorModel::default();
        let wide = rendered(&model, 100, 30);
        assert!(wide.contains("SPITFIRE NG sfmonitor"));
        assert!(wide.contains("Dashboard"));
        let compact = rendered(&model, 80, 24);
        assert!(compact.contains("SPITFIRE NG sfmonitor"));
        let minimum = rendered(&model, 59, 19);
        assert!(minimum.contains("60x20"));
    }

    #[test]
    fn activity_never_renders_typed_attribute_payloads() {
        let mut model = MonitorModel {
            view: View::Activity,
            ..MonitorModel::default()
        };
        model.snapshot.events.push(EventWire {
            event_id: 1,
            occurred_at_utc: 1_700_000_000,
            board_day: 20260904,
            category: "authentication".to_owned(),
            severity: "notice".to_owned(),
            event_code: "authentication.failed".to_owned(),
            outcome: "denied".to_owned(),
            node_id: Some(1),
            session_id: Some(8),
            correlation_id: None,
            object_kind: None,
            object_id: None,
            attributes: json!({"forbidden_secret": "never-render-this"}),
        });
        let output = rendered(&model, 160, 30);
        assert!(output.contains("Authentication failed"));
        assert!(!output.contains("never-render-this"));
        assert!(!output.contains("forbidden_secret"));
    }

    #[test]
    fn disconnected_state_is_explicit_and_non_color_only() {
        let mut model = MonitorModel::default();
        model.mark_disconnected("operator-endpoint-unavailable");
        let output = rendered(&model, 80, 24);
        assert!(output.contains("DISCONNECTED"));
        assert!(output.contains("STALE"));
    }

    #[test]
    fn actions_explain_unsupported_and_unauthorized_without_hiding_the_action() {
        let mut model = MonitorModel {
            view: View::Nodes,
            show_actions: true,
            ..MonitorModel::default()
        };
        model.connection = ConnectionState::Connected {
            daemon_generation: "test".to_owned(),
            features: crate::model::MONITOR_FEATURES.to_vec(),
        };
        let unsupported = rendered(&model, 100, 30);
        assert!(unsupported.contains("Add 5 minutes"));
        assert!(unsupported.contains("does not support"));
        if let ConnectionState::Connected { features, .. } = &mut model.connection {
            features.push(sf_bbs::OperatorFeature::SessionTimeAdjustment);
        }
        let denied = rendered(&model, 80, 24);
        assert!(denied.contains("Add 5 minutes"));
        assert!(denied.contains("not authorized"));
        model
            .snapshot
            .authorized_capabilities
            .push(sf_core::LocalOperatorCapability::AdjustSessionTime);
        let enabled = rendered(&model, 80, 24);
        assert!(!enabled.contains("not authorized"));
        assert!(!enabled.contains("does not support"));
    }

    #[test]
    fn configuration_doorway_is_honest() {
        let model = MonitorModel {
            view: View::SystemConfiguration,
            ..MonitorModel::default()
        };
        let output = rendered(&model, 100, 30);
        assert!(output.contains("System Configuration"));
        assert!(output.contains("not available"));
    }

    #[test]
    fn empty_notifications_use_the_existing_human_message() {
        let model = MonitorModel {
            view: View::Notifications,
            ..MonitorModel::default()
        };
        let output = rendered(&model, 80, 24);
        assert!(output.contains("No operator notifications need attention."));
        assert!(!output.contains("requested text is unavailable"));
    }

    #[test]
    fn notification_view_renders_safe_localized_attention_state() {
        let mut model = MonitorModel {
            view: View::Notifications,
            ..MonitorModel::default()
        };
        model.snapshot.notifications.push(NotificationWire {
            notification_id: 1,
            source_event_id: 2,
            created_at: 1_700_000_000,
            category: "storage".to_owned(),
            severity: "warning".to_owned(),
            reason_key: "operator-notification-storage-unavailable".to_owned(),
            remediation_key: None,
            state: "open".to_owned(),
            state_version: 1,
        });
        let output = rendered(&model, 160, 30);
        assert!(output.contains("Warning"));
        assert!(output.contains("Storage"));
        assert!(output.contains("configured storage location is unavailable"));
        assert!(output.contains("Open"));
        assert!(!output.contains("notification_id"));
        assert!(!output.contains("source_event_id"));
    }
}
