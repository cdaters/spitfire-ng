# Quick Start: Run and Call Your First Board

<!-- help-topic: sysop.quick-start -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 18)
>
> **Latest downloadable release:** SPITFIRE NG 0.1.0 Development Preview
>
> This page uses features from the current source tree. The older downloadable
> Development Preview does not include every current feature.

Want to get the board running first and read the full manual later? This guide
takes the shortest safe route: build the current source, create a local board,
connect through SSH as the setup-created Sysop, post one message, download one
starter text file, log off, and make the first backup.

The example listens only on your own computer. Keep it that way until you have
read the security and transport guidance.

## 1. What you need

You need:

- Git;
- a current stable Rust toolchain with Cargo;
- the normal compiler/linker tools required by Rust on your operating system;
- an interactive terminal for setup; and
- an SSH terminal client. The verified example below uses macOS OpenSSH.

SQLite is built into SPITFIRE NG. You do not need to install or run a database
server.

## 2. Get and build current source

If you do not already have the public source checkout:

```bash
git clone https://github.com/cdaters/spitfire-ng.git
cd spitfire-ng
git switch main
```

Build the BBS from the locked dependency set:

```bash
cargo build --release --locked -p sf-bbs
./target/release/spitfire --version
```

The current package version prints:

```text
SPITFIRE NG Bulletin Board System 0.1.0
```

The package version is still 0.1.0 even though current source is ahead of the
downloadable Development Preview. The source/release notice at the top of this
page is therefore important.

## 3. Choose the board and backup directories

Create parent directories, but do not create the board or backup target
itself. Setup and backup refuse to overwrite an existing target.

```bash
mkdir -p "$HOME/Spitfire/boards" "$HOME/Spitfire/backups"
```

This guide uses:

```text
$HOME/Spitfire/boards/my-board
$HOME/Spitfire/backups/my-board-001
```

## 4. Run first-time setup

From the repository root:

```bash
./target/release/spitfire setup "$HOME/Spitfire/boards/my-board"
```

Setup is interactive. Use these choices for the local first call:

| Prompt | Safe first-board choice |
| --- | --- |
| Board name | A caller-visible name for your board |
| Sysop display name | The name callers should see for the Sysop |
| Sysop caller name | `sysop` for this example |
| Number of nodes | `1` |
| Board timezone | Your board's IANA timezone, such as `America/Phoenix` |
| Board default locale | `en-US` |
| Board access | `public` for a later local new-caller test |
| Enable Telnet | `no` |
| Enable RAW TCP | `no` |
| Enable RLogin | `no` |
| Enable SSH caller access | `yes` |
| SSH bind address | `127.0.0.1` |
| SSH port | `2222`, or another unused port above 1024 |
| Caller experience | Accept `modern` |
| Active/base profile | Accept `modern-ng` for both |
| Menu presentation | Accept `display-overrides` |
| Post-login journey | Accept `none` |
| Caller security and time limits | Accept the displayed defaults |
| Address, phone, email, and birth-date policy | Accept `disabled` for this first board |
| Initial Sysop password | A unique password between 10 and 128 bytes |

The password and confirmation are not echoed. Setup requires a real
interactive terminal so it can read them safely.

Successful setup reports:

- the configuration and database paths;
- schema version 18;
- one configured node;
- two starter message conferences;
- two starter file areas; and
- the new Sysop caller name.

Setup also installs project-authored menus, presentation resources, language
resources, and two small starter files. It does not install proprietary
historical SPITFIRE files.

## 5. Check the stopped board

```bash
./target/release/spitfire status \
  "$HOME/Spitfire/boards/my-board/spitfire.toml"
```

Before the first start, confirm:

- the board name and Sysop name are correct;
- `Runtime` says `offline`;
- the effective language and presentation say `READY` or `ready`;
- Telnet, RAW, and RLogin are disabled;
- SSH is enabled on `127.0.0.1:2222`; and
- the SSH fingerprint says `not generated`.

The board creates its SSH host key only when the enabled SSH listener starts
for the first time.

## 6. Start the board

In the first terminal:

```bash
./target/release/spitfire run \
  "$HOME/Spitfire/boards/my-board/spitfire.toml"
```

Leave that terminal open. SPITFIRE NG currently runs in the foreground; it
does not install a background service.

In a second terminal, run `status` again:

```bash
./target/release/spitfire status \
  "$HOME/Spitfire/boards/my-board/spitfire.toml"
```

Record the reported SSH Ed25519 fingerprint. You will compare it with the
fingerprint shown by your SSH client on first connection.

## 7. Make the first call with SSH

Using the default example port:

```bash
ssh -p 2222 sysop@127.0.0.1
```

On the first call, OpenSSH asks whether to trust the board's host key. Compare
the displayed SHA-256 fingerprint with `spitfire status`. Accept it only when
they match. Then enter the Sysop password created during setup.

SSH authenticates the existing caller and takes you directly into SPITFIRE
NG. It does not ask for the BBS password a second time. It also does not offer
an operating-system shell, command execution, SCP, SFTP, or forwarding.

You should see the welcome text followed by the Main menu.

## 8. Post the first message

Commands may be entered as a letter followed by Enter.

1. At the Main menu, enter `M` for Messages.
2. Enter `E` to enter a new message.
3. Press Enter at the recipient prompt to choose All Callers.
4. Enter a subject, such as `Welcome to the board`.
5. Enter one or more message lines.
6. Enter a blank line to open the editor commands.
7. Enter `S`, then confirm with `Y`.

SPITFIRE NG reports the new message number and returns to the Message menu.
The setup-created General conference is enough for this first post; you do not
need to create a conference first.

## 9. List and download the starter file

1. From the Message menu, enter `F` for Files.
2. Enter `L` to list the current file area.
3. Confirm that `WELCOME.TXT` is listed, then press Enter to return.
4. Enter `D` to download.
5. Enter `WELCOME.TXT` at the queue prompt.
6. Choose protocol `1` for ASCII.

The welcome text appears in the terminal, SPITFIRE NG reports completion, and
the File menu returns. ASCII is suitable here because the starter file is
small seven-bit text. Use a client with a matching XMODEM, YMODEM, ZMODEM, or
TeLink implementation for normal binary files.

## 10. Log off and stop cleanly

1. Enter `Q` to return from Files to the Main menu.
2. Enter `G` for Goodbye.
3. Wait for the SSH connection to close.
4. In the server terminal, press Ctrl-C once.
5. Wait for `SPITFIRE NG listeners shut down cleanly`.

Check that the board is stopped:

```bash
./target/release/spitfire status \
  "$HOME/Spitfire/boards/my-board/spitfire.toml"
```

`Runtime` should say `offline`. Configuration and backup are cold-board
operations; do not run them while the board owns its listeners.

## 11. Create the first backup

The backup parent already exists from step 3. The target must not exist:

```bash
./target/release/spitfire backup \
  "$HOME/Spitfire/boards/my-board/spitfire.toml" \
  "$HOME/Spitfire/backups/my-board-001"
```

The command validates the board before publishing the backup. A successful
summary includes schema 18, the resource count, the cataloged file count, and
the number of verified bytes.

Protect the entire backup directory. It contains password hashes, caller and
message data, configuration, cataloged file bytes, and the board's SSH host
key. Do not edit files inside the snapshot.

Read [Backup and Restore](../operator/backup-restore.md) before restoring or
replacing a board.

## Where do I go next?

- **Review every setting:** [Configuration](../operator/configuration.md)
- **Add or manage callers:** [Caller Management](../operator/caller-management.md)
- **Configure messages:** [Messages](../operator/messages.md)
- **Configure file areas:** [Files](../operator/files.md)
- **Use binary transfer protocols and batches:** [File Transfers](../operator/transfers.md)
- **Enable Telnet, RAW, or RLogin deliberately:** [Terminal Clients](../operator/terminal-clients.md)
- **Configure and protect SSH:** [Secure SSH Caller Transport](../sfng-secure-ssh-transport.md)
- **Add nodes:** [Multinode Runtime](../sfng-multinode-runtime.md)
- **Customize displays:** [Custom Display Screens](../operator/custom-display-screens.md)
- **Install a language package:** [Language Packages](../operator/localization.md)
- **Restore or replace a board:** [Backup and Restore](../operator/backup-restore.md)
- **Solve a problem:** [Troubleshooting](../operator/troubleshooting.md)
- **Understand implementation details:** [Technical Reference](../technical/README.md)

## Current operational limits

- Current source and the older downloadable Development Preview both report
  package version 0.1.0. Confirm the source checkpoint when the distinction
  matters.
- The setup wizard is interactive and asks every core first-board question; it
  does not yet offer a shorter preset-only mode.
- The board runs in the foreground. Service installation and managed log
  rotation are not supplied.
- SSH admits an existing caller such as the setup-created Sysop. To offer
  self-registration, configure an appropriate caller path and understand the
  security of its transport first.
- Static configuration and cold backup require a stopped board.

This exact source-build, schema-18 setup, loopback OpenSSH call, message post,
starter-file ASCII download, clean shutdown, and cold-backup path was exercised
on a clean board on 2026-09-03.
