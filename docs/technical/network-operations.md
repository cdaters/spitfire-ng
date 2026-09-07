# Networking operations and recovery — N5

Status: N5 COMPLETE / ACCEPTED. N1–N4 remain accepted. [M044](../research/m044-networking-foundation-gate.md)
remains binding. Native messages, network receipts, routing and queue authority
are unchanged. No public-network traffic is authorized.

## Operator contracts

A negotiated, bounded Networks read projection supplies overview, links, queues,
areas, directory generations, quarantine and recovery status. Bounded offsets page
long collections; rows carry typed action targets and resource versions. The
projection contains no subjects, bodies, private paths, secrets or raw protocol
transcripts. Existing events/notifications retain their authority.

sfmonitor adds Networks navigation and contextual typed Test/Poll/queue/scan actions.
Actions use existing explicit capabilities, CommandId receipts, daemon generation
and current policy checks. sfconfig edits named fields in typed network records;
static FTN/BinkP policy uses aggregate configuration CAS, QWK partners and area
mappings retain separate relational transactions and reviews. Secret replacement
is a distinct write-only operation, never a generic draft value or save review.

Directory and quarantine remain local authority. Quarantine is read-only because
N3 does not define deterministic destructive/reprocess transitions. Optional
NodelistDB access is deferred: no external service is needed for N5 operation.
Existing 21 capabilities fit the bound of 32; read-only bootstrap gains no grants.

## Recovery contract

FTS-0009 origin serial uniqueness and M044 prohibit random/clock-based recovery
floors. A restored snapshot cannot know later traffic. Replacement restore must
capture verified serial floors and matching accepted-queue evidence from the
newer exclusively locked target before it is replaced. Reconciliation never
lowers a serial floor; a held source cannot authorize resumed origination.

New-root recovery uses a surviving stopped board through explicit offline
configuration authority. The source is held before transferring its verified
floors, so both roots cannot originate under the same identity. Missing or already
uncertain source evidence leaves origination held. A new unrelated AKA is a
separate deliberate configuration choice, not an automatic recovery identity.

Only existing matching frozen queue identities/artifacts may inherit proven peer
acceptance from later source evidence. No unsent work becomes accepted by guess,
and no accepted item becomes pending. Other restored work remains held for review.
No message bodies, newer messages or unrelated configuration are imported by this
recovery operation. Existing directory generations and duplicate history remain
snapshot authority; remote duplicates across lost history remain an explicit
rollback limitation, not a claim of exactly-once delivery across time travel.

## Reference and scope

Bounded read-only FireComm profile/troubleshooting review: ADAPT independent
geometry/encoding diagnostics and separation of binary transfer from presentation;
RETAIN existing NG terminal guard/handoff behavior; no new terminal protocol or
reference code is imported. NodelistDB OpenAPI metadata was reviewed, not called.
FTSC remains primary for wire semantics. N6 hub/AreaFix and N7 file networking,
general scheduling and CircuitNET implementation remain outside N5.

## Concrete projection and configuration interfaces

Protected operator protocol 1.9 negotiates `Networks` through authenticated control
feature discovery; older hello vocabulary remains unchanged. `NetworkQuery` is a
closed section plus bounded offset. `NetworkPage` supplies at most 100 queue,
area, directory or quarantine rows and `more`; callers must refresh before acting
if rows move between pages. Resource versions, not list position, authorize work.
The complete daemon `networks::Snapshot` is capped at 512 KiB below the 1 MiB IPC
frame limit. Static link/AKA/source bounds and the native 784-conference bound
remain enforced. Queries are metadata-only, including directory Sysop/host fields
already admitted by local directory policy. Terminal controls/bidi overrides are
removed from projection presentation and visible scalar values are bounded.

The monitor worker performs protected reads and dispatches existing command IDs.
It retains no mailer socket and does not infer peer acceptance from a started
request. Link details distinguish current policy from last authenticated remote
observations. Queue details expose publication/artifact identity, route reason,
final destination/next hop, timestamps and attempts, never body/subject/path.
QWK hold is a new closed CAS wrapper over the existing shared queue; retry and FTN
hold/release reuse accepted N2/N4 semantics. Read-only bootstrap gains no authority.
The 21 explicit capabilities still fit the 32-capability bound; no new capability
was required. Directory activation uses its existing distinct grant.

sfconfig's network forms deserialize named draft fields into the existing typed
policies only at validation/save. Arrays use typed templates. A complete review
includes added/removed nested fields. Empty endpoint means no override; it never
means an arbitrary remote socket command. Replacing/clearing a credential is a
separate daemon request; the secret never becomes a generic draft or review value.
Submenu/form read heartbeats keep idle operator IPC alive without rebasing a dirty
form. Static saves take the network lock before the configuration gate, matching
network action lock order and preventing a mapping/source creation race during
reference validation. Relational references prevent orphaned maps, reinterpretation
of retained link identities and mutation of an ingested directory source profile.
Disable is permitted; profile correction uses a new source ID.

Static policy updates use aggregate revision/digest CAS, persisted by existing
configuration authority. EchoMail mappings and QWK partners/maps use their separate
native relational versioned transactions. No frontend opens SQLite or writes TOML.
The listener is restart-required; live link policy affects subsequent work and
cancels stale sessions through N4. Source/AKA/route/map/credential actions are
covered by existing privacy-safe audit. No new schema is needed: **24 → 24**.

## Verified restoration algorithm

`RecoveryEvidence` is non-deserializable and can only be obtained from native
SQLite through `ftn_recovery_evidence`. The board layer must own both roots
exclusively. It captures at most 1,024 origin rows and 10,000 accepted FTN delivery
attempts, rejecting excess rather than silently truncating. Source databases are
validated against their configured native board identity. Normal source ownership
and the explicit operator selection are prerequisites; this is not a mechanism
for reconciling independently operating clones of one FTN AKA.

Replacement restore captures the newer target evidence before staged restoration.
Only after the old backup passes its ordinary validation does one immediate SQLite
transaction reconcile serials and matching accepted queues in the staged copy.
Failure leaves the published target intact. New-root recovery locks both roots,
checks native identity, captures evidence, recovers stale claims, holds source
QWK/FTN work and disables source FTN/BinkP policy through configuration CAS before
applying the evidence. Failure after retirement leaves a safe held source; no
cross-root atomic transaction or automatic undo is claimed.

For each domain/address, retain the maximum of existing and proven next serial.
A smaller source floor cannot clear an existing hold; a held source does not
become proof of safe origination. No exhausted serial wraps. Matching queue proof
requires queue ID, frozen artifact digest, publication ID, native FTN fingerprint,
link ID and routing policy digest. The actual accepted attempt's ordinal/time is
retained; a contradictory attempt rolls back the transaction. Matching unsent
work becomes accepted with `recovered-peer-acceptance`; nonmatching work remains
held. Replaying reconciliation cannot lower floors or recreate deliveries.

N3 MSGIDs still use the original per-AKA monotonically allocated eight-hex-digit
serial. Native database row numbers are local snapshot identifiers, not FTN wire
identity; existing native publication IDs and network receipts remain distinct.
N2 QWK publication identity uses its existing fresh random identity and does not
borrow FTN serials. No N1/N2 identity generator was changed.

Restoration still invalidates BinkP sessions/claims and creates a new daemon
generation. It preserves directory generations, priorities/provenance, duplicate
history and accepted receipts present in the snapshot. Matching later acceptance
is the only added delivery evidence. Newer message bodies and lost inbound
history are not copied. A queue with no matching frozen artifact evidence stays
uncertain; missing post-snapshot history cannot support an exactly-once claim.

## Security and acceptance

Threat cases include read-only clients attempting mutation, stale CAS/queue
versions, configuration changes orphaning retained identities, terminal-control
injection through directory/config metadata, credential exposure in review,
private mail in diagnostics, and restoring a counter below later sent history.
Tests and the real journey exercise these authority boundaries. N4 framing,
authentication, remote-address checks, resource/session limits and private custody
remain unchanged. No polling to public systems or automatic directory retrieval
was introduced.

The real collision journey backs up frozen queued mail, delivers it and originates
additional mail, restores the older copy, reconciles and originates again. It then
repeats recovery into a new root while retiring the surviving source. An independent
native peer observes ten unique messages after both paths; earlier accepted work
is not sent again. A separate controlled Synchronet 3.19c / SBBSecho 3.15 regression
verifies real point NetMail/EchoMail sent from the operator Poll action. The
[M049 implementation report](../research/m049-networking-n5-operator-recovery.md)
records the final tests, terminal journeys, provenance and publication evidence.

Quarantine remains read-only; bounded historical link-attempt browsing and external
NodelistDB intelligence are deferred. Last-state health and compact durable counters
are sufficient without a speculative schema. N6 hub administration and N7 file
networking are separate milestones. CircuitNET is a planned preservation/revival
adapter under the [independent research policy](../09-circuitnet.md), not an N5
implementation. Windows live networking/UI/restore acceptance remains deferred to
a real Windows environment.

## N6 hub extension

The [FTN hub contract](ftn-hub.md) defines downstream/point configuration, separate area subscriptions, authenticated AreaFix, bounded rescan, safe activity and per-recipient recovery. Schema 25 and operator protocol 1.10 add these typed services; existing N1–N5 authority remains intact. Network bodies and credentials are absent from operator projections.

## N7 file-network extension

FileEcho, TIC, native-file hatching and exact approved FREQ reuse native file authority and BinkP transport. sfconfig offers typed file policy/mapping/subscription/grant/hatch forms; sfmonitor Networks → 9 Files includes staging, history and Enter delivery detail. [File-network workflows and authority](ftn-files.md). No raw path or credential appears in projections.

## Schema 28 identity integration

The [identity policy contract](identity-policy.md) defines private components,
configuration precedence, pre-submission preview, immutable posted author and
exact queue sender custody. Real-name requirements are configured policy, not an
inferred property of every FTN/QWK network. Imports retain external authors without
local name matching. Operational projections contain no private components.
