#!/usr/bin/env python3
"""Append only frozen historical auxiliary inputs to a verified core bridge."""
import pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]))
from common import *
import argparse,shutil

CORE='53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
OVERLAY='f61460acd26bd9f8bf783b02c49315d0a950507834de945fe1ae07fd25f4c401'
PATCH='2df49ce00cbb0e4483a9018b90279cf6b9d42c204c767692d0e740493e507bee'
BRIDGE='0a67058d7592c1ce50ba52933784826f6883f566445bfe3da4aff49662f12e25'
SELECTED='a8e24a8d14c8b47140297f9b2b37adc75b2932911f8745df5937d58bdb2dc963'

def prepare(bridge_receipt,core_manifest,overlay_manifest,overlay_patch,auxiliary_root,output):
    require(identity(core_manifest)['sha256']==CORE,'original core selection differs')
    require(identity(overlay_manifest)['sha256']==OVERLAY and identity(overlay_patch)['sha256']==PATCH,'native overlay selection differs')
    bridge=read_json(bridge_receipt);core=read_json(core_manifest);overlay=read_json(overlay_manifest)
    require(bridge['schema']==SCHEMA+'-platform-bridge'and bridge['status']=='verified-derived-core-v1','verified bridge required')
    require(bridge['controller']['sha256']==BRIDGE and bridge['selected_current_manifest']['sha256']==SELECTED and bridge['original_core_manifest']['sha256']==CORE,'bridge identities differ')
    require(bridge['production_source_deltas']==0 and bridge['unchanged_input_count']==116 and bridge['direct_successor_execution']is False,'bridge attribution differs')
    for name in ('controller','selected_current_manifest','original_core_manifest','platform_patch','original_test'):
        verify(bridge[name]['path'],bridge[name])
    selected=read_json(bridge['selected_current_manifest']['path'])
    check_manifest(bridge['selected_repository'],selected['files'])
    derived=pathlib.Path(bridge['derived_root']);check_manifest(derived,core['files'],exact=True)
    require(bridge['derived_inputs']==files_manifest(derived),'bridge-derived input identity differs')
    core_by_path={x['path']:x for x in core['files']};overlay_by_path={x['path']:x for x in overlay['files']}
    require(len(core_by_path)==117 and len(overlay_by_path)==199,'frozen input membership differs')
    lines=pathlib.Path(overlay_patch).read_text().splitlines();added=[]
    for i,line in enumerate(lines):
        if line=='--- /dev/null':
            require(i+1<len(lines)and lines[i+1].startswith('+++ b/'),'invalid added patch target')
            added.append(str(relative(lines[i+1][6:])))
    require(added==['src/frontend/driver/unit3_native_driver.rs'],'native added module differs')
    auxiliary=[item for name,item in overlay_by_path.items()if name not in core_by_path and name not in added]
    require(len(auxiliary)==81 and all(x['path'].startswith('tests/fixtures/')for x in auxiliary),'auxiliary inventory differs')
    auxiliary_root=pathlib.Path(auxiliary_root);check_manifest(auxiliary_root,auxiliary,exact=True)
    out=fresh(output);source=out/'inputs';source.mkdir()
    for root,items in ((derived,core['files']),(auxiliary_root,auxiliary)):
        for item in items:
            destination=source/relative(item['path']);require(not destination.exists(),'auxiliary overlaps core')
            destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(root/item['path'],destination)
    expected=core['files']+auxiliary;check_manifest(source,expected,exact=True)
    receipt={'schema':SCHEMA+'-native-inputs','status':'verified-198-prepatch-inputs','assertion_mode':assertion_mode(),'bridge_receipt':bound_file(bridge_receipt),'core_manifest':bound_file(core_manifest),'overlay_manifest':bound_file(overlay_manifest),'overlay_patch':bound_file(overlay_patch),'auxiliary_manifest':auxiliary,'input_root':str(source.resolve()),'inputs':files_manifest(source),'semantic_receipt_label':bridge['semantic_receipt_label'],'direct_successor_execution':False,'controller':bound_file(__file__)}
    write_json(out/'native-inputs.json',receipt);return receipt

def main():
    p=argparse.ArgumentParser()
    for name in ('bridge-receipt','core-manifest','overlay-manifest','overlay-patch','auxiliary-root','output'):p.add_argument('--'+name,required=True)
    a=p.parse_args();r=prepare(a.bridge_receipt,a.core_manifest,a.overlay_manifest,a.overlay_patch,a.auxiliary_root,a.output);print(json.dumps({'status':r['status'],'input_root':r['input_root']}))
if __name__=='__main__':main()
