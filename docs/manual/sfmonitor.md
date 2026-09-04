# Using sfmonitor

<!-- help-topic: operator.dashboard -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)
>
> **Latest downloadable release:** SPITFIRE NG 0.1.0 Development Preview
>
> The downloadable Development Preview predates `sfmonitor`.

`sfmonitor` is the local, live view of a running SPITFIRE NG board. You can
leave it open to see callers arrive, nodes change, activity appear, and
conditions that need attention. It is read-only: quitting or losing the
monitor does not stop the board, and the monitor cannot disconnect callers or
change configuration.

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

**Notifications** shows open operator attention items. Acknowledgement is not
available in this read-only release.

<!-- help-topic: operator.errors -->

**Maintenance / Errors** shows open warnings and errors, unavailable storage,
files awaiting review, active or incomplete transfers, and retention periods.
It cannot run cleanup, backup, repair, or other maintenance.

**System Configuration** establishes the route that will later open
`sfconfig`. Current source says clearly that configuration is not available;
it does not show fake editors or write board state.

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
