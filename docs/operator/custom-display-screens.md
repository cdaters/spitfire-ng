# Customizing SPITFIRE NG Display Screens

This guide is for a Sysop creating board-specific ANSI and text screens. It
uses the board-owned override layer in the 0.1.0 Development Preview.
It does not require editing a presentation package or a database.

## The three display extensions

| Extension | Meaning | Current SPITFIRE NG status |
|---|---|---|
| `.CLR` | ANSI color terminal byte stream, normally with IBM CP437 characters | Supported for ANSI-capable callers |
| `.BBS` | Non-ANSI text/IBM byte stream; CP437 artwork is permitted | Supported and used as text fallback |
| `.RIP` | Historical RIPscrip graphics | Not implemented; do not install RIP files expecting them to render |

These are DOS-oriented byte formats. A visually correct Unicode document is
not automatically a compatible display file.

## Where your custom file belongs

Do not normally edit:

```text
<board>/system/presentation-profiles/<profile>/resources/display/
```

Those files belong to installed, hashed packages. Copy the resource you want
to customize into the board's editable layer:

```sh
cp /absolute/path/to/board/system/presentation-profiles/modern-ng/resources/display/MAIN10.CLR \
   /absolute/path/to/board/display/MAIN10.CLR
```

Edit only the copy under `<board>/display/`. For an exact-security menu the
resolution order is:

1. board `display/` override;
2. active profile's exact-security resource; and
3. engine-generated, security-filtered menu.

Deleting or moving the board-local override restores the lower-priority
profile resource or generated menu. It does not delete caller data or alter
the installed package.

## Filename and security examples

The numeric suffix on menu resources is an **exact caller security level**:

- `MAIN10.CLR` is ANSI Main artwork only for security 10;
- `MAIN10.BBS` is its non-ANSI security-10 counterpart;
- `MAIN50.CLR` is a different exact-security resource; and
- `MSG10`, `FILE10`, and `SOP10` follow the same rule for their sections.

The suffix is not automatically replaced by the configured Sysop threshold.
The historical manual mentions supplied `SOP999` files while the inspected
archive actually contains `SOP100`; that discrepancy remains unresolved and
neither number is a universal Sysop level.

Static artwork never grants authority. If an edited menu advertises a command
that `.MNU` and the engine do not authorize, the command remains unavailable.

## Recommended `.CLR` workflow on macOS

Moebius 1.0.29 is **VERIFIED for `.CLR` authoring on macOS** with the exact
workflow below. An untouched Save Without Sauce export passed byte inspection
and rendered through SPITFIRE NG in Qodem 1.0.1 and SyncTERM 1.9rc4. This does
not validate direct `.BBS` export, normal Save, iCE backgrounds, animation, or
RIP.

Use these validated settings:

1. Create an **80 columns × 25 rows** document.
2. Use the IBM VGA/CP437 character set and the normal 16-color ANSI palette.
3. Keep **iCE colors off**. Use the eight normal backgrounds and normal/bright
   foregrounds; avoid blink-dependent high-intensity backgrounds.
4. Keep artwork static. Avoid animation delays, 256/true color, exotic cursor
   modes, and unnecessary cursor tricks.
5. Use **File → Save Without Sauce Info…**. Do not use normal Save for a
   finished SPITFIRE file.
6. Do **not** use **Export As UTF-8…**.
7. Save as an ANSI file first, inspect it, and then use the correct `.CLR`
   name. Moebius 1.0.29's no-SAUCE ANSI export produced the required legacy
   byte stream on macOS.
8. If the display intentionally owns a fresh screen, arrange for a leading
   SPITFIRE `@CLS@` control. Do not assume every menu should clear: historical
   supplied menu CLR files do not universally clear or home the cursor.
9. Provide a functionally equivalent `.BBS` fallback for caller-critical
   content.

Why **Save Without Sauce** matters: Moebius normal save appends DOS EOF and a
SAUCE metadata record. Current SPITFIRE NG passes those bytes through rather
than stripping them. The no-SAUCE path avoids that incompatibility.

## Recommended `.BBS` workflow

For a 7-bit-only screen, use a plain text editor configured for:

- plain text, not rich text;
- no UTF-8 BOM;
- CRLF line endings;
- lines composed for the intended terminal width; and
- no ANSI escape sequences or SAUCE metadata.

For IBM box/block artwork, the saved file must contain single-byte CP437—not
UTF-8 encodings of Unicode box characters. Do not assume a normal macOS text
editor can do that.

Moebius is useful for drawing CP437, but its ANSI save always begins with an
ANSI reset even when the canvas uses default colors. Selecting `.ASC` or
`.TXT` does not select a separate plain-text encoder in 1.0.29. Therefore do
not copy a Moebius export directly to `.BBS` until its bytes have been checked
and a strict no-ANSI preparation workflow is separately validated.

CRLF is required by the current NG compatibility edge: a standalone LF byte
is the historical `^J`/uploads display control, while LF following CR is a
line ending. UTF-8 high-byte sequences are not converted into CP437.

## Inspect before installing

Use the research-only, non-destructive inspector:

```sh
ruby tools/inspect-display-resource.rb /absolute/path/to/MYSCREEN.CLR
```

For a `.CLR`, verify:

- `SAUCE record: none`;
- no DOS EOF unless a later NG compatibility change explicitly permits it;
- `bare-LF=0` and preferably `bare-CR=0`;
- no NUL;
- expected ANSI CSI sequences and CP437 high bytes; and
- file size below the current 1 MiB resource limit.

For `.BBS`, also require zero ANSI ESC bytes. A physical encoded ANSI line may
be longer than 80 bytes because escape sequences do not occupy screen cells;
do not use byte length alone as the display width.

## Test on a rehearsal board

1. Make a cold backup using the supported operator workflow.
2. Use a clean or disposable board—not your only working board.
3. Copy the candidate into that board's `display/` with its final supported
   filename.
4. Start SPITFIRE NG and connect with SyncTERM and Qodem at ANSI-BBS/CP437,
   80×25.
5. Confirm `spitfire status` identifies
   `exact-security-board-override` where an exact menu override is expected.
6. Test the matching caller security and a level with no matching override.
7. Test Text mode against the BBS counterpart.
8. Confirm commands and security still come from `.MNU`, then exercise redraw,
   Help, paging, and Goodbye.

If anything is wrong, stop the board and move the candidate out of
`<board>/display/`. The active profile or generated fallback becomes visible
again. No SQLite editing is required.

## Reproducing the validated sample

SPITFIRE NG validated a direct Moebius 1.0.29 export created with this recipe:

1. In Moebius, create an 80×25 document with IBM VGA/CP437, 16-color ANSI,
   iCE colors off.
2. Put `SPITFIRE NG ANSI REFERENCE` in ordinary ASCII on row 1, followed by
   uppercase, lowercase, number, and punctuation rows.
3. On rows 7–11, draw a 26-cell single-line CP437 box containing a text label
   plus real shade, block, and symbol glyphs.
4. On rows 13–23, place labeled samples using ordinary and bright foreground
   colors on a black background.
5. Put `END OF TEST` on row 25. Do not add animation, iCE backgrounds, Unicode
   paste, or SAUCE comments.
6. Choose **File → Save Without Sauce Info…** and save it to a local working
   directory, for example:

   ```text
   ./moebius-reference.ans
   ```

7. Do not open or resave it in another editor before inspection.

The untouched 499-byte export had SHA-256
`c687d3ce941b6536b96b8b3c8bfbc1de66a76353ecb29776dd032d3b0219f64d`.
It used 24 CRLF pairs, 21 complete ANSI SGR sequences, and 92 single-byte
CP437 glyph bytes, with no BOM, NUL, DOS EOF, SAUCE, or appended metadata.
Qodem and SyncTERM rendered its frame, shade/block row, normal/bright colors,
and final marker correctly through NG's board-local CLR override path.

The reference fills 25 logical rows. If a 25-row screen is used as Welcome,
engine-owned connection identity and the following prompt can scroll part of
it. That is normal terminal page budgeting: compose the deployed resource for
the surrounding journey rather than assuming it owns every terminal row.
The accepted reference's longest visible row is 33 cells, so this validation
does not by itself prove an exporter-specific column-80 wrapping behavior.

## Troubleshooting

- **Box characters are accented letters:** the file is probably UTF-8 or the
  terminal is not using CP437.
- **Metadata appears after the art:** the file was saved with SAUCE; re-export
  with **Save Without Sauce Info**.
- **Uploads count appears where a newline belongs:** the file contains bare LF;
  export CRLF.
- **Text callers see ANSI escapes:** an ANSI stream was installed as `.BBS`.
- **Your file is ignored:** check exact security, filename, caller graphics
  preference, active profile/menu mode, size, and malformed ANSI diagnostics.
- **Removing the override changes the screen:** expected; you revealed the
  active profile or generated fallback.

The inspector and this reproducible recipe preserve the relevant validation
details without distributing the private reference export.
