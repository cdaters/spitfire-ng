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


"""Independent signed catalog verification and lifecycle; standard Ed25519 only."""
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
from protocol import Failure, canonical, digest, hexstr, integer, require, shape, text, token

DOMAIN = b'CIRCUITNET-NG-CATALOG-1\n'
BODY = 'format schema network catalog_id revision previous_revision previous_hash published_at publisher governance_reference rationale intent entries'.split()
ENTRY = 'id codename display_name description category required status effective_revision retired_revision historical_reference moderator_role'.split()

def authority(a):
    shape(a, ['network', 'catalog_id', 'publisher', 'public_key'])
    return dict(network=token(a['network'], 'network'), catalog_id=hexstr(a['catalog_id'], 16),
                publisher=token(a['publisher']), public_key=hexstr(a['public_key'], 32))

def body(obj):
    shape(obj, [f for f in BODY if f != 'previous_hash'], ['previous_hash'])
    r = {f: obj.get(f) for f in BODY}
    r['network'], r['publisher'] = token(r['network'], 'network'), token(r['publisher'])
    hexstr(r['catalog_id'], 16)
    require(r['format'] == 'circuitnet-ng-catalog' and integer(r['schema'], 65535) in (1, 2), 'invalid-catalog')
    integer(r['revision'], 2**63-1, 1)
    require(integer(r['previous_revision'], 2**64-1) == r['revision']-1, 'invalid-catalog')
    require((r['revision'] == 1) == (r['previous_hash'] is None), 'invalid-catalog')
    if r['previous_hash'] is not None:
        hexstr(r['previous_hash'], 32)
    integer(r['published_at'], 253402300799)
    text(r['governance_reference'], 160, empty=False)
    text(r['rationale'], 1024, empty=False)
    require(r['intent'] in ('ordinary', 'reactivate', 'reuse'), 'invalid-catalog')
    require(type(r['entries']) is list and len(r['entries']) <= 256, 'invalid-catalog')
    entries, codes, last = [], set(), ''
    for e in r['entries']:
        shape(e, [f for f in ENTRY if f != 'retired_revision'], ['retired_revision', 'access'])
        access = e.get('access', 'public')
        require(access in ('public', 'sysops') and (r['schema'] == 2 or access == 'public'), 'invalid-catalog')
        n = {'access': access} if access == 'sysops' else {}
        n.update((f, e.get(f)) for f in ENTRY)
        require(hexstr(n['id'], 16) > last, 'invalid-catalog')
        last = n['id']
        n['codename'] = token(n['codename'], 'code')
        for field, size, empty in [('display_name', 80, False), ('description', 1024, False),
                                   ('category', 60, False), ('historical_reference', 160, True),
                                   ('moderator_role', 80, False)]:
            text(n[field], size, empty=empty)
        require(type(n['required']) is bool and n['status'] in ('proposed', 'active', 'deprecated', 'retired'), 'invalid-catalog')
        integer(n['effective_revision'], r['revision'], 1)
        require((n['status'] == 'retired') == (n['retired_revision'] is not None), 'invalid-catalog')
        if n['retired_revision'] is not None:
            integer(n['retired_revision'], r['revision'], 1)
        if n['status'] != 'retired':
            require(n['codename'] not in codes, 'catalog-lifecycle')
            codes.add(n['codename'])
        entries.append(n)
    r['entries'] = entries
    require(len(canonical(r)) <= 524288-512, 'invalid-catalog')
    return r

def verify(obj, pin):
    shape(obj, ['body', 'hash', 'signature'])
    require(len(canonical(obj)) <= 524288, 'invalid-catalog')
    a, b = authority(pin), body(obj['body'])
    require(all(b[f] == a[f] for f in ('network', 'catalog_id', 'publisher')), 'invalid-publisher')
    raw = canonical(b)
    require(hexstr(obj['hash'], 32) == digest(raw), 'invalid-signature')
    try:
        Ed25519PublicKey.from_public_bytes(bytes.fromhex(a['public_key'])).verify(
            bytes.fromhex(hexstr(obj['signature'], 64)), DOMAIN + raw)
    except (InvalidSignature, ValueError) as e:
        raise Failure('invalid-signature') from e
    return dict(body=b, hash=obj['hash'], signature=obj['signature'])

def follows(new, old):
    b = new['body']
    if old is None:
        require(b['revision'] == 1, 'catalog-missing-revision')
        require(b['intent'] == 'ordinary' and all(e['effective_revision'] == 1 for e in b['entries'])
                and len({e['codename'] for e in b['entries']}) == len(b['entries']), 'catalog-lifecycle')
        return True
    a = old['body']
    require(b['revision'] >= a['revision'], 'catalog-rollback')
    if b['revision'] == a['revision']:
        require(new == old, 'catalog-fork')
        return False
    require(b['revision'] == a['revision']+1, 'catalog-missing-revision')
    require(b['previous_hash'] == old['hash'], 'catalog-fork')
    require(b['schema'] >= a['schema'] and b['published_at'] >= a['published_at'], 'invalid-catalog')
    lookup = {e['id']: e for e in b['entries']}
    for before in a['entries']:
        require(before['id'] in lookup, 'catalog-lifecycle')
        after = lookup[before['id']]
        require(before['codename'] == after['codename'] and
                (before == after or after['effective_revision'] == b['revision']), 'catalog-lifecycle')
        x, y = before['status'], after['status']
        valid = x == y or (x, y) in {('proposed', 'active'), ('proposed', 'retired'),
                                    ('active', 'deprecated'), ('active', 'retired'), ('deprecated', 'retired')}
        valid |= x in ('deprecated', 'retired') and y == 'active' and b['intent'] == 'reactivate'
        require(valid, 'catalog-lifecycle')
        if y == 'retired':
            expected = before['retired_revision'] if x == 'retired' else b['revision']
            require(after['retired_revision'] == expected, 'catalog-lifecycle')
    for e in b['entries']:
        if any(p['id'] == e['id'] for p in a['entries']):
            continue
        require(e['effective_revision'] == b['revision'], 'catalog-lifecycle')
        if any(p['codename'] == e['codename'] for p in a['entries']):
            require(b['intent'] == 'reuse' and all(p['status'] == 'retired' for p in b['entries']
                    if p['codename'] == e['codename'] and p['id'] != e['id']), 'catalog-lifecycle')
    return True
