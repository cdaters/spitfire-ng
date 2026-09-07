# CircuitNET NG: offline public conferences

CircuitNET NG is SPITFIRE NG's native conference-networking service. It preserves
CircuitNET's Node IDs, END/HOST/ROOT tree, conference codenames and Dossiers.
SPITFIRE messages remain ordinary native messages. There is no FTN or QWK
translation and no separate CircuitNET message base.

**Development Preview — C2 offline exchange only.** There is no CircuitNET
Internet listener, TCP/TLS transport, legacy packet reader/writer, private mail,
file networking or remote administration. Use isolated disposable boards. Do not
import files obtained from untrusted sources: this profile relies on an operator
who has established custody of each configured neighbor's files. Names inside a
file do not authenticate its producer.

## Identity and the tree

Each profile has a network slug, such as `circuitnet-test`, and a display name.
Each participating board has a unique Node ID within that profile. Node IDs use
one to eight letters/digits and normalize to uppercase. They are independent of
hostnames, native node slots, telephone area codes and filenames.

- **END** has one configured parent and no dependents; it does not forward traffic.
- **HOST** serves branches and can have one upstream parent. A parentless HOST
  supports the small isolated C2 fixture.
- **ROOT** sits at the top and has no parent. Its role does not grant governance
  office or automate network administration.

C2 configures the whole small tree explicitly on each participating board. Cycles,
missing parents, duplicate IDs, multiple top nodes, ROOT parents and END children
are rejected. The bounded tree also checks whether an asserted origin can arrive
through that neighbor. Enrollment and live topology moves remain future work;
a node's identity/tree cannot be changed after it has retained message history.

## Conferences and Dossiers

A codename identifies a network conference independently of its local number.
HOST conference 17 and END conference 4 can both be `CNTEST`. Codenames use one
to eight uppercase letters/digits, with internal hyphens permitted. Local native
conferences must already exist and be public-only. Mapping creates no conferences.
No historical conference catalog is installed.

A **Dossier** is the set of codenames an adjacent node is subscribed to receive.
It is separate from the board's own local mapping. Configure both ends of each
link explicitly, including the parent's Dossier. C2 also uses the subscription as
an admission permission for new incoming traffic in that codename. HOSTs can
relay subscribed conferences they do not carry locally, using native restricted
transit storage. Callers cannot read that transit container.

Removing a subscription stops new traffic and holds unsent work for that neighbor.
Previously accepted messages and receipts remain. A file already offered to a
neighbor may still be acknowledged after removal. Re-subscribing permits future
traffic; it does not automatically replay history or release held work.

## Operator commands

Stop the board first. The commands acquire the normal cold-board operation lock;
they fail if a daemon or another cold operation owns it. The local operator needs
`read-configuration` for inspection, `change-sensitive-configuration` for policy,
and `network-run` for exchange/retry. All commands also require
`read-configuration` to resolve the current board policy. These are existing explicit operator grants;
new board defaults do not silently grant networking administration. See
[operator controls](sfconfig.md) for the established authority model.

The same commands work as `sfconfig circuitnet` and `spitfire circuitnet`.
`BOARD` below means the chosen disposable board's configuration file. A minimal
HOST and two END tree is:

```sh
sfconfig circuitnet BOARD init circuitnet-test HOST0001 "C2 Test" --trusted-offline HOST0001:HOST:- END00001:END:HOST0001 END00002:END:HOST0001
sfconfig circuitnet BOARD map circuitnet-test 17 CNTEST
sfconfig circuitnet BOARD map circuitnet-test 18 CNTECH
sfconfig circuitnet BOARD subscribe circuitnet-test END00001 CNTEST
sfconfig circuitnet BOARD subscribe circuitnet-test END00001 CNTECH
sfconfig circuitnet BOARD subscribe circuitnet-test END00002 CNTEST
sfconfig circuitnet BOARD status circuitnet-test
```

On each END, use the identical tree but its own local Node ID. Map its own native
conference numbers. Subscribe its HOST Dossier to the codenames it will exchange.
For example, END00001 maps local 4/5 to CNTEST/CNTECH and subscribes HOST0001 to
both; END00002 maps local 9 to CNTEST and subscribes HOST0001 only to CNTEST.
All values are typed and validated; normal setup does not require SQL or raw TOML.
The initial C2 surface is this bounded command interface, not a live TUI cockpit.

Post through the normal native caller interface after configuring the mapping.
C2 permits handles. A stronger existing board/conference identity rule still
applies and is shown before saving. No private first/last name is inserted in a
message. Changing a profile or handle later never renames an old posted author.
Adding a network mapping does not grant permission to export older posts.

## First exchange

Use explicit new output filenames chosen by the operator. Output never overwrites
an existing file. Each exchange can contain multiple messages.

```sh
spitfire circuitnet END1_BOARD scan circuitnet-test
spitfire circuitnet END1_BOARD export circuitnet-test HOST0001 end1-host.json
spitfire circuitnet HOST_BOARD import circuitnet-test END00001 end1-host.json host-receipt.json
spitfire circuitnet END1_BOARD ack circuitnet-test HOST0001 host-receipt.json
spitfire circuitnet HOST_BOARD export circuitnet-test END00002 host-end2.json
spitfire circuitnet END2_BOARD import circuitnet-test HOST0001 host-end2.json end2-receipt.json
spitfire circuitnet HOST_BOARD ack circuitnet-test END00002 end2-receipt.json
```

Import stores a native message once and queues eligible onward deliveries in one
transaction. HOST does not reflect END1's message back to END1. Origin remains
END1 when END2 receives it. Reply relationships follow CircuitNET identities;
a missing parent can link later without fabricating a message.

Export means a prepared file, not successful delivery. Only the neighbor's durable
import receipt completes those delivery items. If receipt writing or transfer fails,
import the identical file again using a new receipt output filename, then acknowledge
that receipt. Replay does not create another native message or fanout. Changed
content under an existing identity rejects the whole artifact.

```sh
sfconfig circuitnet BOARD queue circuitnet-test
sfconfig circuitnet BOARD unsubscribe circuitnet-test END00002 CNTEST
sfconfig circuitnet BOARD enable circuitnet-test no
```

Queue inspection returns up to 100 entries; pass the last queue ID after the network
slug to get the next page. Status is safe metadata and counts; it does not show
message bodies or private profile names. C2 adds no sfmonitor cockpit.

## Recovery

A failed output leaves that neighbor pending/recoverable; other acknowledged
neighbors stay complete. Preparing again retries its outstanding offer before
adding newer traffic. Transfers have a finite twelve-failure budget. Exhausted
work requires a later explicit recovery design; retries do not reset the budget.

Native cold backup includes CircuitNET identity, topology, mappings, Dossiers,
provenance, immutable sender snapshots, queues, artifacts and receipts. Restore
keeps acknowledged work complete and holds uncertain work. Review `queue`, then
explicitly release an eligible held item using its current ID and version:

```sh
sfconfig circuitnet BOARD retry circuitnet-test QUEUE_ID VERSION
```

Current policy and identity must still authorize it. An old snapshot cannot know
about acknowledgements obtained after the snapshot; receiver replay suppression
and a fresh receipt reconcile those deliveries. Keep the original node stopped
when recovering a replacement with the same identity. See
[backup and restore](../operator/backup-restore.md).

The [technical contract](../technical/circuitnet.md) defines exact bounds and
ownership. C2 stops at this offline slice; live transport, enrollment, governance,
revived catalogs and third-party adapters require subsequent review.
