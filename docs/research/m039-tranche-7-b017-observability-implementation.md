# M039 Tranche 7 — Schema 18 and B-017 Observability Implementation

**Date:** 2026-09-03

**Published scope:** schema 18 plus B-017 only

**Canonical gate:** [Operator Observability and Reports](m039-tranche-7-operator-observability-reports-gate.md)

## Result

Schema 18 implements a retained, privacy-bounded operational event ledger,
board-day summaries, versioned retention, actionable notifications, separate
operator-observability audit, and bounded daemon-owned projections. B-017's
accepted native semantic matrix is verified. B-021 remains PARTIAL and B-022
remains NOT STARTED: there is no attach endpoint, `sfmonitor`, `sfconfig`,
general report engine, export destination, publication transaction, or
printer path in this change.

The implementation preserves four authorities:

- domain state remains the fact owner;
- operational events describe safe committed outcomes;
- security and operator audit remains purpose-specific and durable; and
- reports/publications remain derived and are not implemented here.

No CALLERS.LOG-shaped text file is native authority. Exact legacy log/report
bytes remain unclaimed.

## Historical and secondary evidence used

The gate's focused primary-source review remains authoritative: SPITFIRE.DOC
documents CALLERS.LOG views, daily statistics, Sysop status/maintenance, and
screen/disk/printer outcomes. Official SFLOG, SFTOPTEN, SFUSERS `/S`, and
SFMKLIST establish Buffalo Creek utility workflows for derived bulletins,
rankings, caller statistics, and file lists. They do not turn utility input or
generated `.BBS`/`.CLR` files into current authority.

The existing public [Synchronet reference](synchronet-reference.md) and a
bounded reread of the official [Synchronet utilities
index](https://wiki.synchro.net/util%3Aindex) were used only as secondary
engineering references. The useful lesson was the product-level separation of
configuration, node/status monitoring, statistics/log viewing, and repair
utilities. SPITFIRE NG adapted that separation into future `spitfire setup`,
`sfconfig`, `sfmonitor`, and common CLI clients of one typed daemon service.
It rejected direct utility ownership of counters, direct database/log parsing,
and Synchronet-specific terminology/data models. Synchronet is not historical
SPITFIRE authority.

FireComm was not reviewed because this slice changes no terminal rendering,
encoding, screen layout, or client capability contract.

## Schema 17 to 18

The migration is one SQLite transaction. A validation snapshot records caller,
message, file, transfer, storage-root, and existing purpose-specific audit
counts/identifier sums before creating new tables. Post-migration validation
requires those facts unchanged and the event, summary, and notification tables
empty. A failure-injected invalid retention default rolls back to an unchanged
schema 17 database.

New native state is deliberately limited to B-017:

| Table | Purpose |
|---|---|
| `operational_events` | Append-only retained semantic outcomes with monotonic EventId |
| `operational_daily_summaries` | Durable typed board-day/timezone-version totals |
| `operational_retention_policy` | Versioned 30-day detail / 400-day summary policy |
| `operator_notifications` | Open/acknowledged/resolved action-needed state linked to an event |
| `operator_observability_audit` | Append-only audit for acknowledgement, resolution, retention changes, and cleanup |

Saved report definitions and report-generation journals are deferred to
B-022 because B-017 neither publishes nor exports reports. The schema creates
no fabricated pre-18 calls, transfers, messages, errors, daily totals, or
notifications. Existing lifetime counters remain usable with an explicit
activation-time boundary for detailed history.

## Operational event contract

`EventId` is a SQLite `AUTOINCREMENT` identity so retention cannot cause reuse.
Each event stores UTC time, numeric board day, timezone-policy version,
category, severity, stable code, outcome, optional safe correlations, one
typed payload kind/version, and a retention class. Current categories are
system, node, session, caller, authentication, message, file, transfer,
storage, backup, operator, and error. Future scheduler, door, network, QWK,
FidoNet, CircuitNet, and scripting categories are not emitted.

Payloads are a closed enum. Each string is at most 256 bytes, all control
characters are rejected, and total payload text is at most 768 bytes. Codes,
correlations, object identifiers, and operator IDs have separate bounds.
Unknown database values and malformed payload versions fail closed. The stable
`spitfire-operational-event/v1` JSON Lines encoder is an in-memory record
boundary only; no B-022 file export was added.

Events for durable message, file, transfer, caller, session-accounting, and
storage mutations are inserted in the owner transaction. Message fan-out IDs,
file-operation IDs, transfer/item IDs, caller IDs, and storage versions supply
stable idempotency where the domain operation itself can be replayed. Runtime
startup, authentication denial, and backup outcome use the shared event writer
without placing credentials, paths, or arbitrary error prose in the payload.

No event is generated per block, byte, keystroke, render, or poll.

## Privacy and audit

The event type system has no generic string map or content blob. Tests inspect
stored/debug projections for passwords, supplied login data, private message
subject/body/recipient, private identity, fixture root, and absolute
home-directory path fragments. Authentication failures retain only node/session correlation,
transport family, and reason class. Recent callers retain the public handle;
ordinary events use stable IDs.

Security audit tables remain separate. Operational cleanup never selects
them. A notification acknowledgement updates only optimistic notification
state and appends an observability-audit success/conflict; it cannot update the
source event. Simultaneous acknowledgement commits exactly once.

Explicit capabilities independently protect board statistics, node status,
events, caller activity, notifications, maintenance, and acknowledgement.
Named Sysop and host-operator presets differ, and no security-level threshold
implicitly grants host capability. Establishing a protected local attach
identity remains B-021.

## Summaries, time, and retention

The owner event transaction incrementally updates typed counters for calls,
new callers, posted message deliveries, successful upload/download items and
bytes, transfer failures/cancellations, and warning/error/critical outcomes.
The rebuild operation recomputes one board-day/timezone-version row from the
ledger with a transactional upsert and is idempotent.

Day assignment reuses the accepted board timezone authority. Tests cover
ordinary midnight, New York's skipped and repeated hours, Phoenix's non-DST
behavior, and a timezone-policy version change. Retention of summary rows uses
board-local calendar-day arithmetic; detailed event expiry uses UTC.

The live ring holds at most 2,048 events and approximately 15 minutes. It is
memory-only and starts empty after daemon restart/restore. A capacity, horizon,
or ID gap is explicit so a future client can resume from durable history.
Each live subscriber queue is independently limited to 256 events and reports
overflow as a gap rather than growing without bound.

Durable detail defaults to 30 days and summaries to 400 days. Policy values
are bounded to 1–365 and 31–3,650 days. Cleanup handles at most 500 eligible
notifications/events/summaries per transaction and reports more work. Open
notifications retain their source. Shortening retention requires a matching
current impact preview, then a versioned audited update.

## Read models

The B-017 service surface now supplies:

- board runtime/health status;
- privacy-safe node status;
- stable-cursor recent events and a live batch with gap indication;
- open or historical notifications and host-authorized acknowledgement;
- Today, lifetime, and live-now statistics boundaries;
- recent callers and authorized one-caller activity;
- message activity grouped by conference and public/private class;
- file additions/moves/removals through a bounded event projection;
- transfer activity grouped by protocol, direction, and outcome;
- recent error events; and
- maintenance status combining notifications, warnings/errors, storage,
  PendingReview, and nonterminal transfers.

Event paging defaults to 100 and rejects values outside 1–500. Detail and
aggregate ranges are limited to 31 days. A cursor contains the captured maximum
EventId plus the last `(time, EventId)` ordering key, so later or back-dated
inserts cannot destabilize the snapshot.

## Backup and recovery

Cold backup's consistent SQLite image retains events, summaries, policy,
notifications, audit, and SQLite sequence state. A backup-start event enters
the snapshot; completion or safe failure is recorded after the outcome.
Backup failure opens a localized actionable notification without recording the
destination or error path.

New-root restore preserves durable state and continues EventId allocation
above the restored maximum. Its live ring starts empty. A schema-17 backup
restores exactly as schema 17; one subsequent normal writable migration creates
empty schema-18 observability state. The implementation tests both paths.

## Localization and documentation

The en-US package advances from 1.8.0 to 1.9.0 and from 526 to 613 semantic
messages. New messages cover event categories/severity/outcomes, activity
filters/results, statistics, notifications/remediation, maintenance, history
activation, pagination, and retention. Modern 1.5.0, Minimal 1.5.0, and Classic
1.6.0 do not change; presentation cannot change event facts, privacy,
authorization, counts, or retention.

The [Sysop Manual](../manual/board-activity.md) explains the operational
behavior and current UI boundary in ordinary language. The [Technical
Reference](../technical/observability.md) documents schema, transactions,
queries, privacy, and recovery. Stable topics are `operator.activity`,
`operator.nodes`, `operator.statistics`, `operator.errors`, and
`operator.retention`. B-017 adds no caller-facing report and does not alter
first-run operation. The Caller Guide and Quick Start receive only the
source-current schema-18 applicability correction; the Quick Start journey is
unchanged.

## Acceptance matrix

| Area | Accepted result |
|---|---|
| Historical semantics | Native recent/today/date/status outcomes map to confirmed F1/F5 semantics; exact bytes remain unclaimed. |
| Event correctness | Owner transactions, idempotent mutation IDs, distinct completion/failure/cancel, summary rebuild, and retry tests pass. |
| Event schema | Closed payloads, bounds, migration constraints, Rust decoding, and versioned JSONL record pass. |
| Audit separation | Retention/acknowledgement cannot alter security or domain audit. |
| Privacy | Deny-list tests cover credentials, login/private identity, message/file content, endpoints, and host paths. |
| Identity | Operator views use stable ID and public handle only; caller publication remains B-022. |
| Daily statistics | Domain facts, midnight, DST skip/repeat, non-DST zone, and policy-version tests pass. |
| Retention | Defaults, bounds, impact confirmation, optimistic mutation, 500-row cleanup, restart, and audit immunity pass. |
| Notifications | Open/acknowledged/resolved state is linked, versioned, stale-safe, and audited; concurrent acknowledgement commits once. |
| Multinode | Two concurrent event writers and two real RAW caller sessions produce unique IDs, isolated sessions/nodes, exact calls and downloads. |
| Backup/recovery | Schema-18 new-root restore, sequence continuation, schema-17 restore/migrate, backup failure, and empty live ring pass. |
| Localization/presentation | Complete en-US 1.9.0 catalog; all profiles retain one data/privacy result. |
| Operator journey | The two-node real session/download journey plus real message workflow and safe synthetic warning/backup paths exercise ordering, filters, statistics, maintenance, notification and privacy projections. |
| Exact legacy evidence | CALLERS.LOG/SFLOG/SFTOPTEN bytes, ties, colors, and prompt trivia remain explicitly deferred and do not block native semantics. |

No 86Box run is required before B-017 promotion. A future controlled original
runtime check may refine exact CALLERS.LOG fields or utility formatting for a
compatibility export, which belongs to B-022 evidence.

## Scope and next action

B-021 code did not begin beyond the narrow B-017-required notification
acknowledgement and reusable read projections. B-022 did not begin. There is no
`sfmonitor`, `sfconfig`, scheduler, report publication engine, networking,
door, scripting, graphics, presentation-platform, or FireComm change.

Schema 18 and VERIFIED B-017 are published in current source. The next separate
work item is the B-021 local/Sysop operator-controls architecture/interface
gate. B-022 remains outside that work.
