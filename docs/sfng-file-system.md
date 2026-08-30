# Native SPITFIRE NG File System

This is the canonical implementation and compatibility specification for the
native file-area subsystem introduced by Stock SPITFIRE 3.7 Core Parity
Increment 5. Read it before changing file metadata, storage paths, file-area
authorization, transfers, uploads, or transfer accounting.

The caller-visible authority is Buffalo Creek Software's preserved SPITFIRE
3.7 manual (`research/samples/shareware-software/sf37-2/spitfire.doc`). The
manual is a read-only, ignored research input. The durable parity status is in
[the stock checklist](stock-spitfire-3.7-parity.md). Synchronet was used only
as a secondary engineering reference; see
[the corpus note](research/synchronet-reference.md).

## Increment 5 Boundary

Increment 5 makes Files a working subsystem rather than a menu placeholder. A
caller can:

- list accessible numbered file areas and change the current area;
- list stock-oriented file rows with filename, comma-grouped byte size,
  board-local `MM-DD-YY` date, and safe multiline description;
- search permitted areas by filename wildcard or one-to-six description
  words;
- list new files in one accessible area or all accessible areas from either a
  caller-entered date or the caller's dedicated last-files-checked checkpoint;
- download verified 7-bit text through SPITFIRE's documented ASCII protocol;
- upload a `.TXT` file through a bounded, cancelable ASCII line transfer into
  per-session staging; and
- reconnect and see persisted catalog entries and successful-transfer
  statistics.

Setup creates two original, non-proprietary starter areas, `General Files` and
`SPITFIRE Files`, with small generated text files. The interactive
configuration command can list, create, edit, and safely disable areas.

The binary-transfer increment implements the complete documented internal
protocol family without changing this storage boundary. Actual SyncTERM
XMODEM checksum/CRC, XMODEM-1K, YMODEM single/batch, YMODEM-g download, and
ZMODEM upload/download interoperability is verified; the remaining external-
client variants, exact ratio policy, archive inspection, and complete
duplicate heuristics remain tracked follow-up work. See
[SPITFIRE NG File Transfers](sfng-file-transfers.md).

## Stock SPITFIRE 3.7 Behavior Established

The manual documents up to 65,535 numbered file areas. An area has a caller-
visible description, upload and download locations, security policy, optional
preview access, up to five privileged security levels, CD-ROM behavior,
duplicate-search policy, and a no-charge/free setting. Threshold and exact
security comparisons are documented.

The stock File menu provides area change, listing/tagging, download, upload,
new-file listing, filename find, description text search, ZIP view, text-file
read, and privileged maintenance. Filename find supports wildcards, adds `.*`
when an extension is omitted, and rejects the unhelpful `*.*` query.
Description search accepts up to six words.

SPITFIRE 3.7 documents these internal protocols by number:

1. ASCII;
2. XMODEM checksum;
3. XMODEM CRC;
4. 1K-XMODEM;
5. YMODEM batch;
6. ZMODEM batch;
7. 1K-XMODEM-g;
8. YMODEM-g batch; and
9. Telink.

The manual also supports configured external transfer drivers. Those are an
extension mechanism, not the native Increment 5 path. ASCII historically
refuses common binary/archive/executable types. SPITFIRE records upload and
download counts/bytes, supports file and kilobyte ratios through
`DAILYLMT.DAT`, and has upload-time compensation and disk-space policy.

Historical file listings use `SFFILES.BBS`, whose file rows begin the filename
at column 1, end size at column 21, end date at column 31, and begin the brief
description at column 34. Description continuation lines are caller-visible.
The manual classifies `SFFILES.BBS` as BBS/ASCII-only even for ANSI or RIP
callers: SPITFIRE colorizes that listing dynamically, using the comma position
in the formatted file size as part of the coloring rule. `SFFILESA` confirms
that each file-area list carries filename, comma-grouped size, date, and up to
ten 45-character description lines, with create/append, sort, size/date repair,
and missing-file review workflows. These are historical requirements for the
future enhanced file-display/legacy-listing gate, not behavior added by M043.
Numbered CD-ROM listing files and extended directory resources such as
`FA<n>.TXT` remain legacy adapter concerns. Companion tools can extract
`FILE_ID.DIZ` descriptions from archives; neither import nor archive handling
is part of the native presentation closure.

## Domain Model

The narrow domain boundary lives in
[`file.rs`](../crates/sf-core/src/file.rs). Its current concepts are:

- `FileAreaId` and `FileId`: stable internal positive identifiers;
- `FileArea`: caller-visible number/name/description plus storage and policy;
- `FileAccessMode`: stock threshold (`AtLeast`) or exact-level comparison;
- `FileAccess`: full use or preview/list-only use;
- `FileEntry`: safe filename, description, size, SHA-256, upload attribution,
  time, state, and completed-download count;
- `FileStatistics`: caller-authorized new/available counts and downloadable
  bytes for the stock new-file presentation;
- `FileActor`: authenticated caller identity plus configured Sysop threshold;
- `FileSearch`: filename wildcard, description words, or new-since time; and
- `FileBackend`: the caller-facing storage/authorization operations exercised
  by the File menu, including the caller's private new-file checkpoint.

Caller-facing area numbers are not SQLite row IDs. Stable internal identity
survives edits. Area renumbering and storage relocation fail closed until a
dedicated, audited move workflow exists.

## Storage and Authority

Three state classes remain deliberately separate:

| State | Authority | Location |
|---|---|---|
| Area/file metadata, hashes, attribution, counters | SQLite operational database | logical `WORK` database |
| Final file bytes | Host filesystem through `FileStorage` | logical `EXTERNAL/files/<storage-key>/` |
| In-progress uploads | Ephemeral per-session staging | logical `WORK/upload-staging/session-<id>/` |

SQLite blobs are not the file store. Session/menu code never constructs a host
path. `FileStorage` receives already validated area and filename identities,
confines them to resolved logical roots, and rejects symlinks/non-files and
canonical-path escapes.

Safe caller filenames are one ASCII basename of at most 64 bytes using
letters, digits, `.`, `_`, `-`, or `+`; leading-dot names, absolute paths,
separators, `.`/`..`, and traversal are rejected. Area storage keys are one
validated alphanumeric/underscore/hyphen component. Final creation uses
exclusive `create_new` semantics, so concurrent uploads cannot overwrite one
another.

## SQLite Schema Versions 4 and 9

Migration 4 upgrades an existing schema-3 caller/message board without reset.
It adds:

- `file_areas` with stable identity, unique number/storage key, access mode,
  read/upload security, preview/no-charge flags, maximum upload size, and
  active state;
- `file_area_privileged_security`, limited by the domain validator to the five
  levels documented by SPITFIRE;
- `files` with an area foreign key, normalized per-area filename uniqueness,
  description, size, SHA-256, upload attribution/time, download count, and
  availability state; and
- caller counters for completed upload/download files and bytes.

Foreign keys use restrictive deletion for catalog/history safety. Normal
listing and upload-time scans are indexed. Metadata updates and statistics use
parameterized SQL and transactions. Migration tests cover empty databases and
upgrades from schemas 1, 2, and 3, including preserved callers and messages.

Schema 9 adds one nullable, nonnegative `callers.last_files_checked_at`
timestamp. `NULL` means the caller has never completed a new-file scan. The
8→9 migration does not rewrite callers, credentials, profiles, messages,
queues, receipts, file areas, catalog entries, transfer statistics, or stored
file bytes. Checkpoint updates are monotonic so concurrent or stale sessions
cannot move a caller's checkpoint backward.

## Authorization

Authorization is enforced inside the backend on every list, search, select,
download, upload commit, and accounting operation. Menu visibility is not an
authority boundary.

- Threshold areas allow the configured security level or higher.
- Exact areas require the exact configured level.
- The configured Sysop threshold and up to five privileged levels override
  ordinary comparison.
- Preview permits listing/searching but not download or upload.
- Disabled areas disappear from caller operations without deleting metadata
  or bytes.
- Search enumerates already authorized areas; it cannot reveal restricted
  filenames or descriptions.
- Upload commit reloads current caller/area state so a disabled account or
  concurrently disabled area cannot rely on stale menu state.

## Listing, Search, and New Files

Listings are terminal-height aware and pause between bounded pages. Native
8.3-compatible names use the documented `SFFILES.BBS` columns: filename at 1,
comma-grouped size ending at 21, board-local `MM-DD-YY` date ending at 31, and
description beginning at 34. Safe CRLF/LF description continuations are
indented to the same description column and wrap to the effective terminal
width. Longer modern filenames remain visible on a preceding line instead of
being truncated. Stored escape/control bytes and bare carriage returns remain
invalid metadata.

Filename matching is case-insensitive ASCII with `*` and `?`. Missing
extensions receive `.*`, matching the documented stock behavior; `*.*` is
rejected. Description search requires every one of one-to-six normalized
words. Both searches span only accessible areas.

The New Files command first accepts all accessible areas, the current area, or
one accessible area number. It then accepts the caller's last completed check
or a real `MM-DD-YY`/`MM-DD-YYYY` board-local date. Two-digit operational input
means 2000..2099, consistent with the established SPITFIRE 3.7 operational-date
policy; display remains the historical two-digit form. The result reports new
files since the dedicated checkpoint plus total caller-downloadable files and
bytes. Preview and restricted areas cannot inflate downloadable statistics.
A completed result, including an empty result, advances the checkpoint to the
scan start; cancellation, invalid input, backend failure, or paging abort does
not. New-file selection state remains local to the session, while the
checkpoint is deliberately durable per caller.

## Transfer Boundary and Protocols

`FileTransfer` separates a requested transfer from file storage and from
Telnet/raw/RLogin/serial/modem adapters. `AsciiTransfer` remains the
line-oriented stock path. The binary `TransferProtocol` engines temporarily
own the Terminal application-byte stream and implement XMODEM checksum/CRC,
1K-XMODEM/g, YMODEM/g batch, ZMODEM batch, and Telink. Wire state and transport
framing remain outside the file catalog/storage layer.

Before an ASCII download, `FileStorage`:

1. reloads and authorizes the catalog record;
2. rejects unsafe filesystem objects and canonical escapes;
3. recomputes size and SHA-256 and compares both with SQLite;
4. scans the complete file for non-ASCII or NUL bytes before emitting any
   payload; and
5. rewinds the verified file for streaming.

The terminal adapter remains responsible for its framing. Telnet already
negotiates binary mode and escapes `IAC`; raw TCP, RLogin application data,
serial, and modem-established streams use the same session request. Transfer
activity is published as `downloading`/`uploading` with only the catalog
filename, never the host path.

SQLite schema 6 adds the caller's validated stock transfer-protocol preference
without changing file-area/catalog schema. Binary uploads reuse the same
per-session staging and exclusive commit path below; batch members commit
independently after each successfully completed file.

Statistics change only after the transfer reports complete and the emitted
byte count equals the verified catalog size. Interrupted or rejected transfers
receive no success credit. A no-charge area still records the file's aggregate
download count but does not debit/increment caller download statistics.

## Upload Staging and Commit

The initial upload protocol accepts bounded 7-bit `.TXT` lines. `/S` completes
and `/A` cancels. EOF/disconnect, invalid data, size excess, or an error drops
the staging object and removes its partial file.

Successful commit is:

1. revalidate active caller and current area/upload authorization;
2. flush/sync and independently hash/size the staged bytes;
3. reject empty or oversized content;
4. create the final destination exclusively;
5. copy and sync bytes;
6. insert catalog metadata and successful-upload statistics transactionally;
7. delete the final byte file if database insertion fails; and
8. remove staging after the catalog commit succeeds.

Exact case-insensitive filename duplication within an area is rejected. The
manual's broader same-basename/different-extension and digit-stripping checks,
content-hash policy, Sysop review/approval, and virus-scanning hooks remain
follow-up. No upload is placed directly into trusted final storage.

## Setup and Administration

`spitfire setup` creates the two starter areas, final storage directories, and
generated text fixtures together with the board's schema-4 database.
`spitfire config` exposes only implemented behavior:

- list area number/name/state/count/security/storage key;
- create a validated area and its storage directory;
- edit names, descriptions, access mode, read/upload levels, preview,
  no-charge, maximum upload size, and privileged levels; and
- enable/disable without deleting catalog entries or physical files.

The UI calls the same domain validator and `BoardAdmin` service used elsewhere.
It does not expose raw SQL, internal paths, or destructive delete/relocate
operations.

## Concurrency and Failure Behavior

Each connection opens its own SQLite handle with foreign keys and a bounded
busy timeout. Reads and atomic counters coexist across nodes. Final-file
exclusive creation resolves simultaneous same-name upload races: exactly one
commit succeeds and the losing stage is cleaned. Area state and caller state
are rechecked at the backend boundary.

Expected missing, restricted, duplicate, malformed, canceled, disconnected,
and content-mismatch conditions return useful errors rather than panicking.
The node lease releases through the existing RAII path even when transfer or
session I/O fails.

## Verification Status

Committed synthetic tests cover schema-3 and schema-8 upgrades, setup and administration,
area identity/security, listing/search privacy, traversal/symlink rejection,
hash checking, successful/interrupted ASCII downloads, staged upload cleanup,
duplicate races, statistics, node transfer state, a complete reconnect flow,
Telnet upload/download, and simultaneous raw-TCP downloads on two nodes. The
file-presentation closure adds exact row/date rendering, multiline wrapping,
paging abort, date validation, area/all scope, checkpoint persistence and
monotonicity, downloadable-statistics privacy, caller isolation, and reconnect
coverage.

A separate acceptance run used `spitfire setup` to create a four-node board
with two starter areas, then connected through its real localhost Telnet
listener. The caller listed and searched files, downloaded `WELCOME.TXT`,
uploaded and re-downloaded a 24-byte `ACCEPT.TXT`, and logged off. The stored
upload and the transfer report independently agreed on SHA-256
`de7d4c8baba657f611136729b5578c2060fcc8035c0c12b8e083bf189e7d51be`.
A second connection found the same catalog entry and bytes and displayed the
persisted success-only transfer counters.

The 2026-08-22 presentation acceptance created a board through normal setup,
added controlled native catalog metadata, and exercised listing plus explicit-
date and last-check scans over a real Telnet listener with ANSI/SyncTERM-style
negotiation. A reconnect observed the persisted checkpoint and an empty last-
check result. A separate RAW/text listener repeated listing and a current-area
four-digit-date scan. The external SyncTERM application was not available, so
this is not a new manual SyncTERM-version run; the already completed external
X/Y/ZMODEM interoperability record is unchanged.

No committed test uses a Buffalo Creek or Synchronet file. Physical
serial/modem transfer remains hardware-unverified. Actual SyncTERM 1.9rc4 and
1.10a acceptance now covers the principal X/Y/ZMODEM paths; see the transfer
specification for the exact matrix and the narrowly blocked ZMODEM batch UI
path.

## Known Fidelity Gaps and Follow-Up

- second-client evidence for Telink and 1K-XMODEM-g, external YMODEM-g upload/
  batch evidence, ZMODEM multi-file client selection, tag queues, and external
  protocol drivers;
- historical file/KB ratios, daily limits, transfer-time credit, and disk-free
  threshold behavior;
- tagging, file requests, Sysop-only upload and validation queues;
- full duplicate heuristics and optional content-hash policy;
- `FILE_ID.DIZ` extraction and bounded archive/text inspection;
- `SFFILES.BBS`/extended directory import, historical dynamic colorization,
  comma-sensitive size recognition, and historical file-area records;
- CD-ROM/read-only area behavior and legacy metadata import; and
- privileged move/delete/shuffle/maintenance with audit history.

These are preserved in the parity checklist as Category A gaps or Category B/C
follow-up rather than being implied complete.

## If You Return After Six Months

Increment 5 proved a functioning native file library over the same caller and
multinode session engine used by messages. Metadata began at SQLite schema 4;
schema 6 adds transfer preference state; schema 9 adds only the caller's
dedicated new-file checkpoint; the current schema 10 independently adds the
privacy-bounded latest-access-denial context described in
[Caller Authentication](sfng-caller-authentication.md); bytes
are confined under logical `EXTERNAL`; in-progress uploads are confined under
logical `WORK`; security is enforced below menus; and only successful verified
transfers update statistics. Stock ASCII and the documented binary family now
share that same trust boundary; actual SyncTERM acceptance covers the
principal X/Y/ZMODEM modes. Native file rows now preserve stock columns,
board-local file dates, and multiline descriptions; New Files supports
specific/all area scope plus explicit-date/last-check selection without
changing the transfer or storage model.

Read this document, [the public status](../STATUS.md), and
[the parity checklist](stock-spitfire-3.7-parity.md) first. Then inspect
[`file.rs`](../crates/sf-core/src/file.rs),
[`file_session.rs`](../crates/sf-core/src/file_session.rs), and the setup/admin
integration, then read [SPITFIRE NG File Transfers](sfng-file-transfers.md)
before changing wire protocols. The next stock-core increment is chosen from
remaining Category-A rows, not assumed to be generic integration work.
