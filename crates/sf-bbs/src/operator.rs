use std::io::{BufRead, Write};
use std::sync::Arc;
use std::time::Duration;

use sf_core::{
    Caller, CallerState, OperatorChat, PageRequest, SecurityLevel, SessionId, SysopAvailability,
};
use tracing::info;

use crate::{ApplicationError, BoardRuntime};

/// Presentation-independent operator operations for the running board. The
/// terminal console is one client; a future web console can use this same API.
#[derive(Clone)]
pub struct OperatorService {
    runtime: Arc<BoardRuntime>,
}

impl OperatorService {
    pub fn new(runtime: Arc<BoardRuntime>) -> Self {
        Self { runtime }
    }

    pub fn board_name(&self) -> &str {
        self.runtime.board_name()
    }

    pub fn nodes(&self) -> Result<Vec<sf_core::NodeSnapshot>, ApplicationError> {
        self.runtime.node_snapshots()
    }

    pub fn pages(&self) -> Result<Vec<PageRequest>, ApplicationError> {
        self.runtime.interaction().pages().map_err(Into::into)
    }

    pub fn set_availability(
        &self,
        availability: SysopAvailability,
    ) -> Result<(), ApplicationError> {
        self.runtime.interaction().set_availability(availability)?;
        info!(?availability, "Sysop page availability changed");
        Ok(())
    }

    pub fn answer(&self, session: SessionId) -> Result<OperatorChat, ApplicationError> {
        self.runtime
            .interaction()
            .answer(session)
            .map_err(Into::into)
    }

    pub fn decline(&self, session: SessionId) -> Result<(), ApplicationError> {
        self.runtime
            .interaction()
            .decline(session)
            .map_err(Into::into)
    }

    pub fn disconnect(&self, session: SessionId) -> Result<(), ApplicationError> {
        self.runtime
            .interaction()
            .request_disconnect(session)
            .map_err(Into::into)
    }

    pub fn callers(&self) -> Result<Vec<Caller>, ApplicationError> {
        self.runtime.callers()
    }

    pub fn set_caller_state(
        &self,
        name: &str,
        state: CallerState,
    ) -> Result<Caller, ApplicationError> {
        let caller = self.runtime.set_caller_state(name.as_bytes(), state)?;
        info!(
            caller_id = caller.id.get(),
            ?state,
            "operator changed caller state"
        );
        Ok(caller)
    }

    pub fn set_caller_security(&self, name: &str, level: u16) -> Result<Caller, ApplicationError> {
        let caller = self.runtime.set_caller_security(
            name.as_bytes(),
            SecurityLevel::new(level).map_err(sf_core::DatabaseError::from)?,
        )?;
        info!(
            caller_id = caller.id.get(),
            level, "operator changed caller security"
        );
        Ok(caller)
    }

    pub fn set_caller_purge_protection(
        &self,
        name: &str,
        protected: bool,
    ) -> Result<Caller, ApplicationError> {
        self.runtime
            .set_caller_purge_protection(name.as_bytes(), protected)
    }

    pub fn update_caller_subscription(
        &self,
        name: &str,
        expires_on: Option<chrono::NaiveDate>,
    ) -> Result<Caller, ApplicationError> {
        self.runtime
            .update_caller_subscription(name.as_bytes(), expires_on)
    }

    pub fn caller(&self, name: &str) -> Result<Caller, ApplicationError> {
        self.runtime.caller(name.as_bytes())
    }

    pub fn set_caller_profile(
        &self,
        name: &str,
        profile: sf_core::CallerProfile,
    ) -> Result<Caller, ApplicationError> {
        let caller = self.runtime.set_caller_profile(name.as_bytes(), profile)?;
        info!(
            caller_id = caller.id.get(),
            "operator changed caller profile"
        );
        Ok(caller)
    }

    pub fn set_caller_identity(
        &self,
        name: &str,
        login_identifier: &str,
        display_handle: &str,
        real_name: Option<String>,
    ) -> Result<Caller, ApplicationError> {
        let caller = self.runtime.set_caller_identity(
            name.as_bytes(),
            login_identifier.as_bytes(),
            display_handle.as_bytes(),
            real_name,
        )?;
        info!(
            caller_id = caller.id.get(),
            "operator changed caller identity"
        );
        Ok(caller)
    }
}

pub fn run_operator_console(
    service: &OperatorService,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<(), ApplicationError> {
    writeln!(
        output,
        "{}",
        op_args(
            "operator-console-title",
            sf_core::LocalizationArgs::new().with("board", service.board_name())
        )
    )
    .map_err(ApplicationError::SetupIo)?;
    writeln!(output, "{}", op("operator-console-commands")).map_err(ApplicationError::SetupIo)?;
    loop {
        write!(output, "{}", op("operator-console-prompt")).map_err(ApplicationError::SetupIo)?;
        output.flush().map_err(ApplicationError::SetupIo)?;
        let Some(line) = read_console_line(input)? else {
            return Ok(());
        };
        let (command, rest) = split_command(&line);
        match command.as_str() {
            "STATUS" => show_nodes(service, output)?,
            "PAGES" => show_pages(service, output)?,
            "AVAILABLE" => match rest.to_ascii_uppercase().as_str() {
                "ON" => service.set_availability(SysopAvailability::Available)?,
                "OFF" => service.set_availability(SysopAvailability::Unavailable)?,
                _ => writeln!(output, "{}", op("operator-console-availability-help"))
                    .map_err(ApplicationError::SetupIo)?,
            },
            "ANSWER" => {
                let session = parse_session(rest)?;
                let chat = service.answer(session)?;
                writeln!(
                    output,
                    "{}",
                    op_args(
                        "operator-console-chat-active",
                        sf_core::LocalizationArgs::new().with("session", session.get())
                    )
                )
                .map_err(ApplicationError::SetupIo)?;
                run_operator_chat(chat, input, output)?;
            }
            "DECLINE" => service.decline(parse_session(rest)?)?,
            "DISCONNECT" => service.disconnect(parse_session(rest)?)?,
            "CALLERS" => show_callers(service, output)?,
            "PROFILE" => show_caller_profile(service, rest, output)?,
            "PROFILE-SET" => update_caller_profile(service, rest)?,
            "IDENTITY" => {
                write_caller_mutation(
                    output,
                    update_caller_identity(service, rest),
                    "operator-caller-identity-updated",
                )?;
            }
            "ENABLE" => {
                write_caller_mutation(
                    output,
                    service.set_caller_state(rest, CallerState::Active),
                    "operator-caller-enabled",
                )?;
            }
            "DISABLE" => {
                write_caller_mutation(
                    output,
                    service.set_caller_state(rest, CallerState::Disabled),
                    "operator-caller-disabled",
                )?;
            }
            "DELETE" => {
                write_caller_mutation(
                    output,
                    service.set_caller_state(rest, CallerState::Deleted),
                    "operator-caller-deleted",
                )?;
            }
            "RESTORE" => {
                write_caller_mutation(
                    output,
                    service.set_caller_state(rest, CallerState::Active),
                    "operator-caller-restored",
                )?;
            }
            "PURGE" => {
                let (value, name) =
                    rest.split_once(' ')
                        .ok_or(ApplicationError::InvalidSetupValue(
                            "use PURGE <ALLOW|PROTECT> <caller name>",
                        ))?;
                let protected = match value.to_ascii_uppercase().as_str() {
                    "ALLOW" => false,
                    "PROTECT" => true,
                    _ => {
                        return Err(ApplicationError::InvalidSetupValue(
                            "use PURGE <ALLOW|PROTECT> <caller name>",
                        ))
                    }
                };
                write_caller_mutation(
                    output,
                    service.set_caller_purge_protection(name.trim(), protected),
                    "operator-caller-purge-updated",
                )?;
            }
            "SUBSCRIPTION" => {
                let (value, name) =
                    rest.split_once(' ')
                        .ok_or(ApplicationError::InvalidSetupValue(
                            "use SUBSCRIPTION <YYYY-MM-DD|PERMANENT> <caller name>",
                        ))?;
                let expires = if value.eq_ignore_ascii_case("PERMANENT") {
                    None
                } else {
                    Some(
                        chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
                            ApplicationError::InvalidSetupValue(
                                "subscription date must be YYYY-MM-DD or PERMANENT",
                            )
                        })?,
                    )
                };
                write_caller_mutation(
                    output,
                    service.update_caller_subscription(name.trim(), expires),
                    "operator-caller-subscription-updated",
                )?;
            }
            "SECURITY" => {
                let (level, name) =
                    rest.split_once(' ')
                        .ok_or(ApplicationError::InvalidSetupValue(
                            "use SECURITY <level> <caller name>",
                        ))?;
                let level = level.parse::<u16>().map_err(|_| {
                    ApplicationError::InvalidSetupValue("security level must be numeric")
                })?;
                write_caller_mutation(
                    output,
                    service.set_caller_security(name.trim(), level),
                    "operator-caller-security-changed",
                )?;
            }
            "QUIT" | "EXIT" => return Ok(()),
            "" => {}
            _ => writeln!(output, "{}", op("operator-console-unknown-command"))
                .map_err(ApplicationError::SetupIo)?,
        }
    }
}

fn write_caller_mutation(
    output: &mut dyn Write,
    result: Result<Caller, ApplicationError>,
    success_key: &str,
) -> Result<(), ApplicationError> {
    let key = match result {
        Ok(_) => success_key,
        Err(ApplicationError::Database(sf_core::DatabaseError::ProtectedNamedSysop)) => {
            "operator-caller-protected"
        }
        Err(ApplicationError::Database(sf_core::DatabaseError::CallerStateConflict { .. })) => {
            "operator-caller-conflict"
        }
        Err(error) => return Err(error),
    };
    writeln!(output, "{}", op(key)).map_err(ApplicationError::SetupIo)
}

fn run_operator_chat(
    chat: OperatorChat,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<(), ApplicationError> {
    loop {
        let Some(line) = chat.receive_line(Duration::from_secs(300))? else {
            writeln!(output, "{}", op("operator-console-caller-left"))
                .map_err(ApplicationError::SetupIo)?;
            chat.end();
            return Ok(());
        };
        writeln!(
            output,
            "{}",
            op_args(
                "operator-console-caller-line",
                sf_core::LocalizationArgs::new().with("line", line)
            )
        )
        .map_err(ApplicationError::SetupIo)?;
        write!(output, "{}", op("operator-console-sysop-prompt"))
            .map_err(ApplicationError::SetupIo)?;
        output.flush().map_err(ApplicationError::SetupIo)?;
        let Some(reply) = read_console_line(input)? else {
            chat.end();
            return Ok(());
        };
        if reply.eq_ignore_ascii_case("/Q") {
            chat.end();
            return Ok(());
        }
        chat.send_line(&reply)?;
    }
}

fn show_nodes(service: &OperatorService, output: &mut dyn Write) -> Result<(), ApplicationError> {
    for node in service.nodes()? {
        writeln!(
            output,
            "{}",
            op_args(
                "operator-console-node-row",
                sf_core::LocalizationArgs::new()
                    .with("node", node.id.get())
                    .with("state", format!("{:?}", node.state))
                    .with(
                        "session",
                        format!("{:?}", node.session_id.map(SessionId::get))
                    )
                    .with("caller", format!("{:?}", node.caller_name))
                    .with("transport", format!("{:?}", node.transport))
            )
        )
        .map_err(ApplicationError::SetupIo)?;
    }
    Ok(())
}

fn show_pages(service: &OperatorService, output: &mut dyn Write) -> Result<(), ApplicationError> {
    for page in service.pages()? {
        writeln!(
            output,
            "{}",
            op_args(
                "operator-console-page-row",
                sf_core::LocalizationArgs::new()
                    .with("session", page.session_id.get())
                    .with("node", page.node_id.get())
                    .with("caller", format!("{:?}", page.caller_name))
                    .with("state", format!("{:?}", page.state))
            )
        )
        .map_err(ApplicationError::SetupIo)?;
    }
    Ok(())
}

fn show_callers(service: &OperatorService, output: &mut dyn Write) -> Result<(), ApplicationError> {
    for caller in service.callers()? {
        writeln!(
            output,
            "{}",
            op_args(
                "operator-console-caller-row",
                sf_core::LocalizationArgs::new()
                    .with("id", caller.id.get())
                    .with("login", caller.login_identifier)
                    .with("handle", caller.display_name)
                    .with("security", caller.security_level.get())
                    .with("base_security", caller.base_security_level.get())
                    .with("state", format!("{:?}", caller.state))
                    .with("version", caller.state_version)
                    .with("purge_protected", caller.purge_protected.to_string())
                    .with(
                        "subscription",
                        caller
                            .subscription_expires_on
                            .map(|date| date.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "permanent".to_owned()),
                    )
                    .with("calls", caller.call_count)
            )
        )
        .map_err(ApplicationError::SetupIo)?;
    }
    Ok(())
}

fn show_caller_profile(
    service: &OperatorService,
    name: &str,
    output: &mut dyn Write,
) -> Result<(), ApplicationError> {
    let caller = service.caller(name)?;
    let address = &caller.profile.address;
    writeln!(
        output,
        "{}",
        op_args(
            "operator-console-profile-title",
            sf_core::LocalizationArgs::new().with("caller", caller.display_name.as_str())
        )
    )
    .map_err(ApplicationError::SetupIo)?;
    for (label, value) in [
        (
            op("operator-console-profile-address-1"),
            address.line_1.as_deref(),
        ),
        (
            op("operator-console-profile-address-2"),
            address.line_2.as_deref(),
        ),
        (op("operator-console-profile-city"), address.city.as_deref()),
        (
            op("operator-console-profile-region"),
            address.region.as_deref(),
        ),
        (
            op("operator-console-profile-postal"),
            address.postal_code.as_deref(),
        ),
        (
            op("operator-console-profile-country"),
            address.country.as_deref(),
        ),
        (
            op("operator-console-profile-phone"),
            caller.profile.phone.as_deref(),
        ),
        (
            op("operator-console-profile-email"),
            caller.profile.email.as_deref(),
        ),
    ] {
        writeln!(
            output,
            "  {}",
            op_args(
                "operator-console-profile-value",
                sf_core::LocalizationArgs::new().with("label", label).with(
                    "value",
                    value.map_or_else(|| op("operator-console-not-provided"), str::to_owned)
                )
            )
        )
        .map_err(ApplicationError::SetupIo)?;
    }
    writeln!(
        output,
        "  {}",
        op_args(
            "operator-console-profile-value",
            sf_core::LocalizationArgs::new()
                .with("label", op("operator-console-profile-birthday"))
                .with(
                    "value",
                    caller
                        .profile
                        .birthday_iso()
                        .unwrap_or_else(|| op("operator-console-not-provided"))
                )
        )
    )
    .map_err(ApplicationError::SetupIo)
}

fn update_caller_profile(
    service: &OperatorService,
    arguments: &str,
) -> Result<(), ApplicationError> {
    let (field, rest) = arguments
        .split_once(' ')
        .ok_or(ApplicationError::InvalidSetupValue(
            "use PROFILE-SET <field> <caller name>|<value>",
        ))?;
    let (name, value) = rest
        .split_once('|')
        .ok_or(ApplicationError::InvalidSetupValue(
            "separate caller name and value with |",
        ))?;
    let mut caller = service.caller(name.trim())?;
    let value = (!value.trim().is_empty()).then(|| value.trim().to_owned());
    match field.to_ascii_lowercase().as_str() {
        "address1" => caller.profile.address.line_1 = value,
        "address2" => caller.profile.address.line_2 = value,
        "city" => caller.profile.address.city = value,
        "region" => caller.profile.address.region = value,
        "postal" => caller.profile.address.postal_code = value,
        "country" => caller.profile.address.country = value,
        "phone" => caller.profile.phone = value,
        "email" => caller.profile.email = value,
        "birthday" => {
            caller.profile.birthday =
                sf_core::parse_birth_date(value.as_deref().unwrap_or_default())
                    .map_err(sf_core::DatabaseError::from)?
        }
        _ => {
            return Err(ApplicationError::InvalidSetupValue(
                "unknown caller-profile field",
            ))
        }
    }
    service.set_caller_profile(name.trim(), caller.profile)?;
    Ok(())
}

fn update_caller_identity(
    service: &OperatorService,
    arguments: &str,
) -> Result<Caller, ApplicationError> {
    let fields = arguments.split('|').map(str::trim).collect::<Vec<_>>();
    let [name, login_identifier, display_handle, real_name] = fields.as_slice() else {
        return Err(ApplicationError::InvalidSetupValue(
            "use IDENTITY <current name>|<login identifier>|<display handle>|<real name or blank>",
        ));
    };
    if name.is_empty() || login_identifier.is_empty() || display_handle.is_empty() {
        return Err(ApplicationError::InvalidSetupValue(
            "caller name, login identifier, and display handle cannot be blank",
        ));
    }
    service.set_caller_identity(
        name,
        login_identifier,
        display_handle,
        (!real_name.is_empty()).then(|| (*real_name).to_owned()),
    )
}

fn read_console_line(input: &mut dyn BufRead) -> Result<Option<String>, ApplicationError> {
    let mut line = String::new();
    let read = input
        .read_line(&mut line)
        .map_err(ApplicationError::SetupIo)?;
    if read == 0 {
        Ok(None)
    } else {
        Ok(Some(line.trim().to_owned()))
    }
}

fn split_command(line: &str) -> (String, &str) {
    line.split_once(char::is_whitespace).map_or_else(
        || (line.to_ascii_uppercase(), ""),
        |(command, rest)| (command.to_ascii_uppercase(), rest.trim()),
    )
}

fn parse_session(value: &str) -> Result<SessionId, ApplicationError> {
    let value = value
        .parse::<u64>()
        .map_err(|_| ApplicationError::InvalidSetupValue("session identifier must be numeric"))?;
    SessionId::new(value)
        .map_err(|_| ApplicationError::InvalidSetupValue("invalid session identifier"))
}

fn op(key: &str) -> String {
    sf_core::text(key, &sf_core::LocalizationArgs::new())
}

fn op_args(key: &str, args: sf_core::LocalizationArgs) -> String {
    sf_core::text(key, &args)
}
