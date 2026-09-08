# M068 — C5 Events and CircuitNET operations

**C5 COMPLETE / ACCEPTED / PUBLISHED. C6 NOT STARTED.**
This independently authored public summary contains no proprietary historical
material, private archaeology, acceptance credentials or production configuration.

## Implemented authority

Schema **31 → 32**, operator IPC **14 → 15**, CircuitNET wire **1.2 unchanged**.
en-US **1.29.0 / 1,382 messages**. Native messages remain canonical. A durable
preparation generation is recorded with eligible native activity; a daemon worker
uses existing CircuitNET/FTN scanners independently from exchange scheduling.

One generic Event worker invokes typed CircuitNET profile/neighbor/all-neighbor
Poll and BinkP link Poll. Definitions, versions, due times and bounded history are
durable. Manual, interval, daily and selected-day schedules use an explicit IANA
zone. DST gaps skip and repeated local times run once. Missed slots either catch
up once or skip; no accumulated week of polls is replayed. Run Now coalesces.
Enabled target ownership rejects overlapping Events. The daemon board lock and
shutdown admission fence prevent duplicate schedulers and new shutdown work.
Abandoned runs recover as interrupted, including after cold restore.

Immediate policy reacts to eligible target work with minimum spacing; Scheduled
waits for recurrence; Manual requires Run Now/operator Poll; Hybrid combines prompt
work and recurring catch-up. Existing boards receive no unsolicited automatic
Event. Successful bounded batches can continue draining, while failures and empty
sessions with unsupported work do not create a competing retry loop. Existing
native link permits, TLS/authentication, queue backoff and delivery receipts remain
authoritative. Native directed posting commits destination intent atomically.

QWK retains build/ingest and confirmed handoff; there is no invented live QWK Poll.
Backup remains cold until a consistent online snapshot API exists. Startup/once-only
schedules, shell hooks and other maintenance actions remain future work.

The [manual](../manual/events.md) and [technical contract](../technical/events.md)
cover commands, clock/retry semantics, recovery and limits. sfconfig provides
list/save/edit/enable/disable/run/history; sfmonitor Networks E shows next/running/
last result and permitted Run Now. CircuitNET peer details include next exchange;
`why` and the role guides explain mapping, Dossiers, routes, hold, health and timing.

## Historical and modern documents

[Historical research](m067-spitfire-events-network-scheduling.md) informed the
scheduler without recreating DOS internals. The dated official historical catalog
already contained all 23 additions in the retained change notice. Reconciliation
therefore yields **60 active conferences**, plus **5 deleted and 6 rejected**
records. The private evidence remains separate from modernization.

The [modern proposal](../circuitnet-ng/CONFERENCES.md) classifies all 71 historical
records and offers **46 proposed definitions**, including two explicitly new areas.
The machine-readable catalog contains no local SPITFIRE conference numbers and
cannot automatically adopt policy or create conferences. Original modern
[Charter](../circuitnet-ng/CHARTER.md), [rules](../circuitnet-ng/RULES.md),
[security](../circuitnet-ng/SECURITY.md), joining and END/HOST/ROOT guides form a
[future distribution outline](../circuitnet-ng/README.md), not a final ZIP release.
All governance choices remain review drafts; no governance automation is added.

[Future Files obligations](../technical/files-future.md) cover archive formats,
validation, malicious archive limits, original-byte preservation, FILE_ID.DIZ and
branding, hashing, scanners, quarantine and rescans as shared SPITFIRE infrastructure.
C5 does not implement CircuitNET file networking or investigate ReComment.

## Validation and limits

The disposable Darwin arm64 C5 campaign covers scheduled/manual/immediate/Hybrid
exchange, immediate queue preparation, a 50-message burst, held/offline recovery,
directed HOST transit, remote Dossier approval/unsubscribe, restart, Run Now receipt
replay and cold backup/restore. The existing BinkP daemon journey now begins with a
scheduled Event. C4's six-node and C3 live/C2 offline campaigns remain regressions,
as do FTN/FileEcho/FREQ and both QWK workflows. Reproduce with `RUST_TEST_THREADS=1 cargo test --workspace`;
loopback listener permission is required. Optional evidence retention stays outside
public source. No generated keys, boards, packets, backups or logs are published.

Public workspace: **734 passed / 0 failed / 7 existing ignored**, all six doctest
groups. **14 tests added.** All 133 headers, fmt, all-target Clippy with warnings
denied, diff and provenance pass. Local documentation validation covers **142
Markdown files / 1,015 links**, zero issues. The complete public workspace runs the
C5 and six-node C4 campaigns, C2/C3, FTN/FileEcho/FREQ, QWK, migration and restore
regressions. Serial execution avoids competing localhost setup during validation;
production transport deadlines and wire assertions remain unchanged.

The sanitized public delta contains **57 paths (20 added / 37 updated)**. The 27
changed source paths match the accepted implementation. Public documentation and
banner/history are preserved, with separate rights-safe research summaries.
cargo-audit is unavailable; no audit success is claimed.

C1 remains historical authority; C2 native/offline foundation; C3 encrypted and
authenticated transport; C4 directed routing and remote Dossiers. C5 adds generic
Events. CircuitNET remains independent of FTN/QWK/BinkP and each retains its own
native authority. Encrypted transport is not an encrypted-message claim. Directed
conference messages are not private mail; delivery follows BBS conference access
rules. No E2EE, private-mail addition, legacy CNP/CND, CircuitNET files, governance
automation, production changes or external BBS polling occurred. Stop after C5;
review this milestone and the draft network documents before any C6 work.
