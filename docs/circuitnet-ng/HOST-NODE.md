# Operate a HOST

<!-- public-identity:start -->
Founding Network Administrator role contact: founder@circuitnetng.org.

Official network home: https://circuitnetng.org/

These are canonical locations; web, mail and download deployment has not
been verified. See [JOINING-INFO](JOINING-INFO.md) for opening status.
<!-- public-identity:end -->

A HOST serves its direct ENDs and routes their traffic toward the parent ROOT or
HOST. Use [END-NODE](END-NODE.md) for the common identity, catalog, mapping and
Event steps. Use [OPERATIONS](OPERATIONS.md) for permissions and command conventions.

## Upstream and children

Obtain an approved HOST Node ID and complete tree from the Administrator. Stop
the board before profile/enrollment changes. The init command has the same shape
as the END guide, but your local node has HOST role and an approved parent.
Enroll the parent's public certificate and your own node identity. Enroll each
direct child's certificate against its assigned Node ID; a claimed ID alone is
not authentication. Never request a child's private key.

Configure a reachable listener and an enrolled peer for every direct neighbor:

```
sfconfig circuitnet spitfire.toml listener circuitnet-ng BIND:PORT
sfconfig circuitnet spitfire.toml peer circuitnet-ng CHILD \
  CHILD_ADDRESS CHILD_PORT CHILD_CERT_NAME child-public.der yes no
```

Replace BIND:PORT and the child fields with agreed values. The final `yes no`
allows the child to call in but disables outbound calls to that child, useful
for an END behind NAT. Its outbound endpoint fields remain syntactically valid
planning values, not a discovered route. For a reachable child that accepts your
calls, use its verified endpoint and `yes yes`. Configure the parent similarly
with outbound enabled. Start the board and Test Link with reachable neighbors;
a child behind NAT should initiate its own Test Link and Poll.

## Dossiers and approval

Map only catalog areas you intend to carry, with the required local access policy.
SUPPORT is Sysop-only and core. SYSOP, SPITFIRE and DOORS are Sysop-only and optional.
CHITCHAT is public and core. A catalog entry alone grants no caller access and
creates no subscription. Verified visiting Sysops need deliberate local access.

Keep the default approval policy unless you deliberately choose another:

```
sfconfig circuitnet spitfire.toml control-policy circuitnet-ng \
  require-approval
sfconfig circuitnet spitfire.toml live-status circuitnet-ng
sfconfig circuitnet spitfire.toml approve circuitnet-ng REQUEST_ID
sfconfig circuitnet spitfire.toml deny circuitnet-ng REQUEST_ID
```

Use the actual pending request identity shown by status/sfmonitor. Only a direct
child may remotely request changes to its own Dossier. Check the requested area
and membership before approving. Results and retries are durable. `auto-approve`
accepts authorized requests automatically; `deny` disables remote changes.

For local Dossier management on a stopped board:

```
sfconfig circuitnet spitfire.toml subscribe circuitnet-ng CHILD \
  SUPPORT
sfconfig circuitnet spitfire.toml unsubscribe circuitnet-ng CHILD \
  OPTIONAL_CODE
```

A Dossier describes what that neighbor receives from you. Arrange both directions:
request your own upstream subscriptions at the parent and ensure children have
configured the subscriptions needed for their outbound fanout. Do not subscribe
every child to every catalog area.

## Routing and exchange

Use Route Test for a destination before investigating a cross-branch failure.
Sibling traffic can stay inside your branch. Traffic outside it goes toward the
parent; the configured tree decides the path. There is no arbitrary peer mesh.
Directed conference traffic keeps its conference access rules.

Use the END guide's Event definition for your upstream. Add a distinct target
Event only for each child you are authorized and able to call. Outbound-polling
children receive your queued work when they call you. Catalog updates retain the
publisher signature as they pass to children; HOST does not re-sign them.

File distribution uses separate file-area mappings and subscriptions, with local
inspection/scan policy at each receiving board. See [FILES](FILES.md). It uses the
same exchange Events, not another timer.

## Daily checks

Inspect Networks and Events in sfmonitor: failed peers, holds, pending requests,
queue age, catalog revision and next exchange. Use `why`, Test Link and Poll as
shown in [OPERATIONS](OPERATIONS.md). Compare catalog revision with the parent;
resolve unsupported capabilities before enabling operator-only traffic on a link.
Back up metadata and payload stores, and coordinate certificate renewal before
expiry. A missed exchange does not justify recreating messages or receipts.
