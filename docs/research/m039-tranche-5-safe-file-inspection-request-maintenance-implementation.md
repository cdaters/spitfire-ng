# M039 Tranche 5 Safe File Inspection, Request, and Maintenance Implementation

## Result and status boundary

This implementation realizes the accepted Tranche 5 native contracts in
dependency order: schema 15, B-013 inspection, B-015 maintenance, then the
remaining B-012 request/review/upload-policy behavior. Native SQLite semantic
state plus confined managed bytes are authoritative. Historical flat files
remain explicit compatibility adapters.

The implementation is complete in current source. This report originally
marked all three rows IMPLEMENTED pending verification. The later verification
closure promotes B-013 to VERIFIED; B-015 and B-012 remain IMPLEMENTED. That
distinction avoids turning implemented behavior into unsupported legacy-byte,
recovery, operator-workflow, or client claims.

## Historical authority and compatibility boundary

The implementation follows the evidence and authority boundary summarized in
the [native file-system specification](../sfng-file-system.md).
SPITFIRE 3.7 documentation establishes Read A Text File, View A File Archive,
Preview-area inspection without transfer, SFFILES columns and OFFLINE request
behavior, Shuffle/Erase, duplicate warnings, slash-marked Sysop review,
SFNOUP/SFUPCASE, and FILE_ID.DIZ workflows. The distributed `SFFILE.MNU`
confirms command letters `R`/`V` and immutable action identifiers `J`/`G`.

No exact undocumented bytes were invented. Native state does not parse or
treat `SFFILES.BBS`, `SFILEREQ.LOG`, `SFNOUP.DAT`, `SFUPCASE.DAT`, or
`FA<x>.TXT` as concurrent authority. SFFILES publication preserves only the
documented filename/size/date/description columns, comma-formatted size, and
OFFLINE marker. Publication is CP437 and fails instead of truncating a native
filename or emitting text outside that historical byte boundary. Exact
tolerant input, round trip, line endings, and utility
output remain evidence-gated.

## Schema 15

The transactional 14→15 migration:

- preserves existing FileId/FileAreaId, callers, credentials, lifecycle and
  subscription state, messages, transfers, SSH identity/configuration, and
  schema-14 public-information state;
- adds independent file lifecycle and integrity state plus optimistic file
  and area versions;
- adds private versioned requests, native upload-denial and optional
  description-normalization policy, review provenance, operation journal,
  leases/name reservations, active-use drains, legacy-publication generations,
  and append-only semantic file events; and
- creates no request, review, operation, denial, or fabricated file row.

Pre/post migration snapshots and injected validation failure prove rollback to
an unchanged schema-14 database. Cold restore retains an old backup at schema
14 until a later writable startup performs this migration.

## Inspection authority and bounds

Text inspection opens only a currently authorized catalog FileId through the
confined regular-file resolver. It rejects symlinks, traversal, devices,
binary/NUL data, ESC and unsafe terminal controls. The limit is 256 KiB and
2,000 lines. ASCII, valid UTF-8, and explicit CP437 decoding are separate;
downloadable content is rendered as sanitized text and never implicitly
interpreted as ANSI.

ZIP metadata inspection uses `zip` 8.6.0 with default features disabled and
only the `deflate-flate2-zlib-rs` feature. It never extracts or executes. The
crate declares MIT licensing and Rust 1.88 MSRV; both fit the project license
and current Rust 1.97.1 toolchain. Implementation-time upstream/RustSec review
found no advisory requiring a different selected version. The
current limits are 512 MiB input, 4,096 members, 1,024 bytes per member name,
4 MiB aggregate metadata, 4 GiB aggregate declared expansion, and a five-
second inspection deadline. Overlapping data, malformed archives, excess
metadata/declared size, and unsafe member paths fail or are marked safely
without exposing host paths.

FILE_ID.DIZ discovery is case-insensitive exact-basename matching. Multiple
matches are ambiguous; encrypted matches are not read. Content is limited to
64 KiB, a 200:1 expansion ratio, the multiline description limits, the same
control/encoding safety, and an explicit versioned operator review before it
can replace authoritative description text.

Preview-area callers may list/search/read/view but cannot upload or download.
Inspection and download hold active-use records, and output reauthorizes at
bounded page intervals so lifecycle/access changes stop further disclosure.

## File lifecycle, integrity, and maintenance

Lifecycle is `Active`, `Offline`, `PendingReview`, `Disabled`, or
`Tombstoned`. Integrity is independently `Unknown`, `Present`, `Missing`, or
`DigestMismatch`. FileId survives metadata edits, review, moves, requests,
reconciliation, and tombstoning; filename is not ownership identity.

Typed daemon/domain commands cover operator add, metadata/lifecycle change,
move, recoverable remove, review, request resolution, reconciliation, and
legacy listing publication. Every privileged dispatch reloads current
operator lifecycle/effective security, accepts expected versions, reports a
structured stale conflict, and appends a content-free semantic event.

Add, caller upload, and move use persistent operation IDs, destination-local
staging or an already bounded caller staging file, SHA-256 verification,
leases/name reservations, atomic destination publication, and journal phases.
Move preserves FileId. Destination collision fails. Copy plus digest
verification supports the cross-filesystem shape; source removal occurs only
after the destination catalog commit. Active transfers/inspections block move
and remove.

Remove requires explicit confirmation and creates a recoverable tombstone.
Managed bytes remain protected and hidden rather than being irreversibly
deleted; a future separately authorized purge is required for destruction.
Pending requests become Stale. Adding the filename to denial policy is a
separate explicit mutation, never an implicit erase side effect.

Restart reconciliation classifies nonterminal journal entries as rolled back,
resumable/committed, or needs operator review. Safe staged paths are cleaned;
catalog-committed moves finish source cleanup; bytes-published ambiguity is
retained for review. Deep catalog/digest reconciliation requires maintenance
mode and records only result counts. Cold backup refuses nonterminal
operations or active uses instead of capturing an ambiguous board.

## Requests, review, duplicates, and policy

Only a currently visible Offline entry or an entry with Missing integrity may
create a private request. Requests use stable request/file/caller IDs, an
Offline/Missing reason, Pending/Fulfilled/Rejected/Cancelled/Stale status,
board-local creation day, resolution actor/time, and optimistic version.
Creation is idempotent per caller/file, bounded to 25 pending and 100 per
board-local day, and hidden from other callers. Caller cancellation and
operator resolution use compare-and-swap transitions.

A slash-prefixed upload description becomes durable `PendingReview`; the
slash is a compatibility signal, not stored public text. Pending files are
absent from ordinary list/search/inspect/request/download. Versioned operator
accept/reject and reviewed DIZ application reuse the same authority.

Exact normalized filename duplicates are hard conflicts under a persistent
name reservation. Same basename with another extension and bounded trailing-
digit families are advisory warnings; the latter is enabled by board policy
and capped at 50 results. Native SFNOUP rules are versioned, bounded to 1,024,
DOS-wildcard compatible, deterministic, audited, and permit only an explicit
threshold-Sysop override. A bounded legacy parser reports malformed lines.
Entered description case is preserved by default; optional historical-style
uppercase policy and exception terms are separate. FILE_ID.DIZ text is never
case-normalized implicitly.

## Storage, presentation, privacy, and recovery

The existing confined managed root is exposed as an ordered logical root with
read/write capability. The type admits future read-only secondary roots—the
modern equivalent of CD-ROM/extended-directory storage—without adopting raw
DOS paths or implementing B-023. Native descriptions support 20 lines and
4,096 UTF-8 bytes independently from SFFILES wrapping.

Modern, Minimal, and Classic use the same R/V commands, authority, projection,
and localized semantic states. The en-US package advances to 1.6.0; Modern and
Minimal advance to 1.4.0 and Classic to 1.5.0. No proprietary display bytes
were added.

Audit stores stable IDs, versions, operation IDs, digests, transition names,
and bounded result summaries. It does not store file/DIZ/archive contents,
host paths, request queries, login identifiers, real names, credentials, or
secrets. Cold backup preserves schema-15 catalog/lifecycle/integrity,
requests/review/policy, journal normalization, publication state, event audit,
and all cataloged managed bytes. New-root restore and schema-14 restore then
writable migration are covered.

## Acceptance completed and remaining

Automated coverage includes migration/rollback, text/ZIP/DIZ bounds,
malformed/traversal/bomb-like cases, Preview authorization, request
idempotence/CAS, PendingReview privacy, duplicate/SFNOUP policy, move identity,
tombstone, active-use draining, crash recovery, maintenance-mode
reconciliation, SFFILES publication columns, backup/new-root restore,
schema-14 restore/migration, common transport regressions, and the caller
request journey. Existing transport suites exercise Telnet, RAW, RLogin, and
SSH common-session behavior and clean disconnect.

At the initial implementation checkpoint, promotion still required focused
ZIP edge vectors, saga phase/filesystem-failure coverage, and real Qodem,
SyncTERM, and macOS OpenSSH Tranche 5 caller journeys. The established exact
legacy tests for SFFILES, SFILEREQ, SFNOUP/SFUPCASE edge cases, slash-review,
duplicate-family ordering, shuffle/erase failures, and utility output also
remain deferred. No original-runtime run was needed for this semantic
implementation.

The later bounded [verification pass](m039-tranche-5-verification.md) completed
the three real-client journeys, repaired a Preview-area request-prompt
fallthrough, and materially expanded archive/DIZ and saga failure coverage.
B-013 is now VERIFIED. B-015 and B-012 remain IMPLEMENTED because that report
identifies their exact filesystem/recovery/import and request/review/race/
legacy-adapter cases still unproved.
