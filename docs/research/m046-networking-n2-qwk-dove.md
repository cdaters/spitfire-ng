# M046 — N2 QWK networking and controlled DOVE interoperability

Status: **N2 COMPLETE / ACCEPTED**. Date: 2026-09-05.
Private starting checkpoint: `b9585f006b56afb8f2aab057a497aece6fcf8906`.
Public starting checkpoint: `fd743975d0c9865da57d7ebd1e2e75e68fc8c6d0`.
Private-mail continuation starts at `a272d911ef0373c7aedfa1e95c8392f1c5ee7d10`.
Schema **20 → 21 → 22** (this continuation **21 → 22**). [M044](m044-networking-foundation-gate.md) and
[N1](m045-networking-n1-qwk-offline.md) remain binding.

## Implementation contracts

Native messages remain authoritative. The existing `sf-net::qwk` archive and
record machinery supplies both offline and network workflows. Network metadata
is a bounded extension around those records, never a second message codec/store.
Network ingress is a configured link principal, never an authenticated caller.

The manual exchange boundary is a daemon-owned, explicitly invoked artifact
handoff. Configuration and operation requests use typed identities, expected
versions, authenticated operator capabilities and command receipts. Packet names
are derived from configured QWK IDs; packet metadata cannot choose a filesystem
path, partner, native caller, mapping, or destination. Successful handoff means
durable custody by the operator exchange endpoint, not proven remote import.
No socket protocol or scheduler is introduced.

Each link has a stable ID, network namespace, local and remote QWK IDs, profile,
direction policy, enabled state and version. Mappings explicitly bind a link wire
conference to a network area and native ConferenceId. Runtime state is relational:
publication identity, frozen routing decisions, queue entries, attempts, import
and export receipts, provenance and quarantine. Packet files remain artifacts.

Publication IDs survive retries and forwarding. Imported IDs retain their network
namespace and asserted origin; paths are ordered evidence, independently bounded
and checked for repeated/local/destination IDs. Ingress and previously served
destinations are excluded. Sharing a local conference never enables implicit
cross-network bridging. Reply IDs bind only to unique permitted native parents.

## Bounded secondary review

Synchronet is a read-only GPL-family interoperability reference. Relevant areas:
`docs/syncqnet.txt`, `docs/dove-net.txt`, QWK header definitions, packet packing,
QWK/REP unpacking and message conversion, QNET HTTP handoff, and field-name
serialization. No source, comments, UI, configuration or resources are incorporated.

| Finding | Disposition |
| --- | --- |
| QWK toward a downstream node, REP toward a hub; remote hub conference numbers | ADOPT interchange direction and explicit per-link mapping |
| Dedicated QWK network account at a Synchronet hub | ADAPT to native link principals; REJECT caller-account authority in NG |
| HEADERS.DAT sections identify exact hexadecimal message-header byte offsets | ADOPT bounded exact association and conflicting-metadata rejection |
| Message-ID, In-Reply-To and SenderNetAddr carry identity/reply/reverse-path evidence | ADAPT to native publications and retained ordered provenance |
| Unique short system IDs and path-based circular-route detection | ADAPT with durable per-destination receipts and namespace isolation |
| SMB-backed messages, scripts/semaphore control, arbitrary extra-file movement | REJECT as NG authority/dependencies |
| DOVE restricted conferences, no guest posting, plain-text interoperability policy | ADAPT only within explicit DOVE profile/mapping policy |
| Voting, MIME, attachments, dynamic routes and network-wide discovery | DEFER; unimplemented data must not be interpreted as ordinary accepted mail |

Official operational references: [DOVE joining](https://wiki.synchro.net/howto:dove-net),
[DOVE rules and conference policy](https://wiki.synchro.net/network:dove-net),
[QWK extensions](https://wiki.synchro.net/ref:qwk). Current online retrieval of the
extension page was unavailable; local source/documentation provides bounded
secondary evidence pending independent packet acceptance. DOVE conventions are
an application profile, not universal QWK requirements. No public network traffic
is authorized or sent by this work.

## Acceptance and boundaries

A real native macOS daemon and isolated independent Synchronet peer exchanged
public and private messages both ways. Peer-generated transit was imported into a
native transit container and routed only to the configured second partner.
Self-generated packets are not the final interoperability proof. Windows live
networking acceptance remains **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**.
B-021 remains VERIFIED; B-022 remains NOT STARTED. FTN/BinkP, directories, FileEcho,
doors, scheduler, DDEV, production, FireComm, private FidoNet corpus and release work
remain outside this pass. Public DOVE-Net traffic: **NONE**.

## Private QWK scope clarification and native completion

The user clarified that the earlier NetMail prohibition means **FTN/FidoNet NetMail
only**, and explicitly authorized M044's private QWK mailbox/transit obligation.
M044 is unchanged. Schema 21 could not express a native delivery without a
conference/public number; the minimal schema-22 rebuild adds checked native
containers and explicit private audience, enrollment, routing and envelope relations.
It preserves all existing native data transactionally and fabricates no history.

`MailPolicy` is typed per-link configuration with explicit native caller aliases,
destination systems, direction/transit permissions and CAS version. The authenticated
operator configures it through `configure-mail`; the caller cannot create a partner
or select its own authenticated author field. `NewNetworkMail` uses an authenticated
`MessageActor` and enrolled network alias. Creation and Pending publication/queue
intent commit together. Inbound private aliases map only to enrolled active callers.
An external author never becomes a local caller or Sysop identity.

The native payload/fanout/delivery tables own all public, mailbox and transit content.
Private containers have no conference or public number. `qwk_mailbox` and
`read_qwk_mail` are explicit authorized paths; ordinary conference views and offline
QWK do not expose them. Transit is inaccessible to ordinary callers and cannot be
made public by conference mutation. The bounded native service hooks provide send,
list/read and reply relationships; a stock terminal mailbox menu is not included.
No separate QWK private store, DOVE store/codec or SMB compatibility was added.

Private duplicate identity includes namespace, destination/recipient audience and
strong wire Message-ID, with retained origin/content-conflict evidence. Replay and
repacking reuse receipts/publications; transit publication and queue creation are
atomic with native import. Configured next hops, ingress/path exclusions and durable
receipts prevent return-to-origin and broadcast. Private replies link only when
native participant/origin evidence identifies one permitted parent.

The additional bounded read-only Synchronet review covered private sections of
`msgtoqwk.cpp`, `qwktomsg.cpp`, `un_qwk.cpp`, `un_rep.cpp` and the QWK conversion
function in `netmail.cpp`, plus its public JavaScript message interfaces. No FTN
branch was implemented. Dispositions:

| Finding | Disposition |
| --- | --- |
| Conference-zero private records and direct local recipients | ADOPT framing; ADAPT to explicit native mailbox enrollment |
| RecipientNetAddr, recipient@system, legacy private address-line container and reverse path | ADAPT bounded QWK-only normalization with configured next-hop authority |
| Numeric recipients, Sysop aliases and local command recipients | REJECT implicit privilege and ambiguous local recipient resolution |
| Dynamic learned routes and unrestricted private relaying | REJECT; explicit namespace/destination route and transit consent only |
| SMB mailbox internals, FTN/Internet address branches | REJECT as dependencies/implementation; no copied code or comments |
| Full mailbox terminal UI, private offline-reader integration and policy-stale retargeting | DEFER; native typed service hooks and safe Held behavior are implemented |

The shared codec retains private recipient/path evidence and never converts private
records into a public conference. Generic QWK private behavior is shared by DOVE
profile links; DOVE restrictions remain confined to public conference policy.
Quarantine retains malformed/unresolved artifacts with finite reasons. Events,
operator audit, queue/status and errors contain no bodies/subjects or private names.

## Demonstrated independent interoperability

The primary implementation acceptance host was Darwin arm64 with Rust host
`aarch64-apple-darwin`. The independent peer was Synchronet Terminal Server/JSexec
**3.19c HEAD/a5de4b9, 2023-04-25**, GCC 10.2.1, Linux amd64 in a disposable emulated
container. Synchronet's QWK engine is GPL-2.0-or-later; it is a secondary engineering
and interoperability reference, never an NG code/runtime dependency. The image
was the official-wiki-referenced `bbsio/synchronet` distribution, pinned in private
acceptance evidence. Its external network was disabled (`--network none`), no host
ports or host source/config mounts were exposed, and public hubs/timed events were
removed from its disposable internal configuration before acceptance. No DDEV or
production instance was touched.

SPITFIRE built REP through the real daemon's typed operator endpoint from native
public messages in two mapped areas. The independent peer's normal admitted QWK
account path unpacked the packet: **two messages, zero errors, zero duplicates**.
Its own message API confirmed mapped areas, public author, subject, native wire IDs,
origin path and UTC timestamps. Byte inspection confirmed CP437 café/box/block bytes
`82`, `B3`, `DB` exactly, with ordinary CRLF normalization at interchange.

The peer independently created replies in its own native authority and its normal
QWK packer generated the return packet. The same running disposable NG daemon
imported those replies, preserved their CP437 bodies (including the peer's generated
tagline as received), timestamps and external attribution, and bound In-Reply-To
to the original native parent messages. Reingest added zero native messages. The initial public-only continuous run passed in **14.58 seconds** after compilation.
Native-origin exclusion, same-origin return prevention, a second configured generic
partner, restart with Retry state, manual retry, Accepted handoff, partner disable,
invalid mapping, malformed/unknown/oversized input and cold restore receipts all
passed through the disposable journey or focused authority tests.

The first attempt to repeat an identical synthetic native body hit Synchronet's
own historical body-hash filter. Subsequent continuous runs used unique synthetic
publication content. NG did not adopt that body-hash-only identity convention.
The peer's new-account scan position also required explicit disposable history-scan
configuration. These were isolated peer setup issues, not public-network traffic.

Two demonstrated codec adjustments reused N1 machinery: a 4 KiB bounded small-member
compression floor allows Synchronet's highly compressed NETFLAGS.DAT, and the
network-only record marker accepts its documented packet convention while caller
profiles remain strict. To/Recipient aliases are checked for conflict. Compact and
T-separated offset-bearing WhenWritten timestamps are recognized. No second QWK
implementation, SMB parser/writer, copied peer source/configuration/UI/resource, or
external archiver was added to NG.

## Reproduction and retained evidence

The ordinary `cargo test -p sf-bbs --test qwk_network` journey needs no independent
peer and covers the disposable daemon/IPC/queue/restart/restore path. It is not, by
itself, independent interoperability proof. Optional `SFNG_N2_PEER_DIR` points to
private developer acceptance storage: the test emits a REP and consumes a separately
produced QWK. With `SFNG_N2_WAIT_FOR_PEER=1`, a bounded completed-response marker
lets an operator return a fresh peer packet while that same native board is running.
It then requires native parent linkage. The test does not launch arbitrary commands
or a scheduler from packet contents. Acceptance artifacts, peer identity/config,
credentials, logs and orchestration remain outside the repository.

For independent reproduction, configure a disposable network-disabled Synchronet
peer, admit a dedicated QWK node there, enable HEADERS.DAT, agree on two explicit
wire conferences and set a bounded initial scan. Deliver NG's completed REP through
that peer's normal admitted import path. Verify its native records, create replies
using its own public message API, invoke its normal QWK packer, and hand the resulting
QWK to the running NG test. The [Sysop manual](../manual/qwk-networking.md) gives NG
commands; the [Technical Reference](../technical/qwk-networking.md) owns exact policy,
limits, schema, privacy and recovery semantics. No private peer fixture is necessary
for automated workspace tests or a public source build.

## Private continuation acceptance and reproduction

The same controlled Synchronet **3.19c HEAD/a5de4b9**, GPL-2.0-or-later peer received
NG conference-zero private REP mail in its intended native mailbox, verified the
network sender and local recipient, and generated its own private reply. Its normal
QWK packer returned that reply with direct recipient framing, Message-ID/In-Reply-To
and source timestamp. The 3.19c peer emitted the legacy address-line transit
container; optional `RecipientNetAddr` support is covered by focused codec fixtures,
not falsely attributed to that peer packet. NG imported it only into native private authority and preserved
the original native parent. An unauthorized active Sysop caller could not read it.
The same packet also carried independently generated public replies and private
transit addressed beyond NG. Transit remained outside caller/public views and its
second-partner output retained the peer origin and private recipient. Independent
transit input and NG onward packet semantics were demonstrated; a second independent
BBS consuming that onward packet was not run.

The expanded real macOS journey exercises two configured partners, private/public
packet separation, unchanged public conference imports, repeated packet import,
origin exclusion, queued private restart/retry, disabled partner, unknown partner,
malformed/oversized packets, native mailbox authorization, cold backup/restore of
private queues, aliases, recipients and receipts. Focused tests additionally cover
repacked private duplicates, unresolved recipient/destination, local identity spoof,
atomic receipt failure, private reply routing, no-broadcast transit, return-to-origin,
policy changes and codec rejection of foreign address families. N1's real offline
QWK and automated caller authority/transfer/hostile-input tests remain part of the
workspace gate.

To reproduce the independent private extension, also enroll a synthetic native
mailbox alias in NG and create the intended private recipient on the isolated peer.
After peer import, verify its recipient, QWK origin and body using its own public
message API; generate a private reply addressed to NG and a transit message addressed
through NG to its configured second partner. Its normal QWK packer supplies the
return packet. The bounded integration hook verifies native private parent links and
onward private recipient/path. No peer scripts/configuration, private logs, packet
fixtures, credentials or local acceptance paths are published.

## Quality, publication and final boundary

Final private gates pass **597 tests / 0 failed / 2 existing ignored**, including
doctest phases; **103 headers**, formatting, workspace/all-target Clippy with warnings
denied, diff and **162 Markdown/local-target/anchor/fence checks** pass. The final
independent public/private/transit macOS journey passes in **15.55 seconds** after
compilation, including onward recipient/path and daemon-log privacy assertions.
Private/public commits and synchronization alignment are recorded in session closure. en-US advances to **1.19.0 / 1,047 messages**;
Modern/Minimal **1.6.0** and Classic **1.7.0** remain unchanged. Category B remains
**15 VERIFIED / 2 IMPLEMENTED / 3 PARTIAL / 5 NOT STARTED**. B-021 remains VERIFIED;
B-022 remains NOT STARTED. cargo-audit remains unavailable locally.

Native SPITFIRE NG message authority remains canonical. SMB is unimplemented.
The N1 QWK codec/parser/serializer is reused. Generic networking and DOVE share
that architecture. Duplicate and loop prevention are durable. Private content is
excluded from public areas and operator logs. Public DOVE-Net traffic: **NONE**.
FTN/FidoNet NetMail, EchoMail/addressing, BinkP, nodelists/pointlists, AreaFix,
FileEcho/TIC/FREQ remain unimplemented. No doors, scheduler, B-022, DDEV, production,
FireComm, private FidoNet corpus or release/tag/package/installer work began.
Windows live networking acceptance remains deferred under the standing policy.

Exact later N3 action: in a separately scoped pass implement M044's native FTN
address/domain/point/AKA plus packet/tosser/routing/directory vertical slice.
BinkP remains N4. This pass stops after accepted N2 and sanitized public synchronization.

## Sanitized source publication

Public starting checkpoint: `fd743975d0c9865da57d7ebd1e2e75e68fc8c6d0`.
Accepted private source checkpoint: `1f6250148d333566d76b72a405679f64553da93e`.
The publication copies 25 source/catalog files exactly, adds three rights-safe
networking documents and updates public operator/reference/status documentation.
Eight files are added and 43 existing files updated. Schema 20 upgrades through
21 and 22. Generic public/private QWK networking, native transit, shared DOVE
profile behavior, localization and synthetic tests are included.
Private continuity/history, original samples/corpora, peer source/config/content,
acceptance packets/logs/identities, credentials and local paths are excluded.
No private Git history is merged. Final public gates pass **535 tests / 0 failures /
2 existing ignored**, all doctest phases, **82 source headers**, formatting,
workspace/all-target Clippy with warnings denied, diff and **108 Markdown/local
target/anchor/fence checks**. Added-text privacy/path/secret/corpus scans are clean;
all 25 changed source/catalog files match the accepted private checkpoint exactly.
Windows runtime claims remain deferred; no release/tag/package is created.
