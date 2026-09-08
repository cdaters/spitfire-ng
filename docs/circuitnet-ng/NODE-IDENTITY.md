# Enroll a node certificate

A node certificate identifies one BBS on a live CircuitNET link. Its private key
is separate from the catalog signing key. Your HOST enrolls your public certificate
and you enroll theirs. An application form is not a credential exchange.

This example uses OpenSSL to create a self-signed certificate suitable for explicit
peer enrollment. Agree the server name and validity period with your neighbors.
The example name is for MYBBS and must be replaced for your assigned node.

On a protected local working directory, use a restrictive file-creation mask:

```
umask 077
```

Write `certificate.conf`:

```
[req]
prompt=no
distinguished_name=dn
x509_extensions=ext
[dn]
CN=MYBBS
[ext]
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=serverAuth,clientAuth
subjectAltName=DNS:mybbs.circuitnet.invalid
```

Create and convert the key; these commands never send it anywhere:

```
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 \
  -nodes -days 365 -outform DER -config certificate.conf \
  -keyout identity.key -out identity.der
openssl pkcs8 -topk8 -nocrypt -outform DER -in identity.key \
  -out identity-private.der
openssl x509 -inform DER -in identity.der -noout -fingerprint \
  -sha256
```

Protect both private files with owner-only permissions or equivalent OS ACLs.
Store them outside public File Areas and web roots. Share only identity.der and
its fingerprint. Compare the peer fingerprint by an independent agreed channel.
The live enrollment command copies the necessary identity into board custody;
protect board data and backups too. Keep the source private key only in protected
custody or remove the extra copy under your local retention policy.

Arrange replacement before certificate expiry. Hold affected exchanges, verify
the new public certificate with each direct neighbor, update peer enrollment on
stopped boards and rerun Test Link. Do not silently accept an unexpected key.
See [END-NODE](END-NODE.md) for enrollment commands and [SECURITY](SECURITY.md).
