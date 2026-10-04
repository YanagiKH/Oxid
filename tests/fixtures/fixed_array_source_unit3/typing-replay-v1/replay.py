#!/usr/bin/env python3
"""Replay the frozen source/type observer through verified public source inputs."""
import sys
sys.dont_write_bytecode = True
import argparse, hashlib, importlib.util, io, json, os, pathlib, shutil, subprocess, tarfile

HERE = pathlib.Path(__file__).resolve().parent
BASE = '2e84c9de9d9b02b62283fff1902cf1a840254ac4'
TREE = '86b90e5cb5eaf03a81f9faaa3b4f494248d2c60c'
PUBLIC = '2cba1dbd200d75fbeb33b504b08279d89d2baa1784f88718413305446d458c35'
HISTORICAL = '7cec518fdc38c02f8cfc6c3cf467239423621bf242e44c3e838be490a3060039'
CORE = 'f44586df897fb27b0b47684f32108f1c6d24eede7b852730308a48939992c101'
COMPARE = '1c96f181be1dc92889b3738374954376ff93c7bcbff9181fa6d1b0dc2298157a'
LIMITS = [200000, 16777216, 33554432, 16777216, 256000000]
SEMANTIC = ['COMPARISON.md','OBSERVER.md','RESOURCE.md','child-diagnostics.json','fence-controls.json','positive-facts.json','resource-controls.json','reused-authority.json','source-manifest.json']
PACKAGE_PATH = 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1'
TEST = 'frontend::oir::owned::source::array_typing_observer::array_typing_observe_requests'

def need(condition, message):
    if not condition:
        raise RuntimeError(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def digest(path):
    with pathlib.Path(path).open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()

def pairs(items):
    result = {}
    for key, value in items:
        need(key not in result, 'duplicate JSON key: ' + key)
        result[key] = value
    return result

def read(path):
    data = pathlib.Path(path).read_bytes()
    need(len(data) <= 16*1024*1024, 'metadata byte limit')
    return json.loads(data, object_pairs_hook=pairs)

def write(path, data):
    with pathlib.Path(path).open('xb') as f:
        f.write(data)

def report(path, data):
    write(path, (json.dumps(data, indent=2, sort_keys=True)+'\n').encode())

def run_logged(command, log, *, cwd=None, env=None, stdin=None):
    with pathlib.Path(log).open('xb') as output:
        result = subprocess.run(command, cwd=cwd, env=env, input=stdin, stdout=output, stderr=subprocess.STDOUT)
    need(result.returncode == 0, 'command failed; preserved log: ' + str(log))
    return result.returncode

def package_check():
    manifest = read(HERE/'freeze-manifest.json')
    actual = {p.relative_to(HERE).as_posix() for p in HERE.rglob('*') if p.is_file()}
    need(actual == set(manifest['files']) | {'freeze-manifest.json'}, 'exact replay package membership')
    for name, entry in manifest['files'].items():
        path = HERE/name
        need(not path.is_symlink() and path.is_file(), 'ordinary package member required')
        need(path.stat().st_size == entry['bytes'] and digest(path) == entry['sha256'], 'package member changed: ' + name)
    need(manifest['base'] == BASE and manifest['base_tree'] == TREE, 'selected base identity')
    need(manifest['public_input_sha256'] == PUBLIC and manifest['historical_authority_sha256'] == HISTORICAL, 'input lineage')
    need(digest(HERE/'components/compare.py') == COMPARE, 'unchanged comparison predicates')
    return manifest

def source_check(root):
    closure = read(HERE/'components/full-source-closure.json')
    need(closure['base'] == BASE and closure['base_tree'] == TREE, 'source closure base')
    actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
    need(actual == {x['path'] for x in closure['files']}, 'exact full source closure')
    for entry in closure['files']:
        p = root/entry['path']
        need(not p.is_symlink() and p.stat().st_size == entry['bytes'] and digest(p) == entry['sha256'], 'source closure changed: ' + entry['path'])

def import_comparator():
    spec = importlib.util.spec_from_file_location('array_typing_comparison', HERE/'components/compare.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

class PublicAuthority:
    """Exact public projection; does not claim historical package membership."""
    def __init__(self, comparator, package, fixtures):
        self.root = package
        self.read = lambda name: comparator.read_json(package/name)
        self.manifest = self.read('freeze-manifest.json')
        need(digest(package/'freeze-manifest.json') == PUBLIC, 'public package identity')
        need(digest(package/'historical-freeze-manifest.json') == HISTORICAL, 'historical manifest identity')
        historical = self.read('historical-freeze-manifest.json')
        self.projection = {}
        for name in SEMANTIC:
            raw = (package/name).read_bytes()
            entry = historical['files'][name]
            need(len(raw) == entry['bytes'] and sha(raw) == entry['sha256'], 'semantic projection differs: ' + name)
            self.projection[name] = dict(bytes=len(raw), sha256=sha(raw))
        ledger = HERE/'components/project-work-ledger-source-v1.json'
        need(digest(ledger) == comparator.PROJECT_LEDGER_SHA, 'source-derived project work ledger')
        self.cases = self.read('source-manifest.json')['cases']
        self.by_id = {c['id']: c for c in self.cases}
        need(len(self.cases) == len(self.by_id) == 54 and sum(len(c['files']) for c in self.cases) == 65, 'exact source roster')
        self.positive = {c['id']: c for c in self.read('positive-facts.json')['cases']}
        reused = self.read('reused-authority.json')
        self.reused = {c['id']: c['authority_case']['expected'] for c in reused['selected_cases']}
        self.diagnostics = {c['id']: c for c in reused['language_diagnostics']}
        self.diagnostics.update({c['id']: c for c in self.read('child-diagnostics.json')['cases']})
        self.resources = {c['id']: c for c in self.read('resource-controls.json')['cases']}
        self.fences = {c['id']: c for c in self.read('fence-controls.json')['cases']}
        self.source = {}
        actual = {p.relative_to(fixtures).as_posix() for p in fixtures.rglob('*') if p.is_file()}
        expected = {c['id']+'/'+f['path'] for c in self.cases for f in c['files']}
        need(actual == expected, 'exact materialized source roster')
        for c in self.cases:
            values = []
            for position, f in enumerate(c['files']):
                need(f['file'] == position, 'source file order')
                path = fixtures/c['id']/f['path']
                need(not path.is_symlink(), 'ordinary materialized source')
                raw = path.read_bytes()
                need(len(raw) == f['bytes'] and sha(raw) == f['sha256'], 'materialized source identity')
                raw.decode('utf-8')
                values.append(raw)
            self.source[c['id']] = values

def output_path(path, repo):
    resolved = path.resolve()
    need(not resolved.is_relative_to(HERE), 'output must be outside the immutable replay package')
    need(not resolved.is_relative_to(repo.resolve()), 'output must be outside the input Git repository')
    need(not resolved.exists(), 'replay output directory must be absent')
    return resolved

def replay(args):
    manifest = package_check()
    review_raw = args.review.read_bytes()
    need(digest(HERE/'freeze-manifest.json').encode() in review_raw and b'PASS' in review_raw, 'review must admit this exact public wrapper package')
    out = output_path(args.out, args.repo)
    out.mkdir(parents=True)
    try:
        git = ['git', '-C', str(args.repo.resolve())]
        actual_tree = subprocess.check_output(git+['rev-parse', BASE+'^{tree}'], text=True).strip()
        need(actual_tree == TREE, 'exact selected Git tree')
        archive = subprocess.check_output(git+['archive', BASE])
        source = out/'source'; source.mkdir()
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
            need(all(x.isfile() or x.isdir() for x in tar.getmembers()), 'archive must contain ordinary files/directories')
            tar.extractall(source, filter='data')
        patch = (HERE/'components/candidate.patch').read_bytes()
        run_logged(['git','apply','--check','-'], out/'patch-check.log', cwd=source, stdin=patch)
        run_logged(['git','apply','-'], out/'patch-apply.log', cwd=source, stdin=patch)
        source_check(source)
        package = source/PACKAGE_PATH
        need(digest(package/'freeze-manifest.json') == PUBLIC, 'materialized public input package')
        fixtures = out/'fixtures'
        run_logged([sys.executable,'-B','-O',str(package/'verify-inputs.py'),'--repo',str(args.repo.resolve()),'--materialize-sources',str(fixtures)], out/'verify-materialize.log')
        verified = read(out/'verify-materialize.log')
        need(verified['status'] == 'SCOPED_REPLAY_INPUT_CLOSURE_PASS' and verified['source_cases'] == 54 and verified['source_bodies'] == 65, 'public input verification result')
        cmp = import_comparator()
        authority = PublicAuthority(cmp, package, fixtures)
        report(out/'projection.json', dict(public_input_sha256=PUBLIC,historical_authority_sha256=HISTORICAL,historical_membership_claimed=False,semantic_files=authority.projection,source_cases=54,source_bodies=65))
        requests = []
        for c in authority.cases:
            requests.append(dict(id=c['id'],scope=c['scope'],source_root=str(fixtures/c['id']),limits=LIMITS,files=c['files']))
        tsv = ''.join('\t'.join([r['id'],r['scope'],r['source_root'],*map(str,r['limits'])])+'\n' for r in requests).encode()
        need(0 < len(tsv) <= 256*1024, 'request descriptor bound')
        write(out/'requests.tsv', tsv)
        report(out/'request-manifest.json', dict(schema='oxid-array-typing-public-requests-v1',public_input_sha256=PUBLIC,historical_authority_sha256=HISTORICAL,request_sha256=sha(tsv),request_bytes=len(tsv),requests=requests))
        binary_dir = out/'bin'; binary_dir.mkdir()
        binary = binary_dir/(args.profile+'-test')
        if args.reuse_binary:
            expected = manifest['qualified_binaries'][args.profile]
            need(args.reuse_binary.stat().st_size == expected['bytes'] and digest(args.reuse_binary) == expected['sha256'], 'only exact reviewed binary reuse is accepted')
            shutil.copy2(args.reuse_binary, binary)
            build = dict(mode='exact-reviewed-binary-reuse',profile=args.profile,binary_sha256=digest(binary),binary_bytes=binary.stat().st_size,source_closure_sha256=digest(HERE/'components/full-source-closure.json'))
        else:
            env = os.environ.copy(); env.update(CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='1', CARGO_TARGET_DIR=str(out/'target'))
            command = ['cargo','test','--no-run','--locked','--bin','oxid']
            if args.profile == 'release': command.insert(2,'--release')
            run_logged(command,out/'build.log',cwd=source,env=env)
            candidates = [p for p in (out/'target'/args.profile/'deps').glob('oxid-*') if p.is_file() and '.' not in p.name and os.access(p,os.X_OK)]
            need(len(candidates) == 1, 'exact built observer binary')
            shutil.copy2(candidates[0],binary)
            build = dict(mode='fresh-source-build',profile=args.profile,command=command,binary_sha256=digest(binary),binary_bytes=binary.stat().st_size,source_closure_sha256=digest(HERE/'components/full-source-closure.json'),rustc_vV=subprocess.check_output(['rustc','-vV'],text=True),cargo_vV=subprocess.check_output(['cargo','-vV'],text=True),build_log_sha256=digest(out/'build.log'))
        source_check(source)
        report(out/'build.json',build)
        observation_dir = out/'observations'
        command = [str(binary),'--exact',TEST,'--ignored','--nocapture','--test-threads=1']
        env = os.environ.copy(); env.update(OXID_ARRAY_TYPING_REQUEST=str(out/'requests.tsv'),OXID_ARRAY_TYPING_OUTPUT=str(observation_dir),PYTHONDONTWRITEBYTECODE='1')
        with (out/'observe.log').open('xb') as log:
            process = subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT)
        log = (out/'observe.log').read_bytes()
        cmp.execution_witness(log.decode('utf-8'),command,process.returncode,binary)
        outputs = {p.stem:dict(file=p.name,bytes=p.stat().st_size,sha256=digest(p)) for p in observation_dir.iterdir() if p.is_file()}
        cmp.roster_check(authority,requests,outputs,[p.name for p in observation_dir.iterdir()])
        need(all(not p.is_symlink() for p in observation_dir.iterdir()), 'ordinary observation artifacts')
        report(out/'run.json',dict(schema='oxid-array-typing-public-run-v1',base=BASE,base_tree=TREE,core_manifest_sha256=CORE,public_input_sha256=PUBLIC,historical_authority_sha256=HISTORICAL,request_manifest_sha256=digest(out/'request-manifest.json'),request_sha256=sha(tsv),build_sha256=digest(out/'build.json'),binary_sha256=digest(binary),review_sha256=sha(review_raw),command=command,returncode=process.returncode,run_log_sha256=sha(log),outputs=outputs))
        reports = []
        binding = dict(base=BASE,core_manifest_sha256=CORE)
        for r in requests:
            path = observation_dir/(r['id']+'.jsonl')
            need(path.stat().st_size <= cmp.MAX_BYTES, 'observation byte limit')
            raw = path.read_bytes()
            request = dict(case=r['id'],scope=r['scope'],fixture_root=r['source_root'],**dict(zip(['row_limit','byte_limit','retained_limit','scratch_limit','work_limit'],r['limits'])))
            reports.append(cmp.Case(authority,r['id'],raw,request,binding).compare())
        source_check(source); package_check()
        result = dict(status='PUBLIC_SOURCE_TYPING_REPLAY_PASS',profile=args.profile,cases=reports,public_input_sha256=PUBLIC,historical_authority_sha256=HISTORICAL,comparator_sha256=COMPARE,package_sha256=digest(HERE/'freeze-manifest.json'),source_closure_sha256=digest(HERE/'components/full-source-closure.json'),binary_sha256=digest(binary),request_manifest_sha256=digest(out/'request-manifest.json'),run_sha256=digest(out/'run.json'),scope='54 source resolution/type observations; separate control completion is not claimed',deferred=cmp.DEFERRED,not_claimed=['lowering','ownership','runtime','native','fuel','Q','pilot','activation','CI','historical package membership'])
        report(out/'result.json',result)
        return dict(status=result['status'],cases=len(reports),profile=args.profile)
    except Exception as error:
        if not (out/'result.json').exists():report(out/'result.json',dict(status='REJECTED',first_failure=str(error),qualification_credit=0))
        raise

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--repo',type=pathlib.Path,required=True)
    p.add_argument('--out',type=pathlib.Path,required=True)
    p.add_argument('--profile',choices=['debug','release'],default='debug')
    p.add_argument('--reuse-binary',type=pathlib.Path)
    p.add_argument('--review',type=pathlib.Path,required=True,help='Independent report admitting the exact wrapper package manifest')
    args=p.parse_args()
    print(json.dumps(replay(args),sort_keys=True))

if __name__ == '__main__':
    main()
