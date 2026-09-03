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

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sf_core::{
    LogicalPaths, NodeRuntimeState, NodeSnapshot, RuntimeConfig, RuntimeDatabase,
    TransportAdapterConfig, TransportConfig, TransportKind,
};

use crate::{ApplicationError, PresentationResolver};

pub const RUNTIME_STATUS_FILE: &str = "runtime-status.toml";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeStatusDocument {
    pub format_version: u32,
    pub board_name: String,
    pub started_at: i64,
    pub updated_at: i64,
    pub listeners: Vec<ListenerStatus>,
    pub nodes: Vec<PublishedNodeStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ListenerStatus {
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedNodeStatus {
    pub number: u32,
    pub description: Option<String>,
    pub state: String,
    pub session_id: Option<u64>,
    pub caller_id: Option<i64>,
    #[serde(default)]
    pub caller_login_identifier: Option<String>,
    #[serde(default)]
    pub caller_lifecycle: Option<String>,
    pub caller_name: Option<String>,
    pub transport: Option<String>,
    pub connected_at: Option<i64>,
    #[serde(default)]
    pub activity_file: Option<String>,
    #[serde(default)]
    pub presentation: Option<PublishedPresentationStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedPresentationStatus {
    pub terminal_type: Option<String>,
    pub ansi: bool,
    pub encoding: String,
    pub columns: Option<u16>,
    pub rows: Option<u16>,
    pub page_length: Option<u16>,
    pub locale: String,
    pub presentation_profile: String,
    pub menu_mode: String,
    pub menu_context: Option<String>,
    #[serde(default)]
    pub renderer_path: Option<String>,
    pub caller_security: Option<u16>,
    pub sysop_threshold: u16,
    pub visible_action_count: Option<usize>,
}

pub(crate) fn publish_runtime_status(
    path: &Path,
    board_name: &str,
    started_at: i64,
    transports: &[TransportConfig],
    database_path: &Path,
    nodes: &[NodeSnapshot],
) -> Result<(), ApplicationError> {
    let document = RuntimeStatusDocument {
        format_version: 2,
        board_name: board_name.to_owned(),
        started_at,
        updated_at: unix_seconds()?,
        listeners: listener_statuses(transports),
        nodes: nodes
            .iter()
            .map(|node| published_node(node, database_path))
            .collect(),
    };
    let encoded = toml::to_string_pretty(&document)
        .map_err(|error| ApplicationError::StatusSerialize(error.to_string()))?;
    let parent = path.parent().ok_or(ApplicationError::Coordination(
        "runtime status path has no parent",
    ))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|source| {
        ApplicationError::WriteStatus {
            path: path.to_path_buf(),
            source,
        }
    })?;
    temporary
        .write_all(encoded.as_bytes())
        .and_then(|()| temporary.as_file_mut().sync_all())
        .map_err(|source| ApplicationError::WriteStatus {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .persist(path)
        .map_err(|error| ApplicationError::WriteStatus {
            path: path.to_path_buf(),
            source: error.error,
        })?;
    Ok(())
}

pub fn board_status(config_path: &Path) -> Result<String, ApplicationError> {
    let canonical =
        config_path
            .canonicalize()
            .map_err(|source| ApplicationError::ResolveConfiguration {
                path: config_path.to_path_buf(),
                source,
            })?;
    let root = canonical
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| ApplicationError::MissingBoardRoot(canonical.clone()))?;
    let config = RuntimeConfig::load(&canonical)?;
    let validated = config.validate()?;
    let paths = LogicalPaths::resolve(root, &validated)?;
    let presentation = PresentationResolver::load(&paths, &validated.presentation);
    let language = sf_core::LanguageResolver::load(&paths, &validated.language.default_locale);
    let localizer = language.localizer();
    let text = |key: &str, args: sf_core::LocalizationArgs| localizer.text(key, &args);
    let database = RuntimeDatabase::open(paths.database())?;
    let identity = database
        .load_board_identity()?
        .ok_or(sf_core::DatabaseError::MissingBoardIdentity)?;
    let public_information = database.public_directory_policy()?;
    let status_path = paths
        .get(sf_core::LogicalPath::Work)
        .join(RUNTIME_STATUS_FILE);
    let runtime_state = if status_path.is_file() {
        text(
            "operator-status-runtime-published",
            sf_core::LocalizationArgs::new(),
        )
    } else {
        text(
            "operator-status-runtime-offline",
            sf_core::LocalizationArgs::new(),
        )
    };
    let mut output = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n",
        text("operator-status-header", sf_core::LocalizationArgs::new()),
        text(
            "operator-status-software",
            sf_core::LocalizationArgs::new().with("version", sf_core::PRODUCT_VERSION),
        ),
        text(
            "operator-status-board",
            sf_core::LocalizationArgs::new().with("board", identity.name()),
        ),
        text(
            "operator-status-sysop",
            sf_core::LocalizationArgs::new().with("sysop", identity.sysop_name()),
        ),
        text(
            "operator-status-configuration",
            sf_core::LocalizationArgs::new().with("path", canonical.display().to_string()),
        ),
        text(
            "operator-status-runtime",
            sf_core::LocalizationArgs::new().with("state", runtime_state),
        ),
    );
    let language_status = language.status();
    output.push_str(&format!(
        "{}\n",
        text(
            "operator-status-language-title",
            sf_core::LocalizationArgs::new()
        )
    ));
    for line in [
        text(
            "operator-status-default-locale",
            sf_core::LocalizationArgs::new()
                .with("locale", language_status.default_locale.as_str()),
        ),
        text(
            "operator-status-effective-locale",
            sf_core::LocalizationArgs::new()
                .with("locale", language_status.effective_locale.as_str()),
        ),
        text(
            "operator-status-language-package",
            sf_core::LocalizationArgs::new()
                .with("locale", language_status.effective_locale.as_str())
                .with("version", language_status.package_version.as_str()),
        ),
        text(
            "operator-status-language-state",
            sf_core::LocalizationArgs::new().with(
                "status",
                if language_status.degraded {
                    "DEGRADED"
                } else {
                    "READY"
                },
            ),
        ),
    ] {
        output.push_str("  ");
        output.push_str(&line);
        output.push('\n');
    }
    for issue in &language_status.issues {
        output.push_str("  ");
        output.push_str(&text(
            "operator-status-language-issue",
            sf_core::LocalizationArgs::new().with("issue", issue.as_str()),
        ));
        output.push('\n');
    }
    let profile = presentation.status();
    output.push_str(&format!(
        "\n{}\n",
        text(
            "operator-status-presentation-title",
            sf_core::LocalizationArgs::new()
        )
    ));
    output.push_str(&text(
        "operator-status-presentation",
        sf_core::LocalizationArgs::new()
            .with(
                "mode",
                match profile.mode {
                    sf_core::PresentationMode::LegacyResources => "legacy-resources",
                    sf_core::PresentationMode::Profile => "profile",
                },
            )
            .with(
                "menu",
                match validated.presentation.menu_mode {
                    sf_core::MenuPresentationMode::DisplayOverrides => "display-overrides",
                    sf_core::MenuPresentationMode::Generated => "generated",
                },
            )
            .with(
                "active",
                profile
                    .configured_active
                    .as_deref()
                    .map(|id| {
                        format!(
                            "{}{}",
                            id,
                            profile
                                .active_version
                                .as_deref()
                                .map_or_else(String::new, |version| format!(" {version}"))
                        )
                    })
                    .unwrap_or_else(|| "-".to_owned()),
            )
            .with(
                "base",
                profile
                    .configured_base
                    .as_deref()
                    .map(|id| {
                        format!(
                            "{}{}",
                            id,
                            profile
                                .base_version
                                .as_deref()
                                .map_or_else(String::new, |version| format!(" {version}"))
                        )
                    })
                    .unwrap_or_else(|| "-".to_owned()),
            )
            .with("effective", profile.effective_source.as_str())
            .with(
                "status",
                if profile.degraded {
                    "DEGRADED"
                } else {
                    "ready"
                },
            ),
    ));
    output.push('\n');
    output.push_str(&text(
        "operator-status-public-information",
        sf_core::LocalizationArgs::new()
            .with("enabled", public_information.enabled.to_string())
            .with(
                "last_call",
                public_information.show_last_call_date.to_string(),
            )
            .with("location", public_information.show_city_region.to_string())
            .with(
                "caller_additions",
                public_information.caller_bbs_additions_enabled.to_string(),
            )
            .with("version", public_information.state_version),
    ));
    output.push('\n');
    for issue in &profile.issues {
        output.push_str("  ");
        output.push_str(&text(
            "operator-status-language-issue",
            sf_core::LocalizationArgs::new().with("issue", issue.as_str()),
        ));
        output.push('\n');
    }
    for line in [
        text(
            "operator-status-post-login",
            sf_core::LocalizationArgs::new().with(
                "journey",
                match validated.caller.post_login_journey {
                    sf_core::PostLoginJourney::None => "none",
                    sf_core::PostLoginJourney::Stock => "stock",
                },
            ),
        ),
        text(
            "operator-status-new-caller-security",
            sf_core::LocalizationArgs::new().with("security", validated.caller.new_caller_security),
        ),
        text(
            "operator-status-sysop-security",
            sf_core::LocalizationArgs::new().with("security", validated.caller.sysop_security),
        ),
    ] {
        output.push_str("  ");
        output.push_str(&line);
        output.push('\n');
    }
    let document = if status_path.is_file() {
        let input =
            fs::read_to_string(&status_path).map_err(|source| ApplicationError::ReadStatus {
                path: status_path.clone(),
                source,
            })?;
        Some(
            toml::from_str::<RuntimeStatusDocument>(&input)
                .map_err(|error| ApplicationError::StatusParse(error.to_string()))?,
        )
    } else {
        None
    };
    output.push_str(&format!(
        "\n{}\n",
        text(
            "operator-status-terminal-services",
            sf_core::LocalizationArgs::new()
        )
    ));
    let listeners = document.as_ref().map_or_else(
        || listener_statuses(&config.transports),
        |value| value.listeners.clone(),
    );
    for listener in listeners {
        output.push_str(&text(
            "operator-status-listener",
            sf_core::LocalizationArgs::new()
                .with("name", listener.name)
                .with("transport", listener.transport)
                .with("enabled", listener.enabled.to_string())
                .with("endpoint", listener.endpoint),
        ));
        output.push('\n');
    }
    for transport in &config.transports {
        let TransportAdapterConfig::Ssh { host_key, .. } = &transport.adapter else {
            continue;
        };
        let key_path = paths.get(sf_core::LogicalPath::System).join(host_key);
        let fingerprint = crate::transports::host_key_fingerprint(
            paths.get(sf_core::LogicalPath::System),
            host_key,
        )?
        .unwrap_or_else(|| "not generated".to_owned());
        output.push_str(&text(
            "operator-status-ssh-host-key",
            sf_core::LocalizationArgs::new()
                .with("path", key_path.display().to_string())
                .with("fingerprint", fingerprint),
        ));
        output.push('\n');
    }
    output.push_str(&format!(
        "\n{}\n",
        text("operator-status-nodes", sf_core::LocalizationArgs::new())
    ));
    if let Some(document) = document {
        let observed_at = unix_seconds()?;
        output.push_str(&text(
            "operator-status-published-times",
            sf_core::LocalizationArgs::new()
                .with("published", document.updated_at)
                .with("started", document.started_at),
        ));
        output.push('\n');
        for node in document.nodes {
            let duration = node.connected_at.map_or_else(
                || "-".to_owned(),
                |connected| format!("{}s", observed_at.saturating_sub(connected)),
            );
            let caller = match (
                node.caller_login_identifier.as_deref(),
                node.caller_name.as_deref(),
            ) {
                (Some(login), Some(handle)) => format!("{login} ({handle})"),
                (_, Some(handle)) => handle.to_owned(),
                _ => "-".to_owned(),
            };
            output.push_str(&text(
                "operator-status-node-live",
                sf_core::LocalizationArgs::new()
                    .with("node", format!("{:>3}", node.number))
                    .with("state", node.state)
                    .with("caller", caller)
                    .with("lifecycle", node.caller_lifecycle.as_deref().unwrap_or("-"))
                    .with("transport", node.transport.as_deref().unwrap_or("-"))
                    .with("duration", duration)
                    .with("file", node.activity_file.as_deref().unwrap_or("-")),
            ));
            output.push('\n');
            if let Some(presentation) = node.presentation {
                output.push_str(&text(
                    "operator-status-node-presentation",
                    sf_core::LocalizationArgs::new()
                        .with(
                            "terminal",
                            presentation.terminal_type.as_deref().unwrap_or("-"),
                        )
                        .with("ansi", presentation.ansi.to_string())
                        .with("encoding", presentation.encoding)
                        .with(
                            "columns",
                            presentation
                                .columns
                                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                        )
                        .with(
                            "rows",
                            presentation
                                .rows
                                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                        )
                        .with(
                            "page",
                            presentation
                                .page_length
                                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                        )
                        .with("locale", presentation.locale)
                        .with("profile", presentation.presentation_profile)
                        .with("menu_mode", presentation.menu_mode)
                        .with(
                            "context",
                            presentation.menu_context.as_deref().unwrap_or("login"),
                        )
                        .with(
                            "renderer",
                            presentation.renderer_path.as_deref().unwrap_or("-"),
                        )
                        .with(
                            "security",
                            presentation
                                .caller_security
                                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                        )
                        .with("threshold", presentation.sysop_threshold)
                        .with(
                            "actions",
                            presentation
                                .visible_action_count
                                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                        ),
                ));
                output.push('\n');
            }
        }
    } else {
        for node in validated.nodes {
            output.push_str(&text(
                "operator-status-node-offline",
                sf_core::LocalizationArgs::new()
                    .with("node", format!("{:>3}", node.id.get()))
                    .with("state", if node.enabled { "offline" } else { "disabled" })
                    .with("description", node.description.as_deref().unwrap_or("")),
            ));
            output.push('\n');
        }
    }
    Ok(output)
}

pub(crate) fn remove_runtime_status(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            tracing::warn!(path = %path.display(), error = %error, "could not remove runtime status file")
        }
    }
}

fn listener_statuses(transports: &[TransportConfig]) -> Vec<ListenerStatus> {
    transports
        .iter()
        .enumerate()
        .map(|(index, transport)| ListenerStatus {
            name: transport.effective_name(index),
            transport: adapter_name(&transport.adapter).to_owned(),
            enabled: transport.enabled,
            endpoint: transport.network_listener().map_or_else(
                || match &transport.adapter {
                    TransportAdapterConfig::Serial { device, baud, .. }
                    | TransportAdapterConfig::Modem { device, baud, .. } => {
                        format!("{device} @ {baud}")
                    }
                    _ => "not configured".to_owned(),
                },
                |address| address.to_string(),
            ),
        })
        .collect()
}

fn published_node(node: &NodeSnapshot, database_path: &Path) -> PublishedNodeStatus {
    let caller_identity = node.caller_id.and_then(|caller_id| {
        RuntimeDatabase::open(database_path)
            .ok()?
            .caller_by_id(caller_id)
            .ok()?
    });
    PublishedNodeStatus {
        number: node.id.get(),
        description: node.description.clone(),
        state: match node.state {
            NodeRuntimeState::Waiting => "waiting",
            NodeRuntimeState::Connecting => "connecting",
            NodeRuntimeState::Login => "login",
            NodeRuntimeState::Online => "online",
            NodeRuntimeState::PagePending => "page-pending",
            NodeRuntimeState::Chatting => "chatting",
            NodeRuntimeState::Downloading => "downloading",
            NodeRuntimeState::Uploading => "uploading",
            NodeRuntimeState::Disconnecting => "disconnecting",
            NodeRuntimeState::Disabled => "disabled",
        }
        .to_owned(),
        session_id: node.session_id.map(sf_core::SessionId::get),
        caller_id: node.caller_id.map(sf_core::CallerId::get),
        caller_login_identifier: caller_identity
            .as_ref()
            .map(|caller| caller.login_identifier.clone()),
        caller_lifecycle: caller_identity
            .as_ref()
            .map(|caller| caller.state.as_database_value().to_owned()),
        caller_name: node.caller_name.clone(),
        transport: node.transport.map(transport_name).map(str::to_owned),
        connected_at: node.connected_at,
        activity_file: node.activity_file.clone(),
        presentation: node
            .presentation
            .as_ref()
            .map(|presentation| PublishedPresentationStatus {
                terminal_type: presentation.terminal_type.clone(),
                ansi: presentation.ansi,
                encoding: presentation.encoding.clone(),
                columns: presentation.columns,
                rows: presentation.rows,
                page_length: presentation.page_length,
                locale: presentation.locale.clone(),
                presentation_profile: presentation.presentation_profile.clone(),
                menu_mode: presentation.menu_mode.clone(),
                menu_context: presentation.menu_context.clone(),
                renderer_path: presentation
                    .renderer_path
                    .map(|renderer| renderer.as_str().to_owned()),
                caller_security: presentation.caller_security,
                sysop_threshold: presentation.sysop_threshold,
                visible_action_count: presentation.visible_action_count,
            }),
    }
}

fn adapter_name(adapter: &TransportAdapterConfig) -> &'static str {
    match adapter {
        TransportAdapterConfig::Telnet { .. } => "telnet",
        TransportAdapterConfig::Raw { .. } => "raw",
        TransportAdapterConfig::Rlogin { .. } => "rlogin",
        TransportAdapterConfig::Ssh { .. } => "ssh",
        TransportAdapterConfig::Serial { .. } => "serial",
        TransportAdapterConfig::Modem { .. } => "modem",
    }
}

fn transport_name(transport: TransportKind) -> &'static str {
    match transport {
        TransportKind::InMemory => "in-memory",
        TransportKind::Telnet => "telnet",
        TransportKind::RawTcp => "raw",
        TransportKind::Rlogin => "rlogin",
        TransportKind::UnixShell => "shell",
        TransportKind::Ssh => "ssh",
        TransportKind::DirectSerial => "serial",
        TransportKind::HayesModem => "modem",
    }
}

fn unix_seconds() -> Result<i64, ApplicationError> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApplicationError::Coordination("system clock is before the Unix epoch"))?
        .as_secs();
    i64::try_from(seconds)
        .map_err(|_| ApplicationError::Coordination("system clock value is too large"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{setup_board, SetupPlan};
    use sf_core::SessionStatusObserver as _;

    #[test]
    fn reports_offline_and_published_runtime_state_without_secrets() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        let mut plan = SetupPlan::stock_defaults("Status Board", "Status Sysop", "Sysop", 2);
        plan.config.caller.password.memory_kib = 8;
        plan.config.caller.password.iterations = 1;
        let validated = plan.config.clone().validate().unwrap();
        let report = setup_board(&root, &plan, b"test-only status password").unwrap();
        let offline = board_status(&report.config_path).unwrap();
        assert!(offline.contains("Runtime: offline"));
        assert!(offline.contains("Active: modern-ng 1.5.0"));
        assert!(offline.contains("Public information: directory=false"));
        assert!(offline.contains("Effective: active profile modern-ng"));
        assert!(offline.contains("Status: ready"));
        assert!(offline.contains("Menu presentation: display-overrides"));
        assert!(offline.contains("Post-login journey: none"));
        assert!(offline.contains("New-caller security: 10"));
        assert!(offline.contains("Sysop security threshold: 50"));
        assert!(offline.contains("SSH host key:"));
        assert!(offline.contains("fingerprint=not generated"));
        assert!(offline.contains("Node   1 offline"));
        assert!(!offline.contains("test-only status password"));

        let manager = sf_core::NodeManager::new(validated.nodes.clone()).unwrap();
        let lease = manager
            .acquire(
                sf_core::SessionId::new(1).unwrap(),
                sf_core::TransportKind::Telnet,
                1_700_000_000,
            )
            .unwrap();
        lease
            .mark_online(sf_core::CallerId::new(7).unwrap(), "Synthetic Caller")
            .unwrap();
        lease
            .presentation_changed(sf_core::NodePresentationContext {
                terminal_type: Some("ANSI".to_owned()),
                ansi: true,
                encoding: "cp437".to_owned(),
                columns: Some(80),
                rows: Some(25),
                page_length: Some(24),
                locale: "en-US".to_owned(),
                presentation_profile: "modern-ng".to_owned(),
                menu_mode: "generated".to_owned(),
                menu_context: Some("main".to_owned()),
                renderer_path: Some(sf_core::MenuRendererPath::GeneratedStock),
                caller_security: Some(10),
                sysop_threshold: 50,
                visible_action_count: Some(11),
            })
            .unwrap();
        let status_path = root.join("WORK").join(RUNTIME_STATUS_FILE);
        publish_runtime_status(
            &status_path,
            "Status Board",
            1_700_000_000,
            &validated.transports,
            &report.database_path,
            &manager.snapshots().unwrap(),
        )
        .unwrap();
        let live = board_status(&report.config_path).unwrap();
        assert!(live.contains("terminal=ANSI ansi=true encoding=cp437 size=80x25 page=24"));
        assert!(live.contains("locale=en-US profile=modern-ng menu-mode=generated"));
        assert!(live.contains("renderer=generated-stock"));
        assert!(live.contains(
            "context=main renderer=generated-stock security=10 sysop-threshold=50 actions=11"
        ));
        assert!(!live.contains("test-only status password"));
    }
}
