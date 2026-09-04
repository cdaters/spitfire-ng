# SPITFIRE NG Sysop Reference Manual

<!-- help-topic: sysop.manual -->

> **Applies to:** Current SPITFIRE NG source (`main`, schema 19)
>
> **Latest downloadable release:** SPITFIRE NG 0.1.0 Development Preview
>
> The downloadable Development Preview is older than the current source and
> may not include every feature described here. Pages call out a difference
> when it matters.

SPITFIRE NG is a modern, cross-platform reimplementation of Buffalo Creek
Software's SPITFIRE Bulletin Board System. It keeps the terminology and caller
experience that made SPITFIRE recognizable while replacing DOS-era hardware
limits with maintainable native software.

This manual is for the person running the board—the Sysop. It explains how to
install SPITFIRE NG, create a board, configure caller access, operate message
conferences and file areas, choose presentation resources, protect private
data, make backups, and recover when something goes wrong.

You do not need to understand Rust or SQLite to use this manual.

## Start here

Want to get a board running first and explore the details afterward? Follow
the [Quick Start](quick-start.md). It uses current source, one loopback-only
SSH listener, the setup-created Sysop account, the starter message and file
areas, and the supported cold-backup command.

For a fuller first-board walkthrough, continue with [Getting
Started](../operator/getting-started.md). If you are upgrading or using the
older downloadable archive, begin with
[Installation](../operator/installation.md) and
[Upgrades](../operator/upgrades.md).

## How to use this manual

The chapters are arranged around normal Sysop work:

- start with installation and setup;
- learn the caller-facing parts of the board;
- configure presentation, nodes, and transports;
- establish routine operations, security, and backup; and
- use troubleshooting and reference material as needed.

The [Caller Guide](../caller-guide/README.md) explains the board from a
caller's point of view. You may give that guide to callers without exposing
operator-only procedures. The [Technical Reference](../technical/README.md)
contains database, transaction, protocol, concurrency, security, and
compatibility detail for advanced Sysops and developers.

## Contents

### Welcome, installation, and first setup

- **Quick Start:** [Get a local board running safely](quick-start.md).
- **Installation:** [Build current source or install the Development
  Preview](../operator/installation.md).
- **macOS first run:** [Verify and open the unsigned Development Preview](../operator/macos-first-run.md).
- **Initial setup:** [Create and review your first board](../operator/getting-started.md#2-create-the-first-board).
- **Configuration:** [Use the supported configuration menu](../operator/configuration.md).
- **Directory layout:** [Understand board-owned files and directories](../13-directory-layout.md).

### Running the board

- **Starting and stopping:** [Run the board in the foreground and shut it down
  cleanly](../operator/getting-started.md#4-start-inspect-stop-and-restart).
- **Sysop operations:** [Status, the operator console, and routine
  operation](../operator/sysop-guide.md).
- **Board activity and statistics:** [Understand recent activity, live node
  status, notifications, maintenance, privacy, retention, and protected
  read-only operator attachment](board-activity.md).
- **Nodes and multinode operation:** [Node ownership and simultaneous caller
  sessions](../sfng-multinode-runtime.md).
- **Caller accounts:** [Creation, access changes, privacy, and public caller
  information](../operator/caller-management.md).
- **Security and access:** [Authentication and caller-access rules](../sfng-caller-authentication.md).

### Messages and conferences

- **Message conferences:** [Configure conferences](../operator/messages.md#configure-conferences).
- **Messages:** [Post, read, reply, and use Your Messages](../operator/messages.md).
- **Caller and Sysop interaction:** [Comments, pages, and chat](../sfng-caller-sysop-interaction.md).

### Files and transfers

- **File areas:** [Configure, list, search, inspect, and upload files](../operator/files.md).
- **Uploads and downloads:** [Follow the caller transfer sequence](../operator/transfers.md).
- **Transfer protocols:** [ASCII, XMODEM variants, YMODEM variants, ZMODEM,
  and TeLink](../operator/transfers.md#stock-protocol-menu).
- **Tagging and batch downloads:** [Queue files and choose a batch-capable
  protocol](../sfng-file-transfers.md).
- **Ratios, daily limits, no-charge areas, Preview areas, and upload credit:**
  [Transfer policy and accounting](../sfng-file-transfers.md).
- **Extended and read-only storage:** [Logical roots, unavailable media, and
  safe staging](../sfng-file-system.md).

The transfer-policy and extended-storage links currently lead to the detailed
Technical Reference. Human-focused chapters for those subjects are the next
manual migration targets; the features themselves are current and verified.

### Menus, displays, and presentation

- **Menus and commands:** [Resource-driven menu behavior](../operator/getting-started.md#7-make-the-first-calls).
- **Custom display files:** [Create board-owned BBS and CLR screens](../operator/custom-display-screens.md).
- **Presentation profiles:** [Select and diagnose presentation packages](../presentation-profiles.md).
- **Classic presentation:** [Enable the Classic SPITFIRE-inspired profile](../operator/classic-presentation.md).
- **Language packages:** [Install, select, validate, and recover localization](../operator/localization.md).

Optional future presentation methods, including Sixel, will belong in this
part of the manual after they are implemented and verified. They are not
current SPITFIRE NG capabilities.

### Connections and terminal clients

- **Terminal clients:** [SyncTERM, Qodem, OpenSSH, RAW, and RLogin guidance](../operator/terminal-clients.md).
- **Telnet, RAW, and RLogin:** [Transport boundaries and plaintext warnings](../08-network-architecture.md).
- **SSH caller access:** [Encrypted caller authentication and host-key
  handling](../sfng-secure-ssh-transport.md).

SSH provides access to the BBS only. It does not provide an operating-system
shell, command execution, SCP, SFTP, or forwarding.

### Maintenance, security, and recovery

- **Backup and restore:** [Create, protect, verify, and restore a cold backup](../operator/backup-restore.md).
- **Upgrades:** [Protect a board before changing source or binaries](../operator/upgrades.md).
- **Security and privacy:** [Deployment and data-protection principles](../03-security-philosophy.md).
- **Troubleshooting:** [Common installation, listener, terminal, and recovery
  problems](../operator/troubleshooting.md).
- **Support:** [What to collect and how to report a problem safely](../operator/support.md).

### Compatibility and reference

- **Legacy SPITFIRE compatibility:** [What SPITFIRE NG preserves and what it
  modernizes](../02-compatibility-principles.md).
- **Current compatibility matrix:** [Implemented and planned boundaries](../05-compatibility-matrix.md).
- **Historical background:** [SPITFIRE history and preservation context](../HISTORICAL-SPITFIRE.md).
- **Technical Reference:** [Implementation-level documentation index](../technical/README.md).
- **Documentation index:** [All project documentation](../README.md).

Reference tables and a glossary will be added as real manual content is
consolidated. Until then, the linked operator and technical documents remain
the canonical references; no empty chapter is presented as finished.

## A note about board-specific choices

Every SPITFIRE NG board can choose its own name, access policy, conferences,
file areas, menus, presentation, listener addresses, and security levels.
Examples in this manual show safe starting points, not values every board must
use. When a caller-facing command is unavailable, check the caller's access
and the board's menu configuration before assuming the software is broken.

## Documentation and help

Stable topic names in this manual are designed for future use by local help,
the website, and operator tools. The repository Markdown remains the editable
source; website and downloadable versions should be generated or synchronized
from it rather than maintained as separate copies.
