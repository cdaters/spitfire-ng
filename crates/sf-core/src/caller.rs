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

use std::time::Duration;

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use thiserror::Error;

use crate::{CallerConfig, CallerProfilePolicy, ProfileFieldPolicy};

pub const MAX_CALLER_NAME_BYTES: usize = 30;
pub const MAX_LOGIN_IDENTIFIER_BYTES: usize = 32;
pub const MAX_SECURITY_LEVEL: u16 = 9_999;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallerId(i64);

impl CallerId {
    pub fn new(value: i64) -> Result<Self, CallerError> {
        (value > 0)
            .then_some(Self(value))
            .ok_or(CallerError::InvalidCallerId(value))
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SecurityLevel(u16);

impl SecurityLevel {
    pub fn new(value: u16) -> Result<Self, CallerError> {
        (value <= MAX_SECURITY_LEVEL)
            .then_some(Self(value))
            .ok_or(CallerError::InvalidSecurityLevel(value))
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    pub const fn allows(self, required: Self) -> bool {
        self.0 >= required.0
    }

    pub const fn is_sysop(self, threshold: Self) -> bool {
        self.allows(threshold)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallerState {
    Active,
    Disabled,
    Deleted,
}

impl CallerState {
    pub const fn as_database_value(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Deleted => "deleted",
        }
    }

    pub fn from_database_value(value: &str) -> Result<Self, CallerError> {
        match value {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            "deleted" => Ok(Self::Deleted),
            _ => Err(CallerError::InvalidStoredState(value.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphicsPreference {
    Auto,
    Ansi,
    Text,
}

/// Stock caller-selectable default transfer protocols. The streaming `-g`
/// variants are available at transfer time but were not choices in the
/// documented saved-default menu.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferPreference {
    Select,
    Ascii,
    XmodemChecksum,
    XmodemCrc,
    Xmodem1k,
    Ymodem,
    Zmodem,
    Telink,
}

impl TransferPreference {
    pub const fn as_database_value(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Ascii => "ascii",
            Self::XmodemChecksum => "xmodem-checksum",
            Self::XmodemCrc => "xmodem-crc",
            Self::Xmodem1k => "xmodem-1k",
            Self::Ymodem => "ymodem",
            Self::Zmodem => "zmodem",
            Self::Telink => "telink",
        }
    }

    pub fn from_database_value(value: &str) -> Result<Self, CallerError> {
        match value {
            "select" => Ok(Self::Select),
            "ascii" => Ok(Self::Ascii),
            "xmodem-checksum" => Ok(Self::XmodemChecksum),
            "xmodem-crc" => Ok(Self::XmodemCrc),
            "xmodem-1k" => Ok(Self::Xmodem1k),
            "ymodem" => Ok(Self::Ymodem),
            "zmodem" => Ok(Self::Zmodem),
            "telink" => Ok(Self::Telink),
            _ => Err(CallerError::InvalidTransferPreference(value.to_owned())),
        }
    }

    pub const fn stock_name(self) -> &'static str {
        match self {
            Self::Select => "Select at time of transfer",
            Self::Ascii => "Ascii",
            Self::XmodemChecksum => "Xmodem Checksum",
            Self::XmodemCrc => "Xmodem CRC",
            Self::Xmodem1k => "1K-Xmodem",
            Self::Ymodem => "Ymodem (Batch)",
            Self::Zmodem => "Zmodem (Batch)",
            Self::Telink => "Telink",
        }
    }
}

impl GraphicsPreference {
    pub const fn as_database_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Ansi => "ansi",
            Self::Text => "text",
        }
    }

    pub fn from_database_value(value: &str) -> Result<Self, CallerError> {
        match value {
            "auto" => Ok(Self::Auto),
            "ansi" => Ok(Self::Ansi),
            "text" => Ok(Self::Text),
            _ => Err(CallerError::InvalidGraphicsPreference(value.to_owned())),
        }
    }

    pub const fn allows_ansi(self, terminal_supports_ansi: bool) -> bool {
        terminal_supports_ansi && !matches!(self, Self::Text)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallerPreferences {
    pub graphics: GraphicsPreference,
    pub screen_width: Option<u16>,
    pub page_length: Option<u16>,
    pub more_prompt: bool,
    pub scroll_prompt: bool,
    pub hot_keys: bool,
    pub transfer_protocol: TransferPreference,
}

impl Default for CallerPreferences {
    fn default() -> Self {
        Self {
            graphics: GraphicsPreference::Auto,
            screen_width: None,
            page_length: None,
            more_prompt: true,
            scroll_prompt: false,
            hot_keys: false,
            transfer_protocol: TransferPreference::Select,
        }
    }
}

impl CallerPreferences {
    pub fn validate(self) -> Result<Self, CallerError> {
        if self
            .screen_width
            .is_some_and(|value| !(40..=144).contains(&value))
        {
            return Err(CallerError::InvalidScreenWidth(self.screen_width));
        }
        if self
            .page_length
            .is_some_and(|value| !(10..=24).contains(&value))
        {
            return Err(CallerError::InvalidPageLength(self.page_length));
        }
        Ok(self)
    }

    pub fn effective_width(self, negotiated: Option<u16>) -> u16 {
        self.screen_width
            .or(negotiated)
            .unwrap_or(80)
            .clamp(40, 144)
    }

    pub fn effective_page_length(self, negotiated: Option<u16>) -> u16 {
        self.page_length.map_or_else(
            || {
                negotiated
                    .map(|height| height.saturating_sub(1).clamp(10, 200))
                    .unwrap_or(24)
            },
            |configured| configured.clamp(10, 24),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Caller {
    pub id: CallerId,
    /// Stable, normalized authentication label used by secure transports.
    pub login_identifier: String,
    /// Public BBS identity. This retains the historical `display_name` field
    /// name in storage and APIs to preserve attribution compatibility.
    pub display_name: String,
    pub normalized_name: String,
    /// Privacy-sensitive identity retained separately from public display.
    pub real_name: Option<String>,
    pub security_level: SecurityLevel,
    pub base_security_level: SecurityLevel,
    pub state: CallerState,
    pub state_version: u64,
    pub subscription_expires_on: Option<NaiveDate>,
    pub purge_protected: bool,
    pub lifecycle_prior_state: Option<CallerState>,
    /// Caller-controlled participation in the board's public directory.
    pub public_directory_listed: bool,
    /// Optimistic concurrency version for public-directory preference changes.
    pub publicity_state_version: u64,
    pub first_call_at: i64,
    pub last_call_at: Option<i64>,
    pub call_count: u64,
    pub total_time_seconds: u64,
    pub messages_posted: u64,
    pub files_uploaded: u64,
    pub upload_bytes: u64,
    pub files_downloaded: u64,
    pub download_bytes: u64,
    pub preferences: CallerPreferences,
    pub profile: CallerProfile,
    pub is_new_caller: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CallerProfile {
    pub address: PostalAddress,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birthday: Option<NaiveDate>,
}

impl CallerProfile {
    pub fn validate_for_policy(
        mut self,
        policy: &CallerProfilePolicy,
    ) -> Result<Self, CallerError> {
        self.address = self.address.normalized()?;
        self.phone = normalize_optional(self.phone, 40, "phone")?;
        self.email = normalize_optional(self.email, 254, "email")?;
        if let Some(email) = &self.email {
            validate_email(email)?;
        }
        validate_group_policy(
            "address",
            policy.address,
            !self.address.is_empty(),
            self.address.has_required_core(),
        )?;
        validate_group_policy(
            "phone",
            policy.phone,
            self.phone.is_some(),
            self.phone.is_some(),
        )?;
        validate_group_policy(
            "email",
            policy.email,
            self.email.is_some(),
            self.email.is_some(),
        )?;
        validate_group_policy(
            "birthday",
            policy.birthday,
            self.birthday.is_some(),
            self.birthday.is_some(),
        )?;
        Ok(self)
    }

    /// Validates an edit while preserving values collected before a Sysop
    /// disabled a profile group. Disabled values may remain private in the
    /// record, but neither caller nor operator edit paths may change them.
    pub fn validate_update_for_policy(
        self,
        existing: &Self,
        policy: &CallerProfilePolicy,
    ) -> Result<Self, CallerError> {
        if policy.address == ProfileFieldPolicy::Disabled && self.address != existing.address {
            return Err(CallerError::ProfileFieldDisabled("address"));
        }
        if policy.phone == ProfileFieldPolicy::Disabled && self.phone != existing.phone {
            return Err(CallerError::ProfileFieldDisabled("phone"));
        }
        if policy.email == ProfileFieldPolicy::Disabled && self.email != existing.email {
            return Err(CallerError::ProfileFieldDisabled("email"));
        }
        if policy.birthday == ProfileFieldPolicy::Disabled && self.birthday != existing.birthday {
            return Err(CallerError::ProfileFieldDisabled("birthday"));
        }

        let mut validation_policy = policy.clone();
        for group in [
            &mut validation_policy.address,
            &mut validation_policy.phone,
            &mut validation_policy.email,
            &mut validation_policy.birthday,
        ] {
            if *group == ProfileFieldPolicy::Disabled {
                *group = ProfileFieldPolicy::Optional;
            }
        }
        self.validate_for_policy(&validation_policy)
    }

    pub fn birthday_iso(&self) -> Option<String> {
        self.birthday
            .map(|date| date.format("%Y-%m-%d").to_string())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PostalAddress {
    pub line_1: Option<String>,
    pub line_2: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

impl PostalAddress {
    pub fn is_empty(&self) -> bool {
        self.line_1.is_none()
            && self.line_2.is_none()
            && self.city.is_none()
            && self.region.is_none()
            && self.postal_code.is_none()
            && self.country.is_none()
    }

    fn has_required_core(&self) -> bool {
        self.line_1.is_some() && self.city.is_some() && self.country.is_some()
    }

    fn normalized(mut self) -> Result<Self, CallerError> {
        self.line_1 = normalize_optional(self.line_1, 120, "address line 1")?;
        self.line_2 = normalize_optional(self.line_2, 120, "address line 2")?;
        self.city = normalize_optional(self.city, 80, "city")?;
        self.region = normalize_optional(self.region, 80, "region")?;
        self.postal_code = normalize_optional(self.postal_code, 32, "postal code")?;
        self.country = normalize_optional(self.country, 80, "country")?;
        Ok(self)
    }

    pub fn city_region(&self) -> Option<String> {
        match (&self.city, &self.region) {
            (Some(city), Some(region)) => Some(format!("{city}, {region}")),
            (Some(city), None) => Some(city.clone()),
            (None, Some(region)) => Some(region.clone()),
            (None, None) => None,
        }
    }
}

fn normalize_optional(
    value: Option<String>,
    maximum: usize,
    field: &'static str,
) -> Result<Option<String>, CallerError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim().to_owned();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > maximum || trimmed.chars().any(char::is_control) {
        return Err(CallerError::InvalidProfileField { field, maximum });
    }
    Ok(Some(trimmed))
}

pub(crate) fn validate_email(email: &str) -> Result<(), CallerError> {
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || parts.next().is_some()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || !domain.contains('.')
        || email.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(CallerError::InvalidEmail);
    }
    Ok(())
}

fn validate_group_policy(
    field: &'static str,
    policy: ProfileFieldPolicy,
    supplied: bool,
    complete: bool,
) -> Result<(), CallerError> {
    match policy {
        ProfileFieldPolicy::Disabled if supplied => Err(CallerError::ProfileFieldDisabled(field)),
        ProfileFieldPolicy::Required if !complete => Err(CallerError::RequiredProfileField(field)),
        _ => Ok(()),
    }
}

pub fn parse_birth_date(value: &str) -> Result<Option<NaiveDate>, CallerError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || value
            .bytes()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err(CallerError::InvalidBirthDate);
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| CallerError::InvalidBirthDate)
}

/// Stable local-date key for daily accounting after IANA timezone conversion.
pub fn board_local_day(timestamp: i64, timezone: Tz) -> Result<i32, CallerError> {
    let utc = chrono::DateTime::<Utc>::from_timestamp(timestamp, 0)
        .ok_or(CallerError::InvalidTimestamp(timestamp))?;
    let local = utc.with_timezone(&timezone);
    Ok(local.year() * 10_000 + local.month() as i32 * 100 + local.day() as i32)
}

/// Formats a persisted Unix timestamp in the board's configured civil
/// timezone. Caller-facing dates remain full-year and unambiguous internally;
/// presentation code must not silently fall back to the host timezone.
pub fn format_board_local_timestamp(timestamp: i64, timezone: Tz) -> Result<String, CallerError> {
    let utc = chrono::DateTime::<Utc>::from_timestamp(timestamp, 0)
        .ok_or(CallerError::InvalidTimestamp(timestamp))?;
    Ok(utc
        .with_timezone(&timezone)
        .format("%Y-%m-%d %H:%M %Z")
        .to_string())
}

/// Returns the portion of a node-local monotonic session elapsed duration
/// attributable to the board's current civil day. This keeps daily context
/// honest across DST and midnight without replacing the monotonic call clock.
pub fn daily_session_elapsed_seconds(
    session_started_at: i64,
    board_now: i64,
    elapsed: Duration,
    timezone: Tz,
) -> Result<u64, CallerError> {
    if board_local_day(session_started_at, timezone)? == board_local_day(board_now, timezone)? {
        return Ok(elapsed.as_secs());
    }
    let utc = chrono::DateTime::<Utc>::from_timestamp(board_now, 0)
        .ok_or(CallerError::InvalidTimestamp(board_now))?;
    let local = utc.with_timezone(&timezone);
    let midnight = timezone
        .with_ymd_and_hms(local.year(), local.month(), local.day(), 0, 0, 0)
        .earliest()
        .ok_or(CallerError::InvalidTimestamp(board_now))?;
    Ok(board_now
        .saturating_sub(midnight.timestamp())
        .max(0)
        .try_into()
        .unwrap_or(u64::MAX)
        .min(elapsed.as_secs()))
}

/// Privacy-bounded category for the most recent caller-associated access
/// denial. No password material, remote address, supplied identity, or backend
/// error detail enters this value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessDenialReason {
    InvalidCredentials,
    AccountUnavailable,
    PrivateBoardPolicy,
    DailyCallLimit,
    DailyTimeLimit,
}

impl AccessDenialReason {
    pub const fn as_database_value(self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid-credentials",
            Self::AccountUnavailable => "account-unavailable",
            Self::PrivateBoardPolicy => "private-board-policy",
            Self::DailyCallLimit => "daily-call-limit",
            Self::DailyTimeLimit => "daily-time-limit",
        }
    }

    pub fn from_database_value(value: &str) -> Result<Self, CallerError> {
        match value {
            "invalid-credentials" => Ok(Self::InvalidCredentials),
            "account-unavailable" => Ok(Self::AccountUnavailable),
            "private-board-policy" => Ok(Self::PrivateBoardPolicy),
            "daily-call-limit" => Ok(Self::DailyCallLimit),
            "daily-time-limit" => Ok(Self::DailyTimeLimit),
            _ => Err(CallerError::InvalidStoredAccessDenial(value.to_owned())),
        }
    }

    pub const fn caller_description(self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid credentials",
            Self::AccountUnavailable => "an unavailable caller account",
            Self::PrivateBoardPolicy => "board access policy",
            Self::DailyCallLimit => "the daily call limit",
            Self::DailyTimeLimit => "the daily time limit",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallerAccessDenial {
    generation: u64,
    occurred_at: i64,
    reason: AccessDenialReason,
}

impl CallerAccessDenial {
    pub(crate) const fn new(generation: u64, occurred_at: i64, reason: AccessDenialReason) -> Self {
        Self {
            generation,
            occurred_at,
            reason,
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn occurred_at(&self) -> i64 {
        self.occurred_at
    }

    pub const fn reason(&self) -> AccessDenialReason {
        self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedCaller {
    pub caller: Caller,
    /// True only for the first successfully authenticated session after native
    /// registration. This drives the stock NEWUSER display without retaining
    /// a stale caller-record flag after session accounting begins.
    pub first_session: bool,
    pub previous_call_at: Option<i64>,
    pub calls_today: u32,
    pub time_used_today_seconds: u64,
    pub daily_limit_seconds: u64,
    pub session_started_at: i64,
    pub pending_access_denial: Option<CallerAccessDenial>,
    pub allowance: SessionAllowance,
}

/// Engine-owned, privacy-safe snapshot of the live facts approved for caller
/// presentation. It contains no authority or mutation hooks and is rebuilt
/// from persisted caller state plus the node-local monotonic session clock.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallerSessionContext {
    board_now: i64,
    timezone: Tz,
    caller_id: CallerId,
    security_level: SecurityLevel,
    elapsed_seconds: u64,
    call_remaining_seconds: u64,
    daily_remaining_seconds: u64,
    calls_today: u32,
    calls_remaining_today: u32,
    total_calls: u64,
    previous_call_at: Option<i64>,
}

impl CallerSessionContext {
    pub fn from_authenticated(
        authenticated: &AuthenticatedCaller,
        config: &CallerConfig,
        timezone: Tz,
        board_now: i64,
        elapsed: Duration,
    ) -> Result<Self, CallerError> {
        let policy = TimePolicy::for_security(config, authenticated.caller.security_level);
        let elapsed_seconds = elapsed.as_secs();
        let same_day = board_local_day(authenticated.session_started_at, timezone)?
            == board_local_day(board_now, timezone)?;
        let daily_session_seconds = daily_session_elapsed_seconds(
            authenticated.session_started_at,
            board_now,
            elapsed,
            timezone,
        )?;
        let time_used_before_session = if same_day {
            authenticated.time_used_today_seconds
        } else {
            0
        };
        let daily_limit_seconds = if same_day {
            authenticated.daily_limit_seconds
        } else {
            policy.daily_limit_seconds(None)
        };
        let calls_today = if same_day {
            authenticated.calls_today
        } else {
            0
        };
        Ok(Self {
            board_now,
            timezone,
            caller_id: authenticated.caller.id,
            security_level: authenticated.caller.security_level,
            elapsed_seconds,
            call_remaining_seconds: authenticated.allowance.remaining(elapsed).as_secs(),
            daily_remaining_seconds: daily_limit_seconds
                .saturating_sub(time_used_before_session.saturating_add(daily_session_seconds)),
            calls_today,
            calls_remaining_today: policy.maximum_daily_calls.saturating_sub(calls_today),
            total_calls: authenticated.caller.call_count,
            previous_call_at: authenticated.previous_call_at,
        })
    }

    pub fn board_local_now(&self) -> Result<String, CallerError> {
        format_board_local_timestamp(self.board_now, self.timezone)
    }

    pub fn previous_call_local(&self) -> Result<Option<String>, CallerError> {
        self.previous_call_at
            .map(|timestamp| format_board_local_timestamp(timestamp, self.timezone))
            .transpose()
    }

    pub const fn board_now_timestamp(&self) -> i64 {
        self.board_now
    }

    pub const fn board_timezone(&self) -> Tz {
        self.timezone
    }

    pub const fn previous_call_timestamp(&self) -> Option<i64> {
        self.previous_call_at
    }

    pub const fn caller_id(&self) -> CallerId {
        self.caller_id
    }

    pub const fn security_level(&self) -> SecurityLevel {
        self.security_level
    }

    pub const fn elapsed_seconds(&self) -> u64 {
        self.elapsed_seconds
    }

    pub const fn call_remaining_seconds(&self) -> u64 {
        self.call_remaining_seconds
    }

    pub const fn daily_remaining_seconds(&self) -> u64 {
        self.daily_remaining_seconds
    }

    pub const fn calls_today(&self) -> u32 {
        self.calls_today
    }

    pub const fn calls_remaining_today(&self) -> u32 {
        self.calls_remaining_today
    }

    pub const fn total_calls(&self) -> u64 {
        self.total_calls
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionAllowance {
    limit_seconds: u64,
}

impl SessionAllowance {
    pub const fn new(limit_seconds: u64) -> Self {
        Self { limit_seconds }
    }

    pub const fn limit_seconds(self) -> u64 {
        self.limit_seconds
    }

    /// Applies a bounded, already-authorized upload-time credit to this live
    /// session. Durable policy/accounting remains the database's authority.
    pub const fn credit_seconds(self, seconds: u64) -> Self {
        Self {
            limit_seconds: self.limit_seconds.saturating_add(seconds),
        }
    }

    pub fn remaining(self, elapsed: Duration) -> Duration {
        Duration::from_secs(self.limit_seconds.saturating_sub(elapsed.as_secs()))
    }

    pub fn expired(self, elapsed: Duration) -> bool {
        elapsed.as_secs() >= self.limit_seconds
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimePolicy {
    pub minutes_per_call: u32,
    pub minutes_per_day: u32,
    pub maximum_daily_calls: u32,
}

impl TimePolicy {
    pub fn for_security(config: &CallerConfig, security: SecurityLevel) -> Self {
        let override_limit = config
            .security_limits
            .iter()
            .find(|limit| limit.security_level == security.get());
        Self {
            minutes_per_call: override_limit
                .map_or(config.minutes_per_call, |limit| limit.minutes_per_call),
            minutes_per_day: override_limit
                .map_or(config.minutes_per_day, |limit| limit.minutes_per_day),
            maximum_daily_calls: config.maximum_daily_calls,
        }
    }

    pub fn allowance(
        self,
        used_today_seconds: u64,
        first_day_cap_minutes: Option<u32>,
    ) -> SessionAllowance {
        let daily_remaining = self
            .daily_limit_seconds(first_day_cap_minutes)
            .saturating_sub(used_today_seconds);
        let per_call = u64::from(self.minutes_per_call).saturating_mul(60);
        SessionAllowance::new(per_call.min(daily_remaining))
    }

    pub fn daily_limit_seconds(self, first_day_cap_minutes: Option<u32>) -> u64 {
        let minutes = first_day_cap_minutes.map_or(self.minutes_per_day, |first_day| {
            self.minutes_per_day.min(first_day)
        });
        u64::from(minutes).saturating_mul(60)
    }
}

/// Stock SPITFIRE caller lookup is case-insensitive. The first native model
/// also collapses ASCII whitespace and deliberately accepts only printable
/// ASCII until a complete CP437 identity policy is specified.
pub fn canonicalize_caller_name(input: &[u8]) -> Result<(String, String), CallerError> {
    if !input
        .iter()
        .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
    {
        return Err(CallerError::CallerNameEncoding);
    }
    let raw = std::str::from_utf8(input).map_err(|_| CallerError::CallerNameEncoding)?;
    let display = raw.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    if display.is_empty() || display.len() > MAX_CALLER_NAME_BYTES {
        return Err(CallerError::CallerNameLength(display.len()));
    }
    let normalized = display.to_ascii_lowercase();
    Ok((display, normalized))
}

/// Normalizes the durable login label shared by SSH and future secure
/// transports. Display handles and real names deliberately use different
/// rules and are never accepted here by implication.
pub fn canonicalize_login_identifier(input: &[u8]) -> Result<String, CallerError> {
    if input.is_empty() || input.len() > MAX_LOGIN_IDENTIFIER_BYTES {
        return Err(CallerError::LoginIdentifierLength(input.len()));
    }
    if !input[0].is_ascii_alphanumeric()
        || !input
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(CallerError::LoginIdentifierSyntax);
    }
    Ok(input
        .iter()
        .map(u8::to_ascii_lowercase)
        .map(char::from)
        .collect())
}

/// Produces the collision-independent base used by schema migration. The
/// migration stores this result and resolves collisions by stable caller ID.
pub fn derive_login_identifier_base(normalized_name: &str) -> String {
    let mut derived = String::new();
    let mut separator = false;
    for byte in normalized_name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.') {
            if separator && !derived.is_empty() {
                derived.push('-');
            }
            separator = false;
            derived.push(char::from(byte.to_ascii_lowercase()));
        } else if !derived.is_empty() {
            separator = true;
        }
    }
    while derived.ends_with(['-', '_', '.']) {
        derived.pop();
    }
    if derived.is_empty() {
        derived.push_str("caller");
    }
    derived.truncate(MAX_LOGIN_IDENTIFIER_BYTES);
    while derived.ends_with(['-', '_', '.']) {
        derived.pop();
    }
    derived
}

#[derive(Debug, Error)]
pub enum CallerError {
    #[error("caller identifier must be positive, got {0}")]
    InvalidCallerId(i64),
    #[error("security level must be in 0..=9999, got {0}")]
    InvalidSecurityLevel(u16),
    #[error("caller name must use printable ASCII until the CP437 identity policy is complete")]
    CallerNameEncoding,
    #[error("caller name must contain 1..=30 bytes after whitespace normalization, got {0}")]
    CallerNameLength(usize),
    #[error("login identifier must contain 1..=32 ASCII bytes, got {0}")]
    LoginIdentifierLength(usize),
    #[error("login identifier must begin with an ASCII letter or digit and contain only ASCII letters, digits, '-', '_', or '.'")]
    LoginIdentifierSyntax,
    #[error("real name must be absent or contain 1..=120 UTF-8 bytes without control characters")]
    InvalidRealName,
    #[error("database contains unknown caller state {0:?}")]
    InvalidStoredState(String),
    #[error("database contains unknown graphics preference {0:?}")]
    InvalidGraphicsPreference(String),
    #[error("database contains unknown transfer preference {0:?}")]
    InvalidTransferPreference(String),
    #[error("database contains unknown access-denial category {0:?}")]
    InvalidStoredAccessDenial(String),
    #[error("screen width must be automatic or in 40..=144, got {0:?}")]
    InvalidScreenWidth(Option<u16>),
    #[error("page length must be automatic or in 10..=24, got {0:?}")]
    InvalidPageLength(Option<u16>),
    #[error("caller {field} must contain no control characters and at most {maximum} UTF-8 bytes")]
    InvalidProfileField { field: &'static str, maximum: usize },
    #[error("caller email address has invalid syntax")]
    InvalidEmail,
    #[error("caller birthday must be a real date in YYYY-MM-DD form")]
    InvalidBirthDate,
    #[error("caller profile field {0} is disabled by board policy")]
    ProfileFieldDisabled(&'static str),
    #[error("caller profile field {0} is required by board policy")]
    RequiredProfileField(&'static str),
    #[error("timestamp {0} cannot be represented for board-local accounting")]
    InvalidTimestamp(i64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn caller_names_are_ascii_case_insensitive_and_space_normalized() {
        assert_eq!(
            canonicalize_caller_name(b"  Craig   Daters ").unwrap(),
            ("Craig Daters".to_owned(), "craig daters".to_owned())
        );
        assert!(canonicalize_caller_name(&[0x80]).is_err());
        assert!(canonicalize_caller_name(&[b'X'; 31]).is_err());
    }

    #[test]
    fn login_identifiers_have_a_bounded_ssh_safe_canonical_form() {
        assert_eq!(
            canonicalize_login_identifier(b"Pixel.WIZARD_7").unwrap(),
            "pixel.wizard_7"
        );
        assert!(canonicalize_login_identifier(b"not valid").is_err());
        assert!(canonicalize_login_identifier(b"-leading").is_err());
        assert!(canonicalize_login_identifier(&[b'x'; 33]).is_err());
        assert_eq!(
            derive_login_identifier_base("craig  daters"),
            "craig-daters"
        );
        assert_eq!(derive_login_identifier_base("***"), "caller");
    }

    #[test]
    fn security_comparison_matches_stock_threshold_semantics() {
        let caller = SecurityLevel::new(50).unwrap();
        assert!(caller.allows(SecurityLevel::new(10).unwrap()));
        assert!(caller.is_sysop(SecurityLevel::new(50).unwrap()));
        assert!(!SecurityLevel::new(49).unwrap().is_sysop(caller));
    }

    #[test]
    fn time_policy_uses_exact_security_override_and_first_day_cap() {
        let config = CallerConfig {
            security_limits: vec![crate::SecurityLimitConfig {
                security_level: 10,
                minutes_per_call: 45,
                minutes_per_day: 60,
            }],
            ..CallerConfig::default()
        };
        let policy = TimePolicy::for_security(&config, SecurityLevel::new(10).unwrap());
        assert_eq!(policy.minutes_per_call, 45);
        assert_eq!(policy.allowance(600, Some(20)).limit_seconds(), 600);
        assert_eq!(policy.allowance(1_800, Some(20)).limit_seconds(), 0);
        assert!(policy.allowance(3_600, None).expired(Duration::ZERO));
    }

    #[test]
    fn caller_session_context_is_board_local_bounded_and_node_local_input() {
        let config = CallerConfig {
            security_limits: vec![crate::SecurityLimitConfig {
                security_level: 10,
                minutes_per_call: 45,
                minutes_per_day: 60,
            }],
            ..CallerConfig::default()
        };
        let caller = Caller {
            id: CallerId::new(7).unwrap(),
            login_identifier: "context-caller".to_owned(),
            display_name: "Context Caller".to_owned(),
            normalized_name: "context caller".to_owned(),
            real_name: Some("Context Caller".to_owned()),
            security_level: SecurityLevel::new(10).unwrap(),
            base_security_level: SecurityLevel::new(10).unwrap(),
            state: CallerState::Active,
            state_version: 0,
            subscription_expires_on: None,
            purge_protected: true,
            lifecycle_prior_state: None,
            public_directory_listed: false,
            publicity_state_version: 0,
            first_call_at: 1_735_689_600,
            last_call_at: Some(1_735_689_600),
            call_count: 12,
            total_time_seconds: 3_600,
            messages_posted: 0,
            files_uploaded: 0,
            upload_bytes: 0,
            files_downloaded: 0,
            download_bytes: 0,
            preferences: CallerPreferences::default(),
            profile: CallerProfile::default(),
            is_new_caller: false,
        };
        let authenticated = AuthenticatedCaller {
            caller,
            first_session: false,
            previous_call_at: Some(1_735_689_600),
            calls_today: 2,
            time_used_today_seconds: 600,
            daily_limit_seconds: 3_600,
            session_started_at: 1_735_689_600,
            pending_access_denial: None,
            allowance: SessionAllowance::new(2_700),
        };
        let context = CallerSessionContext::from_authenticated(
            &authenticated,
            &config,
            chrono_tz::America::Phoenix,
            1_735_689_600,
            Duration::from_secs(120),
        )
        .unwrap();
        assert_eq!(context.board_local_now().unwrap(), "2024-12-31 17:00 MST");
        assert_eq!(
            context.previous_call_local().unwrap(),
            Some("2024-12-31 17:00 MST".to_owned())
        );
        assert_eq!(context.elapsed_seconds(), 120);
        assert_eq!(context.call_remaining_seconds(), 2_580);
        assert_eq!(context.daily_remaining_seconds(), 2_880);
        assert_eq!(context.calls_remaining_today(), 8);
    }

    #[test]
    fn daily_session_elapsed_resets_at_board_midnight_and_respects_dst() {
        let phoenix = chrono_tz::America::Phoenix;
        let start = phoenix
            .with_ymd_and_hms(2025, 1, 1, 23, 59, 0)
            .unwrap()
            .timestamp();
        let now = phoenix
            .with_ymd_and_hms(2025, 1, 2, 0, 1, 0)
            .unwrap()
            .timestamp();
        assert_eq!(
            daily_session_elapsed_seconds(start, now, Duration::from_secs(120), phoenix).unwrap(),
            60
        );

        let new_york = chrono_tz::America::New_York;
        let dst_start = new_york
            .with_ymd_and_hms(2025, 3, 8, 23, 59, 0)
            .unwrap()
            .timestamp();
        let after_jump = new_york
            .with_ymd_and_hms(2025, 3, 9, 3, 30, 0)
            .unwrap()
            .timestamp();
        assert_eq!(
            daily_session_elapsed_seconds(
                dst_start,
                after_jump,
                Duration::from_secs(9_060),
                new_york,
            )
            .unwrap(),
            9_000
        );
    }

    #[test]
    fn preferences_preserve_stock_ranges_and_capability_precedence() {
        let automatic = CallerPreferences::default();
        assert_eq!(automatic.effective_width(Some(132)), 132);
        assert_eq!(automatic.effective_page_length(Some(25)), 24);
        assert!(automatic.graphics.allows_ansi(true));
        let text = CallerPreferences {
            graphics: GraphicsPreference::Text,
            screen_width: Some(80),
            page_length: Some(15),
            ..automatic
        }
        .validate()
        .unwrap();
        assert!(!text.graphics.allows_ansi(true));
        assert_eq!(text.effective_page_length(Some(24)), 15);
        assert!(CallerPreferences {
            screen_width: Some(39),
            ..automatic
        }
        .validate()
        .is_err());
    }

    #[test]
    fn profile_policy_validates_private_contact_data_without_us_assumptions() {
        let policy = CallerProfilePolicy {
            address: ProfileFieldPolicy::Required,
            phone: ProfileFieldPolicy::Optional,
            email: ProfileFieldPolicy::Required,
            birthday: ProfileFieldPolicy::Required,
        };
        let profile = CallerProfile {
            address: PostalAddress {
                line_1: Some("  001 Harbour Road  ".to_owned()),
                city: Some("Auckland".to_owned()),
                postal_code: Some("0010".to_owned()),
                country: Some("New Zealand".to_owned()),
                ..PostalAddress::default()
            },
            phone: Some("+64 9 555 0100".to_owned()),
            email: Some("caller@example.nz".to_owned()),
            birthday: parse_birth_date("1985-02-28").unwrap(),
        }
        .validate_for_policy(&policy)
        .unwrap();
        assert_eq!(profile.address.line_1.as_deref(), Some("001 Harbour Road"));
        assert_eq!(profile.address.postal_code.as_deref(), Some("0010"));
        assert_eq!(profile.birthday_iso().as_deref(), Some("1985-02-28"));
        assert!(parse_birth_date("25-02-28").is_err());
        assert!(parse_birth_date("2025-02-29").is_err());
    }

    #[test]
    fn disabled_and_required_profile_policies_fail_closed() {
        let supplied = CallerProfile {
            phone: Some("555-0100".to_owned()),
            ..CallerProfile::default()
        };
        assert!(matches!(
            supplied.validate_for_policy(&CallerProfilePolicy::default()),
            Err(CallerError::ProfileFieldDisabled("phone"))
        ));
        let required = CallerProfilePolicy {
            email: ProfileFieldPolicy::Required,
            ..CallerProfilePolicy::default()
        };
        assert!(matches!(
            CallerProfile::default().validate_for_policy(&required),
            Err(CallerError::RequiredProfileField("email"))
        ));
    }

    #[test]
    fn disabled_profile_values_are_preserved_but_cannot_be_changed() {
        let existing = CallerProfile {
            phone: Some("+1 602 555 0100".to_owned()),
            ..CallerProfile::default()
        };
        let policy = CallerProfilePolicy {
            phone: ProfileFieldPolicy::Disabled,
            email: ProfileFieldPolicy::Optional,
            ..CallerProfilePolicy::default()
        };

        let mut unrelated_edit = existing.clone();
        unrelated_edit.email = Some("caller@example.test".to_owned());
        let validated = unrelated_edit
            .validate_update_for_policy(&existing, &policy)
            .unwrap();
        assert_eq!(validated.phone, existing.phone);
        assert_eq!(validated.email.as_deref(), Some("caller@example.test"));

        let mut forbidden_edit = existing.clone();
        forbidden_edit.phone = Some("+1 480 555 0101".to_owned());
        assert!(matches!(
            forbidden_edit.validate_update_for_policy(&existing, &policy),
            Err(CallerError::ProfileFieldDisabled("phone"))
        ));
    }

    #[test]
    fn board_local_day_uses_the_configured_civil_midnight_across_dst() {
        let phoenix: Tz = "America/Phoenix".parse().unwrap();
        let los_angeles: Tz = "America/Los_Angeles".parse().unwrap();
        let before = phoenix
            .with_ymd_and_hms(2026, 8, 20, 23, 59, 59)
            .single()
            .unwrap()
            .timestamp();
        let after = before + 1;
        assert_eq!(board_local_day(before, phoenix).unwrap(), 20_260_820);
        assert_eq!(board_local_day(after, phoenix).unwrap(), 20_260_821);
        let spring = los_angeles
            .with_ymd_and_hms(2026, 3, 8, 23, 59, 59)
            .single()
            .unwrap()
            .timestamp();
        assert_eq!(board_local_day(spring, los_angeles).unwrap(), 20_260_308);
        assert_eq!(
            board_local_day(spring + 1, los_angeles).unwrap(),
            20_260_309
        );
    }
}
