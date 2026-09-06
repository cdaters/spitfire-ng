# M048 / N4 — native BinkP mailer

Status: **COMPLETE / ACCEPTED** (2026-09-06). Private/public Git closure is recorded below.

This is the canonical rights-safe report for native BinkP transport, controlled
leaf/node/point interoperability, restart/restore and bounded operator surfaces.
[M044](m044-networking-foundation-gate.md) and [N3](m047-networking-n3-ftn-core.md)
remain binding. [Technical Reference](../technical/binkp.md) owns exact contracts;
the [Sysop manual](../manual/binkp.md) owns configuration and recovery instructions.

## Checkpoints, authority and references

Private start: `ccacf8d114dfedcf22ef4d29585d1746d1268dd3`.
Accepted N3 private source published publicly:
`fa44a3aca2c66825f5e291c31c66aaa385a3c287`.
Public start: `c27855c5388d222211453cb536b0e55cd1210afe`.
Schema **23 → 24** adds transport claims/results around the existing shared queue.
Category B remains **15 VERIFIED / 2 IMPLEMENTED / 3 PARTIAL / 5 NOT STARTED**;
B-021 VERIFIED, B-022 NOT STARTED. Native SPITFIRE message authority is canonical;
BinkP does not own routing, message semantics, duplicate identity or final addresses.

Primary documents actually used:

| Document | Revision/date/status | N4 use |
| --- | --- | --- |
| [FTS-1026](https://ftsc.org/docs/fts-1026.001) | 1, 2005-12-01, published FTS | BinkP 1.0 framing, commands, address/password flow, offer/GOT/GET/SKIP, EOB and state/error behavior |
| [FTS-1027](https://ftsc.org/docs/fts-1027.001) | 1, 2005-12-01, published FTS; optional wire extension | CRAM challenge/digest and known vector; required by NG's default link profile |
| [FTS-1028](https://ftsc.org/docs/fts-1028.001) | 1, 2005-12-01, published optional FTS | Evaluate NR; defer advertisement and persistent partial resume, retain base M_GET behavior |
| [FTS-5001](https://ftsc.org/docs/fts-5001.006) | Header says 6 draft 10, 2017-08-13; status says FTS | Existing N3 INA/IBN directory service metadata and endpoint precedence |

The pinned private files were read as primary evidence; a web fetch was unavailable,
so no claim of newly verified online revision is made. FSP-1024's 1.1 draft is not
silently promoted over the selected 1.0 standard. The private corpus index records
full identifiers, hashes and provenance without copying standards passages. Current
inventory is 382 files / 8,599,056 bytes: all 381 prior indexed files are unchanged,
plus an informational NodelistDB OpenAPI 3.0.3/API 1.0.0 document inspected for
metadata only. Its collection date is unknown; no API service was called.

Secondary sources were bounded and read-only:

| Reference | ADOPT / ADAPT | REJECT / DEFER / implementation-specific |
| --- | --- | --- |
| Downloaded Synchronet BinkIT wiki (page revision 2026-05-04) | ADAPT sysop link/AKA/password/poll terminology and explicit endpoint overrides | REJECT global AKA guessing, DNS suffix fallback as NG policy and copying configuration; DEFER timed polling, CRYPT/TLS |
| Synchronet `exec/load/binkp.js`, BinkIT callbacks and FTN address utility | ADOPT independent real client/answerer acceptance; ADAPT duplex callback/custody boundary and explicit AKA selection | REJECT BSO file deletion as NG queue authority, SMB dependency, frame debugging and source copying; eight-character legacy domain handling is peer-specific |
| NodelistDB BinkP tester | ADAPT bounded reachability/latency and separate address validation | REJECT anonymous test success as mail authorization, remote text as trusted status and analytics/runtime dependency |
| FireComm transport interfaces and Phase 10 transport document | ADAPT bounded worker ownership, cancellation and handshake/application separation under standing cross-project rule | TLS trust implementation is FireComm-specific; no code/dependency/edit imported |

Reference checkout identities: Synchronet `60b70e526213285b61ae11f82dc8412ad5b405b4`,
NodelistDB `a3ad2bc6114d4ec8bcca846bdac32852e9e2e9dd`, and FireComm
`9bb51c6abe7a3dc957e73eb1b0b7430b2d293491`.

FTSC stayed primary. Policy documents provide governance context only. No reference
source, comments, UI or configuration text was copied into the implementation.

## Implementation accounting

The numbers below correspond to the requested private final-report checklist.

| Items | Result |
| --- | --- |
| 1–3 checkpoints/schema | Starts above; transactional schema 24; no fabricated transport history |
| 4–6 evidence | Primary and secondary review table above; prior indexed corpus unchanged; additional local API metadata indexed |
| 7–8 architecture/state | Pure bounded `sf-net::binkp` codec/auth primitives; daemon finite session engine; native core claims and health; explicit connect/greet/authenticate/ready/exchange/finish/complete/fail phases |
| 9–11 client/listener/link | Native outbound TCP client and daemon inbound listener; static typed link extension refers to N3 links, local AKA set, remote aliases, endpoint/directory/port, roles and auth policy |
| 12–14 addresses/points/AKAs | Full domain+zone/net/node/point matching; expected primary remote required; only configured/presented remote aliases admitted; explicit local AKA advertisement; domainless compatibility opt-in |
| 15–17 authentication/TLS | Required CRAM-MD5 default; explicit AllowPlain compatibility; fresh random challenge and constant-time RustCrypto HMAC verification; no silent downgrade; TLS/CRYPT deferred; base CRAM is not mutual authentication or encryption |
| 18 capabilities | BinkP 1.0 with CRAM; recognize bounded safe peer option tokens, ignore unknown optional tokens; never enable unimplemented extensions |
| 19–24 artifact and queue flow | N3 native scan/routing → existing queue → immutable packet → BinkP M_FILE/data → matching M_GOT → durable acceptance; complete private inbound bytes → immutable custody → N3 toss/quarantine → M_GOT; claims overlay shared queue, not a parallel queue |
| 25 retry/backoff | Durable 5-minute exponential retry, 6-hour cap plus 0–30-second jitter, 12 attempts/7 days; security/custody failures held; no automatic scheduler |
| 26–27 Poll/Test | Typed asynchronous local operator commands; explicit network-run and separate network-test authorization; Test never claims or accepts local mail and declines eager offers |
| 28 partial transfer | Base outbound M_GET reseeks immutable offer; incoming nonzero offset requests restart at zero; private memory discarded on interruption; persistent partial resume/NR deferred |
| 29–30 bounds/timeouts | 16 MiB packet, 64 files, 64 MiB per direction, 32 addresses/options, 4,096 commands; 10-second connect, 30-second handshake/auth, 60-second idle, 600-second session, bounded abort/drain |
| 31–32 concurrency | Two daemon-wide sessions including unauthenticated admissions; one durable session per link; queue uniqueness prevents same-item workers; busy refusal; restart invalidates stale leases |
| 33 endpoint resolution | Explicit override first; usable active directory IBN host or INA+IBN; no system-name guesses, public FTN DNS fallback or download scheduler |
| 34 health | Durable last attempt/success/error, latency, held/backoff, safe admitted addresses/options and queued count; active is tied to daemon session generation |
| 35 sfmonitor | Minimal read-only Networks dashboard link summary, queued/last contact/safe localized state; no full N5 cockpit |
| 36 sfconfig | Typed BinkP policy import online/offline; write-only online credential update/explicit clear; sensitive changes audited; listener changes restart-aware |
| 37 secrets | Missing/Configured/Invalid only; private atomic files, no secret in Debug/status/events/audit; credential receipt fingerprints omit value and are not password verifiers |
| 38–39 observability/audit | Finite session/auth/identity/custody/complete/failure/hold/retry events; privileged typed configuration, credential, poll/test/queue actions use existing audit and CommandId receipts |
| 40 threat review | Explicit resource/auth/spoof/filename/replay/slow-peer/flood/race/downgrade/NetMail matrix in Technical Reference |
| 41–44 shutdown/restart/restore | Existing daemon drain owns workers; no new polls/admissions during shutdown; private partial memory discarded; stale claims recover to safe retry; restore changes generation, clears active claims, retains accepted receipts/credentials and holds unsent work/origin serials for explicit review |
| 45–57 interoperability/security outcomes | Detailed independent and automated evidence below |
| 58–60 macOS/regressions | Native Apple Silicon journey and full workspace QWK N1/N2 + FTN N3 gates below; Windows live behavior deferred |
| 61–62 tests/totals | Codec/vector tests, socket auth/identity/duplex/partial/no-ACK/limits/idle tests, native queue/directory/migration/recovery/secret tests and actual daemon exchange/backup restore; final totals below |
| 63–66 localization/docs | en-US 1.21.0; Sysop manual, Technical Reference, this canonical report, architecture/privacy/recovery/operator/index/continuity updates |
| 67–70 gates/provenance/audit | Final gate record below; private corpus and peer evidence excluded from publication |
| 71–73 status/bounded scope | Acceptance status at top; client/listener, Test/Poll, queue hold/release, health and small surfaces implemented; TLS, persistent partial resume, periodic polling and full Networks TUI deferred |
| 74–75 Git | Closure record below; private main first, then sanitized public copy, no private Git history import |

## Controlled independent peer and exact demonstrated level

The independent peer is **Synchronet 3.19c / SBBSecho 3.15**, using its native
JavaScript **BinkP revision 4** implementation (Synchronet GPL distribution
policy; SBBSecho GPL-2.0-or-later), compiled image
revision **a5de4b9**. The pinned container image digest is
`sha256:8b5da5117126f31a4209fd1ca4101febd67c5a23655c9955b516b09685f66564`.
The read-only current source reference has a later BinkP revision; results are
attributed to the actual exercised peer, not that newer checkout.

The host is Apple Silicon macOS; the cached peer image uses its existing amd64
runtime. It runs in a disposable Docker **internal** network with no external
route and no production/DDEV connection. Internal Docker networking disabled
published host ports, so private acceptance infrastructure relayed raw bytes
between host loopback and peer loopback through `docker exec` pipes. The relay
contains no BinkP semantics. Both real implementations own framing, authentication,
file transfer and acknowledgement over TCP sockets. This is controlled isolated
protocol interoperability, not a claim of public Internet reachability or Docker
port-publication support.

An independently authored private harness calls the peer's BinkP library public
API in caller and answerer roles. It supplies explicit test AKAs and a random
private credential, rejects failed CRAM/identity, disables peer CRYPT, and records
only safe counts/results. The peer's MsgBase and SBBSecho provide independent
message generation/import. Acceptance configuration, credentials, packets, message
bases, relay scripts and logs are private evidence, never public project assets.

| Journey | Evidence/result |
| --- | --- |
| SPITFIRE → peer NetMail | Native private service authors, N3 routes/scans/builds, Poll connects to independent answerer, M_GOT marks queue accepted; SBBSecho imports into private MsgBase with exact human body, private attribute and point origin |
| SPITFIRE → peer EchoMail | Native conference author/scan and the same BinkP session deliver a second packet; SBBSecho secure inbound imports the configured echo into its native sub-board |
| Peer → SPITFIRE NetMail | Peer native MsgBase/SBBSecho create packet; independent caller connects to NG listener; automatic N3 toss creates native private delivery; authorized caller reads exact body and unrelated caller is denied |
| Peer → SPITFIRE EchoMail | Independent exported packet arrives in the same batch and automatically imports into the mapped native conference; controls do not appear as body garbage |
| Point and multiple AKA | Actual BinkP sessions in both connection roles use a local point identity plus node AKA; peer presents node plus point; packet source/destination point controls remain N3 authority |
| Duplicate/replay | Independent inbound batch sends three files representing two messages; native receipts record two imports and one suppressed duplicate; a second three-file delivery after native restart remains exactly two native imports |
| Authentication/address failures | Real TCP tests use wrong password and wrong point address; independent Synchronet caller also tried wrong password and wrong presented address. All fail before custody with zero sent/imported mail; a real daemon restart retains failed/held health and no active session |
| Interrupted outbound | Real socket peer consumes all file bytes then closes without GOT; accepted count stays zero; retry succeeds; durable core retry/claim tests retain exact immutable artifact |
| Interrupted inbound | Authenticated peer sends all but one byte then closes; no tosser callback/import; complete retry imports once |
| Malformed/resource/idle | Authenticated traversal and oversized offers fail without custody; stalled handshake times out; codec rejects malformed/truncated/oversized commands and unsafe filenames |
| Native daemons/restart/restore | Two real NG daemons exchange point NetMail and EchoMail both ways, poll empty queues without duplicate, Test Link without custody, graceful shutdown, cold backup/restore, new daemon/listener, Configured credential and preserved accepted queue; held unsent packets released and delivered, active directory retained, and a live stalled greeting worker cancelled during graceful drain |
| Queue/directory reconciliation | Core tests clear active claims/generation, hold restored work, reject stale release CAS, release frozen artifacts under unchanged routing, preserve origin hold; active directory provenance and explicit endpoint precedence remain valid |
| Operator surfaces | Real sfconfig typed policy application passed; its hidden credential prompt updated the private secret without echo; audited capability enrollment enabled real sfmonitor Networks display; independent authenticated Test Link took no mail custody |
| Privacy | Native access checks deny unrelated readers; audit/events/quarantine/health scanned for the private body and actual credential; safe option capture ignores secret-reflection tokens |

The accepted path does not manually copy packets into or out of SPITFIRE. Peer-only
staging between BinkP library callbacks and SBBSecho is part of the independent
harness, not an NG runtime dependency. Initial private harness/relay defects were
corrected: legacy peer domains truncate to eight characters, invalid callback
password return handling required explicit rejection, secure EchoMail belongs in
the peer secure inbound, and the byte relay needed a blocking stream after its
bounded connection. Corrected acceptance was rerun; earlier attempts are not counted
as passing evidence. No Synchronet behavior overrides FTSC semantics.

## Reproduction and final quality gates

Run the normal source-header, fmt, workspace tests (including doctests), Clippy and
diff gates. `cargo test -p sf-bbs --test binkp` exercises real local daemons; socket
unit tests exercise protocol failures and interruption. The independent opt-in
integration test requires an explicitly isolated private peer and is ignored in
ordinary/public workspace runs. Its outbound phase retains a disposable native
board; its inbound verification phase checks automatic private/conference import,
permissions, duplicate count and privacy after independent peer delivery. Private
acceptance scripts provide the peer setup/relay; they are not public dependencies.

Private acceptance: **640 passed / 0 failed / 4 ignored**, including doctests.
Twenty new automatically run tests extend the accepted 620-test N3 workspace.
The new ignored independent journey passed explicitly in outbound, authenticated
operator enrollment and inbound verification phases. Independent final Test Link
and replay also passed after restarting the final native build. The other ignored
checks remain their existing opt-in cases; no Windows live claim is inferred.

**120 source headers**, `cargo fmt --all --check`,
`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`git diff --check`, and **168 Markdown/local-link/anchor/fence documents** pass.
The workspace gates used the cached dependencies in offline mode; local socket
acceptance ran with the required macOS permissions. en-US is **1.21.0 / 1,071
messages**. `cargo-audit` is **unavailable** (`cargo audit --version`: command not
installed), not a claimed security scan pass.

The actual private credential and NetMail sentinel are absent from diagnostic,
audit, health and quarantine summaries. Added source/docs contain no peer secret,
packet dump or imported reference code. The private corpus's 381 prior hashes
match exactly; its additional API metadata is indexed, not published.

Private main is committed/pushed before sanitized public synchronization. Public
quality/file/Git accounting is appended when that separate validation completes.

## Boundaries and next action

Checklist 91–102: live public FidoNet traffic **NONE**. AreaFix, FileEcho/TIC/FREQ,
hub subscriptions, general scheduler, B-022, doors, DDEV, production, FireComm edits,
OS service packaging, releases/tags/distribution binaries and public announcements
were not begun. The private standards/wiki/source/packet/credential corpus is
unpublished. SMB remains unimplemented. N1/N2/N3 retain canonical native authority.

Windows live listener/runtime, multi-session and sfmonitor/sfconfig acceptance is
**DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED** under standing policy. Portable
source architecture is retained; no Windows VM/cloud runner effort blocks macOS.

After accepted private and sanitized public closure, **stop N4**. The next separately
scoped action is **N5 operator/network surfaces and explicit recovery ergonomics**
over the accepted N1–N4 authorities. It does not authorize public FidoNet onboarding,
AreaFix/FileEcho, scheduling, B-022 or release work.

## Sanitized public source and gate accounting

| Requested items | Result |
| --- | --- |
| 76 public start | `c27855c5388d222211453cb536b0e55cd1210afe` |
| 77 private source | `d95b853a17dc73cc358c49ec7c794b9a6882d27f`, pushed to private main and verified aligned/clean before public staging |
| 78 files | **9 added / 43 updated** public files; 31 shared source/catalog/manifests byte-identical to private source; Cargo.lock differs only by removal of private `sf-date-research` and `sf-reg` package records |
| 79 schema | Transactional schema 24 transport claims/health/observations/custody published; no scheduler/FileEcho/AreaFix authority |
| 80 BinkP | Native codec/auth/client/listener, N3 queue/tosser integration, directory resolution, retry/hold/release and recovery published |
| 81 operator surfaces | Typed policy, write-only credential update/clear, Poll/Test/Hold/Release and minimal sfmonitor read-only Networks summary |
| 82 docs/localization | Sysop manual, Technical Reference, rights-safe report, current public status/roadmap/architecture/privacy/recovery and N3 cross-references; en-US 1.21.0 / 1,071 messages |
| 83–84 public tests | **578 passed / 0 failed / 4 ignored**, doctests included; independent opt-in exercised explicitly in private isolated acceptance; 20 new automatically run tests over N3's 558 |
| 85 gates | **99 source headers**, fmt, workspace tests, Clippy all targets with warnings denied, and diff checks pass |
| 86 links | **114 Markdown/local-link/anchor/fence documents**, zero errors |
| 87 privacy/provenance | Added-text path/key/token scan, actual private-credential absence, corpus-digest scan and file allowlist pass; no standards/wiki/source/peer bytes imported |
| 88 Windows | Live BinkP and operator behavior remains DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED; no Windows VM/cloud/long CI acceptance attempted |
| 89–90 Git | Independent public history based on the public checkpoint; ordinary main push, no force or private history import; final commit/alignment recorded in the handoff |

Excluded: private CURRENT-STATE, MILESTONES, DECISIONS, SESSION-LOG and corpus index;
all private standards, downloaded wiki/source, NodelistDB source/API corpus,
acceptance identities/configuration/credentials, packets, logs and uncertain-rights
nodelist samples. Published test fixtures are independently authored synthetic
examples, not peer message bases or private acceptance assets. No binary, tag or
release changed.
