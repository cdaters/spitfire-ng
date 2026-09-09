# Node application field specification

<!-- public-identity:start -->
Applications Closed. Keep your form locally until applications open.

When opened, the application location is https://circuitnetng.org/apply
and the joining contact is join@circuitnetng.org.
Do not send passwords, private keys or authentication secrets.
<!-- public-identity:end -->

Network Kit 1.0. This is a form specification, not a deployed submission endpoint.
Use bounded plain text, escape rendered values, and keep submissions private.

| Field | Required | Bound / choice | Purpose |
| --- | --- | --- | --- |
| sysop_handle | yes | 80 characters | Administrative identity; legal name not required |
| contact | yes | 160 characters | Email or agreed alternative; private by default |
| bbs_name | yes | 80 characters | Board identity |
| software | yes | 120 characters | Software/version and compatibility |
| requested_role | yes | END / HOST | Service expectations |
| charter_ack | yes | Version 1.0, explicit yes | Membership agreement |
| rules_ack | yes | Version 1.0, explicit yes | Membership agreement |
| location | no | 120 characters | General region only |
| operating_system | no | 80 characters | Optional interoperability context |
| public_bbs_address | no | 256 characters | Deliberately public connection address |
| website | no | 256 characters | Optional public information |
| requested_node_id | no | 1–8 ASCII alphanumeric | Request, subject to uniqueness approval |
| upstream_host | no | 1–8 ASCII alphanumeric | Preferred parent |
| inbound_capability | no | yes / no / unknown | Connection planning, not END eligibility |
| address_families | no | IPv4 / IPv6 / both / unknown | Optional connection planning |
| description | no | 500 characters | Board summary |
| public_fields_consent | no | Explicit selection of supplied contact/address fields | Publication consent; default none |

No password, private key, token or authentication fields. No automatic confirmation
email or network call is implied. Retain only membership/operational information
needed after approval; Secretary handles correction and removal requests. Public
node information is a separately reviewed projection, never a dump of applications.

## Explicit publication consent contract

The human form supersedes the earlier free-form consent prompt. Add required
`country_name` and `region_name` strings (1-120 characters each) for assignment;
these are administrative information, not consent to publish location. The helper
matches approved names/codes; an unsupported region goes to human review.

Require `public_membership_acknowledged=true` before approval: only assigned
Node ID, BBS name and role form the minimal public membership record. No public
caller address or inbound-connect capability is mandatory. Explain that a regional
Node ID reveals its geographic assignment even when location publication is off.

`public_fields_consent` is a bounded set, default empty, selected by individual
checkboxes from: sysop_handle, location, public_bbs_address, website, software,
public_contact_email, description. Reject unknown values. A separate optional
public_contact_email (254 characters maximum) is released only with its checked
consent; never project the private review contact as a fallback. Missing optional
values remain omitted even if checked. OS, private review contact, connectivity,
certificate information and internal review notes never enter directory output.

Directory rows use Node ID, BBS name and role plus only selected optional fields.
Search may use BBS name and Node ID, or region/handle only when published. Record
consent and withdrawal date in membership records. No distributed registry or
web form service is implemented by this field specification.
