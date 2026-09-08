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

//! Native Conference Health presentation; no reader identities or message bodies.
use crate::{
    model::MonitorModel,
    worker::{MonitorWorker, WorkerCommand},
};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use sf_core::{
    conference_health::{Filter, Query, Status, Trend},
    LocalizationArgs,
};
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub query: Query,
    pub selected: usize,
    pub detail: bool,
    pub scroll: u16,
}
fn t(key: &str) -> String {
    sf_core::text(key, &LocalizationArgs::new())
}
fn safe(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
        .collect()
}
fn status(s: Status) -> String {
    t(match s {
        Status::New => "health-new",
        Status::Healthy => "health-healthy",
        Status::Quiet => "health-quiet",
        Status::LocallyUnread => "health-unread",
        Status::Dormant => "health-dormant",
        Status::Disabled => "health-disabled",
        Status::CatchingUp => "health-catching-up",
    })
}
fn trend(s: Trend) -> String {
    t(match s {
        Trend::Rising => "health-rising",
        Trend::Stable => "health-stable",
        Trend::Falling => "health-falling",
        Trend::InsufficientHistory => "health-insufficient",
    })
}
fn time(t: Option<i64>) -> String {
    t.and_then(|n| chrono::DateTime::from_timestamp(n, 0))
        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "-".into())
}
fn refresh(model: &mut MonitorModel, worker: &MonitorWorker) {
    model.health.selected = 0;
    model.health.detail = false;
    model.health.scroll = 0;
    worker.send(WorkerCommand::Health(model.health.query.clone()));
}
pub fn key(model: &mut MonitorModel, worker: &MonitorWorker, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Down if model.health.detail => {
            model.health.scroll = model.health.scroll.saturating_add(1).min(256)
        }
        KeyCode::Up if model.health.detail => {
            model.health.scroll = model.health.scroll.saturating_sub(1)
        }
        KeyCode::Down => {
            model.health.selected = model.health.selected.saturating_add(1).min(
                model
                    .snapshot
                    .health
                    .as_ref()
                    .map_or(0, |p| p.snapshot.rows.len().saturating_sub(1)),
            )
        }
        KeyCode::Up => model.health.selected = model.health.selected.saturating_sub(1),
        KeyCode::Enter => {
            model.health.detail = !model.health.detail;
            model.health.scroll = 0;
        }
        KeyCode::Esc if model.health.detail => model.health.detail = false,
        KeyCode::Char('f') => {
            model.health.query.filter = match model.health.query.filter {
                Filter::All => Filter::Local,
                Filter::Local => Filter::Ftn,
                Filter::Ftn => Filter::Qwk,
                Filter::Qwk => Filter::Circuitnet,
                Filter::Circuitnet => Filter::Dormant,
                Filter::Dormant => Filter::LocallyUnread,
                Filter::LocallyUnread => Filter::Rising,
                Filter::Rising => Filter::All,
            };
            model.health.query.offset = 0;
            refresh(model, worker);
        }
        KeyCode::Char('i') => {
            model.health.query.include_retired = !model.health.query.include_retired;
            model.health.query.offset = 0;
            refresh(model, worker);
        }
        KeyCode::Char('s') => {
            model.health.query.sort_posts = !model.health.query.sort_posts;
            model.health.query.offset = 0;
            refresh(model, worker);
        }
        KeyCode::PageDown => {
            if model
                .snapshot
                .health
                .as_ref()
                .is_some_and(|p| p.total > model.health.query.offset + 32)
            {
                model.health.query.offset += 32;
                refresh(model, worker);
            }
        }
        KeyCode::PageUp => {
            model.health.query.offset = model.health.query.offset.saturating_sub(32);
            refresh(model, worker);
        }
        KeyCode::Char('r') => refresh(model, worker),
        _ => return false,
    }
    true
}
pub fn render(frame: &mut Frame<'_>, area: Rect, model: &MonitorModel) {
    let mut lines = vec![t("health-help")];
    if let Some(page) = &model.snapshot.health {
        let s = &page.snapshot;
        lines.push(format!(
            "{}: {} | {}: {} | {}/{}",
            t("health-filter"),
            t(match model.health.query.filter {
                Filter::All => "health-filter-all",
                Filter::Local => "health-filter-local",
                Filter::Ftn => "health-filter-ftn",
                Filter::Qwk => "health-filter-qwk",
                Filter::Circuitnet => "health-filter-circuitnet",
                Filter::Dormant => "health-dormant",
                Filter::LocallyUnread => "health-unread",
                Filter::Rising => "health-rising",
            }),
            t("health-include-retired"),
            model.health.query.include_retired,
            page.offset,
            page.total
        ));
        lines.push(format!(
            "{}: {} / {} | {}: {}",
            t("health-pending"),
            s.pending_messages,
            s.pending_conferences,
            t("health-tracking"),
            time(Some(s.monitoring_since))
        ));
        if model.health.detail {
            if let Some(row) = s.rows.get(model.health.selected) {
                lines.push(format!("#{} {}", row.number, safe(&row.name)));
                for c in &row.contexts {
                    lines.push(format!(
                        "{} / {} / {}{}",
                        c.adapter,
                        safe(&c.network),
                        safe(&c.area),
                        if c.retired {
                            format!(" ({})", t("health-retired"))
                        } else {
                            String::new()
                        }
                    ));
                }
                if let Some(d) = &row.detail {
                    lines.push(format!(
                        "{} / {} | {}: {}",
                        status(d.status),
                        trend(d.trend),
                        t("health-as-of"),
                        time(Some(d.as_of))
                    ));
                    for m in &d.windows {
                        lines.push(format!(
                            "{}d: {} {} | {} {} | {} {}",
                            m.days,
                            m.readers,
                            t("health-readers"),
                            m.local_posts,
                            t("health-local-posts"),
                            m.inbound_posts,
                            t("health-inbound")
                        ));
                        lines.push(format!(
                            "  {} {} | {} {} | {} {}",
                            m.progress,
                            t("health-progress"),
                            m.replies,
                            t("health-replies"),
                            m.active_threads,
                            t("health-threads")
                        ));
                    }
                    lines.push(format!(
                        "{}: {} {} / {} {}",
                        t("health-previous"),
                        d.previous_30.readers,
                        t("health-readers"),
                        d.previous_30.local_posts,
                        t("health-local-posts")
                    ));
                    lines.push(format!("{}: {}", t("health-last-read"), time(d.last_read)));
                    lines.push(format!(
                        "{}: {}",
                        t("health-last-activity"),
                        time(d.last_message)
                    ));
                    for reason in &d.reasons {
                        lines.push(safe(reason));
                    }
                } else {
                    lines.push(t("health-not-ready"));
                }
            }
        } else {
            lines.push(t("health-columns"));
            let visible = area.height.saturating_sub(7) as usize;
            let start = model
                .health
                .selected
                .saturating_sub(visible.saturating_sub(1));
            for (index, row) in s.rows.iter().enumerate().skip(start).take(visible) {
                let (readers, local, inbound, label) = row
                    .detail
                    .as_ref()
                    .map(|d| {
                        (
                            d.windows[1].readers,
                            d.windows[1].local_posts,
                            d.windows[1].inbound_posts,
                            status(d.status),
                        )
                    })
                    .unwrap_or((0, 0, 0, t("health-not-ready")));
                lines.push(format!(
                    "{}{:3} {:18} {:5} {:5} {:6} {}",
                    if index == model.health.selected {
                        ">"
                    } else {
                        " "
                    },
                    row.number,
                    safe(&row.name).chars().take(18).collect::<String>(),
                    readers,
                    local,
                    inbound,
                    label
                ));
            }
        }
    } else {
        lines.push(t("health-unavailable"));
    }
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .scroll((
                if model.health.detail {
                    model.health.scroll
                } else {
                    0
                },
                0,
            ))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(t("health-title")),
            ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn health_view_fits_classic_terminal_and_sanitizes_metadata() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut model = MonitorModel::default();
        model.snapshot.health = Some(sf_core::conference_health::Page {
            total: 1,
            offset: 0,
            snapshot: sf_core::conference_health::Snapshot {
                settings: sf_core::conference_health::Settings {
                    enabled: true,
                    retention_days: 365,
                    dormant_days: 30,
                    bulletin: false,
                    bulletin_limit: 10,
                },
                monitoring_since: 0,
                now: 0,
                pending_messages: 0,
                historical_scan_pending: false,
                pending_conferences: 0,
                rows: vec![sf_core::conference_health::Row {
                    conference_id: 1,
                    number: 1,
                    name: "Public\x1b[2J\u{202e}Area".into(),
                    retired: false,
                    contexts: vec![],
                    detail: None,
                    pending: false,
                }],
            },
        });
        for (width, height) in [(80, 25), (60, 20)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| render(frame, frame.area(), &model))
                .unwrap();
            let rendered = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(rendered.contains("Conference Health"));
            assert!(!rendered.contains('\x1b'));
            assert!(!rendered.contains('\u{202e}'));
        }
    }
}
