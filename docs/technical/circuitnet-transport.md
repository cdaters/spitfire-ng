# CircuitNET NG live transport, controls and files

CIRCUITNET-NG 1.4 is the current live transport contract. The sections below
distinguish the baseline exchange from later negotiated capabilities.
Independent implementers should start with the consolidated
[CIRCUITNET-NG 1.4 specification](circuitnet-ng-specification.md).
Native SPITFIRE messages remain canonical. This transport supplies authenticated
neighbor authority to the C2 service; it owns no alternate message store.

## Protocol contract

TCP carries TLS 1.3 with mandatory mutual certificate authentication, ALPN
`circuitnet-ng/1`, and no early data or resumption. Each profile has its own
optional listener and local certificate/private key. Trust consists exclusively
of enrolled direct-neighbor certificates; the presented leaf must exactly match
the certificate bound to the asserted network and Node ID. Standard rustls
certificate, signature, validity and server-name verification remains enabled.
There is no trust-on-first-use, public trust-store fallback or anonymous mode.
The node certificate/private key is its credential; a second password is absent.

Application frames are a four-byte unsigned big-endian JSON byte length followed
by strict UTF-8 JSON. Zero length and lengths above 4 MiB + 4096 reject before
allocation. Hello/ACK/Close frames have a 4096-byte bound; C4 control-work bounds are below. Protocol identity is
`CIRCUITNET-NG`, major 1, supported minor range 0 through 4. Minors 0–1 use the
same baseline semantics; minor 2 has optional C4 capabilities and minor 3 optional file distribution; minor 4 supports catalog-sync and the optional
catalog-access extension for schema-2 access metadata. Select the highest common minor. Unknown major,
nonoverlapping minor ranges, identity/profile/role mismatches and missing required
capabilities reject. Required capabilities: `atomic-batch`, `symmetric-poll`.

The initiator sends Hello (identity, network, role, versions, capabilities,
mode test/poll); the responder validates it against the authenticated certificate
and returns its Hello. Test then exchanges Close without transferring messages.
Poll sends one optional C2 Batch, receives its C2 Receipt, then receives one
optional reverse Batch and returns its Receipt; Close completes the session.
An empty offer means no eligible work. Connection direction is independent of role.
One poll is bounded to 32 messages / 4 MiB in each direction.

Batch and Receipt are the existing C2 typed JSON objects, including their retained
`circuitnet-ng-offline` format identifiers. No second message representation is
introduced. Hashes refer to the deterministic C2 encoding, not outer frame bytes.
The receiver commits validation, native import, fanout and receipt custody before
sending ACK. Socket write success never completes delivery. All exact members
must appear in the receipt in manifest order. Atomic batch policy is deliberate:
a valid A, duplicate B and conflicting C rejects the whole new batch; A stays
pending and B's earlier acceptance survives. No partial success is invented.
Replay of a committed artifact returns the same member receipt without re-fanout.

## Host boundary and bounds

The host owns TCP, TLS, private key custody, timeouts and daemon lifecycle.
The core owns topology, Dossiers, native messages, queue eligibility and receipts.
Endpoint enrollment is explicit; no discovery, learned routing or FTN adapter is
used. New outbound preparation rechecks current Dossiers. Unsubscription holds
unsent work as in C2. Neighbor hold prevents new exchanges and retains work.
Restore holds uncertain queue work for explicit policy-valid retry.

Connect, TLS, authentication and individual frame deadlines are 10 seconds;
entire sessions are at most 120 seconds. Four active sessions total and one
per profile/neighbor are admitted, including pending unauthenticated connections.
Workers retry transient failures at most three times with 1/2-second delays;
C2's 12-attempt per-delivery ceiling remains. Operator Retry is explicit.
Safe bounded link health contains counts, timing and typed error classes only.
No bodies, private keys, certificates or internal paths appear in link projections.

## Engineering references

A bounded read-only FireComm TLS review informed the standard rustls choice and
bounded certificate inputs (adapted). Its terminal behavior and platform trust
store are FireComm-specific and not adopted. No source was copied or dependency
introduced. Standard mutual-authentication configuration follows the
[rustls verifier contract](https://docs.rs/rustls/0.23.43/rustls/server/struct.WebPkiClientVerifier.html).
C1 remains historical authority; legacy CNP/CND is outside this contract.

## Exact frame objects

JSON object field order in an outer frame is immaterial; duplicate or unknown
fields reject. Objects use the following shapes (the ellipses in this table are
explanatory, not wire bytes):

| Type | Object |
| --- | --- |
| Hello | `{"type":"hello","hello":{...}}` |
| Offer | `{"type":"offer","batch":null}` or a C2 Batch object as `batch` |
| ACK | `{"type":"ack","receipt":{...},"imported":1,"duplicates":0}` |
| Error | `{"type":"error","code":"auth-failed"}` |
| Close | `{"type":"close"}` |

A baseline initiator hello is:

```json
{"type":"hello","hello":{"protocol":"CIRCUITNET-NG","major":1,"minimum_minor":0,"maximum_minor":1,"capabilities":["atomic-batch","symmetric-poll"],"network":"circuitnet-test","node":"END00001","role":"END","mode":"poll"}}
```

Mode is `test` or `poll` and the responder echoes it. Role is `END`, `HOST`, or
`ROOT`. Both peers independently compute the highest common minor from the two
hellos; no work frame is legal before both hellos validate. Minor 0 and 1 have
identical mandatory behavior in this implementation; new incompatible semantics
must not be added under those version numbers. Minor 2 adds only negotiated C4 phases. Unknown capabilities (at most eight
strings of at most 32 bytes) may be ignored, but both required capabilities must
be present. Unsupported major/range rejects with `unsupported-version` after TLS;
ALPN failure can reject at TLS before an application error is possible.

An offer contains exactly the [C2 envelope](circuitnet.md): fields in canonical
hash order are `format`, `version`, `network`, `sender`, `neighbor`, `messages`.
Message field order is optional `conference_identity`, then `id`, `origin`,
`codename`, `author`, `subject`, `body`, `timestamp`, `reply`, `path`, then optional
`destination`. Absent extension fields are omitted. Strings use the canonical JSON
rules in the consolidated specification with no whitespace; optional reply is explicit JSON null. Canonical
receipt order is `format`, `version`, `network`, `sender`, `neighbor`, `artifact`,
`accepted`. The artifact is lowercase SHA-256 of the reconstructed canonical C2
Batch encoding. Independent implementations must reproduce that encoding: UTF-8
non-ASCII characters remain literal, control characters and quotes/backslashes use
JSON escapes. The current strict message text rules exclude ambiguous controls.
Public independent golden vectors and both production/Python codec tests verify
these bytes; source-code defaults are not an unstated contract. Receipt sender is
the accepting node and neighbor is the original batch sender, reversing batch direction.

`imported` and `duplicates` are nonnegative 32-bit counts whose sum must equal the
exact offer member count. They describe this receipt observation. A committed
artifact replay reports all members as duplicates. They do not replace the exact
receipt member list or its artifact hash. A null offer receives no ACK; the next
phase begins immediately. A nonnull offer always requires one ACK or Error.

After the initiator's offer/ACK, the responder sends its offer (even when null),
then awaits its ACK if nonnull. The initiator sends Close; the responder returns
Close. Both send TLS close-notify best-effort. Unexpected frame types, truncated
frames, EOF before a required frame and extra application sequencing fail the
session; they never infer an ACK. Reverse-direction failure cannot undo a prior
committed forward-direction receipt. One session carries at most two batches.

## Errors and admission

The finite wire error vocabulary is `connect`, `tls`, `timeout`, `auth-failed`,
`unknown-node`, `wrong-network`, `topology-mismatch`, `unsupported-version`,
`malformed-frame`, `oversized`, `conflicting-message`, `unauthorized-codename`,
`custody`, `held`, `busy`, `interrupted`. Local setup/I/O failures may appear only
in local health because no authenticated wire channel exists. Unknown certificate
holders are rejected by TLS without an application hello. A recognized certificate
asserting another Node ID receives `unknown-node`; possession of some other
configured certificate does not authorize that asserted identity.

Errors contain no free-text detail, message content, certificate, credential or
host path. A terminal Error closes that exchange. Authentication/version/policy
errors do not automatically reconnect; connection interruption, unavailability
and timeout may trigger the finite retry schedule. Repeated operator polls are
explicit new operations, not a hidden scheduler.

TLS trust anchors are the exact configured certificates, with no system trust
store. After standard TLS verification the presented leaf must also equal its
enrolled DER bytes. This prevents a certificate signed by an enrolled identity
from acquiring that identity's Node ID. TLS 1.3 transcript signatures bind proof
of key possession to a fresh connection; early data and session resumption are
disabled. Forwarded origin is the C2 authenticated-neighbor assertion, not an
end-to-end signature by every earlier node.

## Durable implementation authority

C3 schema 30 adds `circuitnet_live` for CAS-versioned profile listener/public identity/
peer configuration and `circuitnet_link_health` for one safe latest observation per
enrolled neighbor. Private keys use the existing restricted SYSTEM secret-custody
pattern, keyed by the public certificate's SHA-256. Native cold backup includes
those files and preserves restricted permissions on restore. Runtime session and
listener state exist only in memory. C2 history/queue/schema-29 ownership remains
as documented; C4 schema 31 adds the separately documented directed/control relations; no governance relations are added.

## C4 compatible minor 2

C4 advertises minimum minor 0, maximum minor 2, and the existing `atomic-batch`
and `symmetric-poll` capabilities plus `directed-routing` and
`remote-dossier-control`. Only the original two are mandatory for a session.
Each optional feature requires minor 2 and both peers advertising that capability.
Unknown capabilities remain bounded and ignorable. ALPN and major 1 are unchanged.

A broadcast message omits `destination`, preserving the exact C2 encoding and
fingerprint. A directed message appends `"destination":"END3"` after `path`.
The destination participates in its immutable content fingerprint and artifact
hash. The remaining batch and receipt format/version fields stay unchanged.
Old strict decoders reject the extra field; live C4 senders withhold directed
work unless its capability was negotiated. They also withhold any already-built
mixed artifact containing directed work, preserving artifact identity. Ordinary
unprepared broadcast work can still exchange. Receipt semantics are unchanged:
a hop's ACK proves its native durable acceptance, not final-node delivery.

When control capability is negotiated, each direction's Offer phase is prefixed
by exactly one Controls frame and one matching ControlResults frame, even when
both lists are empty. The initiator sends controls/results exchange then its
Offer/ACK; the responder does the same in reverse; Close remains unchanged.
Test mode never sends either control or message work. Without control capability,
the C3 phase sequence is unchanged. Controls use the same authenticated TLS stream.

```json
{"type":"controls","requests":[{"network":"circuitnet-test","id":"END3:00000000000000000000000000000001","requester":"END3","target":"HOST2","operation":"subscribe","codename":"CNTECH"}]}
```

Request field order above is the canonical SHA-256 input: compact UTF-8 JSON,
C2 escaping rules, explicit null codename for `query-subscriptions`. The other
mutation operation is `unsubscribe`. IDs use Node ID plus random 128-bit hex,
in a control namespace distinct from conference message identities. They expose
no native database IDs. Duplicate and unknown fields, unknown operations,
inconsistent codename presence and invalid tokens reject.

```json
{"type":"control-results","results":[{"network":"circuitnet-test","id":"END3:00000000000000000000000000000001","fingerprint":"<canonical-request-sha256>","requester":"END3","target":"HOST2","outcome":"pending-approval","subscriptions":[]}]}
```

The illustrative fingerprint placeholder must be replaced by the lowercase
64-character SHA-256. Result order and IDs must match the exact request list.
Every result binds network, requester, parent target and request fingerprint.
Results are `accepted` (local queued state), `pending-approval`, `applied`,
`already-subscribed`, `already-unsubscribed`, `denied`, `unknown-codename`,
`unauthorized`, `malformed` or `replay-conflict`. Shape failures may instead close
with `malformed-frame`; bounds failures close with `oversized`. No arbitrary
operator details are returned. Query results contain only the authenticated
requester's subscribed codenames, sorted, at most 4,096 entries.

Each direction carries at most 16 requests. A Controls frame is at most 65,536
bytes; ControlResults at most 1 MiB, within the existing outer frame ceiling.
The existing connection/session deadlines and admission permits also bound these
phases. There is no unbounded control dialogue or subscription scheduler.

The receiving service compares the authenticated TLS peer against requester,
request-ID origin, configured profile and direct-child relationship to local
parent. ROOT cannot request upstream control; no child may mutate another child
or remote branch. A request is an administrative operation, never a caller post.
Authorization is rechecked when an operator approves a pending mutation.

Incoming request/result and any Dossier mutation commit before returning a result.
The requester persists results before marking terminal work settled. Pending
requests are resent by subsequent explicit polls. Losing a result does not require
an independent result-ACK protocol: retransmitting the identical request recovers
the current durable result. A terminal result never regresses to pending locally;
a conflicting terminal response fails closed. Queries are immutable snapshots.
Cold restore retains these identities and results without recreating operations.

## Transport confidentiality and stored visibility

CircuitNET transport is encrypted and authenticated hop by hop. TLS protects
packets in transit; it does not encrypt the stored native conference message for
only its final reader. Conference access rules govern readers after import.
Directed routing selects one destination and does not provide private messaging.
Intermediate HOST/ROOT operators may access retained native transit records.
Local/private BBS messages are restricted by BBS access controls unless a separate,
explicit feature supplies end-to-end encryption. C4 supplies no E2EE capability.


## C5 invocation policy

Generic [SPITFIRE Events](events.md) may initiate the existing finite Poll session.
C5 retained wire **1.2 and its capabilities**; C6 adds the optional phase below. Queue preparation is independent
of Event recurrence, and incoming symmetric Polls remain allowed by peer policy even
when outbound initiation is scheduled/manual. Role is independent of TCP direction.
TLS protects transport in transit; it does not encrypt conference content end-to-end
or make directed traffic private after delivery.

## C6 file-distribution phase (1.3)

C6 is implemented and accepted.

`file-distribution` enables the file phase only when both peers advertise it and
minor 3 is negotiated. `file-hash-have` additionally permits payload omission for a new publication whose
verified content is already owned;
it has no effect without file distribution. No resume capability is advertised.
Minor 0–2 peers retain exactly their earlier message/control phase sequence.

For each direction of a Poll, controls (when negotiated) and the optional message
Batch/ACK are followed by zero to eight file publications, then a null file offer.
The initiator completes its entire sending phase before the reverse phase begins.
Test Link sends no file work. Frames are bounded to 16 KiB for file metadata/results:

| Type | Shape |
| --- | --- |
| File offer | `{"type":"file-offer","publication":{...}}` or `publication:null` |
| Want bytes | `{"type":"file-want","want":{"decision":"send"}}` |
| Have bytes | `{"type":"file-want","want":{"decision":"have"}}` |
| Replay receipt | `{"type":"file-want","want":{"decision":"complete","receipt":{...}}}` |
| File receipt | `{"type":"file-receipt","receipt":{...}}` |

Publication fields are `network`, `id`, `origin`, `codename`, `sha256`, `size`,
`filename`, `description`, `timestamp` and `path`. IDs use the existing validated
origin-prefixed 128-bit random token spelling, in a separate file-publication
namespace. No message row or message body carries file protocol authority. Hash
identifies immutable content; ID identifies publication. Path starts at origin and
ends at the sender, must equal the configured tree path, and cannot contain the
receiver. Each forwarding node appends itself. Filename is a bounded basename and
never an extraction path. Maximum transport payload is 64 MiB, additionally limited
by mapping and native area policy. Description is bounded safe text.

After `send`, binary payload framing replaces JSON until its terminator: four-byte
unsigned big-endian chunk length, then 1–65,536 raw bytes; a zero length terminates.
The receiver rejects excess advertised bytes, oversized chunks, wrong final length
and SHA-256 mismatch. No multi-gigabyte allocation or base64 payload is used. Temporary
payloads have generated private names. Incomplete streams are discarded and restarted
from zero. File I/O waits have a 90-second bound inside the existing 120-second session
ceiling. No second socket, TLS identity or scheduler is introduced.

Receipt fields are `publication`, `sha256`, `outcome` and `reason`. Outcomes are
`published`, `pending-approval`, `quarantined`, `rejected`; reasons are finite safe
classes (`local-policy`, `native-admission-rejected`, `hash-mismatch`). A receipt is
durable before transmission. Exact replay returns the retained result without
reimport or repeated fanout; identity reuse with changed metadata rejects. The
fingerprint excludes the per-hop path and includes immutable publication metadata.
A hash mismatch produces a durable rejection, never a publication acknowledgement.

Hash-have checks local bytes against size/hash, then new publication admission still
uses receiving policy. Upstream clean assertions cannot substitute for local scanning;
C6 does not transmit scanner claims. Delivery receipts are independent per neighbor.
The existing finite session retry and Events recurrence govern retry; successful
branches remain complete. [Native Files custody](files-custody.md) owns payloads,
inspection, quarantine, area access and backup.

TLS protects transfers between nodes. It does not make downloadable files private
or end-to-end encrypted after delivery.

A completed publication replay returns its durable receipt without payload regardless
of hash-have support. Hash-have concerns a new publication sharing existing content.

## C7 catalog phase (1.4)

Minor 1.4 adds optional `catalog-sync`. Existing required baseline capabilities are
unchanged; maximum advertised capability count remains eight. Both ends must negotiate
minor >=4 and advertise catalog-sync. Test Link has no catalog phase. In Poll mode,
after mutual Hello validation and before all existing content phases:

1. Initiator sends its catalog phase; responder receives it.
2. Responder sends its catalog phase; initiator receives it.
3. Existing symmetric controls/messages/files phases run unchanged.

A catalog phase sends `catalog-head` with revision/hash. Only a direct parent offers
nonzero authority downstream. The receiver sends `catalog-request` with its current
revision/hash and enabled flag (explicit local authority pin). The parent checks its
retained matching predecessor, then sends at most 64 consecutive `catalog-object`
frames, each containing the unchanged signed snapshot. Each accepted snapshot gets
`catalog-ack` revision/hash. An object with catalog=null ends the phase. A disabled
receiver receives only the terminator, not an automatically trusted authority.

Head/request/ack use the existing 4-KiB control bound. A signed object is <=512 KiB;
the enclosing frame is bounded by 512 KiB + 1 KiB. Full snapshots contain <=256
retained immutable conference identities. Revision zero has no hash and means no
accepted catalog. Authenticated peer/profile/tree checks and link holds still apply;
object authentication separately verifies the locally pinned Ed25519 key and publisher.

Message envelopes gain optional `conference_identity`: 32 lowercase hexadecimal
digits identifying the catalog generation. It is part of the immutable message
fingerprint; forwarding preserves it. Absence preserves C2-C6 serialization for
uncataloged profiles. Catalog-enabled new traffic requires its current active or
deprecated identity; generation-aware messages are withheld from older peers rather
than stripping identity. Catalog metadata never contains a local conference number.
See the [catalog schema and chain contract](circuitnet-catalog.md).

## Catalog access compatibility within minor 4

`catalog-access` is the eighth bounded capability. It requires `catalog-sync`.
Catalog schema 2 carries explicit `access: "sysops"`; omission means Public and
preserves schema-1 canonical bytes. Both peers must advertise catalog-access before
schema-2 catalog frames or Sysop-only conference messages are sent. An older
catalog-sync peer is offered the latest schema-1 predecessor instead; public
message/control/file exchange remains available according to its capabilities.
Restricted work and public generations absent from the compatible predecessor stay
durable and are never downgraded to unclassified messages. Existing compatible
public work remains eligible even when an unsupported offer awaits retry.
Local receive mappings enforce catalog access through native conference policy.
Key replacement is a separate explicit local trust operation, not a new wire frame.
