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


"""Independent CIRCUITNET-NG 1.4 data/framing contract; no BBS imports."""
import copy
import hashlib
import json
import re
import secrets
import struct
import unicodedata

MAX_FRAME = 4198400
CAPS = ["atomic-batch", "symmetric-poll", "directed-routing", "remote-dossier-control",
        "file-distribution", "file-hash-have", "catalog-sync", "catalog-access"]
ERRORS = set("connect tls timeout auth-failed unknown-node wrong-network topology-mismatch "
             "unsupported-version malformed-frame oversized conflicting-message "
             "unauthorized-codename custody held busy interrupted".split())

class Failure(Exception):
    """Safe finite diagnostic, never exception details or received text."""
    def __init__(self, code="malformed-frame"):
        self.code = code
        super().__init__(code)

def require(test, code="malformed-frame"):
    if not test:
        raise Failure(code)

def shape(obj, required, optional=()):
    require(type(obj) is dict and set(required) <= obj.keys()
            and obj.keys() <= set(required) | set(optional))

def integer(value, maximum, minimum=0):
    require(type(value) is int and minimum <= value <= maximum)
    return value

def token(value, kind="node"):
    require(type(value) is str)
    size = 32 if kind == "network" else 8
    require(1 <= len(value) <= size and value.isascii())
    pattern = r"[A-Za-z0-9]+" if kind == "node" else r"[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?"
    require(re.fullmatch(pattern, value) is not None)
    return value.lower() if kind == "network" else value.upper()

def hexstr(value, size):
    require(type(value) is str and re.fullmatch('[0-9a-f]{%d}' % (size * 2), value) is not None)
    return value

def new_identity(node):
    """Generate a new publication/control/message token without a local counter."""
    return token(node) + ':' + secrets.token_hex(16)

def identity(value):
    require(type(value) is str and value.count(':') == 1)
    n, h = value.split(':')
    return token(n) + ':' + hexstr(h.lower(), 16)

def text(value, size, body=False, empty=True):
    require(type(value) is str and (empty or bool(value)))
    require(len(value.encode('utf-8')) <= size)
    require(not any((unicodedata.category(c) == 'Cc' and not (body and c in '\r\n\t'))
                    or c in '\u2028\u2029\u202a\u202b\u202c\u202d\u202e\u2066\u2067\u2068\u2069'
                    for c in value))
    return value

def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result)
        result[key] = value
    return result

def loads(data, limit=MAX_FRAME):
    require(0 < len(data) <= limit, 'oversized' if len(data) > limit else 'malformed-frame')
    try:
        obj = json.loads(data.decode('utf-8'), object_pairs_hook=pairs,
                         parse_float=lambda _: (_ for _ in ()).throw(Failure()),
                         parse_constant=lambda _: (_ for _ in ()).throw(Failure()))
        def walk(x, depth=0):
            require(depth <= 64)
            if isinstance(x, str):
                x.encode('utf-8')
            elif type(x) is dict:
                for k, v in x.items():
                    walk(k, depth+1)
                    walk(v, depth+1)
            elif type(x) is list:
                for v in x:
                    walk(v, depth+1)
        walk(obj)
        return obj
    except (ValueError, UnicodeError, RecursionError, OverflowError) as e:
        raise Failure() from e

def canonical(obj):
    return json.dumps(obj, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode('utf-8')

def digest(data):
    return hashlib.sha256(data).hexdigest()

def message(obj):
    shape(obj, 'id origin codename author subject body timestamp path'.split(),
          ['reply', 'conference_identity', 'destination'])
    result = {}
    if obj.get('conference_identity') is not None:
        result['conference_identity'] = hexstr(obj['conference_identity'], 16)
    result.update(id=identity(obj['id']), origin=token(obj['origin']), codename=token(obj['codename'], 'code'),
                  author=text(obj['author'], 120, empty=False), subject=text(obj['subject'], 72),
                  body=text(obj['body'], 65536, True), timestamp=integer(obj['timestamp'], 253402300799),
                  reply=identity(obj['reply']) if obj.get('reply') is not None else None,
                  path=path(obj['path']))
    if obj.get('destination') is not None:
        result['destination'] = token(obj['destination'])
    require(len(result['author']) <= 60 and result['id'].split(':')[0] == result['origin']
            and result['reply'] != result['id'] and result['path'][0] == result['origin'])
    return result

def path(value):
    require(type(value) is list and 1 <= len(value) <= 128)
    result = [token(n) for n in value]
    require(len(set(result)) == len(result))
    return result

def batch(obj):
    shape(obj, 'format version network sender neighbor messages'.split())
    require(obj['format'] == 'circuitnet-ng-offline' and integer(obj['version'], 65535) == 1)
    require(type(obj['messages']) is list and 1 <= len(obj['messages']) <= 32)
    result = dict(format=obj['format'], version=1, network=token(obj['network'], 'network'),
                  sender=token(obj['sender']), neighbor=token(obj['neighbor']),
                  messages=[message(m) for m in obj['messages']])
    require(result['sender'] != result['neighbor'])
    require(len({m['id'] for m in result['messages']}) == len(result['messages']))
    require(all(m['path'][-1] == result['sender'] and result['neighbor'] not in m['path']
                for m in result['messages']))
    require(len(canonical(result)) <= 4194304, 'oversized')
    return result

def fingerprint(m):
    value = copy.deepcopy(m)
    value['path'] = []
    return digest(canonical(value))

def receipt(b):
    return dict(format='circuitnet-ng-offline-receipt', version=1, network=b['network'],
                sender=b['neighbor'], neighbor=b['sender'], artifact=digest(canonical(b)),
                accepted=[m['id'] for m in b['messages']])

def validate_ack(frame, offered):
    shape(frame, ['type', 'receipt', 'imported', 'duplicates'])
    require(frame['type'] == 'ack' and frame['receipt'] == receipt(offered), 'conflicting-message')
    require(integer(frame['imported'], 2**32-1) + integer(frame['duplicates'], 2**32-1)
            == len(offered['messages']), 'conflicting-message')

def control(obj):
    shape(obj, 'network id requester target operation'.split(), ['codename'])
    r = dict(network=token(obj['network'], 'network'), id=identity(obj['id']),
             requester=token(obj['requester']), target=token(obj['target']), operation=obj['operation'],
             codename=token(obj['codename'], 'code') if obj.get('codename') is not None else None)
    require(r['operation'] in ('subscribe', 'unsubscribe', 'query-subscriptions')
            and (r['operation'] == 'query-subscriptions') == (r['codename'] is None)
            and r['id'].split(':')[0] == r['requester'] and r['requester'] != r['target'])
    return r

def publication(obj):
    shape(obj, 'network id origin codename sha256 size filename description timestamp path'.split())
    require(type(obj['filename']) is str and 1 <= len(obj['filename']) <= 64
            and not obj['filename'].startswith('.')
            and re.fullmatch(r'[A-Za-z0-9._+-]+', obj['filename']) is not None)
    d = obj['description']
    require(type(d) is str and len(d.encode('utf-8')) <= 4096
            and len(d.splitlines()) <= 20
            and not any(unicodedata.category(c) == 'Cc' and c not in '\n\t' for c in d))
    r = dict(network=token(obj['network'], 'network'), id=identity(obj['id']), origin=token(obj['origin']),
             codename=token(obj['codename'], 'code'), sha256=hexstr(obj['sha256'], 32),
             size=integer(obj['size'], 67108864), filename=obj['filename'], description=d,
             timestamp=integer(obj['timestamp'], 2**63-1, -2**63), path=path(obj['path']))
    require(r['id'].split(':')[0] == r['origin'] and r['path'][0] == r['origin'])
    return r

def publication_fingerprint(p):
    value = copy.deepcopy(p)
    value['path'] = [p['origin']]
    return digest(canonical(value))

def hello(obj):
    shape(obj, 'protocol major minimum_minor maximum_minor capabilities network node role mode'.split())
    for field in ('major', 'minimum_minor', 'maximum_minor'):
        integer(obj[field], 65535)
    require(type(obj['capabilities']) is list and len(obj['capabilities']) <= 8
            and all(type(c) is str and len(c.encode('utf-8')) <= 32 for c in obj['capabilities']),
            'unsupported-version')
    require(obj['role'] in ('END', 'HOST', 'ROOT') and obj['mode'] in ('test', 'poll'))
    result = dict(obj)
    result.update(network=token(obj['network'], 'network'), node=token(obj['node']))
    return result

def negotiate(local, remote, expected_node, expected_role):
    l, r = hello(local), hello(remote)
    require(r['protocol'] == 'CIRCUITNET-NG' and r['major'] == 1
            and r['minimum_minor'] <= r['maximum_minor']
            and max(l['minimum_minor'], r['minimum_minor']) <= min(l['maximum_minor'], r['maximum_minor'])
            and set(CAPS[:2]) <= set(r['capabilities']), 'unsupported-version')
    require(l['network'] == r['network'], 'wrong-network')
    require(l['mode'] == r['mode'])
    require(r['node'] == expected_node, 'unknown-node')
    require(r['role'] == expected_role, 'topology-mismatch')
    minor = min(l['maximum_minor'], r['maximum_minor'])
    caps = set(l['capabilities']) & set(r['capabilities']) & set(CAPS)
    if minor < 4:
        caps -= {'catalog-sync', 'catalog-access'}
    if minor < 3:
        caps -= {'file-distribution', 'file-hash-have'}
    if minor < 2:
        caps -= {'directed-routing', 'remote-dossier-control'}
    if 'file-distribution' not in caps:
        caps.discard('file-hash-have')
    if 'catalog-sync' not in caps:
        caps.discard('catalog-access')
    return minor, caps

FRAME_FIELDS = {
    'hello': ['hello'], 'offer': ['batch'], 'ack': ['receipt', 'imported', 'duplicates'],
    'error': ['code'], 'close': [], 'controls': ['requests'], 'control-results': ['results'],
    'file-offer': ['publication'], 'file-want': ['want'], 'file-receipt': ['receipt'],
    'catalog-head': ['revision', 'hash'], 'catalog-request': ['revision', 'hash', 'enabled'],
    'catalog-object': ['catalog'], 'catalog-ack': ['revision', 'hash'],
}
NULLABLE = {'batch', 'publication', 'catalog', 'hash'}

def frame(obj):
    require(type(obj) is dict and type(obj.get('type')) is str and obj['type'] in FRAME_FIELDS)
    fields = FRAME_FIELDS[obj['type']]
    shape(obj, ['type'] + [f for f in fields if f not in NULLABLE], [f for f in fields if f in NULLABLE])
    result = dict(obj)
    for f in fields:
        if f in NULLABLE:
            result.setdefault(f, None)
    return result

def read_exact(stream, count):
    out = bytearray()
    while len(out) < count:
        part = stream.read(count - len(out))
        require(bool(part), 'interrupted')
        out.extend(part)
    return bytes(out)

def read_frame(stream, limit=MAX_FRAME):
    size = struct.unpack('!I', read_exact(stream, 4))[0]
    require(size != 0)
    require(size <= min(limit, MAX_FRAME), 'oversized')
    obj = frame(loads(read_exact(stream, size), limit))
    if obj['type'] == 'error':
        require(obj['code'] in ERRORS)
        raise Failure(obj['code'])
    return obj

def write_frame(stream, obj):
    data = canonical(frame(obj))
    require(len(data) <= MAX_FRAME, 'oversized')
    stream.write(struct.pack('!I', len(data)) + data)
    stream.flush()
