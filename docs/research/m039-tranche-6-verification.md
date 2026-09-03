# M039 Tranche 6 — Verification and Interoperability Summary

Status: **B-024, B-011, B-014, AND B-023 VERIFIED; TRANCHE 6 CLOSED**

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

## B-011 queue closure

The Files session exposes bounded list, remove/skip, recompute, continue, and
clear/cancel operations over session-ephemeral stable-FileId items. Protocol
engines report the completed member prefix, allowing completed items to settle
exactly once while failed and unstarted items remain ordered and recoverable.
Recompute reauthorizes each item and refreshes total and chargeable bytes.
Disconnect, Files exit, daemon restart, caller/session separation, stale-item,
partial-result, and multinode quota/accounting matrices pass. Qodem and
SyncTERM batch journeys exercise the same authority.

## B-014 policy and accounting closure

Tests cover DLPD and decimal-KB boundaries, VWR warnings, VER enforcement and
restoration, file-count and byte ratios, no-charge exclusions, Preview denial
before reservation or negotiation, and idempotent capped upload credit for
Active and PendingReview receipts. Deterministic clocks cover ordinary
midnight, spring-forward and fall-back transitions, non-DST zones, timezone
version conflicts, cross-midnight settlement to the reservation day, and
concurrent-node quota contention.

## B-023 extended-storage closure

Logical roots and stable per-file locators retain catalog identity while
external media moves between available and unavailable states. Read-only
caller list, inspect, and download pass; upload, rename, delete, and mutating
move are denied. Always-stage sources use bounded-copy, digest-verified,
delete-on-drop preparation, making mid-transfer media loss deterministic
without whole-file buffering. Cold backup preserves managed bytes plus
external catalog/locator authority, restores external roots to Unknown, and
permits versioned rebind/probe without changing FileIds. Explicit confined FA
mapping and numbered `SFFILES.<x>` publication pass as compatibility adapters,
not native raw-path authority.

## Final row decision

| Row | Status |
|---|---|
| B-024 | **VERIFIED** |
| B-011 | **VERIFIED** |
| B-014 | **VERIFIED** |
| B-023 | **VERIFIED** |

M039 Tranche 6 is semantically **VERIFIED / CLOSED**. Category B is
**13 VERIFIED, 2 IMPLEMENTED, 5 PARTIAL, and 5 NOT STARTED**. B-015 and B-012
remain IMPLEMENTED under their separately documented acceptance matrices.

## Exact nonclaims

Exact historical ASCII newline/termination, stock prompt timing, stale-batch
wording, DAILYLMT rounding/reset edges, upload-credit idle/retry behavior, and
legacy extended-directory lookup details remain evidence-gated. No exact byte
behavior is inferred from the modern semantic implementation.
