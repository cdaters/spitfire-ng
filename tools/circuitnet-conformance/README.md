# CIRCUITNET-NG independent conformance suite

Start with the [implementation contract](../../docs/technical/circuitnet-ng-specification.md)
and [implementation guide](../../docs/technical/circuitnet-implementation.md).
This original Python peer implements its own JSON codec, normalization, hashes,
session sequencing, controls, file streaming, catalog validation and receipt store.
It does not import/link SPITFIRE protocol code or read a native board database.
`protocol.py`, `catalog.py` and `peer.py` can run outside the repository alongside
their configuration and public vectors. Rust integration tests configure only the
SPITFIRE side; they are not an oracle invoked by the peer.

## Prerequisites and tests

Python 3.10+ with TLS1.3-capable OpenSSL and the generic `cryptography` package
(Ed25519 and ephemeral X.509 fixture generation). No package manager or system
service is invoked by the runner. Tested runtime versions are recorded in the
milestone report. The only nonstandard Python dependency is cryptography; use an
isolated environment and your normal dependency approval policy if not installed.
`requirements.txt` pins the tested generic dependency; no installation is performed
by any runner. Acceptance used Python3.14.6/OpenSSL3.6.3/cryptography49.0.0.

From repository root:

```sh
python3 tools/circuitnet-conformance/runner.py --report conformance-report.json
cargo test -p sf-net --test circuitnet_conformance
cargo test -p sf-bbs --test circuitnet_independent
```

The first command runs Python codec/state/security tests and Python↔Python TLS.
The second compares production codecs to public independent golden/malformed data.
The third launches a disposable actual SPITFIRE daemon and separate Python peer
processes on localhost. Do not call the first two native interoperability evidence.
No tests contact an external BBS. No permanent keys, production credentials,
historical binaries or real malware are used. Generated evidence belongs outside
Git or in an ignored build directory, not alongside committed public vectors.

## Profiles and claims

| Profile | Evidence expected |
| --- | --- |
| Core | TLS/enrollment, framing, negotiated baseline, both message directions, threading, atomic ACK/replay/conflict |
| Routing | Directed destination preservation, unique-path/no-broadcast policy; HOST/ROOT fanout explicitly additional |
| Control | Subscribe/unsubscribe/query, authentication, stable result/retry and authorization |
| Files | Metadata, bounded streaming, integrity, durable publication outcomes; hash-have stated separately |
| Catalog | Signed sync, authority/hash chain, rollback/fork, lifecycle/generation/access preservation |

Official-network participation requires Core plus Catalog/access to enforce its
current signed policy. This is a protocol profile statement, not membership approval.
Applications remain Closed. A Core-only implementation can describe that limited
profile; it cannot claim all current official-network behavior.

Reference scope: one enrolled direct link, END or parent responder/initiator,
final-destination directed messages, fixed configurable synthetic control/file policy.
It does **not** claim full Routing HOST fanout, a caller access implementation,
malware inspection, automatic retry scheduler, network registry or node relocation.
It retains public/sysops metadata and has no caller UI to expose restricted content.
Production BBS adapters must supply their own access and Files admission policies.

## Running the peer independently

Generate disposable certificates and sample configuration/work, then inspect them:

```sh
python3 tools/circuitnet-conformance/fixtures.py --output /tmp/circuitnet-fixture
python3 tools/circuitnet-conformance/peer.py \
  --config /tmp/circuitnet-fixture/peer.json \
  --state /tmp/circuitnet-fixture/peer-state \
  --port 36400 --work /tmp/circuitnet-fixture/work.json \
  --report /tmp/circuitnet-fixture/result.json
```

This example connects **only 127.0.0.1**. Enroll the generated peer certificate on
the disposable other implementation, and configure its certificate on the peer.
The sample peer is USAZ001 END, expected HOST USAZ000, network conformance, tree
ROOT→USAZ000→USAZ001. The ROOT catalog publisher is a synthetic authority; these
keys are unrelated to the official Network Kit. All generated private files stay
in the temporary fixture directory. Never enroll these identities in a real network.

Use `--listen --port PORT --ready FILE` for one bounded inbound session; port0
chooses a local unused port and writes it to READY. `--mode test` performs only
Hello/Close. `--sessions 2` can keep a listener available for a deliberately lost
ACK retry; at most3 sessions, no automatic outbound scheduler. Each connection uses
a fresh TLS context, enrolled leaf checking, deadlines and no resumption.

Configuration fields are ordinary private operator configuration, not wire frames:
network, node, role, peer, peer_role, topology (id/role/parent rows), certificate PEM,
private_key PKCS8 PEM, peer_certificate PEM, server_name, receive codenames,
inbound_subscriptions, file_subscriptions, control_policy (`deny`/`auto-approve`),
file_policy (explicit receipt outcome), optional authority (network/catalog_id/
publisher/public_key). Optional maximum_minor/capabilities exercise old profiles.
`drop_ack_once` is an explicit test fault; it commits one incoming batch then drops
its response. It is not a protocol option sent to peers.

Work JSON has optional batch, controls array and files array. Each file item contains
publication plus a local source path; source is never serialized on the wire.
Work is submitted explicitly and may be replayed; the peer records ACKs but does not
invent a queue scheduler. For new work, `protocol.new_identity(node)` generates an origin-prefixed128-bit
cryptographically random identifier without SPITFIRE code or a local counter.
Source publication IDs must remain immutable on retry. Deterministic serial tokens
in golden/synthetic isolated fixtures are test data, not a production allocation policy.
Null/missing batch is no message work. Empty/missing arrays are no control/file work.

`--seed HISTORY.json` imports a signed consecutive array against the configured pin
without making a connection. It does not enroll a key. Existing state binds network,
node, neighbor, topology and pin; changing them cannot reinterpret saved receipts.

## Custody and recovery

State is private JSON plus content-addressed test payloads, not SPITFIRE-compatible
SQL. New message batches validate atomically, including late-parent cycle checks,
and commit before ACK. File content verifies before publication receipt. Same
identity with different fingerprint conflicts; repeated accepted artifacts recover
receipts. A crash/partial binary stream leaves no acknowledged new publication.

One writer uses an exclusive directory lock; after a crash, verify no peer process
remains before removing `writer.lock`. State replace is flushed/fsynced, and parent
directory synchronization is used where supported. Content paths derive only from
validated SHA256. Files are streamed in64KiB chunks. Metadata history is capped at
10000 total records/16MiB; catalogs retain at most4096 revisions within that bound.
No automatic pruning or garbage collection. Stop the peer and copy its complete
state/content directory for a snapshot. Restoring old peer state cannot reconstruct
acknowledgments committed after that snapshot; tests never claim otherwise.

## Public vectors and reporting

`vectors/valid.json` contains exact canonical bytes as hex, expected SHA256,
message/reply/directed/control/file examples, and two signed synthetic catalog
revisions with public key only. No private signing seed is committed. JSON formatting
of the vector container is unimportant. `vectors/malformed.json` contains exact
length-prefixed bytes and expected finite failure codes. Tests separately exercise
semantic invalid fields, paths, conflicts, signatures and lifecycle.
`vectors/semantic.json` makes version/unknown-capability, signature/chain and exact
replay outcomes explicit; chain-only vectors deliberately isolate chain validation
and do not imply skipping signature verification during admission.

Runner report format `circuitnet-ng-conformance-report`, version1: protocol,
implementation, profiles, UTC date, tests (name/outcome), passed/failed/skipped and
limitations. Outcomes pass/fail/error/skip. A peer invocation report separately uses
`circuitnet-ng-peer-result`, version1, finite result, negotiated minor/capabilities
and counts; no bodies, credentials, paths or caller names. Do not submit private
fixture directories as public reports. Publish versions, exact commands, scopes,
counts and deviations when reporting interoperability; a green subset is not a
certification badge or proof of untested features.
