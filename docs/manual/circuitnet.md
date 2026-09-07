# CircuitNET NG public conferences

CircuitNET NG is SPITFIRE NG's native conference-networking service. It preserves
CircuitNET's Node IDs, END/HOST/ROOT tree, conference codenames and Dossiers.
SPITFIRE messages remain ordinary native messages. There is no FTN or QWK
translation and no separate CircuitNET message base.

C3 adds native encrypted, authenticated live exchange. Use explicitly configured
neighbors; no public port or automatic discovery is assigned. C2 offline exchange
remains available when explicitly enabled. Private mail, file networking, legacy
packets and remote network administration remain outside this implementation.

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
through that neighbor. Topology moves remain future work;
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

## Offline operator commands

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

## Live setup: END, HOST and ROOT

Create four disposable boards with their own native public conferences. Use the
same profile name and full tree on each board. The local Node ID differs:

```sh
sfconfig circuitnet BOARD init circuitnet-test END00001 "CircuitNET Test" --live ROOT0001:ROOT:- HOST0001:HOST:ROOT0001 END00001:END:HOST0001 END00002:END:HOST0001
```

For the HOST use `HOST0001` as the local ID; for the ROOT use `ROOT0001`;
for the other END use `END00002`. `--live` does not enable offline file trust.
Use `--trusted-offline` instead when the same isolated profile should support both
transports. Map the appropriate local conference numbers to `CNTEST` and `CNTECH`
using the earlier `map` command. END1 carries both, END2 only CNTEST. Configure
HOST/ROOT mappings to both codenames. Run `subscribe` for both ends of each direct
relationship: ROOT/HOST, HOST/END1, HOST/END2. Only grant END2 CNTEST.

### Certificates and credentials

Each board/profile owns a unique TLS certificate and private key. A neighbor's
public certificate is its enrollment credential. Transfer and verify that public
certificate through an operator-controlled channel before enrollment. Never copy
a node's private key to a neighbor. There is no additional shared password.

For isolated acceptance, OpenSSL can generate a self-signed certificate without
a public CA. Save this as `certificate.conf`, replacing the Node ID and DNS name
for each board:

```ini
[req]
prompt=no
distinguished_name=dn
x509_extensions=ext
[dn]
CN=END00001
[ext]
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=serverAuth,clientAuth
subjectAltName=DNS:end00001.circuitnet.invalid
```

```sh
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 30 -outform DER -config certificate.conf -keyout identity.key -out identity.der
openssl pkcs8 -topk8 -nocrypt -outform DER -in identity.key -out identity-private.der
sfconfig circuitnet BOARD identity circuitnet-test identity.der identity-private.der
```

Keep source key files private; the host imports the matching key into its existing
restricted SYSTEM custody boundary. Status exposes only credential presence.
The certificate must be valid for both TLS server/client use and its configured
server name. Expiry, invalid signatures, missing credentials and wrong server
names fail closed. To rotate, stop the affected daemons, install the new identity,
and update the certificate enrolled at each direct neighbor before restarting.
Old public receipts and native messages do not change. Production automation for
certificate issuance/renewal is deferred.

### Listener and neighbor enrollment

Choose distinct non-privileged localhost ports, for example ROOT 36401, HOST
36402, END1 36403, END2 36404. These are examples, not assigned protocol ports.
While each board is stopped:

```sh
sfconfig circuitnet HOST_BOARD listener circuitnet-test 127.0.0.1:36402
sfconfig circuitnet END1_BOARD peer circuitnet-test HOST0001 127.0.0.1 36402 host0001.circuitnet.invalid host-identity.der yes yes
sfconfig circuitnet HOST_BOARD peer circuitnet-test END00001 127.0.0.1 36403 end00001.circuitnet.invalid end1-identity.der yes yes
```

The last two values grant inbound and outbound connections, respectively. Add the
ROOT/HOST and HOST/END2 links in the same way. Certificate assignment must be unique
among a profile's neighbors. Only topology-adjacent nodes can be enrolled. Use
`peer-enabled PROFILE NODE no` to disable a peer while stopped. Re-running `peer`
updates its endpoint, certificate and direction flags. `listener PROFILE off`
disables incoming connections. An END with its listener off can still initiate
polls to retrieve queued HOST traffic; give the HOST inbound permission for it.
Connection initiation never changes END/HOST/ROOT role.

Start the ordinary `spitfire run BOARD` daemon on each board. A CircuitNET-only
board does not require a caller listener. Listener/identity/endpoint changes are
cold-board operations and take effect on the next start. Each profile has its own
listener and local identity, so incoming profile selection is unambiguous.

### Test, exchange, hold and retry

These commands require the running daemon and protected operator connection:

```sh
sfconfig circuitnet END1_BOARD test-link circuitnet-test HOST0001
sfconfig circuitnet END1_BOARD poll circuitnet-test HOST0001
sfconfig circuitnet END1_BOARD live-status circuitnet-test
sfconfig circuitnet HOST_BOARD hold circuitnet-test END00002
sfconfig circuitnet HOST_BOARD release circuitnet-test END00002
```

Test Link checks TCP, TLS, protocol, Node ID, profile and role without preparing or
transferring messages. Poll starts an asynchronous bounded exchange: at most one
32-message batch in each direction. Inspect `live-status` for completion. Repeat
polls to drain larger backlogs. The worker retries transient failures up to three
times; there is no unattended polling scheduler. Sender preparation scans eligible
native posts, while HOST import immediately creates eligible onward queue intent.

Hold applies to both connection directions and leaves durable work intact. It also
blocks later batch admission/preparation on an already authenticated session;
a receipt for a batch already committed can still complete delivery. Release
permits catch-up. Dossiers are rechecked for new work. Online operator-managed
changes use `live-subscribe` or `live-unsubscribe` followed by profile, neighbor,
codename and expected Dossier revision. These are local operator commands, never
remote Dossier administration.

After restore, review held queue rows with the cold `queue` command, then use
`retry PROFILE QUEUE_ID REVISION` while stopped, or
`live-retry PROFILE QUEUE_ID REVISION` while running. Dossier/identity policy must
still permit the delivery. Receipt loss is safe to retry: the receiver recognizes
the original identity/artifact and returns durable acceptance without importing
or forwarding it twice. Twelve failed delivery attempts require operator review;
there is no silent reset of that admission ceiling.

The operator needs `network-status` to inspect live state, `network-test` for
Test Link, `network-run` for Poll, `network-queue` for Hold/Release/live Retry,
and `change-sensitive-configuration` for Dossiers. Enrollment retains the cold
configuration grants documented above. Commands report admission/start separately
from eventual link success.

### Networks view and troubleshooting

Open Networks in sfmonitor and select CircuitNET Networks. Each profile shows its
local ID/role, listener and credential status. Neighbor rows show endpoint, held
and active state, queued work, last attempt, last success and safe result. Select
a neighbor and use `t` Test Link, `p` Poll, `h` Hold, `r` Release; the normal
operator confirmation/capability controls apply.

- `connect`: check the configured address/port and whether the daemon is running.
- `tls`: verify the enrolled certificate, matching private key, certificate expiry,
  server name, TLS 1.3 and mutual authentication on both ends.
- `unknown-node`, `wrong-network`, `topology-mismatch`: compare the exact profile,
  local IDs, roles and full tree. Do not relax admission to make a test pass.
- `unsupported-version`: the peer needs a common CircuitNET NG major/minor range
  and required capabilities.
- `held` or `unauthorized-codename`: inspect neighbor hold, permissions and Dossiers.
- `conflicting-message`: preserve queue/artifact evidence for review. Do not edit
  accepted identities or discard receipts.
- `timeout` or `interrupted`: retry after the link is available; ambiguous delivery
  is resolved through the receiver's durable receipt.

A rejected batch is atomic. A valid member remains pending if another member
conflicts or is unauthorized. Existing duplicate history remains intact. Native
backup/restore preserves config, credentials, Dossiers, queue/artifact custody and
receipts; it never restores an active socket. See the
[wire specification](../technical/circuitnet-transport.md) for exact protocol rules.
