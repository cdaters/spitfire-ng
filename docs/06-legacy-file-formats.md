# SPITFIRE Legacy Data and File Format Specification

## 1. Purpose

This document will serve as the central technical record for SPITFIRE legacy file formats.

It is intended to support:

- preservation
- migration
- interoperability
- testing
- modern reimplementation
- independent utility development

The specification should distinguish clearly between confirmed facts, reconstructed behavior, and unresolved assumptions.

## 2. Confidence Levels

Each documented structure should carry one of the following classifications.

### Confirmed

Verified through original documentation, multiple files, or reproducible testing.

### High Confidence

Strongly supported by binary/file analysis.

### Probable

Evidence suggests the interpretation but additional testing is needed.

### Unknown

Field or behavior has not yet been identified.

## 3. General DOS Data Assumptions

Historical SPITFIRE data originated in Turbo Pascal and DOS.

Formats may therefore contain:

- little-endian integers
- fixed-length records
- Turbo Pascal short strings
- DOS dates
- DOS times
- Boolean values
- fixed record padding
- reserved expansion fields
- CP437 text

Modern implementations must decode these values explicitly.

Native machine structure packing must never be assumed.

## 4. Turbo Pascal Short Strings

A Turbo Pascal short string contains a length byte followed by character storage.

For example:

    String[12]

occupies:

    13 bytes

Layout:

    Byte 0       String length
    Bytes 1–12   Character storage

Unused character storage may contain stale data.

Therefore:

    0C "Alex Caller" "old unused bytes"

means only:

    Alex Caller

The bytes after the specified length are not part of the logical value.

Modern parsers must honor the length byte.

## 5. `SPITFIRE.HLP`

### Status

Confirmed.

### File Size

    20,130 bytes

### Records

    55 records

### Record Size

    366 bytes

### Structure

Each record contains six Turbo Pascal `String[60]` fields.

Calculation:

    String[60] = 61 bytes
    61 × 6     = 366 bytes
    366 × 55   = 20,130 bytes

Conceptual Pascal structure:

    type
      HelpRecord = record
        Line1 : String[60];
        Line2 : String[60];
        Line3 : String[60];
        Line4 : String[60];
        Line5 : String[60];
        Line6 : String[60];
      end;

### Modern Requirement

SPITFIRE NG reads and writes this format directly with exact record-size and
Pascal-string bounds. A missing, truncated, or malformed help file does not
terminate a caller session; current commands receive a bounded contextual
fallback.

A modern help editor should be capable of preserving the original file.

The preserved `SFHELP.DOC` assigns the current stock flows these one-based
records: Page 1, Goodbye 2, Xpert 3, Change File Area 7, List Files 8,
Download 10, File→Message 12, File→Main 13, New Files 15, description search
16, filename find 17, Help 18, Upload 20, Main→Message 21, Main→File 22,
Comment to Sysop 24, statistics 30, Browse 34, Change Conference 35, Read 36,
Enter 38, Your Messages 39, Message→File 41, and Message→Main 42. The menu's
immutable internal identifier selects the record; changing its displayed
command letter does not change help or action semantics.

## 6. Menu Files

### Known Files

    SFMAIN.MNU
    SFMSG.MNU
    SFFILE.MNU
    SFSYSOP.MNU

### Status

Confirmed as text-based configuration.

### General Structure

Historical documentation indicates fields representing:

    Command Character
    Command Description
    Reserved/Formatting Field
    Required Security Level
    Internal Command Identifier

Example conceptual line:

    M,<M>.......... Message Section,,5,E

### Modern Requirement

Original menu files must remain loadable.

Unknown fields should be preserved where possible when files are edited and rewritten.

SPITFIRE NG treats the first field as the caller-visible command letter and
the final field as the historical immutable action identifier. Dispatch and
help follow the identifier, not a hard-coded display key. Input is
case-insensitive and security is checked when resolving the command. With
stock hot keys enabled, network and serial adapters execute a displayed menu
key immediately; line mode requires Enter. Password, message-composition, and
binary-transfer input never use menu hot-key interception.

The final Category-A navigation boundary loads all four stock menus. Main,
Message, and File remain required board configuration, matching the manual's
startup requirement. `SFSYSOP.MNU` is generated for new boards and parsed by
the same bounds/identifier/security rules. A board created before this fourth
native menu was introduced receives a narrow built-in `Q` (Main), `X` (Xpert),
and `G` (Goodbye) menu if `SFSYSOP.MNU` is missing or malformed; this is an
upgrade-safety fallback, not an implementation of historical maintenance
commands. Valid supplied historical Sysop commands remain visible according
to their security and receive a bounded unavailable result when their owning
advanced capability is not implemented.

## 7. Display Files

### Known Extensions

    .BBS
    .CLR
    .RIP

### `.BBS`

Non-ANSI ASCII/IBM-compatible display. Historical supplied files demonstrate
that this class may contain single-byte CP437 above `0x7F`; it does not mean
seven-bit bytes only.

### `.CLR`

ANSI/color terminal instruction stream with ASCII/CP437 glyph bytes.

### `.RIP`

RIP ASCII command script. It is documented historically but not loaded or sent
by the current SPITFIRE NG ANSI/text tier.

### Authoring byte contract

Primary `SPITFIRE.DOC` does not prescribe TheDraw, clear/home, line length,
CRLF, code page, EOF, or SAUCE settings. The distributed 27-member DISPLAY
archive uses CRLF throughout, CP437 high bytes in every BBS member and some CLR
members, ANSI ESC only in CLR, no NUL, no SAUCE, and mixed terminal DOS EOF.
Clear/home behavior is resource-specific rather than universal.

Current NG requires CRLF for authored line structure because standalone LF is
the historical `^J` display control. It performs no UTF-8-to-CP437 conversion,
does not currently stop at terminal DOS EOF, and does not strip SAUCE. New BBS/
CLR authoring must therefore omit UTF-8 BOM, NUL, SAUCE, and DOS EOF, stay
within one MiB, and use the intended ANSI/CP437 bytes. See the
[operator workflow](operator/custom-display-screens.md).

### Modern Requirement

Display files should be treated as original BBS assets rather than converted automatically.

Terminal capability determines which version is displayed.

For the same logical stem, stock precedence is `.CLR` when ANSI is both
supported and desired, then `.BBS`. SPITFIRE NG also supplies bounded built-in
text for the required current journey when neither optional resource is
usable. An oversized or malformed preferred `.CLR` falls to `.BBS`; missing or
malformed optional content never panics the session. Legacy bytes remain
CP437-oriented, while modern internal strings remain Unicode. ANSI artwork is
not reflowed merely because a wider terminal was negotiated.

Current resource order is `SFPRELOG`, `WELCOME1`, authentication,
`NEWUSER`/`WELCOME2`…`WELCOME9`, `ALL`, `SFNOD<n>`, `<security>SEC`, and the
caller-number display. Exact-security `MAIN<n>`, `MSG<n>`, `FILE<n>`, and
`SOP<n>` files override generated security-filtered menu text. `SF1STM`,
`SF1STF`, `SFMSG<n>`, `SFIL<n>`, `SFDOWN`, `SFUP`, page/chat resources, and
`GOODBYE` cover the implemented section journey. RIP selection and the
complete event/questionnaire/bulletin/caller-state inventory remain
stock-advanced B-005/B-006 work.

### Deterministic caller-visible fallback matrix

| Lookup | Preferred | Fallback | Built-in/final behavior |
|---|---|---|---|
| Optional display stem | usable `.CLR` when effective ANSI is on | usable same-stem `.BBS` | The owning operation continues with its bounded native prompt/status; decorative absence is silent, as §5.4 permits. |
| Required core display (`SFPRELOG`, `WELCOME1`, `GOODBYE`, page/chat and policy outcomes) | usable `.CLR` where supported | usable `.BBS` | Deterministic built-in text describes the same outcome; terminal/session failure is returned normally rather than panicking. |
| Exact-security Main/Message/File/Sysop artwork | `<stem><security>.CLR` | `<stem><security>.BBS` | Render the caller-authorized `.MNU` descriptions. No command is invented by artwork. |
| Main/Message/File `.MNU` | case-insensitive bounded historical file | none | Missing/malformed required configuration prevents board startup before a caller is admitted. |
| `SFSYSOP.MNU` | case-insensitive bounded historical file | native upgrade fallback | Security-filtered `Q`/`X`/`G`; unavailable advanced commands never strand the caller. |
| `SPITFIRE.HLP` record | mapped valid record | none | Bounded contextual unavailable/help text; return to the same menu. |

ANSI, text-only, changed caller preference, and reconnect all use this same
matrix. Transports supply capability metadata only; Telnet, RAW, RLogin,
stdio, direct serial, and simulated modem do not choose resources or
navigation independently.

## 8. Display Macros

The confirmed SF37 §5.7 control table is implemented in both control-byte and
documented string form where a string form exists:

| Control | String form | Meaning in SPITFIRE NG |
|---|---|---|
| `^B` | `@PROMPTOFF@` | Suppress automatic paging prompts for this output unit |
| `^C` | `@NOABORT@` | Disable caller abort for this output unit |
| `^D` | `@FNAME@` | First word of the authenticated caller name |
| `^E` | `@SUBDATE@` | Subscription date; explicit `N/A` until modeled |
| `^F` | `@CITYSTATE@` | Explicitly not collected by the native caller model |
| `^G` | `@BEEP@` | Terminal bell |
| `^J` | `@UPLOADS@` | Successful upload count; DOS CRLF remains line structure |
| `^K` | `@DOWNLOADS@` | Successful download count |
| `^L` | `@CLS@` | Clear screen |
| `^N` | `@ABORTON@` | Re-enable abort for this output unit |
| `^O` | `@ORGLOG@` | Original/first logon timestamp |
| `^P` | `@PROMPT@` | Re-enable paging prompts for this output unit |
| `^Q` | `@LOGTIME@` | Remaining session minutes |
| `^R` | `@PHONENUM@` | Explicitly not collected |
| `^S` | `@LASTCALL@` | Previous call timestamp |
| `^T` | `@PASSWORD@` | Deliberately unavailable; never exposes a credential |
| `^U` | `@BIRTHDATE@` | Explicitly not collected |
| `^V` | `@NAME@` | Authenticated caller display name |
| `^W` | `@UPK@` | Successful upload KiB |
| `^X` | `@DOWNK@` | Successful download KiB |
| `^Y` | `@SLEVEL@` | Numerical SPITFIRE security level |

Native current-flow context also defines bounded `@BOARD@`, `@SYSOP@`, and
`@NODE@`. Unknown string macros remain visible byte-for-byte instead of being
assigned guessed meanings. Dates use four-digit UTC rendering at this native
boundary; original DOS date repair remains the independent SFDATE track.

The original shareware executable provides the caller-facing behavior behind
these controls even though manual §5.7 does not name the keys. Pascal strings
at file offset `0x211E0` compose `MORE: <S>top, <N>onstop, < ENTER > to
continue?`. SPITFIRE NG therefore treats Stop as aborting only the current
output unit, Nonstop as suppressing later prompts only in that unit, and Enter
as continuing normal page-by-page output. Q and Escape remain documented,
undisplayed modern Stop aliases; they are not historical claims.

Modern macros may be added, for example:

    @NODE@
    @IP@
    @PROTOCOL@
    @TLS@
    @HOSTNAME@

Historical macro meanings must not be changed, and macro expansion must never
expose credentials, host paths, or transport-authentication material.

## 9. Message Base Files

Known historical files include:

    SFMSGx.DAT
    SFMSGx.PTR
    SFMSGx.IDX
    SFMSGx.LMR

where `x` identifies a message conference.

Additional conference metadata appears in:

    SFMCONF.DAT

Exact SPITFIRE 3.7 structures remain under investigation.

## 10. Documented Historical Message Body

Earlier Buffalo Creek developer documentation identifies a structure conceptually similar to:

    MessageBody = record
      MsgData : String[127];
    end;

This implies fixed 128-byte body records.

A message may span multiple body records.

SPITFIRE 3.7 behavior must be independently confirmed.

## 11. Documented Historical Message Index

Earlier documentation describes an index containing values such as:

    From CRC
    To CRC
    Message Number
    Original Message Number

Potential conceptual structure:

    MessageIdx = record
      FromWhoCRC   : LongInt;
      ToWhoCRC     : LongInt;
      MsgNumber    : LongInt;
      OrgMsgNumber : LongInt;
    end;

Version-specific differences must be tested.

## 12. Message Pointer/Header Records

Historically documented fields include:

    Message Date
    From
    To
    Original To
    Subject
    NetMail
    Sent
    Purge When Sent
    Thread flags
    Private flag
    Deleted flag
    Received flag
    Body location
    Number of body records
    Conference
    Message number
    Original message number
    Reserved bytes

The exact SPITFIRE 3.7 field order and sizes must be verified.

## 13. Last Message Read

`SFMSGx.LMR` stores caller last-message-read information.

Required research:

- record key
- caller identifier
- message number representation
- conference mapping
- behavior when messages are purged
- multi-node synchronization

## 14. Caller Database

The exact SPITFIRE 3.7 caller record should be reconstructed.

Likely historical categories include:

    Caller name
    Password
    Location
    Telephone
    Birth date
    Security level
    First call
    Last call
    Calls
    Uploads
    Downloads
    Time statistics
    Preferences
    Flags

Modern storage should not require obsolete personal information.

Stock Core Increment 2 implements the native caller lifecycle independently
of this still-incomplete legacy record parser. See
[Native Caller and Authentication Model](sfng-caller-authentication.md) for
the implemented fields, normalization, SQLite schema, and deferred import
boundary.

## 15. Password Migration

Historical password storage must be handled cautiously.

If a legacy password can be validated but is not stored securely, migration should preferably work as follows:

    Legacy caller logs in successfully
              │
              ▼
     Historical password verified
              │
              ▼
      Modern strong hash created
              │
              ▼
     Legacy credential retired

This permits seamless migration without preserving weak password storage indefinitely.

## 16. Door Files

### `DOOR.SYS`

Industry-standard BBS door drop file.

Modern SPITFIRE should generate compatible files.

### `SFDOORS.DAT`

SPITFIRE-specific door information.

Exact historical structure should be documented from Buffalo Creek developer records and test files.

## 17. CircuitNet Files

Known formats include:

    .CNP
    .CND
    dossier/configuration files
    routing lists
    history files

These should receive a dedicated specification.

Research should identify:

- file headers
- record sizes
- node addressing
- conference addressing
- message representation
- routing metadata
- duplicate tracking

## 18. QWK

The QWK specification is external to SPITFIRE but forms part of compatibility requirements.

SPITFIRE-specific behavior to document includes:

- conference numbering
- LAKOTA configuration
- last-read behavior
- REP import
- network routing
- private messages
- aliases
- message conversion

## 19. File-Area Databases

SPITFIRE file-area configuration and file descriptions should be documented.

Fields of interest include:

    area number
    area name
    storage path
    required security
    upload path
    download accounting
    file descriptions
    upload metadata

## 20. Date Handling

Date formats require special attention because historical SPITFIRE exhibits post-2024 date problems.

Research must identify:

- internal year representation
- two-digit year assumptions
- epoch calculations
- packed DOS dates
- date comparison routines
- subscription dates
- caller dates
- file dates
- message dates
- event dates

The preservation project should document the original behavior before applying compatibility fixes.

## 21. Version Differences

File structures may differ across SPITFIRE releases.

Every tested file should therefore be tagged with:

    SPITFIRE version
    File size
    Record size
    Record count
    Source
    Hash
    Registration status where relevant

A versioned specification may eventually be required.

Example:

    SPITFIRE 3.1
    SPITFIRE 3.5
    SPITFIRE 3.6
    SPITFIRE 3.7

## 22. Unknown Bytes

Unknown or reserved bytes must never be casually discarded.

When rewriting a historical record, the safest default is:

    preserve unknown bytes unchanged

unless their semantics are understood.

This is particularly important for structures containing reserved expansion areas.

## 23. Test Corpus

The project should maintain a legal preservation corpus containing representative files such as:

    clean distribution
    registered installation
    populated caller database
    populated message bases
    menu files
    help files
    display files
    QWK packets
    CircuitNet packets
    door drop files

Each test fixture should document its provenance.

## 24. Parser Requirements

Legacy parsers should:

- verify file size
- verify record boundaries
- reject impossible lengths
- prevent integer overflow
- prevent out-of-range indexing
- tolerate unused padding
- preserve unknown data
- avoid interpreting raw structures through unsafe memory casts

Malformed data should produce an error, not a process crash.

## 25. Documentation Goal

Ultimately, a developer who has never seen the original SPITFIRE source code should be able to read this specification and independently implement tools capable of:

- reading a SPITFIRE installation
- interpreting its resources
- displaying its messages
- reading its users
- modifying menus
- generating help
- producing compatible door files
- importing or exporting network messages

That is the standard by which this specification should be judged.
