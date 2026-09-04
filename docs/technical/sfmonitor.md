# sfmonitor Technical Architecture

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)

`sfmonitor` 0.1 is a separate workspace binary and process. The `sf-monitor`
crate owns only terminal presentation, ephemeral view state, and a bounded
client worker. It imports the platform-neutral `OperatorClient` from `sf-bbs`
and the localization/event filter types from `sf-core`; it does not link to a
`BoardRuntime`, open SQLite, parse logs, or create a second observability
authority.

## Dependency and process boundary

The terminal stack is Ratatui 0.30.2 with Crossterm 0.29.0. Both are MIT
licensed. Ratatui is used without its default feature collection and enables
only the Crossterm backend. Crossterm supplies native terminal input, resize,
raw-mode, alternate-screen, and Windows/Unix support. Cursive, Termion,
Termwiz, and a custom renderer were considered but add no necessary capability
to this bounded monitor or provide a narrower platform fit. Their authoritative
upstreams are [`ratatui/ratatui`](https://github.com/ratatui/ratatui) and
[`crossterm-rs/crossterm`](https://github.com/crossterm-rs/crossterm); binary
packaging must retain their MIT notices through the repository's normal
third-party notice process.

The daemon and monitor remain independently disposable processes. Zero, one,
or several monitors may attach; a monitor exit or panic does not affect
caller sessions. The board configuration path is mandatory, preventing
accidental discovery/attachment when multiple boards run on one host.

## Connection and refresh lifecycle

Startup follows the existing B021-A path: resolve the selected board endpoint,
verify the OS peer through UID/SID policy, complete challenge/session binding,
negotiate protocol 1.x/features, then request the B-017 snapshots. One bounded
worker owns two `OperatorClient` connections so a one-second live-event wait
cannot delay status refreshes. The UI and worker communicate through a
16-command and 64-update bounded channel; full delivery marks an explicit gap
rather than blocking event production or caller work.

Snapshots refresh no more frequently than every five seconds. Activity uses a
100-row typed recent-event query plus the existing bounded live subscription.
Category, severity, outcome, node, and recent-time filters compile to
`OperatorEventQuery`; the same filter is applied to newly received events.
The client retains no more than 100 activity rows. A server or client-flow gap
is visible until a durable refresh succeeds.

A lost connection freezes the last snapshot and labels it stale. Reconnect is
manual in 0.1. `R` creates new clients, repeats peer authentication and feature
negotiation, and replaces all snapshots. It never reuses a challenge, session,
subscription, or daemon generation.

## View and privacy model

The eight implemented destinations are Dashboard, Nodes, Callers, Activity,
Statistics, Notifications, Maintenance / Errors, and System Configuration.
The last is an informational doorway only. Transfers remain on Dashboard and
node detail because B-017 currently supplies an aggregate active-transfer
count and per-node transfer state, not a dedicated transfer-list projection.

Rendering uses only `BoardStatusWire`, `NodeStatusWire`, `EventWire`,
`NotificationWire`, `StatisticsWire`, `RecentCallerWire`, and
`MaintenanceWire`. Event attributes, correlation/object identifiers, and
session identifiers are deliberately not rendered. Tests place a sentinel
secret in typed event attributes and verify it cannot reach the terminal
buffer. No mutation feature, protocol operation, worker command, or keybinding
exists in this crate.

## Keyboard, size, and terminal lifecycle

All essential operation uses arrows, Tab/Shift-Tab, Enter, Esc, `/`, `R`, `?`,
and `Q`; F1 help plus F2/F3/F4 shortcuts preserve a modest SPITFIRE operator
flavor without making function keys mandatory. Key-release events are ignored.
Mouse handling is deferred.

At `100x30`, navigation and node list/detail can coexist. `80x24` uses a
single compact content pane and Enter for node detail. Below `60x20`, only a
localized minimum-size message is rendered. Resize events cause layout to be
recomputed while selection/filter state remains in the model.

The terminal boundary enables raw mode and the alternate screen only after
interactive stdin/stdout are confirmed. An idempotent restoration guard
disables raw mode, leaves the alternate screen, and shows the cursor on normal
return, I/O error, Ctrl-C, or stack unwinding. The panic boundary restores the
terminal before resuming the panic.

## Platform and future boundary

The same binary and `OperatorClient` use Unix-domain sockets on supported
Unix-like systems and the accepted ACL/SID named pipe on Windows. Current
Windows CI provides source/build and focused monitor coverage. Interactive
Windows input, resize, and rendered-TUI acceptance are deferred until a
suitable real Windows environment is available; compile-time or automated
coverage is not presented as that live acceptance.

B021-B may later add stale-safe page/chat, time, disconnect, and daemon
shutdown commands to the daemon and client. B021-C will add versioned typed
configuration and the separate `sfconfig` binary. Those additions must not
move business logic into this TUI. B-022 reports/publication, networking,
doors, and scheduler/jobs remain separate domains.

See [Protected Operator Attachment](operator-control.md) for transport and
authorization, and [Operator Observability](observability.md) for the data
projections.
