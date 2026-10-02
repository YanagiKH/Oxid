#!/usr/bin/env python3
"""Freeze full source/native collection paths without running a compiler."""
from common import *
from collect import PLAN,validate,native_module
import argparse

def make(kind,materialization,prepared,wrapper,debug_build,release_build,receipt_root,build_wrapper=None):
    require(kind in ('source','native'),'unsupported plan kind');material=pathlib.Path(materialization).resolve();prepared=pathlib.Path(prepared).resolve();destination=pathlib.Path(receipt_root).resolve()
    require(not destination.exists(),'fresh future receipt root required')
    plan={'schema':PLAN,'kind':kind,'scope':'full','materialization':bound_file(material),'builds':{},'rows':[]}
    for profile,build in (('debug',debug_build),('release',release_build)):
        key=kind+'-'+profile;binding={'family':kind,'profile':profile,'build':bound_file(build),'prepared':bound_file(prepared),'wrapper':bound_file(wrapper)}
        if kind=='native':require(build_wrapper is not None,'native build wrapper required');binding['build_wrapper']=bound_file(build_wrapper)
        plan['builds'][key]=binding
    if kind=='source':
        requests=[json.loads(line)for line in(material.parent/'requests.jsonl').read_text().splitlines()]
        for profile in ('debug','release'):
            for request in requests:
                location=destination/profile/request['id'];plan['rows'].append({'id':request['id'],'profile':profile,'build':kind+'-'+profile,'receipt':str(location/'receipt.json'),'wrapper_receipt':str(location)+'.portable.json'})
    else:
        metadata=read_json(prepared);component=prepared.parent/relative(metadata['native_component_directory']);group=native_module()
        for profile in ('debug','release'):
            for index,row in enumerate(group.roster(component)):
                request=row['request'];suffix=row['id']+'--'+row['operation']+'--'+row['format']+(f"--fuel-{row['fuel']}"if row['fuel']is not None else'')+(f"--{row['noclobber']}"if row['noclobber']else'')
                location=component/'receipts'/row['group']/profile/suffix;root=material.parent/relative(request['source_root'])
                plan['rows'].append({k:row[k]for k in ('id','group','operation','format','fuel','noclobber')}|{'profile':profile,'build':kind+'-'+profile,'receipt':str(location/'receipt.json'),'wrapper_receipt':str(destination/profile/f'{index:03d}.json'),'source_root':str(root),'entry':str(root/relative(request['entry'])),'output':str(location/('occupied-output'if row['noclobber']else'program'))if row['operation']=='compile'else None,'input_request_sha256':hashlib.sha256(json.dumps(request,sort_keys=True).encode()).hexdigest()})
    return validate(plan)

def main():
    p=argparse.ArgumentParser()
    for name in ('kind','materialization','prepared','wrapper','debug-build','release-build','receipt-root','output'):p.add_argument('--'+name,required=True)
    p.add_argument('--build-wrapper');a=p.parse_args();output=pathlib.Path(a.output).resolve();require(not output.exists(),'plan output exists')
    plan=make(a.kind,a.materialization,a.prepared,a.wrapper,a.debug_build,a.release_build,a.receipt_root,a.build_wrapper);output.parent.mkdir(parents=True,exist_ok=True);write_json(output,plan)
    print(json.dumps({'status':'source-only-plan-frozen','kind':plan['kind'],'scope':plan['scope'],'rows':len(plan['rows']),'plan':bound_file(output)}))
if __name__=='__main__':main()
