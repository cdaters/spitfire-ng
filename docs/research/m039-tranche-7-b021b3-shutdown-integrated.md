# M039 Tranche 7 — B021-B3 shutdown and integrated live controls

Status: **B021-B3 AND INTEGRATED B021-B COMPLETE / ACCEPTED**

## Authority and scope

Current source implements the accepted B021-B milestone. Schema remains **19**. The [B021-B gate](m039-tranche-7-b021b-live-operator-controls-gate.md)
and [D-064](m039-tranche-7-b021b-live-operator-controls-gate.md#compatibility-and-command-authority)
remain binding. The accepted B1 negotiation, bounded explicit authorization,
and persisted stale-audit corrections are incorporated, not redesigned.

Historical SPITFIRE F10 terminated SPITFIRE, not its host. This slice implements
that daemon-only outcome with modern explicit authorization and bounded safe
drain. It adds no host shutdown, reboot, restart, service-manager command,
configuration editor, maintenance execution, network, door, scheduler, or export.

## Protocol and authorization

Protocol **1.4** adds `graceful-shutdown` only through authenticated control
discovery. The initial hello still carries the nine baseline reads; even a
client requesting shutdown in its hello cannot enable it there. Older 1.2/1.3
clients receive only their known feature/capability vocabulary. The existing
typed `LiveControl` envelope gains `PrepareGracefulShutdown` and
`RequestGracefulShutdown`; `ShutdownStatus` is a safe board-statistics read.

The independent capability is **`request-graceful-shutdown`**. Bootstrap and
omitted/default capabilities remain exactly the six monitor reads. All seven
B021-B controls require explicit grants; all six reads plus seven controls
fit within the unchanged **32**-capability ceiling. Unix UID, Windows SID,
local attachment, capability revocation, and dispatch authority are unchanged.
Support discovery is not permission. Sysop status, ownership, and OS elevation
do not implicitly grant this IPC mutation.

## Preflight, confirmation, and identity

Shutdown preflight is bounded to 128 ephemeral entries with a **30-second**
lifetime. It binds the attachment, CommandId, daemon generation, and current
caller/transfer/chat/interaction consequences. The server retains exact session,
occupancy, transfer-state, and interaction identities for consequence checking;
the wire shows only aggregate counts, phase, generation, and an opaque token.
A changed consequence or expired/wrong-owner token requires fresh confirmation.

Dashboard → `A` Actions → `S` **Shutdown SPITFIRE NG** opens authoritative
impact. Enter confirms; Esc cancels before dispatch. Unsupported and unauthorized
actions stay understandable but non-executable. `Q` still quits only sfmonitor.
There is no destructive direct function-key action and no restart command.

Same CommandId/principal/fingerprint replays the recorded receipt before any
second effect. Changed fingerprint/principal fails closed. A distinct request
after acceptance returns `shutdown-already-requested`. Shutdown cannot be
cancelled or overridden by another monitor, and survives requesting-client loss.

## Shared lifecycle and persistence ordering

`BoardRuntime` owns one ephemeral shutdown/admission transition lock. Node
acquisition and accepted shutdown share this barrier, so no session can be
admitted after acceptance. Foreground-console QUIT/EOF and the existing signal
flag enter the same daemon drain; they do not introduce another IPC permission
path. The console reader cannot hold an IPC-requested daemon shutdown hostage.

The daemon runner, not an operator task, performs:

1. validate and durably journal the command; persist checked request audit and
   safe event; complete its receipt as **`shutdown-requested`**;
2. commit the admission barrier and publish requested/draining state;
3. request existing session-owned cancellation with the distinct localized
   board-shutdown notice, ending interactions and pausing no further caller time;
4. allow **three seconds** for cooperative transfer/session finalization;
5. revalidate each complete exact session tuple before any owned transport close;
6. allow up to **six additional seconds** for finalization and outstanding
   control/stream evidence (the existing five-second complete-frame deadline
   plus scheduling margin);
7. verify completed accounting, released nodes, and drained control evidence;
   commit correlated final audit and the lifecycle event;
8. stop caller/operator listeners and return normally from the daemon runner.

The ordinary fast path does not wait out either maximum. B2's existing
disconnect timeout remains three plus five seconds. Shutdown reuses its
cooperative cancellation, binary CAN sequence, transfer staging/queue/settlement,
and exact TCP/SSH ownership; it does not kill threads or processes. Existing
serial/modem cooperative behavior remains; real hardware fallback is not claimed.

If safe finalization or evidence persistence cannot be established, the bounded
attempt reports failure and retains the daemon with admissions closed. It does
not label unfinished writes/files as a clean exit or fall back to process kill.
No new durable recovery or service-manager authority is invented.

Every authoritative write uses the existing SQLite transaction/commit authority;
the journal/audit/event tables and valid schema-19 vocabularies are reused.
`shutdown-requested` is the durable receipt's semantic result, not a promise of
post-exit persistence. `shutdown-complete` is final correlated audit/lifecycle
evidence written before exit. While the old daemon can answer, same-generation
receipt lookup recovers acceptance. After exit the endpoint is unavailable;
normal process completion and retained lifecycle evidence establish the outcome.
A restarted generation does not replay an old shutdown or silently resume chat.

## Chat, transfers, and privacy

Active chat ends through InteractionHub, releasing the existing single allowance
pause guard and discarding monitor memory. No transcript enters journal, audit,
events, logs, SQLite messages, or diagnostics. The distinct caller notice says
the board is shutting down, not that the caller timed out or was individually
disconnected. Session accounting records `board-shutdown` through its existing
factual accounting authority.

Active X/Y/ZMODEM work uses B2's bounded cancellation/finalization. Only completed
accepted files/items receive credit. Upload staging, download source files,
batch queue authority, and earned upload allowance credit are unchanged.

## Regression and native evidence

Focused tests cover read-only bootstrap, explicit enrollment and live revocation,
1.2/1.3 closed-enum compatibility and authenticated-only shutdown discovery,
preflight impact changes, replay/conflict/requester loss, distinct duplicate
requests, sixteen concurrent post-barrier arrivals, owned fallback, pending
evidence drain, durable receipt/audit/event ordering, and Dashboard Enter/Esc/Q.
The former test treating the now-implemented capability as unknown instead
rejects host shutdown; wildcard, unknown, duplicate, oversized, and malformed
profile rejection remains in force.

Disposable Apple Silicon macOS evidence:

- Independent daemon, two real callers, and two real sfmonitor PTYs passed B2
  page availability, pending pages, answer/decline, invitation/busy race,
  bidirectional private chat, context return, and both disconnect notices.
- Actual incomplete XMODEM upload/download disconnects retained completed-only
  accounting, no unfinished upload catalogue/staging, unchanged download source
  hash, released nodes, and SQLite integrity.
- Real B3 +5/-5/replay and monitor notification acknowledgement passed. The
  notification was a deliberately seeded offline synthetic fixture record;
  acknowledgement itself used the live daemon's normal authority.
- With chat and a download active together, shutdown preflight reported two
  callers, one transfer, and one chat. Esc left them running. Enter then produced
  both distinct notices, transfer CAN, chat end, normal session/node finalization,
  persisted requested/final evidence, and normal daemon exit.
- Both monitors observed disconnection without crashing and restored raw-mode
  flags/alternate screen. Separately restarted daemon/callers survived monitor Q.
- Live shutdown revocation denied an already-preflighted command. After explicit
  restoration, lost requesting response recovered via the same CommandId while
  a competing distinct request returned already requested; only one drain ran.
- Unique chat phrases generated only in process memory were absent from SQLite
  logical dumps and all inspected disposable durable files/logs/artifacts. No
  transcript or phrase is recorded in this report or publication inputs.
- Foreground-console QUIT, IPC shutdown while console input waited, and SIGINT
  each used the shared caller notice/finalizer and exited normally.
- A real empty-allowlist bootstrap attachment exposed exactly six reads. The
  daemon advertised support but denied shutdown, the actual monitor displayed
  its disabled action, and Q left the daemon/caller running.

The integrated sweep is representative, not a mechanical repetition of all prior
journeys. B1/B2's accepted time-pause, stale-slot, receipt, privacy, and accounting
regressions remain in the full workspace suite and their canonical reports.

## Bounded corrections and reference review

Ordinary fixes enforce the gate: exact admission serialization, tracked pending
B2 finalizer/stream evidence before exit, shutdown-only minor/discovery filtering,
atomic shutdown-notice classification, failure event mapping, capability-aware
global actions, and synchronized localization/help/tests. None changes accepted
identity, capability meaning, schema, journal, fingerprint, privacy, accounting,
transfer authority, or daemon ownership. No STOP-class contradiction was found.

A bounded read-only FireComm lifecycle/boundary review confirmed the already
adopted session-owned transport teardown and transfer stream ownership. Capture,
automation, and client-specific runtime design remain FireComm-specific and were
not adopted. No source, UI, dependency, or private reference material was copied.

## Platforms, final gates, and next action

en-US advances to **1.14.0**. Sysop Manual, Caller Guide, technical references,
contextual `operator.shutdown` / `operator.actions`, status/indexes, and parity
continuity are updated for current source, not the downloadable Development
Preview binary.

Windows live B1/B2/B3 mutation, chat/input/rendering, transfer disconnect/shutdown,
two-monitor races, and shutdown TUI remain **DEFERRED — REAL WINDOWS ENVIRONMENT
REQUIRED**. Existing named-pipe/SID/ACL architecture is preserved; no new Windows
compile/live acceptance is claimed, and no long Windows job was run. Linux/BSD
source architecture remains; live acceptance was not required.

B021-B1/B2/B3 and B021-B are **COMPLETE / ACCEPTED** after native integrated
acceptance. This source publication excludes private history, test identities,
phrases, transcripts, screenshots, and historical/reference corpora. Public
workspace gates and sanitization are verified separately from native acceptance.

Public source gates passed: **446 tests passed, 2 ignored**, doctests green;
67 project-authored source headers; `cargo fmt --all --check`;
`cargo clippy --workspace --all-targets -- -D warnings`; and `git diff --check`.
All public Markdown local links/anchors and balanced fences were checked.
The 34 published source/resource files match the accepted implementation;
only project-owned source, public-safe tests, and sanitized human documentation
are synchronized, without importing implementation Git history. Path, identity,
secret, and excluded-corpus scans passed. cargo-audit is unavailable, not passed.

B-021 remains PARTIAL; B-022 NOT STARTED; Category-B remains 14 VERIFIED /
2 IMPLEMENTED / 4 PARTIAL / 5 NOT STARTED. The exact next development action is
separately authorize B021-C typed configuration / sfconfig; B021-D follows later.
Neither starts here. No networking, doors, scheduler, reports/export, host/service
packaging, release, tag, installer, or binary distribution accompanies this
source synchronization. The downloadable 0.1.0 Development Preview is unchanged.
