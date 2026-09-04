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

use sf_bbs::{
    BoardStatusWire, EventWire, MaintenanceWire, NodeStatusWire, NotificationWire,
    OperatorEventQuery, OperatorFeature, RecentCallerWire, StatisticsWire,
};
use sf_core::{EventCategory, EventOutcome, EventSeverity};

pub const RECENT_EVENT_LIMIT: usize = 100;
pub const RECENT_CALLER_LIMIT: usize = 100;
pub const NOTIFICATION_LIMIT: usize = 100;
pub const PREFERRED_WIDTH: u16 = 100;
pub const PREFERRED_HEIGHT: u16 = 30;
pub const MINIMUM_WIDTH: u16 = 60;
pub const MINIMUM_HEIGHT: u16 = 20;

pub const MONITOR_FEATURES: [OperatorFeature; 9] = [
    OperatorFeature::BoardStatus,
    OperatorFeature::NodeList,
    OperatorFeature::NodeStatus,
    OperatorFeature::RecentEvents,
    OperatorFeature::LiveEvents,
    OperatorFeature::Notifications,
    OperatorFeature::Statistics,
    OperatorFeature::RecentCallers,
    OperatorFeature::MaintenanceStatus,
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum View {
    #[default]
    Dashboard,
    Nodes,
    Callers,
    Activity,
    Statistics,
    Notifications,
    Maintenance,
    SystemConfiguration,
}

impl View {
    pub const ALL: [Self; 8] = [
        Self::Dashboard,
        Self::Nodes,
        Self::Callers,
        Self::Activity,
        Self::Statistics,
        Self::Notifications,
        Self::Maintenance,
        Self::SystemConfiguration,
    ];

    pub const fn localization_key(self) -> &'static str {
        match self {
            Self::Dashboard => "sfmonitor-view-dashboard",
            Self::Nodes => "sfmonitor-view-nodes",
            Self::Callers => "sfmonitor-view-callers",
            Self::Activity => "sfmonitor-view-activity",
            Self::Statistics => "sfmonitor-view-statistics",
            Self::Notifications => "sfmonitor-view-notifications",
            Self::Maintenance => "sfmonitor-view-maintenance",
            Self::SystemConfiguration => "sfmonitor-view-system-configuration",
        }
    }

    pub const fn help_key(self) -> &'static str {
        match self {
            Self::Dashboard => "sfmonitor-help-dashboard",
            Self::Nodes => "sfmonitor-help-nodes",
            Self::Callers => "sfmonitor-help-callers",
            Self::Activity => "sfmonitor-help-activity",
            Self::Statistics => "sfmonitor-help-statistics",
            Self::Notifications => "sfmonitor-help-notifications",
            Self::Maintenance => "sfmonitor-help-maintenance",
            Self::SystemConfiguration => "sfmonitor-help-configuration",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutMode {
    MinimumNotice,
    Compact,
    Wide,
}

pub const fn layout_mode(width: u16, height: u16) -> LayoutMode {
    if width < MINIMUM_WIDTH || height < MINIMUM_HEIGHT {
        LayoutMode::MinimumNotice
    } else if width >= PREFERRED_WIDTH && height >= PREFERRED_HEIGHT {
        LayoutMode::Wide
    } else {
        LayoutMode::Compact
    }
}

#[derive(Clone, Debug, Default)]
pub struct EventFilter {
    pub category: Option<EventCategory>,
    pub minimum_severity: Option<EventSeverity>,
    pub outcome: Option<EventOutcome>,
    pub node_id: Option<u32>,
    pub recent_minutes: Option<u16>,
}

impl EventFilter {
    pub fn query(&self, now_utc: i64) -> OperatorEventQuery {
        OperatorEventQuery {
            from_utc: self
                .recent_minutes
                .map(|minutes| now_utc.saturating_sub(i64::from(minutes) * 60)),
            category: self.category,
            minimum_severity: self.minimum_severity,
            outcome: self.outcome,
            node_id: self.node_id,
            limit: Some(RECENT_EVENT_LIMIT),
            ..OperatorEventQuery::default()
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active(&self) -> bool {
        self.category.is_some()
            || self.minimum_severity.is_some()
            || self.outcome.is_some()
            || self.node_id.is_some()
            || self.recent_minutes.is_some()
    }

    pub fn matches(&self, event: &EventWire, now_utc: i64) -> bool {
        self.category
            .is_none_or(|category| event.category == category.as_str())
            && self.minimum_severity.is_none_or(|minimum| {
                severity_rank(&event.severity) >= severity_rank(minimum.as_str())
            })
            && self
                .outcome
                .is_none_or(|outcome| event.outcome == outcome.as_str())
            && self.node_id.is_none_or(|node| event.node_id == Some(node))
            && self.recent_minutes.is_none_or(|minutes| {
                event.occurred_at_utc >= now_utc.saturating_sub(i64::from(minutes) * 60)
            })
    }

    pub fn cycle_category(&mut self) {
        self.category = match self.category {
            None => Some(EventCategory::System),
            Some(EventCategory::System) => Some(EventCategory::Node),
            Some(EventCategory::Node) => Some(EventCategory::Session),
            Some(EventCategory::Session) => Some(EventCategory::Caller),
            Some(EventCategory::Caller) => Some(EventCategory::Authentication),
            Some(EventCategory::Authentication) => Some(EventCategory::Message),
            Some(EventCategory::Message) => Some(EventCategory::File),
            Some(EventCategory::File) => Some(EventCategory::Transfer),
            Some(EventCategory::Transfer) => Some(EventCategory::Storage),
            Some(EventCategory::Storage) => Some(EventCategory::Backup),
            Some(EventCategory::Backup) => Some(EventCategory::Operator),
            Some(EventCategory::Operator) => Some(EventCategory::Error),
            Some(EventCategory::Error) => None,
        };
    }

    pub fn cycle_severity(&mut self) {
        self.minimum_severity = match self.minimum_severity {
            None => Some(EventSeverity::Info),
            Some(EventSeverity::Info) => Some(EventSeverity::Notice),
            Some(EventSeverity::Notice) => Some(EventSeverity::Warning),
            Some(EventSeverity::Warning) => Some(EventSeverity::Error),
            Some(EventSeverity::Error) => Some(EventSeverity::Critical),
            Some(EventSeverity::Critical) => None,
        };
    }

    pub fn cycle_outcome(&mut self) {
        self.outcome = match self.outcome {
            None => Some(EventOutcome::Succeeded),
            Some(EventOutcome::Succeeded) => Some(EventOutcome::Failed),
            Some(EventOutcome::Failed) => Some(EventOutcome::Cancelled),
            Some(EventOutcome::Cancelled) => Some(EventOutcome::Denied),
            Some(EventOutcome::Denied) => Some(EventOutcome::Unavailable),
            Some(EventOutcome::Unavailable) => Some(EventOutcome::Observed),
            Some(EventOutcome::Observed) => None,
        };
    }

    pub fn cycle_time(&mut self) {
        self.recent_minutes = match self.recent_minutes {
            None => Some(15),
            Some(15) => Some(60),
            Some(60) => Some(24 * 60),
            Some(1_440) => Some(7 * 24 * 60),
            Some(_) => None,
        };
    }
}

#[derive(Clone, Debug, Default)]
pub struct MonitorSnapshot {
    pub board: Option<BoardStatusWire>,
    pub nodes: Vec<NodeStatusWire>,
    pub events: Vec<EventWire>,
    pub notifications: Vec<NotificationWire>,
    pub statistics: Option<StatisticsWire>,
    pub callers: Vec<RecentCallerWire>,
    pub maintenance: Option<MaintenanceWire>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ConnectionState {
    #[default]
    Connecting,
    Connected {
        daemon_generation: String,
        features: Vec<OperatorFeature>,
    },
    Disconnected {
        reason_key: &'static str,
    },
}

#[derive(Clone, Debug, Default)]
pub struct MonitorModel {
    pub view: View,
    pub connection: ConnectionState,
    pub snapshot: MonitorSnapshot,
    pub filter: EventFilter,
    pub selected_node: usize,
    pub selected_caller: usize,
    pub selected_event: usize,
    pub selected_notification: usize,
    pub show_node_detail: bool,
    pub show_help: bool,
    pub show_filters: bool,
    pub event_gap: bool,
    pub status_key: Option<&'static str>,
}

impl MonitorModel {
    pub fn next_view(&mut self) {
        let current = View::ALL
            .iter()
            .position(|view| *view == self.view)
            .unwrap_or(0);
        self.view = View::ALL[(current + 1) % View::ALL.len()];
        self.close_overlays();
    }

    pub fn previous_view(&mut self) {
        let current = View::ALL
            .iter()
            .position(|view| *view == self.view)
            .unwrap_or(0);
        self.view = View::ALL[(current + View::ALL.len() - 1) % View::ALL.len()];
        self.close_overlays();
    }

    pub fn select_view(&mut self, view: View) {
        self.view = view;
        self.close_overlays();
    }

    pub fn close_overlays(&mut self) {
        self.show_help = false;
        self.show_filters = false;
        self.show_node_detail = false;
    }

    pub fn move_selection(&mut self, delta: isize) {
        let (selection, length) = match self.view {
            View::Nodes => (&mut self.selected_node, self.snapshot.nodes.len()),
            View::Callers => (&mut self.selected_caller, self.snapshot.callers.len()),
            View::Activity => (&mut self.selected_event, self.snapshot.events.len()),
            View::Notifications => (
                &mut self.selected_notification,
                self.snapshot.notifications.len(),
            ),
            _ => return,
        };
        if length == 0 {
            *selection = 0;
            return;
        }
        *selection = selection
            .saturating_add_signed(delta)
            .min(length.saturating_sub(1));
    }

    pub fn clamp_selections(&mut self) {
        self.selected_node = clamp(self.selected_node, self.snapshot.nodes.len());
        self.selected_caller = clamp(self.selected_caller, self.snapshot.callers.len());
        self.selected_event = clamp(self.selected_event, self.snapshot.events.len());
        self.selected_notification = clamp(
            self.selected_notification,
            self.snapshot.notifications.len(),
        );
    }

    pub fn merge_live_events(&mut self, mut events: Vec<EventWire>, gap: bool, now_utc: i64) {
        self.event_gap |= gap;
        events.retain(|event| self.filter.matches(event, now_utc));
        self.snapshot.events.append(&mut events);
        self.snapshot.events.sort_by_key(|event| event.event_id);
        self.snapshot.events.dedup_by_key(|event| event.event_id);
        if self.snapshot.events.len() > RECENT_EVENT_LIMIT {
            let excess = self.snapshot.events.len() - RECENT_EVENT_LIMIT;
            self.snapshot.events.drain(0..excess);
        }
        self.snapshot.events.reverse();
        self.clamp_selections();
    }

    pub fn mark_disconnected(&mut self, reason_key: &'static str) {
        self.connection = ConnectionState::Disconnected { reason_key };
        self.status_key = Some("sfmonitor-status-stale");
    }
}

fn severity_rank(value: &str) -> u8 {
    match value {
        "critical" => 4,
        "error" => 3,
        "warning" => 2,
        "notice" => 1,
        _ => 0,
    }
}

fn clamp(value: usize, length: usize) -> usize {
    value.min(length.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(id: u64) -> EventWire {
        EventWire {
            event_id: id,
            occurred_at_utc: i64::try_from(id).unwrap(),
            board_day: 20260904,
            category: "system".to_owned(),
            severity: "info".to_owned(),
            event_code: "system.started".to_owned(),
            outcome: "observed".to_owned(),
            node_id: None,
            session_id: None,
            correlation_id: None,
            object_kind: None,
            object_id: None,
            attributes: json!({}),
        }
    }

    #[test]
    fn navigation_wraps_without_mutation_actions() {
        let mut model = MonitorModel::default();
        model.previous_view();
        assert_eq!(model.view, View::SystemConfiguration);
        model.next_view();
        assert_eq!(model.view, View::Dashboard);
        let features = format!("{:?}", MONITOR_FEATURES).to_ascii_lowercase();
        for forbidden in ["disconnect", "time", "chat", "shutdown", "config-write"] {
            assert!(!features.contains(forbidden));
        }
    }

    #[test]
    fn layout_has_safe_minimum_and_two_useful_modes() {
        assert_eq!(layout_mode(59, 30), LayoutMode::MinimumNotice);
        assert_eq!(layout_mode(100, 19), LayoutMode::MinimumNotice);
        assert_eq!(layout_mode(80, 24), LayoutMode::Compact);
        assert_eq!(layout_mode(100, 30), LayoutMode::Wide);
    }

    #[test]
    fn filters_are_bounded_typed_queries() {
        let mut filter = EventFilter::default();
        filter.cycle_category();
        filter.cycle_severity();
        filter.cycle_outcome();
        filter.cycle_time();
        filter.node_id = Some(2);
        let query = filter.query(10_000);
        assert_eq!(query.category, Some(EventCategory::System));
        assert_eq!(query.minimum_severity, Some(EventSeverity::Info));
        assert_eq!(query.outcome, Some(EventOutcome::Succeeded));
        assert_eq!(query.node_id, Some(2));
        assert_eq!(query.from_utc, Some(9_100));
        assert_eq!(query.limit, Some(RECENT_EVENT_LIMIT));
        filter.clear();
        assert!(!filter.active());
    }

    #[test]
    fn live_events_deduplicate_bound_and_mark_gaps() {
        let mut model = MonitorModel::default();
        model.snapshot.events = (1..=100).map(event).collect();
        model.merge_live_events(vec![event(100), event(101)], true, 101);
        assert!(model.event_gap);
        assert_eq!(model.snapshot.events.len(), RECENT_EVENT_LIMIT);
        assert_eq!(model.snapshot.events[0].event_id, 101);
        assert_eq!(model.snapshot.events.last().unwrap().event_id, 2);
    }

    #[test]
    fn live_events_obey_the_same_typed_filter_as_history() {
        let mut model = MonitorModel::default();
        model.filter.category = Some(EventCategory::Transfer);
        model.merge_live_events(vec![event(1)], false, 1);
        assert!(model.snapshot.events.is_empty());
    }

    #[test]
    fn disconnect_preserves_snapshot_but_marks_it_stale() {
        let mut model = MonitorModel::default();
        model.snapshot.events.push(event(1));
        model.mark_disconnected("operator-endpoint-unavailable");
        assert_eq!(model.snapshot.events.len(), 1);
        assert!(matches!(
            model.connection,
            ConnectionState::Disconnected { .. }
        ));
        assert_eq!(model.status_key, Some("sfmonitor-status-stale"));
    }
}
