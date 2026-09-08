# CircuitNET catalog authority — C7 interface contract

The interface was defined before implementation. Schema 34 and CIRCUITNET-NG 1.4
implement this contract. Verification is recorded in
[M070](../research/m070-circuitnet-governance-catalog-distribution.md).

The catalog defines network conference identity and policy, never local conference
numbers. C1 remains historical authority; C5's historical reconciliation is retained
unchanged. C7 reviews the separate modern proposal. Native SPITFIRE messages remain
the message authority. This module stores signed network metadata and local choices.

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
immutable ID. JSON field order and compact UTF-8 serialization are specified by the
wire implementation; no floating point, arbitrary object keys or local IDs occur.
The SHA-256 catalog hash covers this unsigned body. Ed25519 signs the domain
`CIRCUITNET-NG-CATALOG-1\n` followed by the exact canonical body bytes. The wrapper
contains body, hash and hexadecimal signature. Standard ring Ed25519 is used.

Accept only a verified pinned publisher/key/network/catalog. The next revision
must be exactly current+1 with the current hash as predecessor. Exact current
replay is idempotent; older revisions, alternate same-number revisions, missing
predecessors and signatures from another authority fail closed. Bootstrap starts
at revision 1 (previous revision 0 / no previous hash), including offline signed
artifact import. A package carries a signed seed and public trust information;
private signing keys never enter the kit. Key rotation is outside C7.

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
retain C2-C6 operation; adopting authority never retroactively reclassifies old
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
C5 Events and existing holds/session admission remain authoritative.

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
older peers, C2-C6 and reproducible rights-safe kit output.

## Schema 1 and canonical bytes

The [JSON schema](../circuitnet-ng/config/catalog.schema.json) describes structure;
[revision 1](../circuitnet-ng/config/catalog.json) is a signed testable example with
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
A committed revision advances the generic C5 activity generation. Missing acknowledgments
cause state comparison on the next permitted exchange; no separate timer or repeated
mutation is needed. An older peer's unsupported capability remains visible through
negotiated protocol health; unsupported work does not become an endless successful
empty-session drain.
