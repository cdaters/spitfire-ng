# Protected Operator Attachment

<!-- help-topic: operator.security -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)

Schema 19 and B021-A provide the first protected control-plane slice for a
running board. Attachment defaults remain read-only; B021-B adds explicit live
controls below. The `spitfire operator` client,
the foreground console, `sfmonitor`, and `sfconfig` clients share the
daemon-owned `OperatorService`; none reads SQLite, transient status files, or
diagnostic logs as operational authority.

## Durable state

Migration 18→19 is transactional and adds only:

- `operator_command_journal`, a bounded 30-day retry receipt for
  state-changing commands; and
- `operator_control_audit`, append-only semantic security accountability for
  authentication, protocol, authorization, and later privileged actions.

Both start empty. Migration does not synthesize commands or audit and does not
alter schema-18 caller, message, file, transfer, storage, observability, or
security-audit authority. Cold backup preserves both tables. Restore never
preserves live connections, challenges, subscriptions, endpoint files, or a
running daemon generation.

## Local endpoint and identity

On Unix-like systems the daemon normally binds `sfop.sock` inside the
configured board-private Work directory. If that pathname exceeds the native
Unix-socket limit, it derives a board-specific socket name under an
owner-specific `/tmp/spitfire-ng-operator-<uid>` directory instead. Either
parent is a real directory owned by the board owner and is tightened to mode
`0700`; the socket is mode `0600`. Startup rejects symlinks, wrong ownership,
non-socket collisions, and a live pre-existing endpoint; only a stale socket
is removed. The SHA-256-derived short name identifies the board without
exposing its path. There is no TCP operator listener.

The daemon obtains the peer UID from the Unix socket and maps it to the typed
`operators.local_identities` board configuration. New setup and fixture boards
record the creating board owner's UID with the six B021-A read capabilities.
For an existing board whose list is empty, ownership of the board root is the
bootstrap identity with exactly the same six read-only capabilities, never
mutation authority. The policy is reloaded and authorization is checked again
for each request, so removing an identity or capability affects an attached
client's next dispatch.

On Windows the endpoint is
`\\.\pipe\spitfire-ng-operator-<board-id>`, where `board-id` is the first 32
lowercase hexadecimal characters of a domain-separated SHA-256 digest of the
canonical UTF-16LE configuration path. It is deterministic per board without
exposing the path or a secret. The daemon creates at most 32 local-only pipe
instances with first-instance protection and an explicit protected DACL.

The DACL grants generic-all access to the daemon process SID and exact client
read/write/synchronize/metadata rights to configured operator SIDs, excluding
pipe-instance creation. It contains no `Everyone`, `Anonymous`, or broad
`Authenticated Users` grant. Administrator membership, elevation, and
LocalSystem are not implicit operator authorization. A daemon service SID and
human operator SID may differ.

The server briefly impersonates the connected named-pipe client, opens its
thread token, reads `TokenUser`, canonicalizes the SID, and reverts under RAII
before it performs other work. The application protocol never supplies peer
identity. Setup records the creating Windows SID; an older board with no
Windows SID fails closed rather than trusting its first connection. No
fallback key is used on normal Windows systems.

The board policy is reloaded at each dispatch. Removing a SID or capability
therefore denies the next request on an existing connection. The listener
DACL is fixed when it is created, so adding a previously absent SID requires
listener recreation or daemon restart before a new connection can pass both
layers. Windows kernel object lifetime means there is no Unix-style stale
socket file to remove.

## Protocol

Protocol 1.0 uses a four-byte big-endian length followed by a typed JSON
message. JSON is an encoding, not a generic RPC surface: every request and
response is a closed enum. Frames are limited to 1 MiB, requested features to
32, ordinary deadlines to five seconds, and accepted deadlines to 30 seconds.
Zero, oversized, truncated, malformed, wrongly sequenced, and unsupported
messages fail explicitly without exposing Rust errors or backtraces.

The sequence is:

1. client hello with major/minor and requested features;
2. server hello with negotiated minor/features, schema, opaque daemon
   generation, one-use challenge, and operator-session ID;
3. challenge echo bound to the authenticated connection and generation; and
4. typed read requests carrying the session ID, generation, request ID, and
   deadline.

A major mismatch fails. A compatible minor negotiates the common feature set.
Feature support and operator authorization remain separate checks. Daemon
restart changes the generation and requires a fresh connection, peer check,
negotiation, and challenge.

Ordinary successful reads do not create durable audit noise. Authentication,
protocol incompatibility/violation, and sensitive read denials create semantic
control-audit rows without payloads, responses, or secrets. An unauthenticated
peer is recorded only as `unknown-peer`; its OS identity is not retained.

## Read projections

The transport delegates to the existing schema-18 services for board status,
node list/detail, recent operational events, live events, notifications,
statistics, recent callers, and maintenance status. Recent-event queries keep
their typed filters, 100 default, 500 maximum, 31-day detail window, stable
ordering, and snapshot cursor across the protocol. The live operation uses the
existing 2,048-event ring and a connection-lived bounded 256-event subscriber
queue. Cancellation drops that ephemeral subscription. A gap is explicit; the
client recovers through durable recent events.

Only privacy-safe projection types cross the endpoint. They exclude login
identifiers, private real names/contact fields, credentials, message bodies
and recipients, terminal input, endpoints, host paths, file contents, keys,
raw packets, and future network secrets.

## Failure and lifecycle

Client timeout, crash, or disconnect drops only connection-local state. It
does not stop the daemon or callers and leaves no durable IPC session. Multiple
read clients may attach concurrently. The initial CLI returns a nonzero
process status for endpoint, authentication, authorization, compatibility,
timeout, and safe internal failures.

B021-A exposes no page, chat, time grant, disconnect, shutdown, notification
acknowledgement, configuration, backup, maintenance, or arbitrary host action.
Current B021-B adds the explicitly authorized controls described below;
configuration and maintenance mutation remain outside this implementation.

The completed B021-B implementation follows the binding gate.
Protocol major 1 remains; supporting minor versions expose typed control
discovery, mutations, and principal-bound command-result lookup. Live session
targets require daemon generation, NodeId, SessionId, and an ephemeral node-
occupancy generation at domain dispatch. CommandId plus a canonical SHA-256
request fingerprint uses the existing 30-day schema-19 receipt journal to
make retries safe and distinguish missing, in-progress, completed, and
conflicting requests. Time adjustment, disconnect, and shutdown also require
short-lived server preflight tokens bound to the operator, CommandId, target,
generation, parameters, and current impact.

The only gated B021-B controls are page availability/page-chat, signed
−120..+120-minute session-only adjustment, graceful disconnect with explicit
notice choice, expected-version notification acknowledgement, and graceful
daemon shutdown. They require separate capabilities and durable semantic
control audit. Chat content remains non-recorded. Restart, host control,
configuration, maintenance, and observation remain excluded. See the
[B021-B Live Operator Controls Gate](../research/m039-tranche-7-b021b-live-operator-controls-gate.md).

The original `sfmonitor` 0.1 MVP used only the nine read features. Current
source adds capability-aware Actions over the same client, and System
Configuration launches the separate B021-C sfconfig. See
[sfmonitor Technical Architecture](sfmonitor.md).

Previously accepted native B021-A Windows tests exercise SID parsing, DACL construction,
multi-client attachment, every read projection, dispatch-time revocation,
malformed/oversized frames, protocol mismatch, replay, stale generation, and
disconnect. A separate-process Windows journey additionally proves setup,
ordinary CLI use, two caller connections, two live subscribers, denial of a
disposable unlisted local account, and restart/reattach. The same
`OperatorClient` and protocol are used on Windows and Unix.

This evidence does not establish live Windows B021-B acceptance. Mutation,
chat/TUI, disconnect, transfer cancellation, and shutdown acceptance remain
DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED.

See the [B021-A implementation record](../research/m039-tranche-7-b021a-protected-operator-attachment.md)
and [B021-AW Windows acceptance](../research/m039-tranche-7-b021aw-windows-operator-attachment.md).
## B021-B1 mutation boundary

### Explicit grants and bounded profiles

Every B021-B mutation requires an explicit `operators.local_identities`
capability grant. Creation/ownership, empty-list Unix bootstrap, omitted
capability defaults, Sysop names/security levels, or OS Administrator status
do not authorize mutations. The six defaults are `board-statistics`,
`node-status`, `operational-events`, `caller-activity`, `notifications`, and
`maintenance-status`. Older Windows boards still require explicit SID setup.

`MAX_LOCAL_OPERATOR_CAPABILITIES` is 32, reusing the established discovery
capability-list ceiling for configuration validation and client decoding.
Only unique recognized entries count as grants; empty, duplicate, unknown,
malformed, or oversized lists fail closed. This replaces the incorrect
six-entry profile ceiling and represents all 16 implemented capabilities:
six monitor reads, seven B1/B2/B3 controls, and three B021-C configuration grants.
The shared explicit vocabulary is not a grant-all preset. Remaining capacity grants nothing.

Existing read-only profiles are unchanged. Empty-list boards lose the
previously unintended implicit B1 grant; enroll desired controls explicitly
using the [Sysop Manual](../manual/sfmonitor.md#explicit-mutation-enrollment).
sfmonitor refreshes discovery with its snapshot, keeps unsupported and
unauthorized actions distinct, and disables action keys without grants.
Revocation between refresh and dispatch is still denied by the daemon.

### D-064 compatible discovery (protocol 1.2)

The current client sends only the nine established read features in its hello.
The server negotiates the common minor. After authentication, minor 1.2 peers
use `DescribeOperatorControls` to obtain bounded typed control features,
current capabilities, preflight/confirmation metadata, command bounds, and
receipt-lookup support. Discovery describes support and current policy;
mutation dispatch still reauthorizes against current configuration.

Minor 1.2 distinguishes discovery support from the earlier 1.1
implementation, which has mutation envelopes but no discovery operation.
Corrected clients retain baseline read access to 1.0 and uncorrected 1.1
daemons and report mutation support unavailable. Corrected daemons still
accept existing 1.1 clients' B1 feature negotiation. No new control family or
mutation semantics accompany this compatibility correction.

Schema 19's command journal and control audit now back two typed local
operator mutations: notification acknowledgement and bounded current-session
time adjustment. Command IDs are client-generated and replay-safe only for the
same authenticated operator, daemon generation, target, and SHA-256 semantic
fingerprint. Node targets include an ephemeral occupancy generation so node
slot reuse is stale-safe. Receipt lookup is principal/generation scoped.

Session adjustment is -120..=120 minutes per command, rejects zero, and is
preceded by a typed target-bound preflight token. It is
relative to the live session's original policy allowance; it never edits
durable caller policy or historical accounting. sfmonitor's `A` Actions menu
retains acknowledgement and +/-5-minute presets alongside B2's controls below.
Configuration and maintenance execution remain unavailable; B3 adds shutdown below.

## B021-B2 page/chat and disconnect (protocol 1.3)

The [canonical B2 report](../research/m039-tranche-7-b021b2-chat-disconnect.md#b021-b2-implementation)
defines implementation details, bounds, tests, and native evidence. Minor 1.3
adds the page-availability, caller-pages, caller-chat, and session-disconnect
features through D-064 discovery. A 1.2 peer sees only the B1 feature/capability
vocabulary; no unfamiliar closed-enum values are sent in the initial hello.

`LiveControl` carries an existing CommandId plus a typed action: availability,
page answer/decline, invitation, disconnect preflight, or confirmed disconnect.
Session actions bind daemon/NodeId/SessionId/occupancy generation; page disposition
also binds the pending interaction ID. OperatorService dispatches into existing
InteractionHub and NodeManager authority. Explicit capabilities are separate
from feature support and are rechecked at dispatch. Bootstrap remains read-only.

The chat response carries a one-time, principal-bound handoff token. A fresh
authenticated OperatorClient redeems it to enter bounded framed line chat.
This is not a raw terminal stream. Lines are at most 512 UTF-8 bytes, channels
hold 32 messages, responses drain at most 16 lines, and complete frames have a
five-second deadline. The original owner attachment and stream must stay alive;
sfmonitor maintains both. Consent, exact interaction ID, and current policy are
checked before active chat. Accepted operator invitations pause ordinary caller
allowance through a single scope-owned guard, not factual accounting. End/loss
returns a connected caller to its previous menu context. No automatic resumption
or transcript recovery exists.

Chat content never enters the command journal/fingerprint, control audit,
operational events, ordinary logs, SQLite messages, or diagnostics. The monitor's
100-line current-chat buffer is ephemeral and redacted from Debug output.

Disconnect preflight is runtime-only, 30-second, bounded, and tied to attachment,
CommandId, exact target, notice choice, node/transfer state, and interaction ID.
Confirmation requires current matching impact. A single cooperative request
ends chat, cancels the active transfer through its existing protocol/finalizer,
settles completed work and caller accounting, releases the node, and closes
transport. No-notice skips only the caller notice. After three seconds the
daemon revalidates the complete target before an owned emergency TCP/SSH close;
it allows at most five further seconds for normal finalization. Unsupported
hardware close handles are never replaced with global/path-based operations.
Final results distinguish completion, fallback, stale target, and failure to
finalize. The durable receipt, not monitor state or socket closure, is authority.

CommandId/fingerprint/principal conflicts fail closed; replay precedes effects.
Concurrent operators serialize only scoped target/interaction transitions.
Lost replies recover the same receipt without duplicate invitations/disconnects.
Both mutation and transition evidence use schema-19-valid checked control audit;
B-017 receives only safe Operator-category metadata, never chat text.

## B021-B3 graceful shutdown (protocol 1.4)

Authenticated discovery adds `graceful-shutdown` with the independent explicit
`request-graceful-shutdown` capability. It never appears in the initial hello;
1.2/1.3 discovery cannot receive its closed-enum values. `ShutdownStatus` exposes
safe aggregate impact to authorized board-statistics readers without granting
shutdown permission.

`PrepareGracefulShutdown` / `RequestGracefulShutdown` extend the same LiveControl
envelope, fingerprint, and journal. Runtime-only preflights bind attachment,
CommandId, generation, and current exact consequences for 30 seconds, with at
most 128 entries. Same-identity replay precedes effects; distinct duplicate
requests receive `shutdown-already-requested`.

The daemon runtime owns admission serialization and drain, independent of client
lifetime. Its existing console/signal stop path shares the same drain. Cooperative
session/interaction/transfer cancellation receives a distinct board notice and
retains accounting/transfer ownership. Three seconds precede exact-session owned
fallback; at most six more seconds cover finalizers and outstanding B2
receipt/chat-end evidence. Failure to prove safe completion leaves admissions
closed and reports failure without process kill.

The durable command result is `shutdown-requested`, committed before irreversible
listener shutdown. Final correlated audit and the safe `shutdown-complete`
operational event precede normal daemon exit. Existing SQLite commits preserve
authority; no new schema, durable shutdown table, service-manager state, or
post-exit write is assumed. Queries work only while the same generation remains
available. Restart is external; an old shutdown cannot be replayed onto it.

See the [B3/integrated report](../research/m039-tranche-7-b021b3-shutdown-integrated.md)
for exact bounds, races, failure boundaries, native evidence, and Windows deferrals.

## B021-C typed configuration (protocol 1.5)

The authenticated minor-gated `configuration` feature adds typed snapshots and
ApplyConfiguration over the existing session/generation/CommandId boundary.
ReadConfiguration, ChangeOnlineConfiguration, and ChangeSensitiveConfiguration
are independent explicit grants; the six-read bootstrap is unchanged. Older
clients receive only their negotiated vocabulary. The daemon serializes CAS,
validation, atomic file replacement, session-policy publication, and schema-19
receipt/audit recovery. No raw configuration editor or direct client database
access exists. See [Configuration Authority](configuration.md) and the
[sfconfig manual](../manual/sfconfig.md).

## B021-D maintenance and recovery boundary

`MaintenanceService::ALL` is a closed three-owner navigation descriptor registry
for B-015 files, B-017 retention, and native cold backup/recovery. It supplies
localized routes/help topics to sfmonitor and sfconfig; it has no invocation,
wire mutation, capability grant, or job state. Live metrics remain the existing
MaintenanceStatus read over OperatorService. No protocol increment is required
for local presentation metadata, and no new enum is sent to older peers.

The gate's maintenance execution contracts remain conditional on an approved
operation being exposed. No additional attached execution is necessary for the
stock B-021 outcome: file/retention services retain their existing owners, and
backup/restore retain explicit exclusive cold-board commands. Acknowledgement
only records attention. B-018 pack/purge and B-022 output remain separate.

The foreground console retains its accepted daemon-owning caller/security
administration and domain audit; it cannot attach as a second board owner.
Those established B-016 services are not duplicated as new monitor controls.
Host-local startup/offline administration and enrolled protected IPC are distinct
accepted entry boundaries. Bootstrap and every IPC mutation retain their existing
read-only/explicit-enrollment policy. See [Operator Startup and Recovery](../manual/operator-recovery.md).
