# Future shared Files, archive and scanner architecture obligation

C5 recorded this design obligation without implementing it. C6 implements the
[initial native custody, inspection, scanner and distribution contract](files-custody.md);
its remaining formats, derivatives, automatic rescans and cleanup remain future work. File networking is not
part of CircuitNET C5. Future CircuitNET Files must reuse board-wide SPITFIRE Files
services and native file/content authority, rather than introducing a separate
archive or malware subsystem inside the network adapter.

The design gate must cover:

- ZIP, 7z, RAR, TAR, GZIP/TGZ, BZIP2, XZ and Zstandard, with practical preservation
  support for historical ARC/ARJ/LZH. Detect actual content type rather than trusting
  filename extensions. Validate archive structure and member names.
- Explicit nested-archive depth, expanded-byte, member-count, CPU and time limits;
  decompression bombs; path traversal; absolute paths; symlink and hardlink escapes;
  encrypted archives and unavailable decoders. Unknown/unscannable is not clean.
- Cryptographic content identity, hashing and duplicate detection, preserving
  original uploaded bytes. A board-branded/repacked distribution derivative must
  have separate identity and provenance and never silently replace the original.
- Pluggable malware scanners, quarantine, scanner failure and timeout outcomes,
  definition update policy and local rescans. Scanner failure must never equal clean.
- FILE_ID.DIZ and archive descriptions as metadata, distinct from optional archive
  branding/comment files. The historical ReComment concept is adequate input;
  locating or reverse-engineering that utility is not required.
- Content-addressed network distribution, local availability/access decisions,
  scanner trust boundaries, receipts/replay and rescan policy on receiving boards.

Events may eventually trigger scanner-definition updates, quarantine rescans,
maintenance and distribution using typed service APIs. Define those interfaces and
failure/restore semantics first. No speculative file or governance tables are added
by C5, and no current upload bytes, file areas or production boards are changed.
