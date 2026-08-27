# SPITFIRE Doors and Legacy Runtime Architecture

## 1. Purpose

This document defines how modern SPITFIRE should support traditional BBS doors and other historical external programs without requiring the entire BBS server to operate in a DOS environment.

Doors are an essential part of traditional BBS culture and should remain a first-class SPITFIRE feature.

## 2. Design Principle

Modern SPITFIRE should distinguish between:

    BBS Runtime

and:

    Door Runtime

The SPITFIRE server should remain native to the host operating system.

A DOS door may run inside an appropriate compatibility environment.

Conceptually:

    Native SPITFIRE
          |
          v
      Door Bridge
          |
          v
     DOS Runtime
          |
          v
       DOS Door

The legacy application is contained.

The BBS is not.

## 3. Door Types

SPITFIRE should support several door runtime types.

### Native Door

A modern executable compiled for the host operating system.

### DOS Door

A historical DOS executable run through a compatibility environment.

### Script Door

A script launched through a configured runtime.

Possible examples:

    Python
    Lua
    JavaScript

### Internal Door

A module implemented directly inside SPITFIRE but presented as a traditional door.

### Remote Door

A future possibility where a door session is provided by another service.

## 4. Historical Door Identity

Traditional SPITFIRE Door A through Door Z behavior should remain available.

For example:

    Door A    TradeWars
    Door B    LORD
    Door C    BRE
    Door D    Usurper

Historical menu identifiers should continue to launch the corresponding configured door.

## 5. Door Configuration

A door definition may include:

    ID
    Name
    Description

    Door Letter

    Required Security Level

    Runtime Type

    Command
    Arguments
    Working Directory

    Drop File Type

    Time Limit

    Allow Network

    File Access

    Environment Variables

    Cleanup Policy

## 6. Drop Files

SPITFIRE should generate historical drop files where practical.

Primary targets include:

    SFDOORS.DAT
    DOOR.SYS

Additional formats may eventually include:

    DORINFOx.DEF
    CALLINFO.BBS
    CHAIN.TXT
    PCBOARD.SYS

Support for widely used door formats may substantially increase compatibility.

## 7. `SFDOORS.DAT`

The original SPITFIRE-specific door file should be documented and reproduced accurately.

Fields should be determined from:

- Buffalo Creek documentation
- surviving developer records
- actual door utilities
- controlled comparisons with historical SPITFIRE

Unknown fields should be preserved in the specification.

## 8. `DOOR.SYS`

SPITFIRE should generate a broadly compatible `DOOR.SYS`.

Historical quirks may require configurable compatibility profiles for individual doors.

## 9. Session Bridge

The door runtime must receive the caller's terminal stream.

Conceptually:

    Caller
      |
      v
    SPITFIRE Session
      |
      v
    Door Bridge
      |
      v
    Door Process

Input and output should return through the same caller connection.

A Telnet caller launching a DOS door should remain in the same session.

A browser caller should also be able to use that door through the same mechanism.

## 10. DOS Runtime

Possible DOS runtimes may include:

    DOSBox-X
    DOSBox Staging
    DOSEMU2
    virtual machine
    another compatible emulator

No single emulator should be permanently embedded into the core architecture.

The door layer should invoke a configured runtime adapter.

## 11. Host Platform Differences

Door execution may vary by operating system.

Example:

### Linux x86-64

DOSEMU2 or DOSBox may be available.

### Linux ARM64

DOSBox or CPU emulation may be required.

### macOS Apple Silicon

CPU emulation may be required.

### Windows x86-64

DOSBox or another runtime may be used.

SPITFIRE should hide these differences behind runtime adapters.

## 12. Door Sandbox

Legacy doors should execute with limited access.

A per-session environment may contain:

    /door-session/
        DOOR.SYS
        SFDOORS.DAT
        door files
        temporary files

The DOS runtime should not automatically receive access to:

    caller database
    message database
    network credentials
    web configuration
    SPITFIRE executable files
    SSH keys
    TLS keys
    arbitrary host filesystem

## 13. Shared Door Data

Some doors require persistent game data shared between sessions.

The configuration should explicitly identify directories that need persistent access.

Example:

    Persistent:
        /doors/lord/data

    Session:
        /runtime/node3

This allows multi-user games to retain state without exposing unrelated server files.

## 14. Network Access

Some modern or historical doors may need network access.

Network permission should therefore be configurable.

Possible settings:

    none
    outbound only
    unrestricted
    custom

The default for an unknown DOS door should be conservative but not prohibit legitimate configuration.

## 15. File Permissions

Door processes should run under limited privileges.

A door should not require administrator or root access.

On platforms that support it, doors may execute using:

- separate OS users
- filesystem namespaces
- containers
- sandbox profiles

These are implementation options rather than requirements for every installation.

## 16. Simple Installation Mode

Door isolation must not make SPITFIRE impossible for a hobbyist to configure.

A simple configuration should work with reasonable defaults.

Example:

    Runtime: DOSBox
    Program: C:\LORD\START.BAT
    Drop File: DOOR.SYS
    Persistent Directory: ./doors/lord

Advanced sandbox settings may remain optional.

## 17. Node Awareness

Each door session should know the SPITFIRE node number.

Example:

    Node 2 launches LORD

The server may create:

    runtime/node2/

This permits multiple callers to use multi-node-aware doors simultaneously.

## 18. Door Exit

When the door terminates:

1. the server captures the exit status
2. updated drop-file values are read
3. caller time or statistics are updated
4. temporary files are removed
5. the caller returns to SPITFIRE

The caller connection should remain alive throughout.

## 19. Carrier Detection

Traditional doors often expected modem carrier-detect behavior.

The compatibility layer may emulate historical signals using the state of the network connection.

Example:

    TCP connection active
            =
    carrier present

If the caller disconnects:

    carrier lost

The door runtime can then be notified appropriately.

## 20. FOSSIL Compatibility

SPITFIRE itself should not require a FOSSIL driver.

A particularly old door that requires FOSSIL services may run inside a DOS environment containing an appropriate emulated or software FOSSIL implementation.

This dependency remains inside the compatibility environment.

## 21. Virtual Serial Compatibility

Some doors may insist on communicating through a serial interface.

A future compatibility adapter may expose the caller stream as a virtual serial device inside the DOS runtime.

This should be used only where necessary.

The modern SPITFIRE core remains socket-native.

## 22. Native Door API

New doors should not need DOS drop files.

A modern door API may expose:

    Caller ID
    Caller Name
    Security Level
    Remaining Time
    Node
    Terminal Type

and services such as:

    send text
    receive input
    update statistics
    log activity
    return to BBS

The API should remain small and stable.

## 23. External Door Protocol

Modern native doors may communicate with SPITFIRE using:

- stdin/stdout
- local IPC
- structured messages
- a dedicated local socket

Network-exposed control APIs should not be required.

## 24. Door SDK

The project may eventually provide an SDK for new SPITFIRE doors.

Possible languages:

    Rust
    C++
    C
    Python
    JavaScript

The SDK should let developers write modern doors while retaining the traditional BBS interaction model.

## 25. Door Web Integration

The browser terminal should not need special knowledge of DOS doors.

From the browser's perspective:

    SPITFIRE output
        |
        v
    Door output
        |
        v
    SPITFIRE output

is simply one continuous terminal session.

## 26. Door Administration

The Sysop should be able to manage doors through:

### Terminal

Traditional Sysop menu.

### Local Console

Modern management interface.

### Web

Door configuration and status.

Possible information:

    Door
    Runtime
    Active Nodes
    Last Launch
    Exit Status
    Errors

## 27. Door Logging

Useful log entries include:

    caller
    node
    door
    launch time
    exit time
    exit code
    abnormal termination

Door output itself should not necessarily be logged unless explicitly enabled.

## 28. Hung Doors

The system should detect doors that exceed configured limits.

The Sysop may configure:

    warning
    maximum runtime
    idle timeout
    forced termination

Traditional game doors may legitimately run for extended periods, so limits should remain configurable.

## 29. Door Crashes

A crashed door should not crash SPITFIRE.

The caller should receive an appropriate message and return to the BBS where possible.

Example:

    The external program terminated unexpectedly.

    Returning to SPITFIRE...

## 30. Legacy Utilities

The same runtime architecture may support old SPITFIRE maintenance utilities.

Examples:

    message packers
    user packers
    menu tools
    historical editors
    networking utilities

These may execute manually or through Event definitions.

## 31. Trusted Versus Untrusted Utilities

A Sysop should be able to designate a legacy program as trusted when additional filesystem access is genuinely required.

This should be an explicit configuration choice.

Example:

    Trust Level:
        Restricted
        Legacy Compatible
        Custom

This provides flexibility without making unrestricted access the default.

## 32. Historical Testing

Compatibility testing should eventually include popular doors such as:

    LORD
    TradeWars 2002
    BRE
    Usurper
    Legend of the Red Dragon II
    Global War
    Operation Overkill II

The project should also test SPITFIRE-specific doors and utilities.

## 33. Preservation Principle

If an original door cannot run natively on a modern host, the answer should not be:

> SPITFIRE no longer supports that door.

The preferred answer is:

> SPITFIRE knows how to provide the historical environment that door expects.

## 34. Guiding Principle

Legacy DOS software may remain in 1994.

SPITFIRE itself does not have to.
