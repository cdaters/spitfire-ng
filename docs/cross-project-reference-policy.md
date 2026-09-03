# FireComm and SPITFIRE NG Cross-Project Reference Policy

SPITFIRE NG and FireComm are independent projects. Where their technical
domains overlap, either project may perform a bounded read-only review of the
other project's public research, architecture, interfaces, test vectors,
renderer decisions, capability models, and interoperability findings.

## Authority and independence

Historical SPITFIRE evidence remains authoritative for what SPITFIRE did.
FireComm may inform how SPITFIRE NG implements terminal and interoperability
concerns safely; it does not redefine historical behavior.

FireComm is an engineering reference and potential interoperability peer, not
a runtime or compile-time dependency. Concepts may be adopted, adapted,
rejected, deferred, or left project-specific. Code is not copied merely
because a similar solution exists. A genuinely reusable abstraction should be
documented as an interface before any separate decision to extract common
code.

## Capability taxonomy

Support claims should name the layer actually implemented:

| Layer | Meaning | Examples |
|---|---|---|
| Character encoding or repertoire | Mapping between bytes, code points, and characters | CP437, PETSCII, ATASCII |
| Terminal behavior or protocol | Input/output control semantics and terminal state | ANSI, VT100-family, VT52-family |
| Graphics or presentation protocol | Negotiated visual commands or content representation | ANSI art, RIPscrip, Sixel |
| Visual or font profile | Glyph appearance and styling | IBM PC/VGA, Commodore, Amiga Topaz, Atari, ZX Spectrum, Amstrad CPC |

A font or character repertoire alone does not establish support for an entire
historical platform, terminal protocol, or dial-up client.

## Future presentation research

Sixel is a possible future optional, capability-negotiated presentation
format. It must have graceful ANSI or text fallback and must never change
commands, authorization, caller identity, message or file authority, transfer
accounting, or required workflows. No Sixel capability or resource is present
today.

PETSCII, ATASCII and Atari terminal modes, Amiga/Topaz presentation, ZX
Spectrum and Amstrad CPC character/font presentation, and other relevant
microcomputer environments are research candidates—not implemented
capabilities. Any future work must distinguish character repertoire, terminal
protocol, graphics protocol, and font rendering, and must verify historical
nomenclature and behavior independently.
