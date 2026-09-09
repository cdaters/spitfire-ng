# Catalog administration

The catalog defines network conference names, purpose, access and lifecycle.
Local conference numbers belong to each BBS. Catalog publication records a human
decision; it is not an electronic voting system.

## Local choices

Use the stopped-board commands in [END-NODE](END-NODE.md) to pin/import the catalog,
list available areas and create, map or ignore them. Normal commands select a
codename. Local creation chooses a number you supply and generates no subscription.
No network packet assigns a local number or deletes local messages.

The four operator areas admit Sysops and verified visiting/network Sysops only.
SUPPORT is required; SYSOP, SPITFIRE and DOORS are optional. Set restricted native
read/post levels to 9999 and deliberately grant verified visitors an appropriate
privileged conference level. Do not lower normal levels to expose these areas.
Use a dedicated level assigned only to verified visitors, not a level shared with
ordinary callers. Local access grants apply to every account at the granted level.
A weak existing mapping is rejected or stops being network-eligible. CHITCHAT
remains public/core. Report an inability to carry a core area to the Administrator.

## Create or edit a conference

The examples use a stopped, authorized ROOT board. Replace APPROVAL and the
rationale with the actual governance record. The administrator supplies the name,
codename, description, access and Required/Optional choice. The immutable identity
is generated automatically; it is not a field the operator must invent.

```
sfconfig circuitnet spitfire.toml catalog-add circuitnet-ng \
  BASKETS "Underwater Basket Weaving" "Basket weaving discussion" \
  public optional draft.json APPROVAL "Approved new discussion"
```

This writes a new draft without overwriting a file. Review it before signing.
Use `catalog-edit` with the same arguments to change an existing conference's
name, description, access or core status while preserving its identity. `sysops`
selects restricted access; `required` selects core status. A normal add rejects
an already-used codename, including a retired one.

## Deprecate, retire, reactivate or reuse

```
sfconfig circuitnet spitfire.toml catalog-deprecate circuitnet-ng \
  BASKETS draft.json APPROVAL "Transition notice"
sfconfig circuitnet spitfire.toml catalog-retire circuitnet-ng \
  BASKETS draft.json APPROVAL "Approved retirement"
sfconfig circuitnet spitfire.toml catalog-reactivate circuitnet-ng \
  BASKETS draft.json APPROVAL "Same conference resumes"
```

Run only the intended command, using a fresh output filename. Deprecation warns
of transition and prevents new subscriptions. Retirement stops new distribution
without deleting local messages or conferences. Reactivation resumes the same
conference with its historical identity; nodes explicitly remap if needed.

`catalog-reuse` has the same fields as `catalog-add`, but deliberately creates a
new identity for a different conference using a retired codename. Obtain explicit
reuse approval and explain the new concept. The old record and traffic remain
attached to the old identity. Publishing reuse requires the extra confirmation
`confirm-new-identity-reuse`. It never silently rebinds old mappings or messages.

## Sign and publish

Use the protected/offline procedure in [KEY-CUSTODY](KEY-CUSTODY.md). Where the
signing key is deliberately available on a protected ROOT console, the equivalent
command is:

```
sfconfig circuitnet spitfire.toml catalog-publish circuitnet-ng \
  draft.json signing.key publish
```

For deliberate reuse, replace the final `publish` with
`confirm-new-identity-reuse`. Then export the signed result and generated report:

```
sfconfig circuitnet spitfire.toml catalog-export circuitnet-ng \
  catalog-new.json
sfconfig circuitnet spitfire.toml catalog-changes circuitnet-ng \
  changes.md
```

Poll/Events propagate catalog changes through the existing links. Keep signed
predecessors. An older peer lacking the operator-access capability receives only
compatible catalog history; restricted traffic waits instead of losing its access
classification. Public messaging and supported file/control exchange continue.

## Diagnostics and history

`catalog-status` shows revision, publisher and local attention. `catalog-list`
shows codenames and human policy. `catalog-details` is the advanced diagnostic
view, including immutable identities. The signed machine catalog, audit records
and technical specification retain these identities for recovery and history.

Reject an unexpected key, broken chain, older revision or conflicting same-number
revision. Fetch missing predecessors in order. See [OPERATIONS](OPERATIONS.md)
for retained-history limits and [KEY-CUSTODY](KEY-CUSTODY.md) for recovery.


Local visiting-Sysop access is configured with the stopped-board
`catalog-access-levels` command; see [SYSOP-ACCESS](SYSOP-ACCESS.md) for dedicated
levels, account assignment, verification and revocation. Do not lower normal
security requirements to simulate a privileged grant.
