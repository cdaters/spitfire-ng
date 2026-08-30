use std::num::NonZeroU64;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use tracing::{info, warn};

use crate::file_session::{run_file_menu, run_post_login_new_files, FileMenuExit};
use crate::message_session::{
    compose_sysop_comment, message_actor, run_message_menu, ComposeOutcome, MessageMenuExit,
};
use crate::{
    board_local_day, format_board_local_timestamp, render_display, render_generated_menu,
    AccessDenialReason, AuthenticatedCaller, AuthenticationResult, BoardAccessMode, BoardIdentity,
    Caller, CallerConfig, CallerError, CallerId, CallerProfile, CallerSessionContext, CallerState,
    CredentialError, CredentialHasher, DatabaseError, DisplayCallerContext, DisplayContext,
    DisplaySource, FileActor, FileBackend, FileError, FileStorage, GraphicsPreference,
    InteractionError, InteractionHub, MenuDefinition, MenuRendererPath, MenuSection,
    MessageBackend, MessageError, NewOtherBbsEntry, NodeError, NodeId, NodePresentationContext,
    PageAnswer, PagingTerminal, PostLoginJourney, PostalAddress, ProfileFieldPolicy,
    PublicInformationActor, PublicInformationError, ResourceError, RuntimeDatabase, SecurityLevel,
    StockResources, Terminal, TerminalError, TerminalInfo, TerminalTextEncoding,
    ThoughtCatalogReader, TransferDirection,
};

const MAX_MENU_COMMAND_BYTES: usize = 8;
const INVALID_MENU_COMMAND: u8 = 0;
const MAX_CALLER_NAME_INPUT: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(NonZeroU64);

impl SessionId {
    pub fn new(value: u64) -> Result<Self, SessionError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(SessionError::InvalidSessionId(value))
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Created,
    Active,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationState {
    NotStarted,
    Unauthenticated,
    ExistingCallerLogin,
    NewCallerRegistration,
    Authenticated(CallerId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionCloseReason {
    Goodbye,
    EndOfInput,
    TransportLost,
    AuthenticationFailed,
    AccountUnavailable,
    TimeLimit,
    Inactivity,
    OperatorDisconnect,
}

#[derive(Debug)]
pub struct Session {
    id: SessionId,
    node_id: NodeId,
    state: SessionState,
    authentication: AuthenticationState,
    close_reason: Option<SessionCloseReason>,
    authenticated_at: Option<Instant>,
    authenticated_unix_seconds: Option<i64>,
}

impl Session {
    pub(crate) fn new(id: SessionId, node_id: NodeId) -> Self {
        Self {
            id,
            node_id,
            state: SessionState::Created,
            authentication: AuthenticationState::NotStarted,
            close_reason: None,
            authenticated_at: None,
            authenticated_unix_seconds: None,
        }
    }

    pub fn id(&self) -> SessionId {
        self.id
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn authentication_state(&self) -> AuthenticationState {
        self.authentication
    }

    pub fn close_reason(&self) -> Option<SessionCloseReason> {
        self.close_reason
    }

    pub fn activate(&mut self) -> Result<(), SessionError> {
        if self.state != SessionState::Created {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                operation: "activate",
            });
        }
        self.state = SessionState::Active;
        self.authentication = AuthenticationState::Unauthenticated;
        Ok(())
    }

    fn set_authentication(&mut self, state: AuthenticationState) -> Result<(), SessionError> {
        if self.state != SessionState::Active {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                operation: "change authentication state for",
            });
        }
        self.authentication = state;
        Ok(())
    }

    fn mark_authenticated(&mut self, caller_id: CallerId, now: i64) -> Result<(), SessionError> {
        self.set_authentication(AuthenticationState::Authenticated(caller_id))?;
        self.authenticated_at = Some(Instant::now());
        self.authenticated_unix_seconds = Some(now);
        Ok(())
    }

    pub fn accounting(
        &self,
        timezone: chrono_tz::Tz,
    ) -> Result<Option<(CallerId, u64, u64, i64)>, SessionError> {
        let AuthenticationState::Authenticated(caller_id) = self.authentication else {
            return Ok(None);
        };
        let elapsed = self
            .authenticated_at
            .map(|started| started.elapsed())
            .unwrap_or_default();
        let now = unix_seconds()?;
        let today = i64::from(board_local_day(now, timezone)?);
        let authenticated_unix_seconds = self
            .authenticated_unix_seconds
            .ok_or(SessionError::MissingAuthenticationClock)?;
        let daily_elapsed = crate::daily_session_elapsed_seconds(
            authenticated_unix_seconds,
            now,
            elapsed,
            timezone,
        )?;
        Ok(Some((caller_id, elapsed.as_secs(), daily_elapsed, today)))
    }

    pub fn close(&mut self, reason: SessionCloseReason) -> Result<(), SessionError> {
        if self.state != SessionState::Active {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                operation: "close",
            });
        }
        self.state = SessionState::Closed;
        self.close_reason = Some(reason);
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionOutcome {
    pub session_id: SessionId,
    pub node_id: NodeId,
    pub close_reason: SessionCloseReason,
    pub commands_processed: usize,
    pub caller_id: Option<CallerId>,
    pub caller_name: Option<String>,
}

/// Receives privacy-safe session lifecycle changes for the transient node
/// registry. Transport adapters never implement caller identity themselves.
pub trait SessionStatusObserver: Send + Sync {
    fn login_started(&self) -> Result<(), NodeError>;
    fn caller_authenticated(&self, caller_id: CallerId, caller_name: &str)
        -> Result<(), NodeError>;
    fn transfer_started(
        &self,
        direction: TransferDirection,
        filename: &str,
    ) -> Result<(), NodeError>;
    fn transfer_finished(&self) -> Result<(), NodeError>;
    fn page_pending(&self) -> Result<(), NodeError>;
    fn chat_started(&self) -> Result<(), NodeError>;
    fn interaction_finished(&self) -> Result<(), NodeError>;
    fn presentation_changed(
        &self,
        presentation: crate::NodePresentationContext,
    ) -> Result<(), NodeError>;
}

/// Immutable presentation/status dependencies for one stock session. Grouping
/// them keeps the terminal/session entry point small without coupling the core
/// to application or transport types.
#[derive(Clone, Copy)]
pub struct StockSessionContext<'a> {
    pub board: &'a BoardIdentity,
    pub timezone: chrono_tz::Tz,
    pub board_access: BoardAccessMode,
    pub private_security_level: SecurityLevel,
    pub resources: &'a StockResources,
    pub text_resources: &'a StockResources,
    pub status: &'a dyn SessionStatusObserver,
    pub file_storage: &'a FileStorage,
    pub interaction: &'a InteractionHub,
    pub page_timeout: Duration,
    pub chat_timeout: Duration,
    pub presentation_profile: &'a str,
    pub menu_mode: &'a str,
    pub locale: &'a str,
    pub joker_policy: &'a crate::JokerPolicy,
}

/// Runs one stock-oriented SPITFIRE session. Every transport calls this same
/// engine; transport-supplied credentials remain untrusted until the native
/// caller verifier accepts them.
pub fn run_stock_session(
    session: &mut Session,
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    caller_config: &CallerConfig,
    hasher: &CredentialHasher,
    stock: StockSessionContext<'_>,
) -> Result<SessionOutcome, SessionError> {
    terminal.set_idle_timeout(Duration::from_secs(
        u64::from(caller_config.inactivity_minutes).saturating_mul(60),
    ))?;
    let result = run_stock_session_inner(session, terminal, database, caller_config, hasher, stock);
    if !matches!(result, Err(SessionError::Terminal(TerminalError::TimedOut))) {
        return result;
    }

    let caller = match session.authentication_state() {
        AuthenticationState::Authenticated(id) => database.caller_by_id(id)?,
        _ => None,
    };
    let context = DisplayContext {
        board: stock.board,
        node: session.node_id(),
        timezone: stock.timezone,
        caller: caller.as_ref().map(DisplayCallerContext::from_caller),
        logon_minutes: None,
    };
    let resources = caller.as_ref().map_or(stock.text_resources, |caller| {
        if caller
            .preferences
            .graphics
            .allows_ansi(terminal.info().capabilities.ansi)
        {
            stock.resources
        } else {
            stock.text_resources
        }
    });
    if resources.display("SFASLEEP").is_some() {
        render_named_display(terminal, resources, "SFASLEEP", &context)?;
    } else {
        write_key_line(
            terminal,
            "caller-inactivity-goodbye",
            &crate::LocalizationArgs::new(),
        )?;
    }
    if session.state() == SessionState::Active {
        session.close(SessionCloseReason::Inactivity)?;
    }
    terminal.disconnect()?;
    stock.interaction.session_ended(session.id())?;
    warn!(
        session = session.id().get(),
        "caller session ended for inactivity"
    );
    let identity = caller.map(|caller| (caller.id, caller.display_name));
    session_outcome(session, 0, identity)
}

fn run_stock_session_inner(
    session: &mut Session,
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    caller_config: &CallerConfig,
    hasher: &CredentialHasher,
    stock: StockSessionContext<'_>,
) -> Result<SessionOutcome, SessionError> {
    session.activate()?;
    stock.status.login_started()?;
    let negotiated_terminal = terminal.negotiated_info();
    publish_presentation_context(
        &stock,
        &negotiated_terminal,
        &negotiated_terminal,
        None,
        None,
        None,
        caller_config,
    )?;
    let prelogin_context = DisplayContext {
        board: stock.board,
        node: session.node_id(),
        timezone: stock.timezone,
        caller: None,
        logon_minutes: None,
    };
    render_display(terminal, &stock.resources.prelogin, &prelogin_context)?;
    ensure_line_ending(terminal, stock.resources.prelogin.bytes.as_slice())?;
    write_key_line(
        terminal,
        "caller-connection-sysop-node",
        &crate::LocalizationArgs::new()
            .with("sysop", stock.board.sysop_name())
            .with("node", session.node_id().get()),
    )?;
    write_key_line(
        terminal,
        "caller-connection-product-version",
        &crate::LocalizationArgs::new().with("version", crate::PRODUCT_VERSION),
    )?;
    render_display(terminal, &stock.resources.welcome, &prelogin_context)?;
    ensure_line_ending(terminal, stock.resources.welcome.bytes.as_slice())?;

    let Some(mut authenticated) = authenticate_session(
        session,
        terminal,
        database,
        caller_config,
        hasher,
        &stock,
        &prelogin_context,
    )?
    else {
        return session_outcome(session, 0, None);
    };
    let caller_id = authenticated.caller.id;
    let caller_name = authenticated.caller.display_name.clone();
    stock.status.caller_authenticated(caller_id, &caller_name)?;
    let mut terminal = PagingTerminal::new(terminal, authenticated.caller.preferences);
    let terminal = &mut terminal;
    let context = DisplayContext {
        board: stock.board,
        node: session.node_id(),
        timezone: stock.timezone,
        caller: Some(DisplayCallerContext::from_authenticated(&authenticated)),
        logon_minutes: Some(authenticated.allowance.limit_seconds().div_ceil(60)),
    };
    write_authenticated_greeting(
        terminal,
        database,
        &authenticated,
        caller_config,
        stock.timezone,
        unix_seconds()?,
    )?;
    let login_resources = active_resources(terminal, stock.resources, stock.text_resources);
    render_post_login_resources(terminal, login_resources, &context, &authenticated)?;
    if caller_config.post_login_journey == PostLoginJourney::Stock {
        run_stock_post_login_journey(
            terminal,
            database,
            &authenticated,
            caller_config,
            stock.timezone,
        )?;
    }

    let mut section = MenuSection::Main;
    let mut expert = false;
    let mut first_message_entry = true;
    let mut first_file_entry = true;
    let mut commands_processed = 0;
    loop {
        if !refresh_caller_access_for_dispatch(
            session,
            terminal,
            database,
            &mut authenticated,
            caller_config,
            &stock,
            &context,
        )? {
            break;
        }
        let elapsed = session
            .authenticated_at
            .map_or(Duration::ZERO, |started| started.elapsed());
        if authenticated.allowance.expired(elapsed) {
            let resources = active_resources(terminal, stock.resources, stock.text_resources);
            render_policy_display(
                terminal,
                resources,
                "SFTIMEUP",
                &context,
                "Your available SPITFIRE time has expired.",
            )?;
            session.close(SessionCloseReason::TimeLimit)?;
            terminal.disconnect()?;
            warn!(
                caller_id = caller_id.get(),
                "caller session time limit expired"
            );
            break;
        }

        if section == MenuSection::File {
            let resources = active_resources(terminal, stock.resources, stock.text_resources);
            let renderer_path = selected_menu_renderer(
                resources,
                MenuSection::File,
                authenticated.caller.security_level,
                expert,
            );
            publish_presentation_context(
                &stock,
                &terminal.negotiated_info(),
                &terminal.info(),
                Some(resources.menu(MenuSection::File)?),
                Some(authenticated.caller.security_level),
                Some(renderer_path),
                caller_config,
            )?;
            if first_file_entry {
                render_named_display(terminal, resources, "SF1STF", &context)?;
                first_file_entry = false;
            }
            let file_result = run_file_menu(
                resources,
                &context,
                terminal,
                database,
                stock.file_storage,
                stock.status,
                session,
                &mut authenticated,
                caller_config,
                &stock,
                &mut expert,
            )?;
            commands_processed += file_result.commands;
            match file_result.exit {
                FileMenuExit::Main => section = MenuSection::Main,
                FileMenuExit::Message => section = MenuSection::Message,
                FileMenuExit::Sysop => section = MenuSection::Sysop,
                FileMenuExit::Goodbye => {
                    render_display(terminal, &resources.goodbye, &context)?;
                    ensure_line_ending(terminal, &resources.goodbye.bytes)?;
                    session.close(SessionCloseReason::Goodbye)?;
                    terminal.disconnect()?;
                    break;
                }
                FileMenuExit::EndOfInput => {
                    session.close(SessionCloseReason::EndOfInput)?;
                    break;
                }
            }
            continue;
        }

        let resources = active_resources(terminal, stock.resources, stock.text_resources);
        let menu = resources.menu(section)?;
        let menu_display = if expert {
            None
        } else {
            resources.menu_display(section, authenticated.caller.security_level.get())
        };
        let renderer_path = selected_menu_renderer(
            resources,
            section,
            authenticated.caller.security_level,
            expert,
        );
        publish_presentation_context(
            &stock,
            &terminal.negotiated_info(),
            &terminal.info(),
            Some(menu),
            Some(authenticated.caller.security_level),
            Some(renderer_path),
            caller_config,
        )?;
        if !expert {
            let caller_context = CallerSessionContext::from_authenticated(
                &authenticated,
                caller_config,
                stock.timezone,
                unix_seconds()?,
                elapsed,
            )?;
            let status_lines = caller_status_lines(&caller_context)?;
            if let Some(display) = menu_display {
                render_display(terminal, display, &context)?;
                ensure_line_ending(terminal, &display.bytes)?;
                render_caller_status(terminal, &status_lines)?;
            } else {
                render_generated_menu(
                    terminal,
                    menu,
                    authenticated.caller.security_level,
                    SecurityLevel::new(caller_config.sysop_security)?,
                    &status_lines,
                )?;
            }
        }
        write_key(
            terminal,
            section.prompt_key(),
            &crate::LocalizationArgs::new(),
        )?;
        let Some(command) = read_menu_command(terminal, authenticated.caller.preferences.hot_keys)?
        else {
            session.close(SessionCloseReason::EndOfInput)?;
            break;
        };
        commands_processed += 1;
        if !refresh_caller_access_for_dispatch(
            session,
            terminal,
            database,
            &mut authenticated,
            caller_config,
            &stock,
            &context,
        )? {
            break;
        }
        let Some(item) = menu.find(command, authenticated.caller.security_level.get()) else {
            write_key_line(
                terminal,
                "session-invalid-selection",
                &crate::LocalizationArgs::new(),
            )?;
            continue;
        };

        match (section, item.identifier) {
            (MenuSection::Main, b'E') | (MenuSection::File, b'E') => {
                if first_message_entry {
                    render_named_display(terminal, resources, "SF1STM", &context)?;
                    first_message_entry = false;
                }
                publish_presentation_context(
                    &stock,
                    &terminal.negotiated_info(),
                    &terminal.info(),
                    Some(resources.menu(MenuSection::Message)?),
                    Some(authenticated.caller.security_level),
                    Some(selected_menu_renderer(
                        resources,
                        MenuSection::Message,
                        authenticated.caller.security_level,
                        expert,
                    )),
                    caller_config,
                )?;
                let message_result = run_message_menu(
                    resources,
                    &context,
                    terminal,
                    database,
                    session,
                    &stock,
                    &mut authenticated,
                    caller_config,
                    &mut expert,
                )?;
                commands_processed += message_result.commands;
                match message_result.exit {
                    MessageMenuExit::Main => section = MenuSection::Main,
                    MessageMenuExit::File => section = MenuSection::File,
                    MessageMenuExit::Sysop => section = MenuSection::Sysop,
                    MessageMenuExit::Goodbye => {
                        render_display(terminal, &resources.goodbye, &context)?;
                        ensure_line_ending(terminal, &resources.goodbye.bytes)?;
                        session.close(SessionCloseReason::Goodbye)?;
                        terminal.disconnect()?;
                        break;
                    }
                    MessageMenuExit::EndOfInput => {
                        session.close(SessionCloseReason::EndOfInput)?;
                        break;
                    }
                }
            }
            (MenuSection::Main, b'Q') | (MenuSection::Message, b'D') => {
                section = MenuSection::File;
            }
            (MenuSection::Message, b'C') | (MenuSection::File, b'C') => {
                section = MenuSection::Main;
            }
            (MenuSection::Main, b'F')
            | (MenuSection::Message, b'R')
            | (MenuSection::File, b'F') => {
                let threshold = SecurityLevel::new(caller_config.sysop_security)?;
                if authenticated.caller.security_level.is_sysop(threshold) {
                    section = MenuSection::Sysop;
                } else {
                    write_line(
                        terminal,
                        "Sysop Utilities require the configured Sysop security threshold.",
                    )?;
                }
            }
            (MenuSection::Sysop, b'C') => {
                section = MenuSection::Main;
            }
            (MenuSection::Main, b'G') => {
                show_caller_statistics(terminal, database, &authenticated, caller_config)?
            }
            (MenuSection::Main, b'H') => {
                run_sysop_page(session, terminal, &authenticated, &stock, &context)?
            }
            (MenuSection::Main, b'R') => {
                edit_terminal_preferences(terminal, database, &mut authenticated)?
            }
            (MenuSection::Main, b'Y') if item.command.eq_ignore_ascii_case(&b'R') => {
                edit_caller_profile(terminal, database, &mut authenticated, caller_config)?
            }
            (MenuSection::Main, b'D') => {
                edit_caller_profile(terminal, database, &mut authenticated, caller_config)?
            }
            (MenuSection::Main, b'I') if item.command.eq_ignore_ascii_case(&b'A') => {
                show_about(terminal, resources, &context)?
            }
            (MenuSection::Main, b'V') => show_about(terminal, resources, &context)?,
            (MenuSection::Main, b'L') => {
                show_caller_directory(terminal, database, &mut authenticated, stock.timezone)?
            }
            (MenuSection::Main, b'I') => locate_caller(terminal, database, stock.timezone)?,
            (MenuSection::Main, b'P') => show_other_bbs(terminal, database)?,
            (MenuSection::Main, b'C') => add_other_bbs(terminal, database, &authenticated)?,
            (MenuSection::Main, b'Y') => show_bulletins(terminal, resources, &context)?,
            (MenuSection::Main, b'X') => {
                show_newsletter(terminal, database, resources, &context, &authenticated)?
            }
            (MenuSection::Main, b'K') => {
                show_system_information(terminal, database, stock.board, stock.timezone)?
            }
            (MenuSection::Main, b'J') => {
                if compose_sysop_comment(terminal, database, &authenticated, caller_config)?
                    == ComposeOutcome::Disconnected
                {
                    session.close(SessionCloseReason::EndOfInput)?;
                    break;
                }
            }
            (_, b'A') => {
                render_display(terminal, &resources.goodbye, &context)?;
                ensure_line_ending(terminal, &resources.goodbye.bytes)?;
                session.close(SessionCloseReason::Goodbye)?;
                terminal.disconnect()?;
                break;
            }
            (_, b'B') => {
                expert = !expert;
                write_line(
                    terminal,
                    if expert {
                        "Xpert command mode is ON."
                    } else {
                        "Xpert command mode is OFF."
                    },
                )?;
            }
            (_, b'?') => show_help(
                section,
                menu,
                resources,
                terminal,
                authenticated.caller.security_level,
                authenticated.caller.preferences.hot_keys,
            )?,
            _ => {
                terminal.write_all(b"\r\n")?;
                write_key_line(
                    terminal,
                    "session-command-unavailable",
                    &crate::LocalizationArgs::new().with(
                        "command",
                        String::from_utf8_lossy(&item.description).into_owned(),
                    ),
                )?;
            }
        }
    }

    info!(caller_id = caller_id.get(), "caller logged off");
    stock.interaction.session_ended(session.id())?;
    session_outcome(session, commands_processed, Some((caller_id, caller_name)))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn refresh_caller_access_for_dispatch(
    session: &mut Session,
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    authenticated: &mut AuthenticatedCaller,
    caller_config: &CallerConfig,
    stock: &StockSessionContext<'_>,
    context: &DisplayContext<'_>,
) -> Result<bool, SessionError> {
    if stock.interaction.take_disconnect(session.id())? {
        write_key_line(
            terminal,
            "caller-operator-disconnected",
            &crate::LocalizationArgs::new(),
        )?;
        session.close(SessionCloseReason::OperatorDisconnect)?;
        terminal.disconnect()?;
        return Ok(false);
    }

    let caller_id = authenticated.caller.id;
    let dispatch_policy = database.enforce_caller_access_at_dispatch(
        caller_id,
        caller_config,
        unix_seconds()?,
        stock.timezone,
    )?;
    let refreshed = database
        .caller_by_id(caller_id)?
        .ok_or(DatabaseError::MissingCaller(caller_id.get()))?;
    if refreshed.state != CallerState::Active {
        let resources = active_resources(terminal, stock.resources, stock.text_resources);
        render_policy_display(
            terminal,
            resources,
            "LOCKOUT",
            context,
            "This caller account is unavailable.",
        )?;
        session.close(SessionCloseReason::AccountUnavailable)?;
        terminal.disconnect()?;
        return Ok(false);
    }
    authenticated.caller = refreshed;
    if dispatch_policy.adjustment_applied {
        let resources = active_resources(terminal, stock.resources, stock.text_resources);
        render_policy_display(
            terminal,
            resources,
            "SFSUBCHG",
            context,
            "Your subscription access level has changed.",
        )?;
    }
    Ok(true)
}

fn publish_presentation_context(
    stock: &StockSessionContext<'_>,
    negotiated: &TerminalInfo,
    effective: &TerminalInfo,
    menu: Option<&MenuDefinition>,
    security: Option<SecurityLevel>,
    renderer_path: Option<MenuRendererPath>,
    caller_config: &CallerConfig,
) -> Result<(), NodeError> {
    let negotiated_size = negotiated.capabilities.size;
    let effective_size = effective.capabilities.size;
    let sysop_threshold = SecurityLevel::new(caller_config.sysop_security)
        .expect("validated caller configuration has a valid Sysop threshold");
    stock.status.presentation_changed(NodePresentationContext {
        terminal_type: negotiated.capabilities.terminal_type.clone(),
        ansi: effective.capabilities.ansi,
        encoding: match crate::terminal_text_encoding(effective) {
            TerminalTextEncoding::Utf8 => "utf-8",
            TerminalTextEncoding::Cp437 => "cp437",
            TerminalTextEncoding::Ascii => "ascii",
        }
        .to_owned(),
        columns: negotiated_size.map(|size| size.width),
        rows: negotiated_size.map(|size| size.height),
        page_length: effective_size.map(|size| size.height),
        locale: stock.locale.to_owned(),
        presentation_profile: stock.presentation_profile.to_owned(),
        menu_mode: stock.menu_mode.to_owned(),
        menu_context: menu.map(|menu| {
            match menu.section {
                MenuSection::Main => "main",
                MenuSection::Message => "message",
                MenuSection::File => "file",
                MenuSection::Sysop => "sysop",
            }
            .to_owned()
        }),
        renderer_path,
        caller_security: security.map(SecurityLevel::get),
        sysop_threshold: sysop_threshold.get(),
        visible_action_count: menu.zip(security).map(|(menu, security)| {
            crate::visible_menu_action_count(menu, security, sysop_threshold)
        }),
    })
}

fn selected_menu_renderer(
    resources: &StockResources,
    section: MenuSection,
    security: SecurityLevel,
    expert: bool,
) -> MenuRendererPath {
    if expert {
        MenuRendererPath::ExpertSuppressed
    } else {
        match resources
            .menu_display(section, security.get())
            .map(|display| display.source)
        {
            Some(DisplaySource::BoardOverride) => MenuRendererPath::ExactSecurityBoardOverride,
            Some(DisplaySource::ActiveProfile | DisplaySource::BaseProfile) => {
                MenuRendererPath::ExactSecurityActiveProfile
            }
            Some(DisplaySource::EngineBuiltIn) | None => MenuRendererPath::GeneratedStock,
        }
    }
}

fn authenticate_session(
    session: &mut Session,
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    config: &CallerConfig,
    hasher: &CredentialHasher,
    stock: &StockSessionContext<'_>,
    context: &DisplayContext<'_>,
) -> Result<Option<AuthenticatedCaller>, SessionError> {
    if let Some(grant) = terminal.take_verified_caller_grant() {
        let Some(caller) = database.caller_by_id(grant.caller_id)? else {
            warn!("verified transport grant referred to a missing caller");
            session.close(SessionCloseReason::AuthenticationFailed)?;
            terminal.disconnect()?;
            return Ok(None);
        };
        if reject_joker_name(
            session,
            terminal,
            database,
            caller.display_name.as_bytes(),
            Some(caller.id),
            stock,
            context,
        )? {
            return Ok(None);
        }
        if caller.state != CallerState::Active {
            database.record_caller_access_denial(
                caller.id,
                unix_seconds()?,
                AccessDenialReason::AccountUnavailable,
            )?;
            warn!(
                caller_id = caller.id.get(),
                "disabled or deleted SSH caller rejected at session dispatch"
            );
            render_policy_display(
                terminal,
                stock.resources,
                "LOCKOUT",
                context,
                "This caller account is unavailable.",
            )?;
            session.close(SessionCloseReason::AccountUnavailable)?;
            terminal.disconnect()?;
            return Ok(None);
        }
        if caller.state_version != grant.authenticated_state_version {
            info!(
                caller_id = caller.id.get(),
                authenticated_state_version = grant.authenticated_state_version,
                current_state_version = caller.state_version,
                "SSH caller state changed after transport authentication; current state reauthorized"
            );
        }
        info!(
            caller_id = caller.id.get(),
            "caller login succeeded through SSH authentication"
        );
        return begin_or_close(session, terminal, database, config, caller, stock, context);
    }
    if let Some(credentials) = terminal.take_supplied_credentials() {
        let known_caller = known_caller_for_login(database, credentials.username())?;
        if reject_joker_name(
            session,
            terminal,
            database,
            credentials.username(),
            known_caller.as_ref().map(|caller| caller.id),
            stock,
            context,
        )? {
            return Ok(None);
        }
        let result = if credentials.password().len() <= config.maximum_password_length {
            database.authenticate(credentials.username(), credentials.password(), hasher)
        } else {
            Ok(AuthenticationResult::Invalid)
        };
        match result {
            Ok(AuthenticationResult::Valid(caller)) => {
                info!(
                    caller_id = caller.id.get(),
                    "caller login succeeded through configured RLogin auto-login"
                );
                return begin_or_close(session, terminal, database, config, caller, stock, context);
            }
            Ok(AuthenticationResult::Unavailable(caller)) => {
                database.record_caller_access_denial(
                    caller.id,
                    unix_seconds()?,
                    AccessDenialReason::AccountUnavailable,
                )?;
                warn!(
                    caller_id = caller.id.get(),
                    "disabled or deleted caller rejected"
                );
                render_policy_display(
                    terminal,
                    stock.resources,
                    "LOCKOUT",
                    context,
                    "This caller account is unavailable.",
                )?;
                session.close(SessionCloseReason::AccountUnavailable)?;
                terminal.disconnect()?;
                return Ok(None);
            }
            Ok(AuthenticationResult::Invalid) | Err(DatabaseError::InvalidCaller(_)) => {
                if let Some(caller) = known_caller.as_ref() {
                    database.record_caller_access_denial(
                        caller.id,
                        unix_seconds()?,
                        AccessDenialReason::InvalidCredentials,
                    )?;
                }
                warn!("RLogin supplied credentials were not accepted");
                write_line(
                    terminal,
                    "Automatic RLogin login was not accepted; continuing with normal login.",
                )?;
            }
            Err(error) => return Err(error.into()),
        }
    }

    if stock.board_access.is_private() {
        session.set_authentication(AuthenticationState::ExistingCallerLogin)?;
        return login_existing_caller(session, terminal, database, config, hasher, stock, context);
    }
    write_key(
        terminal,
        "caller-auth-new-question",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(answer) = terminal.read_line(8)? else {
        session.close(SessionCloseReason::EndOfInput)?;
        return Ok(None);
    };
    if answer
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'Y'))
    {
        session.set_authentication(AuthenticationState::NewCallerRegistration)?;
        register_new_caller(session, terminal, database, config, hasher, stock, context)
    } else {
        session.set_authentication(AuthenticationState::ExistingCallerLogin)?;
        login_existing_caller(session, terminal, database, config, hasher, stock, context)
    }
}

fn login_existing_caller(
    session: &mut Session,
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    config: &CallerConfig,
    hasher: &CredentialHasher,
    stock: &StockSessionContext<'_>,
    context: &DisplayContext<'_>,
) -> Result<Option<AuthenticatedCaller>, SessionError> {
    for _ in 0..config.maximum_login_attempts {
        write_key(
            terminal,
            "caller-auth-name-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(name) = terminal.read_line(MAX_CALLER_NAME_INPUT)? else {
            session.close(SessionCloseReason::EndOfInput)?;
            return Ok(None);
        };
        let known_caller = known_caller_for_login(database, &name)?;
        if reject_joker_name(
            session,
            terminal,
            database,
            &name,
            known_caller.as_ref().map(|caller| caller.id),
            stock,
            context,
        )? {
            return Ok(None);
        }
        write_key(
            terminal,
            "caller-auth-password-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let Some(mut password) = terminal.read_secret_line(config.maximum_password_length)? else {
            session.close(SessionCloseReason::EndOfInput)?;
            return Ok(None);
        };
        terminal.write_all(b"\r\n")?;
        let result = database.authenticate(&name, &password, hasher);
        password.fill(0);
        match result {
            Ok(AuthenticationResult::Valid(caller)) => {
                info!(caller_id = caller.id.get(), "caller login succeeded");
                return begin_or_close(session, terminal, database, config, caller, stock, context);
            }
            Ok(AuthenticationResult::Unavailable(caller)) => {
                database.record_caller_access_denial(
                    caller.id,
                    unix_seconds()?,
                    AccessDenialReason::AccountUnavailable,
                )?;
                warn!(
                    caller_id = caller.id.get(),
                    "disabled or deleted caller rejected"
                );
                render_policy_display(
                    terminal,
                    stock.resources,
                    "LOCKOUT",
                    context,
                    "This caller account is unavailable.",
                )?;
                session.close(SessionCloseReason::AccountUnavailable)?;
                terminal.disconnect()?;
                return Ok(None);
            }
            Ok(AuthenticationResult::Invalid) | Err(DatabaseError::InvalidCaller(_)) => {
                if let Some(caller) = known_caller_for_login(database, &name)? {
                    database.record_caller_access_denial(
                        caller.id,
                        unix_seconds()?,
                        AccessDenialReason::InvalidCredentials,
                    )?;
                }
                warn!("caller login failed");
                if stock.resources.display("SFONFAIL").is_some() {
                    render_named_display(terminal, stock.resources, "SFONFAIL", context)?;
                } else {
                    write_key_line(
                        terminal,
                        "caller-login-failed",
                        &crate::LocalizationArgs::new(),
                    )?;
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    if stock.board_access.is_private() {
        render_policy_display(
            terminal,
            stock.resources,
            "PRIVATE",
            context,
            "This private board requires a pre-authorized caller account.",
        )?;
    } else {
        write_key_line(
            terminal,
            "caller-login-attempts-exceeded",
            &crate::LocalizationArgs::new(),
        )?;
    }
    session.close(SessionCloseReason::AuthenticationFailed)?;
    terminal.disconnect()?;
    Ok(None)
}

fn known_caller_for_login(
    database: &RuntimeDatabase,
    name: &[u8],
) -> Result<Option<Caller>, SessionError> {
    match database.caller_by_name(name) {
        Ok(caller) => Ok(caller),
        Err(DatabaseError::InvalidCaller(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn reject_joker_name(
    session: &mut Session,
    terminal: &mut dyn Terminal,
    database: &RuntimeDatabase,
    name: &[u8],
    caller_id: Option<CallerId>,
    stock: &StockSessionContext<'_>,
    context: &DisplayContext<'_>,
) -> Result<bool, SessionError> {
    if stock.joker_policy.denial_for(name).ok().flatten().is_none() {
        return Ok(false);
    }
    database.record_joker_denial(caller_id, stock.joker_policy.generation(), unix_seconds()?)?;
    render_policy_display(
        terminal,
        stock.resources,
        "LOCKOUT",
        context,
        "This caller name is unavailable.",
    )?;
    session.close(SessionCloseReason::AccountUnavailable)?;
    terminal.disconnect()?;
    warn!(
        caller_id = caller_id.map(CallerId::get),
        policy_generation = stock.joker_policy.generation(),
        "caller denied by JOKER name policy"
    );
    Ok(true)
}

fn register_new_caller(
    session: &mut Session,
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    config: &CallerConfig,
    hasher: &CredentialHasher,
    stock: &StockSessionContext<'_>,
    context: &DisplayContext<'_>,
) -> Result<Option<AuthenticatedCaller>, SessionError> {
    let name = loop {
        write_key(
            terminal,
            "caller-registration-name-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let name = match terminal.read_line(MAX_CALLER_NAME_INPUT) {
            Ok(Some(name)) => name,
            Ok(None) => {
                session.close(SessionCloseReason::EndOfInput)?;
                return Ok(None);
            }
            Err(TerminalError::InputTooLong { .. }) => {
                write_key_line(
                    terminal,
                    "caller-registration-name-too-long",
                    &crate::LocalizationArgs::new(),
                )?;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        if reject_joker_name(session, terminal, database, &name, None, stock, context)? {
            return Ok(None);
        }
        match database.caller_by_name(&name) {
            Ok(Some(_)) => {
                write_line(
                    terminal,
                    "That caller name is already registered. Continuing with returning-caller login.",
                )?;
                session.set_authentication(AuthenticationState::ExistingCallerLogin)?;
                return login_existing_caller(
                    session, terminal, database, config, hasher, stock, context,
                );
            }
            Ok(None) => break name,
            Err(DatabaseError::InvalidCaller(_)) => {
                write_key_line(
                    terminal,
                    "caller-registration-name-invalid",
                    &crate::LocalizationArgs::new(),
                )?;
            }
            Err(error) => return Err(error.into()),
        }
    };
    let password_hash = loop {
        write_key(
            terminal,
            "caller-registration-password-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        let mut password = match terminal.read_secret_line(config.maximum_password_length) {
            Ok(Some(password)) => password,
            Ok(None) => {
                session.close(SessionCloseReason::EndOfInput)?;
                return Ok(None);
            }
            Err(TerminalError::InputTooLong { .. }) => {
                write_line(
                    terminal,
                    "Password is longer than the configured maximum. Please try again.",
                )?;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        terminal.write_all(b"\r\n")?;
        write_key(
            terminal,
            "caller-registration-password-confirm",
            &crate::LocalizationArgs::new(),
        )?;
        let mut confirmation = match terminal.read_secret_line(config.maximum_password_length) {
            Ok(Some(confirmation)) => confirmation,
            Ok(None) => {
                password.fill(0);
                session.close(SessionCloseReason::EndOfInput)?;
                return Ok(None);
            }
            Err(TerminalError::InputTooLong { .. }) => {
                password.fill(0);
                write_line(
                    terminal,
                    "Password confirmation is too long. Please try again.",
                )?;
                continue;
            }
            Err(error) => {
                password.fill(0);
                return Err(error.into());
            }
        };
        terminal.write_all(b"\r\n")?;
        if password.len() < config.minimum_password_length {
            password.fill(0);
            confirmation.fill(0);
            write_line(
                terminal,
                "Password is shorter than the configured minimum. Please try again.",
            )?;
            continue;
        }
        if password != confirmation {
            password.fill(0);
            confirmation.fill(0);
            write_line(
                terminal,
                "Password confirmation did not match. Please try again.",
            )?;
            continue;
        }
        confirmation.fill(0);
        let hash_result = hasher.hash(&password);
        password.fill(0);
        break hash_result?;
    };
    let now = unix_seconds()?;
    let mut profile = CallerProfile::default();
    let caller = loop {
        let Some(collected) = collect_caller_profile(terminal, &config.profile, profile)? else {
            write_line(
                terminal,
                "New caller registration canceled. Returning to caller login.",
            )?;
            session.set_authentication(AuthenticationState::ExistingCallerLogin)?;
            return login_existing_caller(
                session, terminal, database, config, hasher, stock, context,
            );
        };
        profile = collected;
        match database.create_caller_with_profile(
            &name,
            &password_hash,
            SecurityLevel::new(config.new_caller_security)?,
            CallerState::Active,
            true,
            now,
            profile.clone(),
            &config.profile,
        ) {
            Ok(caller) => break caller,
            Err(DatabaseError::DuplicateCaller(_)) => {
                write_line(
                    terminal,
                    "That caller name was registered by another session. Continuing with returning-caller login.",
                )?;
                session.set_authentication(AuthenticationState::ExistingCallerLogin)?;
                return login_existing_caller(
                    session, terminal, database, config, hasher, stock, context,
                );
            }
            Err(DatabaseError::InvalidStoredCaller(error)) => {
                if profile_validation_is_recoverable(terminal, &error)? {
                    continue;
                }
                return Err(DatabaseError::InvalidStoredCaller(error).into());
            }
            Err(error) => return Err(error.into()),
        }
    };
    info!(caller_id = caller.id.get(), "new caller created");
    write_key_line(
        terminal,
        "caller-registration-complete",
        &crate::LocalizationArgs::new(),
    )?;
    begin_or_close(session, terminal, database, config, caller, stock, context)
}

fn begin_or_close(
    session: &mut Session,
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    config: &CallerConfig,
    mut caller: Caller,
    stock: &StockSessionContext<'_>,
    context: &DisplayContext<'_>,
) -> Result<Option<AuthenticatedCaller>, SessionError> {
    let now = unix_seconds()?;
    let subscription =
        database.evaluate_caller_subscription(caller.id, config, now, stock.timezone)?;
    if subscription.adjustment_applied {
        render_policy_display(
            terminal,
            stock.resources,
            "SFSUBCHG",
            context,
            "Your subscription access level has changed.",
        )?;
    } else if subscription.warning {
        render_policy_display(
            terminal,
            stock.resources,
            "SUBWARN",
            context,
            "Your SPITFIRE subscription is nearing expiration.",
        )?;
    }
    caller = database
        .caller_by_id(caller.id)?
        .ok_or(DatabaseError::MissingCaller(caller.id.get()))?;
    if stock.board_access.is_private()
        && caller.security_level.get() < stock.private_security_level.get()
    {
        database.record_caller_access_denial(
            caller.id,
            now,
            AccessDenialReason::PrivateBoardPolicy,
        )?;
        render_policy_display(
            terminal,
            stock.resources,
            "PRIVATE",
            context,
            "Your caller account is not authorized for this private board.",
        )?;
        session.close(SessionCloseReason::AccountUnavailable)?;
        terminal.disconnect()?;
        return Ok(None);
    }
    match database.begin_caller_session(&caller, config, now, stock.timezone) {
        Ok(authenticated) => {
            session.mark_authenticated(caller.id, now)?;
            Ok(Some(authenticated))
        }
        Err(DatabaseError::DailyCallLimitReached) => {
            database.record_caller_access_denial(
                caller.id,
                now,
                AccessDenialReason::DailyCallLimit,
            )?;
            render_policy_display(
                terminal,
                stock.resources,
                "TOOMANY",
                context,
                "Maximum daily caller access has been reached.",
            )?;
            session.close(SessionCloseReason::TimeLimit)?;
            terminal.disconnect()?;
            Ok(None)
        }
        Err(DatabaseError::DailyTimeLimitReached) => {
            database.record_caller_access_denial(
                caller.id,
                now,
                AccessDenialReason::DailyTimeLimit,
            )?;
            render_policy_display(
                terminal,
                stock.resources,
                "SFTIMEUP",
                context,
                "Your daily SPITFIRE time allowance has been exhausted.",
            )?;
            session.close(SessionCloseReason::TimeLimit)?;
            terminal.disconnect()?;
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}

fn render_policy_display(
    terminal: &mut dyn Terminal,
    resources: &StockResources,
    stem: &str,
    context: &DisplayContext<'_>,
    fallback: &str,
) -> Result<(), SessionError> {
    if resources.display(stem).is_some() {
        render_named_display(terminal, resources, stem, context)
    } else {
        write_line(terminal, fallback).map_err(Into::into)
    }
}

fn collect_caller_profile(
    terminal: &mut dyn Terminal,
    policy: &crate::CallerProfilePolicy,
    current: CallerProfile,
) -> Result<Option<CallerProfile>, SessionError> {
    write_line(
        terminal,
        "Enter /Q at any profile prompt to cancel registration.",
    )?;
    let original = current.clone();
    let mut profile = current;
    loop {
        for field in PROFILE_COLLECTION_ORDER {
            let field_policy = field.policy(policy);
            if !field_policy.enabled() {
                continue;
            }
            if !collect_profile_field(terminal, field, field_policy, &mut profile)? {
                return Ok(None);
            }
        }
        match profile
            .clone()
            .validate_update_for_policy(&original, policy)
        {
            Ok(profile) => return Ok(Some(profile)),
            Err(error) => {
                if !profile_validation_is_recoverable(terminal, &error)? {
                    return Err(error.into());
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ProfileCollectionField {
    Address,
    Phone,
    Email,
    Birthday,
}

const PROFILE_COLLECTION_ORDER: [ProfileCollectionField; 4] = [
    ProfileCollectionField::Address,
    ProfileCollectionField::Phone,
    ProfileCollectionField::Email,
    ProfileCollectionField::Birthday,
];

impl ProfileCollectionField {
    const fn policy(self, policy: &crate::CallerProfilePolicy) -> ProfileFieldPolicy {
        match self {
            Self::Address => policy.address,
            Self::Phone => policy.phone,
            Self::Email => policy.email,
            Self::Birthday => policy.birthday,
        }
    }
}

fn collect_profile_field(
    terminal: &mut dyn Terminal,
    field: ProfileCollectionField,
    policy: ProfileFieldPolicy,
    profile: &mut CallerProfile,
) -> Result<bool, SessionError> {
    match field {
        ProfileCollectionField::Address => {
            write_key_line(
                terminal,
                "caller-registration-address-title",
                &crate::LocalizationArgs::new(),
            )?;
            let Some(address) = prompt_address(terminal, policy, &profile.address)? else {
                return Ok(false);
            };
            profile.address = address;
        }
        ProfileCollectionField::Phone => {
            let Some(value) =
                prompt_profile_value(terminal, "Phone", policy, profile.phone.as_deref(), 40)?
            else {
                return Ok(false);
            };
            profile.phone = value;
        }
        ProfileCollectionField::Email => loop {
            let Some(value) =
                prompt_profile_value(terminal, "Email", policy, profile.email.as_deref(), 254)?
            else {
                return Ok(false);
            };
            match value
                .as_deref()
                .map_or(Ok(()), crate::caller::validate_email)
            {
                Ok(()) => {
                    profile.email = value;
                    break;
                }
                Err(CallerError::InvalidEmail) => write_line(
                    terminal,
                    if policy.required() {
                        "Please enter a valid email address; this information is required by the Sysop."
                    } else {
                        "Please enter a valid email address or leave it blank if optional."
                    },
                )?,
                Err(error) => return Err(error.into()),
            }
        },
        ProfileCollectionField::Birthday => loop {
            let current_birthday = profile.birthday_iso();
            let Some(value) = prompt_profile_text(
                terminal,
                "Birth Date (YYYY-MM-DD)",
                policy,
                current_birthday.as_deref(),
                10,
            )?
            else {
                return Ok(false);
            };
            match crate::parse_birth_date(value.as_deref().unwrap_or_default()) {
                Ok(birthday) => {
                    profile.birthday = birthday;
                    break;
                }
                Err(_) => write_key_line(
                    terminal,
                    "caller-registration-birthday-invalid",
                    &crate::LocalizationArgs::new(),
                )?,
            }
        },
    }
    Ok(true)
}

fn profile_validation_is_recoverable(
    terminal: &mut dyn Terminal,
    error: &CallerError,
) -> Result<bool, SessionError> {
    let message = match error {
        CallerError::RequiredProfileField(field) => format!(
            "The profile is incomplete: {field} is required by the Sysop. Please correct the profile."
        ),
        CallerError::InvalidProfileField { field, maximum } => format!(
            "The {field} value is invalid; enter at most {maximum} bytes without control characters. Please correct the profile."
        ),
        CallerError::InvalidEmail => {
            "The email address is invalid. Please correct the profile.".to_owned()
        }
        CallerError::InvalidBirthDate => {
            "The birth date is invalid. Please correct the profile.".to_owned()
        }
        _ => return Ok(false),
    };
    write_line(terminal, &message)?;
    Ok(true)
}

fn prompt_address(
    terminal: &mut dyn Terminal,
    policy: ProfileFieldPolicy,
    current: &PostalAddress,
) -> Result<Option<PostalAddress>, SessionError> {
    let required = policy.required();
    let line_1_policy = if required {
        ProfileFieldPolicy::Required
    } else {
        policy
    };
    let city_policy = line_1_policy;
    let country_policy = line_1_policy;
    let Some(line_1) = prompt_profile_text(
        terminal,
        "Address Line 1",
        line_1_policy,
        current.line_1.as_deref(),
        120,
    )?
    else {
        return Ok(None);
    };
    let Some(line_2) = prompt_profile_text(
        terminal,
        "Address Line 2",
        ProfileFieldPolicy::Optional,
        current.line_2.as_deref(),
        120,
    )?
    else {
        return Ok(None);
    };
    let Some(city) =
        prompt_profile_text(terminal, "City", city_policy, current.city.as_deref(), 80)?
    else {
        return Ok(None);
    };
    let Some(region) = prompt_profile_text(
        terminal,
        "State / Province / Region",
        ProfileFieldPolicy::Optional,
        current.region.as_deref(),
        80,
    )?
    else {
        return Ok(None);
    };
    let Some(postal_code) = prompt_profile_text(
        terminal,
        "Postal Code",
        ProfileFieldPolicy::Optional,
        current.postal_code.as_deref(),
        32,
    )?
    else {
        return Ok(None);
    };
    let Some(country) = prompt_profile_text(
        terminal,
        "Country",
        country_policy,
        current.country.as_deref(),
        80,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(PostalAddress {
        line_1,
        line_2,
        city,
        region,
        postal_code,
        country,
    }))
}

fn prompt_profile_value(
    terminal: &mut dyn Terminal,
    label: &str,
    policy: ProfileFieldPolicy,
    current: Option<&str>,
    maximum: usize,
) -> Result<Option<Option<String>>, SessionError> {
    prompt_profile_text(terminal, label, policy, current, maximum)
}

/// `None` is cancellation; `Some(None)` is a deliberately empty optional
/// value. During editing an empty line retains the displayed current value and
/// a single dash clears an optional value.
fn prompt_profile_text(
    terminal: &mut dyn Terminal,
    label: &str,
    policy: ProfileFieldPolicy,
    current: Option<&str>,
    maximum: usize,
) -> Result<Option<Option<String>>, SessionError> {
    loop {
        let suffix = current.map_or_else(String::new, |value| format!(" [{value}]"));
        terminal.write_all(format!("{label}{suffix}: ").as_bytes())?;
        let input = match terminal.read_line(maximum) {
            Ok(Some(input)) => input,
            Ok(None) => return Ok(None),
            Err(TerminalError::InputTooLong { .. }) => {
                write_line(
                    terminal,
                    &format!("That value is too long; enter at most {maximum} bytes."),
                )?;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let value = String::from_utf8_lossy(&input).trim().to_owned();
        if value.eq_ignore_ascii_case("/Q") {
            return Ok(None);
        }
        if value.is_empty() {
            if let Some(current) = current {
                return Ok(Some(Some(current.to_owned())));
            }
            if policy.required() {
                write_key_line(
                    terminal,
                    "caller-registration-required",
                    &crate::LocalizationArgs::new(),
                )?;
                continue;
            }
            return Ok(Some(None));
        }
        if value == "-" && !policy.required() {
            return Ok(Some(None));
        }
        if value.chars().any(char::is_control) {
            write_line(
                terminal,
                "That value contains an unsupported control character.",
            )?;
            continue;
        }
        return Ok(Some(Some(value)));
    }
}

fn edit_caller_profile(
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    authenticated: &mut AuthenticatedCaller,
    config: &CallerConfig,
) -> Result<(), SessionError> {
    if config.profile.all_disabled() {
        return write_line(
            terminal,
            "The Sysop has disabled optional caller-profile fields.",
        )
        .map_err(Into::into);
    }
    write_key_line(
        terminal,
        "caller-profile-title",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(profile) = collect_caller_profile(
        terminal,
        &config.profile,
        authenticated.caller.profile.clone(),
    )?
    else {
        write_key_line(
            terminal,
            "caller-profile-unchanged",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    };
    authenticated.caller =
        database.update_caller_profile(authenticated.caller.id, profile, &config.profile)?;
    info!(
        caller_id = authenticated.caller.id.get(),
        "caller profile updated"
    );
    write_key_line(
        terminal,
        "caller-profile-saved",
        &crate::LocalizationArgs::new(),
    )?;
    Ok(())
}

fn show_caller_directory(
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    authenticated: &mut AuthenticatedCaller,
    timezone: chrono_tz::Tz,
) -> Result<(), SessionError> {
    write_key_line(
        terminal,
        "caller-directory-listing-status",
        &crate::LocalizationArgs::new().with(
            "status",
            if authenticated.caller.public_directory_listed {
                crate::text("caller-directory-listed", &crate::LocalizationArgs::new())
            } else {
                crate::text("caller-directory-unlisted", &crate::LocalizationArgs::new())
            },
        ),
    )?;
    write_key(
        terminal,
        "caller-directory-listing-change-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    if read_yes_no_choice(terminal)? == Some(true) {
        write_key(
            terminal,
            "caller-directory-listing-enable-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        if let Some(listed) = read_yes_no_choice(terminal)? {
            match database.update_caller_publicity(
                PublicInformationActor::Caller(authenticated.caller.id),
                authenticated.caller.id,
                authenticated.caller.publicity_state_version,
                listed,
                unix_seconds()?,
            ) {
                Ok(publicity) => {
                    authenticated.caller.public_directory_listed = publicity.listed;
                    authenticated.caller.publicity_state_version = publicity.state_version;
                    write_key_line(
                        terminal,
                        "caller-directory-listing-saved",
                        &crate::LocalizationArgs::new(),
                    )?;
                }
                Err(DatabaseError::PublicInformation(
                    PublicInformationError::CallerPublicityConflict { .. },
                )) => {
                    authenticated.caller = database
                        .caller_by_id(authenticated.caller.id)?
                        .ok_or(DatabaseError::MissingCaller(authenticated.caller.id.get()))?;
                    write_key_line(
                        terminal,
                        "caller-public-information-conflict",
                        &crate::LocalizationArgs::new(),
                    )?;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    if !database.public_directory_policy()?.enabled {
        return write_key_line(
            terminal,
            "caller-directory-disabled",
            &crate::LocalizationArgs::new(),
        )
        .map_err(Into::into);
    }
    terminal.begin_output();
    write_key_line(
        terminal,
        "caller-directory-title",
        &crate::LocalizationArgs::new(),
    )?;
    let mut offset = 0;
    let mut displayed = 0_u64;
    loop {
        let candidates = database.public_caller_directory(offset, 11)?;
        for candidate in candidates.iter().take(10) {
            if let Some(current) = database.revalidate_public_caller(candidate.caller_id)? {
                write_public_caller(terminal, &current, timezone)?;
                displayed += 1;
            }
            if terminal.output_aborted() {
                return Ok(());
            }
        }
        if candidates.len() <= 10 {
            break;
        }
        write_key(
            terminal,
            "caller-directory-more-prompt",
            &crate::LocalizationArgs::new(),
        )?;
        if read_yes_no_choice(terminal)? != Some(true) {
            break;
        }
        offset += 10;
    }
    if displayed == 0 {
        write_key_line(
            terminal,
            "caller-directory-empty",
            &crate::LocalizationArgs::new(),
        )?;
    }
    Ok(())
}

fn locate_caller(
    terminal: &mut dyn Terminal,
    database: &RuntimeDatabase,
    timezone: chrono_tz::Tz,
) -> Result<(), SessionError> {
    write_key(
        terminal,
        "caller-locate-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(query) = read_utf8_input(terminal, crate::MAX_DIRECTORY_QUERY_BYTES)? else {
        return Ok(());
    };
    let matches = match database.locate_public_callers(&query) {
        Ok(matches) => matches,
        Err(DatabaseError::PublicInformation(PublicInformationError::InvalidQuery)) => {
            write_key_line(
                terminal,
                "caller-locate-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    for candidate in matches {
        let Some(current) = database.revalidate_public_caller(candidate.caller_id)? else {
            continue;
        };
        write_key(
            terminal,
            "caller-locate-confirm",
            &crate::LocalizationArgs::new().with("handle", current.handle.clone()),
        )?;
        if read_yes_no_choice(terminal)? == Some(true) {
            if let Some(current) = database.revalidate_public_caller(current.caller_id)? {
                write_public_caller(terminal, &current, timezone)?;
            }
            return Ok(());
        }
    }
    write_key_line(
        terminal,
        "caller-locate-none",
        &crate::LocalizationArgs::new(),
    )?;
    Ok(())
}

fn write_public_caller(
    terminal: &mut dyn Terminal,
    caller: &crate::PublicCallerSummary,
    timezone: chrono_tz::Tz,
) -> Result<(), SessionError> {
    write_key_line(
        terminal,
        "caller-directory-row",
        &crate::LocalizationArgs::new().with("handle", caller.handle.clone()),
    )?;
    if let Some(last_call) = caller.last_call_at {
        let date = chrono::DateTime::from_timestamp(last_call, 0)
            .map(|value| value.with_timezone(&timezone).date_naive().to_string())
            .unwrap_or_else(|| {
                crate::text(
                    "caller-public-information-unavailable",
                    &crate::LocalizationArgs::new(),
                )
            });
        write_key_line(
            terminal,
            "caller-directory-last-call",
            &crate::LocalizationArgs::new().with("date", date),
        )?;
    }
    if let Some(location) = &caller.city_region {
        write_key_line(
            terminal,
            "caller-directory-location",
            &crate::LocalizationArgs::new().with("location", location.clone()),
        )?;
    }
    Ok(())
}

fn show_other_bbs(
    terminal: &mut dyn Terminal,
    database: &RuntimeDatabase,
) -> Result<(), SessionError> {
    terminal.begin_output();
    write_key_line(
        terminal,
        "caller-other-bbs-title",
        &crate::LocalizationArgs::new(),
    )?;
    let entries = database.other_bbs_entries(false)?;
    if entries.is_empty() {
        write_key_line(
            terminal,
            "caller-other-bbs-empty",
            &crate::LocalizationArgs::new(),
        )?;
    }
    for entry in entries {
        write_key_line(
            terminal,
            "caller-other-bbs-row",
            &crate::LocalizationArgs::new()
                .with("name", entry.name)
                .with("speed", entry.speed)
                .with("dial", entry.dial_string),
        )?;
        if terminal.output_aborted() {
            break;
        }
    }
    Ok(())
}

fn add_other_bbs(
    terminal: &mut dyn Terminal,
    database: &mut RuntimeDatabase,
    authenticated: &AuthenticatedCaller,
) -> Result<(), SessionError> {
    if !database
        .public_directory_policy()?
        .caller_bbs_additions_enabled
    {
        return write_key_line(
            terminal,
            "caller-other-bbs-add-disabled",
            &crate::LocalizationArgs::new(),
        )
        .map_err(Into::into);
    }
    write_key(
        terminal,
        "caller-other-bbs-name-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(name) = read_utf8_input(terminal, crate::MAX_OTHER_BBS_NAME_BYTES)? else {
        return Ok(());
    };
    write_key(
        terminal,
        "caller-other-bbs-speed-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(speed) = read_utf8_input(terminal, crate::MAX_OTHER_BBS_SPEED_BYTES)? else {
        return Ok(());
    };
    write_key(
        terminal,
        "caller-other-bbs-dial-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(dial_string) = read_utf8_input(terminal, crate::MAX_OTHER_BBS_DIAL_BYTES)? else {
        return Ok(());
    };
    match database.add_other_bbs(
        PublicInformationActor::Caller(authenticated.caller.id),
        NewOtherBbsEntry {
            name,
            speed,
            dial_string,
        },
        unix_seconds()?,
    ) {
        Ok(_) => write_key_line(
            terminal,
            "caller-other-bbs-saved",
            &crate::LocalizationArgs::new(),
        )?,
        Err(DatabaseError::PublicInformation(
            PublicInformationError::InvalidText { .. } | PublicInformationError::DuplicateOtherBbs,
        )) => write_key_line(
            terminal,
            "caller-other-bbs-invalid",
            &crate::LocalizationArgs::new(),
        )?,
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn show_bulletins(
    terminal: &mut dyn Terminal,
    resources: &StockResources,
    context: &DisplayContext<'_>,
) -> Result<(), SessionError> {
    terminal.begin_output();
    write_key_line(
        terminal,
        "caller-bulletins-title",
        &crate::LocalizationArgs::new(),
    )?;
    if resources
        .display("BULLETIN")
        .is_some_and(|resource| resource.source == DisplaySource::BoardOverride)
    {
        render_named_display(terminal, resources, "BULLETIN", context)?;
    }
    let available = (1..=99)
        .filter(|number| {
            resources
                .display(&format!("BULLET{number}"))
                .is_some_and(|resource| resource.source == DisplaySource::BoardOverride)
        })
        .collect::<Vec<_>>();
    if available.is_empty() {
        write_key_line(
            terminal,
            "caller-bulletins-unavailable",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    }
    for number in &available {
        write_key_line(
            terminal,
            "caller-bulletin-catalog-row",
            &crate::LocalizationArgs::new().with("number", *number as u64),
        )?;
    }
    write_key(
        terminal,
        "caller-bulletin-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(input) = read_utf8_input(terminal, 2)? else {
        return Ok(());
    };
    let Ok(number) = input.parse::<u8>() else {
        write_key_line(
            terminal,
            "caller-bulletin-invalid",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    };
    if available.contains(&number) {
        render_named_display(terminal, resources, &format!("BULLET{number}"), context)?;
    } else {
        write_key_line(
            terminal,
            "caller-bulletin-unavailable",
            &crate::LocalizationArgs::new(),
        )?;
    }
    Ok(())
}

fn show_newsletter(
    terminal: &mut dyn Terminal,
    database: &RuntimeDatabase,
    resources: &StockResources,
    context: &DisplayContext<'_>,
    authenticated: &AuthenticatedCaller,
) -> Result<(), SessionError> {
    if resources
        .display("SFNWSLTR")
        .is_some_and(|resource| resource.source == DisplaySource::BoardOverride)
    {
        if database
            .public_resource_state("newsletter")?
            .is_some_and(|state| {
                authenticated
                    .previous_call_at
                    .is_some_and(|previous| state.published_at > previous)
            })
        {
            write_key_line(
                terminal,
                "caller-newsletter-updated",
                &crate::LocalizationArgs::new(),
            )?;
        }
        render_named_display(terminal, resources, "SFNWSLTR", context)
    } else {
        write_key_line(
            terminal,
            "caller-newsletter-unavailable",
            &crate::LocalizationArgs::new(),
        )
        .map_err(Into::into)
    }
}

fn show_system_information(
    terminal: &mut dyn Terminal,
    database: &RuntimeDatabase,
    board: &BoardIdentity,
    timezone: chrono_tz::Tz,
) -> Result<(), SessionError> {
    let (started, calls) = database.public_system_facts()?;
    let start_date = chrono::DateTime::from_timestamp(started, 0)
        .map(|value| value.with_timezone(&timezone).date_naive().to_string())
        .unwrap_or_else(|| {
            crate::text(
                "caller-public-information-unavailable",
                &crate::LocalizationArgs::new(),
            )
        });
    terminal.begin_output();
    write_key_line(
        terminal,
        "caller-system-information-title",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "caller-system-information-board",
        &crate::LocalizationArgs::new().with("board", board.name()),
    )?;
    write_key_line(
        terminal,
        "caller-system-information-sysop",
        &crate::LocalizationArgs::new().with("sysop", board.sysop_name()),
    )?;
    write_key_line(
        terminal,
        "caller-system-information-started",
        &crate::LocalizationArgs::new().with("date", start_date),
    )?;
    write_key_line(
        terminal,
        "caller-system-information-calls",
        &crate::LocalizationArgs::new().with("calls", calls),
    )?;
    Ok(())
}

fn read_yes_no_choice(terminal: &mut dyn Terminal) -> Result<Option<bool>, TerminalError> {
    match terminal.read_line(8)? {
        Some(value) => Ok(value
            .iter()
            .find(|byte| !byte.is_ascii_whitespace())
            .and_then(|byte| match byte.to_ascii_uppercase() {
                b'Y' => Some(true),
                b'N' => Some(false),
                _ => None,
            })),
        None => Ok(None),
    }
}

fn read_utf8_input(
    terminal: &mut dyn Terminal,
    maximum: usize,
) -> Result<Option<String>, SessionError> {
    let bytes = match terminal.read_line(maximum) {
        Ok(value) => value,
        Err(TerminalError::InputTooLong { .. }) => {
            write_key_line(
                terminal,
                "caller-public-information-input-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    match String::from_utf8(bytes) {
        Ok(value) => Ok(Some(value.trim().to_owned())),
        Err(_) => {
            write_key_line(
                terminal,
                "caller-public-information-input-invalid",
                &crate::LocalizationArgs::new(),
            )?;
            Ok(None)
        }
    }
}

fn show_about(
    terminal: &mut dyn Terminal,
    resources: &StockResources,
    context: &DisplayContext<'_>,
) -> Result<(), SessionError> {
    if resources.display("ABOUT").is_some() {
        render_named_display(terminal, resources, "ABOUT", context)?;
    } else {
        terminal.begin_output();
        write_key_line(
            terminal,
            "caller-about-product",
            &crate::LocalizationArgs::new(),
        )?;
        write_key_line(
            terminal,
            "caller-about-copyright",
            &crate::LocalizationArgs::new(),
        )?;
        write_key_line(
            terminal,
            "caller-about-project",
            &crate::LocalizationArgs::new(),
        )?;
        write_line(
            terminal,
            "An independent preservation-driven reimplementation of the original SPITFIRE BBS.",
        )?;
        write_line(
            terminal,
            "Original SPITFIRE BBS Copyright (C) 1987-2010 by Mike Woltz, Buffalo Creek Software.",
        )?;
    }

    if terminal.output_aborted() {
        return Ok(());
    }
    terminal.write_all(b"\r\n")?;
    write_key(
        terminal,
        "caller-about-return-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    match terminal.read_line(8) {
        Ok(_) | Err(TerminalError::InputTooLong { .. }) => {
            terminal.write_all(b"\r\n")?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn write_authenticated_greeting(
    terminal: &mut dyn Terminal,
    database: &RuntimeDatabase,
    authenticated: &AuthenticatedCaller,
    config: &CallerConfig,
    timezone: chrono_tz::Tz,
    now: i64,
) -> Result<(), SessionError> {
    write_key_line(
        terminal,
        "caller-welcome",
        &crate::LocalizationArgs::new().with("caller", authenticated.caller.display_name.clone()),
    )?;
    let context = CallerSessionContext::from_authenticated(
        authenticated,
        config,
        timezone,
        now,
        Duration::ZERO,
    )?;
    write_key_line(
        terminal,
        "caller-context-board-time",
        &crate::LocalizationArgs::new().timestamp(
            "timestamp",
            context.board_now_timestamp(),
            context.board_timezone(),
        ),
    )?;
    write_key_line(
        terminal,
        "caller-context-call-count",
        &crate::LocalizationArgs::new()
            .with("caller_number", context.caller_id().get() as u64)
            .with("calls_today", context.calls_today() as u64)
            .with("total_calls", context.total_calls()),
    )?;
    match context.previous_call_timestamp() {
        Some(previous) => write_key_line(
            terminal,
            "caller-context-last-call",
            &crate::LocalizationArgs::new().timestamp(
                "timestamp",
                previous,
                context.board_timezone(),
            ),
        )?,
        None => write_key_line(
            terminal,
            "caller-context-first-call",
            &crate::LocalizationArgs::new(),
        )?,
    }
    write_key_line(
        terminal,
        "caller-context-security-time",
        &crate::LocalizationArgs::new()
            .with("security", context.security_level().get())
            .with(
                "call_minutes",
                context.call_remaining_seconds().div_ceil(60),
            )
            .with(
                "daily_minutes",
                context.daily_remaining_seconds().div_ceil(60),
            ),
    )?;
    if let Some(denial) = authenticated.pending_access_denial.as_ref() {
        let occurred = format_board_local_timestamp(denial.occurred_at(), timezone)?;
        let reason_key = match denial.reason() {
            AccessDenialReason::InvalidCredentials => "caller-denial-invalid-credentials",
            AccessDenialReason::AccountUnavailable => "caller-denial-account-unavailable",
            AccessDenialReason::PrivateBoardPolicy => "caller-denial-private-policy",
            AccessDenialReason::DailyCallLimit => "caller-denial-daily-call-limit",
            AccessDenialReason::DailyTimeLimit => "caller-denial-daily-time-limit",
        };
        let reason = crate::text(reason_key, &crate::LocalizationArgs::new());
        write_key_line(
            terminal,
            "caller-context-denial",
            &crate::LocalizationArgs::new()
                .with("timestamp", occurred)
                .with("reason", reason),
        )?;
        database.acknowledge_caller_access_denial(authenticated.caller.id, denial.generation())?;
    }
    Ok(())
}

fn caller_status_lines(context: &CallerSessionContext) -> Result<Vec<String>, CallerError> {
    Ok(vec![
        crate::text(
            "caller-context-board-time",
            &crate::LocalizationArgs::new().timestamp(
                "timestamp",
                context.board_now_timestamp(),
                context.board_timezone(),
            ),
        ),
        crate::text(
            "caller-context-status-identity",
            &crate::LocalizationArgs::new()
                .with("caller_number", context.caller_id().get() as u64)
                .with("security", context.security_level().get())
                .with("calls_today", context.calls_today() as u64)
                .with("calls_remaining", context.calls_remaining_today() as u64)
                .with("total_calls", context.total_calls()),
        ),
        crate::text(
            "caller-context-status-time",
            &crate::LocalizationArgs::new()
                .with("elapsed_minutes", context.elapsed_seconds() / 60)
                .with(
                    "call_minutes",
                    context.call_remaining_seconds().div_ceil(60),
                )
                .with(
                    "daily_minutes",
                    context.daily_remaining_seconds().div_ceil(60),
                ),
        ),
    ])
}

fn render_caller_status(
    terminal: &mut dyn Terminal,
    status_lines: &[String],
) -> Result<(), TerminalError> {
    let width = usize::from(
        terminal
            .info()
            .capabilities
            .size
            .map_or(80, |size| size.width),
    )
    .max(8);
    terminal.begin_output();
    terminal.write_all(b"\r\n")?;
    for line in status_lines {
        for chunk in line.as_bytes().chunks(width) {
            terminal.write_all(chunk)?;
            terminal.write_all(b"\r\n")?;
            if terminal.output_aborted() {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn active_resources<'a>(
    terminal: &dyn Terminal,
    capable: &'a StockResources,
    text: &'a StockResources,
) -> &'a StockResources {
    if terminal.info().capabilities.ansi {
        capable
    } else {
        text
    }
}

pub(crate) fn render_named_display(
    terminal: &mut dyn Terminal,
    resources: &StockResources,
    stem: &str,
    context: &DisplayContext<'_>,
) -> Result<(), SessionError> {
    let Some(display) = resources.display(stem) else {
        return Ok(());
    };
    render_display(terminal, display, context)?;
    ensure_line_ending(terminal, &display.bytes)?;
    Ok(())
}

fn render_post_login_resources(
    terminal: &mut dyn Terminal,
    resources: &StockResources,
    context: &DisplayContext<'_>,
    authenticated: &AuthenticatedCaller,
) -> Result<(), SessionError> {
    if authenticated.first_session {
        render_named_display(terminal, resources, "NEWUSER", context)?;
    }
    for number in 2..=9 {
        render_named_display(terminal, resources, &format!("WELCOME{number}"), context)?;
    }
    render_named_display(terminal, resources, "ALL", context)?;
    render_named_display(
        terminal,
        resources,
        &format!("SFNOD{}", context.node.get()),
        context,
    )?;
    render_named_display(
        terminal,
        resources,
        &format!("{}SEC", authenticated.caller.security_level.get()),
        context,
    )?;
    render_named_display(
        terminal,
        resources,
        &authenticated.caller.id.get().to_string(),
        context,
    )?;
    if let Some(thought) = resources.thoughts.as_ref().and_then(|catalog| {
        catalog.select(authenticated.caller.id.get() as u64 ^ authenticated.caller.call_count)
    }) {
        write_key_line(
            terminal,
            "caller-thought",
            &crate::LocalizationArgs::new().with("thought", thought),
        )?;
    }
    Ok(())
}

pub(crate) fn read_menu_command(
    terminal: &mut dyn Terminal,
    hot_keys: bool,
) -> Result<Option<u8>, TerminalError> {
    if hot_keys {
        return terminal.read_key().map(|value| {
            value.map(|byte| {
                if byte.is_ascii_graphic() {
                    byte.to_ascii_uppercase()
                } else {
                    INVALID_MENU_COMMAND
                }
            })
        });
    }
    let input = match terminal.read_line(MAX_MENU_COMMAND_BYTES) {
        Ok(Some(input)) => input,
        Ok(None) => return Ok(None),
        Err(TerminalError::InputTooLong { .. }) => return Ok(Some(INVALID_MENU_COMMAND)),
        Err(error) => return Err(error),
    };
    let command = input
        .into_iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .filter(u8::is_ascii_graphic)
        .map(|byte| byte.to_ascii_uppercase())
        .unwrap_or(INVALID_MENU_COMMAND);
    Ok(Some(command))
}

fn run_sysop_page(
    session: &Session,
    terminal: &mut dyn Terminal,
    authenticated: &AuthenticatedCaller,
    stock: &StockSessionContext<'_>,
    context: &DisplayContext<'_>,
) -> Result<(), SessionError> {
    let resources = active_resources(terminal, stock.resources, stock.text_resources);
    let ticket = match stock.interaction.request_page(
        session.id(),
        session.node_id(),
        authenticated.caller.id,
        &authenticated.caller.display_name,
        unix_seconds()?,
    ) {
        Ok(ticket) => ticket,
        Err(InteractionError::SysopUnavailable) => {
            render_display(terminal, &resources.page_off, context)?;
            ensure_line_ending(terminal, &resources.page_off.bytes)?;
            info!(
                caller_id = authenticated.caller.id.get(),
                "caller attempted to page while the Sysop was unavailable"
            );
            return Ok(());
        }
        Err(InteractionError::AlreadyPaged(_)) => {
            render_display(terminal, &resources.page_already_requested, context)?;
            ensure_line_ending(terminal, &resources.page_already_requested.bytes)?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    stock.status.page_pending()?;
    write_key_line(
        terminal,
        "caller-page-wait",
        &crate::LocalizationArgs::new(),
    )?;
    info!(
        caller_id = authenticated.caller.id.get(),
        node = session.node_id().get(),
        session = session.id().get(),
        "caller paged the Sysop"
    );
    match ticket.wait(stock.page_timeout)? {
        PageAnswer::Declined | PageAnswer::TimedOut => {
            stock.status.interaction_finished()?;
            render_display(terminal, &resources.page_unanswered, context)?;
            ensure_line_ending(terminal, &resources.page_unanswered.bytes)?;
        }
        PageAnswer::Accepted(chat) => {
            stock.status.chat_started()?;
            render_display(terminal, &resources.chat_caller_initiated, context)?;
            ensure_line_ending(terminal, &resources.chat_caller_initiated.bytes)?;
            loop {
                write_key(
                    terminal,
                    "caller-chat-caller-prompt",
                    &crate::LocalizationArgs::new(),
                )?;
                let Some(line) = terminal.read_line(512)? else {
                    chat.end();
                    break;
                };
                let text = String::from_utf8_lossy(&line).trim().to_owned();
                if text.eq_ignore_ascii_case("/Q") || line.first() == Some(&0x1b) {
                    chat.end();
                    break;
                }
                chat.send_line(&text)?;
                match chat.receive_line(stock.chat_timeout) {
                    Ok(Some(reply)) => write_key_line(
                        terminal,
                        "caller-chat-sysop-line",
                        &crate::LocalizationArgs::new().with("reply", reply),
                    )?,
                    Ok(None) | Err(InteractionError::TimedOut) => break,
                    Err(error) => return Err(error.into()),
                }
            }
            stock.status.interaction_finished()?;
            render_display(terminal, &resources.chat_done, context)?;
            ensure_line_ending(terminal, &resources.chat_done.bytes)?;
            info!(
                caller_id = authenticated.caller.id.get(),
                session = session.id().get(),
                "Sysop chat ended"
            );
        }
    }
    Ok(())
}

fn edit_terminal_preferences(
    terminal: &mut PagingTerminal<'_>,
    database: &RuntimeDatabase,
    authenticated: &mut AuthenticatedCaller,
) -> Result<(), SessionError> {
    let mut preferences = authenticated.caller.preferences;
    terminal.begin_output();
    write_key_line(
        terminal,
        "caller-terminal-title",
        &crate::LocalizationArgs::new(),
    )?;
    write_line(
        terminal,
        &format!(
            "Graphics: {:?}  Width: {}  Page Length: {}",
            preferences.graphics,
            preferences
                .screen_width
                .map_or("AUTO".to_owned(), |v| v.to_string()),
            preferences
                .page_length
                .map_or("AUTO".to_owned(), |v| v.to_string())
        ),
    )?;
    write_line(
        terminal,
        &format!(
            "More Prompt: {}  Scroll Prompt: {}  Hot Keys: {}\r\nDefault Transfer: {}",
            on_off(preferences.more_prompt),
            on_off(preferences.scroll_prompt),
            on_off(preferences.hot_keys),
            preferences.transfer_protocol.stock_name()
        ),
    )?;
    terminal.write_all(
        b"Change [G]raphics [W]idth [L]ength [M]ore [S]croll [H]ot Keys [F]ile Protocol, or [Q]: ",
    )?;
    let Some(choice) = terminal.read_line(8)? else {
        return Ok(());
    };
    match first_command(&choice) {
        Some(b'G') => {
            write_key(
                terminal,
                "caller-terminal-graphics-prompt",
                &crate::LocalizationArgs::new(),
            )?;
            let Some(value) = terminal.read_line(8)? else {
                return Ok(());
            };
            preferences.graphics = match first_command(&value) {
                Some(b'A') => GraphicsPreference::Auto,
                Some(b'N') => GraphicsPreference::Ansi,
                Some(b'T') => GraphicsPreference::Text,
                _ => {
                    write_key_line(
                        terminal,
                        "caller-terminal-graphics-invalid",
                        &crate::LocalizationArgs::new(),
                    )?;
                    return Ok(());
                }
            };
        }
        Some(b'W') => {
            preferences.screen_width = prompt_dimension(
                terminal,
                "Screen width (40-144, 0=AUTO): ",
                40,
                144,
                preferences.screen_width,
            )?
        }
        Some(b'L') => {
            preferences.page_length = prompt_dimension(
                terminal,
                "Page length (10-24, 0=AUTO): ",
                10,
                24,
                preferences.page_length,
            )?
        }
        Some(b'M') => preferences.more_prompt = !preferences.more_prompt,
        Some(b'S') => preferences.scroll_prompt = !preferences.scroll_prompt,
        Some(b'H') => preferences.hot_keys = !preferences.hot_keys,
        Some(b'F') => {
            terminal.write_all(
                b"<1> Ascii  <2> Xmodem Checksum  <3> Xmodem CRC  <4> 1K-Xmodem\r\n<5> Ymodem (Batch)  <6> Zmodem (Batch)  <T> Telink  <S> Select at transfer\r\nEnter the protocol of your choice [1 2 3 4 5 6 T S]: ",
            )?;
            let Some(value) = terminal.read_line(8)? else {
                return Ok(());
            };
            preferences.transfer_protocol = match first_command(&value) {
                Some(b'1') => crate::TransferPreference::Ascii,
                Some(b'2') => crate::TransferPreference::XmodemChecksum,
                Some(b'3') => crate::TransferPreference::XmodemCrc,
                Some(b'4') => crate::TransferPreference::Xmodem1k,
                Some(b'5') => crate::TransferPreference::Ymodem,
                Some(b'6') => crate::TransferPreference::Zmodem,
                Some(b'T') => crate::TransferPreference::Telink,
                Some(b'S') => crate::TransferPreference::Select,
                _ => {
                    write_key_line(
                        terminal,
                        "caller-terminal-transfer-invalid",
                        &crate::LocalizationArgs::new(),
                    )?;
                    return Ok(());
                }
            };
        }
        _ => return Ok(()),
    }
    let caller = database.update_caller_preferences(authenticated.caller.id, preferences)?;
    authenticated.caller = caller;
    terminal.set_preferences(preferences);
    write_key_line(
        terminal,
        "caller-terminal-saved",
        &crate::LocalizationArgs::new(),
    )?;
    Ok(())
}

fn prompt_dimension(
    terminal: &mut dyn Terminal,
    prompt: &str,
    minimum: u16,
    maximum: u16,
    current: Option<u16>,
) -> Result<Option<u16>, SessionError> {
    terminal.write_all(prompt.as_bytes())?;
    let Some(input) = terminal.read_line(8)? else {
        return Ok(None);
    };
    let value = String::from_utf8_lossy(&input).trim().parse::<u16>().ok();
    match value {
        Some(0) => Ok(None),
        Some(value) if (minimum..=maximum).contains(&value) => Ok(Some(value)),
        _ => {
            write_line(
                terminal,
                "That value is outside the supported SPITFIRE range.",
            )?;
            Ok(current)
        }
    }
}

fn first_command(input: &[u8]) -> Option<u8> {
    input
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .map(|byte| byte.to_ascii_uppercase())
}

fn on_off(value: bool) -> &'static str {
    if value {
        "ON"
    } else {
        "OFF"
    }
}

fn show_caller_statistics(
    terminal: &mut dyn Terminal,
    backend: &RuntimeDatabase,
    authenticated: &AuthenticatedCaller,
    caller_config: &CallerConfig,
) -> Result<(), SessionError> {
    terminal.begin_output();
    write_key_line(
        terminal,
        "caller-statistics-title",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "caller-statistics-name",
        &crate::LocalizationArgs::new().with("name", authenticated.caller.display_name.clone()),
    )?;
    write_key_line(
        terminal,
        "caller-statistics-security",
        &crate::LocalizationArgs::new().with("security", authenticated.caller.security_level.get()),
    )?;
    write_key_line(
        terminal,
        "caller-statistics-times-on",
        &crate::LocalizationArgs::new().with("count", authenticated.caller.call_count),
    )?;
    write_key_line(
        terminal,
        "caller-statistics-accumulated",
        &crate::LocalizationArgs::new()
            .with("minutes", authenticated.caller.total_time_seconds / 60),
    )?;
    let stats = backend.stats(message_actor(authenticated, caller_config)?)?;
    write_key_line(
        terminal,
        "caller-statistics-messages-sent",
        &crate::LocalizationArgs::new().with("count", stats.sent),
    )?;
    write_key_line(
        terminal,
        "caller-statistics-waiting",
        &crate::LocalizationArgs::new().with("count", stats.new_waiting),
    )?;
    let caller = backend
        .caller_by_id(authenticated.caller.id)?
        .ok_or(DatabaseError::MissingCaller(authenticated.caller.id.get()))?;
    write_key_line(
        terminal,
        "caller-statistics-uploads",
        &crate::LocalizationArgs::new()
            .with("count", caller.files_uploaded)
            .with("bytes", caller.upload_bytes),
    )?;
    write_key_line(
        terminal,
        "caller-statistics-downloads",
        &crate::LocalizationArgs::new()
            .with("count", caller.files_downloaded)
            .with("bytes", caller.download_bytes),
    )?;
    write_line(
        terminal,
        &format!(
            "Terminal: {:?}, width {}, page {}, More {}",
            caller.preferences.graphics,
            caller
                .preferences
                .screen_width
                .map_or("AUTO".to_owned(), |value| value.to_string()),
            caller
                .preferences
                .page_length
                .map_or("AUTO".to_owned(), |value| value.to_string()),
            on_off(caller.preferences.more_prompt)
        ),
    )?;
    Ok(())
}

fn run_stock_post_login_journey(
    terminal: &mut dyn Terminal,
    backend: &mut RuntimeDatabase,
    authenticated: &AuthenticatedCaller,
    caller_config: &CallerConfig,
    timezone: chrono_tz::Tz,
) -> Result<(), SessionError> {
    let message_stats = backend.stats(message_actor(authenticated, caller_config)?)?;
    terminal.begin_output();
    write_key_line(
        terminal,
        "caller-message-summary-title",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "caller-message-summary-waiting",
        &crate::LocalizationArgs::new().with("count", message_stats.new_waiting),
    )?;
    write_key_line(
        terminal,
        "caller-message-summary-received",
        &crate::LocalizationArgs::new().with("count", message_stats.already_received),
    )?;
    write_key_line(
        terminal,
        "caller-message-summary-sent",
        &crate::LocalizationArgs::new().with("count", message_stats.sent),
    )?;
    if terminal.output_aborted() {
        return Ok(());
    }
    show_caller_statistics(terminal, backend, authenticated, caller_config)?;
    if terminal.output_aborted() {
        return Ok(());
    }

    let actor = FileActor::new(
        authenticated.caller.id,
        SecurityLevel::new(caller_config.sysop_security)?,
    );
    let checkpoint = backend.new_file_checkpoint(actor)?;
    let file_stats = backend.file_statistics(actor, checkpoint)?;
    terminal.begin_output();
    write_key_line(
        terminal,
        "caller-new-files-title",
        &crate::LocalizationArgs::new(),
    )?;
    write_key_line(
        terminal,
        "caller-new-files-count",
        &crate::LocalizationArgs::new().with("count", file_stats.new_since_checkpoint),
    )?;
    write_key(
        terminal,
        "caller-new-files-question",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(answer) = terminal.read_line(8)? else {
        return Ok(());
    };
    terminal.write_all(b"\r\n")?;
    if first_command(&answer) == Some(b'Y') {
        run_post_login_new_files(terminal, backend, authenticated, caller_config, timezone)?;
    }
    Ok(())
}

pub(crate) fn show_help(
    section: MenuSection,
    menu: &crate::MenuDefinition,
    resources: &StockResources,
    terminal: &mut dyn Terminal,
    security: SecurityLevel,
    hot_keys: bool,
) -> Result<(), SessionError> {
    write_key(
        terminal,
        "session-command-help-prompt",
        &crate::LocalizationArgs::new(),
    )?;
    let Some(command) = read_menu_command(terminal, hot_keys)? else {
        return Ok(());
    };
    let Some(item) = menu.find(command, security.get()) else {
        write_key_line(
            terminal,
            "session-help-unavailable",
            &crate::LocalizationArgs::new(),
        )?;
        return Ok(());
    };
    let Some(record_number) = help_record_number(section, item.identifier) else {
        write_line(
            terminal,
            "No documented help record maps to that command yet.",
        )?;
        return Ok(());
    };
    let Some(record) = resources.help_record(record_number) else {
        write_line(
            terminal,
            "The configured SPITFIRE.HLP record is unavailable.",
        )?;
        return Ok(());
    };
    terminal.begin_output();
    terminal.write_all(b"\r\n")?;
    for line in &record.lines {
        match line.as_slice() {
            b"\\" => {}
            b";" => terminal.write_all(b"\r\n")?,
            _ => {
                terminal.write_all(line)?;
                terminal.write_all(b"\r\n")?;
            }
        }
        if terminal.output_aborted() {
            break;
        }
    }
    Ok(())
}

/// Numeric assignments are documented by Buffalo Creek's SFHELP 3.51 manual.
fn help_record_number(section: MenuSection, identifier: u8) -> Option<usize> {
    match (section, identifier) {
        (_, b'H') => Some(1),
        (_, b'A') => Some(2),
        (_, b'B') => Some(3),
        (_, b'?') => Some(18),
        (MenuSection::Main, b'F') => Some(4),
        (MenuSection::Main, b'@') => Some(6),
        (MenuSection::Main, b'E') => Some(21),
        (MenuSection::Main, b'Q') => Some(22),
        (MenuSection::Main, b'Y') => Some(23),
        (MenuSection::Main, b'J') => Some(24),
        (MenuSection::Main, b'L') => Some(25),
        (MenuSection::Main, b'I') => Some(26),
        (MenuSection::Main, b'K') => Some(28),
        (MenuSection::Main, b'X') => Some(29),
        (MenuSection::Main, b'G') => Some(30),
        (MenuSection::Main, b'P') => Some(31),
        (MenuSection::Main, b'C') => Some(32),
        (MenuSection::Main, b'O') => Some(33),
        (MenuSection::Main, b'D') => Some(50),
        (MenuSection::Main, b'V') => Some(51),
        (MenuSection::Message, b'J') => Some(34),
        (MenuSection::Message, b'Z') => Some(35),
        (MenuSection::Message, b'I') => Some(36),
        (MenuSection::Message, b'S') => Some(37),
        (MenuSection::Message, b'L') => Some(38),
        (MenuSection::Message, b'G') => Some(39),
        (MenuSection::Message, b'X') => Some(40),
        (MenuSection::Message, b'D') => Some(41),
        (MenuSection::Message, b'C') => Some(42),
        (MenuSection::Message, b'@') => Some(52),
        (MenuSection::Message, b'K') => Some(53),
        (MenuSection::Message, b'E') => Some(55),
        (MenuSection::Message, b'R') => Some(4),
        (MenuSection::File, b'Z') => Some(7),
        (MenuSection::File, b'X') => Some(8),
        (MenuSection::File, b'K') => Some(9),
        (MenuSection::File, b'L') => Some(10),
        (MenuSection::File, b'J') => Some(11),
        (MenuSection::File, b'E') => Some(12),
        (MenuSection::File, b'C') => Some(13),
        (MenuSection::File, b'G') => Some(14),
        (MenuSection::File, b'N') => Some(15),
        (MenuSection::File, b'S') => Some(16),
        (MenuSection::File, b'P') => Some(17),
        (MenuSection::File, b'D') => Some(19),
        (MenuSection::File, b'I') => Some(20),
        (MenuSection::File, b'@') => Some(5),
        (MenuSection::File, b'Q') => Some(54),
        _ => None,
    }
}

fn session_outcome(
    session: &Session,
    commands_processed: usize,
    caller: Option<(CallerId, String)>,
) -> Result<SessionOutcome, SessionError> {
    let close_reason = session
        .close_reason()
        .ok_or(SessionError::MissingCloseReason)?;
    Ok(SessionOutcome {
        session_id: session.id(),
        node_id: session.node_id(),
        close_reason,
        commands_processed,
        caller_id: caller.as_ref().map(|(id, _)| *id),
        caller_name: caller.map(|(_, name)| name),
    })
}

pub(crate) fn unix_seconds() -> Result<i64, SessionError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SessionError::ClockBeforeEpoch)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| SessionError::ClockOverflow)
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

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session identifier must be nonzero, got {0}")]
    InvalidSessionId(u64),
    #[error("cannot {operation} a session in state {from:?}")]
    InvalidTransition {
        from: SessionState,
        operation: &'static str,
    },
    #[error("closed session has no close reason")]
    MissingCloseReason,
    #[error("system clock is before the Unix epoch")]
    ClockBeforeEpoch,
    #[error("system clock value is too large")]
    ClockOverflow,
    #[error("authenticated session is missing its accounting clock")]
    MissingAuthenticationClock,
    #[error(transparent)]
    Terminal(#[from] TerminalError),
    #[error(transparent)]
    Resource(#[from] ResourceError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Message(#[from] MessageError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    TransferProtocol(#[from] crate::TransferProtocolError),
    #[error(transparent)]
    Node(#[from] NodeError),
    #[error(transparent)]
    Caller(#[from] CallerError),
    #[error(transparent)]
    Credential(#[from] CredentialError),
    #[error(transparent)]
    Interaction(#[from] InteractionError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InMemoryTerminal, NodeDefinition, NodeManager, TransportKind};

    #[test]
    fn session_lifecycle_and_authentication_are_explicit() {
        let manager = NodeManager::new(vec![NodeDefinition {
            id: NodeId::new(1).unwrap(),
            enabled: true,
            description: None,
        }])
        .unwrap();
        let lease = manager
            .acquire(SessionId::new(1).unwrap(), TransportKind::InMemory, 1)
            .unwrap();
        let mut session = lease.start_session();
        assert_eq!(session.state(), SessionState::Created);
        assert_eq!(
            session.authentication_state(),
            AuthenticationState::NotStarted
        );
        assert!(session.close(SessionCloseReason::Goodbye).is_err());
        session.activate().unwrap();
        assert_eq!(
            session.authentication_state(),
            AuthenticationState::Unauthenticated
        );
        session.close(SessionCloseReason::Goodbye).unwrap();
        assert_eq!(session.state(), SessionState::Closed);
        assert_eq!(session.close_reason(), Some(SessionCloseReason::Goodbye));
        assert!(session.activate().is_err());
        lease.release(&session).unwrap();
    }

    #[test]
    fn terminal_rejects_oversized_input_without_panicking() {
        let mut terminal = InMemoryTerminal::with_lines([vec![b'A'; 65]]);
        assert!(matches!(
            terminal.read_line(64),
            Err(TerminalError::InputTooLong {
                actual: 65,
                maximum: 64
            })
        ));
    }

    #[test]
    fn sfhelp_record_map_covers_every_current_stock_menu_action() {
        for (section, identifier, expected) in [
            (MenuSection::Main, b'H', 1),
            (MenuSection::Main, b'J', 24),
            (MenuSection::Main, b'G', 30),
            (MenuSection::Message, b'J', 34),
            (MenuSection::Message, b'Z', 35),
            (MenuSection::Message, b'I', 36),
            (MenuSection::Message, b'S', 37),
            (MenuSection::Message, b'L', 38),
            (MenuSection::Message, b'X', 40),
            (MenuSection::Message, b'K', 53),
            (MenuSection::File, b'Z', 7),
            (MenuSection::File, b'X', 8),
            (MenuSection::File, b'L', 10),
            (MenuSection::File, b'I', 20),
            (MenuSection::Message, b'R', 4),
        ] {
            assert_eq!(help_record_number(section, identifier), Some(expected));
        }
        assert_eq!(help_record_number(MenuSection::Main, b'R'), None);
    }

    #[test]
    fn menu_input_honors_hot_key_and_line_modes() {
        let mut hot = InMemoryTerminal::with_lines([b"m".to_vec()]);
        assert_eq!(read_menu_command(&mut hot, true).unwrap(), Some(b'M'));
        let mut line = InMemoryTerminal::with_lines([b"  f  ".to_vec()]);
        assert_eq!(read_menu_command(&mut line, false).unwrap(), Some(b'F'));
    }

    #[test]
    fn menu_input_treats_empty_unsupported_and_oversized_lines_as_invalid() {
        for input in [
            Vec::new(),
            b"   ".to_vec(),
            b"\x1bOM".to_vec(),
            b"\x1b[13;2uXY".to_vec(),
        ] {
            let mut terminal = InMemoryTerminal::with_lines([input]);
            assert_eq!(
                read_menu_command(&mut terminal, false).unwrap(),
                Some(INVALID_MENU_COMMAND)
            );
        }

        let mut terminal = InMemoryTerminal::default();
        assert_eq!(read_menu_command(&mut terminal, false).unwrap(), None);
    }
}
