# Upgrades

## Development Preview policy

There is no automatic update channel or universal compatibility promise. Keep
the old archive/binary, checksum, release metadata, and cold backup as recovery
evidence. Database migrations exist through schema 10, but native restore
deliberately is not conversion between incompatible versions.

## Safest preview upgrade

1. Stop the old board cleanly.
2. Create and validate a cold backup with the old binary.
3. Preserve the old binary or exact Git commit and `Cargo.lock`.
4. Use the old binary to restore that backup to a separate rehearsal board.
5. Install or build the proposed new version.
6. Start the rehearsal board with the new binary. Runtime startup applies
   supported forward migrations.
7. Rehearse Sysop login, messages, files/transfers, configuration, backup, and
   clean restart.
8. Only then stop and start the production board with the new binary.
9. Create a new-version backup after acceptance, while retaining the old cold
   backup for the chosen rollback window.

Use different listener ports for any rehearsal that may overlap another
board, and never run two processes against one board directory.

## Rollback

Do not attempt to downgrade a migrated database in place. Stop the new binary
and use the preserved old binary to restore the old-version backup, either to
a new directory or as an explicit same-identity replacement. This intentionally
loses state created after that snapshot.

If the new release cannot read the old backup or the old release cannot read a
new backup, that is the incompatible-version boundary behaving safely—not a
reason to edit the manifest or database.

## Files an executable upgrade preserves

An executable replacement must not overwrite board configuration, SQLite
state, cataloged bytes, messages, callers, or `<board>/display/`. Installed
presentation and language packages are board-local too. Version 0.1.0 does not include
an independent profile updater, and executable replacement does not silently
refresh existing managed packages. The public language installer refuses to
replace an existing locale. Package changes therefore require a future
explicit validated workflow and release note.

Do not hand-edit managed package contents to simulate an update. Board display
customizations belong in `display/`; see the
[Development Preview Package](development-preview-package.md#the-30-second-ownership-rule).
Configuration is validated strictly; unknown fields fail rather than being
ignored.

## Not implemented

There is no live rolling upgrade, database downgrade, cross-version restore
converter, schema skipping, automated service restart, or release-channel
updater.
