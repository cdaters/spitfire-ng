# A subscription between two boards

Your BBS is USAZ017; your HOST is USAZ000. You want CHITCHAT, RETRO and SUPPORT.
These are current catalog codenames. SUPPORT is restricted to Sysops and verified
visiting Sysops. CHITCHAT and SUPPORT are required on every participating board;
RETRO is optional. Arrange this example with your HOST before making changes.

Three separate choices are involved:

1. The catalog says an area is available and describes its rules.
2. Your local mapping connects that codename to one of your conferences.
3. Your HOST's Dossier for your BBS says which areas it sends to you.

Mapping alone does not create a subscription. A Dossier is directional: the
Dossier for USAZ017 on USAZ000 controls what USAZ000 sends to USAZ017.

## On USAZ017

Use [END-NODE](END-NODE.md) to enroll, sync the catalog, and map SUPPORT and
CHITCHAT. Stop the board and use an unused local number for RETRO, for example 19:

```
sfconfig circuitnet spitfire.toml catalog-create-map circuitnet-ng RETRO 19
sfconfig circuitnet spitfire.toml subscribe circuitnet-ng USAZ000 SUPPORT
sfconfig circuitnet spitfire.toml subscribe circuitnet-ng USAZ000 CHITCHAT
sfconfig circuitnet spitfire.toml subscribe circuitnet-ng USAZ000 RETRO
```

The three local subscriptions above let your board send those areas to its HOST
and authorize normal incoming area traffic from that neighbor. They do not change
the HOST's Dossier for you. Restart your board, then request the reverse direction:

```
sfconfig circuitnet spitfire.toml remote-subscribe circuitnet-ng SUPPORT
sfconfig circuitnet spitfire.toml remote-subscribe circuitnet-ng CHITCHAT
sfconfig circuitnet spitfire.toml remote-subscribe circuitnet-ng RETRO
sfconfig circuitnet spitfire.toml poll circuitnet-ng USAZ000
```

## On USAZ000

The authenticated requests appear in live status and sfmonitor Networks. With
the default approval policy, the HOST operator checks membership and area policy,
then approves each actual request identifier shown by status:

```
sfconfig circuitnet spitfire.toml live-status circuitnet-ng
sfconfig circuitnet spitfire.toml approve circuitnet-ng REQUEST_ID
```

REQUEST_ID is the value from that request, not the word printed here. Approval
rechecks eligibility. A denied or pending request is not an active subscription.
The resulting HOST-side Dossier for USAZ017 contains CHITCHAT, RETRO and SUPPORT.
The HOST need not create caller conferences merely to relay subscribed traffic.

## Confirm both directions

On USAZ017, Poll again to receive results. Query the HOST's current view:

```
sfconfig circuitnet spitfire.toml query-subscriptions circuitnet-ng
sfconfig circuitnet spitfire.toml poll circuitnet-ng USAZ000
sfconfig circuitnet spitfire.toml live-status circuitnet-ng
```

Coordinate one short SUPPORT test with your HOST using authorized Sysop accounts.
Verify receipt in both directions and confirm that an ordinary caller cannot see
SUPPORT. Use CHITCHAT for a normal introduction after joining. A successful Test
Link proves connectivity, not message delivery; inspect receipts and the received
message. See [OPERATIONS](OPERATIONS.md) if either direction is missing.
