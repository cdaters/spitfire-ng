# SPITFIRE Modern System Architecture

## 1. Purpose

This document defines the proposed high-level architecture for the modern SPITFIRE Bulletin Board System.

The architecture is intended to support:

- historical SPITFIRE compatibility
- modern operating systems
- native Internet connectivity
- web access
- traditional BBS clients
- multiple message-base formats
- legacy DOS doors
- modern networking
- reasonable security
- long-term maintainability

The architecture should remain portable and avoid unnecessary dependence on any single operating system, processor architecture, database, or network protocol.

## 2. Architectural Principle

SPITFIRE should be designed as a portable core surrounded by replaceable interfaces.

The central BBS engine should understand concepts such as:

- callers
- sessions
- nodes
- menus
- message conferences
- file areas
- doors
- events
- security levels

It should not inherently depend on:

- Windows
- Linux
- macOS
- DOS
- COM ports
- FOSSIL drivers
- a particular database
- Telnet
- HTTP
- a specific terminal emulator

These should exist at the edges of the system.

### Implemented native boundary through Increment 2

The first native runtime foundation establishes two cohesive crates:

- `sf-core` owns portable board identity, validated configuration, logical
  SPITFIRE paths, SQLite migrations, Node/Session state, resource rendering,
  menu traversal, and the byte-oriented terminal boundary with capability
  metadata;
- `sf-bbs` owns fixture/resource loading, transport adapters, application
  lifecycle, logging, CLI handling, and the `spitfire` executable.

`sf-legacy` and `sf-reg` remain separate historical-format/research support.
The core does not know about sockets, serial device APIs, or command-line
arguments, and the application does not spread configured host paths into
domain behavior. In-memory, Telnet, raw TCP, RLogin, SSH, Unix stdio, and
direct serial adapters exercise the same `Terminal` and SPITFIRE session engine. The
optional inbound Hayes controller establishes a carrier-backed serial stream
and then enters that engine; it does not implement separate BBS behavior.

SQLite migration 1 contains schema history and singleton board identity;
later transactional migrations add their owning caller, message, file,
presentation, audit, access, identity, and public-information state. Schema 13
separates login identifier, public handle, and private real name without
changing stable caller ID or historical attribution. Current schema 14 adds
only versioned public-directory policy and caller choice, native ordered Other
BBS rows, content-addressed public-resource state, and content-free semantic
events. The exact caller model and modernization boundary are specified in
[Native Caller and Authentication Model](sfng-caller-authentication.md).

M043 implements caller-facing public information as a handle-only,
privacy-filtered projection rather than caller/configuration serialization.
Native Other BBS rows are board-local SQLite state; bulletins, newsletter, and
thought catalogs remain board-owned DISPLAY resources presented through the
existing resolver. See [Public Information](sfng-public-information.md).

## 3. Conceptual Architecture

    ┌──────────────────────────────────────────────────────┐
    │                   CONNECTION LAYER                   │
    │                                                      │
    │ Telnet  Raw  RLogin  SSH  Serial/Modem  Local Shell  │
    └──────┬───────┬──────────┬──────────────┬─────────────┘
           │       │          │              │
           └───────┴──────────┴──────────────┘
                          │
                  ┌───────▼────────┐
                  │ SESSION ENGINE │
                  └───────┬────────┘
                          │
              ┌───────────▼───────────┐
              │    SPITFIRE CORE      │
              │                       │
              │ Callers               │
              │ Menus                 │
              │ Security Levels       │
              │ Messages              │
              │ Files                 │
              │ Events                │
              │ Doors                 │
              │ Nodes                 │
              │ Display Engine        │
              └───────────┬───────────┘
                          │
        ┌─────────────────┼────────────────────┐
        │                 │                    │
    ┌───▼────┐       ┌────▼────┐         ┌─────▼─────┐
    │Storage │       │Networks │         │Extensions │
    └───┬────┘       └────┬────┘         └─────┬─────┘
        │                 │                    │
     Native SF          QWK                  Native
     SQLite             FidoNet              Doors
     SMB                DOVE-Net             DOS Doors
     Other              CircuitNet           Scripts

## 4. SPITFIRE Core

The SPITFIRE Core contains the historical behavior of the BBS.

It should expose well-defined internal interfaces for:

- caller management
- authentication
- security levels
- menus
- help
- messages
- files
- terminal display
- doors
- events
- nodes
- networking

The core should be usable without the web interface.

The core should also be usable without an Internet connection.

A Sysop should be able to launch SPITFIRE locally and interact with it entirely on the host computer.

## 5. Session Engine

Every caller connection should become a SPITFIRE session.

The transport used to reach the server should not fundamentally change the caller experience.

Possible transports include:

- Telnet
- raw TCP
- RLogin
- direct serial and Hayes-established inbound serial
- SSH
- browser WebSocket
- local console
- testing interface

A session should contain information such as:

    Session ID
    Node Number
    Caller
    Security Level
    Connection Type
    Terminal Type
    ANSI Capability
    RIP Capability
    Screen Dimensions
    Remote Address
    Login Time
    Last Activity
    Current Menu
    Current Conference
    Current File Area

The BBS core should interact with a generic terminal/session interface rather than directly with sockets.

Increment 1 implements one byte-oriented `Terminal` contract with optional
metadata for transport kind, local/remote scope, terminal type, ANSI and CP437
capability, dimensions, remote address, connection time, declared speed,
carrier state, and transport-supplied identity. Missing capabilities remain
absent rather than receiving fabricated values. Transport identity is metadata
only: RLogin declarations and local process ownership never authenticate a
SPITFIRE caller.

Current adapter status (including Increment 2 authentication):

| Adapter | Implementation status | Verification status |
|---|---|---|
| Telnet | Implemented with bounded option/subnegotiation handling, terminal type, and NAWS | Automated negotiation plus end-to-end loopback traversal |
| Raw TCP | Implemented without protocol negotiation | Automated and end-to-end loopback traversal |
| RLogin | Implemented with bounded initial framing; optional SyncTERM/Synchronet credential convention defaults off | Automated and end-to-end loopback traversal/login; valid supplied credentials use the ordinary verifier, while identity alone remains untrusted |
| Unix stdio shell | Implemented on Unix-like hosts | Automated adapter checks and manual common-session traversal |
| Direct serial | Implemented through a maintained serial API | Synthetic PTY tested; physical hardware unverified |
| Inbound Hayes modem | Implemented as a serial controller for initialization, answer, connect, and carrier loss | Deterministic simulation tested; physical hardware unverified |
| SSH | Implemented with `russh`, password-only caller authentication, PTY/resize, and a board-local Ed25519 key | Automated real-client protocol/no-shell/status tests plus macOS OpenSSH traversal; Qodem external SSH reached Main/Messages/Files; tested SyncTERM 1.9rc4 configuration did not complete the modern handshake |

Increment 4 replaces the initial Node 1 singleton with one configured,
race-safe pool. Every adapter acquires the lowest numbered available enabled
node; transport kind does not reserve a node. When every configured node is
occupied, an additional connection receives a bounded busy notice and
disconnects. This is coordination policy, not protocol-specific behavior.

The active session now carries an explicit unauthenticated, existing-login,
new-registration, or authenticated `CallerId` state. Untrusted transport
identity remains metadata only. SSH is the bounded exception: it verifies a
login identifier/password through the authoritative SPITFIRE credential
domain and passes a one-use caller grant to the common session, which reloads
lifecycle/security before post-login. It does not ask for the same credentials
twice. Clean logout, failed login, EOF, invalidation, and transport loss all
release the acquired node and finalize available call accounting through the
same runtime path. See [Secure SSH Caller Transport](sfng-secure-ssh-transport.md).

## 6. Node Model

The historical SPITFIRE node concept should remain.

A connected session receives a node number.

For example:

    Node 1    Local Sysop
    Node 2    Telnet caller
    Node 3    Browser caller
    Node 4    SSH caller

Node numbers should remain visible where historically appropriate.

Modern implementations may support substantially more nodes than the original software.

## 7. Terminal Abstraction

The terminal subsystem should interpret SPITFIRE output independently of the connection protocol.

It should support:

- ASCII
- IBM CP437
- ANSI
- ANSI color
- cursor positioning
- traditional BBS control codes
- SPITFIRE display macros
- RIP where practical
- UTF-8 as an optional modern capability

The system should not automatically convert historical CP437 artwork into UTF-8 unless explicitly configured.

Preserving original ANSI artwork correctly is more important than forcing modern character encoding everywhere.

## 8. Menu Engine

The menu engine should natively understand original SPITFIRE `.MNU` files.

It should preserve:

- command keys
- command descriptions
- security requirements
- internal command identifiers
- Sysop and caller menu behavior

The new engine may add extended command types, but historical menu definitions must remain valid.

## 9. Display Engine

The display engine should understand original display types:

    .BBS
    .CLR
    .RIP

The historical behavior of security-specific display files should be preserved.

For example:

    MAIN10.CLR
    MAIN50.CLR
    MAIN255.CLR

The engine should support original SPITFIRE substitution macros.

Modern extensions may introduce additional macros without altering historical ones.

## 10. Caller System

The caller system should support both historical and modern data representations.

A caller record may contain:

    Caller ID
    Handle
    Real Name
    Password Credential
    Security Level
    Location
    Last Call
    First Call
    Call Count
    Upload Count
    Download Count
    Time Used
    Preferences
    Terminal Capability
    Message Pointers
    Optional MFA Credentials

Historical fields that contain unnecessary personal information should not automatically be mandatory.

## 11. Storage Layer

Storage should be abstracted from BBS behavior.

Possible storage providers include:

### Native SPITFIRE

Reads and writes historical files directly.

### Modern Internal Storage

A modern database such as SQLite.

### SMB

Synchronet Message Base support.

### Future Providers

Additional storage engines may be added later.

The core should request information through a defined interface rather than opening database files directly.

## 12. Message Architecture

The message subsystem should distinguish between:

- message presentation
- message storage
- network transport

A message conference may therefore use:

    SPITFIRE native storage
    SMB storage
    modern internal storage

while participating in:

    local-only messaging
    QWK networking
    DOVE-Net
    FidoNet
    CircuitNet

The storage format should not dictate the network transport.

## 13. File Areas

Historical SPITFIRE file areas should remain recognizable.

Features may include:

- file descriptions
- upload/download permissions
- security levels
- download accounting
- upload accounting
- file search
- new-file scans
- optional checksums
- optional malware scanning
- optional web download access

Legacy file metadata should remain importable.

## 14. Door Architecture

Doors should be represented internally by a generic definition.

Example:

    Door ID
    Name
    Command Key
    Security Level
    Runtime Type
    Command
    Working Directory
    Drop File Type
    Network Permission
    Time Limit

Runtime types may include:

    native
    DOS
    script
    external service

Historical door letters such as Door A, Door B, and Door C may remain supported.

## 15. DOS Door Compatibility

Legacy DOS doors should run outside the SPITFIRE core process.

The BBS should:

1. create the required drop files
2. create a temporary session directory
3. launch the configured DOS environment
4. connect terminal input/output
5. monitor execution
6. retrieve updated drop-file information
7. destroy temporary session resources

Possible DOS environments may include:

- DOSBox variants
- DOSEMU where supported
- virtual machines
- other future compatibility systems

SPITFIRE itself should remain native.

## 16. Event System

Historical events should remain recognizable.

Examples:

    Event A
    Event B
    Event C

Events should be capable of launching:

- native commands
- scripts
- maintenance jobs
- message-network polling
- backups
- DOS utilities

Modern schedules may extend the original event system without eliminating it.

## 17. Network Modules

Network functionality should exist as independent modules.

Initial targets:

    QWK
    QWK networking
    DOVE-Net
    FidoNet
    CircuitNet

Future systems may be added without modifying the SPITFIRE core.

## 18. Web Layer

The web layer should be optional.

Possible features include:

    Home page
    BBS information
    Embedded terminal
    Caller account management
    Message browsing
    File browsing
    Sysop dashboard
    Node status
    Logs
    Configuration

A local-only installation should not require the web layer to be publicly reachable.

## 19. Browser Terminal

The browser terminal should behave as another SPITFIRE connection transport.

Architecture:

    Browser
       │
      WSS
       │
       ▼
    Web Terminal Gateway
       │
       ▼
    SPITFIRE Session Engine

It should not simply relay traffic to the public Telnet port.

## 20. Administrative Separation

Caller access and server administration should be conceptually separate.

SPITFIRE security level 255 may still represent the traditional Sysop inside the BBS.

Server-level administrative operations may require additional authorization.

This prevents a compromised caller credential from automatically granting control over the host system.

## 21. Implementation Language

The preferred architecture should favor memory-safe implementation for new network-facing components.

A candidate implementation stack is:

    Rust         Core server and protocol processing
    TypeScript   Browser interface
    HTML/CSS     Web presentation

C and C++ libraries may be used selectively when an established compatible library provides substantial benefit.

The project should avoid unnecessary language fragmentation.

## 22. Portability

Primary targets:

    Linux x86-64
    Linux ARM64
    macOS Intel
    macOS Apple Silicon
    Windows x86-64

The core architecture should avoid assumptions about:

- pointer size
- byte alignment
- path separators
- endianness
- Windows registry
- POSIX-only services

Legacy SPITFIRE formats may require explicit little-endian decoding because of their DOS heritage.

## 23. Guiding Rule

When architectural choices conflict, priority should generally be:

1. Preserve recognizable SPITFIRE behavior.
2. Protect historical data.
3. Maintain interoperability.
4. Keep configuration understandable.
5. Maintain portability.
6. Provide reasonable security.
7. Add modern convenience.
8. Optimize performance.

SPITFIRE is not intended to become an enterprise application framework.

It is intended to become a modern BBS.
