# macOS First Run and Gatekeeper

The SPITFIRE NG 0.1.0 Development Preview for Apple Silicon is not signed
with an Apple Developer ID and is not notarized. macOS may therefore block the
first launch of a copy downloaded from the Internet. A linker-generated ad-hoc
signature is not publisher signing.

## Verify before opening

Keep the archive and its adjacent checksum together. From the download
directory, verify the exact bytes before extracting or approving the program:

```sh
shasum -a 256 -c \
  spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz.sha256
tar -xzf spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin.tar.gz
cd spitfire-ng-0.1.0-development-preview-aarch64-apple-darwin
shasum -a 256 -c MANIFEST.SHA256
./bin/spitfire --version
```

The expected version is:

```text
SPITFIRE NG Bulletin Board System 0.1.0
```

Do not approve a file whose published archive checksum or internal manifest
fails.

## If macOS blocks the first launch

First attempt `./bin/spitfire --version` so macOS records the blocked launch.
If the warning says the developer cannot be verified or Apple cannot check the
software, and the checksums above passed:

1. Open **System Settings**.
2. Select **Privacy & Security**.
3. Scroll to **Security** and find the notice for `spitfire`.
4. Select **Open Anyway**, authenticate to macOS, and confirm **Open**.
5. Run `./bin/spitfire --version` again.

Apple documents this as a per-program exception. The option is normally
available for about one hour after the blocked launch. Managed Macs may prevent
the override.

Do not disable Gatekeeper or System Integrity Protection. Removing quarantine
attributes with `xattr` is not part of the supported installation workflow and
should not be necessary. If macOS says the executable is damaged or contains
malware, do not bypass that warning: delete the copy, download it again from
the official release location, and verify the checksum again.

Apple's current guidance is:

- [Open a Mac app from an unknown developer](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac)
- [Gatekeeper and runtime protection](https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web)

Developer ID signing and Apple notarization remain future release-hardening
work. No signing secret belongs in a board, archive, shell transcript, issue,
or repository file.
