#!/usr/bin/env python3
"""Corrupt only cloned transport envelopes; retain actual artifacts unchanged."""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
from pathlib import Path
import copy,hashlib,json,tempfile
from compare_mutations import compare_envelope,load,PACKAGE,ROOT,HERE

def main():
    path=ROOT/'typed-project-unit3-mutations/smoke-v1/debug/correspondence-wrong-target-scalar/receipt.json'
    original=json.loads(path.read_text());cases=load();expected={e['id']:e for e in json.loads((PACKAGE/'mutation-expectations.json').read_text())['mutations']}
    controls=[]
    def control(name,edit,child_edit=None):
        e=copy.deepcopy(original);edit(e)
        with tempfile.TemporaryDirectory(prefix='envelope-control-',dir=HERE) as td:
            if child_edit is not None:
                child=json.loads(Path(e['mutant_receipt']['path']).read_text());child_edit(child)
                cp=Path(td)/'child-receipt.json';cp.write_text(json.dumps(child));data=cp.read_bytes()
                e['mutant_receipt']=dict(path=str(cp),bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
            p=Path(td)/'receipt.json';p.write_text(json.dumps(e))
            try:compare_envelope(p,cases,expected)
            except Exception as error:controls.append(dict(id=name,status='REJECTED',reason=repr(error)))
            else:raise AssertionError(('envelope corruption escaped',name))
    control('changed-frozen-request',lambda e:e['request']['mutation'].__setitem__('replacement','a different helper'))
    control('changed-profile',lambda e:e.__setitem__('profile','release'))
    control('changed-controller-pin',lambda e:e['mutation_controller'].__setitem__('sha256','0'*64))
    control('changed-marker-before',lambda e:e['mutation_applied'].__setitem__('before',999999))
    control('changed-marker-path',lambda e:e['mutation_applied'].__setitem__('path','functions[9].blocks[0].terminator.target'))
    control('changed-parsed-artifact',lambda e:e['mutation_artifacts']['raw-verifier-start.debug'].__setitem__('value','owned'))
    control('changed-observation-hash',lambda e:e['mutant_observations'].__setitem__('sha256','0'*64))
    control('child-build-not-envelope-build',lambda e:None,lambda c:c['build_receipt'].__setitem__('sha256','1'*64))
    control('child-observer-not-envelope-observer',lambda e:None,lambda c:c['observer_controller'].__setitem__('sha256','2'*64))
    control('child-source-operations-not-frozen',lambda e:None,lambda c:c['source_request'].__setitem__('operations',['check']))
    control('child-binary-not-envelope-binary',lambda e:None,lambda c:c['argv'].__setitem__(0,'/unrelated/observer'))
    report=dict(schema='unit3-mutation-envelope-sensitivity-v1',input=dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest()),controls=controls,compiler_invocations=0,expected_edits=0,actual_artifact_edits=0)
    (HERE/'envelope-sensitivity.json').write_text(json.dumps(report,indent=2)+'\n');print('Rejected envelope corruptions',len(controls))
if __name__=='__main__':main()
