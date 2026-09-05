# M045 — N1 shared adapter foundation and caller QWK offline mail

Status: **N1 COMPLETE / ACCEPTED / PUBLIC SOURCE SYNCHRONIZED**. Date: 2026-09-05.
This report records accepted source functionality, not a new downloadable release.

## Scope and architecture

Published source checkpoint: `4cd3076ddeb29b04b318c08f456c052c2248a5c8`.
Starting public checkpoint: `97ea7b2cd04309dc21e71902d2acd3a35ff5118c`.
Schema **19 → 20**. Category B remains **15 VERIFIED / 2 IMPLEMENTED / 3 PARTIAL /
5 NOT STARTED**; **B-021 VERIFIED; B-022 NOT STARTED**. C-001 is PARTIAL because
original LAKOTA LMR compatibility remains evidence-qualified.

The binding [M044 architecture](m044-networking-foundation-gate.md) is followed:
`sf-net` provides reusable pure QWK format machinery, `sf-core::network` provides
native mappings/artifacts/receipts and typed caller import/export, and `sf-bbs`
owns restricted host storage and the existing session workers. The established
caller workflow modules remain in sf-core alongside other transport-neutral
message/file workflows. No unused future protocol hierarchy is introduced.

Native SPITFIRE NG message authority remains canonical. QWK is implemented as an
adapter/interchange format. No separate QWK message store exists. No SMB parsing,
writing, runtime dependency, migration or conference compatibility is implemented.
Native payloads remain existing CP437 bytes, as M044 requires; no schema-wide
Unicode reinterpretation occurs. Public Unicode handle conversion is explicit.

The [Technical Reference](../technical/qwk-offline.md) is the canonical detailed
contract for schema, record offsets, identity, atomicity, encoding, timestamps,
resource bounds, permissions, custody, transfer and recovery. The
[Caller Guide](../caller-guide/qwk-offline.md) and [Sysop chapter](../manual/qwk-offline.md)
provide practical procedures. This report records decisions and evidence without
duplicating those specifications.

## Format and reference provenance

| Source | Version/license and bounded purpose | Disposition |
|---|---|---|
| Primary SPITFIRE manual | Stock 3.7 §24.4, Messages menu evidence, read-only research | **ADOPT** L and D/U/S/Q, New/To You, All/Selected/Queued and post-transfer pointer confirmation; independently authored wording/resources |
| Companion LAKOTA manual | 2.0, read-only research | **ADAPT** ZIP QWK/REP, board identity, preview, queues/ADD/DROP; **DEFER** undocumented exact LMR bytes and routing |
| [Lee layout](https://wmcbrine.com/mmail/specs/qwklay.html) | QWK layout 1.6, format authority | **ADOPT** field/record framing, count-minus-one CONTROL list and Microsoft binary indexes |
| [Herring reference](https://wmcbrine.com/mmail/specs/qwk1st.html) | 1stReader programmer reference 2.0, creator's format documentation | **ADOPT/ADAPT** classic packet/REP requirements; explicit profile resolves extensions |
| [Rocca QWKE](https://wmcbrine.com/mmail/specs/qwke.html) | Peter Rocca 1.02, published format documentation | **ADOPT** long To/From/Subject and selected reader metadata; **REJECT** unimplemented attachments/control claims |
| [Synchronet](https://github.com/SynchronetBBS/sbbs) | Commit `60b70e526213285b61ae11f82dc8412ad5b405b4`, GPL-2.0-or-later; bounded qwk.h and relevant packet/parser workflow references | **ADOPT** explicit profiles and lifecycle separation; **ADAPT** conference/identity lessons; **REJECT** code/resource copying and SMB coupling; **DEFER** generic network routing and DOVE-specific conventions to N2 |
| [MultiMail](https://github.com/wmcbrine/MultiMail) | 0.52, commit `e1dca54b0d40ca45cd99f0077621fde1b08f9450`, GPL-3.0-or-later | **ADOPT** independent reader/reply acceptance; **ADAPT** observed interoperability through primary format references; **REJECT** implementation/resource copying |
| Independent FireComm reference | Bounded file-transfer engineering document review, read-only | **ADAPT** exclusive binary-stream ownership, bounded cancellation and safe filenames through existing NG transfer engines; no source/runtime dependency or project modification |

No reference code, comments, UI, configuration, original sample bytes, external
reader assets or private packets are committed. Third-party peers were built and
used only in disposable external work areas. QWK structure was derived from format
documentation, not inferred from Synchronet. Generic future QWK partner routing
and network-specific DOVE enrollment conventions remain distinct, both unimplemented.

## Implemented behavior

| Requested acceptance area | Implemented outcome / evidence |
|---|---|
| Shared foundation and mapping | Used NetworkKind/profile, explicit stable mapping, immutable artifact identity/custody, export manifests and import receipts; no speculative FTN code |
| Schema and migration | One transactional migration to 20; native bytes/history retained; clean setup, actual schema-19 upgrade, late real-migration failure rollback, idempotence and old snapshot migration tests |
| Packet structure | Stored ZIP export; Stored/Deflate input; CONTROL.DAT, MESSAGES.DAT, conference/PERSONAL indexes, honest DOOR.ID and selected TOREADER metadata |
| Record mapping | Fixed 128-byte framing, bounded multi-record bodies, native display handles, recipient/subject/number, explicit conference and safe parent resolution |
| Export state | Native read pointers, retained per-request manifests, no advancement on build or failed transfer, affirmative post-delivery confirmation, preview and explicit reset CAS |
| Caller workflow | Messages L → D/U/S/Q/help; ordinary binary protocol selection and engines, private bounded transfer buffers, no normal file-catalog upload |
| Author and permissions | Authenticated current caller supplies native author; current conference access/posting, recipient/private rules and native limits checked per member inside native transaction |
| Duplicate semantics | Canonical uncompressed packet/member identity; same/recompressed packet replay, per-member resume, possible-duplicate hold across submissions and explicit intentional-new confirmation |
| Import atomicity | Framing validated first; per-member semantic outcomes; native post/statistics/receipt commit together; receipt failure rolls back native post; retry sees prior outcomes |
| Encoding and time | Explicit CP437/QWKE, long-header preservation; impossible controls/pi/Unicode conversion refused; raw padding retained; board-local export and unresolved source wall-time provenance with trusted native receipt UTC |
| Security and limits | Archive preflight/CRC/no extraction, safe member names, bounds/ratio/count, identity/access checks, bounded admission and durable storage/history capacity |
| Custody and recovery | Pending journal before file creation, restricted directory/files, known-incomplete-only recovery, unknown files preserved, complete artifact hash validation, no restored live ownership |
| Observability/privacy | Content-free `message.qwk.*` activity, ordinary native posting statistics, typed transfer purpose; no private identity/path/body/recipient logging or operator audit misuse |
| Documentation/localization | Caller/Sysop/Technical guides and indexes; en-US **1.17.0 / 1,014**; Modern/Minimal **1.6.0**, Classic **1.7.0** |

## Real macOS and independent interoperability

Acceptance uses Apple Silicon macOS and a newly created disposable board. The
integration test starts the `spitfire run` daemon as an independent subprocess,
authenticates synthetic callers, and uses the existing RAW TCP binary-stream
boundary and YMODEM engines. It does not run a daemon in the test process or write
production data. Original/historical inputs remain untouched.

MultiMail 0.52 was built locally without installation into the project. Its real
interactive reader opened a downloaded packet, recognized both conferences and
the personal-mail index, displayed public author/recipient, local timestamps,
CP437 accented text and the full QWKE subject. Its default terminal filtering
rendered box-drawing/block glyphs as ASCII approximations; raw packet assertions
verify the original CP437 bytes. This is a reader display setting, not a claim of
pixel-identical terminal rendering. MultiMail generated a ZIP REP independently,
including a long Subject extension. The daemon imports that packet through the
normal caller upload path, and ordinary reupload is checked for zero duplicates.

No original LAKOTA LMR round-trip, attachment workflow, arbitrary reader extension,
Windows interactive session or real network-partner exchange is claimed.

### Acceptance coverage map

| Journey / threat | Evidence |
|---|---|
| Disposable board, independent daemon, authentication and multiple conferences | `crates/sf-bbs/tests/qwk_offline.rs` |
| Public, CP437, authorized private and reply-chain seeds | Typed native posts in that test; packet excludes another caller's private sentinel |
| Caller workflow and real download/upload | Real YMODEM transfer on connected daemon sockets, not direct service handoff |
| Archive members, conference list, handles, subject/date/body and indexes | Codec assertions plus actual MultiMail reader display |
| Offline reply generation and native import | Independent MultiMail REP, including long Subject; native author and parent assertions |
| Same packet twice, malformed/forbidden conference and author spoof | Live caller upload and native-authority tests; durable outcome counts |
| No mail, failed download and retry | Live workflow confirms feedback and unchanged pointer before successful regeneration |
| Incomplete reply upload | Deliberately truncated real protocol frame and disconnect; native count unchanged |
| Traversal, malformed archive and declared oversized expansion | Live uploads of hostile synthetic fixtures; no filesystem escape |
| Two callers concurrently, same caller overlapping manifests | Simultaneous live downloads plus pointer-version/reset native tests |
| Disconnect/cancellation and restart | Real dropped sockets, transfer finalization and recovery, no pointer advancement |
| Privacy/events, artifact cleanup and retained custody | Native event tests, live inventory/hash checks, POSIX modes and targeted crash-recovery tests |
| Backup/restore and duplicate protection | Complete board cold backup/new-root restore followed by ordinary replay suppression |
| Existing online workflows | Normal workspace message/file/session/operator regression suite, without another full B-021 manual campaign |

Reproduce the portable tests with `cargo test --workspace`. The optional integration
environment variable `SFNG_N1_PEER_DIR` saves a synthetic downloaded QWK outside the
repository for a reader. `SFNG_N1_PEER_REPLY` supplies a synthetic one-message REP
independently produced by that reader; the test uploads it and checks replay. These
are test-only inputs, not production path-selection features. Source peer assets,
packets and local acceptance identities do not form public fixtures.

## Final verification and publication

Private quality gates pass: **570 tests / 0 failed / 2 existing ignored**, doctests,
**98 source headers**, `cargo fmt --all --check`, workspace/all-target Clippy with
warnings denied, `git diff --check`, Markdown/local links/anchors/balanced fences
and added-text privacy/provenance checks. The complete workspace run follows the
final manifest-budget and hard-crash recovery regressions. The independent long-subject peer journey also
passes. cargo-audit remains unavailable. The accepted implementation checkpoint is identified above. Public quality gates
and publication alignment are recorded below.

Sanitized public synchronization follows private push/clean alignment and copies
public-safe source, synthetic tests, schema and documentation without merging
private history. The public M044 reference is a rights-safe architecture summary,
not the private corpus/research record.

Windows live networking acceptance remains **DEFERRED — REAL WINDOWS ENVIRONMENT
REQUIRED**. No VM/cloud Windows creation, long Windows CI wait or expanded live
platform claim is part of N1. cargo-audit is accurately unavailable locally.

## Boundaries and exact next action

QWK networking is NOT implemented. DOVE-Net is NOT implemented. FidoNet/BinkP are
NOT implemented. Nodelists/pointlists, FileEcho/TIC/FREQ/AreaFix, Networks views and
configuration, scheduler, B-022, doors and CircuitNet were not started. DDEV,
production, FireComm and the FidoNet corpus were not modified. No release, tag,
distributed binary, package or installer was created. Ordinary development/test
binaries and the isolated interoperability peer are acceptance tools only.

**Exact next action:** stop at accepted, published N1. The next implementation
slice is separately authorized N2 under M044; do not start it in this pass.

## Public source verification

Public gates pass: **508 tests / zero failed / two existing ignored**, doctests,
**77 source headers**, `cargo fmt --all --check`, all-target workspace Clippy with
warnings denied and `git diff --check`. All **105 Markdown documents / 584 local
links and anchors** pass, with balanced fences. Added-text path/credential and
corpus/provenance checks pass; all 31 changed crate files match accepted source.
The publication adds **12 files and updates 41**. The source includes schema 20
and caller QWK offline mail. Private continuity, corpus/index material, historical
samples, external peer assets and acceptance packet files are excluded. No private
Git history is merged. This containing publication commit identifies the public
snapshot; the source checkpoint is recorded above.

The normal public workspace run includes the independent-daemon QWK caller test
and security/recovery regressions. MultiMail's independently authored long-subject
REP passed the real private daemon journey on the same runtime/codec code; the
final source follow-up only synchronizes two existing chat tests. cargo-audit is
unavailable, and Windows live acceptance remains deferred under standing policy.
The downloadable 0.1.0 Development Preview remains older and unchanged.

## Publication regression follow-up

Public workspace testing exposed an intermittent failure of the existing two-node
ordinary-file download test. Inspection identified deferred read-to-write lock
upgrades in quota reservation and settlement. Both now acquire an immediate write
reservation before reading quota/receipt state, preserving existing authorization
and accounting semantics. Five focused repetitions pass, followed by fresh private
and public workspace gates. Failure assertions retain the synthetic transcript.

Recovery review also found that packet transfers have no ordinary file-quota row,
so quota-based restart reconciliation alone could leave a packet transfer active.
N1 now explicitly fails unfinished packet transfer records and prepared requests
on exclusive restart/restore. A real independent daemon is killed during an
incomplete upload; restart, replay, no partial import and cold backup/restore pass.
A separate native regression verifies idempotent recovery and unchanged pointers.
The final private workspace total is 570 tests plus two existing ignored tests.

### Reader text boundary and asynchronous SSH test follow-up

A later public run exposed a fixed 300 ms delay in an existing SSH status test.
The test now waits up to five seconds for the same authenticated caller and terminal
state, without weakening either assertion or changing SSH runtime behavior.

An independent MultiMail experiment showed that leading header-like body text can
be consumed as QWKE metadata even after a blank separator. The codec now refuses
that ambiguous export in the selected extended profile; native content and pointers
remain unchanged. The regression covers mixed-case To/From/Subject and known
reader header variants, including leading whitespace; ordinary later body lines
remain intact. No unsupported escape extension is invented. The caller, Sysop and
technical guides disclose this bounded interoperability limitation.

### Existing chat acceptance endpoint ordering

A public regression run exposed another pre-existing test race: immediately ending
the operator endpoint after queueing its reply can remove chat authority before
the scripted caller receives that queued reply. Both affected caller chat tests
now keep the endpoint alive until the caller consumes the reply and leaves chat.
The original transcript and persistence assertions remain unchanged; no chat
runtime semantics, operator authority or terminal transport behavior changes.
Focused tests and renewed workspace gates verify the correction.
