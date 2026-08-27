# Development Preview Package

This is the operator contract for the SPITFIRE NG 0.1.0 Development Preview
archive. It explains what the package owns, what a board owns, how to verify an
archive, and how to recover from an unsuccessful upgrade.

## Supported package

The 0.1.0 preview has one accepted prebuilt target:

| Platform | Architecture / Rust target | Archive | Acceptance |
|---|---|---|---|
| macOS | Apple Silicon / `aarch64-apple-darwin` | `spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz` | Built, extracted, and live-tested |

Linux, Windows, Intel macOS, native installers, code signing, notarization,
background service definitions, and an automatic update channel are not
claimed by this release. Developers may still attempt source builds on
other Rust-supported hosts, but that is not a prebuilt-binary support claim.

The executable keeps normal Cargo SemVer `0.1.0`. `development-preview` is the
release channel in the archive name and `RELEASE.toml`; it is not a historical
SPITFIRE version.

## Verify and install a prebuilt archive

Keep the archive and its adjacent `.sha256` file together. In the download
directory, verify the archive before extracting it:

```sh
shasum -a 256 -c \
  spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz.sha256
tar -xzf spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz
cd spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin
shasum -a 256 -c MANIFEST.SHA256
./bin/spitfire --version
```

Expected version output is:

```text
SPITFIRE NG Bulletin Board System 0.1.0
```

The release candidate is checksum-protected but not signed with an Apple
Developer ID and is not notarized. Obtain the archive and checksum from the
same official versioned Release, compare the hash with the release manifest,
and keep all three with your recovery records. Follow the
[macOS first-run procedure](macos-first-run.md) if Gatekeeper blocks the first
launch. Install the executable by copying `bin/spitfire` to a directory on
`PATH`, or run it in place. It has no runtime dependency on a Git checkout or
the repository's research tree.

## Create a board

Run setup from a real terminal so the initial password can be entered without
echo:

```sh
spitfire setup /absolute/path/to/my-board
spitfire status /absolute/path/to/my-board/spitfire.toml
spitfire console /absolute/path/to/my-board/spitfire.toml
```

Normal setup creates configuration plus the historically meaningful `system/`,
`work/`, `display/`, `message/`, and `external/` trees. It installs the
`modern-ng`, `classic-spitfire`, and `minimal-terminal` presentation packages
and the `en-US` language package. Modern remains selected unless the operator
explicitly chooses another profile.

## The 30-second ownership rule

- Customize **this board** in `<board>/display/`.
- Treat `<board>/system/presentation-profiles/<profile>/` as an installed,
  versioned package. Do not hand-edit it.
- If neither layer supplies eligible artwork, SPITFIRE NG safely generates the
  menu from the authorized `.MNU` records.

For an exact-security display such as `MAIN10.CLR`, precedence is:

1. `<board>/display/MAIN10.CLR` — board-owned override;
2. `<board>/system/presentation-profiles/<active>/resources/display/MAIN10.CLR`
   — active package resource;
3. engine-generated stock menu — safe fallback.

The configured base profile does not supply exact-security menu artwork. It
continues to provide the documented fallback for non-menu resources. Exact
suffix selection is still exact: `MAIN10` does not become `MAIN50`.

To customize safely, copy the packaged file and edit only the copy:

```sh
cp /absolute/path/to/board/system/presentation-profiles/modern-ng/resources/display/MAIN10.CLR \
   /absolute/path/to/board/display/MAIN10.CLR
```

Removing that board-owned copy reveals the active-profile resource again, or
the generated menu when no eligible package resource exists. Package upgrades
must not overwrite `display/`. Editing a managed package can invalidate its
hashes/provenance and may be replaced by a future explicit package update.

For the byte-safe ANSI/text authoring workflow, exact-security naming,
verified Moebius settings, inspection, and recovery, see
[Customizing SPITFIRE NG Display Screens](custom-display-screens.md).

For a live caller, `spitfire status` distinguishes
`exact-security-board-override`, `exact-security-active-profile`,
`generated-stock`, and `expert-suppressed`. These values identify the source
that actually rendered the current menu; they do not grant authority.

## Managed package versions

Boards created by the 0.1.0 executable receive:

| Package | Version | Role |
|---|---:|---|
| `modern-ng` | 1.0.1 | Default active/base presentation |
| `classic-spitfire` | 1.1.1 | Independently authored Classic presentation |
| `minimal-terminal` | 1.0.1 | Plain/minimal presentation |
| `en-US` | 1.0.1 | Complete engine-language baseline |

Presentation and language packages keep separate descriptors, inventories,
hashes, compatibility declarations, and license/provenance records. Version 0.1.0 does
not add an independent profile updater. Replacing the executable does not
silently replace packages already installed in an existing board. New package
installation/update remains an explicit, separately validated operator action;
the current public language installer refuses to replace an existing locale.

## Upgrade without losing the board

1. Stop every process using the board and confirm `spitfire status` reports it
   offline.
2. Create a cold backup with the old executable.
3. Preserve that executable/archive, checksum, release metadata, and backup.
4. Restore the backup to a separate rehearsal directory.
5. Install the proposed new executable and run `status` against the rehearsal.
6. Start the rehearsal and verify login, menus, messages, files/transfers,
   profile/language readiness, Goodbye, and clean shutdown.
7. Compare important board-owned overrides before and after, then start the
   real board only after acceptance.
8. Keep the old snapshot for the chosen rollback window and create a fresh
   new-version backup.

Executable replacement preserves `spitfire.toml`, SQLite state, messages,
files, callers, `display/`, and installed board-local packages. Supported
forward schema migrations occur at startup. There is no rolling upgrade,
automatic package replacement, database downgrade, or update daemon.

## Roll back

Do not point an older executable at a database after a forward migration.
Stop the new executable and use the preserved old executable to restore its
old cold backup to a new directory, or use the documented same-board
`--replace` operation. This deliberately returns to snapshot state and loses
later activity; no hand-editing of SQLite or package manifests is required.

See [Upgrades](upgrades.md) and [Backup and Restore](backup-restore.md) for the
complete operational checks.

## Archive contents and licensing

The archive contains the `spitfire` executable, operator documentation,
release metadata, project licenses, `Cargo.lock`, target-specific third-party
notices, and SHA-256 manifests. Presentation and language resources are
embedded in the executable and materialized as validated board-local packages
by `spitfire setup`; they are not loose research fixtures.

Original SPITFIRE historical binaries, manuals, DISPLAY/HLP/MNU/RIP bytes,
registered/private binaries, research archives, and Synchronet material are
not release content. Original SPITFIRE NG code and project-authored
distributable resources are available under `MIT OR Apache-2.0`; external
components retain their own terms. Package-level provenance remains
authoritative when it declares another compatible license.

## Explicit future directions

The architecture can later support rights-clean third-party/community
presentation packages, including independently authored styles inspired by
other BBS traditions. This is not permission to copy their artwork or other
copyrighted resources.

A future caller preference may select among operator-approved installed
profiles, probably through the existing caller profile/preferences surface.
It must remain separate from board policy, locale, terminal encoding/
capability, menu mode, and board-owned overrides, with safe fallback when the
preference is unavailable. Version 0.1.0 does not implement that preference.
