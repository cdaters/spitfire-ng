# Set up an END node

<!-- public-identity:start -->
Official network home: https://circuitnetng.org/

Applications Closed. Keep your form locally until applications open.

When opened, the application location is https://circuitnetng.org/apply
and the joining contact is join@circuitnetng.org.
Do not send passwords, private keys or authentication secrets.

These are canonical locations; web, mail and download deployment has not
been verified. See [JOINING-INFO](JOINING-INFO.md) for opening status.
<!-- public-identity:end -->

Start here after membership approval. Have your assigned Node ID, parent HOST,
approved topology, HOST certificate and connection details, plus the independently
confirmed catalog authority fingerprint. Read [OPERATIONS](OPERATIONS.md) for
command conventions and permissions. Examples use MYBBS and MYHOST; replace them.

## Confirm the information before you start

Use the checklist in [JOINING](JOINING.md). MYBBS and MYHOST are substitutions,
not instructions to self-assign a name. For the regional example they may be
USAZ017 and USAZ000. Keep the approved ROOT identifier exactly as supplied: the
kit seed currently expects ROOT, not CNETROOT. [ADDRESSING](ADDRESSING.md) explains
that reservation and the present limits on moving established nodes.

Both directions need a Dossier. In addition to requesting your HOST's approval,
configure your own stopped-board Dossier for MYHOST with SUPPORT and CHITCHAT
using `subscribe`, as shown in the complete [DOSSIERS](DOSSIERS.md) example.
Use [SYSOP-ACCESS](SYSOP-ACCESS.md) for actual visiting-Sysop grants and denial checks.

## 1. Configure your board and identity

Stop the board. Configure the approved tree. This small example shows the shape;
use the complete approved tree supplied for your network:

```
sfconfig circuitnet spitfire.toml init circuitnet-ng MYBBS \
  "CircuitNET NG" --live ROOT:ROOT:- MYHOST:HOST:ROOT \
  MYBBS:END:MYHOST
```

Generate a unique node certificate and private key on your own system using the
procedure in [NODE-IDENTITY](NODE-IDENTITY.md), or follow the approved enrollment
procedure from your HOST. Exchange only the public certificate. Confirm the HOST
certificate fingerprint through your agreed contact before enrolling it.

```
sfconfig circuitnet spitfire.toml identity circuitnet-ng \
  identity.der identity-private.der
sfconfig circuitnet spitfire.toml listener circuitnet-ng off
sfconfig circuitnet spitfire.toml peer circuitnet-ng MYHOST \
  HOST_ADDRESS HOST_PORT HOST_CERT_NAME host-public.der yes yes
```

HOST_CERT_NAME must match the HOST certificate's server name. These are enrollment
values, not names to guess. Listener off is appropriate for outbound-polling ENDs.
The HOST must also enroll your public certificate and allow inbound calls from
your assigned identity before Test Link can succeed.

## 2. Enroll the catalog

Unzip the kit into a directory named `circuitnet-kit` inside your board directory.
Keep running these commands from the board directory, where spitfire.toml lives.
Check [VERIFICATION](VERIFICATION.md) first. Confirm the authority independently,
then enroll it and import the retained seed revisions in order:

```
sfconfig circuitnet spitfire.toml catalog-pin circuitnet-ng \
  circuitnet-kit/config/catalog-authority.json
sfconfig circuitnet spitfire.toml catalog-import circuitnet-ng \
  circuitnet-kit/config/catalog-history/000001.json
sfconfig circuitnet spitfire.toml catalog-import circuitnet-ng \
  circuitnet-kit/config/catalog.json
```

This build's current revision is 2. A later kit may include more predecessor files;
import them in numeric order before its current catalog. Existing boards keep their
pin and history; use the documented replacement process for an announced key change.

## 3. Test the link and synchronize

```
spitfire run spitfire.toml
```

In another terminal:

```
sfconfig circuitnet spitfire.toml test-link circuitnet-ng MYHOST
sfconfig circuitnet spitfire.toml poll circuitnet-ng MYHOST
sfconfig circuitnet spitfire.toml live-status circuitnet-ng
```

Test Link verifies the authenticated neighbor. Poll also synchronizes newer catalog
revisions and exchanges eligible work. If the HOST is several revisions ahead,
repeat Poll until status agrees. Failure is not a reason to trust a new certificate
or catalog key without verification.

## 4. Choose local conferences

Stop the board gracefully. Inspect the catalog:

```
sfconfig circuitnet spitfire.toml catalog-list circuitnet-ng
```

Choose unused local conference numbers. These examples use 17 and 18 only if they
are free on your board:

```
sfconfig circuitnet spitfire.toml catalog-create-map circuitnet-ng \
  SUPPORT 17
sfconfig circuitnet spitfire.toml catalog-create-map circuitnet-ng \
  CHITCHAT 18
```

SUPPORT is required and restricted to Sysops and verified visiting Sysops.
CHITCHAT is required and public to authorized callers. New restricted mappings
set local read/post security to 9999; the board's Sysop rule still grants its
operators access. Do not lower those levels to give ordinary callers access.
Grant a verified visiting Sysop an explicit privileged conference security level
through local conference administration; catalog membership grants no account rights.
Use a dedicated level assigned only to verified visitors, not a level shared with
ordinary callers. Local access grants apply to every account at the granted level.

Use `catalog-map circuitnet-ng CODENAME NUMBER` after the same command prefix to
map an existing conference. Restricted areas require read/post security 9999.
Use `catalog-ignore circuitnet-ng CODENAME` for an optional area you do not want.
Neither creation nor mapping automatically subscribes you.

## Configure your HOST's local Dossier

Before restarting, allow the reverse direction on your stopped board:

```
sfconfig circuitnet spitfire.toml subscribe circuitnet-ng MYHOST SUPPORT
sfconfig circuitnet spitfire.toml subscribe circuitnet-ng MYHOST CHITCHAT
```

These entries describe what you send to your HOST. The next requests ask what
it sends to you. [DOSSIERS](DOSSIERS.md) follows both boards through approval.

## 5. Request subscriptions

Restart the board. Request each desired active area at your HOST:

```
sfconfig circuitnet spitfire.toml remote-subscribe circuitnet-ng \
  SUPPORT
sfconfig circuitnet spitfire.toml remote-subscribe circuitnet-ng \
  CHITCHAT
sfconfig circuitnet spitfire.toml poll circuitnet-ng MYHOST
sfconfig circuitnet spitfire.toml query-subscriptions circuitnet-ng
```

The default HOST policy requires operator approval. Poll again after approval and
check the returned result. Your HOST also arranges its subscription to traffic
from your board; confirm both directions together. Consult [HOST-NODE](HOST-NODE.md)
for local Dossier management. A pending request does not enable distribution.

## 6. Set an exchange schedule

Save this as `exchange.json`, replacing MYHOST with your assigned parent:

```
{
  "id": "circuitnet-upstream",
  "name": "CircuitNET upstream exchange",
  "enabled": true,
  "action": {
    "kind": "circuitnet",
    "network": "circuitnet-ng",
    "node": "MYHOST"
  },
  "schedule": {"kind": "interval", "seconds": 900},
  "timezone": "UTC",
  "policy": "hybrid",
  "missed": "run-once",
  "minimum_spacing_seconds": 30
}
```

```
sfconfig events spitfire.toml save exchange.json
sfconfig events spitfire.toml list
sfconfig events spitfire.toml run circuitnet-upstream
```

Hybrid attempts prompt work and scheduled catch-up. Scheduled waits for the Event;
Manual requires Poll/Run Now; Immediate reacts to eligible queued work. These
choices affect exchange timing, not whether a saved message is queued. Use one
Event for this target, not a second timer for files or catalogs.

## 7. Verify a message in both directions

Arrange a brief operator test in SUPPORT with your HOST. Post from the mapped
local SUPPORT conference as an authorized Sysop, Poll and ask the HOST to confirm
one received post. Have the HOST reply, Poll again and confirm the reply locally.
Check the expected posting identity and that ordinary callers cannot browse SUPPORT.
Use CHITCHAT for relevant public introductions after the test.

If nothing arrives, follow [OPERATIONS](OPERATIONS.md): catalog/mapping, Dossier,
approval, hold, Event, Test Link, Poll and receipt. Do not create an unofficial
conference to work around a rejected codename.

## What the schedule means

The 900-second interval above means every 15 minutes SPITFIRE checks this link for
queued exchange work. Hybrid permits an earlier exchange when new traffic arrives;
the recurring Event provides catch-up and retry. Scheduled queues work immediately
but waits for the Event before connecting. Manual keeps it queued until Poll or
Run Now. Immediate attempts eligible work promptly subject to link holds and retry
spacing. A held link remains held regardless of its schedule.

An outbound-polling END receives waiting HOST traffic during the same connection;
it does not need an inbound listener just to receive messages or files. Event
history and receipts distinguish a failed connection from work waiting for approval.
SPITFIRE NG Events here means the scheduler. The EVENTS conference is a separate
current-affairs discussion area; subscribing to it does not configure a schedule.

## Check catalog health without separate tools

On a stopped board, `sfconfig circuitnet spitfire.toml catalog-status circuitnet-ng`
shows authority, revision, hash, pending_sync, rejected, last_error and last_success.
On a running board use `live-status` and sfmonitor Networks. Import and live sync
verify the accepted authority, signature and consecutive revision/hash chain before
persisting the catalog. A reported accepted revision is the result of those checks.
Compare the expected revision and authority with your enrollment contact. A rejected
update leaves the last accepted catalog intact; investigate its error rather than
resetting history. [VERIFICATION](VERIFICATION.md) separates this normal flow from
advanced offline artifact checks.
