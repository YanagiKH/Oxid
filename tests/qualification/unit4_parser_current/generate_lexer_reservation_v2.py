#!/usr/bin/env python3
"""Approved source-only map recipe. No compiler or private PSI2 research access.

Run only after independent approval of the two fixed input checkpoints. This
regenerates reviewed-output candidates; its success is not independent review
or semantic qualification. It never edits compiler source or frozen helpers.
"""
# Source-only review recipe. No authority activation is authorized here.
# This recipe derives ONLY the pre-phase C2 predecessor authority. The exact
# outer phase authority/projection contract is a separately reviewed addendum.
# It does not write active authority.json or change portable.py AUTHORITY_SHA.
from pathlib import Path, PurePosixPath
import sys,importlib.util,json,copy,subprocess,tempfile,os,re,argparse
sys.dont_write_bytecode = True
root=Path(__file__).resolve().parents[3]; folder=root/'tests/qualification/unit4_parser_current'

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

spec=importlib.util.spec_from_file_location('current_parser',folder/'portable.py');p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)
p.require(not sys.flags.optimize, 'generator requires normal Python')
row=lambda n,b:{'path':n,'bytes':len(b),'sha256':p.sha(b)}
identity=lambda n:row(n,(root/n).read_bytes())
prior=p.read(folder/'byte-storage-authority-v1.json')
p.same(p.sha((folder/'byte-storage-authority-v1.json').read_bytes()),'446f9c022dd7ae3070a9fe8a07e4973fdbb12f4623b8a293b6826ed3ef576c5b','retained parser pin')
prior['source_binding_runner']=identity('tests/fixtures/typed_project_source_binding/run.py')
a=p.read(p.FROZEN/'authority.json');p.same(p.sha((p.FROZEN/'authority.json').read_bytes()),p.HISTORICAL_AUTHORITY_SHA,'frozen metadata pin')
p.verify_map(p.FROZEN,a['package_files']);p.verify_map(p.FROZEN/'frozen/helpers',a['helper_files'],exact=True)
a['current']=prior
api=p.u8_binding_api(prior)
newraw=(root/'tests/fixtures/typed_project_source_binding/current-source.json').read_bytes()
p.same(newraw,(root/'tests/fixtures/typed_project_source_binding/lexer-reservation-source-v2.json').read_bytes(),'source-v2 active alias')
p.same((len(newraw),p.sha(newraw)),(74328,'9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'),'accepted source-v2 candidate')
newsource=json.loads(newraw)
p.same((newsource['reviewed_source_head'],newsource['source_only_tree']),('b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489','7f5c9aa08c569c4d0b5a27391d8fc68337075d36'),'fixed C2/T2')
inputs={r['path']:subprocess.check_output(['git','show',newsource['reviewed_source_head']+':'+r['path']],cwd=root) for r in newsource['files']}
patch=(root/'tests/fixtures/typed_project_source_binding/lexer-reservation-transition-v2.patch').read_bytes(); outer=p.read(root/'tests/fixtures/typed_project_source_binding/lexer-reservation-authority-v2.json')
oldinputs,touched=api.apply_inverse_patch(inputs,patch,p.sha(patch),len(patch),tuple(outer['transition_paths']))
oldsource=p.read(root/'tests/fixtures/typed_project_source_binding/byte-storage-source-v1.json')
p.same([row(n,b) for n,b in sorted(oldinputs.items())],oldsource['files'],'exact complete old input recovery')
# Resolve original base bytes through their immutable Git identity; selected
# current bodies override them. No derived map is copied and rehashed.
original={}
for r in a['original_files']:
    n=r['path']; b=subprocess.check_output(['git','show',a['base_commit']+':'+n],cwd=root)
    p.same(row(n,b),r,'historical base actual bytes '+n);original[n]=b
base=original|inputs
active=copy.deepcopy(prior)
active.update(current_source_manifest=identity('tests/fixtures/typed_project_source_binding/current-source.json'),reviewed_source_head=newsource['reviewed_source_head'],source_only_tree=newsource['source_only_tree'],current_base_files=[row(n,b) for n,b in sorted(base.items())])
oldrows={r['path']:r for r in a['original_files']}
active['source_delta']=[{'path':r['path'],'before':oldrows.get(r['path']),'after':r} for r in newsource['files'] if oldrows.get(r['path'])!=r]
p.same([r['path'] for r in active['source_delta']],list(p.CURRENT_PATHS),'complete delta scope unchanged')
active['lexer_reservation']={k:identity(n) for k,n in {
'parser_predecessor':'tests/qualification/unit4_parser_current/byte-storage-authority-v1.json',
'source_predecessor':'tests/fixtures/typed_project_source_binding/byte-storage-source-v1.json',
'source_authority':'tests/fixtures/typed_project_source_binding/lexer-reservation-authority-v2.json',
'source_patch':'tests/fixtures/typed_project_source_binding/lexer-reservation-transition-v2.patch',
'observer_adapter':'tests/qualification/lexer_reservation_current/adapters.py',
'helper':'tests/qualification/unit4_parser_current/lexer_reservation.py'}.items()}
current_a=dict(a,current=active)
candidate=(json.dumps(p.current_candidate(current_a),sort_keys=True,indent=2)+'\n').encode()
active['current_candidate_source_manifest_sha256']=p.sha(candidate)
# Public retained composers authenticate unchanged frozen helper relationships.
# Their implementations and generated helper bodies remain opaque to this script.
helper=p.lexer_module(active).adapter(p.lexer_api(),active)
correspondence=[]
# Ask the authenticated public historical preparer for its unchanged generated
# patch through its existing command-line interface; inspect no helper source.
retained_patches = {}
with tempfile.TemporaryDirectory(prefix='unit4-lexer-public-prepare-', dir=candidate_output) as directory:
    historical_root = Path(directory) / 'historical'
    subprocess.run(['git', 'clone', '--shared', '--no-checkout', str(root), str(historical_root)], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    subprocess.run(['git', '-C', str(historical_root), 'update-ref', 'HEAD', a['base_commit']], check=True)
    subprocess.run(['git', '-C', str(historical_root), 'read-tree', a['base_commit']], check=True)
    for name, raw in original.items():
        target = historical_root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
    for control in (False, True):
        out = Path(directory) / ('control' if control else 'observer')
        command = [sys.executable, '-B', str(p.FROZEN / 'frozen/helpers/prepare.py'), '--repo', str(historical_root), '--output', str(out)]
        if control:
            command.append('--control')
        done = subprocess.run(command, env=dict(os.environ, PYTHONDONTWRITEBYTECODE='1'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        p.same(done.returncode, 0, 'public historical preparer failed: ' + done.stderr.decode())
        retained_patches[control] = (out / 'instrumentation.patch').read_bytes()
for control,field in [(False,'current_derived_files'),(True,'current_control_derived_files')]:
    derived=dict(base)
    retained={r['path']:r for r in a['control_derived_files' if control else 'derived_files']}
    # Only public named composers produce instrumented compiler bytes.
    for name in p.ARRAY_INSTRUMENTATION_PATHS:
        if control and name=='src/frontend/project/budget.rs':continue
        old=p.compose_array_instrumentation(a,name,oldinputs[name],control)
        expected=next(r for r in prior[field] if r['path']==name)
        p.same(row(name,old),expected,'old complete composition '+name)
        if name=='src/frontend/project/budget.rs':
            new=helper.replace_exact(inputs[name],helper.BUDGET_SEAMS)
            new,receipt=helper.instrument_budget(inputs[name],row(name,inputs[name]),row(name,new));correspondence.append(receipt)
        else:
            p.same(oldinputs[name],inputs[name],'unchanged nonlexer composition source');new=old
        derived[name]=new
    if not control:
        name='src/frontend/lexer.rs';old=p.compose_division_lexer(a,oldinputs[name]);p.same(row(name,old),next(r for r in prior[field] if r['path']==name),'old lexer full composition')
        new=helper.replace_exact(inputs[name],helper.TOKEN_SEAMS)
        new,receipt=helper.instrument_lexer(inputs[name],row(name,inputs[name]),row(name,new),'token');correspondence.append(receipt);derived[name]=new
        for name,composer in [('src/frontend/declaration_index/resource.rs',p.compose_namespace_resource),('src/frontend/source.rs',p.compose_source_read)]:
            p.same(oldinputs[name],inputs[name],'unchanged nonlexer observer body');new=composer(a,inputs[name]);p.same(row(name,new),next(r for r in prior[field] if r['path']==name),'unchanged nonlexer observer');derived[name]=new
    name='src/frontend/parser/conversions.rs';derived[name]=p.compose_u8_closed_policy(a,name,inputs[name])
    name='src/frontend/parser/unit4_observer.rs';derived[name]=p.compose_observer_initializer(a)
    name = 'instrumentation.patch'
    p.same(row(name, retained_patches[control]), retained[name], 'exact public historical patch bytes')
    derived[name] = retained_patches[control]
    derived['candidate-source-manifest.json']=candidate
    active[field]=[row(n,derived[n]) for n in sorted(derived,key=lambda n:PurePosixPath(n).parts)]
    p.same(len(active[field]),542,'derived complete count')
raw=(json.dumps(active,sort_keys=True,indent=2)+'\n').encode()
write_candidate('lexer-reservation-authority-v2.json',raw)
proof={'schema':'oxid-unit4-lexer-reservation-composition-v1','authority':row('lexer-reservation-authority-v2.json',raw),'base_count':len(base),'observer_count':len(active['current_derived_files']),'control_count':len(active['current_control_derived_files']),'hooks':correspondence,'candidate':row('candidate-source-manifest.json',candidate),'compiler_invocations':0,'execution_qualified':False}
write_candidate('lexer-reservation-composition-v2.json',(json.dumps(proof,sort_keys=True,indent=2)+'\n').encode('utf-8'))
print(json.dumps(proof,indent=2))
