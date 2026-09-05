# B021-B2 — Page/chat and graceful caller disconnect

Status: COMPLETE / ACCEPTED. Schema 19; current source, not the 0.1.0 binary.

## B021-B2 implementation

The [accepted contract](m039-tranche-7-b021b-live-operator-controls-gate.md)
adds explicitly granted runtime page availability, pending caller-page
observation/answer/decline, caller-consent operator invitations, bounded chat,
and confirmed graceful caller disconnect with or without notice. Protocol 1.3
discovers these after the baseline hello; older clients retain known vocabulary.

OperatorService, InteractionHub, NodeManager, transfer engines, and common
session accounting/finalization remain the single authorities. Every caller
target binds daemon generation, NodeId, SessionId, and occupancy generation.
Exact interaction IDs additionally protect page disposition and caller consent.
Concurrent operators receive deterministic busy/already-handled/stale outcomes.

## Chat lifecycle and privacy

Invitations appear at a safe caller menu boundary and require visible Y/N
consent within 30 seconds. No hidden takeover or terminal observation exists.
Accepted operator-initiated chat pauses ordinary allowance exactly once; existing
caller-page accounting is unchanged. Normal end/loss restores prior caller
context and valid partial menu input. Factual usage accounting remains intact.

One-use principal-bound handoffs open a fresh authenticated line stream. Lines
are bounded to 512 UTF-8 bytes, channels to 32 messages, responses to 16 lines,
complete frames to five seconds, and idle caller chat to five minutes. Loss,
revocation, restart, or stale targeting ends the interaction without automatic
resume. Same CommandId recovery cannot create a duplicate invitation.

Chat contents never enter command fingerprints/receipts, audit, B-017 events,
ordinary logs, SQLite messages, diagnostics, or durable monitor history. The
monitor keeps only its current ephemeral 100-line conversation in memory and
discards it on end/loss; debug output is redacted. No transcript recovery exists.

## Disconnect and integrity

Server preflight binds current exact caller, notice choice, active transfer/
interaction impact, attachment, and CommandId for 30 seconds. Explicit Enter
confirmation commits; Esc cancels. Changed impact requires fresh review.

Both variants use cooperative interaction/transfer cancellation, completed-only
settlement, accounting, node release, audit/events, and transport close. No-notice
omits only the caller display. After three seconds the daemon may revalidate the
exact target and use its owned emergency TCP/SSH handle, then wait up to five
more seconds for normal finalization. Unsupported hardware handles are never
replaced with global/path-based closes. Receipts distinguish actual completion,
fallback, stale targets, and failed finalization.

Old CommandId replay cannot disconnect a replacement caller. Two operators
produce one transition without a global monitor lock. Transfer cancellation
preserves staging, batch/file authority, source integrity, and completed-only
credit, including earned upload allowance during B1 refresh.

## Acceptance

Native macOS acceptance covers two callers/two real monitor PTYs, page answer/
decline, invitation consent/races, bidirectional private text, allowance pause,
context return, both disconnect choices, active chat, real XMODEM upload/download
cancellation, source hash and SQLite integrity, stale reuse, response loss,
revocation, Q-only exit, and terminal restoration. Unique private phrases were
absent from inspected durable state/logs/artifacts; no phrase or transcript is
published. Focused state-machine, privacy, bounds, concurrency, and fallback
regressions remain reproducible in the workspace tests.

See [integrated B1/B2/B3 acceptance](m039-tranche-7-b021b3-shutdown-integrated.md).
Windows live page/chat/rendering/input/disconnect/transfer/multi-monitor acceptance
remains DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED. Linux/BSD source architecture
is preserved; hardware-specific acceptance is not inferred from native tests.
