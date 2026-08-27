# Development Preview: Getting Started

## Status

- **Verified:** The Apple Silicon prebuilt archive, setup, caller, message,
  file, backup, new-root restore, and restored-board workflow passed on the
  stated macOS/client acceptance environment. The source workflow remains verified.
- **Development Preview:** The archive is unsigned and not notarized; this is
  not a production deployment promise.
- **Planned:** Service installation, encrypted public listeners, and a broader
  verified host matrix are not available today.

This guide takes a first-time Sysop from a verified Development Preview archive
to a working local board, a first caller, a message and file check, and a
verified cold backup/restore. It uses only supported public commands. It does
not use a fixture board or edit SQLite. Developers may use the source route in
[Installation](installation.md).

The procedure was revalidated on Apple Silicon macOS with SPITFIRE NG 0.1.0,
SyncTERM 1.9rc4, and Qodem 1.0.1. Only `aarch64-apple-darwin` is an accepted
prebuilt target, so this is a Development Preview rather than a general 1.0
installation promise.

## What you will create

The examples use these local paths:

```text
~/Downloads/spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin/
~/Spitfire/boards/preview-board/        live board
~/Spitfire/backups/preview-board-001/   first cold backup
~/Spitfire/boards/preview-restored/     restore test
```

Choose different paths if needed, but use the chosen board configuration path
consistently. A setup, backup, or new-restore target must not already exist.

> Telnet, RAW TCP, and RLogin are plaintext compatibility transports. Keep the
> first board bound to `127.0.0.1`. Do not expose it to an untrusted network.

## 1. Verify and install the package

Follow the complete checksum procedure in
[Development Preview Package](development-preview-package.md). In short:

```bash
shasum -a 256 -c spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz.sha256
tar -xzf spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz
cd spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin
shasum -a 256 -c MANIFEST.SHA256
cp ./bin/spitfire "$HOME/.local/bin/spitfire"
command -v spitfire
spitfire --version
```

The expected version output for this Development Preview is:

```text
SPITFIRE NG Bulletin Board System 0.1.0
```

Create `$HOME/.local/bin` first if necessary and ensure it is on `PATH`, or run
the verified archive binary in place. SQLite is embedded; no separate database
server is required. For locked developer builds, use the source procedure in
[Installation](installation.md#developer-source-workflow).

## 2. Create the first board

Create only the parent directories, then run setup against a new board path:

```bash
mkdir -p "$HOME/Spitfire/boards" "$HOME/Spitfire/backups"
spitfire setup "$HOME/Spitfire/boards/preview-board"
```

Setup is interactive. For a safe local trial, use values like these:

| Prompt | Local-preview value |
|---|---|
| Board name | A caller-visible name, for example `The Night Owl BBS` |
| Sysop display name | The name shown to callers |
| Sysop caller name | A unique login name for the Sysop account |
| Number of nodes | `2` is enough to test simultaneous clients |
| Board timezone | The correct IANA name, for example `America/Phoenix` |
| Board access | `public` so a test caller can self-register |
| Telnet | enabled, `127.0.0.1`, port `2323` |
| RAW TCP | disabled, retain `127.0.0.1`, port `2324` |
| RLogin | disabled, retain `127.0.0.1`, port `2513` |
| Caller limits/security | Accept the defaults for the first local trial |
| Profile fields | Keep unnecessary personal-data groups disabled |
| Initial Sysop password | A unique password of at least ten characters |

Press Enter to accept a displayed default. If one of the example ports is
already occupied, select another unique port above 1024 and use it everywhere
below. The password and confirmation are intentionally not echoed.

Setup creates a complete startable board and reports its configuration,
database, schema, node, conference, file-area, and Sysop-caller results. It
does not install historical Buffalo Creek resources.

### Board directory map

The normal layout is:

```text
preview-board/
├── spitfire.toml
├── system/
│   ├── *.MNU
│   └── presentation-profiles/modern-ng/
├── work/
│   └── spitfire-ng.sqlite3
├── display/
├── message/
└── external/
    └── files/
        ├── general/WELCOME.TXT
        └── spitfire/SFNGINFO.TXT
```

- `spitfire.toml` is the static board configuration.
- `work/spitfire-ng.sqlite3` contains callers, conferences, messages, file
  catalog metadata, checkpoints, and statistics. Do not edit it manually.
- `system/` contains authoritative menus and the packaged Modern presentation
  profile. `display/` is the board-override layer.
- `external/` contains bytes that are also represented in the SQLite file
  catalog. Do not copy files into it by hand.
- `message/` is the reserved logical message path; native messages currently
  live in SQLite.
- While online, `work/runtime-status.toml` publishes transient status and
  `work/upload-staging/` may hold incomplete per-session uploads.
- The operation lock may exist beside the board as
  `.preview-board.spitfire-ng.lock`. Do not delete it; operating-system lock
  ownership, not file presence, determines whether the board is busy.
- Runtime logs currently go to the foreground terminal. There is no managed
  log directory or log rotation.

## 3. Inspect and configure the board

Inspect the setup result while it is offline:

```bash
spitfire status "$HOME/Spitfire/boards/preview-board/spitfire.toml"
```

Confirm:

- `Runtime: offline`;
- presentation mode `profile`, active/base `modern-ng 1.0.1`, and
  `Status: ready`;
- the intended board name and Sysop identity;
- the enabled listeners and their loopback ports; and
- the configured nodes.

Setup already configured identity, nodes, listeners, timezone, caller policy,
two message conferences, and two file areas. Use the supported configuration
menu to review or change them:

```bash
spitfire config "$HOME/Spitfire/boards/preview-board/spitfire.toml"
```

The menu sections are:

| Section | Current authority |
|---|---|
| 1 General | Board name, Sysop display/caller names, timezone, and public/private policy |
| 2 Nodes | Maximum simultaneous sessions owned by this board process |
| 3 Terminal Services | Listener enable state and bind address/port |
| 4 Caller Defaults | New-caller security, time limits, idle timeout, and profile-data policy |
| 5 Message Conferences | Add, edit, enable, or disable conferences |
| 6 File Areas | Add, edit, enable, or disable file areas |
| 7 Presentation Profile | Current Modern/legacy resource selection; leave `modern-ng` active/base for this guide |

Sections 1–4 and 7 stage changes until you select `S`. `Q` without `S`
discards those static edits. Conference and file-area changes commit
immediately because they live in SQLite.

For a first acceptance run, the two setup-created conferences and areas are
enough. To practice administration, add conference 3 and file area 3 through
sections 5 and 6. Use unique numbers and a unique safe storage key; file-area
numbers and storage keys cannot be changed after creation. Conference 1 must
remain enabled because Comment to Sysop uses it.

After leaving configuration, run `status` again. Static changes require a
restart; live reload is not implemented.

## 4. Start, inspect, stop, and restart

Start the listener-owning process in the foreground:

```bash
spitfire run "$HOME/Spitfire/boards/preview-board/spitfire.toml"
```

Leave that terminal open. From another terminal, inspect the running nodes:

```bash
spitfire status "$HOME/Spitfire/boards/preview-board/spitfire.toml"
```

Stop the board with Ctrl-C in the server terminal. Wait for the clean listener
shutdown message before configuring, backing up, or restoring. Start it again
with the same `spitfire run` command.

`spitfire console <CONFIG>` is an alternative foreground server that includes
operator commands. It is not a client that attaches to an existing `run`
process. Never start `run` and `console` for the same board at once.

There is no supplied daemon, background service, remote administration
service, persistent log manager, or live configuration reload. Host-service
packaging is a planned operator enhancement, not current functionality. The
documented Development Preview mode is foreground execution and clean signal
shutdown.

## 5. Connect with SyncTERM

The accepted SyncTERM client is 1.9rc4. Create a local Telnet entry with:

| Setting | Value |
|---|---|
| Connection | Telnet |
| Host | `127.0.0.1` |
| Port | The Telnet port chosen in setup, normally `2323` |
| Screen/emulation | ANSI-BBS |
| Character set | CP437 |
| Dimensions | 80 columns × 25 rows |
| Local echo | Off |

For a quick directory call, press Ctrl-D and enter:

```text
telnet://127.0.0.1:2323
```

For regular use, save a directory entry with the same connection values and
choose upload/download directories. Keep Telnet binary negotiation enabled so
file-transfer bytes are not translated.

## 6. Connect with Qodem

The accepted Qodem client is 1.0.1. Use its Phone Book; the tested direct
`--connect host:port` shortcut did not reliably select a custom port.

1. Press `I` or Insert to create an entry.
2. Set Method to `TELNET`.
3. Set Address to `127.0.0.1` and Port to the setup Telnet port.
4. Select `ANSI` emulation and `CP437`.
5. Press F10 or Alt-Enter to save.
6. Highlight the entry and press Enter to dial.

The same 80×25 and local-echo-off recommendations apply. Read
[Terminal Clients](terminal-clients.md) before trying RAW or RLogin.

## 7. Make the first calls

### Log in as the Sysop caller

At `Are you a new caller?`, answer `N`, then enter the Sysop caller name and
password created by setup. Confirm these paths:

1. Main `M` enters Messages; Message `Q` returns to Main.
2. Main `F` enters Files; File `Q` returns to Main.
3. `?` shows contextual Help and returns to the current menu.
4. Main `G` shows Goodbye and disconnects cleanly.

The Sysop is an ordinary authenticated BBS caller with the configured Sysop
security. Host-shell access alone does not create a caller session.

### Register a test caller

Reconnect and answer `Y` at the new-caller question. Enter:

1. a unique caller name;
2. a password and matching confirmation; and
3. only the profile fields enabled by the board policy.

Optional fields may be blank. `/Q` cancels at a profile prompt. A private board
does not offer public registration, so keep this first local board public until
the test caller exists.

Log off, reconnect, answer `N`, and authenticate the new caller. This proves
the caller persisted rather than existing only in one session.

## 8. Test messages

Setup creates empty General and SPITFIRE conferences. As the Sysop caller:

1. Enter Main `M`.
2. Select `E` to enter a message.
3. Press Enter at the recipient prompt for All Callers.
4. Enter a subject and at least one body line.
5. Enter a blank line, choose `S`, and confirm save with `Y`.
6. Select `R`, then This Message Conference, and read the message.
7. Select `Y` for Your Messages and open the sent presentation.
8. Return to Main with `Q`.

Reconnect as the test caller, read the public message, reply if desired, and
use Your Messages again. Public All Callers posts appear in the author's sent
list; only directly addressed mail appears as received mail.

## 9. Test files and one transfer

As either authorized caller:

1. Enter Main `F`.
2. Select `L` and confirm `WELCOME.TXT` appears in General Files with size,
   board-local date, and description.
3. Select `C`, change to SPITFIRE Files, and confirm `SFNGINFO.TXT` appears.
4. Return to General Files.
5. Select `D`, enter `WELCOME.TXT`, and choose protocol `1` (ASCII) for the
   simplest verified text download. Confirm the welcome text completes and the
   File menu returns.

To prove upload/catalog persistence without a host-side import tool:

1. Select `U` in the File menu.
2. Enter a safe new filename such as `FIRSTCALL.TXT` and a description.
3. Choose protocol `1` (ASCII).
4. Enter one or more 7-bit text lines.
5. Enter `/S` on a line by itself to finish.
6. List the area and confirm the new file, size, date, and description.

ASCII is only for small 7-bit text. XMODEM, YMODEM, ZMODEM, and their stock
variants are implemented; use [File Transfers](transfers.md) for client-side
binary transfer sequencing. Files copied into `external/` by the host are not
cataloged, visible, or backed up.

## 10. Create and prove a cold backup

Log off every caller and stop the board cleanly. Confirm it is offline:

```bash
spitfire status "$HOME/Spitfire/boards/preview-board/spitfire.toml"
```

The backup parent must exist and the backup target must not:

```bash
spitfire backup \
  "$HOME/Spitfire/boards/preview-board/spitfire.toml" \
  "$HOME/Spitfire/backups/preview-board-001"
```

The validated snapshot contains the exact configuration, consistent SQLite
state, SYSTEM and DISPLAY resources, presentation packages, catalog metadata,
and every cataloged file's bytes. It excludes runtime status, incomplete
upload staging, terminal logs, source, and research material. Protect it as
sensitive because it includes password hashes, caller profiles, and private
messages.

Restore first to a new directory:

```bash
spitfire restore \
  "$HOME/Spitfire/backups/preview-board-001" \
  "$HOME/Spitfire/boards/preview-restored"

spitfire status \
  "$HOME/Spitfire/boards/preview-restored/spitfire.toml"
```

Do not run the original and restored copies simultaneously: they contain the
same listener endpoints. With the original stopped, start the restored copy:

```bash
spitfire run "$HOME/Spitfire/boards/preview-restored/spitfire.toml"
```

Reconnect, authenticate the persisted caller, read the posted message, list
`FIRSTCALL.TXT`, and log off. Stop the restored board with Ctrl-C. This is the
practical proof that configuration, callers, messages, resources, catalog
metadata, and cataloged bytes survived.

Replacement restore is intentionally separate and destructive to changes made
after the snapshot. Use it only after reading
[Backup and Restore](backup-restore.md); it requires a stopped same-identity
board and an explicit `--replace` flag.

## Current limitations

- The published prebuilt archive is unsigned/unnotarized; no native installer
  or package repository exists.
- The documented acceptance host is Apple Silicon macOS. Linux and Windows are
  architectural targets, not a verified release matrix for 0.1.0.
- Server operation is foreground-only; no supplied service/daemon or log
  rotation exists.
- Telnet, RAW, and RLogin are unencrypted. SSH is not implemented.
- Configuration and native backup/restore are cold-board operations.
- The operator console cannot attach to an already-running `run` process.
- Files must enter through the authenticated upload path; there is no host-side
  catalog import command.
- The packaged caller presentation is Modern SPITFIRE NG. No alternative
  presentation profile is a current operator choice.

For build, port, ANSI, path, shutdown, and restore failures, continue with
[Troubleshooting](troubleshooting.md). The remaining guides in the
[Operator Documentation index](README.md) provide deeper task references
without changing this supported first-board path.
