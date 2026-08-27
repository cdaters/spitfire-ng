# Native Caller and Authentication Model

## Purpose and status

This is the canonical implementation and compatibility specification for the
native SPITFIRE NG caller lifecycle introduced by Stock SPITFIRE 3.7 Core
Parity Increment 2. It explains which stock caller concepts are preserved,
which unsafe or unnecessary historical mechanisms are modernized, and where
the implementation lives.

Increment 2 established the model. The 2026-08-21 caller-policy/profile
closure adds schema 7 private profiles, board-local daily accounting,
private-board admission, no-activity enforcement, caller/Sysop profile
editing, and production product identity. It does not import `SFUSERS.DAT` or
implement the Category-B `SFNEWU.QUE` questionnaire engine.

Primary historical evidence is Buffalo Creek's read-only SPITFIRE 3.7 manual,
`research/samples/shareware-software/sf37-2/spitfire.doc`, especially sections
3.2, 5.4, 5.9, 7, 8.2, 9.2, and the `DAILYLMT.DAT` description. The current
parity status is maintained in
[the stock checklist](stock-spitfire-3.7-parity.md).

## Historical behavior established from the manual

The manual establishes the following operational behavior for stock 3.7:

- The configured Sysop display name and the Sysop caller/login name are
  distinct. The latter is the ordinary caller record used to log in and the
  recipient identity for caller comments.
- Caller names contain up to 30 characters. Caller searching is explicitly
  case-insensitive. The documented login flow asks for caller name and
  password.
- New callers receive a configured initial numerical security level and a
  configured first-day time allowance.
- Menu, conference, file-area, and privileged access normally compare the
  caller's numerical security level with a required threshold. Up to four
  decimal security digits are accepted by caller maintenance. A configured
  Sysop threshold grants stock BBS authority to callers at or above it.
- Global minutes-per-call and minutes-per-day values exist.
  `DAILYLMT.DAT` can override both for an exact security level using `MPC` and
  `MPD`. The configuration also limits caller accesses per day.
- The caller record includes original/first call, last call, times on, time
  information, security, locked/deletion state, and many message/file fields.
- A locked caller is denied access. Mark-for-deletion was historically a
  separate state followed by physical packing.
- Stock new-user setup collected telephone, city/state, and optionally birth
  date. Caller and Sysop maintenance could edit these values. The historical
  birth-date second-password and Sysop password display are not reproduced.
- `No Activity Time Limit` is the live session's keyboard-idle timeout and
  displays `SFASLEEP` before logoff. Dormant-account purging is instead an
  operator-run `SFPCKUSR` maintenance action with an age selection and caller
  purge protection; SFNG does not conflate the two or automatically destroy
  dormant records.
- Private-board mode accepts only existing callers whose security meets the
  configured private-board level; rejected callers receive the `PRIVATE`
  display and are disconnected.

The manual does not establish that SPITFIRE 3.7 used a separate caller alias
for login. The first native model therefore uses one caller display/login name
rather than inventing a handle/real-name distinction.

## Native caller model

Each caller has a stable integer `CallerId` independent of the displayed name.
The native record currently stores:

| Field | Treatment | Reason |
|---|---|---|
| Stable caller ID | Preserved directly in a modern form | Future messages, files, and revisions need identity independent of a mutable name. |
| Caller display/login name | Preserved directly | Central stock identity; limited to 30 normalized bytes in the first implementation. |
| Normalized lookup name | Derived | Enforces case-insensitive uniqueness without changing display spelling. |
| Password | Modernized into a separate Argon2id PHC credential | Stock plaintext/viewable handling is unsafe. |
| Numerical security level | Preserved directly | Stock menu/conference/file/Sysop access depends on it. |
| Active/disabled/deleted state | Preserved in a non-destructive modern form | Represents stock lockout and deletion intent without requiring record packing. |
| New-caller state | Preserved directly during registration completion | Supports first-entry behavior without collecting unnecessary fields. |
| First call, last call, call count, accumulated seconds | Preserved directly | These are caller-visible stock statistics and policy inputs. |
| Daily call/time counters | Derived operational state | Enforces stock access policy without DOS clock arithmetic. |
| Structured postal address, phone, email, full birth date | Modernized, private by default | Sysop policy makes each group disabled, optional, or required. Postal/phone remain strings; birth date is strict `YYYY-MM-DD`. |
| Upload/download/message counters and last area/conference | Deferred | Their owning stock increments have not been implemented. |
| Expert/more/terminal preferences | Preserved/modernized | Schema 5 persists graphics/text, width, page length, MORE, scroll-prompt, and hot-key choices. Session-local Xpert remains separate. See [Caller/Sysop Interaction](sfng-caller-sysop-interaction.md). |
| Subscription, purge protection, questionnaire responses | Deferred | Stock advanced/maintenance behavior remains listed in the parity checklist. |

`CallerState::Disabled` denies login. `Deleted` remains a durable state rather
than causing immediate physical removal. Administrative state-changing UI is
not part of Increment 2.

## Name normalization

The first native identity policy:

1. accepts printable ASCII plus spaces;
2. trims leading/trailing ASCII whitespace;
3. collapses runs of ASCII whitespace to one space;
4. preserves the resulting spelling for display;
5. converts ASCII letters to lower case for the unique lookup key; and
6. rejects empty or longer-than-30-byte names.

Thus `Alex   Caller`, `ALEX CALLER`, and `alex caller` address one account.
Printable CP437 caller-name support is deferred until its database and
case-folding semantics can be specified without silently converting bytes.

## Credentials

`CredentialHasher` is the one password authority used by interactive login,
new-caller creation, explicit fixture-Sysop initialization, and optional
RLogin auto-login. It uses RustCrypto Argon2 **Argon2id version 19**, a fresh
operating-system-random salt for every credential, and a PHC string that
retains algorithm and cost parameters for later upgrades.

The configurable defaults are 19,456 KiB memory, two iterations, and one lane.
The default password length is 10–128 bytes. Passwords are never logged,
included in errors, displayed, or stored reversibly. Telnet suppresses
server-side input echo during secret entry; Unix TTY mode uses a maintained
no-echo password reader. Raw TCP, RLogin, serial, and modem streams do not add
server-side echo.

Argon2id protects stored credentials. It does **not** encrypt a connection.
Telnet, raw TCP, RLogin, and direct serial are plaintext compatibility
transports. They should be restricted to appropriate private/trusted paths
when credential exposure is a concern. Future SSH remains a separate secure
transport; it does not redefine caller identity.

## SQLite schema and private profiles

Schema 2 established caller identity and credentials. Schema 5 added terminal
preferences, schema 6 added transfer preference, schema 7 added nullable
structured address fields, phone, email, and ISO birth date, and schema 10
adds one privacy-bounded latest access-denial record per known caller without
changing the authentication boundary. Its
canonical behavior is documented in
[Caller/Sysop Interaction](sfng-caller-sysop-interaction.md).

Migration 2 adds only `callers` and `caller_credentials`. The migration is
transactional and upgrades an Increment 0/1 schema without replacing board
identity.

`callers` enforces unique normalized names, the 0–9999 security range,
recognized account states, nonnegative counters, and stable primary keys.
`caller_credentials` has a one-to-one foreign key to callers and records an
explicit credential scheme plus PHC hash. Foreign keys remain enabled and all
queries are parameterized.

Call accounting is committed at authenticated entry and cleanly finalized at
session teardown. Daily usage keys are derived from the configured IANA board
timezone. UTC timestamps are converted to a local civil date, so the reset is
at the board's local midnight and naturally follows daylight-saving offset
changes; ambiguous or nonexistent wall-clock hours are irrelevant because the
boundary is derived from an existing instant rather than parsing a local
time.

The schema-10 denial record is not a security-event history. It stores only
the caller ID, timestamp, one generic allowlisted reason, and generation-safe
acknowledgement state. Unknown supplied names do not create a row. Passwords,
submitted values, remote addresses, contact details, and backend error detail
are neither stored nor displayed. A caller can receive only their own newest
unacknowledged notice after successful authentication.

## Session flow

The common session state is explicit:

```text
Created
  -> Active / Unauthenticated
       -> ExistingCallerLogin
       -> NewCallerRegistration
       -> Authenticated(CallerId)
  -> Closed(reason)
```

New callers enter a concise flow: caller name, password, password confirmation,
only profile groups enabled by the Sysop, default security assignment, initial
time-policy check, then Main menu. Required groups must validate; optional
groups may be blank; disabled groups are never prompted. Privacy-conscious
defaults disable all four groups. Main `R` edits the caller's own enabled
private profile using the same validator. The complete `SFNEWU.QUE`
branch/question model remains Category B.
If the proposed new-caller name already exists, no duplicate is created; the
session moves into the ordinary bounded returning-caller login path.

M032 makes validation ownership explicit: each prompt validates only the value
it has collected, while the complete profile policy is validated again at the
database commit boundary. A later required field therefore cannot reject an
earlier valid field before it has been asked. Recoverable name, password,
confirmation, profile-length, email, and birth-date errors reprompt in place;
they do not end the connection. `/Q` during profile entry returns to the
caller-login boundary, and a concurrent duplicate creation does the same.

Existing login accepts a case-insensitive name and password, uses a configured
maximum of three attempts by default, rejects disabled/deleted records, and
handles disconnect/EOF without retaining the acquired node. Unknown and wrong-password
responses use one generic message. Transport identity never bypasses this
flow.

Private-board mode skips new-caller registration entirely. A successfully
verified credential still must meet the configured private-board security
level. RLogin-supplied credentials pass through the same check and never gain
authority from transport metadata.

After authentication, the current caller security level filters `.MNU`
commands. `Y`/Your Statistics reports caller name, security, times on, and
accumulated time. Main/Message/File/Help/Goodbye behavior remains the same
common session engine used by every adapter.

## Sysop initialization and authority

The synthetic fixture contains no default secret. After `init-fixture`, the
operator explicitly runs:

```text
cargo run -p sf-bbs -- init-sysop ./var/fixture-board/spitfire.toml
```

The command reads and confirms a password from the controlling terminal
without echo. It creates the configured `caller.sysop_caller_name` as a normal
caller at the configured Sysop security threshold. Unix shell ownership does
not grant Sysop status. Host/server administration remains distinct from
traditional SPITFIRE BBS authority.

## Security and time policy

`SecurityLevel` is a validated 0–9999 value. Access is `caller >= required`;
the same reusable comparison will govern message conferences and file areas.
`caller.sysop_security` is the threshold for traditional BBS authorization.

Configuration includes global per-call/per-day minutes, first-day new-caller
minutes, maximum calls per day, and optional exact-security overrides that
model the stock `MPC`/`MPD` subset of `DAILYLMT.DAT`. Session elapsed time uses
a monotonic clock. Remaining time is the minimum of per-call, daily remaining,
and first-day cap when applicable. The current engine checks expiry at menu
interaction boundaries and reports the allowance at login.

The configurable no-activity limit is applied to the transport read boundary;
timeout renders `SFASLEEP`, disconnects, accounts the session, and releases the
node. `TOOMANY` handles daily-call rejection and `SFTIMEUP` handles exhausted
daily/per-call allowance. Exact-security `MPC` and `MPD`, the global call cap,
and first-day cap all use the same board-local day. Non-time `DAILYLMT.DAT`
fields (file ratios, node-chat policy, and quick-logon behavior) remain with
their owning Category-B surfaces rather than being guessed into A-016.

## Transport-supplied credentials

RLogin auto-login is a disabled-by-default compatibility option documented in
[the dedicated SyncTERM research note](research/syncterm-rlogin-autologin.md).
When enabled, credentials extracted from the handshake are consumed once by
the same `CredentialHasher` and database path. Valid credentials establish the
same `Authenticated(CallerId)` state. Missing or invalid credentials fall back
to bounded interactive login; the ordinary RLogin identity field alone never
authenticates a caller.

## Configuration example

```toml
[board]
name = "My SPITFIRE BBS"
sysop = "Sysop"
timezone = "America/Phoenix"
access = "public"
private_security_level = 50

[caller]
sysop_caller_name = "Sysop"
new_caller_security = 10
sysop_security = 50
minutes_per_call = 60
minutes_per_day = 60
new_caller_first_day_minutes = 45
maximum_daily_calls = 10
inactivity_minutes = 3
maximum_login_attempts = 3
minimum_password_length = 10
maximum_password_length = 128

[caller.profile]
address = "disabled"
phone = "disabled"
email = "disabled"
birthday = "disabled"

[caller.password]
memory_kib = 19456
iterations = 2
parallelism = 1

[[caller.security_limits]]
security_level = 10
minutes_per_call = 45
minutes_per_day = 60

[[transports]]
type = "rlogin"
listen = "127.0.0.1:2513"
auto_login = false
```

Unknown fields, duplicate security overrides, invalid time/security/password
parameters, and incompatible transport options fail configuration validation.

## Verification and known limitations

Committed synthetic tests cover schema upgrade/idempotence, caller creation
and duplication, case-insensitive lookup, random-salted Argon2id hashing,
correct/incorrect passwords, disabled callers, registration, reconnect,
security comparisons, daily/per-call allowance, first/last call and call
counts, clean/failed session teardown, optional RLogin auto-login, and common
behavior across transport metadata. No proprietary sample or real credential
is used.

Still unresolved or deferred:

- direct `SFUSERS.DAT` import and safe legacy-credential upgrade;
- full CP437/non-ASCII caller-name normalization;
- configurable `SFNEWU.QUE` questions and other stock-advanced caller fields;
- dormant-caller `SFPCKUSR` maintenance/purge protection, legacy profile
  provenance, and brute-force throttling beyond bounded attempts; and
- physical direct-serial/modem authentication validation.

All caller-visible profile contact data is private to that caller. The
operator service can inspect/edit enabled fields without revealing credential
hashes; caller lists, node status, message headers, and unrelated caller
contexts do not include contact values. Historical city/phone/birth macros
expand only from the active caller's self-context. A future public profile
must be an explicit separate consent model.
