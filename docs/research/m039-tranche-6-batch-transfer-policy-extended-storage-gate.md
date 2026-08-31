# M039 Tranche 6 — Batch Transfer Policy and Extended Storage Gate

Status: **ACCEPTED ARCHITECTURE CONTRACT**

This public summary records the interface, transaction, security, and
acceptance boundary for B-024, B-011, B-014, and B-023. It omits private
research holdings and test-session archaeology.

## Required protocol set

B-024 covers ASCII, XMODEM Checksum, XMODEM CRC, 1K-XMODEM,
1K-XMODEM-g, YMODEM Batch, YMODEM-g Batch, ZMODEM Batch, and TeLink.
Protocol engines operate on a transport-neutral binary stream and cannot
authorize callers, resolve host paths, mutate the catalog, or settle quota.

Every item is reauthorized immediately before transfer. Preflight checks the
caller, lifecycle, integrity, Preview restrictions, expected file version,
policy, storage availability, and active-use conflicts. Authorization then
reserves quota atomically; success settles once, while failure or cancellation
releases unused authority.

## Queue and policy model

B-011 queues are bounded, session-ephemeral collections of stable `FileId`
values. They never use mutable filenames as identity and never survive a
disconnect or daemon restart. YMODEM, YMODEM-g, and ZMODEM can execute true
multi-file batches. Single-file protocols reject multi-item queues.

B-014 uses versioned per-security policy, board-local civil days in a named
IANA timezone, daily usage buckets, atomic whole-batch reservations, and
idempotent per-item settlement. Ratio, daily-limit, no-charge, Preview, and
upload-credit decisions remain distinct. Preview denies transfer before
reservation or negotiation.

## Storage authority

B-023 uses stable logical roots and explicit per-file locators. Native
resolution is `FileId → locator → StorageRootId → confined source`, never a
caller-supplied path or an arbitrary filename search. Roots may be writable,
read-only, secondary, or temporarily unavailable. Temporary root loss is
`StorageUnavailable`, not permanent file `Missing`.

Uploads always enter confined Tranche 5 staging before inspection, duplicate
policy, review/publication, and settlement. Downloads acquire active-use
authority over a confined seekable source and use bounded streaming reads.

## Persistence and recovery

Schema 16 owns transfer policy, timezone versions, usage buckets,
reservations, idempotent settlements, transfer history, storage roots, and
locators. Protocol sockets, raw frames, terminal streams, and session queues
are intentionally ephemeral. Cold backup drains active transfers and
reservations or fails safely; it never attempts to preserve a live protocol
connection.

Cancellation is protocol-aware, idempotent, audited, and releases quota,
active-use, and staging authority. Restart classifies nonterminal transfers
and releases authority deterministically rather than resuming raw wire state.

## Security and privacy

The implementation bounds retries, timeouts, metadata, filenames, queue size,
aggregate bytes, staging use, and protocol parser work. Negotiated filenames
are sanitized basenames and never select a final filesystem path. Audit stores
stable IDs, protocol, states, sizes, and semantic outcomes—not contents, raw
frames, credentials, private identity, or host paths.

## Acceptance boundary

- B-024 requires all nine engines, both required directions, independent-peer
  interoperability, cancellation/resource/accounting coverage, and carrier
  acceptance.
- B-011 additionally requires complete caller-visible partial/recompute and
  queue-lifecycle acceptance.
- B-014 additionally requires the complete live policy, DST, quota, and
  upload-credit matrix.
- B-023 additionally requires external-media-loss, restore/rebind, legacy
  mapping, and live read-only client acceptance.

Exact historical prompt text, ASCII newline timing, and legacy flat-file byte
edges remain separately evidence-gated and are not invented by this contract.
