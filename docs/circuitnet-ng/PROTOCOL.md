# CircuitNET NG protocol participation

Network Kit 1.0 targets the implementation-neutral CIRCUITNET-NG 1.x family.
Catalog synchronization and immutable conference generation require minor 1.4 and
`catalog-sync`. Messages, controls and files retain the existing authenticated
session and explicit topology. The kit itself does not configure endpoints or ports.

Optional capabilities are negotiated. Older implementations can retain existing
uncataloged messaging and control/file features according to their supported minors;
they do not interpret catalog frames or receive messages whose generation they cannot
preserve. Offline catalog artifacts are independently signed and require an enrolled
public authority key, even if their transport is untrusted. No local database ID or
conference number appears in catalog or network conference identity.

Canonical specifications are the project's CircuitNET transport and catalog documents.
The kit includes copies under technical/ for offline readers. These documents define
framing, bounded objects, canonical signature bytes, chain checks, lifecycle and local
mapping. ROOT publishes signed changes only after the appropriate human decision.
There is no legacy CNP/CND translation, FTN/QWK/BinkP transport bridge, private-mail
extension, election engine or CircuitNET file-request operation in this milestone.
