# M039 Tranche 5 Verification

## Result

This bounded pass verifies the current schema-15 implementation against the
accepted B-013, B-015, and B-012 matrices. It adds focused archive and staged-
operation failure coverage and completes representative journeys with Qodem,
SyncTERM, and macOS OpenSSH. It does not implement another tranche or claim
undocumented legacy bytes.

The initial result was **verification evidence improved; all three rows remain
IMPLEMENTED**. The 2026-08-30 closure follow-up below completes the accepted
safe-inspection matrix and promotes B-013 to VERIFIED. B-015 and B-012 remain
IMPLEMENTED because their individually listed recovery/import/operator and
caller-workflow items are still material. A row is not promoted merely because
most behavior is working.

## Real-client acceptance

All fixtures used fictional callers and board-local temporary data outside the
repository. No credentials, capture files, host keys, or board fingerprints
were retained.

| Client | Transport/profile | Result |
| --- | --- | --- |
| Qodem 1.0.1 | Telnet, ANSI/CP437 | Passed normal listing, bounded text preview, ZIP member listing, binary/control rejection, unavailable-file request, Preview-area listing/text/ZIP inspection, return to Main, and Goodbye. It exposed one common-session defect: denied Preview download fell through to the request prompt. The session never transferred bytes; the defect is fixed and regression-tested with localized Preview denials. |
| SyncTERM 1.9rc4 | Telnet, ANSI/CP437 | Passed the same representative file journey, including request and Preview inspection, with menus usable after previews and a clean Goodbye. SyncTERM SSH was not retried and server crypto was not weakened. |
| macOS OpenSSH | SSH, negotiated PTY | Passed authenticated common-session file listing, ZIP inspection, binary/control rejection, private request creation, Preview listing/ZIP, return, and clean Goodbye without a second BBS password prompt. The narrow PTY caused one pagination input to consume the planned Preview text command; Qodem/SyncTERM and automated common-session coverage prove that path. Independent real exec, SFTP, SCP, and forwarding attempts failed; no OS-shell or subsystem route appeared. |

Telnet was directly rechecked by Qodem and SyncTERM. RAW and RLogin were
rechecked through the workspace transport/common-session suites rather than a
new manual client. SSH was directly rechecked with OpenSSH.

## Archive and DIZ matrix

Focused tests now cover:

- valid empty and multi-member ZIP files;
- long names at the accepted bound and traversal-style names;
- highly compressible content under metadata-only inspection;
- 4,096 members and rejection at 4,097;
- aggregate name/comment/extra metadata overflow;
- aggregate declared uncompressed-size overflow;
- malformed central directory and truncated archive;
- encrypted members rejected fail-closed;
- archive catalog-size limit and deterministic deadline expiry;
- duplicate case-insensitive `FILE_ID.DIZ` candidates;
- DIZ content above 64 KiB, compression ratio above 200:1, controls, and
  encrypted DIZ rejection.

Inspection uses raw metadata and does not extract or execute members. Unsafe
member names are labeled without becoming host paths. Errors are mapped through
localized caller states and no archive/file/DIZ content enters semantic audit.

The initial remaining B-013 matrix cases were: a synthetic in-bounds ZIP64
fixture; CP437 filename-flag behavior; duplicate ordinary member names;
unsupported-codec and deliberately overlapping-member fixtures; long-line
truncation versus the current fail-closed 2,000-line contract; direct
symlink/device fixture coverage at the inspection entry; and profile-specific
Modern/Minimal/Classic snapshot acceptance. These are verification tests, not
new product behavior.

## Saga and filesystem matrix

Focused failure injection now proves deterministic recovery for journal-only,
staged, published-before-catalog, catalog-committed, and source-cleanup phases.
The persisted outcomes are `rolled-back`, `needs-review`, or `committed` and
their catalog/source/destination byte states are asserted. Additional tests
cover stale file/area versions, held lease, destination collision, corrupted
source/short-copy equivalent, staging-directory creation failure, database
catalog-commit failure, remove database failure, request-state failure,
PendingReview transition failure, and reconciliation restart.

No tested failure silently diverges catalog and managed bytes. The remaining
B-015 matrix is nevertheless material:

- explicit fsync and final-rename failure injection;
- an actual cross-filesystem copy/verify/publish fixture rather than its shared
  code path alone;
- read-only-root and operator-cancellation cases;
- expired-lease takeover and simultaneous-operator/multinode mutation cases;
- explicit persisted `resumable` and `orphaned` recovery classifications (the
  current state vocabulary records committed, rolled-back, and needs-review);
- bounded legacy SFFILES import preview/apply, which is not implemented;
- external caller-visible aftermath after an operator move/remove in each
  common transport, beyond common projection/runtime tests.

After the closure work, bounded legacy SFFILES import preview/apply, explicit
operator cancellation, persisted resumable/orphaned vocabulary, true-EXDEV and
read-only-root evidence, and live operator move/remove aftermath remain. Those
accepted items prevent B-015 promotion.

## Backup, authorization, and privacy

Workspace acceptance reconfirms schema-15 cold backup/new-root restore,
managed bytes and schema-15 state, schema-14 backup restore followed by
writable 14→15 migration, and safe rejection of nonterminal operations/active
uses. Requests, review/lifecycle/integrity, policies, journal normalization,
quarantine, publications, and semantic events remain authoritative.

Dispatch tests reconfirm that ordinary callers cannot modify, review, remove,
or reconcile; Preview callers inspect but cannot upload/download; PendingReview
items remain absent from ordinary projection; requests are private; active
uses drain destructive operations; and privileged commands reauthorize and
use expected versions. Semantic audit excludes host paths, file/DIZ/member
content, login identifiers, private real names, and credentials.

Remaining B-012 matrix items are: caller cancellation through each live common
transport; live operator accept/reject outcomes paired with caller
non-disclosure; warning continue/cancel and recomputation under a concurrent
upload race; full malformed/oversized/unknown-preservation matrices for legacy
SFNOUP/SFUPCASE adapters; and all-transport live review outcomes. These prevent
B-012 promotion even though the native request/review/duplicate/policy domain
is implemented and working.

## Initial row decision

| Row | Decision | Reason |
| --- | --- | --- |
| B-013 | **IMPLEMENTED** | Real clients and a substantially expanded safe-inspection matrix pass, but the remaining accepted ZIP/text/special-file/profile cases above are not all proved. |
| B-015 | **IMPLEMENTED** | Staged recovery is materially stronger and deterministic for tested phases, but accepted filesystem/concurrency/recovery-class/import/client-aftermath cases remain. |
| B-012 | **IMPLEMENTED** | Request creation and privacy pass in real clients; accepted cancellation/review/race/legacy-adapter matrices are incomplete. |

Category B therefore remains **8 VERIFIED, 3 IMPLEMENTED, 7 PARTIAL, and 7
NOT STARTED**.

## 2026-08-30 verification-matrix closure

The bounded follow-up closes the initial B-013 list with synthetic in-bounds
ZIP64, CP437 and malformed-Unicode member names, stored and deflated enabled
codecs, explicit unsupported-codec rejection, duplicate central-directory
entry detection, aliased/overlapping headers, malformed extra metadata,
odd/empty names and timestamps, exact text byte/line boundaries, UTF-8 and
CP437 edges, CR/LF variants, C0/C1/DEL/ANSI/OSC/DCS handling, direct symlink and
non-regular-file rejection, and identical semantics under Modern, Minimal, and
Classic profiles. The ZIP service now rejects compression methods not enabled
in the minimized build and rejects a standard EOCD count that disagrees with
the library's central-directory projection. It still never extracts.

Qodem 1.0.1, SyncTERM 1.9rc4, and macOS OpenSSH each completed a focused
unsafe-text rejection, returned to usable menus, created the harness-required
private Offline request, and disconnected cleanly. Telnet and SSH were live;
RAW and RLogin remain covered by the common-session transport suites. The
server completed all three sessions and observed exactly three private
requests.

Publication and move renames now synchronize the containing directory where
supported. SFFILES publication persists its staging path and digest before the
write, syncs the staged file, records bytes-published after rename, and cleans a
failed pre-rename stage deterministically on restart. Expired-lease takeover,
stale metadata/move/remove, request/tombstone, PendingReview/DIZ/CAS review,
policy ordering/version, and staged-backup rejection/recovery tests pass.

The final row decision is:

| Row | Final decision | Remaining boundary |
| --- | --- | --- |
| B-013 | **VERIFIED** | No semantic/security/recovery blocker remains. Exact historical layout is a separate presentation/evidence claim. |
| B-015 | **IMPLEMENTED** | Legacy SFFILES import, explicit operator cancellation, persisted resumable/orphaned states, true-EXDEV/read-only evidence, and live operator aftermath remain. |
| B-012 | **IMPLEMENTED** | Live caller cancellation and operator review across transports, upload warning continue/cancel/recompute integration, and lossless legacy SFNOUP/SFUPCASE adapter matrices remain. |

Category B is now **9 VERIFIED, 2 IMPLEMENTED, 7 PARTIAL, and 7 NOT
STARTED**. No 86Box test is required for this semantic decision; exact legacy
byte behavior remains separately evidence-gated.

## Historical and scope boundary

No 86Box run is required before semantic verification can continue. Exact
SFFILES tolerant parsing, SFILEREQ bytes, SFNOUP/SFUPCASE edge grammar,
slash-review runtime details, duplicate-family ordering, Shuffle/Erase failure
behavior, and utility-generated bytes remain separately deferred. None was
invented.

Future B-015/B-012 verification must close only the exact matrix items listed
here; their current IMPLEMENTED status must not be overstated. The next
separately gated Category-B work is M039 Tranche 6, beginning with B-024 and
then B-011/B-014/B-023. This report does not begin that gate or implementation.

## Related documents

- [Implementation report](m039-tranche-5-safe-file-inspection-request-maintenance-implementation.md)
- [File system architecture](../sfng-file-system.md)
- [Parity ledger](../stock-spitfire-3.7-parity.md)
