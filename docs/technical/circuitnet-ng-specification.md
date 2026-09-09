# CIRCUITNET-NG 1.4 implementation contract

This is the starting wire contract for independent implementations. CircuitNET NG
is the network/product; CIRCUITNET-NG is the protocol name. SPITFIRE NG is a
reference implementation, not a source of unstated protocol requirements. Local
conference numbers, account security levels, database schemas, queues and storage
paths are not protocol fields. See [implementing guide](circuitnet-implementation.md)
and [conformance suite](../../tools/circuitnet-conformance/README.md).

## Identity and trust

A network is a 1–32 ASCII alphanumeric/internal-hyphen token. Its canonical spelling
is lowercase. Node IDs are 1–8 ASCII alphanumeric, canonically uppercase. Conference
codenames are 1–8 ASCII alphanumeric/internal-hyphen, canonically uppercase. No token
starts or ends with a hyphen. Receivers accept ASCII case variations and normalize
before comparisons/hashing. Senders use canonical spelling. Geographic assignment
is [administrative policy](circuitnet-addressing.md), never routing syntax.

A message/control/file-publication identity is `NODE:hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh`:
origin Node ID plus 16 independently generated random bytes as lowercase hex.
Receivers normalize hex case. The three object classes have separate namespaces,
each also scoped by network. Conference identities and catalog IDs are exactly
32 lowercase hex digits, automatically generated; no immutable node-generation
field exists. Do not invent local database numbers or an email addressing layer.

Roles are `END`, `HOST`, `ROOT`. Enrollment configures the complete bounded tree
(up to 128 nodes), local node, direct neighbors and their expected roles. Exactly
one top node, no cycles/disconnected branches; END is a leaf with a parent; ROOT
has none; parentless HOST is permitted for an isolated network. A path must be the
unique configured origin-to-sender path, and exclude the receiver. Geography does
not choose a parent. HOST/ROOT may also be ordinary caller-facing BBS systems.

TCP carries TLS 1.3 only, mutual X.509 authentication, ALPN `circuitnet-ng/1`.
No early data or session resumption. Independently enroll each direct neighbor's
exact leaf certificate and expected Node ID/role/network. Use normal certificate
validity, signature and server DNS-name checks, then compare the presented leaf's
DER bytes to the enrolled leaf. Trust only enrolled certificates, not a system CA
store, arbitrary certificates they sign, or a key advertised during exchange.
A self-signed enrolled leaf may be a trust anchor. Do not derive Node ID from CN
or DNS text; Hello must match the explicit enrollment. Connection direction is
independent of role. TLS failure need not produce an application Error.

## Framing and JSON

Four unsigned big-endian bytes give the following JSON byte count. Zero rejects;
maximum is 4,198,400 bytes (4 MiB + 4 KiB), additionally bounded by phase:

| Phase | Maximum JSON bytes |
| --- | ---: |
| Hello, ACK, Error, Close, catalog head/request/ack | 4,096 |
| Controls | 65,536 |
| Control results | 1,048,576 |
| File metadata/decision/receipt | 16,384 |
| Catalog object | 525,312 |
| Message offer | 4,198,400; canonical batch itself ≤4,194,304 |

Read prefix and body completely with deadlines; truncated data never acknowledges
work. UTF-8 must be strict, with no BOM, invalid surrogate, trailing document,
NaN/infinity or comments. Unknown fields, duplicate keys (including equal duplicate
values at any depth), unknown enum values and wrong JSON types reject. Object key
order is otherwise immaterial. Boolean is not an integer. Integer tokens are decimal
integers, not floating-point/exponent notation. Versions fit unsigned 16 bits,
ACK counts unsigned 32 bits, catalog head counters unsigned 64 bits. Message times
are integer UTC Unix seconds 0 through 253402300799. File timestamp uses signed
64-bit seconds; interoperable senders should use the same nonnegative UTC range.

Nullable fields accept explicit null; the existing decoder also accepts omitted
nullable fields as null. Canonical serialization restores them as described below.
Optional extension fields are omitted when null. Do not interpret omitted required
nonnullable fields as zero/empty. Lists preserve order. Receivers bound nesting and
allocation; no full-file JSON/base64 payloads. Suggested resource limits inherited
from the reference host: 10s per connect/handshake/frame, 120s entire session,
90s file phase, four sessions total and one per network/neighbor. No endless retries.

## Canonical bytes

Only content hashes, request fingerprints and signatures require canonical JSON.
Outer frame formatting is not signed. Serialize compactly without whitespace or
trailing newline in the field orders below. UTF-8 non-ASCII stays literal; quote and
backslash use `\"` and `\\`. Tab/LF/CR use `\t`, `\n`, `\r`; other admitted controls
use lowercase `\u00hh` (controls are excluded from most text). Do not escape `/` or
normalize Unicode text. Integers use ordinary decimal; true/false/null lowercase.
Normalize identity tokens first. No arbitrary JSON maps or floating point occur.

## Hello and negotiation

```json
{"type":"hello","hello":{"protocol":"CIRCUITNET-NG","major":1,"minimum_minor":0,"maximum_minor":4,"capabilities":["atomic-batch","symmetric-poll","directed-routing","remote-dossier-control","file-distribution","file-hash-have","catalog-sync","catalog-access"],"network":"conformance","node":"USAZ001","role":"END","mode":"poll"}}
```

Initiator sends Hello; responder validates against TLS enrollment and sends Hello.
Both select highest common minor. Current supported range is 0–4; major must be1.
Mode is `test` or `poll`, echoed by responder. At most eight capability strings,
each ≤32 UTF-8 bytes. Unknown optional names are ignored; duplicate names confer no
extra power. Both baseline capabilities below must be present. Missing required
capability/nonoverlapping range yields unsupported-version, not a new error code.

| Capability | Minimum minor | Meaning/dependency |
| --- | ---: | --- |
| atomic-batch | 0 | Required; all-or-nothing new batch admission |
| symmetric-poll | 0 | Required; both directions exchange in one connection |
| directed-routing | 2 | Explicit message destination |
| remote-dossier-control | 2 | Message subscription request/result phase |
| file-distribution | 3 | Bounded binary file phase |
| file-hash-have | 3 | Requires file-distribution; new publication may omit known bytes |
| catalog-sync | 4 | Signed catalog phases and generation-aware messages |
| catalog-access | 4 | Requires catalog-sync; schema2 and restricted conference traffic |

Every optional capability requires both advertisements. Unsupported optional work
waits locally; never strip identity/access/destination to force an older peer to
accept it. Unknown optional capability does not break baseline traffic. Minors0–1
have identical baseline semantics. No capabilities for resume, private mail, user
addressing, remote file subscriptions or file requests exist.

## Session ordering

Test: initiator Hello, responder Hello, initiator Close, responder Close. No work.
Poll, after Hellos:

1. If catalog-sync: initiator catalog phase, then responder catalog phase.
2. Initiator controls/results (if negotiated), message Offer/ACK, files (if negotiated).
3. Responder controls/results, message Offer/ACK, files under the same rules.
4. Initiator Close, responder Close, best-effort TLS close-notify.

Always send a message Offer, even null; null gets no ACK. Always send Controls and
receive ControlResults when negotiated, including empty lists. A file phase ends
with null file-offer. No additional phases, speculative frames or pipelining.
A terminal Error closes the session. Failure in the reverse direction cannot undo
already committed forward work. Unexpected type/EOF is failure, never acceptance.

## Messages, receipts and threading

Batch canonical fields: `format,version,network,sender,neighbor,messages`.
Format `circuitnet-ng-offline`, version1 also applies over TLS. Nonnull batch has
1–32 distinct messages. Sender/neighbor differ and equal this hop's authenticated
participants. Message canonical order:
`conference_identity` (only if present), `id,origin,codename,author,subject,body,
timestamp,reply,path`, then `destination` (only if present).

```json
{"format":"circuitnet-ng-offline","version":1,"network":"conformance","sender":"USAZ001","neighbor":"USAZ000","messages":[{"id":"USAZ001:00000000000000000000000000000001","origin":"USAZ001","codename":"RETRO","author":"Synthetic","subject":"Parent","body":"Public test text.\r\n","timestamp":1788800000,"reply":null,"path":["USAZ001"]}]}
```

Origin matches ID prefix. Author is nonempty, ≤120 UTF-8 bytes and60 Unicode scalar
values; subject ≤72 bytes; body ≤65536 bytes. No Unicode control characters except
CR/LF/tab in body. U+2028/2029, U+202A–202E, U+2066–2069 reject in all message text.
Path has1–128 distinct nodes, begins origin and ends sender. Forwarders append their
Node ID without changing origin, identity, text, reply, destination or generation.

Reply is null or another full message identity, never a local message number or
self-reference. Unresolved parents are retained; linking requires network/codename/
generation agreement and no cycle. The human To field is not a separate wire field:
the current envelope has author, subject and body only. Ordinary conference routing
uses codename and Dossiers, not a global person identifier.

Offer: `{"type":"offer","batch":BATCH_OR_NULL}`.
ACK: `{"type":"ack","receipt":RECEIPT,"imported":N,"duplicates":N}`.
Receipt canonical order: `format,version,network,sender,neighbor,artifact,accepted`.
Format is `circuitnet-ng-offline-receipt`, version1. **sender is the accepting BBS and neighbor is the original batch sender**,
so the receipt reverses the batch direction. Artifact is lowercase SHA256
of canonical batch. Accepted is exactly all message IDs in batch order. Counts sum
to member count. No partial or reordered receipt. Acceptance means durable local
custody, validation and forwarding intents committed before ACK, not final delivery.

Message fingerprint is SHA256 of canonical message with `path:[]`; other fields
remain. Exact identity/fingerprint replay creates no duplicate or repeat fanout.
Changed fingerprint under an existing ID is conflicting-message. Any new member
conflict/policy/invalid thread rejects the entire new batch. Exact accepted artifact
replay returns its receipt with imported0 and all members duplicates, even if later
subscription policy changed. Lost ACK retries the same artifact. Keep receipts and
identity conflict history durable; capacity exhaustion rejects rather than pruning
history silently. Receiver restore cannot invent acknowledgments lost with backup.

## Routing and Dossiers

Normal new traffic requires explicit local receive mapping and inbound-neighbor
subscription; outbound fanout requires each neighbor's Dossier and excludes ingress/
path. Mapping/catalog presence is not subscription. END does not forward imported
broadcasts. HOST/ROOT forwarding uses the configured tree. Never bridge other
network profiles/adapters merely because local areas are mapped together.

Directed messages add destination, require negotiated directed-routing, and follow
only the unique origin-to-destination path. Intermediates retain transit custody,
without importing into caller conferences. Final recipient imports into its allowed
mapping. Broadcast Dossiers do not authorize/limit this directed route. No broadcast
fallback. ACK remains hop custody. Replies do not inherit a destination implicitly.

Controls frame: `{"type":"controls","requests":[REQUESTS]}`; at most16.
Request canonical order: `network,id,requester,target,operation,codename`.
Operations subscribe/unsubscribe require codename; query-subscriptions uses null.
ID prefix equals requester; requester and target differ. Authenticated direct child
may request only its own subscriptions at its parent; ROOT cannot request upstream.

Results: `{"type":"control-results","results":[RESULTS]}` in exact request order.
Result fields: `network,id,fingerprint,requester,target,outcome,subscriptions`.
Fingerprint is SHA256 of canonical request. Outcomes: accepted, pending-approval,
applied, already-subscribed, already-unsubscribed, denied, unknown-codename,
unauthorized, malformed, replay-conflict. First two are nonterminal. Default human
approval may be replaced by explicit local auto-approve/deny. Terminal results do
not regress. Queries return an immutable snapshot sorted by codename, ≤4096 items.
Unknown/inactive catalog area does not create a mapping. Results commit before
send; identical retry recovers current result, conflicting ID/request never mutates
Dossiers. There is no result-ACK; pending requests retry in subsequent polls.

## Files

File offer: `{"type":"file-offer","publication":PUBLICATION_OR_NULL}`; maximum8
nonnull offers each direction. Canonical publication order:
`network,id,origin,codename,sha256,size,filename,description,timestamp,path`.
Publication ID namespace differs from messages/controls. Origin matches ID; hash is
64 lowercase hex, size0–67108864. Filename1–64 ASCII bytes, only alphanumeric/`._+-`,
not beginning dot. Description ≤4096 UTF-8 bytes/20 lines, controls prohibited except
LF/tab. Path obeys message tree rules. Fingerprint is SHA256 of canonical publication
with path replaced by **[origin]**, not the message fingerprint's empty path.

Want: `{"type":"file-want","want":{"decision":"send"}}`, decision have, or
`{"decision":"complete","receipt":RECEIPT}`. Complete replays retained
publication result, independently of hash-have. Have for a *new* publication needs
file-hash-have and locally verified size/hash. Metadata still undergoes local policy.

After send, read binary chunks on the same TLS connection: unsigned four-byte
big-endian length then raw bytes, length1–65536; zero length ends stream. Final
count and SHA256 must match. Reject overrun/oversized/truncated data. No partial
resume: discard incomplete temporary bytes, retry from zero. Use bounded streaming.

After send/have, receiver sends `{"type":"file-receipt","receipt":RECEIPT}`.
Receipt fields: `publication,sha256,outcome,reason`. Outcomes published,
pending-approval, quarantined, rejected. Reason is `local-policy`,
`native-admission-rejected`, or `hash-mismatch`. No paths/free-text scanner evidence.
Result commits before transmission; hash mismatch is rejected, never published.
A malformed/oversized stream may terminate with Error instead of a receipt.

File codenames/subscriptions are separate from message Dossiers. No remote file
subscription protocol. The receiving BBS enforces its archive/scanning/approval
policy even with hash-have; no upstream scanner claim is transmitted or trusted.
Reference fixtures use harmless bytes and a fixed synthetic admission policy, not
an antivirus claim or substitute for a BBS Files system.

## Catalog synchronization and portable access

See [catalog contract](circuitnet-catalog.md) for canonical body/entry field order,
all text bounds, signature domain and lifecycle. Its implementation/storage sections
are reference-BBS details; network access is `public` or `sysops`, not level9999.
Sysops includes locally verified visiting operators, granted/revoked locally. No
certificate, catalog or subscription grants a caller account privilege. Required
is a network obligation, not permission to override local security. Retired history
and immutable identities remain; no remote local-number assignment or deletion.

Catalog phase frame shapes:

```json
{"type":"catalog-head","revision":0,"hash":null}
{"type":"catalog-request","revision":0,"hash":null,"enabled":false}
{"type":"catalog-object","catalog":null}
{"type":"catalog-ack","revision":1,"hash":"64 lowercase hex digits"}
```

Head/ACK/request hash is null iff revision0, otherwise64 lowercase hex. Head is
nonzero only from direct parent. Receiver requests its current revision/hash;
enabled only with explicit local authority pin and expected parent. Sender verifies
requested predecessor against retained history and sends at most64 consecutive
signed objects, awaiting matching revision/hash ACK after each, then null terminator.
A catalog-disabled receiver receives no objects. Head rollback/fork rejects.

Signed object fields `body,hash,signature`; hash SHA256 canonical body, signature
128 lowercase hex. Ed25519 signs literal ASCII `CIRCUITNET-NG-CATALOG-1\n` followed
by canonical body bytes. Pin network/catalog ID/publisher/public key independently. The configured publisher
must have ROOT role in the enrolled tree; technical publication authority remains
separate from human governance approval.
TLS/HTTP delivery cannot establish or replace that pin. Verify signature and body
hash, then consecutive previous revision/hash and lifecycle; persist before ACK.
Exact current replay idempotent; lower revision rollback; same number/different
object fork; missing predecessor rejects. Bootstrap starts at revision1.

Schema1 entries omit access; schema2 allows sysops as `access` before id. Public
access is omitted in canonical bytes even if explicitly received. Schema2 needs
catalog-access; otherwise parent offers last schema1 predecessor and withholds
restricted/unknown generation traffic. Do not downgrade a signed object. Key rotation
uses deliberate out-of-band local trust enrollment, not an automatic wire frame.

Lifecycle validation retains every prior immutable identity/codename. Changed entry
has effective_revision=current. Proposed may become active/retired; active may
become deprecated/retired; deprecated may retire. Retired/deprecated → active needs
intent reactivate. Same status allowed, but retained retired_revision cannot drift.
New retirement sets retired_revision=current. New ID sharing old codename requires
intent reuse and all other generations retired; default rejects reuse. New entries
have effective_revision=current. Bootstrap intent ordinary, effective_revision1,
unique codenames. Schema and publication time never decrease. All revisions preserve
history rather than interpreting codename as eternal generation identity.

## Errors, privacy and conformance

Wire Error is exactly `{"type":"error","code":"CODE"}`. Finite codes:
connect, tls, timeout, auth-failed, unknown-node, wrong-network, topology-mismatch,
unsupported-version, malformed-frame, oversized, conflicting-message,
unauthorized-codename, custody, held, busy, interrupted. No `duplicate` error:
accepted duplicates use ACK. No `unsupported-capability` error: use
unsupported-version or reject unexpected phase as malformed-frame. No exception
text, credentials, message bodies or local paths. Transport failure before an
application channel may be local only.

| Failure | Wire class |
| --- | --- |
| Wrong network | wrong-network |
| Enrolled peer asserts different Node ID/role | unknown-node / topology-mismatch |
| Exact leaf mismatch | auth-failed or TLS rejection |
| Shape/encoding/unexpected phase | malformed-frame |
| Frame/chunk admission bound | oversized |
| Message/publication identity conflict | conflicting-message |
| Area receive authorization | unauthorized-codename |
| Storage or catalog verification failure | custody (local catalog diagnostic may be more specific) |
| Hold/admission/concurrency | held / busy |

Local catalog diagnostics invalid-signature/invalid-publisher/catalog-rollback/
catalog-fork/catalog-missing-revision/catalog-lifecycle are **not additional wire
Error codes**. Implementations may expose equivalent local diagnostics safely.
Transient retry is bounded; the reference host makes at most3 attempts per invocation,
with1/2-second waits, and caps delivery attempts at12 before operator review.

TLS encrypts/authenticates each hop; conferences and directed messages are not
private mail or E2EE. Destination local access governs visibility; transit operators
may inspect custody. Public downloadable files remain public according to local
File Area policy after delivery. No telemetry or central certification service.

Conformance profiles: Core (TLS, framing, negotiation, messages, ACK/replay/threading),
Routing (directed metadata; full HOST tree forwarding separately stated), Control,
Files (including hash-have claim separately), Catalog (sync, signatures, chain,
lifecycle, access). A generic Core-only implementation may claim Core conformance;
participating official-network implementations need Core plus Catalog/access to
honor current signed area policy. Roles determine routing obligations. Report exact
profiles, tests, versions and limitations; do not claim features solely advertised.
