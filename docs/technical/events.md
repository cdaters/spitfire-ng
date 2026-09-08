# Native SPITFIRE NG Events

C5 implementation contract. Events schedule work; the operational event ledger
records facts. Native messages, networking queues and protocol receipts retain
their existing authority. Historical rationale is in [M067](../research/m067-spitfire-events-network-scheduling.md).

## Interfaces defined before implementation

`sf-core::events` owns validated durable definitions, next-run calculation, atomic
claims, coalesced Run Now, completion history and restart recovery. `sf-bbs::events`
owns a single daemon worker and invokes typed existing services. Operator clients
use protected IPC and existing network-read, network-run and sensitive-config
capabilities. They do not run a scheduler or write live SQLite.

Initial actions: CircuitNET exchange for a profile and one/all eligible neighbors;
BinkP Poll for an existing link. QWK build/ingest remains an explicit preparation
and custody workflow; there is no live QWK connection API to wrap. Online Backup,
external hooks, file maintenance and other future actions are deferred until their
typed service and consistency boundaries exist. No arbitrary shell action.

Schedules: manual, fixed elapsed interval, daily wall time, selected weekdays.
Each definition stores an explicit IANA timezone; operators use their board-local
timezone unless deliberately choosing another. Spring-forward nonexistent wall times are skipped; fall-back ambiguous
times use the first occurrence only. Recurrence advances beyond the last claimed
slot, preventing clock rollback from duplicating that slot. Intervals use persisted
UTC due times. Wakeups use bounded waits, not one-second busy polling.

Missed policy defaults to one catch-up, never a backlog. Skip-missed advances to
the next future slot. Run Now coalesces to one pending request; disabled Events
do not run. A claim advances next due before action execution. Restart records an
interrupted result and clears running authority; restore cannot resurrect a lease.
The daemon board lock prevents another scheduler process. Transport permits still
prevent overlapping sessions with operator/inbound work.

Exchange policy is part of an Event definition: Manual permits only Run Now;
Scheduled invokes recurrence; Immediate responds to durable queue activity;
Hybrid combines activity with recurring catch-up. Existing boards have no implicit
Events and therefore retain manual outbound behavior. New operator-created network
Events default to Hybrid only when explicitly configured. Minimum exchange spacing
and coalescing bound bursts; protocol/session retry and queue eligibility remain in
the existing network service. An Event reports completion, not merely admission.

Publication preparation runs independently of Event due times. Native commits
record durable preparation work; the daemon consumes it promptly using the existing
scanners. This closes the crash window between message commit and publication and
does not require an Event to discover mail. Mapping, identity, Dossier, hold and
retry rules remain authoritative. Directed intent must be fixed before publication;
operator changes to already-published destinations remain prohibited.

History contains finite result classes, timestamps, duration and target counts,
never message bodies, endpoints, credentials or private caller identities.
Definitions and history are backed up with the native database; network restore
holds remain effective. Transport encryption continues to protect links in transit;
it does not make conference messages end-to-end encrypted or directed mail private.

## Implemented bounds and integration details

Schema 32 stores at most 128 definitions. History retains the latest 4,096 completed
runs plus the current run, and returns at most 50 per Event. Every run snapshots its
typed action so later edits cannot rewrite historical target meaning. Definition
edits use versions and are rejected while running. The initial concurrency policy
is one serial Event worker, with coalesced Run Now and existing transport permits.
One enabled Event owns a concrete target; overlapping all-peer/single-peer targets
are rejected, preventing schedule multiplication. No attempt is made to parallelize
separate Event actions in C5.

A native public-message INSERT transaction increments the durable generic
preparation generation. Native message queue/control and existing FTN FileEcho/FREQ delivery creation
also advances activity for
prompt exchange. A separate daemon preparation worker consumes generations using
existing CircuitNET and FTN scanners; it checks every two seconds, independently of
an in-flight exchange. Scanning continues through bounded pages, including past
ineligible historical rows, with shutdown checked between pages. The scheduler uses a condition-variable wakeup with a maximum
five-second wait. These are two responsibilities of the one generic daemon service,
not protocol-specific recurring pollers. Invalid/capacity-limited preparation keeps
its durable obligation; no message is removed. Native identity and mapping checks
remain in the scanner. QWK build/ingest remains manual because packet preparation
and confirmed external custody are its current accepted operator boundary.

Immediate/Hybrid activity is filtered to work for the Event target and coalesces
until minimum spacing permits another run. Unrelated native activity cannot cause
empty polls to every configured link. A
successful finite session with remaining message batches rearms activity, so a
large burst can drain in bounded sessions. A failed session does not rearm a competing
retry loop. Unsupported directed work left by an otherwise successful empty older-peer
session reports Held instead of endlessly rearming. Queue eligibility/backoff remains
a prerequisite. CircuitNET retains its three bounded transient attempts; BinkP retains
its queue/link backoff. Hybrid/Scheduled recurrence invokes the same service later.
Manual Polls and incoming sessions share their existing permits with Event actions.

CircuitNET sessions retain 10-second I/O and 120-second session bounds, including
shutdown cancellation. BinkP retains its existing cancellation and deadline model.
C5 does not expose arbitrary per-Event timeout overrides or invent unkillable worker
threads. Run duration is derivable from persisted start/completion times. Cold
backup includes definitions and history; restore and daemon startup both clear
abandoned run authority and record Interrupted. Held networking work stays held.

Operator IPC minor 15 adds the bounded Events action feature. Older clients keep
existing features and ignore additive snapshot fields; they cannot dispatch Events
without negotiated support. CircuitNET wire protocol remains **1.2**, unchanged.

The new `post_directed_circuitnet` native API records destination intent in the same
transaction as the public message. This prevents background preparation from racing
a two-step destination selection. It does not introduce private mail or a second
message authority. The older live operator selection remains valid before publication; `stage-direct`
provides the same selection under the exclusive cold-board lock.
