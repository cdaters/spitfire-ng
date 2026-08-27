# CircuitNet Preservation and Modernization Specification

## 1. Purpose

This document defines the goals for preserving and potentially resurrecting the CircuitNet message network originally used by SPITFIRE Bulletin Board Systems.

CircuitNet should be treated as both:

1. a historical SPITFIRE networking system worthy of preservation
2. a candidate for modern functional revival

Modernization should preserve the recognizable CircuitNet model while replacing obsolete or unsafe assumptions where appropriate.

## 2. Historical Role

CircuitNet was designed as a message-networking system specifically associated with SPITFIRE BBS installations.

Its operational model included concepts such as:

    Node IDs
    Host systems
    Dependent nodes
    Conferences
    Dossiers
    Routing
    Packet export
    Packet import
    Duplicate/history tracking

CircuitNet used store-and-forward techniques suitable for dial-up-era BBS operation.

## 3. Preservation Goals

The project should preserve:

- original CircuitNet software archives
- documentation
- configuration formats
- packet formats
- conference mappings
- node addressing
- routing behavior
- dossier behavior
- host/dependent relationships
- control-message syntax
- duplicate-history logic
- example packets where available

Documentation should distinguish confirmed behavior from reconstruction.

## 4. Known Historical Components

The CircuitNet distribution includes tools with roles such as:

    CONFIG
    PRIMER
    EXTRACT
    IMPORT
    MAILCALL

The precise role of each tool should be documented from original manuals and controlled testing.

The modern implementation should reproduce required behavior internally rather than require these DOS programs.

## 5. Historical Workflow

The approximate historical workflow is:

    SPITFIRE Message Bases
             |
           PRIMER
             |
             v
       Network Message Data
             |
           EXTRACT
             |
             v
        Node Packet Files
             |
         compression
             |
       physical/dial-up exchange
             |
         decompression
             |
           IMPORT
             |
             v
       SPITFIRE Messages

This workflow should be verified and documented in detail.

## 6. Node Identity

Historical CircuitNet nodes use alphanumeric node identifiers.

The project should determine:

- maximum identifier length
- permitted characters
- hierarchy or numbering conventions
- whether identifiers contain geographical or hub information
- reserved IDs

Original identifiers should remain valid in compatibility mode.

## 7. Host and Dependent Nodes

CircuitNet appears to organize systems into hosts and dependent nodes.

The modern implementation should preserve this topology.

Conceptually:

    CircuitNet Host
        |
        +-- Node A
        +-- Node B
        +-- Node C

The host may maintain routing and conference-distribution information for dependent systems.

## 8. Dossiers

Dossiers define which conferences or traffic a dependent node receives.

The exact historical format should be documented.

Modern CircuitNet should retain the dossier concept because it is part of CircuitNet's identity.

A dossier may conceptually contain:

    Node ID
    Conference Subscription
    Routing Permission
    Status
    Last Exchange
    Optional Modern Identity

## 9. Conference Codes

CircuitNet conference identifiers should remain separate from local SPITFIRE conference numbers.

Mapping should therefore resemble:

    CircuitNet Conference
             |
             v
    Local SPITFIRE Conference

Example:

    CircuitNet Code: SPITFIRE
    Local Conference: 12

Mappings should be editable through:

- configuration files
- local console
- web administration

## 10. Routing

CircuitNet supports routed messages.

Historical syntax includes route information embedded in messages or control fields.

The modern implementation should preserve recognizable routing syntax for compatibility.

Internally, routing should be represented structurally.

Possible representation:

    Source Node
    Destination Node
    Next Hop
    Conference
    Route History

## 11. Legacy Packet Compatibility

Original packet files should be supported where technically and legally practical.

Known packet extensions include:

    .CNP
    .CND

The project should document:

- file header
- record layout
- message format
- destination information
- conference information
- routing data
- checksums if present
- version fields
- compression assumptions

Legacy packet readers must treat all packet contents as untrusted input.

## 12. Historical Transport

Original CircuitNet transport may have relied on:

- telephone calls
- file transfers
- external mail programs
- manually exchanged archives

The modern implementation should not require these mechanisms.

However, offline packet exchange should remain possible.

## 13. Modern Transport

A modern CircuitNet implementation should support direct TCP/IP transport.

Possible secure transport:

    CircuitNet Node
          |
        TLS
          |
    CircuitNet Node

The packet contents may retain legacy-compatible message representation while the transport becomes modern.

## 14. Secure Node Identity

Historical CircuitNet trust mechanisms may rely on information that is easy to spoof today.

Modern operation should therefore support cryptographic node identity.

A node may possess:

    Node ID
    Public Key
    Private Key

The public key becomes associated with the registered CircuitNet node.

This should not change the caller-visible CircuitNet address.

## 15. Packet Envelope

A secure modern packet may wrap legacy-compatible content in a modern envelope.

Example:

    CircuitNet Version
    Packet UUID
    Source Node
    Destination Node
    Created Time
    Sequence Number
    Payload Type
    Payload Length
    Payload Hash
    Digital Signature
    Payload

The legacy payload may still contain traditional messages.

## 16. Administrative Commands

CircuitNet historically supports remote control operations.

These behaviors should be documented and preserved semantically.

However, authorization should not depend solely on message fields such as:

    From Name

Modern administrative commands should require:

    authenticated source node
    authorized key
    permitted command
    valid command syntax

Legacy command packets may still be parsed for preservation and compatibility.

## 17. Legacy Mode

Legacy Mode should prioritize historical interoperability.

Possible capabilities:

    Read original packets
    Write original packets
    Preserve original routing syntax
    Import historical conference data
    Reproduce dossier behavior
    Perform manual packet exchange

Legacy Mode may display warnings when insecure historical controls are encountered.

It should not unnecessarily prevent preservation work.

## 18. Secure Mode

Secure Mode should retain CircuitNet concepts while introducing:

    authenticated nodes
    encrypted transport
    signed packets
    replay protection
    modern routing state
    stronger administrative authorization

Secure Mode should remain recognizable as CircuitNet rather than becoming an unrelated protocol.

## 19. Replay Protection

Secure packets should include values such as:

    packet UUID
    sequence number
    creation time

Nodes should record recently processed packets.

A valid signed packet should not be accepted repeatedly merely because it can be replayed.

## 20. Duplicate Message Detection

Historical CircuitNet uses history information to prevent duplicate messages.

This behavior should be preserved.

Modern operation may additionally maintain:

    message hash
    origin node
    network message ID

The historical history-file behavior should remain documented even if the internal implementation changes.

## 21. Compression

Historical CircuitNet may package traffic using ZIP archives or similar tools.

Legacy compatibility should support original archive workflows.

Modern direct transport should not require external compression utilities.

Optional packet compression may be built into the protocol.

## 22. SPITFIRE Integration

CircuitNet should appear inside SPITFIRE as a native message-network module.

Example configuration:

    Network:
        CircuitNet

    Node ID:
        503004

    Host:
        503000

    Poll:
        Every 30 minutes

    Conferences:
        SPITFIRE -> Local 10
        GENERAL  -> Local 11

CircuitNet messages should appear as normal SPITFIRE conference messages.

## 23. Local Conference Storage

CircuitNet should not require a particular message backend.

Possible targets:

    Native SPITFIRE
    SMB
    SQLite

This permits a historically compatible network to operate over modern storage.

## 24. Web Administration

The web interface may provide:

    CircuitNet Status
    Node Identity
    Host
    Routes
    Conferences
    Dossiers
    Pending Packets
    Packet History
    Last Poll
    Errors

The web layer should not expose private keys.

## 25. Terminal Administration

Traditional Sysop access should remain available.

Possible commands:

    CircuitNet Status
    Poll Now
    Export Packet
    Import Packet
    List Nodes
    List Routes
    List Conferences

This helps retain the traditional BBS-admin experience.

## 26. Offline Preservation Mode

CircuitNet should remain usable without any live network.

A researcher or Sysop should be able to:

    import old packets
    inspect messages
    export packets
    recreate routing
    examine dossiers

entirely on a local machine.

## 27. CircuitNet Registry

If CircuitNet is revived as an active network, a modern optional registry may maintain:

    Node ID
    System Name
    Sysop
    Public Key
    Host
    Network Capabilities
    Last Seen

Registration should not prevent private or experimental CircuitNet networks.

## 28. Historical Node Preservation

If historical CircuitNet node identities can be verified, the project should preserve them where practical.

For example, a Sysop possessing a historical system configuration might be allowed to associate:

    Historical Node ID

with:

    Modern CircuitNet Identity

without erasing the historical identifier.

## 29. Modern Private Networks

Sysops should be able to create independent CircuitNet networks.

Example:

    Network Name:
        RetroRealm CircuitNet

    Root Host:
        RR0001

This enables experimentation without depending on a central infrastructure.

## 30. CircuitNet Compatibility Testing

Test fixtures should eventually include:

    original packets
    known conference maps
    dossiers
    routing examples
    packet history
    multi-hop traffic
    malformed packets
    duplicate packets
    control commands

Tests should verify both:

    historical compatibility

and:

    modern secure behavior

## 31. Legal and Historical Care

The project should distinguish:

- original CircuitNet code
- published documentation
- observed file behavior
- independently created modern code

New implementation code should be independently written.

Historical software should be preserved as archival material rather than copied into the modern implementation.

## 32. Long-Term Objective

The ideal result is that CircuitNet can exist in three forms:

### Historical

Original DOS utilities and packets preserved for study.

### Compatible

Modern SPITFIRE can exchange historical CircuitNet messages and packets.

### Revived

Modern systems can participate in an active CircuitNet using Internet transport and modern node authentication.

The goal is not merely to remember CircuitNet.

The goal is to make it possible for CircuitNet to speak again.
