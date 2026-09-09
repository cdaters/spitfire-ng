#!/usr/bin/env python3
# SPITFIRE NG
# Preservation-driven modern cross-platform reimplementation of
# Buffalo Creek Software's SPITFIRE Bulletin Board System
#
# Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
# Licensed under MIT OR Apache-2.0
#
# This file is part of the SPITFIRE NG project.
# See the repository documentation for architecture, provenance,
# compatibility research, security, and contribution guidelines.


"""Original harmless fixtures; credentials generated only in disposable directories."""
import copy
from datetime import datetime, timedelta, timezone
from pathlib import Path
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, ed25519
from cryptography.x509.oid import NameOID, ExtendedKeyUsageOID
import catalog
from protocol import CAPS, canonical, digest

NETWORK = 'conformance'

def certificate(root, node):
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    key = ec.generate_private_key(ec.SECP256R1())
    subject = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, node)])
    now = datetime.now(timezone.utc)
    cert = (x509.CertificateBuilder().subject_name(subject).issuer_name(subject)
            .public_key(key.public_key()).serial_number(x509.random_serial_number())
            .not_valid_before(now-timedelta(minutes=5)).not_valid_after(now+timedelta(days=2))
            .add_extension(x509.BasicConstraints(ca=False, path_length=None), True)
            .add_extension(x509.KeyUsage(True, False, False, False, False, False, False, None, None), True)
            .add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH, ExtendedKeyUsageOID.CLIENT_AUTH]), False)
            .add_extension(x509.SubjectAlternativeName([x509.DNSName(node.lower()+'.circuitnet.invalid')]), False)
            .sign(key, hashes.SHA256()))
    for suffix, data in [('pem', cert.public_bytes(serialization.Encoding.PEM)),
                         ('der', cert.public_bytes(serialization.Encoding.DER)),
                         ('key', key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption())),
                         ('key.der', key.private_bytes(serialization.Encoding.DER, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))]:
        target = root / (node+'.'+suffix)
        target.write_bytes(data)
        target.chmod(0o600)
    return root/(node+'.pem'), root/(node+'.key')

def config(root, local='USAZ001', remote='USAZ000', authority=None):
    root = Path(root)
    roles = {'USAZ000': 'HOST', 'USAZ001': 'END', 'ROOT': 'ROOT'}
    c = dict(network=NETWORK, node=local, role=roles[local], peer=remote, peer_role=roles[remote],
             topology=[dict(id='ROOT', role='ROOT', parent=None),
                       dict(id='USAZ000', role='HOST', parent='ROOT'),
                       dict(id='USAZ001', role='END', parent='USAZ000')],
             certificate=str(root/(local+'.pem')), private_key=str(root/(local+'.key')),
             peer_certificate=str(root/(remote+'.pem')), server_name=remote.lower()+'.circuitnet.invalid',
             receive=['RETRO', 'SUPPORT'], inbound_subscriptions=['RETRO', 'SUPPORT'],
             file_subscriptions=['KITDOCS'], file_policy='published', control_policy='auto-approve',
             authority=authority)
    return c

def message(node='USAZ001', serial=1, reply=None, generation=None, destination=None):
    m = dict(id=f'{node}:{serial:032x}', origin=node, codename='RETRO', author='Synthetic',
             subject='Parent' if reply is None else 'Reply', body='Public test text.\r\n',
             timestamp=1788800000, reply=reply, path=[node])
    if generation:
        m = dict(conference_identity=generation, **m)
    if destination:
        m['destination'] = destination
    return m

def batch(node='USAZ001', neighbor='USAZ000', messages=None):
    return dict(format='circuitnet-ng-offline', version=1, network=NETWORK, sender=node,
                neighbor=neighbor, messages=messages or [message(node)])

def control(serial=1, operation='subscribe', code='RETRO', requester='USAZ001', target='USAZ000'):
    return dict(network=NETWORK, id=f'{requester}:{serial:032x}', requester=requester,
                target=target, operation=operation, codename=code)

def publication(payload=b'Synthetic file\n', node='USAZ001', serial=1):
    return dict(network=NETWORK, id=f'{node}:{serial:032x}', origin=node, codename='KITDOCS',
                sha256=digest(payload), size=len(payload), filename='fixture.txt',
                description='Harmless conformance fixture', timestamp=1788800000, path=[node])

def signed(body, key):
    body = catalog.body(body)
    raw = canonical(body)
    return dict(body=body, hash=digest(raw), signature=key.sign(catalog.DOMAIN+raw).hex())

def catalogs():
    key = ed25519.Ed25519PrivateKey.generate()
    pin = dict(network=NETWORK, catalog_id='a'*32, publisher='ROOT',
               public_key=key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw).hex())
    entry = dict(id='1'*32, codename='RETRO', display_name='Retro Computing', description='Synthetic discussion',
                 category='computing', required=False, status='active', effective_revision=1,
                 retired_revision=None, historical_reference='', moderator_role='Test moderator')
    b = dict(format='circuitnet-ng-catalog', schema=1, network=NETWORK, catalog_id=pin['catalog_id'],
             revision=1, previous_revision=0, previous_hash=None, published_at=1788800000,
             publisher='ROOT', governance_reference='synthetic-approval', rationale='Conformance seed',
             intent='ordinary', entries=[entry])
    first = signed(b, key)
    b = copy.deepcopy(b)
    b.update(schema=2, revision=2, previous_revision=1, previous_hash=first['hash'], published_at=1788800001)
    operator = dict(access='sysops', **dict(entry, id='2'*32, codename='SUPPORT', required=True,
                                          display_name='Operator Support', effective_revision=2))
    b['entries'].append(operator)
    return pin, [first, signed(b, key)], key

def prepare(directory):
    """Generate local test enrollment and initial work; never production identities."""
    root=Path(directory)
    root.mkdir(parents=True,exist_ok=True)
    for node in ('ROOT','USAZ000','USAZ001','OTHER'):
        certificate(root,node)
    pin,objects,_=catalogs()
    for name,value in [('authority.json',pin),('catalogs.json',objects),
                       ('catalog1.json',objects[0]),('catalog2.json',objects[1]),
                       ('peer.json',config(root,authority=pin))]:
        (root/name).write_bytes(canonical(value)+b'\n')
    payload=b'Harmless independent payload\n'*3000
    source=root/'payload.txt';source.write_bytes(payload)
    first=message(generation='1'*32)
    reply=message(serial=2,reply=first['id'],generation='1'*32)
    work=dict(batch=batch(messages=[first,reply]),controls=[control()],
              files=[dict(publication=publication(payload),source=str(source))])
    (root/'work.json').write_bytes(canonical(work)+b'\n')

if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',required=True,type=Path)
    prepare(parser.parse_args().output)
