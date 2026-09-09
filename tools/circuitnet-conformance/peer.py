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


"""Independent, loopback-only protocol peer with bounded synthetic custody.

Single writer; no BBS/database/production codec imports. Not a production daemon.
"""
import argparse
import copy
import ipaddress
import json
import os
from pathlib import Path
import socket
import ssl
import struct
import tempfile
import time

import catalog
from protocol import (CAPS, ERRORS, Failure, batch, canonical, control, digest, fingerprint,
                      frame, hexstr, identity, integer, loads, negotiate, publication,
                      publication_fingerprint, read_exact, read_frame, receipt, require,
                      shape, token, validate_ack, write_frame)

class Store:
    def __init__(self, directory):
        self.root = Path(directory)
        self.root.mkdir(mode=0o700, parents=True, exist_ok=True)
        require(not self.root.is_symlink(), 'custody')
        self.file = self.root / 'state.json'
        self.content = self.root / 'content'
        self.content.mkdir(mode=0o700, exist_ok=True)
        require(not self.content.is_symlink(), 'custody')
        self.data = loads(self.file.read_bytes(), 16*1024*1024) if self.file.exists() else {
            'messages': {}, 'batches': {}, 'controls': {}, 'subscriptions': [],
            'files': {}, 'catalogs': [], 'sent': {}, 'dropped_ack': False}
        self.stats = dict(messages_imported=0, duplicates=0, payload_sent=0,
                          payload_received=0, hash_have_sent=0, hash_have_received=0,
                          catalogs_received=0, catalogs_sent=0)

    def save(self, data=None):
        value = self.data if data is None else data
        require(sum(len(value[k]) for k in ('messages', 'batches', 'controls', 'files', 'sent')) <= 10000,
                'custody')
        raw = canonical(value)
        require(len(raw) <= 16*1024*1024, 'custody')
        fd, name = tempfile.mkstemp(dir=self.root, prefix='pending-')
        try:
            with os.fdopen(fd, 'wb') as out:
                out.write(raw)
                out.flush()
                os.fsync(out.fileno())
            os.replace(name, self.file)
            self.sync_directory(self.root)
        finally:
            if os.path.exists(name):
                os.unlink(name)
        self.data = value

    @staticmethod
    def sync_directory(directory):
        # Directory synchronization where supported; local filesystems only.
        if hasattr(os, 'O_DIRECTORY'):
            d = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(d)
            finally:
                os.close(d)

    def owned(self, p):
        target = self.content / p['sha256']
        if not target.is_file() or target.is_symlink() or target.stat().st_size != p['size']:
            return False
        import hashlib
        h = hashlib.sha256()
        with target.open('rb') as src:
            for chunk in iter(lambda: src.read(65536), b''):
                h.update(chunk)
        return h.hexdigest() == p['sha256']

class Peer:
    def __init__(self, config, store, work=None):
        self.c, self.store, self.work = config, store, work or {}
        for f in ('node', 'peer'):
            self.c[f] = token(self.c[f])
        self.c['network'] = token(self.c['network'], 'network')
        require(self.c['role'] in ('END', 'HOST', 'ROOT') and self.c['peer_role'] in ('END', 'HOST', 'ROOT'))
        self.nodes = {token(n['id']): n for n in self.c['topology']}
        require(1 <= len(self.nodes) <= 128 and len(self.nodes) == len(self.c['topology']))
        require(self.c['node'] != self.c['peer'])
        for name, n in self.nodes.items():
            require(n['role'] in ('END', 'HOST', 'ROOT'))
            require((n['role'] != 'ROOT' or n['parent'] is None)
                    and (n['role'] != 'END' or n['parent'] is not None))
            seen = set()
            while n['parent'] is not None:
                require(name not in seen and n['parent'] in self.nodes)
                seen.add(name)
                name = n['parent']
                n = self.nodes[name]
                require(n['role'] != 'END')
        require(sum(n['parent'] is None for n in self.nodes.values()) == 1)
        self.parent = self.nodes[self.c['node']]['parent'] == self.c['peer']
        self.child = self.nodes[self.c['peer']]['parent'] == self.c['node']
        require(self.parent or self.child, 'topology-mismatch')
        require(self.nodes[self.c['node']]['role'] == self.c['role']
                and self.nodes[self.c['peer']]['role'] == self.c['peer_role'], 'topology-mismatch')
        self.pin = catalog.authority(self.c['authority']) if self.c.get('authority') else None
        if self.pin:
            require(self.pin['network'] == self.c['network'] and self.pin['publisher'] in self.nodes
                    and self.nodes[self.pin['publisher']]['role'] == 'ROOT', 'invalid-publisher')
        binding = dict(network=self.c['network'], node=self.c['node'], peer=self.c['peer'],
                       topology=self.c['topology'], authority=self.pin)
        require(self.store.data.get('binding', binding) == binding, 'custody')
        self.store.data['binding'] = binding
        for obj in self.store.data['catalogs']:
            require(self.pin is not None, 'custody')
            catalog.verify(obj, self.pin)
        for i, obj in enumerate(self.store.data['catalogs']):
            catalog.follows(obj, self.store.data['catalogs'][i-1] if i else None)
        self.caps, self.minor = set(), 0

    def route(self, origin, target):
        require(origin in self.nodes and target in self.nodes, 'topology-mismatch')
        pending = [(origin, [origin])]
        while pending:
            node, path = pending.pop()
            if node == target:
                return path
            neighbors = [n for n in self.nodes if self.nodes[n]['parent'] == node]
            if self.nodes[node]['parent'] is not None:
                neighbors.append(self.nodes[node]['parent'])
            pending.extend((n, path+[n]) for n in neighbors if n not in path)
        raise Failure('topology-mismatch')

    def path_policy(self, obj):
        require(obj['path'] == self.route(obj['origin'], self.c['peer'])
                and self.c['node'] not in obj['path'], 'topology-mismatch')
        if obj.get('destination'):
            full = self.route(obj['origin'], obj['destination'])
            require(full[:len(obj['path'])+1] == obj['path']+[self.c['node']], 'topology-mismatch')
            # This peer proves final-node directed import, not HOST transit/fanout.
            require(obj['destination'] == self.c['node'], 'unauthorized-codename')

    def entry(self, code, generation=None, active=False):
        if not self.pin:
            return None
        require(bool(self.store.data['catalogs']), 'unauthorized-codename')
        entries = self.store.data['catalogs'][-1]['body']['entries']
        states = ('active',) if active else ('active', 'deprecated')
        found = [e for e in entries if e['codename'] == code and e['status'] in states
                 and (generation is None or e['id'] == generation)]
        require(len(found) == 1, 'unauthorized-codename')
        return found[0]

    def import_batch(self, obj):
        b = batch(obj)
        require(b['network'] == self.c['network'], 'wrong-network')
        require(b['sender'] == self.c['peer'] and b['neighbor'] == self.c['node'], 'unknown-node')
        r = receipt(b)
        data = copy.deepcopy(self.store.data)
        if r['artifact'] in data['batches']:
            self.store.stats['duplicates'] += len(b['messages'])
            return dict(type='ack', receipt=data['batches'][r['artifact']], imported=0, duplicates=len(b['messages']))
        new, duplicates = 0, 0
        for m in b['messages']:
            require(not m.get('destination') or 'directed-routing' in self.caps, 'unsupported-version')
            require(not m.get('conference_identity') or 'catalog-sync' in self.caps, 'unsupported-version')
            self.path_policy(m)
            old = data['messages'].get(m['id'])
            if old:
                require(old['fingerprint'] == fingerprint(m), 'conflicting-message')
                duplicates += 1
                continue
            require(m['codename'] in self.c['receive'], 'unauthorized-codename')
            if not m.get('destination'):
                require(m['codename'] in self.c['inbound_subscriptions'], 'unauthorized-codename')
            e = self.entry(m['codename'], m.get('conference_identity'))
            if e:
                require(m.get('conference_identity') == e['id'], 'unauthorized-codename')
                require(e.get('access', 'public') != 'sysops' or 'catalog-access' in self.caps, 'unsupported-version')
            data['messages'][m['id']] = dict(fingerprint=fingerprint(m), message=m)
            new += 1
        # Parent arrival can resolve old references: validate the complete bounded graph.
        for ident, stored in data['messages'].items():
            start = stored['message']
            seen, cursor = {ident}, start
            while cursor.get('reply') in data['messages']:
                other = data['messages'][cursor['reply']]['message']
                if (other['codename'], other.get('conference_identity')) != (start['codename'], start.get('conference_identity')):
                    break  # Retain unresolved cross-area reference; never link it.
                require(other['id'] not in seen, 'conflicting-message')
                seen.add(other['id'])
                cursor = other
        data['batches'][r['artifact']] = r
        self.store.save(data)
        self.store.stats['messages_imported'] += new
        self.store.stats['duplicates'] += duplicates
        return dict(type='ack', receipt=r, imported=new, duplicates=duplicates)

    def import_control(self, obj):
        r = control(obj)
        fp = digest(canonical(r))
        old = self.store.data['controls'].get(r['id'])
        outcome = None
        if old:
            if old['fingerprint'] == fp:
                return old
            outcome = 'replay-conflict'
        elif r['network'] != self.c['network'] or r['requester'] != self.c['peer'] or r['target'] != self.c['node'] or not self.child:
            outcome = 'unauthorized'
        elif r['codename'] is not None and r['codename'] not in self.c['receive']:
            outcome = 'unknown-codename'
        elif r['codename'] is not None and self.pin:
            try:
                self.entry(r['codename'], active=True)
            except Failure:
                outcome = 'unknown-codename'
        subs = set(self.store.data['subscriptions'])
        if outcome is None:
            if self.c.get('control_policy', 'deny') != 'auto-approve' and r['operation'] != 'query-subscriptions':
                outcome = 'denied'
            elif r['operation'] == 'subscribe':
                outcome = 'already-subscribed' if r['codename'] in subs else 'applied'
                subs.add(r['codename'])
            elif r['operation'] == 'unsubscribe':
                outcome = 'already-unsubscribed' if r['codename'] not in subs else 'applied'
                subs.discard(r['codename'])
            else:
                outcome = 'applied'
        result = dict(network=r['network'], id=r['id'], fingerprint=fp, requester=r['requester'],
                      target=r['target'], outcome=outcome,
                      subscriptions=sorted(subs) if r['operation'] == 'query-subscriptions' and outcome == 'applied' else [])
        if not old:
            data = copy.deepcopy(self.store.data)
            data['controls'][r['id']] = result
            data['subscriptions'] = sorted(subs)
            self.store.save(data)
        return result

    def receive_catalog(self, io):
        h = self.expect(io, 'catalog-head', 4096)
        self.check_head(h)
        require(self.parent or h['revision'] == 0, 'auth-failed')
        current = self.store.data['catalogs'][-1] if self.store.data['catalogs'] else None
        revision, hash_ = (current['body']['revision'], current['hash']) if current else (0, None)
        enabled = self.parent and self.pin is not None
        if enabled:
            require(h['revision'] >= revision, 'catalog-rollback')
            require(h['revision'] != revision or h['hash'] == hash_, 'catalog-fork')
        write_frame(io, dict(type='catalog-request', revision=revision, hash=hash_, enabled=enabled))
        for count in range(65):
            obj = self.expect(io, 'catalog-object', 525312)['catalog']
            if obj is None:
                return
            require(enabled and count < 64)
            signed = catalog.verify(obj, self.pin)
            require(signed['body']['revision'] <= h['revision'])
            require(signed['body']['schema'] == 1 or 'catalog-access' in self.caps, 'unsupported-version')
            if catalog.follows(signed, current):
                require(len(self.store.data['catalogs']) < 4096, 'custody')
                self.store.data['catalogs'].append(signed)
                self.store.save()
            current = signed
            self.store.stats['catalogs_received'] += 1
            write_frame(io, dict(type='catalog-ack', revision=signed['body']['revision'], hash=signed['hash']))
        raise Failure('oversized')

    @staticmethod
    def check_head(h):
        integer(h['revision'], 2**64-1)
        require((h['revision'] == 0) == (h['hash'] is None))
        if h['hash'] is not None:
            hexstr(h['hash'], 32)

    def send_catalog(self, io):
        objects = self.store.data['catalogs'] if self.child else []
        if 'catalog-access' not in self.caps:
            objects = [s for s in objects if s['body']['schema'] == 1]
        head = objects[-1] if objects else None
        revision, hash_ = (head['body']['revision'], head['hash']) if head else (0, None)
        write_frame(io, dict(type='catalog-head', revision=revision, hash=hash_))
        request = self.expect(io, 'catalog-request', 4096)
        self.check_head(request)
        require(type(request['enabled']) is bool)
        if request['enabled'] and self.child and revision:
            n = request['revision']
            require(n <= revision, 'conflicting-message')
            require(n == 0 or objects[n-1]['hash'] == request['hash'], 'conflicting-message')
            for signed in objects[n:n+64]:
                write_frame(io, dict(type='catalog-object', catalog=signed))
                ack = self.expect(io, 'catalog-ack', 4096)
                require(ack['revision'] == signed['body']['revision'] and ack['hash'] == signed['hash'], 'conflicting-message')
                self.store.stats['catalogs_sent'] += 1
        write_frame(io, dict(type='catalog-object', catalog=None))

    @staticmethod
    def expect(io, kind, bound):
        obj = read_frame(io, bound)
        require(obj['type'] == kind)
        return obj

    def send_work(self, io):
        if 'remote-dossier-control' in self.caps:
            requests = [control(r) for r in self.work.get('controls', [])]
            require(len(requests) <= 16, 'oversized')
            write_frame(io, dict(type='controls', requests=requests))
            results = self.expect(io, 'control-results', 1048576)['results']
            require(type(results) is list and len(results) == len(requests))
            for r, result in zip(requests, results):
                shape(result, 'network id fingerprint requester target outcome subscriptions'.split())
                require(all(result[k] == r[k] for k in ('network', 'id', 'requester', 'target'))
                        and result['fingerprint'] == digest(canonical(r)), 'conflicting-message')
                require(result['outcome'] in 'accepted pending-approval applied already-subscribed already-unsubscribed denied unknown-codename unauthorized malformed replay-conflict'.split())
                require(type(result['subscriptions']) is list and len(result['subscriptions']) <= 4096)
                require(result['subscriptions'] == sorted(set(token(c, 'code') for c in result['subscriptions'])))
                old = self.store.data['sent'].get('control:'+r['id'])
                if old and old['outcome'] not in ('accepted', 'pending-approval'):
                    require(result == old, 'conflicting-message')
                self.store.data['sent']['control:'+r['id']] = result
                self.store.save()
        obj = self.work.get('batch')
        b = batch(obj) if obj else None
        if b:
            require(b['sender'] == self.c['node'] and b['neighbor'] == self.c['peer'] and b['network'] == self.c['network'])
            # Work lacking peer capabilities stays with its submitting operator.
            if any((m.get('destination') and 'directed-routing' not in self.caps)
                   or (m.get('conference_identity') and 'catalog-sync' not in self.caps)
                   or (self.entry(m['codename'], m.get('conference_identity')) or {}).get('access') == 'sysops'
                   and 'catalog-access' not in self.caps for m in b['messages']):
                b = None
        write_frame(io, dict(type='offer', batch=b))
        if b:
            ack = self.expect(io, 'ack', 4096)
            validate_ack(ack, b)
            self.store.data['sent']['batch:'+ack['receipt']['artifact']] = ack
            self.store.save()
        if 'file-distribution' in self.caps:
            self.send_files(io)

    def receive_work(self, io):
        if 'remote-dossier-control' in self.caps:
            requests = self.expect(io, 'controls', 65536)['requests']
            require(type(requests) is list and len(requests) <= 16, 'oversized')
            results = [self.import_control(r) for r in requests]
            write_frame(io, dict(type='control-results', results=results))
        obj = self.expect(io, 'offer', 4198400)['batch']
        if obj is not None:
            ack = self.import_batch(obj)
            if self.c.get('drop_ack_once') and not self.store.data['dropped_ack']:
                self.store.data['dropped_ack'] = True
                self.store.save()
                raise Failure('interrupted')
            write_frame(io, ack)
        if 'file-distribution' in self.caps:
            self.receive_files(io)

    def send_files(self, io):
        if isinstance(io, TimedIO):
            io.deadline = min(io.deadline, time.monotonic()+90)
        items = self.work.get('files', [])
        require(len(items) <= 8, 'oversized')
        for item in items:
            p = publication(item['publication'])
            require(p['network'] == self.c['network'] and p['path'][-1] == self.c['node'])
            write_frame(io, dict(type='file-offer', publication=p))
            want = self.expect(io, 'file-want', 16384)['want']
            require(type(want) is dict and want.get('decision') in ('send', 'have', 'complete'))
            shape(want, ['decision', 'receipt'] if want['decision'] == 'complete' else ['decision'])
            if want['decision'] == 'send':
                with Path(item['source']).open('rb') as src:
                    remaining = p['size']
                    while remaining:
                        chunk = src.read(min(65536, remaining))
                        require(bool(chunk), 'custody')
                        io.write(struct.pack('!I', len(chunk))+chunk)
                        remaining -= len(chunk)
                        self.store.stats['payload_sent'] += len(chunk)
                    io.write(b'\0\0\0\0')
                    io.flush()
            elif want['decision'] == 'have':
                require('file-hash-have' in self.caps, 'unsupported-version')
                self.store.stats['hash_have_sent'] += 1
            r = want['receipt'] if want['decision'] == 'complete' else self.expect(io, 'file-receipt', 16384)['receipt']
            shape(r, 'publication sha256 outcome reason'.split())
            require(r['publication'] == p['id'] and r['sha256'] == p['sha256'], 'conflicting-message')
            require(r['outcome'] in ('published', 'pending-approval', 'quarantined', 'rejected')
                    and r['reason'] in ('local-policy', 'native-admission-rejected', 'hash-mismatch'))
            old = self.store.data['sent'].get('file:'+p['id'])
            require(old is None or old == r, 'conflicting-message')
            self.store.data['sent']['file:'+p['id']] = r
            self.store.save()
        write_frame(io, dict(type='file-offer', publication=None))

    def receive_files(self, io):
        if isinstance(io, TimedIO):
            io.deadline = min(io.deadline, time.monotonic()+90)
        import hashlib
        for count in range(9):
            obj = self.expect(io, 'file-offer', 16384)['publication']
            if obj is None:
                return
            require(count < 8, 'oversized')
            p = publication(obj)
            require(p['network'] == self.c['network'], 'wrong-network')
            self.path_policy(p)
            fp = publication_fingerprint(p)
            old = self.store.data['files'].get(p['id'])
            if old:
                require(old['fingerprint'] == fp, 'conflicting-message')
                write_frame(io, dict(type='file-want', want=dict(decision='complete', receipt=old['receipt'])))
                continue
            require(p['codename'] in self.c.get('file_subscriptions', []), 'unauthorized-codename')
            have = 'file-hash-have' in self.caps and self.store.owned(p)
            write_frame(io, dict(type='file-want', want=dict(decision='have' if have else 'send')))
            valid = True
            if have:
                self.store.stats['hash_have_received'] += 1
            else:
                fd, name = tempfile.mkstemp(dir=self.store.content, prefix='incoming-')
                try:
                    with os.fdopen(fd, 'wb') as out:
                        total, h = 0, hashlib.sha256()
                        while True:
                            n = struct.unpack('!I', read_exact(io, 4))[0]
                            if n == 0:
                                break
                            require(n <= 65536 and total+n <= p['size'], 'oversized')
                            chunk = read_exact(io, n)
                            out.write(chunk)
                            h.update(chunk)
                            total += n
                        valid = total == p['size'] and h.hexdigest() == p['sha256']
                        out.flush()
                        os.fsync(out.fileno())
                    self.store.stats['payload_received'] += total
                    if valid:
                        os.replace(name, self.store.content / p['sha256'])
                        self.store.sync_directory(self.store.content)
                finally:
                    if os.path.exists(name):
                        os.unlink(name)
            # This fixture's admission policy is explicit; no scan-clean claim.
            outcome = self.c.get('file_policy', 'pending-approval') if valid else 'rejected'
            require(outcome in ('published', 'pending-approval', 'quarantined', 'rejected'))
            r = dict(publication=p['id'], sha256=p['sha256'], outcome=outcome,
                     reason='local-policy' if valid else 'hash-mismatch')
            self.store.data['files'][p['id']] = dict(fingerprint=fp, publication=p, receipt=r)
            self.store.save()
            write_frame(io, dict(type='file-receipt', receipt=r))
        raise Failure('oversized')

    def session(self, io, initiator=True, mode='poll'):
        local = dict(protocol='CIRCUITNET-NG', major=1, minimum_minor=0,
                     maximum_minor=self.c.get('maximum_minor', 4), capabilities=self.c.get('capabilities', CAPS),
                     network=self.c['network'], node=self.c['node'], role=self.c['role'], mode=mode)
        if initiator:
            write_frame(io, dict(type='hello', hello=local))
        remote = self.expect(io, 'hello', 4096)['hello']
        if not initiator:
            require(remote.get('mode') in ('test', 'poll'))
            local['mode'] = remote['mode']
        self.minor, self.caps = negotiate(local, remote, self.c['peer'], self.c['peer_role'])
        if not initiator:
            write_frame(io, dict(type='hello', hello=local))
        if local['mode'] == 'poll':
            if 'catalog-sync' in self.caps:
                if initiator:
                    self.send_catalog(io)
                    self.receive_catalog(io)
                else:
                    self.receive_catalog(io)
                    self.send_catalog(io)
            if initiator:
                self.send_work(io)
                self.receive_work(io)
            else:
                self.receive_work(io)
                self.send_work(io)
        if initiator:
            write_frame(io, dict(type='close'))
            self.expect(io, 'close', 4096)
        else:
            self.expect(io, 'close', 4096)
            write_frame(io, dict(type='close'))


def tls_context(config, server):
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER if server else ssl.PROTOCOL_TLS_CLIENT)
    ctx.minimum_version = ctx.maximum_version = ssl.TLSVersion.TLSv1_3
    ctx.verify_mode = ssl.CERT_REQUIRED
    ctx.load_verify_locations(cafile=config['peer_certificate'])
    ctx.load_cert_chain(config['certificate'], config['private_key'])
    ctx.set_alpn_protocols(['circuitnet-ng/1'])
    ctx.options |= ssl.OP_NO_TICKET
    if server:
        ctx.num_tickets = 0
    # No default CA load, key logging, early data or client session reuse.
    return ctx

class TimedIO:
    def __init__(self, sock):
        self.sock = sock
        self.deadline = time.monotonic()+120
    def budget(self):
        remaining = self.deadline-time.monotonic()
        require(remaining > 0, 'timeout')
        self.sock.settimeout(min(10, remaining))
    def read(self, n):
        self.budget()
        return self.sock.recv(n)
    def write(self, data):
        self.budget()
        self.sock.sendall(data)
    def flush(self):
        pass

def run_connection(peer, port, server=False, mode='poll', ready=None):
    ctx = tls_context(peer.c, server)
    expected = ssl.PEM_cert_to_DER_cert(Path(peer.c['peer_certificate']).read_text())
    listener = None
    try:
        if server:
            listener = socket.socket()
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(('127.0.0.1', port))
            listener.listen(1)
            listener.settimeout(20)
            if ready:
                Path(ready).write_text(str(listener.getsockname()[1]))
            raw, _ = listener.accept()
        else:
            raw = socket.create_connection(('127.0.0.1', port), timeout=10)
        with raw:
            raw.settimeout(10)
            with ctx.wrap_socket(raw, server_side=server,
                                 server_hostname=None if server else peer.c['server_name']) as secure:
                require(secure.version() == 'TLSv1.3' and secure.selected_alpn_protocol() == 'circuitnet-ng/1', 'tls')
                require(secure.getpeercert(binary_form=True) == expected and not secure.session_reused, 'auth-failed')
                io = TimedIO(secure)
                try:
                    peer.session(io, not server, mode)
                except Failure as e:
                    if not (e.code == 'interrupted' and peer.c.get('drop_ack_once')):
                        try:
                            write_frame(io, dict(type='error', code=e.code if e.code in ERRORS else 'custody'))
                        except OSError:
                            pass
                    raise
                try:
                    secure.settimeout(1)
                    secure.unwrap().close()
                except OSError:
                    pass
    finally:
        if listener:
            listener.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', required=True, type=Path)
    parser.add_argument('--state', required=True, type=Path)
    parser.add_argument('--work', type=Path)
    parser.add_argument('--seed', type=Path, help='Explicit signed history array import; no key enrollment')
    parser.add_argument('--listen', action='store_true')
    parser.add_argument('--sessions', type=int, default=1, help='1–3 bounded listener attempts for lost-ACK tests')
    parser.add_argument('--port', type=int, default=0)
    parser.add_argument('--mode', choices=['test', 'poll'], default='poll')
    parser.add_argument('--ready', type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    args.state.mkdir(mode=0o700, parents=True, exist_ok=True)
    lock = args.state / 'writer.lock'
    acquired = False
    result = dict(format='circuitnet-ng-peer-result', version=1, protocol='1.4', result='custody')
    try:
        lock.mkdir()
        acquired = True
        config = loads(args.config.read_bytes(), 65536)
        work = loads(args.work.read_bytes(), 4198400) if args.work else None
        peer = Peer(config, Store(args.state), work)
        if args.seed:
            for obj in loads(args.seed.read_bytes(), 16*1024*1024):
                signed = catalog.verify(obj, peer.pin)
                old = peer.store.data['catalogs'][-1] if peer.store.data['catalogs'] else None
                if catalog.follows(signed, old):
                    peer.store.data['catalogs'].append(signed)
                    peer.store.save()
        else:
            integer(args.port, 65535, 0 if args.listen else 1)
            integer(args.sessions, 3, 1)
            require(args.listen or args.sessions == 1)
            for attempt in range(args.sessions):
                try:
                    run_connection(peer, args.port, args.listen, args.mode, args.ready)
                except Failure as e:
                    if e.code != 'interrupted' or attempt+1 == args.sessions:
                        raise
        result.update(result='ok', negotiated_minor=peer.minor, capabilities=sorted(peer.caps), **peer.store.stats)
    except Failure as e:
        result['result'] = e.code
    except (ssl.SSLError, OSError, ValueError, KeyError, TypeError) as e:
        result['result'] = 'tls' if isinstance(e, ssl.SSLError) else 'custody'
    finally:
        if acquired:
            lock.rmdir()
        args.report.write_bytes(canonical(result)+b'\n')
    return 0 if result['result'] == 'ok' else 1

if __name__ == '__main__':
    raise SystemExit(main())
