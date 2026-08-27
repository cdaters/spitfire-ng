# SPITFIRE Nodes, Events, and System Services

## 1. Purpose

This document defines the modern implementation of SPITFIRE nodes, scheduled events, background services, local console behavior, and system lifecycle.

These features should preserve the recognizable operational model of classic SPITFIRE while taking advantage of modern multitasking operating systems.

## 2. Historical Context

Classic SPITFIRE operated in an environment where:

- individual nodes might run separate processes
- DOS batch files controlled external behavior
- ERRORLEVEL values triggered events and doors
- external utilities performed maintenance
- communications depended on modems and FOSSIL drivers

Modern operating systems provide:

- native multitasking
- threads or asynchronous tasks
- TCP/IP
- service supervision
- filesystem notifications
- process isolation

Modern SPITFIRE should use these facilities without discarding the original concepts.

The preserved 3.7 manual specifically documents node numbers beginning at 1,
configuration through a current node number plus total-node count, and support
up to 255 nodes. Its multinode arrangement used separate DOS copies; shared
WORK/MESSAGE/DISPLAY paths; per-node SYSTEM/EXTERNAL paths; and
`SFWHOSON.DAT` activity state. These are confirmed historical facts, not a
requirement to recreate the DOS process/file-locking mechanism.

## 3. Node Concept

A node represents an active SPITFIRE caller/session channel.

Nodes remain a visible and meaningful part of the BBS.

Possible states:

    Available
    Connecting
    Logging In
    Online
    In Messages
    In Files
    In Door
    Chatting
    Logging Off
    Maintenance
    Disabled

## 4. Node Numbers

Nodes should receive stable configured identifiers.

Example:

    Node 1
    Node 2
    Node 3
    Node 4

The server may support dozens or hundreds of nodes.

Traditional Sysops may configure a smaller fixed number.

## 5. Reserved Local Node

A system may optionally reserve Node 1 for local Sysop use.

Example:

    Node 1    Local
    Node 2    Telnet
    Node 3    Telnet
    Node 4    Web

This should be configurable rather than mandatory.

Increment 4 does not reserve a local node. Any enabled transport acquires the
next available configured node. A future reservation/eligibility policy must
remain optional and must not equate a protocol with a node number.

## 6. Dynamic Nodes

Modern SPITFIRE may optionally support dynamic node allocation.

For example:

    configured maximum = 50

Node numbers are allocated as callers connect.

This permits modern scalability while retaining node identity during the session.

## 7. Node Information

Each active node should expose information such as:

    Node Number
    Caller
    Security Level
    Connection Type
    Remote Address
    Terminal Type
    Current Menu
    Current Conference
    Door
    Login Time
    Idle Time
    Remaining Time

Sensitive information should not be displayed to ordinary callers unless appropriate.

Increment 4 implements stable positive-`u32` node IDs, enabled/disabled
definitions, optional descriptions, and transient waiting/connecting/login/
online/disconnecting state with caller, transport, and connection-time fields.
The configured safety bound is 4,096 nodes rather than the historical 255.
See [SPITFIRE NG Multinode Runtime](sfng-multinode-runtime.md).

## 8. Who's Online

The classic caller-facing node list should remain available.

Example:

    Node  Caller       Activity
    ----  -----------  ----------------
      1   Alex         Sysop
      2   RetroRob     Reading Messages
      3   Z80Wizard    LORD
      4   Available

The appearance should remain customizable through SPITFIRE display resources.

## 9. Inter-Node Communication

Modern SPITFIRE should support node-to-node functionality such as:

    page Sysop
    Sysop chat
    caller-to-caller messages
    optional node chat

Historical behavior should be reproduced where documented.

New features should remain optional.

## 10. Session Isolation

A fault in one caller session should not terminate unrelated sessions.

Session-specific state should remain independent.

A malformed terminal sequence from Node 7 should not crash Node 1 through Node 6.

## 11. Event Concept

Historical events such as:

    Event A
    Event B
    ...
    Event L

should remain recognizable.

Events may invoke:

- maintenance
- networking
- backup
- external utilities
- scripts
- announcements
- scheduled shutdown/restart
- file processing

## 12. Event Configuration

A modern event may contain:

    Event ID
    Name
    Enabled
    Schedule
    Command Type
    Command
    Timeout
    Run If Nodes Active
    Retry Policy
    Log Level

Example:

    Event A
    Nightly Maintenance
    03:00 daily

## 13. Historical Event Mapping

Original SPITFIRE Event A-L configuration should be imported where practical.

Legacy behavior may map to modern event definitions.

Example:

    ERRORLEVEL 22
        ->
    Event A

The modern system should not require DOS `ERRORLEVEL` processing internally.

## 14. Event Types

Possible event types include:

### Internal

Runs built-in SPITFIRE maintenance.

### Native Command

Runs a host executable.

### Script

Runs a supported script.

### Legacy DOS

Runs a historical utility inside the legacy runtime.

### Network

Triggers a configured message-network action.

## 15. Internal Maintenance Events

Examples:

    Pack Message Bases
    Pack Caller Database
    Rebuild Indexes
    Clean Temporary Files
    Rotate Logs
    Purge Old Messages
    Create Backup

Historical terminology should be retained where sensible.

## 16. Networking Events

Examples:

    Poll DOVE-Net
    Poll FidoNet
    Process QWK Network
    Exchange CircuitNet
    Import Network Mail

Networks may also poll independently when configured.

Events provide explicit Sysop control.

## 17. Event Execution With Active Callers

Events should define whether they can run while callers are online.

Possible policies:

    Always
    Only When No Callers
    Wait Until Clear
    Force Exclusive Maintenance

Dangerous maintenance should not modify an active legacy data file concurrently unless the backend supports it safely.

## 18. Maintenance Mode

SPITFIRE should support a maintenance mode.

Possible behavior:

    Existing callers may finish
    New connections receive a maintenance notice
    Scheduled maintenance runs
    Normal service resumes

A Sysop may also choose immediate maintenance when necessary.

## 19. Event History

SPITFIRE should retain event history.

Example:

    Event A
    Last Run: 2026-08-19 03:00
    Result: Success
    Duration: 4.2 seconds

Failures should contain useful diagnostic information.

## 20. Failure Behavior

A failed event should not normally terminate SPITFIRE.

The system should:

    log failure
    preserve output
    optionally retry
    notify Sysop where configured

Repeated failure may disable an event automatically only if configured.

## 21. External Commands

External commands should run with controlled environment variables.

Possible variables:

    SF_HOME
    SF_SYSTEM
    SF_WORK
    SF_MESSAGE
    SF_DISPLAY
    SF_NODE
    SF_EVENT

Legacy utilities may receive DOS-style equivalents through the compatibility environment.

## 22. System Services

Modern SPITFIRE consists of several logical services.

Potential services:

    Core
    Telnet
    SSH
    Web
    WebSocket Terminal
    Scheduler
    Network Pollers
    Door Runtime Manager

These may initially run inside a single executable while maintaining internal boundaries.

They need not become numerous separate operating-system services unless that provides practical benefit.

## 23. Single-Process Default

The default hobby installation should favor simplicity.

Example:

    spitfire

starts:

    BBS core
    configured terminal listeners
    web server
    scheduler
    networking

from one program.

Advanced deployments may split selected components later.

## 24. Service Management

The application should support normal host service managers.

Examples:

### Linux

    systemd

### macOS

    launchd

### Windows

    Windows Service

Running SPITFIRE interactively should remain fully supported.

## 25. Local Console

A local Sysop console should provide immediate operational visibility.

Possible display:

    SPITFIRE

    System: The Dragon's Den
    Uptime: 4 days 07:13
    Nodes: 3 / 10

    Node 2   RetroRob   Messages
    Node 3   PixelMage  Door: LORD

    FidoNet    Last poll 18:30  OK
    DOVE-Net   Last poll 18:45  OK
    CircuitNet Last poll 18:00  OK

The exact interface may begin as text-based and evolve later.

## 26. Local Console Commands

Potential commands:

    nodes
    callers
    chat
    kick
    events
    networks
    maintenance
    reload
    shutdown

The local console should not become dependent on the web interface.

## 27. Graceful Shutdown

Shutdown should:

1. stop accepting new connections
2. optionally notify callers
3. allow a configurable grace period
4. terminate active doors safely
5. flush message/storage operations
6. stop network activity
7. close logs
8. exit

Forced shutdown should remain available.

## 28. Restart

A controlled restart should be possible without corrupting state.

Later implementations may support zero-downtime replacement, but this is not an initial requirement.

## 29. Configuration Reload

Selected configuration may be reloadable without restart.

Examples:

    menus
    display files
    events
    file areas
    message conferences

Changes to fundamental listener configuration may require restart.

The behavior should be explicit rather than unpredictable.

## 30. Clock and Time Handling

Modern SPITFIRE should internally use robust full-date representations.

It must not reproduce the original post-2024 date limitation.

Historical two-digit dates should be interpreted only at compatibility boundaries.

Internally, use:

    full year
    explicit timezone where needed
    monotonic timers for session duration

## 31. Time Zones

A BBS should have a configured local timezone.

Caller-visible times may default to BBS local time.

Modern web clients may optionally request display in caller-local time.

Network protocols should retain whatever timezone semantics their specification requires.

## 32. Idle Timeout

Each connection type may define an idle timeout.

Example:

    Telnet    20 minutes
    SSH       30 minutes
    Web       30 minutes
    Local     Unlimited

The Sysop may change these values.

Doors may define their own policies.

## 33. Time Limits

Traditional daily and per-call limits should remain part of SPITFIRE.

Examples:

    Daily Time Limit
    Log On Time Limit
    Maximum Caller Daily Access

Modern session management should enforce these values consistently regardless of transport.

## 34. Caller Disconnects

Unexpected disconnect should:

    mark session ended
    update statistics
    terminate or signal active door
    release node
    clean temporary files

The system should distinguish clean logoff from carrier/connection loss where useful.

## 35. Crash Recovery

On startup, SPITFIRE should detect stale runtime state.

Examples:

    abandoned node directories
    stale session markers
    partially written packet files
    interrupted maintenance

Recovery should avoid silently deleting potentially useful evidence.

## 36. Health Information

The server should expose basic health state internally.

Examples:

    database reachable
    message bases valid
    listeners active
    disk space
    network queues
    scheduler running

The web dashboard and local console may display this information.

## 37. Resource Limits

Configurable limits may include:

    maximum sessions
    maximum upload size
    maximum packet size
    maximum door runtime
    network queue limits

Defaults should be appropriate to hobby operation rather than hyperscale infrastructure.

## 38. Guiding Principle

The classic SPITFIRE Sysop should still think in terms of:

    nodes
    callers
    events
    doors
    maintenance

The implementation underneath may be thoroughly modern.

The vocabulary does not need to change merely because DOS is gone.
