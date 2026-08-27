# SPITFIRE NG 0.1.0 Publication Checklist

This is the single operator handoff for publishing the prepared Development
Preview. It does not authorize or perform publication.

## Before uploading

1. Read `release/RELEASE-CANDIDATE-MANIFEST.md` and confirm every filename,
   hash, source commit, target, and unsigned/unnotarized label matches the
   candidate files.
2. Use the freshly initialized, sanitized `spitfire-ng` public repository with
   Issues enabled and a tested private vulnerability-reporting path. Never
   import or expose the private preservation repository's history.
3. GitHub Releases should own immutable versioned artifacts, notes, ordinary
   Issues, and private vulnerability reports; `spitfirebbs.com` provides the
   human landing/getting-started pages and links to that Release. Test both the
   included bug-report form and private vulnerability reporting from a
   non-owner account before uploading.
4. Do not upload research folders, source-tree archives, screenshots with
   private values, board backups, or any historical SPITFIRE material.

## Create the GitHub Release

Use:

- **Tag:** `v0.1.0-development-preview`
- **Title:** `SPITFIRE NG 0.1.0 Development Preview (Apple Silicon macOS)`
- **Prerelease:** yes
- **Release text:** the exact contents of
  `release/RELEASE-NOTES-0.1.0-DEVELOPMENT-PREVIEW.md`

Upload exactly:

1. `spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz`
2. `spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz.sha256`
3. `RELEASE-CANDIDATE-MANIFEST.md`

Do not rename or recompress the accepted archive after its final hash was
recorded.

## After the Release is public

1. Download both package files from the public Release into a new directory.
2. Verify the published archive SHA-256 and compare it with the release
   manifest.
3. Extract it; verify `MANIFEST.SHA256`; run `./bin/spitfire --version`.
4. On Apple Silicon macOS, complete one clean setup on loopback, start the
   board, and complete Qodem and SyncTERM login/menu/Goodbye calls.
5. Exercise RAW Text, status, cold backup, and new-root restore.
6. Confirm the Release is visibly marked **prerelease**, **Development
   Preview**, **Apple Silicon only**, **unsigned**, and **unnotarized**.
7. Confirm Issues and private vulnerability reporting work from a non-owner
   view.
8. Confirm no unexpected/private assets are downloadable.
9. Only after those checks pass, update the approved `spitfirebbs.com` pages
   through the repository → DDEV → Site Safeguard → production workflow.

## Website facts to publish afterward

Update the SPITFIRE NG landing/status, Development Preview status, download,
installation/getting-started, roadmap, and custom-display pages. State:

- SPITFIRE NG 0.1.0 Development Preview is available for Apple Silicon macOS;
- the exact archive is checksum-protected but unsigned/unnotarized;
- Stock Core and the defined ANSI/text Operator/Caller Experience parity tiers
  are accepted;
- Modern, Classic, Minimal, en-US, backup/restore, and Moebius CLR authoring
  are included as described in the release notes; and
- RIP, Category-B/ecosystem expansion, SFDraw, SFDATE, SFREG, production
  translations, SSH, and web administration are not included.

Do not say “downloadable” or post links before the public-download verification
above succeeds.
