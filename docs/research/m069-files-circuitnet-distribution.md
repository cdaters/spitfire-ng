# M069 — Native Files and CircuitNET file distribution

C6 is COMPLETE / ACCEPTED. This source is the sanitized C6 publication. This is the
rights-safe modern implementation report. No historical software, raw samples,
private research corpus or acceptance credentials are included.

## Historical input and modern authority

A bounded local-primary-source review documented SPITFIRE File Areas/security,
caller upload/download and descriptions, configurable duplicate filenames,
FILE_ID.DIZ suggestions and external upload-checking workflows. External utilities
used a staging directory, archive tools and scanners before returning approved
files to board custody. These are documented historical concepts; adopting a
native durable state machine is a modern engineering decision. Exact utility
behavior across every historical version remains unknown. No historical binary
was executed or reverse-engineered for C6.

Existing native File Areas, catalog, access controls and transfer services are
extended, not replaced. CircuitNET references approved native objects and does not
own a second payload store. The following is the modern implementation record.

## C6 implementation and acceptance record

The following records accepted implementation and completed verification. Deferred operations do not advertise protocol capabilities.

| # | Requested result | C6 implementation / evidence |
| --- | --- | --- |
| 3 | Schema | 32 → 33; migration and snapshot validation cover native admission and file distribution. |
| 4 | Historical Files | Bounded primary-source findings above: native areas/security, uploads/imports/descriptions, duplicate filenames, DIZ and external validation workflows; not a DOS utility port. |
| 5 | Native architecture | Existing `files`, `file_areas`, FileStorage, FileBackend and transfer authority remain canonical. |
| 6 | Area authority | Existing durable identity/access/download/upload settings plus explicit safety policy, approval and network mapping. |
| 7 | File objects | Native catalog association plus immutable content, source/original filename, inspection, scan, admission and history; CircuitNET publication provenance references that object. |
| 8 | Content store | One native SHA-256 content object with hard-link area references; no network-specific payload store. |
| 9 | Hash identity | Streaming SHA-256 and size verification; publication identity remains separate. |
| 10 | Original bytes | Preserved, read-only content custody; inspection/scanning/DIZ do not rewrite them. |
| 11 | Admission | Inspecting → Quarantined / PendingApproval / Published; explicit rejection retained. Incomplete admission cannot become caller-visible. |
| 12 | Duplicates | Exact content reuse; same area/name/hash returns existing; duplicate DIZ suggestion reported without deletion. |
| 13 | Filename collision | Same name with different content fails explicitly; rename is required, no overwrite. Modern native basename limits remain, without an 8.3 assumption. |
| 14 | Recognition | ZIP, TAR, GZIP, 7z, RAR, BZIP2, XZ, Zstandard and limited ARC/ARJ/LHA/LZH markers; recognition is not successful inspection. |
| 15 | Inspection | Bounded ZIP, TAR and GZIP/TGZ. Unsupported containers quarantine. |
| 16 | Creation | No native archive creation action implemented. Synthetic test builders are fixtures only. |
| 17 | Recompression | Deferred; never performed during admission. |
| 18 | ZIP | Classic bounded directory, Stored/Deflated members, CRC validation; ZIP64 outside C6. |
| 19 | 7z | Recognized; inspection unavailable, quarantined. |
| 20 | RAR | Recognized; inspection unavailable, quarantined; generation not required. |
| 21 | TAR/compressors | TAR regular files/directories and GZIP/TGZ inspected. BZIP2/XZ/Zstandard recognized only. PAX/GNU extensions/sparse records rejected. |
| 22 | Historical archives | Limited recognition only; no legacy decoder or binary execution. |
| 23 | Type detection | Magic/parser structure, recorded separately from claimed filename; common executable/image/PDF signatures recognized. Archive extension mismatch quarantines. |
| 24 | Traversal | No member name becomes an extraction path; reject traversal, absolute/drive paths, backslashes, links and conflicting names. |
| 25 | Expansion limits | Default source 64 MiB, expanded 256 MiB, member 64 MiB, 1,024 members, ratio 100, 60-second inspection budget; configurable bounded ceilings. |
| 26 | Nesting | Shared cumulative budget, default depth 3 with root depth zero; incomplete inspection quarantines. |
| 27 | Encryption | Password/encrypted archives cannot be declared inspected/clean; quarantine. |
| 28 | DIZ | Root-level case-insensitive FILE_ID.DIZ, 16 KiB bound, original bytes and UTF-8/CP437 provenance retained; NUL/malformed content rejected. |
| 29 | Description | Explicit Use/Edit/Ignore; terminal controls filtered, native text limits enforced; uploader/operator description is not silently replaced. |
| 30 | Derivatives | Separate future objects; branding/repacking deferred, original remains authoritative. |
| 31 | Scanner | Typed provider trait scans bounded streams and returns structured provenance/results. |
| 32 | ClamAV | Optional loopback ClamD INSTREAM provider, no shell/executable invocation; mock stream tested. Actual ClamAV unavailable and not installed. |
| 33 | Results | Clean, MalwareDetected, Suspicious, ScannerUnavailable, ScannerError, Unsupported, SkippedByPolicy. |
| 34 | Required scanning | Anything other than Clean prevents publication. Optional/disabled policy is explicit and does not label skipped/error results clean. |
| 35 | Quarantine | Durable unpublished custody, reason/report/history retained; ordinary download and export blocked. |
| 36 | Review | Capability-checked approve/reject; existing review action shares the admission fence and review timestamps. Unsafe content cannot be approved around it. |
| 37 | Rescan | Re-evaluates retained original under current local policy; required failures stay quarantined. |
| 38 | Caller UX | Existing area browse/search/description/download and X/Y/ZMODEM/Telink retained; completed uploads enter native admission, review message states unavailable until reviewed. |
| 39 | sfconfig | Cold-board `files` area/settings/import/list/status/review/rescan/DIZ/integrity commands; CircuitNET file mapping, subscriptions, status and bounded retry. |
| 40 | sfmonitor | Native publication/pending/quarantine/scanner counts and CircuitNET file queue/peer health in existing operator projections; no payload contents. |
| 41 | Backup/restore | Database plus one copy of each cataloged native payload; validate hashes/sizes, rebuild area links, preserve safety/history/queue/receipts. Missing content fails visibly. |
| 42 | Consistency | Integrity reports verified/missing/corrupt/unreferenced content and reference counts; no automatic destructive repair. |
| 43 | Deletion | Listing removal does not delete shared content. Physical GC deferred; legacy moves cannot carry approval into another area's policy. |
| 44 | Protocol | CIRCUITNET-NG 1.3, optional `file-distribution` and `file-hash-have`; no resume capability. Older peers retain conference/control phases. |
| 45 | Area codenames | Explicit network codename ↔ native area mapping, send/receive/size policy; no local IDs on wire. |
| 46 | File Dossiers | Typed per-neighbor file subscriptions separate from message subscriptions; both outbound recipient and incoming source authorization required. Remote file-subscription control deferred. |
| 47 | Envelope | Network/publication ID, original origin, codename, SHA-256, size, filename, description, timestamp and forwarding path. No local paths or account identity. |
| 48 | Transfer | FileOffer / FileWant / FileReceipt in the same authenticated TLS session, followed by bounded binary payload records. |
| 49 | Streaming | At most 64 KiB chunks, four-byte big-endian length and zero terminator; metadata ≤16 KiB, eight publications per direction, network file ≤64 MiB. |
| 50 | Recovery | Interrupted payload discarded and restarted from zero; final hash/size required. No offset resume claim. |
| 51 | Hash-have | Receiver verifies existing canonical payload before Have, reuses bytes and enforces current local admission policy for a new publication. |
| 52 | END → HOST | Real independent daemon transfer and durable native import; END cannot transit unrelated publications. |
| 53 | Sibling | END1 → HOST1 → END2, original origin retained and no reflection. |
| 54 | Cross-ROOT | END1 → HOST1 → ROOT1 → HOST2 → END3 with configured file subscriptions. ROOT-origin downward distribution also covered. |
| 55 | Filtering | Unsubscribed destination gets no new publication; no automatic mapping or subscription creation. |
| 56 | Duplicate publication | Durable publication fingerprint and receipt prevent duplicate native import/forwarding. Same payload may support another metadata association. |
| 57 | Hash mismatch | Durable rejected receipt, no publication; remote metadata cannot override integrity failure. |
| 58 | Partial delivery | Each neighbor has independent delivery truth; completed branch remains complete, failed branch remains pending. |
| 59 | Replay / lost ACK | Replay returns durable receipt; intermediate acceptance with lost response converges without another import/transfer. Conflicting reuse fails closed. |
| 60 | Events | C5 Exchange moves messages, controls and files; prompt queue preparation independent of Manual/Immediate/Scheduled/Hybrid timing. Existing permits/coalescing/hold/retry apply. |
| 61 | Receiver safety | Local policy always applies; remote clean claims are not an admission bypass. Scan claims are not sent in C6. Quarantine/pending/rejected receipts differ from published. |
| 62 | Added tests | 19 added tests: native Files/archive/scanner/migration/wire/fanout and one expanded six-daemon acceptance. |
| 63 | Workspace total | Public: 753 passed / 0 failed / 7 existing ignored, 6 doctest groups. |
| 64 | macOS acceptance | Six independent Apple Silicon loopback daemons; synthetic archives, harmless scanner mocks, files/Events/C4/recovery/backup. Expanded current-source campaign passed in 101.73 seconds; full workspace acceptance also passes. |
| 65 | Localization | en-US 1.30.0 / 1,404 messages, 22 new strings; operator IPC 16. |
| 66 | Files manual | [Native Files manual](../manual/files.md). |
| 67 | CircuitNET manual | [File distribution workflow](../manual/circuitnet.md#file-distribution-c6). |
| 68 | Technical docs | [Custody/support matrix](../technical/files-custody.md), [adapter](../technical/circuitnet.md), [1.3 transport](../technical/circuitnet-transport.md#c6-file-distribution-phase-13). |
| 69 | Static gates | PASS: 142 source headers, fmt, all-target Clippy with warnings denied, diff check. |
| 70 | Markdown/links | 145 Markdown files; final local-link check has zero issues. |
| 71 | Privacy/provenance | Wire metadata excludes credentials/local paths; audit and monitor exclude contents. Raw historical artifacts stay private; descriptions use safe text paths. Transfer encryption is distinct from File Area visibility. |
| 72 | cargo-audit | Unavailable; no audit success claimed. |
| 75 | Public delta | 58 paths: 13 added / 45 updated; 41 changed source paths byte-identical to the canonical implementation. Public gates pass. |
| 76 | Legacy CNP/CND | NONE. |
| 77 | FTN FileEcho/TIC reuse | NONE in CircuitNET. Existing FTN adapter retained; explicit C6 safety-area handoff limitation documented. |
| 78 | CircuitNET request/FREQ | NONE. |
| 79 | Conference governance | NONE. C5 drafts remain proposals. |
| 80 | Production changes | NONE. No production access was needed. |
| 81 | External CircuitNET traffic | NONE; disposable loopback only. |
| 82 | Exact next action | Stop for review of C6 and its documented limits. Do not begin C7 without review. |

## Reproduction and boundaries

The committed six-daemon acceptance is
`crates/sf-bbs/tests/support/circuitnet_files.rs`, included by `circuitnet_live`.
Run it with the workspace tests on a machine permitting disposable loopback sockets.
Optional `SPITFIRE_C6_EVIDENCE` selects a new private evidence directory; omit it for
a temporary directory. No actual malware, historical executable or external BBS is
used. Unit tests use synthetic structures and scanner providers. The deterministic ClamD mock stream validates framing and result classes without installing software.

C6 keeps existing FTN FileEcho/FREQ protocol behavior in legacy areas. Its atomic
immediate-publication callback rejects explicitly configured C6 safety areas until
a later adapter reconciles staged admission with FTN receipts. CircuitNET has its own
typed publication receipts and therefore can expose quarantine/pending outcomes now.
This limitation is intentional and is not an FTN redesign or a scanner bypass.

File requests, remote file-Dossier mutation, offline file packet exchange, offset
resume, archive creation/recompression/branding, physical payload GC and unsupported
archive decoders remain deferred. C2 offline conference exchange is unchanged.
A future official network may explicitly distribute documents, catalog releases,
node kits and approved utilities through this adapter; C6 does not configure that
or finalize file-area governance.

Files are board-wide SPITFIRE NG infrastructure. CircuitNET consumes native Files;
original artifacts remain immutable and derivatives would be separate. Archive
recognition/inspection/creation/recompression are distinct. Scanner failure is not
clean; remote scan claims cannot bypass local policy. CircuitNET file distribution
is independent of FTN FileEcho. C1–C5 remain accepted, native messages remain canonical,
and transport encryption never promises secrecy after publication. No production
systems changed and no external live network traffic occurred.
