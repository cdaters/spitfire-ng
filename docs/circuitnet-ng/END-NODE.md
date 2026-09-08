# Operating a CircuitNET NG END

Configure one upstream HOST and no children. Use a stable unique Node ID and the
same explicit network tree as your neighbors. Request only your own Dossier changes
at that HOST; it chooses approval, auto-approve or disabled remote changes. For a
firewall/NAT installation, an outbound Hybrid or Scheduled Poll can collect mail
without exposing an inbound listener. Route Test confirms the first hop is your
HOST. An END never carries unrelated transit traffic.

Before enabling unattended exchange, configure native CNTEST mappings and Dossiers,
Test Link, run Route Test, and perform one Poll Now. Create an Event with the intended
profile/neighbor, schedule and timezone. Watch next due, last result, peer health and
queued work in Events and CircuitNET Networks. Enabled listeners alone do not initiate
outbound calls. Normal fanout requires a Dossier; directed traffic follows its typed
route and still requires an authorized receive mapping at the destination.

If mail does not move, follow the [operator troubleshooting path](../manual/events.md#why-didnt-this-message-move).
Keep cold backups, review pending requests and failed Events, and test recovery on
disposable boards. Directed conference traffic remains a conference message, and
transport encryption does not change who may read it after delivery.

See [CircuitNET commands](../manual/circuitnet.md) and [Events](../manual/events.md).

## Catalog-enabled membership

Obtain Network Kit 1.0, review Charter/Rules and apply for an approved Node ID.
Enroll the catalog pin separately from TLS identity and import its signed seed.
Poll your parent to sync revisions, then create/map/ignore areas deliberately.
Request only desired Active-area subscriptions; core omissions require an agreed
remedy. An END behind NAT may rely on scheduled outbound Polls for both catalog
updates and incoming traffic. Catalog retirement leaves a local archive; it does
not erase messages. Use Catalog Administration for mapping and recovery details.
