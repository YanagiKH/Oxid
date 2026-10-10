#!/usr/bin/env python3
"""Approved source-only lifecycle map recipe; independent review still required."""
# Source-only review recipe. No authority activation is authorized here.
from pathlib import Path
import sys, importlib.util, json, difflib, hashlib, subprocess, argparse
sys.dont_write_bytecode = True
root=Path(__file__).resolve().parents[3]
p=root/'tests/qualification/unit4_public_v3'

# Candidate generation has no activation mode. The checkout and its Git stores
# are read-only inputs; every generated or transient artifact stays under this
# explicitly selected, initially absent candidate directory.
def checked_candidate_directory(repository, value, git_stores):
    output = Path(value)
    if not output.is_absolute() or output != output.resolve(strict=False):
        raise ValueError('candidate output must be an absolute canonical path')
    forbidden = (repository.resolve(), *(Path(p).resolve() for p in git_stores))
    if any(output == p or p in output.parents or output in p.parents for p in forbidden):
        raise ValueError('candidate output overlaps the checkout or Git stores')
    if any(p.is_symlink() for p in (output, *output.parents)):
        raise ValueError('symlink in candidate output path')
    if output.exists() or not output.parent.is_dir():
        raise ValueError('candidate output must be absent with an existing parent')
    return output

parser = argparse.ArgumentParser(description='Create source-only candidates; never activate them')
parser.add_argument('--candidate-output-dir', required=True)
args = parser.parse_args()
git_stores = [subprocess.check_output(
    ['git', 'rev-parse', '--path-format=absolute', option], cwd=root
).decode().strip() for option in ('--git-dir', '--git-common-dir')]
candidate_output = checked_candidate_directory(root, args.candidate_output_dir, git_stores)
candidate_output.mkdir()

def write_candidate(name, raw):
    if not isinstance(raw, bytes) or Path(name).name != name or name in ('', '.', '..'):
        raise ValueError('invalid named candidate output')
    # Exclusive creation rejects duplicate or stale outputs. This function has
    # no path back to the active checkout and no overwrite/activation option.
    with (candidate_output / name).open('xb') as handle:
        handle.write(raw)

spec=importlib.util.spec_from_file_location('lexer_public',p/'lexer_reservation_lifecycle.py'); m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
m.need(not sys.flags.optimize, 'generator requires normal Python')
manifest=root/'tests/fixtures/typed_project_source_binding/lexer-reservation-source-v2.json'
manifest_raw=manifest.read_bytes()
m.need((len(manifest_raw),m.sha(manifest_raw)) == (74328,'9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'), 'unreviewed source-v2 candidate bytes')
current=json.loads(manifest_raw)
m.need((current['reviewed_source_head'],current['source_only_tree']) == ('b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489','7f5c9aa08c569c4d0b5a27391d8fc68337075d36'), 'wrong fixed compiler checkpoint')
inputs={r['path']:subprocess.check_output(['git','show',current['reviewed_source_head']+':'+r['path']],cwd=root) for r in current['files']}
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
proof={'schema':'oxid-unit4-lexer-reservation-lifecycle-v1','reviewed_source_head':current['reviewed_source_head'],'source_only_tree':current['source_only_tree'],'source_manifest':m.row('tests/fixtures/typed_project_source_binding/current-source.json',manifest.read_bytes()),'patch':m.row('observer-lexer-reservation-v2.patch',patch),'predecessor_authority':m.row('byte_storage_source_authority.py',(p/'byte_storage_source_authority.py').read_bytes()),'predecessor_patch':m.row('observer-u8-v1.patch',prior_patch),'base_files':m.rows(inputs),'observer_files':m.rows(derived,True),'predecessor_base_files':m.rows(old),'predecessor_observer_files':m.rows(oldderived,True),'lexer_hook_correspondence':receipt,'inserted_roster':roster,'complete_inverse_sha256':m.sha(canonical(m.rows(m.exact_patch(derived,patch,True)))),'compiler_invocations':0,'execution_qualified':False}
# This equality is required for the approved cross-host-only source amendment.
m.need(patch == (p/'observer-lexer-reservation-v1.patch').read_bytes(), 'unexpected lifecycle overlay drift')
write_candidate('observer-lexer-reservation-v2.patch',patch)
proofraw=(json.dumps(proof,sort_keys=True,indent=2)+'\n').encode();write_candidate('lexer-reservation-lifecycle-v2.json',proofraw)
a=(p/'byte_storage_source_authority.py').read_text()
for oldvalue,newvalue in [('402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa',m.sha(manifest.read_bytes())),('04774d690abd8fa8813be24865b98e7e75ebcf326568efb0f8c6cd4698890bab',m.sha(canonical(m.rows(inputs)))),(m.PREDECESSOR_PATCH_SHA,m.sha(patch)),('8805b56dfd40fa835553b60d2b80484550c14b4214d95653f5507756185686fe',m.sha(canonical(m.rows(derived,True))))]:
    assert a.count(oldvalue)==1; a=a.replace(oldvalue,newvalue)
a+='\nLEXER_LIFECYCLE_AUTHORITY_SHA = '+repr(m.sha(proofraw))+'\nLEXER_LIFECYCLE_HELPER_SHA = '+repr(m.sha((p/'lexer_reservation_lifecycle.py').read_bytes()))+'\n'
write_candidate('authority.py',a.encode('utf-8'))
print(json.dumps({'patch':proof['patch'],'authority':m.row('lexer-reservation-lifecycle-v2.json',proofraw),'helper':m.row('lexer_reservation_lifecycle.py',(p/'lexer_reservation_lifecycle.py').read_bytes()),'base_files_sha':m.sha(canonical(m.rows(inputs))),'observer_files_sha':m.sha(canonical(m.rows(derived,True))),'counts':[len(inputs),len(derived)],'hook':receipt},indent=2))
