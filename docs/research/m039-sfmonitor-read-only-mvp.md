# M039 `sfmonitor` 0.1 Read-Only MVP

**Date:** 2026-09-04
**Schema:** 19; no migration
**Parity:** B-021 remains PARTIAL; B-022 remains NOT STARTED

## Outcome

`sfmonitor` is now a real, separately runnable, read-only local operator
application. It attaches to an independently running board through the
cross-platform `OperatorClient`, renders the existing B-017 projections, and
survives client exit or daemon loss without taking ownership of the daemon.
It has no direct SQLite, text-log, runtime-internal, or mutation path.

The implemented views are Dashboard, Nodes, Callers, Activity, Statistics,
Notifications, Maintenance / Errors, and the honest future doorway named
System Configuration. Selecting the last view explains that `sfconfig` is not
available; no placeholder editor or configuration protocol was added.

## Dependency decision

The accepted candidates remain the smallest practical fit:

| Dependency | Selected version | License | Role |
|---|---:|---|---|
| Ratatui | 0.30.2 | MIT | state-driven widgets and responsive layout |
| Crossterm | 0.29.0 | MIT | keyboard/resize events and terminal lifecycle on Unix and Windows |

Ratatui is built without default features and with only its Crossterm backend.
The authoritative upstreams are
[`ratatui/ratatui`](https://github.com/ratatui/ratatui) and
[`crossterm-rs/crossterm`](https://github.com/crossterm-rs/crossterm). Both
are actively maintained, support current Rust and the required platform
families, and do not change SPITFIRE NG's MIT-or-Apache-2.0 license. A future
binary package must preserve the applicable MIT notices through the existing
third-party notice collection process.
Cursive and Termwiz remain possible for other products but add a less direct
abstraction for this accepted layout. Termion is Unix-oriented. A custom TUI
framework was rejected because it would duplicate terminal lifecycle,
event-normalization, resize, and widget work without improving the authority
model.

## Reference review

A bounded secondary engineering review considered mature BBS monitor and
configuration workflows without treating them as SPITFIRE authority or
copying their source, documentation, artwork, or layout.

- **ADOPT:** a status-first live cockpit, persistent keyboard discovery,
  selected-node detail, and a visible path from monitoring to System
  Configuration.
- **ADAPT:** dense node/statistics/activity organization into responsive
  SPITFIRE-branded, privacy-safe views backed only by typed projections.
- **REJECT:** direct node-file/config/log/process authority, caller spying,
  private endpoint display, fixed DOS geometry, and literal palette/layout
  reproduction.
- **DEFER:** live actions, configuration editing, networking, doors, jobs,
  reports, and maintenance execution to their owning slices.

A bounded read-only FireComm review was appropriate because this slice changes
terminal interaction. The adopted lessons were portable key-event
normalization, logical-state preservation across full resize redraws, and
explicit non-color status. SPITFIRE NG uses Crossterm for its own terminal
lifecycle. FireComm supplied no code, dependency, or product authority and was
not modified.

## Architecture and authority

The new `sf-monitor` workspace crate contains a small reusable library and the
`sfmonitor` binary. UI/model/worker concerns are separate, while protocol,
authentication, board targeting, B-017 wire projections, and safe errors stay
in the existing operator client/service boundary.

Startup requires `sfmonitor --board <CONFIG-FILE>`. The worker establishes two
ordinary `OperatorClient` sessions: one for bounded snapshots and one for the
bounded live-event wait. Both use the existing protected UID/SID attachment,
challenge/session binding, daemon generation, protocol negotiation, feature
discovery, and dispatch-time authorization. The required feature set is
exactly the nine accepted reads: board status, node list/detail, recent/live
events, notifications, statistics, recent callers, and maintenance status.

No new server protocol operation was required. The UI command channel contains
only Refresh, Reconnect, and Stop. The operator protocol still exposes no
B021-B or B021-C mutation feature.

Initial and five-second bounded snapshots use limits of 100 recent events, 100
recent callers, and 100 notifications. A one-second long-poll carries live
events without aggressive client polling. Worker command/update channels are
bounded to 16/64 entries. A full local update channel marks a visible gap;
source gap markers are preserved and a snapshot refresh recovers authoritative
history. Slow UI rendering cannot block event production or caller sessions.

## Views and interaction

- Dashboard shows board/connection state, uptime, active nodes/callers/
  transfers, open notifications, warning/error state, today totals, and recent
  activity when space permits.
- Nodes provides selection and a privacy-safe detail pane on wide terminals;
  compact mode toggles detail with Enter.
- Callers clearly presents completed recent calls; online callers remain in
  Nodes.
- Activity combines bounded history and the live stream. Its typed filter
  cycles category, minimum severity, outcome, node, and recent time range.
  Event/category/severity/outcome text is localized; typed attribute payloads
  are never rendered.
- Statistics separates Live now, Today, and Lifetime and retains the honest
  Schema-18 history boundary.
- Notifications is read-only. Maintenance / Errors reports status but runs no
  operation. Transfer facts stay on Dashboard and Nodes because the current
  service has no dedicated active-transfer list projection.

Keyboard operation is complete: Left/Right or Tab/Shift-Tab changes views;
Up/Down, Page Up/Page Down, Home, and End select; Enter toggles node detail or
opens the configuration explanation; `/` opens Activity filters; `R` refreshes
or reconnects; `?`/F1 opens contextual help; F2/F3/F4 open Dashboard, Nodes,
and Activity; and `Q` or Ctrl-C quits only the monitor. Key-release events are
ignored. Mouse support is deferred because it is unnecessary for complete
operation.

The footer always names the current view, connection state, and key hints.
Color is restrained and never the sole signal. The title and vocabulary retain
SPITFIRE NG's board/node/caller/Sysop identity without copying a historical or
third-party screen.

## Terminal and connection lifecycle

100x30 is the preferred list/detail layout. 80x24 is a complete compact mode.
Below 60x20 the monitor shows a safe size notice and continues listening for a
resize. Repeated 59x19, 80x24, 100x30, and 120x40 transitions preserve logical
state and redraw without panic.

Raw mode, alternate-screen entry, cursor hiding, and their inverse are wrapped
in an idempotent guard. Normal quit, returned error, panic unwinding, and
Ctrl-C through the event boundary restore raw mode, alternate screen, and
cursor. Any client panic remains in the monitor process and cannot affect the
daemon.

Daemon loss changes the heading to `DISCONNECTED — STALE`, preserves but
labels the last snapshot, and stops claiming it is live. `R` creates new
connections, reauthenticates, renegotiates, reloads all snapshots, and opens a
new subscription; it never reuses the previous daemon generation. Automatic
reconnect is deliberately deferred so repeated authorization or board-target
errors cannot be hidden.

## Privacy and read-only proof

Only B-017 wire projections enter the view model. The monitor does not render
event attributes, login identifiers, private real names, contact/birth data,
message bodies or recipients, file contents, terminal input, endpoints, host
paths, credentials, hashes, keys, packets, or future network secrets. Tests
place sentinel secret data in an event attribute and prove it never appears in
the rendered terminal buffer.

The monitor has no mutation enum, request, feature, service call, or key
binding. It cannot acknowledge notifications, page/chat, grant time,
disconnect, shut down, edit configuration, execute backup/retention/
maintenance, edit callers, or mutate files. `Q` terminates only `sfmonitor`.

## Acceptance

The real macOS run used a disposable schema-19 board with two configured nodes
and a daemon in a separate process. Two simultaneous caller sockets appeared
in Nodes. A registered caller then logged in, posted a safe public message,
and disconnected; Activity showed the ordered live session/message events,
Callers showed the completed calls, and Statistics showed two completed calls
and one message in Today and Lifetime. Dashboard, Nodes/detail, Callers,
Activity/filtering, Statistics, empty Notifications, Maintenance / Errors,
System Configuration, and contextual help were visually inspected.

Two `sfmonitor` processes attached concurrently. Quitting one restored its
terminal and left the daemon and other monitor running. Stopping the daemon
made the remaining monitor explicitly stale; restarting the daemon and
pressing `R` completed new authentication/negotiation and refreshed state.
The actual PTY was resized through 59x19, 80x24, 100x30, and 120x40; the
minimum notice, compact and wide layouts, and return to a usable shell passed.
No screenshot is published.

The x86_64-pc-windows-msvc workflow includes native `sf-monitor` tests and
executable startup/version validation. That provides source/build coverage and
compatibility with the accepted named-pipe client path; it is not a substitute
for a rendered interactive Windows TUI journey. Interactive Windows terminal
input, resize, and presentation remain deferred until a suitable real Windows
environment is available for a dedicated acceptance pass. Linux/BSD live TUI
execution was not available; the selected terminal stack and code introduce no
Unix-only monitor path, but live platform acceptance remains future work.

Portability testing also exposed an existing high-resolution timestamp
collision in concurrent upload operation identities. The narrow correction
uses a random 128-bit operation suffix, acquires the filename lease in an
immediate transaction, and removes a destination during byte-publication
rollback only when that operation created it. Regression coverage preserves
the one-winner rule; the correction changes no monitor feature or schema
contract.

The accepted implementation suite passed all applicable workspace tests, with
the two established manual interoperability servers intentionally ignored.
The monitor contributes 22 focused tests. Source-header, formatting, Clippy,
diff, localization, privacy/provenance, and Markdown/link validation passed.
`cargo-audit` was not installed.

## Status and next boundary

The monitor MVP is IMPLEMENTED and accepted as the intended operator-product
interleave. B-021 remains PARTIAL because B021-B live controls, B021-C typed
configuration/`sfconfig`, and B021-D full maintenance/platform integration are
still open. B-022 remains NOT STARTED. Category B remains 14 VERIFIED, 2
IMPLEMENTED, 4 PARTIAL, and 5 NOT STARTED. Schema remains 19.

The next separately authorized implementation slice is B021-B stale-safe live
operator controls over the existing command journal, control audit,
generations, and operator client. That work must extend this monitor through
typed daemon authority rather than moving business logic into the TUI.
