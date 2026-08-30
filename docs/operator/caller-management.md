# Caller Management

The lifecycle, subscription, purge-protection, and JOKER sections below
describe current post-0.1.0 source. The latest downloadable binary remains the
original 0.1.0 Development Preview and does not contain those additions.

## How callers are created

On a public board, a terminal connection asks whether the person is a new
caller. Registration collects:

1. a unique case-insensitive caller name of at most 30 printable ASCII bytes;
2. a password within the configured length range, entered twice;
3. only the profile groups enabled by Sysop policy; and
4. the configured new-caller security/time policy.

Optional profile fields may be blank. Required fields must validate.
`/Q` at a profile prompt cancels the incomplete registration. Passwords are
stored only as salted Argon2id PHC hashes.

Schema 13 stores that initial caller name as the public display handle and
compatibility real name, and derives a unique SSH-safe login identifier once.
This preserves the familiar prompt count and gives every caller a durable
secure-transport identity without making real name public.

On a private board, new-caller registration is intentionally absent. Only an
existing active caller whose verified account meets the private security
threshold is admitted.

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
IDENTITY Example Caller|example-login|Example Handle|Example Real Name
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

`IDENTITY` separates the current caller into login identifier, public display
handle, and optional private real name. Use a blank final field to clear real
name. Login values are normalized lowercase and accept only ASCII letters,
digits, `-`, `_`, and `.` with an alphanumeric first character. Login and
handle collisions are rejected; stable caller/message ownership and existing
attribution snapshots are preserved. SSH uses the login identifier,
traditional BBS login/presentation uses the handle, and real name remains
private unless an explicit future network policy requires it.

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
