# SPITFIRE NG Operator Guide

These guides are for Sysops running the SPITFIRE NG Development Preview. The
accepted Apple Silicon package does not require a source checkout.

The latest downloadable binary remains 0.1.0. Sections explicitly labeled
current source may describe post-0.1.0 behavior on `main`, including schema-13
caller identity and SSH caller access; those features are not in the published
0.1.0 archive.

## Start a board

1. [Verify and install the package](development-preview-package.md)
2. [Handle macOS first-run security](macos-first-run.md)
3. [Create and test a board](getting-started.md)
4. [Review configuration](configuration.md)
5. [Run and monitor the board](sysop-guide.md)

## Day-to-day operation

- [Caller Management](caller-management.md)
- [Messages](messages.md)
- [Files](files.md)
- [File Transfers](transfers.md)
- [Terminal Clients](terminal-clients.md)
- [Secure SSH Caller Transport](../sfng-secure-ssh-transport.md)
- [Backup and Restore](backup-restore.md)
- [Upgrades and Rollback](upgrades.md)
- [Troubleshooting](troubleshooting.md)
- [Support and Bug Reports](support.md)

## Presentation and language

- [Customizing Display Screens](custom-display-screens.md)
- [Classic SPITFIRE-Inspired Presentation](classic-presentation.md)
- [Language Packages](localization.md)

## Status labels

- **Verified** means an acceptance workflow demonstrated the behavior.
- **Development Preview** means the documented workflow is usable within its
  stated platform, security, and support limits.
- **Planned** means it is not implemented yet.

For current release scope see [Status](../../STATUS.md). For implementation
details use the main [documentation index](../README.md).
