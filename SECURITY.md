# SPITFIRE NG Security Policy and Development Principles

SPITFIRE NG is a hobbyist BBS and preservation project. It should be safe to place on the modern Internet without turning ordinary BBS use into an enterprise-security obstacle course.

The security goal is:

> **Sane defaults, low friction, strong boundaries.**

## Scope

This document describes development, deployment, and Development Preview
reporting principles. Private vulnerability reporting is enabled for this
public repository; use the **Report a vulnerability** action on the
[Security Advisories page](https://github.com/cdaters/spitfire-ng/security/advisories).

## Development Preview release integrity

The published 0.1.0 Development Preview is an Apple Silicon macOS archive that
is not signed with an Apple Developer ID and is not notarized. Verify its
adjacent SHA-256 file before extraction and its internal `MANIFEST.SHA256`
after extraction. The release builder rejects unsafe archive paths and the
verifier rejects symlinks or missing required license/notice files. SHA-256
detects accidental or malicious byte changes but is not publisher
authentication; obtain the archive and checksum from the same official,
versioned Release and compare the hash with the release manifest.
The builder remaps local source paths and the verifier rejects private build-
home paths in both package text and the executable.

Language and presentation packages remain untrusted input and retain their
existing strict path, inventory, hash, size, syntax, provenance, and
compatibility validation.

## General Principles

SPITFIRE NG should:

- protect caller credentials and personally identifiable information;
- treat all network input as untrusted;
- treat all historical binary formats as untrusted input;
- isolate legacy DOS software from the BBS core;
- provide practical authentication without excessive friction;
- preserve traditional protocols where they remain important to BBS culture;
- clearly identify security tradeoffs instead of silently removing compatibility.

## Traditional Protocols

### Telnet

Telnet should remain supported because traditional BBS clients rely on it.

However:

- Telnet is unencrypted;
- public deployments should make that fact visible;
- SSH and HTTPS/WSS should be available as secure alternatives;
- highly sensitive server-administration operations should not depend solely on a Telnet session.

Telnet support is a compatibility feature, not a statement that plaintext transport is modern security best practice.

### SSH

SSH should be supported as a secure terminal transport where practical.

### Raw TCP and RLogin

Raw TCP and RLogin are plaintext compatibility transports, like Telnet. The
optional SyncTERM/Synchronet RLogin auto-login convention carries a password
inside the RLogin handshake and therefore defaults off. It must use the same
SPITFIRE caller verifier as interactive login; RLogin usernames, host/source
trust, and Unix identity never grant caller authority. Restrict plaintext
auto-login to a trusted LAN, localhost, VPN, or equivalent controlled path.

### Web Terminal

The embedded browser terminal should connect through a dedicated WebSocket transport into the SPITFIRE session engine.

It should not simply proxy the public Telnet port.

Internet-facing browser terminal connections should normally use HTTPS/WSS.

## Caller Authentication

Ordinary callers should be able to use a traditional:

```text
Name / Handle
Password
```

login.

Defaults should favor:

- reasonable minimum password length;
- rejection of obviously weak/common passwords;
- generous maximum length;
- password-manager compatibility;
- no unnecessary mandatory punctuation/capitalization rules.

Optional features may include:

- TOTP;
- passkeys;
- recovery codes.

MFA should not be required for ordinary callers by default.

## Sysop and Administrative Authentication

Traditional SPITFIRE Sysop authority and host/server administration should be distinct concepts.

A Security Level 255 caller may have full traditional BBS authority without automatically receiving:

- operating-system access;
- TLS private keys;
- network private keys;
- server credential management;
- extension installation;
- unrestricted filesystem access.

Web-based sensitive administration may use stronger authentication than ordinary caller login.

## Password Storage

Modern credentials must never be stored as plaintext.

Use an established password-hashing algorithm intended for credential storage.

The Increment 2 native implementation uses Argon2id version 19, a unique
operating-system-random salt, validated configurable costs, and an upgradeable
PHC representation. The exact implemented policy is documented in
`docs/sfng-caller-authentication.md`.

Legacy password formats may be accepted temporarily during migration, but a successful legacy login should be upgraded to a modern credential whenever practical.

## Personally Identifiable Information

Native address, phone, email, and full birth date are optional historical
caller-profile data, disabled by default, and private by default. The Sysop may
configure each group as disabled, optional, or required, but ordinary caller
lists, node status, message headers, and unrelated display contexts must not
expose them. Full birth date is stored as an unambiguous four-digit-year date
and is never an authentication secret. A future public profile requires an
explicit consent model separate from the private caller record.

Do not require historical fields solely because old BBS software did.

Fields such as:

- telephone number;
- street address;
- legal name;
- full birth date;

should normally be optional.

Imported historical information should not become publicly visible by default.

## Legacy Formats

All parsers must assume malformed or hostile input.

Examples include:

- SPITFIRE message records;
- QWK/REP;
- FidoNet packets;
- CircuitNet packets;
- ANSI/RIP resources;
- uploaded archives;
- door drop files;
- historical configuration files.

Parsers should:

- bounds-check every read;
- reject impossible lengths;
- avoid unchecked pointer arithmetic;
- prevent integer overflow;
- avoid unsafe binary structure casts;
- preserve unknown legacy fields where practical;
- fail gracefully rather than crash the server.

## Archive Processing

QWK, CircuitNet, uploads, and other archives may contain malicious paths or pathological compression.

Archive processing should defend against:

- `../` traversal;
- absolute paths;
- symlink escape;
- excessive decompressed size;
- excessive file count;
- overwrite of system files.

## DOS Doors

Original DOS doors are inherently legacy software and should not execute with the privileges of the SPITFIRE server.

Prefer an isolated runtime with:

- a session-specific working directory;
- only required drop files;
- explicit persistent game directories;
- no automatic access to caller/message databases;
- no network credentials;
- no TLS/SSH keys;
- restricted host filesystem access;
- configurable network access.

A door crash must not crash SPITFIRE.

## CircuitNet

Historical CircuitNet packet and routing semantics may be preserved.

Historical administrative trust assumptions must not become modern security boundaries.

In particular, a message `From` field alone must never authorize modern remote administrative changes.

A revived secure CircuitNet may use authenticated node identities while preserving historical addressing, conferences, routing, dossiers, and packet compatibility.

## Web Security

Internet-facing web features should use established protections including:

- HTTPS;
- secure cookies;
- CSRF protection where relevant;
- origin checking for WebSockets;
- output escaping;
- parameterized queries;
- authentication throttling;
- conservative Content Security Policy;
- secure session identifiers.

The default configuration should make the safe path easy.

## Local and Preservation Modes

SPITFIRE NG must remain usable offline and on localhost.

A local preservation installation should not require:

- TLS certificates;
- public DNS;
- cloud identity;
- MFA;
- external services.

Security requirements should be proportional to deployment context.

## Logging

Logs may record:

- login success/failure;
- caller sessions;
- network connections;
- door execution;
- message-network activity;
- administrative changes;
- errors.

Logs must not record:

- passwords;
- private keys;
- authentication tokens;
- recovery codes;
- full session secrets.

IP-address retention should be configurable.

## Memory Safety

New Internet-facing parsers and server code should prefer memory-safe implementation.

Rust is the preferred language for the core and protocol-processing components unless a strong compatibility reason dictates otherwise.

Unsafe Rust should be rare, documented, and isolated.

## Dependencies

Prefer dependencies that are:

- actively maintained;
- portable;
- narrowly scoped;
- widely reviewed;
- appropriate for long-lived software.

Avoid adding large frameworks for minor convenience.

## Fuzzing and Tests

Binary and network parsers should receive:

- malformed-input tests;
- boundary tests;
- regression tests;
- fuzz testing where practical.

Every security-relevant bug should receive a regression test when feasible.

## Secrets

Network passwords, API tokens, private keys, and other secrets should be isolated from ordinary configuration where practical.

Secrets must never be passed to DOS doors or exposed through normal BBS display macros.

## Security Presets

A future setup system may provide profiles such as:

### Local / Preservation

For offline experimentation and historical research.

### Private Network

For trusted LAN/VPN operation.

### Internet BBS

For publicly exposed systems with recommended protections enabled.

These are convenience presets, not separate editions.

## Reporting Security Issues

Do not publish exploit details for an unpatched vulnerability in a public
issue tracker. Use the private vulnerability-reporting channel identified on
the official SPITFIRE NG release/download page. Include the affected version,
impact, safe reproduction conditions, and a contact method; do not send live
board credentials, private caller databases, or registered historical
binaries.

Private vulnerability reporting is enabled and the 0.1.0 publication gate is
satisfied. Future releases remain blocked if that channel becomes unavailable
or untested. Ordinary non-security bugs use the public issue tracker and
follow `docs/operator/support.md`.

## Final Principle

SPITFIRE NG should be secure enough that a Sysop can reasonably put it online, while remaining simple enough that retro-computing enthusiasts actually want to run it.
