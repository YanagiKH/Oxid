"""Review-time fixed Git input/overlay reproduction, never activation or fetch."""
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
ROOT='tests/qualification/stdlib_trim_storage_v1/'
Q='tests/qualification/package_json_current_v1/'
U='tests/qualification/unit4_ci/'
S='tests/fixtures/typed_project_source_binding/'
HEAD='8debaa96c0238e3376567fa2c76517fda9937037'
TREE='daa438a1c817793c528123153bbc320c7fb2aae1'
BASE='31f49568b1142dd58d80eef8f71faeeb99f32ba5'
BASE_TREE='c31f377ccca7ea34b8f741e8ce725975108e481d'
CONSUMERS=(
 '.github/workflows/ci.yml','scripts/test_bounded_enum_native.py',
 'scripts/test_lexer_reservation_adapters_current.py','scripts/test_owned_array_native.py',
 'scripts/test_record_composition_runtime_view.py','scripts/verify_bounded_byte_storage.py',
 'scripts/verify_bounded_enum_native.py','scripts/verify_bounded_stdin_native.py',
 'scripts/verify_bounded_stdout_native.py','scripts/verify_owned_array_native.py',
 'tests/qualification/lexer_reservation_current/test_current_source.py',
 'tests/qualification/lexer_reservation_current/unit1_current_bindings.json',
 'tests/qualification/lexer_reservation_current/unit1_current_support.py',
 'tests/qualification/unit2_u8_current/test_current.py',
 'tests/qualification/unit4_parser_current/test_current.py',
 'tests/qualification/unit4_parser_current/test_lexer_reservation.py',
 'tests/qualification/unit4_public_v3/run.py')
OVERLAYS=(
 'scripts/test_lexer_current_ci.py','scripts/test_published_formatter_source.py',
 'scripts/verify_owned_array_native.py',S+'run.py',S+'package-manifest.json',
 'tests/qualification/lexer_reservation_current/test_current_source.py',
 'tests/qualification/lexer_reservation_current/unit1_current_support.py',
 Q+'test_generation.py',Q+'test_hosted.py',U+'common.py',U+'test_integration.py',U+'test_scheduling.py',
 'tests/qualification/unit4_parser_current/authority.json',
 'tests/qualification/unit4_parser_current/portable.py',
 'tests/qualification/unit4_parser_current/test_current.py',
 'tests/qualification/unit4_parser_current/test_lexer_reservation.py',
 'tests/qualification/unit4_public_v3/authority.py',
 'tests/qualification/unit4_public_v3/build.py',
 'tests/qualification/unit4_public_v3/test_lexer_reservation.py')
PRESERVED=(U+'inputs-package-json-v1.json',Q+'package-json-direct-consumer-dispositions-v1.json',
 Q+'source-proof/consumer-derivation-v1.json',Q+'input-roster-v1.json',
 U+'gate.py',U+'join.py',U+'evidence.py',S+'current-source.json')
GIT_GENERATE=('.github/workflows/ci.yml','scripts/test_lexer_current_ci.py',
 'scripts/test_owned_array_native.py',U+'common.py',U+'inputs.json')
GIT_ARCHIVES=('.github/workflows/ci.yml','scripts/test_lexer_current_ci.py',
 'scripts/test_owned_array_native.py',U+'common.py',Q+'accepted-view-v1.json')


def git(*args):return subprocess.check_output(['git',*args],cwd=REPO)
def old(commit,name):return git('show',commit+':'+name)
def sha(raw):return hashlib.sha256(raw).hexdigest()
def blob(kind,raw):return hashlib.sha1(kind.encode()+b' '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
def identity(n,b):return dict(path=n,bytes=len(b),sha256=sha(b),git_blob=blob('blob',b),mode='100644')
def need(ok,message):
    if not ok:raise ValueError(message)


def generate():
    for commit,tree in ((HEAD,TREE),(BASE,BASE_TREE)):
        need(git('rev-parse',commit+'^{tree}').decode().strip()==tree,'fixed Git baseline tree')
    ds=ast.parse(old(HEAD,U+'scheduling_derivation.py'))
    relocation=ast.literal_eval(next(n.value for n in ds.body if isinstance(n,ast.Assign)
        and isinstance(n.targets[0],ast.Name) and n.targets[0].id=='IDENTITY_FIXTURE_RELOCATION'))
    before=json.loads(old(BASE,U+'inputs.json'))
    names={row['path'] for row in before['files']}|{Q+'package-json-scheduling-dispositions-v1.json'}|set(relocation)
    early={U+'integration-roster-v1.json','scripts/test_owned_array_native.py','.github/workflows/ci.yml',
        Q+'package-json-scheduling-dispositions-v1.json','scripts/test_lexer_current_ci.py'}
    data=(names-early)|set(relocation)|{U+'integration-roster-v1.json',
        'tests/qualification/published_formatter_current/published-direct-consumer-dispositions-v1.json',
        Q+'predecessor-v1/.github/workflows/ci.yml',S+'current-source.json'}
    need(len(data)==1048,'fixed complete scheduling data membership')
    expected=early|{U+'inputs-scheduling-v1.json',U+'inputs.json',U+'common.py',
        Q+'hosted_controls.py','scripts/test_published_formatter_source.py',Q+'source-proof/hosted-control-derivation-v1.json'}
    code={U+'derive_scheduling.py',U+'scheduling_derivation.py',Q+'generation.py',Q+'derive_consumers.py',Q+'ci.py',Q+'derive_hosted.py'}
    data|={Q+'predecessor-v1/'+n for n in CONSUMERS}|{Q+'source-proof/'+n+'.json' for n in ('accepted-main','source-checkpoint')}
    data|={S+n for n in ('current-source.json','run.py','package-manifest.json')}|{Q+'accepted-view-v1.json'}
    data|={Q+'predecessor-v1/scripts/test_published_formatter_source.py',Q+'source-seal-v1.json',
           Q+'hosted_controls.py','scripts/test_published_formatter_source.py',Q+'source-proof/hosted-control-derivation-v1.json'}
    data|=set(PRESERVED)|{Q+'accepted-view-v1.json',Q+'predecessor-v1/'+U+'test_source_identity.py'}
    testcode={Q+'test_hosted.py',U+'test_scheduling.py'}
    activation={U+'test_integration.py'}
    need(len(data)==1052,'fixed mechanical data union')
    rows=[];archives={}
    for n in sorted(data|expected|code|testcode|activation):
        b=old(HEAD,n);row=identity(n,b);storage=n
        if n in OVERLAYS:storage=ROOT+'archive/pr64/'+n;archives[storage]=b
        if n==U+'inputs.json':storage=U+'inputs-scheduling-v1.json';need(old(HEAD,storage)==b,'fixed scheduling alias')
        roles=[]
        for role,domain in [('data',data),('expected',expected),('code',code),('test-code',testcode),('activation-input',activation)]:
            if n in domain:roles.append(role)
        if n==Q+'hosted_controls.py':roles.append('executed-generated-body')
        rows.append(dict(row,storage=storage,roles=roles))
    gitrows=[];trees={};commit=git('cat-file','commit',BASE)
    for n in sorted(set(GIT_GENERATE)|set(PRESERVED)|{U+'test_source_identity.py',Q+'accepted-view-v1.json'}):
        b=old(BASE,n);storage=n
        if n in GIT_ARCHIVES:storage=ROOT+'archive/base31f/'+n;archives[storage]=b
        elif n==U+'inputs.json':storage=U+'inputs-package-json-v1.json'
        elif n==U+'test_source_identity.py':storage=Q+'predecessor-v1/'+n
        if n not in GIT_ARCHIVES:need(old(HEAD,storage)==b,'fixed Git alias body')
        gitrows.append(dict(identity(n,b),commit=BASE,storage=storage))
        parent=''
        for component in n.split('/')[:-1]+['']:
            oid=git('rev-parse',BASE+(':'+parent if parent else '^{tree}')).decode().strip()
            trees[oid]=git('cat-file','tree',oid).hex()
            if not component:break
            parent=parent+'/'+component if parent else component
    value=dict(schema='oxid-trim-pr64-mechanical-inputs-v1',baseline=HEAD,baseline_tree=TREE,
               git_base=BASE,git_base_tree=BASE_TREE,base_commit_raw=commit.hex(),base_trees=trees,
               rows=rows,git_rows=gitrows)
    raw=(json.dumps(value,sort_keys=True,indent=2)+'\n').encode()
    return {ROOT+'mechanical-inputs-v1.json':raw,**archives}


if __name__=='__main__':
    need(sys.argv[1:] in (['--check'],['--write']),'use --check or --write')
    outputs=generate()
    for name,raw in outputs.items():
        path=REPO/name
        if sys.argv[1]=='--write':path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw)
        else:need(path.read_bytes()==raw,'retained input differs: '+name)
    raw=outputs[ROOT+'mechanical-inputs-v1.json']
    print('PASS',len(outputs),'fixed retained metadata/archive outputs;',len(raw),sha(raw))
