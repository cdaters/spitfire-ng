use std::collections::BTreeMap;

use thiserror::Error;

use crate::{
    format_board_local_timestamp, localized_bytes, terminal_text_encoding, text,
    AuthenticatedCaller, BoardIdentity, Caller, LocalizationArgs, NodeId, SecurityLevel, Terminal,
    TerminalError, TerminalTextEncoding,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MenuSection {
    Main,
    Message,
    File,
    Sysop,
}

impl MenuSection {
    pub const fn prompt_key(self) -> &'static str {
        match self {
            Self::Main => "menu-prompt-main",
            Self::Message => "menu-prompt-message",
            Self::File => "menu-prompt-file",
            Self::Sysop => "menu-prompt-sysop",
        }
    }

    pub const fn title_key(self) -> &'static str {
        match self {
            Self::Main => "menu-title-main",
            Self::Message => "menu-title-message",
            Self::File => "menu-title-file",
            Self::Sysop => "menu-title-sysop",
        }
    }

    /// Whether the common NG session engine currently dispatches this stock
    /// identifier in the given menu. Generated presentation must not advertise
    /// parsed historical records whose operation is still deferred.
    pub const fn supports_identifier(self, identifier: u8) -> bool {
        match self {
            Self::Main => matches!(
                identifier,
                b'E' | b'Q' | b'F' | b'G' | b'H' | b'R' | b'Y' | b'I' | b'J' | b'A' | b'B' | b'?'
            ),
            Self::Message => matches!(
                identifier,
                b'Z' | b'J'
                    | b'I'
                    | b'L'
                    | b'G'
                    | b'K'
                    | b'S'
                    | b'X'
                    | b'D'
                    | b'C'
                    | b'R'
                    | b'A'
                    | b'B'
                    | b'?'
            ),
            Self::File => matches!(
                identifier,
                b'Z' | b'X'
                    | b'P'
                    | b'S'
                    | b'N'
                    | b'L'
                    | b'I'
                    | b'E'
                    | b'C'
                    | b'F'
                    | b'A'
                    | b'B'
                    | b'?'
            ),
            Self::Sysop => matches!(identifier, b'C' | b'A' | b'B' | b'?'),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuItem {
    pub command: u8,
    pub description: Vec<u8>,
    pub required_security: u16,
    pub identifier: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuDefinition {
    pub section: MenuSection,
    pub items: Vec<MenuItem>,
}

impl MenuDefinition {
    pub fn find(&self, command: u8, security: u16) -> Option<&MenuItem> {
        let command = command.to_ascii_uppercase();
        self.items.iter().find(|item| {
            item.command.to_ascii_uppercase() == command && item.required_security <= security
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayFormat {
    Bbs,
    Clr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplaySource {
    BoardOverride,
    ActiveProfile,
    BaseProfile,
    EngineBuiltIn,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayResource {
    pub format: DisplayFormat,
    pub source: DisplaySource,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpRecord {
    pub lines: [Vec<u8>; 6],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockResources {
    pub prelogin: DisplayResource,
    pub welcome: DisplayResource,
    pub goodbye: DisplayResource,
    pub page_off: DisplayResource,
    pub page_unanswered: DisplayResource,
    pub page_already_requested: DisplayResource,
    pub chat_caller_initiated: DisplayResource,
    pub chat_done: DisplayResource,
    pub menus: BTreeMap<MenuSection, MenuDefinition>,
    /// Optional display resources keyed by their case-normalized filename
    /// stem. Extension selection has already applied the stock CLR/BBS
    /// precedence for this terminal capability set.
    pub displays: BTreeMap<String, DisplayResource>,
    /// One-based historical help records stored at zero-based vector indexes.
    pub help_records: Vec<HelpRecord>,
}

impl StockResources {
    pub fn menu(&self, section: MenuSection) -> Result<&MenuDefinition, ResourceError> {
        self.menus
            .get(&section)
            .ok_or(ResourceError::MissingMenu(section))
    }

    pub fn display(&self, stem: &str) -> Option<&DisplayResource> {
        self.displays.get(&stem.to_ascii_uppercase())
    }

    /// Stock menu artwork is selected by the caller's exact security level.
    /// When no exact file exists, SPITFIRE's generated menu remains available.
    pub fn menu_display(&self, section: MenuSection, security: u16) -> Option<&DisplayResource> {
        let prefix = match section {
            MenuSection::Main => "MAIN",
            MenuSection::Message => "MSG",
            MenuSection::File => "FILE",
            MenuSection::Sysop => "SOP",
        };
        self.display(&format!("{prefix}{security}"))
    }

    pub fn help_record(&self, one_based: usize) -> Option<&HelpRecord> {
        one_based
            .checked_sub(1)
            .and_then(|index| self.help_records.get(index))
    }
}

/// Render the engine-owned menu derived from the parsed `.MNU` authority.
/// A normal ANSI 80x25 session receives the evidenced compact two-column
/// grammar. Plain-text or constrained sessions use a bounded single column.
pub fn render_generated_menu(
    terminal: &mut dyn Terminal,
    menu: &MenuDefinition,
    security: SecurityLevel,
    sysop_threshold: SecurityLevel,
    status_lines: &[String],
) -> Result<(), TerminalError> {
    let info = terminal.info();
    let width = usize::from(info.capabilities.size.map_or(80, |size| size.width)).max(8);
    let height = info.capabilities.size.map_or(24, |size| size.height);
    let visible = menu
        .items
        .iter()
        .filter(|item| menu_item_is_visible(menu.section, item, security, sysop_threshold))
        .map(|item| {
            let localized = menu_action_key(menu.section, item.command, item.identifier)
                .map(|key| text(key, &LocalizationArgs::new()).into_bytes())
                .unwrap_or_else(|| menu_item_fallback_label(item));
            (item, localized)
        })
        .collect::<Vec<_>>();
    let column_width = 30;
    let gutter_width = 8;
    let title = localized_bytes(&info, menu.section.title_key(), &LocalizationArgs::new());
    let two_columns = info.capabilities.ansi
        && width >= 80
        && height >= 20
        && title.len().saturating_add(2) <= column_width
        && visible
            .iter()
            .all(|(_, label)| menu_item_fits(label, column_width));

    terminal.begin_output();
    if info.capabilities.ansi {
        terminal.write_all(b"\x1b[2J\x1b[H\x1b[1;36m")?;
    } else {
        terminal.write_all(b"\r\n")?;
    }
    if two_columns {
        terminal.write_all(&stock_heading(&title, column_width))?;
        terminal.write_all(b"\r\n\r\n")?;
        let rows = visible.len().div_ceil(2);
        for row in 0..rows {
            let (left_item, left_label) = &visible[row];
            terminal.write_all(&stock_menu_row(left_item.command, left_label, column_width))?;
            if let Some((_, right)) = visible.get(row + rows) {
                for _ in 0..gutter_width {
                    terminal.write_all(b" ")?;
                }
                terminal.write_all(&stock_menu_row(
                    visible[row + rows].0.command,
                    right,
                    column_width,
                ))?;
            }
            terminal.write_all(b"\r\n")?;
            if terminal.output_aborted() {
                break;
            }
        }
    } else {
        write_bounded_line(terminal, &title, width, terminal_text_encoding(&info))?;
        for (item, label) in visible {
            let mut line = vec![b'<', item.command.to_ascii_uppercase(), b'>', b' '];
            line.extend_from_slice(&label);
            write_bounded_line(terminal, &line, width, terminal_text_encoding(&info))?;
            if terminal.output_aborted() {
                break;
            }
        }
    }
    if info.capabilities.ansi {
        terminal.write_all(b"\x1b[0m")?;
    }
    if !status_lines.is_empty() && !terminal.output_aborted() {
        terminal.write_all(b"\r\n")?;
        for line in status_lines {
            write_bounded_line(terminal, line.as_bytes(), width, TerminalTextEncoding::Utf8)?;
            if terminal.output_aborted() {
                break;
            }
        }
    }
    Ok(())
}

/// Count the commands the generated menu may truthfully advertise. The same
/// predicate is used by the renderer and transient operator diagnostics.
pub fn visible_menu_action_count(
    menu: &MenuDefinition,
    security: SecurityLevel,
    sysop_threshold: SecurityLevel,
) -> usize {
    menu.items
        .iter()
        .filter(|item| menu_item_is_visible(menu.section, item, security, sysop_threshold))
        .count()
}

fn menu_item_is_visible(
    section: MenuSection,
    item: &MenuItem,
    security: SecurityLevel,
    sysop_threshold: SecurityLevel,
) -> bool {
    security.get() >= item.required_security
        && section.supports_identifier(item.identifier)
        && (security.is_sysop(sysop_threshold)
            || !matches!(
                (section, item.identifier),
                (MenuSection::Main, b'F')
                    | (MenuSection::Message, b'R')
                    | (MenuSection::File, b'F')
                    | (MenuSection::Sysop, _)
            ))
}

fn menu_action_key(section: MenuSection, command: u8, identifier: u8) -> Option<&'static str> {
    if section == MenuSection::Message
        && command.eq_ignore_ascii_case(&b'A')
        && identifier.eq_ignore_ascii_case(&b'X')
    {
        return Some("menu-action-message-queue");
    }
    Some(match (section, identifier.to_ascii_uppercase()) {
        (MenuSection::Main, b'E') => "menu-action-main-messages",
        (MenuSection::Main, b'J') => "menu-action-main-comment",
        (MenuSection::Main, b'Q') => "menu-action-main-files",
        (MenuSection::Main, b'H') => "menu-action-main-page",
        (MenuSection::Main, b'G') => "menu-action-main-statistics",
        (MenuSection::Main, b'Y') => "menu-action-main-profile",
        (MenuSection::Main, b'R') => "menu-action-main-terminal",
        (MenuSection::Main, b'I') => "menu-action-main-about",
        (MenuSection::Main, b'F') => "menu-action-main-sysop",
        (MenuSection::Message, b'Z') => "menu-action-message-change",
        (MenuSection::Message, b'I') => "menu-action-message-read",
        (MenuSection::Message, b'J') => "menu-action-message-browse",
        (MenuSection::Message, b'L') => "menu-action-message-enter",
        (MenuSection::Message, b'G') => "menu-action-message-yours",
        (MenuSection::Message, b'K') => "menu-action-message-queue",
        (MenuSection::Message, b'S') => "menu-action-message-search-caller",
        (MenuSection::Message, b'X') => "menu-action-message-search-text",
        (MenuSection::Message, b'D') => "menu-action-message-files",
        (MenuSection::File, b'Z') => "menu-action-file-change",
        (MenuSection::File, b'X') => "menu-action-file-list",
        (MenuSection::File, b'L') => "menu-action-file-download",
        (MenuSection::File, b'I') => "menu-action-file-upload",
        (MenuSection::File, b'N') => "menu-action-file-new",
        (MenuSection::File, b'S') => "menu-action-file-search-description",
        (MenuSection::File, b'P') => "menu-action-file-find",
        (MenuSection::File, b'E') => "menu-action-file-messages",
        (MenuSection::Sysop, b'C') => "menu-action-sysop-main",
        (_, b'C') => "menu-action-common-main",
        (_, b'B') => "menu-action-common-xpert",
        (_, b'A') => "menu-action-common-goodbye",
        (_, b'?') => "menu-action-common-help",
        _ => return None,
    })
}

fn menu_item_fallback_label(item: &MenuItem) -> Vec<u8> {
    let description = item.description.as_slice();
    let start = description
        .iter()
        .position(|byte| *byte != b'<' && *byte != b'>' && *byte != item.command)
        .unwrap_or(0);
    let label = description[start..]
        .iter()
        .position(|byte| !matches!(*byte, b'.' | b' ' | b'\t'))
        .map(|offset| &description[start + offset..])
        .unwrap_or(description);
    label.to_vec()
}

fn menu_item_fits(label: &[u8], column_width: usize) -> bool {
    // Command token, at least one dot leader, one separating space, and label.
    3usize
        .saturating_add(1)
        .saturating_add(1)
        .saturating_add(label.len())
        <= column_width
}

fn stock_menu_row(command: u8, label: &[u8], width: usize) -> Vec<u8> {
    let mut row = Vec::with_capacity(width);
    row.extend_from_slice(&[b'<', command.to_ascii_uppercase(), b'>']);
    let dots = width.saturating_sub(row.len() + 1 + label.len());
    row.extend(std::iter::repeat_n(b'.', dots));
    row.push(b' ');
    row.extend_from_slice(label);
    row.resize(width, b' ');
    row
}

fn stock_heading(title: &[u8], width: usize) -> Vec<u8> {
    debug_assert!(title.len().saturating_add(2) <= width);
    let decoration = width.saturating_sub(title.len() + 2);
    let left = decoration.div_ceil(2);
    let right = decoration / 2;
    let mut heading = Vec::with_capacity(width);
    heading.extend(std::iter::repeat_n(b'>', left));
    heading.push(b' ');
    heading.extend_from_slice(title);
    heading.push(b' ');
    heading.extend(std::iter::repeat_n(b'<', right));
    heading
}

fn write_bounded_line(
    terminal: &mut dyn Terminal,
    bytes: &[u8],
    width: usize,
    encoding: TerminalTextEncoding,
) -> Result<(), TerminalError> {
    if bytes.is_empty() {
        return terminal.write_all(b"\r\n");
    }
    let mut remaining = bytes;
    while !remaining.is_empty() {
        let mut end = remaining.len().min(width);
        if encoding == TerminalTextEncoding::Utf8 && end < remaining.len() {
            while end > 0 && std::str::from_utf8(&remaining[..end]).is_err() {
                end -= 1;
            }
            if end == 0 {
                // Localization output is validated UTF-8; this is a bounded
                // defensive fallback for an unexpected invalid byte stream.
                end = remaining.len().min(width);
            }
        }
        terminal.write_all(&remaining[..end])?;
        terminal.write_all(b"\r\n")?;
        remaining = &remaining[end..];
    }
    Ok(())
}

pub struct DisplayContext<'a> {
    pub board: &'a BoardIdentity,
    pub node: NodeId,
    pub timezone: chrono_tz::Tz,
    pub caller: Option<DisplayCallerContext>,
    pub logon_minutes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayCallerContext {
    pub full_name: String,
    pub security_level: u16,
    pub first_call_at: i64,
    pub last_call_at: Option<i64>,
    pub files_uploaded: u64,
    pub upload_bytes: u64,
    pub files_downloaded: u64,
    pub download_bytes: u64,
    city_region: String,
    phone: String,
    birthday: String,
}

impl DisplayCallerContext {
    /// Contact values are deliberately private implementation fields. This
    /// context is created only for the caller who owns the active session, so
    /// historical self-profile macros cannot expose another caller's PII.
    pub fn from_caller(caller: &Caller) -> Self {
        Self {
            full_name: caller.display_name.clone(),
            security_level: caller.security_level.get(),
            first_call_at: caller.first_call_at,
            last_call_at: caller.last_call_at,
            files_uploaded: caller.files_uploaded,
            upload_bytes: caller.upload_bytes,
            files_downloaded: caller.files_downloaded,
            download_bytes: caller.download_bytes,
            city_region: caller.profile.address.city_region().unwrap_or_default(),
            phone: caller.profile.phone.clone().unwrap_or_default(),
            birthday: caller.profile.birthday_iso().unwrap_or_default(),
        }
    }

    pub fn from_authenticated(authenticated: &AuthenticatedCaller) -> Self {
        let mut context = Self::from_caller(&authenticated.caller);
        context.last_call_at = authenticated.previous_call_at;
        context
    }
}

/// Renders the confirmed stock-core SPITFIRE display control table with
/// deliberately bounded modern values. Unknown strings remain byte-for-byte
/// visible rather than being assigned guessed meanings, and CP437 bytes pass
/// through unchanged.
pub fn render_display(
    terminal: &mut dyn Terminal,
    resource: &DisplayResource,
    context: &DisplayContext<'_>,
) -> Result<(), ResourceError> {
    terminal.begin_output();
    if resource.format == DisplayFormat::Clr && !terminal.info().capabilities.ansi {
        return Err(ResourceError::AnsiRequired);
    }

    let bytes = &resource.bytes;
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        let parsed = match bytes[index] {
            0x02 => Some((Control::PromptOff, 1)),
            0x03 => Some((Control::NoAbort, 1)),
            0x04 => Some((Control::FirstName, 1)),
            0x05 => Some((Control::SubscriptionDate, 1)),
            0x06 => Some((Control::CityState, 1)),
            0x07 => Some((Control::Beep, 1)),
            // DOS display resources use CRLF line endings. A standalone ^J
            // is the documented UPLOADS control, while LF following CR is
            // ordinary line structure and must pass through unchanged.
            0x0A if index == 0 || bytes[index - 1] != b'\r' => Some((Control::Uploads, 1)),
            0x0B => Some((Control::Downloads, 1)),
            0x0C => Some((Control::Clear, 1)),
            0x0E => Some((Control::AbortOn, 1)),
            0x0F => Some((Control::OriginalLogon, 1)),
            0x10 => Some((Control::Prompt, 1)),
            0x11 => Some((Control::LogonTime, 1)),
            0x12 => Some((Control::PhoneNumber, 1)),
            0x13 => Some((Control::LastCall, 1)),
            0x14 => Some((Control::Password, 1)),
            0x15 => Some((Control::BirthDate, 1)),
            0x16 => Some((Control::FullName, 1)),
            0x17 => Some((Control::UploadK, 1)),
            0x18 => Some((Control::DownloadK, 1)),
            0x19 => Some((Control::SecurityLevel, 1)),
            b'@' => parse_string_control(&bytes[index..]),
            _ => None,
        };
        let Some((control, consumed)) = parsed else {
            index += 1;
            continue;
        };
        if start < index {
            terminal.write_all(&bytes[start..index])?;
            if terminal.output_aborted() {
                return Ok(());
            }
        }
        render_control(terminal, control, context)?;
        if terminal.output_aborted() {
            return Ok(());
        }
        index += consumed;
        start = index;
    }
    if start < bytes.len() {
        terminal.write_all(&bytes[start..])?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Control {
    PromptOff,
    NoAbort,
    AbortOn,
    Beep,
    Clear,
    Prompt,
    FirstName,
    SubscriptionDate,
    CityState,
    Uploads,
    Downloads,
    OriginalLogon,
    LogonTime,
    PhoneNumber,
    LastCall,
    BirthDate,
    FullName,
    UploadK,
    DownloadK,
    SecurityLevel,
    Board,
    Sysop,
    Node,
    Password,
}

fn parse_string_control(input: &[u8]) -> Option<(Control, usize)> {
    const CONTROLS: &[(&[u8], Control)] = &[
        (b"@PROMPTOFF@", Control::PromptOff),
        (b"@NOABORT@", Control::NoAbort),
        (b"@ABORTON@", Control::AbortOn),
        (b"@BEEP@", Control::Beep),
        (b"@CLS@", Control::Clear),
        (b"@PROMPT@", Control::Prompt),
        (b"@FNAME@", Control::FirstName),
        (b"@SUBDATE@", Control::SubscriptionDate),
        (b"@CITYSTATE@", Control::CityState),
        (b"@UPLOADS@", Control::Uploads),
        (b"@DOWNLOADS@", Control::Downloads),
        (b"@ORGLOG@", Control::OriginalLogon),
        (b"@LOGTIME@", Control::LogonTime),
        (b"@PHONENUM@", Control::PhoneNumber),
        (b"@LASTCALL@", Control::LastCall),
        (b"@BIRTHDATE@", Control::BirthDate),
        (b"@NAME@", Control::FullName),
        (b"@UPK@", Control::UploadK),
        (b"@DOWNK@", Control::DownloadK),
        (b"@SLEVEL@", Control::SecurityLevel),
        (b"@BOARD@", Control::Board),
        (b"@SYSOP@", Control::Sysop),
        (b"@NODE@", Control::Node),
        (b"@PASSWORD@", Control::Password),
    ];
    CONTROLS
        .iter()
        .find(|(text, _)| input.starts_with(text))
        .map(|(text, control)| (*control, text.len()))
}

fn render_control(
    terminal: &mut dyn Terminal,
    control: Control,
    context: &DisplayContext<'_>,
) -> Result<(), TerminalError> {
    match control {
        Control::PromptOff => {
            terminal.set_output_paging(false);
            Ok(())
        }
        Control::NoAbort => {
            terminal.set_output_abort(false);
            Ok(())
        }
        Control::AbortOn => {
            terminal.set_output_abort(true);
            Ok(())
        }
        Control::Beep => terminal.write_all(&[0x07]),
        Control::Clear => {
            if terminal.info().capabilities.ansi {
                terminal.write_all(b"\x1B[2J\x1B[H")
            } else {
                Ok(())
            }
        }
        Control::Prompt => terminal.prompt_more(),
        Control::FirstName => write_text(terminal, first_name(context)),
        Control::SubscriptionDate => terminal.write_all(b"N/A"),
        Control::CityState => write_private_profile_value(
            terminal,
            context
                .caller
                .as_ref()
                .map(|caller| caller.city_region.as_str()),
        ),
        Control::PhoneNumber => write_private_profile_value(
            terminal,
            context.caller.as_ref().map(|caller| caller.phone.as_str()),
        ),
        Control::BirthDate => write_private_profile_value(
            terminal,
            context
                .caller
                .as_ref()
                .map(|caller| caller.birthday.as_str()),
        ),
        Control::Uploads => write_number(
            terminal,
            caller_value(context, |caller| caller.files_uploaded),
        ),
        Control::Downloads => write_number(
            terminal,
            caller_value(context, |caller| caller.files_downloaded),
        ),
        Control::OriginalLogon => write_timestamp(
            terminal,
            context.caller.as_ref().map(|caller| caller.first_call_at),
            context.timezone,
        ),
        Control::LogonTime => write_number(terminal, context.logon_minutes),
        Control::LastCall => write_timestamp(
            terminal,
            context
                .caller
                .as_ref()
                .and_then(|caller| caller.last_call_at),
            context.timezone,
        ),
        Control::FullName => write_text(
            terminal,
            context
                .caller
                .as_ref()
                .map_or("Caller", |caller| caller.full_name.as_str()),
        ),
        Control::UploadK => write_number(
            terminal,
            caller_value(context, |caller| caller.upload_bytes / 1024),
        ),
        Control::DownloadK => write_number(
            terminal,
            caller_value(context, |caller| caller.download_bytes / 1024),
        ),
        Control::SecurityLevel => write_number(
            terminal,
            context
                .caller
                .as_ref()
                .map(|caller| u64::from(caller.security_level)),
        ),
        Control::Board => terminal.write_all(context.board.name().as_bytes()),
        Control::Sysop => terminal.write_all(context.board.sysop_name().as_bytes()),
        Control::Node => terminal.write_all(context.node.get().to_string().as_bytes()),
        // Historical password-display behavior is intentionally unavailable.
        Control::Password => terminal.write_all(b"[PASSWORD UNAVAILABLE]"),
    }
}

fn caller_value(
    context: &DisplayContext<'_>,
    value: impl FnOnce(&DisplayCallerContext) -> u64,
) -> Option<u64> {
    context.caller.as_ref().map(value)
}

fn write_private_profile_value(
    terminal: &mut dyn Terminal,
    value: Option<&str>,
) -> Result<(), TerminalError> {
    match value.filter(|value| !value.is_empty()) {
        Some(value) => terminal.write_all(value.as_bytes()),
        None => terminal.write_all(b"[NOT PROVIDED]"),
    }
}

fn first_name<'a>(context: &'a DisplayContext<'_>) -> &'a str {
    context
        .caller
        .as_ref()
        .and_then(|caller| caller.full_name.split_ascii_whitespace().next())
        .unwrap_or("Caller")
}

fn write_text(terminal: &mut dyn Terminal, value: &str) -> Result<(), TerminalError> {
    terminal.write_all(value.as_bytes())
}

fn write_number(terminal: &mut dyn Terminal, value: Option<u64>) -> Result<(), TerminalError> {
    match value {
        Some(value) => terminal.write_all(value.to_string().as_bytes()),
        None => terminal.write_all(b"N/A"),
    }
}

fn write_timestamp(
    terminal: &mut dyn Terminal,
    value: Option<i64>,
    timezone: chrono_tz::Tz,
) -> Result<(), TerminalError> {
    let Some(seconds) = value else {
        return terminal.write_all(b"Never");
    };
    let rendered = format_board_local_timestamp(seconds, timezone)
        .unwrap_or_else(|_| "Invalid date/time".to_owned());
    terminal.write_all(rendered.as_bytes())
}

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("missing {0:?} menu definition")]
    MissingMenu(MenuSection),
    #[error("ANSI .CLR resource cannot be rendered to a non-ANSI terminal")]
    AnsiRequired,
    #[error(transparent)]
    Terminal(#[from] TerminalError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InMemoryTerminal, PagingTerminal, TerminalCapabilities, TerminalInfo, TerminalSize,
        TransportKind,
    };

    fn generated_menu() -> MenuDefinition {
        MenuDefinition {
            section: MenuSection::Main,
            items: vec![
                MenuItem {
                    command: b'M',
                    description: b"<M>.......... Message Section".to_vec(),
                    required_security: 10,
                    identifier: b'E',
                },
                MenuItem {
                    command: b'F',
                    description: b"<F>............. File Section".to_vec(),
                    required_security: 10,
                    identifier: b'Q',
                },
                MenuItem {
                    command: b'@',
                    description: b"<@>.......... Sysop Utilities".to_vec(),
                    required_security: 50,
                    identifier: b'F',
                },
                MenuItem {
                    command: b'D',
                    description: b"<D>.......... Deferred Historical Door".to_vec(),
                    required_security: 10,
                    identifier: b'V',
                },
            ],
        }
    }

    fn contains(output: &[u8], needle: &[u8]) -> bool {
        output.windows(needle.len()).any(|part| part == needle)
    }

    fn section_menu(section: MenuSection) -> MenuDefinition {
        let pairs: &[(u8, u8)] = match section {
            MenuSection::Main => &[
                (b'M', b'E'),
                (b'C', b'J'),
                (b'F', b'Q'),
                (b'P', b'H'),
                (b'Y', b'G'),
                (b'R', b'Y'),
                (b'U', b'R'),
                (b'A', b'I'),
                (b'@', b'F'),
                (b'X', b'B'),
                (b'G', b'A'),
                (b'?', b'?'),
            ],
            MenuSection::Message => &[
                (b'C', b'Z'),
                (b'R', b'I'),
                (b'B', b'J'),
                (b'E', b'L'),
                (b'Y', b'G'),
                (b'A', b'K'),
                (b'S', b'S'),
                (b'T', b'X'),
                (b'F', b'D'),
                (b'Q', b'C'),
                (b'@', b'R'),
                (b'X', b'B'),
                (b'G', b'A'),
                (b'?', b'?'),
            ],
            MenuSection::File => &[
                (b'C', b'Z'),
                (b'L', b'X'),
                (b'D', b'L'),
                (b'U', b'I'),
                (b'N', b'N'),
                (b'T', b'S'),
                (b'F', b'P'),
                (b'M', b'E'),
                (b'Q', b'C'),
                (b'@', b'F'),
                (b'X', b'B'),
                (b'G', b'A'),
                (b'?', b'?'),
            ],
            MenuSection::Sysop => &[(b'Q', b'C'), (b'X', b'B'), (b'G', b'A')],
        };
        MenuDefinition {
            section,
            items: pairs
                .iter()
                .map(|(command, identifier)| MenuItem {
                    command: *command,
                    description: format!("<{command}> fallback").into_bytes(),
                    // Deliberately lower than the configured threshold: the
                    // engine-owned Sysop transition guard must still win.
                    required_security: 5,
                    identifier: *identifier,
                })
                .collect(),
        }
    }

    fn generated_output(
        menu: &MenuDefinition,
        security: u16,
        cp437: bool,
        width: u16,
        height: u16,
    ) -> Vec<u8> {
        let mut info = TerminalInfo::in_memory();
        info.capabilities.cp437 = cp437;
        info.capabilities.terminal_type = Some(if cp437 { "ANSI" } else { "XTERM" }.to_owned());
        info.capabilities.size = Some(TerminalSize { width, height });
        let mut terminal = InMemoryTerminal::with_info(Vec::<Vec<u8>>::new(), info);
        render_generated_menu(
            &mut terminal,
            menu,
            SecurityLevel::new(security).unwrap(),
            SecurityLevel::new(50).unwrap(),
            &[],
        )
        .unwrap();
        terminal.output().to_vec()
    }

    fn stock_geometry_lines(output: &[u8]) -> Vec<Vec<u8>> {
        output
            .split(|byte| *byte == b'\n')
            .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
            .map(|line| {
                let line = line.strip_prefix(b"\x1b[2J\x1b[H").unwrap_or(line);
                let line = line.strip_prefix(b"\x1b[1;36m").unwrap_or(line);
                line.strip_suffix(b"\x1b[0m").unwrap_or(line).to_vec()
            })
            .filter(|line| line.starts_with(b">") || line.starts_with(b"<"))
            .collect()
    }

    #[test]
    fn generated_menu_is_security_filtered_and_two_column_at_ansi_80x25() {
        let mut info = TerminalInfo::in_memory();
        info.capabilities.size = Some(TerminalSize {
            width: 80,
            height: 25,
        });
        let mut terminal = InMemoryTerminal::with_info(Vec::<Vec<u8>>::new(), info);
        render_generated_menu(
            &mut terminal,
            &generated_menu(),
            SecurityLevel::new(10).unwrap(),
            SecurityLevel::new(50).unwrap(),
            &["Caller: Synthetic Caller  Security: 10".to_owned()],
        )
        .unwrap();
        let output = terminal.output();
        assert!(contains(output, b">>>>>>>>>> MAIN MENU <<<<<<<<<"));
        assert!(contains(output, b"<M>........... Message Section"));
        assert!(contains(output, b"<F>.............. File Section"));
        assert!(!contains(output, b"<@> Sysop Utilities"));
        assert!(!contains(output, b"Deferred Historical Door"));
        assert!(output.windows(2).any(|part| part == b"  "));
        assert!(output.contains(&0x1b));
    }

    #[test]
    fn generated_menu_handles_arbitrary_security_and_constrained_text() {
        let mut info = TerminalInfo::in_memory();
        info.capabilities.ansi = false;
        info.capabilities.size = Some(TerminalSize {
            width: 20,
            height: 10,
        });
        let mut terminal = InMemoryTerminal::with_info(Vec::<Vec<u8>>::new(), info);
        render_generated_menu(
            &mut terminal,
            &generated_menu(),
            SecurityLevel::new(777).unwrap(),
            SecurityLevel::new(50).unwrap(),
            &[],
        )
        .unwrap();
        assert!(!terminal.output().contains(&0x1b));
        assert!(terminal
            .output()
            .split(|byte| *byte == b'\n')
            .all(|line| line.strip_suffix(b"\r").unwrap_or(line).len() <= 20));
        assert!(contains(terminal.output(), b"<@>"));
        assert!(contains(terminal.output(), b"Utilities"));
        assert!(!contains(terminal.output(), b"Deferred Historical Door"));
    }

    #[test]
    fn all_generated_sections_use_thirty_eight_thirty_stock_geometry() {
        for (section, security, visible) in [
            (MenuSection::Main, 10, 11usize),
            (MenuSection::Message, 10, 13),
            (MenuSection::File, 10, 12),
            (MenuSection::Sysop, 50, 3),
        ] {
            let menu = section_menu(section);
            assert_eq!(
                visible_menu_action_count(
                    &menu,
                    SecurityLevel::new(security).unwrap(),
                    SecurityLevel::new(50).unwrap()
                ),
                visible
            );
            let lines = stock_geometry_lines(&generated_output(&menu, security, true, 80, 25));
            assert_eq!(lines[0].len(), 30, "{section:?} heading");
            assert!(lines[0].contains(&b'>') && lines[0].contains(&b'<'));
            let rows = visible.div_ceil(2);
            assert_eq!(lines.len(), rows + 1);
            for (index, line) in lines[1..].iter().enumerate() {
                let expected = if index + rows < visible { 68 } else { 30 };
                assert_eq!(line.len(), expected, "{section:?} row {index}");
                if expected == 68 {
                    assert_eq!(&line[30..38], b"        ");
                    assert_eq!(line[30], b' ');
                }
            }
        }
    }

    #[test]
    fn encoding_does_not_change_geometry_but_security_changes_only_visible_rows() {
        let menu = section_menu(MenuSection::Main);
        let ansi = generated_output(&menu, 10, true, 80, 25);
        let utf8 = generated_output(&menu, 10, false, 80, 25);
        assert_eq!(stock_geometry_lines(&ansi), stock_geometry_lines(&utf8));
        assert!(!contains(&ansi, b"<@>"));

        let sysop = generated_output(&menu, 50, true, 80, 25);
        assert!(contains(&sysop, b"<@>........... Sysop Utilities"));
        let sysop_lines = stock_geometry_lines(&sysop);
        assert_eq!(sysop_lines.len(), 7);
        assert!(sysop_lines[1..].iter().all(|line| line.len() == 68));
    }

    #[test]
    fn security_levels_change_authorized_contents_without_changing_stock_geometry() {
        for security in [10, 37, 50, 777] {
            for section in [
                MenuSection::Main,
                MenuSection::Message,
                MenuSection::File,
                MenuSection::Sysop,
            ] {
                let menu = section_menu(section);
                let visible = visible_menu_action_count(
                    &menu,
                    SecurityLevel::new(security).unwrap(),
                    SecurityLevel::new(50).unwrap(),
                );
                let lines = stock_geometry_lines(&generated_output(&menu, security, true, 80, 25));
                assert_eq!(lines[0].len(), 30, "{section:?} security {security}");
                let rows = visible.div_ceil(2);
                assert_eq!(lines.len(), rows + 1);
                for (index, line) in lines[1..].iter().enumerate() {
                    let expected = if index + rows < visible { 68 } else { 30 };
                    assert_eq!(line.len(), expected, "{section:?} security {security}");
                    if expected == 68 {
                        assert_eq!(&line[30..38], b"        ");
                    }
                }
                if security < 50 {
                    assert!(!contains(
                        &generated_output(&menu, security, true, 80, 25),
                        b"<@>"
                    ));
                }
            }
        }
    }

    #[test]
    fn wide_and_constrained_terminals_are_deterministic_and_bounded() {
        let menu = section_menu(MenuSection::Main);
        let normal = stock_geometry_lines(&generated_output(&menu, 10, true, 80, 25));
        let wide = stock_geometry_lines(&generated_output(&menu, 10, true, 132, 40));
        assert_eq!(normal, wide);

        let constrained = generated_output(&menu, 10, true, 48, 10);
        assert!(!contains(&constrained, b">>>>>>>>>>"));
        assert!(constrained.split(|byte| *byte == b'\n').all(|line| line
            .strip_suffix(b"\r")
            .unwrap_or(line)
            .len()
            <= 48));
        assert!(contains(&constrained, b"<M> Message Section"));
        assert!(!menu_item_fits(&[b'X'; 26], 30));
    }

    #[test]
    fn semantic_identifier_keeps_localized_label_when_command_letter_changes() {
        let mut menu = section_menu(MenuSection::Main);
        menu.items[0].command = b'Z';
        let output = generated_output(&menu, 10, true, 80, 25);
        assert!(contains(&output, b"<Z>........... Message Section"));
        assert!(!contains(&output, b"<M>........... Message Section"));
    }

    #[test]
    fn pre_m040_queue_identifier_keeps_its_queue_label() {
        let menu = MenuDefinition {
            section: MenuSection::Message,
            items: vec![MenuItem {
                command: b'A',
                description: b"<A> legacy queue".to_vec(),
                required_security: 5,
                identifier: b'X',
            }],
        };
        let output = generated_output(&menu, 10, true, 80, 25);
        assert!(contains(&output, b"<A>.... Alter Conference Queue"));
        assert!(!contains(&output, b"Text Search"));
    }

    #[test]
    fn preserves_cp437_and_renders_safe_controls() {
        let board = BoardIdentity::new("Test Board", "Sysop").unwrap();
        let node = NodeId::new(1).unwrap();
        let mut terminal = InMemoryTerminal::default();
        let resource = DisplayResource {
            format: DisplayFormat::Bbs,
            source: DisplaySource::EngineBuiltIn,
            bytes: b"\xB3 @BOARD@ @NODE@ @PASSWORD@\x07".to_vec(),
        };
        render_display(
            &mut terminal,
            &resource,
            &DisplayContext {
                board: &board,
                node,
                timezone: chrono_tz::UTC,
                caller: None,
                logon_minutes: None,
            },
        )
        .unwrap();
        assert_eq!(terminal.output()[0], 0xB3);
        assert!(terminal
            .output()
            .windows(10)
            .any(|part| part == b"Test Board"));
        assert!(terminal
            .output()
            .windows(22)
            .any(|part| part == b"[PASSWORD UNAVAILABLE]"));
    }

    #[test]
    fn refuses_clr_for_non_ansi_terminal() {
        let board = BoardIdentity::new("B", "S").unwrap();
        let info = TerminalInfo {
            transport: TransportKind::RawTcp,
            local: false,
            capabilities: TerminalCapabilities {
                terminal_type: None,
                ansi: false,
                cp437: true,
                size: Some(TerminalSize {
                    width: 80,
                    height: 25,
                }),
            },
            remote_address: None,
            connected_at: std::time::SystemTime::now(),
            connection_speed: None,
            carrier: None,
            declared_identity: None,
        };
        let mut terminal = InMemoryTerminal::with_info(Vec::<Vec<u8>>::new(), info);
        assert!(matches!(
            render_display(
                &mut terminal,
                &DisplayResource {
                    format: DisplayFormat::Clr,
                    source: DisplaySource::EngineBuiltIn,
                    bytes: b"ansi".to_vec(),
                },
                &DisplayContext {
                    board: &board,
                    node: NodeId::new(1).unwrap(),
                    timezone: chrono_tz::UTC,
                    caller: None,
                    logon_minutes: None,
                },
            ),
            Err(ResourceError::AnsiRequired)
        ));
    }

    #[test]
    fn renders_confirmed_caller_macros_without_exposing_omitted_private_data() {
        let board = BoardIdentity::new("Test Board", "Fixture Sysop").unwrap();
        let mut terminal = InMemoryTerminal::default();
        let context = DisplayContext {
            board: &board,
            node: NodeId::new(3).unwrap(),
            timezone: chrono_tz::UTC,
            caller: Some(DisplayCallerContext {
                full_name: "Alex Caller".to_owned(),
                security_level: 10,
                first_call_at: 0,
                last_call_at: Some(86_400),
                files_uploaded: 2,
                upload_bytes: 3 * 1024,
                files_downloaded: 4,
                download_bytes: 5 * 1024,
                city_region: "Phoenix, Arizona".to_owned(),
                phone: "+1 555 0100".to_owned(),
                birthday: "1980-03-04".to_owned(),
            }),
            logon_minutes: Some(45),
        };
        let resource = DisplayResource {
            format: DisplayFormat::Bbs,
            source: DisplaySource::EngineBuiltIn,
            bytes: b"@FNAME@|@NAME@|@UPLOADS@|@DOWNLOADS@|@UPK@|@DOWNK@|@SLEVEL@|@LOGTIME@|@ORGLOG@|@LASTCALL@|@CITYSTATE@|@PHONENUM@|@BIRTHDATE@|@SUBDATE@|@UNKNOWN@".to_vec(),
        };
        render_display(&mut terminal, &resource, &context).unwrap();
        let output = terminal.output_text().unwrap();
        assert!(output.starts_with(
            "Alex|Alex Caller|2|4|3|5|10|45|1970-01-01 00:00 UTC|1970-01-02 00:00 UTC"
        ));
        assert!(output.contains("|Phoenix, Arizona|+1 555 0100|1980-03-04|"));
        assert!(output.ends_with("|N/A|@UNKNOWN@"));
    }

    #[test]
    fn display_controls_govern_more_and_abort_only_for_the_current_output() {
        let board = BoardIdentity::new("B", "S").unwrap();
        let context = DisplayContext {
            board: &board,
            node: NodeId::new(1).unwrap(),
            timezone: chrono_tz::UTC,
            caller: None,
            logon_minutes: None,
        };
        let info = TerminalInfo {
            capabilities: TerminalCapabilities {
                size: Some(TerminalSize {
                    width: 80,
                    height: 2,
                }),
                ..TerminalCapabilities::default()
            },
            ..TerminalInfo::in_memory()
        };
        let mut inner = InMemoryTerminal::with_info([b"S".to_vec()], info);
        let preferences = crate::CallerPreferences {
            more_prompt: true,
            ..crate::CallerPreferences::default()
        };
        {
            let mut terminal = PagingTerminal::new(&mut inner, preferences);
            let resource = DisplayResource {
                format: DisplayFormat::Bbs,
                source: DisplaySource::EngineBuiltIn,
                bytes: b"@PROMPTOFF@one\r\ntwo\r\n@NOABORT@@PROMPT@1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9\r\n10\r\n11\r\n"
                    .to_vec(),
            };
            render_display(&mut terminal, &resource, &context).unwrap();
            assert!(!terminal.output_aborted());
        }
        let output = inner.output_text().unwrap();
        assert_eq!(output.matches("MORE:").count(), 2);
        assert!(output.contains("11"));
    }

    #[test]
    fn board_local_timestamp_format_does_not_use_a_two_digit_year_pivot() {
        assert_eq!(
            format_board_local_timestamp(0, chrono_tz::UTC).unwrap(),
            "1970-01-01 00:00 UTC"
        );
        assert_eq!(
            format_board_local_timestamp(1_735_689_600, chrono_tz::America::Phoenix).unwrap(),
            "2024-12-31 17:00 MST"
        );
    }
}
