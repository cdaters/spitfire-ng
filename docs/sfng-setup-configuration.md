# SPITFIRE NG Setup and Configuration

## Purpose

This document is the canonical Increment 4 description of how a Sysop creates
and administers a native SPITFIRE NG board. It covers implemented behavior,
authority boundaries, validation, and safe operational use. The broader stock
requirement remains the [SPITFIRE 3.7 parity checklist](stock-spitfire-3.7-parity.md).

The preserved Buffalo Creek `spitfire.doc` is the primary historical source.
Original SPITFIRE exposed board identity, Sysop identity, logical paths, caller
defaults, message conferences, and per-node settings through Sysop
configuration. SPITFIRE NG preserves those operating concepts while replacing
manual DOS file choreography with validated configuration and transactional
storage.

## Implemented Commands

```text
spitfire setup <BOARD-DIRECTORY>
spitfire config <CONFIG-FILE>
spitfire status <CONFIG-FILE>
spitfire backup <CONFIG-FILE> <BACKUP-DIRECTORY>
spitfire restore <BACKUP-DIRECTORY> <BOARD-DIRECTORY> [--replace]
spitfire run <CONFIG-FILE>
```

`setup` is an interactive first-run workflow. It refuses an existing board
directory before collecting wizard data and asks for board/Sysop identity,
node count, enabled Telnet/raw TCP/RLogin services and their localhost-safe
bind addresses/ports, caller-experience preset, active/base presentation
profiles, generated/display-override menu mode, independent post-login
journey, new-caller security, configurable Sysop threshold, initial Sysop
caller security, IANA board timezone, public/private admission, idle and daily
time policy, Disabled/Optional/Required caller profile groups, and a
non-echoed initial Sysop password. Disabled listeners skip their dependent
endpoint questions. Listener choices accept `y`/`yes` and `n`/`no`
case-insensitively. The prompt explains the timezone's board-local effect
and gives IANA examples; password length/confirmation errors reprompt before
board creation. It then creates
validated TOML, the five logical directories, SQLite schema, the Sysop caller,
General and SPITFIRE message conferences, General Files and SPITFIRE Files
areas with small generated text resources, and clearly labeled project-owned
display/help resources packaged as `modern-ng`, `minimal-terminal`, and
`classic-spitfire`. Modern 1.0.0 remains the normal active/base default; the
other profiles are not selected unless the operator explicitly chooses them.
It does not install proprietary Buffalo Creek files or a default secret.

`config` is a SPITFIRE-oriented terminal menu over the same domain validation.
It currently edits implemented settings only: general identity/timezone/access,
nodes, terminal services, caller/security/profile defaults, message
conferences, file areas, and presentation mode/active/base profile IDs. Static
changes are explicitly saved. Conference and file-area changes are
transactional and immediate; disabling either preserves stable identity and
content.

`status` is read-only. It reports the configured board and services and, while
the board is running, the transient state of each node without exposing
credentials. A status file left by a process that did not shut down cleanly is
labeled as potentially stale rather than treated as proof that a process is
alive. It also reports the active/base presentation IDs and loaded versions,
effective source, menu mode, post-login journey, new-caller security, Sysop
threshold, and a path-free ready/degraded result. A missing active
profile may therefore be diagnosed while its valid base keeps the board usable.

`backup` and `restore` are stopped-board Sysop workflows. Backup writes a
validated native snapshot containing exact configuration, consistent SQLite,
SYSTEM/DISPLAY resources, and every cataloged byte. Restore creates a new board
by default; `--replace` is required for an existing same-identity board. Both
use the runtime/configuration operation lock and validate the complete snapshot
before target mutation. The canonical content, recovery, rollback, and
limitation contract is [Native Backup and Restore](sfng-backup-restore.md).

## Configuration Authority Boundaries

Three kinds of state deliberately have different authorities:

| State | Authority | Examples |
|---|---|---|
| Editable static configuration | `spitfire.toml` | board/Sysop display identity, IANA timezone, public/private policy, logical paths, database filename, presentation mode and active/base IDs, node definitions, listeners, caller/profile defaults, password-cost policy |
| Persistent operational state | SQLite | caller identities/credentials/private profiles/statistics, message conferences/messages/last-read, file-area and file metadata |
| Persistent file bytes | confined host storage under logical `EXTERNAL` | generated starter files and committed caller uploads |
| Ephemeral upload state | per-session staging under logical `WORK` | incomplete/canceled caller upload bytes, never a valid catalog entry |
| Transient runtime state | in-memory node manager; published snapshot under `WORK` | waiting/login/online/uploading/downloading node state, active caller, transport, connection time, catalog filename |

The setup wizard, TOML loader, interactive configuration menu, and runtime all
use `RuntimeConfig::validate`; they do not maintain divergent validation rules.
Board identity is mirrored in SQLite as a consistency guard. An administrative
identity edit uses a compare-and-update operation and attempts rollback if the
atomic TOML write fails. Moving logical paths or the operational database is
rejected by the current interactive service because it requires a separately
designed data-migration workflow.

## Static Configuration Model

Configuration format version 2 adds the node pool and named/enabled listener
shape. Existing Increment 0 format-1 singleton `[node]` files continue to
load. M031 adds an optional strict `[presentation]` section without changing
the configuration-format number: omitted sections mean explicit
`legacy-resources`, while newly written boards select `modern-ng` as active and
base. Newly written boards use:

```toml
[presentation]
mode = "profile"
menu_mode = "display-overrides"
active_profile = "modern-ng"
base_profile = "modern-ng"

[language]
default_locale = "en-US"

[nodes]
count = 4

[[nodes.overrides]]
number = 4
enabled = false
description = "Reserved node"
```

Listeners are repeated, named adapters. Multiple instances of one transport
are permitted when endpoints differ:

```toml
[[transports]]
name = "telnet-primary"
enabled = true
type = "telnet"
listen = "127.0.0.1:2323"

[[transports]]
name = "telnet-secondary"
enabled = false
type = "telnet"
listen = "127.0.0.1:3323"
```

The validator rejects missing/all-disabled nodes, duplicate node overrides,
duplicate names, enabled listener/address conflicts, enabled serial-device
conflicts, port zero, invalid terminal dimensions, unsafe serial/modem values,
unknown types/options, and enabled SSH. SSH remains a recognized but
fail-closed future adapter.

M037.2 adds the optional `[language]` table without changing configuration
format 2; omitted legacy configurations resolve to `en-US`, while new setup
writes it explicitly and installs the validated board-local en-US package.
Locale identity is canonical BCP 47 and remains independent from presentation.
See [Localization Contract](localization.md) and the
[operator language guide](operator/localization.md).

Security levels are operator-defined integers from 0 through 9999. Setup keeps
the new-caller level, configured Sysop threshold, and initial Sysop caller
level distinct; the initial value must be at least the threshold. The threshold
default of 50 is an NG setup choice, not a claimed historical constant.
`.MNU` minimum-security records and exact display suffixes are separate again.

`presentation.menu_mode = "display-overrides"` permits board/active-profile
exact-security BBS/CLR menu art and otherwise generates from the parsed `.MNU`.
`"generated"` bypasses exact menu art deliberately. The post-login `none` or
`stock` setting remains under caller policy and cannot be selected or reordered
by a profile.

Default network listeners bind `127.0.0.1`, using nonprivileged ports 2323
(Telnet), 2324 (raw TCP), and 2513 (RLogin). These are setup defaults, not
protocol requirements. A Sysop must explicitly select any non-loopback bind
and accept the plaintext security properties of Telnet/raw/RLogin.

## Message Conference Administration

Conference numbers remain stable caller-visible identities. The configuration
service can list, create, rename, redescribe, configure current read/entry
security and access mode, set the current public-only and line-limit fields,
and enable/disable a conference. Conference 1 cannot be disabled because the
implemented stock Comment-to-Sysop path depends on it. The service does not
delete conference rows or cascade message deletion. Network, retention,
packing, and advanced conference fields remain deferred and visible in the
parity checklist.

## File Area Administration

The configuration service lists area number/name/state/file count and can
create, edit, or safely disable an area. Implemented policy includes stable
number and storage key, description, threshold/exact access, read and upload
security, preview/no-charge flags, maximum upload bytes, and up to five
privileged security levels. Creating an area also creates its confined
`EXTERNAL/files/<storage-key>` directory.

Renumbering, storage relocation, and destructive deletion fail closed. These
require separately designed workflows so an edit cannot orphan metadata,
escape a logical path, or delete physical files. See the
[native file specification](sfng-file-system.md).

## Security and Failure Behavior

- Setup refuses an existing target before collecting wizard data.
- Setup validates Sysop password length and confirmation at password entry and
  does not create a partial board after a recoverable password mistake.
- Static configuration is validated before an atomic replacement.
- Password entry is not echoed, logged, or stored in TOML; only Argon2id PHC
  credentials enter SQLite.
- Runtime listener defaults are local-only and every listener is optional.
- Expected validation/database/setup errors return useful errors rather than
  panicking.
- Configuration does not grant caller identity or Sysop access. Local shell
  and transport identities remain separate from SPITFIRE authentication.
- Reconfigure a stopped board. Live configuration reload and coordinated data
  relocation are not implemented.
- Backup and restore are likewise cold operations. A running runtime,
  configuration session, or other recovery operation holds the board-wide OS
  lock and makes them fail closed.

## Verification

Committed synthetic tests cover setup/preset independence, initial-Sysop
security validation, disabled-listener prompting, refusal to overwrite, strict
configuration round trips, listener conflicts and same-type listeners,
caller-default changes, board-identity persistence, safe conference edits,
safe file-area edits/disable/create with preserved files, status output, and
loading boards created by earlier schema revisions. No test requires
historical assets or a committed password.

For node allocation and live status behavior, see
[SPITFIRE NG Multinode Runtime](sfng-multinode-runtime.md).

## Known Gaps

- No live reload or complete final operator console. Increment 6 adds
  `spitfire console` for live status/page/chat/disconnect and essential caller
  list/state/security operations; see
  [Caller/Sysop Interaction](sfng-caller-sysop-interaction.md).
- Caller administration is intentionally limited to list, enable/disable, and
  security adjustment rather than arbitrary record editing.
- Native backup/restore is CLI-based; there is no remote/web UI, legacy
  `.$$$`/`.$??` importer, configuration-history browser, or live snapshot.
- File-area configuration covers the working Increment 5 fields; destructive
  delete/relocation, ratios, transfer-protocol selection, and archive/upload
  review policy remain absent.
- No event, door, network, menu/display, or SSH configuration UI.
- No stable service-control protocol for a separately running shell session or
  remote web administration.
- Full stock SPITFIRE configuration fields remain checklist work; the menu
  intentionally shows only settings backed by current behavior.
