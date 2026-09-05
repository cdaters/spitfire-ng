# M039 Tranche 7 — B021-D operator closure

Status: **B021-D COMPLETE / ACCEPTED; B-021 VERIFIED**

## Authority and bounded implementation

Schema remains 19. B021-A/B/C remain accepted. This report publishes
project-owned implementation and acceptance conclusions without acceptance
artifacts or historical sample material.
The [B-021 gate](m039-tranche-7-b021-operator-controls-gate.md), subsequent
accepted slices, and their domain contracts define this closure pass.

## Maintenance descriptor contract

B021-D supplies a small shared, closed descriptor list for the existing B-015,
B-017, and cold-backup owners. Descriptors are presentation/help metadata, not
new protocol commands, grants, jobs, or durable authority. Actual health counts
continue through the authorized daemon MaintenanceStatus projection.

| Descriptor | Owner and existing service | Execution boundary | Operator route |
|---|---|---|---|
| File integrity/review | B-015 file maintenance/reconciliation | Existing typed domain service; no new attached execution | Maintenance / Errors counts; Activity; file-maintenance reference |
| Activity retention | B-017 bounded cleanup and summary reconciliation | Existing typed domain service; no scheduled runtime or new attached execution | Maintenance retention policy; Activity; retention reference |
| Cold backup/recovery | Native backup_board / restore_board | Exclusive stopped-board lock; validated snapshot | Graceful shutdown, exit offline sfconfig, spitfire backup / restore |

No historical B-021 outcome requires attached pack/purge, arbitrary repair,
backup deletion, or an online backup. B-018 pack/purge and B-022 screen/export/
print remain separate. A descriptor does not imply that a service-only operation
has a Sysop execute command. No maintenance execute action is added in this pass.

## Reference review

A bounded read-only FireComm review of interactive-session polish and live-session
lifecycle supports publishing failed state before presenting errors (**ADAPTED**)
and keeping transport ownership separate from presentation (**RETAINED**).
Capture and platform-native window/scaling behavior remain **FIRECOMM-SPECIFIC**;
no source, assets, dependency, or runtime integration is copied.


## Stock outcome reconciliation

The primary read-only review revisited SPITFIRE.DOC §8.1–8.2, the F2/F3/F5–F10
and Alt+F1/F2/L entries, the menu-security example in §5, the distributed
SFSYSOP menu, and the accepted B-021/B021-B gates. Historical bytes were decoded
explicitly as CP437 only for inspection and were not changed or published.
Exact key/prompt/forced-chat timing is distinguished from native semantic parity.

| Historical outcome | Current native route | Evidence and classification |
|---|---|---|
| F1 activity/log attention; F5 statistics | sfmonitor Activity, Statistics, Notifications and Maintenance; attach CLI | B-017 verified domain/retention tests and accepted real journeys; current two-monitor journey. VERIFIED. |
| F2 change caller security privately | Existing daemon-owning console SECURITY → OperatorService → caller authority | Accepted M042/B-016 real operator acceptance, current shared-service/access/lifecycle regressions. VERIFIED; no new private-caller IPC surface. |
| F3 availability and caller Page | Explicitly enrolled page availability, pending page, answer/decline through monitor/shared client | B021-B2 accepted matrix and current real caller page/chat. VERIFIED. |
| F6/F7 repeatable five-minute changes | Enrolled signed per-session adjustment, ±5 presets, preflight, exact occupancy and receipt | B1 bounds/replay/stale/expiry/accounting matrix plus current real +5/−5. VERIFIED. |
| F8 operator caller/status screen | Privacy-safe Nodes/detail, responsive monitor layout | B-017/B021-A projection privacy tests; 100×30/80×24 and minimum-size acceptance. REPLACED-NATIVE / VERIFIED. |
| Alt+C answer or initiate chat | Answer pages or invite with caller consent; operator invitation pauses allowance only after acceptance | Accepted B2 pause/return/disconnect matrix, current bidirectional chat. Forced entry replaced by invitation. REPLACED-NATIVE / VERIFIED. |
| Alt+F1/F2 notice/no-notice disconnect | One typed confirmed cooperative disconnect with exact-session fallback | Accepted active-transfer/accounting matrix and current real notice/reconnect/no-notice. VERIFIED. |
| Alt+L reversible caller lockout | Existing console DISABLE/ENABLE, current caller lifecycle enforcement | Accepted M042/B-016 and active-caller invalidation regressions. Reversible identity-preserving lockout retained; no new broad caller editor. VERIFIED. |
| F10 terminate SPITFIRE | Enrolled Dashboard shutdown; shared admission barrier and bounded session/transfer drain | B3 accepted lifecycle/receipt/fallback tests and current native shutdown acceptance. VERIFIED. |
| Alt+A / caller record administration | Existing typed foreground console and caller domain services | B-016 accepted privacy/version/security/lifecycle outcome; broad packing belongs to B-018. VERIFIED for included B-021 entry/outcome. |
| Alt+M/Z/P/R/F configuration | Setup bootstrap; sfconfig typed online/offline fields; existing stopped-board identity/conference/file-area editors | B021-C accepted effects/CAS/atomicity/privacy plus current two-client/handoff/recovery matrix. VERIFIED for implemented owner routes. |
| Maintenance/status/recovery | Three closed descriptors, authoritative B-017 reads, acknowledgement, cold backup/restore and deliberate offline recovery | Current maintenance consistency, invalid-config, exclusive-lock, restore, audit tests and native journey. VERIFIED. |
| Home help/local control | Localized contextual help, visible action outcomes, scrollable compact help, isolated terminal ownership | Current render/input regressions and real native PTYs. VERIFIED. |
| F4/printer, screen export, chat capture | B-022 output and separate privacy/consent boundary | Excluded by accepted gate. B-022 NOT STARTED; chat capture is not implemented or required. |
| Events, pack/purge, legacy backup-file deletion | B-020, B-018, B-025 owners | No work imported to B021-D. Generic host shell/reboot/DOS access is historical-only. |

Retaining an existing console/cold-domain route is not a claim that sfmonitor
has a new caller editor or sfconfig has mutable database-domain forms. The gate
preserves outcomes through their accepted owners. B021-C explicitly accepted
these product limits. B-016, B-017, and B-025 are not relabeled by this pass.

## Ordinary corrections

- sfconfig latches loss of online authority, stops background probes, retains
  staged candidate/CommandId/input, closes save review and pending reload, and
  requires explicit process reopen. It does not silently switch modes or cause
  repeated five-second authorization-denial audit.
- A successful replay receipt cleans the proven candidate before the next read;
  a self-revoked read cannot turn a committed save back into a dirty draft.
- Ctrl-C from configuration help or confirmation reaches the same deliberate
  dirty-exit path while retaining input until discard is confirmed.
- sfmonitor gives action results a visible wrapped footer area and concise key
  hints. Results were previously clipped after long hints at normal widths.
- Missing disconnect targets dismiss their choice pane so stale/refresh guidance
  is visible. No command is sent without a valid target.
- Monitor help scrolls at compact sizes; maintenance removes blank spacing at
  small sizes. Shared owner guidance is used by both operator tools.
- Shared configuration vocabulary replaces the duplicate frontend list:
  16 unique recognized capabilities fit the unchanged 32-entry ceiling.
- Current setup/recovery/manual/index/localization wording is reconciled;
  setup remains a new-board bootstrap, not an overwrite or comprehensive editor.

These changes add no durable authority, schema migration, security relaxation,
new mutation family, generic maintenance executor, or dependency.

## Concurrency, audit, and privacy review

Two monitors remain independent, and configuration clients use aggregate revision
and digest CAS. Existing callers retain captured new-session policy; permissions
are rechecked at dispatch. Revocation after a rendered action or preflight denies
effect. Reconnection creates fresh daemon/session identity. Offline sfconfig and
daemon startup contend for the existing cross-process board lock.

All inspected B021-A/B/C privileged audit writers propagate their Result; schema-19
outcome literals remain valid. New fault-injection coverage proves that failed
preparation audit prevents configuration replacement and produces no false success.
Existing post-replacement receipt/audit recovery, B1 stale-denial audit, B2 chat-end/
disconnect finalization and B3 pre-exit ordering remain in the complete suite.
Domain security/caller changes retain their purpose-specific audit. Ordinary
successful reads do not create command receipts or audit rows.

Operational events describe safe state transitions and outcomes separately from
operator/security audit. No chat body, credential, raw configuration, private key,
terminal input, host path, or raw request payload is added. Repeated probes stop
on lost configuration authority. Acknowledgement changes only attention state;
source warnings/errors remain historical evidence, not contradictory repair status.

## Bounded download-assertion investigation

The prior public failure was the completion-text assertion in
`two_nodes_download_the_same_file_concurrently_over_raw_tcp`. Its board/database
are uniquely disposable, callers are separate, reads drain concurrently, and the
existing test applies bounded 30-second socket reads. The earlier accepted Windows
correction added parallel drains; B021-C did not modify download semantics.
Download settlement increments counters without changing the selected file's
identity/version, and reservations/uses retain their established transaction and
session ownership. No deterministic shared fixture or order dependency was found.

This pass ran one focused native probe, six further focused repetitions, the
neighboring matching runtime filter, five core concurrency tests, and the full
workspace. All native runs passed. The earlier
assertion's cause remains **NON-REPRODUCED / MONITORED / NOT A BLOCKER**. No claimed
root cause, weakened assertion, arbitrary sleep, or transfer workaround was added.

## Platform matrix

| Environment | Evidence / remaining boundary |
|---|---|
| Apple Silicon macOS | Real Darwin arm64 independent processes, loopback callers, native sfmonitor/sfconfig PTYs and board locks; final journey and gates recorded below. |
| Windows protected attach foundation | Accepted B021-AW named pipe, DACL, SID, transport and independent-client evidence remains accepted. No new run or compile claim. |
| Windows monitor rendering/input/live mutation | DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED. |
| Windows config rendering/input/CAS/live mutation/SID enrollment | DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED. |
| Windows maintenance/cold execution/filesystem behavior | DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED. |
| Windows terminal handoff/chat/disconnect/shutdown overlap | DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED. |
| Windows service deployment | DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED; service packaging is outside this pass. |
| Linux / BSD | Shared source architecture retained; live acceptance not required or claimed. |

No VM, cloud resource, repeated Windows CI job, or platform-specific core path was
introduced. Deferred interactive Windows confirmation is not a missing native
semantic implementation; any future security/build/data-integrity defect must be
handled on its own evidence.

## Automated and native acceptance inventory

Twelve added regressions cover configuration online-loss/pending-reload/draft
preservation; saved-receipt/read-revocation consistency; Ctrl-C from overlays;
offline error identity; explicit vocabulary bounds/serialization/bootstrap;
self-revocation/offline reenrollment; invalid saved config/known-good restore/
setup/daemon lock boundaries; audit failure durability; notification/maintenance
consistency; compact help without dispatch; visible result/key hints; and missing
disconnect-target feedback. Prior service, protocol, handoff, privacy, session,
transfer, shutdown, migration, backup and concurrency tests remain active.

Native acceptance uses disposable project-owned setup and fixture boards only.
The fixture supplies known transfer content and one deliberately seeded offline
notification; real acknowledgement uses daemon authority. Raw captures, generated
identities, random chat phrases, local paths, and harnesses are private acceptance
inputs, not publication files. No unchanged historical matrix is claimed to have
been manually repeated merely because the complete regression suite passed.


## Integrated acceptance and status

- Native integrated macOS journey passed actual setup/read-only bootstrap;
  explicit offline 16-capability enrollment; daemon lock exclusion; independent
  daemon/two callers/two monitors; Dashboard/Nodes/Activity; ±5; notification
  acknowledgement; page answer and invitation; private bidirectional chat;
  notice disconnect, node release/reconnect and no-notice disconnect; monitor →
  config → monitor; two-config CAS; permission revoke/restore and disabled action;
  SSH state safety; maintenance owner help; active XMODEM transfer shutdown with
  CAN and completed-only accounting; dirty sfconfig authority-loss/reopen behavior;
  exact cold backup; invalid-config refusal; new-root restore and reopened authority;
  durable final receipts/audit/events; unchanged file hash; SQLite integrity and
  foreign keys; chat/secret exclusion; and terminal restoration.
- Separate native policy journey passed exact six-read empty-list bootstrap,
  denied configuration/handoff recovery, live grants/revocation, actual new-session
  one-minute inactivity expiry while the existing caller remained usable, and
  exact configuration/receipt cold restore.
- Separate final native terminal check passed 80×24, below-minimum and 100×30,
  Ctrl-C from dirty help, cancel/confirm, original termios, alternate-screen exit,
  visible cursor, and no accidental save.
- B021-A/B/C remain accepted. B021-D **COMPLETE / ACCEPTED**. B-021 **VERIFIED**:
  no included native semantic outcome remains incomplete. B-022 **NOT STARTED**.
  All 25 Category-B rows recount to **15 VERIFIED / 2 IMPLEMENTED / 3 PARTIAL /
  5 NOT STARTED**.
- en-US **1.16.0 / 988 unique messages**; presentation versions unchanged.
- No B-022/networking/doors/scheduler, deployment, external reference project,
  historical corpus, OS-service/release packaging, tag or distributed binary changed.

## Public source validation

The sanitized public workspace passes **481 tests / 2 existing ignored**,
including doctests. Twelve regressions were added; none was weakened or newly
ignored. All 71 source headers, cargo fmt, Clippy all-targets with warnings
denied, and diff hygiene pass. Documentation checks cover **100 Markdown files /
555 local links and anchors / zero errors**, with balanced fences.

All ten changed Rust/Fluent source files equal the accepted implementation.
The publication contains 34 project-owned files: 2 added and 32 updated. All
25 Category-B rows and 988 unique localization keys were independently counted.
Full tracked-text scans found no private host paths, key/token markers or account
SIDs; sample inventory contains only the preexisting exclusion-policy README.
No dependency, license/provenance metadata, schema or corpus changed. The
concurrent-download assertion also passed this public workspace run.

`cargo-audit` is unavailable; no advisory-check success is claimed. The source
milestone is distinct from the unchanged 0.1.0 Development Preview archive.
No private acceptance scripts, captures, screenshots, identities, local paths,
credentials, historical samples or session continuity records are published.

Exact next action: stop at completed B-021. Subsequent development requires a
separately scoped B-022 interface/resource/transaction gate; no implementation
begins in this milestone.
