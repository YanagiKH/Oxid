#!/usr/bin/env python3
"""Check the exact staged package membership before qualification starts."""
from common import *
import argparse

def verify_package(root,manifest):
    root=pathlib.Path(root).resolve();manifest=pathlib.Path(manifest).resolve();require(manifest.parent==root and manifest.name=='package-manifest.json','exact package manifest location required')
    spec=read_json(manifest);require(spec['schema']=='unit3-independent-publication-package-v1','package schema')
    require(spec['source_member_count']==445 and spec['source_transport_copies']==1,'one445-source transport required')
    check_manifest(root,spec['files']);actual={p.relative_to(root).as_posix()for p in root.rglob('*')if p.is_file()}
    require(actual=={x['path']for x in spec['files']}|{'package-manifest.json'},'package extra/missing members')
    require(not any(p.is_symlink()for p in root.rglob('*')),'package aliases are not admitted')
    require(not list(root.rglob('*.ox')),'Unit3 corpus must remain in its exact source archive until external materialization')
    return {'status':'verified','schema':SCHEMA+'-package-verification','assertion_mode':assertion_mode(),'manifest':bound_file(manifest),'file_count':len(spec['files'])}

def main():
    p=argparse.ArgumentParser();p.add_argument('--package',required=True);p.add_argument('--manifest',required=True);p.add_argument('--receipt',required=True);a=p.parse_args();receipt=pathlib.Path(a.receipt).resolve();require(not receipt.exists(),'package verification receipt exists');require(not receipt.is_relative_to(pathlib.Path(a.package).resolve()),'package receipt must be external')
    result=verify_package(a.package,a.manifest);receipt.parent.mkdir(parents=True,exist_ok=True);write_json(receipt,result);print(json.dumps(result))
if __name__=='__main__':main()
