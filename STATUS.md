# SPITFIRE NG Status

C7.1 is complete and accepted: the revised Network Kit 1.0 has a root README.TXT,
FILE_ID.DIZ and intentional ASCII/CRLF BBS editions, complete role/joining guides,
historical Sysop-area policy and operational catalog signing-key recovery.
Schema **35**, wire **1.4 / catalog-sync / catalog-access**, en-US **1.32.0**.
Conference IDs are generated automatically; local numbers/access remain local.
Catalog revision 2 retains the 46 identities and original signed history.
Applications are explicitly not yet open until public contact metadata is supplied.

[Network Kit](docs/circuitnet-ng/README.md),
[Key custody/recovery](docs/circuitnet-ng/KEY-CUSTODY.md),
[C7.1 report](docs/research/m071-circuitnet-network-kit-release-readiness.md).
C1-C7 remain accepted. No production or external BBS traffic, no C8 work.
Stop for review before another milestone.

Public workspace: **774 passed / zero failed / seven existing ignored**, six doctest
groups. All seven live CircuitNET campaigns, four kit tests, headers (151), fmt,
strict Clippy, diff and local links pass. Cargo-audit remains unavailable.

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

## Previous checkpoint: FTN interoperability and recovery — COMPLETE / ACCEPTED

That checkpoint used **schema 27**. Contextual RESCANNED normalization, authenticated
direct-hatch TIC history, bounded same-request FREQ recovery and negotiated BinkP
1.1 response batches are accepted. Exact peer, filename, destination and domain
authority remains fail-closed; generic parsers and routing do not acquire broader
compatibility. Native messages and files remain canonical.

Independent Synchronet interoperability verifies rescan provenance, FileEcho in
both directions, one FREQ payload after EOB, unauthorized request denial,
interrupted transfer/retry, restart and duplicate/reflection suppression. Tests
used isolated configured peers. Operational identities, transcripts, configuration,
credentials and research inputs are excluded from this public source tree.

See the [BinkP contract](docs/technical/binkp.md),
[FileEcho/FREQ contract](docs/technical/ftn-files.md),
[rescan contract](docs/technical/ftn-hub.md) and
[operator recovery workflow](docs/manual/ftn-files.md).
The Windows operator workflow accepts its dedicated branch, PRs to main and
manual dispatch; normal pushes to main do not trigger it. Windows live acceptance
remains deferred. No release, tag, binary or public-network participation is added.

Publication gates pass **661 tests / 0 failed / 7 existing ignored**, including
six doctest suites, formatting, all-target Clippy with warnings denied, 117 source
headers, documentation links/fences and privacy/provenance checks. Synthetic
fixtures retain the private source's authority boundaries; no operational identity
or captured peer artifact is published. cargo-audit remains unavailable.

Exact next action: stop after source publication closure. No additional
interoperability traffic is required; further development needs a new scope.
Earlier sections below record historical checkpoints.

## N7 FTN file networking — COMPLETE / ACCEPTED

Native file authority now supports FileEcho leaf/hub operation, authenticated
fail-closed TIC, native-file hatching and explicitly authorized exact-name FREQ.
Per-link acknowledgement truth survives partial fanout and restore. Staging is
bounded/private; traversal and arbitrary-file access are blocked. Native message
and file catalogs remain canonical; BinkP remains transport only. N1–N6 accepted.

Schema **25 → 26**; en-US **1.24.0 / 1,291 messages**. Accepted source:
`6a6a7e9dd93b911e9281085e62b116160bffafb9`. [M051](docs/research/m051-networking-n7-ftn-files.md),
[manual](docs/manual/ftn-files.md), [reference](docs/technical/ftn-files.md).
Four native macOS daemons and independent TickIT/BinkP prove the file flows;
independent FREQ is not claimed. Public gates pass 620 tests, six doctest suites, 112 headers and 123 Markdown/local-link documents; fmt/Clippy/diff/privacy pass. Full evidence is recorded in M051.

No public FidoNet traffic. Private corpus, peer source/config, credential files,
acceptance artifacts and private Git history are excluded. Scheduler, CircuitNET
implementation, B-022, doors, DDEV, production, FireComm and release/service
packaging are unchanged. Windows live acceptance remains deferred. Exact next
action: stop after N7 source synchronization; later work needs a new scoped pass.

## N6 FTN hub — COMPLETE / ACCEPTED

SPITFIRE NG can act as a leaf, routing node, EchoMail hub and point boss using
one native message authority. Separate durable downstream subscriptions govern
fanout. Authenticated AreaFix/AreaMgr changes only its own link; bounded native
rescan preserves identity and isolates intentional replay. Partial deliveries and
cold restore preserve each peer acknowledgement. Private NetMail bodies and
credentials are absent from operator projections. BinkP remains transport only.

Schema **24 → 25**; en-US **1.23.0 / 1,249 messages**. N1–N5 remain accepted.
[M050](docs/research/m050-networking-n6-ftn-hub.md) records five-daemon Apple Silicon,
independent Synchronet/SBBSecho, terminal/CAS, privacy and restore evidence.
[Manual](docs/manual/ftn-hub.md); [Technical Reference](docs/technical/ftn-hub.md).
Accepted private source: `03ff31f724bbd66ba5e47a338928c9a969052590`. Public gates: **602 passed / 0 failed / 6 opt-in ignored**, six doctest suites,
108 headers, fmt/Clippy/diff, 120 Markdown/local-link documents and privacy/provenance
pass. Publication accounting is recorded in M050.

No live public FidoNet traffic. Private corpora, credentials, peer source/config
and acceptance artifacts are excluded. N7 FileEcho/TIC/FREQ/hatching, general
scheduler, CircuitNET implementation, B-022/doors and release work remain untouched.
DDEV, production and FireComm are unchanged. Windows live acceptance remains
**DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**. Exact next action: stop after
N6 source publication; any later milestone requires a separately scoped pass.

## Previous checkpoint: N5 networking operator and recovery — COMPLETE / ACCEPTED

sfmonitor Networks provides overview, links, queues, areas, directory, quarantine
and recovery. sfconfig provides named QWK/DOVE/FTN/BinkP policy/mapping forms,
conference selection, CAS, write-only credentials and verified stopped-board
recovery. Native message/routing/queue authority is canonical; BinkP is transport
only. Schema **24 → 24**; en-US **1.22.0 / 1,217 messages**.

Verified later serial floors and matching peer acknowledgements survive an older
restore. New-root recovery retires the surviving source before resuming origin
allocation. Hard collision, real macOS terminal/configuration/CAS/reconnect and
independent Synchronet point NetMail/EchoMail checks pass. See [M049](docs/research/m049-networking-n5-operator-recovery.md),
[manual](docs/manual/network-operations.md) and [Technical Reference](docs/technical/network-operations.md).

N1–N4 and B-021 remain accepted; B-022 NOT STARTED. SMB remains unimplemented.
Quarantine mutations, historical link browsing and optional NodelistDB integration
are deferred. CircuitNET preservation/revival is planned, with no implementation
or compatibility claim. Windows live networking remains **DEFERRED — REAL WINDOWS
ENVIRONMENT REQUIRED**. Live public FidoNet traffic: **NONE**.

No AreaFix/FileEcho/TIC/FREQ/general scheduler/doors/DDEV/production/FireComm/release
work occurred. Private corpora and acceptance material are excluded. Stop after
N5 publication; N6 requires a separately scoped pass. Accepted private source:
`c79889ef793c3002999dc291f2264de145e28227`.

Public N5 gates: **591 tests / 0 failed / 5 opt-in ignored**, doctests, 104 source
headers, fmt/Clippy/diff, 117 Markdown/link documents and privacy/provenance pass.
Published source contains eight added / 46 updated files; 24 shared files match
private source exactly, with only private-package pruning in Cargo.lock.

## Previous checkpoint: N4 native BinkP — COMPLETE / ACCEPTED

Native BinkP client/listener exchange NetMail and EchoMail through the accepted
native FTN queue and automatic tosser. Controlled independent Synchronet BinkP
interoperability passed both connection roles, including a point AKA and multiple
AKAs. CRAM-required authentication/address checks fail closed; M_GOT owns peer
custody, partial transfer never imports, and durable queues survive restart/restore.
Schema **24**; en-US **1.21.0 / 1,071 messages**. Small sfconfig policy/credential,
Poll/Test/Hold/Release and sfmonitor status surfaces are implemented.

[Manual](docs/manual/binkp.md), [Technical Reference](docs/technical/binkp.md) and
[M048 report](docs/research/m048-networking-n4-binkp.md) record exact evidence and
limits. TLS and persistent partial resume are deferred. Live public FidoNet traffic:
**NONE**; participation/membership is not claimed. Windows live BinkP remains
**DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**. QWK N1/N2 and FTN N3 remain
accepted; native messages are canonical; SMB remains unimplemented.

B-021 VERIFIED; B-022 NOT STARTED. No AreaFix/FileEcho/TIC/FREQ, scheduler, doors,
production/DDEV/FireComm or release work. The existing preview binary is unchanged.
Public N4 gates: **578 passed / 0 failed / 4 ignored**, doctests, **99 headers**,
fmt/Clippy/diff and **114 Markdown/link checks** pass. Privacy/provenance scans
exclude private corpus, peer credentials/artifacts and private history.

Exact next separately scoped action: N5 operator surfaces/recovery ergonomics;
stop this pass after N4 publication. Private source checkpoint: `d95b853a17dc73cc358c49ec7c794b9a6882d27f`.

## Previous checkpoint: N3 native FTN core — COMPLETE / ACCEPTED

Current source implements the **native FTN/FidoNet core** on schema **23**:
NetMail, transit, EchoMail, points, multiple domains/AKAs, scanner/tosser,
explicit routing, durable shared queues and nodelist/pointlist directory authority.
Independent Synchronet/SBBSecho packet interoperability passed in both directions,
including points and a forwarded return loop. SPITFIRE messages remain canonical;
QWK N1/N2 remain accepted. [FTN Sysop procedure](docs/manual/ftn-core.md),
[Technical Reference](docs/technical/ftn-core.md) and
[M047 report](docs/research/m047-networking-n3-ftn-core.md) state exact scope.
At that N3 checkpoint BinkP was not implemented; N4 above supersedes that boundary.
Live public FidoNet participation is not claimed.
SMB remains unimplemented. B-021 VERIFIED; B-022 NOT STARTED. en-US 1.20.0 /
1,051 messages. The 0.1.0 preview binary remains unchanged; no new release or tag.
Windows live FTN acceptance remains deferred to a real Windows environment.

At the N3 checkpoint, the next separately scoped action was N4 BinkP over accepted FTN queues/artifacts,
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
Public validation: 535 tests, zero failures, two existing ignored, doctests, 82
headers, fmt, Clippy, diff and 108 Markdown/link checks pass. cargo-audit is unavailable.


## Development Preview 0.1.0

SPITFIRE NG 0.1.0 Development Preview is publicly available. The published
Apple Silicon macOS release passed packaging, runtime, terminal-client,
backup/restore, license, privacy, public-redownload, checksum, and first-run
checks.

The public `main` branch is now ahead of that binary release. Current source
adds post-0.1.0 advanced message discovery, auditable message mutation, an
auditable caller-access lifecycle, schema-13 caller identity, secure SSH caller
transport, schema-14 privacy-bounded public information, schema-15 file
inspection/maintenance, schema-16 transfer policy and storage, and the
schema-17 zero-byte invariant. Schema 18 adds privacy-safe operational events,
daily summaries, retention, notifications, and status/maintenance projections.
Schema 19 adds cross-platform protected read-only operator attachment, the
command-receipt/control-audit foundation, the `spitfire operator` client, and
the `sfmonitor` 0.1 operator application. Completed B021-B adds explicitly
authorized live controls over the read-only bootstrap boundary: acknowledgement,
session time, page/chat, caller disconnect, and daemon-only shutdown. The downloadable 0.1.0
archive is unchanged and does not contain those additions.

## Current source additions

The Category-B ledger now records 15 VERIFIED, 2 IMPLEMENTED, 3 PARTIAL, and
5 NOT STARTED rows. B-024, B-011, B-014, and B-023 are VERIFIED, and M039
Tranche 6 is semantically closed. B-017 and B-021 are VERIFIED;
B-022 remains NOT STARTED.

B021-B1/B2/B3 and B021-B are COMPLETE / ACCEPTED as an internal B-021 slice.
B021-C typed configuration / sfconfig and B021-D operator integration are
COMPLETE / ACCEPTED. All accepted B-021 stock outcomes are satisfied. Schema remains 19; all sixteen current capabilities fit the bounded
32-entry profile, and mutations require explicit enrollment. Chat is ephemeral;
transfer/file/accounting authority is preserved. Q quits only sfmonitor, while
confirmed Shutdown SPITFIRE NG stops only the selected daemon. No restart exists.
See the [integrated acceptance report](docs/research/m039-tranche-7-b021b3-shutdown-integrated.md).

Native macOS integrated acceptance passed. Windows source architecture and prior
B021-A attachment acceptance remain; live Windows B021-B/C/D mutation, monitor/config rendering and input, maintenance
execution, terminal handoff, chat/disconnect, transfer shutdown, and overlap remain **DEFERRED —
REAL WINDOWS ENVIRONMENT REQUIRED**. No new Windows live/build acceptance is
claimed from this source synchronization. No new release, tag, or binary was issued.

- Stock-style Message-menu Specific Caller and Text Search
- Current, all, or queued conference scope
- Existing message visibility/security reused for every search and result open
- Read-only discovery with no last-read or receipt mutation
- ASCII-case-insensitive body substring matching; Subject is not searched
- All whitespace-delimited terms required regardless of order or separation
- One primary recipient plus up to nine separately numbered CC deliveries
- Per-delivery receipt, public/private visibility, and deleted/tombstone state
- Authorized caller deletion plus threshold-Sysop Delete, contextual
  Undelete, and public/private audience transitions
- Source-retaining same/cross-conference Copy and Forward through Copy with a
  changed recipient
- Schema-11 immutable payloads, delivery identities, Copy/Forward lineage,
  privacy-safe mutation audit, and state-version conflict protection
- Schema-10→11 migration plus schema-10/schema-11 cold backup and restore
- Active, Disabled/Locked Out, and recoverable Deleted caller lifecycle
- Disable/enable, tombstone/restore, and persisted purge protection without a
  physical purge
- Operator-assigned base security plus derived effective security and
  reasoned adjustments
- Board-local nullable subscription expiration, warning window, post-date
  expiry, and renewal that restores current base security where appropriate
- Bounded `JOKER.DAT` complete-name and `@` substring policy with generic,
  privacy-safe denials
- Named-Sysop identity protection separate from threshold privilege
- Active-session invalidation and lifecycle/security reauthorization at every
  main, message, and file command dispatch
- Append-only privacy-safe caller-access audit and optimistic state-version
  conflict handling
- Transactional schema-11→12 migration plus exact schema-10/schema-11/
  schema-12 cold backup and restore
- Schema-13 separation of stable caller ID, normalized login identifier,
  public handle, and optional private real name without rewriting historical
  attribution or merging migration collisions
- Disabled-by-default SSH-2 caller transport through the common node/session
  engine, using ordinary SQLite/Argon2id caller authority and no second BBS
  password prompt
- Board-local Ed25519 host identity, modern `russh` defaults, bounded
  authentication resources, PTY/resize/encoding propagation, lifecycle
  invalidation, privacy-safe diagnostics, and cold-backup continuity
- No OS shell, Unix account login, SCP, SFTP, command execution, forwarding,
  remote filesystem, X11, agent, or subsystem access
- Transactional schema-13→14 migration with private defaults, rollback, and
  exact schema-13 restore followed by writable migration
- Board-disabled and caller-opt-in public directory using only Active callers'
  public handles, plus policy-controlled board-local last-call/location fields
- Bounded handle-only partial locate with deterministic ordering, a 50-result
  cap, sequential confirmation, and visibility recheck before disclosure
- Native ordered/versioned Other BBS authority with stable IDs, lifecycle,
  optional stable contributor identity, conflict-safe operator maintenance,
  and caller additions disabled by default
- Board-owned numbered bulletins, newsletter, safe system facts, and bounded
  project-native `THOUGHTS.NG` through the shared resource, presentation,
  localization, paging, and encoding boundaries
- Privacy-safe semantic audit and cold recovery for directory policy,
  publicity state, Other BBS rows, resource generations, and board resources
- Transactional schema-14→15 migration preserving callers, messages, files,
  transfers, SSH, and public-information authority without fabricated file
  requests, review rows, or operation journals
- Stable file IDs with separate Active/Offline/PendingReview/Disabled/
  Tombstoned lifecycle and Unknown/Present/Missing/DigestMismatch integrity
- Confined, bounded, sanitized text inspection and metadata-only Stored/
  Deflated ZIP inspection with no extraction or execution
- Bounded FILE_ID.DIZ discovery and explicit versioned review before any
  authoritative description replacement
- Preview-area inspection separated from upload/download authority
- Private versioned Offline/Missing requests, PendingReview uploads, duplicate
  warnings, native SFNOUP denial rules, and optional description normalization
- Typed, versioned staged file maintenance with leases, name reservations,
  semantic audit, crash reconciliation, recoverable tombstones, and legacy
  SFFILES publication while SQLite plus confined managed bytes remain native
  authority
- Schema-15 cold backup and restore, including safe rejection of nonterminal
  operations and exact schema-14 restore followed by writable migration
- Transactional schema-15→16 transfer policy, board-day usage, atomic quota
  reservation, idempotent settlement, transfer history, storage roots, and
  per-file locators
- Session-ephemeral stable-FileId queues, multi-file YMODEM/YMODEM-g/ZMODEM,
  cancellation, per-item reauthorization, and multinode-safe accounting
- Native file-count/byte ratios, DAILYLMT policy, no-charge accounting,
  Preview denial before negotiation, and capped completed-upload time credit
- Managed and external read-only storage, versioned rebind/probe,
  StorageUnavailable distinct from Missing, active-use conflicts, and bounded
  seekable large-source streaming
- All nine B-024 choices: ASCII, XMODEM Checksum, XMODEM CRC, 1K-XMODEM,
  1K-XMODEM-g, YMODEM Batch, YMODEM-g Batch, ZMODEM Batch, and TeLink
- Transactional schema-16→17 migration with rollback and exact zero-byte
  catalog, upload, protocol, backup, and restore support
- Transactional schema-18→19 migration with bounded command receipts,
  append-only control audit, no fabricated history, and schema-19 cold recovery
- Protected board-local Unix-domain-socket and Windows named-pipe operator
  attachment using verified peer UID/SID, an explicit allowlist, restrictive
  permissions/DACL, one-use challenge binding, daemon generation, protocol and
  capability negotiation, and dispatch-time authorization
- Shared `OperatorClient` and localized read-only `spitfire operator` status,
  nodes, events/live events, notifications, statistics, callers, and
  maintenance commands backed by the existing B-017 projections
- Read-only-by-default `sfmonitor` with Dashboard, Nodes/detail, Callers, Activity,
  Statistics, Notifications, Maintenance / Errors, contextual help, typed
  filters, visible gap recovery, manual reconnect, and responsive terminal
  layouts over the same `OperatorClient`, plus explicitly enrolled B021-B
  acknowledgement, time, page/chat, disconnect, and daemon shutdown Actions

The all-term behavior intentionally improves the historical contiguous-phrase
limitation without changing SPITFIRE's Text Search command flow, conference
selection, visibility, or result presentation.

## Available today

- Stock SPITFIRE 3.7 Core Parity for the defined core scope
- ANSI/text caller and operator experience parity
- Modern, Classic SPITFIRE-inspired, and Minimal Terminal profiles
- Generated stock menus and exact-security `.BBS`/`.CLR` overrides
- Telnet, RAW TCP, and RLogin compatibility listeners
- Secure SSH caller transport when built from current post-0.1.0 source
- Caller registration, authentication, privacy, profiles, and security levels
- Message conferences, mail, replies, threads, queues, and receipts
- Advanced caller/text discovery and auditable message mutation when built
  from current post-0.1.0 source
- Auditable caller lifecycle, base/effective security, subscription policy,
  JOKER name denial, named-Sysop protection, and schema-12 recovery when built
  from current post-0.1.0 source
- Privacy-bounded caller directory, locate, Other BBS, bulletins, newsletter,
  system information, and native thoughts when built from current post-0.1.0
  source
- File areas, catalogs, search, uploads, downloads, and new-file checks
- Schema-15 bounded text/ZIP inspection, Preview inspection, private requests,
  PendingReview, and staged maintenance when built from current post-0.1.0
  source. B-013 is VERIFIED; B-015 and B-012 remain IMPLEMENTED.
- Schema-16/17 transfer, batch-policy, accounting, and extended-storage source.
  B-024, B-011, B-014, and B-023 are VERIFIED.
- Schema-18 privacy-bounded operational events, board-day statistics,
  retention, notifications, board/node status, recent caller and activity
  projections, and maintenance/error views. B-017 is VERIFIED.
- Schema-19 protected local operator attachment on Unix/macOS and Windows,
  with the read-only operator CLI and reusable `OperatorClient`. B021-A is
  cross-platform complete; B-021 is VERIFIED after B021-D closure.
- Capability-aware `sfmonitor` live controls from current source. Real visual/interaction
  acceptance passed on Apple Silicon macOS. Interactive rendered Windows TUI
  acceptance is deferred until a suitable real Windows environment is
  available.
- ASCII, XMODEM, YMODEM, ZMODEM, and TeLink transfer support as documented
- Multinode runtime and session isolation
- Operator configuration, status, and renderer diagnostics
- Cold backup, restore, upgrade-preservation, and rollback procedures
- Versioned presentation and language packages with an en-US baseline
- Verified Moebius 1.0.29 `.CLR` authoring on macOS

## Current binary

| Item | Status |
|---|---|
| Version | 0.1.0 |
| Channel | Development Preview |
| Tag | `v0.1.0-development-preview` |
| Platform | Apple Silicon macOS |
| Target | `aarch64-apple-darwin` |
| Archive | `spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz` |
| SHA-256 | `6c4d7ad492b1acee92481a3a577b49934c08e79822e98de50e918489a8fc9c97` |
| Signing | No Apple Developer ID signature |
| Notarization | Not notarized |
| Publication | [Published on GitHub](https://github.com/cdaters/spitfire-ng/releases/tag/v0.1.0-development-preview) |

This table describes the downloadable binary release, not every later source
change on `main`.

The archive was downloaded again from the public GitHub Release and matched
the canonical SHA-256. Expected unsigned/unnotarized Gatekeeper behavior was
observed on Apple Silicon macOS; **System Settings → Privacy & Security → Open
Anyway** succeeded, after which `spitfire --version` returned:

```text
SPITFIRE NG Bulletin Board System 0.1.0
```

Apple Developer ID signing and notarization are intentionally deferred and do
not block this Development Preview.

Only this target has completed package and live-client acceptance. Source code
is intended to remain portable, but other prebuilt platforms are not claimed
until they are built and tested.

## Not implemented yet

- RIP graphics and RIP terminal behavior
- Caller-selectable presentation profiles
- Production non-English translations and caller locale selection
- Remaining advanced Category-B commands and resources
- FTN/FidoNet networking and BinkP
- CircuitNet adapter/revival work beyond preserved compatibility knowledge
- Web administration
- SFDraw, the planned display-authoring companion tool
- SFDATE and SFREG preservation tools

These are future directions, not partially shipped features.

## Maturity and support

Development Preview means the documented workflows are usable and tested, not
that production hardening or stable 1.0 compatibility is complete. Preview
upgrades should be paired with a cold backup and the previous executable.
Telnet, RAW, and RLogin do not encrypt caller credentials or session data.

See [Support and Bug Reports](docs/operator/support.md),
[Security](SECURITY.md), and the [Roadmap](ROADMAP.md).

## Next step

Stop after the accepted B021-D / B-021 source closure. B021-A/B/C remain
accepted; B-021 is VERIFIED and B-022 remains NOT STARTED. Any next development
requires a separately scoped B-022 interface/resource/transaction gate. Preserve
the unchanged 0.1.0 release boundary; no excluded implementation begins here.

## B021-C configuration source milestone

The separate [sfconfig application](docs/manual/sfconfig.md) works directly or
through sfmonitor System Configuration. It provides typed staged edits, shared
validation, semantic review/save/cancel, conflict reload, contextual keyboard
help, and eight implemented sections. Messages/Files show actual read-only
summaries; storage/backup guidance retains existing cold-board maintenance.

The daemon is sole online configuration authority. Explicit offline mode holds
the board lock. Revision/digest CAS rejects stale writers; complete-file atomic
replacement, one prior configuration, and existing schema-19 receipts provide
recoverable saves. Operator profiles require explicit bounded enrollment;
bootstrap stays read-only. SSH key state is status-only and opaque device
commands are redacted. Live/new-session/restart effects are explicit; there is
no restart command. See the [technical reference](docs/technical/configuration.md)
and [acceptance report](docs/research/m039-tranche-7-b021c-sfconfig.md).

Native Apple Silicon macOS acceptance covers terminal handoff/return, two
sfconfig clients, caller continuity, live permissions, new-session policy,
external restart, offline editing, and cold recovery. Windows named-pipe config,
SID enrollment UI, rendered TUI/handoff, and filesystem behavior remain
**DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**. Linux/BSD source architecture
is preserved without a new runtime claim. At the B021-C checkpoint, en-US was 1.15.0 / 984 semantic messages.
The downloadable Development Preview is unchanged; no release/tag/binary is added.

## B021-D closure

The [closure report](docs/research/m039-tranche-7-b021d-operator-closure.md) maps
stock operator outcomes to existing native owners. Setup remains first-run and
offline recovery preparation; sfmonitor is the live cockpit; sfconfig is the
separate typed online/offline configuration environment. Maintenance adds shared
owner guidance, not execute buttons. No B-018 pack/purge or B-022 output is imported.

At the B-021 closure, en-US was 1.16.0 / 988 messages. The supported [recovery journey](docs/manual/operator-recovery.md)
covers deliberate local reenrollment, invalid configuration, exclusive offline
authority, known-good cold restore, conflicts and loss of online authority.
B-022 remains NOT STARTED. No networking, doors, scheduler or release work began.

Public closure gates pass 481 tests / 2 existing ignored, doctests, 71 source
headers, fmt, Clippy with warnings denied, diff hygiene, documentation links,
localization and publication privacy/provenance scans. cargo-audit is unavailable.

## Previous publication accounting — N3

Public publication scope: 16 added files and 48 updated files, including schema 23,
FTN codecs/native services/directory authority, synthetic tests, operator surfaces
and en-US 1.20.0. All 37 changed source/catalog/manifest files match accepted
private `fa44a3aca2c66825f5e291c31c66aaa385a3c287` exactly. Public history is
independent. No FTSC corpus/index, downloaded wiki/source, private list sample,
peer artifact, credential or private continuity document is included.

Final public gates: **558 passed / 0 failed / 3 ignored**, including the two prior
ignored tests and the independently supplied opt-in test (passed explicitly in
private acceptance). Doctests, **94 headers**, formatting, Clippy with warnings
denied, diff checks and **111 Markdown/local-link/anchor/fence files** pass.
The corrected directory-source case is included. Added-text privacy/provenance and
corpus-digest scans pass. cargo-audit remains unavailable. No real Windows runtime
acceptance, BinkP, live public FTN traffic or release is claimed.
