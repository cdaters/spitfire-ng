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

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sf_bbs::{
    EventBatchWire, MutationResult, OperatorClient, OperatorControlError, OperatorEventQuery,
};

use crate::model::{MonitorSnapshot, MONITOR_FEATURES, NOTIFICATION_LIMIT, RECENT_CALLER_LIMIT};

const UPDATE_QUEUE_CAPACITY: usize = 64;
const COMMAND_QUEUE_CAPACITY: usize = 16;
const LIVE_WAIT_MS: u64 = 1_000;
const SNAPSHOT_REFRESH: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub enum WorkerCommand {
    LiveControl {
        command_id: String,
        action: sf_bbs::LiveControlAction,
    },
    ChatLine(String),
    EndChat,
    Refresh(OperatorEventQuery),
    Reconnect(OperatorEventQuery),
    Stop,
    AcknowledgeNotification {
        command_id: String,
        notification_id: u64,
        expected_version: u64,
    },
    AdjustSessionTime {
        command_id: String,
        node_id: u32,
        session_id: u64,
        occupancy_generation: u64,
        delta_minutes: i16,
    },
}

impl std::fmt::Debug for WorkerCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WorkerCommand { payload: redacted }")
    }
}

#[derive(Clone, Debug)]
pub enum WorkerUpdate {
    ChatSendResult(bool),
    Chat(sf_bbs::ChatServerFrame),
    ChatEnded(&'static str),
    Uncertain(String),
    Connected {
        daemon_generation: String,
        features: Vec<sf_bbs::OperatorFeature>,
    },
    Snapshot(Box<MonitorSnapshot>),
    Events(EventBatchWire),
    Disconnected {
        reason_key: &'static str,
    },
    MutationResult(MutationResult),
    MutationDenied,
}

pub struct MonitorWorker {
    commands: SyncSender<WorkerCommand>,
    updates: Receiver<WorkerUpdate>,
    dropped_updates: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MonitorWorker {
    #[cfg(test)]
    pub fn test_channels() -> (Self, Receiver<WorkerCommand>) {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_QUEUE_CAPACITY);
        let (_, updates) = mpsc::sync_channel(UPDATE_QUEUE_CAPACITY);
        (
            Self {
                commands,
                updates,
                dropped_updates: Arc::new(AtomicBool::new(false)),
                handle: None,
            },
            receiver,
        )
    }

    pub fn start(config_path: PathBuf, query: OperatorEventQuery) -> Self {
        let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_QUEUE_CAPACITY);
        let (update_tx, update_rx) = mpsc::sync_channel(UPDATE_QUEUE_CAPACITY);
        let dropped_updates = Arc::new(AtomicBool::new(false));
        let worker_gap = Arc::clone(&dropped_updates);
        let handle = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            match runtime {
                Ok(runtime) => runtime.block_on(worker_loop(
                    config_path,
                    query,
                    command_rx,
                    update_tx,
                    worker_gap,
                )),
                Err(_) => {
                    let _ = update_tx.try_send(WorkerUpdate::Disconnected {
                        reason_key: "operator-client-start-failed",
                    });
                }
            }
        });
        Self {
            commands: command_tx,
            updates: update_rx,
            dropped_updates,
            handle: Some(handle),
        }
    }

    pub fn send(&self, command: WorkerCommand) -> bool {
        self.commands.try_send(command).is_ok()
    }

    pub fn drain_updates(&self) -> Vec<WorkerUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.updates.try_recv() {
            updates.push(update);
        }
        updates
    }

    pub fn take_transport_gap(&self) -> bool {
        self.dropped_updates.swap(false, Ordering::SeqCst)
    }

    pub fn stop(mut self) {
        let _ = self.commands.send(WorkerCommand::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MonitorWorker {
    fn drop(&mut self) {
        let _ = self.commands.try_send(WorkerCommand::Stop);
    }
}

async fn worker_loop(
    config_path: PathBuf,
    mut query: OperatorEventQuery,
    commands: Receiver<WorkerCommand>,
    updates: SyncSender<WorkerUpdate>,
    dropped_updates: Arc<AtomicBool>,
) {
    let mut reconnect = true;
    let mut receipts: Vec<String> = Vec::new();
    loop {
        if !reconnect {
            match commands.recv_timeout(Duration::from_millis(100)) {
                Ok(WorkerCommand::Reconnect(new_query)) | Ok(WorkerCommand::Refresh(new_query)) => {
                    query = new_query;
                }
                Ok(WorkerCommand::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(WorkerCommand::AcknowledgeNotification { .. })
                | Ok(WorkerCommand::AdjustSessionTime { .. })
                | Ok(WorkerCommand::LiveControl { .. })
                | Ok(WorkerCommand::ChatLine(_))
                | Ok(WorkerCommand::EndChat) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
            }
        }

        let connected = connect_pair(&config_path).await;
        let (mut snapshot_client, mut live_client) = match connected {
            Ok(pair) => pair,
            Err(error) => {
                send_update(
                    &updates,
                    WorkerUpdate::Disconnected {
                        reason_key: error_key(&error),
                    },
                    &dropped_updates,
                );
                reconnect = false;
                continue;
            }
        };
        let generation = snapshot_client.daemon_generation().to_owned();
        let features = snapshot_client.features().to_vec();
        if !MONITOR_FEATURES
            .iter()
            .all(|feature| features.contains(feature))
        {
            send_update(
                &updates,
                WorkerUpdate::Disconnected {
                    reason_key: "operator-feature-unsupported",
                },
                &dropped_updates,
            );
            reconnect = false;
            continue;
        }
        send_update(
            &updates,
            WorkerUpdate::Connected {
                daemon_generation: generation,
                features,
            },
            &dropped_updates,
        );
        match load_snapshot(&mut snapshot_client, query.clone()).await {
            Ok(snapshot) => send_update(
                &updates,
                WorkerUpdate::Snapshot(Box::new(snapshot)),
                &dropped_updates,
            ),
            Err(error) => {
                send_update(
                    &updates,
                    WorkerUpdate::Disconnected {
                        reason_key: error_key(&error),
                    },
                    &dropped_updates,
                );
                reconnect = false;
                continue;
            }
        }

        let mut last_refresh = Instant::now();
        let mut chat: Option<sf_bbs::OperatorChatClient> = None;
        reconnect = false;
        'connected: loop {
            loop {
                match commands.try_recv() {
                    Ok(WorkerCommand::LiveControl { command_id, action }) => {
                        let preflight = matches!(
                            action,
                            sf_bbs::LiveControlAction::PrepareDisconnect { .. }
                                | sf_bbs::LiveControlAction::PrepareGracefulShutdown { .. }
                        );
                        if !preflight && receipts.len() >= COMMAND_QUEUE_CAPACITY {
                            send_update(
                                &updates,
                                WorkerUpdate::MutationResult(MutationResult::Rejected {
                                    command_id,
                                    reason: "control-busy".to_owned(),
                                }),
                                &dropped_updates,
                            );
                            continue;
                        }
                        match snapshot_client
                            .live_control(command_id.clone(), action)
                            .await
                        {
                            Ok(result) => {
                                let mut chat_failed = false;
                                if let MutationResult::LiveControl {
                                    value: sf_bbs::LiveControlResult::ChatReady { join_token, .. },
                                    ..
                                } = &result
                                {
                                    match sf_bbs::OperatorChatClient::connect(
                                        &config_path,
                                        join_token.clone(),
                                    )
                                    .await
                                    {
                                        Ok(client) => chat = Some(client),
                                        Err(_) => chat_failed = true,
                                    }
                                }
                                if matches!(
                                    result,
                                    MutationResult::LiveControl {
                                        value: sf_bbs::LiveControlResult::Pending { .. },
                                        ..
                                    }
                                ) && !receipts.contains(&command_id)
                                {
                                    receipts.push(command_id);
                                }
                                send_update(
                                    &updates,
                                    WorkerUpdate::MutationResult(result),
                                    &dropped_updates,
                                );
                                if chat_failed {
                                    send_update(
                                        &updates,
                                        WorkerUpdate::ChatEnded("sfmonitor-chat-connection-lost"),
                                        &dropped_updates,
                                    );
                                }
                            }
                            Err(OperatorControlError::AuthorizationDenied) => send_update(
                                &updates,
                                WorkerUpdate::MutationDenied,
                                &dropped_updates,
                            ),
                            Err(_) => {
                                if !preflight
                                    && receipts.len() < COMMAND_QUEUE_CAPACITY
                                    && !receipts.contains(&command_id)
                                {
                                    receipts.push(command_id.clone());
                                }
                                send_update(
                                    &updates,
                                    WorkerUpdate::Uncertain(command_id),
                                    &dropped_updates,
                                );
                                send_update(
                                    &updates,
                                    WorkerUpdate::Disconnected {
                                        reason_key: "sfmonitor-disconnected",
                                    },
                                    &dropped_updates,
                                );
                                break 'connected;
                            }
                        }
                    }
                    Ok(WorkerCommand::ChatLine(line)) => {
                        if let Some(client) = chat.as_mut() {
                            match client.exchange(Some(line), false).await {
                                Ok(frame) => {
                                    send_update(
                                        &updates,
                                        WorkerUpdate::ChatSendResult(frame.state == "chat-started"),
                                        &dropped_updates,
                                    );
                                    send_update(
                                        &updates,
                                        WorkerUpdate::Chat(frame),
                                        &dropped_updates,
                                    );
                                }
                                Err(_) => {
                                    chat = None;
                                    send_update(
                                        &updates,
                                        WorkerUpdate::ChatEnded("sfmonitor-chat-connection-lost"),
                                        &dropped_updates,
                                    );
                                }
                            }
                        }
                    }
                    Ok(WorkerCommand::EndChat) => {
                        if let Some(mut client) = chat.take() {
                            let _ = client.exchange(None, true).await;
                        }
                        send_update(
                            &updates,
                            WorkerUpdate::ChatEnded("sfmonitor-chat-ended"),
                            &dropped_updates,
                        );
                    }
                    Ok(WorkerCommand::Stop) | Err(TryRecvError::Disconnected) => return,
                    Ok(WorkerCommand::Reconnect(new_query)) => {
                        query = new_query;
                        reconnect = true;
                        break 'connected;
                    }
                    Ok(WorkerCommand::Refresh(new_query)) => {
                        query = new_query;
                        match load_snapshot(&mut snapshot_client, query.clone()).await {
                            Ok(snapshot) => send_update(
                                &updates,
                                WorkerUpdate::Snapshot(Box::new(snapshot)),
                                &dropped_updates,
                            ),
                            Err(error) => {
                                send_update(
                                    &updates,
                                    WorkerUpdate::Disconnected {
                                        reason_key: error_key(&error),
                                    },
                                    &dropped_updates,
                                );
                                break 'connected;
                            }
                        }
                        last_refresh = Instant::now();
                    }
                    Ok(WorkerCommand::AcknowledgeNotification {
                        command_id,
                        notification_id,
                        expected_version,
                    }) => {
                        match snapshot_client
                            .acknowledge_notification(command_id, notification_id, expected_version)
                            .await
                        {
                            Ok(result) => send_update(
                                &updates,
                                WorkerUpdate::MutationResult(result),
                                &dropped_updates,
                            ),
                            Err(OperatorControlError::AuthorizationDenied) => send_update(
                                &updates,
                                WorkerUpdate::MutationDenied,
                                &dropped_updates,
                            ),
                            Err(error) => send_update(
                                &updates,
                                WorkerUpdate::Disconnected {
                                    reason_key: error_key(&error),
                                },
                                &dropped_updates,
                            ),
                        }
                    }
                    Ok(WorkerCommand::AdjustSessionTime {
                        command_id,
                        node_id,
                        session_id,
                        occupancy_generation,
                        delta_minutes,
                    }) => {
                        let preflight = snapshot_client
                            .prepare_session_time_adjustment(
                                command_id.clone(),
                                node_id,
                                session_id,
                                occupancy_generation,
                                delta_minutes,
                            )
                            .await;
                        match preflight {
                            Ok(sf_bbs::MutationResult::Preflight {
                                valid: true,
                                preflight_token,
                                ..
                            }) => {
                                match snapshot_client
                                    .adjust_session_time(
                                        command_id,
                                        node_id,
                                        session_id,
                                        occupancy_generation,
                                        delta_minutes,
                                        preflight_token,
                                    )
                                    .await
                                {
                                    Ok(result) => send_update(
                                        &updates,
                                        WorkerUpdate::MutationResult(result),
                                        &dropped_updates,
                                    ),
                                    Err(OperatorControlError::AuthorizationDenied) => send_update(
                                        &updates,
                                        WorkerUpdate::MutationDenied,
                                        &dropped_updates,
                                    ),
                                    Err(error) => send_update(
                                        &updates,
                                        WorkerUpdate::Disconnected {
                                            reason_key: error_key(&error),
                                        },
                                        &dropped_updates,
                                    ),
                                }
                            }
                            Ok(result) => send_update(
                                &updates,
                                WorkerUpdate::MutationResult(result),
                                &dropped_updates,
                            ),
                            Err(OperatorControlError::AuthorizationDenied) => send_update(
                                &updates,
                                WorkerUpdate::MutationDenied,
                                &dropped_updates,
                            ),
                            Err(error) => send_update(
                                &updates,
                                WorkerUpdate::Disconnected {
                                    reason_key: error_key(&error),
                                },
                                &dropped_updates,
                            ),
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }

            if dropped_updates.load(Ordering::SeqCst) {
                // Losing an ephemeral stream update cannot leave an unseen
                // conversation running behind the monitor's gap indicator.
                chat = None;
            }
            if let Some(client) = chat.as_mut() {
                match client.exchange(None, false).await {
                    Ok(frame) => {
                        if !matches!(
                            frame.state.as_str(),
                            "chat-started" | "chat-invited" | "chat-busy"
                        ) {
                            chat = None;
                        }
                        send_update(&updates, WorkerUpdate::Chat(frame), &dropped_updates);
                    }
                    Err(_) => {
                        chat = None;
                        send_update(
                            &updates,
                            WorkerUpdate::ChatEnded("sfmonitor-chat-connection-lost"),
                            &dropped_updates,
                        );
                    }
                }
            }
            if !receipts.is_empty() {
                let pending = std::mem::take(&mut receipts);
                for command_id in pending {
                    match snapshot_client.command_result(command_id.clone()).await {
                        Ok(MutationResult::Receipt { receipt }) if receipt.state != "accepted" => {
                            send_update(
                                &updates,
                                WorkerUpdate::MutationResult(MutationResult::Receipt { receipt }),
                                &dropped_updates,
                            )
                        }
                        _ => receipts.push(command_id),
                    }
                }
            }
            match live_client
                .subscribe_events(if chat.is_some() { 100 } else { LIVE_WAIT_MS })
                .await
            {
                Ok(batch) if !batch.events.is_empty() || batch.gap_before_first => {
                    send_update(&updates, WorkerUpdate::Events(batch), &dropped_updates)
                }
                Ok(_) => {}
                Err(error) => {
                    send_update(
                        &updates,
                        WorkerUpdate::Disconnected {
                            reason_key: error_key(&error),
                        },
                        &dropped_updates,
                    );
                    break;
                }
            }

            if last_refresh.elapsed() >= SNAPSHOT_REFRESH {
                match load_snapshot(&mut snapshot_client, query.clone()).await {
                    Ok(snapshot) => send_update(
                        &updates,
                        WorkerUpdate::Snapshot(Box::new(snapshot)),
                        &dropped_updates,
                    ),
                    Err(error) => {
                        send_update(
                            &updates,
                            WorkerUpdate::Disconnected {
                                reason_key: error_key(&error),
                            },
                            &dropped_updates,
                        );
                        break;
                    }
                }
                last_refresh = Instant::now();
            }
        }
        send_update(
            &updates,
            WorkerUpdate::ChatEnded("sfmonitor-chat-connection-lost"),
            &dropped_updates,
        );
    }
}

async fn connect_pair(
    config_path: &std::path::Path,
) -> Result<(OperatorClient, OperatorClient), OperatorControlError> {
    let snapshot = OperatorClient::connect(config_path).await?;
    let live = OperatorClient::connect(config_path).await?;
    Ok((snapshot, live))
}

async fn load_snapshot(
    client: &mut OperatorClient,
    query: OperatorEventQuery,
) -> Result<MonitorSnapshot, OperatorControlError> {
    let authorized_capabilities =
        if client.supports_mutation(sf_bbs::OperatorFeature::MutationReceipts) {
            client
                .describe_operator_controls()
                .await?
                .authorized_capabilities
        } else {
            Vec::new()
        };
    let board = client.board_status().await?;
    let nodes = client.nodes().await?;
    let events = client.query_events(query).await?.events;
    let notifications = client.notifications(false, NOTIFICATION_LIMIT).await?;
    let statistics = client.statistics().await?;
    let callers = client.recent_callers(RECENT_CALLER_LIMIT).await?;
    let maintenance = client.maintenance_status().await?;
    Ok(MonitorSnapshot {
        shutdown: if client.supports_mutation(sf_bbs::OperatorFeature::GracefulShutdown) {
            Some(client.shutdown_status().await?)
        } else {
            None
        },
        interactions: if client.supports_mutation(sf_bbs::OperatorFeature::CallerPages) {
            Some(client.live_interactions().await?)
        } else {
            None
        },
        authorized_capabilities,
        board: Some(board),
        nodes,
        events,
        notifications,
        statistics: Some(statistics),
        callers,
        maintenance: Some(maintenance),
    })
}

fn send_update(
    sender: &SyncSender<WorkerUpdate>,
    update: WorkerUpdate,
    dropped_updates: &AtomicBool,
) {
    match sender.try_send(update) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => dropped_updates.store(true, Ordering::SeqCst),
        Err(TrySendError::Disconnected(_)) => {}
    }
}

pub fn error_key(error: &OperatorControlError) -> &'static str {
    match error {
        OperatorControlError::EndpointUnavailable => "operator-endpoint-unavailable",
        OperatorControlError::UnsafeEndpoint(_) => "operator-endpoint-unsafe",
        OperatorControlError::AuthenticationFailed => "operator-authentication-failed",
        OperatorControlError::AuthorizationDenied => "operator-authorization-failed",
        OperatorControlError::ProtocolMismatch => "operator-protocol-mismatch",
        OperatorControlError::UnsupportedFeature => "operator-feature-unsupported",
        OperatorControlError::Timeout => "operator-request-timeout",
        OperatorControlError::StaleDaemonGeneration => "operator-daemon-restarted",
        OperatorControlError::Conflict | OperatorControlError::InvalidCommand => {
            "operator-request-failed"
        }
        OperatorControlError::PeerIdentityUnavailable => "operator-peer-identity-unavailable",
        OperatorControlError::InvalidWindowsSid => "operator-windows-sid-invalid",
        OperatorControlError::PipeSecurityUnavailable => "operator-pipe-security-failed",
        OperatorControlError::MalformedFrame | OperatorControlError::OversizedFrame => {
            "sfmonitor-error-protocol"
        }
        OperatorControlError::Io(_)
        | OperatorControlError::Serialization(_)
        | OperatorControlError::PlatformUnavailable
        | OperatorControlError::Service(_) => "operator-request-failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_errors_do_not_expose_internal_details() {
        assert_eq!(
            error_key(&OperatorControlError::Service("private detail".to_owned())),
            "operator-request-failed"
        );
        assert_eq!(
            error_key(&OperatorControlError::AuthorizationDenied),
            "operator-authorization-failed"
        );
    }
}
