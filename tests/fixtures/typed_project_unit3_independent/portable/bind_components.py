#!/usr/bin/env python3
"""Bind observer, oracle, native and mutation components without executing them."""
from common import *
import argparse,shutil
KINDS={'observer','expectations','declaration-supplement','comparator','native-driver','mutation-adapter'}

def main():
    p=argparse.ArgumentParser();p.add_argument('--components',required=True);p.add_argument('--source-transport',required=True);p.add_argument('--output',required=True);a=p.parse_args();spec=read_json(a.components)
    transport=read_json(a.source_transport);require(transport['schema']==SCHEMA+'-sources'and transport['member_count']==445,'shared source transport required')
    require(set(spec)=={'schema','components'}and spec['schema']==SCHEMA+'-component-inputs','component input schema');require(isinstance(spec['components'],list),'component list required')
    seen=set();verified=[]
    for component in spec['components']:
        require(set(component)=={'name','kind','root','manifest','manifest_sha256','files','entrypoint','status'},'unknown component input fields')
        name=component['name'];relative(name);require('/'not in name and name not in seen,'component name duplicate/path');seen.add(name)
        require(component['kind']in KINDS and component['status']in ('reviewed','pending'),'component kind/status')
        root=pathlib.Path(component['root']).resolve();manifest=root/relative(component['manifest']);require(identity(manifest)['sha256']==component['manifest_sha256'],'component manifest identity mismatch')
        # Files are explicit manifests, not globs. Expectations are copied as
        # opaque bytes and never supplied to an observer/compiler invocation.
        check_manifest(root,component['files']);entry=component['entrypoint'];require(entry is None or entry in {x['path']for x in component['files']},'unbound component entrypoint')
        require(not any(x['path'].startswith('sources/')for x in component['files']),'source corpus must use shared transport, not component copies')
        verified.append((component,root,manifest))
    require({c['kind']for c,_,_ in verified}==KINDS,'all six qualification components must be explicitly bound')
    out=fresh(a.output);bindings=[]
    for component,root,manifest in verified:
        directory=out/component['name'];directory.mkdir()
        for item in component['files']:
            destination=directory/relative(item['path']);destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(root/relative(item['path']),destination)
        bindings.append({k:component[k]for k in ('name','kind','status','entrypoint')}|{'upstream_manifest':bound_file(manifest),'files':files_manifest(directory)})
    write_json(out/'components.json',{'schema':SCHEMA+'-bound-components','assertion_mode':assertion_mode(),'input_spec':bound_file(a.components),'source_transport':bound_file(a.source_transport),'components':bindings,'status':'bound-not-executed'})
    print(json.dumps({'status':'bound-not-executed','components':len(bindings)}))
if __name__=='__main__':main()
