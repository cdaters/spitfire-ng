# M039 Tranche 7 — B021-AW Windows Operator Attachment

**Date:** 2026-09-04

**Published scope:** protected Windows attachment and cross-platform B021-A
closure

**Scope:** protected Windows operator attachment and cross-platform B021-A
closure only

**Status:** VERIFIED ON NATIVE WINDOWS; B021-A CLOSED; B-021 remains PARTIAL

## Outcome

B021-AW implements the Windows transport beneath the same read-only
`OperatorClient`, protocol, authorization, and `OperatorService` used on Unix.
It does not add a Windows protocol or a second implementation of board status.
Schema remains 19 and no operator mutation is exposed.

A GitHub-hosted `windows-latest` runner performs native Windows Rust tests and
a separate-process acceptance journey on the `x86_64-pc-windows-msvc` target.
Windows ARM is not claimed. The native setup test proves the same `setup_board`
path used by interactive setup records the creator SID. The journey creates a
disposable fixture board, verifies SID bootstrap, starts the daemon, attaches
through the ordinary `spitfire operator` CLI, exercises every B021-A read
projection, observes two caller connections and two concurrent event
subscribers, proves an unlisted Windows identity is denied, and restarts and
reattaches to the daemon. This is real Windows named-pipe and security-token
execution, not mocked or cross-compiled evidence.

## Endpoint naming and lifecycle

The endpoint is:

```text
\\.\pipe\spitfire-ng-operator-<board-id>
```

`board-id` is the first 32 lowercase hexadecimal characters of a domain-
separated SHA-256 digest of the canonical UTF-16LE board configuration path.
The result is bounded, contains no raw user-controlled path or secret, and is
deterministic for one board while separating boards on the same host. Moving
the board changes the endpoint rather than causing broad endpoint discovery.

The daemon creates the first instance with first-instance protection, rejects
remote clients, and permits at most 32 simultaneous instances. Each accepted
connection receives its own bounded task. A slow or crashed client cannot
block later attachments or the B-017 producer, and shutdown cancels the accept
loop. Windows kernel object lifetime replaces Unix stale-socket-file cleanup;
an absent daemon has no stale filesystem object to delete.

## ACL and SID authentication

The listener uses an explicit protected DACL. It grants generic-all access to
the daemon process SID and grants each configured operator SID only the exact
named-pipe client rights needed to read, write, synchronize, and inspect the
pipe. The client mask deliberately excludes `FILE_CREATE_PIPE_INSTANCE`.
`Everyone`, `Anonymous`, and broad `Authenticated Users` entries are absent.
Pipe creation fails closed when the security descriptor cannot be parsed or
applied.

The daemon process SID and authorized operator SIDs are independent, so the
design supports a future dedicated service account without requiring the
human operator to share that identity. Administrator membership, elevation,
and LocalSystem are not implicit SPITFIRE authority; an identity is an
operator only when its stable SID is configured. No fallback key is used on
normal Windows named pipes.

After reading the initial hello on a connected pipe, the server briefly calls
`ImpersonateNamedPipeClient`, opens the thread token, reads `TokenUser`,
converts and canonicalizes the SID, and immediately calls `RevertToSelf`.
RAII guards guarantee reversion and token-handle closure on every return path.
No database, filesystem, network, or domain operation occurs while
impersonating. Application protocol fields cannot assert an identity.

New Windows setup and fixture boards record the creating process user's
canonical SID with the six B021-A read capabilities. SID parsing rejects
malformed and noncanonical strings, entries are deduplicated, and Unix UID
entries remain independent. An older board without an allowed Windows SID
fails closed with enrollment guidance; the first connecting account is never
silently trusted.

## Authorization and protocol reuse

The accepted one-use challenge remains bound to peer identity, operator
session, daemon generation, and negotiated protocol/features. Protocol 1.0
continues to use a four-byte big-endian length followed by a closed typed JSON
message, with a 1 MiB frame maximum, bounded deadlines, explicit safe errors,
and no TCP fallback.

Pipe ACL and application authorization are separate layers. The daemon
reloads the board-local allowlist at every dispatch, so removing an identity
or capability rejects the next request even on an established connection.
The listener DACL is a startup snapshot: removing an entry is immediately
effective in the application layer, while adding a previously absent SID
requires listener recreation or daemon restart before the ACL admits a new
connection.

The Windows client transports the same board status, node list/detail, recent
and live events, notification list/detail, statistics, recent callers, and
maintenance status projections. The same privacy-safe B-017 response types
cross both transports. No internal caller or database object is serialized.

## Verification

Native Windows tests cover board-specific names, protected DACL construction,
canonical and malformed SIDs, setup bootstrap, all read projections, two
simultaneous clients, dispatch-time revocation, protocol-major mismatch,
unsupported features, malformed/oversized frames, challenge replay, stale daemon
generation, and abrupt disconnect. The PowerShell journey additionally
verifies fixture bootstrap, the common CLI, two real TCP caller connections, two live subscribers, a
separately created unauthorized local account, daemon survival after an
operator-client kill, and restart/reattach. The test account and disposable
board are removed afterward; logs do not print personal SIDs or passwords.

The Windows workflow also runs formatting, the full workspace suite, and
Clippy with warnings denied. The macOS quality run preserves the accepted Unix
socket permissions, attach/negotiation, CLI reads, subscription, detach, and
restart behavior. The existing foreground console and schema-19 cold backup /
new-root restore tests remain green. Live Windows Service packaging is not
implemented, but separating daemon and operator SIDs keeps the transport
compatible with a later service-account acceptance pass.

The accepted native run includes three focused Windows control tests plus the
complete public-safe workspace suite. The two established manual
interoperability servers remain intentionally ignored.

The first full Windows suites exposed two older backup portability defects:
SQLite snapshot durability reopened the completed file read-only before
`sync_all`, while Windows `FlushFileBuffers` requires a write-capable handle;
and a validation connection remained open when the staging directory was
published, while Windows correctly refuses to rename that open tree. The
snapshot now reopens read/write for durability and the validation connection
closes before publication. All 17 affected backup tests then pass natively;
snapshot contents, authority, and schema remain unchanged.

The full suite also exposed one Unix-shaped test fixture that treated
`/fixture-root` as an absolute path on every host. It now uses a temporary
host-native absolute root; runtime path resolution did not change.

The Windows suite exposed an older two-node TCP download test that drained
nominally concurrent clients serially. An unread Windows socket could apply
backpressure to the other session indefinitely. Its two readers now run
concurrently with bounded timeouts. The runtime transfer path did not change;
the corrected test continues to prove two completed downloads and their
accounting/events.

That bounded rerun advanced through the corrected test and exposed a second
pre-existing Windows distinction in the duplicate-upload race. Windows may
report an already-created destination that another thread still has open as a
sharing/access error rather than `AlreadyExists`. The publication boundary
now classifies any failed exclusive create whose confined destination exists
as the same semantic duplicate-filename result. The one-winner rule and
filesystem confinement are unchanged; no raw Windows error number enters the
core model.

## Scope boundary and next action

B021-A is now cross-platform complete. B-021 remains PARTIAL because B021-B
live controls, B021-C typed configuration, and B021-D integrated maintenance
and platform acceptance remain. B-022 remains NOT STARTED.

The exact next separately authorized action is the first read-only `sfmonitor`
MVP over the now-portable `OperatorClient`. It must not add live mutations;
B021-B remains a later explicit slice. No `sfconfig`, networking, doors,
scheduler, report publication, release, tag, or binary capability was
introduced in B021-AW.
