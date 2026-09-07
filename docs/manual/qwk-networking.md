# QWK BBS networking for Sysops

QWK networking exchanges public conference and explicitly addressed private messages between configured BBS
partners. Caller [offline QWK](qwk-offline.md) downloads mail for a caller; a network
partner instead has its own identity, conference map, publication receipts and
outbound queue. Both workflows use native SPITFIRE NG messages and the same QWK
codec. DOVE-Net is an optional interoperability profile of this network service.

N2 includes native private mailboxes and configured transit. Controlled Synchronet
interoperability is documented in [M046](../research/m046-networking-n2-qwk-dove.md).
FidoNet/FTN NetMail, BinkP, directories, FileEcho, doors and a general scheduler
remain unimplemented. No public DOVE-Net membership or live traffic is implied.

## Before configuring a partner

Back up the stopped board before upgrading to schema 22. Start the disposable
board normally with `spitfire run <config>`. Use sfconfig's existing operator
identity/capability editor to enroll the intended host operator explicitly:

- `network-status` reads link/queue status;
- `network-run` builds and ingests packets;
- `network-queue` confirms handoff and retries held/retry work;
- `change-sensitive-configuration` creates or changes partners/maps.

Network operations use the existing protected local operator endpoint. Caller
credentials and node slots do not identify partners. No Networks TUI was added to
sfconfig/sfmonitor; the typed CLI is the minimal surface for this slice. Existing
configuration/status and monitor Activity/Errors remain available.

Choose a stable local QWK system ID and the partner's agreed remote ID. Use two to
eight uppercase letters/digits, starting with a letter. Reserved names are rejected.
All links in one network namespace share the same local ID. Record the IDs with the
peer operator; never derive them from downloaded display text. Network namespace,
system IDs, role and profile are immutable for an existing link.

## Configure explicit mappings

The following fictional values are examples, not public-network enrollment.
Native ConferenceIds 2 and 3 must be active areas deliberately approved for network
publication. Confirm their native identities before submitting the request.

```sh
spitfire network /path/to/board/spitfire.toml configure-example-0001 '{"action":"configure","expected":0,"link":{"id":"example","network":"isolated","local_id":"MYBBS","remote_id":"TESTHUB","name":"Isolated peer","profile":"dove-headers","role":"hub","enabled":true,"inbound":true,"outbound":true,"version":1},"mappings":[{"wire_conference":2001,"area":"general","conference_id":2,"enabled":true,"inbound":true,"outbound":true,"version":1},{"wire_conference":2006,"area":"programming","conference_id":3,"enabled":true,"inbound":true,"outbound":true,"version":1}]}'
spitfire network-status /path/to/board/spitfire.toml
```

`hub` describes the partner: send it a REP and receive its QWK. `node` reverses that
packet direction. `qwk-headers` selects the generic supported metadata profile;
`dove-headers` additionally enforces the implemented DOVE conference restrictions.
The example numbers are selected DOVE conventions, not a built-in global catalog.
Restricted/special 2008/2010/2013/2030 mappings currently fail closed.

A mapping explicitly relates native area, network area token and partner wire
conference. These are different identities. Duplicate mappings fail. Same-network
area tokens must identify the same native area across partners. Disabled mappings
stop that direction. Only public All Callers messages are exported; private local
mail remains private even in a mapped mixed-use conference.

Every mutation has a 16–64-character command ID. Reuse that ID for an uncertain retry
of the same request; do not reuse it for a different action. Later policy updates
supply the current `expected` version and increment both link and mapping versions.
Changes hold unsent queue entries. To disable a partner, submit its complete typed
configuration with `enabled:false`, the current expected version and the next version.

## Build, exchange and acknowledge

Post eligible native messages, then build manually:

```sh
spitfire network /path/to/board/spitfire.toml build-example-000001 '{"action":"build","link":"example","expected":1}'
spitfire network-status /path/to/board/spitfire.toml
spitfire network-queue /path/to/board/spitfire.toml example -
```

Build returns an opaque artifact ID. Its immutable bytes are under the configured
SYSTEM directory at `network-artifacts/<artifact-id>`. A hub expects a completed
file named from its system ID, such as `TESTHUB.REP`; a downstream node receives
`MYBBS.QWK`. Copy only the selected immutable artifact through your controlled
exchange procedure. Keep packets private and use an atomic completed-file handoff.
The CLI does not execute a shell, choose arbitrary paths, connect to public networks,
or claim that mere packing means delivery.

After the controlled receiving endpoint has accepted custody, acknowledge the exact
artifact returned by build:

```sh
spitfire network /path/to/board/spitfire.toml handoff-example-0001 '{"action":"handoff","link":"example","expected":1,"artifact":"<artifact-id>","accepted":true}'
```

The placeholder must be replaced by the real ID. `accepted:false` records a failed
or uncertain handoff and schedules durable Retry state. Accepted means that exchange
boundary accepted custody; it does not mean every remote reader saw the message.
Repeating build/retry cannot create a second publication for that destination.

Status includes queue counts, first-page rows and a next cursor. Pass that cursor
instead of `-` to `network-queue` to inspect subsequent pages. Queue states are
Pending, Ready, Held, Retry, Accepted and Failed. Preserve the packet and diagnose
Held/Failed reasons rather than deleting SQLite rows or artifact files.

## Receive a completed packet

Configuration creates private `SYSTEM/qwk-handoff/example/`. Place the completed
peer packet there as `inbound.packet`, using a temporary sibling and atomic rename
while no ingest runs. Only the explicitly selected configured link supplies partner
context. Then invoke:

```sh
spitfire network /path/to/board/spitfire.toml ingest-example-00001 '{"action":"ingest","link":"example","expected":1}'
```

The result counts imported, duplicate, loop-suppressed and quarantined messages.
Reingesting the same packet does not duplicate native messages. Network authors
remain external attribution; packet names cannot impersonate local callers. Native
conference access still controls who can read imported public messages. Known
reply IDs retain native parent links; ambiguous numeric references do not fabricate
threads.

Do not identify a partner from its display name alone. The operator's trusted
handoff context and configured system check must agree. Unknown partners fail
before reading. Wrong system IDs, invalid maps, malformed packets, unsupported
private mail and routing anomalies are rejected/quarantined. Durable original
artifacts are retained; an oversized candidate rejected before custody remains
in the operator inbox. No unexplained inbound mail is silently deleted.

## Retry, restore and isolated DOVE testing

A failed handoff retains the same artifact and publication identity. Backoff is
bounded to twelve attempts/seven days; no automatic worker or scheduler runs.
An explicit retry uses a queue ID and its current version:

```sh
spitfire network /path/to/board/spitfire.toml retry-example-000001 '{"action":"retry","queue":"<queue-id>","expected":3}'
```

Current partner, mapping and message policy are rechecked before release. A disabled
partner must first be enabled by an authorized versioned configuration update.
A materially changed destination/map is not silently rerouted by retry. Preserve
held evidence and correct the policy deliberately.

Cold backup includes links/maps, native messages, queues, attempts, receipts,
provenance and complete immutable artifacts. Inbox candidates are temporary and
excluded. Restore holds unsent work for review and resumes no live session. Verify
what the remote endpoint accepted after the snapshot before releasing restored
work; a snapshot cannot remember future delivery. Retained receipts still suppress
replay of messages included in that snapshot.

Controlled DOVE-compatible acceptance used a network-isolated Synchronet 3.19c peer,
two configured areas, real REP import and real QWK generation. No public DOVE-Net
traffic or membership was involved. Keep initial testing local/isolated, disable
unsupported peer voting/MIME/attachments, enable its HEADERS.DAT profile and agree
on identity/mappings. Public participation requires separate explicit authorization.
Windows live networking acceptance remains deferred to a real Windows environment.

See the [Technical Reference](../technical/qwk-networking.md) for schema, exact
limits, profile restrictions, identity/loop rules and future scheduler/N3 boundaries.

## Private mailbox and transit policy

Private QWK mail differs from local private messages in ordinary conferences.
It has a native mailbox or transit container with no conference number. Merely
mapping a public area never exports local private mail. Enroll explicit network
aliases and destinations after configuring the link:

```sh
spitfire network /path/to/board/spitfire.toml mail-policy-000001 '{"action":"configure-mail","expected":0,"policy":{"link":"example","enabled":true,"inbound":true,"outbound":true,"transit":false,"version":1,"aliases":[{"alias":"Example Caller","caller_id":2}],"destinations":["PEER"]}}'
```

The caller ID must already identify the intended active native caller. The example
is synthetic: select your caller deliberately. Alias matching is case-insensitive;
no numeric recipient shortcut, login-name lookup or automatic Sysop recipient is
allowed. Use the expected/current private policy version independently of the link
version. Changes are audited without aliases or message contents.

A destination system has exactly one configured next partner per network. For
transit, enroll the destination on the next link and enable transit on both links.
Unknown recipients/routes quarantine; they never enter public areas. Disabled
links/policy stop exchange. Do not route a system through itself or its origin.
Private delivery to a partner uses the same build, ingest, handoff and retry commands
as conference traffic. The packet can contain both kinds, with distinct envelopes.
No ordinary caller can view transit or another caller's mailbox, including through
conference search. Operator status does not display bodies or subjects.

The N2 native caller interface consists of typed service hooks: `send_qwk_mail`,
`qwk_mailbox`, and `read_qwk_mail`. The authenticated actor comes from native caller
authority; operator IPC cannot impersonate that actor or submit a private body.
A stock terminal mailbox menu is not included. The deterministic developer journey
in `crates/sf-bbs/tests/qwk_network.rs` demonstrates caller-authorized native send,
operator build/exchange, private receive, unauthorized-read denial, transit and
backup/restore using a disposable daemon. See the [Technical Reference](../technical/qwk-networking.md)
for the exact interfaces and frozen-policy limitations.

Back up schema 22 before changing policy. Restore preserves mailbox aliases, native
private content, pending queue identities, provenance and receipts, and holds unsent
work for explicit review. A retry with unchanged policy reuses its original identity.
Policy-stale envelopes remain held; N2 supplies no general retargeting command.

## N5 operator and recovery extension

[Operating networking](network-operations.md) now provides the Networks cockpit, typed
configuration, safe queue actions, directory/quarantine visibility and verified
restore recovery. Same-root replacement retains proven later FTN serial floors
and matching peer acknowledgements. New-root recovery uses a stopped surviving
source, retires its origination, and keeps uncertain work held. No live public
FidoNet participation or N6/N7 functionality is claimed.

## Posting names

A network mapping or link can require **Real name required** while the local
conference ordinarily allows handles. The caller sees the resulting name before
submission. Missing First Name or Last Name prevents a required-name post. Handle
allowed remains the default; FTN and QWK do not acquire a universal real-name rule.
Queued senders remain frozen through profile edits. A stricter requirement holds
unsuitable old work; it never substitutes a current profile name. Configuration
and queue summaries do not show private account names.

See [posting identity](../technical/identity-policy.md) and
[caller management](../operator/caller-management.md).
