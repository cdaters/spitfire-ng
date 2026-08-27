# SPITFIRE NG File Transfers

This is the canonical design, implementation, and interoperability record for
SPITFIRE NG's stock SPITFIRE 3.7 binary-transfer family. Read it with the
[file-system specification](sfng-file-system.md): that document owns catalog,
authorization, physical storage, and upload staging; this document owns bytes
on the wire and temporary binary-session ownership.

## Historical Authority and Inventory

Buffalo Creek's preserved `spitfire.doc` is the authority for caller-visible
SPITFIRE behavior. Its internal protocol list is:

1. `Ascii`;
2. `Xmodem Checksum`;
3. `Xmodem CRC`;
4. `1K-Xmodem`;
5. `Ymodem (Batch)`;
6. `Zmodem (Batch)`;
7. `1K-Xmodem-g`;
8. `Ymodem-g (Batch)`; and
9. `Telink`.

The saved-default menu exposes 1–6 plus `T`/`S` (Telink/select at transfer
time); the live stock transfer prompt can expose the complete internal list.
SPITFIRE records successful upload/download file and byte totals. Its count and
kilobyte ratio tests, daily limits, and upload-time credit remain separate
policy work; they are not invented inside protocol engines.

Wire behavior was independently implemented from published protocol
references, principally the [Synchronet XMODEM reference](https://wiki.synchro.net/ref:xmodem),
[YMODEM reference](https://wiki.synchro.net/ref:ymodem),
[ZMODEM reference](https://wiki.synchro.net/ref:zmodem), and FidoNet
FTS-0007 Telink specification. Synchronet source was comparative GPL research,
not copied implementation.

## Architecture and Safety Boundary

The transfer path is deliberately layered:

```text
authenticated File command
        -> FileBackend authorization and verified FileStorage
        -> protocol-neutral transfer request
        -> stock protocol engine
        -> Terminal binary application-byte stream
        -> Telnet / RAW / RLogin / serial / modem peer
```

`Terminal::begin_binary_mode`, `read_binary`, `write_binary`, and
`end_binary_mode` grant temporary stream ownership. During that interval the
normal line reader, menu prompts, CP437/ANSI rendering, MORE/abort processing,
and page/chat output do not touch the wire. End or failure returns ownership
through one cleanup path; protocol residue is not intentionally queued as menu
input.

Every parser uses safe Rust, bounded block/frame sizes, bounded retries and
timeouts, checked sizes, and validated remote basenames. Protocol code cannot
authorize a file, resolve a host path, commit a catalog row, or award caller
statistics.

## Transport Binary Behavior

- **Telnet:** both directions must negotiate Telnet BINARY. Application `IAC`
  (`0xFF`) is doubled on output and collapsed on input while option and
  subnegotiation framing remains active. Tests cover all 256 byte values,
  including CR/LF and `IAC`.
- **RAW TCP:** the application stream is byte transparent.
- **RLogin:** only the initial NUL-delimited handshake is parsed. Subsequent
  binary bytes are not reinterpreted and auto-login fields never enter the
  transfer stream.
- **Direct serial:** the protocol uses the established serial adapter and its
  configured framing. PTY coverage is synthetic; physical hardware remains
  unverified.
- **Hayes modem:** AT/RING/ATA/CONNECT remains above serial. After CONNECT the
  same binary stream is used; the deterministic modem peer verifies that
  transition. No transfer logic is in the modem controller.
- **Shell/stdio:** the contract exists, but local TTY file transfer is not an
  acceptance path. **SSH remains deferred.**

## Protocol Implementations

### XMODEM family

Checksum and CRC modes implement receiver-driven initiation (`NAK` or `C`),
SOH 128-byte blocks, sequence/complement validation, checksum or CRC-16/XMODEM
polynomial `0x1021`, duplicate-block acknowledgment, bounded retries, EOT, and
eight-CAN cancellation. A CRC receiver falls back to checksum after repeated
unanswered `C` requests; a CRC sender honors an explicit checksum fallback.

`1K-Xmodem` adds STX 1024-byte CRC blocks and accepts a valid SOH/STX mixture.
`1K-Xmodem-g` uses `G` initiation and streaming without per-block ACK; detected
corruption aborts instead of requesting retransmission. Plain XMODEM has no
standard exact-length metadata, so a receiver may retain terminal CP/M `0x1A`
padding. SPITFIRE NG does not fabricate a length field.

### YMODEM family

YMODEM is not treated as a synonym for 1K-XMODEM. It implements block zero
filename, exact decimal length, optional octal modification time, CRC-16,
128/1024-byte data blocks, the two-EOT handshake, multiple files, and the empty
block-zero batch terminator. Declared length trims padding exactly.

YMODEM-g uses `G`, streamed blocks, file-boundary synchronization, and abort on
error. Batch upload safety is **per-file transactional**: each completed file
gets its own staging object and catalog commit; a later batch failure does not
make an earlier committed file partial or silently delete it.

### ZMODEM

The implementation integrates `zmodem2` 0.7.2 (MIT OR Apache-2.0) behind the
project's Terminal contract. It supports sender and receiver roles, ZRQINIT /
ZRINIT, ZFILE, ZRPOS, ZDATA, ZEOF, ZFIN/final `OO`, ZSKIP/cancellation paths,
hex and binary headers, CRC-16 and negotiated CRC-32, ZDLE link escaping,
streaming subpackets, and multiple files. ZRPOS is honored for protocol error
repositioning; durable resume across reconnects is not claimed.

The engine consumes the final `OO` before returning to Files. This was
specifically corrected during real-client testing so terminator bytes cannot
become menu input.

### Telink

Telink implements the FTS-0007 descriptor block (`SYN`, block zero and
complement), little-endian length, DOS date/time slots, bounded filename and
sender fields, checksum/CRC indicator, XMODEM-like data blocks, EOT, and
cancellation. Remote pathnames remain untrusted basenames. Verification is a
specification-derived independent peer test; SyncTERM does not expose Telink.

## Upload Staging and Accounting

Every received file, including batch members, follows the existing path:

```text
bounded protocol receive -> per-session staging -> basename/size/security
revalidation -> SHA-256 -> duplicate check -> exclusive final creation ->
catalog transaction -> success-only statistics -> staging removal
```

Timeout, CRC/checksum error, cancellation, disconnect, unsafe name, duplicate,
or commit failure cannot award success statistics. No protocol writes directly
to `EXTERNAL/files`. Downloads reauthorize and revalidate catalog size and
SHA-256 before transmission, then award statistics only on protocol success.

## Caller Selection and Preference

The File menu uses the exact stock protocol names and complete 1–9/T transfer
menu. Caller terminal preferences now persist the stock saved choices 1–6,
Telink, or `Select each transfer`; the two `-g` choices remain selectable at
transfer time, matching the distinction in the manual. Schema migration 6
adds only this checked preference column.

## SyncTERM Interoperability

Two real external-client builds were exercised on macOS over Telnet against a
four-node board created with normal `spitfire setup`:

- the installed **SyncTERM 1.9rc4** universal x86-64/arm64 application; and
- **SyncTERM 1.10a**, built without local source changes from upstream commit
  `dc5eb88e3852dfa673c7c72ab5df955b89a21dbc` (arm64).

The current client exposes ZMODEM, YMODEM, YMODEM batch, XMODEM-1K,
XMODEM-128, ASCII, and Raw in its normal picker. Its transfer API also exposes
the separately tested XMODEM checksum, XMODEM CRC, and YMODEM-g modes. A small
temporary Wren key binding invoked those public client transfer operations so
the protocol implementation itself remained the unmodified SyncTERM code.

Formal ZMODEM acceptance used a synthetic 4096-byte file with SHA-256
`ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7`:

| Scenario | Result |
|---|---|
| SyncTERM 1.9rc4 ZMODEM upload | **PASS** — CRC-32 displayed; staging, catalog, exact size/bytes/hash, statistics, and menu return passed |
| SyncTERM 1.9rc4 ZMODEM download | **PASS** — CRC-32 displayed; exact size/bytes/hash, statistics, and menu return passed |
| SyncTERM 1.10a XMODEM-128 upload to checksum receiver | **PASS** — client reported receiver-requested 8-bit checksum; exact bytes/hash and commit passed |
| SyncTERM 1.10a XMODEM-128 upload to CRC receiver | **PASS** — client reported receiver-requested 16-bit CRC; exact bytes/hash and commit passed |
| SyncTERM 1.10a XMODEM checksum download | **PASS** — client explicitly requested 8-bit checksum; exact bytes/hash and menu return passed |
| SyncTERM 1.10a XMODEM CRC download | **PASS** — client explicitly requested 16-bit CRC; exact bytes/hash and menu return passed |
| SyncTERM 1.10a XMODEM-1K upload | **PASS** — four 1K CRC blocks, exact bytes/hash, staging/catalog/statistics passed |
| SyncTERM 1.10a YMODEM upload/download | **PASS** — block-zero length preserved exact 4096-byte output in both directions |
| SyncTERM 1.10a YMODEM batch upload/download | **PASS** — two files in each direction; per-file bytes, hashes, catalog, statistics, and menu return passed |
| SyncTERM 1.10a YMODEM-g download | **PASS** — actual `G` streaming/no-block-ACK behavior, exact bytes/hash, termination, and menu return passed |
| SyncTERM ZMODEM batch | Controlled-peer batch **PASS**; current client auto-ZRINIT path opens its single-file picker, so a real multi-file run is narrowly blocked by client UI/dispatch behavior |

XMODEM-128 does not imply one integrity mode: the observed sender honored the
receiver's request, and both checksum and CRC negotiation passed. SyncTERM's
YMODEM-g behavior also established a subtle interoperability fact: it responds
to metadata block zero with the next `G`, not `ACK` plus `G`, and does not ACK
the empty batch terminator. The native state machine now models that behavior.

## Protocol Status Matrix

| Stock protocol | Implementation | Verification |
|---|---|---|
| ASCII | VERIFIED | Existing end-to-end Telnet acceptance |
| XMODEM checksum | VERIFIED | Actual SyncTERM 1.10a send/receive plus deterministic corruption/retry/cancel peer |
| XMODEM CRC | VERIFIED | Actual SyncTERM 1.10a send/receive with `C`, plus corruption and checksum-fallback peer |
| 1K-XMODEM | VERIFIED | Actual SyncTERM 1.10a upload plus deterministic STX/mixed-block peer |
| 1K-XMODEM-g | IMPLEMENTED | Deterministic `G` streaming peer; external client pending |
| YMODEM batch | VERIFIED | Actual SyncTERM 1.10a single and two-file upload/download plus deterministic peer |
| YMODEM-g batch | VERIFIED for single-file SyncTERM download; IMPLEMENTED for batch/upload | Actual SyncTERM 1.10a `G` streaming plus deterministic bidirectional batch peer |
| ZMODEM batch | VERIFIED for single-file SyncTERM upload/download; IMPLEMENTED for batch | Actual SyncTERM 1.9rc4 CRC-32 plus deterministic batch peer |
| Telink | IMPLEMENTED | FTS-0007-derived peer; external client pending |

## Known Gaps and Next Verification

- Perform a real multi-file ZMODEM batch run when a client path can select
  multiple files after SPITFIRE NG's automatic ZRINIT handshake. SyncTERM
  1.10a exposes a batch picker, but its auto-detection dispatch currently
  selects the single-file upload operation.
- Add a second external Telink/1K-XMODEM-g peer and external YMODEM-g upload/
  batch evidence where compatible clients can be identified and run safely;
  physical serial/modem hardware also remains unverified.
- Stock ratio/daily-limit/upload-credit policy, persistent interrupted-transfer
  resume, file tagging queues, and complete operator progress percentages are
  separate parity work.

The intentionally promoted transfer milestone does not reclassify these
protocols from Category B. Development priority and historical parity class are
separate. After this milestone, the queued Category-A resource/menu fidelity
closure remains the expected next SPITFIRE NG action unless a fresh parity
audit identifies a more serious blocker.
