# Implementing CircuitNET NG

Start with the [CIRCUITNET-NG 1.4 contract](circuitnet-ng-specification.md), then run
the [independent conformance suite](../../tools/circuitnet-conformance/README.md).
No SPITFIRE database or Rust knowledge is required to implement the wire protocol.
SPITFIRE NG remains the reference BBS implementation. The separate Python peer
provides a second implementation with deliberately smaller storage/application scope.

## Build in bounded steps

1. Implement identity normalization, strict frame decoding and golden canonical
   hashes. Preserve message IDs and parent references independently of local numbers.
2. Enroll a test neighbor's certificate explicitly. Implement TLS1.3, mutual
   certificate validation, exact leaf binding, ALPN and expected Hello identity.
3. Implement Test mode and a symmetric empty Poll, then atomic message acceptance,
   exact receipts, replay/conflict and crash recovery before adding UI integration.
4. Add signed catalog pin/bootstrap/sync, revision/lifecycle validation and local
   access mapping. Official-network profiles require Catalog/access with Core.
5. Add directed routing, controls or Files only when advertised and tested. HOST
   forwarding needs the complete tree/path/Dossier policy; a two-node success alone
   does not prove full Routing conformance.

Caller conferences and File Areas stay native to your BBS. Local mapping does not
subscribe an area. A catalog's public/sysops classification must map to your own
security/access system; no SPITFIRE security level is a portable wire requirement.
Native posting freezes author/content, and a durable network identity survives local
renumbering. No implicit other-network bridging, private mail or global person ID.

For Files, use your own staged admission/scanning/quarantine policy. Hash-have saves
bytes, not policy checks. A durable pending/quarantine/reject receipt must never be
reported as publication. Use harmless synthetic data in development.

## Verification and reporting

The suite's public vectors can be consumed by any language. The Python peer shares
no production CircuitNET codec/state-machine helpers. Its generic library choices
follow [Python SSL](https://docs.python.org/3/library/ssl.html) and standard
[Ed25519 verification](https://cryptography.io/en/latest/hazmat/primitives/asymmetric/ed25519/).
Those libraries supply cryptographic operations, not network semantics.

Compare canonical hashes and signatures before opening sockets. Then run actual
bidirectional sessions, replies, lost-ACK retry, conflicts and malformed input tests.
Keep production certificates and personal data out of fixtures. Capture finite error
codes and counts, not sensitive stream dumps. All supplied socket tests use loopback.

Report implementation/language versions, protocol minor, exact claimed profiles,
commands/test revisions, passes/failures/skips and limitations. Separate codec tests,
self-interoperation and independent BBS interoperability. A useful future statement
is “CIRCUITNET-NG 1.4 Core and Catalog conformance tests passed,” with evidence;
there is no central certification service or automatic network admission.

File requests, remote file Dossier management, private mail/E2EE, node-generation
migration and third-party BBS plugins are separate work. Existing Events merely
invoke exchanges on the SPITFIRE side; implementing another scheduler is unnecessary.
