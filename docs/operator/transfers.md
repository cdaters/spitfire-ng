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
| 9 or T | TeLink |

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
M029, Qodem 1.0.1 downloaded generated `WELCOME.TXT` and uploaded a two-line
`QODEM.TXT`; size, SHA-256, accounting, catalog listing, backup, and restored
listing all passed.

ASCII is not a general binary protocol. SPITFIRE NG preflights downloads and
rejects NUL/high-bit content before sending. ASCII uploads end with `/S` on a
line by itself or cancel with `/A`.

## Verified client boundary

- M029 verified Qodem 1.0.1 Telnet and ASCII upload/download on a fresh board.
- M029 verified SyncTERM 1.9rc4 Telnet login/message/file presentation but did
  not rerun binary transfer.
- Preserved project acceptance already verifies actual SyncTERM ZMODEM
  upload/download and current SyncTERM XMODEM/YMODEM variants with exact
  bytes/SHA-256 and clean menu return.
- Independent original and modern peers close B-024 across all nine required
  choices; client-specific support and exact historical prompt/timing limits
  remain recorded in
  [Native File Transfers](../sfng-file-transfers.md).

Do not infer that every client/version/protocol combination was retested by
M029. Binary transfers temporarily own the application byte stream so paging,
hot keys, and menu input cannot interfere; accounting occurs only after exact
successful completion.

## Tranche 6 administration boundary

The [Tranche 6 gate](../research/m039-tranche-6-batch-transfer-policy-extended-storage-gate.md)
defines implemented typed daemon/domain commands for protocol policy, active transfers,
daily usage/reservations, and logical storage roots. While the board is live,
no operator utility may edit SQLite, counters, `DAILYLMT.DAT`, `FA<x>.TXT`,
root paths, or transfer staging directly. Queue/start/cancel and settlement are
online operations; versioned policy/root changes use daemon authority;
relocation/deep repair requires maintenance; migration/restore is offline.

The queue is bounded session state, not a durable operator queue.
TeLink is one of B-024's required choices and remains a dedicated engine even
though no suitable library was available. Public interoperability evidence
proves both directions against an independent original peer.

See the [Tranche 6 implementation report](../research/m039-tranche-6-batch-transfer-policy-extended-storage-implementation.md)
and [verification record](../research/m039-tranche-6-verification.md) for
schema-16 transfer behavior, schema-17 zero-byte authority, and the completed
B-011/B-014/B-023 acceptance matrices. External-root rebind is a versioned
Sysop-only mutation that returns observed availability to Unknown; a separate
safe probe must precede the daemon's availability update. The current operator
surface is domain/CLI-testable; `sfconfig` and `sfmonitor` remain unimplemented
future clients of the same authority.
