# FTN file networking

N7 implements the interfaces defined here before code. The
[M051 report](../research/m051-networking-n7-ftn-files.md) records acceptance.
Native `files`, `file_areas`, storage locators, lifecycle and SHA-256 remain the
only generic file authority. Native uploader/upload time, descriptions and caller
download accounting keep their existing semantics. FileEcho/FREQ transfers do
not become caller downloads, charges or access grants; network ingress/origin
and transfer activity are separate provenance. Network rows reference native file IDs and own
protocol provenance, admission, subscriptions and per-link delivery intent.
They must not create another caller catalog or bypass native integrity checks.

## Evidence and selected profile

Primary authority is FTS-5006.001 (TIC), FTS-1026.001 (BinkP), and FTS-0006.002
request-file conventions, contextualized by FTS-5005.003. The indexed private FTSC corpus is read-only.
Live retrieval of the first two documents failed on 2026-09-06; no claim of
freshly verified supersession is made. Synchronet TickIT/HatchIT/FREQIT/BinkIT
are secondary interoperability references, never copied implementation assets.

TIC parsing is bounded, case-insensitive for keywords, and accepts CRLF, CR or
LF. Output is CRLF. Area, Origin, From, File, CRC, Path and Seenby are required
by FTS-5006. NG additionally requires Size and a configured per-link password
for admission. Domainless four-dimensional addresses use only the authenticated
link's configured domain. To, when supplied, must identify the receiving AKA.
CP437 text is explicitly decoded/encoded; unrepresentable output rejects.
Repeated singleton fields reject. Ordered descriptions and Path remain ordered.
Unknown fields are bounded opaque provenance, never commands or configuration.

### Authenticated direct-hatch interoperability profile

The generic TIC parser/encoder remains strict. A separate receive profile handles
the observed HatchIT direct-hatch shape: exactly one PATH address, equal to both
From and Origin, addressed explicitly to the link's local AKA. PATH may have its
normal timestamp or exactly no timestamp; malformed/trailing timestamp data and
missing/multiple PATH entries reject. Seenby may independently be empty. This is
an interoperability exception to FTS-5006.001, not a claim of full conformance.
FSC-0028.001 documents historical address-only PATH syntax; current HatchIT emits
it, and TickIT/HTick source tolerates it. Seenby omission has implementation
evidence but is not permitted by the selected standard.

Only a currently authenticated configured BinkP link with complete configured
transport context and restricted artifact custody may use this profile. Exact
TIC password, peer, local destination, area/subscription, payload and loop checks
remain mandatory. Any cross-domain configured address overlap rejects this
profile, including local/remote AKAs and explicit route identities. Relayed
incomplete history and local-address loop evidence reject. A product banner
never grants authority. Existing strict APIs without that context do not acquire
the exception.

The parser records an untimed peer PATH separately from timestamped operational
history; it never supplies zero, file date or transport time as a remote PATH
timestamp. Received Seenby remains an empty set when absent. Immutable private
receipt metadata records authenticated peer/session, receive time, original TIC
artifact reference and the two independent omission flags. Original TIC bytes,
including password and all field spelling, reside only in the restricted native
network artifact store, with its existing quotas and cold-backup custody. Raw Pw
does not enter JSON staging, status, logs or outbound metadata. Legacy staged and
published metadata remains readable without new schema or invented provenance.

Ingress and Origin exclusions still prevent reflection; normal hash identity
suppresses native duplicates and re-fanout. Forwarding preserves timestamped
history and appends NG's actual processing hop. An untimed peer hop is retained
in private received provenance, not emitted with an invented timestamp. The
authenticated direct sender is included in outgoing Seenby as witnessed ingress,
alongside normal local/target history. No arbitrary received Seenby is invented,
and zero received entries never create routes or subscriptions.

File uses safe ASCII DOS 8.3 names. No paths, device names, leading dots, shell
metacharacters or alternate streams are admitted. Lfile/Fullname is metadata,
never a storage selector. Native names remain unchanged during hatch; a stable
transfer alias records any compatibility mapping. Replaces is retained as
metadata but does not delete or overwrite files. Magic received in a TIC never
grants FREQ eligibility. Archives remain opaque and no content executes.

## Authority interfaces

Typed expected-version mutations configure `FileEchoArea` (domain/tag to native
area), `FileSubscription` (link/tag, inbound permission and outbound membership),
file-link hold, global finite policy and explicit native-file FREQ eligibility.
The existing FTN link owns address/AKA, and existing BinkP policy owns endpoint
and transport credentials. A separate write-only TIC credential uses the same
private credential custodian. Operator capability checks precede mutations;
mutations and safe audit commit together. Bootstrap remains read-only.

`preview_hatch` selects an existing active, integrity-verified managed native
file and displays safe metadata and recipients. `hatch` rechecks expected native
and mapping versions and commits provenance plus distinct per-link queue work.
It does not create another native file or rewrite native descriptions.

`receive_file_artifact` accepts only an authenticated current BinkP session.
Incomplete artifacts remain private; complete TIC and payload arrivals pair by
link plus case-insensitive transfer filename, then exact size/CRC/SHA-256.
No native import occurs before full validation. Conflicting content under an
occupied pair name rejects; neither arrival replaces the other. Staging has
finite byte/count/age limits, survives cold backup, and never becomes a caller
area. Strong identity is mapped native area plus SHA-256 and size. Native import
and its recovery journal precede atomic provenance/fanout completion; retry must
recover the same native file, never infer success from a filesystem filename.

Routing excludes ingress, origin, local addresses, Path and Seenby. Each target
owns separate payload/TIC acknowledgement state. A target is complete only after
both matching BinkP M_GOT acknowledgements. Retry reuses accepted artifact truth;
successful targets are never requeued because another target failed. Hold stops
exchange while preserving future queue intent. Unsubscribe preserves queued
work for review. No cross-zone EchoMail gateway transformation is introduced.

FREQ is a distinct authenticated FTN service. A bounded `.REQ` contains explicit
safe filenames only; resolution consults explicit per-link native-file grants,
active public native areas and integrity-verified managed files. No host path,
wildcard, executable alias or automatic nodelist publication exists. Counts,
individual size, aggregate bytes and session time are bounded. Failures reveal
only semantic results. Requester responses may be queued for a subsequent
controlled poll; socket/session existence is not durable authorization.

Cold backup includes policy, subscriptions, credentials, provenance, private
staging and per-artifact delivery truth. Restore clears live session/claims and
preserves accepted work. Reconciliation must include file acknowledgements;
missing later history remains held. Restoring an older snapshot never implies
permission to resend accepted downstream work.

## Outstanding exact-FREQ recovery contract (schema 27)

The original request ID, peer, immutable request bytes/creation provenance and
per-name response receipts remain authoritative. BinkP M_GOT acknowledges custody
of the request file, not delivery of the requested file. The lifecycle is pending,
sent, acknowledged-but-unanswered, and complete only after every requested payload
has a native receipt. Partial responses remain individually visible.

A schema-27 request recovery row and append-only attempt links associate each
transport delivery with the SAME original request. Attempt one is the existing
delivery, including on migration from schema 26. Its acknowledgement is never
cleared. A manual `RetryRequest { request, expected }` operator action requires
NetworkRun, no active BinkP session, current exact peer/local policy and active
native destinations, unanswered names, and at least 900 seconds since the latest
acknowledgement. It queues only unanswered original exact names. It never inserts
another per-name request receipt or changes the existing pending-name unique index.

At most three request attempts, including the original, may exist. Each retains
the normal maximum of 12 transport tries; generic hold/release cannot reset FREQ
transport budgets. Exhausted requests grant no further sends; a legitimate delayed
response may still complete the original request. There is no automatic reissue
scheduler. Expected-version checks and a transaction prevent parallel attempts.

Attempt links, offered evidence and acknowledgements survive restart. A response
from any previously offered attempt attaches to the original exact peer/name
receipt; only one native result may commit. A queued reissue is suppressed when
all its names have already arrived. Wrong peer/name cannot satisfy a FREQ receipt;
ordinary bounded FileEcho payload staging remains a separate authority.

Cold restore holds request families as well as outstanding deliveries. Generic
queue release cannot bypass the family hold. Verified recovery evidence may
release only an identical family/receipt history; a snapshot missing later
attempts or responses remains held instead of resetting the retry budget. No
inferred receipt, deleted request, relaxed uniqueness or new unrelated request ID
is used to recover unanswered work.

## Cross-project reference

Bounded read-only FireComm review covered its Phase 10 transport architecture.
Adapted lessons: bounded workers, explicit handshake identity, secret exclusion
from projections, and acknowledgement before success. Terminal framing,
encoding, pacing and TLS choices remain FireComm-specific; no shared dependency
or FireComm change is introduced.

## Deferred optional scope

Remote FileFix/FileMgr, wildcard/magic FREQ, quarantine mutations, automatic
Replaces deletion, archive extraction, scanner execution and persistent partial
wire resume are deferred. No public-network onboarding, scheduler, CircuitNET,
B-022, doors, packaging or service installation is part of N7.

## Schema 26 and implementation map

`sf-net::tic` owns the bounded codec, CRC32, transfer-name mapping and exact
request grammar. `sf-core::ftn::files` owns typed admission/configuration,
publication, pairing, native import and file delivery receipts. Schema 26 adds
only the file adapter tables in `crates/sf-core/src/ftn/files.sql`; `files` and
`file_areas` retain metadata and bytes ownership. Native maintenance's internal
commit callback commits a new file and its publication/fanout in one transaction.
A callback failure removes only its own new destination and retains a semantic
maintenance recovery record. A crash at an earlier native journal boundary uses
existing native recovery and may require operator review; it never proves import.

File policy defaults disabled. Limits are 16 MiB per payload, 32 KiB per TIC,
64 MiB default staging (128 MiB hard maximum), 128 default staged artifacts
(256 maximum), and 24 hours default staging retention (60 seconds–7 days).
Staging expiration occurs on later arrival, in the same admission transaction;
there is no scheduler. No new arrivals means bounded stale rows may remain.
Configuration caps are 256 mappings, 1,024 subscriptions and 256 FREQ grants.
Publications plus deliveries have a 10,000-row lifetime admission ceiling;
retained provenance is not automatically pruned. Capacity exhaustion rejects
new work safely and needs an explicitly scoped future retention mechanism.
History retains 256 safe activity rows; cockpit lists are bounded. These ceilings
bound adapter storage; the operator's native file catalog has its existing policy.

A mapping's native area is immutable. Tags and native area mappings are unique.
An inbound grant and outbound subscription are separate booleans on an existing
link/area relationship. Missing rows deny. There is no remote self-subscription;
manual operator admission is the simple allowed/denied policy. Held subscriptions
still acquire future deliveries. Unsubscribe holds prior pending work, requiring
explicit review/release if later resubscribed. Config edits reject active BinkP
sessions. Static link edits cannot orphan retained file references.

The native filename stays intact on hatch. Safe existing 8.3 names become
uppercase transfer identities; longer names use the first eight SHA-256 hex
characters plus a safe extension, with Lfile retaining native metadata. A hash
prefix collision is not content identity: staged conflicts reject and native
filename conflicts receive a deterministic digest prefix. Native names are
currently ASCII and at most 64 bytes; CP437 long-name metadata is retained even
when it cannot become a native filename. Descriptions are bounded to 4,096 UTF-8
bytes and 20 source lines; empty inbound descriptions use the transfer name only
for the required native caption. Original TIC metadata remains provenance.

## FREQ receipt and authorization details

The implemented convention is a DOS-named `.REQ` artifact containing CRLF exact
filenames, carried on the existing authenticated BinkP session. NG emits the
remote net/node hexadecimal request name. FTS-1026 itself does not specify a
complete FREQ service; this is the conservative FTSC request-file convention,
not a claim of complete Wazoo session transport or wildcard/update semantics.
Replies use ordinary acknowledged BinkP files. Negotiated 1.1 permits a response
batch in the same session; a queued response may also require a later poll.
Schema 27 adds the retained request-attempt authority described above, implemented
in `crates/sf-core/src/ftn/freq.rs` and `freq.sql`.

An eligible native file must be active, SHA-256/size verified, on its area's
primary managed root, and explicitly granted to that link. Its area must be
active, `at-least`, and read security zero. External mounts, operator/private
areas and noncatalog data are excluded. Revalidation occurs when work is claimed;
revocation holds unsent responses. No alias is implicit: the grant's name must
match the file's transfer name. Request files default to 8 names/32 MiB per
request and authenticated session, capped at 32 names/64 MiB, within BinkP's
64-file/64-MiB session and timeout bounds. Whole-request failure queues nothing.
A request identity contains link, wire name, wire timestamp and exact request
bytes: retries reuse receipts, a later intentional request has a new timestamp.

The requester selects a native area through typed authority. Durable inbound
receipts authorize only that link/name; a complete response imports or reuses
one strong-identity native record. Unsolicited files remain untrusted staging.
A requested payload can also be paired with a later TIC, so its private staging
copy expires under the same bounded policy. FREQ receipt import never creates
FileEcho fanout. Restored requests/responses are durable work, never live sessions.

## Trust, backup and failure handling

TIC passwords use a separate `tic-credentials` namespace in the existing private
custodian (owner-only directory/files on Unix), excluded from Debug, request
fingerprints, status, logs and audit. The transport receives a credential only
for verification or generating the target's outbound TIC. Staging stores parsed
metadata after authentication with Pw removed; rotating/clearing a credential
reserves the existing two BinkP worker slots, then invalidates its incomplete
TIC controls before publishing the credential. New inbound/outbound sessions
cannot enter that interval; active workers make rotation conflict. Stored
payloads retain no trust.
A failed credential write may therefore require TIC resubmission, conservatively.

BinkP M_GOT acknowledges durable custody. Invalid TICs produce a bounded safe
quarantine activity record rather than native import; custody acknowledgement
is not a claim that remote application policy accepted the file. Successful
FileEcho delivery requires both payload and TIC custody acknowledgements.
Interrupted wire bytes stay in bounded session memory and vanish on disconnect;
retry starts from zero. Fully received incomplete pairs survive database backup.
Snapshot restore removes live claims and holds uncertain pending work. Existing
same-board reconciliation merges later independently recorded acknowledgements
without reducing any acceptance flag. If surviving later history is unavailable,
held work needs review; no restoration can reconstruct an unknown remote receipt.

Neither archive extraction, executable hooks, scanner commands, raw path reveal
nor quarantine reprocess/discard exists in this adapter. Storage/SQL failures
return semantic errors without file contents, credentials or host paths. Native
storage/journal recovery remains the source for filesystem repair. See
[backup/recovery](../sfng-backup-restore.md) and the
[native file specification](../sfng-file-system.md).

Reproduction: run workspace gates and the focused `file_tests`, `tic::tests`,
`schema_twenty_six_upgrade_and_failure_are_atomic` migration test and `four_daemon_fileecho_hatch_freq_and_cold_restore`
integration test. Socket tests require an environment permitting loopback binds.

Twelve unsuccessful delivery attempts hold pending work with `file-attempt-limit`.
An audited explicit release renews the attempt budget (the displayed attempts
counter starts again), without clearing either artifact acknowledgement. Only
missing artifacts are offered. A held subscription alone does not consume attempts.
