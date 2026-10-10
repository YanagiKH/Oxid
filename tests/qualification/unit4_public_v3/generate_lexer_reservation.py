#!/usr/bin/env python3
"""Approved source-only lifecycle map recipe; independent review still required."""
from pathlib import Path
import sys, importlib.util, json, difflib, hashlib, subprocess
root=Path(__file__).resolve().parents[3]
p=root/'tests/qualification/unit4_public_v3'
spec=importlib.util.spec_from_file_location('lexer_public',p/'lexer_reservation_lifecycle.py'); m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
m.need(not sys.flags.optimize, 'generator requires normal Python')
manifest=root/'tests/fixtures/typed_project_source_binding/lexer-reservation-source-v1.json'
current=json.loads(manifest.read_bytes()); inputs={r['path']:(root/r['path']).read_bytes() for r in current['files']}
assert m.rows(inputs)==current['files']
prior_patch=(p/'observer-u8-v1.patch').read_bytes()
derived,receipt=m.compose(inputs,prior_patch)
patch=b''
for name in sorted(set(inputs)|set(derived)):
    before=inputs.get(name,b''); after=derived[name]
    if before!=after:
        patch+=''.join(difflib.unified_diff(before.decode().splitlines(keepends=True),after.decode().splitlines(keepends=True),fromfile='a/'+name if name in inputs else '/dev/null',tofile='b/'+name)).encode()
assert m.exact_patch(inputs,patch)==derived
assert m.exact_patch(derived,patch,True)==inputs
oldmanifest=root/'tests/fixtures/typed_project_source_binding_byte_storage_v1/current-source.json'
prior=json.loads(oldmanifest.read_bytes())
old={r['path']:subprocess.check_output(['git','show',prior['reviewed_source_head']+':'+r['path']],cwd=root) for r in prior['files']}
assert m.rows(old)==prior['files']
# The predecessor's historical contexts are exact unique substitutions; its
# retained patch coordinates are not rewritten or treated as current offsets.
oldderived=dict(old)
for name,added,hunks in m.sections(prior_patch):
    if added:
        oldderived[name]=hunks[0][3]
    else:
        for _,_,before,after,_ in hunks:
            assert oldderived[name].count(before)==1,name
            oldderived[name]=oldderived[name].replace(before,after,1)
canonical=lambda x:json.dumps(x,sort_keys=True,separators=(',',':')).encode()
assert m.sha(canonical(m.rows(oldderived,True)))=='8805b56dfd40fa835553b60d2b80484550c14b4214d95653f5507756185686fe'
roster=m.inserted_roster(patch); priorroster=m.inserted_roster(prior_patch)
assert {r['path']:r['inserted'] for r in roster if r['path']!=m.LEXER}=={r['path']:r['inserted'] for r in priorroster if r['path']!=m.LEXER}
proof={'schema':'oxid-unit4-lexer-reservation-lifecycle-v1','reviewed_source_head':current['reviewed_source_head'],'source_only_tree':current['source_only_tree'],'source_manifest':m.row('tests/fixtures/typed_project_source_binding/current-source.json',manifest.read_bytes()),'patch':m.row('observer-lexer-reservation-v1.patch',patch),'predecessor_authority':m.row('byte_storage_source_authority.py',(p/'byte_storage_source_authority.py').read_bytes()),'predecessor_patch':m.row('observer-u8-v1.patch',prior_patch),'base_files':m.rows(inputs),'observer_files':m.rows(derived,True),'predecessor_base_files':m.rows(old),'predecessor_observer_files':m.rows(oldderived,True),'lexer_hook_correspondence':receipt,'inserted_roster':roster,'complete_inverse_sha256':m.sha(canonical(m.rows(m.exact_patch(derived,patch,True)))),'compiler_invocations':0,'execution_qualified':False}
(p/'observer-lexer-reservation-v1.patch').write_bytes(patch)
proofraw=(json.dumps(proof,sort_keys=True,indent=2)+'\n').encode();(p/'lexer-reservation-lifecycle-v1.json').write_bytes(proofraw)
a=(p/'byte_storage_source_authority.py').read_text()
for oldvalue,newvalue in [('402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa',m.sha(manifest.read_bytes())),('04774d690abd8fa8813be24865b98e7e75ebcf326568efb0f8c6cd4698890bab',m.sha(canonical(m.rows(inputs)))),(m.PREDECESSOR_PATCH_SHA,m.sha(patch)),('8805b56dfd40fa835553b60d2b80484550c14b4214d95653f5507756185686fe',m.sha(canonical(m.rows(derived,True))))]:
    assert a.count(oldvalue)==1; a=a.replace(oldvalue,newvalue)
a+='\nLEXER_LIFECYCLE_AUTHORITY_SHA = '+repr(m.sha(proofraw))+'\nLEXER_LIFECYCLE_HELPER_SHA = '+repr(m.sha((p/'lexer_reservation_lifecycle.py').read_bytes()))+'\n'
(p/'authority.py').write_text(a)
print(json.dumps({'patch':proof['patch'],'authority':m.row('lexer-reservation-lifecycle-v1.json',proofraw),'helper':m.row('lexer_reservation_lifecycle.py',(p/'lexer_reservation_lifecycle.py').read_bytes()),'base_files_sha':m.sha(canonical(m.rows(inputs))),'observer_files_sha':m.sha(canonical(m.rows(derived,True))),'counts':[len(inputs),len(derived)],'hook':receipt},indent=2))
