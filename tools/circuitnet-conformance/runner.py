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


"""Run independent conformance tests and emit a bounded machine-readable report."""
import argparse
from datetime import datetime, timezone
import io
import json
from pathlib import Path
import sys
import unittest

class Results(unittest.TextTestResult):
    def startTest(self, test):
        super().startTest(test)
        self.records.append(dict(name=test.id(), outcome='running'))
    def addSuccess(self,test):
        super().addSuccess(test);self.records[-1]['outcome']='pass'
    def addFailure(self,test,err):
        super().addFailure(test,err);self.records[-1]['outcome']='fail'
    def addError(self,test,err):
        super().addError(test,err);self.records[-1]['outcome']='error'
    def addSkip(self,test,reason):
        super().addSkip(test,reason);self.records[-1]['outcome']='skip'
    def addSubTest(self,test,subtest,err):
        super().addSubTest(test,subtest,err)
        if err:self.records[-1]['outcome']='fail'
    def __init__(self,*args,**kwargs):
        super().__init__(*args,**kwargs);self.records=[]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report',type=Path,required=True)
    args=parser.parse_args()
    suite=unittest.defaultTestLoader.discover(str(Path(__file__).parent),pattern='test_conformance.py')
    result=unittest.TextTestRunner(stream=sys.stderr,verbosity=2,resultclass=Results).run(suite)
    report=dict(format='circuitnet-ng-conformance-report',version=1,protocol='1.4',
                implementation='Independent Python reference peer',
                profiles=['Core','Routing: directed final-node only','Control: fixed test policy','Files: hash-have','Catalog: sync/access'],
                date=datetime.now(timezone.utc).isoformat(),tests=result.records,
                passed=sum(r['outcome']=='pass' for r in result.records),
                failed=sum(r['outcome'] in ('fail','error') for r in result.records),
                skipped=sum(r['outcome']=='skip' for r in result.records),
                limitations=['Not a full HOST/ROOT forwarder','No BBS UI or production scanner',
                             'Native SPITFIRE interoperability is a separate Cargo campaign; not inferred here'])
    args.report.write_text(json.dumps(report,indent=2)+'\n')
    return 0 if result.wasSuccessful() else 1

if __name__=='__main__':
    raise SystemExit(main())
