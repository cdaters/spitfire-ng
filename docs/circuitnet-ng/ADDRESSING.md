# Node IDs and addresses

Your Node ID identifies your BBS within CircuitNET. The Network Administrator
assigns it after membership approval. Give your country and state, province or
region by name; you do not need to assemble the code yourself. A requested ID is
only a request until the Administrator confirms that it is available.

## Reading an address

New geographic assignments use CCRRNNN: two country letters, two CircuitNET region
characters and three digits. USAZ017 means United States, Arizona, assignment 017.
CAON001 means Canada, Ontario; AUNS012 means Australia, New South Wales;
GBEN021 means United Kingdom, England. These are examples, not assigned members.

- CNETROOT is reserved for the network ROOT in new assignment records.
- CCRR000 is the primary regional HOST assignment.
- CCRR001 through CCRR899 are ordinary END assignments.
- CCRR900 through CCRR999 are additional HOST assignments.

An address is still one to eight letters or digits. It has no punctuation or
second routing hierarchy. The assigned role must agree with the convention;
software routing uses the configured END, HOST or ROOT role and parent, never
these digits. HOST and ROOT may run ordinary caller-facing BBSes, post messages
and offer File Areas as well as serve downstream nodes.

## Country and region choices

Country letters follow ISO 3166-1 alpha-2. CircuitNET's two-character region codes
are network assignments mapped to named ISO 3166-2 subdivisions. They are not
standalone ISO subdivision codes. For example NS maps to AU-NSW, VI to AU-VIC,
QL to AU-QLD; EN, SC, WA and NI map to GB-ENG, GB-SCT, GB-WLS and GB-NIR.

The versioned [region table](config/regions.json) starts with selected US,
Canadian, Australian and UK regions. Missing countries or regions are welcome:
ask the Administrator to add the named subdivision, formal reference and a unique
unused two-character code. Publish the revised table and decision; do not change
an existing code's meaning. No protocol update is needed. This table is not a
worldwide inventory or a statement about national borders.

## Assignment, retirement and moves

The Administrator checks one network-wide assignment record, including reserved
and retired addresses, before confirming any ID. The Secretary keeps approvals,
BBS identity, role, assignment dates and retirement records. Optional public
information is separate: see [the application](NODE-APPLICATION.md).

Your HOST may be outside your assigned region; geography does not select a route.
Changing HOST does not conceptually change your Node ID. A permanent geographic
move should eventually retire the old assignment and issue a new address while
preserving the same BBS's history. Current software has no safe established-tree
or address migration command: once a profile retains catalog or traffic history,
it rejects identity/tree edits. Contact the Administrator before a move; hold
traffic and keep backups. Do not edit the database, recreate the profile or rewrite
old message origins to bypass this restriction.

Current software cannot safely distinguish a different BBS deliberately reusing
an old address from that address's historical owner. Retired IDs therefore remain reserved in this release. Reactivation of
the same BBS requires administrative review of its retained state and certificate
enrollment; it is not automatic. A future explicit migration/reuse operation must
preserve old assignments, traffic and change records, and reenroll each
affected neighbor's certificate. No private key is transferred to a new owner.

## Existing identities and the kit catalog

Existing IDs such as ROOT, ROOT0001, HOST0001 and END00001 remain valid. They are
not reinterpreted geographically. CNETROOT is a forward assignment reservation,
not an instruction to rename an established ROOT. The signed seed catalog in this
kit still names ROOT as publisher. Use the approved topology containing that
publisher when importing it. Activating a differently named publisher requires a
separately reviewed catalog-authority and topology transition; simply substituting
CNETROOT in the END setup command will not make the seed valid.

## When callers need a destination

Ordinary shared-conference posts do not need a Node ID. The conference codename
and subscriptions determine distribution. The human To field names an intended
reader but is neither a route nor a unique worldwide person identifier.

A deliberately directed conference post selects a destination BBS such as USAZ017.
Only that BBS imports the directed traffic; intermediate nodes route it. The caller
still needs access to the conference. This is not private mail or end-to-end
encryption. Replies do not automatically inherit the destination.

A future public directory can let people search by BBS name, Node ID and consented
location or Sysop handle. Until then ask your HOST for the approved node list;
there is no automatic directory lookup or requirement to memorize addresses.

## Assignment administration

The kit includes a read-only Python assignment helper in technical/. It checks
role/range consistency and suggests the first unused address using a supplied
reservation list. It does not enroll a node, issue a certificate or atomically
reserve anything. Only the Administrator's recorded approval creates an assignment.
See [the technical addressing specification](../technical/circuitnet-addressing.md).
