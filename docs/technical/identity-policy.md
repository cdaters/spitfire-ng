# Account names and posting identity

Schema 28 implements the coordinated identity architecture. Authentication,
public handles, private name components, historical author text and network
sender bytes have separate authority. This contract governs all new local posts;
network imports continue to preserve an external assertion.

## Account authority

`caller_id` is stable ownership. `login_identifier` is the existing unique,
normalized authentication Login; `display_name` is the public **Handle**, with its existing
unique case-insensitive lookup. Handle admission remains 1–30 printable ASCII
bytes, collapsed ASCII whitespace, and the existing reserved-name/JOKER rules.
The typed operator rename changes login/handle only when explicitly requested.
A profile name edit cannot change either. D2 uses Login for authentication on interactive caller transports and SSH.
Handle remains public presentation identity, never an implicit login fallback.

`CallerProfile.identity: PrivateIdentity` owns nullable `first_name` and
`last_name`. Each is at most 60 UTF-8 bytes. Unicode space separators normalize
to one ASCII space and edge spaces disappear; case and punctuation are retained.
Controls, line/paragraph separators and bidi override/isolate characters are
rejected. There is no case folding, transliteration, NFC conversion, uniqueness
constraint or claim of verified legal identity. The joined form is at most
120 UTF-8 bytes and 60 Unicode scalar values. Both components must be present
for `real_name()` to return `First + space + Last`.

The old `callers.real_name` remains read-only, unclassified compatibility data.
It is not a fallback, derived value, posting input or authentication identifier.
New accounts leave it NULL. Schema 13 historically copied handles into it; schema
28 preserves every value exactly and leaves **all** new components NULL, including
unequal legacy values. No arbitrary handle/full-name splitting is defensible.
An authorized caller/operator must explicitly supply the components.

## Configuration and resolver

Only two policy values are implemented:

| Value | Behavior |
| --- | --- |
| `handle-allowed` / `HandleAllowed` | Use the public handle for native conference posts. |
| `real-name-required` / `RealNameRequired` | Require both components and use the derived name. |

`caller.posting_identity` in canonical TOML is the board fallback and defaults to
`handle-allowed`. `caller.profile.require_names` defaults false and controls
collection; it does not itself publish names. A conference's nullable
`posting_identity` overrides the fallback. FTN area mappings and configured FTN
links, and QWK mappings/links, carry typed requirements with handle defaults.
Only enabled outbound destinations participate.

The resolver takes the conference override or board default, then joins every
applicable mapping and link requirement. Real-name-required dominates that join.
Source reporting prefers a link hard requirement, then a mapping, then conference,
then board. Multiple destinations resolve **one strongest posting identity**, shown
locally and sent to all destinations; there is no hidden export substitution.
A handle-friendly conference mapped to a required-name area therefore previews a
real-name post. No duplicate conference is needed.

The host binds the canonical configuration snapshot to each database operation.
Caller sessions follow the existing new-session configuration effect; network
operations bind current configuration and revalidate before release. Relational
conference/mapping changes are checked within the posting transaction. The core
does not create a second database copy of TOML defaults.

Preferred and network-defined modes are deferred. A future adapter must publish
bounded typed requirements to the resolver, not obtain arbitrary profile access.
The existing account components can later support separate header and body
publication requirements. No CircuitNET enum, disclosure line or codec is added.

## Preview, submission and historical authority

`PostingIdentityPreview` is a core-minted, non-deserializable capability containing
the resolved name and evidence. The caller sees `Posting as` before the body and
again at the existing Save question. Incomplete required names fail before the
editor. Names unrepresentable by the selected terminal or destination encoding
also fail before submission; no replacement or truncation creates another sender.

The immediate posting transaction re-resolves and compares account state version,
conference policy version, board revision, destination scope/version, policy,
mode and resolved name. A stale preview cancels the save rather than silently
changing attribution. Legacy API callers may omit a preview only for handle mode;
real-name publication always requires an explicit preview.

`messages.author_name` is the immutable **posted-as** string and already was a
snapshot before schema 28. `author_caller_id` remains optional stable ownership;
`identity_mode` records handle, real-name, enrolled-network-alias, external-asserted,
system or legacy-unclassified. `identity_proof` records resolver version, policy,
configuration/destination evidence and account state version, not private
components. SQLite rejects mutations of this author/evidence tuple.

Handle/component edits never rewrite old authors. Message rendering does not join
current account names. Replies to a native parent use its account reference for
recipient authorization, while retaining the historical displayed author.
Copy preserves the source author/evidence and rejects an insufficient stricter
identity requirement. Copy is not permission to publish to a newly mapped network.

Deletion is the existing recoverable tombstone, not hard row removal. Deleted or
disabled authors still render from the snapshot; queued local work fails active
account checks until legitimate recovery. Existing foreign keys continue to protect
ownership. This milestone does not add destructive account purge.

UTF-8 terminals display admitted Unicode names directly. CP437 terminals use exact
encoding. Historical names containing unrepresentable characters show explicit
`[U+XXXX]` code points in headers; stored names and outbound bytes never change.
The independent terminal reference review adopted strict representability and
separate encoding authority; no terminal negotiation or transport changed.

## Network sender custody

`network_sender_snapshots` records queue ID, message ID, posted sender, exact wire
sender bytes/profile, mode and proof. It is append-only, including delete guards.
New FTN queue admission and QWK queue/build admission freeze from immutable
message/envelope authority. Later profile edits are irrelevant. Cached artifacts,
retries and final handoffs revalidate policy, destination and lifecycle authority
before release; they never repair an invalid sender after packet creation.

New local proof binds the approved destination scopes. Adding a destination later
holds old work rather than treating the earlier preview as permission for a new
network. A stricter current rule holds handle/legacy work. Relaxation does not
rename a prior real-name post. Exact bytes/profile must match the frozen record.
The default FTN link identity field is omitted from serialized policy so its
schema-27 fingerprint remains byte-for-byte stable. Required-name settings are
explicitly serialized and change that fingerprint. Operator forms still expose
the default field. Migration leaves existing queue decisions and artifacts unchanged; on first
validation it records their existing immutable sender. Legacy evidence cannot
satisfy a newly required real-name policy.

FTN packet semantics remain unchanged. Native CP437 EchoMail sender admission is
at most 35 encoded bytes; private mail uses the selected FTN charset. **FTN does
not universally require real names.** Requirements come from configured board,
network/link or area policy. The retained technical evidence did not establish a
universal real-name-header rule.

Private FTN/QWK mail retains explicit mailbox enrollment. A required derived real
name must also match that enrollment; no automatic alias enrollment, recipient
association, route creation or disclosure occurs. Public conference composition
and private-mail core APIs have separate existing surfaces; this milestone adds
private-mail preview APIs, not a new private-mail terminal menu.

QWK offline packets use stored authors. CONTROL/ALIAS caller metadata stays handle
based; TOREADER advertises H only for effective handle areas. Uploaded REP From is
untrusted. Each required-name reply must receive explicit caller review before
native submission; a noninteractive importer rejects it. Durable receipt replay
never republishes or reprompts. QWK/DOVE network exports retain their existing
profile, metadata and path rules while checking the same sender evidence.

Imported FTN/QWK authors remain literal external assertions with no local author
account. A matching handle or first/last name does not link an account. Origin,
partner, publication and route evidence stay separate. Local account requirements
cannot verify an external person's name; forwarding preserves imported assertions
under existing network admission/routing authority, without inventing local proof.

## Privacy and recovery

First/last components are visible only in the caller's own profile/preview and
explicitly authorized local operator profile editing. Publishing the resolved
name is intentional only in a required-name post. Ordinary caller lists, online
nodes, statistics, sfmonitor, configuration, queue summaries, notifications and
operational events receive no components. PrivateIdentity and caller/preview Debug
projections redact private values. There is no ordinary public real-name directory
or new visibility preference.

Historical display controls that extract a first token from the public Handle
retain that meaning; they do not start reading the new private First Name field.

Name edits use an account-version compare-and-swap transaction and append-only
`caller_name_events`: stable account ID, old/new versions, time and actor kind.
Neither values, hashes nor before/after text enter audit. Credentials and login
lookup are unchanged. Backup contents remain sensitive account data; generic
backup metadata does not list names.

Migration 28 is transactional: component columns, policy columns/versions, immutable
author evidence, sender snapshots and name-event journal. Existing conferences
inherit the handle board default; mappings/links default handle. Existing author
text is untouched, classified legacy-unclassified except proven external-origin,
accountless messages. No speculative CircuitNET fields are added. A failed
migration rolls back both schema and data. Cold backup/restore retains the complete
database, configuration and artifact store, with the existing exact-schema checks.

## Verification map

`sf-core::identity::tests` covers normalization, missing components, preview races,
privacy, handle/name edits, history, lifecycle and immutable SQL authority.
`database::identity_migration_tests` covers schema 27, equal/unequal legacy names,
queued work, history and injected migration rollback. FTN identity tests decode
actual packets after profile changes and revalidate cached artifacts. QWK tests
cover native-author override, required review/replay, network sender freeze and
new-destination holds. `sf-bbs/tests/identity.rs` runs a disposable native daemon
with a synthetic caller, three conferences, isolated FTN artifact export, profile
changes, restart and cold restore. No public-network traffic is required.

See [caller management](../operator/caller-management.md),
[message administration](../operator/messages.md),
[FTN](ftn-core.md), [QWK](qwk-networking.md), and
[backup/restore](../operator/backup-restore.md).

## CircuitNET C2 extension in schema 29

The [CircuitNET service](circuitnet.md) contributes a UTF-8, network/codename
destination scope and mapping/profile revision to the same native posting preview.
Its initial test profile requires HandleAllowed; stronger board/conference policy
continues to apply. Scanner admission requires that destination in the immutable
posting proof. Queue admission freezes exact UTF-8 author bytes through the shared
sender snapshot authority. Export revalidates lifecycle, scope and policy without
reading private components. Imported authors remain external assertions, with no
local account matching. No historical real-name-body disclosure is automated.
