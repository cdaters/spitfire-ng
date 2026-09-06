# FTN / FidoNet core: isolated operation

SPITFIRE NG can now scan native messages into FTN packets and toss FTN packets
into native messages. FTN is the family of addressed store-and-forward message
networks that includes FidoNet. **BinkP is not implemented. Live public FidoNet
participation is not claimed or enabled by this procedure.** Use a disposable
board and an isolated peer. N4 will add delivery transport.

## Addresses, identities and messages

An address has a zone, net, node and point: `10:100/1.3`. Point zero is written
`10:100/1`; point 3 belongs to boss `10:100/1`. The domain identifies the network:
`10:100/1.3@synthetic`. Identical numbers in two domains are different endpoints.
An AKA is one of your board's configured local addresses. Each enabled domain
has one primary AKA and may have additional node or point AKAs.

NetMail is private addressed mail. An explicit local mailbox alias maps the
recipient at a local AKA to a native caller ID. A remote name never creates or
impersonates a caller. Unknown recipients quarantine. Transit NetMail retains its
final destination and is visible to no ordinary caller; the queue separately
records its configured next hop. There is no implicit Sysop catch-all: enroll a
`Sysop` alias at each desired AKA if that delivery is intended.

EchoMail is public conference distribution. An explicit AREA tag maps to one
existing native conference and a set of permitted links. Callers read and reply
through the normal conference interface. Unknown areas never create conferences.
Only eligible local public messages are scanned. FTN controls, SEEN-BY and PATH
are network metadata, not visible message text.

## Configure the disposable board

Use an explicitly enrolled host operator with ReadConfiguration,
ChangeOnlineConfiguration and ChangeSensitiveConfiguration for online policy
changes. NetworkStatus grants read-only status; NetworkRun grants manual work;
NetworkDirectoryActivate grants directory activation. Configuration and mapping
changes are audited without message content. Use the existing sfconfig enrollment
and [operator recovery procedure](operator-recovery.md).

`sfconfig --board <config> --apply-ftn <policy.json>` validates a complete typed
FTN policy and applies it through normal configuration version checking. Add
`--offline` only when the board is stopped and exclusively locked. The JSON is an
operator input format; the saved configuration authority remains board TOML.
This minimal surface does not include a full Networks editor or address book.

Example isolated policy (all identities are synthetic):

```json
{
  "enabled": true,
  "akas": [
    {"id":"node","endpoint":{"domain":"synthetic","address":"10:100/1"},"enabled":true,"primary":true},
    {"id":"point","endpoint":{"domain":"synthetic","address":"10:100/1.3"},"enabled":true,"primary":false}
  ],
  "links": [
    {"id":"peer","remote":{"domain":"synthetic","address":"10:100/2"},"aka":"point","enabled":true,"inbound":true,"outbound":true,"transit":false,"profile":"type2-plus","charset":"cp437"}
  ],
  "routes": [
    {"domain":"synthetic","target":{"kind":"default"},"link":"peer"}
  ],
  "sources": [
    {"id":"nodes","domain":"synthetic","enabled":true,"format":"nodelist","charset":"ascii","default_zone":10,"priority":10,"cadence_days":7,"require_crc":true},
    {"id":"points","domain":"synthetic","enabled":true,"format":"boss","charset":"cp866","default_zone":10,"priority":0,"cadence_days":7,"require_crc":false}
  ]
}
```

A link identifies an admitted packet sender and its expected destination AKA.
Area mappings may select another local AKA for outbound EchoMail; enroll that
AKA at the peer too. No socket endpoint, session password or BinkP credential is
part of this policy. A packet password must be empty for this controlled manual
profile. Host access and explicit operator/link context provide admission.

Routes are explicit: exact address, direct configured link, configured point boss,
net, zone, then domain default. A disabled matching link fails closed; it does not
silently fall through to a less specific route. There is no implicit route to an
unconfigured boss. All next hops must be configured outbound links. For transit,
also enable ingress `transit`; the selected next hop must differ from ingress.

## Manual commands before N4

Start the disposable daemon. Commands use the existing protected operator endpoint:

```text
spitfire ftn-status <config>
spitfire ftn-queue <config>
spitfire network <config> <unique-command-id> <action-json>
```

Use a new 32-hex-digit command ID for a new action. Retry an uncertain command
with its original ID and unchanged payload. FTN action envelopes have this shape:

```json
{"action":"ftn","request":{"operation":"scan"}}
```

Implemented requests are:

| Operation | Additional fields | Result / purpose |
|---|---|---|
| `mapping` | `mapping`, `expected` | Create at expected 0, update with current version; submitted mapping version is expected + 1 |
| `alias` | `alias` | Enroll AKA, remote-visible alias and stable caller ID |
| `prepare` | `link` | Create protected manual packet ingress slot |
| `toss` | `link` | Admit the completed packet in that slot |
| `scan` | none | Scan up to 1,000 eligible local conference messages |
| `build` | `queue`, `expected` | Build/reuse the immutable artifact for a versioned queue item |
| `prepare-directory` | `source` | Create the source's protected manual input slot |
| `directory-ingest` | `source`, `date` | Validate a complete edition, with explicit `YYYY-MM-DD` date |
| `directory-activate` | `generation`, `expected` | Atomically select a validated source generation |

A mapping contains `domain`, `area`, `conference_id`, `aka`, `receive`, `send`,
`origin`, `links` and `version`. An alias contains `aka`, `alias`, `caller_id`.
Configuration IDs use bounded lowercase tokens. A mapping example is:

```json
{"action":"ftn","request":{"operation":"mapping","expected":0,"mapping":{"domain":"synthetic","area":"TEST1","conference_id":2,"aka":"node","receive":true,"send":true,"origin":"Isolated Board","links":["peer"],"version":1}}}
```

After preparing, the authorized local operator places completed input at
`SYSTEM/ftn-handoff/<link-or-source>/inbound.packet`. Close the file before tossing
or ingesting. Symlinks, oversized input and arbitrary remote paths are rejected.
The daemon preserves accepted input under digest-addressed private artifact
custody. Build returns a digest; the local operator can copy exactly that
`SYSTEM/network-artifacts/<digest>` file to the isolated peer as a `.pkt`.
These are temporary acceptance mechanics, not a permanent mailer workflow.
Directory input and packet bytes must not be sent through caller commands.

Locally authored NetMail uses the authenticated native service
`send_ftn_mail(MessageActor, Policy, NewNetMail, timestamp)`. Read access uses
`read_ftn_mail`; sender enrollment is required. A broad caller network composer and
mailbox menu remain deferred. The daemon test demonstrates this service path
alongside ordinary conference authoring; operator commands cannot impersonate a
caller or supply a private message body.

## Directory, quarantine and recovery

Nodelists describe nodes; pointlists describe points under bosses. Full nodelists,
FTS-5002 Boss lists and combined Point lists are supported. Supply the source's
actual encoding; ASCII, CP437 and CP866 are explicit choices, never guessed.
Header day/CRC is checked when supplied; mandatory CRC can be required per source.
Malformed input and within-source duplicate/conflicting addresses reject the
candidate. Original bytes and safe issue classes remain retained.

Ingestion does not activate. Activation uses the source's current version (zero
before first activation). Equal-priority conflicting active sources prevent new
activation and quarantine the conflict. Lower numeric priority wins; lower-priority
observations remain traceable. Another domain never collides. Two missed source
cadences mark stale data; beyond four it is unavailable for effective lookup.
Eight retained generations per source are allowed. Capacity pressure refuses new
admission; N5 will add retention/review management. To change source parsing or
priority, configure a new source ID and explicitly activate its generation.

The small sfmonitor projection shows queued FTN work, quarantine count and active
directory count. CLI status and queue records expose protocol facts without bodies.
Quarantine is retained private artifact evidence with finite reason classes. N3
has no automatic reprocessing or broad quarantine browser: correct policy/input
and submit a new artifact; exact previously quarantined input remains receipted.

Cold [backup/restore](../sfng-backup-restore.md) preserves policy, native messages,
FTN provenance, serial high-water marks, mappings, directory generations, duplicate
history, queue decisions and artifacts. Manual handoff slots are excluded. Restore
holds unsent work and origin serial allocation because later traffic may have
existed beyond the snapshot. Do not clear holds with SQL. Reconciliation and safe
transport release belong to N4/N5; this is not an exactly-once claim across rollback.

N4 will consume these artifacts with BinkP. FileEcho/TIC/FREQ, AreaFix, general
scheduling and public participation remain outside this implementation. Real
Windows FTN acceptance is **DEFERRED — REAL WINDOWS ENVIRONMENT REQUIRED**.
See the [Technical Reference](../technical/ftn-core.md) and
[implementation report](../research/m047-networking-n3-ftn-core.md) for evidence,
limits and reproducible acceptance.
