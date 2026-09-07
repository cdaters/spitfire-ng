# CircuitNET NG native foundation

C2 defines an independent, development/offline public-conference adapter around
native SPITFIRE messages. The interface gate was written before implementation. C1/M063 remains
private historical authority; this independently authored contract contains no
legacy packet layout or proprietary implementation. No legacy compatibility is claimed. C3 supplies the additional
[live transport boundary](circuitnet-transport.md) without replacing this foundation.

## Interface gate

### C4 interface gate

C4 adds optional typed `destination` to native CircuitNET publication metadata.
Absence retains byte-identical C2 encoding. A destination selects the unique path
in the profile's validated tree; only its next direct neighbor receives queue
intent. Every hop verifies the complete origin-to-destination path. Intermediate
nodes retain native transit records, without placing them in caller conferences.
The destination must have an active public receive mapping. Directed traffic does
not require or modify broadcast Dossiers. This deliberately modernizes C1's
route-driven subscription learning into explicit operator-controlled subscriptions.
Normal fanout retains C2 Dossier rules. Replies retain typed parent identity but do
not inherit a destination. Historical subject parsing remains deferred.

Typed controls use a separate namespace and request/result frames on the existing
mutually authenticated session. Subscribe, Unsubscribe and QuerySubscriptions
identify network, requester, direct parent target and origin-scoped random request
ID; mutation operations require a codename. No caller identity or message payload
authorizes controls. Only a configured direct child can request its own Dossier
at its parent; ROOT has no upstream target. HOST follows the same upstream rule.
Local operator management of direct neighbors remains available.

The default remote policy is require-approval; auto-approve and deny are explicit
operator choices. Requests, authenticated source, canonical request hash, time,
pending/terminal result and decisions are durable. Approval revalidates policy and
codename availability in the same transaction as Dossier mutation and result.
Identical retries return current durable results; conflicting reuse rejects.
Unknown/unavailable codenames never create conferences or mappings. Results expose
only finite codes and the requester's own subscription list. Pending results are
retried in subsequent finite polls; terminal results stop automatic resubmission.

Protocol 1.2 negotiates `directed-routing` and `remote-dossier-control` independently.
Minors 0–1 keep their existing phase sequence and conference encoding. Directed
work is withheld from peers without its capability; controls are sent only after
mutual capability agreement. Offline directed envelopes share C2 custody and
receipts. Remote controls over offline files are deferred because trusted file
custody does not provide the live certificate identity anchor.

CircuitNET transport is encrypted. Conference messages are readable by users
authorized for the destination conference after delivery. Directed routing does
not provide private messaging or end-to-end encryption. Transit is accessible to
intermediate operators under native BBS access controls. Other local/private BBS
messages are access-restricted unless an explicit feature supplies end-to-end
encryption; C4 supplies no such feature.

The network slug is 1–32 lowercase ASCII letters/digits/hyphens, beginning and
ending alphanumeric. Node IDs are 1–8 ASCII alphanumerics, normalized uppercase;
codenames are 1–8 ASCII alphanumerics or internal hyphens, normalized uppercase.
Whitespace, wildcards, paths and Unicode identities reject. Display names are
bounded text. IDs never become filenames. Profiles namespace every identity.

A profile owns one local node and a bounded, explicitly configured tree (at most
128 nodes). ROOT has no parent; END has exactly one HOST/ROOT parent and no
children; HOST has zero or one HOST/ROOT parent. Exactly one top node is required;
a parentless HOST permits the isolated three-board fixture. Every ancestor must
exist; cycles, duplicate IDs and disconnected trees reject. Each board's configured
tree supplies branch authorization; there is no learned route authority. Local
identity and topology become immutable after publication/import history exists;
disable remains possible. Re-enrollment/moving a live branch requires a later
migration contract.

Typed expected-revision transactions configure profiles, mappings and Dossiers.
Mappings associate a codename with one native public conference per profile;
receive/send flags are explicit. Dossiers separately grant direct-neighbor delivery.
Parent transmission also requires a Dossier, avoiding implicit subscriptions.
Unsubscription holds unsent work; completed receipts remain immutable. Re-subscribe
affects new publications; held historical work needs explicit policy-valid retry.

C2 uses HandleAllowed as its network requirement. Native conference/board policy
may strengthen it through M061.1. Posting previews include CircuitNET network/area
scope before save. Export uses immutable posted author and frozen UTF-8 sender,
never current private names. Adding a mapping cannot retroactively authorize older
posts. Imported authors are external assertions with no local caller ownership.
Only native public standard posts and same-profile imports qualify; other adapters
are never implicitly bridged. Native messages remain the sole payload store.

## Exchange and durability

The codec in sf-net has no database/filesystem authority. The service in sf-core
owns validated policy, native import, identity/provenance, queue and receipts.
The host owns bounded file I/O and the existing restricted artifact store.
C3 live transport invokes the same prepare/import/acknowledge boundary with
independently established neighbor authority. C2 explicitly requires operator-trusted
offline custody and exact expected neighbor; files carry assertions, not credentials.
This is suitable for isolated development exchange, not admission of Internet files.

Version 1 JSON is UTF-8, strict about unknown fields, with deterministic struct
field order. A batch contains format/version, network, sender, intended neighbor,
and up to 32 messages; maximum encoded artifact size is 4 MiB. A message contains
origin-scoped random 128-bit identity, origin, codename, frozen author, subject,
body, Unix timestamp, optional parent identity, and bounded forwarding path.
No local database IDs appear. Author is at most 120 UTF-8 bytes / 60 Unicode scalar values, subject 72,
body 64 KiB; no controls except CR/LF/tab in bodies. The path must equal the
configured unique path from origin to sender and exclude the receiver. A conflicting
identity/content pair rejects atomically. Parent references are retained unresolved
until the parent arrives; linking requires the same profile and codename and must
not create a native thread cycle.

One native publication creates per-neighbor queue intent. Generic network queue,
artifact custody, attempt history and sender snapshots are reused; CircuitNET
metadata owns namespace, provenance and recipient decisions. Prepare never means
accepted. A receipt acknowledges durable native acceptance of the exact artifact
hash and members. Import and fanout commit together before receipt issuance.
Replays record receipt observations, create no native duplicate and never re-fanout.
Lost receipts are recovered by importing the identical artifact again. Content
fingerprints exclude forwarding path but include origin, codename and parent.
Completed delivery never becomes pending because a sibling failed.

Queues and retained history have finite admission ceilings; exhaustion rejects
new work rather than deleting duplicate history. No history pruning is provided.
Cold backup uses the existing native snapshot and artifact manifest. Restore
preserves acknowledged truth and holds uncertain outbound work for explicit retry;
random message IDs avoid restored counters reusing identities. An old snapshot
cannot invent later acknowledgements; replay to the receiver safely recovers them.

C3 defines authenticated enrollment, version/capability negotiation, bounded
batches, acknowledgement/retry state and channel protection in its separate wire contract.
Legacy codecs are a separate deferred compatibility boundary, with no empty codec
or speculative C3 tables. Third-party adapters can implement this envelope without
SPITFIRE database knowledge. Conference Health remains future native analytics
across all conferences. Private mail, directed public routing, files, governance, catalog creation,
remote Dossier commands and live transport are outside C2; live transport is C3.

## Schema 29 and implementation map

Migration 28→29 rebuilds only the generic queue adapter discriminator to admit
`circuitnet`, preserving existing QWK/FTN queue IDs and foreign keys. The migration
is transactional, restores foreign-key enforcement after failure, and checks all
foreign keys before commit. No preexisting message gains a CircuitNET publication.

| Relation | Durable authority |
| --- | --- |
| `circuitnet_profiles` | Typed validated profile/tree JSON and CAS revision; no secrets |
| `circuitnet_mappings` | Network/codename/native conference, send/receive and revision |
| `circuitnet_dossiers` | Direct neighbor/codename subscription and revision |
| `circuitnet_messages` | Native message reference, CircuitNET identity, origin, ingress, path, parent and fingerprint |
| `circuitnet_deliveries` | Unique publication/neighbor intent referencing generic queue work |
| `circuitnet_batches`, `circuitnet_batch_members` | Immutable artifact and exact queue-member manifest |
| `circuitnet_imports` | Durable artifact acceptance and duplicate counts |
| `circuitnet_receipts` | Append-only import/replay/acknowledgement observations |
| `circuitnet_changes` | Append-only operator mutation journal without message/profile content |

The seven historical publication/delivery/receipt relations reject UPDATE/DELETE.
The generic `network_outbound_queue`, `network_delivery_attempts`,
`network_sender_snapshots` and `network_artifacts` retain their existing authority.
Artifact files remain under the host's restricted, content-addressed store and
participate in its backup manifest. No ID derived from an input filename is trusted.

Implementation: `sf-net/src/circuitnet.rs` owns the strict codec and identity/tree
types; `sf-core/src/circuitnet.rs` owns native services; `sf-bbs/src/circuitnet.rs`
owns file I/O and explicitly authorized cold-board commands. `sfconfig circuitnet`
uses that same host service. C2 has no online operator protocol extension or
sfmonitor UI. The process/lock/capability boundary precedes all mutations.

Admission is bounded to eight local profiles, 128 topology nodes, 4,096 Dossier
associations per profile, 1,000 pending deliveries per neighbor, 100,000 retained
CircuitNET history/intent rows and the existing 512 MiB artifact budget with a
20,000-file ceiling. Scanner pages contain at most 100 candidates, with a command
ceiling of 100 pages. Queue inspection pages contain at most 100 entries.
Preparation fits both the 32-message and encoded-byte ceilings and finishes an
outstanding offer before adding newer traffic. No automatic scheduler or pruning.

Input is explicitly UTF-8. Native payloads tagged CP437 are decoded using the
existing exact CP437 table at this named export boundary; existing UTF-8 is
validated without replacement. Source payload bytes and encoding remain unchanged.
This is a modern text exchange, not a byte-preserving legacy packet profile.

An import's full artifact validates before native mutation. A batch is atomic:
wrong network/neighbor/branch, unsubscribed new message, identity conflict or thread
cycle rejects without partial native acceptance or fanout. Repeated accepted
artifacts can recover receipts even after subscription removal. Fingerprints bind
all immutable message fields except forwarding path; path separately validates
against the configured unique origin-to-sender route. Local import appends its own
Node ID without overwriting true origin. Late native parent linking leaves immutable
CircuitNET parent metadata untouched and rejects native cycles.

Offline trust is deliberately local operator authority, not packet authentication.
No cryptographic origin verification, secret enrollment or public wire stability
is claimed. A future transport must supply established neighbor authority rather
than treating this profile's opt-in as Internet trust. The boundaries permit a
future non-SPITFIRE implementation to supply its own canonical native storage.

Verification lives in the codec/core CircuitNET tests, database migration tests
and `sf-bbs/tests/circuitnet.rs`. The last test creates independent boards and runs
actual operator processes, file handoffs and native cold backup/restore. Set
`SPITFIRE_C2_EVIDENCE` to a nonexistent disposable directory to retain that journey's
private board/artifact evidence. This test opens no CircuitNET or caller listeners.

## C3 transport addition

Schema 30 adds live configuration and bounded health only; see the
[wire contract](circuitnet-transport.md#durable-implementation-authority).
The original offline `prepare/import/acknowledge` APIs still require the profile's
explicit trusted-offline opt-in. Their shared `*_neighbor` implementations accept
host-established neighbor authority and apply identical topology, Dossier,
identity, native-storage and receipt policy. Live TLS admission occurs in sf-bbs,
never in this core. Native scanning also works for a live-only profile.

Daemon networking owns profile listeners, finite outbound workers and protected
operator actions. sfconfig preserves the cold C2 interface and adds live commands;
sfmonitor adds the CircuitNET Networks section. C2 atomic batch and restore-held
semantics remain accepted. Test names and acceptance details for the live layer
are documented with C3, while the original offline journey remains a regression.


## Schema 31: C4 authority and recovery

The transactional 30→31 migration appends nullable `destination` to immutable
`circuitnet_messages`; old rows retain null and identical fingerprints. The
immutable `circuitnet_destinations` table selects a native post's destination
before publication. Rejected selection records prevent later broadcast fallback.
`circuitnet_control_policy` owns CAS-versioned policy. `circuitnet_controls` owns
incoming/outgoing request JSON, authenticated neighbor, fingerprint, result,
settled flag and creation/update times. Its identity fields cannot be updated or
deleted. Native message bodies remain solely in existing native payload storage.
No governance, private-mail or file-networking tables are added.

A pending approval becomes a terminal result in the transaction that mutates the
Dossier. Approved is therefore not a separate crash-prone durable intermediate
state: successful approval produces `applied` or the corresponding no-op. Failure
of the SQLite transaction preserves pending authority for recovery. Invalidated
policy or mapping produces a durable denial/unknown-codename result. Terminal
request identities cannot be reopened by the service. Incoming requests are kept
for replay; outgoing terminal results stop resubmission until explicit retry.

The existing 100,000-row history budget includes controls. Request identity and
content are immutable; unknown/forged identities are rejected with a safe audit
entry before any Dossier change. `circuitnet_changes` records request, decision,
mutation, route selection/next hop and rejected routing without message bodies,
private names or credentials. Native cold backup covers the complete database,
artifact manifests and existing credential custody. Restoring an old sender can
recover later receipts from its receiver without duplicate import or transit.
It cannot reconstruct decisions made after the receiver's own backup; restoring
such a receiver correctly restores the snapshot's truth, not invented later ACKs.

C4 operator IPC uses minor 14's `circuitnet-controls` feature for new mutations;
C3 actions retain minor 13. Networks projects 16 control/subscription entries per
profile/page, prioritizing unsettled controls, with count/sample metadata for
large query results. The full request/result remains in durable core history.
Human instructions and the cold history command are in the
[CircuitNET manual](../manual/circuitnet.md#troubleshooting-remote-dossier-requests).

The independently configured topology is the only route authority. `Topology::path`
validates node membership, one connected acyclic rooted tree and END leaf roles,
then performs bounded traversal to return its unique path. Native publication
uses just the second node, or no hop for self. Import verifies that the arriving
origin/path/local prefix belongs to the same complete destination route. No route
learning, peer mesh, subscription inference or broadcast fallback is permitted.
