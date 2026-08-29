# Terminal Clients

## Status

- **Verified:** SyncTERM 1.9rc4 and Qodem 1.0.1 passed the explicitly recorded
  local Telnet workflows below.
- **Development Preview:** Client support claims are limited to the named
  versions, settings, and protocol evidence; Telnet remains plaintext.
- **SSH:** macOS OpenSSH completed the caller journey. Qodem 1.0.1 external
  SSH reached Main/Messages/Files with the limits below. SyncTERM 1.9rc4 did
  not negotiate the modern SSH policy in the tested version/configuration.
- **Planned:** Physical serial/modem client acceptance is not available at
  this checkpoint.

## Baseline connection profile

For a normal local call:

| Setting | Value |
|---|---|
| Connection | Telnet |
| Host | `127.0.0.1` for the default local-only board |
| Port | `2323` unless changed during setup |
| Emulation | ANSI-BBS |
| Character set | CP437 |
| Screen | 80 columns × 25 rows |
| Local echo | Off |

The board negotiates Telnet terminal capability and dimensions. Callers can
override graphics/text, width, page length, MORE, and related preferences
under Main `U`. If presentation is corrupted, select text there and reconnect.

Telnet does not encrypt the connection. Use loopback or a trusted protected
network, or enable the separate SSH caller listener.

## SSH callers

SSH authenticates with the caller's login identifier, not the display handle
or private real name. It accepts password authentication through the ordinary
SPITFIRE credential authority and then enters the post-login BBS journey
without another caller-name/password prompt. It does not expose an operating-
system shell, SCP, SFTP, forwarding, or commands.

For macOS OpenSSH with the setup default:

```bash
ssh -p 2222 login-identifier@board.example
```

Verify the Ed25519 fingerprint with the Sysop before accepting it. A changed
fingerprint is expected only after deliberate host-key rotation or a restored
board with a different key. Do not bypass host-key checking as a routine fix.
`xterm`/`xterm-256color` defaults to ANSI and the UTF-8-oriented presentation
path; `ansi` and SyncTERM-like TERM values select ANSI/CP437 defaults. Caller
preferences still apply, and the adapter reports TERM rather than guessing a
client product.

The accepted macOS OpenSSH call authenticated, negotiated PTY dimensions,
traversed caller-profile policy and terminal preferences, entered Main,
browsed Messages, entered Files, and logged off cleanly. Qodem 1.0.1 in
documented external-SSH mode authenticated and negotiated `ansi`, CP437, and
80×23, then reached Main, Messages, and Files; the automated curses harness
ended at input EOF, so this is not a clean-Goodbye claim. SyncTERM 1.9rc4
opened TCP but did not complete the secure handshake in the tested
version/configuration. The server was not downgraded for either client.

See [Secure SSH Caller Transport](../sfng-secure-ssh-transport.md) for setup,
host keys, exact cryptographic boundary, identity migration, and diagnostics.

## SyncTERM

SyncTERM 1.9rc4 was tested on macOS over Telnet. The final guide validation
authenticated the setup-created Sysop,
rendered the ANSI menu path, traversed messages/files, posted a message, used
the sent side of Your Messages, listed files, uploaded a 47-byte ASCII file,
and logged off cleanly.

For a quick connection in SyncTERM's directory:

1. Press Ctrl-D.
2. Enter `telnet://127.0.0.1:2323` with the configured port.
3. Accept the connection.

For a saved entry, add a directory entry and set its connection type, address,
port, ANSI-BBS screen mode, and upload/download directories. Keep Telnet
binary negotiation enabled; SPITFIRE NG uses it to protect binary transfer
bytes from NVT translation.

The acceptance harness also proved the command-line URL form using SyncTERM's ANSI
output mode:

```bash
syncterm -IA -T telnet://127.0.0.1:2323
```

Ordinary macOS users should use the application UI. The command form is useful
for reproducible terminal acceptance, not a requirement for normal calls.

SyncTERM's published manual is the external authority for client UI and
version-specific options: [SyncTERM Manual](https://syncterm.bbsdev.net/Manual.html).

## Qodem

Homebrew Qodem 1.0.1 was tested in its text/curses interface. The working
entry used:

- method `TELNET`;
- address `127.0.0.1`;
- the configured custom port;
- emulation `ANSI`; and
- codepage `CP437`.

Create the entry through Qodem's Phone Book:

1. Press `I` or Insert for a new entry.
2. Set Name, Address, Port, and Method.
3. Select ANSI emulation and CP437.
4. Press F10 (or Alt-Enter) to save.
5. Highlight the entry and press Enter to dial.

In the tested 1.0.1 build, a saved phone-book entry was the reliable way to
use a non-default Telnet port. Passing `127.0.0.1:<port>` to the direct
`--connect` option opened the dialing interface instead of completing the
intended call, so this guide does not claim that shortcut.

The accepted call completed new-caller registration, message post/read and
Your Messages, file listing, ASCII download, and ASCII upload. Final guide
validation again registered a caller, read the SyncTERM-created
message, listed its uploaded file, and then authenticated the same caller and
repeated the message/file checks after new-root restore. A final restored-board
call also exercised contextual Help and a 94-byte ASCII download. Qodem's
terminal transfer shortcuts are Alt/Ctrl-PgUp for upload and Alt/Ctrl-PgDn for
download. Its ZMODEM implementation advertises auto-start, but neither
acceptance record claims a new Qodem binary-transfer pass.

## RAW TCP

RAW is an 8-bit transparent application stream, not a Telnet session. It is
useful for controlled clients and diagnostics but has no negotiation, no
encryption, and no server-side password echo control.

Restored-board persistence was verified with `nc` against the RAW listener.
That client needed exactly one LF per answer. Sending CRLF as two application
bytes introduced blank answers and exhausted login attempts. A different RAW
client may have a line-ending setting; test it before giving callers access.

RAW also does not automatically mean text-only. Listener capability and the
caller's saved graphics preference still determine `.CLR` versus `.BBS`
presentation.

## RLogin, serial, and modem

RLogin is configured by normal setup but remains plaintext. Optional
SyncTERM/Synchronet-style credential auto-login is disabled by default and
uses the ordinary password verifier if enabled. Identity supplied by the
transport alone is never authority.

Direct serial and simulated Hayes modem adapters enter the common session
engine, but no physical-hardware client run has been performed. Treat them as
implemented with synthetic acceptance, not client-verified hardware.

## Paging and input

The visible MORE prompt is:

```text
MORE: <S>top, <N>onstop, < ENTER > to continue?
```

`S` stops the current output unit, `N` disables further prompts for that unit,
and Enter continues one page. Q and Escape are documented modern Stop aliases.
Paging input is not reused as the next menu command.
