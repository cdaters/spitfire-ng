# M039 Tranche 7 — B021-A Protected Operator Attachment

**Date:** 2026-09-03

**Published scope:** schema 19 and protected read-only operator attachment

**Scope:** schema 19 and protected read-only operator attachment only

**Status:** CROSS-PLATFORM COMPLETE after the 2026-09-04 B021-AW Windows
acceptance; B-021 remains PARTIAL

## Outcome

B021-A establishes a local, daemon-authoritative operator control plane over
the already verified B-017 projections. Unix hosts use a protected board-local
socket and OS peer UID, a typed allowlist, per-connection challenge, daemon
generation, protocol 1.0 negotiation, separate feature/capability checks, and
dispatch-time reauthorization. A shared client supports bounded noninteractive
status, nodes, activity/live activity, notifications, statistics, recent
callers, and maintenance queries.

Schema 19 adds only the accepted future-mutation foundation: bounded command
receipts and append-only control audit. It does not add UI, credentials,
connections, challenges, configuration copies, reports, network, door, or
scheduler state. No operator mutation is reachable through IPC.

## Synchronet secondary-reference review

A bounded secondary engineering review of official Synchronet operator-tool
documentation inspected UMONITOR status/node/action/log groupings and System
Configuration entry, plus SCFG's hierarchical categories, help, edit,
validation, and save flow. No code, text, or screen layout was copied.

- **ADOPT:** an attachable live monitor independent of daemon lifetime;
  status-first organization; capability discovery; and a clear future
  Monitor → System Configuration relationship.
- **ADAPT:** node/activity/statistics/error groupings to SPITFIRE terminology,
  privacy-safe B-017 projections, typed daemon services, and native local IPC.
- **REJECT:** direct shared-file/node-file mutation, log parsing as authority,
  arbitrary process/tool launching, printer/DOS-shell assumptions, and UI
  controls that imply authority merely because they are visible.
- **DEFER:** TUI layout/key discovery to the first `sfmonitor` MVP; SCFG-style
  configuration families to B021-C/`sfconfig`; and network, door, event,
  packing, report, and maintenance utilities to their owning milestones.

## Schema 19

`operator_command_journal` stores a bounded CommandId, daemon generation,
operator identity, semantic command family/type, SHA-256-sized fingerprint,
optional safe target generation, state/result class/version, and timestamps.
All receipts expire after 30 days and cleanup is limited to 500 rows per bounded
transaction. Identical CommandId/fingerprint is a retry; a changed fingerprint
is a conflict. B021-A read requests use RequestId and do not create journal
noise.

`operator_control_audit` stores append-only semantic authentication,
negotiation, authorization, and later command outcomes. It excludes secrets,
payloads, content, and response bodies. Ordinary successful read queries are
not durably audited; successful/failed authentication, protocol mismatch, and
read denial are. Failed unauthenticated peers are recorded as `unknown-peer`
without retaining their OS identity.

Migration tests establish preservation, empty initial history, and rollback
to intact schema 18 under injected failure. Backup/new-root restore preserves
the two durable tables while a restored runtime creates a new generation.

## Transport and security

The Unix endpoint is normally the Work-directory `sfop.sock`; parent/socket
modes are 0700/0600. A board-specific SHA-256 name in an owner-only short
runtime directory is used when a deep board root would exceed the native
Unix-socket pathname limit. Owned parents are tightened to mode 0700. Active
endpoints, wrong ownership, symlinks, and non-socket path collisions fail.
Stale sockets are removed only after type and connection checks. There is no
TCP fallback.

Setup records the board creator's stable UID and explicit read capabilities.
An empty allowlist on an older board bootstraps only the board-root owner.
The server uses platform peer credentials, not display names or BBS security
levels. Named/threshold Sysop status never grants local attachment.

The separately completed B021-AW pass adds the protected named-pipe listener,
explicit DACL, verified peer-token SID, Windows setup bootstrap, and native
Windows acceptance beneath the same client/protocol/service abstraction. See
[B021-AW Windows Operator Attachment](m039-tranche-7-b021aw-windows-operator-attachment.md)
for the endpoint, ACL, lifecycle, and acceptance evidence.

## Protocol and projections

Length-delimited typed frames are capped at 1 MiB. Protocol major mismatch,
unsupported feature, malformed/oversized input, authentication failure,
authorization denial, deadline, and stale generation have stable safe error
classes. A one-use challenge binds the peer, session ID, negotiation, and
daemon generation; replay on another connection fails.

The IPC delegates through `OperatorService` to B-017 for:

- board status;
- node list and one node lookup;
- recent events (typed filters, 100-row default, 500-row maximum, 31-day
  detail window, and transported stable snapshot cursor);
- connection-lived bounded subscription with explicit gap recovery and
  cancellation;
- notification list/detail data (read-only);
- system/today statistics;
- recent callers; and
- maintenance status.

The operator client library is the backend intended for the next read-only
`sfmonitor` MVP. It contains no TUI logic and leaves typed configuration
families available for later negotiation without defining false capabilities.

## CLI and foreground console

The implemented command family is:

```text
spitfire operator status <CONFIG-FILE>
spitfire operator nodes <CONFIG-FILE>
spitfire operator events <CONFIG-FILE>
spitfire operator watch-events <CONFIG-FILE>
spitfire operator notifications <CONFIG-FILE>
spitfire operator statistics <CONFIG-FILE>
spitfire operator callers <CONFIG-FILE>
spitfire operator maintenance <CONFIG-FILE>
```

Each command resolves exactly the named board configuration and endpoint,
authenticates, negotiates, makes one bounded request, prints localized
human-readable output, and returns failure on a safe protocol/client error.
The embedded en-US operator catalog advances from 1.9.0/613 messages to
1.10.0/633 messages. B021-AW advances it to 1.11.0/636 messages for safe
Windows endpoint, peer-identity, and pipe-security failures; presentation
resource versions do not change.
The foreground console retains its historical interactive actions but now
shares its daemon-owned `OperatorService` read facade with IPC and future
clients. No existing action was added to IPC.

## Acceptance and remaining work

Automated coverage includes migration/rollback, receipt retry/conflict and
retention, Unix permissions/path substitution, oversized frame, protocol
mismatch, challenge replay, policy revocation, concurrent clients, B-017
projection transport, persistent subscription cancellation, slow-subscriber
overflow/gap recovery, daemon generation change, privacy
deny-list checks, and backup migration/restore regressions. Network/socket
tests require the normal unsandboxed test environment on this host.

A separate-process disposable-board journey started the daemon independently,
attached with each CLI view, negotiated all nine read features, and showed two
simultaneous real callers on distinct nodes by public handle. A caller then
posted a real public message; recent activity reported the typed event without
its body or recipients. Operator-client exits did not affect either caller or
the daemon, both callers logged off cleanly, the daemon completed both
sessions, and a restarted daemon accepted a fresh attachment. An invalid board
target failed closed with a localized nonzero result. Automated coverage proves
the old generation/challenge cannot be reused.

The original macOS cross-target attempt stopped in bundled C dependencies and
was never treated as Windows evidence. B021-AW instead ran the source and its
security-token/named-pipe tests on native Windows. That result closes the
earlier portability limitation.

The complete implementation matrix passed workspace tests on Unix/macOS and
native Windows, with the two established manual interoperability servers
remaining intentionally ignored. Formatting, Clippy with warnings denied,
source headers, diff hygiene, Markdown/local links, and balanced fences pass.
Focused privacy/provenance checks found no local path, disposable identity,
credential, key, or secret leakage. Advisory coverage is reported separately
when `cargo-audit` is available.

B021-A is cross-platform complete, while B-021 remains PARTIAL. Remaining work
is:

1. build the separately authorized read-only `sfmonitor` MVP over this client;
2. B021-B stale-safe page/chat/time/disconnect/shutdown commands and command
   receipt/audit execution;
3. B021-C typed configuration/CAS plus the first `sfconfig` MVP; and
4. B021-D integrated cross-platform, accessibility, documentation, and real
   operator/caller acceptance.

B-022 remains NOT STARTED. No reports/publication, networking, door,
scheduler, platform-presentation, or release capability was introduced.
