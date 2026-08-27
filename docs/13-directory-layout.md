# SPITFIRE Configuration and Directory Layout

## 1. Purpose

This document defines the proposed configuration model, directory structure, legacy-path handling, and installation philosophy for the modern SPITFIRE Bulletin Board System.

The system should remain recognizable to an experienced SPITFIRE Sysop while avoiding unnecessary dependence on DOS-era filesystem assumptions.

The design must support:

- fresh modern installations
- imported historical SPITFIRE installations
- portable installations
- system-wide installations
- local development
- preservation/testing environments

## 2. Design Principle

SPITFIRE historically organized its environment around logical working areas rather than a monolithic application directory.

That philosophy should remain.

The modern implementation should distinguish between:

- application binaries
- system configuration
- mutable BBS data
- message data
- display assets
- doors
- networking data
- temporary runtime data
- logs
- backups

The physical directory layout may vary by operating system, but the logical SPITFIRE paths should remain consistent.

## 3. Original Logical Paths

Historical SPITFIRE uses concepts including:

    SYSTEM
    WORK
    MESSAGE
    DISPLAY
    EXTERNAL

These names should remain part of the modern configuration model.

Additional logical paths may be added for modern features.

Proposed logical paths:

    SYSTEM
    WORK
    MESSAGE
    DISPLAY
    EXTERNAL
    DOORS
    NETWORK
    WEB
    LOG
    TEMP
    BACKUP

## 4. Portable Installation Layout

A portable installation should be capable of existing entirely inside one directory.

Example:

    spitfire/
    |
    +-- bin/
    |
    +-- system/
    |
    +-- work/
    |
    +-- messages/
    |
    +-- display/
    |
    +-- external/
    |
    +-- doors/
    |
    +-- network/
    |
    +-- web/
    |
    +-- logs/
    |
    +-- temp/
    |
    +-- backup/
    |
    +-- docs/
    |
    +-- spitfire.toml

This should be the easiest installation type for:

- hobbyists
- preservationists
- USB/removable installations
- testing
- retro-computing projects

## 5. Historical Compatibility Layout

The system should also understand a traditional SPITFIRE directory.

Example:

    C:\SPITFIRE\
        SPITFIRE.EXE
        SPITFIRE.OVR
        SPITFIRE.HLP

        SFMAIN.MNU
        SFMSG.MNU
        SFFILE.MNU
        SFSYSOP.MNU

        SFUSERS.DAT
        SFMCONF.DAT

        ...

A modern server may be launched against such a directory in compatibility mode.

Conceptually:

    spitfire --legacy /bbs/spitfire

The exact command-line syntax is not yet final.

## 6. Legacy Installation Detection

When opening a historical directory, SPITFIRE should inspect known files and determine:

    probable SPITFIRE version
    known resource formats
    available message bases
    caller database presence
    display assets
    menu files
    help files
    door configuration
    networking utilities
    unknown files

The original files should not be modified automatically.

The first operation should be analysis.

## 7. Import Modes

A historical installation may be opened using several modes.

### Read-Only Preservation

Original files remain untouched.

SPITFIRE interprets them for inspection and testing.

### Native Legacy

Original supported formats remain active files.

### Hybrid

Historical resources are used directly while selected subsystems use modern storage.

Example:

    Menus        Legacy
    Display      Legacy
    Help         Legacy
    Messages     SMB
    Callers      SQLite

### Migrated

Historical data is converted into modern storage.

The original installation remains preserved separately.

## 8. Configuration File

The native server uses a primary human-readable TOML configuration file.
Current format version 2 contains board identity, the node pool, the five stock
logical paths, the SQLite filename, caller policy, and a named repeated
transport list. Format 1 with the singleton `[node]` shape written by Increment
0 remains readable; new configurations use version 2 and `[nodes]`:

```toml
format_version = 2

[board]
name = "SPITFIRE NG Fixture Board"
sysop = "Fixture Sysop"

[nodes]
count = 4

[paths]
system = "system"
work = "work"
display = "display"
message = "message"
external = "external"

[storage]
database_file = "spitfire-ng.sqlite3"

[[transports]]
name = "telnet-primary"
enabled = true
type = "telnet"
listen = "127.0.0.1:2323"

[transports.terminal]
ansi = true
cp437 = true
width = 80
height = 25

[[transports]]
name = "raw-primary"
enabled = true
type = "raw"
listen = "127.0.0.1:2324"

[[transports]]
name = "rlogin-primary"
enabled = true
type = "rlogin"
listen = "127.0.0.1:2513"
```

Relative logical paths resolve from the configuration directory and reject
parent traversal. Explicit absolute logical paths are supported for
system-wide installations. The database file is one filename under WORK.
Runtime code accesses these through `LogicalPaths`, not raw configuration
paths.

Setup and fixture defaults bind only to localhost and use nonprivileged ports.
Network listeners may provide terminal defaults; Telnet can subsequently
negotiate terminal type and dimensions. Direct-serial and inbound-Hayes
entries use a device plus validated baud rate. Duplicate listeners/devices,
zero ports, impossible dimensions/baud rates, incompatible or unknown options,
and unsupported transport types fail closed. SSH configuration is recognized
but rejected until its maintained-library, host-key, and transport-auth policy
is implemented.

The `spitfire setup` wizard creates a complete startable board and `spitfire
config` edits implemented settings through the same validator. This file
remains understandable and editable without those tools. Static TOML,
persistent SQLite state, and transient node status are deliberately separate;
see [Setup and Configuration](sfng-setup-configuration.md).

Caller defaults and message-conference operational configuration are now
implemented. File-area settings will be added only with the file increment.
Unknown configuration keys and unsupported format versions fail closed.

## 9. Configuration Hierarchy

Configuration may exist at several levels.

Suggested order:

    Built-in defaults
          |
          v
    System configuration
          |
          v
    Legacy SPITFIRE configuration
          |
          v
    Local overrides
          |
          v
    Environment or command-line overrides

The exact precedence should be documented and predictable.

## 10. Modern Configuration and Legacy Files

The project should avoid forcing historical configuration data into modern syntax unnecessarily.

For example:

    SFMAIN.MNU

should remain an `.MNU` file.

Modern configuration should reference or augment it rather than translate it permanently into TOML.

Likewise:

    SPITFIRE.HLP

should remain usable directly.

## 11. Secrets

Secrets should not be stored in ordinary configuration files when avoidable.

Examples:

    TLS private keys
    CircuitNet private keys
    DOVE-Net credentials
    FidoNet session passwords
    SMTP credentials
    API tokens

Possible storage methods include:

    separate secrets file
    OS credential storage
    environment variables
    encrypted local store

A simple hobby installation should not require an elaborate external secrets manager.

## 12. File Permissions

SPITFIRE should create files with conservative permissions appropriate to the operating system.

Sensitive data may include:

    caller credentials
    network credentials
    private messages
    server keys
    backups

Public display resources do not require the same protection.

## 13. Case Sensitivity

Historical DOS filesystems were generally case-insensitive.

Modern Linux filesystems may be case-sensitive.

The compatibility layer should therefore treat known historical SPITFIRE filenames case-insensitively where practical.

For example:

    SFMAIN.MNU
    sfmain.mnu
    SfMain.Mnu

should be recognized as the same legacy resource.

The modern filesystem should not depend on this behavior for unrelated files.

## 14. Path Separators

Legacy configuration may contain DOS paths:

    C:\SPITFIRE\DISPLAY

Modern systems may use:

    /opt/spitfire/display

The path layer should normalize operating-system differences.

Legacy paths should not be interpreted through unsafe string substitution.

## 15. Drive Letters

Historical configuration may reference DOS drive letters.

Compatibility mode may provide mappings.

Example:

    C: -> /bbs/spitfire
    D: -> /bbs/files

A DOS door runtime may receive those mappings directly.

The native SPITFIRE core should use normalized host paths internally.

## 16. Distribution Resources

Default system resources may include:

    SFMAIN.MNU
    SFMSG.MNU
    SFFILE.MNU
    SFSYSOP.MNU
    SPITFIRE.HLP

along with default:

    welcome screens
    logoff screens
    bulletins
    menus
    documentation

Where licensing permits, original historical assets may remain separate preservation resources.

New distributions should clearly identify reconstructed or newly created assets.

## 17. Web Assets

Web files should live separately from caller-accessible BBS files.

Example:

    web/
        templates/
        static/
        themes/

A web server must never treat:

    work/
    messages/
    system/

as a public document root.

## 18. Temporary Runtime Area

The TEMP path may contain:

    active upload staging
    QWK extraction
    CircuitNet packet processing
    FidoNet packet processing
    door session directories
    temporary exports

Temporary data should be safely cleaned after use.

Startup recovery should detect abandoned temporary directories from crashed sessions.

## 19. Upload Staging

Uploads should initially enter a staging area.

Conceptually:

    incoming network data
             |
             v
          TEMP
             |
       validation
             |
             v
      final file area

This helps prevent incomplete or malformed uploads from appearing as valid files.

## 20. Network Directory

Network-specific data should be logically separated.

Example:

    network/
        qwk/
        dovenet/
        fido/
        circuitnet/

Each module may maintain:

    inbound
    outbound
    archive
    history
    config

The user-facing configuration should remain simpler than the internal organization.

## 21. Door Directory

Suggested structure:

    doors/
        lord/
        tradewars/
        bre/
        custom/

Persistent door files should remain separate from temporary node/session data.

## 22. Node Runtime Directories

Active nodes may receive temporary runtime directories.

Example:

    temp/
        node1/
        node2/
        node3/

These may contain:

    DOOR.SYS
    SFDOORS.DAT
    transient uploads
    session files

They should not contain master caller or message databases.

## 23. Logs

Logs may be separated by purpose.

Example:

    logs/
        system.log
        callers.log
        security.log
        networks.log
        doors.log
        web.log

Sysops should not be forced to manage numerous logs manually.

A combined view should be available through the Sysop interface.

## 24. Log Rotation

Public Internet installations may generate substantial logs.

The server should support:

    maximum file size
    number of retained logs
    maximum age

Defaults should be modest.

A hobby system should not wake up six months later with 80 GB of logs.

## 25. Backups

The BACKUP path should contain SPITFIRE-created backups.

Backup operations may include:

    configuration
    callers
    message bases
    menus
    display files
    network configuration
    web customization

Large file libraries may be optionally excluded.

## 26. Backup Portability

A full configuration/data backup should be restorable on another supported operating system wherever practical.

OS-specific paths should be normalized or translated during restoration.

## 27. Configuration Validation

Before startup, SPITFIRE should validate:

    required directories
    writable paths
    conflicting ports
    malformed configuration
    inaccessible message bases
    duplicate node definitions
    invalid security ranges

Warnings and errors should identify the actual problem in understandable terms.

## 28. Interactive Setup

A first-run setup utility may ask:

    BBS Name
    Sysop Name
    Number of Nodes
    Enable Telnet?
    Enable SSH?
    Enable Web?
    Create Initial Sysop Account?

The result should be ordinary configuration files.

The setup wizard should not create an opaque configuration database.

## 29. Advanced Editing

Experienced Sysops should be able to edit configuration manually.

The web interface and local tools should preserve comments and unknown configuration values where practical.

## 30. Guiding Principle

A Sysop should be able to understand where SPITFIRE keeps its things.

The directory tree should resemble a well-organized BBS installation, not the internal anatomy of a cloud platform.
