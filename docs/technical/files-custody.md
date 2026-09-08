# Native Files custody and distribution contract (C6)

Status: implemented and accepted in C6. This extends
[native Files](../sfng-file-system.md), whose `file_areas` and `files` remain the
only area and file catalog. It does not introduce a CircuitNET-owned file store.

## Native admission boundary

`FileStorage::import_file` accepts an authorized area, bounded source stream,
filename, caller/operator/network source, description and scanner provider. It
preserves the original bytes, calculates SHA-256, inspects actual content, scans
according to area policy and records a durable decision. Import, completed caller
upload and received network publications must converge on this boundary.

The native `files` row represents an area association. Content identity is SHA-256
plus verified size; publication identity is separate. A repeated filename/hash in
one area is an explicit existing-file result. Changed bytes under an occupied
filename require an explicit new name. Different names can share content. No
implicit replacement or duplicate deletion occurs.

Original payloads live in the native content store, with hard-link area references
for compatibility with existing transfer and locator authority. Store names are
internal, never wire identities. Published originals are immutable. Unsupported
hard-link storage fails visibly. Legacy files are not silently rewritten by a
migration. Derivative creation and physical garbage collection are deferred;
removing an area reference never deletes content custody automatically.

Area safety policy selects Disabled, Optional or Required scanning, an approval
requirement and bounded archive limits. New explicit safety policy defaults to
Required and operator approval. Existing areas retain their previous scan-disabled
policy until configured; their new inputs still undergo archive inspection.
Required scanning with unavailable/error/unsupported results cannot publish.
Malware, suspicious content and incomplete archive inspection quarantine regardless
of approval. Legacy review commands cannot override this fence. Rescan evaluates
current policy; approval never means scanner failure was clean.

Durable states are Inspecting, Quarantined, PendingApproval, Published and Rejected.
A crash before completion leaves Inspecting unpublished and recoverable by rescan.
History stores safe decision classes, never payloads, host paths or credentials.
Quarantine retains original evidence and is excluded from caller downloads and
network export. File Area access checks still govern published downloads.

## Inspection and scanner interface

Recognition, inspection, creation and recompression are separate capabilities.
C6 implements ZIP, TAR and GZIP/TGZ inspection; other recognized archive formats fail
closed until a bounded decoder exists. No archive creation/recompression or branding
is part of admission. Extension alone never establishes actual content type.

Limits cover source bytes, cumulative expanded bytes, member count, individual
member bytes, expansion ratio, nesting and elapsed inspection time. Members are
streamed into private generated names; archive paths are never extraction paths.
Traversal, absolute/drive paths, links, conflicting names, malformed/encrypted
archives and incomplete inspection are quarantine reasons. Nested decoding shares
one cumulative budget. FILE_ID.DIZ is a root-level, case-insensitive bounded byte
source. Its safe UTF-8/CP437 text is a suggestion; Use/Edit/Ignore is explicit.
Original bytes and encoding provenance remain available to authorized tooling.

A scanner provider returns Clean, MalwareDetected, Suspicious, ScannerUnavailable,
ScannerError, Unsupported or SkippedByPolicy, with bounded engine/signature metadata.
A ClamD provider streams bytes through its documented INSTREAM interface. It does
not launch a shell or pass uploaded paths to a command. Original and inspected
member streams are subject to local policy. Test providers are harmless mocks.
Reference: [ClamD protocol](https://docs.clamav.net/manual/Usage/ClamdProtocol.html).

## CircuitNET boundary

File codenames map to native areas; file Dossiers are explicitly separate from
message subscriptions. Publication envelopes use network identity, origin, codename,
publication identity, hash, size, filename, description and forwarding path, never
local IDs/paths. Only approved native content is exportable. Remote scan claims
are provenance and cannot override receiving policy.

Compatible minor 1.3 adds metadata offers, hash-have decisions,
bounded binary chunks and typed durable publication receipts to the existing TLS
session. A receipt distinguishes Published, PendingApproval, Quarantined and Rejected.
C6 restarts interrupted payloads from zero and validates final size/hash before
native admission. It does not advertise resume capability.
Durable publication identity makes lost receipts/replay idempotent. Ingress and
path suppress reflection; configured file Dossiers govern tree fanout.

Existing CircuitNET Exchange and generic Events move messages, controls and file
work. Native publication queues independently of timing. Older peers continue
message/control exchange without file frames. Transfer is encrypted in transit;
files after delivery are available according to destination File Area access rules.

## Recovery obligations

Backup must include content custody once, area references, safety/history and network
receipt/queue authority, and validate hashes/sizes. Restore recreates references and
never publishes incomplete admission. Missing/corrupt objects fail visibly. Integrity
reporting distinguishes missing, corrupt and unreferenced payloads; repair is explicit.
C6 acceptance proves this with disposable boards and payload-aware restore.

## Archive operation matrix

| Format | Recognition | Inspection | Creation | Recompression |
| --- | --- | --- | --- | --- |
| ZIP | PK signatures plus directory/parser validation | Stored/Deflated members; classic bounded directory, CRC validation | Not implemented | Not implemented |
| TAR | ustar signature or valid classic header checksum | Regular files/directories; links, sparse and extension records rejected | Not implemented | Not implemented |
| GZIP / TGZ | GZIP signature plus decoder validation | Bounded decompression, then content inspection; TAR roots support DIZ | Not implemented | Not implemented |
| 7z | Signature | Unsupported; quarantine | Not implemented | Not implemented |
| RAR | Signature | Unsupported; quarantine | Not implemented or required | Not implemented |
| BZIP2 | Signature | Unsupported; quarantine | Not implemented | Not implemented |
| XZ | Signature | Unsupported; quarantine | Not implemented | Not implemented |
| Zstandard | Signature | Unsupported; quarantine | Not implemented | Not implemented |
| ARC / ARJ / LHA/LZH | Limited conventional signature recognition | Unsupported; quarantine | Not implemented | Not implemented |

Recognition of an unsupported marker is not parser validation. Filename extensions
that claim an archive without matching recognized content produce a signature-mismatch
quarantine. Content detection additionally distinguishes common executable/image/PDF
signatures and bounded UTF-8 text from otherwise unknown binary input. It is not a
claim to recognize every media format. TAR PAX/GNU extension records and ZIP64 are
explicitly outside C6 inspection, including when another utility can open them.
The maintained [tar entry API](https://docs.rs/tar/latest/tar/enum.EntryType.html)
provides typed entry classes; no archive path is passed to an extraction API.

Default limits: source 64 MiB, cumulative expansion 256 MiB, member 64 MiB, 1,024
members, nested-container depth 3, expansion ratio 100 and inspection budget 60 seconds.
The ratio denominator has a 1 KiB floor for small valid containers. The root container
is depth zero. GZIP contributes one expanded member; directory entries also count.
Limits are configurable through `sfconfig files ... limits`, with hard validation
ceilings (source 1 GiB, expansion 4 GiB, 10,000 members, depth 8, ratio 1,000,
300 seconds). No action can silently increase these ceilings. Scanner calls have
separate bounded I/O; the inspection budget is checked between provider operations.

Root DIZ input is limited to 16 KiB, rejects NUL-bearing input, retains exact bytes
and records UTF-8 versus CP437 decoding. CRLF/CR normalize for display, control
characters cannot execute, and descriptions remain within native 4 KiB/20-line
limits when applied. Duplicate DIZ suggestions are reported independently of hash
and filename duplicates; they never cause deletion or replacement.

## Custody, history and operational limits

Schema 33 retains the existing `files`/`file_areas` rows. `file_content` records
hash/size custody; `file_validation` records source, original filename, policy,
inspection and status; `file_safety_history` retains decisions. `safety_required`
is committed with a newly admitted native file row, so a crash before insertion
of the validation record still cannot bypass publication checks. Rescan can recover
an unpublished completed upload. Review integrates with the established versioned
Sysop action and the schema-15 review timestamps, rather than weakening backup checks.

Original content is read-only and area paths are hard links. Access decisions remain
in the native FileBackend and transfer authority. Inspection and ClamD never rewrite
bytes. A legacy area move cannot transfer a C6 approval into another area's policy;
use explicit re-import into the destination and retire the old listing. Normal
metadata edits do not rewrite a queued network envelope. Metadata revisions and
branded derivatives are future explicit publications, not mutation of an original.

Existing FTN callbacks assume atomic immediate native publication. They retain their
accepted behavior for legacy areas without C6 safety configuration. They fail closed
for explicitly configured C6 safety areas rather than bypass approval/scanning; a
future adapter extension must reconcile those receipt semantics with staged admission.
No FTN protocol, FileEcho/TIC translation or duplicate CircuitNET payload store is added.

ClamD configuration accepts a loopback socket address only, streams INSTREAM bytes
in at most 64 KiB chunks, bounds responses to 1 KiB and each socket wait to three
seconds, with a 30-second operation budget. Engine/signature metadata fields are
optional and remain absent when unavailable. The local provider is optional software,
not bundled or installed by SPITFIRE. A deterministic mock stream exercises Clean, detection and error responses, exact
INSTREAM bytes and response bounds. Actual ClamAV was unavailable; no live ClamAV
integration success is claimed.

New settings govern subsequent admissions/rescans; changing an area policy does not
silently rewrite historical inspection results. New network publications, including
hash-have content, enforce current receiving policy. A receipt records the historical
admission outcome, not a perpetual promise that the file remains downloadable after
later local moderation or rescan.

## Backup, consistency and cleanup

Cold backups include `native-content/<sha256>` entries once per cataloged content
object, together with the database and existing legacy file entries. Managed C6 area
aliases are reconstructed instead of copied repeatedly. Native content inventories
must match database hashes/sizes; restore verifies payloads before linking references
and does not change admission status. Quarantine and unfinished inspection survive.
Pending delivery and completed receipts live in the same snapshot; replay at a peer
converges if the restored sender is uncertain about a later acknowledgement.

Integrity reporting verifies size/hash and distinguishes missing/corrupt content,
multiple references and unreferenced retained objects. A missing or corrupt canonical
object fails complete backup visibly. Uncataloged temporary/orphan bytes are reported
but are not represented as a complete native file backup. Physical garbage collection
and automatic repair are deferred. No operation removes a shared canonical object
merely because one area listing is removed. Restore reconstruction is an explicit
repair from independently verified backup custody.

C6 file exchange is live-only. C2 offline conference/direct-message exchange is unchanged;
file offers need a future explicitly authenticated offline container/custody contract.
File subscriptions, requests/FREQ, governance and external command Events are not
silently inferred from C6 file advertisements. Remote file-subscription management
is deferred; configured local file Dossiers are implemented.

A new remote publication cannot reopen an existing locally rejected file association.
An authorized local rescan is required to reconsider that rejection.

File distribution mutations take SQLite write ownership before reading decision
state. This avoids deferred-transaction read-to-write upgrade failures when native
Event/result writers overlap preparation. Existing per-link permits and the shared
preparation lock still coalesce exchanges. Diagnostics record a fixed operation,
error class and SQLite numeric code, never the underlying path-bearing error text.
