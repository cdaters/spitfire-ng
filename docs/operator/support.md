# Development Preview Support and Bug Reports

SPITFIRE NG 0.1.0 is a public-testing Development Preview, not a supported
production service or a stable compatibility promise. Reports that clearly
identify the exact build and reproduce a problem are welcome; response times,
individual deployment consulting, and data recovery cannot be guaranteed.

Use the issue tracker linked from the official release/download page for
ordinary bugs. The publication checklist requires that tracker to be publicly
reachable before the download is announced. Search existing reports first and
file one problem per issue.

## Include this information

Sanitize the output, then include:

- `spitfire --version`;
- macOS version and Apple Silicon model/architecture;
- the relevant portions of `spitfire status /path/to/board/spitfire.toml`;
- transport and client, such as Telnet with Qodem or SyncTERM;
- terminal emulation, encoding, columns, rows, and page length;
- active presentation profile, menu mode, and renderer source when relevant;
- exact reproduction steps, expected behavior, and observed behavior; and
- the smallest relevant foreground log excerpt.

Say whether the problem reproduces on a clean setup-created board. For data or
upgrade problems, state the source version, target version, schema reported by
status, whether the board was stopped, and whether a cold backup/restore was
rehearsed.

## Never attach or paste

- passwords, password hashes, recovery material, API tokens, or private keys;
- registered/private historical SPITFIRE binaries;
- a real caller database, full backup, or complete configuration containing
  secrets;
- private messages, contact fields, birth dates, phone or postal information;
- unredacted public IP addresses or local usernames/paths when they are not
  needed; or
- proprietary historical DISPLAY, documentation, or research archives.

Create a synthetic board/caller and redact logs or screenshots. If a maintainer
needs more evidence, agree on a private, minimal transfer rather than posting a
live board publicly.

## Security vulnerabilities

Do not publish exploit instructions for an unpatched vulnerability in a public
issue. Use the private vulnerability-reporting channel identified on the
official release page. Publication must not proceed until that channel is
enabled and tested. Include impact, affected version, reproduction conditions,
and a safe way to contact you, but never send active board credentials.

## Preview upgrade expectations

Schema migrations and documented cold backup/restore are tested, but broad
forward/backward compatibility is best-effort during the 0.x Development
Preview. Preserve the old archive and checksum, old executable, and a verified
cold backup. Rehearse every upgrade on a restored copy and roll back by
restoring the old snapshot with the old executable; do not downgrade a migrated
database in place.

Production use, exposure of plaintext transports to untrusted networks, and
operation without tested backups are discouraged during this preview.
