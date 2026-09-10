# SPITFIRE NG — D3 menus and messages

D3 completes the local caller message journey: Main → Messages, permitted
conferences, paged indexes, sequential reads, linked replies, new-message scans,
confirmed posts, menu return and Goodbye. Selection survives menu round-trips;
read positions are caller-specific and durable. Current access is checked again
when reading and saving. Cancellation or disconnection publishes no partial post.

[Caller guide](docs/caller-guide/README.md),
[message and conference configuration](docs/operator/messages.md),
[message contract](docs/sfng-message-system.md), and
[login/session policy](docs/operator/caller-management.md).

D1 and D2 remain accepted. Schema **37**, configuration format **2**, local
operator protocol **1.19** and CIRCUITNET-NG **1.4** are unchanged. Embedded en-US
is **1.37.0**. Native identity, frozen author, immutable reply parent and existing
network preparation remain authoritative. No adapter or deployment semantics
changed. This is current source; no new prebuilt binary archive or production
deployment is implied.

**D3 COMPLETE / ACCEPTED.** Independent public workspace: **824 passed / 0
failed / 8 existing ignored**, all 27 result groups including six empty doctest
groups. Real Apple Silicon native D3 Telnet **4 passed**, D2 **4 passed**, D1
deployment **11 passed**, backup/restore **21 passed**, separate native D1 CLI
preservation journey **1 passed**. FTN/BinkP, QWK and CircuitNET regressions pass.
Fmt, strict workspace/all-target/all-feature Clippy, diff, **183 source headers**
and **182 Markdown / 1304 inline local path links / 0 missing targets** pass.
Link checks exclude remote URLs and fragment anchors. Intel macOS compilation
passes; real acceptance uses disposable local boards on Apple Silicon macOS.

The line editor remains the supported composition interface; durable drafts and
a full-screen editor are outside D3. Read state is a per-conference high-water,
so displaying a higher number makes earlier messages old. Linked navigation is
bounded to 1000 connected messages. Real non-Apple-Silicon platform acceptance
is deferred. Cargo-audit is unavailable on the acceptance host.

Exact next action after publication: **Work/DDEV website progress refresh, then
review, followed by D4 only after authorization.** Neither website nor D4 is part
of this milestone.

## Previous accepted D1 checkpoint

D1 adds authenticated local release packages, stopped-board updates, verified
pre-upgrade checkpoints, backup/storage policy and compatible runtime rollback.
An update failure before commit can recover the prior database/configuration.
Runtime rollback after commit preserves current board data and network evidence;
an incompatible previous runtime is refused. Manual disaster restore is separate
and destructive. An interrupted manual restore can require reviewed intervention;
D1 has no automatic finalization command for ambiguous publication.

[Updating and runtime rollback](docs/operator/upgrades.md),
[backup and manual-restore recovery](docs/operator/backup-restore.md),
[storage policy and validation limits](docs/operator/deployment-storage.md),
[deployment contract](docs/technical/deployment.md).

Schema **36**, CIRCUITNET-NG **1.4** and FTN/QWK semantics remain unchanged.
Release authority is separate from CircuitNET catalog trust. en-US advances to
**1.35.0**. This is current-source publication,
not a new prebuilt binary archive. No production board or public service changed.

There is no full installer/service manager, live update, whole-host/cloud backup,
automatic pruning or classic 3.7 migration. Large managed libraries still require
validation reads with metadata-only backups; terabyte-scale acceptance is not
claimed. Real Windows activation/reboot acceptance remains deferred.

Next major milestone: **D2 — Caller login, new-user, and session completeness**.
Exact next action: **review D1 publication, then authorize D2**. D2 has not begun.

## Previous accepted C9 checkpoint

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

## Previous accepted C8.1 checkpoint

<p align="center">

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

  <img src="docs/assets/branding/spitfire-ng-banner.png"
       alt="SPITFIRE NG — Next Generation BBS Software"
       width="1200">
</p>

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


# SPITFIRE NG


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

Earlier sections retain their historical checkpoint scope.

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


The earlier identity milestone introduced [account and posting identity policy](docs/technical/identity-policy.md)
on schema **28**. Handle remains the normal public name; First and Last Name are
private by default. Callers see the posting identity before submission. Historical
authors and queued network senders retain the name originally used, even after
profile changes. Configured conference/network requirements determine real-name
use; FTN does not universally force it.

The [N7 file-network layer](docs/manual/ftn-files.md) adds FileEcho, authenticated
TIC processing, native-file hatching and explicitly authorized FREQ to the
accepted FTN/BinkP stack. Native messages and files remain canonical; schema 28;
en-US 1.25.0 / 1,308 messages. No public FidoNet traffic or new binary release.

The [N6 FTN hub services](docs/manual/ftn-hub.md) add durable downstream
subscriptions, authenticated AreaFix, bounded rescan and point-boss operation to
the accepted native FTN/BinkP engine. Native messages remain canonical; schema 25;
en-US 1.23.0 / 1,249 messages. No live public FidoNet traffic or new binary release.

The [N5 Networks cockpit and recovery](docs/manual/network-operations.md) operate
accepted QWK/DOVE-compatible networking, native FTN and BinkP through sfmonitor
and named sfconfig forms. Verified restore protects origin identity and accepted
queue receipts. CircuitNET now has the independent offline adapter described above. No live public FidoNet participation is claimed.

Native BinkP client/listener and controlled independent NetMail/EchoMail exchange
are implemented on schema 24, including point/AKA identity and safe queue recovery.
QWK N1/N2 and FTN N3 remain accepted. en-US is 1.21.0 / 1,071 messages; B-021
VERIFIED, B-022 NOT STARTED. The published preview binary and tags are unchanged.
See [N4 acceptance](docs/research/m048-networking-n4-binkp.md) and the
[Sysop guide](docs/manual/binkp.md). Live public FidoNet participation is not claimed.

SPITFIRE NG is a modern, cross-platform reimplementation of the SPITFIRE
Bulletin Board System. It preserves the caller experience and operating model
that made SPITFIRE recognizable while replacing DOS-era constraints with safe,
maintainable Rust code.

The project is independently implemented. It is not an original Buffalo Creek
Software executable, an official historical release, or SPITFIRE 3.7 itself.

## What is SPITFIRE NG?

SPITFIRE NG is both a usable BBS and a preservation project. Its guiding rule
is simple:

> If a behavior is part of SPITFIRE's identity, preserve it. If it is merely a
> DOS or hardware limitation, modernize it.

That means familiar security levels, command-driven menus, conferences, file
areas, caller statistics, Sysop interaction, and editable displays—backed by
modern authentication, SQLite storage, portable paths, reliable terminal
input, multinode isolation, and tested backup and restore.

## Current status

SPITFIRE NG 0.1.0 is a publicly available **Development Preview**. Source,
operator documentation, and the accepted Apple Silicon macOS binary are
published in this repository.

The latest downloadable binary remains 0.1.0. The `main` branch now contains
post-0.1.0 source improvements, including advanced caller/text message
discovery, auditable message mutation, an auditable caller-access lifecycle,
schema-13 caller identity separation, secure SSH caller transport, and
schema-14 privacy-bounded public information. Current source also includes
schema-15 safe file inspection, private file requests and review, staged file
maintenance, schema-16 batch transfer policy and logical storage, and the
schema-17 zero-byte file invariant. Schema 18 adds privacy-safe operational
events, daily statistics, retention, notifications, and board/node/maintenance
projections. Schema 19 adds protected read-only operator
attachment on Unix/macOS and Windows plus the reusable `OperatorClient` and
`spitfire operator` CLI. Current source also includes the read-only-by-default
`sfmonitor` 0.1 local operator application over that same client. The completed
B021-B slice adds explicitly enrolled acknowledgement, +/-5 session time,
page/chat, graceful caller disconnect, and confirmed daemon-only shutdown.
Bootstrap remains read-only; chat is never durably persisted. Those
improvements are not present in the published 0.1.0 archive.

The public Category-B ledger is now **15 VERIFIED, 2 IMPLEMENTED, 3 PARTIAL,
and 5 NOT STARTED**. B-024 transfer interoperability, B-011 batch queues,
B-014 transfer policy/accounting, and B-023 extended storage are VERIFIED;
M039 Tranche 6 is semantically closed. B-017 observability is also VERIFIED;
B-021 operator controls are VERIFIED and B-022 report publication remains
NOT STARTED.

B021-A/B/C remain accepted and B021-D is COMPLETE / ACCEPTED. The typed
`sfconfig` environment and `sfmonitor` handoff now join coherent setup, permissions,
maintenance guidance and cold recovery. See the [closure evidence](docs/research/m039-tranche-7-b021d-operator-closure.md)
and [operator recovery](docs/manual/operator-recovery.md). Schema remains 19;
Current en-US is 1.21.0. Windows live integrated operator acceptance remains
DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED. This source update creates no release.

Available today:

- accepted Stock SPITFIRE 3.7 Core Parity;
- accepted ANSI/text caller and operator experience parity;
- Modern, Classic SPITFIRE-inspired, and Minimal Terminal presentation
  profiles;
- engine-generated stock menus and exact-security display overrides;
- Telnet, RAW TCP, and RLogin compatibility listeners plus—when built from
  current source—a disabled-by-default SSH-2 caller listener through the same
  session engine, with no host OS shell route;
- caller registration, Argon2id authentication, profiles, and security plus—
  when built from current source—disable/restore, recoverable deletion,
  base/effective security, subscription expiry/warnings/renewal, JOKER name
  denial, named-Sysop protection, active-session invalidation, and separate
  normalized login identifier/public handle/optional private real name;
- when built from current source, a disabled-by-default, caller-opt-in public
  directory and handle-only locate; versioned Other BBS entries; numbered
  bulletins; newsletter; safe system information; and a bounded native thought
  catalog, without exposing login identifiers, private real names, or contact
  data;
- messages, conferences, private mail, replies, queues, receipts, and—when
  built from current source—bounded Specific Caller/Text Search, primary plus
  CC delivery, authorized Delete/Undelete, audience transitions, and
  source-retaining Copy/Forward;
- file areas, uploads, downloads, new-file checks, and supported transfer
  protocols plus—when built from current source—bounded text/ZIP inspection,
  Preview inspection without transfer, private Offline/Missing requests,
  PendingReview uploads, versioned staged file maintenance, session-ephemeral
  stable-ID batch queues, atomic daily/ratio accounting, logical read-only
  storage roots, bounded large-source streaming, and all nine B-024 transfer
  choices including TeLink;
- when built from current source, schema-18 activity history, board-day
  statistics, bounded retention, actionable notifications, and privacy-safe
  board, node, recent-caller, error, and maintenance services;
- when built from current source, schema-19 protected local operator
  attachment through Unix-domain sockets or Windows named pipes, verified OS
  peer identity and board allowlists, protocol/capability negotiation, and
  read-only status, node, event, notification, statistics, caller, and
  maintenance CLI commands;
- when built from current source, the keyboard-complete, responsive,
  `sfmonitor` application for Dashboard, Nodes, Callers, Activity,
  Statistics, Notifications, and read-only Maintenance / Errors views, with
  explicitly authorized live controls through Actions; Q quits only the monitor;
- multinode operation, operator status, cold backup, and restore, including
  schema-19 command/control audit, schema-18 observability state, schema-17
  file/transfer/storage state,
  schema-14 public-information state, caller
  identity/access state, SSH configuration, and SSH host-key continuity when
  built from current source;
- an en-US localization baseline and versioned language-pack interface; and
- a verified Moebius 1.0.29 workflow for authoring `.CLR` screens on macOS.

The current prebuilt target is Apple Silicon macOS
(`aarch64-apple-darwin`). It is unsigned and unnotarized. See
[Status](STATUS.md) for the exact release boundary and current limitations.

## Development Preview

The published release is:

```text
spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz
SHA-256: 6c4d7ad492b1acee92481a3a577b49934c08e79822e98de50e918489a8fc9c97
```

Download the archive, checksum, manifest, and release notes together from the
[official `v0.1.0-development-preview` GitHub Release](https://github.com/cdaters/spitfire-ng/releases/tag/v0.1.0-development-preview).
The published archive was downloaded again, matched the SHA-256 above, and
returned `SPITFIRE NG Bulletin Board System 0.1.0` on Apple Silicon macOS.
Synchronizing newer source does not change that accepted archive, tag, or
checksum.

## Installation

The Development Preview package contains a prebuilt `spitfire` executable,
operator documentation, release metadata, licenses, and dependency notices.
Git and Cargo are not required for normal operation.

Start with:

1. [Development Preview Package](docs/operator/development-preview-package.md)
2. [macOS First Run](docs/operator/macos-first-run.md)
3. [Getting Started](docs/operator/getting-started.md)

Developers can build from source with:

```sh
cargo build --release --locked -p sf-bbs -p sf-monitor
./target/release/spitfire --version
./target/release/sfmonitor --version
```

## Quick start

After verifying and extracting the package:

```sh
./bin/spitfire setup /path/to/your-board
./bin/spitfire status /path/to/your-board
./bin/spitfire run /path/to/your-board
```

Setup creates a self-contained board with configuration, data directories,
presentation profiles, and the en-US language package. The operator guide
explains listener configuration, first calls, messages, files, and backups.
For the complete, verified current-source journey, use the
[SPITFIRE NG Quick Start](docs/manual/quick-start.md).

## Documentation

- [Documentation index](docs/README.md)
- [Sysop Reference Manual](docs/manual/README.md)
- [Caller Guide](docs/caller-guide/README.md)
- [Technical Reference](docs/technical/README.md)
- [Operator guide](docs/operator/README.md)
- [Configuration](docs/operator/configuration.md)
- [Architecture](docs/04-system-architecture.md)
- [Compatibility principles](docs/02-compatibility-principles.md)
- [Presentation profiles](docs/presentation-profiles.md)
- [Localization](docs/localization.md)
- [Caller access lifecycle and security](docs/sfng-caller-access.md)
- [Secure SSH caller transport](docs/sfng-secure-ssh-transport.md)
- [Privacy-bounded public information](docs/sfng-public-information.md)
- [Native file system and schema-17 authority](docs/sfng-file-system.md)
- [Tranche 5 implementation](docs/research/m039-tranche-5-safe-file-inspection-request-maintenance-implementation.md)
- [Tranche 5 verification](docs/research/m039-tranche-5-verification.md)
- [Tranche 6 implementation](docs/research/m039-tranche-6-batch-transfer-policy-extended-storage-implementation.md)
- [Tranche 6 verification](docs/research/m039-tranche-6-verification.md)
- [Independent transfer interoperability](docs/research/m039-tranche-6-transfer-interoperability.md)
- [Board activity and system statistics](docs/manual/board-activity.md)
- [Schema-18 observability technical reference](docs/technical/observability.md)
- [Tranche 7 observability gate](docs/research/m039-tranche-7-operator-observability-reports-gate.md)
- [B-017 implementation and verification](docs/research/m039-tranche-7-b017-observability-implementation.md)
- [Schema-19 protected operator attachment](docs/technical/operator-control.md)
- [Using sfmonitor](docs/manual/sfmonitor.md)
- [sfmonitor technical architecture](docs/technical/sfmonitor.md)
- [sfmonitor 0.1 implementation](docs/research/m039-sfmonitor-read-only-mvp.md)
- [B-021 operator-controls gate](docs/research/m039-tranche-7-b021-operator-controls-gate.md)
- [B021-A implementation](docs/research/m039-tranche-7-b021a-protected-operator-attachment.md)
- [B021-AW Windows acceptance](docs/research/m039-tranche-7-b021aw-windows-operator-attachment.md)
- [Cross-project reference policy](docs/cross-project-reference-policy.md)
- [Roadmap](ROADMAP.md)

## Custom ANSI screens

Sysops can customize a board without editing managed profile packages. Put
board-owned overrides in the board's `display/` directory; they take
precedence over active-profile resources, with generated menus as the final
fallback.

The [Customizing Display Screens](docs/operator/custom-display-screens.md)
guide includes exact-security filenames and the verified Moebius 1.0.29 macOS
workflow: IBM VGA/CP437, 16-color ANSI, iCE colors off, static ANSI, **Save
Without Sauce Info**, and no UTF-8 export.

## Compatibility and original SPITFIRE

SPITFIRE NG uses original documentation and legally held local artifacts as
read-only evidence. Proprietary binaries, registered copies, DISPLAY files,
and other historical assets are not distributed in this repository.

For original software, manuals, and preservation downloads, visit
[Original SPITFIRE Software & Documentation](https://spitfirebbs.com/).
The [historical overview](docs/HISTORICAL-SPITFIRE.md) and
[parity checklist](docs/stock-spitfire-3.7-parity.md) explain how historical
behavior maps to the modern implementation.

## What is not included yet?

The Development Preview does not include RIP graphics, caller-selectable
presentation profiles, production non-English translations, the remaining
advanced Category-B command set, exact original LAKOTA compatibility, FidoNet/CircuitNet,
web administration, SFDraw, SFDATE, or SFREG. The 0.1.0 downloadable binary
also predates QWK offline mail, SSH, schemas 13–24, public-information additions, Tranche 5 file
inspection/request/maintenance, Tranche 6 transfer/storage source, B-017
observability, B021-A protected operator attachment, `sfmonitor`, and B021-B
live controls and B021-C configuration / `sfconfig`. Current source includes
the [native sfconfig application](docs/manual/sfconfig.md), typed versioned
configuration authority, and sfmonitor System Configuration handoff. B021-D integration is accepted; B-021 is VERIFIED.
B-022 report publication remains NOT STARTED.

Traditional Telnet, RAW, and RLogin transports are plaintext compatibility
features. Use them only on networks where that risk is understood.

## Contributing

Contributions are welcome from BBS developers, Sysops, preservationists, and
retro-computing enthusiasts. Read [CONTRIBUTING.md](CONTRIBUTING.md) before
submitting code, format research, documentation, or presentation resources.
Project-source changes can be checked with
[`tools/verify-source-headers.rb`](tools/verify-source-headers.rb).

Historical compatibility claims need evidence. Never commit proprietary
software, private caller data, registered binaries, or unlicensed artwork.

## Support and security

Use the public issue tracker for reproducible, sanitized bugs. Include the
SPITFIRE NG version, platform, terminal client, transport, terminal size and
encoding, active profile, and reproduction steps. See
[Support and Bug Reports](docs/operator/support.md).

Security vulnerabilities should be reported privately according to
[SECURITY.md](SECURITY.md). Never post passwords, caller databases, secrets,
private messages, or registered historical binaries.

## License and provenance

Original SPITFIRE NG source code and project-authored distributable resources
are available under **MIT OR Apache-2.0**, at your option. See
[LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

That license does not relicense original Buffalo Creek Software material,
third-party research archives, external source code, or community packages.
Each independently distributed presentation or language package retains its
own license and provenance metadata. See
[Licensing and Provenance](docs/licensing-and-provenance.md) for the complete
boundary.
