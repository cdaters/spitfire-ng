# Operate ROOT

<!-- public-identity:start -->
Founding Network Administrator role contact: founder@circuitnetng.org.

Official network home: https://circuitnetng.org/

These are canonical locations; web, mail and download deployment has not
been verified. See [JOINING-INFO](JOINING-INFO.md) for opening status.
<!-- public-identity:end -->

ROOT is the top routing node for the configured tree. It has no upstream target.
The Network Administrator is a human governance office; ROOT is a technical role.
The same person may perform both during founding administration, but owning ROOT
or a signing key does not confer permanent decision-making authority.

## Establish service

Obtain the approved tree and publisher designation. Use the profile and identity
commands in [END-NODE](END-NODE.md) with the assigned local ROOT and no parent.
Use [HOST-NODE](HOST-NODE.md) to configure the listener and enroll each direct HOST.
Confirm each certificate independently. Test links before allowing unattended
exchange. Configure one appropriate Event per reachable HOST or agree that HOSTs
will initiate their own polls. ROOT needs no upstream polling Event.

Maintain mappings and Dossiers for the conferences ROOT carries. Preserve the
Sysop-only classification of SUPPORT, SYSOP, SPITFIRE and DOORS. Do not expose
operator conference traffic to ordinary callers just because ROOT can route it.
Use Route Test to explain paths and inspect Networks/Events for pending work.

## Publish catalog changes

Follow [CATALOG-ADMIN](CATALOG-ADMIN.md) for human fields, draft generation,
publication and change reports. Obtain and record the governance decision first,
or a documented founding decision while bootstrap is active. A technical signature
does not count votes or establish that a proposal was approved.

The catalog signing key need not remain on ROOT. Prepare the draft on the board,
transfer it to a protected signing system, sign it there, and import the signed
result at ROOT. Normal routing needs only the public authority and accepted catalog.
See [KEY-CUSTODY](KEY-CUSTODY.md) for exact commands and recovery procedures.

After publication, Poll or Events carry the unchanged signed revision to HOSTs
and then ENDs. Compare their reported revisions. Retain predecessor artifacts
for offline recovery and nodes that missed several changes. Receiving a catalog
never assigns local conference numbers or subscribes all nodes automatically.

## Back up and recover

Keep board metadata, payload stores, catalog revisions, trust transitions and
public decision records backed up. Test a restore on an isolated replacement
before an emergency. Signing-key custody and its optional protected recovery
copy are separate from ordinary ROOT backups and from downloadable kits.

If signing is unavailable, ordinary messages, files, controls and routing continue
under the last accepted catalog. Suspend new catalog publication until recovery
is verified. For loss or compromise, use the deliberate node-by-node trust
transition in [KEY-CUSTODY](KEY-CUSTODY.md). Never edit the database pin or reset
revision history to make a new key work.

Review retained-history limits and topology migration constraints in
[OPERATIONS](OPERATIONS.md). A network-wide change requires coordination, not
an unannounced local tree edit.
