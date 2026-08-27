# Historical SPITFIRE

SPITFIRE NG is rooted in Buffalo Creek Software's SPITFIRE Bulletin Board
System. This page gives contributors and Sysops enough historical context to
understand the compatibility work without redistributing the original
software or turning the source repository into an archive vault.

## The original system

SPITFIRE 3.7 was a 16-bit DOS BBS built around callers, security levels,
nodes, message conferences, file areas, editable menus and displays, doors,
events, and Sysop control. Its operating environment included serial/modem
communication, FOSSIL drivers, DOS paths, batch files, companion utilities,
and per-node processes.

Some of those details define the historical experience; others are platform
constraints. SPITFIRE NG preserves the caller/session model, commands,
security filtering, conferences, file areas, resources, and recognizable
presentation while using modern storage, authentication, networking, and
multinode coordination.

## Historical resource model

Important historical resource families include:

- `.MNU` command records for Main, Message, File, and Sysop menus;
- `.HLP` fixed records for command help;
- `.BBS` text/IBM display streams;
- `.CLR` ANSI/color display streams; and
- `.RIP` Remote Imaging Protocol resources.

SPITFIRE NG currently supports bounded menu/help compatibility and BBS/CLR
presentation. RIP is evidence and future scope only; RIP bytes are not sent to
callers.

Historical display names may carry an exact security suffix, such as
`MAIN10.CLR`. The suffix selects artwork for that exact caller security level;
it does not grant commands or authority. If matching artwork is absent or
unusable, NG falls back safely to generated, security-filtered menus.

## Formats and preservation

The original program was written in Turbo Pascal with assembler support.
That matters when reading its data:

- integers are little-endian where documented;
- Turbo Pascal short strings include a length byte and fixed storage;
- padding and stale bytes may carry preservation value;
- CP437 bytes are not interchangeable with UTF-8; and
- native Rust or C structure layout cannot be assumed to match historical
  records.

SPITFIRE NG's legacy readers bounds-check input, reject impossible lengths,
avoid unsafe structure casts, and preserve unknown data when a round trip
requires it. See [Legacy Data and File Formats](06-legacy-file-formats.md).

## Compatibility scope

The defined Stock SPITFIRE 3.7 Core Parity tier covers the essential caller
and operator experience: setup, authentication, security, messages, files,
transfers, menus, caller context, Sysop interaction, multinode operation, and
recoverable board storage. The
[Stock SPITFIRE 3.7 Parity Checklist](stock-spitfire-3.7-parity.md) records the
detailed implemented scope and deliberate deferrals.

Advanced resources, offline-mail ecosystems, legacy networking, broader door
support, and RIP remain separate roadmap work.

## Evidence and confidence

Compatibility work distinguishes:

- **confirmed** behavior supported by original documentation, direct format
  analysis, or repeatable runtime evidence;
- **inferred** behavior that fits available evidence but needs corroboration;
  and
- **unknown** behavior that must not be guessed.

Historical screenshots, binaries, manuals, and archives used during research
remain outside the public source repository unless their redistribution rights
are independently established. Public tests use synthetic, project-authored
fixtures.

## Original software and documentation

For original SPITFIRE software, manuals, companion programs, and preservation
downloads, visit
[Original SPITFIRE Software & Documentation](https://spitfirebbs.com/).

Those materials retain their original copyrights and licenses. The SPITFIRE
NG `MIT OR Apache-2.0` license applies only to original NG code and
project-authored distributable resources unless a component says otherwise.
