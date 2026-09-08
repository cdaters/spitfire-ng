# Catalog administration

A catalog defines network identity, scope and lifecycle. A local conference number
belongs to your BBS. A catalog signing key proves technical publication authority;
it does not record an electronic vote or grant unlimited governance authority.

## Enroll and inspect

On a stopped board, `sfconfig circuitnet CONFIG catalog-pin NETWORK AUTHORITY.json`
requires sensitive-configuration capability. Independently confirm the publisher,
network, catalog ID and public key. Pins are durable and cannot be replaced by a
packet or ordinary configuration edit. Key rotation is deferred to an explicit
future enrollment procedure. Keep the retained initial signing key outside the kit
and public repository, restrict access and back it up securely.

Import the signed seed with `catalog-import NETWORK catalog.json`. A node with no
prior revision accepts revision 1; later artifacts require each consecutive signed
predecessor. `catalog-status NETWORK` shows revision, publisher, mapping attention
and rejected updates; `catalog-list NETWORK` lists entries and local decisions.
Normal Poll/Exchange synchronizes signed objects from your direct parent, independent
of which side opens TCP. Existing Events and holds apply. `live-status` and sfmonitor
show catalog health alongside links. A peer without `catalog-sync` can continue older
message/control/file operation; catalog-bound messages wait for an identity-capable
peer rather than losing their conference-generation identity.

## Local choice

After `sfconfig circuitnet CONFIG`, these commands operate on NETWORK:

- `catalog-create-map NETWORK ID NUMBER`: choose a free local number, create a public
  native conference and map it. Initial read/post security is 10; review local access
  before caller use. A failed mapping leaves a recoverable local conference rather
  than pretending the network operation succeeded.
- `catalog-map NETWORK ID NUMBER`: map an existing enabled public conference.
- `catalog-ignore NETWORK ID`: leave it unsubscribed/unmapped and disable any old
  routing projection. No local messages are deleted.

ID is the immutable conference ID displayed in the catalog; NUMBER is chosen locally.
Creating/mapping does not subscribe. Request active-area Dossier subscriptions through
your direct HOST and resolve its approval result. Core-area omissions show compliance
attention; they never override local access policy. Catalog adoption does not relabel
previously uncataloged messages or grant them retroactive export.

Deprecated areas warn of transition and retain existing delivery, but do not admit
new subscriptions. Retired areas stop network send/receive. Keep their local conference
as an archive or local-only area; historical messages and identity remain. Reactivation
uses the same ID and needs explicit local remapping to resume sending. A deliberately
reused codename has a new ID: choose mapping/subscriptions anew. Old messages do not
become the new conference. No destructive Delete is offered for published identities.

## Publish an approved change

Only the pinned publisher operating ROOT can publish. Obtain the human approval or
explicit bootstrap decision first. The signed governance reference and rationale must
identify it; software does not claim to verify an actual committee vote.

1. `catalog-draft NETWORK draft.json APPROVAL-REFERENCE RATIONALE` writes a new,
   non-overwriting typed draft based on current revision. Edit its entries as JSON
   according to the schema; local numbers are forbidden. New entries need new 32-digit
   lowercase hexadecimal IDs. Keep entries sorted by ID.
2. Add a Proposed/Active definition, update metadata, deprecate or retire an existing
   definition. Changed entries set effective_revision to the new revision. Retirement
   sets retired_revision; retain the entry forever in later snapshots. Removing an
   unpublished local draft file is harmless; removing an established identity is not.
3. Reactivation sets intent `reactivate`, retains the same ID, changes status to active
   and clears retired_revision in this snapshot; earlier signed history remains.
4. Deliberate reuse sets intent `reuse`, retains the retired old entry, adds a new ID,
   and supplies a clear rationale/approval reference. It never silently renames an ID.
5. `catalog-publish NETWORK draft.json PRIVATE-KEY publish` validates/signs/commits.
   Reuse instead requires the explicit final argument `confirm-new-identity-reuse`.
   Read the warning: this creates a **new conference identity**, not a reactivation.
6. `catalog-export NETWORK catalog.json` exports the signed object;
   `catalog-changes NETWORK changes.md` generates its actual change report. Neither
   command overwrites an existing file. Poll children or let configured Events run.

`catalog-key NETWORK PRIVATE-KEY` generates a new restricted PKCS#8 signing key and
prints only its public key, for initial authority enrollment. It does not rotate an
existing pin. Never put the private file in a File Area, application or kit.

## Troubleshoot and recover

Unknown/retired areas, missing mapping, disabled send/receive, no generation-specific
Dossier, ignored area, held/offline peers and pending scheduled exchanges each prevent
movement. Inspect catalog status/list, then existing CircuitNET queue/link and Event
views. Do not fix an invalid signature by trusting the received key. Recheck the
independently enrolled pin. Fetch missing revisions consecutively; reject rollback
and alternate same-number/hash chains. No broadcast or unsigned fallback exists.

Backups retain catalog pins/history/choices. Restoring over a stopped board that knows
a newer revision is rejected before replacement. Keep a current backup. An empty
replacement cannot know a revision it never observed: import retained verified history
and sync with the parent before enabling export. Do not remove the known board merely
to bypass a restore rejection. Catalog signing keys have separate protected custody;
confirm their backup independently from ordinary documentation-kit contents.
