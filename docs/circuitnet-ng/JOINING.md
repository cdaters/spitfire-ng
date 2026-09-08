# Joining CircuitNET NG

## Before applying

Read the [Charter](CHARTER.md), [Rules](RULES.md) and
[conference list](CONFERENCES.md). Check [current joining information](JOINING-INFO.md)
for application availability and the approved contact. If applications are not
open, keep your completed form locally and check that information again later.
There is no implied submission address.

The network begins under a Founding Network Administrator. That person approves
initial members and Node IDs, maintains records and arranges upstream HOSTs until
the Charter's committee transition takes effect. ROOT is a technical routing role;
it does not itself grant permanent governance authority.

## Apply

Complete [the application](NODE-APPLICATION.md). Supply a working contact and only
information you intend the administrators to use. Public listing of your contact
or address needs your explicit consent. Do not send passwords or private keys.

Most boards join as END nodes. An END does not need a static IP or an inbound
listener: it can poll its HOST from behind NAT or a firewall. Request HOST status
only if you can reliably serve downstream boards.

You may request a 1-8 character alphanumeric Node ID. The Administrator checks
network-wide uniqueness and confirms the assignment. An application alone does
not reserve an ID. The Secretary retains assignment and retirement history;
private application details are not automatically published in a node registry.

## After approval

You should receive your Node ID, role, parent HOST, approved tree configuration,
HOST contact, connection address and port, certificate-enrollment instructions,
and the catalog authority fingerprint through an agreed administrative channel.
No live endpoint or port is assigned by this kit's example file.

Enroll credentials separately, then follow [END-NODE](END-NODE.md): configure the
profile, verify the HOST, import the catalog, choose local mappings, request
subscriptions and set an exchange Event. Arrange a short operator test in SUPPORT
with your HOST before posting it. Public conversation can use CHITCHAT.

Tell your HOST about planned outages and changes to connection or certificate
information. Node retirement and reassignment are recorded; old identity and
traffic history are not silently erased.
