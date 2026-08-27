# SPITFIRE Modernization Project Charter

## 1. Project Vision

The SPITFIRE Modernization Project exists to preserve, document, and extend the Buffalo Creek Software SPITFIRE Bulletin Board System into a modern, cross-platform BBS platform.

The goal is not to create a generic BBS inspired by SPITFIRE, nor to reproduce SPITFIRE merely as a visual theme.

The goal is to build a system that is unmistakably SPITFIRE.

Wherever practical, the modern implementation should preserve the original terminology, concepts, workflows, file formats, menus, message structures, help system, display files, networking concepts, door interfaces, configuration philosophy, and overall character of the original Buffalo Creek Software product.

At the same time, the new system should remove dependencies that no longer make sense on modern systems, including DOS, FOSSIL drivers, serial ports, modem emulation, and artificial serial-to-TCP translation.

The resulting software should feel like the SPITFIRE that might have evolved naturally had development continued into the modern Internet era.

## 2. Primary Goals

The project will strive to provide:

- Native operation on modern operating systems.
- Support for Linux, macOS, and Windows wherever practical.
- Architecture that avoids unnecessary dependence on a specific CPU architecture or operating system.
- Native TCP/IP networking.
- Telnet connectivity for traditional BBS clients.
- SSH connectivity where practical.
- Browser-based access through an embedded terminal.
- A local Sysop interface.
- A web-accessible administration and information layer.
- Compatibility with original SPITFIRE menus, help files, display files, and data wherever technically feasible.
- Compatibility with traditional BBS message networking.
- Support for QWK and QWK networking.
- Support for FidoNet.
- Support for Synchronet SMB message bases.
- DOVE-Net interoperability.
- Preservation and possible modernization of CircuitNet.
- Legacy DOS door support through an isolated compatibility environment.
- Modern native doors or extensions that do not require DOS emulation.
- Import and migration tools for historical SPITFIRE installations.

## 3. Preservation Principle

Original SPITFIRE concepts should be preserved whenever modernization does not require abandoning them.

A new feature should not replace an established SPITFIRE feature merely because a newer implementation technique exists.

For example:

- `SFMAIN.MNU` should remain meaningful.
- `SPITFIRE.HLP` should remain usable.
- `.BBS`, `.CLR`, and `.RIP` display files should remain supported.
- Caller security levels should remain recognizable.
- Message conferences should remain message conferences.
- File areas should remain file areas.
- Doors should remain doors.
- Events should remain events.
- Nodes should remain nodes.
- The Sysop should still be called the Sysop.

Modern internals may change substantially while the SPITFIRE-facing model remains familiar.

## 4. Compatibility Principle

Compatibility will be pursued at several levels:

### Native Compatibility

Original files are usable without modification.

Examples may include:

- `.MNU`
- `.HLP`
- `.BBS`
- `.CLR`
- `.RIP`

### Format Compatibility

The new software can read and write the original SPITFIRE format.

Possible examples include:

- caller databases
- message pointer files
- message indexes
- message bodies
- last-message-read information
- door drop files

### Behavioral Compatibility

The implementation reproduces the historical behavior even when the internal implementation is new.

### Import Compatibility

Historical data is imported into a modern representation when retaining the original format is impractical.

### Network Compatibility

Historical network concepts and protocols remain interoperable where practical.

### Legacy Runtime Compatibility

Original DOS doors and utilities may execute inside an isolated DOS compatibility environment.

## 5. Modernization Principle

Modernization should remove obsolete dependencies without erasing the historical behavior they enabled.

For example:

Historical connection path:

    TCP/IP
      ↓
    Serial bridge
      ↓
    Virtual COM port
      ↓
    FOSSIL driver
      ↓
    SPITFIRE

Modern connection path:

    TCP/IP
      ↓
    SPITFIRE

Traditional Telnet clients should continue to work, but the server should understand Telnet directly.

Likewise, modern FidoNet support should not require pretending that an Internet connection is a modem connection.

## 6. User Experience

The primary audience includes:

- former SPITFIRE Sysops
- users of historical BBS systems
- retro-computing enthusiasts
- preservationists
- curious newcomers
- developers interested in classic online systems

The software should therefore favor understandable configuration and familiar BBS terminology over enterprise-style complexity.

A hobbyist should be able to install SPITFIRE, configure a board, and begin experimenting without becoming a systems-security specialist.

Advanced features should be available without becoming prerequisites for basic operation.

## 7. Security Philosophy

Security should protect users and systems without making a hobby BBS unpleasant to operate.

The project will favor:

- safe defaults
- sensible authentication
- protection of personally identifiable information
- separation of administrative privileges from normal caller privileges
- isolation of untrusted legacy software
- modern encryption where appropriate
- careful parsing of externally supplied files and network traffic
- strong but practical password requirements
- optional stronger authentication for Sysops
- straightforward configuration

Historical protocols such as Telnet may remain supported even when they are not considered secure by modern standards.

Where appropriate, the software should clearly identify the tradeoff rather than remove the functionality.

Security controls should be proportional to the risk.

## 8. Open Architecture

Major subsystems should be separable so that future developers can replace or extend them independently.

Expected major subsystems include:

- BBS session engine
- terminal handling
- caller database
- security and authentication
- menu engine
- message system
- file system
- event scheduler
- door engine
- networking
- QWK networking
- FidoNet
- CircuitNet
- web services
- legacy compatibility
- administration
- storage backends

## 9. Historical Integrity

The project should clearly distinguish between:

- original Buffalo Creek Software behavior
- reconstructed behavior
- compatibility behavior
- newly introduced functionality

When historical implementation details are uncertain, documentation should say so.

The project should avoid presenting reconstructed assumptions as historical fact.

## 10. Long-Term Objective

The ultimate success criterion is simple:

A former SPITFIRE Sysop should be able to sit down in front of the modern system, recognize it immediately, reuse as much of an old installation as possible, and say:

> This is SPITFIRE.

At the same time, a new user should be able to run it on a modern computer without needing DOS, a modem, a serial port, or thirty years of BBS archaeology.
