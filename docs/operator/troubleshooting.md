# Troubleshooting

## macOS blocks the downloaded executable

Verify the archive and internal manifest first, then use the bounded per-
program **Open Anyway** procedure in [macOS First Run](macos-first-run.md).
Do not disable Gatekeeper/SIP or use a blanket quarantine-removal command. If
macOS reports damage or malware rather than an unidentified developer, stop
and obtain a fresh official copy.

## `spitfire: command not found`

Run the installed binary by full path or add Cargo's binary directory to
`PATH`. Verify with `spitfire --version`. See [Installation](installation.md).

## Cargo cannot reach crates.io

Fix the host's DNS/proxy/network access. If every locked dependency is already
cached, retry with `--offline`; it cannot retrieve a missing crate.

## Setup cannot read the password

Run setup directly from a real interactive terminal. Do not pipe answers or
launch it without a controlling TTY. The password is intentionally read with
echo disabled. Setup validates before creating the board, so an early password
prompt failure should not leave a claimed board.

## Setup says the target exists

Choose a new path. `setup` never overwrites a directory. Do not delete an
existing board until its identity, backup, and recovery value are understood.

## Listener bind fails

Common causes are:

- another process already uses the address/port;
- the address is not assigned to this host;
- a privileged port was selected; or
- host sandbox/firewall policy denies listening.

Stop the board, use `spitfire config` section 3, select a unique nonprivileged
endpoint, choose `S`, and restart.

## Another computer cannot connect

Normal setup binds `127.0.0.1`, which is reachable only on the server itself.
Changing to a LAN/interface address is possible but exposes plaintext Telnet,
RAW, or RLogin. Review the firewall/VPN/security boundary before doing so.

## `status` says running after a crash

The runtime snapshot may be stale. Check whether the actual process exists
and whether a new `run` can acquire the board lock. A clean start/stop rewrites
and removes status normally. Do not remove the board-operation lock file as a
guess; operating-system lock ownership is authoritative.

## Configuration changes disappeared

Sections 1–4 require `S` before `Q`. Conference and file-area edits are
immediate. Reopen `spitfire config` and verify the intended state.

## Configuration/backup says the board is busy

Stop `run`, `console`, and `shell`, and make sure another config/backup/restore
operation is not active. These operations intentionally fail closed instead
of racing board state.

## ANSI art or box characters are wrong

Configure ANSI-BBS, CP437, and 80×25 in the client. Under Main `U`, choose ANSI
or Auto only when the client supports it; choose Text to force `.BBS`
resources. Reconnect after changing a persistent preference.

For custom art that shows UTF-8 mojibake, trailing SAUCE metadata, bare-LF
macro substitution, or ANSI in a BBS fallback, use the byte checks in
[Customizing Display Screens](custom-display-screens.md#troubleshooting).

## The terminal stops at MORE

Press Enter to continue a page, `N` for nonstop within the current display, or
`S` to stop the current display. Q/Escape are modern Stop aliases.

## Qodem does not dial a custom port

Create a Phone Book entry with Method TELNET, Address, Port, ANSI, and CP437.
That path passed with Qodem 1.0.1. The tested direct `--connect
host:port` attempt did not.

## Qodem says the terminal is too small

Qodem 1.0.1 requires at least 80 columns by 25 rows for its dialogs. Resize the
host terminal to 80×25 or larger before opening the Phone Book. The local-guide
revalidation reproduced this warning in an 80×24 automation terminal; the
documented 80×25 client geometry worked.

## RAW login consumes blank answers

RAW has no Telnet line-ending negotiation. Configure the client to send one
line terminator. The `nc` diagnostic passed with LF-only and failed when
CRLF arrived as two input bytes. Prefer an actual Telnet client for ordinary
callers.

## Password appears on a RAW client

RAW provides no server-side echo negotiation; local client echo may display
what you type. Do not use RAW over an untrusted path. Telnet suppresses
server-side password echo but still does not encrypt the network traffic.

## A host-copied file is not listed or backed up

File bytes are not authoritative without SQLite catalog metadata. Upload the
file through the authenticated File menu. Do not edit SQLite or place files
directly into managed storage.

## Backup or restore refuses a path

The destination parent must exist; the destination itself must not. Backups
must be outside the board and managed logical paths. Replacement requires
`--replace` and matching board/Sysop identity. Hash, manifest, schema, catalog,
or confinement failures must be investigated, never bypassed.

## No opening messages exist

That is normal for a setup-created board. Log in as the Sysop caller and post
an All Callers announcement in Conference 1.

## I cannot attach the console to `run`

That feature does not exist. Stop `run` and start `spitfire console` instead.
The console owns the listeners and node pool in the same process.

## I need to report a bug

Follow [Development Preview Support](support.md). Include sanitized version,
status, terminal, profile, and reproduction details; never post a real board,
password, private caller data, or a security exploit publicly.
