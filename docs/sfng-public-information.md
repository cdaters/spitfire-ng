# SPITFIRE NG Public Information

M043 implements the stock caller-information and board-information surfaces
without exposing complete caller records or internal configuration. Historical
classification and remaining adapter gaps are summarized in the
[stock parity checklist](stock-spitfire-3.7-parity.md).

## Caller privacy

The directory is disabled on new and migrated boards. Every caller is
unlisted until choosing otherwise. Enabling the board policy permits listing;
it cannot override a caller's opt-out.

A visible caller row uses only the public handle. The operator may also enable
board-local last-call date and city/region. Login identifier, optional private
real name, credentials, contact details, birthday, subscription/security
state, JOKER details, audit, and Disabled/Deleted callers never appear.

Main `#` shows the caller's own listed state, offers a listed/unlisted change,
and then shows the bounded directory when enabled. Main `L` searches public
handles only: 1–30 ASCII bytes, case-insensitive contiguous substring,
deterministic order, at most 50 candidates, and sequential confirmation.
Visibility is checked again immediately before a row is shown.

## Other BBS information

Main `O` displays ordered Active native entries. Main `A` permits an Active
caller to add a name, speed label, and dial string only when the operator has
enabled caller additions. Caller additions are disabled by default. Public
output never identifies the contributor.

The local operator console provides:

```text
INFO-POLICY
INFO-POLICY-SET <version> <ON|OFF> <LAST|NO-LAST> <LOCATION|NO-LOCATION> <CALLER-ADD|NO-CALLER-ADD>
BBS-LIST
BBS-ADD <name>|<speed>|<dial>
BBS-EDIT <id> <version>|<name>|<speed>|<dial>
BBS-MOVE <id> <version> <order>
BBS-STATE <id> <version> ACTIVE|DISABLED
```

Versions are required so concurrent maintenance conflicts instead of silently
overwriting. Native SQLite rows are authoritative. The original manual
documents `SFBBSLST.DAT` as a WORK-path list of board names, BPS rates, and
phone numbers; callers could append those three values, and the added number
was recorded in the caller log so the Sysop could trace the contribution.
Import and export are not implemented because the available documentation
does not define record delimiters, field widths, escaping, line endings,
duplicate handling, append ordering, or the exact caller-log representation.

## Board information

- Main `B` lists and displays board-owned numbered bulletins 1–99.
- Main `N` displays the board-owned newsletter and may report that its
  accepted content generation changed since the caller's prior call.
- Main `T` displays only board name, configured public Sysop display name,
  board start date, and completed-call count.
- An optional project-native `DISPLAY/THOUGHTS.NG` contains one UTF-8 thought
  per nonempty line, up to 256 records of 512 bytes each. It is deliberately
  not a `THOUGHTS.BBS` parser.

The original manual separately documents `THOUGHTS.BBS` as an optional
DISPLAY-path file produced by `THOUGHTS.EXE` and shown to callers when present.
It explicitly belongs to SPITFIRE's BBS/ASCII-only display class: no CLR or RIP
counterpart is defined. The available corpus does not define its header or
record layout, delimiters, limits, malformed-input behavior, or selection
algorithm. `THOUGHTS.NG` therefore supplies bounded current-source semantics
without claiming byte compatibility.

Bulletin/newsletter BBS/CLR files retain existing board DISPLAY authority,
path confinement, one-MiB bound, paging, profile selection, and CP437/terminal
encoding handling. Profiles and language packages style the UI but cannot
change visibility, ordering, identity, or content authority.

In the original terminology, `.BBS` is the non-ANSI “ASCII” form, `.CLR` is
ANSI/color, and `.RIP` is RIPscrip. Historical BBS resources may still contain
CP437 high bytes. Display files may also contain documented dynamic control
bytes or strings; the current interpreter treats those as bounded presentation
instructions rather than inert text and never expands the historical password
control into a credential.

## Persistence and recovery

Schema 14 persists the directory policy, each caller's choice/version, Other
BBS rows/order/lifecycle/contributor/version, content-free semantic events,
and recognized resource generation/digest state. Cold backup preserves those
SQLite facts and exact SYSTEM/DISPLAY bytes. Restoring an older schema-13
backup keeps it at 13 until normal writable startup performs the transactional
migration with private defaults.

Searches are not audited. Mutation audit uses stable IDs and semantic
operations; it does not copy queries, private profile data, public row text,
resource contents or paths, credentials, or security secrets.
