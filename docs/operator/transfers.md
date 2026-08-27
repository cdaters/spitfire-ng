# File Transfers

## Stock protocol menu

SPITFIRE NG presents the documented internal choices:

| Choice | Protocol |
|---|---|
| 1 | ASCII |
| 2 | XMODEM checksum |
| 3 | XMODEM CRC |
| 4 | 1K-XMODEM |
| 5 | YMODEM batch |
| 6 | ZMODEM batch |
| 7 | 1K-XMODEM-g |
| 8 | YMODEM-g batch |
| 9 or T | Telink |

The caller can save a supported default or choose Select Each Transfer under
Main `U`. The two `-g` variants remain selected at transfer time.

## Download sequence

1. In Files, select `D`.
2. Enter one filename or a comma-separated batch.
3. Confirm the announced filename(s), size, and protocol.
4. Select the matching receive mode in the terminal client if it does not
   auto-detect.
5. Wait for SPITFIRE NG to report completion and return to the File menu.

ZMODEM clients commonly auto-start receive. XMODEM does not carry a filename,
so the client normally asks for a destination. YMODEM/ZMODEM batch protocols
carry member metadata.

## Upload sequence

1. In Files, select `U`.
2. Enter the destination filename and description. Batch protocols may use
   names supplied by the client.
3. Select the same protocol the client will send.
4. Start the client's send operation only after SPITFIRE NG is ready.
5. Wait for the catalog-complete message, then list the area.

For Qodem terminal mode, Alt-PgUp/Ctrl-PgUp opens upload and
Alt-PgDn/Ctrl-PgDn opens download. For SyncTERM, use its transfer menu or
automatic ZMODEM receive behavior and configure upload/download directories
before the call.

## ASCII

ASCII is useful for small 7-bit text and for diagnosing a first setup. During
acceptance, Qodem 1.0.1 downloaded generated `WELCOME.TXT` and uploaded a two-line
`QODEM.TXT`; size, SHA-256, accounting, catalog listing, backup, and restored
listing all passed.

ASCII is not a general binary protocol. SPITFIRE NG preflights downloads and
rejects NUL/high-bit content before sending. ASCII uploads end with `/S` on a
line by itself or cancel with `/A`.

## Verified client boundary

- Qodem 1.0.1 passed Telnet and ASCII upload/download on a fresh board.
- SyncTERM 1.9rc4 passed Telnet login/message/file presentation but did
  not rerun binary transfer.
- Preserved project acceptance already verifies actual SyncTERM ZMODEM
  upload/download and current SyncTERM XMODEM/YMODEM variants with exact
  bytes/SHA-256 and clean menu return.
- Controlled peers cover the remaining protocol mechanics; the exact external
  breadth and batch limits remain recorded in
  [Native File Transfers](../sfng-file-transfers.md).

Do not infer that every client/version/protocol combination was retested by
the current release. Binary transfers temporarily own the application byte stream so paging,
hot keys, and menu input cannot interfere; accounting occurs only after exact
successful completion.
