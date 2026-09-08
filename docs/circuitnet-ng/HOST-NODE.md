# Operating a CircuitNET NG HOST

Configure your parent and directly serviced children, with exact certificate
bindings. An Event may target the parent, an individual child, or all eligible
neighbors. A child may initiate the connection and receive its pending mail. Manage
child Dossiers locally; approve/deny their authenticated requests in Networks.
Request only your own upstream subscriptions at your parent. Route Test should
keep a sibling destination in your branch and send other branches toward the ROOT.
Hold one peer without changing another peer's subscriptions or queue truth.

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

## Signed catalog forwarding

A HOST verifies its parent's catalog revisions and forwards the original signed
objects to its direct children through normal exchanges. It does not become the
publisher or grant itself governance powers. Keep upstream and child Events useful;
a child's outbound poll can receive updates even without its own inbound listener.
Review mapping attention and retired areas, and approve only Active-area Dossier
requests. A newly reused codename has a different immutable ID and requires fresh
local choice/subscription authority. Do not convert catalog receipt into mass subscribe.
