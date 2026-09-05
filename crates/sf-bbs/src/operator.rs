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

use std::io::{BufRead, Write};
use std::sync::Arc;
use std::time::Duration;

use sf_core::{
    Caller, CallerState, NewOtherBbsEntry, OperatorChat, OtherBbsEntry, OtherBbsId,
    OtherBbsLifecycle, PageRequest, PublicDirectoryPolicy, SecurityLevel, SessionId,
    SysopAvailability,
};
use tracing::info;

use crate::runtime::{BoardStatus, LiveNodeStatus, OperatorObservabilityContext};
use crate::{ApplicationError, BoardRuntime};

/// Presentation-independent operator operations for the running board. The
/// terminal console is one client; a future web console can use this same API.
#[derive(Clone)]
pub struct OperatorService {
    runtime: Arc<BoardRuntime>,
}

impl OperatorService {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_live_control(
        &self,
        principal: String,
        owner: String,
        capabilities: &[sf_core::LocalOperatorCapability],
        authorize_chat: sf_core::ChatAuthorization,
        command_id: String,
        fingerprint: String,
        action: crate::LiveControlAction,
    ) -> Result<crate::MutationResult, ApplicationError> {
        crate::live_control::dispatch(
            self.runtime.clone(),
            principal,
            owner,
            capabilities,
            authorize_chat,
            command_id,
            fingerprint,
            action,
        )
    }
    pub fn live_interactions(&self) -> Result<crate::InteractionSnapshot, ApplicationError> {
        crate::live_control::snapshot(&self.runtime)
    }
    pub fn shutdown_status(&self) -> Result<crate::ShutdownImpact, ApplicationError> {
        crate::shutdown::status(&self.runtime)
    }
    pub fn new(runtime: Arc<BoardRuntime>) -> Self {
        Self { runtime }
    }

    pub fn board_name(&self) -> &str {
        self.runtime.board_name()
    }

    pub fn nodes(&self) -> Result<Vec<sf_core::NodeSnapshot>, ApplicationError> {
        self.runtime.node_snapshots()
    }

    pub fn board_status(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<BoardStatus, ApplicationError> {
        self.runtime.board_status(context)
    }
    pub fn live_nodes(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<Vec<LiveNodeStatus>, ApplicationError> {
        self.runtime.live_node_statuses(context)
    }
    pub fn recent_events(
        &self,
        context: &OperatorObservabilityContext,
        query: &sf_core::EventQuery,
    ) -> Result<sf_core::EventPage, ApplicationError> {
        self.runtime.recent_operational_events(context, query)
    }
    pub fn notifications(
        &self,
        context: &OperatorObservabilityContext,
        include_closed: bool,
        limit: usize,
    ) -> Result<Vec<sf_core::OperatorNotification>, ApplicationError> {
        self.runtime
            .operator_notifications(context, include_closed, limit)
    }
    pub fn statistics(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<sf_core::SystemStatistics, ApplicationError> {
        self.runtime.system_statistics(context)
    }
    pub fn recent_callers(
        &self,
        context: &OperatorObservabilityContext,
        limit: usize,
    ) -> Result<Vec<sf_core::RecentCaller>, ApplicationError> {
        self.runtime.recent_callers(context, limit)
    }
    pub fn maintenance_status(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<sf_core::MaintenanceStatus, ApplicationError> {
        self.runtime.maintenance_status(context)
    }
    pub fn acknowledge_operator_notification(
        &self,
        context: &OperatorObservabilityContext,
        notification_id: sf_core::NotificationId,
        expected_version: u64,
    ) -> Result<bool, ApplicationError> {
        self.runtime
            .acknowledge_operator_notification(context, notification_id, expected_version)
    }
    pub fn subscribe_events(
        &self,
        context: &OperatorObservabilityContext,
    ) -> Result<sf_core::LiveEventSubscription, ApplicationError> {
        self.runtime.subscribe_operational_events(context)
    }
    pub fn poll_events(
        &self,
        context: &OperatorObservabilityContext,
        subscription: &sf_core::LiveEventSubscription,
    ) -> Result<sf_core::LiveEventBatch, ApplicationError> {
        self.runtime
            .poll_operational_event_subscription(context, subscription)
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

    pub fn adjust_session_time(
        &self,
        context: &OperatorObservabilityContext,
        session: SessionId,
        delta_minutes: i16,
    ) -> Result<(), ApplicationError> {
        if !context.capabilities.adjust_session_time {
            return Err(ApplicationError::Usage(
                "operator authorization denied".to_owned(),
            ));
        }
        self.runtime.adjust_session_time(session, delta_minutes)
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

    pub fn public_information_policy(&self) -> Result<PublicDirectoryPolicy, ApplicationError> {
        self.runtime.public_information_policy()
    }

    pub fn update_public_information_policy(
        &self,
        expected_version: u64,
        enabled: bool,
        show_last_call: bool,
        show_location: bool,
        caller_additions: bool,
    ) -> Result<PublicDirectoryPolicy, ApplicationError> {
        self.runtime.update_public_information_policy(
            expected_version,
            enabled,
            show_last_call,
            show_location,
            caller_additions,
        )
    }

    pub fn other_bbs_entries(&self) -> Result<Vec<OtherBbsEntry>, ApplicationError> {
        self.runtime.other_bbs_entries()
    }

    pub fn add_other_bbs(
        &self,
        entry: NewOtherBbsEntry,
    ) -> Result<OtherBbsEntry, ApplicationError> {
        self.runtime.add_other_bbs(entry)
    }

    pub fn edit_other_bbs(
        &self,
        id: OtherBbsId,
        version: u64,
        entry: NewOtherBbsEntry,
    ) -> Result<OtherBbsEntry, ApplicationError> {
        self.runtime.edit_other_bbs(id, version, entry)
    }

    pub fn reorder_other_bbs(
        &self,
        id: OtherBbsId,
        version: u64,
        order: usize,
    ) -> Result<OtherBbsEntry, ApplicationError> {
        self.runtime.reorder_other_bbs(id, version, order)
    }

    pub fn set_other_bbs_lifecycle(
        &self,
        id: OtherBbsId,
        version: u64,
        lifecycle: OtherBbsLifecycle,
    ) -> Result<OtherBbsEntry, ApplicationError> {
        self.runtime.set_other_bbs_lifecycle(id, version, lifecycle)
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
            "INFO-POLICY" => show_public_information_policy(service, output)?,
            "INFO-POLICY-SET" => update_public_information_policy(service, rest, output)?,
            "BBS-LIST" => show_other_bbs_entries(service, output)?,
            "BBS-ADD" => {
                let entry = parse_other_bbs_fields(rest)?;
                write_other_bbs_mutation(output, service.add_other_bbs(entry))?;
            }
            "BBS-EDIT" => {
                let (head, fields) =
                    rest.split_once('|')
                        .ok_or(ApplicationError::InvalidSetupValue(
                            "use BBS-EDIT <id> <version>|<name>|<speed>|<dial>",
                        ))?;
                let (id, version) = parse_other_bbs_identity(head)?;
                write_other_bbs_mutation(
                    output,
                    service.edit_other_bbs(id, version, parse_other_bbs_fields(fields)?),
                )?;
            }
            "BBS-MOVE" => {
                let values = rest.split_ascii_whitespace().collect::<Vec<_>>();
                if values.len() != 3 {
                    return Err(ApplicationError::InvalidSetupValue(
                        "use BBS-MOVE <id> <version> <order>",
                    ));
                }
                let id = OtherBbsId::new(values[0].parse().map_err(|_| {
                    ApplicationError::InvalidSetupValue("Other BBS id must be numeric")
                })?)?;
                let version = values[1].parse().map_err(|_| {
                    ApplicationError::InvalidSetupValue("Other BBS version must be numeric")
                })?;
                let order = values[2].parse().map_err(|_| {
                    ApplicationError::InvalidSetupValue("Other BBS order must be numeric")
                })?;
                write_other_bbs_mutation(output, service.reorder_other_bbs(id, version, order))?;
            }
            "BBS-STATE" => {
                let values = rest.split_ascii_whitespace().collect::<Vec<_>>();
                if values.len() != 3 {
                    return Err(ApplicationError::InvalidSetupValue(
                        "use BBS-STATE <id> <version> <ACTIVE|DISABLED>",
                    ));
                }
                let id = OtherBbsId::new(values[0].parse().map_err(|_| {
                    ApplicationError::InvalidSetupValue("Other BBS id must be numeric")
                })?)?;
                let version = values[1].parse().map_err(|_| {
                    ApplicationError::InvalidSetupValue("Other BBS version must be numeric")
                })?;
                let lifecycle = match values[2].to_ascii_uppercase().as_str() {
                    "ACTIVE" => OtherBbsLifecycle::Active,
                    "DISABLED" => OtherBbsLifecycle::Disabled,
                    _ => {
                        return Err(ApplicationError::InvalidSetupValue(
                            "Other BBS lifecycle must be ACTIVE or DISABLED",
                        ))
                    }
                };
                write_other_bbs_mutation(
                    output,
                    service.set_other_bbs_lifecycle(id, version, lifecycle),
                )?;
            }
            "QUIT" | "EXIT" => return Ok(()),
            "" => {}
            _ => writeln!(output, "{}", op("operator-console-unknown-command"))
                .map_err(ApplicationError::SetupIo)?,
        }
    }
}

fn show_public_information_policy(
    service: &OperatorService,
    output: &mut dyn Write,
) -> Result<(), ApplicationError> {
    let policy = service.public_information_policy()?;
    writeln!(
        output,
        "{}",
        op_args(
            "operator-public-information-policy",
            sf_core::LocalizationArgs::new()
                .with("enabled", policy.enabled.to_string())
                .with("last_call", policy.show_last_call_date.to_string())
                .with("location", policy.show_city_region.to_string())
                .with(
                    "caller_additions",
                    policy.caller_bbs_additions_enabled.to_string()
                )
                .with("version", policy.state_version)
        )
    )
    .map_err(ApplicationError::SetupIo)
}

fn update_public_information_policy(
    service: &OperatorService,
    rest: &str,
    output: &mut dyn Write,
) -> Result<(), ApplicationError> {
    let values = rest.split_ascii_whitespace().collect::<Vec<_>>();
    if values.len() != 5 {
        return Err(ApplicationError::InvalidSetupValue("use INFO-POLICY-SET <version> <ON|OFF> <LAST|NO-LAST> <LOCATION|NO-LOCATION> <CALLER-ADD|NO-CALLER-ADD>"));
    }
    let version = values[0]
        .parse()
        .map_err(|_| ApplicationError::InvalidSetupValue("policy version must be numeric"))?;
    let enabled = parse_policy_flag(values[1], "ON", "OFF")?;
    let last = parse_policy_flag(values[2], "LAST", "NO-LAST")?;
    let location = parse_policy_flag(values[3], "LOCATION", "NO-LOCATION")?;
    let additions = parse_policy_flag(values[4], "CALLER-ADD", "NO-CALLER-ADD")?;
    let policy =
        service.update_public_information_policy(version, enabled, last, location, additions)?;
    writeln!(
        output,
        "{}",
        op_args(
            "operator-public-information-policy-updated",
            sf_core::LocalizationArgs::new().with("version", policy.state_version)
        )
    )
    .map_err(ApplicationError::SetupIo)
}

fn parse_policy_flag(
    value: &str,
    yes: &'static str,
    no: &'static str,
) -> Result<bool, ApplicationError> {
    if value.eq_ignore_ascii_case(yes) {
        Ok(true)
    } else if value.eq_ignore_ascii_case(no) {
        Ok(false)
    } else {
        Err(ApplicationError::InvalidSetupValue(
            "invalid public-information policy flag",
        ))
    }
}

fn show_other_bbs_entries(
    service: &OperatorService,
    output: &mut dyn Write,
) -> Result<(), ApplicationError> {
    for entry in service.other_bbs_entries()? {
        writeln!(
            output,
            "{}",
            op_args(
                "operator-other-bbs-row",
                sf_core::LocalizationArgs::new()
                    .with("id", entry.id.get())
                    .with("order", entry.order)
                    .with("state", format!("{:?}", entry.lifecycle))
                    .with("version", entry.state_version)
                    .with("name", entry.name)
                    .with("speed", entry.speed)
                    .with("dial", entry.dial_string)
                    .with(
                        "contributor",
                        entry
                            .contributor_caller_id
                            .map(|id| id.get().to_string())
                            .unwrap_or_else(|| "operator".to_owned())
                    )
            )
        )
        .map_err(ApplicationError::SetupIo)?;
    }
    Ok(())
}

fn parse_other_bbs_fields(value: &str) -> Result<NewOtherBbsEntry, ApplicationError> {
    let fields = value.split('|').map(str::trim).collect::<Vec<_>>();
    if fields.len() != 3 {
        return Err(ApplicationError::InvalidSetupValue(
            "use <name>|<speed>|<dial>",
        ));
    }
    Ok(NewOtherBbsEntry {
        name: fields[0].to_owned(),
        speed: fields[1].to_owned(),
        dial_string: fields[2].to_owned(),
    })
}

fn parse_other_bbs_identity(value: &str) -> Result<(OtherBbsId, u64), ApplicationError> {
    let values = value.split_ascii_whitespace().collect::<Vec<_>>();
    if values.len() != 2 {
        return Err(ApplicationError::InvalidSetupValue(
            "Other BBS id and version are required",
        ));
    }
    Ok((
        OtherBbsId::new(
            values[0]
                .parse()
                .map_err(|_| ApplicationError::InvalidSetupValue("Other BBS id must be numeric"))?,
        )?,
        values[1].parse().map_err(|_| {
            ApplicationError::InvalidSetupValue("Other BBS version must be numeric")
        })?,
    ))
}

fn write_other_bbs_mutation(
    output: &mut dyn Write,
    result: Result<OtherBbsEntry, ApplicationError>,
) -> Result<(), ApplicationError> {
    match result {
        Ok(entry) => writeln!(
            output,
            "{}",
            op_args(
                "operator-other-bbs-updated",
                sf_core::LocalizationArgs::new()
                    .with("id", entry.id.get())
                    .with("version", entry.state_version)
            )
        )
        .map_err(ApplicationError::SetupIo),
        Err(ApplicationError::Database(sf_core::DatabaseError::PublicInformation(
            sf_core::PublicInformationError::OtherBbsConflict { .. }
            | sf_core::PublicInformationError::PolicyConflict { .. },
        ))) => writeln!(output, "{}", op("operator-public-information-conflict"))
            .map_err(ApplicationError::SetupIo),
        Err(error) => Err(error),
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
                    .with("listed", caller.public_directory_listed.to_string())
                    .with("publicity_version", caller.publicity_state_version)
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
