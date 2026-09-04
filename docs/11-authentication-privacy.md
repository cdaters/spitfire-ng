# SPITFIRE Caller, Authentication, and Privacy Model

## 1. Purpose

This document defines how modern SPITFIRE should represent callers, authenticate users, protect personal information, and retain compatibility with historical caller records.

The goal is practical security without excessive friction.

### Implemented native baseline

Stock Core Increment 2 implements the first native caller/authentication
baseline. The authoritative implementation/compatibility specification is
[Native Caller and Authentication Model](sfng-caller-authentication.md).
It records the exact SQLite schema, name normalization, Argon2id parameters,
new/existing caller flow, security/time policy, explicit fixture-Sysop setup,
and plaintext-transport warning. The remainder of this document continues to
describe longer-term authentication/privacy direction and must not be read as
claiming that every optional feature below already exists.

M042.5 now implements the minimum durable modern identity separation and SSH
password transport described by that specification: stable caller ID, unique
login identifier, public display handle, private optional real name, and one
authoritative Argon2id domain. See
[Secure SSH Caller Transport](sfng-secure-ssh-transport.md). MFA, public-key
management, web login, and network adapters remain future work.

M043 adds a separate public-information projection. Stable caller ID is used
internally for ownership/revalidation, handle is the only caller-facing
identity, and login identifier plus optional real name remain private. The
board directory and every caller default unlisted; board policy can permit but
cannot force disclosure. Disabled, Deleted, and opted-out callers appear
absent to ordinary directory/locate commands. See
[Public Information](sfng-public-information.md).

Schema 18 adds privacy-bounded operator observability. Normal operational
events may correlate a stable caller ID and may retain a public-handle snapshot
for completed-call history, but never store supplied login identifiers,
passwords or hashes, private profile details, message content or recipient
lists, file content, terminal input, remote endpoints, keys, or host paths.
Authentication failures retain only a safe reason class, transport, and
node/session correlation. Purpose-specific security audit remains separate and
cannot be removed through ordinary activity retention. See [Operator
Observability](technical/observability.md).

## 2. Caller Principle

The historical concept of a Caller remains central.

SPITFIRE should continue to use terms such as:

    Caller
    New Caller
    Returning Caller
    Sysop
    Security Level

Modern identity features should extend these concepts rather than replace them with enterprise terminology.

## 3. Caller Identity

Each caller should have a stable internal Caller ID.

Caller-visible identity may include:

    Handle
    Real Name
    Display Name

A board may eventually configure protocol-specific identity policy. Current
local presentation uses the handle, SSH authentication uses the login
identifier, and real name remains private unless explicit policy requires it.

Historical SPITFIRE behavior should be preserved where practical.

## 4. Modern Caller Record

A modern caller record may include:

    Caller ID

    Handle
    Display Name
    Real Name

    Password Credential

    Security Level
    Account Flags

    Location

    First Call
    Last Call
    Number of Calls

    Upload Statistics
    Download Statistics
    Message Statistics
    Time Statistics

    Terminal Preferences

    QWK Preferences

    Optional Email Address

    Optional MFA Information

    Legacy SPITFIRE Record Information

    Registration/Import Metadata

Not all fields should be mandatory.

## 5. Personally Identifiable Information

Historical BBS software often requested information such as:

    legal name
    telephone number
    street address
    birth date

Modern SPITFIRE should not require these merely for historical authenticity.

A Sysop may configure additional fields where appropriate.

Default account creation should request only information necessary for operation.

## 6. Recommended Default New-Caller Fields

A reasonable default may include:

    Handle or Name
    Password
    Location

Optional:

    Email Address

Additional fields may be enabled by the Sysop.

## 7. Password Policy

Passwords should be strong enough to resist trivial guessing without creating unnecessary frustration.

The default policy should favor:

    minimum length
    resistance to known/common passwords
    generous maximum length
    support for password managers
    no arbitrary composition rules

The system should not require things such as:

    one uppercase letter
    one lowercase letter
    one number
    one punctuation symbol

unless the Sysop specifically chooses such a policy.

## 8. Suggested Default

A practical default might require:

    minimum 10 characters

while accepting longer passphrases.

Very common passwords such as:

    password
    password123
    qwerty
    letmein

should be rejected.

A private or preservation installation may relax this requirement.

## 9. Password Storage

Modern passwords must never be stored as plaintext. The implemented native
credential uses Argon2id version 19 with unique operating-system-random salts
and upgradeable PHC strings; its cost parameters are validated configuration.

Passwords should be stored using a modern password-hashing algorithm suitable for credential storage.

The exact algorithm may evolve over time.

The credential system should support automatic hash upgrades when stronger parameters become appropriate.

## 10. Historical Password Migration

Legacy SPITFIRE caller records may contain password information stored using historical methods.

Where possible, migration should allow:

    Caller imports old record
            |
            v
    Caller logs in using historical password
            |
            v
    Legacy credential verified
            |
            v
    Modern password hash generated
            |
            v
    Future logins use modern credential

Direct historical caller import and legacy-password upgrade are not yet
implemented.

This permits a caller to retain an old account without indefinitely retaining an insecure authentication mechanism.

## 11. Failed Authentication

The system should protect against automated guessing without severely inconveniencing callers.

Possible measures:

- progressively increasing delay
- per-account throttling
- per-source throttling
- temporary cooldown
- logging repeated failures

Permanent account lockout should generally be avoided because it can be abused to deny service to legitimate callers.

## 12. CAPTCHA

CAPTCHA should not be required for normal terminal login.

A public web registration page may optionally use anti-automation measures if abuse becomes a problem.

The system should prefer quieter defenses such as:

- rate limiting
- honeypot fields
- registration throttling
- temporary proof-of-work or challenge systems if ever necessary

before forcing callers through irritating visual puzzles.

## 13. Multi-Factor Authentication

MFA should be available but not mandatory for ordinary callers.

Potential options:

    TOTP
    Passkeys
    Recovery Codes

A Sysop may require MFA for selected privileged accounts.

## 14. Passkeys

The web interface may support passkeys.

Passkeys may be used:

- instead of a password on the website
- as a second factor
- to authorize sensitive Sysop operations

Traditional terminal login should remain available.

## 15. Sysop Accounts

The system should distinguish between:

    BBS Sysop authority

and:

    Server administration authority

A caller with Security Level 255 may possess traditional SPITFIRE Sysop capabilities.

Server-level operations may require additional privileges.

Examples:

    changing listening ports
    managing TLS
    changing network credentials
    installing extensions
    modifying server paths
    changing administrator accounts

## 16. Security Levels

Traditional numerical SPITFIRE security levels should remain supported.

Example:

    5
    10
    20
    50
    100
    200
    255

The exact historical behavior should be preserved where known.

Modern systems may introduce additional capability flags without eliminating security levels.

## 17. Capability Flags

Some permissions may be represented separately from numeric security.

Examples:

    CanUpload
    CanDownload
    CanPost
    CanUseDoors
    CanUseQWK
    CanChat
    CanAccessSysopMenu
    CanModerate
    CanManageNetworks

This avoids forcing every authorization decision into a single number.

Historical security-level checks remain supported.

## 18. New Caller Approval

The Sysop may choose between:

### Open Registration

New callers may immediately create accounts.

### Limited New Caller

New callers receive a restricted security level until validated.

### Sysop Approval

New accounts require manual approval.

### Invitation Only

Accounts require an invitation or pre-created record.

This preserves traditional BBS operational styles.

## 19. Caller Validation

Validation methods may include:

- Sysop review
- email verification
- invitation code
- manual security-level change
- external validation extension

No single method should be mandatory.

## 20. Email Address

Email addresses should generally be optional.

A Sysop may require one for:

- password recovery
- private boards
- network communities

The BBS should not publish caller email addresses by default.

## 21. Password Recovery

If an email address is configured, web-based password recovery may be available.

Alternative recovery mechanisms may include:

- Sysop reset
- recovery codes
- passkey
- local administrative reset

Traditional terminal-only boards should not require an email system.

## 22. Account Enumeration

Public login interfaces should avoid unnecessarily revealing whether a specific account exists.

However, traditional BBS caller lists and Who's Online features may legitimately reveal caller handles.

The goal is reasonable protection, not pretending public BBS identities are secrets.

## 23. Caller Privacy Settings

Schema 14 and current schema 15 keep private profile/contact data out of public caller
projections and implements the minimum M043 policy: the board directory and
every caller default unlisted; a caller may
choose listed/unlisted; the public identity is handle; last-call is only a
board-local date when enabled; and city/region is the only optional historical
location projection. Login identifier, real name, email, phone, street/postal
address, birthday, credentials, security/subscription/JOKER state, and audit
details remain private.

The preference has its own optimistic version, and every display revalidates
current policy/lifecycle/opt-in before disclosure. See
[Public Information](sfng-public-information.md).

## 24. Legacy Data Import

A legacy caller record should retain provenance.

Example:

    Imported From:
        SPITFIRE 3.7

    Legacy Caller Number:
        127

    Original First Call:
        1994-03-12

Unknown data should be preserved where practical.

## 25. Birth Dates

Birth dates should not be mandatory by default.

If the original system contains a historical birth date, it may be:

- retained privately
- omitted during migration
- converted to month/day only
- preserved in a legacy-data field

depending on Sysop preference.

## 26. Telephone Numbers

Telephone numbers should not be required for Internet-based operation.

Historical numbers may be retained only when deliberately imported.

They should not be displayed publicly by default.

## 27. Activity Logs

Caller activity logs may record:

    login
    logout
    failed login
    connection protocol
    message posting
    file transfer
    door launch
    network action
    Sysop action

Logs should not record:

    passwords
    recovery codes
    authentication secrets
    full session cookies
    private encryption keys

## 28. IP Addresses

Internet operation necessarily exposes source addresses to the server.

SPITFIRE may log IP addresses for:

- troubleshooting
- abuse prevention
- security
- connection history

Retention should be configurable.

IP addresses should not normally be shown publicly.

## 29. Data Export

A caller may optionally be able to export personal account information through the web interface.

This is useful regardless of whether any specific privacy law requires it.

## 30. Account Deletion

Modern installations should support account deletion.

The Sysop should be able to define what happens to historical public messages.

Possible policy:

    Delete account credentials
    Anonymize profile
    Preserve public messages under historical handle

Private information should not need to remain merely because public message history is preserved.

## 31. Backup Protection

Backups contain caller information and should be treated accordingly.

The system should support:

- restricted backup permissions
- optional encryption
- clear backup locations

A backup should never be publicly web-accessible by default.

## 32. Local Installations

A Sysop running SPITFIRE privately on localhost should not be forced into Internet-oriented account requirements.

Possible local configuration:

    Password policy: relaxed
    MFA: disabled
    Email: disabled
    Web access: localhost only

This is a legitimate operating mode.

## 33. Internet Installations

A public preset may enable:

    stronger password defaults
    connection throttling
    HTTPS recommendations
    protected web administration
    secure cookies
    security logging

These should be sensible defaults rather than bureaucratic obstacles.

## 34. Guiding Principle

SPITFIRE callers should feel like callers, not corporate directory objects.

Modern authentication exists to protect their accounts and information.

It should not turn logging onto a hobby BBS into applying for a security clearance.

## 35. File inspection, request, and maintenance privacy

The M039 Tranche 5 schema-15 implementation keeps file content and maintenance state
inside purpose-specific projections. Caller listing/inspection may expose
safe catalog metadata and policy-approved public attribution, but never host
paths, staging tokens, private uploader identity, request histories, review
notes, or audit internals. Preview-area access grants inspection without
granting transfer.

Requests use stable caller/file IDs internally and are visible only to their
caller and authorized operators. Pending-review uploads are absent from every
ordinary listing/search/inspect/download path. Semantic audit records stable
IDs, versions, digests where useful, action, and result—not text/archive/DIZ
contents, paths, login identifiers, real names, contacts, credentials, or
secrets. Every privileged command reauthorizes at dispatch. See the
[Native File System](sfng-file-system.md) and
[Tranche 5 implementation report](research/m039-tranche-5-safe-file-inspection-request-maintenance-implementation.md).

## 36. Transfer, queue, and storage privacy

Schema 16 transfer authority reauthorizes every queue item before reservation
and again at dispatch. Protocol metadata cannot select a host path or bypass
staging/review. Audit records stable IDs, protocol, bounded sizes, state, and
semantic outcome—not payloads, raw frames, negotiated private metadata,
credentials, caller real names, or storage bindings. Queue ownership is
session-local and cannot cross callers or nodes.

SSH remains a caller carrier with no shell, exec, SCP, SFTP, subsystem, or
forwarding route. Independent closure signaling ensures a saturated input
queue cannot strand reservations or active-use authority after disconnect.

## 37. Operator observability and report privacy

The accepted [M039 Tranche 7
contract](research/m039-tranche-7-operator-observability-reports-gate.md)
separates retained operational history, durable security audit, read-only
reports, and generated publications. Schema 18 implements the B-017 event,
summary, retention, notification, and maintenance foundation. Ordinary
activity views use public handles where useful and stable IDs internally; they
do not expose credentials, login identifiers, private profiles, content,
terminal input, remote endpoints, or host paths.

Named Sysop, threshold Sysop, and host operator remain separate authorities.
Schema 19/B021-A now provides a protected read-only operator attachment
boundary. It uses local endpoint protection plus verified OS peer identity
mapped through a board-local host-operator allowlist; it does not reuse a
caller password, login identifier, named-Sysop identity, security threshold,
SSH host key, or command-line bearer secret. Every request is capability-
checked again at dispatch.

Unix sockets identify the peer UID. Windows named pipes use an explicit
protected DACL and obtain the peer SID from the client's Windows security
token; Administrator membership and caller BBS authority do not grant access.
All state-changing operator commands remain unimplemented. B-022 will govern
caller-facing reports and confined publication. Live screen observation
remains outside this tranche under its separate notice, redaction, consent,
and retention boundary. See [Protected Operator
Attachment](technical/operator-control.md).
