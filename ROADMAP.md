# SPITFIRE NG Roadmap

> Current source checkpoint: **M045 / N1 caller QWK offline COMPLETE / ACCEPTED**.
> Schema 20; QWK networking, DOVE-Net and FidoNet/BinkP remain unimplemented.


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
- QWK/offline mail and QWK networking;
- DOVE-Net interoperability and FidoNet NetMail/Echomail with BinkP;
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
