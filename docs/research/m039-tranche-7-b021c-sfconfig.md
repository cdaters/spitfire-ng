# B021-C — Typed configuration authority and sfconfig

Status: **COMPLETE / ACCEPTED**. Current source; schema **19**.

## Product and ownership

`sfconfig` is a separate native terminal application for SPITFIRE NG configuration.
It shares RuntimeConfig, typed validation, configuration/client/protocol authority,
and localization with the daemon and existing setup/administration tools. It can
start directly with `sfconfig --board /path/to/board/spitfire.toml`, or through
sfmonitor **System Configuration**. Explicit `--offline` selects stopped-board
ownership. The [Sysop Manual](../manual/sfconfig.md) gives practical instructions;
the [Technical Reference](../technical/configuration.md) specifies contracts and
recovery. The accepted [operator gate](m039-tranche-7-b021-operator-controls-gate.md)
remains the configuration ownership and historical terminology reference.

| Area | Implemented behavior |
|---|---|
| Online authority | Protected OperatorClient → OperatorService → serialized daemon configuration service. No client TOML or SQLite mutation. |
| Offline authority | Explicit mode holds the established exclusive board lock and validates configuration, schema, database identity/integrity, and candidates. No silent fallback after attachment loss. |
| Version/CAS | Monotonic static revision and canonical digest; stale expected versions return typed conflict without replacement. Independent database resources keep their own versions. |
| Idempotence | Existing principal/generation-bound CommandId and candidate fingerprint; replay returns the existing result without another increment. |
| Atomicity/recovery | Complete validated same-directory file replacement, synced contents, Unix directory sync, one prior configuration, and a bounded current-file link for transactional receipt/audit recovery. |
| Schema | No migration: additive canonical TOML metadata and existing schema-19 journal/audit suffice. Older closed parsers require matching source/recovery planning. |
| Validation | Shared field and cross-field rules; malformed/duplicate/unknown fields, capability profiles, and invalid bounds cannot commit. |
| Effects | Operator policy applies at dispatch; caller/admission policy applies to new sessions; node/listener/timezone/presentation/locale changes require restart. Existing identity/storage/backup maintenance retains offline ownership. |
| Restart | Pending values persist and remain labeled while active runtime values stay unchanged. Normal external restart consumes them. No restart command or automatic restart. |
| Operators | Separate configuration read, ordinary-change, and sensitive-change grants. Current-local-identity enrollment, individual capability toggles, exact semantic diffs, and staged removal. |
| Bootstrap | Unchanged six existing read capabilities; 32-entry bound, recognized unique grants, explicit mutation enrollment, and no wildcard/root shortcut. |
| Secrets | SSH key status is Missing/Configured/Invalid only. Opaque modem commands are redacted in projections and preserved in canonical storage. No secret bytes enter UI, journal, audit, events, or diagnostics. |
| Audit | Semantic preparation categories are distinct from successful durable apply; no raw old/new configuration values or secret payloads. |
| Application | Separate sf-config crate / sfconfig binary, using existing Ratatui/Crossterm and shared domain/client/localization interfaces. |
| Sections | General; Nodes / Listeners; Caller Access; Presentation; Security; Operators; actual read-only Messages / Files summaries; Storage / Backup guidance. |
| Interaction | Local staged edits; Enter edit/retain, Esc field cancel, S validate/review then Enter apply, C cancel-all, R explicit reload, Q tool-only exit. |
| Dirty/conflict | Explicit text/field markers; navigation, conflict, and disconnection retain drafts. Discard prompts are proportionate; clean Q needs no confirmation. |
| Help/accessibility | Contextual configuration.* help, full keyboard navigation, logical stable focus, explicit labels/errors/state, scrollable help/review, no flashing or color-only state. |
| Size | Preferred 100×30, usable 80×24; below 60×20 shows a safe resize notice and preserves edits. |
| Handoff | sfmonitor restores its terminal, launches its sibling sfconfig with the explicit board argument, waits, resumes terminal ownership, and refreshes. Q returns to sfmonitor without stopping daemon/callers. |

## Acceptance and regression coverage

Native **Apple Silicon macOS** acceptance exercised actual executable processes,
native terminal sessions, authenticated local IPC, and real caller connections on
disposable boards. It covered:

- Direct online/offline launch, sfmonitor handoff/return, all sections, edit and
  cancel, validation/review/save, help, supported sizes/resize, dirty prompts,
  clean Q, caller continuity, and exact terminal restoration.
- Two real sfconfig clients reading N: A saves N+1; B's stale write conflicts and
  leaves A intact; explicit reload and fresh edit let B save N+2.
- Empty-allowlist six-read bootstrap rejection, safe failed handoff return,
  explicit offline enrollment, and live individual capability grant/revocation.
- Pending node-count change retaining the active pool until external restart,
  which consumes the saved value without an automatic restart command.
- New-session idle policy: the new caller receives the shorter limit while the
  existing caller remains coherent under its captured policy and continues into
  Messages.
- SSH status-only projection, offline atomic save/version/prior backup, next
  daemon startup, and exact cold backup/new-root restore including receipts.

Automated coverage adds typed snapshot/version/CAS/replay/conflict, validation,
offline lock exclusion, atomic failure, post-replacement receipt recovery,
restart state, actual secret states/redaction, operator profiles/bootstrap/live
revocation, navigation/edit/save/cancel/dirty/conflict/focus/resize, and the
sfmonitor doorway. Existing B021-B action/time/acknowledgement/page/chat/
disconnect/shutdown regression tests remain green; its integrated acceptance is
preserved. No new full Windows runtime or rendered-UI claim is made.

Public workspace gates: **469 passed, 2 existing ignored**, doctests green;
71 authored source headers, formatting, Clippy with warnings denied, diff hygiene,
98 Markdown files / 526 local links and anchors / balanced fences, and
privacy/provenance scans pass. Embedded en-US is
**1.15.0 / 984 semantic messages**. cargo-audit is unavailable and is not claimed.

## Limits and future extension

Message/file sections show actual domain summaries; their existing stopped-board
editors remain authoritative. Identity renaming, storage relocation, key rotation,
and backup execution do not become new online commands. Static configuration
has no typed mutable secret field, so this MVP does not invent a replacement or
clear form; future secrets require explicit write-only unchanged/replace/clear
operations, outside generic fingerprints and diagnostics. No generic file editor,
SQLite editor, filesystem browser, host shell, or empty future-system forms exist.

Ordinary implementation corrections addressed minor-gated discovery, per-session
policy publication, saved-state/focus synchronization, dirty-header/input handling,
secret projection, preparation-vs-commit audit, legacy revision participation,
receipt recovery, and boxed snapshot representation. Regression tests cover them;
no accepted identity, bootstrap, enrollment, journal, CAS, ownership, or secret
contract was weakened.

Bounded engineering reference review adopted explicit focus/state and configured
versus active capability separation, adapted SPITFIRE terminal guards and explicit
edit/review/save, rejected copied source/layout/hierarchy and runtime coupling,
and deferred platform-specific rendered acceptance. FireComm remains independent
and unchanged; historical SPITFIRE authority remains primary. No reference source,
artwork, private corpus, or original sample is redistributed.

| Platform | Status |
|---|---|
| Apple Silicon macOS | Native configuration, IPC, terminal handoff, effects, and recovery acceptance passed. |
| Windows | Named-pipe online configuration, SID enrollment UI, rendered TUI, terminal handoff, and filesystem atomicity: **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**. Source architecture retained; no new build/runtime claim. |
| Linux/BSD | Source architecture retained; no new live acceptance claim. |

B021-C is COMPLETE / ACCEPTED; B021-B remains fully accepted. B-021 stays
**PARTIAL** because B021-D remains. B-022 stays **NOT STARTED**. Independently
recounted Category-B totals remain **14 VERIFIED / 2 IMPLEMENTED / 4 PARTIAL /
5 NOT STARTED**. The next separately scoped development action is B021-D.

This source milestone adds no networking, doors, scheduler, report/export system,
service packaging, release, tag, installer, or binary distribution. The published
Development Preview download is unchanged. Publication copies project-owned
source/docs/tests into the existing public history; it imports no private Git
history, acceptance artifacts, paths, identities, credentials, or session records.
