# SPITFIRE Legacy Compatibility Matrix

## 1. Purpose

This document tracks historical SPITFIRE components and defines the intended compatibility target for the modern implementation.

Compatibility classifications may change as reverse engineering and testing continue.

This matrix defines **how** each historical component should be supported. The
[Stock SPITFIRE 3.7 Parity Checklist](stock-spitfire-3.7-parity.md) separately
defines **which documented stock capabilities** are core, advanced, optional,
or modernized and tracks their implementation status.

## 2. Compatibility Levels

### Level A — Native Compatible

Original file may be used directly without conversion.

### Level B — Read/Write Compatible

The modern system understands and produces the historical format.

### Level C — Behavioral Compatible

The original behavior is reproduced using a modern implementation.

### Level D — Import Compatible

Historical information can be migrated but is not used as the primary live format.

### Level E — Legacy Runtime Compatible

Original executable operates through an emulator or compatibility environment.

### Level F — Historical Preservation Only

The component is documented and preserved but not necessarily executed.

## 3. Core Resources

| Component | Target | Notes |
|---|---|---|
| `SFMAIN.MNU` | A/B | Original main-menu format |
| `SFMSG.MNU` | A/B | Original message menu |
| `SFFILE.MNU` | A/B | Original file menu |
| `SFSYSOP.MNU` | A/B | Original Sysop menu |
| `SPITFIRE.HLP` | A/B | Record format substantially understood |
| `.BBS` displays | A | Plain/IBM display |
| `.CLR` displays | A | ANSI/color display |
| `.RIP` displays | A | Preserve where practical |
| Display macros | A/C | Original macros retained |
| Security-specific displays | C | Preserve lookup behavior |

## 4. Caller System

| Component | Target | Notes |
|---|---|---|
| Historical caller database | B/D | Exact 3.7 structure and import remain to be completed; native schema is separate |
| Caller security levels | A/C | Native threshold semantics implemented in Stock Core Increment 2 |
| Caller statistics | B/C | Calls and time implemented; message/file counters follow their owning increments |
| Last-call information | B/C | Native first/last call and reconnect persistence implemented |
| Historical password field | D | Import only if insecure |
| Modern password hashes | New | Default for modern accounts |
| Optional MFA | New | Not part of legacy format |

Historical plaintext or weak password representations should never force the modern server to retain insecure credential storage.
The implemented native model and deliberate compatibility boundaries are
specified in [Native Caller and Authentication Model](sfng-caller-authentication.md).

## 5. Message Base

| Component | Target | Notes |
|---|---|---|
| `SFMSGx.DAT` | B | Native message body storage |
| `SFMSGx.PTR` | B | Headers/pointers |
| `SFMSGx.IDX` | B | Index records |
| `SFMSGx.LMR` | B | Last-message-read data |
| `SFMCONF.DAT` | B | Conference definitions |
| Private messages | C | Preserve behavior |
| Threading | C | Preserve |
| Message numbering | C | Preserve |
| Deleted flag | C | Preserve |
| Received flag | C | Preserve |
| NetMail indicators | C | Preserve |

The original SPITFIRE message base should remain usable as a live backend if reliability can be established.

## 6. Alternate Message Bases

| System | Target |
|---|---|
| Synchronet SMB | Native backend |
| SQLite/internal | Native backend |
| Future formats | Pluggable |

Different message-base types should appear to callers through the normal SPITFIRE message interface.

## 7. QWK / LAKOTA

| Feature | Target |
|---|---|
| QWK packet generation | C |
| REP import | C |
| Offline message reading | C |
| Conference selection | C |
| Last-read tracking | C |
| NetMail behavior | C |
| Original LAKOTA executable | E |
| LAKOTA-style BBS interface | C |

The preferred implementation is native QWK support that behaves like the historical LAKOTA subsystem.

## 8. QWK Networking and DOVE-Net

| Feature | Target |
|---|---|
| Standard QWK networking | Native |
| Extended QWK headers | Native |
| DOVE-Net | Native |
| Synchronet interoperability | Native |

DOVE-Net support must not require the BBS itself to use Synchronet.

## 9. FidoNet

| Feature | Target |
|---|---|
| NetMail | Native |
| EchoMail | Native |
| FTN addressing | Native |
| BinkP | Native |
| Packet import/export | Native |
| External tosser support | Optional |
| Legacy modem mailers | E/Optional |

Historical SPITFIRE/FidoNet utilities may be supported as legacy programs where practical.

## 10. CircuitNet

| Component | Target |
|---|---|
| CircuitNet addresses | C |
| Conference identifiers | C |
| Hub/node model | C |
| Routing | C |
| Dossiers | C |
| Legacy `.CNP/.CND` files | B |
| Duplicate-history behavior | C |
| Original CircuitNet utilities | E/F |
| Historical insecure controls | F |
| Secure modern node transport | New |

The goal is to preserve CircuitNet as a network system without reproducing insecure trust assumptions unnecessarily.

## 11. Doors

| Component | Target |
|---|---|
| `DOOR.SYS` | B |
| `SFDOORS.DAT` | B |
| Door A–Z configuration | C |
| Native modern doors | New |
| DOS doors | E |
| DOS networking | Optional sandbox capability |

Original doors should operate whenever practical through an isolated DOS runtime.

## 12. Events

| Component | Target |
|---|---|
| Event A–L concepts | C |
| DOS `ERRORLEVEL` mechanism | E/C |
| Modern scheduled actions | New |
| Existing event utilities | E |

The historical terminology should remain even when internal scheduling becomes modern.

## 13. Networking and Modem Features

| Component | Target |
|---|---|
| FOSSIL drivers | F |
| Physical modem operation | Optional |
| Hayes command emulation | Optional |
| Serial ports | Optional |
| Telnet | Native |
| SSH | Native |
| WebSocket | Native |
| Local console | Native |

FOSSIL support is not required for the modern core.

A compatibility bridge may eventually be created if historical hardware operation becomes desirable.

## 14. DOS Utilities

Original utilities should be classified individually.

Possible outcomes:

### Reimplemented

A modern utility performs the same function.

### Supported Through DOS Runtime

Original utility remains usable.

### Replaced by Built-In Feature

Functionality becomes part of the modern server.

### Preservation Only

Utility is retained for historical study.

Examples include:

    SFHELP
    SFPCKMSG
    SFPCKUSR
    LAKOTA
    CircuitNet tools
    configuration utilities
    menu editors
    message maintenance tools

## 15. Distribution Compatibility

The project should preserve the historical notion of:

    Core system
    Utilities
    Documentation
    Examples

Modern release archives may mirror the flavor of the original distribution while adding modern packages.

Possible archival distribution:

    SFNGxx-1.ZIP
    SFNGxx-2.ZIP

Possible modern distribution:

    spitfire-linux-x64.tar.gz
    spitfire-linux-arm64.tar.gz
    spitfire-macos-universal.zip
    spitfire-windows-x64.zip

## 16. Compatibility Goal

The ideal historical migration experience is:

    Original SPITFIRE directory
               │
               ▼
         Modern SPITFIRE
               │
    ┌──────────┼───────────┐
    │          │           │
   Menus      Users      Messages
    │          │           │
 Displays    Stats       Networks
    │                      │
 Doors                   QWK/Fido
    │
    ▼
Board comes back online

Conversion should occur only where necessary.

## 17. Compatibility Rule

No historical format should be discarded merely because a cleaner replacement exists.

New formats may become recommended defaults while original formats remain available for preservation and interoperability.
