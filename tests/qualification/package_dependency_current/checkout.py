"""Dormant checkout source-only admission; historical Git objects are never read.

The externally reviewed entrypoint is the trust anchor. seal.py is hash-bound
literal data, not executable code, and does not reciprocally pin this entrypoint.
Actual new current source and old predecessor evidence remain separate.
"""
import hashlib
import json
import os
import ast
import stat
from pathlib import Path, PurePosixPath
import types
import sys

BASE = 'fc5515d09dce7e8cdb67a43c890272683f7b4755'
BASE_HEAD = '0fef9bf4c664c52f441e1c8b842edfcb57fa15d0'
C = 'ce522e393646ce6478848cee31f307cb4b6ad5e8'
CT = '8c3f4837e1fc89a6272ebee3552ce617ce584131'
A = '06fbffae6fa74ec08f5c3fcc761b15b393bb1b8e'
AT = 'a15ab7812a67d4f639f6c3427beeff216b0aa11a'
SOURCE = 'tests/fixtures/typed_project_source_binding/'
SAVED = 'tests/fixtures/typed_project_source_binding_byte_storage_v1/'
HELPERS = 'tests/qualification/lexer_reservation_current/'
OLD = 'lexer-reservation-source-v2.json'
PINS = {
 'package-dependency-source-v1.json': (74565, 'f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'),
 'package-dependency-authority-v1.json': (265515, 'a5b8ff80ce9610c558672e1745afa31b2552de040fa996acf10f21025bb98b5c'),
 'package-dependency-transition-v1.patch': (752, '3b123cfa29136244e62038e885761e68858d2b9b472bc14231d2e7a60a13586d'),
}
OLD_SHA = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
OLD_AUTH_SHA = 'a3853a2ba52b581bf80af53cb7ddd25e990deec87c221902e7b9d88f33b15c9c'
RUNNER_SHA = '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76'
SCANNER_SHA = '7664dc9ff6597c3c586154af3b3a38f095147d025b8fd6e544f6c7a93f250ff8'
RECIPE_SHA = '5d724e0c588901fce69c59cd923ae9b83f6cfecb37865d7d9256dec156062b24'
ACCOUNTING = ('src/frontend/declaration_index/resource.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/declaration_index/u8_reservation.rs')

class Reject(ValueError):
    pass

class NotReady(RuntimeError):
    pass

def need(ok, message):
    if not ok:
        raise Reject(message)

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def row(name, raw):
    return dict(path=name, bytes=len(raw), sha256=sha(raw))

def identity(name, raw):
    return dict(row(name, raw), mode='100644', git_blob=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest())

def regular(path):
    path = Path(path)
    need(path.is_absolute() and path.is_file() and not any(p.is_symlink() for p in (path, *path.parents)), 'nonregular input')
    need(not path.stat().st_mode & 0o111, 'executable data input')
    return path.read_bytes()

def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs)


SEAL_SHA = '1e12e4adf310ae73953a25f228e28477503b48af9082658b9ffee8ba6f810143'
HERE = Path(__file__).resolve().parent
DATA = HERE.parents[1] / 'fixtures/package_dependency_source_v1'

def safe_name(name):
    need(isinstance(name,str) and name and str(PurePosixPath(name))==name and not name.startswith('/') and '\\' not in name and '..' not in PurePosixPath(name).parts, 'unsafe relative name')
    return name

def verify_rows(bodies, rows):
    names=[safe_name(r['path']) for r in rows]
    need(len(names)==len(set(names)) and set(bodies)==set(names), 'body membership mismatch')
    need(all(set(r)=={'path','bytes','sha256'} and row(r['path'],bodies[r['path']])==r for r in rows), 'body identity mismatch')

def closed(root, names):
    root=Path(root)
    need(root.is_dir() and not any(p.is_symlink() for p in (root,*root.parents)), 'nonregular closed root')
    files=set();dirs=set()
    for parent,ds,fs in os.walk(root,followlinks=False):
        for name in ds+fs:
            p=Path(parent)/name;mode=p.lstat().st_mode
            need(not stat.S_ISLNK(mode) and (stat.S_ISREG(mode) or stat.S_ISDIR(mode)), 'special closed member')
            rel=p.relative_to(root).as_posix()
            if stat.S_ISDIR(mode):dirs.add(rel)
            else:
                need(not mode & 0o111,'executable closed member');files.add(rel)
    wantdirs={str(p) for n in names for p in PurePosixPath(n).parents if str(p)!='.'}
    need(files==set(names) and dirs==wantdirs,'closed files/directories mismatch')

def anchored_data():
    need(sys.flags.optimize==0 and __debug__,'normal Python required')
    closed(HERE,{'checkout.py','seal.py','predecessor-layout-v1.json','test_checkout.py','generate_source_v1.py'})
    need(sha(regular(HERE/'generate_source_v1.py'))==RECIPE_SHA,'changed preserved recipe')
    seal_raw=regular(HERE/'seal.py');need(sha(seal_raw)==SEAL_SHA,'seal pin')
    tree=ast.parse(seal_raw)
    need(len(tree.body)==1 and isinstance(tree.body[0],ast.Assign) and len(tree.body[0].targets)==1 and isinstance(tree.body[0].targets[0],ast.Name) and tree.body[0].targets[0].id=='SEAL','seal must be one literal assignment')
    seal=ast.literal_eval(tree.body[0].value)
    need(row('test_checkout.py',regular(HERE/'test_checkout.py'))==seal['test_body'],'test body pin')
    need(seal['source_head']==C and seal['source_tree']==CT and seal['recipe_head']==A and seal['base_tree']==BASE and seal['base_head']==BASE_HEAD,'seal provenance')
    layout_raw=regular(HERE/'predecessor-layout-v1.json');need(row('predecessor-layout-v1.json',layout_raw)==seal['predecessor_layout'],'layout pin')
    manifest_raw=regular(DATA/'package-manifest.json');need(row('package-manifest.json',manifest_raw)==seal['data_manifest'],'data manifest pin')
    manifest=decode(manifest_raw)
    need(manifest['schema']=='oxid-package-dependency-data-package-v1','data schema')
    raw={n:regular(DATA/n) for n in PINS}
    verify_rows(raw,manifest['files']);closed(DATA,set(PINS)|{'package-manifest.json'})
    for n,(size,digest) in PINS.items():need(len(raw[n])==size and sha(raw[n])==digest,'candidate data pin')
    layout=decode(layout_raw)
    need(layout['file_count']==619 and layout['total_bytes']==31445683 and layout['baseline_tree']==BASE,'layout identity')
    return seal,layout,raw

def retained_helper(references,name,digest):
    raw=references[SAVED+name];need(sha(raw)==digest,'retained executable pin')
    module=types.ModuleType('package_checkout_retained_'+name[:-3]);module.__file__=str(HERE/name)
    exec(compile(raw,module.__file__,'exec'),module.__dict__)
    return module

def verify_current(after, references, seal, layout, raw):
    """Pure-byte admission, no writes or historical Git reads."""
    # The pure-byte surface uses the same fixed data anchor as capture; test
    # callers cannot supply alternate seals/layouts or bypass artifact pins.
    expected_seal,expected_layout,expected_raw=anchored_data()
    need(seal==expected_seal and layout==expected_layout and raw==expected_raw,'unapproved admission data')
    current = decode(raw['package-dependency-source-v1.json'])
    authority = decode(raw['package-dependency-authority-v1.json'])
    patch = raw['package-dependency-transition-v1.patch']
    old_raw = references[SOURCE + OLD]
    need(sha(old_raw) == OLD_SHA, 'source-v2 manifest pin')
    old = decode(old_raw)
    old_authority_raw = references[SOURCE + 'lexer-reservation-authority-v2.json']
    need(sha(old_authority_raw) == OLD_AUTH_SHA, 'source-v2 authority pin')
    old_authority = decode(old_authority_raw)
    names = [r['path'] for r in old['files']]
    need(names == sorted(set(names)) and len(names) == 376, 'source roster')
    verify_rows(after,current['files'])
    need(list(after)==names,'ordered current body membership')
    # Authenticate all 243 reference bodies before executing retained declarations.
    selected=set(names)
    refs=[r for r in layout['files'] if r['path'] not in selected]
    need(len(refs)==243,'reference partition')
    verify_rows(references,refs)
    api=retained_helper(references,'run.py',RUNNER_SHA)
    scanner=retained_helper(references,'u8_cross_host.py',SCANNER_SHA)
    before,touched=api.apply_inverse_patch(after,patch,sha(patch),len(patch),('src/main.rs',))
    need(tuple(touched)==('src/main.rs',),'inverse touched scope')
    before=dict(sorted(before.items()))
    verify_rows(before,old['files'])
    verify_rows({**before,**references},layout['files'])
    need([n for n in names if before[n]!=after[n]]==['src/main.rs'],'one-file source delta')
    expected_current = dict(old, files=[row(n,b) for n,b in after.items()], purpose='Exact duplicate dependency alias validation source successor; execution unqualified', reviewed_source_head=C, source_only_tree=CT, package_dependency_predecessor_sha256=OLD_SHA, package_dependency_base_head=BASE_HEAD, package_dependency_base_tree=BASE)
    need(current == expected_current, 'current source semantics/provenance differ')
    restored, touched = api.apply_inverse_patch(after, patch, sha(patch), len(patch), ('src/main.rs',))
    need(restored == before and tuple(touched) == ('src/main.rs',), 'complete actual inverse failed')
    for inputs in (before, restored):
        try:
            api.apply_inverse_patch(inputs, patch, sha(patch), len(patch), ('src/main.rs',))
        except api.BindingError:
            pass
        else:
            raise Reject('wrong-stage/double inverse accepted')
    includes = scanner.include_directives(after, api)
    fixtures = [identity(n,b) for n,b in after.items() if n.startswith(('fixtures/','tests/fixtures/'))]
    accounting = [identity(n,after[n]) for n in ACCOUNTING]
    need(includes == scanner.include_directives(before, api) == old_authority['compile_time_include_directives'] and len(includes) == 136, 'include closure changed')
    need(fixtures == old_authority['compile_time_fixture_inputs'] and len(fixtures) == 78, 'fixture closure changed')
    need(accounting == old_authority['unit2_accounting_dependencies'], 'accounting closure changed')
    expected_authority = dict(schema='oxid-package-dependency-source-transition-v1', recipe='git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS', git_extra_safety_flags=['--no-textconv'], base_head=BASE_HEAD, base_tree=BASE, reviewed_source_head=C, source_only_tree=CT, adapter_head=A, predecessor_source=row(OLD, old_raw), current_source=row('package-dependency-source-v1.json',raw['package-dependency-source-v1.json']), transition_patch=row('package-dependency-transition-v1.patch',patch), transition_paths=['src/main.rs'], compiler_additions=[], fixture_additions=[], removed_paths=[], current_source_members=376, predecessor_source_members=376, compiler_source_members=288, compiler_bodies=291, current_input_identities=[identity(n,b) for n,b in after.items()], predecessor_input_identities=[identity(n,b) for n,b in before.items()], transition_inputs=[dict(path='src/main.rs',before=identity('src/main.rs',before['src/main.rs']),after=identity('src/main.rs',after['src/main.rs']))], compile_time_fixture_inputs=fixtures, compile_time_include_directives=includes, compile_time_include_additions=[], unit2_accounting_dependencies=accounting)
    need(authority == expected_authority, 'complete authority semantics differ')
    return dict(current_source=current, current_inputs=after, predecessor_source=old, predecessor_inputs=before, authority=authority, status='NotReady', execution_qualified=False, predecessor_preflight='not-run', remote_provenance=dict(method='reviewed seal anchored in previously authenticated connector proof', sha256=seal['remote_provenance_readback_sha256']))


def capture(repo):
    repo=Path(repo)
    need(repo.is_absolute() and repo==repo.resolve() and repo.is_dir(),'canonical checkout required')
    seal,layout,raw=anchored_data()
    source=decode(raw['package-dependency-source-v1.json'])
    names=[r['path'] for r in source['files']]
    for root in ('src','native'):
        closed(repo/root,{n[len(root)+1:] for n in names if n.startswith(root+'/')})
    after={n:regular(repo/safe_name(n)) for n in names}
    references={r['path']:regular(repo/safe_name(r['path'])) for r in layout['files'] if r['path'] not in after}
    # All predecessor package membership remains exact, including empty dirs.
    for prefix in (SOURCE,SAVED):
        names_in_package={n[len(prefix):] for n in references if n.startswith(prefix)}
        closed(repo/prefix,names_in_package)
    # The existing recipe is preserved at its original path. The dormant new
    # files do not rewrite or exempt it from future helper-root inventories.
    need(sha(regular(repo/'tests/qualification/package_dependency_current/generate_source_v1.py'))==RECIPE_SHA,'retained recipe differs')
    return verify_current(after,references,seal,layout,raw),references

def execution_context(*args, **kwargs):
    raise NotReady('Current execution remains unimplemented; old preflight never qualifies new source')


def materialize_predecessor(root, bodies):
    need(not root.exists(),'predecessor view exists')
    root.mkdir()
    for name,body in sorted(bodies.items()):
        path=root/safe_name(name);path.parent.mkdir(parents=True,exist_ok=True)
        with path.open('xb') as stream:stream.write(body)
    closed(root,bodies)
    for name,body in bodies.items():need(regular(root/name)==body,'written body differs')


def worker(root):
    import resource
    import runpy
    resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824))
    resource.setrlimit(resource.RLIMIT_CPU,(60,60))
    resource.setrlimit(resource.RLIMIT_FSIZE,(67108864,67108864))
    seal,layout,raw=anchored_data()
    root=Path(root);closed(root,{r['path'] for r in layout['files']})
    bodies={r['path']:regular(root/safe_name(r['path'])) for r in layout['files']}
    verify_rows(bodies,layout['files'])
    namespace=runpy.run_path(str(root/SOURCE/'run.py'),run_name='package_checkout_predecessor')
    historical=namespace['preflight'](root,root/SOURCE)
    old=decode(bodies[SOURCE+OLD])
    need(historical['current']==old,'wrong historical current')
    verify_rows(historical['inputs'],old['files'])
    need(len(historical['archived'])==117,'incomplete historical archive')
    closed(root,bodies)
    for name,body in bodies.items():need(regular(root/name)==body,'predecessor changed')
    print(json.dumps(dict(schema='oxid-package-checkout-predecessor-result-v1',source_sha256=OLD_SHA,selected_members=376,archived_members=117,current_execution_status='NotReady',execution_qualified=False),sort_keys=True))


def require_unchanged_capture(before, references, fresh, fresh_references):
    need(fresh==before and fresh_references==references,'checkout changed during preflight')


def admit(repo, output):
    """Checkout source-only admission; no compiler, generator or current execution."""
    import signal
    import subprocess
    repo=Path(repo);output=Path(output)
    # All actual current data and references admit before the first write.
    context,references=capture(repo)
    _,layout,_=anchored_data()
    before_view={**context['predecessor_inputs'],**references}
    verify_rows(before_view,layout['files'])
    need(output.is_absolute() and output==output.resolve(strict=False) and not output.exists(),'absent canonical output required')
    need(repo!=output and repo not in output.parents and output not in repo.parents,'output overlaps checkout')
    need(output.parent.is_dir() and not any(p.is_symlink() for p in output.parents),'nonregular output parent')
    output.mkdir()
    root=output/'predecessor';materialize_predecessor(root,before_view)
    scratch=output/'scratch';scratch.mkdir()
    env=dict(PATH='/usr/bin:/bin',HOME='/nonexistent',LANG='C.UTF-8',LC_ALL='C.UTF-8',PYTHONNOUSERSITE='1',TMPDIR=str(scratch))
    command=['/usr/bin/timeout','--kill-after=5s','90s','/usr/bin/python3','-B','-s',str(Path(__file__).resolve()),'--worker',str(root)]
    with (output/'stdout').open('xb') as stdout,(output/'stderr').open('xb') as stderr:
        process=subprocess.Popen(command,env=env,cwd=output,stdout=stdout,stderr=stderr,start_new_session=True)
        try:returncode=process.wait(timeout=100)
        finally:
            try:os.killpg(process.pid,signal.SIGKILL)
            except ProcessLookupError:pass
            process.wait()
    need(returncode==0,'predecessor preflight failed; evidence retained')
    need(not regular(output/'stderr'),'unexpected predecessor stderr')
    result=decode(regular(output/'stdout'))
    need(result==dict(schema='oxid-package-checkout-predecessor-result-v1',source_sha256=OLD_SHA,selected_members=376,archived_members=117,current_execution_status='NotReady',execution_qualified=False),'wrong predecessor evidence')
    fresh,fresh_references=capture(repo)
    require_unchanged_capture(context,references,fresh,fresh_references)
    closed(root,before_view)
    for name,body in before_view.items():need(regular(root/name)==body,'view changed after preflight')
    context['predecessor_preflight']='passed'
    context['historical_evidence']=result
    context['process_evidence']=dict(pid=process.pid,returncode=returncode,owned_group_cleanup='kill-and-reap-complete',command=command)
    return context

if __name__=='__main__':
    need(len(sys.argv)==3 and sys.argv[1]=='--worker','worker-only CLI')
    worker(sys.argv[2])
