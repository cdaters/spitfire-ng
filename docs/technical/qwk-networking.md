# QWK network partners, native publications and manual exchange

N2 implements public conference, private mailbox and configured private transit
exchange with QWK network partners. [M046](../research/m046-networking-n2-qwk-dove.md)
records controlled acceptance and its limits.
[Caller offline QWK](qwk-offline.md) remains a separate consumer of the same codec.
Native SPITFIRE NG messages remain canonical. SMB parsing, writing, storage and
runtime compatibility are absent. DOVE-Net is a QWK networking profile, not a
second codec or message database. FTN and BinkP are not implemented.

## Authority and interfaces

`sf-net::qwk` continues to own ZIP framing, records and CONTROL.DAT serialization.
`sf-net::qwk_network` adds bounded HEADERS.DAT association and metadata around that
codec. `sf-core::qwk_network` owns relational policy and native transactions.
`sf-bbs::qwk_network` authorizes on-demand operations through operator protocol 1.6.
The host owns confined filesystem custody. See the [Sysop procedure](../manual/qwk-networking.md).

`Link` contains a stable local link ID, network namespace, local and remote QWK
system IDs, display name, profile, partner role, enabled/inbound/outbound policy
and configuration version. Roles identify the **partner**: `hub` means outbound
REP/inbound QWK; `node` means outbound QWK/inbound REP. IDs do not derive from a
path, process, node slot, caller account or packet display name. Local IDs are
consistent across links in one network. Network/system identity and profile/role
are immutable after creation; a changed identity requires a new link.

`Mapping` binds a partner wire conference to a stable network area token and native
ConferenceId. Native numbers need not equal wire numbers. Duplicate wire/native/area
mappings fail. Shared namespace/area bindings must agree across links. Only active
native areas can be mapped. A mapped area may permit local private messages, but
network export admits only active public All Callers deliveries. Private and
addressed-local deliveries are excluded. Changes use compare-and-swap versions and
hold unsent work. This is typed relational configuration, not generic JSON storage;
JSON is only the typed IPC/CLI request representation.

The manual operator endpoint is the authenticated exchange context. Packet claims
never grant caller or operator authority. Local author exports use public handles;
network imports use external attribution with a null local author reference and
`origin_kind=external-network`. System-authored native seed messages are excluded.
Imported content shares the existing native payload/fan-out/delivery services and
conference authorization; no fabricated caller or Sysop account is used.

## Schema 21

Schema 20 remains the offline authority. The next transactional migration adds:

| Relation | Authority |
| --- | --- |
| `qwk_links`, `qwk_link_mappings` | Typed versioned partner and mapping policy |
| `qwk_link_state` | Last contact and finite result |
| `network_publications` | Native MessageId, namespace/area, stable wire ID, origin, ingress, reply, source time and recipient evidence |
| `network_publication_path` | Bounded ordered reverse route |
| `network_routing_decisions` | Frozen destination, mapping/link versions, native message version and policy digest |
| `network_outbound_queue` | State, artifact, version, attempts, due time and reserved capacity |
| `network_delivery_attempts` | Per-entry monotonically numbered handoff attempts |
| `qwk_network_import_receipts` | Partner/canonical packet/member outcome and native/publication references |
| `network_quarantine` | Retained artifact and finite failure reason |
| `network_history_capacity` | Conservative reserved history capacity; no fabricated exchange history |

Native `messages.origin_kind` defaults to `native`; payload `encoding` defaults to
`cp437`. Existing native bytes, IDs, numbers, permissions and N1 receipts remain
unchanged. New imported Unicode payloads are strict UTF-8. Read/search/quote/copy
branch through the encoding contract; CP437-only presentation uses explicit
`[U+XXXX]` escapes for unrepresentable Unicode. Offline export refuses unrepresentable
content instead of silently replacing it. The bounded FireComm review informed this
encoding/display separation; no FireComm code or dependency was adopted.

Publication, path, routing and import-receipt records have immutable/retained
constraints. External author references cannot become local caller IDs. Tests
exercise empty setup, schema-20 upgrade, late migration failure rollback, native
receipt rollback and cold backup/restore. No FTN address, directory, FileEcho or
BinkP relation is introduced.

## Identity, replay and loops

A local publication receives a random 128-bit identity with the configured stable
system suffix. Identity is retained in SQLite before packet creation. A publication
belongs to one native delivery and network/area. A unique publication/destination
routing decision prevents duplicate queue entries across repeated scans, packet
builds, operator retries and restart. Packet files do not determine eligibility.

Incoming identity is `(network namespace, mapped area, Message-ID)` plus retained
origin evidence. Partner/canonical uncompressed packet/member receipts handle
ordinary and repacked retries. Content evidence detects conflicting reuse of an
identity; it is never the identity itself. An origin conflict or changed content
under an existing imported ID quarantines. This profile requires strong message
IDs and HEADERS.DAT; unsupported ID-less legacy packets remain retained for review.
No body-hash-only duplicate or loop suppression is used.

The admitted partner is prepended to `SenderNetAddr` reverse-path evidence. Local
IDs in a received path suppress a loop; repeated systems, a sender already present
or excessive depth fail closed. Export excludes ingress and any destination already
in the retained path or routing receipts. Legitimate forwarding preserves origin,
message/reply identity, recipient attribution, encoding and source time. Sharing a
native conference never silently bridges two network namespaces. Unknown header
fields remain in immutable input evidence and are not blindly republished.

`In-Reply-To` binds only a unique visible, active native parent in the same namespace,
area and conference. Numeric QWK references alone do not fabricate a native thread.
Known offset-bearing timestamps determine native creation UTC; placement retains
receipt UTC. Classic wall time remains separately retained. Missing timezone
information is not invented when forwarding.

## Queue and exchange lifecycle

Manual build scans eligible native deliveries, atomically freezes decisions and
creates Pending entries, then materializes a bounded immutable artifact. A final
transaction revalidates policy/message versions and marks its entries Ready. Existing
Ready/Retry packets are reused only after revalidation. No packet append or mutable
bundle is used. New messages form later artifacts.

| State | Implemented behavior |
| --- | --- |
| Pending | Durable decision awaiting preparation |
| Ready | Immutable packet prepared and current policy checked |
| Retry | Failed/uncertain handoff; same publication and artifact, durable due time |
| Held | Changed policy/message, unsupported representation or restored uncertain work; explicit revalidation required |
| Accepted | Operator confirms completed custody at the exchange boundary; not proof of final recipient reading |
| Failed | Twelve attempts or seven-day automatic retry budget exhausted |
| Quarantined / Cancelled | Reserved terminal vocabulary; no general release/cancel UI in N2 |

A typed `handoff` records success/failure only for a named artifact belonging to the
configured enabled link. Packing alone never means Accepted. Manual retry uses the
expected queue version, clears the due time and revalidates current policy. Backoff
starts at five minutes plus 0–30 seconds deterministic jitter, doubles to six hours,
and stops after twelve attempts/seven days. No busy retry loop, scheduler, arbitrary
shell command, HTTP client or invented socket protocol exists. A future scheduler
can consume the same bounded typed operations and durable due states.

There is no live network transfer lease in the manual-only profile: the authenticated
operator explicitly owns the completed artifact handoff. Exactly-once delivery is
not promised after a lost acknowledgement; independent receiver identity suppression
remains necessary. Queue state can survive a command receipt failure; a fresh command
inspects/reuses that durable state rather than republishing messages.

## Input security, quotas and observability

Input is completed `SYSTEM/qwk-handoff/<link>/inbound.packet`, in a confined private
operator directory. Requests accept link/artifact/queue IDs, never arbitrary paths
or executable commands. Unknown/disabled/version-stale links fail before reading.
For a hub, CONTROL.DAT must match its configured system ID; for a downstream node,
REP framing must identify our configured local system. These checks supplement
host ownership and authenticated operator context; they are not packet authentication
by themselves. Manual handoff has no credential fields or external endpoints.

Shared ZIP checks reject traversal, devices, duplicates, unsupported types, framing
conflicts, excessive expansion, malformed records and invalid names. A bounded
4 KiB per-member expansion floor permits small compressed advisory files such as
NETFLAGS.DAT; larger members retain the 100:1 ratio limit. Only implemented QWK
members and bounded advisory/index files are accepted by this network profile.
Attachments, voting, MIME, unsupported address families and unsupported controls are
quarantined instead of executed or imported as public text. Original artifacts
remain private evidence. Oversized pre-admission candidates remain in the operator
inbox; they are not silently deleted or falsely registered as accepted custody.

| Bound | Initial implementation |
| --- | --- |
| Links / mappings | 32 links; 64 per link; 512 mappings overall |
| Packet | 16 MiB ZIP; 64 MiB expanded; 1,000 messages; 1,024 members |
| Metadata | 32 path systems; 16 KiB/128 fields per message; 1 KiB field line |
| Outbound reservations | 10,000 entries / 256 MiB board; 1,000 / 64 MiB per link |
| Prepared packet batch | Conservative 8 MiB reservation |
| Retained artifacts | N1 1 GiB / 10,000 artifacts |
| Quarantine | 128 entries / 128 MiB; pressure refuses new admission |
| History | Two million receipts / 512 MiB conservative reservation including publication/index overhead; referenced identity never evicted |
| Status | At most 32 links/512 maps; 20 queue rows per link with stable-ID next cursor and state counts |

Privacy-safe events record configuration, packet build, receipt, import, duplicates,
loops, quarantine, retry/release and acceptance. Operator command admission/outcome
and privileged policy/handoff/retry actions use existing audit/command journals.
No author, subject, body, private recipient, credential or raw packet appears in
events/audit. Status exposes finite reasons and opaque artifact IDs. Full retained
artifacts remain sensitive board data, never ordinary caller-download catalog files.

## Recovery and compatibility limits

Cold backup includes the one SQLite authority and complete immutable artifacts.
Uncommitted manual inbox candidates are excluded. Restore holds Pending/Ready/Retry
work for explicit operator review, retains Accepted/Failed state and all duplicate/
path identities, and resumes no session. Restart retains normal queue states and
due times; N1 artifact custody recovery still owns unfinished journaled writes.
Receipts survive restart/restore and reimport adds no native duplicates. A backup
cannot know about remote acceptance or local imports that happened after its snapshot.

DOVE profile interoperability uses the same headers/routing machinery, short system
IDs and configurable conference numbers. The profile currently refuses special or
restricted conferences 2008, 2010, 2013 and 2030. It does not embed a global catalog,
silently enable voting/attachments, or configure public network membership. Isolated
Synchronet 3.19c imported NG REP and generated QWK replies imported into the same
running NG board with native parent links. This proves controlled compatibility,
not live DOVE-Net membership. Windows live networking acceptance is **DEFERRED —
REAL WINDOWS ENVIRONMENT REQUIRED**.

Private QWK mailbox/transit behavior is part of N2. The NetMail exclusion means
FTN/FidoNet NetMail only. The next separately authorized N3 step is M044's FTN
address/domain/point/AKA and packet/tosser/routing/directory slice; no N3 code begins here.

## Schema 22: private native delivery containers

Schema 21 required non-null conference and message numbers on every delivery and
had only public/local-recipient audiences. It could not represent M044 transit
without inventing a conference allocation. Schema 22 transactionally rebuilds the
same `messages` table with a checked `container_kind`: `conference`,
`local-network-mailbox`, or `network-transit`. Existing rows retain every value,
reference, payload, recipient, number and permission and default to `conference`.
The existing migration procedure temporarily disables foreign keys for the table
rebuild, checks all foreign keys before commit, then restores enforcement even on
failure. A late-conflict test proves rollback and no fabricated private history.

Mailbox/transit deliveries have null conference/number and mandatory private
visibility. Transit requires external origin, no local author and external audience.
Native `message_payloads`, `message_fanouts`, `messages` and
`message_delivery_recipients` remain the only content/delivery authority.
No separate QWK body table or SMB mailbox exists.

| New relation | Authority |
| --- | --- |
| `qwk_private_policy` | Per-link enabled/inbound/outbound/transit policy and CAS version |
| `qwk_mailbox_aliases` | Explicit per-link, case-insensitive alias to active native caller; unique alias and caller |
| `qwk_private_routes` | One configured next link per namespace/destination system |
| `network_private_envelopes` | Immutable native MessageId, origin/destination, recipient, ingress and frozen next link/policy |

`MailPolicy` accepts at most 256 aliases and 64 destinations per link. Aliases are
bounded ASCII network-visible identifiers; numeric account shortcuts, Sysop/SBBS
commands, address delimiters and ambiguous duplicates are rejected. No login,
real name or packet display field implicitly enrolls a mailbox. New policy cannot
silently change a frozen private envelope: changed versions hold unsent work.
Ordinary retries and restore review work against unchanged policy. N2 has no
retarget/republish operation for policy-stale private envelopes.

`send_qwk_mail(MessageActor, NewNetworkMail, now)` authenticates through current
native caller authority and explicit link enrollment. Its native creation,
publication, frozen next hop and Pending queue intent share one transaction.
The caller supplies destination/recipient and content, never author identity.
It discloses only the enrolled network alias. `qwk_mailbox` returns at most 20 IDs
per cursor; `read_qwk_mail` permits only the current active author or enrolled
recipient. A Sysop threshold alone grants no access. Ordinary conference message
lookup, listing, discovery and offline export exclude these containers. Transit
has no caller mailbox audience. These are bounded native service hooks; a stock
terminal compose/read menu and offline-reader mailbox integration are not added.

## Private wire routing and privacy

Conference zero is accepted only with private framing and explicit private link
policy; it confers no local privilege. Direct recipients resolve only through the
admitted link's alias table. An omitted destination means the local system in that
configured context. Explicit `RecipientNetAddr` must parse as a bounded QWK system
path. `recipient@system`, the legacy private `NETMAIL` address-line container and
matching `@VIA` evidence are normalized by the shared metadata extension. The
legacy marker is a QWK envelope convention; it enables no FTN address parser.
Conflicting address/path evidence, unrelated systems and unimplemented controls
quarantine without public fallback.

Transit requires permission at ingress and egress and one configured next hop in
the same namespace. Wire route evidence must agree with that next hop. An ingress,
origin/path member or local system cannot become the return destination. The
native envelope retains full destination and origin while outgoing hop addressing
is derived from configured routing. No broadcast or dynamic route learning occurs.
Direct private delivery and legitimate transit share public networking's durable
publication/path/receipt/queue/attempt authority and archive bounds.

Private duplicate identity is namespace + private audience (destination system and
case-folded recipient) + Message-ID, with origin/content collision checks. A
reserved private audience namespace cannot collide with configured public area
tokens. A repacked duplicate retains the same native delivery. Parent linking
requires a unique private publication and matching native recipient, remote origin
and sender/recipient relationship; ambiguity never creates a thread.

Quarantine summaries, events, audit, status and queue diagnostics contain finite
reasons/opaque IDs, never content. Native messages and retained input artifacts are
sensitive backup data. Restart preserves private queues and receipts. Cold restore
holds all unsent work, including private/transit, without resuming a live session;
recipient enrollment, envelope identity and duplicate receipts remain coherent.

## N5 authority extension

[Network operations and recovery](network-operations.md) specifies protected protocol 1.9
projections, named configuration forms and CAS, queue actions, retained-reference
checks and verified origin/acceptance reconciliation. Schema remains 24. Older
restore descriptions above describe the safe held default; N5 adds verified
recovery without inventing post-snapshot history. Wire/message authority is unchanged.
