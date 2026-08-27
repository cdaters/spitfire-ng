# Language Packages

SPITFIRE NG presentation and language are separate. Selecting
`classic-spitfire` changes visual resources; selecting `en-US` changes
engine-owned prompts and generated-menu labels. Neither setting changes
commands, security, authentication, or stored caller data.

Clean setup installs `en-US` 1.0.1 under
`SYSTEM/language-packs/en-US/` and writes:

```toml
[language]
default_locale = "en-US"
```

With the board stopped, validate and install another independently licensed
package through the public workflow:

```sh
spitfire language-validate /absolute/path/to/language-packs/es-ES
spitfire language-install /absolute/path/to/board/spitfire.toml /absolute/path/to/language-packs/es-ES
spitfire config /absolute/path/to/board/spitfire.toml
spitfire status /absolute/path/to/board/spitfire.toml
```

The installer never replaces an existing locale. `status` must show the
intended default/effective locale, package version, and `READY`. A missing or
invalid pack produces bounded `DEGRADED` issues and safe en-US/emergency ASCII
fallback rather than raw keys or corrupted text.

Cold backup/restore includes the locale selection, manifest, catalogs,
licenses, and provenance automatically. Never edit package hashes manually on
a live board. See [Localization Contract](../localization.md) for format,
translation, fallback, encoding, and security requirements.
