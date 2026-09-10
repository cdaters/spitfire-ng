# CircuitNET catalog authority

Catalog storage introduced through schema 35 (current board schema 37) and
CIRCUITNET-NG 1.4 implement signed network conference metadata,
local mapping choices and explicit signing-authority recovery. Native SPITFIRE
messages remain canonical; no local conference number is sent on the wire.

## Signed revisions and trust

A profile opts into catalog enforcement by explicitly pinning a catalog ID,
publisher Node ID and Ed25519 public key. ROOT is the initial publisher; technical
key custody is separate from the human approval reference recorded in every change.
No key received in a catalog or TLS session becomes trusted automatically.

A revision is a complete bounded snapshot with format/schema, network, catalog ID,
revision, previous revision/hash, publication time, publisher, governance reference,
change rationale and entries. Each entry has an immutable 128-bit hexadecimal ID,
codename, name, description, category, required flag, lifecycle, effective revision,
optional retirement revision, historical reference and moderator role. Snapshots
retain all established identities, including retired generations. Entries sort by
immutable ID. JSON field order and compact UTF-8 serialization are specified below and in the
[implementation contract](circuitnet-ng-specification.md); no floating point, arbitrary object keys or local IDs occur.
The SHA-256 catalog hash covers this unsigned body. Ed25519 signs the domain
`CIRCUITNET-NG-CATALOG-1\n` followed by the exact canonical body bytes. The wrapper
contains body, hash and hexadecimal signature. Standard ring Ed25519 is used.

Accept only a verified pinned publisher/key/network/catalog. The next revision
must be exactly current+1 with the current hash as predecessor. Exact current
replay is idempotent; older revisions, alternate same-number revisions, missing
predecessors and signatures from another authority fail closed. Bootstrap starts
at revision 1 (previous revision 0 / no previous hash), including offline signed
artifact import. A package carries a signed seed and public trust information;
private signing keys never enter the kit. Explicit key replacement is specified below.

## Lifecycle and local decisions

Proposed -> Active -> Deprecated -> Retired is the ordinary lifecycle. Deprecated
areas retain delivery but produce operator attention. Active is required for new
Dossier subscriptions. Retired areas stop new publication and delivery, preserving
messages, mappings, receipts and subscriptions as historical records. Reactivation
is an explicit change to the same immutable ID. Reuse requires an explicit publish
intent, rationale and governance reference, a retired predecessor and a new ID.
No implicit rename or deletion of an established ID is accepted.

Catalog-enabled messages carry the immutable conference ID. Old generations cannot
be imported into a new generation's local mapping. Existing uncataloged profiles
retain uncataloged operation; adopting authority never retroactively reclassifies old
traffic. Catalog-aware publication is withheld from peers that cannot preserve its
identity. Ordinary uncataloged message/control/file exchange remains compatible.

Local choices are available/unmapped, mapped, ignored or needs-attention/retired.
Create+map takes an explicit operator-selected local number. Map-existing uses an
existing eligible public conference. Ignore creates no conference or subscription.
Reusing a codename never silently rebinds a prior generation's mapping. Required
areas report compliance attention instead of overriding local policy.

## Exchange, recovery and operations

Optional `catalog-sync` is negotiated at minor 1.4. A bounded catalog phase precedes
messages/controls/files in normal Poll/Exchange. Direct parent/child sessions compare
revision/hash, request consecutive missing snapshots and forward original signed
objects unchanged. No new socket, scheduler or learned topology is introduced.
Generic Events and existing holds/session admission remain authoritative.

Database revisions, trust pins, choices and sync health are durable and included in
backup. Restore into an existing board must reject a snapshot older than its current
accepted catalog or with conflicting authority/hash; it cannot discard the board's
known high-water revision. An empty replacement board requires retained current
backup/trust material and live synchronization before export; no disconnected node
can infer a revision it has never observed. Restore never resurrects local deletion.

Operator tools expose pin/import/export/status/publish, lifecycle intent, local
create/map/ignore and generated change reports. Publication is capability-gated.
Safe audit metadata records outcome/identity/revision; no message bodies, private
caller details or keys. Human governance has no voting/election workflow engine.

## Bounds and verification

At most 256 retained conference identities per snapshot, 512 KiB encoded object,
64 revisions per exchange and bounded text fields. Reaching history capacity fails
visibly; identities are never erased to make room. Distribution generation derives
human conference/change lists from signed data and Markdown documents from canonical
sources, with deterministic archive timestamps and SHA-256 manifest. Tests cover
signing/chain/lifecycle/generation safety, mapping, restore, six-node transport,
older peers, existing networking and reproducible rights-safe kit output.

## Schema 1 and canonical bytes

The [JSON schema](../circuitnet-ng/config/catalog.schema.json) describes structure;
[revision 1](../circuitnet-ng/config/catalog-history/000001.json) is a signed testable example with
its public [authority pin](../circuitnet-ng/config/catalog-authority.json). Trust the
pin only after independent administrative confirmation. Structural JSON validation
does not replace signature, byte bounds, chain or lifecycle checks.

Unsigned body field order is exactly: format, schema, network, catalog_id, revision,
previous_revision, previous_hash, published_at, publisher, governance_reference,
rationale, intent, entries. Entry field order: id, codename, display_name, description,
category, required, status, effective_revision, retired_revision, historical_reference,
moderator_role. Serialize JSON without whitespace; integers in ordinary decimal,
booleans as true/false, absent predecessor/retirement as null. Strings are UTF-8,
with quotation mark and reverse solidus escaped as `\"` and `\\`; ordinary non-ASCII
characters stay literal UTF-8. Control characters and bidi override/isolate characters
are not admitted in metadata. No floating-point values or arbitrary metadata maps.
Entries strictly increase by lowercase hexadecimal immutable ID. Network/Node/codename
tokens normalize according to existing protocol rules before canonical serialization.
The canonical body has no trailing newline. Wrapper order is body, hash, signature.
SHA-256 hash and Ed25519 signature are lowercase hexadecimal, 64 and 128 digits.

Verification uses standard [ring Ed25519](https://docs.rs/ring/0.17.14/ring/signature/static.ED25519.html).
The SHA-256 catalog hash is the revision-chain/content identifier; Ed25519 performs
its standard signing algorithm over the domain-prefixed body, not a custom signature
construction. No TLS certificate or public key carried by the object replaces the pin.

Body schema=1 is independent of wire minor=4 and Network Kit version=1.0. Fields have
UTF-8 byte limits: display name 80, description/rationale 1,024, category 60,
governance/historical reference 160, moderator role 80. Publication timestamps are
UTC Unix seconds through year 9999; revision fits signed 64-bit storage. The database
bounds retained revisions at 4,096 and fails visibly on capacity rather than pruning.

## Local enforcement and historical generations

The current snapshot selects an Active/Deprecated generation by codename. New
subscriptions require Active; ongoing traffic permits Deprecated. Retired generations
remain in every subsequent snapshot. The per-generation local choice records the
native conference reference and a native-message high-water mark, so adopting/remapping
an area cannot discover and export older local archive posts. Metadata-only catalog
updates preserve this choice. Retirement disables the local routing projection; a
reactivation needs explicit local remapping before new send/receive resumes.

Message fingerprint and stored CircuitNET provenance include conference_identity.
Thread reconciliation requires matching generation as well as network/codename;
old traffic is never attached to a new generation merely because its codename matches.
Existing accepted-message replay remains idempotent after retirement. A new incoming
publication must satisfy current locally accepted policy. Changes cannot retroactively
recall a message durably accepted before that node learned the new revision.

Per-neighbor catalog acknowledgments are retained to identify pending downstream work.
A committed revision advances the generic Event activity generation. Missing acknowledgments
cause state comparison on the next permitted exchange; no separate timer or repeated
mutation is needed. An older peer's unsupported capability remains visible through
negotiated protocol health; unsupported work does not become an endless successful
empty-session drain.

## Schema 2 access classification and human administration

Schema 2 adds optional entry field `access` BEFORE `id` in canonical field order.
Allowed values are `public` and `sysops`; default Public is OMITTED in canonical
serialization. A schema-1 entry cannot specify Sysops. This preserves every accepted
schema-1 signature byte. Schema downgrade in a later revision is rejected. The
current [revision 2](../circuitnet-ng/config/catalog.json) retains the 46 identities
and changes SUPPORT/SYSOP/SPITFIRE/DOORS to Sysops. SUPPORT remains Required/Core;
the other three are optional. CHITCHAT remains Public/Core.

Normal catalog-add/edit/reuse commands take codename, display name, description,
access, core status and decision reference/rationale. New identities are 16 random
bytes from the operating system CSPRNG, encoded as lowercase hexadecimal. Draft
validation rejects collisions; no operator-supplied opaque ID is necessary. Ordinary
catalog lists and generated conference/change documents omit IDs. Advanced
catalog-details, signed artifacts and audit/history retain them. Reactivation selects
the existing generation; explicit reuse generates another and preserves the old one.

Sysop mapping requires both native read/post levels 9999. The existing board Sysop
threshold and explicitly configured privileged conference levels continue to grant
local access. Verified visiting Sysops may receive such a local grant without global
Sysop privileges. No Node ID, certificate, subscription or catalog record creates an
account grant. Weak map-existing is rejected; if a local operator later weakens access,
the mapping reports Needs Attention and new network delivery/export stops. A catalog
update does not remotely edit local security or erase already-stored messages: the
Sysop must review existing local content/access when adopting a changed classification.

`catalog-access` negotiation gates schema-2 snapshots and restricted messages. An
older schema-1 peer receives only still-active public generations present in the
last schema-1 snapshot. New public generations also wait: their unknown identity
must not make an otherwise compatible batch fail. The same filter applies when
selecting an existing retry artifact, so unsupported frozen work cannot starve
compatible queued work. See
the transport specification. Existing offline exchange remains an explicitly trusted
operator boundary: both boards must enforce their independently enrolled catalogs and
native mappings; an offline artifact does not carry a key-enrollment permission.

## Signing-authority transition format

A local `catalog-replace-key` operation requires sensitive-configuration capability,
a typed transition artifact, independently confirmed new fingerprint and literal
`confirm-key-replacement`. No network frame changes a pin. Same-publisher replacement
retains the network, catalog ID, publisher Node ID and exact accepted revision/hash.
ROOT topology or publisher identity migration is outside this operation.

Transition JSON fields, in order: `change`, `old_signature`, `new_signature`.
Change fields, in canonical order: `format`, `previous`, `replacement`, `revision`,
`catalog_hash`, `mode`, `published_at`, `reference`, `rationale`.
Format is `circuitnet-ng-catalog-key`. Previous/replacement are authority objects with
ordered fields `network`, `catalog_id`, `publisher`, `public_key`. Mode is `planned`
or `emergency`. Revision/hash identify the exact current catalog cutoff; time is UTC
Unix seconds, reference has at most 160 UTF-8 bytes and rationale at most 1,024.
Public keys are 32-byte lowercase hex and signatures 64-byte lowercase hex. The new
fingerprint is SHA-256 of the raw new public key, not its hexadecimal text.

Canonical JSON uses the same compact UTF-8/string/integer rules as the catalog.
Standard Ed25519 signs `CIRCUITNET-NG-CATALOG-KEY-1\n` followed by the canonical
Change bytes. Planned mode requires signatures from both old and new keys. Emergency
mode requires old_signature=null and new-key proof, plus independent LOCAL operator
approval; it makes no claim of old-key consent. Changed mode, checkpoint, identity or
rationale invalidates the signature. Unknown fields and oversized input reject.

The database stores monotonically numbered immutable key epochs with
first_revision=cutoff+1, the signed
transition, accepted time and authority. The migration seeds epoch 1 from existing
pins. New revisions verify with the applicable epoch; old revisions retain original
signatures. Up to 64 epochs are retained; a previously used key cannot be enrolled
again. Multiple locally approved transitions may share a cutoff before another
catalog revision is published; the latest epoch then signs that next revision.
This permits recovery if a newly enrolled key is lost before first use, without
requiring a publication signed by the lost key. Cutoffs cannot decrease.
Exact transition replay is an idempotent no-op. Wrong current pin/checkpoint,
new fingerprint or signatures reject and produce a safe failure audit. Success audits
mode, cutoff revision and PUBLIC fingerprint; no secret key, account details or body.

Backup validation verifies the entire epoch chain against retained revision hashes.
Known newer authority cannot be replaced by restoring an older pin. A new replacement
board imports original public authority/history through each cutoff, explicitly enrolls
the public transition, then continues history. No disconnected empty board can discover
unknown later compromise or revisions. Conflicting accepted history requires incident
reconciliation, not a forced rollback flag. Normal routing continues without the private
catalog key while publication is paused.

Operational custody, protected signing, recovery-copy risk, loss, compromise and
node-by-node enrollment are in [KEY-CUSTODY](../circuitnet-ng/KEY-CUSTODY.md).
The custody policy adapts standard key-inventory, compromise and recovery principles
from [NIST SP 800-57 Part 1 Rev. 5](https://csrc.nist.gov/pubs/sp/800/57/pt1/r5/final):
private signing-key recovery copies are an explicit risk/availability choice, not an
indefinite archive; public verification material is retained, and recovered signing
keys are replaced promptly. No custom signature algorithm or automatic trust discovery
is introduced.

## Public discovery and publication endpoints

[Official public identity](../circuitnet-ng/PUBLIC-IDENTITY.md) is generated from
[release metadata](../circuitnet-ng/config/release.json). It specifies canonical
human/JSON/signature/public-key locations and their exact encoding. These are
publication/discovery endpoints; this release does not deploy or verify them.
The signed object and independently accepted authority remain the trust boundary.
HTTPS or a newly downloaded public key cannot replace an existing authority pin.

The future human authority page must derive revision, previous revision, body hash,
publication timestamp, publisher and fingerprint from the served signed object and
configured public key. Protocol version comes from the current published protocol
contract. Validate their consistency before atomically publishing the matching
JSON/signature/key set; never sign a different wrapper interpretation. Retain prior
catalogs and key-transition history needed by recovering nodes. No new HTTP adapter
or live catalog fetch is added to the daemon.


## Local visiting-Sysop grants

`sfconfig circuitnet BOARD catalog-access-levels NETWORK CODENAME LEVELS`
replaces up to five distinct nonzero local privileged conference levels; `-` clears
all. The stopped-board operation requires Read Configuration and Change Sensitive
Configuration. It uses one transaction to validate a mapped active Sysop area with
read/post 9999, replace native conference grants and append a safe local audit.
It does not alter the signed catalog, caller accounts, subscriptions or messages.
Every caller at a granted effective level gains access; use a dedicated reviewed
level and the [operator procedure](../circuitnet-ng/SYSOP-ACCESS.md).
