# M039 Tranche 7 — Operator Observability and Reports Gate

**Status:** INTERFACE / RESOURCE / TRANSACTION GATE ACCEPTED

**Rows:** B-017 PARTIAL, B-021 PARTIAL, B-022 NOT STARTED

**Schema at gate:** 17. Schema 18 and B-017 were subsequently implemented and
verified in the [B-017 implementation
record](m039-tranche-7-b017-observability-implementation.md). B-021 remains
PARTIAL and B-022 remains NOT STARTED. This document remains the accepted
contract rather than a runtime-status page.

## 1. Purpose and scope

This gate defines how SPITFIRE NG will show a Sysop what the board is doing,
retain useful operational history, calculate reports, and publish or export
those reports without creating a second source of truth. It covers the
semantic interfaces required to implement and verify B-017, B-021, and B-022.

It does not add an event database, report engine, operator IPC endpoint,
`sfmonitor`, `sfconfig`, scheduler, printer adapter, dynamic-display language,
or caller-observation feature. It changes no parity status.

The order is:

1. **B-017** establishes privacy-bounded operational events, daily summaries,
   notifications, statistics, and maintenance views.
2. **B-021** exposes daemon-authoritative operator views and the remaining
   stock-equivalent controls through the same typed service boundary.
3. **B-022** renders those projections to screen and confined exports or
   publications.

B-021 and B-022 both depend on B-017. B-022 also uses B-021's operator
authentication and command boundary. Domain facts continue to belong to the
caller, message, file, transfer, storage, node/session, backup, and operator
services that produce them.

## 2. Canonical row definitions and current state

The canonical parity ledger defines the rows as follows:

| Row | Canonical capability | Evidence | Current status | Exact recorded gap |
|---|---|---|---|---|
| B-017 | Sysop logs, daily statistics, notifications, maintenance views | SF37 §8.2, §12 | PARTIAL | Privacy-safe structured runtime events, operator status, paging, and notifications exist. Stable historical-style daily aggregates, bounded maintenance views, retention/rotation, and safe exports remain. |
| B-021 | Local/Sysop operator controls | SF37 §8, §12–13 | PARTIAL | `spitfire console` provides status, page availability/answer/decline/chat, targeted disconnect, caller list, enable/disable, security change, and clean exit. Time grants, richer maintenance, attachable control IPC, and complete stock command breadth remain. |
| B-022 | Screen/export/print-oriented operations | SF37 §10, §12 | NOT STARTED | Preserve export/report outcomes; printers are optional destinations, not core dependencies. |

Already implemented foundations include structured `tracing` diagnostics,
privacy-safe purpose-specific audit tables, caller and file statistics, daily
transfer usage, transfer records and settlements, node/session snapshots,
page/chat coordination, a foreground `spitfire console`, cold backup, staged
resource publication, localization, and presentation profiles. These are
useful inputs. None is a substitute for the bounded cross-domain operational
history and report contract defined here.

The current `WORK/runtime-status.toml` is a transient same-host status
artifact. Future operator clients must not treat it, SQLite, TOML, text logs,
or process memory as an alternate authority. In particular, future live
operator views use the redacted projection below rather than publishing login
identifiers or host paths from the existing diagnostic document.

## 3. Historical evidence reviewed

### 3.1 Primary SPITFIRE 3.7 behavior

The focused reread covered `SPITFIRE.DOC` work/display files, special function
keys, Sysop utilities, caller/message/file screens, and the Sysop/Sysop-status
distinction, together with the distributed `SFSYSOP.MNU`.

Confirmed stock behavior includes:

- `CALLERS.LOG` was a WORK-file record of caller activity, supported by the
  distributed `SFCALLOG.DAT` configuration. `CALLERS.TMP` was transient input
  later appended to it. `HEYSYSOP.LOG` held special Sysop notifications.
- F1 and Sysop-menu `V` showed a log menu. The Sysop could begin at today's
  entries, the beginning, or a specified date; a missing date advanced to the
  next newer date.
- F5 at the Ready prompt showed total calls, calls today, uploads and downloads
  today, door use today, new callers today, and messages today.
- F2 changed the online caller's security. F6 and F7 removed or added five
  minutes. F3 controlled page availability; F8 switched the local status
  layout; F10 terminated SPITFIRE; ALT+F1/ALT+F2 disconnected a caller with or
  without a notice.
- The divided local screen showed caller identity and activity, but also
  historical contact and birth information that is not appropriate for an
  ordinary modern status projection.
- F4 duplicated `CALLERS.LOG` output to a physical printer. Sysop-menu `P`
  wrote `SFUSERS.LST` to disk or printed caller records. Historical output
  included passwords and contact details and therefore cannot be a modern
  default.
- Sysop-menu `R` deleted selected logs/backups and recreated some logs. This is
  evidence for explicit retention/cleanup, not authority for unrestricted
  deletion of security audit.
- Historical threshold “Sysop Status” and the configured named Sysop were not
  identical. Neither maps automatically to a modern host operator.

`SFCALLOG.DAT` contains distributed activity phrases for login/logoff,
authentication failure, message/file activity, batch/external transfers,
questionnaires, subscription changes, doors, chat, and other actions. It is
one official configuration sample, not a complete stable log grammar. No real
historical caller log is present in the corpus, so raw field layout, escaping,
and every activity line remain unresolved.

### 3.2 Official Buffalo Creek utilities

These programs establish official ecosystem workflows but do not turn their
flat inputs or generated bytes into native NG authority:

- **SFLOG 2.1** reads `CALLERS.LOG`, requires its log-start date line,
  generates an operator-selected `.BBS`, `.CLR`, and `.RIP` activity display,
  and can rename the log to a date-derived filename. The documentation does
  not enumerate every statistic or include generated output.
- **SFTOPTEN 1.41** reads `SFUSERS.DAT` and produces top-caller,
  top-uploader, and top-downloader `.BBS`, `.CLR`, and `.RIP` bulletins. It
  documents multinode file sharing and lifetime caller counters, but not exact
  tie ordering or every exclusion rule. A negative call count was the DOS-era
  opt-out convention; NG must use explicit publicity policy instead.
- **SFUSERS 3.1 `/S`** creates `.BBS`, `.CLR`, and `.RIP` bulletins containing
  caller count and averages for calls, file counts/bytes, age, expert mode,
  and selected transfer protocol. NG may preserve useful aggregate semantics,
  but date of birth remains private and age statistics have no accepted
  operational justification.
- **SFMKLIST 2.5** combines area `SFFILES.BBS` listings into a downloadable
  board-wide list, supports a header, area exclusions and output filename,
  optionally invokes PKZIP, and catalogs the result in a selected file area.
  NG must generate from file authority, then publish through the existing file
  domain; it must not parse its own generated listing as authority or shell to
  an archiver.
- **SFSYSTEM** exposes the stock total/daily counters and board starting date.
  Those editable DOS fields establish report vocabulary, not permission for a
  report client to rewrite native counters.

Stock SPITFIRE supplied log views, daily status, local status, time/security/
disconnect controls, and disk/printer choices. SFLOG, SFTOPTEN, SFUSERS `/S`,
and SFMKLIST were official utilities that derived and published additional
reports. Modern reports identify which lineage they preserve and do not claim
byte-perfect output without a controlled fixture.

## 4. Observability model

SPITFIRE NG separates four concepts:

1. **Operational event:** a bounded semantic fact useful for operating or
   diagnosing the board, such as a session ending, a transfer failing, or a
   storage root becoming unavailable.
2. **Security/audit event:** durable accountability for a privileged or
   security-sensitive action. Existing purpose-specific audits remain
   authoritative; Tranche 7 adds operator-action audit only for its new
   privileged commands.
3. **Report projection:** a read-only, coherent calculation from domain state,
   operational history, daily summaries, or audit projections.
4. **Publication artifact:** rendered output for a terminal, display resource,
   file-area catalog item, or confined operator export. It is never the source
   of the report's values.

An event can cause a notification and can contribute to a report without
becoming security audit. An audit event can appear in an authorized audit
projection without entering ordinary operational history. A generated text
file can be deleted and regenerated without changing caller, message, file,
transfer, or statistics authority.

## 5. Event contract

### 5.1 Envelope

The typed domain API uses an `OperationalEvent` equivalent with:

- stable monotonic `EventId`;
- UTC occurrence time;
- board-local civil day and the timezone-policy version used to assign it;
- category, severity, stable event code, and outcome;
- optional safe NodeId, SessionId, CallerId, correlation ID, and one typed
  domain-object reference;
- retention class; and
- a versioned event-code-specific attribute structure.

Attributes are not an arbitrary map. Each event code defines allowed fields,
types, byte limits, and redaction. The encoded form is bounded to 2 KiB. The
database rejects unknown category/severity/outcome/retention values; the
domain decoder rejects unknown or malformed payload versions without exposing
raw payload to ordinary views.

Current categories are `system`, `node`, `session`, `caller`,
`authentication`, `message`, `file`, `transfer`, `storage`, `backup`,
`operator`, and `error`. Scheduler, door, QWK, network, FidoNet, CircuitNet,
and extension categories are reserved for their future owners; Tranche 7 does
not emit them.

Severity is deliberately small: `info`, `notice`, `warning`, `error`, and
`critical`. Severity describes operational impact. Audit importance describes
accountability. A caller-facing error is localized separately. The three must
not be inferred from one another.

### 5.2 Transaction and delivery

When an event describes a durable domain mutation, the owning transaction
inserts the operational event or a transactional outbox record before commit.
Only committed facts enter durable history or daily summaries. After commit,
the daemon fans out the event to a bounded in-memory live stream. Failed
fan-out does not roll back the domain fact; subscribers receive an explicit
gap marker and can resume from the last durable EventId.

Pure runtime transitions that have no durable owner may enter the live stream
and, when their event code is retained, the operational ledger through the
daemon's event writer. A daemon crash can lose only uncommitted runtime
events. No report claims a session completion until the session/accounting
transaction commits.

Event ordering is deterministic by `(occurred_at_utc, EventId)`. EventId is
the tie-breaker, not a claim that wall clocks on every producer are perfectly
ordered. Each report records a maximum EventId and database snapshot boundary.

### 5.3 Redaction

Normal operational events never contain passwords or hashes, login secrets,
private keys, API credentials, private-message bodies or subjects, message
composition buffers, questionnaire answers, chat text, file/archive/DIZ
contents, raw terminal input, packets, private real names, contact details,
birth dates, arbitrary remote endpoints, or host paths.

Events use stable IDs and semantic outcomes. A display handle may be resolved
under the viewer's authority. A short handle snapshot is permitted only for a
typed call-history event when required to preserve a useful historical
display after an authorized identity change; it is subject to operational
retention and is never a login identifier or real name.

Authentication failure events report reason class, transport, node where
allocated, and bounded correlation—not the supplied identifier or secret.
File and transfer events report stable IDs, direction/protocol, bounded counts,
and result—not negotiated raw names or storage locations.

## 6. Schema-18 decision

Schema 18 is **REQUIRED** for B-017. Current schema 17 cannot preserve a
retention-bounded cross-domain event sequence, historical daily summaries,
notification acknowledgement, or crash-safe report publication generations.
B-021's time grant can remain session state, and B-022 report calculation does
not independently require tables, but both use schema-18 observability and
publication state.

The implementation migration must be transactional, retain all schema-17
caller/message/file/transfer/storage and audit authority, and validate exact
pre/post counts and integrity. It creates no synthetic pre-migration events,
calls, transfers, messages, errors, or daily totals. Lifetime reports continue
to use existing authoritative domain counters. Historical daily/recent-event
views begin explicitly when schema 18 becomes active.

### 6.1 Required native state

Names may follow final Rust/SQLite conventions, but schema 18 requires these
semantic records:

| State | Authority and purpose |
|---|---|
| Operational-event ledger | Append-only retained events with the envelope above and indexes for time/EventId, category/severity, node/session, caller, and correlation. |
| Daily board statistics | One board-day/timezone-version summary updated from committed typed facts. It is derived while detailed events exist and becomes the durable historical summary after their expiry. A high-water EventId permits reconciliation without double counting. It never drives caller policy. |
| Retention policy | Singleton versioned policy with explicit defaults and update audit. |
| Operator notifications | Mutable open/acknowledged/resolved projection linked to its source event; optimistic versioning prevents stale acknowledgement. Event/audit history remains immutable. |
| Operator-action audit | Append-only accountability for new Tranche-7 privileged commands, including actor capability, target stable ID, result, and correlation but no secret/content/path. Existing domain-specific audit remains in place. |
| Report publication definitions | Versioned selection of a built-in report kind, audience, parameters, representation, and approved logical destination. No script or arbitrary template body. |
| Report generation journal | GenerationId, definition/config version, snapshot cutoff, lifecycle, digest/size, approved destination identity, outcome/error code, and timestamps for idempotency and crash recovery. |

Daily summaries use typed columns for the current accepted metrics rather than
an arbitrary key/value counter table: calls started/completed, new callers,
messages posted, successful uploads/downloads and bytes, failed transfers, and
operational error counts by bounded severity. Future door/scheduler/network
owners add their own schema or explicit columns later; unused future totals
are not fabricated now.

Report result caches remain reconstructable memory/disk cache and are not
native authority. A report definition is stored only when an operator saves a
repeatable publication; built-in on-demand reports need no database row.

### 6.2 Initial retention policy

The schema establishes explicit safe defaults:

| Class | Default | Configurable bound | Recovery/backup |
|---|---:|---:|---|
| Live stream | 2,048 events or 15 minutes per daemon, whichever is reached first | fixed implementation safety bound in this tranche | memory only; clears on restart |
| Operational detail | 30 days | 1–365 days | backed up while retained |
| Daily summaries | 400 days | 31–3,650 days | backed up |
| Report-generation metadata | 90 days after completion | 1–365 days | current/nonterminal generations backed up; expired completed history pruned |
| Security/operator audit | existing durable audit policy | not clearable through ordinary operational retention | backed up |
| Generated cache | none required | bounded implementation cache only | excluded; rebuilt |

Cleanup runs in transactions of at most 500 records and yields between
batches. Updating retention is versioned and audited. Shortening retention
shows the affected cutoff and requires confirmation; cleanup never deletes
purpose-specific security audit. “Clear log” means apply the configured
operational cutoff or acknowledge a notification, not erase arbitrary rows.

## 7. Identity and viewer policy

The identity model remains:

- stable CallerId for internal ownership and authorized correlation;
- public handle for ordinary human-readable identity;
- login identifier for authentication only; and
- optional private real name/contact data for specifically authorized caller
  maintenance.

Ordinary operator activity and reports show the public handle where useful.
They do not show login identifiers. Private real names or contacts require a
separate caller-private capability and a caller-maintenance purpose; they are
not fields in activity, top-caller, recent-caller, node, error, or transfer
reports.

Public/caller report publication includes only active callers who have opted
into the existing public directory; that listing preference is the caller's
report-publicity opt-in. Public rankings are disabled by default. If enabled,
they require at least three eligible callers and default to a cohort floor of
ten. A board may raise but not lower the hard floor. Hidden callers,
deleted/disabled callers, PendingReview files, private messages, operator
events, and security errors do not appear.

## 8. Live node and caller-observation boundary

`LiveNodeSummary` is a read-only projection from the existing NodeManager and
session observers. A host operator with `observe.nodes` may see node number,
public handle, transport family, connected/online duration, lifecycle/current
section, TERM, encoding, terminal dimensions, presentation profile, idle
state, connection state, transfer state, and a safe security-context label.
The ordinary view omits CallerId, login identifier, private real name/contact,
remote address, host path, command input, message subject/body, filenames not
already visible under the operator's file capability, and terminal bytes.

This is not a second node authority. Every action carries NodeId, SessionId,
and expected session generation and fails stale after reuse or disconnect.

Rendered-session observation (“spy”) is **not part of B-021 or this tranche**.
The previously accepted future boundary remains: local capability only,
audited, visibly caller-notified, non-recording, redacted rendered output,
and fail-closed during secret entry, private-message composition, and other
sensitive contexts. Chat capture is likewise outside Tranche 7 and requires a
separate consent/retention gate.

## 9. Report model

Every report has a stable `ReportKind`, versioned request, viewer/audience,
typed filters, snapshot cutoff, row/byte budget, and typed result. Reports
query authoritative state or retained summaries. They never update caller or
domain counters.

### 9.1 Required report families

| Family | Required semantic result | Principal source |
|---|---|---|
| Board status | daemon start/health, configured/available/active nodes, listener health without private endpoint, active sessions, open warnings/errors | live daemon projections and notifications |
| Today/system statistics | total completed calls plus current board-day calls, new callers, public/private-safe message counts, successful upload/download count and bytes, current online count, and errors by severity | domain totals, daily summary, live nodes |
| Recent callers | bounded completed calls with public handle, board-local time, transport family, duration, and clean/disconnect outcome | retained session events |
| Caller activity | authorized one-caller counts and recent semantic outcomes without content/contact/login data | caller authority plus retained events |
| Top callers | lifetime calls, successful uploads, or successful downloads, deterministic descending rank and privacy policy | caller counters/publicity policy |
| File/transfer activity | successful/failed/cancelled transfers, protocol/direction/count/bytes, active transfers, unavailable storage, and safe file/area identity where authorized | transfer records/settlements, file/storage authority, events |
| Message activity | messages posted by board day/conference where authorized, public/private aggregate split, and failure counts; no subject/body/recipient list | message authority and daily summary/events |
| Errors and maintenance | unresolved notifications and bounded error events with subsystem, safe reason/remediation key, node/session correlation, and acknowledgement state | events/notifications |
| File catalog publication | authorized current files grouped by area with configured area exclusions and safe catalog fields | file-domain projection |

Door, scheduler, QWK, network, FidoNet, CircuitNet, and extension statistics
remain future. A missing future subsystem is displayed as “not available,” not
zero activity disguised as an implemented metric.

### 9.2 Historical equivalents

- F5 maps to Today/System Statistics. Lifetime calls and current board-day
  values remain visibly distinct.
- CALLERS.LOG maps to Recent Callers plus an authorized Operational Activity
  view. An optional legacy-shaped text export is a compatibility projection,
  not native authority.
- SFLOG maps to a generated activity-summary publication.
- SFTOPTEN maps to three explicitly selected lifetime ranking reports. Ties
  order by descending value, normalized public handle, then stable CallerId;
  exact historical tie behavior remains evidence-gated.
- SFUSERS `/S` maps to privacy-safe caller aggregates. Average age is rejected
  from the stock NG report because it derives from private birth dates and has
  no accepted operational need. Expert-mode and protocol-selection percentages
  require the public cohort floor when caller-facing.
- SFMKLIST maps to a File Catalog report whose publication is committed through
  the file domain. It cannot directly rewrite catalog rows or invoke PKZIP.

## 10. Query, filter, and snapshot bounds

Operator event filters are typed: UTC or board-local time range, category,
minimum severity, NodeId, safe CallerId/handle lookup when authorized,
outcome, and correlation ID. No SQL-like expression is accepted.

- default page: 100 rows;
- maximum page: 500 rows;
- event-detail query range: 31 days, with cursor paging inside retained data;
- report row limit: 10,000 for a confined export and 500 for an interactive
  page before paging/summary;
- rendered publication hard limit: 16 MiB, with a 4 MiB default definition
  budget;
- live subscriber queue: 256 events, then a gap marker rather than unbounded
  memory; and
- event payload: 2 KiB maximum after encoding.

Cursors contain the snapshot generation/cutoff and final ordering key. They
are opaque to clients, bounded in lifetime, and rejected after incompatible
policy or report-definition change. Ordinary report generation uses a SQLite
read transaction and does not lock the whole board. Expensive generation runs
off caller/session threads and observes cancellation and a default 30-second
execution budget.

## 11. B-021 operator command boundary

The implementation extends the presentation-independent operator service and
introduces a small attachable, local-only-by-default authenticated control
endpoint. It is an API/CLI foundation, not `sfmonitor` or `sfconfig`.
Unix/macOS use a protected Unix-domain socket; Windows uses a protected named
pipe. A strongly authenticated loopback fallback requires separate
configuration. Clients never open SQLite, TOML, status files, or logs.

Required typed commands/projections are conceptually:

- get board status and statistics;
- list/inspect live nodes;
- page availability, list/answer/decline/end page/chat;
- add or remove a bounded amount of time from the active session, carrying
  expected SessionId/generation;
- disconnect a session with a localized notice or immediately, with explicit
  reason and result;
- list recent operational events and notifications; acknowledge/resolve an
  allowed notification;
- request/inspect/cancel a report generation;
- generate, publish, or export a report where separately authorized;
- inspect/update versioned retention policy and request bounded cleanup; and
- request graceful daemon shutdown with drain/result state.

Existing caller lifecycle/security and other domain commands remain owned by
their services. Tranche 7 may expose them through the common attach client but
must not reimplement them. Event configuration, file/message packing,
scheduler actions, door/network controls, host shell/drop-to-DOS, process kill,
and arbitrary file access remain with later rows or are deliberately rejected.

Session time grants are ephemeral adjustments to the current session budget.
They do not rewrite the caller's accumulated time or daily policy. They are
bounded, versioned, visible in the caller's next time display, and audited.
Repeated/replayed commands are idempotent by CommandId.

## 12. Authorization model

Capabilities are independent:

- `observe.board-statistics`;
- `observe.nodes`;
- `observe.operational-events`;
- `observe.caller-activity`;
- `observe.security-audit`;
- `operate.page-chat`;
- `operate.session-time`;
- `operate.disconnect`;
- `operate.shutdown`;
- `report.generate`;
- `report.publish`;
- `report.export`;
- `retention.manage`; and
- `retention.prune`.

The daemon reauthenticates the operator channel and reauthorizes each command
at dispatch. High-impact actions record CommandId, actor, capability, target,
expected version, semantic outcome, and correlation in operator audit.

A host/local operator may receive these capabilities from protected local
configuration. A named Sysop logged into the BBS may receive selected
BBS-scoped statistics/report commands. A threshold-Sysop caller does not
automatically receive host observability, security audit, error detail,
retention, export-path, shutdown, or attachment authority. Named Sysop,
threshold privilege, and host operator remain separate.

Caller-visible statistics remain the caller's own existing statistics plus
explicitly published privacy-safe board reports. A caller cannot query raw
events, hidden callers, private-message activity, unpublished files, another
caller's detail, or operator/security state.

## 13. Report generation and publication

Generation is a coherent bounded snapshot:

```text
authoritative state + retained events/summaries
                    |
             typed ReportProjection
                    |
       formatter / presentation adapter
                    |
     confined publication or operator export
```

A GenerationId pins report kind/version, filters, audience, configuration and
policy versions, database snapshot/cutoff EventId, locale, presentation
profile, and destination generation. A repeated CommandId returns the prior
result. Concurrent generation of the same destination uses compare-and-swap;
one winner publishes and the loser receives a conflict.

Output is staged in the approved destination filesystem, length/digest checked,
synced, and atomically renamed. A database journal records planned, rendered,
published, failed, or cancelled state. Recovery removes unclaimed staging,
reconciles a digest-matching published artifact, or reports structured review;
it never treats a partial file as current.

Approved destinations are typed:

- interactive operator/caller terminal view;
- board `DISPLAY` resource stem and `.BBS`/`.CLR` representation;
- a configured confined operator report-export root (defaulting below WORK);
  or
- a selected file area through the existing file-domain add/replace command.

No command accepts an arbitrary host path. Symlinks and traversal fail. A
publication audit records report kind, audience, logical destination identity,
generation, digest/size, and result—not report content or a host path.

## 14. Formats, templates, and presentation

Tranche 7 requires:

- localized terminal view;
- plain text in explicit UTF-8 or CP437 with explicit LF/CRLF selection;
- safe BBS-compatible and ANSI/CLR-compatible display publication; and
- a versioned UTF-8 JSON Lines machine export for noninteractive clients.

JSON Lines is selected because it is streamable, typed/versioned, and avoids
spreadsheet formula interpretation. CSV, HTML, RIP, Sixel, and physical
printer output are not required for this tranche. If CSV is later added, cells
must be protected from formula injection. A physical printer is an optional
licensed/platform adapter over a generated artifact, never a core dependency.

User-supplied strings are encoded as data. Controls are stripped or escaped
according to the target; they cannot become ANSI commands, report variables,
printer instructions, filenames, or structured fields. Encoding failure is a
typed result, not silent replacement unless the selected representation's
documented fallback permits it.

No general report-template language is required now. Built-in typed formatters
use localized labels and bounded presentation resources. Saved definitions
select allowed fields/options; they contain no scripting, expressions, or
filesystem reads. A future dynamic-display system may consume the same report
projection through a separately gated semantic-variable interface. Future
ANSI, RIP, Sixel, and plain representations must not change values,
authorization, privacy, or ordering.

Future scheduler Event A–M support invokes the same `ReportGenerate` command
with a scheduler identity, capability, definition/version, and idempotency key.
It does not run a hidden command string. No scheduler is implemented here.

## 15. Time, multinode, concurrency, and performance

UTC is stored canonically. Board-local dates/times are rendered using the
versioned board timezone authority already accepted for Tranche 6. Daily facts
retain the board day and timezone-policy version assigned at commit. DST
repeated/skipped hours neither duplicate nor lose events; timezone changes
start a new version and do not relabel old summaries.

All nodes emit through the same database sequence/outbox boundary. Same-caller
sessions remain distinct by SessionId and node generation. Reports use one
snapshot and EventId cutoff; an event committed after the cutoff appears in
the next report. Cleanup and report reads use short bounded transactions.
Retention cleanup cannot invalidate a report already holding its snapshot.

Caller/session threads perform only bounded event construction and
transactional insertion. Rendering, large report scans, encoding, and
publication run in cancellable worker tasks with bounded channels and staging.
Backpressure drops only live fan-out delivery and emits a gap marker; it does
not drop committed retained facts or block a caller indefinitely.

## 16. Errors and conventional logs

Operational error events answer: what subsystem failed, when, severity, safe
reason/error code, associated node/session/domain ID where authorized, whether
action is needed, and a correlation ID. Caller output receives a localized
safe error. Stack traces, raw database errors, packets, private data, and host
paths never go to callers or ordinary event views.

The current `tracing` text stream remains a host diagnostic output. It is not
native authority and is not tailed through operator APIs. The default remains
stderr so a service manager may capture it. An optional built-in file sink is
confined to `WORK/diagnostics/`, creates owner-only files where the platform
supports permissions, rotates at 10 MiB, and keeps no more than seven rotated
files or seven days, whichever removes an entry first. Deployment may instead
use journald or another protected host sink with an equivalent documented
retention policy. Diagnostic files are excluded from native backup and never
exposed by arbitrary path. Deep developer diagnostics require host access;
report/event clients receive only the structured redacted projection. A
future bounded importer must not grant filesystem browsing.

## 17. Backup, restore, and recovery

Cold backup at schema 18 preserves:

- retained operational events and daily summaries;
- retention policy;
- open/acknowledged notifications;
- durable operator audit;
- saved report-publication definitions;
- nonterminal and retained report-generation journal rows; and
- current managed DISPLAY/file-area publication artifacts through their
  existing resource/file authority.

It excludes the live ring, subscriber cursors, reconstructable report caches,
temporary rendering/staging, and deployment text logs. Confined operator
exports are artifacts, not board authority, and are excluded unless separately
selected by a future archive workflow.

Restore validates schema/integrity, preserves EventId and GenerationId, clears
the live ring, recovers or fails nonterminal publication journals safely, and
rebuilds live node views from new sessions. It does not manufacture downtime
events. An absent external storage root remains StorageUnavailable according
to the existing schema-17 contract.

## 18. Online/maintenance/offline classification

| Operation | Class | Contract |
|---|---|---|
| Board/node status, recent events, statistics, report preview | ONLINE SAFE | read snapshot, bounded and authorized |
| On-demand report generation to a temporary result | ONLINE SAFE | worker, cancellation, byte/time budget |
| Publish/export report | ONLINE SAFE | versioned definition/destination, staged atomic commit, audit |
| Time grant, page/chat, disconnect | ONLINE SAFE | expected live session generation, dispatch authorization, audit |
| Retention/report/publication policy change | VERSIONED ONLINE | optimistic version, impact preview, audit |
| Ordinary retention cleanup | VERSIONED ONLINE | lease, bounded batches, cutoff fixed before work |
| Deep reconciliation or forced cleanup of inconsistent generation state | MAINTENANCE | plan/dry-run, board service lease, audit |
| Schema migration, restore, database replacement | OFFLINE | exclusive board lock and existing recovery rules |

No ordinary report requires a whole-board lock.

## 19. Localization and documentation

Implementation must add localized labels and results for report titles,
columns, event categories and severity, filters, empty results, paging/gaps,
notification state, safe errors/remediation, time grants/disconnects,
generation/publication/export state, retention impact/confirmation, conflicts,
cancellation, and recovery. Domain code stores stable codes and values, not
English sentences.

Modern, Minimal, and Classic profiles may change framing, columns, colors, and
fallback representation. They cannot change report values, visibility,
authorization, retention, ordering, or publication outcome.

Under the documentation-completion policy, implementation requires:

- **Sysop Manual:** viewing board activity, live nodes, system/daily
  statistics, recent callers, reports/publication, logs/errors, retention, and
  privacy;
- **Caller Guide:** only any new caller-visible published-statistics/report
  behavior and how to interpret it;
- **Technical Reference:** schema 18, event/outbox/summary authority,
  authorization, report snapshots, publication journal, formats, retention,
  backup/recovery, and IPC/API;
- **Contextual help:** `operator.activity`, `operator.nodes`,
  `operator.statistics`, `operator.reports`, `operator.errors`,
  `operator.retention`, and `operator.report-publication`; and
- **Quick Start:** no change. Monitoring/reporting is not necessary to create
  and test a first board.

Human-facing text explains practical effects rather than event envelopes,
projections, idempotency, or retention classes. No full manual chapter is
created by this gate.

## 20. FireComm, graphics, and historical-runtime boundary

No FireComm review was needed. This gate changes no terminal capability,
encoding, emulation, screen-layout algorithm, capture format, or graphics
negotiation. It honors the cross-project policy by keeping report semantics
separate from representation. FireComm remained read-only and untouched.

Sixel, RIP, PETSCII, ATASCII, Amiga/Topaz, ZX Spectrum, Amstrad CPC, and other
platform presentation remain future work. Their eventual representations may
consume a typed report but cannot alter report values or access.

No 86Box run is required before semantic implementation. Controlled original
runtime work may later answer exact CALLERS.LOG fields/escaping, SFLOG output,
SFTOPTEN ties/exclusions, SFUSERS `/S` layout, BBS/CLR control bytes, date/time
formatting, and prompts. Those are exact compatibility evidence, not blockers
for the native semantic contract.

## 21. B-017 implementation acceptance matrix

| Area | Required acceptance |
|---|---|
| Historical semantics | F1-like bounded recent/today/date views, F5-equivalent daily values, notifications and explicit cleanup preserve confirmed stock outcomes without claiming exact text bytes. |
| Event correctness | Every accepted producer emits the right stable code only after commit; failures/cancellation are distinct; replay/retry does not duplicate facts or daily totals. |
| Event schema | Unknown/malformed/oversize attributes fail closed; stable JSONL schema and Rust decoding remain versioned and bounded. |
| Audit distinction | Operational expiry/acknowledgement cannot alter purpose-specific or operator security audit. |
| Privacy | Automated deny-list tests prove secrets, login IDs, real names, contacts, message/questionnaire/chat/file content, endpoints, and host paths never enter normal events/reports. |
| Identity | Views use public handles and stable IDs internally; caller-visible publications honor active/listed/opt-in/cohort policy. |
| Daily statistics | Calls/new callers/messages/transfers/bytes/errors match authoritative domain facts just below/at/after board midnight, DST transitions, and timezone version change. |
| Retention | Defaults/mutations, impact preview, bounded cleanup, concurrent query, crash/restart, and security-audit immunity pass. |
| Notifications | Open/acknowledge/resolve is versioned, stale-safe, audited, and does not mutate source events. |
| Multinode | Concurrent sessions/transfers produce unique EventIds, isolated SessionIds, deterministic views, and exact daily totals. |
| Backup/recovery | Cold backup/new-root restore preserves retained history/policy/summary/notifications/audit and clears only ephemeral live state. |
| Localization/presentation | All operator text is localized; Modern/Minimal/Classic frame the same values and privacy result. |
| Operator journey | A running two-node board shows start/login/activity/transfer/error/logoff, correct Today totals, filtering/paging, notification acknowledgement, restart continuity, and no secret/path disclosure. |
| Exact legacy evidence | Exact CALLERS.LOG/SFLOG field bytes remain explicitly unclaimed and do not block native semantic verification. |

B-017 becomes VERIFIED only when this complete matrix passes.

## 22. B-021 implementation acceptance matrix

| Area | Required acceptance |
|---|---|
| Historical semantics | Status, page availability/chat, security/lifecycle owner commands, ±5-minute equivalent, disconnect with/without notice, and graceful exit/shutdown have accepted modern equivalents; DOS shell/printer/chat capture are explicitly modernized or deferred. |
| Daemon authority | Attached clients use the authenticated local endpoint and typed service; no online SQLite/TOML/status/log access or duplicated node authority exists. |
| Authentication | Protected UDS/named-pipe permissions and peer identity pass; mispermission, stale credential/token, unauthorized loopback, and replay fail closed. |
| Authorization | Every command checks its distinct capability at dispatch; named Sysop, threshold Sysop, and host operator matrices pass. |
| Session safety | Time/disconnect/page/chat commands include SessionId/generation; reused node, ended session, concurrent command, and daemon restart are stale-safe. |
| Time grants | Positive/negative bounds, caller-visible result, repeated CommandId, disconnect, day rollover, and accounting prove no daily/cumulative counter rewrite or double grant. |
| Privacy/audit | Node/event views redact; privileged actions audit semantic target/result without chat, content, contact, secret, endpoint, or path. |
| Console/CLI | Existing foreground console and a noninteractive attach client consume the same typed API and localized errors; console loss does not stop the board. |
| Multinode | Two simultaneous nodes plus two operator clients cannot cross-target, double-act, or corrupt page/session state. |
| Recovery | Endpoint loss/reconnect, daemon shutdown, in-flight read, and stale request unwind boundedly; board authority remains intact. |
| Real journey | A real caller and local attached operator complete status, page/chat, time grant, notice disconnect, reconnect, and clean daemon shutdown. |
| Future boundaries | No observation/spy, scheduler, host shell, sfmonitor/sfconfig TUI, door, or network action is smuggled into B-021. |
| Exact legacy evidence | Exact key/prompt/split-screen bytes remain separate unless needed to explain an accepted semantic result. |

B-021 becomes VERIFIED only when this complete matrix passes.

## 23. B-022 implementation acceptance matrix

| Area | Required acceptance |
|---|---|
| Historical semantics | Screen/disk outcome, activity/statistics/top/file-list publications, and explicit printer modernization are documented against stock versus official-utility evidence. |
| Projection correctness | Terminal/text/JSONL/BBS/CLR outputs represent the same snapshot, values, order, filters, and audience. |
| Authorization/redaction | Private report generation/export/publication is capability-scoped; caller publication applies visibility/opt-in/cohort filters and excludes hidden/private state. |
| Injection safety | ANSI/control, newline, filename, structured-data, and future spreadsheet-formula payloads cannot escape fields, alter terminal state, or create commands/paths. |
| Encoding | UTF-8/CP437 and LF/CRLF choices are explicit; unrepresentable data yields the documented safe result; BBS/CLR controls come only from trusted formatter resources. |
| Confinement | Traversal, absolute paths, symlinks, special files, invalid stems, wrong area/root, and destination races fail closed. |
| Atomic publication | Generation journal, staging, digest/size, sync/rename, CAS, cancellation, disk-full, crash at each phase, restart reconciliation, and no half-current resource pass. |
| Bounds | Interactive paging, 10,000-row export cap, 4/16-MiB budgets, 30-second default, cancellation, and large-source streaming prevent memory/disk/session starvation. |
| File catalog | SFMKLIST-equivalent projection honors area access/exclusions/order and commits the generated item through file authority without recursive self-inclusion or duplicate accounting. |
| Top/recent reports | Deterministic ranks/ties, lifetime/window labels, public-listing policy, cohort floor, changed/deleted caller, and empty/small cohort behavior pass. |
| Localization/presentation | Titles/columns/errors are localized and all profiles preserve values/privacy; no presentation representation becomes authority. |
| Backup/recovery | Definitions, current managed publications, and nonterminal journals restore; caches/staging/text diagnostics do not; interrupted generation recovers safely. |
| Operator/client journey | Local operator previews, exports JSONL/text, publishes BBS/CLR, views from a real BBS client, republishes after data change, and receives a clean conflict/error path. |
| Exact legacy evidence | Exact SFLOG/SFTOPTEN/SFUSERS/SFMKLIST bytes, ties, colors, and RIP remain unclaimed unless later proved. |

B-022 becomes VERIFIED only when this complete matrix passes.

## 24. Implementation slices and exact next action

After this gate is accepted, implementation should remain one Tranche 7 and
proceed in bounded slices:

1. schema-18 migration, typed event/outbox/daily-summary/retention core, and
   backup/recovery;
2. B-017 producer integration, views, notifications, privacy/localization, and
   acceptance;
3. B-021 local attach endpoint, capabilities, time/session commands, CLI/
   foreground-console reuse, and live acceptance; then
4. B-022 projections, safe formatters, atomic publication/export, and complete
   acceptance.

The exact next action is acceptance or revision of this gate. Only after
acceptance should schema-18 and B-017 implementation begin. No other Category-B
tranche, scheduler, operator TUI, graphics/presentation, door, scripting, or
network work is authorized by this document.
