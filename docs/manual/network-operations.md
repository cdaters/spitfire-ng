# Operating networking

Current source: N5 operates the accepted QWK/DOVE-compatible, FTN and BinkP
engine. Native SPITFIRE messages remain authoritative. These instructions are for
controlled networks; live public FidoNet participation has not been claimed.
See [QWK networking](qwk-networking.md), [FTN core](ftn-core.md) and
[BinkP](binkp.md) for mail semantics and link prerequisites.

## Start with Networks in sfmonitor

Attach to your running board with sfmonitor and use Tab to select **Networks**.
The operator needs explicit Network status permission. Bootstrap remains read-only;
mutation permissions must be enrolled deliberately in sfconfig. sfmonitor requests
work from the daemon; closing it does not stop the board or a started poll.

| Section | What it answers |
|---|---|
| 1 Overview | Configured QWK/FTN links, listener policy, queue counts, quarantine and active directory counts, compact durable counters |
| 2 Links | Local/remote identity, enabled direction, endpoint source, credential status, current/held state, last attempt/success/error, latency and last authenticated addresses/capabilities |
| 3 Queues | Adapter, link, NetMail/EchoMail kind, state and attempts; details distinguish final destination from routing next hop |
| 4 Areas | Native conference mappings for QWK/DOVE and FTN EchoMail; private NetMail is never an area |
| 5 Directory | Active/candidate generations, dates, source format/priority, record/issues counts and staleness; local node/point lookup |
| 6 Quarantine | Bounded safe summaries of retained errors, without body, subject or private artifact path |
| 7 Recovery | Per-domain/AKA serial floors and origination holds; stopped-board recovery guidance |

Use Up/Down to select, Enter to open details, F to refresh, and `[` / `]` to
page long collections. Directory **L** accepts a complete address such as
`10:100/1.3@isolated`; the domain is required. A point stays associated with its
boss. A missing entry is reported explicitly. No external directory is queried.
Timestamps are explicit UTC. Last success is evidence of a prior session, not a
promise that an idle peer is reachable now. Configured listener policy takes
effect according to its restart classification; it is not a live socket probe.

## Test, poll and recover queues

Select an FTN link and press **T** for Test Link. Review the named target and
confirm with Enter, or cancel with Esc. Test resolves the configured endpoint,
connects, authenticates and validates presented addresses. It does not take mail
custody or consume the queue. Last authenticated addresses remain historical
observations after a rejected identity; they are not the rejected peer's identity.

**P** requests a bounded Poll Link session. The daemon scans/builds eligible N3
work, exchanges complete packets and updates queue/health authority. A request
being started is not delivery. Inspect the resulting link status and Queues;
**accepted** means the peer acknowledged custody. Independent semantic duplicate
suppression remains necessary when an acknowledgement was lost.

Select a queue item and press **H** to hold it. **R** releases/retries eligible
held or failed work through the existing versioned service. Both require explicit
queue permission and confirmation. Accepted work is never resent by Release.
Frozen routing/message/mapping policy is rechecked; a stale policy, exhausted
retry limit or recovery hold cannot be bypassed by editing a state field.
A queue hold affects that item; it is not an undocumented blanket link toggle.
Disable a link in sfconfig when its configured traffic must stop.

**S** requests the existing FTN Scan operation. Directory **A** activates only a
selected validated, inactive generation with explicit directory-activation
permission. There is no generic scheduler, arbitrary packet-path command or
remote filesystem import. QWK handoff retains the established controlled manual
workflow documented in the QWK manual; N5 does not add a QWK transport.

| Symptom | Safe next step |
|---|---|
| Endpoint missing/unavailable | Check explicit override first, then enabled local directory resolution and its INET/BinkP advertisement. Do not guess hosts from system names. |
| Authentication failed | Check Missing/Configured/Invalid status; replace the write-only credential and Test before Poll. CRAM authenticates but does not encrypt mail. |
| Remote address mismatch | Compare configured domain, node/point and permitted AKAs with the controlled peer. Changing a TCP host never authorizes another identity. |
| Timeout/interruption | Check the peer and bounded retry/held state. Partial input is not tossed; only acknowledged output is accepted. |
| Queue held | Inspect reason, route and recovery state. Release performs policy checks rather than forcing delivery. A successful authenticated Test may clear a failed-link hold. |
| Stale directory/conflict | Review generation/source provenance. Correct the controlled source and follow the existing validated-generation workflow. Parsing/priority profile corrections use a new source ID. |
| Quarantine | Inspect the finite reason and fix the link, area or source configuration. Quarantine is read-only in N5; do not delete or edit packet files as a repair shortcut. |

Activity, Notifications and Maintenance retain their existing roles. Networks
adds networking detail over the same authority; it does not parse a separate log.
No queue/quarantine/link display includes private QWK or NetMail content.

## Configure networks in sfconfig

Open **System Configuration** from sfmonitor or start sfconfig directly. Save or
cancel an existing general draft before entering the ninth **Networks** section.
The menu provides FTN policy, BinkP policy, QWK/DOVE partners and conference maps,
EchoMail maps, credential status/update, and stopped-board recovery.

Forms show named fields, not TOML or JSON. Enter opens a record, edits a value or
toggles a boolean. In a list, **A** adds a disabled/default draft entry and **D**
stages removal. Esc goes up one level. **S** opens a complete change review;
Enter commits that reviewed operation. **Q** closes a changed form only after
confirmation. A stale revision is rejected while retaining the draft: close it,
confirm discard if appropriate, then **R** at the Networks menu reloads authority.
Two clients cannot silently overwrite each other's network configuration.

| Form | Configuration authority and checks |
|---|---|
| FTN / Local AKAs and domains | Typed address includes zone/net/node/point/domain; multiple AKAs and domains are supported. Primary/enabled state is explicit. Referenced identities cannot be removed or reinterpreted. |
| FTN / Links | Stable link ID, local AKA, remote endpoint identity, directions, ingress/transit policy and packet/charset profile |
| FTN / Routes | Exact, direct configured link, configured boss, net, zone, then domain default/uplink under the N3 documented precedence. Route type selects its actual typed fields. |
| FTN / Directory sources | Nodelist/Boss/combined pointlist, domain/default zone, encoding, priority, cadence, checksum and enabled state. No automatic downloading. |
| BinkP links | FTN link ID, explicit endpoint/port or local-directory permission, enabled directions, allowed local/remote AKAs and authentication policy |
| BinkP listener/policy | Enabled, bind/port, admitted AKA set and bounded retry policy; shared validation rejects invalid/conflicting listener configuration |
| QWK/DOVE partner | System IDs, namespace, hub/node role, QWK or DOVE header profile, enable/directions and explicit conference mappings |
| EchoMail mapping | AREA/domain, native conference, local AKA, send/receive, origin presentation and configured links |

Native conference choices show ID, name and conference number while editing the
conference field. Up/Down and Page keys browse all local conferences by name; Enter
confirms the selection, so looking up or typing a database ID is not required. The form does not infer a mapping from a display name. Static
FTN/BinkP edits use the aggregate configuration revision; QWK partner/mapping and
EchoMail mapping saves use their own relational CAS. They are separate reviews,
not a fictional cross-database transaction. Static link changes affect subsequent
work and invalidate stale sessions; listener changes require daemon restart.
Review the displayed effect classification before saving.

Credential entries show only **Missing**, **Configured** or **Invalid**. Enter
opens a separate masked update; blank does not clear. **C** on a credential entry
requires the explicit word `CLEAR`. No credential enters the generic draft,
change review, audit, status projection or screenshot. Offline credential changes
are not offered; use the running daemon's protected credential operation.

## Restore without reusing FTN identities

Use the normal [stopped-board backup/restore](../operator/backup-restore.md)
procedure. A backup cannot contain traffic sent after it was taken. N5 reconciles
that gap only from verified surviving native authority; it never guesses a
serial floor from a clock or random number.

When replacing the same stopped board, restore captures the newer board's
per-AKA serial floors and peer-acceptance evidence before replacement. It applies
them to the validated restored copy transactionally. Matching frozen queue
artifacts that were accepted later remain accepted. Uncertain work remains held.
The listener has no restored live session, and the new daemon has a new generation.

For a restore into another root, stop both boards. Open the restored board with
`sfconfig --board <restored-config> --offline`. In Networks select **Recovery →
Transfer from surviving stopped board**, supply the original configuration path,
and review the instruction before typing `TRANSFER`. This is a privileged local
recovery path, not a remotely supplied packet path. Both boards must have the same
native board identity and be exclusively available. The source is held and its
FTN/BinkP origination is disabled before verified evidence is applied to the target.
Do not run or reactivate the retired source as a clone of the restored identity.

If the surviving source is missing, already held/uncertain or not the same board,
origination remains held. Do not clear counters with SQL. A genuinely new AKA is a
separate deliberate identity decision. Newly originated messages after successful
reconciliation use the retained next serial and cannot reuse the known later
MSGIDs. Directory generations, source priority, duplicate history and native mail
otherwise remain the selected snapshot. Lost post-backup inbound history cannot
be reconstructed automatically, and a queue without matching frozen artifact
proof is not silently declared accepted.

N5 recovery bounds are 1,024 retained origin identities and 10,000 accepted FTN
artifact evidence rows. Excess evidence fails safely before replacement rather
than being truncated. Back up the reconciled board before resuming regular work.
The [technical recovery contract](../technical/network-operations.md) explains
matching, source retirement and failure boundaries.

## Current limits

Quarantine reprocess/discard and external NodelistDB intelligence are deferred.
Link health retains current/last state rather than an unbounded history. Caller
NetMail composition remains the N3 service boundary. AreaFix/hub subscriptions,
FileEcho/TIC/FREQ, general scheduling and CircuitNET remain future work.
Actual Windows terminal/network/restore acceptance is **DEFERRED — REAL WINDOWS
ENVIRONMENT REQUIRED**. N5 does not authorize live public FidoNet participation.

## N6 hub extension

The [FTN hub manual](ftn-hub.md) defines downstream/point configuration, separate area subscriptions, authenticated AreaFix, bounded rescan, safe activity and per-recipient recovery. Schema 25 and operator protocol 1.10 add these typed services; existing N1–N5 authority remains intact. Network bodies and credentials are absent from operator projections.
