# CircuitNET NG Charter

Version 1.0 — 2026-09-08. Initial founding-network Charter.

This original modern Charter carries forward the volunteer spirit of CircuitNet
International. It is not the historical Charter, a claim to its legal succession,
or a requirement to run SPITFIRE software.

## 1. Purpose and independent boards

CircuitNET NG is a hobbyist network for useful discussion, BBS cooperation and
preservation. Participating Sysops retain control of their boards, local conference
numbers, access rules and messages. Any independently implemented BBS may participate
when it follows the published protocol and network rules. Membership does not grant
another operator access to a board or its private account records.

Each member supplies a working administrative contact, agrees an upstream HOST,
protects credentials, maintains backups, explains the Rules to callers and handles
reports about its users. SUPPORT and CHITCHAT are the initial core conferences:
one supports network operations and one supports common caller participation.
A node unable to map a core area must report the limitation and agree a remedy;
software reports compliance attention rather than creating conferences automatically.
SUPPORT, SYSOP, SPITFIRE and DOORS are restricted to Sysops and verified visiting
Sysops; the latter three are optional. Local boards enforce access and verify
visitors. Catalog presence grants no account privileges.
Other catalog areas are optional. Catalog presence never subscribes a node.

## 2. Founding administration and transition

During bootstrap, the publicly identified founding Network Administrator may approve
memberships and unique Node IDs, operate or delegate ROOT, publish approved catalog
revisions, appoint interim moderators, and maintain Charter, Rules and decision
records. A bootstrap decision states its rationale and is labeled as such in the
same public change history used by later administration. No nonexistent committee
approval is required, and bootstrap is not an invisible permanent exception.

When nine independently operated member boards have participated for 60 days, the
Administrator must announce the first committee selection within 30 days and complete
it within the following 60 days. Members may call for earlier transition by a majority
of independent participating Sysops. Publish the eligible roster, nominations,
closing date and results; contact details need not be public. If there are fewer
than seven willing candidates, fill available seats and publish a new nomination
round every 90 days until seven seats are filled. Before five seats are filled,
only documented bootstrap administration continues; do not invent a committee quorum.

The founder appoints an interim Secretary and publishes the appointment. The interim
Secretary administers the first nominations and ballot, with an uninvolved member
checking the tally where available; a candidate does not certify their own result
alone. Bootstrap authority ends when at least five elected representatives convene
the first quorate committee meeting, appoint the normal Administrator and Secretary,
and publish the dated handoff. The founder then has only the offices or permissions
conferred through the normal process. The committee appoints the Secretary from
willing members; the Secretary need not hold a committee seat.

## 3. Offices and technical roles

The Operations Committee has **seven** independently operated member representatives,
preserving historical continuity. Members elect representatives for one-year terms
through an announced electronic ballot, one vote per independent member Sysop;
operating extra nodes does not multiply that person's votes. The Secretary publishes
the tally, with individual private ballots retained only as needed for verification.
Vacancies may be filled for the remaining term by a committee majority after open
nominations. Five members constitute quorum; ordinary decisions require a majority
of votes cast at a quorate meeting. Conflicted members disclose the conflict and
abstain; postpone a non-emergency decision when an uninvolved quorum is unavailable.

If later vacancies reduce the committee below five, founding powers do not return.
The serving Administrator continues routine operations and bounded emergency safety
measures only. The Secretary announces member elections for vacant seats within
14 days and completes the ballot within 45 days, repeating nominations if needed.
During this gap there are no ordinary Charter amendments, new conference approvals
or permanent policy changes. Emergency measures require published seven-day reviews
and an independent member's review where available; restore normal committee review
at its first quorate meeting. Publish why any temporary measure must continue.
If both Administrator and Secretary are unavailable, remaining representatives name
interim record-keeping and service custodians solely to organize that recovery ballot.

The committee appoints a Network Administrator to carry out decisions and coordinate
reliable operation. **ROOT is a protocol/topology role; Network Administrator is a
human office.** They may initially share an operator. Holding ROOT credentials or
a catalog signing key does not confer unrestricted governance authority. A designated
ROOT publisher implements documented decisions; changing that designation/key requires
an announced trust-enrollment procedure, not trust in a key supplied by a packet.

HOSTs service direct children and their Dossiers. ENDs maintain their upstream and
polling arrangements. Connection direction does not determine role. Ordinary ENDs
may poll from behind NAT or firewalls without a static IP or inbound listener.

The Secretary keeps proposals, membership/Node ID records, meeting and vote results,
approval references, conference changes, office terms and amendments. Publish useful
network decisions and deliberately public contact information, not private application
data. Record keeping is a human responsibility, not an election software service.

Moderators explain conference scope, guide discussion, apply the Rules proportionately,
and work through affected Sysops. They neither own the network nor need catalog keys.
The committee appoints/replaces moderators after considering participation and conduct;
the Administrator may appoint interim moderators during bootstrap or a vacancy.
Moderators must provide reasons and an escalation contact for restrictions.

## 4. Conference governance

Any member may propose an area or change, describing its purpose, codename, audience,
moderation, required/optional status and migration effects. Allow at least 14 days
for discussion, then record approval or rejection. The committee votes under section 3;
during bootstrap the Administrator decides with a published rationale. The publisher
records the approval reference and publishes a signed catalog revision. Technical
signature verification proves publication authority, not that software counted a vote.

Conference definitions progress through Proposed, Active, Deprecated and Retired.
Deprecation announces a transition while existing delivery remains possible; new
subscriptions require Active. Retirement stops new network distribution and subscriptions
without deleting local conferences, messages, mappings or receipts. Give at least
30 days' notice for ordinary retirement, unless the published decision explains a
shorter agreed transition. Sysops may retain a local archive.

A codename is a current routing label, not permanent historical identity. Reactivation
resumes the same concept under its original immutable conference ID. Deliberate reuse
for a different concept requires an explicit approved reuse decision, rationale,
notice, a new immutable ID and retained old records. No silent alias, destructive
reuse or automatic rebinding of mappings/subscriptions is permitted. Changing a
codename normally means a new identity and a published migration decision.

## 5. Safety, disputes and inactive nodes

Address ordinary conduct problems through the originating Sysop and moderator, with
notice, a proportionate restriction and an opportunity to respond. Appeals go to
uninvolved committee members; during bootstrap publish the Administrator's response
and invite an uninvolved member to review it. A moderator cannot remove network
membership. Persistent disruption may justify a documented hold or suspension, with
scope, reason, review date and restoration conditions.

For malicious catalog metadata or a serious legal/security emergency, the Administrator
may temporarily deprecate/retire an affected area or hold a link. Publish a minimal
safe explanation within 72 hours, request committee review within seven days, and
end the temporary area measure within 14 days unless a quorate committee approves
continuation. During bootstrap, a continuation requires a new dated public decision
and an independent member's review where another member is available. This exception
cannot amend the Charter or conceal permanent reassignment of conference identities.

Agree expected polling intervals. After 30 days without contact, attempt administrative
contact and mark the node inactive if necessary; after a further 60 days without a
response, publish retirement. Holds may protect queues sooner during an outage.
Preserve Node ID/history and reconcile queued traffic before any future reassignment.
Do not charge for membership; local board costs remain each Sysop's responsibility.

## 6. Identity, privacy and security

Aliases/handles may be posting identities. Unlike the historical real-identity-in-body
rule, this initial NG Charter does not require public real-name disclosure. Sysops
remain responsible for abuse handling; network software must not append private names,
contact information or login identifiers. A future identity-policy change needs an
explicit notice and governance decision, consistent with local informed posting policy.

CircuitNET transport between configured nodes is encrypted and authenticated. Public
conference messages are not end-to-end encrypted. After delivery they are readable
according to each destination BBS's access rules. Directed routing is not private mail.
Publicly distributed files likewise remain available under local File Area access rules;
TLS does not make them confidential. Local BBS messages described as private, where
available, mean access-restricted unless an actual separate capability states otherwise.

Keep node keys outside public files, update software, enforce local file safety policy,
and notify neighbors about compromise. A remote clean-file claim is not a substitute
for required local scanning. Do not forge origin, receipts, catalog signatures or control
identity, or initiate traffic against unrelated systems.

## 7. Amendments

Publish a proposed Charter amendment for at least 30 days. After a quorate committee
recommends it, ratification requires two thirds of votes cast with at least half of
independent member Sysops participating. During bootstrap, amendments require the
same member ratification where multiple members exist, with committee recommendation
explicitly waived until the first committee takes office; a sole founder must publish
a dated rationale and place amendments before the first committee for review.
Version and retain every adopted Charter and its decision reference. Rules and routine
operating policy changes follow the ordinary proposal process. Keep administration
understandable and proportionate to a volunteer BBS community.
