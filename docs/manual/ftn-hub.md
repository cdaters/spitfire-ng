# Running an FTN hub or point boss

N6 lets SPITFIRE NG serve dependent FTN systems while keeping every message in
its ordinary native message authority. A board can exchange its own mail, route
NetMail, distribute EchoMail and serve points at the same time. BinkP transports
packets; it does not own messages or subscriptions. See the [FTN setup guide](ftn-core.md),
[BinkP guide](binkp.md) and [network recovery guide](network-operations.md) first.
This guide describes implemented workflows; [M050](../research/m050-networking-n6-ftn-hub.md)
records isolated acceptance, not public FidoNet membership or onboarding.

## Configure a downstream node

1. In **sfconfig → Networks → FTN**, create its ordinary FTN link. Choose the
   domain, exact remote address and an enabled local AKA in that domain. Enable
   only the inbound/outbound directions you intend to use. Transit permission
   permits that link to submit NetMail for other systems.
2. In **BinkP links**, reference that same link ID, configure its endpoint and
   permitted AKAs, and set its BinkP credential through the write-only credential
   entry. There is one transport configuration, shared with normal FTN operation.
3. Open **FTN Hub / Downstream / Points / AreaFix / Rescan**, then **Add downstream
   relationship**. Enter the existing link ID. Leave Boss AKA empty for a node.
   Set bounds, permissions and enabled state; review and save.
4. Create ordinary EchoMail mappings for each AREA and native conference. An AREA
   mapping is local conference policy. Its Links list now contains ordinary
   peers/uplinks; configure downstream membership separately below.
5. In the Hub menu, add the downstream's subscriptions and save each reviewed
   association. Use the existing Test/Poll Link operations to verify exchange.

Turning an existing mapped peer into a downstream transfers its existing area
memberships into downstream subscriptions in the same transaction. It does not
create another conference or copy local messages. The old mapping editor cannot
also change that downstream's membership.

## Configure a point

Create the FTN and BinkP link first, using the point's real four-dimensional
address. For example, a local laboratory boss `10:100/1@lab` can serve
`10:100/1.1@lab`; these are illustrative local identities, not assigned public
addresses. The point link's local AKA must be the actual boss node in that domain.
Do not configure the point as another local AKA on its boss.

Choose **Add point relationship**, enter the point link and choose its boss AKA.
Review/save, then add subscriptions and optionally AreaFix/rescan permission just
as for another downstream. A point cannot administer its boss, siblings or other
links. Its Origin/MSGID and NetMail FMPT/TOPT retain the real point number.

Explicit direct links deliver NetMail to a configured point. Use normal next-hop
routes to send point-origin NetMail upstream. The hub does not deliver transit
mail to its own callers unless its final FTN destination is local and an explicit
native mailbox alias matches. Pointlist/directory information is supplementary;
it cannot override the configured relationship. Public pointlist publication or
automatic address allocation is not required or implemented.

## Manage subscriptions manually

Choose **Add downstream subscription** for a new association. Select the link,
domain and AREA, set Subscribed, review and save. Existing **Subscription** entries
let you toggle the same association. sfmonitor **Networks → 8 Hub** shows the
state, source (Manual/AreaFix) and revision.

Unsubscribing stops future normal fanout. Unsent, unclaimed work for that
link/area is held, not deleted. Accepted history, native conference messages and
other links are unchanged. A packet already claimed by the active AreaFix session
may finish; its offer predates the request. Manual edits during an active link
session return a conflict: wait for the session, refresh and reapply deliberately.
Resubscribing does not automatically release previously held packets or send old
history. Review those packets or request an authorized bounded rescan.

## Enable AreaFix

In the Hub menu, enable **Allow authenticated AreaFix** on the downstream, then
set its **AreaFix credential (write-only)**. This is separate from its BinkP
credential. Values are hidden, never read back, and limited to 1–71 printable
ASCII characters. Enter replaces the value; **C** on the credential entry requires
explicit CLEAR confirmation to remove it. Secret updates require an online
protected operator connection; ordinary relationship/subscription policy also
supports stopped-board configuration.

For each AREA, open **Area access / rescan policy**. Enable remote subscription
requests only for areas that downstream systems may self-subscribe to. The default
is operator-only; a sysop can still add restricted subscriptions manually. Rescan
permission is separate and must be enabled at both the area and the link.

From the downstream's NetMail editor, send private NetMail to **AreaFix** (or
**AreaMgr**) at the hub's local AKA, with the AreaFix password as the subject.
Send through that downstream's authenticated configured BinkP link. A password
and claimed From address alone are insufficient; manual packet handoffs cannot
authorize subscription changes.

The body can contain:

```text
%HELP
%LIST
%QUERY
+AREA1
-AREA2
%RESCAN AREA1 R=25
```

Commands and tags are case-insensitive. Use one command per line. `%LIST` shows
available areas and current subscriptions; `%QUERY` reports current subscriptions;
`%HELP` describes the supported forms. Rescan also accepts a bare count, such as
`%RESCAN AREA1 25`; omission uses the link's configured per-area maximum. Blank
lines and a conventional `---` tear line are supported. Wildcards, node-targeted
commands and remote credential changes are not supported.

The complete request is validated before any change. Unknown/unauthorized areas,
malformed commands, conflicting changes for one area, bounds or cooldown failures
reject the request without partial subscription changes. The response reports a
safe result and current summary; it never repeats the password or unvalidated
input. Duplicate requests reuse the original receipt and outbound response work.
After correcting a rejected request, send a new message with a new MSGID.

## Request a rescan

A downstream must be subscribed and permitted to rescan both the link and AREA.
Set per-area (1–500) and total (1–1000) bounds and a cooldown (60–86400 seconds).
At most one unfinished rescan per downstream is allowed. These are ceilings, not
promises that an area contains that many eligible messages.

Use `%RESCAN AREA count` through AreaFix, or select **Rescan / link** in sfconfig,
enter an AREA and count, then review/save. The operator action uses the same
subscription, access and resource checks; it is not a privacy bypass.

Only active public native messages already published to the exact mapped
EchoMail AREA are selected, newest first. Private NetMail, deleted messages,
unsubscribed areas and another conference's history are excluded. Rescan leaves
caller read state unchanged and preserves original identity. The recipient may
suppress messages it already has; an acknowledged rescan packet does not imply a
second local post. Rescanned traffic is marked to prevent onward distribution.

sfmonitor **8 Hub** shows requested, queued and peer-accepted counts. Completion
is derived from ordinary queue receipts. Hold, retry, restart and restore act on
that same work; they never generate a replacement rescan behind the operator's back.

## Hold, diagnose, release or disable a downstream

Set **Hold downstream exchange (keep queueing)** on its relationship to pause that
link. Other peers continue. Clear that flag to resume exchange and clear its
transport retry backoff. Explicit per-packet holds still need reviewed release.
Configuration changes during an active session return a conflict; refresh after
it completes.

In sfmonitor, inspect **Links** for authentication/retry information, **Queues**
for individual delivery state and **Hub** for subscriptions/activity. Queue details
show final destination, next hop and route reason: normal EchoMail subscription,
authorized rescan, AreaFix response or NetMail routing. Subjects and bodies are
not operator diagnostics. If A accepted while B failed, retry only B's work;
accepted A receipts remain final. Release/retry also works when a connection
failed before its packet was built and left the item pending.

For retirement, disable the downstream relationship and relevant ordinary link
policy. New distribution stops and pending work is held. Retained links,
subscriptions, point relationships and receipts cannot be silently removed or
rebound to another address. Do not delete database rows or repurpose a link ID.
A full downstream queue can refuse a new atomic fanout; resolve its hold/failure
rather than deleting history to make space.

## Restore a hub

Stop the board and use normal cold backup/restore. The backup includes native
messages, per-target queues and acknowledgements, downstream/point policy,
subscriptions, AreaFix credential files and request/rescan receipts. No live
BinkP session returns as active. Review held work using the existing verified
[N5 recovery workflow](network-operations.md).

For replacement or a new root, retain the surviving board until verified serial
floors and matching later peer acknowledgements have been reconciled. Retire that
source before resuming the restored identity. A snapshot alone cannot prove
traffic accepted after it was taken. Missing evidence stays held. The N6 journey
proved accepted A/point deliveries remain accepted while pending B resumes once;
focused tests also prove a partially accepted rescan resumes only its pending
members. Subscription source/state and point relationships restore exactly.

N6 includes no FileEcho, TIC, FREQ, hatching, file-network services, general
scheduler, public onboarding or release/service packaging. Windows live acceptance
remains deferred pending a real Windows environment.
