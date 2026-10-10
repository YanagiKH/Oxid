#!/usr/bin/env python3
"""Reproduce the independent Unit1 corpus without editing the supplied repository.

Uses Python's standard library and the repository's existing Rust dependencies.
Unsupported hosts exit77 and report a skipped result; they are never qualified.
"""
if not __debug__:
    raise SystemExit('Qualification refuses optimized Python because it can remove verification guards')
import argparse,hashlib,json,os,platform,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode = True
import unit1_current_support as current
rust_string_literal = current.rust_string_literal
PACKAGE=current.original_package()


def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save_json(p,value):p.write_text(json.dumps(value,indent=2)+'\n')
def run(argv,cwd,env=None):return subprocess.run(argv,cwd=cwd,env=env,capture_output=True,text=True)
def checked(argv,cwd,env=None):
    r=run(argv,cwd,env)
    if r.returncode:raise RuntimeError(f'Command failed ({r.returncode}): {argv!r}\n{r.stdout}\n{r.stderr}')
    return r

def main():
    current.authenticate()
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--repo',type=Path,required=True,help='Repository or frozen source snapshot; never modified')
    ap.add_argument('--profile',choices=['debug','release','both'],default='both')
    ap.add_argument('--output',type=Path,help='New or empty directory for evidence (default: unique temporary results directory)')
    ap.add_argument('--target-dir',type=Path,help='Optional isolated reusable Cargo target cache')
    ap.add_argument('--cargo',default='cargo',help='Cargo executable; Rust toolchain and CARGO_HOME are inherited')
    args=ap.parse_args()
    repo=args.repo.resolve()
    current_admission=current.admit(repo)
    temp_parent=Path(tempfile.gettempdir()).resolve()
    if temp_parent==repo or repo in temp_parent.parents:ap.error('TMPDIR must be outside --repo')
    try:str(temp_parent).encode('utf-8')
    except UnicodeEncodeError:ap.error('TMPDIR must be representable as a UTF-8 invoked display path')
    if args.output and (args.output.resolve()==repo or repo in args.output.resolve().parents):ap.error('--output must be outside --repo')
    if args.target_dir and (args.target_dir.resolve()==repo or repo in args.target_dir.resolve().parents):ap.error('--target-dir must be outside --repo')
    if not (repo/'Cargo.toml').is_file() or not (repo/'src/frontend/project.rs').is_file():ap.error('--repo must contain the Unit1 Oxid source')
    output=args.output.resolve() if args.output else Path(tempfile.mkdtemp(prefix='oxid-unit1-results-'))
    if output.exists() and any(output.iterdir()):ap.error('--output must be new or empty')
    output.mkdir(parents=True,exist_ok=True)
    host={'system':sys.platform,'machine':platform.machine(),'pointer_bytes':__import__('struct').calcsize('P')}
    if sys.platform!='linux' or platform.machine()!='x86_64' or host['pointer_bytes']!=8:
        save_json(output/'result.json',{'status':'skipped','host':host,'reason':'Independent filesystem/layout qualification requires Linux x86_64','test_functions_executed':0})
        print(f'Skipped: Linux x86_64 qualification required. Evidence: {output}');return 77
    with tempfile.TemporaryDirectory(prefix='oxid-unit1-work-') as work_text:
        work=Path(work_text);source=work/'source';q=work/'qualification';q.mkdir()
        # Copy the authorized code input, excluding generated/build/tool state.
        # The runner writes only its unique workspace and caller-chosen evidence/cache.
        def ignore(path,names):
            return [n for n in names if n in {'.git','.codex','.agents','__pycache__'} or n=='target' or n.startswith('target-') or (Path(path)/n).resolve()==output or (args.target_dir and (Path(path)/n).resolve()==args.target_dir.resolve())]
        shutil.copytree(repo,source,ignore=ignore)
        source_files={str(p.relative_to(source)):digest(p) for p in sorted(source.rglob('*')) if p.is_file()}
        save_json(q/'input-source-manifest.json',{'repo':str(repo),'files':source_files})
        for name in current.ORIGINAL_NAMES:
            shutil.copyfile(PACKAGE/name,q/name)
        current.stage(q)
        current.check_admission(source,current_admission)
        summary={'status':'running','host':host,'profiles':{},'test_functions_per_profile':69,'socket_status':'unavailable; no creation attempted','package_files':{p.name:digest(p) for p in PACKAGE.iterdir() if p.is_file()},'source_manifest_sha256':digest(q/'input-source-manifest.json')}
        summary.update(current.summary_fields(current_admission))
        try:
            for name in ['materialize_fixtures.py','materialize_resource_fixtures.py','origin_oracle.py','generate_reviewer_tests.py']:
                checked([sys.executable,str(q/name)],q)
            actual=json.loads((q/'fixture-manifest.json').read_text())
            for f in actual['fixtures']:
                f['expected'].pop('canonical_root_file_hex',None);f['expected'].pop('canonical_child_hex',None)
            assert actual==json.loads((PACKAGE/'expected-fixtures.json').read_text()),'Frozen source/diagnostic fixture expectations changed'
            assert (q/'resource-fixture-manifest.json').read_bytes()==(PACKAGE/'expected-resources.json').read_bytes(),'Frozen resource source/count expectations changed'
            test=q/'reviewer_cases.rs'
            with test.open('a') as f:f.write('\ninclude!("reviewer_additional.rs");\ninclude!("reviewer_v2.rs");\n')
            current.include_control(test)
            project=source/'src/frontend/project.rs'
            if 'mod reviewer_unit1;' in project.read_text():raise RuntimeError('Input already contains the scratch review include; supply the frozen production core')
            with project.open('a') as f:
                f.write('\n#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]\n#[path = '+rust_string_literal(str(test))+']\nmod reviewer_unit1;\n')
            compiled_sources={rel:digest(source/rel) for rel in source_files}
            save_json(q/'compiled-source-manifest.json',{'files':compiled_sources,'test_include':'Only the isolated project.rs gained the qualification module'})
            env=dict(os.environ);env['CARGO_TARGET_DIR']=str(args.target_dir.resolve() if args.target_dir else work/'target')
            tool=checked([args.cargo,'--version','--verbose'],q,env);(q/'cargo-version.txt').write_text(tool.stdout)
            rustc=env.get('RUSTC','rustc');r=run([rustc,'--version','--verbose'],q,env);(q/'rustc-version.txt').write_text(r.stdout+r.stderr)
            if r.returncode or not r.stdout.startswith('rustc '):raise RuntimeError('Rust compiler version could not be established')
            for profile in (['debug','release'] if args.profile=='both' else [args.profile]):
                current.profile_guard(repo,current_admission,source,compiled_sources,q)
                checked([sys.executable,str(q/'check_resource_inventory.py'),'--output',f'before-{profile}-inventory.json'],q)
                argv=[args.cargo,'test','--offline','--manifest-path',str(source/'Cargo.toml'),'--bin','oxid','--no-run','--message-format=json']
                if profile=='release':argv.append('--release')
                begin=time.monotonic();build=run(argv,source,env)
                (q/f'build-{profile}.stdout').write_text(build.stdout);(q/f'build-{profile}.stderr').write_text(build.stderr)
                if build.returncode:raise RuntimeError(f'{profile} build failed with exit{build.returncode}')
                executables=[]
                for line in build.stdout.splitlines():
                    try:event=json.loads(line)
                    except json.JSONDecodeError:continue
                    if event.get('reason')=='compiler-artifact' and event.get('profile',{}).get('test') and event.get('executable') and event.get('target',{}).get('name')=='oxid':executables.append(event['executable'])
                if len(executables)!=1:raise RuntimeError(f'Expected one test executable; found{len(executables)}')
                binary=Path(executables[0]);binary_sha=digest(binary)
                checked([sys.executable,str(q/'unit1_run_reviewer_current.py'),'--binary',str(binary),'--tag',profile],q)
                results=json.loads((q/f'{profile}-results.json').read_text())
                if results['total']!=69 or results['passed']!=69:raise RuntimeError(f'{profile}: {results["passed"]}/{results["total"]} tests passed')
                current.check_profile_results(results)
                assert digest(binary)==binary_sha,'Test executable changed during the run'
                checked([sys.executable,str(q/'check_resource_inventory.py'),'--output',f'after-{profile}-inventory.json'],q)
                before=json.loads((q/f'before-{profile}-inventory.json').read_text())['results'];after=json.loads((q/f'after-{profile}-inventory.json').read_text())['results'];assert before==after
                current.profile_guard(repo,current_admission,source,compiled_sources,q)
                summary['profiles'][profile]={'passed':69,'total':69,'current_controls':{'passed':1,'total':1},'binary_sha256':binary_sha,'elapsed_seconds':time.monotonic()-begin}
                print(f'{profile}:69/69 original cases; 1/1 current control passed',flush=True)
            current.check_admission(repo,current_admission)
            # Check exact supplied code bytes again; no source edit is permitted.
            for rel,expected in source_files.items():assert digest(repo/rel)==expected,f'Original source changed during execution: {rel}'
            for rel,expected in compiled_sources.items():assert digest(source/rel)==expected,f'Compiled source changed during execution: {rel}'
            summary['status']='passed';summary['test_functions_executed']=sum(x['total'] for x in summary['profiles'].values())
            summary['current_control_test_functions_executed']=sum(x['current_controls']['total'] for x in summary['profiles'].values())
            return_code=0
        except Exception as error:
            summary['status']='failed';summary['error']=str(error);return_code=1
        finally:
            # Keep proof/logs and generated test source. Large source/fixture and
            # build trees belong to the per-run workspace, which is then removed.
            for p in q.iterdir():
                if p.is_file():shutil.copyfile(p,output/p.name)
                elif p.is_dir() and p.name.endswith('-logs'):shutil.copytree(p,output/p.name)
            save_json(output/'result.json',summary)
            shutil.copyfile(PACKAGE/'expected-fixtures.json',output/'expected-fixtures.json')
            shutil.copyfile(PACKAGE/'expected-resources.json',output/'expected-resources.json')
        print(f'Evidence: {output}',flush=True)
        if return_code:print(summary.get('error','Qualification failed'),file=sys.stderr)
        return return_code
if __name__=='__main__':raise SystemExit(main())
