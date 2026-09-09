# Access to operator conferences

SUPPORT, SYSOP, SPITFIRE and DOORS are for Sysops and verified visiting Sysops.
SUPPORT is required; the others are optional. CHITCHAT is required and available
to ordinary callers who meet your local access rules. Required does not mean
visible to everyone. Catalog presence never grants caller access.

## Set up a restricted local conference

Stop the board gracefully. As an enrolled local operator with Read Configuration
and Change Sensitive Configuration permissions, use catalog-create-map from the
END guide. It creates the restricted area with read/post levels 9999. A mapping
to an existing conference requires those levels already set and public-only
messages. Use `spitfire config spitfire.toml`, Message Conferences (5), Edit (E),
to review its read/post levels and public-only setting; conference edits save
immediately. Keep read/post at 9999 and public-only enabled.

Your own Sysop caller account is separate from the operating-system operator.
Its effective security must meet the board's configured Sysop threshold. Review
that threshold in `spitfire status spitfire.toml`; do not raise ordinary callers
to Sysop merely to grant access to one conference.

## Verify and grant a visiting Sysop

The local Sysop verifies that the person operates an approved participating BBS,
using the Administrator or that BBS's independently known operator contact. A
self-asserted handle or a network message is not sufficient verification. Record
who checked it and when in your private local administration notes.

Choose a dedicated caller security level below your Sysop threshold, unused by
ordinary callers and suitable under your other local area/file/time rules. The
example uses 40; substitute your reviewed level. Every account at that effective
level receives the grant, so check the existing account list before proceeding.
On the stopped board, explicitly replace the permitted visitor levels:

```
sfconfig circuitnet spitfire.toml catalog-access-levels circuitnet-ng \
  SUPPORT 40
```

Repeat for SYSOP, SPITFIRE or DOORS only if you carry them and intend that access.
A comma-separated list allows up to five distinct nonzero levels. The command is
local, capability-gated and audited; it never changes network catalog access,
subscriptions or normal read/post thresholds. It rejects a weak, unmapped,
non-Sysop or inactive catalog area.

To assign the caller's reviewed level, start `spitfire console spitfire.toml`
instead of a separate running daemon. It owns the board and listeners. At its
local operator prompt use CALLERS to review accounts, then:

```
SECURITY 40 Verified Visitor
```

Replace Verified Visitor with the actual local caller name. This existing console
command changes that account's board-wide security, so review its other privileges
before using it. The grant is local to this BBS; it is not network identity
federation. Never exchange account passwords to verify a visiting Sysop.

## Verify denial and revoke

Log in with the visitor account and confirm the allowed conference appears. Then
use an ordinary caller account below the Sysop threshold, at a level absent from
the grants: the operator area must be absent from lists, reads and Hot Conferences.
Check direct conference selection too. New catalog revisions must not expose it.

If verification expires or is revoked, restore that caller's former reviewed level
with SECURITY and confirm effective access after a fresh login. To revoke all
visitor-level grants for one area, stop the board and run:

```
sfconfig circuitnet spitfire.toml catalog-access-levels circuitnet-ng \
  SUPPORT -
```

To remove only one level, supply the complete remaining list instead. Local Sysop
accounts still use the Sysop threshold. Revocation does not delete messages or
change anyone else's password, and a copied historical message cannot be recalled.
