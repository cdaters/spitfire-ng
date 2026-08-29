# SPITFIRE Message Network Architecture

## 1. Purpose

This document defines how the modern SPITFIRE system should participate in local and distributed message networks.

Initial networking targets include:

- QWK
- QWK networking
- DOVE-Net
- FidoNet
- CircuitNet

The architecture should remain extensible for additional network types.

## 2. Architectural Principle

A message network is a transport and routing system.

It should not dictate:

- the BBS user interface
- the message backend
- the operating system
- the terminal protocol

Network modules should exchange messages with the SPITFIRE message system through a common internal representation.

Current source makes the local schema-11 message domain authoritative for
payloads, delivery identities, recipients/audiences, visibility, receipts,
lifecycle, lineage, and mutation audit. Future adapters must import/export
through the message-domain interfaces instead of creating a parallel message
system or writing domain tables directly. No QWK/LAKOTA, SMB/DOVE-Net,
FidoNet, or CircuitNet adapter is implemented yet.

## 3. Network Adapter Model

Conceptually:

    SPITFIRE Message
          |
          v
    Network Adapter
          |
          v
    Network Packet
          |
          v
      Transport

Incoming traffic follows the reverse path.

Each adapter is responsible for:

- message conversion
- network addressing
- routing metadata
- duplicate detection
- import/export rules
- network-specific attributes

## 4. Network Module Interface

A network adapter may implement operations such as:

    export_messages()
    import_messages()
    poll()
    route()
    validate_packet()
    update_history()
    process_control_message()

The SPITFIRE core should not need to know packet-layout details.

## 5. Network Scheduling

Message networking should integrate with the SPITFIRE Event system.

Examples:

    Event A    Poll FidoNet
    Event B    Exchange CircuitNet
    Event C    Poll DOVE-Net
    Event D    Build QWK packets

Modern scheduling may additionally support:

    every 15 minutes
    hourly
    daily
    manual poll
    event-triggered poll

Historical Event terminology should remain available.

## 6. QWK Offline Mail

QWK should serve two purposes:

### Offline Reader Support

Callers download messages and upload replies.

### Network Transport

BBS systems exchange messages through QWK networking.

Both should use the same underlying QWK implementation where practical.

## 7. LAKOTA Compatibility

The modern QWK subsystem should preserve the role of LAKOTA.

Possible terminal presentation:

    <L>.... LAKOTA QWK Mail System

The implementation may be integrated into SPITFIRE rather than executed as a separate program.

Where possible:

- existing QWK readers should work
- existing REP files should import correctly
- historical conference behavior should remain familiar

## 8. QWK Packet Handling

The implementation should safely process:

    CONTROL.DAT
    MESSAGES.DAT
    *.NDX
    PERSONAL.NDX
    NEWFILES.DAT
    BULLETINS
    attachment extensions where supported

Malformed packets should be rejected gracefully.

Archive extraction should enforce:

- size limits
- file-count limits
- path normalization
- no directory traversal
- no overwrite of system files

## 9. QWK Networking

QWK networking should support:

- node identifiers
- conference mapping
- ADD/DROP behavior where applicable
- message IDs
- routing information
- duplicate detection
- extended headers where required

The system should allow network-specific extensions without changing the basic caller experience.

## 10. DOVE-Net

DOVE-Net should be implemented as a QWK network profile.

A DOVE-Net profile may define:

    Hub
    Node ID
    Authentication
    Poll Schedule
    Conference Maps
    Packet Directory
    History

DOVE-Net conferences should map directly into SPITFIRE message conferences.

Example:

    DOVE-Net General
    DOVE-Net Debate
    DOVE-Net Entertainment
    DOVE-Net Programming

The caller should not need to know Synchronet is involved unless the Sysop wishes to display that information.

## 11. SMB and DOVE-Net

SMB may be used as the storage backend for DOVE-Net conferences, but it should not be mandatory.

Possible configurations:

    DOVE-Net
        |
        +-- SMB
        +-- Native SPITFIRE
        +-- SQLite

The network layer and storage layer should remain separable.

## 12. FidoNet

FidoNet should be implemented using native FTN concepts.

The system should understand:

    zone:net/node.point

and support:

    NetMail
    EchoMail
    conference mapping
    packet processing
    dupe checking
    message attributes
    routing
    origin lines
    tear lines

## 13. BinkP

Modern FidoNet transport should support BinkP over TCP/IP.

This removes the need for:

    modem emulation
    virtual serial ports
    external dialers

External mailers may remain supported for Sysops who prefer them.

## 14. FidoNet Separation of Responsibilities

The FidoNet implementation may be divided into:

    FTN Message Adapter
    Packet Processor
    Tosser
    Router
    BinkP Transport

This allows future replacement or external integration of individual components.

## 15. FidoNet Conference Mapping

A SPITFIRE conference may map to an FTN echo area.

Example:

    SPITFIRE Conference:
        Retro Computing

    FTN Area:
        RETROCOMPUTING

The mapping should contain:

    local conference ID
    FTN area tag
    security
    read-only status
    network address
    origin settings

## 16. NetMail

NetMail should map into a dedicated SPITFIRE conference or mailbox view.

The caller interface should remain familiar while retaining FTN destination addressing.

Possible workflow:

    Enter To:
    Enter FTN Address:
    Enter Subject:
    Enter Message:

For experienced users, network address entry may be integrated into the traditional To field.

## 17. Duplicate Detection

Every network adapter should provide network-appropriate duplicate detection.

Potential keys include:

    network message ID
    origin node
    packet ID
    CRC/hash
    timestamp
    message number

Duplicate detection state should be stored independently from caller-visible messages where practical.

## 18. Routing

The network layer should distinguish between:

    direct destination
    next hop
    final destination

This is particularly important for:

    FidoNet
    CircuitNet

Routing tables should be human-readable where possible.

## 19. Network Identities

Each network may maintain its own identity.

Examples:

    SPITFIRE System ID
    QWK Node ID
    DOVE-Net Node ID
    FidoNet Address
    CircuitNet Node ID

These should not be forced into a single identifier.

A board may participate in multiple networks simultaneously.

## 20. Network Credentials

Network passwords, keys and tokens should be stored separately from normal caller configuration.

They should never be:

- shown in ordinary logs
- exposed through message macros
- written to door drop files
- available to DOS doors

## 21. Network Status

The Sysop should be able to view networking status through:

### Local Console

### Terminal Sysop Menu

### Web Administration

Possible information:

    Network
    Last Poll
    Last Successful Exchange
    Messages Imported
    Messages Exported
    Pending Messages
    Last Error

## 22. Manual Networking

Traditional Sysops should still be able to trigger operations manually.

Examples:

    Poll FidoNet Now
    Export CircuitNet Packet
    Build QWK Packet
    Import Network Mail

This is particularly valuable for preservation and troubleshooting.

## 23. Store-and-Forward

The architecture must support store-and-forward networks.

CircuitNet and traditional FidoNet routing may not require an always-connected destination.

The system should therefore be comfortable with:

    queue
    package
    transport later
    forward
    retry

Modern TCP/IP should not imply that every network becomes a real-time service.

## 24. Offline Exchange

Some network operations should remain capable of functioning without direct Internet connectivity.

Examples:

    export CircuitNet packet to removable media
    import received packet manually
    create QWK packet
    import REP packet
    export FTN packets

This supports historical experimentation and isolated systems.

## 25. Security and Compatibility

Legacy packet formats may remain supported even when their original authentication mechanisms are weak.

The system should distinguish between:

    Packet Compatibility
    Transport Security
    Administrative Trust

For example, a legacy CircuitNet message format may travel inside a modern authenticated connection.

## 26. Network Extension Model

Future network adapters may include:

    NNTP
    Matrix gateway
    Usenet bridge
    custom hobby networks

These should be optional extensions.

Adding them must not change the historical SPITFIRE message experience.

## 27. Guiding Principle

SPITFIRE should behave as one BBS connected to many possible message networks, not as a collection of unrelated networking programs.

To the caller:

> Messages are messages.

To the Sysop:

> Networks are configurable transports and routing systems.
