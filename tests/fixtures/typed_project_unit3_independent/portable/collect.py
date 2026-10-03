#!/usr/bin/env python3
"""Execute an explicitly frozen source-only plan through reviewed wrappers."""
from common import *
import argparse,subprocess

PLAN='unit3-independent-portable-plan-v1'
REQUESTS='f405bf46d8945027cd1d79d3c125692b4c00b8fe57b97c0d55c68b83b9795113'
WRAPPERS={'source':'14dfc7142d5ce044afe60caaa85b538c3ae8fdcf21878256f2e1d8a44f7d5fb2','native':'49e857b57b0cf589d4fdf667b4f0b689340e3357122135fa0a4e8786c28245dd'}
def physical(path):
    require(isinstance(path,str)and pathlib.Path(path).is_absolute()and str(pathlib.Path(path).resolve())==path,'absolute physical path required');return pathlib.Path(path)
def binding(item):
    require(set(item)=={'path','bytes','sha256'},'file identity fields');verify(physical(item['path']),item)
def native_module():
    helper=pathlib.Path(__file__).parent/'native-v1/native_groups.py';require(identity(helper)['sha256']=='cd4a952a3cf72b1abd3f6c146e25fe103ed43eeccc070abf467b56126b7b93bf','native selector changed')
    import importlib.util
    spec=importlib.util.spec_from_file_location('unit3_plan_native_groups',helper);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module
def validate(plan):
    require(set(plan)=={'schema','kind','scope','materialization','builds','rows'}and plan['schema']==PLAN,'plan fields/schema')
    kind=plan['kind'];require(kind in WRAPPERS and plan['scope']in('full','bounded'),'unsupported plan kind/scope')
    binding(plan['materialization']);material_path=physical(plan['materialization']['path']);material=read_json(material_path)
    require(material['schema']==SCHEMA+'-materialized'and material['status']=='materialized'and physical(material['source_root'])==material_path.parent,'materialization binding')
    requests=material_path.parent/'requests.jsonl';verify(requests,material['requests']);require(identity(requests)['sha256']==REQUESTS,'source request identity')
    originals={row['id']:row for row in map(json.loads,requests.read_text().splitlines())};require(len(originals)==152,'source roster dimension')
    members=[{'path':str(relative(request['source_root'])/relative(item['path'])),'bytes':item['bytes'],'sha256':item['sha256']}for request in originals.values()for item in request['source_files']]
    require(len(members)==445 and len({x['path']for x in members})==445,'original source member dimension')
    require(sorted(material['members'],key=lambda x:x['path'])==sorted(members,key=lambda x:x['path']),'materialized member rows differ from frozen source requests');check_manifest(material_path.parent,members)
    require(isinstance(plan['builds'],dict)and bool(plan['builds'])and isinstance(plan['rows'],list)and bool(plan['rows']),'nonempty build/row plan')
    for name,build in plan['builds'].items():
        require('/'not in name and str(relative(name))==name,'build key');require(set(build)==({'family','profile','build','prepared','wrapper'}|({'build_wrapper'}if kind=='native'else set())),'build binding fields')
        require(build['family']==kind and build['profile']in('debug','release'),'build family/profile')
        for field in ('build','prepared','wrapper')+ (('build_wrapper',)if kind=='native'else()):binding(build[field])
        require(build['wrapper']['sha256']==WRAPPERS[kind],'unreviewed collector wrapper')
        actual=read_json(build['build']['path']);require(actual['profile']==build['profile'],'build profile binding')
        if kind=='source':require(actual['status']=='built'and actual['portable_schema']==SCHEMA+'-build','successful portable source build required');verify(build['prepared']['path'],actual['prepared_manifest'])
        else:
            require(actual['status']==0,'successful native build required');outer=read_json(build['build_wrapper']['path']);require(outer['status']=='completed'and outer['command']=='build','native build wrapper');verify(build['prepared']['path'],outer['prepared']);verify(build['wrapper']['path'],outer['wrapper'])
            binding(outer['qualified_build_tools']);qualified=read_json(outer['qualified_build_tools']['path']);profile=qualified['profiles'][build['profile']]
            require(build['build']==profile['build_receipt'],'plan native build differs from qualified profile');binding(profile['binary']);require(actual['binary_sha256']==profile['binary']['sha256'],'qualified native binary differs')
            prepared=read_json(build['prepared']['path']);require(prepared['materialization']==plan['materialization'],'native prepared/materialization join')
    fields={'id','profile','build','receipt','wrapper_receipt'};native_fields={'group','operation','format','fuel','noclobber','source_root','entry','output','input_request_sha256'}
    seen=set();paths=set();used=set();allowed=None
    if kind=='native':
        group=native_module();first=next(iter(plan['builds'].values()));prepared=read_json(first['prepared']['path']);component=pathlib.Path(first['prepared']['path']).parent/relative(prepared['native_component_directory']);allowed={group.key(row):row for row in group.roster(component)}
    for row in plan['rows']:
        require(set(row)==fields|(native_fields if kind=='native'else set()),'plan row fields');require(row['id']in originals and row['profile']in('debug','release'),'unknown source/profile')
        require(row['build']in plan['builds'],'missing row build');build=plan['builds'][row['build']];require(build['profile']==row['profile'],'row/build profile mismatch');used.add(row['build'])
        for name in ('receipt','wrapper_receipt'):
            path=str(physical(row[name]));require(path not in paths,'duplicate receipt path');paths.add(path)
        key=(row['profile'],row['id'])
        if kind=='source':require(pathlib.Path(row['receipt']).name=='receipt.json'and row['wrapper_receipt']==str(pathlib.Path(row['receipt']).parent)+'.portable.json','source receipt layout')
        else:
            dispatch=group.key(row);require(dispatch in allowed,'native row outside frozen roster');request=allowed[dispatch]['request'];key=(row['profile'],)+dispatch
            require(row['input_request_sha256']==hashlib.sha256(json.dumps(request,sort_keys=True).encode()).hexdigest(),'native request digest')
            root=material_path.parent/relative(request['source_root']);require(row['source_root']==str(root)and row['entry']==str(root/relative(request['entry'])),'native physical source path')
            prepared=read_json(build['prepared']['path']);component=pathlib.Path(build['prepared']['path']).parent/relative(prepared['native_component_directory'])
            suffix=row['id']+'--'+row['operation']+'--'+row['format']+(f"--fuel-{row['fuel']}"if row['fuel']is not None else'')+(f"--{row['noclobber']}"if row['noclobber']else'')
            location=component/'receipts'/row['group']/row['profile']/suffix
            require(row['receipt']==str(location/'receipt.json'),'native receipt location')
            output=str(location/('occupied-output'if row['noclobber']else'program'))if row['operation']=='compile'else None;require(row['output']==output,'native output location')
        require(key not in seen,'duplicate planned row');seen.add(key)
    require(used==set(plan['builds']),'unused plan build')
    if plan['scope']=='full':
        expected={(profile,id)for profile in ('debug','release')for id in originals}if kind=='source'else{(profile,)+key for profile in ('debug','release')for key in allowed}
        require(seen==expected,'incomplete full plan')
    return plan

def check_result(plan,row):
    wrapper=read_json(row['wrapper_receipt']);inner=read_json(row['receipt']);build=plan['builds'][row['build']]
    require(wrapper['status']==('observed'if plan['kind']=='source'else'completed'),'wrapper did not complete')
    require(inner['profile']==row['profile']and inner['case'if plan['kind']=='source'else'id']==row['id'],'actual row identity differs')
    if plan['kind']=='source':
        require(inner['status']=='observed'and inner['exit_code']==0,'inner observer incomplete');verify(row['receipt'],wrapper['observation_receipt']);verify(build['build']['path'],wrapper['build_receipt']);verify(build['prepared']['path'],wrapper['prepared_manifest'])
        require(wrapper['observation_receipt']==bound_file(row['receipt'])and wrapper['build_receipt']==build['build']and wrapper['prepared_manifest']==build['prepared'],'stale source wrapper path binding')
        require(wrapper['case']==row['id']and wrapper['profile']==row['profile']and wrapper['materialization']==plan['materialization'],'source wrapper request binding')
        prepared=read_json(build['prepared']['path']);adapter=pathlib.Path(build['prepared']['path']).parent/relative(prepared['adapter_directory']);requests=pathlib.Path(plan['materialization']['path']).parent/'requests.jsonl'
        require(wrapper['argv']==[wrapper['selected_python']['path'],'-B',str(adapter/'run.py'),'--requests',str(requests),'--case',row['id'],'--build-receipt',build['build']['path'],'--profile',row['profile'],'--output',str(pathlib.Path(row['receipt']).parent)],'stale source worker argv')
        require(inner['build_receipt']==build['build']and inner['request_file']==bound_file(requests),'source inner build/request binding')
        root=pathlib.Path(row['receipt']).parent
        for artifact in inner['artifacts']:
            require(physical(artifact['path']).is_relative_to(root),'stale source artifact path')
            if 'compressed'in artifact:require(physical(artifact['compressed']['path']).is_relative_to(root),'stale compressed artifact path');binding(artifact['compressed'])
            else:verify(artifact['path'],artifact)
    else:
        require(wrapper['actual']==inner,'native inner/wrapper differs');verify(build['build_wrapper']['path'],wrapper['build_wrapper_receipt'])
        require(wrapper['build_wrapper_receipt']==build['build_wrapper']and inner['environment_overrides']['OXID_UNIT3_NATIVE_RECEIPT']==str(pathlib.Path(row['receipt']).parent),'stale native receipt binding')
        outer=read_json(build['build_wrapper']['path']);require(wrapper['qualified_build_tools']==outer['qualified_build_tools'],'native qualified tool set differs');qualified=read_json(outer['qualified_build_tools']['path']);profile=qualified['profiles'][row['profile']]
        require(inner['verified_build_sha256']==build['build']['sha256']and inner['binary_sha256']==profile['binary']['sha256'],'native actual build/binary differs')
        for field in ('group','operation','format','fuel'):require(inner[field]==row[field],'native actual dispatch differs')
        require(inner['input_request_sha256']==row['input_request_sha256']and inner['output']==row['output'],'native actual request/output differs')
    return {'receipt':bound_file(row['receipt']),'wrapper_receipt':bound_file(row['wrapper_receipt'])}

def execute(plan,python,output,native_tools=None,runner=run_bounded,plan_identity=None):
    validate(plan);python=physical(str(pathlib.Path(python).resolve()));require(python.is_file()and os.access(python,os.X_OK),'Python tool missing')
    for row in plan['rows']:require(not pathlib.Path(row['receipt']).exists()and not pathlib.Path(row['wrapper_receipt']).exists(),'planned output already exists')
    out=fresh(output);result={'schema':SCHEMA+'-collection','status':'collection-failure','assertion_mode':assertion_mode(),'scope':plan['scope'],'kind':plan['kind'],'planned_rows':len(plan['rows']),'completed_rows':0,'rows':[],'python':bound_file(python)}
    try:
        for number,row in enumerate(plan['rows']):
            build=plan['builds'][row['build']];command=[str(python),'-B',build['wrapper']['path']]
            if plan['kind']=='source':command+=['--python',str(python),'--prepared-manifest',build['prepared']['path'],'--materialization',plan['materialization']['path'],'--build-receipt',build['build']['path'],'--case',row['id'],'--profile',row['profile'],'--output',str(pathlib.Path(row['receipt']).parent)]
            else:
                require(native_tools is not None,'explicit native tool paths required');command+=['invoke','--prepared-manifest',build['prepared']['path'],'--build-wrapper-receipt',build['build_wrapper']['path'],'--group',row['group'],'--case',row['id'],'--operation',row['operation'],'--format',row['format'],'--profile',row['profile'],'--receipt',row['wrapper_receipt']]
                for name in ('rustc','llvm-bin','llvm-lib-dir','cargo-home','rustup-home'):command+=['--'+name,native_tools[name]]
                if row['fuel']is not None:command+=['--fuel',str(row['fuel'])]
                if row['noclobber']is not None:command+=['--noclobber',row['noclobber']]
            log=out/f'{number:03d}';log.mkdir();entry={'id':row['id'],'profile':row['profile'],'argv':command,'status':'failed'};result['rows'].append(entry)
            try:
                with(log/'stdout').open('w')as stdout,(log/'stderr').open('w')as stderr:run=runner(command,env=safe_env(),timeout=360,stdout=stdout,stderr=stderr)
            finally:
                for stream in ('stdout','stderr'):
                    if(log/stream).is_file():entry[stream]=bound_file(log/stream)
            entry.update(exit_code=run.returncode);require(run.returncode==0,'portable worker returned nonzero')
            entry.update(check_result(plan,row));entry['status']='observed';result['completed_rows']+=1
        require(result['completed_rows']==result['planned_rows'],'collection incomplete');validate(plan)
        if plan_identity is not None:binding(plan_identity);require(read_json(plan_identity['path'])==plan,'frozen plan changed during collection')
        result['status']='collected-not-compared'
    except BaseException as error:result['error']=f'{type(error).__name__}: {error}'
    write_json(out/'collection.json',result);return result

def main():
    p=argparse.ArgumentParser()
    for name in ('plan','plan-sha256','python','output'):p.add_argument('--'+name,required=True)
    for name in ('rustc','llvm-bin','llvm-lib-dir','cargo-home','rustup-home'):p.add_argument('--'+name)
    a=p.parse_args();require(identity(a.plan)['sha256']==a.plan_sha256,'frozen plan hash differs');plan=read_json(a.plan);tools={name:getattr(a,name.replace('-','_'))for name in ('rustc','llvm-bin','llvm-lib-dir','cargo-home','rustup-home')}
    if plan['kind']=='native':require(all(tools.values()),'explicit native tool arguments required')
    plan_identity=bound_file(a.plan);result=execute(plan,a.python,a.output,tools,plan_identity=plan_identity);result['plan']=plan_identity;write_json(pathlib.Path(a.output)/'collection.json',result);print(json.dumps({k:result[k]for k in ('status','completed_rows','planned_rows')}));return 0 if result['status']=='collected-not-compared'else 1
if __name__=='__main__':sys.exit(main())
