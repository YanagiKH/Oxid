"""Review-time reproduction from the fixed PR65 Git commit; never fetches."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode=True
REPO=Path(__file__).absolute().parents[3]
ROOT='tests/qualification/stdlib_text_case_v1/'
HEAD='b416d460af657a53ca9419aefccbb2d29adafa97'
TREE='dea1fd6a8f2a7cdb071100d6e7e2edacf4120477'
EXPECTED=('scripts/test_lexer_current_ci.py', 'scripts/test_published_formatter_source.py', 'scripts/verify_owned_array_native.py', 'tests/fixtures/typed_project_source_binding/package-manifest.json', 'tests/fixtures/typed_project_source_binding/run.py', 'tests/qualification/lexer_reservation_current/test_current_source.py', 'tests/qualification/lexer_reservation_current/unit1_current_support.py', 'tests/qualification/package_json_current_v1/test_generation.py', 'tests/qualification/package_json_current_v1/test_hosted.py', 'tests/qualification/stdlib_trim_storage_v1/hosted-derivation.json', 'tests/qualification/stdlib_trim_storage_v1/hosted_controls.py', 'tests/qualification/stdlib_trim_storage_v1/lifecycle-proof.json', 'tests/qualification/stdlib_trim_storage_v1/lifecycle.py', 'tests/qualification/stdlib_trim_storage_v1/parser-authority.json', 'tests/qualification/stdlib_trim_storage_v1/predecessor.py', 'tests/qualification/unit4_ci/common.py', 'tests/qualification/unit4_ci/inputs-stdlib-trim-v1.json', 'tests/qualification/unit4_ci/inputs.json', 'tests/qualification/unit4_ci/test_integration.py', 'tests/qualification/unit4_ci/test_scheduling.py', 'tests/qualification/unit4_parser_current/authority.json', 'tests/qualification/unit4_parser_current/portable.py', 'tests/qualification/unit4_parser_current/test_current.py', 'tests/qualification/unit4_parser_current/test_lexer_reservation.py', 'tests/qualification/unit4_public_v3/authority.py', 'tests/qualification/unit4_public_v3/build.py', 'tests/qualification/unit4_public_v3/test_lexer_reservation.py')
EXTRA=('stdlib/strings.ox', 'tests/qualification/stdlib_trim_storage_v1/README.md', 'tests/qualification/stdlib_trim_storage_v1/activate.py', 'tests/qualification/stdlib_trim_storage_v1/archive/stdlib/strings.ox.data', 'tests/qualification/stdlib_trim_storage_v1/derive.py', 'tests/qualification/stdlib_trim_storage_v1/mechanical-inputs-v1.json', 'tests/qualification/stdlib_trim_storage_v1/retained.py', 'tests/qualification/stdlib_trim_storage_v1/review_inputs.py', 'tests/qualification/stdlib_trim_storage_v1/test_active.py', 'tests/qualification/stdlib_trim_storage_v1/test_retained.py', 'tests/qualification/stdlib_trim_storage_v1/test_storage.py', 'tests/stdlib_strings.rs')
STORAGE={'scripts/test_lexer_current_ci.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/scripts/test_lexer_current_ci.py', 'scripts/test_published_formatter_source.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/scripts/test_published_formatter_source.py', 'scripts/verify_owned_array_native.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/scripts/verify_owned_array_native.py', 'tests/fixtures/typed_project_source_binding/package-manifest.json': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/fixtures/typed_project_source_binding/package-manifest.json', 'tests/fixtures/typed_project_source_binding/run.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/fixtures/typed_project_source_binding/run.py', 'tests/qualification/lexer_reservation_current/test_current_source.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/lexer_reservation_current/test_current_source.py', 'tests/qualification/lexer_reservation_current/unit1_current_support.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/lexer_reservation_current/unit1_current_support.py', 'tests/qualification/package_json_current_v1/test_hosted.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/package_json_current_v1/test_hosted.py', 'tests/qualification/unit4_ci/common.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_ci/common.py', 'tests/qualification/unit4_ci/inputs.json': 'tests/qualification/unit4_ci/inputs-stdlib-trim-v1.json', 'tests/qualification/unit4_ci/test_integration.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_ci/test_integration.py', 'tests/qualification/unit4_ci/test_scheduling.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_ci/test_scheduling.py', 'tests/qualification/unit4_parser_current/authority.json': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_parser_current/authority.json', 'tests/qualification/unit4_parser_current/portable.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_parser_current/portable.py', 'tests/qualification/unit4_parser_current/test_current.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_parser_current/test_current.py', 'tests/qualification/unit4_parser_current/test_lexer_reservation.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_parser_current/test_lexer_reservation.py', 'tests/qualification/unit4_public_v3/authority.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_public_v3/authority.py', 'tests/qualification/unit4_public_v3/build.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_public_v3/build.py', 'tests/qualification/unit4_public_v3/test_lexer_reservation.py': 'tests/qualification/stdlib_text_case_v1/archive/pr65/tests/qualification/unit4_public_v3/test_lexer_reservation.py'}

def need(ok,message):
    if not ok:raise ValueError(message)
def git(*args):return subprocess.check_output(['git',*args],cwd=REPO)
def blob(kind,raw):return hashlib.sha1(kind.encode()+b' '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
def old(name):return git('show',HEAD+':'+name)

def generate():
    need(git('rev-parse',HEAD+'^{tree}').decode().strip()==TREE,'fixed PR65 tree')
    out={};rows=[];trees={}
    for name in sorted(set(EXPECTED)|set(EXTRA)|{'tests/qualification/package_json_current_v1/generation.py'}):
        raw=old(name);storage=STORAGE.get(name,name)
        rows.append(dict(path=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),
            git_blob=blob('blob',raw),mode='100644',storage=storage,
            expected=name in EXPECTED,producer_input=name in EXTRA))
        if storage.startswith(ROOT):out[storage]=raw
        else:need(old(storage)==raw,'PR65 unchanged storage alias')
        parts=name.split('/')
        for i in range(len(parts)):
            spec=HEAD+':'+('/'.join(parts[:i])) if i else HEAD+'^{tree}'
            oid=git('rev-parse',spec).decode().strip();trees[oid]=git('cat-file','tree',oid).hex()
    value=dict(schema='oxid-stdlib-text-pr65-expected-v1',head=HEAD,tree=TREE,
               commit_raw=git('cat-file','commit',HEAD).hex(),trees=trees,files=rows)
    out[ROOT+'pr65-expected-v1.json']=(json.dumps(value,indent=2,sort_keys=True)+'\n').encode()
    out[ROOT+'archive/stdlib/text.ox.data']=old('stdlib/text.ox')
    return out

if __name__=='__main__':
    need(sys.argv[1:]==['--check'],'use --check (read-only)')
    outputs=generate()
    for name,body in outputs.items():need((REPO/name).read_bytes()==body,'fixed PR65 fixture: '+name)
    print('PASS',len(outputs),'fixed PR65 review outputs')
