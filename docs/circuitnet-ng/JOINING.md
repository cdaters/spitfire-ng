# Joining CircuitNET NG

<!-- public-identity:start -->
Applications Closed. Keep your form locally until applications open.

When opened, the application location is https://circuitnetng.org/apply
and the joining contact is join@circuitnetng.org.
Do not send passwords, private keys or authentication secrets.

Founding Network Administrator role contact: founder@circuitnetng.org.

These are canonical locations; web, mail and download deployment has not
been verified. See [JOINING-INFO](JOINING-INFO.md) for opening status.
<!-- public-identity:end -->

## Before applying

Read the [Charter](CHARTER.md), [Rules](RULES.md) and
[conference list](CONFERENCES.md). Check [current joining information](JOINING-INFO.md)
for application availability and the approved contact. If applications are not
open, keep your completed form locally and check that information again later.
The addresses above identify the planned submission paths, not an open service.

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

## Collect these before configuration

Your approval contact should supply your assigned Node ID and role, your HOST's
Node ID, the approved complete tree, the HOST endpoint/port/certificate name, and
its public certificate with an independently confirmed fingerprint. Agree how your
own public certificate will be enrolled. Obtain the current signed catalog and
predecessors, accepted catalog authority fingerprint, Charter/Rules, required
conference list, subscription approval procedure and agreed polling interval.
Private keys stay on their owning systems. If any of these is missing, ask your
HOST before configuring a live link.

Give country and region by name for the [address assignment](ADDRESSING.md).
Do not choose a numeric range as a substitute for administrative approval.
The application separates private review information, the minimal public membership
record and optional directory publication choices. Applications remain Closed.
