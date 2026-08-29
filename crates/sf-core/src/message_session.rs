use tracing::info;

use crate::{
    render_display, render_generated_menu, AuthenticatedCaller, CallerConfig, CopyRecipient,
    DisplayContext, Message, MessageActor, MessageBackend, MessageCallerSearchDirection,
    MessageDeliveryRole, MessageDiscoveryQuery, MessageDiscoveryResult, MessageError, MessageKind,
    MessageLifecycle, MessageVisibility, NewMessage, SecurityLevel, SessionError, StockResources,
    Terminal, TerminalError, MAX_MESSAGE_BODY_BYTES, MAX_MESSAGE_CC_RECIPIENTS,
    MAX_MESSAGE_SEARCH_TERMS, MAX_MESSAGE_SEARCH_TERM_BYTES, MAX_MESSAGE_SUBJECT_BYTES,
};
use crate::{Conference, MenuSection, MessageId, MessageRecipient, MessageSummary};

const MAX_MENU_COMMAND_BYTES: usize = 8;
const MAX_CALLER_NAME_INPUT: usize = 64;
const MAX_MESSAGE_NUMBER_INPUT: usize = 20;
const MAX_EDITOR_LINE_BYTES: usize = 1024;
const MAX_MESSAGE_SEARCH_INPUT_BYTES: usize =
    MAX_MESSAGE_SEARCH_TERMS * MAX_MESSAGE_SEARCH_TERM_BYTES + (MAX_MESSAGE_SEARCH_TERMS - 1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MessageMenuExit {
    Main,
    File,
    Sysop,
    Goodbye,
    EndOfInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MessageMenuResult {
    pub exit: MessageMenuExit,
    pub commands: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComposeOutcome {
    Saved,
    Cancelled,
    Disconnected,
}

pub(crate) fn run_message_menu(
    resources: &StockResources,
    context: &DisplayContext<'_>,
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    authenticated: &AuthenticatedCaller,
    caller_config: &CallerConfig,
    expert: &mut bool,
) -> Result<MessageMenuResult, SessionError> {
    let actor = message_actor(authenticated, caller_config)?;
    let named_sysop = authenticated
        .caller
        .display_name
        .eq_ignore_ascii_case(&caller_config.sysop_caller_name);
    let conferences = backend.conferences(actor)?;
    let Some(mut current) = conferences.first().cloned() else {
        write_line(
            terminal,
            "No message conferences are available at your security level.",
        )?;
        return Ok(MessageMenuResult {
            exit: MessageMenuExit::Main,
            commands: 0,
        });
    };
    let menu = resources.menu(MenuSection::Message)?;
    let mut commands = 0;
    loop {
        if !*expert {
            if let Some(display) = resources.menu_display(
                MenuSection::Message,
                authenticated.caller.security_level.get(),
            ) {
                render_display(terminal, display, context)?;
                ensure_line_ending(terminal, &display.bytes)?;
            } else {
                render_generated_menu(
                    terminal,
                    menu,
                    authenticated.caller.security_level,
                    SecurityLevel::new(caller_config.sysop_security)?,
                    &[],
                )?;
            }
        }
        write_line(
            terminal,
            &format!("Conference {}: {}", current.number, current.name),
        )?;
        write_key(
            terminal,
            MenuSection::Message.prompt_key(),
            &crate::LocalizationArgs::new(),
        )?;
        let Some(command) =
            crate::session::read_menu_command(terminal, authenticated.caller.preferences.hot_keys)?
        else {
            return Ok(MessageMenuResult {
                exit: MessageMenuExit::EndOfInput,
                commands,
            });
        };
        commands += 1;
        let Some(item) = menu.find(command, authenticated.caller.security_level.get()) else {
            write_key_line(
                terminal,
                "message-selection-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            continue;
        };
        match item.identifier {
            b'Z' => {
                if let Some(selected) = choose_conference(terminal, backend, actor)? {
                    current = selected;
                    crate::session::render_named_display(
                        terminal,
                        resources,
                        &format!("SFMSG{}", current.number),
                        context,
                    )?;
                }
            }
            b'J' => list_messages(terminal, backend, actor, &current)?,
            b'I' => {
                if select_and_read_messages(terminal, backend, actor, &current, named_sysop)?
                    == ComposeOutcome::Disconnected
                {
                    return Ok(MessageMenuResult {
                        exit: MessageMenuExit::EndOfInput,
                        commands,
                    });
                }
            }
            b'L' => {
                if compose_message(
                    terminal,
                    backend,
                    actor,
                    &current,
                    None,
                    None,
                    MessageVisibility::Public,
                    MessageKind::Standard,
                    None,
                    None,
                )? == ComposeOutcome::Disconnected
                {
                    return Ok(MessageMenuResult {
                        exit: MessageMenuExit::EndOfInput,
                        commands,
                    });
                }
            }
            b'G' => {
                if show_your_messages(terminal, backend, actor, named_sysop)?
                    == ComposeOutcome::Disconnected
                {
                    return Ok(MessageMenuResult {
                        exit: MessageMenuExit::EndOfInput,
                        commands,
                    });
                }
            }
            b'K' => alter_conference_queue(terminal, backend, actor)?,
            // Boards created before M040 used X for the queue entry. Preserve
            // that independently authored menu while restoring stock K/X
            // identifiers in newly generated resources.
            b'X' if item.command.eq_ignore_ascii_case(&b'A') => {
                alter_conference_queue(terminal, backend, actor)?
            }
            b'S' => {
                search_messages_by_caller(terminal, backend, actor, &current)?;
            }
            b'X' => {
                search_message_text(terminal, backend, actor, &current)?;
            }
            b'D' => {
                return Ok(MessageMenuResult {
                    exit: MessageMenuExit::File,
                    commands,
                });
            }
            b'C' => {
                return Ok(MessageMenuResult {
                    exit: MessageMenuExit::Main,
                    commands,
                });
            }
            b'R' => {
                let threshold = SecurityLevel::new(caller_config.sysop_security)?;
                if authenticated.caller.security_level.is_sysop(threshold) {
                    return Ok(MessageMenuResult {
                        exit: MessageMenuExit::Sysop,
                        commands,
                    });
                }
                write_line(
                    terminal,
                    "Sysop Utilities require the configured Sysop security threshold.",
                )?;
            }
            b'A' => {
                return Ok(MessageMenuResult {
                    exit: MessageMenuExit::Goodbye,
                    commands,
                });
            }
            b'B' => {
                *expert = !*expert;
                write_line(
                    terminal,
                    if *expert {
                        "Xpert command mode is ON."
                    } else {
                        "Xpert command mode is OFF."
                    },
                )?;
            }
            b'?' => crate::session::show_help(
                MenuSection::Message,
                menu,
                resources,
                terminal,
                authenticated.caller.security_level,
                authenticated.caller.preferences.hot_keys,
            )?,
            _ => write_line(
                terminal,
                "That message command is not available in this SPITFIRE NG capability set.",
            )?,
        }
    }
}

pub(crate) fn compose_sysop_comment(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    authenticated: &AuthenticatedCaller,
    caller_config: &CallerConfig,
) -> Result<ComposeOutcome, SessionError> {
    let actor = message_actor(authenticated, caller_config)?;
    let conference = match backend.conference(actor, 1) {
        Ok(conference) => conference,
        Err(MessageError::ConferenceNotFound(_) | MessageError::ConferenceAccessDenied(_)) => {
            write_key_line(
                terminal,
                "message-sysop-conference-unavailable",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        Err(error) => return Err(error.into()),
    };
    let recipient = match backend.recipient(caller_config.sysop_caller_name.as_bytes()) {
        Ok(recipient) => recipient,
        Err(MessageError::RecipientNotFound) => {
            write_line(
                terminal,
                "The configured SPITFIRE Sysop caller has not been initialized; comment not saved.",
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        Err(error) => return Err(error.into()),
    };
    compose_message(
        terminal,
        backend,
        actor,
        &conference,
        Some(recipient),
        None,
        MessageVisibility::Private,
        MessageKind::SysopComment,
        None,
        None,
    )
}

pub(crate) fn message_actor(
    authenticated: &AuthenticatedCaller,
    caller_config: &CallerConfig,
) -> Result<MessageActor, SessionError> {
    Ok(MessageActor::new(
        authenticated.caller.id,
        SecurityLevel::new(caller_config.sysop_security)?,
    ))
}

fn choose_conference(
    terminal: &mut dyn Terminal,
    backend: &dyn MessageBackend,
    actor: MessageActor,
) -> Result<Option<Conference>, SessionError> {
    let conferences = backend.conferences(actor)?;
    write_key_line(
        terminal,
        "message-conference-list-title",
        &crate::LocalizationArgs::new(),
    )?;
    for conference in &conferences {
        let last = backend.last_read(actor, conference.id)?;
        let unread = backend
            .messages(actor, conference.id)?
            .iter()
            .filter(|message| message.number > last)
            .count();
        write_line(
            terminal,
            &format!(
                "{:>3}  {:<20}  {:>3} new  {}",
                conference.number, conference.name, unread, conference.description
            ),
        )?;
    }
    write_key(
        terminal,
        "message-conference-number-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MESSAGE_NUMBER_INPUT)? else {
        return Ok(None);
    };
    if input.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let Some(number) = parse_u16(&input) else {
        write_key_line(
            terminal,
            "message-conference-number-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(None);
    };
    match backend.conference(actor, number) {
        Ok(conference) => Ok(Some(conference)),
        Err(MessageError::ConferenceNotFound(_) | MessageError::ConferenceAccessDenied(_)) => {
            write_key_line(
                terminal,
                "message-conference-unavailable",
                &crate::LocalizationArgs::new(),
            )?;
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}

fn list_messages(
    terminal: &mut dyn Terminal,
    backend: &dyn MessageBackend,
    actor: MessageActor,
    conference: &Conference,
) -> Result<(), SessionError> {
    terminal.begin_output();
    write_line(
        terminal,
        &format!(
            "\r\nMessages in Conference {} - {}",
            conference.number, conference.name
        ),
    )?;
    let messages = backend.messages(actor, conference.id)?;
    if messages.is_empty() {
        write_key_line(
            terminal,
            "message-none-available",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    let last_read = backend.last_read(actor, conference.id)?;
    for message in messages {
        let unread = if message.number > last_read { '*' } else { ' ' };
        let private = if message.visibility == MessageVisibility::Private {
            " PRIVATE"
        } else {
            ""
        };
        terminal.write_all(
            format!(
                "{unread}{:>5}  To: {:<20} From: {:<20}{private}\r\n        ",
                message.number, message.recipient_name, message.author_name
            )
            .as_bytes(),
        )?;
        write_cp437_line(terminal, &message.subject)?;
        if terminal.output_aborted() {
            break;
        }
    }
    Ok(())
}

fn select_and_read_messages(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    current: &Conference,
    named_sysop: bool,
) -> Result<ComposeOutcome, SessionError> {
    write_key_line(
        terminal,
        "message-scan-title",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-scan-this",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-scan-all",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-scan-queued",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-scan-change-queue",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-scan-quit",
        &crate::LocalizationArgs::new(),
    )?;
    write_key(
        terminal,
        "message-scan-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
        return Ok(ComposeOutcome::Disconnected);
    };
    let conferences = match first_command(&input) {
        Some(b'T') => vec![current.clone()],
        Some(b'A') => backend.conferences(actor)?,
        Some(b'O') => backend.queued_conferences(actor)?,
        Some(b'C') => {
            alter_conference_queue(terminal, backend, actor)?;
            return Ok(ComposeOutcome::Cancelled);
        }
        Some(b'Q') | None => return Ok(ComposeOutcome::Cancelled),
        _ => {
            write_key_line(
                terminal,
                "message-scan-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
    };
    let mark_received = if named_sysop {
        write_key(
            terminal,
            "message-preview-question",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
            return Ok(ComposeOutcome::Disconnected);
        };
        first_command(&input) != Some(b'Y')
    } else {
        true
    };
    read_conferences(terminal, backend, actor, &conferences, mark_received)
}

fn read_conferences(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    conferences: &[Conference],
    mark_received: bool,
) -> Result<ComposeOutcome, SessionError> {
    let mut scan = Vec::new();
    for conference in conferences {
        let last_read = backend.last_read(actor, conference.id)?;
        for summary in backend.messages(actor, conference.id)? {
            scan.push((conference.clone(), summary, last_read));
        }
    }
    if scan.is_empty() {
        write_key_line(
            terminal,
            "message-scan-empty",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(ComposeOutcome::Cancelled);
    }
    let mut index = scan
        .iter()
        .position(|(_, message, last_read)| message.number > *last_read)
        .unwrap_or(0);

    loop {
        let (conference, summary, _) = &scan[index];
        let number = summary.number;
        let message = match backend.message(actor, conference.id, number) {
            Ok(message) => message,
            Err(MessageError::MessageNotFound { .. } | MessageError::MessageAccessDenied) => {
                write_key_line(
                    terminal,
                    "message-no-longer-available",
                    &crate::LocalizationArgs::new(),
                )?;
                return Ok(ComposeOutcome::Cancelled);
            }
            Err(error) => return Err(error.into()),
        };
        display_message(terminal, conference, &message)?;
        if terminal.output_aborted() {
            return Ok(ComposeOutcome::Cancelled);
        }
        if mark_received {
            backend.mark_read(actor, conference.id, number)?;
        }
        let capabilities = backend.mutation_capabilities(actor, conference.id, number)?;
        let prompt_key = if capabilities.undelete {
            "message-read-prompt-deleted"
        } else if capabilities.toggle_visibility || capabilities.copy {
            "message-read-prompt-sysop"
        } else if capabilities.delete {
            "message-read-prompt-delete"
        } else {
            "message-read-prompt"
        };
        write_key(terminal, prompt_key, &crate::LocalizationArgs::new())?;
        let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
            return Ok(ComposeOutcome::Disconnected);
        };
        let command = first_command(&input);
        match command.unwrap_or(b'N') {
            b'D' if capabilities.delete => {
                write_key_line(
                    terminal,
                    "message-delete-wait",
                    &crate::LocalizationArgs::new().with("number", number),
                )?;
                match backend.delete_message(actor, conference.id, number, message.state_version) {
                    Ok(_) => {}
                    Err(MessageError::MutationConflict | MessageError::AlreadyDeleted) => {
                        write_key_line(
                            terminal,
                            "message-mutation-conflict",
                            &crate::LocalizationArgs::new(),
                        )?;
                        continue;
                    }
                    Err(MessageError::MutationDenied) => {
                        write_key_line(
                            terminal,
                            "message-mutation-denied",
                            &crate::LocalizationArgs::new(),
                        )?;
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                }
                write_key_line(
                    terminal,
                    "message-delete-complete",
                    &crate::LocalizationArgs::new().with("number", number),
                )?;
                if backend
                    .mutation_capabilities(actor, conference.id, number)
                    .map(|value| value.undelete)
                    .unwrap_or(false)
                {
                    // The loop reopens and redisplays the contextual deleted message.
                } else if index + 1 < scan.len() {
                    index += 1;
                } else {
                    return Ok(ComposeOutcome::Cancelled);
                }
            }
            b'U' if capabilities.undelete => {
                match backend.undelete_message(actor, conference.id, number, message.state_version)
                {
                    Ok(_) => {}
                    Err(MessageError::MutationConflict | MessageError::AlreadyActive) => {
                        write_key_line(
                            terminal,
                            "message-mutation-conflict",
                            &crate::LocalizationArgs::new(),
                        )?;
                        continue;
                    }
                    Err(MessageError::MutationDenied) => {
                        write_key_line(
                            terminal,
                            "message-mutation-denied",
                            &crate::LocalizationArgs::new(),
                        )?;
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                }
                write_key_line(
                    terminal,
                    "message-undelete-complete",
                    &crate::LocalizationArgs::new().with("number", number),
                )?;
            }
            b'P' if capabilities.toggle_visibility => {
                let address_all_callers = if message.visibility == MessageVisibility::Private {
                    write_key(
                        terminal,
                        "message-public-all-callers-prompt",
                        &crate::LocalizationArgs::new().with("number", number),
                    )?;
                    let Some(answer) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
                        return Ok(ComposeOutcome::Disconnected);
                    };
                    first_command(&answer) != Some(b'N')
                } else {
                    false
                };
                match backend.toggle_message_visibility(
                    actor,
                    conference.id,
                    number,
                    message.state_version,
                    address_all_callers,
                ) {
                    Ok(updated) => {
                        write_key_line(
                            terminal,
                            if updated.visibility == MessageVisibility::Public {
                                "message-now-public"
                            } else {
                                "message-now-private"
                            },
                            &crate::LocalizationArgs::new().with("number", number),
                        )?;
                    }
                    Err(MessageError::PrivateMessageNeedsRecipient) => write_key_line(
                        terminal,
                        "message-private-needs-recipient",
                        &crate::LocalizationArgs::new(),
                    )?,
                    Err(MessageError::MutationConflict) => write_key_line(
                        terminal,
                        "message-mutation-conflict",
                        &crate::LocalizationArgs::new(),
                    )?,
                    Err(MessageError::MutationDenied) => write_key_line(
                        terminal,
                        "message-mutation-denied",
                        &crate::LocalizationArgs::new(),
                    )?,
                    Err(error) => return Err(error.into()),
                }
            }
            b'C' if capabilities.copy => {
                if copy_message_interaction(terminal, backend, actor, conference, &message)?
                    == ComposeOutcome::Disconnected
                {
                    return Ok(ComposeOutcome::Disconnected);
                }
            }
            b'N' if index + 1 < scan.len() => index += 1,
            b'N' => write_key_line(
                terminal,
                "message-no-later",
                &crate::LocalizationArgs::new(),
            )?,
            b'P' | b'-' if index > 0 => index -= 1,
            b'P' | b'-' => write_key_line(
                terminal,
                "message-no-earlier",
                &crate::LocalizationArgs::new(),
            )?,
            b'R' => {
                let recipient = message.author_caller_id.map(|caller_id| MessageRecipient {
                    caller_id,
                    display_name: message.author_name.clone(),
                });
                write_key(
                    terminal,
                    "message-subject-change-question",
                    &crate::LocalizationArgs::new(),
                )?;
                let Some(change) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
                    return Ok(ComposeOutcome::Disconnected);
                };
                let reply_subject = if first_command(&change) == Some(b'Y') {
                    write_key(
                        terminal,
                        "message-subject-new-prompt",
                        &crate::LocalizationArgs::new(),
                    )?;
                    let Some(subject) = terminal.read_line(MAX_MESSAGE_SUBJECT_BYTES)? else {
                        return Ok(ComposeOutcome::Disconnected);
                    };
                    if subject.is_empty() {
                        message.subject.clone()
                    } else {
                        subject
                    }
                } else {
                    message.subject.clone()
                };
                if compose_message(
                    terminal,
                    backend,
                    actor,
                    conference,
                    recipient,
                    Some(reply_subject),
                    message.visibility,
                    MessageKind::Standard,
                    Some(message.id),
                    Some(message.clone()),
                )? == ComposeOutcome::Disconnected
                {
                    return Ok(ComposeOutcome::Disconnected);
                }
            }
            b'F' => follow_message_thread(
                terminal,
                backend,
                actor,
                conference,
                &message,
                mark_received,
            )?,
            b'Q' => return Ok(ComposeOutcome::Cancelled),
            _ => {
                if let Some(number) = parse_u64(&input) {
                    if let Some(found) =
                        scan.iter().position(|(candidate_conference, message, _)| {
                            candidate_conference.id == conference.id && message.number == number
                        })
                    {
                        index = found;
                    } else {
                        write_key_line(
                            terminal,
                            "message-conference-message-unavailable",
                            &crate::LocalizationArgs::new(),
                        )?;
                    }
                } else {
                    write_key_line(
                        terminal,
                        "message-read-selection-invalid",
                        &crate::LocalizationArgs::new(),
                    )?;
                }
            }
        }
    }
}

fn copy_message_interaction(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    source_conference: &Conference,
    source: &Message,
) -> Result<ComposeOutcome, SessionError> {
    let available = backend.conferences(actor)?;
    let maximum = available.iter().map(|item| item.number).max().unwrap_or(1);
    write_key(
        terminal,
        "message-copy-conference-prompt",
        &crate::LocalizationArgs::new().with("maximum", maximum),
    )?;
    let Some(destination_input) = terminal.read_line(MAX_MESSAGE_NUMBER_INPUT)? else {
        return Ok(ComposeOutcome::Disconnected);
    };
    let Some(destination_number) = parse_u16(&destination_input) else {
        write_key_line(
            terminal,
            "message-conference-number-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(ComposeOutcome::Cancelled);
    };
    if !available
        .iter()
        .any(|item| item.number == destination_number)
    {
        write_key_line(
            terminal,
            "message-conference-unavailable",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(ComposeOutcome::Cancelled);
    }
    write_key_line(
        terminal,
        "message-copy-current-recipient",
        &crate::LocalizationArgs::new().with("name", source.recipient_name.clone()),
    )?;
    write_key(
        terminal,
        "message-copy-change-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(change) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
        return Ok(ComposeOutcome::Disconnected);
    };
    let recipient = if first_command(&change) == Some(b'Y') {
        write_key_line(
            terminal,
            "message-copy-all-callers-help",
            &crate::LocalizationArgs::new(),
        )?;
        write_key(
            terminal,
            "message-copy-recipient-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_CALLER_NAME_INPUT)? else {
            return Ok(ComposeOutcome::Disconnected);
        };
        if input.iter().all(u8::is_ascii_whitespace) {
            CopyRecipient::AllCallers
        } else {
            match backend.recipient(&input) {
                Ok(value) => CopyRecipient::Caller(value),
                Err(MessageError::RecipientNotFound) => {
                    write_key_line(
                        terminal,
                        "message-recipient-unavailable",
                        &crate::LocalizationArgs::new(),
                    )?;
                    return Ok(ComposeOutcome::Cancelled);
                }
                Err(error) => return Err(error.into()),
            }
        }
    } else {
        CopyRecipient::Preserve
    };
    match backend.copy_message(
        actor,
        source_conference.id,
        source.number,
        source.state_version,
        destination_number,
        recipient,
        crate::session::unix_seconds()?,
    ) {
        Ok(_) => write_key_line(
            terminal,
            "message-copy-complete",
            &crate::LocalizationArgs::new(),
        )?,
        Err(MessageError::SelfRecipient) => {
            write_key_line(
                terminal,
                "message-self-recipient",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        Err(MessageError::RecipientConferenceNotQueued(number)) => {
            write_key_line(
                terminal,
                "message-recipient-conference-not-queued",
                &crate::LocalizationArgs::new().with("number", number),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        Err(MessageError::MutationConflict) => {
            write_key_line(
                terminal,
                "message-mutation-conflict",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        Err(MessageError::MutationDenied) => {
            write_key_line(
                terminal,
                "message-mutation-denied",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        Err(error) => return Err(error.into()),
    }
    Ok(ComposeOutcome::Saved)
}

fn follow_message_thread(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    conference: &Conference,
    original: &Message,
    mark_received: bool,
) -> Result<(), SessionError> {
    let thread = backend
        .messages(actor, conference.id)?
        .into_iter()
        .filter(|message| message.subject == original.subject)
        .collect::<Vec<_>>();
    if thread.len() < 2 {
        write_key_line(
            terminal,
            "message-thread-none",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    let mut index = thread
        .iter()
        .position(|message| message.id == original.id)
        .unwrap_or(0);
    loop {
        write_key_line(
            terminal,
            "message-thread-title",
            &crate::LocalizationArgs::new(),
        )?;
        write_key_line(
            terminal,
            "message-thread-forward-back",
            &crate::LocalizationArgs::new(),
        )?;
        write_key_line(
            terminal,
            "message-thread-back-exit",
            &crate::LocalizationArgs::new(),
        )?;
        write_key(
            terminal,
            "message-thread-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
            return Ok(());
        };
        match first_command(&input) {
            Some(b'S') => index = 0,
            Some(b'F') if index + 1 < thread.len() => index += 1,
            Some(b'F') => {
                write_key_line(
                    terminal,
                    "message-thread-end",
                    &crate::LocalizationArgs::new(),
                )?;
                continue;
            }
            Some(b'B') if index > 0 => index -= 1,
            Some(b'B') => {
                write_key_line(
                    terminal,
                    "message-thread-start",
                    &crate::LocalizationArgs::new(),
                )?;
                continue;
            }
            Some(b'E') | None => return Ok(()),
            _ => {
                write_key_line(
                    terminal,
                    "message-thread-invalid",
                    &crate::LocalizationArgs::new(),
                )?;
                continue;
            }
        }
        let message = backend.message(actor, conference.id, thread[index].number)?;
        display_message(terminal, conference, &message)?;
        if terminal.output_aborted() {
            return Ok(());
        }
        if mark_received {
            backend.mark_read(actor, conference.id, message.number)?;
        }
    }
}

fn alter_conference_queue(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
) -> Result<(), SessionError> {
    loop {
        terminal.begin_output();
        write_key_line(
            terminal,
            "message-queue-title",
            &crate::LocalizationArgs::new(),
        )?;
        write_line(
            terminal,
            "<A> Add A Conference       <C> Current Queue List",
        )?;
        write_line(
            terminal,
            "<D> Delete A Conference    <I> Include All Conferences",
        )?;
        write_line(
            terminal,
            "<L> List Msg Conferences   <R> Remove All Conferences",
        )?;
        write_key_line(
            terminal,
            "message-queue-quit",
            &crate::LocalizationArgs::new(),
        )?;
        write_key(
            terminal,
            "message-queue-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
            return Ok(());
        };
        match first_command(&input) {
            Some(b'A') => update_one_queue_conference(terminal, backend, actor, true)?,
            Some(b'D') => update_one_queue_conference(terminal, backend, actor, false)?,
            Some(b'C') => display_queue(terminal, backend, actor)?,
            Some(b'I') => {
                let all = backend
                    .conferences(actor)?
                    .into_iter()
                    .map(|conference| conference.number)
                    .collect::<Vec<_>>();
                backend.replace_queue(actor, &all)?;
                write_key_line(
                    terminal,
                    "message-queue-all",
                    &crate::LocalizationArgs::new(),
                )?;
            }
            Some(b'R') => {
                backend.replace_queue(actor, &[])?;
                write_line(
                    terminal,
                    "All optional conferences were removed; Conference 1 remains queued.",
                )?;
            }
            Some(b'L') => {
                write_key_line(
                    terminal,
                    "message-conference-list-title",
                    &crate::LocalizationArgs::new(),
                )?;
                for conference in backend.conferences(actor)? {
                    write_line(
                        terminal,
                        &format!(
                            "{:>3}  {:<20}  {}",
                            conference.number, conference.name, conference.description
                        ),
                    )?;
                    if terminal.output_aborted() {
                        break;
                    }
                }
                terminal.begin_output();
            }
            Some(b'Q') | None => return Ok(()),
            _ => write_key_line(
                terminal,
                "message-queue-invalid",
                &crate::LocalizationArgs::new(),
            )?,
        }
    }
}

fn search_messages_by_caller(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    current: &Conference,
) -> Result<(), SessionError> {
    let Some(conferences) = choose_discovery_conferences(terminal, backend, actor, current)? else {
        return Ok(());
    };
    write_key(
        terminal,
        "message-search-caller-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(caller_name) = terminal.read_line(MAX_CALLER_NAME_INPUT)? else {
        return Ok(());
    };
    if caller_name.is_empty() {
        return Ok(());
    }
    let recipient = match backend.recipient(&caller_name) {
        Ok(recipient) => recipient,
        Err(MessageError::RecipientNotFound) => {
            write_key_line(
                terminal,
                "message-search-caller-unavailable",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    write_key(
        terminal,
        "message-search-direction-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
        return Ok(());
    };
    let direction = match first_command(&input) {
        Some(b'F') => MessageCallerSearchDirection::From,
        Some(b'T') => MessageCallerSearchDirection::To,
        Some(b'B') => MessageCallerSearchDirection::Both,
        Some(b'Q') | None => return Ok(()),
        _ => {
            write_key_line(
                terminal,
                "message-search-direction-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
    };
    let conference_ids = conferences
        .iter()
        .map(|conference| conference.id)
        .collect::<Vec<_>>();
    let query = MessageDiscoveryQuery::SpecificCaller {
        caller_id: recipient.caller_id,
        direction,
    };
    let discovery = backend.discover_messages(actor, &conference_ids, &query)?;
    present_discovery(terminal, backend, actor, discovery)
}

fn search_message_text(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    current: &Conference,
) -> Result<(), SessionError> {
    let Some(conferences) = choose_discovery_conferences(terminal, backend, actor, current)? else {
        return Ok(());
    };
    write_key(
        terminal,
        "message-search-text-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_EDITOR_LINE_BYTES)? else {
        return Ok(());
    };
    let terms = input
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|term| !term.is_empty())
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    if input.len() > MAX_MESSAGE_SEARCH_INPUT_BYTES
        || terms.is_empty()
        || terms.len() > MAX_MESSAGE_SEARCH_TERMS
        || terms
            .iter()
            .any(|term| term.len() > MAX_MESSAGE_SEARCH_TERM_BYTES)
    {
        write_key_line(
            terminal,
            "message-search-text-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    let conference_ids = conferences
        .iter()
        .map(|conference| conference.id)
        .collect::<Vec<_>>();
    let discovery = backend.discover_messages(
        actor,
        &conference_ids,
        &MessageDiscoveryQuery::Text { terms },
    )?;
    present_discovery(terminal, backend, actor, discovery)
}

fn choose_discovery_conferences(
    terminal: &mut dyn Terminal,
    backend: &dyn MessageBackend,
    actor: MessageActor,
    current: &Conference,
) -> Result<Option<Vec<Conference>>, SessionError> {
    write_key_line(
        terminal,
        "message-search-scope-title",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-search-scope-this",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-search-scope-all",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-search-scope-queued",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "message-search-scope-quit",
        &crate::LocalizationArgs::new(),
    )?;
    write_key(
        terminal,
        "message-search-scope-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
        return Ok(None);
    };
    match first_command(&input) {
        Some(b'T') => Ok(Some(vec![current.clone()])),
        Some(b'A') => Ok(Some(backend.conferences(actor)?)),
        Some(b'O') => Ok(Some(backend.queued_conferences(actor)?)),
        Some(b'Q') | None => Ok(None),
        _ => {
            write_key_line(
                terminal,
                "message-search-scope-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            Ok(None)
        }
    }
}

fn present_discovery(
    terminal: &mut dyn Terminal,
    backend: &dyn MessageBackend,
    actor: MessageActor,
    discovery: MessageDiscoveryResult,
) -> Result<(), SessionError> {
    let mut displayed = 0_u64;
    for found in discovery.matches {
        let conference = match backend.conference(actor, found.conference_number) {
            Ok(conference) if conference.id == found.conference_id => conference,
            Ok(_)
            | Err(MessageError::ConferenceNotFound(_))
            | Err(MessageError::ConferenceAccessDenied(_)) => continue,
            Err(error) => return Err(error.into()),
        };
        let message = match backend.message(actor, conference.id, found.message_number) {
            Ok(message) => message,
            Err(MessageError::MessageNotFound { .. } | MessageError::MessageAccessDenied) => {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        display_message(terminal, &conference, &message)?;
        displayed += 1;
        if terminal.output_aborted() {
            break;
        }
        write_key(
            terminal,
            "message-search-continue-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
            break;
        };
        if first_command(&input) == Some(b'Q') {
            break;
        }
    }
    write_key_line(
        terminal,
        "message-search-result-count",
        &crate::LocalizationArgs::new().with("count", displayed),
    )?;
    if discovery.truncated {
        write_key_line(
            terminal,
            "message-search-truncated",
            &crate::LocalizationArgs::new(),
        )?;
    }
    Ok(())
}

fn update_one_queue_conference(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    add: bool,
) -> Result<(), SessionError> {
    terminal.write_all(if add {
        b"Conference Number To Add: "
    } else {
        b"Conference Number To Delete: "
    })?;
    let Some(input) = terminal.read_line(MAX_MESSAGE_NUMBER_INPUT)? else {
        return Ok(());
    };
    let Some(number) = parse_u16(&input) else {
        write_key_line(
            terminal,
            "message-conference-number-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    };
    if !add && number == 1 {
        write_line(
            terminal,
            "Conference 1 is required in every caller's queue.",
        )?;
        return Ok(());
    }
    let mut numbers = backend
        .queued_conferences(actor)?
        .into_iter()
        .map(|conference| conference.number)
        .collect::<Vec<_>>();
    if add {
        if !numbers.contains(&number) {
            numbers.push(number);
        }
    } else {
        numbers.retain(|queued| *queued != number);
    }
    match backend.replace_queue(actor, &numbers) {
        Ok(_) => write_line(
            terminal,
            if add {
                "Conference added to your message queue."
            } else {
                "Conference removed from your message queue."
            },
        )?,
        Err(MessageError::ConferenceNotFound(_) | MessageError::ConferenceAccessDenied(_)) => {
            write_key_line(
                terminal,
                "message-conference-unavailable",
                &crate::LocalizationArgs::new(),
            )?
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn display_queue(
    terminal: &mut dyn Terminal,
    backend: &dyn MessageBackend,
    actor: MessageActor,
) -> Result<(), SessionError> {
    terminal.begin_output();
    write_key_line(
        terminal,
        "message-queue-current-title",
        &crate::LocalizationArgs::new(),
    )?;
    for conference in backend.queued_conferences(actor)? {
        write_line(
            terminal,
            &format!("{:>3}  {}", conference.number, conference.name),
        )?;
        if terminal.output_aborted() {
            break;
        }
    }
    terminal.begin_output();
    Ok(())
}

fn display_message(
    terminal: &mut dyn Terminal,
    conference: &Conference,
    message: &Message,
) -> Result<(), TerminalError> {
    terminal.begin_output();
    let label = if message.kind == MessageKind::SysopComment {
        "Comment"
    } else {
        "Message"
    };
    write_line(
        terminal,
        &format!(
            "\r\n--- {label} {} / Conference {} {} ---",
            message.number, conference.number, conference.name
        ),
    )?;
    if message.delivery_role == MessageDeliveryRole::CarbonCopy {
        write_key_line(
            terminal,
            "message-field-carbon-copy",
            &crate::LocalizationArgs::new(),
        )?;
    }
    write_key_line(
        terminal,
        "message-field-to",
        &crate::LocalizationArgs::new().with("name", message.recipient_name.clone()),
    )?;
    if message.delivery_role == MessageDeliveryRole::CarbonCopy {
        if let Some(primary) = message.primary_recipient_name.as_ref() {
            write_key_line(
                terminal,
                "message-field-primary-recipient",
                &crate::LocalizationArgs::new().with("name", primary.clone()),
            )?;
        }
    }
    write_key_line(
        terminal,
        "message-field-from",
        &crate::LocalizationArgs::new().with("name", message.author_name.clone()),
    )?;
    write_key(
        terminal,
        "message-field-subject",
        &crate::LocalizationArgs::new(),
    )?;
    write_cp437_line(terminal, &message.subject)?;
    write_line(
        terminal,
        &format!("Date/Time: {}", format_timestamp_utc(message.created_at)),
    )?;
    write_key_line(
        terminal,
        if message.visibility == MessageVisibility::Private {
            "message-field-private"
        } else {
            "message-field-public"
        },
        &crate::LocalizationArgs::new(),
    )?;
    if message.lifecycle == MessageLifecycle::Deleted {
        write_key_line(
            terminal,
            "message-field-deleted",
            &crate::LocalizationArgs::new(),
        )?;
    }
    if message.received {
        write_key_line(
            terminal,
            "message-field-received",
            &crate::LocalizationArgs::new(),
        )?;
    }
    terminal.write_all(b"\r\n")?;
    terminal.write_all(&message.body)?;
    ensure_line_ending(terminal, &message.body)
}

#[allow(clippy::too_many_arguments)]
fn compose_message(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    conference: &Conference,
    fixed_recipient: Option<MessageRecipient>,
    default_subject: Option<Vec<u8>>,
    default_visibility: MessageVisibility,
    kind: MessageKind,
    parent_message_id: Option<MessageId>,
    quote_source: Option<Message>,
) -> Result<ComposeOutcome, SessionError> {
    let recipient = if let Some(recipient) = fixed_recipient {
        Some(recipient)
    } else {
        write_key(
            terminal,
            "message-compose-to-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_CALLER_NAME_INPUT)? else {
            return Ok(ComposeOutcome::Disconnected);
        };
        if input.eq_ignore_ascii_case(b"/A") {
            write_key_line(
                terminal,
                "message-compose-canceled",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        if input.iter().all(u8::is_ascii_whitespace) {
            None
        } else {
            match backend.recipient(&input) {
                Ok(recipient) if recipient.caller_id == actor.caller_id() => {
                    write_key_line(
                        terminal,
                        "message-self-recipient",
                        &crate::LocalizationArgs::new(),
                    )?;
                    return Ok(ComposeOutcome::Cancelled);
                }
                Ok(recipient) => Some(recipient),
                Err(MessageError::RecipientNotFound) => {
                    write_line(
                        terminal,
                        "That local caller does not exist or is unavailable.",
                    )?;
                    return Ok(ComposeOutcome::Cancelled);
                }
                Err(error) => return Err(error.into()),
            }
        }
    };

    let mut cc_recipients = Vec::new();
    if kind == MessageKind::Standard && recipient.is_some() {
        while cc_recipients.len() < MAX_MESSAGE_CC_RECIPIENTS {
            let ordinal = cc_recipients.len() + 1;
            write_key(
                terminal,
                "message-cc-prompt",
                &crate::LocalizationArgs::new().with(
                    "ordinal",
                    u64::try_from(ordinal).map_err(|_| MessageError::MessageNumberOverflow)?,
                ),
            )?;
            let Some(input) = terminal.read_line(MAX_CALLER_NAME_INPUT)? else {
                return Ok(ComposeOutcome::Disconnected);
            };
            if input.iter().all(u8::is_ascii_whitespace) {
                break;
            }
            let candidate = match backend.recipient(&input) {
                Ok(value) => value,
                Err(MessageError::RecipientNotFound) => {
                    write_key_line(
                        terminal,
                        "message-recipient-unavailable",
                        &crate::LocalizationArgs::new(),
                    )?;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            if candidate.caller_id == actor.caller_id() {
                write_key_line(
                    terminal,
                    "message-self-recipient",
                    &crate::LocalizationArgs::new(),
                )?;
                continue;
            }
            if recipient
                .as_ref()
                .is_some_and(|primary| primary.caller_id == candidate.caller_id)
                || cc_recipients
                    .iter()
                    .any(|existing: &MessageRecipient| existing.caller_id == candidate.caller_id)
            {
                write_key_line(
                    terminal,
                    "message-duplicate-recipient",
                    &crate::LocalizationArgs::new().with("name", candidate.display_name),
                )?;
                continue;
            }
            cc_recipients.push(candidate);
        }
    }

    let visibility = if kind == MessageKind::SysopComment {
        MessageVisibility::Private
    } else if recipient.is_some() && !conference.public_only {
        terminal.write_all(if default_visibility == MessageVisibility::Private {
            b"Make this message non-public/private? [Y/n]: "
        } else {
            b"Make this message non-public/private? [y/N]: "
        })?;
        let Some(input) = terminal.read_line(8)? else {
            return Ok(ComposeOutcome::Disconnected);
        };
        match first_command(&input) {
            Some(b'Y') => MessageVisibility::Private,
            Some(b'N') => MessageVisibility::Public,
            _ => default_visibility,
        }
    } else {
        MessageVisibility::Public
    };

    let subject = if let Some(default) = default_subject {
        default
    } else {
        write_key(
            terminal,
            "message-compose-subject-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_MESSAGE_SUBJECT_BYTES)? else {
            return Ok(ComposeOutcome::Disconnected);
        };
        if input.eq_ignore_ascii_case(b"/A") || input.is_empty() {
            write_key_line(
                terminal,
                "message-compose-canceled",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        input
    };

    let body = match edit_message(terminal, conference, quote_source.as_ref())? {
        EditorOutcome::Body(body) => body,
        EditorOutcome::Cancelled => {
            write_key_line(
                terminal,
                "message-compose-canceled",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        EditorOutcome::Disconnected => return Ok(ComposeOutcome::Disconnected),
    };
    write_key(
        terminal,
        "message-compose-save-question",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(answer) = terminal.read_line(8)? else {
        return Ok(ComposeOutcome::Disconnected);
    };
    if first_command(&answer) != Some(b'Y') {
        write_key_line(
            terminal,
            "message-compose-canceled",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(ComposeOutcome::Cancelled);
    }

    let (recipient_caller_id, recipient_name) = recipient.map_or_else(
        || (None, "All Callers".to_owned()),
        |recipient| (Some(recipient.caller_id), recipient.display_name),
    );
    let stored = match backend.post_with_cc(
        actor,
        NewMessage {
            conference_id: conference.id,
            recipient_caller_id,
            recipient_name,
            subject,
            body,
            created_at: crate::session::unix_seconds()?,
            parent_message_id,
            visibility,
            kind,
        },
        &cc_recipients,
    ) {
        Ok(messages) => messages
            .into_iter()
            .next()
            .ok_or(MessageError::MutationInvariant)?,
        Err(MessageError::RecipientConferenceNotQueued(number)) => {
            write_line(
                terminal,
                &format!(
                    "That caller does not have Conference {number} in their message queue; nothing was saved."
                ),
            )?;
            return Ok(ComposeOutcome::Cancelled);
        }
        Err(error) => return Err(error.into()),
    };
    write_line(
        terminal,
        &format!(
            "Message {} was saved in Conference {}.",
            stored.number, conference.number
        ),
    )?;
    info!(
        caller_id = actor.caller_id().get(),
        conference = conference.number,
        message_number = stored.number,
        private = stored.visibility == MessageVisibility::Private,
        "caller posted message"
    );
    Ok(ComposeOutcome::Saved)
}

fn show_your_messages(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    named_sysop: bool,
) -> Result<ComposeOutcome, SessionError> {
    let stats = backend.stats(actor)?;
    terminal.begin_output();
    write_key_line(
        terminal,
        "message-yours-title",
        &crate::LocalizationArgs::new(),
    )?;
    write_line(
        terminal,
        &format!("New Messages Waiting: {}", stats.new_waiting),
    )?;
    write_line(
        terminal,
        &format!("Messages Already Received: {}", stats.already_received),
    )?;
    write_line(terminal, &format!("Messages Sent: {}", stats.sent))?;
    write_line(
        terminal,
        &format!("Total Messages Available: {}", stats.total_available),
    )?;
    loop {
        write_key(
            terminal,
            "message-yours-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
            return Ok(ComposeOutcome::Disconnected);
        };
        match first_command(&input) {
            Some(b'R') => {
                let mark_received = if named_sysop {
                    write_key(
                        terminal,
                        "message-preview-question",
                        &crate::LocalizationArgs::new(),
                    )?;
                    let Some(preview) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
                        return Ok(ComposeOutcome::Disconnected);
                    };
                    first_command(&preview) != Some(b'Y')
                } else {
                    true
                };
                show_personal_message_list(terminal, backend, actor, true, mark_received)?
            }
            Some(b'S') => show_personal_message_list(terminal, backend, actor, false, false)?,
            Some(b'Q') | None => return Ok(ComposeOutcome::Cancelled),
            _ => write_key_line(
                terminal,
                "message-yours-invalid",
                &crate::LocalizationArgs::new(),
            )?,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EditorLine {
    bytes: Vec<u8>,
    quoted: bool,
}

enum EditorOutcome {
    Body(Vec<u8>),
    Cancelled,
    Disconnected,
}

fn edit_message(
    terminal: &mut dyn Terminal,
    conference: &Conference,
    quote_source: Option<&Message>,
) -> Result<EditorOutcome, SessionError> {
    write_line(
        terminal,
        &format!(
            "Enter up to {} lines. Enter a blank line for editor commands; /S saves and /A aborts.",
            conference.maximum_lines
        ),
    )?;
    if quote_source.is_some() {
        write_key_line(
            terminal,
            "message-editor-quote-help",
            &crate::LocalizationArgs::new(),
        )?;
    }
    let mut lines = Vec::<EditorLine>::new();
    loop {
        terminal.write_all(format!("{:>2}> ", lines.len() + 1).as_bytes())?;
        let Some(line) = terminal.read_line(MAX_EDITOR_LINE_BYTES)? else {
            return Ok(EditorOutcome::Disconnected);
        };
        if line.eq_ignore_ascii_case(b"/A") {
            return Ok(EditorOutcome::Cancelled);
        }
        if line.eq_ignore_ascii_case(b"/S") {
            if let Some(body) = editor_body(&lines) {
                return Ok(EditorOutcome::Body(body));
            }
            write_key_line(
                terminal,
                "message-editor-empty",
                &crate::LocalizationArgs::new(),
            )?;
            continue;
        }
        if line == [0x11] {
            if let Some(source) = quote_source {
                quote_original(terminal, conference, &mut lines, source)?;
            } else {
                write_key_line(
                    terminal,
                    "message-editor-quote-unavailable",
                    &crate::LocalizationArgs::new(),
                )?;
            }
            continue;
        }
        if line.is_empty() {
            match editor_command_menu(terminal, conference, &mut lines)? {
                EditorCommandOutcome::Continue => continue,
                EditorCommandOutcome::Save => {
                    if let Some(body) = editor_body(&lines) {
                        return Ok(EditorOutcome::Body(body));
                    }
                    write_key_line(
                        terminal,
                        "message-editor-empty",
                        &crate::LocalizationArgs::new(),
                    )?;
                }
                EditorCommandOutcome::Abort => return Ok(EditorOutcome::Cancelled),
                EditorCommandOutcome::Disconnected => return Ok(EditorOutcome::Disconnected),
            }
            continue;
        }
        let candidate = EditorLine {
            bytes: line,
            quoted: false,
        };
        if !editor_fits(
            lines.iter().chain(std::iter::once(&candidate)),
            conference.maximum_lines,
        ) {
            write_line(
                terminal,
                "Message limit reached; use the editor command menu.",
            )?;
            continue;
        }
        lines.push(candidate);
    }
}

enum EditorCommandOutcome {
    Continue,
    Save,
    Abort,
    Disconnected,
}

fn editor_command_menu(
    terminal: &mut dyn Terminal,
    conference: &Conference,
    lines: &mut Vec<EditorLine>,
) -> Result<EditorCommandOutcome, SessionError> {
    terminal.begin_output();
    write_key_line(
        terminal,
        "message-editor-menu-one",
        &crate::LocalizationArgs::new(),
    )?;
    write_line(
        terminal,
        "<R>eplace Line <L>ist <I>nsert Line <D>elete Line(s)",
    )?;
    write_key(
        terminal,
        "message-editor-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MENU_COMMAND_BYTES)? else {
        return Ok(EditorCommandOutcome::Disconnected);
    };
    match first_command(&input) {
        Some(b'S') => Ok(EditorCommandOutcome::Save),
        Some(b'A') => Ok(EditorCommandOutcome::Abort),
        Some(b'C') | None => Ok(EditorCommandOutcome::Continue),
        Some(b'B') => {
            lines.clear();
            write_key_line(
                terminal,
                "message-editor-cleared",
                &crate::LocalizationArgs::new(),
            )?;
            Ok(EditorCommandOutcome::Continue)
        }
        Some(b'L') => {
            list_editor_lines(terminal, lines)?;
            Ok(EditorCommandOutcome::Continue)
        }
        Some(b'R') => {
            replace_editor_line(terminal, conference, lines)?;
            Ok(EditorCommandOutcome::Continue)
        }
        Some(b'E') => {
            edit_editor_lines(terminal, conference, lines)?;
            Ok(EditorCommandOutcome::Continue)
        }
        Some(b'I') => {
            insert_editor_line(terminal, conference, lines)?;
            Ok(EditorCommandOutcome::Continue)
        }
        Some(b'D') => {
            delete_editor_lines(terminal, lines)?;
            Ok(EditorCommandOutcome::Continue)
        }
        _ => {
            write_key_line(
                terminal,
                "message-editor-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            Ok(EditorCommandOutcome::Continue)
        }
    }
}

fn list_editor_lines(
    terminal: &mut dyn Terminal,
    lines: &[EditorLine],
) -> Result<(), TerminalError> {
    terminal.begin_output();
    if lines.is_empty() {
        return write_key_line(
            terminal,
            "message-editor-no-text",
            &crate::LocalizationArgs::new(),
        );
    }
    for (index, line) in lines.iter().enumerate() {
        terminal.write_all(format!("{:>2}: ", index + 1).as_bytes())?;
        write_cp437_line(terminal, &line.bytes)?;
        if terminal.output_aborted() {
            break;
        }
    }
    terminal.begin_output();
    Ok(())
}

fn replace_editor_line(
    terminal: &mut dyn Terminal,
    conference: &Conference,
    lines: &mut [EditorLine],
) -> Result<(), SessionError> {
    write_key(
        terminal,
        "message-editor-replace-line-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MESSAGE_NUMBER_INPUT)? else {
        return Ok(());
    };
    let Some(index) = parse_line_number(&input, lines.len()) else {
        write_key_line(
            terminal,
            "message-editor-line-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    };
    if lines[index].quoted {
        write_key_line(
            terminal,
            "message-editor-quoted-immutable",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    write_key(
        terminal,
        "message-editor-replacement-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(replacement) = terminal.read_line(MAX_EDITOR_LINE_BYTES)? else {
        return Ok(());
    };
    let old = std::mem::replace(&mut lines[index].bytes, replacement);
    if !editor_fits(lines.iter(), conference.maximum_lines) {
        lines[index].bytes = old;
        write_key_line(
            terminal,
            "message-editor-replacement-too-long",
            &crate::LocalizationArgs::new(),
        )?;
    }
    Ok(())
}

fn edit_editor_lines(
    terminal: &mut dyn Terminal,
    conference: &Conference,
    lines: &mut [EditorLine],
) -> Result<(), SessionError> {
    loop {
        write_key(
            terminal,
            "message-editor-edit-line-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(MAX_MESSAGE_NUMBER_INPUT)? else {
            return Ok(());
        };
        if input.is_empty() {
            return Ok(());
        }
        let Some(index) = parse_line_number(&input, lines.len()) else {
            write_key_line(
                terminal,
                "message-editor-line-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            continue;
        };
        if lines[index].quoted {
            write_key_line(
                terminal,
                "message-editor-quoted-immutable",
                &crate::LocalizationArgs::new(),
            )?;
            continue;
        }
        write_key(
            terminal,
            "message-editor-edited-text-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(replacement) = terminal.read_line(MAX_EDITOR_LINE_BYTES)? else {
            return Ok(());
        };
        let old = std::mem::replace(&mut lines[index].bytes, replacement);
        if !editor_fits(lines.iter(), conference.maximum_lines) {
            lines[index].bytes = old;
            write_key_line(
                terminal,
                "message-editor-edit-too-long",
                &crate::LocalizationArgs::new(),
            )?;
        }
    }
}

fn insert_editor_line(
    terminal: &mut dyn Terminal,
    conference: &Conference,
    lines: &mut Vec<EditorLine>,
) -> Result<(), SessionError> {
    if lines.len() >= usize::from(conference.maximum_lines) {
        write_key_line(
            terminal,
            "message-editor-limit-reached",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    write_key(
        terminal,
        "message-editor-insert-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MESSAGE_NUMBER_INPUT)? else {
        return Ok(());
    };
    let index = if input.is_empty() {
        lines.len()
    } else if let Some(number) = parse_u64(&input) {
        usize::try_from(number.saturating_sub(1))
            .ok()
            .filter(|index| *index <= lines.len())
            .unwrap_or(lines.len() + 1)
    } else {
        lines.len() + 1
    };
    if index > lines.len() {
        write_key_line(
            terminal,
            "message-editor-insert-invalid",
            &crate::LocalizationArgs::new(),
        )?;
    } else {
        lines.insert(
            index,
            EditorLine {
                bytes: Vec::new(),
                quoted: false,
            },
        );
    }
    Ok(())
}

fn delete_editor_lines(
    terminal: &mut dyn Terminal,
    lines: &mut Vec<EditorLine>,
) -> Result<(), SessionError> {
    write_key(
        terminal,
        "message-editor-delete-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MESSAGE_NUMBER_INPUT)? else {
        return Ok(());
    };
    let Some((start, end)) = parse_line_range(&input, lines.len()) else {
        write_key_line(
            terminal,
            "message-editor-range-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    };
    if lines[start..=end].iter().any(|line| line.quoted) {
        write_key_line(
            terminal,
            "message-editor-quoted-immutable",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    lines.drain(start..=end);
    Ok(())
}

fn quote_original(
    terminal: &mut dyn Terminal,
    conference: &Conference,
    lines: &mut Vec<EditorLine>,
    source: &Message,
) -> Result<(), SessionError> {
    let original = message_body_lines(&source.body);
    terminal.begin_output();
    write_key_line(
        terminal,
        "message-editor-quote-title",
        &crate::LocalizationArgs::new(),
    )?;
    for (index, line) in original.iter().enumerate() {
        terminal.write_all(format!("{:>2}: ", index + 1).as_bytes())?;
        write_cp437_line(terminal, line)?;
        if terminal.output_aborted() {
            terminal.begin_output();
            return Ok(());
        }
    }
    write_key(
        terminal,
        "message-editor-quote-range-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_MESSAGE_NUMBER_INPUT)? else {
        return Ok(());
    };
    if input.is_empty() {
        return Ok(());
    }
    let Some((start, end)) = parse_line_range(&input, original.len()) else {
        write_key_line(
            terminal,
            "message-editor-quote-range-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    };
    let prefix = quote_prefix(&source.author_name);
    let quoted = original[start..=end]
        .iter()
        .map(|line| {
            let mut bytes = prefix.clone();
            bytes.extend_from_slice(line);
            EditorLine {
                bytes,
                quoted: true,
            }
        })
        .collect::<Vec<_>>();
    if !editor_fits(lines.iter().chain(quoted.iter()), conference.maximum_lines) {
        write_key_line(
            terminal,
            "message-editor-quote-too-long",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    lines.extend(quoted);
    Ok(())
}

fn editor_fits<'a>(lines: impl Iterator<Item = &'a EditorLine>, maximum_lines: u16) -> bool {
    let mut count = 0usize;
    let mut bytes = 0usize;
    for line in lines {
        count += 1;
        bytes = bytes.saturating_add(line.bytes.len() + 2);
    }
    count <= usize::from(maximum_lines) && bytes <= MAX_MESSAGE_BODY_BYTES
}

fn editor_body(lines: &[EditorLine]) -> Option<Vec<u8>> {
    if lines.is_empty() {
        return None;
    }
    let mut body = Vec::new();
    for line in lines {
        body.extend_from_slice(&line.bytes);
        body.extend_from_slice(b"\r\n");
    }
    Some(body)
}

fn message_body_lines(body: &[u8]) -> Vec<&[u8]> {
    let mut lines = body.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    for line in &mut lines {
        if line.ends_with(b"\r") {
            *line = &line[..line.len() - 1];
        }
    }
    lines
}

fn quote_prefix(author: &str) -> Vec<u8> {
    let mut initials = author
        .split_ascii_whitespace()
        .filter_map(|part| part.as_bytes().first().copied())
        .filter(u8::is_ascii_alphanumeric)
        .map(|byte| byte.to_ascii_uppercase())
        .collect::<Vec<_>>();
    if initials.is_empty() {
        initials.push(b'?');
    }
    initials.extend_from_slice(b"> ");
    initials
}

fn parse_line_number(input: &[u8], line_count: usize) -> Option<usize> {
    let number = usize::try_from(parse_u64(input)?).ok()?;
    (1..=line_count).contains(&number).then_some(number - 1)
}

fn parse_line_range(input: &[u8], line_count: usize) -> Option<(usize, usize)> {
    let input = std::str::from_utf8(input).ok()?.trim();
    let (start, end) = input
        .split_once('-')
        .map_or((input, input), |(start, end)| (start.trim(), end.trim()));
    let start = start.parse::<usize>().ok()?;
    let end = end.parse::<usize>().ok()?;
    (start >= 1 && start <= end && end <= line_count).then_some((start - 1, end - 1))
}

fn show_personal_message_list(
    terminal: &mut dyn Terminal,
    backend: &mut dyn MessageBackend,
    actor: MessageActor,
    received: bool,
    mark_received: bool,
) -> Result<(), SessionError> {
    let mut personal = Vec::<(Conference, MessageSummary, bool)>::new();
    for conference in backend.conferences(actor)? {
        for message in backend.messages(actor, conference.id)? {
            let matches = if received {
                message.recipient_caller_id == Some(actor.caller_id())
            } else {
                message.author_caller_id == Some(actor.caller_id())
            };
            if matches {
                let was_received =
                    received && backend.received(actor, conference.id, message.number)?;
                personal.push((conference.clone(), message, was_received));
            }
        }
    }
    terminal.begin_output();
    write_line(
        terminal,
        if received {
            "\r\n--- YOUR MESSAGES RECEIVED ---"
        } else {
            "\r\n--- YOUR MESSAGES SENT ---"
        },
    )?;
    if personal.is_empty() {
        write_line(
            terminal,
            if received {
                "No messages have been addressed directly to you."
            } else {
                "You have not sent any available messages."
            },
        )?;
        return Ok(());
    }
    for (conference, message, was_received) in &personal {
        let status = if received {
            if *was_received {
                "RECEIVED"
            } else {
                "NEW"
            }
        } else {
            "SENT"
        };
        terminal.write_all(
            format!(
                "[{status:<8}] C{:>3}/M{:>5}  To: {:<20} From: {:<20}\r\n             ",
                conference.number, message.number, message.recipient_name, message.author_name
            )
            .as_bytes(),
        )?;
        write_cp437_line(terminal, &message.subject)?;
        if terminal.output_aborted() {
            terminal.begin_output();
            return Ok(());
        }
    }
    loop {
        write_key(
            terminal,
            "message-personal-read-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(input) = terminal.read_line(48)? else {
            return Ok(());
        };
        if input.is_empty() {
            return Ok(());
        }
        let Some((conference_number, message_number)) = parse_message_reference(&input) else {
            write_key_line(
                terminal,
                "message-personal-reference-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            continue;
        };
        let Some((conference, summary, _)) = personal.iter().find(|(conference, message, _)| {
            conference.number == conference_number && message.number == message_number
        }) else {
            write_key_line(
                terminal,
                "message-personal-unavailable",
                &crate::LocalizationArgs::new(),
            )?;
            continue;
        };
        let message = backend.message(actor, conference.id, summary.number)?;
        display_message(terminal, conference, &message)?;
        if received && mark_received {
            backend.mark_read(actor, conference.id, summary.number)?;
        }
    }
}

fn parse_message_reference(input: &[u8]) -> Option<(u16, u64)> {
    let input = std::str::from_utf8(input).ok()?.trim();
    let (conference, message) = input.split_once('/')?;
    Some((
        conference.trim().parse().ok()?,
        message.trim().parse().ok()?,
    ))
}

fn first_command(input: &[u8]) -> Option<u8> {
    input
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .map(|byte| byte.to_ascii_uppercase())
}

fn parse_u16(input: &[u8]) -> Option<u16> {
    std::str::from_utf8(input).ok()?.trim().parse().ok()
}

fn parse_u64(input: &[u8]) -> Option<u64> {
    std::str::from_utf8(input).ok()?.trim().parse().ok()
}

fn format_timestamp_utc(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let seconds = timestamp.rem_euclid(86_400);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;

    // Proleptic Gregorian conversion adapted from the public-domain civil
    // calendar arithmetic by Howard Hinnant. UTC is explicit until the board
    // timezone policy is implemented.
    let adjusted = days + 719_468;
    let era = if adjusted >= 0 {
        adjusted
    } else {
        adjusted - 146_096
    }
    .div_euclid(146_097);
    let day_of_era = adjusted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

fn write_cp437_line(terminal: &mut dyn Terminal, bytes: &[u8]) -> Result<(), TerminalError> {
    terminal.write_all(bytes)?;
    terminal.write_all(b"\r\n")
}

fn ensure_line_ending(terminal: &mut dyn Terminal, bytes: &[u8]) -> Result<(), TerminalError> {
    if !bytes.ends_with(b"\n") {
        terminal.write_all(b"\r\n")?;
    }
    Ok(())
}

fn write_line(terminal: &mut dyn Terminal, line: &str) -> Result<(), TerminalError> {
    terminal.write_all(line.as_bytes())?;
    terminal.write_all(b"\r\n")
}

fn write_key(
    terminal: &mut dyn Terminal,
    key: &str,
    arguments: &crate::LocalizationArgs,
) -> Result<(), TerminalError> {
    let bytes = crate::localized_bytes(&terminal.info(), key, arguments);
    terminal.write_all(&bytes)
}

fn write_key_line(
    terminal: &mut dyn Terminal,
    key: &str,
    arguments: &crate::LocalizationArgs,
) -> Result<(), TerminalError> {
    write_key(terminal, key, arguments)?;
    terminal.write_all(b"\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CallerState, ConferenceAccessMode, ConferenceDefinition, CredentialHasher,
        InMemoryTerminal, PasswordHashConfig, RuntimeDatabase,
    };

    fn message_database() -> (
        tempfile::TempDir,
        RuntimeDatabase,
        MessageActor,
        MessageActor,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let mut database = RuntimeDatabase::open(&temp.path().join("runtime.sqlite3")).unwrap();
        database.migrate().unwrap();
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher.hash(b"message session test password").unwrap();
        let alice = database
            .create_caller(
                b"Alice Caller",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        let bob = database
            .create_caller(
                b"Bob Caller",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        for (number, name) in [(1, "General"), (2, "SPITFIRE")] {
            database
                .ensure_conference(&ConferenceDefinition {
                    number,
                    name: name.to_owned(),
                    description: format!("{name} messages"),
                    access_mode: ConferenceAccessMode::AtLeast,
                    read_security: SecurityLevel::new(5).unwrap(),
                    post_security: SecurityLevel::new(5).unwrap(),
                    public_only: false,
                    caller_deletion_enabled: true,
                    maximum_lines: 50,
                    privileged_security_levels: Vec::new(),
                })
                .unwrap();
        }
        let sysop = SecurityLevel::new(100).unwrap();
        (
            temp,
            database,
            MessageActor::new(alice.id, sysop),
            MessageActor::new(bob.id, sysop),
        )
    }

    fn post_public(
        database: &mut RuntimeDatabase,
        actor: MessageActor,
        conference: &Conference,
        subject: &[u8],
    ) -> Message {
        database
            .post(
                actor,
                NewMessage {
                    conference_id: conference.id,
                    recipient_caller_id: None,
                    recipient_name: "All Callers".to_owned(),
                    subject: subject.to_vec(),
                    body: b"Synthetic message body\r\n".to_vec(),
                    created_at: 1,
                    parent_message_id: None,
                    visibility: MessageVisibility::Public,
                    kind: MessageKind::Standard,
                },
            )
            .unwrap()
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn b009_contextual_sysop_read_commands_redisplay_and_use_copy_for_forwarding() {
        let (_temp, mut database, alice, bob) = message_database();
        let hasher = CredentialHasher::new(&PasswordHashConfig {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap();
        let hash = hasher.hash(b"threshold session password").unwrap();
        let threshold_caller = database
            .create_caller(
                b"Threshold Caller",
                &hash,
                SecurityLevel::new(100).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        let forward_caller = database
            .create_caller(
                b"Forward Caller",
                &hash,
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                1,
            )
            .unwrap();
        let threshold = MessageActor::new(threshold_caller.id, SecurityLevel::new(100).unwrap());
        let conference = database.conference(alice, 1).unwrap();
        let source = NewMessage {
            conference_id: conference.id,
            recipient_caller_id: Some(bob.caller_id()),
            recipient_name: "Bob Caller".to_owned(),
            subject: b"M041 UI".to_vec(),
            body: b"Mutation UI body\r\n".to_vec(),
            created_at: 1,
            parent_message_id: None,
            visibility: MessageVisibility::Public,
            kind: MessageKind::Standard,
        };
        let source = database.post(alice, source).unwrap();
        let mut terminal = InMemoryTerminal::with_lines([
            b"P".to_vec(),
            b"P".to_vec(),
            b"N".to_vec(),
            b"C".to_vec(),
            b"1".to_vec(),
            b"Y".to_vec(),
            b"Forward Caller".to_vec(),
            b"D".to_vec(),
            b"U".to_vec(),
            b"Q".to_vec(),
        ]);
        assert_eq!(
            read_conferences(
                &mut terminal,
                &mut database,
                threshold,
                std::slice::from_ref(&conference),
                true,
            )
            .unwrap(),
            ComposeOutcome::Cancelled
        );
        assert!(contains(
            terminal.output(),
            b"Message number 1 is now non-public."
        ));
        assert!(contains(
            terminal.output(),
            b"Address Message #1 to \"All Callers\"? [Y/n]"
        ));
        assert!(contains(terminal.output(), b"Message Copied."));
        assert!(contains(terminal.output(), b"[U]ndelete"));
        let reopened = database
            .message(threshold, conference.id, source.number)
            .unwrap();
        assert_eq!(reopened.lifecycle, MessageLifecycle::Active);
        assert_eq!(reopened.visibility, MessageVisibility::Public);
        let forwarded = database
            .messages(threshold, conference.id)
            .unwrap()
            .into_iter()
            .find(|message| message.number != source.number)
            .unwrap();
        assert_eq!(forwarded.author_caller_id, Some(alice.caller_id()));
        assert_eq!(forwarded.recipient_caller_id, Some(forward_caller.id));
    }

    #[test]
    fn formats_stored_message_time_as_explicit_utc() {
        assert_eq!(format_timestamp_utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_timestamp_utc(1_700_000_000), "2023-11-14 22:13 UTC");
    }

    #[test]
    fn current_all_and_queued_scan_choices_select_the_documented_conferences() {
        let (_temp, mut database, alice, _bob) = message_database();
        let general = database.conference(alice, 1).unwrap();
        let spitfire = database.conference(alice, 2).unwrap();
        post_public(&mut database, alice, &general, b"General scan subject");
        post_public(&mut database, alice, &spitfire, b"SPITFIRE scan subject");

        let mut current = InMemoryTerminal::with_lines([b"T".to_vec(), b"Q".to_vec()]);
        select_and_read_messages(&mut current, &mut database, alice, &spitfire, false).unwrap();
        assert!(!contains(current.output(), b"General scan subject"));
        assert!(contains(current.output(), b"SPITFIRE scan subject"));

        let mut all = InMemoryTerminal::with_lines([b"A".to_vec(), b"N".to_vec(), b"Q".to_vec()]);
        select_and_read_messages(&mut all, &mut database, alice, &general, false).unwrap();
        assert!(contains(all.output(), b"General scan subject"));
        assert!(contains(all.output(), b"SPITFIRE scan subject"));

        let mut queued = InMemoryTerminal::with_lines([b"O".to_vec(), b"Q".to_vec()]);
        select_and_read_messages(&mut queued, &mut database, alice, &spitfire, false).unwrap();
        assert!(contains(queued.output(), b"General scan subject"));
        assert!(!contains(queued.output(), b"SPITFIRE scan subject"));
    }

    #[test]
    fn caller_and_text_discovery_journeys_use_scope_and_do_not_mutate_read_state() {
        let (_temp, mut database, alice, bob) = message_database();
        let general = database.conference(alice, 1).unwrap();
        let spitfire = database.conference(alice, 2).unwrap();
        let general_message = database
            .post(
                bob,
                NewMessage {
                    conference_id: general.id,
                    recipient_caller_id: None,
                    recipient_name: "All Callers".to_owned(),
                    subject: b"General discovery".to_vec(),
                    body: b"Exact needle \xDB\r\n".to_vec(),
                    created_at: 1,
                    parent_message_id: None,
                    visibility: MessageVisibility::Public,
                    kind: MessageKind::Standard,
                },
            )
            .unwrap();
        database
            .post(
                bob,
                NewMessage {
                    conference_id: spitfire.id,
                    recipient_caller_id: None,
                    recipient_name: "All Callers".to_owned(),
                    subject: b"Second discovery".to_vec(),
                    body: b"Exact needle second\r\n".to_vec(),
                    created_at: 2,
                    parent_message_id: None,
                    visibility: MessageVisibility::Public,
                    kind: MessageKind::Standard,
                },
            )
            .unwrap();

        let mut queued =
            InMemoryTerminal::with_lines([b"O".to_vec(), b"Exact needle".to_vec(), Vec::new()]);
        search_message_text(&mut queued, &mut database, alice, &spitfire).unwrap();
        assert!(contains(queued.output(), b"General discovery"));
        assert!(!contains(queued.output(), b"Second discovery"));
        assert!(contains(
            queued.output(),
            b"1 matching message was displayed"
        ));
        assert_eq!(database.last_read(alice, general.id).unwrap(), 0);
        assert!(!database
            .received(alice, general.id, general_message.number)
            .unwrap());

        let mut caller = InMemoryTerminal::with_lines([
            b"A".to_vec(),
            b"Bob Caller".to_vec(),
            b"F".to_vec(),
            Vec::new(),
            Vec::new(),
        ]);
        search_messages_by_caller(&mut caller, &mut database, alice, &general).unwrap();
        assert!(contains(caller.output(), b"General discovery"));
        assert!(contains(caller.output(), b"Second discovery"));
        assert!(contains(
            caller.output(),
            b"2 matching messages were displayed"
        ));
        assert_eq!(database.last_read(alice, spitfire.id).unwrap(), 0);

        let mut empty = InMemoryTerminal::with_lines([b"T".to_vec(), b"missing".to_vec()]);
        search_message_text(&mut empty, &mut database, alice, &general).unwrap();
        assert!(contains(
            empty.output(),
            b"0 matching messages were displayed"
        ));

        let mut malformed = InMemoryTerminal::with_lines([
            b"T".to_vec(),
            b"one two three four five six seven".to_vec(),
        ]);
        search_message_text(&mut malformed, &mut database, alice, &general).unwrap();
        assert!(contains(
            malformed.output(),
            b"Enter one to six terms of no more than 64 bytes each."
        ));

        let mut oversized = InMemoryTerminal::with_lines([
            b"T".to_vec(),
            vec![b'x'; MAX_MESSAGE_SEARCH_INPUT_BYTES + 1],
        ]);
        search_message_text(&mut oversized, &mut database, alice, &general).unwrap();
        assert!(contains(
            oversized.output(),
            b"Enter one to six terms of no more than 64 bytes each."
        ));
    }

    #[test]
    fn bounded_editor_commands_and_reply_quoting_preserve_cp437_bytes() {
        let conference = Conference {
            id: crate::ConferenceId::new(1).unwrap(),
            number: 1,
            name: "General".to_owned(),
            description: "General messages".to_owned(),
            access_mode: ConferenceAccessMode::AtLeast,
            read_security: SecurityLevel::new(5).unwrap(),
            post_security: SecurityLevel::new(5).unwrap(),
            public_only: false,
            caller_deletion_enabled: true,
            maximum_lines: 25,
            privileged_security_levels: Vec::new(),
            active: true,
        };
        let source = Message {
            id: MessageId::new(1).unwrap(),
            conference_id: conference.id,
            number: 1,
            author_caller_id: None,
            author_name: "Alice Caller".to_owned(),
            recipient_caller_id: None,
            recipient_name: "All Callers".to_owned(),
            subject: b"Original".to_vec(),
            body: b"Original \xDB line\r\nSecond line\r\n".to_vec(),
            created_at: 1,
            parent_message_id: None,
            visibility: MessageVisibility::Public,
            kind: MessageKind::Standard,
            lifecycle: crate::MessageLifecycle::Active,
            state_version: 1,
            delivery_role: crate::MessageDeliveryRole::Single,
            delivery_ordinal: 0,
            primary_recipient_name: None,
            received: false,
        };
        let mut terminal = InMemoryTerminal::with_lines([
            b"".to_vec(),
            b"C".to_vec(),
            b"temporary".to_vec(),
            b"".to_vec(),
            b"B".to_vec(),
            b"first".to_vec(),
            b"".to_vec(),
            b"L".to_vec(),
            b"".to_vec(),
            b"R".to_vec(),
            b"1".to_vec(),
            b"replaced".to_vec(),
            b"".to_vec(),
            b"I".to_vec(),
            b"1".to_vec(),
            b"".to_vec(),
            b"D".to_vec(),
            b"1".to_vec(),
            b"".to_vec(),
            b"E".to_vec(),
            b"1".to_vec(),
            b"edited".to_vec(),
            b"".to_vec(),
            vec![0x11],
            b"1-1".to_vec(),
            b"".to_vec(),
            b"R".to_vec(),
            b"2".to_vec(),
            b"".to_vec(),
            b"S".to_vec(),
        ]);
        let EditorOutcome::Body(body) =
            edit_message(&mut terminal, &conference, Some(&source)).unwrap()
        else {
            panic!("editor did not save")
        };
        assert_eq!(body, b"edited\r\nAC> Original \xDB line\r\n");
        assert!(contains(
            terminal.output(),
            b"Quoted lines cannot be edited."
        ));

        let mut aborted = InMemoryTerminal::with_lines([b"".to_vec(), b"A".to_vec()]);
        assert!(matches!(
            edit_message(&mut aborted, &conference, None).unwrap(),
            EditorOutcome::Cancelled
        ));
    }

    #[test]
    fn same_subject_thread_traversal_and_personal_lists_respect_privacy_and_receipts() {
        let (_temp, mut database, alice, bob) = message_database();
        let conference = database.conference(alice, 1).unwrap();
        let first = post_public(&mut database, alice, &conference, b"Thread Subject");
        let mut reply = NewMessage {
            conference_id: conference.id,
            recipient_caller_id: Some(bob.caller_id()),
            recipient_name: "Bob Caller".to_owned(),
            subject: b"Thread Subject".to_vec(),
            body: b"Private threaded reply\r\n".to_vec(),
            created_at: 2,
            parent_message_id: Some(first.id),
            visibility: MessageVisibility::Private,
            kind: MessageKind::Standard,
        };
        let private = database.post(alice, reply.clone()).unwrap();
        reply.recipient_caller_id = Some(alice.caller_id());
        reply.recipient_name = "Alice Caller".to_owned();
        reply.parent_message_id = Some(private.id);
        let bob_reply = database.post(bob, reply).unwrap();

        let mut thread = InMemoryTerminal::with_lines([
            b"S".to_vec(),
            b"F".to_vec(),
            b"F".to_vec(),
            b"E".to_vec(),
        ]);
        follow_message_thread(&mut thread, &mut database, bob, &conference, &private, true)
            .unwrap();
        assert!(contains(thread.output(), b"Private threaded reply"));
        assert_eq!(
            database.last_read(bob, conference.id).unwrap(),
            bob_reply.number
        );

        let mut received = InMemoryTerminal::with_lines([
            b"R".to_vec(),
            format!("1/{}", private.number).into_bytes(),
            b"".to_vec(),
            b"Q".to_vec(),
        ]);
        show_your_messages(&mut received, &mut database, bob, false).unwrap();
        assert!(contains(received.output(), b"--- YOUR MESSAGES RECEIVED"));
        assert!(database
            .received(bob, conference.id, private.number)
            .unwrap());
        assert!(!database
            .received(alice, conference.id, bob_reply.number)
            .unwrap());

        let mut preview = InMemoryTerminal::with_lines([
            b"R".to_vec(),
            b"Y".to_vec(),
            format!("1/{}", bob_reply.number).into_bytes(),
            b"".to_vec(),
            b"Q".to_vec(),
        ]);
        show_your_messages(&mut preview, &mut database, alice, true).unwrap();
        assert!(contains(
            preview.output(),
            b"Preview messages without marking them received?"
        ));
        assert!(!database
            .received(alice, conference.id, bob_reply.number)
            .unwrap());

        let mut sent = InMemoryTerminal::with_lines([b"S".to_vec(), b"".to_vec(), b"Q".to_vec()]);
        show_your_messages(&mut sent, &mut database, alice, false).unwrap();
        assert!(contains(sent.output(), b"--- YOUR MESSAGES SENT"));
    }
}
