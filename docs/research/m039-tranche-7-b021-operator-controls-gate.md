# M039 Tranche 7 — B-021 Local/Sysop Operator Controls Gate

Status: **GATE COMPLETE; B021-A SUBSEQUENTLY CLOSED; B-021 REMAINS PARTIAL**

Canonical row: **B-021 PARTIAL — Local/Sysop operator controls**

This gate defined the protected local control plane needed to complete B-021
and support `sfmonitor`, `sfconfig`, and noninteractive CLI clients. The later
B021-A implementation now supplies schema 19 and cross-platform protected
read-only attachment. Live controls, the TUI clients, B-022 reports,
scheduler, doors, and networking remain outside that completed slice.

## 1. Scope and authority

The running SPITFIRE NG daemon remains the only online authority. Operator
clients submit typed requests to it; they do not open SQLite, rewrite TOML,
tail text logs as state, inspect internal session objects, signal worker
threads, or own a second copy of board state.

The authority order used by this gate is:

1. original SPITFIRE 3.7 documentation and runtime evidence;
2. official Buffalo Creek utility documentation;
3. accepted NG domain, security, observability, and recovery contracts; and
4. Synchronet and NodelistDB only as secondary engineering references.

The exact parity-ledger baseline is:

- B-017 **VERIFIED** — Sysop logs, daily statistics, notifications,
  maintenance views;
- B-021 **PARTIAL** — Local/Sysop operator controls; and
- B-022 **NOT STARTED** — Screen/export/print-oriented operations.

B-017 supplies read projections and narrow notification acknowledgement.
B-021 supplies protected attachment and live control. B-022 will later use
both without making generated output authoritative.

## 2. Historical SPITFIRE findings

`SPITFIRE.DOC` §8 classifies controls by where they run: online-only,
ready-prompt-only, or both. The important outcomes are:

| Historical control | Confirmed outcome | NG treatment |
|---|---|---|
| F1 | Ready-only activity, notification, questionnaire, request, and purge-log views by today, beginning, or specified date | B-017 structured bounded views; never arbitrary file browsing |
| F2 | Change the online caller's security without exposing the editing screen to the caller | Existing caller authority, later reached through a distinct sensitive capability and expected caller version |
| F3 / caller Page | Toggle Sysop availability, notify the operator, answer or decline caller-initiated chat | Existing page/chat domain, attached through B-021 |
| F5 | Ready-only calls, uploads, downloads, doors, new callers, and messages for today | B-017 Today statistics |
| F6 / F7 | Subtract or add five minutes, repeatable in five-minute steps | Typed bounded session allowance adjustment; the legacy five-minute keys become presets |
| F8 | Full/divided local screen with caller status | Privacy-safe B-017 node detail; no private profile or terminal capture |
| F9 | Capture chat to disk/printer | Not B-021; recording requires a separate consent, privacy, and retention gate |
| F10 | Terminate SPITFIRE and return to DOS | Typed SPITFIRE NG graceful shutdown; never host shutdown or shell access |
| Alt+F1 / Alt+F2 | Disconnect online caller with or without a display notice | One graceful disconnect command with explicit notice policy and an emergency fallback |
| Alt+A / Sysop menu | Maintain caller records | Existing caller services; broad pack/purge remains B-018 |
| Alt+C | Answer a page or force local Sysop chat; caller time paused for operator-initiated chat | Reuse the current chat domain; operator initiation and explicit time policy remain to implement |
| Alt+L | Lock out the current caller and optionally disconnect | Existing lifecycle authority plus a separately confirmed disconnect |
| Alt+M/Z/P/R/F/E | Ready-only modem, board, paths, conferences, file areas, and events configuration | `sfconfig` typed areas, classified by online/restart/maintenance/offline effect |
| Alt+D/J and Sysop-menu Drop to DOS | Expose DOS while on/off hook | Historical only; no host shell primitive |
| Home | Contextual operator help | Stable localized help topics and action metadata |

The split-screen example displayed private name, phone, birth date, location,
security, and counters. That is historical evidence, not the modern privacy
default. `SFSYSOP.MNU` confirms caller database, message/file configuration,
events, log viewing, local logoff, and DOS entry as stock Sysop-menu concepts.
SFSYSTEM confirms that board configuration and counters could be changed by a
companion program or scheduled event, but also warns against unsafe concurrent
execution. NG preserves the outcomes through daemon-owned services rather
than DOS file mutation.

The exact visual keys, prompt bytes, forced-chat timing, and emergency
disconnect timing remain compatibility details. They do not block the native
B-021 interface.

## 3. Existing implementation and remaining gap

The current `OperatorService` and foreground `spitfire console` already expose:

- node status;
- caller-initiated page listing, availability, answer, decline, and chat;
- SessionId-targeted disconnect;
- caller list and private profile views;
- identity/profile/subscription/purge-protection/security edits;
- Active/Disabled/Deleted caller lifecycle transitions;
- public-information policy and Other BBS maintenance; and
- console exit that cleanly shuts down the foreground-owned board.

The current domain already has stable CallerId, SessionId, NodeId, optimistic
caller/resource versions, named-Sysop protection, semantic domain audits,
schema-18 observability, and cold backup ownership.

The missing B-021 work is:

- attach to a separately running daemon;
- authenticate a local host operator and construct a capability context;
- reauthorize every request at dispatch;
- expose B-017 queries over a bounded protocol;
- add stale-safe operator-initiated chat, time adjustment, notice/disconnect,
  and graceful shutdown;
- make the foreground console and attach CLI consume the same service;
- add durable command receipts and security audit;
- define typed configuration discovery/version/conflict services for the
  first `sfconfig` slice; and
- pass real multinode, multi-operator, recovery, privacy, localization, and
  client acceptance.

## 4. Product roles and information architecture

### `spitfire setup`

`spitfire setup` remains the short offline bootstrap: create the board,
directories, schema, first named Sysop, minimum configuration, and recovery
bootstrap. It does not become the comprehensive editor.

### `sfmonitor`

`sfmonitor` is the primary live operator cockpit:

1. Dashboard
2. Nodes and Callers
3. Transfers
4. Activity
5. Statistics
6. Notifications
7. Errors and Maintenance
8. Operator Actions
9. Reports — future B-022
10. Networks — future
11. Doors — future
12. Events and Jobs — future
13. System Configuration → `sfconfig`

The labels retain SPITFIRE/Sysop terminology. The monitor does not become a
generic infrastructure dashboard.

### `sfconfig`

`sfconfig` is the comprehensive expert configuration environment:

1. Board and Nodes
2. Security and Callers
3. Conferences and Messages
4. File Areas, Transfers, and Storage
5. Presentation and Localization
6. Networks — future QWK, DOVE-Net, and FidoNet providers
7. Doors — future
8. Events and Jobs — future
9. Reports and Publication — future B-022
10. Retention and Logging
11. Backup and Recovery
12. Advanced System Configuration

It may be invoked directly. From `sfmonitor`, **System Configuration** suspends
the monitor screen, launches `sfconfig` for the same selected board endpoint,
and lets `sfconfig` authenticate independently through the protected local
channel. On return, the monitor reconnects or refreshes its snapshot. No
credential is passed on a command line or environment variable. If launching
is unavailable, the monitor shows the exact safe direct command.

### CLI

The CLI provides noninteractive equivalents for suitable reads and actions,
with stable structured output, typed exit categories, explicit expected
versions, and consequence confirmation. It invokes commands, not keystrokes.

## 5. Shared client and UI architecture

Use separate `sfmonitor` and `sfconfig` binaries over shared project crates:

- an operator protocol/types crate shared with the daemon;
- an operator client crate for endpoint discovery, handshake, authentication,
  requests, subscriptions, cancellation, and errors; and
- a shared operator UI crate for keyboard handling, navigation, help,
  localization, layout primitives, and presentation—not business rules.

The foreground console, CLI, tests, and future web administration use the same
command/query service. `sfmonitor` and `sfconfig` remain separate binaries so
headless deployments can install/use only the needed surface and direct
`sfconfig` invocation remains possible.

`ratatui` and `crossterm` are reasonable candidates, not accepted dependencies.
Before adding either, the implementation pass must check current license,
platform support, maintenance, transitive dependencies, resize/input behavior,
and terminal restoration after panic or child launch.

## 6. Local IPC transport

The daemon exposes one platform-neutral `OperatorEndpoint` abstraction:

- Unix, macOS, and BSD/Linux: a Unix-domain socket inside a board-private
  runtime directory. The parent is owner-only by default, the socket is not
  world-accessible, symlinks and wrong ownership fail closed, and peer user
  credentials are verified with the platform facility where supported.
- Windows: a local named pipe with an explicit ACL for the daemon service
  identity and configured operator users/group. Remote-pipe clients are
  rejected and the client process token/SID is checked.
- Loopback TCP is not an automatic fallback. Any future tunneled or remote
  administration channel requires a separate security gate and mutual
  authentication.

The endpoint has no generic filesystem, SQL, signal, process, or shell method.
Socket/pipe discovery uses validated board identity and a configured runtime
location; it never trusts a caller-supplied arbitrary path from an IPC request.

The daemon may run as a nonprivileged service account. Endpoint ownership and
ACLs grant the minimum operator group access; neither root nor Administrator is
assumed. Failure to establish trustworthy permissions or peer identity
prevents the operator endpoint from starting while caller-service policy is
handled explicitly and visibly.

## 7. Authentication and authority classes

Local operator authentication is based on the operating-system identity
presented by the protected socket or pipe plus a board-local allowlist mapping
that identity to an `OperatorPrincipal` and capability set. The protocol binds
that principal to a random connection challenge, board instance ID, daemon
generation, and short-lived operator session. There is no second reusable
Sysop password.

If a platform cannot provide reliable peer credentials, B-021 support on that
platform requires a separately stored, owner-readable board-local key used in
a challenge-response exchange. The key is never accepted on a command line,
environment variable, status page, or log. Endpoint permissions alone are
not enough when the peer cannot be identified.

Authority classes remain distinct:

| Class | Authority |
|---|---|
| Ordinary caller | Caller-visible commands only |
| Threshold Sysop | BBS-level menu capabilities granted by configured security; no host control |
| Named Sysop | Protected BBS identity and explicitly granted remote BBS administration; no host control by identity alone |
| Host/local operator | OS-authenticated local principal with an explicit capability set |
| System | Internal bounded maintenance/recovery actions, never an interactive identity |

A named Sysop who is also an authorized local OS user acts as a host operator
only through the local endpoint; the audit records that host principal. High
numeric caller security never satisfies local operator authentication.

## 8. Capability model

Capabilities are typed enum values, not arbitrary strings supplied by a
client. The initial B-021 set is:

### Observe

- `ObserveBoardStatus`
- `ObserveNodes`
- `ObserveCallerActivity`
- `ObserveOperationalEvents`
- `ObserveStatistics`
- `ObserveNotifications`
- `ObserveMaintenance`
- `ObserveSecurityAudit` — separate, more sensitive, and not in the ordinary
  monitor preset

### Operate

- `ManageSysopAvailability`
- `ManagePages`
- `ChatWithCaller`
- `AdjustSessionTime`
- `DisconnectSession`
- `AcknowledgeNotification`
- `RequestGracefulShutdown`
- `RequestBackup`
- `RunApprovedMaintenance`

### Configure/manage

- `EnterSystemConfiguration`
- `ReadConfiguration`
- `ChangeOnlineConfiguration`
- `ChangeSensitiveConfiguration`
- `ManageCallers`
- `ManageMessages`
- `ManageFiles`
- `ManageSecurity`
- `ManageRecovery`

These are authorization boundaries, not a promise that every command ships in
the first slice. Each request names exactly one required capability; the
daemon rechecks the current principal mapping and target authority immediately
before effect. Broad presets are constructed server-side and cannot be
claimed by the client.

## 9. Protocol and compatibility

The logical protocol is length-delimited, versioned, typed, and bounded. A
connection begins with:

- protocol major/minor range;
- client kind and version;
- board instance identity and daemon generation;
- supported feature/capability identifiers; and
- maximum frame/subscription limits.

The server returns its selected compatible protocol and available features.
An incompatible major version fails with a localized actionable error. Minor
or feature differences degrade by hiding/disabling unsupported actions; exact
binary version equality is not required.

Every request carries:

- `RequestId` for response correlation;
- optional `CommandId` for state-changing retry identity;
- authenticated operator-session identity;
- operation variant and bounded typed payload;
- target stable ID and expected target generation/version where applicable;
- deadline; and
- optional confirmation token for a preflighted consequence.

Results are typed as success, validation failure, authorization failure,
stale target, version conflict, confirmation required, busy, maintenance
required, restart required, unavailable, cancelled, timeout, or internal safe
failure. Detailed host errors remain local diagnostics rather than caller or
ordinary operator payload.

Subscriptions start from a snapshot generation and carry monotonically
sequenced updates. A gap, overflow, reconnect, or daemon-generation change
requires a fresh bounded snapshot. Each connection, request, frame, string,
list, subscription queue, and deadline has an implementation-tested limit.

## 10. Command idempotence and stale-target safety

Node numbers are display slots and may be reused. Every live-session command
therefore carries `NodeId`, `SessionId`, daemon generation, expected session
generation, and `CommandId`. The daemon resolves the tuple atomically and
rejects an ended, replaced, or changed session before applying an effect.

The command journal records the bounded request fingerprint and terminal
result. Repeating the same `CommandId` and identical fingerprint returns the
recorded result. Reusing it with a different request fails closed. Concurrent
duplicates have one executor. A daemon restart makes every previous live
session target stale even if the visible node number is reused.

Read requests use `RequestId` only. Safe naturally idempotent mutations may
still use `CommandId` so audit and retries remain consistent.

## 11. Schema 19 decision

**Schema 19 is required for B-021 implementation, but is not implemented by
this gate.** Schema 18 lacks a durable generic operator-control audit and its
observability audit intentionally accepts only notification/retention actions.
Time adjustment, disconnect, shutdown, and other high-consequence commands
need retry-safe receipts and durable security accountability.

Schema 19 should add only:

1. `operator_command_journal` — unique bounded CommandId, request fingerprint,
   actor kind/stable ID, capability/action code, target kind/stable ID and
   expected generation, requested/completed times, terminal result code, safe
   result version, and bounded receipt retention; and
2. `operator_control_audit` — append-only semantic attempt/result records
   linked to CommandId, actor, action, target, time, result, and safe
   correlation, outside ordinary operational-event pruning.

It must not persist IPC sessions, UI state, passwords, bearer tokens, peer
credential material, chat text, terminal content, private profiles, endpoints,
host paths, arbitrary request JSON, configuration copies, or speculative
network/door/scheduler state.

Configuration generation storage is **future/conditional**, not a schema-19
requirement. The first `sfconfig` mutation gate must choose a coherent
generation/digest authority for typed TOML plus database-owned settings rather
than adding an unused table now. Maintenance job state likewise waits for the
owning capability.

Migration 18→19 must be transactional, create empty command/audit history,
preserve all schema-18 authority, and roll back to intact 18 on failure. Cold
backup retains both new tables. A restored command receipt cannot authorize a
stale live session because daemon/session generations change.

## 12. Node actions

### Page and chat

Caller-initiated page, availability, answer, decline, chat, timeout, and
disconnect continue to use the current `InteractionHub` domain. B-021 adds an
operator-initiated invitation against a fresh session generation. The caller
is notified and may accept/decline unless an exact historical forced-chat mode
is separately enabled by policy. On end, timeout, disconnect, or operator
client loss, both sides leave chat cleanly and the caller returns to the prior
safe section when the section can resume.

For native policy, caller time pauses only during an accepted
operator-initiated local chat, matching the documented stock outcome. Time
does not pause while a page is pending, declined, unanswered, or during
caller-to-caller node chat. The implementation must represent the pause as a
session-clock interval, not rewrite daily usage. Chat text is not audited or
recorded.

### Caller observation

Rendered-session observation is **not part of B-021 completion**. It remains a
later separately gated capability: local-only, audited, caller-notified,
non-recording, rendered-output only, and fail-closed during authentication,
private-message composition, profile/contact/secret entry, or an unidentified
sensitive state. No raw keystroke capture or silent spying is accepted.

### Time adjustment

`AdjustSessionTime` carries the exact live target, signed minutes, reason code,
expected session generation, and CommandId. The service accepts a bounded
range of **−120 through +120 minutes per command**, with five-minute presets
for stock familiarity. Zero and overflow fail validation.

The adjustment changes only that session's effective deadline/allowance. It
does not rewrite cumulative time used, board-day usage, caller history, or
configured security limits. A negative result cannot move the deadline before
the command commit; if no time remains, normal localized time-expiry and clean
session finalization occur. Positive grants cannot bypass account Disabled/
Deleted state, current authorization, or another policy denial. Retry with the
same CommandId cannot apply twice.

### Disconnect

`DisconnectSession` requires the fresh target, a bounded reason code, and an
explicit notice policy. Normal behavior is:

1. mark the session disconnecting and reject new session work;
2. show the selected localized notice when the carrier permits;
3. cooperatively cancel/finalize transfer and chat state;
4. settle only completed accounting;
5. close terminal/capture/resources;
6. release the node; and
7. commit operational event plus security audit result.

An emergency transport abort is a separately labeled fallback after a bounded
grace period. It still performs recovery/finalization and never targets a
replacement session.

### Graceful shutdown and restart

`RequestGracefulShutdown` controls SPITFIRE NG only:

1. enter draining state and stop accepting callers;
2. advertise or send a localized bounded notice according to policy;
3. wait a bounded drain interval;
4. finalize or cooperatively cancel transfers/chat/jobs;
5. flush authoritative state and diagnostics; and
6. stop listeners and exit with a structured result.

Immediate emergency daemon stop is reserved for local process/service
management and is not a normal B-021 API command. Host shutdown/reboot and a
host shell are never exposed.

Application **restart is not a portable B-021 command**. The daemon can request
a graceful exit with `restart-required` intent/status, but an external service
manager may restart it only under a separately documented deployment contract.
The operator client must not pretend a restart occurred.

## 13. Configuration ownership and conflicts

The daemon owns online configuration changes. `sfconfig` first reads a typed
snapshot with a config generation/digest and per-domain resource versions,
submits a complete validated candidate/delta with expected values, and receives
an effect plan:

- applied online;
- persisted, restart required;
- maintenance required;
- offline only;
- conflict; or
- rejected.

No stale edit overwrites a newer value. On conflict, the client refreshes and
offers a field-aware review; it never silently merges secrets or arrays. TOML
is written as a complete validated temporary file, synced, and atomically
replaced. Database-owned policy remains in its owning transaction. A command
that spans authorities must define ordering, rollback, and runtime publication
before implementation; a watcher is not authority.

### Operation classes

| Class | Examples | Rule |
|---|---|---|
| Online safe | read status/configuration; page availability; notification acknowledgement | Typed command/query; current authorization |
| Versioned online | retention policy; public-information policy; safe area metadata; enabled state with supported live apply | Expected version/generation, atomic persist/apply, audit |
| Restart required | listener binding, some transport/host-key path or process-level changes | Validate and persist candidate, report exact pending effect; do not claim active |
| Maintenance required | listener/SSH key rotation, deep cross-domain integrity check, root replacement, destructive reconciliation | Keep operator IPC alive; stop admission and drain/fence affected work |
| Offline only | schema migration, restore, database replacement, board-root relocation, exact legacy-store repair | Daemon stopped; exclusive board operation lock; backup/rollback |

The existing cold backup remains offline-only. B-021 may request a backup
preflight/status and a graceful shutdown for backup, but it does not invent an
online backup. Restore remains offline, explicitly selected, strongly
confirmed, and protected by the existing recovery checks.

## 14. Audit, privacy, and secrets

Every privileged state-changing request records a durable security audit
attempt/result. A privacy-safe operational event is added only when useful to
operate the board. Domain mutations keep their existing purpose-specific audit
and link the CommandId rather than duplicating private values.

Audit contains operator principal, semantic action, stable target, expected
generation/version, time, result, CommandId/correlation, and safe old/new
category or version where appropriate. It excludes passwords, tokens, chat,
terminal input/output, message/file contents, private profiles, login
identifiers, raw endpoints, host paths, secret values, and arbitrary payloads.

Future secret configuration views return `Missing`, `Configured`, or
`Invalid` plus safe metadata. They never reveal a stored value by default.
Changes use write-only secret fields and audit only semantic state. This rule
will apply to FidoNet/BinkP link passwords, network keys, door secrets, and
service credentials.

Destructive/consequential actions use a common two-step contract: preflight
returns impact plus a short-lived digest-bound confirmation token; commit
requires the same generation, CommandId, action, target, and impact. TUI and
CLI can present confirmation differently but cannot bypass the server contract.

## 15. Concurrency and failure behavior

Multiple read-only monitors are allowed. Mutations use scoped leases and
optimistic versions, not a global UI lock. Simultaneous commands against one
session serialize by session generation; notification acknowledgement and
configuration writes commit once; unrelated nodes continue.

Required failure semantics:

- daemon unavailable: clear connection error, no offline mutation fallback;
- version mismatch: negotiated degradation or explicit incompatibility;
- authorization failure: no effect and privacy-safe audit;
- stale target/configuration: explicit conflict with current safe identity;
- timeout/cancellation: no assumed success; query CommandId receipt before
  retrying a mutation;
- daemon restart: endpoint generation changes, subscriptions gap, operator
  reauthenticates, live targets become stale;
- client crash: daemon and caller sessions continue, owned chat exits cleanly;
- partial maintenance: durable owning job/journal reports recovery action;
- configuration persistence/live-apply failure: return a precise pending or
  rolled-back state, never ambiguous success.

No ordinary operator query or command may globally block caller sessions.
IPC has bounded frames, queues, tasks, deadlines, and per-principal rate limits.

## 16. Notifications and maintenance integration

B-021 exposes the B-017 notification list/detail and existing versioned
acknowledgement. Acknowledgement marks operator attention; it does not delete
the source event. Resolution remains owned by the subsystem or an explicitly
authorized repair action.

Maintenance operations use a registry of typed descriptors: ID, owner domain,
required capability, online class, preflight type, expected versions, progress
projection, cancellation support, help topic, and audit policy. B-021 initially
adapts existing approved services rather than creating generic commands:

- B-015 file maintenance and reconciliation;
- B-017 retention cleanup/summary reconciliation;
- existing backup validation/status;
- future B-018 caller/message retention;
- future B-020 scheduler/job actions; and
- future door/network owners.

B-012/B-015 verification work is not pulled into B-021. Their existing typed
services can later be invoked through this framework without reimplementation.

## 17. B-022, scheduler, doors, and networking

B-022 remains NOT STARTED. `Reports` may be a disabled/future navigation
entry, but B-021 adds no formatter, printer, JSONL export, BBS/CLR publication,
or arbitrary filesystem destination.

Future scheduler definitions belong in `sfconfig`; runs, next-event status,
failures, retry, and cancellation belong in `sfmonitor`. These register typed
services and never expose a shell command as an operator primitive.

Future door definitions and DOSBox-X/DOSEMU2 adapter policy belong in
`sfconfig`; running-door state and bounded lifecycle actions belong in
`sfmonitor`. Door processes remain isolated from the daemon and operator IPC.

Future `Networks` must support multiple providers and multiple links. The
operator architecture must not assume one leaf-node topology. A later FidoNet
provider may represent SPITFIRE NG as a leaf/end node, routing node, EchoMail
hub, FileEcho hub, boss node for points, or a valid combination across
multiple FTN domains.

Conceptual FidoNet configuration areas are identity/addresses, links,
nodelists/pointlists, NetMail, EchoMail, routing, BinkP, AreaFix, FileEcho/TIC,
FREQ, points/boss-node policy, security, and advanced. Monitor views may later
show local addresses, listener/queue/link health, last exchange, sessions,
mail/echo/file processing, requests, reachability, nodelist freshness, and
errors. This is navigation extensibility only—no FTN schema or behavior is
defined here.

CircuitNet remains preservation/compatibility knowledge and a possible future
revival, not an assumed active network requirement. The provider boundary
leaves room for it without making it a dependency.

NodelistDB (`https://github.com/xx25/nodelistdb`) is recorded for future
read-only engineering comparison: nodelist/pointlist ingestion, source and
effective snapshots, node history, reachability/BinkP tests, and operator
network-health presentation. It is not FTSC authority, SPITFIRE history, or a
runtime dependency. FTSC documents remain the future technical authority.

Future networking design must start from an independently inventoried,
provenance-classified standards corpus. No FTSC material was interpreted as
part of B-021 and no FidoNet behavior is defined by this gate.

## 18. Synchronet secondary review

The official Synchronet utilities index confirms separate local/LAN Sysop
utilities for node display/control, configuration, user editing, statistics,
repair, files, and networks. A bounded secondary review showed UMONITOR as a
board/node view with contextual node actions and a System Options route into
SCFG and specialist tools; SCFG supplies one hierarchical expert configuration
environment with list/detail editors and contextual help.

NG adopts or adapts:

- monitor-to-configuration navigation;
- one shared client/service boundary;
- board snapshot plus node list;
- contextual actions against a selected fresh session;
- scalable list/detail configuration;
- visible keyboard hints and contextual help; and
- future provider-owned network/door/job areas under a common shell.

NG rejects:

- Synchronet's terminology, data model, colors, layout, keys, and branding;
- direct config/filter-file editing;
- visible secrets and unnecessary paths/endpoints;
- broad recycle/rerun/interrupt/toggle primitives;
- unaudited spying;
- arbitrary process launch; and
- any implication that Synchronet defines SPITFIRE history.

No Synchronet code or text is copied. This is secondary product-engineering
input only.

## 19. TUI interaction and accessibility

Operator tools are keyboard-complete:

- arrows move within lists/fields;
- Tab and Shift-Tab move logical focus;
- Enter opens or commits the selected ordinary action;
- Esc backs out and never discards dirty changes silently;
- F1 opens contextual help;
- Space toggles only an explicit toggle; and
- dangerous actions open a consequence-specific confirmation.

Mouse support is optional and mirrors keyboard actions. Focus, severity,
selection, pending changes, and errors are never color-only. The UI supports
monochrome/reduced-color terminals, resize, bounded scrolling, explicit status
text, and no required animation.

The preferred full layout is 100×30 or larger. At 80×24 the tools collapse
secondary panes into tabs and keep every action available. Below 60×20 they
show a localized minimum-size notice plus CLI guidance rather than clipping,
overlapping, or dispatching hidden actions.

## 20. Localization and contextual help

Domain and protocol code return stable keys and typed arguments. Localization
must cover connection state, authentication/authorization, protocol mismatch,
stale target, version conflict, timeout, unavailable daemon, confirmation,
time adjustment, page/chat, disconnect, drain/shutdown, configuration effect,
restart/maintenance/offline requirements, navigation, empty lists, and
recovery.

Stable help topics are:

- `operator.dashboard`
- `operator.nodes`
- `operator.callers`
- `operator.actions`
- `operator.page-chat`
- `operator.disconnect`
- `operator.time-grant`
- `operator.shutdown`
- `operator.configuration`
- `operator.security`
- existing `operator.activity`, `operator.statistics`, `operator.errors`, and
  `operator.retention`
- future `configuration.board`, `configuration.security`,
  `configuration.messages`, `configuration.files`,
  `configuration.networks`, `configuration.doors`,
  `configuration.events`, and `configuration.reports`

The UI resolves concise help from action/configuration metadata and links to
the Sysop Manual for longer guidance. It does not duplicate entire chapters.

## 21. Documentation impact

Implementation completion requires:

- **Sysop Manual:** Using the Operator Monitor; Nodes and callers; Operator
  actions; Notifications and maintenance; System Configuration; Online versus
  restart/maintenance/offline settings; shutdown/recovery; operator security;
- **Caller Guide:** only caller-visible page/chat, time-grant notice, or
  operator-disconnect wording that actually changes;
- **Technical Reference:** schema 19, IPC, authentication, capability and
  command models, stale-session protection, configuration generations,
  concurrency, audit, recovery, and platform endpoint security;
- **Contextual help:** the stable topics above; and
- **Quick Start:** no change during the gate or backend slices. Revisit only
  when an installed `sfmonitor`/`sfconfig` becomes part of the basic supported
  operator journey.

## 22. 86Box and exact historical evidence

No 86Box run is required before native B-021 implementation. Primary
documentation clearly establishes the outcomes needed for the interface.
Controlled original-runtime tests may later refine:

- exact F6/F7 boundary and immediate-expiry behavior;
- operator-initiated chat return and clock-pause behavior;
- Alt+F1 notice timing versus transfer/chat state;
- forced-chat and page availability transitions; and
- exact F10 prompts/shutdown ordering.

Those details may refine an exact compatibility profile. They do not justify
unsafe DOS mechanisms or block the typed native contract.

## 23. B-021 implementation acceptance matrix

| Area | Required acceptance |
|---|---|
| Historical semantics | Status, availability/page/chat, caller security/lifecycle, five-minute-equivalent time adjustment, disconnect with/without notice, and clean SPITFIRE shutdown have accepted modern outcomes; DOS shell, printer, and chat capture are excluded. |
| Schema 19 | Transactional 18→19 migration adds only command journal/control audit, creates no fabricated history, preserves schema 18, and rolls back intact under injected failure. |
| Daemon authority | Foreground console, attach CLI, and clients use one typed service; no online SQLite/TOML/log/status/session access exists. |
| IPC platforms | Protected UDS permissions/peer identity and Windows named-pipe ACL/SID behavior pass; symlink, wrong owner, remote pipe, and unauthenticated loopback fail closed. |
| Protocol | Major/minor/feature negotiation, bounds, deadlines, cancellation, subscription gap, explicit errors, and incompatible/degraded clients pass. |
| Authentication | OS principal mapping and challenge/session binding pass; missing identity, removed capability, stale connection, replay, and fallback-key failures deny access without secret leakage. |
| Authorization | Every read/action checks one current capability at dispatch; ordinary, threshold Sysop, named Sysop, host operator, and system matrices pass. |
| Session safety | Every page/chat/time/disconnect command checks daemon, NodeId, SessionId, session generation, and CommandId; node reuse, end/reconnect, concurrent commands, and restart cannot cross-target. |
| Idempotence | Identical CommandId retry returns one result; changed fingerprint fails; crash/timeout receipt lookup prevents double time grants or duplicate action. |
| Page/chat | Existing caller page path, availability, answer/decline, operator invitation, timeout/cancel/client loss, safe-section return, and documented session-clock policy pass without recording content. |
| Time grants | −120/+120 bounds, five-minute preset, zero/overflow rejection, caller-visible change, expiry, rollover, accounting, retry, disconnect, and policy reauthorization pass without rewriting usage. |
| Disconnect | Notice/no-notice, active transfer/chat, grace timeout, emergency fallback, accounting/finalization, event/audit, and node release pass. |
| Shutdown | Admission stop, drain, bounded notice, active sessions/transfers, flush, listener stop, duplicate request, timeout, and clean process exit pass; no host action occurs. |
| Configuration entry | Direct `sfconfig` and monitor System Configuration handoff select the same board, authenticate independently, return/refresh safely, and pass no secret in argv/environment. |
| Configuration control | Typed snapshots, generation/digest, per-domain versions, CAS conflict, validate/persist/apply effect, restart/maintenance/offline classification, and audit pass before online editing ships. |
| Privacy | Projections/actions/audit exclude credentials, login/private identity, contacts, chat/content/input, endpoints, paths, and raw payload; private views require separate capability. |
| Audit | Every privileged attempt/result links CommandId and purpose-specific audit; append-only control audit survives ordinary operational retention and backup. |
| Multinode/multi-operator | Two callers and two clients can monitor concurrently; conflicting commands commit once or return stale/conflict without starving unrelated sessions. |
| Failure/recovery | Daemon absent/restart, endpoint loss, client crash, request timeout, partial response, subscription overflow, and maintenance failure have bounded clear recovery. |
| Backup/restore | Schema-19 receipt/audit survives cold backup/new-root restore; live operator sessions/subscriptions do not; restored live targets are stale. |
| Notifications/maintenance | B-017 list/detail/ack remains authoritative; maintenance registry invokes only approved typed owners and never deletes source evidence through acknowledgement. |
| Localization/presentation | Every user-visible state/error/confirmation/help result is localized; presentation changes framing only. |
| Accessibility | Keyboard-only and 80×24 degraded journeys pass; focus/status is non-color-only; terminal resize and child-config launch restore terminal state. |
| Documentation | Affected Sysop, Technical, Caller (if needed), and contextual-help material is current; Quick Start changes only if the installed basic journey changes. |
| Real journey | Two real caller sessions plus two local operator clients complete observe, page/chat, one time adjustment, notice disconnect, node reuse/stale rejection, reconnect, configuration entry, and graceful shutdown. |
| External client | A noninteractive attach client completes bounded status/action/receipt lookup with stable structured output and correct exit categories. |
| Scope | No B-022 output engine, observation/spy, scheduler, doors, networking, generic host access, `sfmonitor`/`sfconfig` business logic, or direct database ownership is smuggled in. |

B-021 remains PARTIAL until the complete matrix passes.

## 24. Recommended implementation sequence

### B021-A — protected read-only attach foundation

- schema-19 migration skeleton and command/audit model tests;
- platform endpoint abstraction and Unix implementation first, with Windows
  contract/tests and implementation before VERIFIED;
- handshake, OS-principal authentication, capabilities, request bounds,
  B-017 queries/subscriptions, and reconnect/gap behavior;
- noninteractive attach CLI; and
- foreground console adaptation to the same service.

**Build the first usable read-only `sfmonitor` MVP immediately after B021-A.**
It can safely provide Dashboard, Nodes, Activity, Statistics, Notifications,
and Errors/Maintenance while B-021 control commands continue. This prevents
the operator surface from remaining theoretical without mixing UI logic into
the backend.

### B021-B — stale-safe live session controls

- command receipt/audit completion;
- page availability, answer/decline, operator invitation/chat lifecycle;
- signed bounded time adjustment;
- notice/no-notice graceful disconnect; and
- graceful daemon shutdown.

Add these actions to the same monitor/client only after their backend matrix
passes.

### B021-C — typed configuration foundation

- configuration schema/field metadata and effect classification;
- typed snapshot, generation/digest, validation, CAS conflict, and safe atomic
  persistence plan;
- read-only configuration plus one low-risk versioned-online setting end to
  end; and
- monitor System Configuration handoff.

**Build the first usable `sfconfig` MVP alongside B021-C**, once snapshot,
validation, effect classification, and conflict handling are real. It should
start with Board summary/read-only areas and the one accepted mutation, not a
large collection of fake forms.

### B021-D — maintenance integration and full acceptance

- approved B-015/B-017/backup service descriptors;
- platform parity, multi-operator, recovery, localization, documentation;
- real two-caller/two-operator journey; and
- promote B-021 only if the complete matrix passes.

This sequence uses separate `sfmonitor`/`sfconfig` binaries and shared
protocol/client/UI crates. Neither tool waits for every Category-B row, but
neither owns business logic before the backing service exists.

## 25. Subsequent result and next action

B021-A followed this gate and is now cross-platform complete: schema 19,
protected Unix-domain-socket and Windows named-pipe attachment, capability
handshake, B-017 projection transport, attach CLI, and foreground-console
service reuse are implemented. B-021 remains PARTIAL.

The next separately authorized private development step is a read-only
`sfmonitor` MVP over that completed `OperatorClient`. It must not add B021-B
live controls, B021-C configuration mutation, B-022 publication, networking,
doors, scheduler, or caller observation.
