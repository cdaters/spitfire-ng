# SPITFIRE NG Roadmap

## C9 independent interoperability

C9 independent CIRCUITNET-NG interoperability is complete and accepted. An
original Python peer, public conformance vectors and implementation-neutral
specification prove bidirectional Core messages, threading, ACK/replay, direct
routing metadata, Dossier controls, Files/hash-have and signed Catalog/access.
The peer shares no production CircuitNET code and does not claim full HOST fanout.
A reproduced unnegotiated generation/access receive bug is fixed atomically at
native admission; an existing Health test clock race is corrected without changing
Health behavior. Schema **36**, protocol **1.4**, localization unchanged.

[Specification](docs/technical/circuitnet-ng-specification.md),
[implementation guide](docs/technical/circuitnet-implementation.md),
[reference peer](tools/circuitnet-conformance/README.md),
[C9 report](docs/research/m076-circuitnet-independent-interoperability.md).

C1–C8.1 remain accepted. Node IDs remain simple 1–8 character identifiers; geographic
assignment is administrative, not a routing hierarchy. Applications remain Closed.
No public service, production system or external live CircuitNET traffic changed.
Exact next action: stop for C9 review; no subsequent milestone is authorized.

## Previous accepted checkpoint

C8.1 Network Kit operator reconciliation is complete and accepted. The kit groups
required and Sysop-only conferences, explains Dossiers/Events/Files from both ends
of a link and uses explicit directory consent. International Node IDs follow a
versioned assignment convention within the existing 1–8 alphanumeric contract;
role and topology remain separate. A read-only assignment helper and a local,
capability-gated visiting-Sysop grant/revoke command support the guides.

Schema **36**, CircuitNET **1.4**, en-US **1.34.0**. Signed catalog revision 2 remains
unchanged: SUPPORT/CHITCHAT required; SYSOP/SPITFIRE/DOORS optional and Sysop-only.
CNETROOT is a new-assignment reservation; the accepted seed publisher remains ROOT.
Established topology/identity migration and address reuse remain explicitly deferred.
[Network Kit](docs/circuitnet-ng/README.md),
[addressing](docs/circuitnet-ng/ADDRESSING.md),
[C8.1 report](docs/research/m075-circuitnet-addressing-onboarding-reconciliation.md).

C1–C8 remain accepted. Applications Closed; no DNS/web/mail deployment or production
changes. Stop for C8.1 review; C9 has not begun.

## Previous accepted C8 checkpoint

C8 Conference Health is complete and accepted. One native projection separates local
reader progress and posting from inbound network volume across local, FTN, QWK and
CircuitNET conferences. It provides 7/30/90-day evidence, transparent trends,
sfmonitor/sfconfig controls, generic Events rollup and optional access-filtered
Hot Conferences. Schema **36**, CircuitNET protocol **1.4**, local operator IPC
**1.18**, en-US **1.33.0**. No user ranking, external telemetry or automatic removal.

[Conference Health manual](docs/manual/conference-health.md),
[technical specification](docs/technical/conference-health.md),
[C8 report](docs/research/m074-conference-health.md).
C1-C7.2 remain accepted. Public identity and Closed applications are unchanged.
No public deployment or production change. Stop before C9 for review.

## Previous accepted C7.2 checkpoint

C7.2 is complete and accepted: the [official public identity](docs/circuitnet-ng/PUBLIC-IDENTITY.md)
now comes from one validated release metadata authority. The revised Network Kit 1.0
uses generated home, joining/founder role addresses, application/download and catalog
locations. Applications remain **Closed**; URL metadata does not claim deployed
web, mail or download services. Schema 35 and protocol 1.4 are unchanged.

[Network Kit](docs/circuitnet-ng/README.md),
[C7.2 report](docs/research/m072-circuitnet-public-identity-activation.md).
C1-C7.1 remain accepted. No DNS, registrar, hosting, email or production changes.
Stop for review; actual deployment and opening require separately authorized work.
No C8 began.

Six kit tests, signature/reproducibility, source headers, formatting, strict Clippy
and local links pass. Full private workspace: 836 passed / zero failed / seven
existing ignored, eight doctest groups. Rust runtime sources are unchanged.

## Previous accepted state


C7 is complete and accepted: signed network conference catalogs, explicit
local mapping choices, Charter/Rules 1.0 and Network Kit 1.0. Schema **34**,
CIRCUITNET-NG **1.4 / catalog-sync**, operator IPC **17**, en-US **1.31.0**.
Network identity stays independent of local conference numbers. Reactivation preserves
identity; deliberate reuse requires a new identity and retained history. Existing
transport and Events move signed revisions. No election engine, private-mail feature,
file request or production network operation is introduced.

[Catalog administration](docs/circuitnet-ng/CATALOG-ADMIN.md),
[Network Kit](docs/circuitnet-ng/README.md),
[C7 report](docs/research/m070-circuitnet-governance-catalog-distribution.md).
Stop for C7 review after acceptance; do not begin C8.

## C6 — native Files and CircuitNET distribution

**C6 COMPLETE / ACCEPTED / PUBLISHED.** Schema **33**,
CircuitNET **1.3**, operator IPC **16**, en-US **1.30.0 / 1,404 messages**.
Native Files owns immutable SHA-256 content, bounded ZIP/TAR/GZIP inspection,
FILE_ID.DIZ suggestions, scanner results, quarantine, review and payload-aware backup.
CircuitNET distributes approved native files through typed file Dossiers, bounded
binary streaming, hash-have and durable per-neighbor receipts. C5 Events governs
exchange timing; existing messages and controls retain their native authority.

See the [C6 report](docs/research/m069-files-circuitnet-distribution.md),
[Files manual](docs/manual/files.md) and [custody contract](docs/technical/files-custody.md).
Recognized unsupported archives quarantine. Offset resume, derivatives, physical
payload GC and file requests remain deferred. Transport encryption does not make
published files private. C1–C5 remain accepted. Stop after C6; C7 requires review.

Public workspace: **753 passed / 0 failed / 7 existing ignored**, all 6 doctest
groups. Nineteen tests added. Headers (142), fmt, strict all-target Clippy, diff,
Markdown/local links and source provenance pass. Expanded six-node macOS C6 and
C2–C5/FTN/QWK regressions pass. ClamAV and cargo-audit remain unavailable; no live
ClamAV integration or audit success is claimed. No production changes or external
BBS traffic occurred. No binary release or tag is created.

## Previous accepted state

## C5 — native Events and CircuitNET operations

**C5 COMPLETE / ACCEPTED / PUBLISHED.** Schema **32**, CircuitNET **1.2**, operator IPC
**15**, en-US **1.29.0 / 1,382 messages**. Generic daemon-owned Events schedule
existing CircuitNET and BinkP services while native work prepares independently.
Manual, Immediate, Scheduled and Hybrid policies have durable history, coalescing,
missed-run handling and cold restore recovery. QWK keeps its existing handoff model.

See the [C5 report](docs/research/m068-circuitnet-operations-events-distribution.md),
[Events manual](docs/manual/events.md) and [modern distribution drafts](docs/circuitnet-ng/README.md).
The historical reconciliation and modern conference proposal remain separate.
Transport encryption does not make directed conference traffic private.
No production changes or external BBS polls occurred. Stop after C5; C6 needs review.

Public workspace: **734 passed / 0 failed / 7 existing ignored**, all six doctest
groups. Fourteen tests added. Headers (133), fmt, all-target Clippy, diff, local
links and provenance pass. Native macOS C5 and six-node C4 acceptance pass, with
C2/C3/FTN/QWK regressions. cargo-audit remains unavailable. No binary release/tag.

## Previous accepted milestones


## CircuitNET NG C4 — directed conferences and remote Dossier controls

Schema **31**, protocol minor **2**, en-US **1.28.0 / 1,354 messages**.
Typed destinations select one path in the configured END/HOST/ROOT tree.
Authenticated children can request their own Dossier subscriptions at their direct
parent; operator approval is the default. Durable decisions/results prevent
retries from duplicating mutations. Native messages remain canonical.

CircuitNET transport is encrypted. Directed routing is not private messaging;
stored conference messages follow BBS access rules. C1 remains historical authority,
C2 the native/offline foundation and C3 the live authenticated transport.

Six independent Apple Silicon daemons exercise same-branch, cross-branch and ROOT
routes, no fanout, ACK/result loss, approval/denial, restart and cold restore.
The [C4 summary](docs/research/m066-circuitnet-directed-routing-controls.md),
[manual](docs/manual/circuitnet.md) and [wire contract](docs/technical/circuitnet-transport.md)
record behavior, bounds, deliberate deferrals and validation.
**C4 COMPLETE / ACCEPTED.** Public workspace: **720 passed / 0 failed / 7 existing
ignored**, all six doctest groups. Headers (131), fmt, all-target Clippy, diff and
local documentation/provenance gates pass. No production or external CircuitNET
traffic, release/tag or binary distribution. Stop after C4; no C5 without review.

## Previous accepted checkpoints

## CircuitNET NG C3 — authenticated live conference exchange

Schema **30** adds native CircuitNET NG TCP/TLS 1.3 links to the accepted C2
native-message/offline foundation. Explicit certificates bind direct neighbors to
Node IDs and profiles. Configured END/HOST/ROOT trees and current Dossiers route
public conferences upstream, downstream and between siblings. SPITFIRE messages
remain canonical; CircuitNET uses its own protocol, without BinkP/FTN/QWK translation.

Symmetric finite polls carry the existing bounded C2 envelopes and exact durable
receipts. Lost acknowledgements recover through replay without a second import or
fanout. Hold/release, explicit retry, restart and native backup/restore preserve
queue truth. sfconfig adds live setup/actions, and sfmonitor adds CircuitNET Networks.
en-US is **1.27.0 / 1,328 messages**.

See the [operator manual](docs/manual/circuitnet.md),
[native contract](docs/technical/circuitnet.md) and
[wire specification](docs/technical/circuitnet-transport.md).
**C3 COMPLETE / ACCEPTED.** Sanitized public acceptance: **707 tests passed /
0 failed / 7 existing ignored**, including all six doctest groups. Headers (129),
fmt, all-target Clippy with warnings denied, diff, 127 Markdown documents / 934
local links and privacy/provenance checks pass. The real Apple Silicon four-board
TLS journey, automatic lost-ACK reconnect and C2 offline regression all pass.
cargo-audit is unavailable. The reviewed public delta is 7 added / 25 updated files.

C1 remains historical authority. C2 remains the native/offline foundation. C3 adds
live native transport only, with third-party-friendly wire boundaries and SPITFIRE
NG as the reference implementation. No legacy CNP/CND work, private mail, file
networking, remote Dossier commands, directed routing or governance automation.
No production systems changed or external live CircuitNET traffic occurred.
Stop after C3 for review; **do not begin C4**. No binary release or public port assignment.

Earlier sections retain their historical checkpoint scope.

## CircuitNET NG C2 — native offline conference exchange

Schema **29** implements CircuitNET as a first-class service around native SPITFIRE
messages. Network profiles, 1–8 character Node IDs, END/HOST/ROOT, configured trees,
conference codenames and per-neighbor Dossiers retain CircuitNET identity. Durable
queue/receipt/provenance authority preserves threading, suppresses replay/reflection
and keeps each recipient's completion independent. No FTN/QWK translation or
second message base is used.

The development/offline UTF-8 JSON profile requires explicit trusted operator
custody and expected-neighbor binding. Commands in sfconfig and spitfire operate
stopped boards under existing locks/capabilities. The independent three-board
macOS journey proves sibling distribution, filtering, replies, Dossier changes,
partial delivery/retry, restart, native cold backup/restore and frozen sender privacy.

See the [operator manual](docs/manual/circuitnet.md) and
[technical contract](docs/technical/circuitnet.md). en-US is **1.26.0 / 1,311 messages**.
Sanitized public acceptance: **697 tests passed / 0 failed / 7 existing ignored**;
all six doctest groups complete. The required headers, fmt, Clippy, diff,
Markdown/local links and privacy/provenance gates pass. cargo-audit is unavailable.

C1 remains historical authority; C2 is an independent modern implementation, not a
port. Legacy packet archaeology is deferred. No live CircuitNET transport, private
mail, file networking, governance automation, production changes or external
CircuitNET traffic. SPITFIRE NG is the reference implementation; the exchange
boundary permits future third-party native adapters. Stop after C2 for review;
**do not begin C3**. No new binary release, tag or website deployment.

Earlier sections below retain their historical checkpoint scope.


## Previous checkpoint — account and posting identity, schema 28

Login, Handle and private First/Last Name remain distinct. Posting identity
resolves before submission from board, conference and configured network
requirements. Historical authors and queued senders never follow later profile
changes. FTN does not universally force real names. Migration preserves legacy
values without splitting names; backup/restore preserves identity authority.

See the [identity contract](docs/technical/identity-policy.md),
[caller management](docs/operator/caller-management.md) and
[conference settings](docs/operator/messages.md).

**COMPLETE / ACCEPTED.** Publication gates pass **679 tests / 0 failed / 7
existing ignored**, including all six doctest suites, 119 source headers,
formatting, all-target Clippy with warnings denied, documentation links/fences
and privacy/provenance checks. The disposable native macOS identity journey
covers posting, profile changes, immutable history/sender, restart and cold restore.
en-US is **1.25.0 / 1,308 messages**. cargo-audit remains unavailable.

No production changes or public FidoNet traffic. Exact next action: stop this
completed identity pass. The next separately scoped major milestone is
**CircuitNET archaeology/resurrection**. No CircuitNET implementation, release,
tag or binary is included. Earlier sections below retain their historical scope.

> Previous checkpoint: **FTN interoperability and bounded FREQ recovery COMPLETE / ACCEPTED**.
> Schema 27. See [current status](STATUS.md) and the [file-network contract](docs/technical/ftn-files.md).

## N6 FTN hub and operator semantics — M050

Implemented routing-node, per-link EchoMail subscriptions/fanout, authenticated
AreaFix/AreaMgr, bounded native-history rescan and explicit point-boss relationships.
Schema 25 keeps messages canonical and reuses existing FTN/BinkP delivery custody.
See the [N6 report](docs/research/m050-networking-n6-ftn-hub.md),
[operator workflows](docs/manual/ftn-hub.md) and [service contract](docs/technical/ftn-hub.md).
N1–N5 remain accepted. N7 FileEcho/TIC/FREQ/hatching, public onboarding, general
scheduler, B-022/doors and release work remain outside this pass. Stop after N6
acceptance and sanitized source synchronization; subsequent work requires its own scope.

## Previous checkpoint: N5 operator cockpit and recovery — COMPLETE / ACCEPTED

sfmonitor Networks and named sfconfig forms operate the accepted QWK/DOVE, FTN
and BinkP engine. Queue actions, directory/point lookup, safe diagnostics and
verified restore floors/peer acknowledgements preserve native authority. Schema
24 remains unchanged; en-US 1.22.0 / 1,217 messages. See [M049](docs/research/m049-networking-n5-operator-recovery.md)
for private/public validation and controlled independent acceptance.

CircuitNET preservation/revival is planned as a future independent adapter.
Surviving documentation, binaries and file evidence will guide format archaeology;
`.CNP`/`.CND` compatibility is conditional on proof. Native networking authority
will replace DOS PRIMER/IMPORT/EXTRACT plumbing. No original implementation code
or proprietary binaries/documentation will ship. [Research policy](docs/09-circuitnet.md).
N5 does not implement CircuitNET.

Stop after N5 publication. The next separately scoped action is N6 hub/operator
semantics under M044 (downstream subscriptions, AreaFix/rescan and point-boss
administration); N7 file networking remains later. No N6/N7/general scheduler,
B-022/doors/release work or live public FidoNet traffic is included.


## Previous checkpoint: N3 native FTN core — COMPLETE / ACCEPTED

The N3 checkpoint implemented the **native FTN/FidoNet core** on schema **23**:
NetMail, transit, EchoMail, points, multiple domains/AKAs, scanner/tosser,
explicit routing, durable shared queues and nodelist/pointlist directory authority.
Independent Synchronet/SBBSecho packet interoperability passed in both directions,
including points and a forwarded return loop. SPITFIRE messages remain canonical;
QWK N1/N2 remain accepted. [FTN Sysop procedure](docs/manual/ftn-core.md),
[Technical Reference](docs/technical/ftn-core.md) and
[M047 report](docs/research/m047-networking-n3-ftn-core.md) state exact scope.
At the N3 checkpoint BinkP was not implemented; accepted N4 transport supersedes
that historical boundary. Live public FidoNet participation is not claimed.
SMB remains unimplemented. B-021 VERIFIED; B-022 NOT STARTED. en-US 1.20.0 /
1,051 messages. The 0.1.0 preview binary remains unchanged; no new release or tag.
Windows live FTN acceptance remains deferred to a real Windows environment.

Exact next separately scoped action: N4 BinkP over accepted FTN queues/artifacts,
controlled leaf/point exchange and restore reconciliation. No public traffic
is authorized by this handoff.

## Previous checkpoint: N2 QWK networking — COMPLETE / ACCEPTED

The accepted N2 checkpoint at schema **22** implemented typed multi-partner QWK networking,
conference mapping, native private mailbox/transit, durable queues, receipts,
provenance, duplicate/loop prevention and explicit manual artifact exchange.
Controlled Synchronet 3.19c imported public/private NG packets and generated
public/private replies plus transit packets imported by the same macOS daemon.
DOVE-compatible behavior uses the shared QWK architecture. No public DOVE-Net
traffic or membership is claimed. N1 remains accepted; native messages are canonical;
At that N2 checkpoint FTN and BinkP were unimplemented; N3 above supersedes
that FTN boundary. SMB remains unimplemented. B-021 VERIFIED; B-022 NOT STARTED.
en-US **1.19.0 / 1,047 messages**. No release/tag/package is created. Real Windows
networking acceptance remains deferred to a real Windows environment.
See [M046](docs/research/m046-networking-n2-qwk-dove.md) for evidence and limitations.


> Current source checkpoint: **M049 / N5 operator/recovery COMPLETE / ACCEPTED**.
> Schema 24; N1–N4 remain accepted. No live public FidoNet participation.


SPITFIRE NG is moving from historical research and stock-core implementation
into public Development Preview releases. This roadmap describes major
outcomes rather than internal checkpoint chronology.

> Current operator checkpoint: **B021-D COMPLETE / ACCEPTED; B-021 VERIFIED**.
> Earlier slice descriptions retain their historical scope. B-022 remains NOT STARTED.

## Completed foundations

### Preservation model

- Defined the boundary between SPITFIRE identity and obsolete DOS limitations.
- Established safe legacy parsing rules, CP437 preservation, and provenance
  requirements.
- Classified the stock SPITFIRE 3.7 command and caller-experience scope.

### Stock core

- Implemented board setup, configuration, callers, security, nodes, messages,
  files, transfers, and operator control.
- Achieved the defined Stock SPITFIRE 3.7 Core Parity scope.
- Achieved the defined ANSI/text Operator/Caller Experience Parity scope.
- Added generated stock menus, exact-security display overrides, live caller
  context, and security-aware fallback.
- Added post-0.1.0 stock-style Specific Caller and Text Search with bounded,
  read-only, visibility-filtered results. Text Search keeps SPITFIRE's command
  flow while intentionally modernizing the historical phrase limitation with
  whitespace-delimited all-term matching.
- Added post-0.1.0 CC delivery fan-out and auditable message mutation:
  separately numbered primary/CC deliveries, per-delivery receipts and
  tombstones, authorized Delete/Undelete and audience transitions,
  source-retaining Copy/Forward, schema-11 lineage/audit, stale-conflict
  protection, and schema-10/schema-11 recovery.
- Added post-0.1.0 auditable caller access: Active/Locked Out/recoverable
  Deleted lifecycle, disable/restore, purge protection, base/effective
  security, board-local subscription warning/expiry/renewal, bounded JOKER
  name denial, named-Sysop invariants, active-session invalidation,
  dispatch-time reauthorization, privacy-safe audit, stale-conflict handling,
  schema-12 persistence, and schema-10/schema-11/schema-12 recovery.
- Added post-0.1.0 schema-14 privacy-bounded public information: a board-
  disabled/caller-opt-in handle-only directory and locate, native ordered
  Other BBS authority, numbered bulletins, newsletter, safe system facts, and
  bounded project-native thoughts with privacy-safe audit and cold recovery.
- Added post-0.1.0 schema-15 safe file inspection, request, and maintenance:
  stable file identities; separate lifecycle/integrity; bounded sanitized
  text, ZIP, and FILE_ID.DIZ inspection; Preview inspection without transfer;
  private requests; PendingReview uploads; duplicate/denial/case policy;
  versioned staged operations; crash reconciliation; semantic audit; and cold
  recovery. B-013 is VERIFIED; B-015 and B-012 remain IMPLEMENTED with
  explicit verification work outstanding.
- Added post-0.1.0 schema-16 batch transfer policy and extended storage:
  versioned board-day accounting and whole-batch reservations, idempotent
  settlement, bounded session queues, all nine required transfer engines,
  logical storage roots/locators, read-only media semantics, active-use, and
  bounded large-source streaming. Schema 17 separately permits valid
  zero-byte catalog objects. B-024, B-011, B-014, and B-023 are VERIFIED, and
  M039 Tranche 6 is semantically closed.
- Added schema-18 operator observability: privacy-bounded retained events,
  board-day summaries, versioned retention, actionable notifications, bounded
  board/node/activity/maintenance projections, and cold recovery. B-017 is
  VERIFIED.
- Added schema-19/B021-A protected read-only operator attachment through
  Unix-domain sockets and Windows named pipes, plus OS-backed host-operator
  identity, protocol/capability negotiation, the shared `OperatorClient`, and
  localized CLI views. B021-A is cross-platform closed; final B-021 closure is recorded below.
  B-022 remains NOT STARTED.
- Added the read-only `sfmonitor` 0.1 local operator application over the same
  `OperatorClient`, with responsive Dashboard, Nodes, Callers, Activity,
  Statistics, Notifications, Maintenance / Errors, help, and reconnect views.
  No mutation or configuration feature was added.

### Modern transport interleave

- Completed M042.5 Secure SSH Caller Transport without changing Category-B
  dependency order: schema-13 login identifier/public handle/private real-name
  separation, disabled-by-default SSH-2 through the common session engine,
  ordinary caller authentication with no duplicate password prompt, Ed25519
  host identity, PTY/resize/encoding propagation, shared nodes and lifecycle
  invalidation, privacy-safe diagnostics, and cold-backup preservation.
- SSH remains caller access only. It provides no host OS shell, Unix account,
  command execution, SCP/SFTP, forwarding, subsystem, or remote-filesystem
  route.

### Presentation and language

- Added Modern, Classic SPITFIRE-inspired, and Minimal Terminal profiles.
- Added board-owned `display/` overrides without weakening engine command or
  security authority.
- Added a versioned language-package contract and complete en-US baseline.
- Verified Moebius 1.0.29 for bounded macOS `.CLR` authoring.

### Operator readiness

- Added prebuilt Development Preview packaging for Apple Silicon macOS.
- Validated clean setup, Qodem, SyncTERM, RAW Text, status, shutdown,
  backup/restore, and upgrade preservation.
- Added checksums, release metadata, dependency notices, and public operator
  documentation.

## Current release stage

SPITFIRE NG 0.1.0 Development Preview is published for Apple Silicon macOS as
tag `v0.1.0-development-preview`. The public-redownload checksum and bounded
unsigned/unnotarized macOS first-run workflow have passed.

Public source `main` now includes post-0.1.0 message-discovery, auditable
message-mutation, auditable caller-access, schema-13 caller identity, SSH
caller-transport improvements, schema-14 public information, schema-15
Tranche 5 file-domain features, schema-16/17 Tranche 6 transfer/storage source,
schema-18 B-017 observability, and schema-19 B021-A protected operator
attachment plus the read-only `sfmonitor` 0.1 application. No new binary, tag,
or release has been published; the downloadable binary remains the accepted
0.1.0 Development Preview and does not contain those later source changes.

## Current Category-B boundary

M039 Tranche 5 has implemented B-013, B-015, and the remaining B-012 native
contracts on schema 15. B-013 completed its accepted semantic, security,
presentation, transport, and client matrix and is VERIFIED. B-015 and B-012
remain IMPLEMENTED while their documented legacy-import/recovery/operator and
caller-workflow acceptance items remain open.

Tranche 6 adds B-024/B-011/B-014/B-023 on schema 16, with schema 17 correcting
zero-byte file authority. Independent original and modern peers close B-024;
member-aware queue recovery, policy/DST/concurrency acceptance, and external-
storage restore/adapter acceptance close B-011, B-014, and B-023. All four
rows are VERIFIED and M039 Tranche 6 is semantically closed. The Category-B
ledger stood at 13 VERIFIED, 2 IMPLEMENTED, 5 PARTIAL, and 5 NOT STARTED at
that closure.

Tranche 7 begins with schema 18 and VERIFIED B-017: structured operational
events, daily statistics, retention, notifications, and bounded status and
maintenance services. Schema 19/B021-A adds protected local attachment,
daemon generation, bounded command receipts/control audit, capability
negotiation, read-only B-017 transport, and the operator CLI on Unix/macOS and
Windows. B021-A is cross-platform complete, but B-021 Local/Sysop Operator
Controls remains PARTIAL. B-022 screen/export/publication work remains NOT
STARTED. The Category-B ledger remains 14 VERIFIED, 2 IMPLEMENTED, 4 PARTIAL,
and 5 NOT STARTED.

The `sfmonitor` 0.1 operator-product interleave and B021-B live controls are
implemented and accepted over the completed client. B021-C implements
typed configuration and the first `sfconfig`; B021-D remains for maintenance/
platform and integrated acceptance. B-022 remains later report/publication
work.

Future operator products share daemon-authoritative typed services. The
existing `spitfire setup` command remains the bootstrap/recovery path;
`sfmonitor` is the implemented read-only-by-default live operator cockpit with
explicitly enrolled B021-B Actions; `sfconfig` is
the implemented typed configuration application; and CLI clients provide
noninteractive access. `sfmonitor` provides **System Configuration** as a real
terminal handoff to independently launchable `sfconfig`. None may edit
SQLite directly or treat diagnostic text logs as authority.

Near-term release work includes:

- collect sanitized operator feedback and reproducible bug reports;
- decide the next tested binary platforms;
- add signing/notarization when suitable release credentials and process are
  available; and
- improve preview upgrade guidance as real public upgrades occur.

## Future SPITFIRE compatibility

The remaining advanced compatibility work is deliberately separate from the
accepted stock-core tier:

- advanced display/resource types, including RIP;
- questionnaires, bulletins, ratios, batch workflows, and deeper maintenance
  controls;
- Further historical LAKOTA format evidence;
- FTN/FidoNet NetMail/EchoMail and BinkP;
- CircuitNet preservation/adaptation with possible future revival, without
  assuming an active network;
- expanded doors and external-program support; and
- deeper events, maintenance, and multinode administration.

Each area needs a documented interface and evidence-based acceptance scope
before implementation.

## Modern improvements

Longer-term modern SPITFIRE NG work may include:

- further secure-transport authentication features such as caller public-key
  management;
- web administration;
- production language packs and community translation workflow;
- caller-selectable installed presentation profiles;
- additional rights-clean community presentation packages;
- broader packaging, service integration, and automatic update support; and
- modern federation and interoperability where it fits SPITFIRE's operating
  model.

Modern work must not weaken privacy, security, data preservation, or command
authority.

## SFDraw

[SFDraw](docs/sfdraw.md) is a planned Rust-based, cross-platform companion
editor for SPITFIRE-compatible `.CLR` and `.BBS` resources. Its goals include
long-form ANSI canvases, classic 80-column presets, VGA/CP437 fidelity,
byte-aware saving, board-local override integration, and optional baud-rate
playback. RIP editing remains deferred until SPITFIRE NG itself supports and
validates RIP.

SFDraw is a future project; no implementation has begun. It remains parked
while established external ANSI-art tools satisfy near-term authoring needs.

## Separate preservation work

SFDATE and SFREG remain private preservation/research streams rather than
features of the 0.1.0 Development Preview. They are not included in this
public source snapshot or release package.

## Development rule

Historical compatibility claims require evidence. Legacy input remains
read-only until its format is understood, and proprietary historical material
is never redistributed merely because it was useful during research.

For current capabilities see [Status](STATUS.md). For detailed implemented
behavior see the [documentation index](docs/README.md) and
[parity checklist](docs/stock-spitfire-3.7-parity.md).
## Completed B021-B source milestone

B1 mutation/time/acknowledgement, B2 page/chat/disconnect, B3 daemon shutdown,
and integrated macOS acceptance are COMPLETE / ACCEPTED. Schema 19, D-064
minor-gated discovery, read-only bootstrap, explicit bounded profiles, durable
audit/receipt recovery, exact-session safety, chat privacy, and transfer integrity
remain binding. Windows live B021-B acceptance remains deferred to a real Windows
environment. See the [integrated report](docs/research/m039-tranche-7-b021b3-shutdown-integrated.md).

At the B021-B checkpoint, B-021 remained PARTIAL and totals were 14/2/4/5.
B021-C and final B021-D closure follow below; B-022 remains NOT STARTED.

## Completed B021-C source milestone

Typed configuration authority and the first native sfconfig MVP are COMPLETE /
ACCEPTED. Shared validation, explicit daemon/offline ownership, CAS, atomic
recoverable saves, effect presentation, bounded operator profiles, secret-safe
status, and sfmonitor terminal handoff pass native macOS acceptance. Schema 19;
en-US 1.15.0. The [configuration report](docs/research/m039-tranche-7-b021c-sfconfig.md)
records scope, tests, and limits. Real Windows sfconfig acceptance remains deferred.
At the B021-C checkpoint, B-021 remained PARTIAL pending B021-D, with totals
14/2/4/5. The accepted closure follows below; no deferred family was implemented.

## Completed B021-D and stock operator controls

B021-A/B/C remain accepted. B021-D is **COMPLETE / ACCEPTED** and **B-021 VERIFIED**.
The [closure report](docs/research/m039-tranche-7-b021d-operator-closure.md) records
the historical outcome map, native macOS integration, narrow frontend fixes,
maintenance-owner guidance, explicit local recovery, audit and regression evidence.

Schema 19; read-only bootstrap; 16 recognized capabilities within the unchanged
32-entry bound; explicit protected-IPC mutation enrollment; no secret projection
or persisted chat. en-US 1.16.0 / 988 messages. All 25 Category-B rows recount to
15 VERIFIED / 2 IMPLEMENTED / 3 PARTIAL / 5 NOT STARTED. B-022 remains NOT STARTED.
Windows live integrated operator acceptance remains DEFERRED — REAL WINDOWS
ENVIRONMENT REQUIRED; Linux/BSD live acceptance is not claimed.

Stop at this source milestone. Subsequent development requires a separately
scoped B-022 interface/resource/transaction gate; implementation has not begun.
No networking, doors, scheduler, host deployment or release work is included.

## N1 complete; later networking remains scoped separately

The shared QWK codec, native mapping/artifact/receipt foundation and real caller
Messages L D/U/S/Q cycle are implemented. Independent MultiMail/QWKE and disposable
macOS transfer/replay/restart/backup acceptance pass. Original LAKOTA LMR remains
evidence-qualified, so C-001 is PARTIAL. B-021 stays VERIFIED; B-022 NOT STARTED.

See the [M044 architecture summary](docs/research/m044-networking-foundation-gate.md)
and [M045 report](docs/research/m045-networking-n1-qwk-offline.md). The next
implementation slice is separately scoped N2 QWK partners/DOVE profile, not part
of N1. FTN/BinkP, directories, Networks operator views, scheduler and doors are not
implemented. Windows live networking acceptance remains deferred to a real Windows
environment. No release, DDEV or production change is part of this source update.

## N4 native BinkP — COMPLETE / ACCEPTED

Native client/listener, CRAM, point/AKA negotiation, N3 queue/tosser custody,
retry/hold/release and restart/restore are implemented on schema 24. Independent
Synchronet exchange passed both directions. Small typed Poll/Test/configuration
and status surfaces are implemented. See [M048](docs/research/m048-networking-n4-binkp.md).
Next separately scoped action: N5 operator surfaces and recovery ergonomics.
No live public FTN, AreaFix/FileEcho, scheduler, B-022, doors or release work.

## N7 FTN file networking — M051

Implemented FileEcho leaf/hub, authenticated TIC processing, native-file hatching, explicit grant-only FREQ and file-network operator/recovery surfaces. Schema 26; en-US 1.24.0 / 1,291. N1–N6 remain accepted. [Report](docs/research/m051-networking-n7-ftn-files.md), [manual](docs/manual/ftn-files.md), [contract](docs/technical/ftn-files.md). Stop after acceptance and sanitized source synchronization; no public FidoNet, scheduler, CircuitNET implementation, B-022/doors or release work begins.
