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

use std::time::Instant;

use chrono::{LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use tracing::{info, warn};

use crate::{
    receive_binary_files, render_display, render_generated_menu, send_binary_streams_report,
    AsciiTransfer, AuthenticatedCaller, CallerConfig, DisplayContext, FileAccess, FileActor,
    FileArea, FileBackend, FileError, FileSearch, FileStorage, FileTransfer, MenuSection,
    ProtocolStreamFile, QuotaReservation, RatioStatus, RuntimeDatabase, SecurityLevel, Session,
    SessionError, SessionId, SessionStatusObserver, StockResources, StockSessionContext, Terminal,
    TerminalError, TextEncodingPolicy, TransferCancelSource, TransferDirection, TransferMethod,
    TransferPreference, TransferProtocol, TransferProtocolError, TransferQueue,
    TransferRuntimeError, TransferRuntimeState, TransferStateChange, UploadCreditRequest,
};

const MAX_AREA_NUMBER_INPUT: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileMenuExit {
    Main,
    Message,
    Sysop,
    Goodbye,
    EndOfInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileMenuResult {
    pub exit: FileMenuExit,
    pub commands: usize,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_file_menu(
    resources: &StockResources,
    context: &DisplayContext<'_>,
    terminal: &mut dyn Terminal,
    backend: &mut RuntimeDatabase,
    storage: &FileStorage,
    status: &dyn SessionStatusObserver,
    session: &mut Session,
    authenticated: &mut AuthenticatedCaller,
    caller_config: &CallerConfig,
    stock: &StockSessionContext<'_>,
    expert: &mut bool,
) -> Result<FileMenuResult, SessionError> {
    let actor = file_actor(authenticated, caller_config)?;
    let areas = backend.file_areas(actor)?;
    let Some((mut current, mut access)) = areas.first().cloned() else {
        write_line(
            terminal,
            "No file areas are available at your security level.",
        )?;
        return Ok(FileMenuResult {
            exit: FileMenuExit::Main,
            commands: 0,
        });
    };
    let menu = resources.menu(MenuSection::File)?;
    // Historical tags are caller-session state, not durable caller records.
    // The queue intentionally disappears on return from Files or disconnect.
    let mut download_queue = TransferQueue::default();
    let mut commands = 0;
    loop {
        if !*expert {
            if let Some(display) =
                resources.menu_display(MenuSection::File, authenticated.caller.security_level.get())
            {
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
        let mode = if access == FileAccess::Preview {
            "PREVIEW / NO DOWNLOAD"
        } else {
            "FULL ACCESS"
        };
        write_line(
            terminal,
            &format!("File Area {}: {} ({mode})", current.number, current.name),
        )?;
        write_key(
            terminal,
            MenuSection::File.prompt_key(),
            &crate::LocalizationArgs::new(),
        )?;
        let Some(command) =
            crate::session::read_menu_command(terminal, authenticated.caller.preferences.hot_keys)?
        else {
            return Ok(FileMenuResult {
                exit: FileMenuExit::EndOfInput,
                commands,
            });
        };
        commands += 1;
        if !crate::session::refresh_caller_access_for_dispatch(
            session,
            terminal,
            backend,
            authenticated,
            caller_config,
            stock,
            context,
        )? {
            return Ok(FileMenuResult {
                exit: FileMenuExit::EndOfInput,
                commands,
            });
        }
        let actor = file_actor(authenticated, caller_config)?;
        let Some(item) = menu.find(command, authenticated.caller.security_level.get()) else {
            write_key_line(
                terminal,
                "file-selection-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            continue;
        };
        match item.identifier {
            b'Z' => {
                if let Some(selected) = choose_area(terminal, backend, actor)? {
                    current = selected.0;
                    access = selected.1;
                    crate::session::render_named_display(
                        terminal,
                        resources,
                        &format!("SFIL{}", current.number),
                        context,
                    )?;
                }
            }
            b'X' => list_area_files(terminal, backend, actor, &current, stock.timezone)?,
            b'J' => inspect_text_file(terminal, backend, storage, actor, &current, session.id())?,
            b'G' => inspect_zip_file(terminal, backend, storage, actor, &current, session.id())?,
            b'P' => search_by_filename(terminal, backend, actor, stock.timezone)?,
            b'S' => search_descriptions(terminal, backend, actor, stock.timezone)?,
            b'N' => list_new_files(
                terminal,
                backend,
                actor,
                &current,
                stock.timezone,
                crate::session::unix_seconds()?,
            )?,
            b'L' => {
                crate::session::render_named_display(terminal, resources, "SFDOWN", context)?;
                download_file(
                    terminal,
                    backend,
                    storage,
                    status,
                    actor,
                    &current,
                    access,
                    authenticated.caller.preferences.transfer_protocol,
                    stock.timezone,
                    session.id(),
                    session.node_id(),
                    &mut download_queue,
                )?;
            }
            b'I' => {
                crate::session::render_named_display(terminal, resources, "SFUP", context)?;
                upload_file(
                    terminal,
                    backend,
                    storage,
                    status,
                    session.id(),
                    actor,
                    &current,
                    access,
                    authenticated.caller.preferences.transfer_protocol,
                    stock.timezone,
                    session.node_id(),
                    authenticated,
                )?;
            }
            b'E' => {
                return Ok(FileMenuResult {
                    exit: FileMenuExit::Message,
                    commands,
                });
            }
            b'C' => {
                return Ok(FileMenuResult {
                    exit: FileMenuExit::Main,
                    commands,
                });
            }
            b'F' => {
                let threshold = SecurityLevel::new(caller_config.sysop_security)?;
                if authenticated.caller.security_level.is_sysop(threshold) {
                    return Ok(FileMenuResult {
                        exit: FileMenuExit::Sysop,
                        commands,
                    });
                }
                write_line(
                    terminal,
                    "Sysop Utilities require the configured Sysop security threshold.",
                )?;
            }
            b'A' => {
                return Ok(FileMenuResult {
                    exit: FileMenuExit::Goodbye,
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
                MenuSection::File,
                menu,
                resources,
                terminal,
                authenticated.caller.security_level,
                authenticated.caller.preferences.hot_keys,
            )?,
            _ => write_line(
                terminal,
                "That file command is not available in this SPITFIRE NG capability set.",
            )?,
        }
    }
}

fn file_actor(
    authenticated: &AuthenticatedCaller,
    caller_config: &CallerConfig,
) -> Result<FileActor, SessionError> {
    Ok(FileActor::new(
        authenticated.caller.id,
        SecurityLevel::new(caller_config.sysop_security)?,
    ))
}

pub(crate) fn run_post_login_new_files(
    terminal: &mut dyn Terminal,
    backend: &mut dyn FileBackend,
    authenticated: &AuthenticatedCaller,
    caller_config: &CallerConfig,
    timezone: Tz,
) -> Result<(), SessionError> {
    let actor = file_actor(authenticated, caller_config)?;
    let areas = backend.file_areas(actor)?;
    let Some((current, _)) = areas.first() else {
        write_line(
            terminal,
            "No file areas are available at your security level.",
        )?;
        return Ok(());
    };
    list_new_files(
        terminal,
        backend,
        actor,
        current,
        timezone,
        crate::session::unix_seconds()?,
    )
}

fn choose_area(
    terminal: &mut dyn Terminal,
    backend: &dyn FileBackend,
    actor: FileActor,
) -> Result<Option<(FileArea, FileAccess)>, SessionError> {
    let areas = backend.file_areas(actor)?;
    write_key_line(
        terminal,
        "file-area-list-title",
        &crate::LocalizationArgs::new(),
    )?;
    for (area, access) in &areas {
        let marker = if *access == FileAccess::Preview {
            "preview"
        } else {
            "download"
        };
        write_line(
            terminal,
            &format!(
                "{:>3}  {:<20}  {:<8}  {}",
                area.number, area.name, marker, area.description
            ),
        )?;
    }
    write_key(
        terminal,
        "file-area-number-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_AREA_NUMBER_INPUT)? else {
        return Ok(None);
    };
    if input.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let Some(number) = parse_u16(&input) else {
        write_key_line(
            terminal,
            "file-area-number-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(None);
    };
    match backend.file_area(actor, number) {
        Ok(area) => Ok(Some(area)),
        Err(FileError::AreaNotFound(_) | FileError::AreaAccessDenied(_)) => {
            write_key_line(
                terminal,
                "file-area-unavailable",
                &crate::LocalizationArgs::new(),
            )?;
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}

fn list_area_files(
    terminal: &mut dyn Terminal,
    backend: &dyn FileBackend,
    actor: FileActor,
    area: &FileArea,
    timezone: Tz,
) -> Result<(), SessionError> {
    write_line(
        terminal,
        &format!("\r\nFiles in Area {} - {}", area.number, area.name),
    )?;
    render_file_results(terminal, backend.files(actor, area.id)?, timezone).map(|_| ())
}

fn search_by_filename(
    terminal: &mut dyn Terminal,
    backend: &dyn FileBackend,
    actor: FileActor,
    timezone: Tz,
) -> Result<(), SessionError> {
    write_key(
        terminal,
        "file-search-name-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(64)? else {
        return Ok(());
    };
    let pattern = String::from_utf8_lossy(&input).trim().to_owned();
    match backend.search_files(actor, None, &FileSearch::Filename(pattern)) {
        Ok(files) => render_file_results(terminal, files, timezone).map(|_| ()),
        Err(FileError::InvalidSearchPattern(_)) => {
            write_key_line(
                terminal,
                "file-search-pattern-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn search_descriptions(
    terminal: &mut dyn Terminal,
    backend: &dyn FileBackend,
    actor: FileActor,
    timezone: Tz,
) -> Result<(), SessionError> {
    write_key(
        terminal,
        "file-search-description-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(200)? else {
        return Ok(());
    };
    let words = String::from_utf8_lossy(&input)
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect();
    match backend.search_files(actor, None, &FileSearch::Description(words)) {
        Ok(files) => render_file_results(terminal, files, timezone).map(|_| ()),
        Err(FileError::InvalidDescriptionSearch) => {
            write_line(
                terminal,
                "Enter between one and six printable search words.",
            )?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn list_new_files(
    terminal: &mut dyn Terminal,
    backend: &mut dyn FileBackend,
    actor: FileActor,
    current: &FileArea,
    timezone: Tz,
    checked_at: i64,
) -> Result<(), SessionError> {
    let Some(area) = choose_new_file_area(terminal, backend, actor, current)? else {
        return Ok(());
    };
    let checkpoint = backend.new_file_checkpoint(actor)?;
    let Some(since) = choose_new_file_since(terminal, checkpoint, timezone)? else {
        return Ok(());
    };
    let statistics = backend.file_statistics(actor, checkpoint)?;
    terminal.begin_output();
    write_line(
        terminal,
        &format!(
            "New files since last checked: {}",
            format_number(statistics.new_since_checkpoint)
        ),
    )?;
    write_line(
        terminal,
        &format!(
            "Total downloadable files: {}",
            format_number(statistics.available_files)
        ),
    )?;
    write_line(
        terminal,
        &format!(
            "Total downloadable bytes: {}",
            format_number(statistics.available_bytes)
        ),
    )?;
    let files = backend.search_files(actor, area, &FileSearch::NewSince(since))?;
    if render_file_results(terminal, files, timezone)? {
        backend.record_new_file_check(actor, checked_at)?;
    } else {
        terminal.begin_output();
        write_line(
            terminal,
            "New-file display stopped; your last-files-checked date was not changed.",
        )?;
    }
    Ok(())
}

fn render_file_results(
    terminal: &mut dyn Terminal,
    files: Vec<crate::FileEntry>,
    timezone: Tz,
) -> Result<bool, SessionError> {
    terminal.begin_output();
    if files.is_empty() {
        write_key_line(
            terminal,
            "file-search-empty",
            &crate::LocalizationArgs::new(),
        )?;
        acknowledge_file_results(terminal)?;
        return Ok(true);
    }
    let width = terminal
        .info()
        .capabilities
        .size
        .map_or(80, |size| usize::from(size.width))
        .max(34);
    for file in &files {
        render_file_entry(terminal, file, timezone, width)?;
        if terminal.output_aborted() {
            return Ok(false);
        }
    }
    acknowledge_file_results(terminal)?;
    Ok(true)
}

fn acknowledge_file_results(terminal: &mut dyn Terminal) -> Result<(), SessionError> {
    terminal.write_all(b"\r\n")?;
    write_key(
        terminal,
        "session-return-file-menu-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    match terminal.read_line(64) {
        Ok(_) | Err(TerminalError::InputTooLong { .. }) => {
            terminal.write_all(b"\r\n")?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn choose_new_file_area(
    terminal: &mut dyn Terminal,
    backend: &dyn FileBackend,
    actor: FileActor,
    current: &FileArea,
) -> Result<Option<Option<crate::FileAreaId>>, SessionError> {
    write_key(
        terminal,
        "file-new-area-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(MAX_AREA_NUMBER_INPUT)? else {
        return Ok(None);
    };
    let input = String::from_utf8_lossy(&input);
    let input = input.trim();
    if input.eq_ignore_ascii_case("Q") {
        return Ok(None);
    }
    if input.is_empty() {
        return Ok(Some(None));
    }
    if input.eq_ignore_ascii_case("C") {
        return Ok(Some(Some(current.id)));
    }
    let Some(number) = input.parse::<u16>().ok() else {
        write_key_line(
            terminal,
            "file-new-area-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(None);
    };
    match backend.file_area(actor, number) {
        Ok((area, _)) => Ok(Some(Some(area.id))),
        Err(FileError::AreaNotFound(_) | FileError::AreaAccessDenied(_)) => {
            write_key_line(
                terminal,
                "file-area-unavailable",
                &crate::LocalizationArgs::new(),
            )?;
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}

fn choose_new_file_since(
    terminal: &mut dyn Terminal,
    checkpoint: Option<i64>,
    timezone: Tz,
) -> Result<Option<i64>, SessionError> {
    write_key(
        terminal,
        "file-new-date-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(16)? else {
        return Ok(None);
    };
    let input = String::from_utf8_lossy(&input);
    let input = input.trim();
    if input.eq_ignore_ascii_case("Q") {
        return Ok(None);
    }
    if input.is_empty() || input.eq_ignore_ascii_case("L") {
        return Ok(Some(checkpoint.unwrap_or(0)));
    }
    match parse_new_file_date(input, timezone) {
        Some(timestamp) => Ok(Some(timestamp)),
        None => {
            write_line(
                terminal,
                "Enter a real date as MM-DD-YY or MM-DD-YYYY (two-digit years mean 2000-2099).",
            )?;
            Ok(None)
        }
    }
}

fn parse_new_file_date(input: &str, timezone: Tz) -> Option<i64> {
    let mut fields = input.split('-');
    let month = fields.next()?.parse::<u32>().ok()?;
    let day = fields.next()?.parse::<u32>().ok()?;
    let year_text = fields.next()?;
    if fields.next().is_some() || !year_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let year = match year_text.len() {
        2 => 2000_i32.checked_add(year_text.parse::<i32>().ok()?)?,
        4 => year_text.parse::<i32>().ok()?,
        _ => return None,
    };
    if !(2000..=2099).contains(&year) {
        return None;
    }
    let midnight = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(0, 0, 0)?;
    let timestamp = match timezone.from_local_datetime(&midnight) {
        LocalResult::Single(value) => value.timestamp(),
        LocalResult::Ambiguous(first, second) => first.timestamp().min(second.timestamp()),
        LocalResult::None => return None,
    };
    (timestamp >= 0).then_some(timestamp)
}

fn render_file_entry(
    terminal: &mut dyn Terminal,
    file: &crate::FileEntry,
    timezone: Tz,
    width: usize,
) -> Result<(), SessionError> {
    let size = if file.lifecycle == crate::FileLifecycle::Offline
        || matches!(
            file.integrity,
            crate::FileIntegrity::Missing | crate::FileIntegrity::DigestMismatch
        ) {
        "OFFLINE".to_owned()
    } else {
        format_number(file.size_bytes)
    };
    let date = format_file_date(file.uploaded_at, timezone);
    let prefix = if file.filename.len() <= 12 && size.len() <= 9 {
        format!("{:<12}{:>9}  {date}  ", file.filename, size)
    } else {
        write_line(terminal, &file.filename)?;
        format!("{:>21}  {date}  ", size)
    };
    let description_width = width.saturating_sub(prefix.len()).max(1);
    let lines = wrap_description(&file.description, description_width);
    write_line(terminal, &format!("{prefix}{}", lines[0]))?;
    let continuation = " ".repeat(prefix.len());
    for line in &lines[1..] {
        if terminal.output_aborted() {
            break;
        }
        write_line(terminal, &format!("{continuation}{line}"))?;
    }
    Ok(())
}

fn format_file_date(timestamp: i64, timezone: Tz) -> String {
    chrono::DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|value| {
            value
                .with_timezone(&timezone)
                .format("%m-%d-%y")
                .to_string()
        })
        .unwrap_or_else(|| "??-??-??".to_owned())
}

fn format_number(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, byte) in digits.bytes().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(char::from(byte));
    }
    formatted
}

fn wrap_description(description: &str, width: usize) -> Vec<String> {
    let normalized = description.replace("\r\n", "\n");
    let mut rendered = Vec::new();
    for logical in normalized.split('\n') {
        if logical.is_empty() {
            rendered.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in logical.split_ascii_whitespace() {
            let needed = usize::from(!line.is_empty()) + word.len();
            if !line.is_empty() && line.len() + needed > width {
                rendered.push(std::mem::take(&mut line));
            }
            if word.len() > width {
                if !line.is_empty() {
                    rendered.push(std::mem::take(&mut line));
                }
                let mut remainder = word;
                while remainder.len() > width {
                    rendered.push(remainder[..width].to_owned());
                    remainder = &remainder[width..];
                }
                if !remainder.is_empty() {
                    line.push_str(remainder);
                }
            } else {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            rendered.push(line);
        }
    }
    if rendered.is_empty() {
        rendered.push(String::new());
    }
    rendered
}

fn inspect_text_file(
    terminal: &mut dyn Terminal,
    backend: &mut RuntimeDatabase,
    storage: &FileStorage,
    actor: FileActor,
    area: &FileArea,
    session_id: SessionId,
) -> Result<(), SessionError> {
    write_key(
        terminal,
        "file-read-text-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(64)? else {
        return Ok(());
    };
    let filename = String::from_utf8_lossy(&input).trim().to_owned();
    if filename.is_empty() {
        return Ok(());
    }
    let file = match backend.file(actor, area.id, &filename, false) {
        Ok(file) => file,
        Err(FileError::FileNotFound(_)) => {
            write_key_line(
                terminal,
                "file-read-text-rejected",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let use_token = match backend.begin_file_inspection_use(actor, file.id, session_id) {
        Ok(token) => token,
        Err(_) => {
            write_key_line(
                terminal,
                "file-read-text-rejected",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
    };
    let inspected = backend.inspect_text_file(storage, actor, file.id, TextEncodingPolicy::Auto);
    let _ = backend.finish_file_use(use_token);
    let inspection = match inspected {
        Ok(inspection) => inspection,
        Err(_) => {
            write_key_line(
                terminal,
                "file-read-text-rejected",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
    };
    write_key_line(
        terminal,
        "file-read-text-title",
        &crate::LocalizationArgs::new().with("filename", file.filename.as_str()),
    )?;
    for (index, line) in inspection.lines.into_iter().enumerate() {
        if index.is_multiple_of(20) && backend.reauthorize_file_inspection(actor, file.id).is_err()
        {
            write_key_line(
                terminal,
                "file-read-text-rejected",
                &crate::LocalizationArgs::new(),
            )?;
            break;
        }
        write_line(terminal, &line)?;
    }
    if inspection.truncated {
        write_key_line(
            terminal,
            "file-read-text-truncated",
            &crate::LocalizationArgs::new(),
        )?;
    }
    acknowledge_file_results(terminal)
}

fn inspect_zip_file(
    terminal: &mut dyn Terminal,
    backend: &mut RuntimeDatabase,
    storage: &FileStorage,
    actor: FileActor,
    area: &FileArea,
    session_id: SessionId,
) -> Result<(), SessionError> {
    write_key(
        terminal,
        "file-view-archive-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(64)? else {
        return Ok(());
    };
    let filename = String::from_utf8_lossy(&input).trim().to_owned();
    if filename.is_empty() {
        return Ok(());
    }
    let file = match backend.file(actor, area.id, &filename, false) {
        Ok(file) => file,
        Err(FileError::FileNotFound(_)) => {
            write_key_line(
                terminal,
                "file-view-archive-rejected",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let use_token = match backend.begin_file_inspection_use(actor, file.id, session_id) {
        Ok(token) => token,
        Err(_) => {
            write_key_line(
                terminal,
                "file-view-archive-rejected",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
    };
    let inspected = backend.inspect_zip_file(storage, actor, file.id);
    let _ = backend.finish_file_use(use_token);
    let inspection = match inspected {
        Ok(inspection) => inspection,
        Err(_) => {
            write_key_line(
                terminal,
                "file-view-archive-rejected",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
    };
    write_key_line(
        terminal,
        "file-view-archive-title",
        &crate::LocalizationArgs::new().with("filename", file.filename.as_str()),
    )?;
    if inspection.members.is_empty() {
        write_key_line(
            terminal,
            "file-view-archive-empty",
            &crate::LocalizationArgs::new(),
        )?;
    }
    for (index, member) in inspection.members.into_iter().enumerate() {
        if index.is_multiple_of(20) && backend.reauthorize_file_inspection(actor, file.id).is_err()
        {
            write_key_line(
                terminal,
                "file-view-archive-rejected",
                &crate::LocalizationArgs::new(),
            )?;
            break;
        }
        write_key_line(
            terminal,
            "file-view-archive-member",
            &crate::LocalizationArgs::new()
                .with("filename", member.filename.as_str())
                .with("size", member.uncompressed_bytes),
        )?;
        if member.unsafe_path {
            write_key_line(
                terminal,
                "file-view-archive-unsafe-name",
                &crate::LocalizationArgs::new(),
            )?;
        }
    }
    acknowledge_file_results(terminal)
}

// The transport-independent session boundary carries each authority and
// accounting dependency explicitly so none can be inferred from UI state.
#[allow(clippy::too_many_arguments)]
fn download_file(
    terminal: &mut dyn Terminal,
    backend: &mut RuntimeDatabase,
    storage: &FileStorage,
    status: &dyn SessionStatusObserver,
    actor: FileActor,
    area: &FileArea,
    access: FileAccess,
    preference: TransferPreference,
    timezone: Tz,
    session_id: SessionId,
    node_id: crate::NodeId,
    queue: &mut TransferQueue,
) -> Result<(), SessionError> {
    if access != FileAccess::Full {
        write_key_line(
            terminal,
            "file-preview-download-denied",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    write_key(
        terminal,
        "file-download-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(64)? else {
        return Ok(());
    };
    if input.eq_ignore_ascii_case(b"/C") {
        queue.clear();
        write_key_line(
            terminal,
            "file-download-queue-cleared",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    if input.eq_ignore_ascii_case(b"/L") {
        write_queue(terminal, queue)?;
        return Ok(());
    }
    if input.eq_ignore_ascii_case(b"/U") {
        let removed = recompute_queue(backend, actor, queue)?;
        write_key_line(
            terminal,
            "file-download-queue-recomputed",
            &crate::LocalizationArgs::new()
                .with("files", u64::try_from(queue.len()).unwrap_or(u64::MAX))
                .with("removed", u64::try_from(removed).unwrap_or(u64::MAX)),
        )?;
        write_queue(terminal, queue)?;
        return Ok(());
    }
    if input.len() > 3 && input[..3].eq_ignore_ascii_case(b"/R ") {
        let filename = String::from_utf8_lossy(&input[3..]).trim().to_owned();
        let removed = queue
            .items()
            .iter()
            .find(|item| item.filename.eq_ignore_ascii_case(&filename))
            .map(|item| item.file_id)
            .is_some_and(|file_id| queue.untag(file_id));
        write_key_line(
            terminal,
            if removed {
                "file-download-queue-removed"
            } else {
                "file-download-queue-not-found"
            },
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    let requested = String::from_utf8_lossy(&input)
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if requested.is_empty() && queue.is_empty() {
        return Ok(());
    }
    for filename in requested {
        match backend.file(actor, area.id, &filename, true) {
            Ok(file) => {
                queue
                    .tag(&file, area.no_charge)
                    .map_err(transfer_runtime_session_error)?;
            }
            Err(FileError::FileNotFound(_) | FileError::DownloadDenied(_)) => {
                write_key_line(
                    terminal,
                    "file-download-unavailable",
                    &crate::LocalizationArgs::new(),
                )?;
                let unavailable = backend
                    .files(actor, area.id)?
                    .into_iter()
                    .find(|file| file.filename.eq_ignore_ascii_case(&filename));
                if let Some(file) = unavailable {
                    write_key(
                        terminal,
                        "file-request-question",
                        &crate::LocalizationArgs::new(),
                    )?;
                    if terminal
                        .read_line(1)?
                        .is_some_and(|answer| answer.eq_ignore_ascii_case(b"Y"))
                    {
                        let board_day =
                            Utc::now().with_timezone(&timezone).date_naive().to_string();
                        match backend
                            .create_file_request_on_board_day(actor, file.id, None, &board_day)
                        {
                            Ok(_) => write_key_line(
                                terminal,
                                "file-request-created",
                                &crate::LocalizationArgs::new(),
                            )?,
                            Err(_) => write_key_line(
                                terminal,
                                "file-request-unavailable",
                                &crate::LocalizationArgs::new(),
                            )?,
                        }
                    }
                }
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        }
    }
    write_queue(terminal, queue)?;
    let protocol = choose_transfer_protocol(terminal, preference)?;
    if protocol == SelectedProtocol::Canceled {
        return Ok(());
    }
    if queue.len() > 1 && !protocol.is_batch() {
        write_key_line(
            terminal,
            "file-download-no-batch",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    let mut files = Vec::with_capacity(queue.len());
    for item in queue.items() {
        let file = backend
            .load_file_by_id(item.file_id)?
            .ok_or_else(|| FileError::FileNotFound(item.filename.clone()))?;
        files.push(file);
    }
    if protocol == SelectedProtocol::Ascii && files.len() != 1 {
        write_key_line(
            terminal,
            "file-download-ascii-single",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    let now = crate::session::unix_seconds()?;
    if backend
        .download_ratio_status(actor)
        .map_err(transfer_runtime_session_error)?
        == RatioStatus::Warning
    {
        write_key_line(
            terminal,
            "file-download-ratio-warning",
            &crate::LocalizationArgs::new(),
        )?;
    }
    let reservation = match backend.reserve_download_queue(
        actor,
        node_id,
        timezone,
        protocol.method().expect("canceled protocol returned above"),
        queue,
        now,
    ) {
        Ok(reservation) => reservation,
        Err(error) => {
            write_transfer_runtime_error(terminal, &error)?;
            return Ok(());
        }
    };
    let mut uses = Vec::with_capacity(files.len());
    for file in &files {
        match backend.begin_file_download_use(actor, file.id, session_id) {
            Ok(token) => uses.push(token),
            Err(_) => {
                for token in uses {
                    let _ = backend.finish_file_use(token);
                }
                let _ = backend.release_transfer(
                    &reservation.id,
                    TransferRuntimeState::Failed,
                    None,
                    Some("active-use-conflict"),
                    crate::session::unix_seconds()?,
                );
                write_key_line(
                    terminal,
                    "file-download-unavailable",
                    &crate::LocalizationArgs::new(),
                )?;
                return Ok(());
            }
        }
    }
    let transfer_result = (|| -> Result<(), SessionError> {
        let mut state_version = backend
            .set_transfer_state(
                &reservation.transfer_id,
                TransferStateChange {
                    expected_version: 1,
                    state: TransferRuntimeState::Negotiating,
                    bytes_transferred: 0,
                    error_class: None,
                    cancel_source: None,
                    occurred_at: crate::session::unix_seconds()?,
                },
            )
            .map_err(transfer_runtime_session_error)?;
        if protocol == SelectedProtocol::Ascii {
            let (root, locator) = backend
                .resolve_file_storage(files[0].id)
                .map_err(transfer_runtime_session_error)?;
            state_version = backend
                .set_transfer_state(
                    &reservation.transfer_id,
                    TransferStateChange {
                        expected_version: state_version,
                        state: TransferRuntimeState::Transferring,
                        bytes_transferred: 0,
                        error_class: None,
                        cancel_source: None,
                        occurred_at: crate::session::unix_seconds()?,
                    },
                )
                .map_err(transfer_runtime_session_error)?;
            let completed = download_ascii(terminal, storage, status, &root, &locator, &files[0])?;
            if completed {
                settle_download(
                    backend,
                    &reservation,
                    queue,
                    state_version,
                    files[0].size_bytes,
                )?;
                queue.clear();
            } else {
                backend
                    .release_transfer(
                        &reservation.id,
                        TransferRuntimeState::Cancelled,
                        Some(TransferCancelSource::Caller),
                        Some("incomplete"),
                        crate::session::unix_seconds()?,
                    )
                    .map_err(transfer_runtime_session_error)?;
            }
            return Ok(());
        }
        let Some(binary) = protocol.binary() else {
            return Ok(());
        };
        let mut payloads = Vec::with_capacity(files.len());
        for file in &files {
            let (root, locator) = backend
                .resolve_file_storage(file.id)
                .map_err(transfer_runtime_session_error)?;
            let input = storage.prepare_resolved_download(&root, &locator, file)?;
            payloads.push(ProtocolStreamFile {
                name: file.filename.clone(),
                size: file.size_bytes,
                source: Box::new(input),
                modified_unix: u64::try_from(file.uploaded_at).ok(),
            });
        }
        write_line(
            terminal,
            &format!("Beginning {} download.", binary.stock_name()),
        )?;
        status.transfer_started(TransferDirection::Download, &files[0].filename)?;
        let state_version = backend
            .set_transfer_state(
                &reservation.transfer_id,
                TransferStateChange {
                    expected_version: state_version,
                    state: TransferRuntimeState::Transferring,
                    bytes_transferred: 0,
                    error_class: None,
                    cancel_source: None,
                    occurred_at: crate::session::unix_seconds()?,
                },
            )
            .map_err(transfer_runtime_session_error)?;
        let result = send_binary_streams_report(terminal, binary, &mut payloads);
        status.transfer_finished()?;
        match result {
            Ok(report) if report.completed_files == queue.len() => {
                settle_download(
                    backend,
                    &reservation,
                    queue,
                    state_version,
                    files.iter().map(|file| file.size_bytes).sum(),
                )?;
                queue.clear();
                write_line(
                    terminal,
                    "Binary download complete; returning to SPITFIRE Files.",
                )?;
                info!(
                    caller_id = actor.caller_id().get(),
                    protocol = binary.stock_name(),
                    files = files.len(),
                    "caller completed binary file download"
                );
            }
            Ok(report) => {
                return Err(FileError::Maintenance(format!(
                    "protocol completed {} of {} files without a terminal result",
                    report.completed_files,
                    queue.len()
                ))
                .into());
            }
            Err(failure) => {
                warn!(caller_id = actor.caller_id().get(), protocol = binary.stock_name(), error = %failure.error, completed_files = failure.completed_files, "binary file download failed");
                let completed = failure.completed_files.min(queue.len());
                if completed > 0 {
                    backend
                        .set_transfer_state(
                            &reservation.transfer_id,
                            TransferStateChange {
                                expected_version: state_version,
                                state: TransferRuntimeState::Settling,
                                bytes_transferred: files[..completed]
                                    .iter()
                                    .map(|file| file.size_bytes)
                                    .sum(),
                                error_class: None,
                                cancel_source: None,
                                occurred_at: crate::session::unix_seconds()?,
                            },
                        )
                        .map_err(transfer_runtime_session_error)?;
                }
                let settled = queue.items()[..completed]
                    .iter()
                    .map(|item| item.item_id.clone())
                    .collect::<Vec<_>>();
                for item in &queue.items()[..completed] {
                    backend
                        .settle_download_item(
                            &reservation.id,
                            &item.item_id,
                            item.bytes,
                            crate::session::unix_seconds()?,
                        )
                        .map_err(transfer_runtime_session_error)?;
                }
                queue.retain_unsettled(&settled);
                write_key_line(
                    terminal,
                    if completed == 0 {
                        "file-download-failed"
                    } else {
                        "file-download-partial"
                    },
                    &crate::LocalizationArgs::new()
                        .with("completed", u64::try_from(completed).unwrap_or(u64::MAX))
                        .with("remaining", u64::try_from(queue.len()).unwrap_or(u64::MAX)),
                )?;
                let (state, source, class) = transfer_failure_class(&failure.error);
                backend
                    .release_transfer(
                        &reservation.id,
                        state,
                        source,
                        Some(class),
                        crate::session::unix_seconds()?,
                    )
                    .map_err(transfer_runtime_session_error)?;
            }
        }
        Ok(())
    })();
    for token in uses {
        if let Err(error) = backend.finish_file_use(token) {
            warn!(error = %error, "file-use lease cleanup failed");
        }
    }
    if transfer_result.is_err() {
        let _ = backend.release_transfer(
            &reservation.id,
            TransferRuntimeState::Failed,
            None,
            Some("local-io-failure"),
            crate::session::unix_seconds()?,
        );
    }
    transfer_result
}

fn write_queue(terminal: &mut dyn Terminal, queue: &TransferQueue) -> Result<(), SessionError> {
    let (chargeable_files, chargeable_bytes) = queue.chargeable_totals();
    write_key_line(
        terminal,
        "file-download-queue-summary",
        &crate::LocalizationArgs::new()
            .with("files", u64::try_from(queue.len()).unwrap_or(u64::MAX))
            .with("bytes", queue.total_bytes())
            .with("chargeable_files", chargeable_files)
            .with("chargeable_bytes", chargeable_bytes),
    )?;
    for item in queue.items() {
        write_key_line(
            terminal,
            "file-download-queue-item",
            &crate::LocalizationArgs::new()
                .with("filename", item.filename.as_str())
                .with("bytes", item.bytes),
        )?;
    }
    Ok(())
}

fn recompute_queue(
    backend: &RuntimeDatabase,
    actor: FileActor,
    queue: &mut TransferQueue,
) -> Result<usize, SessionError> {
    let prior = queue.items().to_vec();
    let areas = backend
        .file_areas(actor)?
        .into_iter()
        .map(|(area, access)| (area.id, (area, access)))
        .collect::<std::collections::BTreeMap<_, _>>();
    queue.clear();
    let mut removed = 0;
    for item in prior {
        let Some((area, access)) = areas.get(&item.area_id) else {
            removed += 1;
            continue;
        };
        let Some(file) = backend.load_file_by_id(item.file_id)? else {
            removed += 1;
            continue;
        };
        if *access != FileAccess::Full || queue.tag(&file, area.no_charge).is_err() {
            removed += 1;
        }
    }
    Ok(removed)
}

fn download_ascii(
    terminal: &mut dyn Terminal,
    storage: &FileStorage,
    status: &dyn SessionStatusObserver,
    root: &crate::StorageRoot,
    locator: &crate::FileStorageLocator,
    file: &crate::FileEntry,
) -> Result<bool, SessionError> {
    let mut input = match storage.open_resolved_ascii_download(root, locator, file) {
        Ok(input) => input,
        Err(FileError::NotAsciiText) => {
            write_line(
                terminal,
                "This initial ASCII protocol can transfer only 7-bit text files.",
            )?;
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    };
    write_line(
        terminal,
        &format!(
            "Beginning ASCII download of {} ({} bytes, SHA-256 {}).",
            file.filename, file.size_bytes, file.sha256
        ),
    )?;
    status.transfer_started(TransferDirection::Download, &file.filename)?;
    let result = AsciiTransfer.download(terminal, &mut input);
    status.transfer_finished()?;
    match result {
        Ok(report) if report.completed && report.bytes == file.size_bytes => {
            write_key_line(
                terminal,
                "file-download-complete",
                &crate::LocalizationArgs::new(),
            )?;
            info!(
                filename = file.filename,
                bytes = report.bytes,
                "caller completed file download"
            );
        }
        Ok(_) => {
            write_line(
                terminal,
                "Download did not complete; statistics were not changed.",
            )?;
            return Ok(false);
        }
        Err(FileError::NotAsciiText) => {
            write_line(
                terminal,
                "This initial ASCII protocol can transfer only 7-bit text files.",
            )?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(true)
}

fn settle_download(
    backend: &mut RuntimeDatabase,
    reservation: &QuotaReservation,
    queue: &TransferQueue,
    state_version: u64,
    bytes_transferred: u64,
) -> Result<(), SessionError> {
    let now = crate::session::unix_seconds()?;
    let state_version = backend
        .set_transfer_state(
            &reservation.transfer_id,
            TransferStateChange {
                expected_version: state_version,
                state: TransferRuntimeState::Settling,
                bytes_transferred,
                error_class: None,
                cancel_source: None,
                occurred_at: now,
            },
        )
        .map_err(transfer_runtime_session_error)?;
    for item in queue.items() {
        backend
            .settle_download_item(&reservation.id, &item.item_id, item.bytes, now)
            .map_err(transfer_runtime_session_error)?;
    }
    backend
        .set_transfer_state(
            &reservation.transfer_id,
            TransferStateChange {
                expected_version: state_version,
                state: TransferRuntimeState::Completed,
                bytes_transferred,
                error_class: None,
                cancel_source: None,
                occurred_at: now,
            },
        )
        .map_err(transfer_runtime_session_error)?;
    Ok(())
}

fn transfer_runtime_session_error(error: TransferRuntimeError) -> SessionError {
    FileError::Maintenance(error.to_string()).into()
}

fn transfer_failure_class(
    error: &TransferProtocolError,
) -> (
    TransferRuntimeState,
    Option<TransferCancelSource>,
    &'static str,
) {
    match error {
        TransferProtocolError::Terminal(TerminalError::OperatorCancelled) => (
            TransferRuntimeState::Cancelled,
            Some(TransferCancelSource::Operator),
            "operator-cancelled",
        ),
        TransferProtocolError::Canceled(_) => (
            TransferRuntimeState::Cancelled,
            Some(TransferCancelSource::Caller),
            "caller-cancelled",
        ),
        TransferProtocolError::Disconnected(_) => (
            TransferRuntimeState::Cancelled,
            Some(TransferCancelSource::Disconnect),
            "disconnected",
        ),
        TransferProtocolError::TimedOut(_) | TransferProtocolError::RetryLimit(_) => (
            TransferRuntimeState::Failed,
            Some(TransferCancelSource::Timeout),
            "timeout",
        ),
        _ => (TransferRuntimeState::Failed, None, "protocol-failure"),
    }
}

fn write_transfer_runtime_error(
    terminal: &mut dyn Terminal,
    error: &TransferRuntimeError,
) -> Result<(), TerminalError> {
    let key = match error {
        TransferRuntimeError::PreviewDenied => "file-transfer-preview-denied",
        TransferRuntimeError::DailyLimitExceeded => "file-transfer-daily-limit",
        TransferRuntimeError::RatioDenied => "file-transfer-ratio-denied",
        TransferRuntimeError::StorageUnavailable => "file-transfer-storage-unavailable",
        TransferRuntimeError::StaleQueueItem(_) => "file-transfer-stale-queue",
        TransferRuntimeError::ProtocolUnsupported
        | TransferRuntimeError::ProtocolUnsupportedForBatch => "file-transfer-protocol-unsupported",
        TransferRuntimeError::Unauthorized => "file-transfer-unauthorized",
        TransferRuntimeError::Conflict | TransferRuntimeError::StaleVersion { .. } => {
            "file-transfer-conflict"
        }
        TransferRuntimeError::RecoveryRequired => "file-transfer-recovery-required",
        TransferRuntimeError::QueueFull | TransferRuntimeError::ResourceLimit => {
            "file-transfer-resource-limit"
        }
        _ => "file-transfer-unavailable",
    };
    write_key_line(terminal, key, &crate::LocalizationArgs::new())
}

#[allow(clippy::too_many_arguments)]
fn upload_file(
    terminal: &mut dyn Terminal,
    backend: &mut RuntimeDatabase,
    storage: &FileStorage,
    status: &dyn SessionStatusObserver,
    session_id: SessionId,
    actor: FileActor,
    area: &FileArea,
    access: FileAccess,
    preference: TransferPreference,
    timezone: Tz,
    node_id: crate::NodeId,
    authenticated: &mut AuthenticatedCaller,
) -> Result<(), SessionError> {
    if access != FileAccess::Full {
        write_key_line(
            terminal,
            "file-preview-upload-denied",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    write_key(
        terminal,
        "file-upload-name-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = terminal.read_line(64)? else {
        return Ok(());
    };
    if input.eq_ignore_ascii_case(b"/A") {
        write_key_line(
            terminal,
            "file-upload-canceled",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    let filename = String::from_utf8_lossy(&input).trim().to_owned();
    match backend.upload_duplicate_warnings(actor, area.id, &filename) {
        Ok(warnings) if !warnings.is_empty() => {
            write_key(
                terminal,
                "file-upload-duplicate-warning",
                &crate::LocalizationArgs::new(),
            )?;
            if !terminal
                .read_line(1)?
                .is_some_and(|answer| answer.eq_ignore_ascii_case(b"Y"))
            {
                write_key_line(
                    terminal,
                    "file-upload-canceled",
                    &crate::LocalizationArgs::new(),
                )?;
                return Ok(());
            }
        }
        Ok(_) => {}
        Err(crate::FileMaintenanceError::File(FileError::DuplicateFilename(_))) => {
            write_key_line(
                terminal,
                "file-upload-duplicate-conflict",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
        Err(error) => return Err(FileError::Maintenance(error.to_string()).into()),
    }
    write_key(
        terminal,
        "file-upload-description-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(description) = terminal.read_line(255)? else {
        return Ok(());
    };
    if description.eq_ignore_ascii_case(b"/A") || description.is_empty() {
        write_key_line(
            terminal,
            "file-upload-canceled",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    let description = String::from_utf8_lossy(&description).trim().to_owned();
    let protocol = choose_transfer_protocol(terminal, preference)?;
    if protocol == SelectedProtocol::Canceled {
        return Ok(());
    }
    let method = protocol.method().expect("canceled protocol returned above");
    if let Err(error) = backend.authorize_upload_protocol(actor, method) {
        write_transfer_runtime_error(terminal, &error)?;
        return Ok(());
    }
    if protocol == SelectedProtocol::Ascii {
        if !filename.to_ascii_uppercase().ends_with(".TXT") {
            write_key_line(
                terminal,
                "file-upload-ascii-txt-only",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
        return upload_ascii(
            terminal,
            backend,
            storage,
            status,
            session_id,
            actor,
            area,
            &filename,
            &description,
            timezone,
            node_id,
            authenticated,
        );
    }
    let Some(binary) = protocol.binary() else {
        return Ok(());
    };
    write_line(
        terminal,
        &format!("Ready to receive with {}.", binary.stock_name()),
    )?;
    status.transfer_started(TransferDirection::Upload, &filename)?;
    let active_started = Instant::now();
    let result = receive_binary_files(
        terminal,
        binary,
        &filename,
        area.maximum_upload_bytes,
        if binary.is_batch() { 64 } else { 1 },
    );
    status.transfer_finished()?;
    let received = match result {
        Ok(files) => files,
        Err(error) => {
            warn!(caller_id = actor.caller_id().get(), protocol = binary.stock_name(), error = %error, "binary file upload failed");
            write_line(
                terminal,
                "Upload failed or was canceled; staged data was discarded.",
            )?;
            return Ok(());
        }
    };
    let active_seconds = active_started.elapsed().as_secs();
    let received_total_bytes = received.iter().fold(0_u64, |total, file| {
        total.saturating_add(file.bytes.len() as u64)
    });
    let mut committed = 0;
    for received in received {
        let item_active_seconds = active_seconds
            .saturating_mul(received.bytes.len() as u64)
            .checked_div(received_total_bytes)
            .unwrap_or(0);
        match backend.upload_duplicate_warnings(actor, area.id, &received.name) {
            Ok(warnings) if !warnings.is_empty() => {
                write_key(
                    terminal,
                    "file-upload-duplicate-warning",
                    &crate::LocalizationArgs::new(),
                )?;
                if !terminal
                    .read_line(1)?
                    .is_some_and(|answer| answer.eq_ignore_ascii_case(b"Y"))
                {
                    write_key_line(
                        terminal,
                        "file-upload-canceled",
                        &crate::LocalizationArgs::new(),
                    )?;
                    continue;
                }
            }
            Ok(_) => {}
            Err(crate::FileMaintenanceError::File(FileError::DuplicateFilename(_))) => {
                write_key_line(
                    terminal,
                    "file-upload-duplicate-conflict",
                    &crate::LocalizationArgs::new(),
                )?;
                continue;
            }
            Err(error) => return Err(FileError::Maintenance(error.to_string()).into()),
        }
        let mut staged = match storage.begin_upload(session_id, &received.name) {
            Ok(staged) => staged,
            Err(FileError::InvalidFilename(_) | FileError::UploadAlreadyStaged(_)) => {
                write_line(
                    terminal,
                    &format!("{} was rejected by filename policy.", received.name),
                )?;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        staged.write_all(&received.bytes)?;
        match backend.commit_upload(
            storage,
            staged,
            actor,
            area,
            &description,
            crate::session::unix_seconds()?,
        ) {
            Ok(file) => {
                committed += 1;
                let credit = apply_live_upload_credit(
                    backend,
                    authenticated,
                    actor,
                    node_id,
                    TransferMethod::Binary(binary),
                    &file,
                    item_active_seconds,
                    timezone,
                    session_id,
                    committed,
                )?;
                write_line(
                    terminal,
                    &format!(
                        "Upload complete: {} ({} bytes, SHA-256 {}).",
                        file.filename, file.size_bytes, file.sha256
                    ),
                )?;
                if file.lifecycle == crate::FileLifecycle::PendingReview {
                    write_key_line(
                        terminal,
                        "file-upload-pending-review",
                        &crate::LocalizationArgs::new(),
                    )?;
                }
                if credit != 0 {
                    write_key_line(
                        terminal,
                        "file-upload-time-credit",
                        &crate::LocalizationArgs::new().with("seconds", credit),
                    )?;
                }
            }
            Err(FileError::DuplicateFilename(_)) => {
                write_line(
                    terminal,
                    &format!("{} already exists; it was not cataloged.", received.name),
                )?;
            }
            Err(FileError::UploadDenied(_)) => {
                write_key_line(
                    terminal,
                    "file-upload-unauthorized",
                    &crate::LocalizationArgs::new(),
                )?;
            }
            Err(FileError::UploadDeniedByPolicy) => write_key_line(
                terminal,
                "file-upload-policy-denied",
                &crate::LocalizationArgs::new(),
            )?,
            Err(error) => return Err(error.into()),
        }
    }
    info!(
        caller_id = actor.caller_id().get(),
        protocol = binary.stock_name(),
        files = committed,
        "caller completed binary upload batch"
    );
    write_line(
        terminal,
        &format!("{committed} uploaded file(s) committed; returning to SPITFIRE Files."),
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn upload_ascii(
    terminal: &mut dyn Terminal,
    backend: &mut RuntimeDatabase,
    storage: &FileStorage,
    status: &dyn SessionStatusObserver,
    session_id: SessionId,
    actor: FileActor,
    area: &FileArea,
    filename: &str,
    description: &str,
    timezone: Tz,
    node_id: crate::NodeId,
    authenticated: &mut AuthenticatedCaller,
) -> Result<(), SessionError> {
    let mut staged = match storage.begin_upload(session_id, filename) {
        Ok(staged) => staged,
        Err(FileError::InvalidFilename(_) | FileError::UploadAlreadyStaged(_)) => {
            write_line(
                terminal,
                "That upload filename is invalid or already staged.",
            )?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    write_line(
        terminal,
        "Send ASCII text now. Enter /S on a line by itself to finish or /A to cancel.",
    )?;
    status.transfer_started(TransferDirection::Upload, filename)?;
    let active_started = Instant::now();
    let result = AsciiTransfer.upload(terminal, &mut staged, area.maximum_upload_bytes);
    status.transfer_finished()?;
    match result {
        Ok(report) if report.completed => match backend.commit_upload(
            storage,
            staged,
            actor,
            area,
            description,
            crate::session::unix_seconds()?,
        ) {
            Ok(file) => {
                let credit = apply_live_upload_credit(
                    backend,
                    authenticated,
                    actor,
                    node_id,
                    TransferMethod::Ascii,
                    &file,
                    active_started.elapsed().as_secs(),
                    timezone,
                    session_id,
                    1,
                )?;
                write_line(
                    terminal,
                    &format!(
                        "Upload complete: {} ({} bytes, SHA-256 {}).",
                        file.filename, file.size_bytes, file.sha256
                    ),
                )?;
                if file.lifecycle == crate::FileLifecycle::PendingReview {
                    write_key_line(
                        terminal,
                        "file-upload-pending-review",
                        &crate::LocalizationArgs::new(),
                    )?;
                }
                if credit != 0 {
                    write_key_line(
                        terminal,
                        "file-upload-time-credit",
                        &crate::LocalizationArgs::new().with("seconds", credit),
                    )?;
                }
                info!(
                    caller_id = actor.caller_id().get(),
                    filename = file.filename,
                    bytes = file.size_bytes,
                    "caller completed file upload"
                );
            }
            Err(FileError::DuplicateFilename(_)) => {
                write_line(
                    terminal,
                    "That filename already exists; upload was not cataloged.",
                )?;
            }
            Err(FileError::UploadDenied(_)) => {
                write_key_line(
                    terminal,
                    "file-upload-unauthorized",
                    &crate::LocalizationArgs::new(),
                )?;
            }
            Err(FileError::UploadDeniedByPolicy) => write_key_line(
                terminal,
                "file-upload-policy-denied",
                &crate::LocalizationArgs::new(),
            )?,
            Err(error) => return Err(error.into()),
        },
        Ok(_) => {
            warn!(
                caller_id = actor.caller_id().get(),
                "caller upload canceled or disconnected"
            );
            write_key_line(
                terminal,
                "file-upload-discarded",
                &crate::LocalizationArgs::new(),
            )?;
        }
        Err(FileError::UploadTooLarge { .. }) => {
            write_line(
                terminal,
                "Upload exceeded the configured area size limit and was discarded.",
            )?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_live_upload_credit(
    backend: &mut RuntimeDatabase,
    authenticated: &mut AuthenticatedCaller,
    actor: FileActor,
    node_id: crate::NodeId,
    method: TransferMethod,
    file: &crate::FileEntry,
    active_seconds: u64,
    timezone: Tz,
    session_id: SessionId,
    sequence: usize,
) -> Result<u64, SessionError> {
    let transfer_id = crate::TransferId::generated(
        i64::try_from(session_id.get()).map_err(|_| SessionError::ClockOverflow)?,
    );
    let item_id = format!("upload-{sequence}-{}", file.id.get());
    let credit = backend
        .apply_upload_credit(UploadCreditRequest {
            transfer_id: &transfer_id,
            item_id: &item_id,
            actor,
            node_id,
            method,
            file_id: file.id,
            active_seconds,
            timezone,
            occurred_at: crate::session::unix_seconds()?,
        })
        .map_err(transfer_runtime_session_error)?;
    authenticated.credit_live_allowance(credit);
    Ok(credit)
}

fn first_command(input: &[u8]) -> Option<u8> {
    input
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .map(|byte| byte.to_ascii_uppercase())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectedProtocol {
    Canceled,
    Ascii,
    Binary(TransferProtocol),
}

impl SelectedProtocol {
    const fn binary(self) -> Option<TransferProtocol> {
        match self {
            Self::Canceled | Self::Ascii => None,
            Self::Binary(protocol) => Some(protocol),
        }
    }

    const fn is_batch(self) -> bool {
        match self {
            Self::Canceled | Self::Ascii => false,
            Self::Binary(protocol) => protocol.is_batch(),
        }
    }

    const fn method(self) -> Option<TransferMethod> {
        match self {
            Self::Canceled => None,
            Self::Ascii => Some(TransferMethod::Ascii),
            Self::Binary(protocol) => Some(TransferMethod::Binary(protocol)),
        }
    }
}

pub(crate) fn choose_transfer_protocol(
    terminal: &mut dyn Terminal,
    preference: TransferPreference,
) -> Result<SelectedProtocol, SessionError> {
    let selected = match preference {
        TransferPreference::Select => {
            terminal.write_all(
                b"\r\nSPITFIRE Transfer Protocols\r\n<1> Ascii  <2> Xmodem Checksum  <3> Xmodem CRC\r\n<4> 1K-Xmodem  <5> Ymodem (Batch)  <6> Zmodem (Batch)\r\n<7> 1K-Xmodem-g  <8> Ymodem-g (Batch)  <9/T> Telink\r\nProtocol [1-9,T; Q cancels]: ",
            )?;
            let Some(input) = terminal.read_line(8)? else {
                return Ok(SelectedProtocol::Canceled);
            };
            match first_command(&input) {
                Some(b'1') => SelectedProtocol::Ascii,
                Some(b'2') => SelectedProtocol::Binary(TransferProtocol::XmodemChecksum),
                Some(b'3') => SelectedProtocol::Binary(TransferProtocol::XmodemCrc),
                Some(b'4') => SelectedProtocol::Binary(TransferProtocol::Xmodem1k),
                Some(b'5') => SelectedProtocol::Binary(TransferProtocol::YmodemBatch),
                Some(b'6') => SelectedProtocol::Binary(TransferProtocol::ZmodemBatch),
                Some(b'7') => SelectedProtocol::Binary(TransferProtocol::Xmodem1kG),
                Some(b'8') => SelectedProtocol::Binary(TransferProtocol::YmodemGBatch),
                Some(b'9' | b'T') => SelectedProtocol::Binary(TransferProtocol::Telink),
                _ => {
                    write_key_line(
                        terminal,
                        "file-transfer-canceled",
                        &crate::LocalizationArgs::new(),
                    )?;
                    return Ok(SelectedProtocol::Canceled);
                }
            }
        }
        TransferPreference::Ascii => SelectedProtocol::Ascii,
        TransferPreference::XmodemChecksum => {
            SelectedProtocol::Binary(TransferProtocol::XmodemChecksum)
        }
        TransferPreference::XmodemCrc => SelectedProtocol::Binary(TransferProtocol::XmodemCrc),
        TransferPreference::Xmodem1k => SelectedProtocol::Binary(TransferProtocol::Xmodem1k),
        TransferPreference::Ymodem => SelectedProtocol::Binary(TransferProtocol::YmodemBatch),
        TransferPreference::Zmodem => SelectedProtocol::Binary(TransferProtocol::ZmodemBatch),
        TransferPreference::Telink => SelectedProtocol::Binary(TransferProtocol::Telink),
    };
    Ok(selected)
}

fn parse_u16(input: &[u8]) -> Option<u16> {
    std::str::from_utf8(input).ok()?.trim().parse().ok()
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
        CallerPreferences, CallerState, FileAccessMode, FileAreaDefinition, FileEntry, FileId,
        FileStorage, InMemoryTerminal, LogicalPaths, PagingTerminal, RuntimeConfig,
    };

    #[test]
    fn cooperative_operator_transfer_cancel_retains_operator_classification() {
        assert_eq!(
            transfer_failure_class(&TransferProtocolError::Terminal(
                TerminalError::OperatorCancelled
            )),
            (
                TransferRuntimeState::Cancelled,
                Some(TransferCancelSource::Operator),
                "operator-cancelled"
            )
        );
    }

    #[test]
    fn stock_file_rows_show_date_columns_and_extended_description_lines() {
        let file = FileEntry {
            id: FileId::new(1).unwrap(),
            area_id: crate::FileAreaId::new(1).unwrap(),
            filename: "ARCHIVE.ZIP".to_owned(),
            description: "Brief description\r\nExtended description line".to_owned(),
            size_bytes: 1_234,
            sha256: "a".repeat(64),
            uploaded_at: chrono::Utc
                .with_ymd_and_hms(2026, 8, 22, 12, 0, 0)
                .single()
                .unwrap()
                .timestamp(),
            uploader_caller_id: None,
            uploader_name: "SPITFIRE NG".to_owned(),
            download_count: 7,
            lifecycle: crate::FileLifecycle::Active,
            integrity: crate::FileIntegrity::Present,
            state_version: 1,
            description_source: "system".to_owned(),
            description_source_digest: None,
        };
        let mut terminal = InMemoryTerminal::default();
        assert!(
            render_file_results(&mut terminal, vec![file], chrono_tz::America::Phoenix).unwrap()
        );
        let output = terminal.output_text().unwrap();
        assert!(output.contains("ARCHIVE.ZIP     1,234  08-22-26  Brief description\r\n"));
        assert!(output.contains("                                 Extended description line\r\n"));
        assert!(output.contains("Press ENTER to return to the File Menu:"));
        assert!(!output.contains("d/l"));
        assert_eq!(format_number(1_234_567), "1,234,567");
    }

    #[test]
    fn empty_file_results_wait_before_the_file_menu_redraws() {
        let mut terminal = InMemoryTerminal::with_lines([Vec::new()]);
        assert!(render_file_results(&mut terminal, Vec::new(), chrono_tz::UTC).unwrap());
        let output = terminal.output_text().unwrap();
        assert!(output.contains("No matching files are available."));
        assert!(output.contains("Press ENTER to return to the File Menu:"));
    }

    #[test]
    fn extended_file_description_paging_can_abort_one_listing() {
        let file = FileEntry {
            id: FileId::new(1).unwrap(),
            area_id: crate::FileAreaId::new(1).unwrap(),
            filename: "PAGED.TXT".to_owned(),
            description: (1..=12)
                .map(|line| format!("Extended line {line}"))
                .collect::<Vec<_>>()
                .join("\r\n"),
            size_bytes: 12,
            sha256: "a".repeat(64),
            uploaded_at: 1_700_000_000,
            uploader_caller_id: None,
            uploader_name: "SPITFIRE NG".to_owned(),
            download_count: 0,
            lifecycle: crate::FileLifecycle::Active,
            integrity: crate::FileIntegrity::Present,
            state_version: 1,
            description_source: "system".to_owned(),
            description_source_digest: None,
        };
        let mut inner = InMemoryTerminal::with_lines([b"Q".to_vec()]);
        {
            let mut terminal = PagingTerminal::new(
                &mut inner,
                CallerPreferences {
                    page_length: Some(4),
                    ..CallerPreferences::default()
                },
            );
            assert!(!render_file_results(&mut terminal, vec![file], chrono_tz::UTC).unwrap());
        }
        let output = inner.output_text().unwrap();
        assert!(output.contains("MORE: <S>top"));
        assert!(output.contains("Extended line 3"));
        assert!(!output.contains("Extended line 12"));
        assert!(!output.contains("Press ENTER to return to the File Menu:"));
    }

    #[test]
    fn new_file_dates_use_the_operational_century_and_board_midnight() {
        let timezone = chrono_tz::America::Phoenix;
        let expected = timezone
            .with_ymd_and_hms(2026, 8, 22, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp();
        assert_eq!(parse_new_file_date("08-22-26", timezone), Some(expected));
        assert_eq!(parse_new_file_date("08-22-2026", timezone), Some(expected));
        assert_eq!(parse_new_file_date("02-29-25", timezone), None);
        assert_eq!(parse_new_file_date("12-31-1999", timezone), None);
        assert_eq!(
            parse_new_file_date("12-31-99", timezone),
            Some(
                timezone
                    .with_ymd_and_hms(2099, 12, 31, 0, 0, 0)
                    .single()
                    .unwrap()
                    .timestamp()
            )
        );
    }

    #[test]
    fn new_file_workflow_selects_scope_and_persists_only_completed_checks() {
        let temp = tempfile::tempdir().unwrap();
        let config = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(temp.path(), &config).unwrap();
        paths.create_directories().unwrap();
        let mut database = crate::RuntimeDatabase::open(paths.database()).unwrap();
        database.migrate().unwrap();
        let caller = database
            .create_caller(
                b"New File Caller",
                "test-only-stored-hash",
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        let actor = FileActor::new(caller.id, SecurityLevel::new(50).unwrap());
        let definition = |number, key: &str| FileAreaDefinition {
            number,
            name: format!("Area {number}"),
            description: "New-file test area".to_owned(),
            storage_key: key.to_owned(),
            access_mode: FileAccessMode::AtLeast,
            read_security: SecurityLevel::new(5).unwrap(),
            upload_security: SecurityLevel::new(5).unwrap(),
            preview: false,
            no_charge: false,
            maximum_upload_bytes: 1_048_576,
            privileged_security_levels: Vec::new(),
        };
        let first = database
            .create_file_area(&definition(1, "new-one"))
            .unwrap();
        let second = database
            .create_file_area(&definition(2, "new-two"))
            .unwrap();
        let storage = FileStorage::new(&paths).unwrap();
        let timezone = chrono_tz::America::Phoenix;
        let old = timezone
            .with_ymd_and_hms(2026, 8, 20, 12, 0, 0)
            .single()
            .unwrap()
            .timestamp();
        let recent = timezone
            .with_ymd_and_hms(2026, 8, 22, 12, 0, 0)
            .single()
            .unwrap()
            .timestamp();
        storage
            .write_seed_file(
                &mut database,
                &first,
                "OLDER.TXT",
                "Older file\r\nDetail two\r\nDetail three\r\nDetail four\r\nDetail five\r\nDetail six\r\nDetail seven\r\nDetail eight\r\nDetail nine\r\nDetail ten\r\nDetail eleven\r\nDetail twelve",
                b"old",
                old,
            )
            .unwrap();
        storage
            .write_seed_file(
                &mut database,
                &second,
                "RECENT.TXT",
                "Recent file",
                b"recent",
                recent,
            )
            .unwrap();

        let checked_at = recent + 60;
        let mut all = InMemoryTerminal::with_lines([b"".to_vec(), b"L".to_vec()]);
        list_new_files(&mut all, &mut database, actor, &first, timezone, checked_at).unwrap();
        let output = all.output_text().unwrap();
        assert!(output.contains("New files since last checked: 2"));
        assert!(output.contains("Total downloadable files: 2"));
        assert!(output.contains("Total downloadable bytes: 9"));
        assert!(output.contains("OLDER.TXT"));
        assert!(output.contains("RECENT.TXT"));
        assert_eq!(
            database.new_file_checkpoint(actor).unwrap(),
            Some(checked_at)
        );

        let mut dated = InMemoryTerminal::with_lines([b"2".to_vec(), b"08-22-26".to_vec()]);
        list_new_files(
            &mut dated,
            &mut database,
            actor,
            &first,
            timezone,
            checked_at + 60,
        )
        .unwrap();
        let output = dated.output_text().unwrap();
        assert!(!output.contains("OLDER.TXT"));
        assert!(output.contains("RECENT.TXT"));
        assert_eq!(
            database.new_file_checkpoint(actor).unwrap(),
            Some(checked_at + 60)
        );

        let mut invalid = InMemoryTerminal::with_lines([b"".to_vec(), b"02-29-25".to_vec()]);
        list_new_files(
            &mut invalid,
            &mut database,
            actor,
            &first,
            timezone,
            checked_at + 120,
        )
        .unwrap();
        assert!(invalid.output_text().unwrap().contains("Enter a real date"));
        assert_eq!(
            database.new_file_checkpoint(actor).unwrap(),
            Some(checked_at + 60)
        );

        let mut inner =
            InMemoryTerminal::with_lines([b"".to_vec(), b"08-20-26".to_vec(), b"Q".to_vec()]);
        {
            let mut paged = PagingTerminal::new(
                &mut inner,
                CallerPreferences {
                    page_length: Some(4),
                    ..CallerPreferences::default()
                },
            );
            list_new_files(
                &mut paged,
                &mut database,
                actor,
                &first,
                timezone,
                checked_at + 180,
            )
            .unwrap();
        }
        let output = inner.output_text().unwrap();
        assert!(
            output.contains("last-files-checked date was not changed"),
            "{output}"
        );
        assert_eq!(
            database.new_file_checkpoint(actor).unwrap(),
            Some(checked_at + 60)
        );
    }

    #[test]
    fn caller_queue_recompute_uses_stable_ids_and_removes_inaccessible_items() {
        let temp = tempfile::tempdir().unwrap();
        let config = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(temp.path(), &config).unwrap();
        paths.create_directories().unwrap();
        let mut database = RuntimeDatabase::open(paths.database()).unwrap();
        database.migrate().unwrap();
        let caller = database
            .create_caller(
                b"Queue Caller",
                "test-only-stored-hash",
                SecurityLevel::new(10).unwrap(),
                CallerState::Active,
                false,
                100,
            )
            .unwrap();
        let actor = FileActor::new(caller.id, SecurityLevel::new(50).unwrap());
        let area = database
            .create_file_area(&FileAreaDefinition {
                number: 1,
                name: "Queue".to_owned(),
                description: "Queue recompute".to_owned(),
                storage_key: "queue-recompute".to_owned(),
                access_mode: FileAccessMode::AtLeast,
                read_security: SecurityLevel::new(5).unwrap(),
                upload_security: SecurityLevel::new(5).unwrap(),
                preview: false,
                no_charge: false,
                maximum_upload_bytes: 1_048_576,
                privileged_security_levels: Vec::new(),
            })
            .unwrap();
        let storage = FileStorage::new(&paths).unwrap();
        let first = storage
            .write_seed_file(&mut database, &area, "ONE.BIN", "One", b"one", 100)
            .unwrap();
        let second = storage
            .write_seed_file(&mut database, &area, "TWO.BIN", "Two", b"two", 101)
            .unwrap();
        let third = storage
            .write_seed_file(&mut database, &area, "THREE.BIN", "Three", b"three", 102)
            .unwrap();
        let mut queue = TransferQueue::default();
        queue.tag(&first, false).unwrap();
        queue.tag(&second, false).unwrap();
        queue.tag(&third, false).unwrap();
        database
            .connection
            .execute(
                "UPDATE files SET lifecycle='disabled',state_version=state_version+1 WHERE file_id=?1",
                rusqlite::params![second.id.get()],
            )
            .unwrap();
        database
            .connection
            .execute(
                "UPDATE files SET state_version=state_version+1 WHERE file_id=?1",
                rusqlite::params![third.id.get()],
            )
            .unwrap();
        assert_eq!(recompute_queue(&database, actor, &mut queue).unwrap(), 1);
        assert_eq!(
            queue
                .items()
                .iter()
                .map(|item| item.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["ONE.BIN", "THREE.BIN"]
        );
        assert_eq!(queue.items()[0].file_id, first.id);
        assert_eq!(queue.items()[1].file_id, third.id);
        assert!(queue.items()[1].expected_file_version > third.state_version);
        assert_eq!(queue.total_bytes(), 8);
    }
}
