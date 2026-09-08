# Protocol participation

This kit targets CIRCUITNET-NG 1.4. Catalog-aware nodes negotiate catalog-sync;
nodes carrying the operator-access catalog also negotiate catalog-access.
Older peers retain their supported ordinary exchanges. Restricted messages do
not cross a link that cannot preserve their catalog access classification.

Live messages, subscription controls, files and catalog updates share the existing
authenticated connection and exchange schedule. Signed catalogs also support
offline import after independent authority enrollment. Local conference numbers
are never network identities.

The included technical/ specifications define framing, signatures, revision chains,
lifecycle, compatibility and resource bounds. [SECURITY](SECURITY.md) explains
what those mechanisms mean for members. [CATALOG-ADMIN](CATALOG-ADMIN.md) covers
operator commands, and [KEY-CUSTODY](KEY-CUSTODY.md) covers trust replacement.
