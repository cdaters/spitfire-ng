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
