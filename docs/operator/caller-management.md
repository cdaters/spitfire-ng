# Caller Management

The lifecycle, subscription, purge-protection, and JOKER sections below
describe current post-0.1.0 source. The latest downloadable binary remains the
original 0.1.0 Development Preview and does not contain those additions.

## How callers are created

On a public board with `caller.allow_new_users = true`, the connection asks
whether the person is new. The board's NEWUSER display precedes registration.
The caller selects a Login and a separate public Handle, enters a password twice,
and completes only the profile fields required or offered by board policy.

Login is 1–32 ASCII letters/digits/dot/underscore/hyphen, starts with a letter or
digit, and is stored lowercase. Spaces and Unicode are rejected. Registration
reserves `sysop`, `system` and the configured Sysop's derived Login; it also
reserves the configured Sysop Handle and respects JOKER policy. Both Login and
Handle must be unique, including against disabled/deleted accounts. Handle uses
1–30 printable ASCII bytes, normalized spaces and case-insensitive uniqueness.

Use **Login** when returning through Telnet, Raw, RLogin or SSH. A Handle is not
an alternate authentication label. Existing accounts retain their stored Login;
operators can inspect it through caller management or explicitly change it with
IDENTITY. First/Last Name never supplies a login or changes an old posted author.

Ordinary validation mistakes can be corrected at the prompt. `/Q` at Login or
Handle ends registration; `/Q` in the profile returns to bounded returning-user
login. No incomplete account is published. The account, profile, Argon2id hash
and creation event commit together after required validation. The new caller
receives the configured initial access level, which must be below Sysop level
when registration is enabled. Remote registration never bootstraps a Sysop.

First Name and Last Name remain private. Enable **Require first and last name**
to collect both during registration; otherwise callers may supply them later
with Main `R`. Other profile groups accept disabled/optional/required. No new
age, address, phone or birthday requirement is imposed. Name collection is not
identity verification or permission for a public directory to reveal those names.
See [posting identity](../technical/identity-policy.md) for configured name use.

Private boards and boards with registration disabled offer returning login only.
Private-board admission also requires the current stored access threshold.

## Login and session policy

| Setting | Default / behavior |
|---|---|
| `allow_new_users` | true; private access still disables registration |
| `maximum_login_attempts` | 3; configurable 1–10 |
| `login_timeout_seconds` | 120; configurable 10–900 |
| `registration_timeout_seconds` | 600; configurable 30–3600 |
| `inactivity_minutes` | 3; independent keyboard-idle limit |
| `minimum_password_length` / `maximum_password_length` | 10 / 128 bytes; validated within 8–1024 |
| `minutes_per_call`, `minutes_per_day`, first-day/security limits | Existing time policy; an admitted call reserves its eligible allowance |

Incorrect credentials receive generic feedback and bounded increasing delays
(250 ms per attempt number). Unknown accounts perform a dummy Argon2id check.
Account availability is disclosed only after a correct password. Passwords are
not echoed, logged or recoverable from storage. Existing Argon2id parameters and
stored hashes are retained; no password-algorithm migration is required.

Only one active session may own an account, including a Sysop account. A second
login receives “This account is already logged in.” It does not disturb the first
call, consume its time, or increment successful calls. Different accounts can
use different configured nodes concurrently.

Login/registration deadlines and active-call limits use monotonic time on the
supported polling caller transports, including Telnet. Continued typing cannot
extend the absolute login/registration budget. Active input observes authorized
operator time adjustments and the established chat-pause policy. Blocking local
stdio remains checked at operation boundaries; it cannot interrupt the host's
blocking terminal read. The displayed allowance informs the caller before Main.

Successful admission alone updates last-login and call counts. Goodbye, EOF,
timeouts and transport failures release the exact owned session. Settlement
adds measured duration and releases its reservation transactionally. Stale
profile/preference saves preserve the newer row and ask the caller to retry;
operator profile edits carry the revision read with the original profile too.

## Interrupted or apparently locked sessions

Inspect `spitfire operator nodes <CONFIG>` or sfmonitor before assuming a lock is
stale. A genuinely active first call must log off or be disconnected through the
existing operator controls. Never delete caller records or edit SQLite to unlock
an account.

On board restart, the exclusive board lock proves that the previous runtime no
longer owns this board. Startup records interrupted-session recovery and releases
old claims before admitting callers. It preserves committed account/profile,
last-login and call-count changes. Uncommitted elapsed duration is not guessed;
the interrupted reservation is released. No wall-clock-age expiry can steal a
live session. If a storage error prevented normal claim cleanup, stop and restart
the board after resolving that error; the same recovery path releases it.

Operator events include admission, duplicate/lifecycle/allowance refusals,
pre-login end reasons, completion, interrupted recovery and setting conflicts.
They contain bounded identifiers/reasons, not passwords or private names.
See the [session contract](../technical/caller-sessions.md) for persistence and
D1 compatibility details.

## Private-board onboarding limitation

There is no host-side `ADD CALLER` command yet. To prepare callers for a
private board without editing SQLite:

1. Bind listeners to loopback or another tightly controlled trusted network.
2. Keep the board public only for the controlled registration window.
3. Start `spitfire console` and let the intended caller register normally.
4. Use `SECURITY <level> <name>` to assign the private-board threshold.
5. Stop the console with `QUIT`.
6. Run `spitfire config`, switch the board to private, set the threshold, and
   select `S`.
7. Restart and verify the caller before exposing the listener.

Do not open a public Internet registration window for this workaround.

## Inspect and change callers

Start the board with the operator console:

```bash
spitfire console /path/to/board/spitfire.toml
```

Examples:

```text
CALLERS
IDENTITY Example Caller|example-login|Example Handle
NAMES Example Handle|Example|Person
DISABLE Example Caller
ENABLE Example Caller
DELETE Example Caller
RESTORE Example Caller
SECURITY 20 Example Caller
PURGE PROTECT Example Caller
PURGE ALLOW Example Caller
SUBSCRIPTION 2027-08-29 Example Caller
SUBSCRIPTION PERMANENT Example Caller
PROFILE Example Caller
PROFILE-SET email Example Caller|caller@example.invalid
PROFILE-SET phone Example Caller|
```

Disabling is stock Locked Out: it prevents login, invalidates an active
session, and retains identity and data. `DELETE` creates a recoverable
tombstone; `RESTORE` returns the same stable identity. `PURGE` changes only the
preserved eligibility flag for future packing—current source has no hard
purge.

`SECURITY` changes base security. Current authorization uses derived effective
security, so an active subscription-expiry restriction remains until renewal.
Subscription dates are inclusive board-local `YYYY-MM-DD`; `PERMANENT` clears
the date and resolves an inapplicable expiry restriction transactionally.
Concurrent stale updates fail instead of overwriting the newer caller state.

`IDENTITY` changes only login identifier and Handle. Its old fourth full-name
field is rejected. Use `NAMES caller|first|last` for an atomic private name edit;
blank components clear them only when board collection policy permits it.
`PROFILE` is an explicit authorized private view. `PROFILE-SET firstname` and
`lastname` are also available for component edits.

Login values retain lowercase normalization and ASCII letters, digits, `-`, `_`,
and `.`, with an alphanumeric first character. All caller login paths use this identifier;
the Handle remains public presentation identity. Name edits never alter login. Handle/name
changes affect future posts only; old messages and queued senders remain exactly
as posted. Existing legacy full-name values remain preserved and unclassified,
not a source of first/last components. See the [identity contract](../technical/identity-policy.md).

The configured named Sysop cannot be locked out, tombstoned, made purge-
eligible, lowered below the configured Sysop threshold, or denied by an
accepted JOKER policy. Threshold privilege and configured identity remain
separate concepts.

## Caller self-service

An authenticated caller can use:

- Main `Y` for caller statistics;
- Main `#` to choose listed or unlisted public-directory status;
- Main `L` to locate visible callers by public handle;
- Main `R` to view/edit enabled private profile groups;
- Main `U` for graphics/text, dimensions, paging, hot-key, and transfer
  preferences; and
- Main `X` for session-local expert-mode menu behavior.

The operator cannot see credentials. There is no host-side password reset,
destructive delete/packing, unrestricted caller-record directory, or arbitrary
record editor in current source. The public directory is separately
board-enabled and caller-opt-in. The 0.1.0 downloadable binary predates the
bounded identity command, SSH listener, and schema-14 public-information work.

## JOKER name policy

With the board stopped, place optional policy at `SYSTEM/JOKER.DAT`. A normal
ASCII line denies that complete caller name. A line beginning `@` denies names
containing the rest of the line. Matching is ASCII case-insensitive. Empty
lines are ignored; comment syntax, wildcards, regular expressions, contact
fields, phone, address, and email matching are not supported.

The parser is bounded and fails board startup on malformed policy. A matching
caller receives the generic `LOCKOUT` presentation; neither output nor audit
reveals the rule text or supplied name. Policy is loaded as one immutable
startup generation, so edit it only while stopped and restart deliberately.
No Unicode normalization is performed. See
[Caller Access Lifecycle and Security](../sfng-caller-access.md) for the exact
policy and parser boundaries.

## Privacy rules

Profile contact values are private to that caller and the deliberate operator
profile commands. They do not appear in caller lists, node status, unrelated
sessions, or message/file presentation. Never include passwords, password
hashes, private messages, or contact data in support screenshots or public
logs.

Caller-access audit events likewise exclude passwords, caller names, supplied
denial values, JOKER rule text, subscription dates, and contact fields.

For the complete model, see
[Caller Access Lifecycle and Security](../sfng-caller-access.md) and
[Native Caller and Authentication Model](../sfng-caller-authentication.md).
For SSH mapping and host-key policy, see
[Secure SSH Caller Transport](../sfng-secure-ssh-transport.md).

## Public caller information

Schema 14 implements a board-disabled, caller-opt-in public directory. Main
`#` lets a caller change only their own listed/unlisted preference; Main `L`
locates visible public handles. Login identifier, real name, contact/profile
data, security/subscription details, and Disabled/Deleted callers are never
part of that projection. Board policy can permit listing but cannot override
an opt-out.

Use `INFO-POLICY` and versioned `INFO-POLICY-SET` in the local operator console
to inspect or change directory, last-call date, city/region, and caller Other
BBS addition policy. Use `BBS-LIST`, `BBS-ADD`, `BBS-EDIT`, `BBS-MOVE`, and
`BBS-STATE` for versioned Other BBS maintenance. Stale versions conflict;
public output never shows contributor identity.

See [Public Information](../sfng-public-information.md) for the complete
projection, authorization, audit, and recovery contract. Exact
`SFBBSLST.DAT` and `THOUGHTS.BBS` adapters remain evidence-gated.

Use the matching D2 `sfconfig` and board runtime to edit the new login/registration
settings. Earlier configuration clients retain their existing settings view;
they cannot display the new policy fields. A D2 configuration client requires a
D2 server for configuration access. Existing operator monitor features retain
their negotiated compatibility.
