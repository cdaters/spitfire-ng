# QWK offline mail and the shared adapter boundary

Current source implements caller QWK offline mail (M045/N1). Native SPITFIRE NG
messages remain the only message authority. QWK is an interchange adapter; there
is no separate QWK message store and no SMB dependency or compatibility layer.
QWK networking, DOVE-Net, FidoNet/BinkP, network scheduling and network operator
views are not implemented. The downloadable 0.1.0 Development Preview predates N1.

## Authority and implementation

The binding design is [M044](../research/m044-networking-foundation-gate.md),
particularly its N1, encoding, duplicate, transfer and recovery contracts.
[Native messages](../sfng-message-system.md) retain MessageId, ConferenceId,
immutable CP437 subject/body bytes, per-delivery identity/version, authorization,
number allocation, mutation and read receipts. Existing payloads are not silently
converted to Unicode. Public caller handles are Unicode and undergo explicit
CP437 encoding at the adapter boundary. Login names and private real names do not
enter packets.

`sf-net::qwk` owns pure, bounded ZIP inspection, record decoding/encoding, CONTROL
serialization, CP437 framing, QWKE long headers and Microsoft binary index
validation. Its Message is asserted wire information, never posting authority.
`sf-core::network` owns typed selection, mappings, manifests, identity/receipts,
import/export transactions and native message calls. `sf-bbs::DiskArtifactStore`
owns filesystem custody and board-scoped admission. The existing session worker
and caller/transfer modules execute the on-demand workflow; no scheduler or shell
archiver is involved. Every new abstraction has an N1 consumer. Later QWK network
partners and DOVE profiles can reuse this codec without duplicating it or gaining
caller posting privileges. FTN code is absent.

## Schema 20

Migration 19 → 20 runs within the existing transactional migration mechanism.
It preserves all native payloads, IDs, numbers, audience and receipts. It adds:

| Authority | Purpose |
|---|---|
| `network_artifacts` | SHA-256 identity, bounded size, creation time, pending/complete custody journal |
| `network_area_mappings` | Stable native ConferenceId ↔ explicit 16-bit QWK conference key, profile and version |
| `qwk_requests` | Authenticated caller, configured board ID, selection, immutable artifact and delivery state |
| `qwk_manifest_conferences` / `qwk_manifest_members` | Selected policy, pointer/reset versions, included native IDs/versions, high waters and wire references |
| `network_import_receipts` | Caller/profile/canonical packet/member ordinal identity, exact member digest/offset, artifact, original wall time, native result and finite reason |
| Existing caller pointers | Version and explicit-reset generation; normal read advancement increments the version |
| Existing transfer records | `message-packet` purpose and artifact relation, independently of file-catalog accounting |

Only initial conference mappings are seeded; migration invents no packet history.
The initial native conference number supplies a *one-time* mapping value, not a
permanent equivalence. A native renumber cannot retarget an issued wire identity.
New conferences are mapped only where both keys are available; a conflict holds
export rather than guessing. Missing/inactive/inaccessible areas never grant access.
Mapping policy is native typed database configuration, not volatile TOML lifecycle.
The sole new TOML field, `caller.qwk_board_id`, is stable operator policy.

## Selection, delivery and pointers

The historical Messages L → D/U/S/Q workflow is retained. D offers New/To You
and All/Selected/Your Queued Conferences. Every candidate passes native online
read authorization, including private deliveries. Unsupported messages hold the
build; export does not skip them and advance beyond them. Generated headers use
public handles, recipient, subject, conference-local number and permitted reply
reference. Packet manifests freeze actual native message versions and policy.

New uses the native conference last-read pointer. To You selects authorized direct
mail, including already-read mail. Construction changes neither pointer nor received
state. Binary transfer completion changes the request to delivered. Only the
caller's subsequent affirmative pointer confirmation advances native pointers
monotonically and marks *included addressed deliveries* received. Declining is a
preview. The system does not claim that transfer proves the caller read every line.

Failure, cancellation or disconnect cannot advance pointers. Regeneration before
confirmation includes the same eligible mail. Overlapping packet completion never
moves a pointer backward. Explicit S resets use compare-and-swap and may move a
pointer backward within the conference range. A completion observing a reset is
stale; the caller rebuilds and confirms against current state. Message deletion,
changed audience, access or conference policy invalidates the prepared manifest.
Reauthorization occurs at transfer admission and confirmation, including inside
the final transaction. It cannot revoke bytes already legitimately delivered.

## Wire profile

Format authority is Patrick Lee's [QWK layout 1.6](https://wmcbrine.com/mmail/specs/qwklay.html),
Mark Herring's [1stReader programmer reference 2.0](https://wmcbrine.com/mmail/specs/qwk1st.html)
and Peter Rocca's [QWKE 1.02](https://wmcbrine.com/mmail/specs/qwke.html).
Primary SPITFIRE 3.7 §24.4 and companion LAKOTA 2.0 determine caller outcomes;
secondary software does not determine the format. These references were read,
not copied into project source.

Exports are ZIP Stored containers, accepted input is ZIP Stored/Deflate. Standard
CONTROL.DAT and MESSAGES.DAT accompany conference NDX files, PERSONAL.NDX when
needed, DOOR.ID and selected TOREADER.EXT metadata. CONTROL's conference count is
count-minus-one, following Lee and independent reader acceptance. Optional board
screens, NEWFILES, attachments and executable content are not exported.

Records are exactly 128 bytes. All offsets in code are zero-based. The message
header contains status at 0, number at 1, date at 8, time at 16, To at 21, From at
46, subject at 71, reply reference at 108, decimal block count at 116, active marker
at 122 and little-endian conference at 123. Blocks include the header; body data
spans as many records as required. Indexes use Microsoft binary floating record
positions, validated against actual header boundaries, never IEEE casts.

REP requires configured BBSID.MSG and matching first-record identity. Its ASCII
number field is the conference, matching the binary conference field. The outer
uploaded name cannot authenticate a board or caller. Classic export refuses long
fields; current caller export selects the QWKE CP437 long To/From/Subject subset.
Long fields are explicit body-prefix extensions, not truncated recipient lookup.
Unknown attachment/routing/control profiles fail safely; this is not a claim to
implement every QWKE extension. LAKOTA ADD/DROP controls use native queue services
and receipts, never public message posting. A real local Lakota handle takes
precedence. Native conference 1 cannot be dropped. Original BBSID.LMR bytes are
preserved with an unsupported result: their exact historical layout remains
unresolved, as M044 explicitly permits for N1. No LMR decoder is invented.

## Encoding and time

Native CP437 bytes remain authoritative. Export terminates the final text line. The codec changes CR/LF to the QWK 0xE3
newline marker and reverses it on import. Unsafe controls and literal CP437 pi
(also 0xE3) are refused; pi cannot silently become a newline. Public Unicode
metadata that cannot be represented in CP437 is deterministically refused with
caller feedback, not replaced into a colliding identity. QWKE extends field
length, not character repertoire. Unknown raw bytes and padding remain in the
immutable source artifact. Logical trailing-space length is ambiguous in QWK and
is not claimed lossless.

The selected QWKE profile also holds a message whose first nonblank body text
looks like To/From/Subject, Title or @Subject metadata. Independent MultiMail
acceptance demonstrated that a blank separator does not prevent that reader from
reinterpreting such text as identity/header fields. No agreed lossless escape is
claimed; refusal preserves the native message and leaves pointers unchanged.
Header-looking lines following ordinary body text remain normal text. Standard
QWKE field names are accepted ASCII-case-insensitively.

Export serializes board-local wall time, minute precision, within the declared
1980–2079 two-digit-year window. Impossible/out-of-window dates fail. REP wall time
has no reliable timezone offset: retain it as unresolved source provenance. The
native post timestamp is trusted receipt UTC. No current DST offset is guessed
for an ambiguous historical wall time.

## Import identity and atomicity

The archive and all record framing are validated before any message import.
Each semantically valid reply uses current authenticated CallerId, current native
posting permission, recipient resolution, private-mail policy, line/byte limits
and ordinary native number allocation. Packet From is untrusted and cannot make
another caller or Sysop the author. Reply references resolve only through an
unambiguous retained caller/board manifest and current parent authorization.
Ambiguity holds the reply rather than choosing the latest packet.

A member transaction commits the native message, normal posting statistics and
immutable import receipt together. Failed receipt insertion rolls back the post.
Semantic failures receive durable finite rejections while other members may
succeed. Infrastructure failure stops the attempt; retry resumes from committed
receipts. Outer framing failure imports nothing.

Identity combines caller, profile, canonical sorted member names/uncompressed
bytes, ordinal and exact record digest/offset. ZIP recompression and timestamps do
not defeat replay detection. The same message in a different packet is held as a
possible duplicate. Explicit caller-confirmed *new submission* uses a server intent
token and can deliberately create a repeated post. Ordinary retry never does.
This is not a body-hash identity or an exactly-once claim across restoration of a
backup predating an import. Receipts are retained without automatic expiry or
pruning; capacity exhaustion refuses intake. Message deletion does not erase them.

## Resource, archive and custody boundaries

| Resource | N1 bound / response |
|---|---|
| Compressed archive | 16 MiB; at most 1,023 validated trailing XMODEM padding bytes on receipt |
| Expanded archive / members | 64 MiB / 1,024; 100:1 per-member expansion ratio |
| Message records | 1,000 messages; 64 KiB native body, 72-byte native subject, current caller/conference line policy |
| CONTROL.DAT | 256 KiB |
| Export candidates | 10,000 across the selection; pressure refuses build without pointer movement |
| ZIP path / metadata | One DOS 8.3 component, no depth; 4 KiB extra fields, 1 KiB comments |
| Retained export requests/manifests | 10,000 requests / 256 MiB including indexes; admission refuses without advancing pointers |
| Durable artifacts | 1 GiB and 10,000 files including unknown files; pending reservations count |
| Duplicate history | 2 million rows / 512 MiB including indexes, checked with bounded admission reserve |
| Packet work | Two concurrent board jobs, immediate capacity feedback; no unbounded work queue |

ZIP inspection rejects traversal, absolute/drive/device names, symlinks, directories,
encryption, ZIP64/multidisk, executable prefixes, nested archives, case collisions,
inconsistent local/central headers, overlapping member regions, corrupt CRCs,
truncation, impossible counts and excessive expansion. No filesystem extraction
occurs. Complete transfer is required before parsing; partial uploads remain in
bounded transfer memory and are discarded on failure/disconnect. The standard
binary engines and protocol chooser are reused, with no file-catalog insertion,
ratio credit, public upload registration or second transfer implementation.

Complete artifacts are retained private evidence, not live temporary delivery
files. SYSTEM/network-artifacts is daemon-owned; POSIX permissions are 0700 for
the directory and 0600 for created files. Names are generated SHA-256 identities.
The database journals pending ownership *before* exclusive file creation, then
marks complete after sync. On exclusive startup, only explicitly journaled pending
files are removed; unknown files remain untouched and count toward capacity.
Referenced complete artifacts are hash/size checked. Prepared requests and unfinished packet transfer records become
failed on restart and restore, so a killed daemon cannot leave phantom active
transfers blocking cold backup. Partial buffers have no surviving filesystem authority.
Malformed artifacts produce content-free operational evidence; N1 does not create
a general network quarantine subsystem.

Cold backup includes the SQLite history and retained artifacts through the existing
SYSTEM inventory. Incomplete custody blocks backup until startup recovery;
restore verifies bytes and recreates restricted storage. No live packet/worker
ownership is restored. Old schema snapshots restore at their recorded schema and
migrate only on writable startup. The ordinary board ownership/generation boundary
continues to apply. A backup older than an import cannot preserve that later receipt.

## Observability and validation

`message.qwk.*` events describe generation, upload, import, rejection and duplicate
outcomes using finite codes and existing caller activity attribution. They contain
no bodies, private recipients, login/real names, packet dumps or filesystem paths.
Caller actions are operational activity, not operator-control audit. Existing
Activity/Errors projections can show them; there is no Networks view/configuration.

The [M045 report](../research/m045-networking-n1-qwk-offline.md) records reproducible
codec, native authority, migration, recovery, transfer and independent MultiMail
acceptance. Windows live QWK acceptance is **DEFERRED — REAL WINDOWS ENVIRONMENT
REQUIRED**. Source portability is preserved without a Windows interactive claim.
