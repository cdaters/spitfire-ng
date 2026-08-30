# Historical `THOUGHTS.BBS` Format

This rights-safe summary records documented behavior and independently
observed byte geometry for SPITFIRE 3.7 `THOUGHTS.BBS`. It redistributes no
historical program, archive, sample record, or sample text. It does not
implement a parser or caller feature.

## Historical purpose

The SPITFIRE 3.7 manual identifies `THOUGHTS.BBS` as an optional DISPLAY-path
resource created by the separate THOUGHTS utility. It belongs to the
historical BBS/ASCII-only display class: no `.CLR` or `.RIP` counterpart is
defined. BBS/ASCII is the product's non-ANSI classification; it does not prove
a universal seven-bit encoding restriction.

THOUGHTS 3.1 documentation describes six lines of up to sixty characters. It
states that SPITFIRE randomly selects a thought when a caller logs on and that
the utility can display a random record from a colocated `THOUGHTS.BBS`.

## Exact physical format

Independent byte inspection of more than one historical file establishes:

```text
THOUGHTS.BBS := ThoughtRecord*
ThoughtRecord := PascalString60 PascalString60 PascalString60
                 PascalString60 PascalString60 PascalString60
PascalString60 := length:u8 backing_store:[u8; 60]
```

Therefore:

- one field occupies 61 bytes;
- one record occupies `6 * 61 = 366` bytes;
- a well-formed file size is divisible by 366;
- record count is `file_size / 366`; and
- there is no observed header, stored count, index, stored record number,
  CR/LF record delimiter, record separator, or EOF marker.

Each length byte must be in `0..=60`. Logical content is only the first
`length` bytes of the following 60-byte backing store. Bytes after the
declared length are undefined and have no semantic or presentation authority.
A reader must never decode, render, search, log, or interpret them.

## Marker and zero-length fields

Historical utility documentation states that `;` represents a blank line and
`\` represents “no line.” Byte evidence confirms both as one-byte Pascal
strings whose complete logical payload is that marker. Historical files also
contain fields with a declared length of zero.

A compatible semantic model keeps these states distinct:

```text
Text(bytes)
SemicolonBlankMarker
BackslashNoLineMarker
ZeroLengthField
```

An ordinary semicolon or backslash inside longer text is not proven to be a
marker. Exact zero-length rendering, whether recognition uses exact equality
or only the first byte, and behavior for `;text` or `\text` remain unresolved.

## Encoding

Observed logical payload in the inspected samples is printable seven-bit
ASCII. That is sample evidence, not a general format rule. A compatible reader
should retain logical payload bytes losslessly and decode them only through an
explicit DOS/legacy encoding boundary. High-bit payload must not be rejected
merely because the inspected samples do not contain it.

## Selection, merge, and purge

Random selection is directly documented. The exact random-number generator,
seed, distribution, repeat suppression, empty-file behavior, and malformed-
record behavior are not established.

Utility documentation describes merge, record numbering, and purge. With no
stored count or record number, physical ordinal and record offset form the
evidence-backed indexing model. Appending records and rebuilding a compacted
file explains renumbering, but exact replacement and failure behavior remains
unverified.

## Parser safety rules

A future parser must:

1. bounds-check every field and reject length bytes above 60;
2. reject a file whose size is not divisible by 366 unless a separately
   documented recovery mode is authorized;
3. read only declared logical bytes;
4. preserve marker and zero-length states independently;
5. keep logical bytes lossless until explicit legacy decoding;
6. impose bounded record and rendered-output limits;
7. never use undefined backing bytes;
8. avoid unsafe structure casts and compiler-packing assumptions; and
9. leave source files unchanged during inspection.

Public tests should generate synthetic 366-byte records with fictional,
project-authored text. Useful cases include ordinary text, both markers, zero
length, maximum length, high-bit logical bytes, stale backing bytes, invalid
length, and truncation.

## Remaining runtime questions

A future controlled original-runtime session may answer:

- zero-length rendering;
- exact `;` and `\` recognition;
- `;text` and `\text` behavior;
- CP437/high-bit entry and display;
- invalid length and truncated-file handling; and
- exact random selection, seeding, and repetition behavior.

No original-runtime test is required to establish the fixed 366-byte format.

## Publication boundary

This repository intentionally includes no historical THOUGHTS executable,
archive, sample database, or sample text. The project license covers this
independently written description and any future synthetic fixtures; it does
not relicense original historical material.

See [Legacy Data and File Formats](../06-legacy-file-formats.md) and
[Public Information](../sfng-public-information.md).
