# SFDraw — Planned SPITFIRE Display Editor

SFDraw is a future Rust-based, cross-platform editor for SPITFIRE-compatible
ANSI and text screens. It is not implemented yet.

## Why SFDraw?

Modern ANSI editors can produce excellent artwork, but a SPITFIRE Sysop also
needs byte-aware saving, CP437 fidelity, exact-security filenames, display
control awareness, and a preview that matches the BBS runtime. SFDraw is
intended to bring those needs together without requiring DOS or copying the
interface and assets of historical editors.

## Planned capabilities

- native `.CLR` and `.BBS` load and save;
- IBM VGA/CP437 glyphs and classic 16-color ANSI attributes;
- classic 80x24, 80x25, 80x50, and 132x25 starting presets;
- custom widths and long-form canvases extending hundreds or thousands of
  rows;
- an independently scrolling viewport with page and direct row/column
  navigation;
- exact CRLF handling and visible final-line behavior;
- ANSI and SPITFIRE display-control awareness;
- explicit SAUCE and DOS EOF detection/preservation choices;
- board-local `display/` override naming and validation;
- runtime-aligned preview; and
- instant or simulated modem/baud-rate playback for reviewing long ANSI
  reveals.

Preset terminal sizes are starting points, not canvas limits. Safety and
performance bounds will be explicit, but the editor should not impose an
artificial 25-row maximum.

## Shared runtime boundary

Where practical, SFDraw and SPITFIRE NG should share GUI-independent Rust code
for CP437, parsing, validation, serialization, and preview. The BBS runtime
remains the compatibility authority; the editor must clearly distinguish its
own visualization from behavior actually accepted by NG.

Loading and saving an existing resource should be lossless unless the author
explicitly requests normalization. Unknown or unsupported bytes must not be
silently discarded.

## RIP

RIP viewing or editing is deferred until SPITFIRE NG implements and validates
RIP capability negotiation, parsing, resource selection, lifecycle, and safe
delivery. An editor preview must never imply that the current BBS can send RIP
to callers.

## Rights and provenance

SFDraw will be independently designed and authored. Historical tools may
inform workflow research, but their source, UI, artwork, icons, help text, and
bundled assets cannot be copied without a verified compatible license.

Tests and documentation will use synthetic project-authored resources. Every
third-party dependency or asset must record its author, source, license, and
modification history.

## Next design step

Before implementation, a separate design milestone must choose the GUI/toolkit
approach and define file round trips, long-canvas limits, shared-library
boundaries, preview semantics, packaging targets, and acceptance tests.
