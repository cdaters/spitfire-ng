# C9 independent CircuitNET interoperability

Status: COMPLETE / ACCEPTED. Public source checkpoint: this repository’s Git HEAD. Start private ab2b0d5be6df59285fee5b3b1cb0708189a37890;
public 32bb39e103287722d0d8c908a5052f033a45328c. Schema 36 / protocol 1.4.

## Specification-first gap register and interface gate

Reviewed public foundation, transport, catalog, Files custody, addressing and
Events contracts before peer implementation. Existing material described overall
phases but left exact implementations dependent on production source. Reconciliation
reads are explicitly implementation checks, not undocumented peer dependencies.

| Gap | Published resolution |
| --- | --- |
| Canonical message extension order | conference_identity first, destination last; absent omitted. |
| Fingerprint path treatment | Message path=[]; file path=[origin]; not simply removing field. |
| Receipt sender/neighbor direction | Receipt sender is accepting BBS; neighbor is original batch sender. |
| Nullable fields/case normalization | Document accepted omission and normalized canonical bytes. |
| Complete catalog frame shapes | Head/request/object/ack fields, enable and terminator defined. |
| Catalog local errors vs wire errors | Local signature/chain diagnostics map to custody; no new wire enums. |
| File exact fields/bounds | Filename/text/time and binary termination defined. |
| Portable Sysop access | public/sysops policy separated from native9999 mapping. |
| Whole session order | Both catalog phases first, then each direction controls/messages/files. |
| No person To field | Clarified author/subject/body envelope; no invented user addressing. |
| Compatibility claim | Core baseline; official network additionally Catalog/access; scoped optional profiles. |

The [current implementation contract](../technical/circuitnet-ng-specification.md)
is the peer's interface. Original Python under tools/circuitnet-conformance uses
standard SSL/JSON/hash/storage plus cryptography Ed25519, not Rust CircuitNET code.
Minimal single-session durable JSON receipts/content custody, connect/listen CLI,
public vectors and machine-readable runner form the implemented boundary. Native board
setup/test assertions may use SPITFIRE APIs; the peer's wire behavior may not.
No full BBS, adapter, scheduler, analytics or production enrollment is added.


## Independently exposed receive-capability bug

The real Python malformed-peer probe negotiated minor1/Core, then deliberately
sent a new generation-aware message into the native HOST's catalog-mapped RETRO
area. The accepted host returned ACK and imported it: reproducible
`unexpected-acceptance`, before any production fix. Existing sending gates were
correct, but incoming work checked only directed-routing. This violates the
published catalog capability requirement, not a request for a new wire feature.

The host now rejects unnegotiated conference_identity and Sysop generations without
catalog-access inside the same native import transaction as catalog-policy validation.
This avoids a check/import race with another link publishing a catalog revision.
Existing explicit trusted-offline/service callers retain their full-feature contract;
live callers now pass the negotiated capability set to atomic admission. The second probe exercises a catalog-sync
peer without catalog-access attempting SUPPORT. Both must receive
unsupported-version. Existing known compatible messages remain unchanged; no
schema/minor, signature, authority, or sender serialization change is required.
The first peer draft incorrectly assumed receipt fields followed batch direction.
Reconciliation of actual durable receipts established the reverse direction; the
specification and vectors were corrected explicitly before successful native
interoperability. This was a documentation/peer correction, not a production behavior
change. The final workspace regressions validate these fixes.


## Existing Health test clock correction

The first full final workspace run passed the live networking campaigns but exposed
an existing Health test race: the fixture captured `now` before SQLite's read
triggers ran. Crossing a second made the later observations correctly invisible to
the earlier snapshot, producing (0 readers, 0 progress, 3 posts) instead of (2, 3, 3).
Only the test changes: it deliberately backdates its fixture clock and then snapshots
at the actual maximum recorded observation time. This reproduces the time separation
without sleeping and preserves the original readership/reset assertions. Production
Health, retention and future-activity exclusion are unchanged. The failed run remains
part of the validation history; the corrected full gate is recorded below.

## Independent implementation and security review

The peer is original project-owned Python under MIT OR Apache-2.0, using only
standard libraries and generic cryptography. No sf-core/sf-net/sf-bbs import,
subprocess codec oracle, SQLite/native-board read, Rust serialization helper or
third-party BBS code supplies peer behavior. An AST import/call-boundary test and
source review check this separation. Native fixture setup and native-state assertions
use BBS services only on the SPITFIRE side. Public vectors are independently encoded
protocol data, not production-derived opaque serialization outputs.

TLS uses Python SSL/OpenSSL, TLS1.3, mutual enrolled certificate trust, DNS-name
verification, exact leaf DER comparison and ALPN. Fresh contexts disable tickets/
resumption and never enable early data/key logging. Connect/listen is hard-limited
to loopback; no hostname discovery. Every read/write has a remaining session budget,
with phase/frame/file bounds. Generic Ed25519 verifies the public signed body and
pinned network/catalog/publisher, never a key delivered by the peer.

JSON rejects duplicate/unknown fields, malformed UTF-8/surrogates, floating numbers,
invalid tokens and impossible paths. Golden bytes include literal non-ASCII text,
quotes, backslash, tab and CR/LF. Bounds include 4MiB batches, 32 messages, 16 controls,
8 file publications, 64KiB binary chunks, 64MiB payloads and 64 catalog revisions
per phase. The independent file tests cover truncation/overrun/hash mismatch with
no durable publication on incomplete streams. Full receipt replay keeps publication
identity separate from content hash-have. Local test admission is explicitly
synthetic; there is no scanner-clean claim or ordinary caller access surface.

Single-writer JSON custody uses copy-before-commit for atomic messages, replace/
fsync before acknowledgments, bounded metadata history, explicit lock recovery and
separate verified content. Content-directory synchronization precedes the publication
receipt commit where the platform supports directory fsync. No automatic history pruning, full native backup adapter,
HOST fanout, scheduler, private mail, file requests or topology migration is claimed.
A restored old reference-peer snapshot cannot reconstruct later lost receipt truth.
This limitation is documented rather than presented as a production-ready BBS.

## Actual independent journey

Apple Silicon acceptance uses a real SPITFIRE HOST USAZ000 and independent Python
END USAZ001, with synthetic ROOT in the configured tree. Certificates and signing
keys are generated only in temporary directories; the production catalog key is
never loaded. The peer is a separate process with its own storage. Both TCP directions
are exercised; this is not a second SPITFIRE instance or a Rust codec wrapper.

The journey proves TLS/Hello, bidirectional parents/replies, preserved native-to-wire
parent identity, directed final-node import, exact artifact replay, conflict rejection,
subscribe/unsubscribe and unauthorized/unknown-area results, binary Files in both
directions, hash-have in both directions, corrupt-byte rejected receipt, catalog
revision1→2 signature/chain/access sync and graceful daemon shutdown. It then lets
SPITFIRE initiate against the peer listener, drops an ACK after peer durable import,
and recovers on SPITFIRE's finite retry: one import, one duplicate observation,
no duplicate publication and no retransmitted known file payload.

Separate malformed-peer probes reproduce/reject unnegotiated generation and Sysop
access metadata. Independent TLS tests additionally reject wrong node/profile/leaf,
old catalog head rollback, invalid signature/wrong signer/fork, and accept unknown
optional capabilities/compatible minor1 Test mode. No independent multi-hop routing
campaign is claimed. Existing native C2–C8.1 regressions cover the accepted full
HOST/ROOT forwarding behavior separately.

## Specification reconciliation and retained boundaries

Current transport/Foundation/catalog docs link the consolidated implementation-neutral
specification and stop delegating canonical byte order to serde/struct definitions.
Exact receipt direction, canonical extension placement, message versus file fingerprint
path rules, nullable defaults, token normalization, catalog frame shapes and finite
wire error registry are now explicit. Local catalog diagnostic names are not extra
wire Error values. The historical 1.2/1.3 sections remain labeled introductions;
TLS1.3 is a transport version, not an obsolete CircuitNET minor. Current range0–4
and protocol1.4 agree throughout. Native schemas/security levels remain described
only as reference implementation mappings, not foreign implementation requirements.

The Network Kit retains its accepted onboarding structure and official metadata.
Revised build5 adds the two technical entry documents and a tiny PROTOCOL navigation
link; no Charter/catalog/region/addressing/application policy changed. Current
accepted official catalog revision2/publisher ROOT and public key remain byte-identical.
A fresh ZIP extraction verifies both signatures with the independent verifier,
81 files, 80 checksum entries and 30 ASCII/CRLF/79-column documents. Final release
archive must be regenerated with the final public source checkpoint; candidate
hashes are not release claims. Public services remain undeployed/unverified.

## Requested completion report

| # | Item | Result |
| --- | --- | --- |
| 1 | Starting private | ab2b0d5be6df59285fee5b3b1cb0708189a37890 |
| 2 | Starting public | 32bb39e103287722d0d8c908a5052f033a45328c |
| 3 | Schema | 36 → 36. |
| 4 | Protocol | 1.4 → 1.4. |
| 5 | Spec gaps | Canonical details, receipt direction, catalog frames, portable access, defaults/errors. |
| 6 | Resolved | Consolidated contract and public golden/semantic vectors; register above. |
| 7 | Peer language | Python 3.14.6, OpenSSL 3.6.3, cryptography 49.0.0; generic primitives only. |
| 8 | Independence | No production CircuitNET implementation imports, codec subprocesses or native DB reads. |
| 9 | Architecture | One direct link, own bounded JSON receipts/content store, explicit work, connect/listen. |
| 10 | TLS | TLS1.3 mutual auth, exact enrolled leaf, name checks, ALPN, no resumption. |
| 11 | Framing | Four-byte big-endian length/strict UTF-8 JSON, per-phase bounds. |
| 12 | Negotiation | Highest common minor0–4, expected identity/role/profile/mode. |
| 13 | Capabilities | Two mandatory, six optional; unknown names ignored within bounds. |
| 14 | Node identity | Existing normalized 1–8 ASCII alphanumeric; geographic assignment not required. |
| 15 | Envelope | Public fields independent of local message/conference IDs. |
| 16 | Message ID | Origin + random128-bit hex, per-network/class namespace. |
| 17 | Threading | Parent IDs, unresolved references, same-area/generation linking and cycle rejection. |
| 18 | ACK | Atomic durable acceptance; exact ordered members/hash, reversed receipt direction. |
| 19 | Replay | Exact artifact/member idempotency; no repeat native import. |
| 20 | Conflict | Changed fingerprint under ID rejects whole new batch. |
| 21 | Errors | Existing finite wire registry; no local paths/stack traces or invented wire codes. |
| 22 | Directed | Destination preserved; final-node path/no-broadcast tested; HOST fanout not claimed. |
| 23 | Dossiers | Subscribe/unsubscribe/query, authentication/results/replay; fixed test approval policy. |
| 24 | Files | Separate publication/content identity, streamed bytes and durable admission result. |
| 25 | Hash-have | Real native/Python verification in both directions, no payload bytes. |
| 26 | Integrity | Final size/SHA256; corrupt bytes rejected, truncated streams not published. |
| 27 | Catalog | Signed consecutive sync, locally pinned authority, schema/access preservation. |
| 28 | Signature | Standard Ed25519/domain + canonical body; synthetic and accepted kit signatures independently verified. |
| 29 | Rollback/fork | Signature, chain and old-head tests fail closed. |
| 30 | Access | public/sysops + required/lifecycle/generation; no portable level9999 meaning. |
| 31 | Profiles | Core, scoped Routing, Control, Files/hash-have, Catalog/access. |
| 32 | Baseline | Core; official-network participants additionally Catalog/access. |
| 33 | Golden vectors | Exact canonical bytes/hashes plus signed synthetic public catalog/key. |
| 34 | Malformed vectors | Prefix/UTF8/JSON/duplicate/unknown/type/token failures, semantic error vectors. |
| 35 | Properties | 500 deterministic bounded malformed byte inputs; 100 generated message round trips. |
| 36 | SPITFIRE → peer | Real native parent/reply/file/catalog accepted. |
| 37 | Peer → SPITFIRE | Real parent/reply/directed/control/file accepted. |
| 38 | Lost ACK | Native sender retries; peer reports one duplicate, no duplicate import. |
| 39 | Files result | Binary/hash-have both directions; corrupt stream rejected receipt. |
| 40 | Catalog result | Real native-parent revision1→2 sync; independent signature/access verification. |
| 41 | Multi-hop | Not performed with independent peer; full HOST Routing not claimed. |
| 42 | Production fix | Unnegotiated generation/access now rejected within atomic import transaction. |
| 43 | Neutral specification | docs/technical/circuitnet-ng-specification.md. |
| 44 | Implementation guide | docs/technical/circuitnet-implementation.md. |
| 45 | Runner | tools/circuitnet-conformance/runner.py, plus native Cargo campaign. |
| 46 | Report format | Version1 JSON, protocol/implementation/profiles/tests/outcomes/date/limits, no sensitive payload. |
| 47 | Tests added | 24 Python conformance tests; two Rust vector tests; one real independent journey with two negative probes; existing Health fixture clock correction. |
| 48 | Workspace totals | Final validation closure below. |
| 49 | macOS | arm64; disposable certificates/boards, loopback only. |
| 50 | C2–C8.1 | Final full workspace gate below. |
| 51 | Security | TLS, framing, streaming, signatures, atomic replay/capability review above. |
| 52 | Kit | Two technical documents/navigation, build5; onboarding retained. |
| 53 | Headers/fmt/Clippy/diff | Final validation closure below. |
| 54 | Links | Repository and actual extracted kit validated. |
| 55 | Provenance | Original code/public synthetic vectors; no private keys, proprietary source or private artifacts published. |
| 56 | cargo-audit | Executable unavailable; not claimed passed. |
| 57 | Final private | Publication closure below; closure checkpoint is Git HEAD. |
| 58 | Final public | Publication closure below. |
| 59 | Public delta | Publication closure below. |
| 60 | Protocol change | NONE; enforcement fix conforms to existing1.4 contract. |
| 61 | Synchronet/Mystic adapter | NONE. |
| 62 | Website/mail deployment | NONE. |
| 63 | Applications | CLOSED. |
| 64 | Production changes | NONE; implementation-source bug fix is not a production deployment. |
| 65 | External live traffic | NONE; only loopback BBS testing, read-only library documentation and Git publication tooling. |
| 66 | Exact next action | Stop for C9 review; no subsequent milestone. |

CIRCUITNET-NG is independently implementable through this published contract and
bounded peer proof. SPITFIRE NG remains the reference implementation, not protocol
authority by implementation accident. The peer shares no production CircuitNET
implementation code. Node IDs remain simple 1–8 character identifiers; C8.1 assignment remains
administrative, not a wire hierarchy. C1–C8.1 remain accepted. No public services or
production systems changed, and applications remain Closed.

## Private validation closure

The corrected full `cargo test --workspace` gate passes: **856 passed, 0 failed,
7 existing ignored**, including eight doctest groups. All seven native live
CircuitNET campaigns, the independent peer journey, FTN/QWK/Files/Health and schema
regressions pass. The initial failed Health fixture run is described above and
is not counted as a passing gate. A final focused native/Python journey also passes
after the peer's content-directory synchronization refinement.

The independent runner passes **24 tests, 0 failures, 0 skips**; kit/addressing
checks pass **12 tests**. The Rust public-vector suite contributes two new workspace
tests; the independent native journey contributes one. Bounded properties cover
500 malformed byte cases and 100 generated round trips.

Source headers **190**, `cargo fmt --all --check`, strict workspace/all-target
Clippy, `git diff --check`, and local Markdown links (**251 files / 1705 links /
0 issues**) pass. Cargo-audit remains unavailable. Accepted signed catalog/key,
schema 36, wire 1.4, localization and canonical public identity remain unchanged.
Only disposable Apple Silicon/loopback test systems were used. No private key,
proprietary historical source or private acceptance output enters the public
allowlist. Full public-workspace repetition is not inferred from this private gate;
the public checks are recorded separately.

Exact next action: push accepted private source, synchronize and validate the
reviewed public source, regenerate the final provenance-bound kit, then stop for
C9 review. No next milestone or public-service deployment is authorized.

## Public source synchronization

Accepted private source `5719d4413deaaa1b4b0261715f17f9b9f76b8998` was pushed first. This public checkpoint
contains the reviewed source allowlist and public navigation updates. Private
project history, raw historical inputs, credentials and disposable acceptance data
remain excluded. Public validation is recorded below before publication.

## Public validation closure

The sanitized public checkout passes the two Rust vector tests, corrected Health
fixture, and real native/Python journey (7.67 seconds). Both independent Python
suites pass 24/0/0; public kit/addressing checks pass12/0. Headers167, fmt, strict
workspace/all-target Clippy, diff and links179/1245/0 pass. All28 copied files other
than this report match the private accepted source byte-for-byte; this report adds
public provenance. STATUS/README/ROADMAP/docs index preserve the public tree’s
separate history. No private CURRENT-STATE, MILESTONES or session log was copied.

This is scoped public revalidation, not a second full-workspace claim. The private
full workspace 856/0/7 and eight doctest groups are recorded above. No credentials,
private keys or disposable acceptance outputs are published. Final kit generation
uses this public commit in RELEASE.TXT; the resulting archive/checksum are ignored
release artifacts, not committed ZIPs or a website deployment.

Exact next action: stop for C9 review. No subsequent milestone, production service,
external live CircuitNET traffic, third-party BBS adapter or application opening.
