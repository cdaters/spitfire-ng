# Sysop Guide

## The two server modes

SPITFIRE NG currently runs in the foreground. Choose one mode for a board:

```bash
spitfire run /path/to/board/spitfire.toml
```

`run` starts listeners and writes runtime logs to the terminal. Stop it with
Ctrl-C.

```bash
spitfire console /path/to/board/spitfire.toml
```

`console` starts the same listeners and keeps an operator command prompt in
that process. It is not an attachable client. Do not start `console` beside an
existing `run`; the board operation lock prevents unsafe dual ownership.

There is no daemon/service unit, background launcher, remote administration,
or log rotation in this Development Preview. If you wrap the process with
host service management, treat that wrapper as local deployment work and
retain clean signal delivery for shutdown.

## Read-only status

From another terminal:

```bash
spitfire status /path/to/board/spitfire.toml
```

Status shows product/board identity, listeners, node state, and the current
public-directory/last-call/location/caller-additions policy with its version.
For an active
authenticated session it also reports the engine-known transport, reported
terminal type when present, ANSI state, effective encoding, negotiated size,
effective page length, locale, profile, menu mode/context, caller security,
configured Sysop threshold, visible authorized action count, and the selected
menu renderer path. `generated-stock` is the engine's equal-column renderer;
`exact-security-board-override` is matching BBS/CLR artwork from this board's
`display/`; `exact-security-active-profile` is matching managed artwork from
the selected package; `expert-suppressed` means expert mode intentionally
omitted menu presentation. A missing terminal type is reported as unknown;
SPITFIRE NG does not guess application names such as Qodem or SyncTERM. These
diagnostics are node-local and contain no password, contact data, remote
address, or private caller profile fields.

A clean stop removes the runtime status snapshot. After a crash,
`published/running or not
cleanly stopped` means exactly that: the file may be stale. Confirm the host
process separately. Do not delete operation-lock files; lock ownership, not
their presence, determines whether the board is busy.

## Live read-only monitor

Current source also builds `sfmonitor`, a separate keyboard-first application
for leaving the board's live status open in another terminal:

```bash
sfmonitor --board /path/to/board/spitfire.toml
```

It uses the running daemon's protected local operator service and shows the
same board, node, activity, statistics, notification, caller, and maintenance
facts as the typed operator client. It does not read the database or logs and
cannot disconnect callers, grant time, acknowledge notifications, stop the
daemon, run maintenance, or change configuration. Press `Q` to quit only the
monitor. See [Using sfmonitor](../manual/sfmonitor.md) for the complete keyboard
and troubleshooting guide.

## Operator console commands

The console prints its command list at startup:

| Command | Purpose |
|---|---|
| `STATUS` | Show current node/session/caller/transport state. |
| `PAGES` | List caller page requests by session and node. |
| `AVAILABLE ON` / `AVAILABLE OFF` | Allow or refuse caller Page the Sysop requests. |
| `ANSWER <session>` | Enter line chat for one pending page; `/Q` ends chat. |
| `DECLINE <session>` | Decline one pending page. |
| `DISCONNECT <session>` | Request a controlled session disconnect. |
| `CALLERS` | List caller IDs, names, security, state, and call count without credentials. |
| `PROFILE <name>` | Show that caller's private enabled profile fields. |
| `PROFILE-SET <field> <name>|<value>` | Set or clear an authorized profile field. |
| `INFO-POLICY` | Show current public-directory and caller-addition policy/version. |
| `INFO-POLICY-SET ...` | Replace public-information policy using its expected version. |
| `BBS-LIST` / `BBS-ADD` | List all native Other BBS rows or append an operator row. |
| `BBS-EDIT` / `BBS-MOVE` / `BBS-STATE` | Versioned edit, atomic reorder, and disable/restore of Other BBS rows. |
| `ENABLE <name>` / `DISABLE <name>` | Change account admission state. |
| `SECURITY <level> <name>` | Set numerical caller security. |
| `QUIT` | Stop the console-owned server and return to the shell. |

Profile field names are `address1`, `address2`, `city`, `region`, `postal`,
`country`, `phone`, `email`, and `birthday`. Birthday uses `YYYY-MM-DD`; an
empty value after `|` clears a field.

## Caller-facing Sysop identity

Host administration and BBS identity are separate:

- `board.sysop` is the caller-visible display name;
- `caller.sysop_caller_name` is an ordinary authenticated caller account;
- `caller.sysop_security` is the traditional BBS authority threshold; and
- operating the host shell does not automatically authenticate a BBS caller.

Log in through a terminal as the Sysop caller to post messages, upload files,
use Comment to Sysop receipt paths, and see the security-gated `@` Sysop menu.
The current caller-facing Sysop menu provides a safe navigation boundary;
historical maintenance commands outside the implemented core report
unavailable. Use the host `config`, `console`, backup, and restore commands for
their documented operations. See [Public Information](../sfng-public-information.md)
for exact M043 console syntax and privacy rules.

## Routine operation

Before startup:

1. Confirm the prior process stopped cleanly.
2. Run `spitfire status`.
3. Check that expected bind addresses and ports are shown.
4. Start `run` or `console`, never both.

While online:

1. Watch startup/listener errors and session logs.
2. Use `status` or console `STATUS` for nodes.
3. If using console paging, set availability deliberately.
4. Do not run configuration, backup, or restore against the live board.

At shutdown:

1. Let callers log off or disconnect sessions deliberately.
2. Use Ctrl-C in `run`, or `QUIT` in `console`.
3. Wait for `listeners shut down cleanly`.
4. Run status and create a cold backup on the selected schedule.

## Network security

Telnet, RAW TCP, and RLogin are compatibility protocols. They expose caller
passwords and content to anyone able to observe the connection. Default
loopback binds are deliberate. Restrict non-loopback listeners to an
appropriate trusted network/VPN and firewall, or use the separately configured
SSH caller transport.
