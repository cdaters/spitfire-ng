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
