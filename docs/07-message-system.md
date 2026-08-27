# SPITFIRE Message System Design

> **Implementation note (2026-08-21):** Stock Core Increment 3 implements the
> first native SQLite backend and caller-facing conference path. See the
> canonical [Native SPITFIRE NG Message System](sfng-message-system.md) for the
> exact schema, authorization rules, implemented commands, tests, and current
> fidelity gaps. This document remains the broader multi-backend design.

## 1. Purpose

This document defines the architecture and compatibility goals for the modern SPITFIRE message system.

The message system must preserve the traditional SPITFIRE caller experience while supporting both historical and modern storage formats and message networks.

The design should allow a caller to use the familiar SPITFIRE message interface without needing to know whether a conference is stored in:

- an original SPITFIRE message base
- Synchronet SMB
- a modern internal database
- a network-backed or virtual message base

The message system should separate:

1. message presentation
2. message storage
3. message networking

These are related responsibilities but should not be permanently coupled.

## 2. Design Principle

To the caller:

> A message conference is a SPITFIRE message conference.

The storage backend and network source should normally remain invisible.

For example:

    Conference 1   General
    Conference 2   Retro Computing
    Conference 3   DOVE-Net General
    Conference 4   FidoNet BBSing
    Conference 5   CircuitNet SPITFIRE

may internally use entirely different storage or routing mechanisms.

The caller should still interact with them through the same familiar SPITFIRE commands.

## 3. Core Message Model

The internal message representation should contain enough information to represent both historical SPITFIRE messages and modern networked messages.

A conceptual message structure may include:

    Message ID
    Conference ID
    Message Number
    Original Message Number

    From
    To
    Subject

    Date Created
    Date Imported
    Date Modified

    Message Body

    Private
    Received
    Deleted
    Sent
    NetMail
    Purge When Sent

    Thread Parent
    Thread Root
    Reply References

    Network Type
    Network Address
    Network Message ID

    Character Encoding

    Origin Information
    Routing Information

    Imported Metadata
    Backend Metadata

Not every backend needs to use every field.

Unknown historical information should be preserved where practical.

## 4. Message Backend Interface

Message storage should be accessed through a common internal interface.

Conceptually:

    MessageBackend
        |
        +-- list_conferences()
        +-- read_message()
        +-- write_message()
        +-- delete_message()
        +-- mark_received()
        +-- get_last_read()
        +-- set_last_read()
        +-- scan_messages()
        +-- rebuild_index()
        +-- validate()

Backends may include:

    LegacySpitfireBackend
    SMBBackend
    NativeBackend
    GatewayBackend

The session engine should not directly manipulate backend files.

## 5. Native SPITFIRE Backend

The legacy backend should aim to read and write historical SPITFIRE message files directly.

Known components include:

    SFMSGx.DAT
    SFMSGx.PTR
    SFMSGx.IDX
    SFMSGx.LMR
    SFMCONF.DAT

The exact SPITFIRE 3.7 structures should be documented through:

- original Buffalo Creek documentation
- actual populated message bases
- binary analysis where necessary
- compatibility testing
- third-party SPITFIRE utilities

The backend should preserve unknown or reserved fields when rewriting records whenever practical.

## 6. Original Message Numbering

Historical SPITFIRE numbering behavior should be reproduced where possible.

The implementation should distinguish between:

    Internal Message ID

and:

    Caller-visible Message Number

This permits modern storage systems to use stable internal identifiers without altering the traditional message-number behavior.

Legacy imports should preserve original message numbers whenever possible.

## 7. Last Message Read

Last-message-read behavior is central to the BBS experience.

The modern system should support:

- per-caller conference pointers
- new-message scans
- conference-specific last-read values
- optional synchronization with QWK
- preservation of imported legacy pointers

Historical `SFMSGx.LMR` support should remain available for native legacy message bases.

Modern backends may store these pointers independently.

## 8. Message Threading

Original SPITFIRE threading semantics should be preserved where they are understood.

Modern message formats may include richer threading metadata.

The presentation layer should map those relationships into the traditional SPITFIRE experience.

Thread information may include:

    Parent Message
    Root Message
    Reply Chain
    Network Reply ID

The system should avoid destroying network threading information simply because the historical SPITFIRE format cannot represent all of it.

## 9. Private Messages

Private messages should retain their historical meaning.

The server must enforce privacy through authorization rather than relying only on display filtering.

A caller should not be able to retrieve a private message through:

- a web API
- direct message-number access
- a network endpoint
- malformed requests

unless authorized.

Imported legacy private-message flags should remain honored.

## 10. Deletion

Historical SPITFIRE may mark messages deleted before physical maintenance removes them.

The modern system should distinguish between:

    Logical deletion
    Physical purge

This permits compatibility with historical behavior while allowing modern storage maintenance.

Network messages may require additional rules depending on the network involved.

## 11. Message Maintenance

Traditional maintenance concepts should remain available.

Possible operations include:

    Pack Message Base
    Rebuild Index
    Purge Deleted Messages
    Validate Message Base
    Rebuild Last-Read Data
    Export Conference
    Import Conference

Equivalent functionality previously handled by external maintenance utilities may become built-in while retaining familiar terminology.

## 12. Synchronet SMB Backend

SMB should be supported as a first-class message backend.

Potential benefits include:

- DOVE-Net interoperability
- mature message storage
- thread metadata
- network metadata
- existing tooling

The SPITFIRE caller interface should remain independent of SMB terminology.

A conference backed by SMB should still appear as a SPITFIRE message conference.

## 13. Modern Native Backend

A modern backend may use SQLite or another embedded database.

Its goals should include:

- transactional integrity
- recovery
- efficient search
- indexing
- Unicode metadata where appropriate
- large message bases
- stable identifiers

SQLite is a strong initial candidate because it is:

- portable
- embedded
- widely supported
- simple to back up
- available on all primary target platforms

The database should remain an implementation detail rather than replacing the traditional BBS experience.

## 14. Mixed Backends

A single installation may use multiple backend types simultaneously.

Example:

    Conference 1
    General
    Backend: Native SPITFIRE

    Conference 2
    Local Technical
    Backend: SQLite

    Conference 10
    DOVE-Net General
    Backend: SMB

    Conference 30
    CircuitNet SPITFIRE
    Backend: Native SPITFIRE

All should appear in the normal conference list.

## 15. Message Network Adapters

Network protocols should connect to the message system through adapters.

Examples:

    QWK Adapter
    DOVE-Net Adapter
    FidoNet Adapter
    CircuitNet Adapter

An adapter converts between:

    Network Representation
            and
    Internal SPITFIRE Message

This avoids embedding network-specific logic throughout the message engine.

## 16. QWK

QWK support should be built into the message system rather than treated merely as an external door.

The interface may preserve LAKOTA terminology and behavior.

Required capabilities include:

    QWK packet creation
    REP reply import
    conference mapping
    new-message selection
    private messages
    last-read updates
    aliases
    network extensions

Original LAKOTA may remain runnable in legacy mode where practical.

## 17. DOVE-Net

DOVE-Net should use the QWK networking adapter.

DOVE-Net conferences should be mapped into normal SPITFIRE conferences.

Network-specific metadata should be preserved internally even when the caller interface does not expose it.

## 18. FidoNet

FidoNet messages require support for metadata beyond a traditional local message.

Possible fields include:

    FTN source address
    FTN destination address
    AREA
    MSGID
    REPLY
    SEEN-BY
    PATH
    origin line
    tear line
    message attributes

The internal model should preserve these fields without requiring the native SPITFIRE message format to contain them directly.

## 19. CircuitNet

CircuitNet conferences should also map into SPITFIRE message conferences.

Historical CircuitNet metadata may include:

    CircuitNet Node ID
    Destination Node
    Conference Code
    Route
    Packet Identifier
    History/Duplicate Information

Legacy packet compatibility and secure modern transport should both feed the same message adapter.

## 20. Character Encoding

Historical BBS material may contain:

    CP437
    ASCII
    ANSI escape sequences

Modern message networks may contain:

    UTF-8
    extended Unicode

The system should record or infer encoding where possible.

Historical content should not be silently damaged through automatic conversion.

The terminal layer may perform presentation conversion when configured.

## 21. Web Message Access

The web component may provide optional message reading and posting.

Web access should use the same message authorization rules as terminal access.

The web interface must not become a separate message system.

A message posted from the browser should be indistinguishable in the message base from one posted through Telnet or SSH except for optional metadata.

## 22. Search

Modern installations may provide message search.

Search should operate across backend types through a common interface.

Possible filters include:

    conference
    sender
    recipient
    subject
    date
    text
    network
    message number

Legacy message bases may use generated indexes to provide efficient searching without altering their underlying format.

## 23. Web and Terminal Consistency

Actions performed through the web interface should remain visible through the terminal interface and vice versa.

Examples:

    Mark message read
    Post message
    Delete message
    Change conference
    Update last-read pointer

The browser is another interface to SPITFIRE, not a parallel service.

## 24. Message Integrity

The system should detect:

    truncated records
    invalid pointers
    invalid indexes
    impossible record counts
    orphaned body records
    duplicate message identifiers

Validation should not automatically destroy questionable legacy data.

Where practical, repair tools should provide:

    Inspect
    Repair
    Backup
    Report

before modifying historical files.

## 25. Migration

A legacy message base should be usable in one of three ways:

### Direct

Use the original files as the active backend.

### Mirror

Maintain a modern representation while preserving the legacy copy.

### Import

Convert the historical messages into another backend.

The Sysop should choose.

## 26. Guiding Rule

Modern message capabilities should expand what SPITFIRE can do without requiring the old message system to disappear.

The desired result is:

> Original SPITFIRE messages, SMB messages, DOVE-Net, FidoNet, QWK and CircuitNet should all feel like they belong in the same SPITFIRE Message Section.
