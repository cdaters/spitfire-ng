# SPITFIRE Message Network Architecture

Status: **N1–N5 accepted**. The [Networks operator/recovery contract](technical/network-operations.md)
adds current operability and verified restore reconciliation. The
[FTN reference](technical/ftn-core.md) documents implemented packet/mail/directory
semantics. [Native BinkP](technical/binkp.md) client/listener and controlled
independent interoperability are accepted; live public FidoNet participation is
not claimed.
See the [N1 Technical Reference](technical/qwk-offline.md) for implemented authority;
the future design below retains M044 scope.
Current schema is 24. The canonical implementation-ready design, evidence, contracts,
limits and N1–N7 acceptance sequence are in the
[M044 networking foundation gate](research/m044-networking-foundation-gate.md).

## One native message authority

Native MessageId, immutable payloads, deliveries, audience, permissions and mutations
remain authoritative. QWK offline mail, QWK networking, DOVE-Net and FidoNet surround
that model as adapters/interchange/routing/transport layers. No separate QWK/FTN
message store, SMB dependency or external mailer-owned primary base is required.
M044 supersedes the early speculative mixed live backend proposal for networking.
See the [native message specification](sfng-message-system.md) for implemented
behavior and [message design](07-message-system.md) for preservation boundaries.

Pure adapters decode/encode typed envelopes and protocol metadata. Core services
validate identity, authorization, mappings, duplicate history and native transactions.
Daemon workers own scanner/tosser jobs, durable transport-independent outbound queues
and BinkP exchange. Operator clients configure or dispatch typed operations; they
never execute networking themselves. Scheduler integration is a later typed-job
consumer; manual execution must work without a scheduler or door runtime.

## Identity, delivery and routing

Native identity does not become a QWK packet number or FTN MSGID. Publication IDs,
network-origin identity, import/export receipts and per-target routing decisions
preserve provenance and reply linkage. External origin/private transit extend the
same native authority in N2; no fake caller accounts or public transit conferences.
Explicit versioned mappings connect native conferences to partner QWK numbers or
FTN EchoMail tags. NetMail uses addressed private delivery/transit policy.

FTN addresses include zone, net, node, point and network/domain from day one.
Multiple AKAs, networks and composable leaf/routing/hub/point-boss roles are designed.
BinkP is the required modern native FTN transport target; transport acceptance is
separate from successful message import. Full addressing, preserved controls,
MSGID receipts and correct SEEN-BY/PATH/target exclusions prevent loops. Unknown
metadata is bounded and preserved separately from caller-visible text.

## Configuration, security and recovery

Typed board configuration owns intended network/link/trust/routing policy; typed
relational configuration owns mappings/subscriptions/recipient aliases. Runtime
SQLite owns queued work, receipts, duplicate history, directory generations and
quarantine. Immutable artifacts are referenced by safe IDs/digests, never remote
paths. Proposed migrations are phased; none is implemented by M044.

Credentials remain write-only in operator surfaces. Native author, network-visible
alias and private caller identity stay separate. No bodies/passwords/raw config in
logs or audit. Packet/archive/resource bounds, authenticated links, quarantine and
explicit reprocessing apply before native admission. Directory updates cannot
silently replace configured link trust. Restores invalidate live leases and hold
uncertain traffic/origin serial state; no exactly-once promise across rollback.

The accepted read-only bootstrap, explicit mutation enrollment, authenticated local
IPC, CommandId/idempotence, CAS/versioning and daemon authority remain intact.
Future Networks views extend sfmonitor/sfconfig through shared typed authority.

## Implementation and compatibility boundaries

1. N1: shared adapter foundation + caller QWK export/import/pointer workflow.
2. N2: QWK networking + independent DOVE-Net profile interoperability.
3. N3: FTN addressing/metadata, routing/scanner/tosser/queues and directory foundation.
4. N4: BinkP + controlled independent leaf/point acceptance.
5. N5: full operator Networks surfaces and directory/health/queue management.
6. N6: hub/routing/point-boss, optional AreaFix and bounded rescan.
7. N7: FileEcho/TIC/hatch/FREQ and attachment workflows on native file authority.

Minimum safe operator controls accompany each feature before N5 consolidation.
CircuitNet remains preservation/possible revival, not an active-network MVP.
Original LAKOTA LMR byte compatibility remains evidence-qualified; N1 implements
known native pointer semantics without inventing unknown bytes. FTSC is technical
authority; SPITFIRE primary sources own historical outcomes. Synchronet/NodelistDB
are read-only secondary references, not dependencies or standards authorities.

No public-network test traffic is authorized by this gate. Independent isolated
peers are required where practical. macOS is the primary acceptance platform;
Windows live networking acceptance is **DEFERRED — REAL WINDOWS ENVIRONMENT
REQUIRED**. No Linux/BSD live acceptance is claimed. B-021 remains VERIFIED;
B-022 remains NOT STARTED. N1 adds caller offline QWK; no external networking,
scheduler, door or release is added.

## M045 / schema 20 caller QWK integration

The [QWK offline Technical Reference](technical/qwk-offline.md) defines the implemented adapter, native authority, delivery/pointer semantics, private artifact custody, transactional receipts and recovery. Caller QWK uses ordinary authenticated message permissions and existing binary transfers. No QWK networking, DOVE-Net, FTN, scheduler or separate message store is added. Earlier dated schema/milestone descriptions retain their historical scope.

## N2 implemented authority

The [QWK network reference](technical/qwk-networking.md) owns schema 21/22 partner/maps, native private mailbox/transit, receipts, immutable queue decisions and manual exchange. M044 remains binding. Native messages remain canonical; no SMB store/codec or FTN/BinkP implementation exists.

## M047 / schema 23 native FTN core

N3 implements NetMail and EchoMail around the canonical native message authority,
with full points, domain-separated identities, multiple AKAs, explicit routing,
scanner/tosser and durable shared queue work. FTN controls/provenance remain
separate from QWK envelopes and visible bodies. EchoMail maps only to enrolled
native conferences; private NetMail requires an explicit active caller alias and
transit has no caller-readable container. No SMB, external message store or
BinkP implementation exists. Independent packet interoperability and native
macOS acceptance pass; no live public FidoNet traffic was sent. The
[N3 Technical Reference](technical/ftn-core.md) owns schema, interfaces, resource
limits, duplicate/loop rules and recovery; the [Sysop procedure](manual/ftn-core.md)
explains isolated configuration and manual artifacts. Earlier milestone sections
retain their dated scope. Windows live FTN acceptance remains deferred.

## M048 / schema 24 BinkP transport

The [BinkP Technical Reference](technical/binkp.md) and [manual](manual/binkp.md)
define implemented daemon caller/answerer roles around N3. Queue claims, health and
custody receipts extend the existing queue; complete inbound artifacts toss
automatically. Explicit AKA/remote identity, CRAM default, M_GOT acceptance,
bounded batches and restart/restore uncertainty holds preserve native authority.
Minimal Poll/Test/Hold/Release and configuration/status surfaces are implemented;
N5's full cockpit and scheduled polling remain later.

## N5 — operator and recovery authority

N5 operates accepted N1–N4 through a bounded sfmonitor Networks cockpit and named
sfconfig policy/mapping forms. [The operations contract](technical/network-operations.md)
owns projection limits, capability/CAS checks, reference validation and verified
restore serial/queue reconciliation. Static policy, relational mappings and native
messages keep their existing owners. No transport becomes routing authority.

CircuitNET is now a **planned preservation/revival adapter**. Future format and
behavior archaeology may recover `.CNP`/`.CND` compatibility from surviving evidence;
it will be implemented independently over native networking authority, without
recreating DOS PRIMER/IMPORT/EXTRACT plumbing. See [the approved research policy](09-circuitnet.md).
N5 includes this roadmap decision only. N6 hub/AreaFix, N7 FileEcho/TIC/FREQ and
general scheduling remain separate future work.

## N6 — hub service authority

Native messages remain canonical. Separate durable downstream subscriptions select
normal EchoMail fanout; one native publication produces independent queue work.
Authenticated AreaFix and bounded rescan operate through the same transactional
native authority. Explicit points retain 4-D identity and existing BinkP links.
See [hub contract](technical/ftn-hub.md). Schema 25 adds only N6 authority; N7 file
networking remains unimplemented.

## N7 — file adapter authority

FileEcho/TIC/hatching/FREQ wrap the existing native file authority. Schema 26 owns only mappings, subscriptions, provenance, bounded staging and delivery/request receipts. The accepted N4 BinkP engine transports every artifact; there is no second mailer or generic file catalog. [Contract](technical/ftn-files.md); [operator guide](manual/ftn-files.md).
