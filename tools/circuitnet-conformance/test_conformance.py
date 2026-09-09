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


"""Bounded conformance tests using only the independent peer and public vectors."""
import ast
import copy
import io
import json
from pathlib import Path
import random
import socket
import struct
import tempfile
import threading
import time
import unittest

import catalog
import fixtures as f
import protocol as p
from peer import Peer, Store, run_connection

ROOT = Path(__file__).resolve().parent

class Conformance(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.valid = p.loads((ROOT/'vectors/valid.json').read_bytes())
    def tearDown(self):
        self.temp.cleanup()
    def peer(self, local='USAZ000', remote='USAZ001', pin=None, work=None):
        peer = Peer(f.config(self.root, local, remote, pin), Store(self.root/local), work)
        peer.caps = set(p.CAPS)
        return peer
    def fails(self, code, func, *args):
        with self.assertRaises(p.Failure) as caught:
            func(*args)
        self.assertEqual(caught.exception.code, code)
    def pair(self, client, server, mode='poll'):
        for n in ('USAZ000', 'USAZ001'):
            if not (self.root/(n+'.pem')).exists():
                f.certificate(self.root,n)
        ready=self.root/'ready'
        ready.unlink(missing_ok=True)
        errors=[]
        def serve():
            try:
                run_connection(server,0,True,ready=ready)
            except Exception as e:
                errors.append(e)
        t=threading.Thread(target=serve)
        t.start()
        end=time.monotonic()+5
        while not ready.exists() and time.monotonic()<end and t.is_alive():
            time.sleep(.01)
        self.assertTrue(ready.exists(),errors)
        try:
            run_connection(client,int(ready.read_text()),False,mode)
        finally:
            t.join(12)
            self.assertFalse(t.is_alive(),'bounded session did not finish')
        if errors:
            raise errors[0]

    def test_golden_messages_controls_files_canonical(self):
        v=self.valid
        b=p.batch(v['batch'])
        self.assertEqual(p.canonical(b).hex(),v['batch_canonical_hex'])
        self.assertEqual(p.digest(p.canonical(b)),v['batch_hash'])
        self.assertEqual([p.fingerprint(m) for m in b['messages']],v['message_fingerprints'])
        self.assertEqual(p.receipt(b),v['receipt'])
        self.assertEqual(p.digest(p.canonical(p.batch(v['directed']))),v['directed_hash'])
        self.assertEqual([p.digest(p.canonical(p.control(r))) for r in v['controls']],v['control_fingerprints'])
        self.assertEqual(p.publication_fingerprint(p.publication(v['publication'])),v['publication_fingerprint'])
    def test_golden_catalog_signature_access(self):
        last=None
        for signed,raw in zip(self.valid['catalogs'],self.valid['catalog_canonical_hex']):
            s=catalog.verify(signed,self.valid['authority'])
            self.assertEqual(p.canonical(s['body']).hex(),raw)
            self.assertTrue(catalog.follows(s,last))
            last=s
        e=last['body']['entries'][1]
        self.assertEqual(e['access'],'sysops')
        self.assertTrue(e['required'])
        self.assertNotIn('9999',p.canonical(last).decode())
    def test_malformed_vectors(self):
        for case in p.loads((ROOT/'vectors/malformed.json').read_bytes()):
            with self.subTest(case=case['name']):
                def decode():
                    obj=p.read_frame(io.BytesIO(bytes.fromhex(case['wire_hex'])))
                    if obj['type']=='hello':
                        p.hello(obj['hello'])
                self.fails(case['expected'],decode)
    def test_node_network_normalization_and_invalid_tokens(self):
        for n in ['A','ROOT','END00001','USAZ001','CNETROOT']:
            self.assertEqual(p.token(n.lower()),n)
        self.assertEqual(p.token('CONFORMANCE','network'),'conformance')
        self.assertEqual(p.identity('end1:'+'A'*32),'END1:'+'a'*32)
        generated={p.new_identity('usaz001') for _ in range(100)}
        self.assertEqual(len(generated),100)
        self.assertTrue(all(p.identity(n)==n and n.startswith('USAZ001:') for n in generated))
        for n in ['', '123456789', 'a/b', 'a.b','é',' a','A:1']:
            self.fails('malformed-frame',p.token,n)
    def test_negotiation_capabilities_versions(self):
        h=self.valid['hello'];r=dict(h,node='USAZ000',role='HOST')
        minor,caps=p.negotiate(h,r,'USAZ000','HOST')
        self.assertEqual((minor,caps),(4,set(p.CAPS)))
        r['capabilities']=p.CAPS[:2]+['future-optional']
        self.assertEqual(p.negotiate(h,r,'USAZ000','HOST')[1],set(p.CAPS[:2]))
        r['maximum_minor']=1
        self.assertEqual(p.negotiate(h,r,'USAZ000','HOST')[0],1)
        r['major']=2
        self.fails('unsupported-version',p.negotiate,h,r,'USAZ000','HOST')
        r.update(major=1,network='wrong')
        self.fails('wrong-network',p.negotiate,h,r,'USAZ000','HOST')
        r['network']=h['network']
        self.fails('unknown-node',p.negotiate,h,r,'OTHER','HOST')
        self.fails('topology-mismatch',p.negotiate,h,r,'USAZ000','END')
    def test_atomic_replay_conflict_restart(self):
        peer=self.peer();b=f.batch()
        ack=peer.import_batch(b)
        p.validate_ack(ack,p.batch(b))
        self.assertEqual(ack['imported'],1)
        restarted=self.peer()
        self.assertEqual(restarted.import_batch(b)['duplicates'],1)
        conflict=copy.deepcopy(b['messages'][0]);conflict['body']='Conflict'
        mixed=f.batch(messages=[f.message(serial=2),conflict])
        self.fails('conflicting-message',restarted.import_batch,mixed)
        self.assertEqual(len(restarted.store.data['messages']),1)
        forged=copy.deepcopy(ack);forged['receipt']['sender']='USAZ001'
        self.fails('conflicting-message',p.validate_ack,forged,p.batch(b))
    def test_thread_late_parent_and_cycle_atomic(self):
        peer=self.peer()
        a=f.message(serial=1,reply='USAZ001:'+f'{2:032x}')
        peer.import_batch(f.batch(messages=[a]))
        b=f.message(serial=2,reply=a['id'])
        self.fails('conflicting-message',peer.import_batch,f.batch(messages=[b]))
        b['reply']=None
        peer.import_batch(f.batch(messages=[b]))
        self.assertEqual(peer.store.data['messages'][a['id']]['message']['reply'],b['id'])
    def test_path_directed_and_capability_admission(self):
        peer=self.peer()
        m=f.message(destination='USAZ000')
        peer.c['inbound_subscriptions']=[]
        self.assertEqual(peer.import_batch(f.batch(messages=[m]))['imported'],1)
        bad=f.message(serial=2,destination='ROOT')
        self.fails('unauthorized-codename',peer.import_batch,f.batch(messages=[bad]))
        bad['destination']='USAZ000';peer.caps.discard('directed-routing')
        self.fails('unsupported-version',peer.import_batch,f.batch(messages=[bad]))
        bad.pop('destination');bad['path']=['USAZ001','ROOT']
        self.fails('malformed-frame',peer.import_batch,f.batch(messages=[bad]))
    def test_controls_idempotency_unauthorized_unknown(self):
        peer=self.peer()
        r=f.control()
        self.assertEqual(peer.import_control(r)['outcome'],'applied')
        self.assertEqual(peer.import_control(r),peer.import_control(r))
        self.assertEqual(peer.import_control(dict(r,codename='SUPPORT'))['outcome'],'replay-conflict')
        self.assertEqual(peer.import_control(f.control(2,'unsubscribe'))['outcome'],'applied')
        self.assertEqual(peer.import_control(f.control(3,code='UNKNOWN'))['outcome'],'unknown-codename')
        self.assertEqual(peer.import_control(f.control(4,requester='ROOT'))['outcome'],'unauthorized')
        self.assertEqual(peer.import_control(f.control(5,'query-subscriptions',None))['subscriptions'],[])
    def test_catalog_bad_signature_wrong_signer_rollback_fork(self):
        pin,objects,key=f.catalogs()
        first,second=objects
        self.fails('catalog-rollback',catalog.follows,first,second)
        self.assertFalse(catalog.follows(second,second))
        alternate=copy.deepcopy(second);alternate['body']['rationale']='Other decision'
        alternate=f.signed(alternate['body'],key)
        catalog.verify(alternate,pin)
        self.fails('catalog-fork',catalog.follows,alternate,second)
        bad=copy.deepcopy(second);bad['signature']='0'*128
        self.fails('invalid-signature',catalog.verify,bad,pin)
        another,_,_=f.catalogs()
        self.fails('invalid-signature',catalog.verify,second,another)
        self.fails('invalid-publisher',catalog.verify,second,dict(pin,publisher='OTHER'))
        self.fails('invalid-publisher',self.peer,'USAZ000','USAZ001',dict(pin,publisher='USAZ001'))
        bad=copy.deepcopy(second);bad['body']['previous_hash']='0'*64
        bad=f.signed(bad['body'],key)
        self.fails('catalog-fork',catalog.follows,bad,first)
    def test_catalog_lifecycle_retirement_reactivation_reuse(self):
        pin,objects,key=f.catalogs();last=objects[-1]
        def next_body():
            b=copy.deepcopy(last['body']);b.update(revision=b['revision']+1,previous_revision=b['revision'],previous_hash=last['hash'])
            return b
        b=next_body();b['entries'][0].update(status='retired',effective_revision=3,retired_revision=3)
        retired=f.signed(b,key);self.assertTrue(catalog.follows(retired,last));last=retired
        b=next_body();b['entries'][0].update(status='active',effective_revision=4,retired_revision=None)
        ordinary=f.signed(b,key);self.fails('catalog-lifecycle',catalog.follows,ordinary,last)
        b['intent']='reactivate';reactivated=f.signed(b,key)
        self.assertTrue(catalog.follows(reactivated,last))
        self.assertEqual(reactivated['body']['entries'][0]['id'],retired['body']['entries'][0]['id'])
        b=next_body();new=dict(b['entries'][0],id='3'*32,status='active',effective_revision=4,retired_revision=None)
        b['entries'].append(new)
        self.fails('catalog-lifecycle',catalog.follows,f.signed(b,key),last)
        b['intent']='reuse'
        self.assertTrue(catalog.follows(f.signed(b,key),last))
    def test_malformed_publication_and_text_bounds(self):
        for update in [dict(filename='../x'),dict(sha256='A'*64),dict(size=67108865),dict(path=['USAZ001','USAZ001'])]:
            self.fails('malformed-frame',p.publication,dict(f.publication(),**update))
        for update in [dict(body='x'*65537),dict(author='\x1b'),dict(body='\u202e'),dict(timestamp=-1)]:
            self.fails('malformed-frame',p.message,dict(f.message(),**update))
    def test_bounded_random_frame_parser(self):
        rng=random.Random(903)
        for i in range(500):
            data=rng.randbytes(rng.randrange(0,300))
            if i % 2:
                data=struct.pack('!I',len(data))+data
            try:
                p.read_frame(io.BytesIO(data))
            except p.Failure:
                pass
        for _ in range(100):
            m=f.message(serial=rng.randrange(1,2**120))
            m['body']=''.join(chr(rng.randrange(32,127)) for _ in range(rng.randrange(0,256)))
            b=p.batch(f.batch(messages=[m]))
            self.assertEqual(p.batch(p.loads(p.canonical(b))),b)
    def test_independence_import_boundary(self):
        allowed={'argparse','ast','catalog','copy','cryptography','datetime','fixtures','hashlib','io','ipaddress','json','os','pathlib','peer','protocol','re','random','secrets','socket','ssl','struct','sys','tempfile','threading','time','unittest','unicodedata'}
        for file in ROOT.glob('*.py'):
            tree=ast.parse(file.read_text())
            for node in ast.walk(tree):
                names=[]
                if isinstance(node,ast.Import): names=[n.name.split('.')[0] for n in node.names]
                if isinstance(node,ast.ImportFrom): names=[node.module.split('.')[0]]
                self.assertTrue(set(names)<=allowed,(file.name,names))
                if isinstance(node,ast.Call) and isinstance(node.func,ast.Name):
                    self.assertNotIn(node.func.id,('eval','exec','__import__'))
    def test_tls_core_both_directions_and_graceful_close(self):
        c=self.peer('USAZ001','USAZ000',work={'batch':f.batch()})
        s=self.peer(work={'batch':f.batch('USAZ000','USAZ001',[f.message('USAZ000')])})
        self.pair(c,s)
        self.assertEqual(len(c.store.data['messages']),1)
        self.assertEqual(len(s.store.data['messages']),1)
        self.assertEqual(c.minor,4)
    def test_tls_catalog_files_hash_have_and_replay(self):
        pin,objects,_=f.catalogs()
        c=self.peer('USAZ001','USAZ000',pin)
        s=self.peer(pin=pin)
        s.store.data['catalogs']=objects;s.store.save()
        payload=b'Harmless bytes\n'*5000
        source=self.root/'payload';source.write_bytes(payload)
        s.work={'files':[dict(publication=f.publication(payload,'USAZ000'),source=str(source))]}
        self.pair(c,s)
        self.assertEqual(len(c.store.data['catalogs']),2)
        self.assertEqual(c.store.stats['payload_received'],len(payload))
        s.work['files'][0]['publication']['id']='USAZ000:'+f'{2:032x}'
        self.pair(c,s)
        self.assertEqual(c.store.stats['payload_received'],len(payload))
        self.assertEqual(c.store.stats['hash_have_received'],1)
        self.pair(c,s)
        self.assertEqual(len(c.store.data['files']),2)
    def test_tls_wrong_enrolled_node_and_profile(self):
        c=self.peer('USAZ001','USAZ000');s=self.peer()
        c.c['node']='OTHER'
        with self.assertRaises(p.Failure): self.pair(c,s)
        c.c['node']='USAZ001';c.c['network']='wrong'
        with self.assertRaises(p.Failure): self.pair(c,s)
    def test_tls_wrong_certificate_rejected(self):
        import ssl
        c=self.peer('USAZ001','USAZ000');s=self.peer()
        for node in ('USAZ001','USAZ000','OTHER'): f.certificate(self.root,node)
        c.c['certificate']=str(self.root/'OTHER.pem');c.c['private_key']=str(self.root/'OTHER.key')
        with self.assertRaises((ssl.SSLError,p.Failure)): self.pair(c,s)
    def test_tls_lost_ack_retry_durable(self):
        c=self.peer('USAZ001','USAZ000',work={'batch':f.batch()});s=self.peer()
        s.c['drop_ack_once']=True
        with self.assertRaises(p.Failure): self.pair(c,s)
        restarted=self.peer();restarted.c['drop_ack_once']=True
        self.pair(c,restarted)
        self.assertEqual(len(restarted.store.data['messages']),1)
        self.assertEqual(next(iter(c.store.data['sent'].values()))['duplicates'],1)
    def test_tls_minor_one_unknown_optional_test_mode(self):
        c=self.peer('USAZ001','USAZ000');s=self.peer()
        c.c.update(maximum_minor=1,capabilities=p.CAPS[:2]+['future-optional'])
        self.pair(c,s,'test')
        self.assertEqual(c.minor,1)
        self.assertEqual(c.store.data['messages'],{})

    def test_binary_stream_integrity_bounds_truncation_and_replay(self):
        class Duplex:
            def __init__(self,data): self.input=io.BytesIO(data);self.output=io.BytesIO()
            def read(self,n): return self.input.read(n)
            def write(self,data): return self.output.write(data)
            def flush(self): pass
        def wire(obj):
            out=io.BytesIO();p.write_frame(out,obj);return out.getvalue()
        peer=self.peer()
        pub=f.publication(b'good')
        prefix=wire(dict(type='file-offer',publication=pub))
        ending=wire(dict(type='file-offer',publication=None))
        bad=Duplex(prefix+struct.pack('!I',4)+b'oops'+b'\0'*4+ending)
        peer.receive_files(bad)
        r=peer.store.data['files'][pub['id']]['receipt']
        self.assertEqual((r['outcome'],r['reason']),('rejected','hash-mismatch'))
        repeat=Duplex(prefix+ending);peer.receive_files(repeat)
        result=p.read_frame(io.BytesIO(repeat.output.getvalue()))
        self.assertEqual(result['want']['decision'],'complete')
        pub['id']='USAZ001:'+f'{2:032x}';prefix=wire(dict(type='file-offer',publication=pub))
        self.fails('oversized',peer.receive_files,Duplex(prefix+struct.pack('!I',65537)))
        self.fails('interrupted',peer.receive_files,Duplex(prefix+struct.pack('!I',4)+b'a'))
        self.assertNotIn(pub['id'],peer.store.data['files'])
        self.assertEqual(list(peer.store.content.iterdir()),[])
    def test_tls_catalog_rollback_head_fails_closed(self):
        pin,objects,_=f.catalogs()
        c=self.peer('USAZ001','USAZ000',pin);s=self.peer(pin=pin)
        s.store.data['catalogs']=objects;s.store.save();self.pair(c,s)
        s.store.data['catalogs']=objects[:1]
        with self.assertRaises(p.Failure): self.pair(c,s)
        self.assertEqual(len(c.store.data['catalogs']),2)

    def test_public_semantic_vectors(self):
        for case in p.loads((ROOT/'vectors/semantic.json').read_bytes()):
            with self.subTest(case=case['name']):
                def run():
                    op=case['operation']
                    if op=='negotiate':
                        minor,caps=p.negotiate(self.valid['hello'],case['remote'],'USAZ000','HOST')
                        self.assertEqual(minor,case['minor']);self.assertEqual(sorted(caps),case['capabilities'])
                    elif op=='catalog-verify':
                        catalog.verify(case['catalog'],self.valid['authority'])
                    elif op=='catalog-follows':
                        catalog.follows(case['catalog'],case['previous'])
                    elif op=='batch-replay':
                        peer=self.peer();peer.import_batch(case['batch'])
                        result=peer.import_batch(case['batch'])
                        self.assertEqual((result['imported'],result['duplicates']),(case['imported'],case['duplicates']))
                    else:
                        self.fail('unknown vector operation')
                if case['expected']=='ok': run()
                else: self.fails(case['expected'],run)

    def test_terminal_control_result_cannot_regress(self):
        parent=self.peer();request=f.control();result=parent.import_control(request)
        client=self.peer('USAZ001','USAZ000',work={'controls':[request]})
        client.store.data['sent']['control:'+request['id']]=result;client.store.save()
        changed=dict(result,outcome='pending-approval')
        incoming=io.BytesIO();p.write_frame(incoming,dict(type='control-results',results=[changed]))
        incoming.seek(0)
        class Duplex:
            def read(self,n):return incoming.read(n)
            def write(self,data):pass
            def flush(self):pass
        self.fails('conflicting-message',client.send_work,Duplex())
        self.assertEqual(client.store.data['sent']['control:'+request['id']],result)

if __name__=='__main__':
    unittest.main()
