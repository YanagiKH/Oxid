#!/usr/bin/env python3
"""Fixed 220-row mutation plan and collection around the reviewed path wrapper."""
from common import *
from collect import PLAN,physical,binding,REQUESTS
import argparse,importlib.util

WRAPPER='24ddd3bb3b85c1e04a3daf82ac2e1001d71e6e28fa8c8b24f077ba545ee2cac7'
EFFECTIVE='628d08d0500d9b98acac658ad3c75ac311272cd493913b55c2e2598a48240bd8'
STALE='source-association-stale_parser_generation'
DRIVERS={'driver-snapshot_after_load','driver-source_free_native','driver-recursion_occupied_output','driver-recursion_symlink_output'}

def wrapper(path):
    require(identity(path)['sha256']==WRAPPER,'reviewed mutation wrapper required before import')
    spec=importlib.util.spec_from_file_location('unit3_reviewed_mutation_paths',path);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module
def requests(prepared_path):
    path=pathlib.Path(prepared_path).parent/'frozen-inputs/effective-requests.json';require(identity(path)['sha256']==EFFECTIVE,'effective source-only mutation roster differs')
    data=read_json(path);rows=data['mutations'];require(data['count']==len(rows)==114 and len({r['id']for r in rows})==114,'mutation roster dimensions')
    require({r['id']for r in rows if r['mutation']['kind']=='driver-boundary'}==DRIVERS,'driver attribution roster differs')
    selected={r['id']:r for r in rows if r['id']not in DRIVERS};require(len(selected)==110 and STALE in selected,'internal mutation roster differs');return selected
def check_material(item):
    binding(item);path=physical(item['path']);data=read_json(path);require(data['schema']==SCHEMA+'-materialized'and data['status']=='materialized'and data['source_root']==str(path.parent),'materialization binding')
    source=path.parent/'requests.jsonl';verify(source,data['requests']);require(identity(source)['sha256']==REQUESTS,'original source roster differs')
    rows=[json.loads(line)for line in source.read_text().splitlines()];members=[{'path':str(relative(r['source_root'])/relative(f['path'])),'bytes':f['bytes'],'sha256':f['sha256']}for r in rows for f in r['source_files']]
    require(len(rows)==152 and len(members)==len({r['path']for r in members})==445,'source dimensions')
    require(sorted(data['members'],key=lambda r:r['path'])==sorted(members,key=lambda r:r['path']),'source member declarations differ');check_manifest(path.parent,members)
    require({p.relative_to(path.parent).as_posix()for p in path.parent.rglob('*.ox')if p.is_file()}=={r['path']for r in members},'extra/missing source files')

def validate(plan):
    require(set(plan)=={'schema','kind','scope','materialization','builds','rows'}and plan['schema']==PLAN and plan['kind']=='mutation'and plan['scope']in('full','bounded'),'mutation plan schema')
    check_material(plan['materialization']);require(plan['rows']and plan['builds'],'empty mutation plan');roster=None
    for key,build in plan['builds'].items():
        require('/'not in key and str(relative(key))==key,'build key');require(set(build)=={'family','profile','build','portable_build','prepared','wrapper'},'mutation build fields')
        require(build['family']in('mutation-v2','mutation-v3')and build['profile']in('debug','release'),'mutation build family/profile')
        for field in ('build','portable_build','prepared','wrapper'):binding(build[field])
        module=wrapper(build['wrapper']['path']);prepared,home,variant=module.checked_preparation(build['prepared']['path']);require(build['family']=='mutation-'+variant,'prepared variant differs')
        view,_=module.build_view(pathlib.Path(build['prepared']['path']),prepared,home,variant,pathlib.Path(build['portable_build']['path']),build['profile']);require(bound_file(view)==build['build'],'planned mutation build view differs')
        current=requests(build['prepared']['path'])
        if roster is None:roster=current
        else:require(roster==current,'cross-build mutation roster differs')
    seen=set();paths=set();used=set()
    for row in plan['rows']:
        require(set(row)=={'id','profile','build','receipt','wrapper_receipt'},'mutation row fields');require(row['id']in roster and row['profile']in('debug','release')and row['build']in plan['builds'],'unlisted mutation row')
        build=plan['builds'][row['build']];require(build['profile']==row['profile']and build['family']==('mutation-v3'if row['id']==STALE else'mutation-v2'),'mutation dispatch variant/profile differs');used.add(row['build'])
        for name in ('receipt','wrapper_receipt'):
            path=str(physical(row[name]));require(path not in paths,'duplicate mutation receipt path');paths.add(path)
        outer=pathlib.Path(row['wrapper_receipt']);require(outer.name=='portable-mutation.json'and row['receipt']==str(outer.parent/'observed/receipt.json'),'mutation receipt layout')
        key=(row['id'],row['profile']);require(key not in seen,'duplicate mutation row');seen.add(key)
    require(used==set(plan['builds']),'unused mutation build')
    if plan['scope']=='full':require(seen=={(id,p)for id in roster for p in('debug','release')},'full internal mutation membership differs')
    return roster

def make(a):
    require(not pathlib.Path(a.receipt_root).exists(),'fresh future mutation output required');module=wrapper(a.wrapper)
    plan={'schema':PLAN,'kind':'mutation','scope':'full','materialization':bound_file(a.materialization),'builds':{},'rows':[]};roster=None
    for variant in ('v2','v3'):
        prepared_path=pathlib.Path(getattr(a,variant+'_prepared')).resolve();prepared,home,actual=module.checked_preparation(prepared_path);require(actual==variant,'explicit prepared variant differs')
        for profile in ('debug','release'):
            build=pathlib.Path(getattr(a,variant+'_'+profile+'_build')).resolve();view,_=module.build_view(prepared_path,prepared,home,variant,build,profile)
            plan['builds']['mutation-'+variant+'-'+profile]={'family':'mutation-'+variant,'profile':profile,'build':bound_file(view),'portable_build':bound_file(build),'prepared':bound_file(prepared_path),'wrapper':bound_file(a.wrapper)}
        current=requests(prepared_path);require(roster is None or roster==current,'mutation source requests differ');roster=current
    for profile in ('debug','release'):
        for id in roster:
            location=pathlib.Path(a.receipt_root).resolve()/profile/id;variant='v3'if id==STALE else'v2';plan['rows'].append({'id':id,'profile':profile,'build':'mutation-'+variant+'-'+profile,'receipt':str(location/'observed/receipt.json'),'wrapper_receipt':str(location/'portable-mutation.json')})
    validate(plan);return plan

def result_for(plan,row):
    build=plan['builds'][row['build']];outer=read_json(row['wrapper_receipt']);inner=read_json(row['receipt']);require(outer['status']=='observed'and inner['status']=='observed','mutation observer incomplete')
    for record in (outer,inner):require(record['mutation_id']==row['id']and record['profile']==row['profile'],'actual mutation row differs')
    for field,expected in (('prepared_manifest',build['prepared']),('materialization',plan['materialization']),('portable_build',build['portable_build']),('build_view',build['build']),('path_wrapper',build['wrapper']),('observation_receipt',bound_file(row['receipt']))):require(outer[field]==expected,'mutation wrapper identity differs: '+field)
    require(inner['build_receipt']==build['build'],'mutation inner build differs')
    require(outer['controller_argv'][1:]==['--id',row['id'],'--profile',row['profile'],'--build-receipt',build['build']['path'],'--output',str(pathlib.Path(row['receipt']).parent)]+(outer['controller_argv'][-2:]if'--baseline'in outer['controller_argv']else[]),'mutation controller argv differs')
    for name in ('baseline_receipt','baseline_observations','mutant_receipt','mutant_observations'):binding(inner[name])
    require(physical(inner['mutant_receipt']['path'])==pathlib.Path(row['receipt']).parent/'mutant/receipt.json','stale mutant path')
    return inner

def collect(plan,python,output,plan_identity=None,runner=run_bounded):
    roster=validate(plan);python=physical(str(pathlib.Path(python).resolve()));require(python.is_file()and os.access(python,os.X_OK),'Python missing')
    for row in plan['rows']:require(not pathlib.Path(row['wrapper_receipt']).parent.exists(),'mutation output already exists')
    out=fresh(output);receipt={'schema':SCHEMA+'-mutation-collection','status':'collection-failure','scope':plan['scope'],'assertion_mode':assertion_mode(),'planned_rows':len(plan['rows']),'completed_rows':0,'rows':[],'driver_results_attributed_separately':8};cache={}
    try:
        for number,row in enumerate(plan['rows']):
            build=plan['builds'][row['build']];key=(row['profile'],roster[row['id']]['positive_control'],build['build']['sha256']);command=[str(python),'-B',build['wrapper']['path'],'run','--prepared-manifest',build['prepared']['path'],'--materialization',plan['materialization']['path'],'--build-receipt',build['portable_build']['path'],'--mutation-id',row['id'],'--profile',row['profile'],'--output',str(pathlib.Path(row['wrapper_receipt']).parent)]
            if key in cache:binding(cache[key]);command+=['--baseline',str(pathlib.Path(cache[key]['path']).parent)]
            log=out/f'{number:03d}';log.mkdir();entry={'id':row['id'],'profile':row['profile'],'argv':command,'status':'failed'};receipt['rows'].append(entry)
            try:
                with(log/'stdout').open('w')as stdout,(log/'stderr').open('w')as stderr:result=runner(command,env=safe_env(),timeout=600,stdout=stdout,stderr=stderr)
            finally:
                for stream in ('stdout','stderr'):
                    if(log/stream).is_file():entry[stream]=bound_file(log/stream)
            entry['exit_code']=result.returncode;require(result.returncode==0,'mutation worker returned nonzero');inner=result_for(plan,row);require(inner['request']==roster[row['id']],'effective mutation request differs');cache[key]=inner['baseline_receipt']
            entry.update(status='observed',receipt=bound_file(row['receipt']),wrapper_receipt=bound_file(row['wrapper_receipt']));receipt['completed_rows']+=1
        require(receipt['completed_rows']==receipt['planned_rows'],'mutation collection incomplete');validate(plan)
        if plan_identity is not None:binding(plan_identity);require(read_json(plan_identity['path'])==plan,'frozen mutation plan changed')
        receipt['status']='collected-not-compared'
    except BaseException as error:receipt['error']=f'{type(error).__name__}: {error}'
    write_json(out/'collection.json',receipt);return receipt

def main():
    p=argparse.ArgumentParser();sub=p.add_subparsers(dest='command',required=True);q=sub.add_parser('make')
    for name in ('materialization','wrapper','v2-prepared','v3-prepared','v2-debug-build','v2-release-build','v3-debug-build','v3-release-build','receipt-root','output'):q.add_argument('--'+name,required=True)
    q=sub.add_parser('collect')
    for name in ('plan','plan-sha256','python','output'):q.add_argument('--'+name,required=True)
    a=p.parse_args()
    if a.command=='make':
        output=pathlib.Path(a.output).resolve();require(not output.exists(),'mutation plan exists');plan=make(a);output.parent.mkdir(parents=True,exist_ok=True);write_json(output,plan);print(json.dumps({'status':'source-only-plan-frozen','rows':len(plan['rows']),'plan':bound_file(output)}));return 0
    require(identity(a.plan)['sha256']==a.plan_sha256,'mutation plan hash differs');plan_identity=bound_file(a.plan);result=collect(read_json(a.plan),a.python,a.output,plan_identity);result['plan']=plan_identity;write_json(pathlib.Path(a.output)/'collection.json',result);print(json.dumps({k:result[k]for k in('status','planned_rows','completed_rows')}));return 0 if result['status']=='collected-not-compared'else 1
if __name__=='__main__':sys.exit(main())
