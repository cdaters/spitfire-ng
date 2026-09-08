# CircuitNET NG security and visibility

CircuitNET NG transport between configured nodes is encrypted and authenticated.
Enroll the intended neighbor's certificate and Node ID/profile binding through an
agreed administrative exchange. Verify Test Link before enabling unattended Events.
Keep keys outside public content, protect backups, and agree certificate rotation.

TLS protects packets between linked nodes. Public conference messages are not
end-to-end encrypted. After delivery, messages are readable according to each
BBS's conference access rules. Intermediate routing nodes can process and retain
transit content and provenance. Directed routing does not make a message private.
Local private BBS messages, where offered, are restricted by BBS access controls
unless an actual separate feature explicitly provides end-to-end encryption.

Use approval-by-default remote Dossier control; only authenticated direct children
may request changes to their own subscriptions. Diagnose an unexpected request by
request identity and outcome, without copying message bodies or credentials into
logs. Hold a suspect peer, review its configured identity, and arrange recovery.
Events do not override transport authentication, conference mapping or holds.

## Signed catalog and downloadable files

Catalog authenticity survives caching, offline import and HOST forwarding because
an independently pinned publisher signs each revision. Verify the trust fingerprint
outside the file being verified; a kit manifest detects corruption but is not itself
an independent trust root. Keep catalog keys separate from public artifacts. Hash
chains detect forks, missing predecessors and rollback against retained state; an
empty replacement must recover current trusted history and sync before export.

Published files are available according to destination File Area rules, not made
private by TLS. Receiving boards enforce local inspection/scanning requirements;
remote assertions do not bypass quarantine. No application form requests secrets.
Only deliberately public membership information belongs in a published registry.

## Two keys, two responsibilities

A node's TLS private key authenticates its live links and stays in protected board
custody. The catalog signing key authorizes conference definitions and can remain
on an offline signing system. Never send either key in a membership application.
Use only the public certificates and fingerprints during enrollment.

Ordinary configuration cannot silently replace the catalog authority. Planned
replacement uses a transition signed by both old and new authorities; emergency
replacement requires a recorded decision and independent fingerprint confirmation
by each local Sysop. Neither arrives as automatically trusted network configuration.
See [KEY-CUSTODY](KEY-CUSTODY.md) for backup, loss, compromise and exact commands.

SUPPORT, SYSOP, SPITFIRE and DOORS are operator-only conferences. Access is restricted
by each board's native conference rules, including verified visiting Sysops where
locally granted. This is access control, not end-to-end encryption. Catalog entry,
Node ID or a successful TLS connection never grants a caller Sysop privileges.
