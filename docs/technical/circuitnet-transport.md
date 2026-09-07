# CircuitNET NG live transport (C3 interface gate)

Status: implemented and accepted for C3, including isolated four-node macOS acceptance.
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
allocation. Hello/control frames have a 4096-byte bound. Protocol identity is
`CIRCUITNET-NG`, major 1, supported minor range 0 through 1. Both minors use the
same baseline semantics; select the highest common minor. Unknown major,
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
must not be added under those version numbers. Unknown capabilities (at most eight
strings of at most 32 bytes) may be ignored, but both required capabilities must
be present. Unsupported major/range rejects with `unsupported-version` after TLS;
ALPN failure can reject at TLS before an application error is possible.

An offer contains exactly the [C2 envelope](circuitnet.md): fields in canonical
hash order are `format`, `version`, `network`, `sender`, `neighbor`, `messages`.
Message field order is `id`, `origin`, `codename`, `author`, `subject`, `body`,
`timestamp`, `reply`, `path`. Strings are encoded by the deterministic C2 serde JSON
encoder with no whitespace; optional reply is explicit JSON null. Canonical
receipt order is `format`, `version`, `network`, `sender`, `neighbor`, `artifact`,
`accepted`. The artifact is lowercase SHA-256 of the reconstructed canonical C2
Batch encoding. Independent implementations must reproduce that encoding: UTF-8
non-ASCII characters remain literal, control characters and quotes/backslashes use
JSON escapes. The current strict message text rules exclude ambiguous controls.
The checked Rust codec and round-trip tests are the executable reference.

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

Schema 30 adds `circuitnet_live` for CAS-versioned profile listener/public identity/
peer configuration and `circuitnet_link_health` for one safe latest observation per
enrolled neighbor. Private keys use the existing restricted SYSTEM secret-custody
pattern, keyed by the public certificate's SHA-256. Native cold backup includes
those files and preserves restricted permissions on restore. Runtime session and
listener state exist only in memory. C2 history/queue/schema-29 ownership remains
as documented; no directed-routing or governance relations are added.
