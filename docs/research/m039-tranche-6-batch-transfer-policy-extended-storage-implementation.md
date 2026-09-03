# M039 Tranche 6 — Batch Transfer Policy and Extended Storage Implementation

Status at implementation checkpoint: **IMPLEMENTED; B-024 VERIFIED,
B-011/B-014/B-023 ACCEPTANCE OPEN**

Subsequent verification promoted B-011, B-014, and B-023. The current status
is **B-024/B-011/B-014/B-023 VERIFIED; TRANCHE 6 CLOSED**. This report retains
the earlier implementation boundary; see the verification summary for closure.

## Schema 16

The transactional schema-15→16 migration adds only durable Tranche 6
authority: versioned transfer policy and timezone, board-day usage, quota
reservations, idempotent settlements, transfer events, logical storage roots,
and per-file locators. Existing caller, message, file, review, journal, SSH,
and public-information identities remain intact. Injected validation failure
leaves schema 15 unchanged.

Session queues, protocol sockets, raw protocol frames, and terminal byte
streams are not persisted.

## Protocol engines and runtime

The shared engine implements ASCII, XMODEM Checksum, XMODEM CRC,
1K-XMODEM, 1K-XMODEM-g, YMODEM Batch, YMODEM-g Batch, ZMODEM Batch, and
TeLink. They share transport-neutral bounded I/O, cancellation, error, and
progress contracts. Caller authorization, catalog changes, storage resolution,
and accounting remain outside protocol parsers.

Downloads resolve a stable file ID to a confined locator, reauthorize current
state, acquire active-use, reserve quota, stream bounded source chunks, and
settle once. Uploads receive into confined staging, hash and inspect the bytes,
apply duplicate and review policy, publish through Tranche 5 authority, and
only then settle accounting and bounded upload-time credit.

SSH connection closure is signaled independently of its bounded input queue.
Protocol reads observe closure within a bounded polling interval, allowing
reservations, active-use, staging, and the node session to unwind even when
input buffering was saturated. SSH remains caller transport only: it provides
no OS shell, exec, SCP, SFTP, subsystem, or forwarding route.

## Queues and accounting

File sessions own bounded, ordered, session-ephemeral stable-`FileId` queues.
Tagging is idempotent; clear and totals are deterministic; stale versions are
detected before transfer. Whole-batch reservation prevents one caller on two
nodes from overspending the same daily quota. Settlement identity prevents
double counting on retry or repeated completion.

Native policy implements file-count and byte ratios, per-security DLPD and
decimal-KB limits, warning/enforcement thresholds, named board-day timezone,
no-charge exclusions, Preview denial before negotiation, and capped completed
active-upload time credit. Legacy DAILYLMT input remains a bounded
compatibility adapter rather than runtime authority.

## Extended storage

Schema 16 adds versioned managed and external roots, ordered priority,
read/write or read-only access, configured/runtime availability, staging
policy, and stable per-file locators. Read-only roots support listing,
inspection, and download, but reject direct upload, rename, move, or delete.
Rebind is versioned and private; probe is confined and symlink-safe.

Download preparation exposes a confined seekable source. XMODEM, YMODEM,
ZMODEM, and TeLink consume bounded blocks or chunks instead of allocating the
entire file. Generated large-source tests enforce that boundary without
committing giant fixtures.

## Schema 17 zero-byte authority

Schema 17 makes zero-byte regular files valid native catalog objects. The
transactional migration changes the size invariant from positive to
nonnegative, validates preserved authority and references, and rolls back to
schema 16 on failure. Empty managed files, staged uploads, every required
protocol, and cold backup/new-root restore retain an exact zero logical length.

File-domain validity is independent of protocol capability. A future adapter
that cannot represent an empty payload must return a typed compatibility error
before negotiation; it must not fabricate bytes or invalidate the catalog.

## Row status at the implementation checkpoint

- B-024 — **VERIFIED**
- B-011 — **IMPLEMENTED**; caller-visible partial/recompute and complete live
  queue-lifecycle acceptance remain.
- B-014 — **IMPLEMENTED**; complete live policy/DST/concurrent-credit
  acceptance remains.
- B-023 — **IMPLEMENTED**; media-loss, restored-root rebind, legacy mapping,
  and full read-only client acceptance remain.

See the [verification summary](m039-tranche-6-verification.md) for external
interoperability and the exact nonclaims.
