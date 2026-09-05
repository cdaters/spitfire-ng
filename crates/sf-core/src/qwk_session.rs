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

//! Historical Messages → L offline-mail workflow on native transfer engines.
use crate::file_session::{choose_transfer_protocol, SelectedProtocol};
use crate::network::{NetworkError, QwkSelection, SubmissionIntent};
use crate::{
    AuthenticatedCaller, CallerConfig, ConferenceId, LocalizationArgs, MessageActor,
    MessageBackend, RuntimeDatabase, Session, SessionError, StockSessionContext, Terminal,
    TransferDirection, TransferPreference,
};

fn say(t: &mut dyn Terminal, key: &str) -> Result<(), SessionError> {
    write(t, key, &LocalizationArgs::new())?;
    t.write_all(b"\r\n")?;
    Ok(())
}
fn prompt(t: &mut dyn Terminal, key: &str) -> Result<Option<Vec<u8>>, SessionError> {
    write(t, key, &LocalizationArgs::new())?;
    Ok(t.read_line(32)?)
}
fn key(input: &[u8]) -> u8 {
    input
        .first()
        .copied()
        .unwrap_or_default()
        .to_ascii_uppercase()
}
fn error(t: &mut dyn Terminal, e: &NetworkError) -> Result<(), SessionError> {
    say(
        t,
        match e {
            NetworkError::NoMessages => "qwk-no-new",
            NetworkError::Stale => "qwk-stale",
            NetworkError::Codec(sf_net::qwk::Error::Unrepresentable) => "qwk-unrepresentable",
            NetworkError::Codec(_) => "qwk-malformed",
            NetworkError::Capacity => "qwk-limit",
            _ => "qwk-unavailable",
        },
    )
}
fn protocol(
    t: &mut dyn Terminal,
    preference: TransferPreference,
) -> Result<Option<crate::TransferProtocol>, SessionError> {
    let p = if preference == TransferPreference::Ascii {
        TransferPreference::Select
    } else {
        preference
    };
    match choose_transfer_protocol(t, p)? {
        SelectedProtocol::Binary(p) => Ok(Some(p)),
        SelectedProtocol::Ascii => {
            say(t, "qwk-binary-only")?;
            Ok(None)
        }
        SelectedProtocol::Canceled => Ok(None),
    }
}
fn scope(
    t: &mut dyn Terminal,
    db: &RuntimeDatabase,
    actor: MessageActor,
) -> Result<Option<Vec<ConferenceId>>, SessionError> {
    let Some(input) = prompt(t, "qwk-scope")? else {
        return Ok(None);
    };
    let cs = match key(&input) {
        b'A' => db.conferences(actor)?,
        b'Y' => db.queued_conferences(actor)?,
        b'S' => {
            let Some(n) = prompt(t, "qwk-conference")? else {
                return Ok(None);
            };
            let Some(n) = std::str::from_utf8(&n)
                .ok()
                .and_then(|s| s.trim().parse::<u16>().ok())
            else {
                return Ok(None);
            };
            match db.conference(actor, n) {
                Ok(c) => vec![c],
                Err(_) => {
                    say(t, "qwk-unavailable")?;
                    return Ok(None);
                }
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(cs.into_iter().map(|c| c.id).collect()))
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    t: &mut dyn Terminal,
    db: &mut RuntimeDatabase,
    session: &Session,
    stock: &StockSessionContext<'_>,
    authenticated: &AuthenticatedCaller,
    config: &CallerConfig,
) -> Result<(), SessionError> {
    let Some(board) = config.qwk_board_id.as_deref() else {
        return say(t, "qwk-unavailable");
    };
    let actor = crate::message_session::message_actor(authenticated, config)?;
    let store = stock.network_artifacts;
    loop {
        say(t, "qwk-title")?;
        let Some(command) = prompt(t, "qwk-menu")? else {
            return Ok(());
        };
        db.conferences(actor)?;
        match key(&command) {
            b'Q' => return Ok(()),
            b'?' => say(t, "qwk-help")?,
            b'S' => {
                let Some(cs) = scope(t, db, actor)? else {
                    continue;
                };
                for c in cs {
                    let old = match db.offline_pointer(actor, c) {
                        Ok(p) => p,
                        Err(e) => {
                            error(t, &e)?;
                            break;
                        }
                    };
                    write(
                        t,
                        "qwk-pointer-current",
                        &LocalizationArgs::new().with("pointer", old.0).with(
                            "conference",
                            db.conferences(actor)?
                                .into_iter()
                                .find(|item| item.id == c)
                                .ok_or(NetworkError::Unavailable)?
                                .name,
                        ),
                    )?;
                    t.write_all(b"\r\n")?;
                    let Some(input) = prompt(t, "qwk-pointer-new")? else {
                        return Ok(());
                    };
                    if let Some(n) = std::str::from_utf8(&input)
                        .ok()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                    {
                        match db.reset_offline_pointer(actor, c, old.1, n) {
                            Ok(()) => say(t, "qwk-pointers-updated")?,
                            Err(e) => error(t, &e)?,
                        }
                    }
                }
            }
            b'D' => {
                let Some(input) = prompt(t, "qwk-selection")? else {
                    return Ok(());
                };
                let selection = match key(&input) {
                    b'N' => QwkSelection::New,
                    b'T' => QwkSelection::ToYou,
                    _ => continue,
                };
                let Some(cs) = scope(t, db, actor)? else {
                    continue;
                };
                let now = crate::session::unix_seconds()?;
                let packet = match db.prepare_offline_packet(
                    actor,
                    board,
                    stock.board.name(),
                    selection,
                    &cs,
                    stock.timezone,
                    store,
                    now,
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        db.qwk_event(
                            actor,
                            "qwk.generation-failed",
                            crate::EventOutcome::Failed,
                            now,
                        )
                        .map_err(SessionError::Network)?;
                        error(t, &e)?;
                        continue;
                    }
                };
                say(t, "qwk-prepared")?;
                let Some(p) = protocol(t, authenticated.caller.preferences.transfer_protocol)?
                else {
                    db.cancel_offline_packet(actor, &packet.request_id)
                        .map_err(SessionError::Network)?;
                    continue;
                };
                let id = db
                    .begin_packet_transfer(
                        actor,
                        session.node_id(),
                        session.id().get() as i64,
                        p,
                        Some(&packet),
                        crate::session::unix_seconds()?,
                    )
                    .map_err(SessionError::Network)?;
                let name = format!("{board}.QWK");
                stock
                    .status
                    .transfer_started(TransferDirection::Download, &name)?;
                let result = crate::send_binary_files(
                    t,
                    p,
                    &[crate::ProtocolFile {
                        name,
                        bytes: packet.bytes.clone(),
                        modified_unix: None,
                    }],
                );
                let status = stock.status.transfer_finished();
                db.finish_packet_transfer(
                    actor,
                    &id,
                    result.is_ok() && status.is_ok(),
                    if result.is_ok() {
                        packet.bytes.len()
                    } else {
                        0
                    },
                    crate::session::unix_seconds()?,
                )
                .map_err(SessionError::Network)?;
                status?;
                if result.is_err() {
                    say(t, "qwk-transfer-failed")?;
                    continue;
                }
                {
                    let Some(input) = prompt(t, "qwk-confirm-pointers")? else {
                        return Ok(());
                    };
                    match db.confirm_offline_delivery(
                        actor,
                        &packet.request_id,
                        key(&input) == b'Y',
                    ) {
                        Ok(()) => {
                            say(
                                t,
                                if key(&input) == b'Y' {
                                    "qwk-pointers-updated"
                                } else {
                                    "qwk-preview"
                                },
                            )?;
                        }
                        Err(NetworkError::Stale) => {
                            say(t, "qwk-stale")?;
                        }
                        Err(e) => {
                            error(t, &e)?;
                        }
                    }
                }
            }
            b'U' => {
                let Some(input) = prompt(t, "qwk-import-intent")? else {
                    return Ok(());
                };
                let intent = match key(&input) {
                    b'R' => SubmissionIntent::Retry,
                    b'N' => {
                        let Some(confirm) = prompt(t, "qwk-confirm-new")? else {
                            return Ok(());
                        };
                        if key(&confirm) != b'Y' {
                            continue;
                        }
                        SubmissionIntent::New(format!("{:032x}", rand::random::<u128>()))
                    }
                    _ => continue,
                };
                let Some(p) = protocol(t, authenticated.caller.preferences.transfer_protocol)?
                else {
                    continue;
                };
                let id = db
                    .begin_packet_transfer(
                        actor,
                        session.node_id(),
                        session.id().get() as i64,
                        p,
                        None,
                        crate::session::unix_seconds()?,
                    )
                    .map_err(SessionError::Network)?;
                stock
                    .status
                    .transfer_started(TransferDirection::Upload, &format!("{board}.REP"))?;
                let result = crate::receive_binary_files(
                    t,
                    p,
                    &format!("{board}.REP"),
                    (sf_net::qwk::MAX_ARCHIVE + 1023) as u64,
                    1,
                );
                let status = stock.status.transfer_finished();
                let count = result
                    .as_ref()
                    .map_or(0, |v| v.iter().map(|f| f.bytes.len()).sum());
                db.finish_packet_transfer(
                    actor,
                    &id,
                    result.is_ok() && status.is_ok(),
                    count,
                    crate::session::unix_seconds()?,
                )
                .map_err(SessionError::Network)?;
                status?;
                let files = match result {
                    Ok(f) if f.len() == 1 => f,
                    _ => {
                        say(t, "qwk-transfer-failed")?;
                        continue;
                    }
                };
                say(t, "qwk-received")?;
                let now = crate::session::unix_seconds()?;
                db.qwk_event(
                    actor,
                    "qwk.reply-uploaded",
                    crate::EventOutcome::Succeeded,
                    now,
                )
                .map_err(SessionError::Network)?;
                match db.import_offline_replies(actor, board, &files[0].bytes, store, &intent, now)
                {
                    Ok(s) => {
                        db.attach_reply_artifact(actor, &id, &files[0].bytes)
                            .map_err(SessionError::Network)?;
                        write(
                            t,
                            "qwk-import-summary",
                            &LocalizationArgs::new()
                                .with("imported", s.imported as u64)
                                .with("duplicates", s.duplicates as u64)
                                .with("rejected", s.rejected as u64)
                                .with("controls", s.controls as u64)
                                .with("held", s.possible_duplicates as u64),
                        )?;
                        t.write_all(b"\r\n")?;
                    }
                    Err(e) => {
                        db.qwk_event(
                            actor,
                            "qwk.packet-rejected",
                            crate::EventOutcome::Denied,
                            now,
                        )
                        .map_err(SessionError::Network)?;
                        error(t, &e)?;
                    }
                }
            }
            _ => say(t, "qwk-help")?,
        }
    }
}

fn write(t: &mut dyn Terminal, key: &str, args: &LocalizationArgs) -> Result<(), SessionError> {
    let bytes = crate::localized_bytes(&t.info(), key, args);
    t.write_all(&bytes)?;
    Ok(())
}
