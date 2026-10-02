#!/usr/bin/env python3
"""Separate original parser association from raw association count stages."""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
from copy import deepcopy
import json
from test_application import HERE,BY_ID,FROZEN_EXPECTED,baseline
from application import validate,require_bind_denial
def main():
    request=BY_ID['source-association-stale_parser_generation'];base=baseline('control-original-scalar-pilot');mutant=deepcopy(base)
    mutant.pop('raw');mutant.pop('raw_before_audit');mutant['reference']={};mutant['native']=[]
    mutant['audit']={k:[] for k in ('visits','count_visits','validate_visits')};mutant['audit']['usage']=None
    want=FROZEN_EXPECTED[request['id']];assert want['stage']=='resolve-project'
    mutant['diagnostics']=[{'code':want['code'],'stage':want['stage'],'primary':None,'secondary':[]}]
    active=deepcopy(base['loaded']['sources']);identity=active['files'][0]['identity']
    artifacts={'active-constructor-map.debug':active,'constructor-parser-identity.debug':[identity,identity+1,True,True,False]}
    marker={'path':'constructor.parser_generation','before':'actual-source','after':'different-source-generation'}
    result=validate(request,base,mutant,artifacts,marker);assert result['status']=='applied',result
    wrong=deepcopy(mutant);wrong['diagnostics'][0]['stage']='oir-project-bind';bad=validate(request,base,wrong,artifacts,marker);assert bad['status']=='invalid',bad
    require_bind_denial(wrong,{})
    try:require_bind_denial(mutant,{})
    except Exception as error:count_wrong_stage=repr(error)
    else:raise AssertionError('count seam accepted original-parser stage')
    report=dict(schema='unit3-original-parser-stage-sensitivity-v1',parser_resolve_project=result,parser_wrong_bind_stage=bad,count_bind_stage='ACCEPT',count_wrong_resolve_stage=count_wrong_stage,compiler_runs=0,expected_edits=0,actual_edits=0)
    (HERE/'original-parser-stage-sensitivity.json').write_text(json.dumps(report,indent=2)+'\n');print('Parser and count association stages remain distinct')
if __name__=='__main__':main()
