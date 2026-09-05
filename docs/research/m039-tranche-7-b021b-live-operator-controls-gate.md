# B021-B Live Operator Controls — Accepted Contract

Status: B021-B COMPLETE / ACCEPTED as an internal B-021 slice. Schema 19.
B-021 remains PARTIAL; B021-C / sfconfig and B021-D remain unimplemented.

## Scope and historical outcomes

SPITFIRE's historical page availability, caller pages, operator chat, repeatable
five-minute time adjustments, notice/no-notice disconnect, and SPITFIRE-only
termination are preserved as semantic outcomes. DOS key mechanics, forced hidden
takeover, chat capture, host shutdown, and shell access are excluded. Historical
SPITFIRE documentation is primary authority; other terminal/BBS implementations
provided engineering reference only, not copied source/UI or product semantics.

The five families are page availability/page-chat, session-time adjustment,
caller disconnect, notification acknowledgement, and daemon shutdown. Restart,
configuration, maintenance execution, observation, reports/export, networks,
doors, and scheduler remain outside B021-B.

## Compatibility and command authority

The canonical D-064 compatibility decision retains major version 1. The initial
hello carries only nine baseline reads understood by old closed-enum peers.
Authenticated minor-gated discovery adds B1 at 1.2, B2 at 1.3, and B3 at 1.4.
Older minors receive only their known capability/feature vocabulary. Negotiating
support never grants permission.

Typed commands retain authenticated attachment, daemon generation, request ID,
deadline, CommandId, and canonical SHA-256 semantic fingerprint. Same principal,
generation, CommandId, and fingerprint replay one bounded receipt; changed
identity or semantics fail closed. Uncertain results recover through the same
CommandId, never a silently recreated action. Schema-19 command journal and
control audit remain authority; no new durable shutdown/chat/session state exists.

Live caller targets bind daemon generation, NodeId, SessionId, and occupancy
generation. Node reuse/restart cannot retarget old commands. Notification actions
bind expected version. Time, disconnect, and shutdown require daemon preflight
and explicit consequence confirmation.

## Explicit authorization

Bootstrap is exactly six monitor reads: board-statistics, node-status,
operational-events, caller-activity, notifications, and maintenance-status.
The seven controls require individual explicit enrollment:

- acknowledge-notifications;
- adjust-session-time;
- manage-page-availability;
- manage-caller-pages;
- chat-with-caller;
- disconnect-session;
- request-graceful-shutdown.

Profiles accept at most 32 recognized unique entries, matching discovery's bound.
No wildcard, role hierarchy, automatic owner/Administrator/Sysop mutation grant,
or configuration UI is introduced. Current UID/SID policy is rechecked at dispatch;
chat also rechecks during interaction. Existing read-only boards remain read-only.

## Interaction, finalization, and privacy

Existing OperatorService/OperatorClient, InteractionHub, NodeManager, transfer
engine, and session finalizers retain authority. Chat is consented, bounded,
ephemeral line exchange, never a raw terminal bridge or transcript authority.
Only accepted operator-initiated chat pauses ordinary caller allowance; existing
caller-page accounting retains the historical distinction. Factual accounting
and completed-only transfer credit are unchanged.

Both caller-disconnect notice choices use cooperative cleanup. Bounded emergency
close revalidates the full exact session and uses only its owned transport handle.
Shutdown closes admissions, notifies callers, drains/finalizes bounded work,
commits audit/events, stops listeners, and exits only the daemon normally.
The receipt records shutdown-requested before listener stop; final lifecycle
evidence precedes exit. Client loss cannot cancel an accepted shutdown.

All attempts/results have checked privacy-safe audit. Chat text, terminal input,
private caller identities/contact data, secrets, host paths, and endpoints do not
enter attached projections or control evidence. B-017 operational events remain
separate from command receipts and security audit.

## Monitor and verification

Nodes Actions offers Page / Chat, Adjust Time, and Disconnect; Notifications
offers Acknowledge; Dashboard offers Shutdown SPITFIRE NG. Known unsupported or
unauthorized actions explain why they are disabled. Confirmation uses Enter/Esc;
Q quits only sfmonitor. Results refresh daemon projections and preserve uncertain
CommandIds across manual reconnect.

Native macOS acceptance covers B1/B2/B3 together, two callers/two monitors,
transfer integrity, privacy, races, revocation, and terminal restoration.
Windows source architecture and prior B021-A attachment acceptance remain;
live Windows B021-B mutation/chat/disconnect/shutdown/TUI acceptance is
DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED.

See [B1](m039-tranche-7-b021b1-mutation-time-ack.md),
[B2](m039-tranche-7-b021b2-chat-disconnect.md), and
[B3/integrated acceptance](m039-tranche-7-b021b3-shutdown-integrated.md).
