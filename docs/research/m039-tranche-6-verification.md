# M039 Tranche 6 — Verification and Interoperability Summary

Status: **B-024 VERIFIED; B-011, B-014, AND B-023 IMPLEMENTED**

This is a rights-safe public summary. External programs were independent test
peers only; their binaries, source, archives, captures, emulator files, and
payloads are not distributed here.

## B-024 result

All nine required engines pass semantic, bounded failure, cancellation,
accounting, and zero-byte tests.

- Qodem 1.0.1 and SyncTERM 1.9rc4 completed representative current-schema
  XMODEM, YMODEM, and ZMODEM caller journeys.
- Original DOS Qmodem 4.6 Test-Drive completed 1K-XMODEM-g and two-member
  YMODEM-g Batch interchange in both directions.
- Independent lrzsz 0.12.20 completed a real three-member ZMODEM upload.
  SyncTERM's single-selection picker is not represented as that evidence.
- Original DOS BinkleyTerm 2.59 completed TeLink interchange in both
  directions. TeLink is included because it is one of B-024's required
  protocol choices; it is not a separate project-wide policy.
- A real macOS OpenSSH caller connection carried a completed binary transfer.
  Disconnect during negotiation now unwinds within the bounded cancellation
  interval without exposing a shell or leaving quota/active-use authority.

Generated payloads verified exact names, logical lengths, digests, batch
termination, per-item settlement, and clean session return. No proprietary
payload is a repository fixture.

## Resource and recovery result

Tests cover corrupt sequence/checksum/CRC, retry exhaustion, timeout, local
and remote cancellation, unsafe and oversized negotiated names, declared-size
limits, path traversal, duplicate/staging conflicts, incompatible batch use,
zero-byte items, idempotent settlement/release, daemon restart, and bounded
large-source reads. Failures do not panic, escape storage, double-account, or
leave nonterminal reservation/active-use authority.

Schema 17 migration and injected rollback preserve file locators and request
references. Cold backup/new-root restore preserves an empty catalog object,
its digest, managed bytes, stable ID, and schema-16 transfer/storage authority.

## Rows not promoted

- B-011 remains **IMPLEMENTED** pending the complete caller-visible
  member-level partial/recompute/continue/skip/cancel and live multinode queue
  matrix.
- B-014 remains **IMPLEMENTED** pending the complete live DST/no-DST policy,
  boundary, concurrent quota/credit, and caller-visible accounting matrix.
- B-023 remains **IMPLEMENTED** pending external-media loss during transfer,
  restored-board rebind/probe, legacy FA/`SFFILES.<x>` adapter acceptance, and
  full read-only client journeys.

Category B is **10 VERIFIED, 5 IMPLEMENTED, 5 PARTIAL, and 5 NOT STARTED**.

## Exact nonclaims

Exact historical ASCII newline/termination, stock prompt timing, stale-batch
wording, DAILYLMT rounding/reset edges, upload-credit idle/retry behavior, and
legacy extended-directory lookup details remain evidence-gated. No exact byte
behavior is inferred from the modern semantic implementation.
