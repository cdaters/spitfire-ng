# Native FTN core

N3 implements native FTN message semantics before transport. This specification refines the accepted
[M044 networking contract](../research/m044-networking-foundation-gate.md).
Native messages remain the only message authority. Packet files are immutable
interchange evidence. BinkP, public network participation, FileEcho and AreaFix
are outside N3.

## Interfaces and ownership

`sf-net::ftn` supplies validated `Address`, `Domain`, `Endpoint`, `Packet`,
`PackedMessage`, `Text` and directory candidate types. It owns bounded conversion
only. Address components are unsigned 16-bit values; zone/net must be nonzero,
node zero is meaningful, point zero denotes the boss node. Domain spelling is
ASCII case canonicalized; distinct domains never share address keys.

`sf-core::ftn` owns native NetMail authoring/read authorization, EchoMail scans,
packet tossing, receipts, routing decisions, queue work and directory activation.
Methods accept validated board policy and explicit ingress/target IDs. Neither
packet names nor asserted author names grant authority. Runtime configuration owns
networks, AKAs, links, routes and directory source policy. Versioned relational
services own EchoMail mappings and local recipient aliases. Native message
payload/fanout/delivery rows own all bodies.

The service contracts are `send_ftn_mail(actor, policy, mail)`,
`read_ftn_mail(actor, message)`, `scan_ftn(policy)`,
`toss_ftn(store, policy, link, bytes)`, `build_ftn(store, policy, queue)`,
`ingest_ftn_directory(store, policy, source, edition, bytes)`, and
`lookup_ftn_directory(domain, address, now)`. Every command uses bounded input;
manual daemon dispatch uses existing protected operator IPC and command receipts.

Queue schema evolution removes the QWK-specific parent of the existing generic
outbound queue by introducing a typed work registry. Existing QWK decision IDs
and queue rows survive unchanged; adapter-specific decisions reference the registry.
FTN uses the same queue states, attempts, capacity and artifact custody.

## Selected interoperability profile

Required N3: FTS-0001.016 Type 2 packed messages and explicit FSC-0048.002
Type-2+ packet headers. The latter is a selected proposal profile, not an FTS.
Validated FSC-0039-compatible extended headers are accepted with their original
net value; FSC-0048's point sentinel uses auxiliary net. Type 2.2 is compatibility
later; Type 1 and Type 3 are rejected. Spare bytes never create a point identity.

FTS-4000 controls remain separate from visible content. Unknown syntactically valid
controls retain exact bytes and body paragraph position. Forwarding retains the
source charset; inability to preserve or represent content holds the target.
FTS-4001 NetMail addressing uses INTL plus FMPT/TOPT; EchoMail disregards these
controls and omits them on export. EchoMail source address comes from Origin and
remains an assertion, distinct from the admitted packet sender.

NetMail final destination survives routing. Precedence: local AKA, exact route,
direct configured link, explicit boss route, net route, zone route, domain default.
Equal-specificity configured ties are invalid. Every hop is an approved link;
directory endpoint data never grants link/security authority. Ingress needs explicit
transit permission. Local delivery needs a unique enrolled alias to an active native
caller. Unknown aliases quarantine. Transit has no caller-readable container.

EchoMail requires an explicit native conference mapping and link subscription.
Same-zone forwarding uses full SEEN-BY, ordered PATH and durable per-target receipts.
Points never become fake nodes or acquire a delivery receipt from their boss's
2D SEEN-BY alone. Cross-zone EchoMail gateway behavior remains N6 under M044.

## Directory profile

FTS-5000 full nodelists and FTS-5002 preferred Boss and combined Point formats are
N3 profiles. Poss is deprecated; fake-net and FidoUser compatibility and NODEDIFF
are later profiles, not silently interpreted as one of these formats. Full inputs
are sufficient for N3. Source date/year, encoding and format are explicit.
CRC checks canonical CRLF bytes as required by FTS-5000, before Unicode conversion.
ASCII is normative; CP437/CP866 are explicit regional compatibility profiles.

Candidates retain source digest, edition, parser profile, original records and
issues. Failed candidates never replace active generations. Source priority is
ascending numeric; equal-priority disagreement is an unresolved conflict. Corrections
are new generations and require explicit activation; previous good generations
remain recoverable. Directory data never authorizes network connections.

## Durable schema 23

The migration is a checked SQLite transaction from schema 22. It re-parents only
the shared outbound queue; all QWK IDs, versions, attempts, dates and bytes remain
unchanged. Foreign keys are temporarily disabled for the rebuild, checked before
commit and restored on failure. Tests inject a mid-migration conflict and migrate
populated QWK retry work. No FTN messages/history are fabricated.

| Authority | Relations |
|---|---|
| Typed endpoint keys | `ftn_addresses` (domain, zone, net, node, point) |
| Stable link identity | `ftn_link_bindings` |
| Local origin serials | `ftn_serials` |
| Conference policy | `ftn_area_mappings`, `ftn_area_links` |
| Private recipient enrollment | `ftn_mailbox_aliases` |
| Native provenance / duplicate identity | `ftn_messages` references canonical `messages`; no body column |
| Protocol evidence | `ftn_controls`, `ftn_echo_history` |
| Frozen target intent | `ftn_routing_decisions`, `network_queue_work`, existing `network_outbound_queue` |
| Admission receipts | `ftn_import_receipts`, existing `network_artifacts`, `network_quarantine` |
| Directory evidence | `ftn_directory_sources`, `ftn_directory_generations`, `ftn_directory_records`, `ftn_directory_flags`, `ftn_directory_services`, `ftn_directory_issues` |
| Effective source generation | `ftn_directory_active` |

Native mailbox/transit containers introduced in schema 22 are reused. FTN
provenance is distinct from `network_private_envelopes` (QWK). Both adapters use
native immutable payloads, fanouts and deliveries. QWK mailbox selectors require
the QWK envelope relation, preventing an FTN message from entering a QWK mailbox.

Static TOML owns AKA/domain/link/route/source intent. Domain and AKA records in
SQLite are stable references and serial state, not a competing policy editor.
Link identity cannot be rebound under a reused ID; use a new configured ID.
Directory source parsing/priority changes likewise use a new source ID. Mapping
updates use expected-version CAS and audited transactions. Alias enrollment is an
explicit audited replacement with one alias per caller/AKA and one recipient per
alias/AKA. There is no alias inference from login, handle or private real name.

## Packet and text contracts

All little-endian fields are accessed with bounded byte reads/writes. The Type 2
header is 58 bytes, each packed-message header is 14 bytes, and the date occupies
20 bytes including NUL. Sender/recipient fields allow 35 payload bytes and subject
71 payload bytes, followed by NUL. Message and packet terminators are checked.
No C/Pascal structure packing, unsafe casts or unbounded NUL searches are used.
The codec retains product data and Type 2 spare bytes. Original artifacts retain
all original bytes even when an extended header is normalized on serialization.
Product ID `00FE` is the FTSC product-list entry for no allocated product ID;
SPITFIRE does not claim another implementation's identity.

Known controls include MSGID, REPLY, INTL, FMPT, TOPT, CHRS/CHARSET, TZUTC /
TZUTCINFO, PID/TID, FLAGS, Via and PATH. Required delimiters and singleton rules
are validated. Invalid addressing, conflicting declarations and unrepresentable
characters quarantine rather than guessing. Unknown valid controls retain their
raw byte sequence and paragraph position. There is no execution or configuration
interpretation. A bounded forward may add Via or regenerate PATH/SEEN-BY; capacity
failure does not silently discard an unknown field. Body authoring cannot inject
SOH controls. A visible literal AREA line after native controls remains body text.

ASCII, CP437, CP850, CP866 and UTF-8 conversions are strict. High-byte tables are
independently derived from standard codec mappings, with exhaustive round-trip
coverage. CHRS selects the network text encoding; absent CHRS uses explicit link
policy. Locally authored NetMail uses its selected route's encoding; local-only
mail uses UTF-8. Local conference encoding becomes explicit CHRS at publication.
Forwarding retains the admitted source encoding and metadata. UTF-8 uses the
current level-4 convention, regional code pages level 2 and ASCII level 1.
Native imported payloads are Unicode; existing CP437 native conference bytes are
not silently rewritten. Unrepresentable outgoing fields fail explicitly.

The packed date's two-digit year uses an explicit 1980–2079 window. TZUTC gives a
signed numeric minute offset, normalized into UTC; original date and offset remain
in provenance. Missing timezone keeps native receipt UTC as the safe timestamp
and retains unresolved original wall time. No current timezone, machine locale,
or guessed DST transition assigns an offset. Native exports use explicit UTC.

EchoMail removes recognized trailing tear/origin/SEEN-BY from the human body and
stores them separately. A quoted SEEN-BY in ordinary prose remains body text.
Local origin text comes only from the configured mapping plus its typed AKA;
remote Origin is asserted provenance, never authenticated local identity. A
supplied origin domain must match ingress namespace. Standard NetMail attributes
are retained in provenance; exports set private semantics. Local received/sent,
local/delete-sent/orphan status does not authorize delivery. Crash, file attach,
file request, hold, reserved/direct-routing, receipt/audit requests and update requests
are rejected by N3 pending their separate policy owners. EchoMail with private
attributes is rejected. FLAGS supports only PVT; other routing/security tokens
are rejected rather than treated as inert.

## Atomic message processing and loops

A caller's NetMail publication and target queue intent commit together. A scan
selects eligible native conference deliveries, assigns an origin serial once and
commits provenance plus all target decisions in one transaction. Already published
native deliveries cannot be scanned again for that network. Incoming QWK messages
are not implicitly bridged into FTN. A tosser validates framing before individual
member transactions, then commits native payload/delivery, provenance, receipts
and any forwarding intent together. A failed member retains a quarantine receipt;
SQL/receipt failure rolls the member back and permits retry. Framing failure
quarantines the whole artifact. Exact receipt replay reports suppression, not a
second import.

Strong duplicate identity is domain + delivery scope + MSGID. Echo scope is the
AREA tag; private scope is final destination and normalized recipient alias.
A fingerprint also covers typed source, final destination for NetMail, attribution,
subject, normalized body, date, offset, attributes and reply identity. Terminal
newlines do not turn a legitimate repack into changed content. Same MSGID with
changed meaningful content quarantines. Without MSGID, exact artifact replay is
safe; an identical weak fingerprint from a new artifact quarantines as ambiguous.
Thus suppression is never based solely on a body hash. Strong and weak identity
retain points and domain separation.

Local serials are durable monotonic 32-bit values per domain/address. Exhaustion
or restored uncertainty holds origin allocation. Retained identities satisfy the
M044 minimum three-year horizon; N3 has no destructive history prune. Shared
row/byte budgets refuse admission instead of silently forgetting history.

Resolvable REPLY metadata binds a native parent only within the same public area
or the authorized private participant relationship. Ambiguous/unresolved network
REPLY remains metadata. Local conference replies project an existing same-domain,
same-area parent MSGID. Native identity is never replaced by MSGID.

Echo forwarding excludes ingress, asserted source, seen node targets and already
receipted targets. Ordered PATH detects return through a local node; point receipts
remain distinct from boss-node SEEN-BY. Export adds the local node and selected
node recipients, preserves bounded full same-zone history, and emits physical
lines with repeated net/node pairs. A point has no invented 2D point entry. Its
boss may anchor otherwise empty 2D history, without suppressing delivery to that
point. Independent SBBSecho forwarding back to an additional AKA and synthetic
multi-hop PATH tests exercise return suppression.

NetMail route choice preserves final destination separately from packet next hop.
Transit requires ingress permission, cannot return to ingress, and checks bounded
Via/local-hop history. A link or route cannot silently create trust through a
nodelist hostname. Directories supply effective typed address metadata for operator
resolution; explicit configured links remain the routing/security authority. No
live endpoint lookup, probing, connection or mailer exists in N3.

## Queue, recovery and resource bounds

Each FTN decision freezes publication, link, local AKA, final destination, next hop,
policy digest, mapping version, native delivery version and routing reason. One
immutable packet is built per queue item. Build retries reuse its artifact; native
lifecycle/version and policy changes prevent stale work from becoming ready.
Shared queue states and attempt fields are retained for the N4 consumer. N3 stops
at ready artifact custody: it does not claim remote receipt or successful transport.
Policy changes and restore hold uncertain unsent work; release/reconciliation is
not an unaudited SQL workflow.

| Input / authority | Bound |
|---|---:|
| Packet or directory artifact | 16 MiB |
| Messages per packet / native scan | 1,000 |
| Human message body | 64 KiB |
| Packed text including controls | 80 KiB |
| Controls | 128; 16 KiB aggregate; 1 KiB each |
| SEEN-BY / PATH entries | 1,024 each |
| Via hops | 128 |
| Directory physical records / line | 100,000 / 2 KiB |
| Configured directory sources / retained source generations | 16 / 8 |
| Shared outstanding queue | 10,000 / 256 MiB |
| Per-link outstanding FTN queue | 1,000 / 64 MiB |
| Shared quarantine | 128 records / 128 MiB referenced bytes |
| Shared receipt/history budget | 2,000,000 receipts / 512 MiB reservation |
| Status queue page | 100 items |

Artifact custody also retains the generic N1/N2 storage/admission bounds. Failed
capacity checks do not discard historical source artifacts or committed mail.
Directory candidates parse fully before storage/activation; activation changes
one source pointer transactionally. Malformed candidates and equal-priority
conflicts remain inactive, with originals and classified issues preserved.
Disabled or policy-superseded sources do not veto activation of a replacement.
Lower-priority observations remain available as provenance. Previous good
snapshots stay recoverable. Staleness is two cadences warning, four unusable.

Cold backup includes static policy, relational FTN authority and immutable packet/
directory artifacts. Restore validates SQLite and artifacts, excludes manual
handoff slots, retains receipts/active generations and holds unsent queues and
origin allocation. No live transport state exists. A restore beyond previously
sent history requires reconciliation before N4 delivery; rollback cannot prove
exactly-once delivery to a peer.

## Operator, privacy and reproduction

Protocol minor 7 negotiates `FtnNetwork`; N2's minor-6 QWK feature remains separate.
Typed manual actions use protected local IPC, explicit capabilities and existing
command receipts. `sfconfig --apply-ftn` uses ordinary candidate validation/CAS;
read-only sfmonitor adds only counts. No body is present in command payloads,
status, audit, event or quarantine summaries. Caller NetMail composition remains
a typed authenticated service; a broad caller composer is deferred. EchoMail uses
existing native conferences unchanged. See the [Sysop manual](../manual/ftn-core.md).

Run ordinary synthetic coverage with `cargo test --workspace`. For isolated peer
acceptance, supply private independent artifacts in `SPITFIRE_FTN_PEER_DIR`, with
`peer-netmail.pkt` and `peer-echo.pkt`; run:

```text
cargo test -p sf-core independent_peer_packet_exchange -- --ignored --nocapture
cargo test -p sf-bbs --test ftn_core -- --nocapture
```

The first test emits native packets to that private directory for independent
inspection/tossing. Optional `peer-return.pkt` and `peer-transit.pkt` extend actual
peer-loop and transit checks. No peer fixture bytes are published. The ordinary
daemon test runs without external artifacts, and the separately recorded peer
run proves bidirectional independent interoperability. See
[M047](../research/m047-networking-n3-ftn-core.md) for pinned standards, peer
version/license, exact acceptance outcomes and remaining N4 boundary.
