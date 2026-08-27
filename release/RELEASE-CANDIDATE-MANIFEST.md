# SPITFIRE NG 0.1.0 Development Preview Release Manifest

This is the finalized human-readable record for the accepted and published
Development Preview bytes.

| Field | Release |
|---|---|
| Product | SPITFIRE NG Bulletin Board System |
| Version | 0.1.0 |
| Channel | development-preview |
| Tag | `v0.1.0-development-preview` |
| Public repository | `https://github.com/cdaters/spitfire-ng` |
| Release | `https://github.com/cdaters/spitfire-ng/releases/tag/v0.1.0-development-preview` |
| Source commit | `75ed259b9acc030446fde500c2d5c33233c5e4fa` |
| Target | `aarch64-apple-darwin` |
| Platform | Apple Silicon macOS |
| Archive | `spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz` |
| Archive SHA-256 | `6c4d7ad492b1acee92481a3a577b49934c08e79822e98de50e918489a8fc9c97` |
| Archive size | 3,005,732 bytes; 362 entries |
| Binary SHA-256 | `ef5daeab29c7e5bdd5504b986b8800c34fc1eeead126fced3fab2f0d2f82bd14` |
| Signing | No Apple Developer ID; linker ad-hoc signature only |
| Notarization | Not notarized |
| Project license | MIT OR Apache-2.0 |
| Profiles | modern-ng 1.0.1; classic-spitfire 1.1.1; minimal-terminal 1.0.1 |
| Language | en-US 1.0.1 |
| Rust toolchain | rustc 1.97.1 (8bab26f4f); cargo 1.97.1 (c980f4866) |
| Cargo.lock SHA-256 | `efe6668eb8119d041444278eee00ad1b81e8258e4a7171d39dab222c3910ec1b` |
| Accepted host | macOS 26.6.2 (25G83), Apple Silicon arm64 |
| Gate status | PUBLISHED; public-redownload verification passed |

The source commit above is the private preservation repository's accepted
build record. A fresh public repository intentionally begins with sanitized
history; that private commit ID will not resolve there. The public snapshot
retains the runtime source while omitting private history, research-only
workspaces, screenshots, and historical inputs. Its lockfile removes only the
two omitted workspace-package entries; dependency versions are unchanged. The
accepted archive still contains and verifies its original lockfile, whose hash
is recorded above. Future releases should record a source commit and lockfile
from the public repository.

## Acceptance record

The exact final binary is byte-identical to the binary used for clean setup,
Qodem 1.0.1 generated-menu traversal, SyncTERM 1.9rc4 exact-profile/new-caller
traversal, RAW Text fallback, clean Goodbye/listener shutdown, status, cold
backup, and new-root restore. The archive and internal manifest verify; all 34
archive Markdown files have valid local links; project licenses match the
repository; 111 locked target dependency packages have notices; and bounded
text/binary scans found no private build-home path, credential, board data,
historical/research payload, or proprietary SPITFIRE resource.

The Mach-O is arm64 with a linker ad-hoc signature, no Developer ID/team
identity, and no notarization. No valid code-signing identity was available on
the build host. Archive assembly is recorded but is not claimed bit-for-bit
reproducible.

After publication, the archive was downloaded again from the public GitHub
Release and matched the canonical SHA-256 above. On Apple Silicon macOS,
expected Gatekeeper behavior was observed and the documented **System Settings
→ Privacy & Security → Open Anyway** workflow succeeded. The downloaded
binary then returned `SPITFIRE NG Bulletin Board System 0.1.0`. Developer ID
signing and notarization remain intentionally deferred and do not block this
Development Preview.

## Known limitations

The Development Preview is Apple Silicon only, unsigned/unnotarized, and not a
stable or production-hardened 1.0 release. RIP, caller-selectable profiles,
production non-English translations, Category-B/ecosystem expansion, SSH, web
administration, automatic updates/services, SFDraw, SFDATE, and SFREG are not
included. Telnet, RAW, and RLogin are plaintext compatibility transports.

The exact public scope and installation instructions are in
[the release notes](RELEASE-NOTES-0.1.0-DEVELOPMENT-PREVIEW.md) and
[publication checklist](PUBLICATION-CHECKLIST.md).
