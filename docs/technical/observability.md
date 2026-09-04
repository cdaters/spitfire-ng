# Operator Observability

<!-- help-topic: technical.observability -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)

## Authority boundaries

Schema 18 implements the B-017 observability core. It keeps four concepts
separate:

1. domain state owns callers, messages, files, transfers, storage, sessions,
   and backups;
2. the operational-event ledger records privacy-bounded committed outcomes;
3. purpose-specific security and operator audit remains durable authority;
4. report projections and future publication artifacts are derived results.

Host tracing remains non-authoritative diagnostics. No service parses a text
log or grants an online client direct SQLite access. B-021 will provide the
protected attach/control boundary, and B-022 will own report formatting,
confined export, and publication.

## Schema 18

The transactional 17-to-18 migration adds:

- `operational_events`, an append-only event ledger with monotonic
  `AUTOINCREMENT` IDs;
- `operational_daily_summaries`, keyed by numeric board day and timezone
  policy version;
- `operational_retention_policy`, a versioned singleton initialized to 30
  detail days and 400 summary days;
- `operator_notifications`, a versioned open/acknowledged/resolved projection
  linked to one source event; and
- `operator_observability_audit`, an append-only audit for acknowledgement,
  resolution, retention-policy changes, and cleanup.

Migration validation compares caller, message, file, transfer, storage-root,
and existing audit counts and identifiers before and after. It requires empty
new event, summary, and notification tables, so no pre-schema-18 history is
fabricated. Failure rolls back the migration and leaves schema 17 intact.
Report definitions and generation journals are deliberately absent from this
B-017 slice; B-022 will introduce only the state its publication transaction
requires.

## Event envelope and payloads

An operational event contains UTC time, assigned board day, timezone-policy
version, category, severity, stable event code, outcome, optional safe node,
session, caller, correlation and domain-object identifiers, retention class,
and a versioned typed attribute payload. Current categories are system, node,
session, caller, authentication, message, file, transfer, storage, backup,
operator, and error. Severity is info, notice, warning, error, or critical.

Payloads are a closed Rust enum for session, transfer, message, file, storage,
backup, error, and operator facts. Each text slot is at most 256 bytes; total
stored payload text is at most 768 bytes, stricter than the 2 KiB gate maximum.
Codes and identifiers have independent ASCII/length checks. Unknown database
enum values or malformed payload versions fail decoding. There is no generic
`HashMap<String, String>` or arbitrary blob.

`OperationalEvent::to_json_line` exposes the stable
`spitfire-operational-event/v1` machine record for future clients. It returns
one bounded in-memory record only; it does not select a host path, write an
export, or publish a report.

## Producer and transaction rules

Durable mutations insert their event in the owning SQLite transaction. Caller
creation, session accounting, message fan-out, transfer terminal state,
per-item upload/download settlement, and storage availability therefore
cannot commit without the associated accepted event. Stable mutation IDs make
replayed message and transfer facts idempotent. Pure runtime facts use the
same service writer.

The current producers cover runtime start, authentication denial, caller
creation, session start/completion, message posting, transfer completion,
failure/cancellation and successful item settlement, storage unavailability,
and cold-backup start/completion/failure. They record milestones, not blocks,
bytes, keystrokes, renders, or polling cycles.

Events sort by `(occurred_at_utc DESC, EventId DESC)`. An initial page captures
the maximum EventId; its cursor carries that snapshot cutoff, time, and final
ID. Later inserts—including back-dated inserts—cannot appear midway through
that cursor sequence. The default is 100 records, the maximum is 500, and a
bounded detail request may span at most 31 days.

## Privacy and authorization

Typed attributes cannot accept passwords, password hashes, login identifiers,
private names or contacts, birth dates, message subjects/bodies/recipients,
file/DIZ contents, terminal input, keys, credentials, packets, remote
endpoints, or host paths. Authentication failures record a reason class and
transport without the supplied identifier. Message activity stores conference
ID, public/private class, and count only. Transfers store stable object IDs,
direction/protocol/count/bytes and outcome.

`OperatorObservabilityContext` carries explicit capabilities for board
statistics, node status, events, caller activity, notifications, maintenance,
and notification acknowledgement. Host operator and named Sysop presets are
distinct. A caller merely meeting the Sysop security threshold receives no
host capability implicitly. The B-021 attach transport must establish and
reauthorize this context; it must not expose SQLite.

## Live ring and durable queries

`ObservabilityService` maintains at most 2,048 events and approximately 15
minutes in memory. It is an acceleration layer, not authority, and starts
fresh on daemon restart. Refresh reads committed ledger records after the
ring's last EventId. Capacity, horizon, or ID discontinuity sets an explicit
gap flag so a client can resume through durable paging rather than assume it
saw everything. Each in-memory subscriber queue is capped at 256 events and
uses the same explicit gap result after overflow.

The monitor service surface includes board status, node list,
recent/live events, notifications, system/today statistics, recent callers,
one-caller activity, message activity, transfer activity, recent errors, and
maintenance status. File mutations have a separately bounded event view.
Message/transfer aggregation is bounded to a 31-day input window and 500
result groups.

## Daily summaries and retention

Each committed event updates one summary row in the same transaction. Typed
columns count calls started/completed, new callers, posted message deliveries,
successful uploads/downloads and bytes, transfer failures/cancellations, and
warning/error/critical events. Rebuild uses one transactional upsert over the
retained ledger and is idempotent.

Board-day assignment reuses the schema-16/17 timezone authority. UTC facts
retain the timezone-policy version in force when accepted. Ordinary midnight,
DST gaps, repeated hours, non-DST zones, and later policy changes therefore do
not relabel or double-count committed facts. Summary-retention cutoffs subtract
calendar days in board-local civil time; detailed-event cutoffs use UTC.

Cleanup deletes at most 500 rows from each eligible class per transaction and
reports whether more work remains. Open notifications keep their source event.
Security/domain audit is never selected. Shortening retention requires the
caller to present the exact current `RetentionImpact`; the policy update uses
optimistic versioning and writes operator audit.

## Notifications and maintenance

Backup failure, storage unavailability, node fault, and error/critical events
can open one notification linked to the source EventId. Acknowledgement and
resolution update only notification state, require the expected version, and
append success or conflict audit. They never mutate the source event.

Maintenance combines open notifications, warning/error counts in the last 24
hours, unavailable storage roots, pending-review files, and nonterminal
transfers. `BoardStatus` combines this durable projection with current
NodeManager state and uptime. `LiveNodeStatus` omits login/private identity,
remote endpoint, terminal bytes, and content.

## Backup, restore, and restart

Cold backup preserves the schema-18 ledger, summaries, policy, notifications,
and operator-observability audit in the consistent SQLite snapshot. Backup
start is inside that snapshot; completion or failure is recorded on the live
source after its outcome is known. A later backup therefore retains the prior
outcome. Restore preserves EventIds and SQLite sequence state, and subsequent
events continue above the restored maximum. The live ring is excluded and
starts empty.

A schema-17 backup restores exactly at 17; normal writable startup performs
the single 17-to-18 migration and begins with honest empty activity history.
Interrupted inserts, summary updates, acknowledgement, and retention batches
use SQLite transactions. No half-written event or summary becomes authority.

## Presentation and localization

The en-US 1.12.0 catalog contains operator activity, category/severity/outcome,
statistics, notification/remediation, maintenance, pagination, and retention
language. Modern 1.5.0, Minimal 1.5.0, and Classic 1.6.0 presentation packages
remain unchanged: profiles may frame values differently but cannot alter
counts, privacy, authorization, retention, or notification state.

See the [Sysop explanation](../manual/board-activity.md), the [Tranche 7
gate](../research/m039-tranche-7-operator-observability-reports-gate.md), and
the [B-017 implementation report](../research/m039-tranche-7-b017-observability-implementation.md).
