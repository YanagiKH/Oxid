#!/usr/bin/env python3
"""Reviewed, explicit platform-test successor -> frozen core-v1 derivation."""
from common import *
from assemble import apply_patch
import argparse,shutil

SELECTED='a8e24a8d14c8b47140297f9b2b37adc75b2932911f8745df5937d58bdb2dc963'
ORIGINAL='53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
PATCH='a6c0a70927fcc44f68244038e88ceb371fe0a78a8b8d4c0aa1d83475d415a1d9'
TEST='src/frontend/oir/project_execution_tests.rs'
OLD_TEST='1185aa4c2e86644c3b14acae290adb043ec27d2a222018b1e67997317dc5851d'
NEW_TEST='6009bb0789ec1ded0b73caf13a52217e33e1fa006813393a6ded7568f60c8b3f'

def derive(repo,selected_manifest,core_manifest,platform_patch,original_test,output):
    selected_path=pathlib.Path(selected_manifest).resolve();core_path=pathlib.Path(core_manifest).resolve();patch_path=pathlib.Path(platform_patch).resolve();old_path=pathlib.Path(original_test).resolve();repo=pathlib.Path(repo).resolve()
    require(identity(selected_path)['sha256']==SELECTED,'explicit reviewed successor a8e24a8d selection required; no fallback')
    require(identity(core_path)['sha256']==ORIGINAL,'frozen original core manifest identity required')
    require(identity(patch_path)['sha256']==PATCH and identity(old_path)['sha256']==OLD_TEST,'reviewed test bridge inputs changed')
    selected=read_json(selected_path);original=read_json(core_path)
    current={x['path']:x for x in selected['files']};before={x['path']:x for x in original['files']}
    require(len(current)==len(selected['files'])==117 and len(before)==len(original['files'])==117 and set(current)==set(before),'source membership/duplicate drift')
    differences=[name for name in current if current[name]!=before[name]]
    require(differences==[TEST]and current[TEST]['sha256']==NEW_TEST and before[TEST]['sha256']==OLD_TEST,'bridge admits exactly the reviewed test-file delta')
    check_manifest(repo,selected['files'])
    out=fresh(output);proof=out/'patch-proof';target=proof/TEST;target.parent.mkdir(parents=True);shutil.copyfile(old_path,target)
    touched=apply_patch(proof,patch_path.read_text());require(touched==[TEST],'platform patch touched another source');verify(target,current[TEST])
    derived=out/'derived-core-v1'
    for item in selected['files']:
        rel=relative(item['path']);destination=derived/rel;destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(repo/rel,destination)
    # Restore this one hash-bound test source only. No production byte is changed.
    shutil.copyfile(old_path,derived/TEST)
    check_manifest(derived,original['files'],exact=True)
    receipt={'schema':SCHEMA+'-platform-bridge','status':'verified-derived-core-v1','semantic_receipt_label':'derived core-v1 with production-byte identity bridge to selected successor','direct_successor_execution':False,'assertion_mode':assertion_mode(),'selected_repository':str(repo),'selected_current_manifest':bound_file(selected_path),'original_core_manifest':bound_file(core_path),'platform_patch':bound_file(patch_path),'original_test':bound_file(old_path),'changed_test_file':TEST,'selected_test':current[TEST],'derived_test':before[TEST],'unchanged_input_count':116,'production_source_deltas':0,'derived_root':str(derived),'derived_inputs':files_manifest(derived),'controller':bound_file(pathlib.Path(__file__))}
    write_json(out/'bridge-receipt.json',receipt);return receipt

def main():
    p=argparse.ArgumentParser()
    for name in ('repo','selected-manifest','core-manifest','platform-patch','original-test','output'):p.add_argument('--'+name,required=True)
    a=p.parse_args();result=derive(a.repo,a.selected_manifest,a.core_manifest,a.platform_patch,a.original_test,a.output);print(json.dumps({'status':result['status'],'derived_root':result['derived_root'],'label':result['semantic_receipt_label']}))
if __name__=='__main__':main()
