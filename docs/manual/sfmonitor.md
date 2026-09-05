# Using sfmonitor

<!-- help-topic: operator.dashboard -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)
>
> **Latest downloadable release:** SPITFIRE NG 0.1.0 Development Preview
>
> The downloadable Development Preview predates `sfmonitor`.

`sfmonitor` is the local, live view of a running SPITFIRE NG board. You can
leave it open to see callers arrive, nodes change, activity appear, and
conditions that need attention. Monitoring is read-only by default. Explicitly
enrolled controls add notification acknowledgement, time adjustment, page/chat,
confirmed caller disconnect, and graceful daemon shutdown. Quitting or losing
the monitor does not itself stop the board. System Configuration opens the separate [sfconfig application](sfconfig.md).

## Start the monitor

Start the board normally. In a second terminal, identify that board explicitly:

```console
sfmonitor --board /path/to/board/spitfire.toml
```

The operating-system account must be enrolled as a local host operator. The
account that creates a new board is enrolled automatically: SPITFIRE NG records
its UID on Unix/macOS or stable SID on Windows. A BBS caller's security level,
including named Sysop status, does not grant local monitor access.

If the daemon is unavailable, `sfmonitor` reports that the selected board is
not running; it does not start the board or fall back to reading its database.
An authorization error means the current local account is not enrolled. A
protocol mismatch means the monitor and daemon need compatible current-source
builds.

## Move around

The monitor is keyboard-complete:

| Key | Result |
|---|---|
| Left/Right or Tab/Shift-Tab | Move between views. |
| Up/Down, Page Up/Page Down | Select a visible row. |
| Enter | Show or close compact node detail. |
| `/` | Open typed Activity filters. |
| `R` | Refresh; when disconnected, establish a new attachment. |
| `F1` or `?` | Show help for the current view. |
| `F2`, `F3`, `F4` | Open Dashboard, Nodes, or Activity. |
| `Esc` | Close help, filters, or compact detail. |
| `Q` or Ctrl-C | Quit `sfmonitor` only. The BBS continues running. |

Mouse input is not required or enabled in version 0.1.

## Views

<!-- help-topic: operator.nodes -->

**Dashboard** combines uptime, active nodes, callers, transfers, today's
counts, open notifications, warnings, errors, and the most recent safe board
activity without overcrowding the screen.

**Nodes** lists configured nodes and shows the selected node's public caller
handle, connection type, time online, current area, terminal, presentation,
security context, and transfer state when those facts are available. It never
shows login names, private profiles, remote addresses, terminal input, or
message text.

<!-- help-topic: operator.callers -->

**Callers** shows recently completed calls from retained activity. Use Nodes
for callers online now. Recent callers use public handles and safe call facts.

<!-- help-topic: operator.activity -->

**Activity** combines the retained recent-event view with new live events.
Press `/` to cycle category, minimum severity, outcome, node, and recent-time
filters. A visible `GAP` warning means the client could not keep up with every
live update; press `R` to recover from durable recent activity instead of
assuming nothing happened.

<!-- help-topic: operator.statistics -->

**Statistics** separates Live now, Today, and Lifetime. Detailed history starts
when observability was activated; the monitor does not invent pre-schema-18
events.

**Notifications** shows open operator attention items. Select one and press
`A`, then choose **Acknowledge**. This is a narrow audited state transition;
the source event remains available.

On a selected live node, press `A` for **Actions**, then `+`/`=` or `-` to
request the historical five-minute session-time adjustment. The monitor uses
the daemon's current target generation and confirms the authoritative result;
it never changes permanent caller policy. A stale or unauthorized target is
reported without mutation.

<!-- help-topic: operator.errors -->

**Maintenance / Errors** shows open warnings and errors, unavailable storage,
files awaiting review, active or incomplete transfers, and retention periods.
It cannot run cleanup, backup, repair, or other maintenance.

**System Configuration** opens [sfconfig](sfconfig.md) for the same explicit
board. Press Enter to hand over the terminal; Q in sfconfig returns to a
refreshed monitor. Install the matching executables beside each other.
Configuration permissions are independent of monitor permissions; bootstrap
remains read-only. The daemon and callers continue during the handoff.

## Explicit mutation enrollment

Board creation and older-board owner bootstrap provide monitoring access,
not mutation permission. Retain the six read capabilities for all monitor
views. To authorize live controls, deliberately add only the desired entries
to the existing local identity in `spitfire.toml`:

```toml
capabilities = [
    "board-statistics", "node-status", "operational-events",
    "caller-activity", "notifications", "maintenance-status",
    "acknowledge-notifications", "adjust-session-time",
    "manage-page-availability", "manage-caller-pages",
    "chat-with-caller", "disconnect-session",
    "request-graceful-shutdown",
]
```

This is the capability field of an existing `[[operators.local_identities]]`
entry, not a complete configuration. Preserve its actual Unix UID or Windows
SID; do not substitute an account name or Administrator group. Omit any
control that is not intended. Profiles allow at most 32 recognized unique
capabilities; the limit grants nothing automatically. No role/configuration
editor is available in sfmonitor.

Default and omitted capability lists remain read-only. An older empty-list
Unix board that previously received unintended B1 mutation access now needs
explicit enrollment; do not remove the allowlist to obtain controls.
Windows still requires its enrolled SID and listener access. See
[operator identity and policy](../technical/operator-control.md#local-endpoint-and-identity).

Actions explain whether the daemon lacks a feature or the operator lacks
permission. Supported but unauthorized actions cannot be executed. Capability
refresh follows the normal monitor refresh; the daemon checks again at every
dispatch, so revocation does not wait for the next screen update.

## Actions and caller pages

<!-- help-topic: operator.actions -->

Select a live caller in Nodes, then press `A` for Actions. The existing `+`/`-`
five-minute time presets remain; Notifications has `A` Acknowledge. `P` opens
Page / Chat and `D` opens Disconnect. Actions explain unsupported features,
missing permissions, busy interactions, and stale targets. `F1` opens help for
the focused control. Always refresh a stale target instead of selecting an old
node number again.

Dashboard `A` Actions offers the separate global **Shutdown SPITFIRE NG** action.
It requires its own explicit grant; `Q` remains quit-monitor only.

## Page / Chat

<!-- help-topic: operator.page-chat -->

In `A` Actions → `P` Page / Chat, `O` changes runtime Sysop page availability.
The selected caller's pending page is indicated; `A` answers and `D` declines.
Disposition needs `manage-caller-pages`; answering also needs `chat-with-caller`.
Decline gives the caller the established unanswered-page outcome.

`I` invites the exact selected session. The caller sees a localized invitation
at a safe menu prompt and accepts or declines; there is no forced takeover or
spy mode. Accepted operator-initiated chat pauses ordinary caller time. The
historical caller-initiated page/chat path retains its existing time behavior.

The focused pane shows the public handle/node, state, current conversation, and
input. Enter sends a bounded line; Esc ends chat and returns the caller to the
previous prompt. The conversation is held only in the running monitor's memory
and discarded when chat ends or either connection is lost. It is never logged,
journaled, audited as text, saved as messages, exported, or restored on reconnect.
Do not expect chat receipts to recover a transcript or restart an invitation.
Within the pane Q is ordinary text; Ctrl-C quits the monitor, ending its chat.

## Graceful caller disconnect

<!-- help-topic: operator.disconnect -->

`A` Actions → `D` Disconnect offers `1` with Sysop notice and `2` without notice.
The daemon supplies current target/impact for a confirmation dialog. Review the
public caller/node, notice choice, and active transfer/chat warnings. Enter
confirms; Esc cancels. A changed session or impact requires fresh confirmation.

Both choices cooperatively end interactions, cancel/finalize transfers, settle
completed work and caller accounting, release the node, and close transport.
No-notice omits only the caller-visible notice; it is not a hard kill. A transfer
warning means unfinished work is cancelled, not credited as complete. Chat's
ephemeral buffer is discarded. After a three-second cooperative grace, the
daemon may use only the still-matching session's owned emergency close handle;
the result identifies fallback and does not claim completion before finalization.

If a response is lost, `R` reconnects and recovers the original CommandId receipt.
Do not repeat a destructive action with a new command ID merely because its
response was lost. Old receipts/targets cannot affect a replacement caller in
the same node slot. Nodes, Dashboard, and Activity refresh from daemon authority.
`disconnect-session` must be explicitly granted and is checked again at dispatch.

Real Windows page/chat/TUI/disconnect/transfer/race acceptance remains
**DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**; native macOS is the acceptance
host for this slice.

## Shutdown SPITFIRE NG

<!-- help-topic: operator.shutdown -->

From Dashboard, press `A` then `S` to request shutdown preflight. Review the
daemon's current caller, transfer, chat, and interaction counts. Enter confirms;
Esc cancels before dispatch. `request-graceful-shutdown` must be explicitly
enrolled; support discovery alone never grants it. Changed impact or an expired
preflight requires fresh review. This stops only the selected SPITFIRE NG daemon,
never the computer or another board.

Acceptance closes new caller admissions. Callers receive a board-shutdown notice;
chat ends and its ephemeral buffer is discarded. Transfers cooperatively cancel
and settle only completed work. The daemon allows three seconds for cooperative
completion, then only exact-session owned transport closure where necessary,
and up to six more seconds for finalization and control evidence. It exits
normally only after safe finalization. If that cannot be proven, it reports
failure and stays running with admissions closed instead of killing unfinished
work. There is no cancellation or restart command.

Shutdown continues if the requesting monitor exits. Other monitors show draining
where observable, then `DISCONNECTED — STALE`. A second request cannot start a
second shutdown. A recovered `shutdown-requested` receipt means the command was
accepted; final shutdown evidence is committed before normal daemon exit. After
exit, receipt queries are unavailable because the endpoint has stopped. Do not
create new CommandIds merely because the last response was lost.

`Q` quits sfmonitor only. To run the board again, use the normal board-start
command or your existing deployment mechanism, then `R` reconnect. sfmonitor
does not start or restart it. Real Windows shutdown/TUI acceptance remains
**DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**.

## Terminal size and reconnecting

`100x30` or larger gives a navigation list and side-by-side node detail.
`80x24` remains usable with one compact focused pane. Below `60x20`, the
monitor shows a safe resize notice instead of drawing a damaged screen.

If the daemon stops or restarts, current values are labelled `DISCONNECTED —
STALE`. Press `R` after the daemon is available. The monitor then verifies the
local account again, negotiates a new connection, and refreshes every view. It
never reuses authority from the old daemon generation.

For the meaning, privacy, and retention of the underlying views, see [Board
Activity and System Statistics](board-activity.md). For protocol and terminal
details, see [sfmonitor Technical Architecture](../technical/sfmonitor.md).
