# Contributing to SPITFIRE NG

SPITFIRE NG welcomes code, documentation, testing, preservation research, and
rights-clean presentation resources. Contributions should preserve the parts
of SPITFIRE that define its identity while modernizing DOS and hardware
limitations safely.

## Development environment

The accepted 0.1.0 build uses stable Rust 1.97.1 on Apple Silicon macOS. Newer
stable Rust releases may work, but changes should not introduce unnecessary
platform-specific behavior into the core.

Clone or download the repository, then run:

```sh
cargo build --workspace --locked
cargo test --workspace
```

The supported prebuilt package is narrower than the source portability goal.
Do not claim a platform as supported without real build and runtime evidence.

## Required checks

Before opening a pull request:

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Update documentation and focused tests when behavior changes. Parser and
storage changes need round-trip, malformed-input, and boundary tests where
applicable.

## Repository structure

```text
crates/sf-bbs/     executable, setup, operator UI, transports, runtime
crates/sf-core/    board, caller, message, file, terminal, localization core
crates/sf-legacy/  bounded readers for documented historical formats
docs/operator/     Sysop-facing installation and operation guides
docs/research/     public-safe compatibility and engineering research
release/           Development Preview package metadata and notes
research/          placeholders and policy for local historical inputs
tools/             release and display-inspection utilities
```

Board data, historical software, reverse-engineering work directories, and
private acceptance captures do not belong in Git.

## Compatibility philosophy

Use this rule when choosing between fidelity and modernization:

> Preserve behavior that is part of SPITFIRE's identity; modernize behavior
> that exists only because of DOS or historical hardware limits.

Examples of identity include command letters, security-filtered menus,
conferences, file areas, caller statistics, editable displays, and the Sysop
operating model. Examples of appropriate modernization include Argon2id
passwords, full-year dates, international profile fields, portable paths,
safe terminal input, SQLite, and reliable multinode coordination.

Do not replace a documented SPITFIRE convention merely because another BBS
uses a different design.

## Legacy parsing rules

Legacy parsers must:

- bounds-check every read;
- reject impossible lengths;
- avoid native structure casts;
- account for Turbo Pascal short strings and layout explicitly;
- preserve unknown bytes when round trips require them;
- avoid silently converting CP437; and
- use more than one sample or independent documentation before generalizing a
  format.

Historical inputs stay read-only during inspection.

## Provenance and historical materials

Do not commit:

- original or registered SPITFIRE executables;
- proprietary DISPLAY, HLP, MNU, RIP, or companion-program files;
- private caller databases, messages, contact data, or credentials;
- screenshots containing names, registered identifiers, local paths, or board
  details;
- third-party source, artwork, archives, or documentation without a verified
  compatible license; or
- generated reverse-engineering work products.

Use synthetic fixtures in tests. Record the author, rightsholder, source,
license, modifications, and redistribution status for distributable assets.
Project-authored work is licensed under MIT OR Apache-2.0; that does not
relicense historical or third-party material.

Original software and manuals are available separately through
[Original SPITFIRE Software & Documentation](https://spitfirebbs.com/).

## Code and design expectations

- Keep authentication, authorization, storage, commands, and state transitions
  engine-owned.
- Treat presentation and language as independent interfaces.
- Keep transport-specific behavior outside the core session model.
- Prefer small, documented interfaces before adding features.
- Preserve unknown legacy data instead of guessing.
- Avoid unsafe Rust unless a narrowly reviewed boundary requires it.
- Keep logs support-oriented and free of secrets or private caller fields.

Substantial architecture changes should explain the problem, historical
evidence, compatibility effect, security effect, migration impact, and test
plan before implementation.

## Issues and pull requests

Bug reports should include:

- `spitfire --version`;
- operating system and architecture;
- sanitized `spitfire status` output;
- transport and terminal client;
- terminal dimensions, encoding, profile, and menu mode; and
- exact reproduction steps.

Never post passwords, tokens, private messages, caller databases, registered
historical binaries, or unredacted personal information.

Keep pull requests focused. Explain what changed, why it belongs in SPITFIRE
NG, how it was tested, and whether compatibility or provenance documentation
changed.

Security vulnerabilities should use the private reporting process described
in [SECURITY.md](SECURITY.md), not a public issue.
