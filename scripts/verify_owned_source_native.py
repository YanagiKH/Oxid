#!/usr/bin/env python3
"""Reproduce the bounded, test-only owned-source native qualification.

From a checkout with Rust and trusted LLVM 19.1.7 installed:
  python3 scripts/verify_owned_source_native.py --llvm-bin /usr/lib/llvm-19/bin --jobs 2

Use --evidence PATH for a fresh evidence root; the default is a unique directory
under target/owned-source-native. --jobs 1 runs the identical corpus serially.
Builds, frozen source/expectations, exact copied test binaries, LLVM identities,
commands, complete logs, receipts, semantic matching, and independent store/guard
audit reports are retained there, including the first failure. No source frontend
activation or general source qualification is implied by this native-only gate.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

import verify_owned_source as harness


PROFILES = ('debug', 'release')
COLLECTOR = 'frontend::oir::owned::source::program::candidate_adapter::candidate_native::emit_candidate_native_receipts'
CONTROL = 'frontend::oir::owned::source::program::candidate_adapter::candidate_native::candidate_native_budget_controls_and_probe_transforms_are_bounded'
COLLECTOR_SOURCE = 'src/frontend/oir/owned/source/candidate_native.rs'


@dataclass(frozen=True)
class Command:
    name: str
    argv: tuple[str, ...]
    cwd: Path
    environment: dict[str, str]


class RunnerFailure(Exception):
    def __init__(self, message, code=1):
        super().__init__(message)
        self.code = code


digest = harness.digest
read_json = lambda path: harness.strict_json_loads(Path(path).read_text())


def write_json(path, value):
    with Path(path).open('x') as handle:
        handle.write(json.dumps(value, indent=2, sort_keys=True) + '\n')


def fresh_directory(path):
    path = Path(path)
    path.mkdir(parents=True, exist_ok=False)
    return path


def require_inventory(actual, expected, label):
    if not expected or len(set(expected)) != len(expected):
        raise ValueError(label + ': empty or duplicate required inventory')
    if actual != expected or len(set(actual)) != len(actual):
        raise ValueError(label + ': missing, duplicate, extra, or unordered entries')


def verify_hashes(root, identities):
    if not identities:
        raise ValueError('empty identity manifest')
    for name, expected in identities.items():
        path = Path(root) / name
        if path.is_symlink() or not path.is_file() or digest(path) != expected:
            raise ValueError('input identity drift: ' + str(path))


def run_commands(commands, directory, *, jobs=1, timeout=1800, check=None, output=None):
    """Run only bounded isolated process groups and persist checked exit records.

    Unlike a worker PASS string, an exit status is only execution evidence. The
    caller must verify each required artifact and complete case inventory. All
    running groups are killed on failure/timeout/cancellation, including children
    whose direct parent exited. Pending commands are never counted as passing.
    """
    if os.name != 'posix':
        raise ValueError('source-native checks require POSIX process groups')
    if jobs not in (1, 2) or not math.isfinite(timeout) or timeout <= 0:
        raise ValueError('jobs must be 1 or 2; timeout must be finite and positive')
    if not commands or len({c.name for c in commands}) != len(commands):
        raise ValueError('commands require a nonempty unique inventory')
    if any(not re.fullmatch(r'[a-zA-Z0-9_.-]+', c.name) for c in commands):
        raise ValueError('command names must be safe filenames')
    directory = fresh_directory(directory)
    output = sys.stdout if output is None else output
    rows = [dict(name=c.name, command=list(c.argv), cwd=str(c.cwd), environment=c.environment,
                 stdout=str(directory / f'{i:02d}-{c.name}.stdout'),
                 stderr=str(directory / f'{i:02d}-{c.name}.stderr'), status='NOT RUN')
            for i, c in enumerate(commands)]
    active, owned, next_index, failure, error = {}, [], 0, 0, None
    cancelled = [0]
    def on_signal(signum, _frame):
        if not cancelled[0]:
            cancelled[0] = signum
    old = {sig: signal.signal(sig, on_signal) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        while next_index < len(commands) or active:
            if cancelled[0]:
                failure = 128 + cancelled[0]
                break
            for index, (process, started) in list(active.items()):
                code = process.poll()
                if code is None:
                    if time.monotonic() - started >= timeout:
                        rows[index]['status'] = 'TIMEOUT'
                        failure = 124
                        break
                    continue
                del active[index]
                rows[index].update(exit_code=code, elapsed_seconds=time.monotonic() - started)
                if code:
                    rows[index]['status'] = 'FAIL'
                    failure = code if code > 0 else 128 - code
                elif harness.signal_group(process, 0):
                    rows[index]['status'] = 'FAIL'
                    failure, error = 1, 'command left running descendants: ' + commands[index].name
                else:
                    if check:
                        check()
                    rows[index]['status'] = 'PASS'
                    owned.remove(process)
                    print(commands[index].name + ': completed', file=output, flush=True)
                if failure:
                    break
            if failure:
                break
            while next_index < len(commands) and len(active) < jobs and not cancelled[0]:
                if check:
                    check()
                index, next_index = next_index, next_index + 1
                command, row = commands[index], rows[index]
                with Path(row['stdout']).open('xb') as stdout, Path(row['stderr']).open('xb') as stderr:
                    process = subprocess.Popen(command.argv, cwd=command.cwd,
                        env={**os.environ, **command.environment}, stdout=stdout, stderr=stderr,
                        start_new_session=True)
                active[index] = process, time.monotonic()
                owned.append(process)
                row['status'] = 'RUNNING'
            if active:
                time.sleep(.01)
    except Exception as exception:
        failure, error = 1, str(exception)
    finally:
        try:
            harness.stop_groups(owned)
            failure = failure or (128 + cancelled[0] if cancelled[0] else 0)
            for row in rows:
                if row['status'] == 'RUNNING':
                    row['status'] = 'CANCELLED'
                for stream in ('stdout', 'stderr'):
                    if Path(row[stream]).is_file():
                        row[stream + '_sha256'] = digest(row[stream])
            if any(row['status'] != 'PASS' for row in rows):
                failure = failure or 1
            write_json(directory / 'process-manifest.json', dict(exit_code=failure,
                       jobs=jobs, timeout_seconds=timeout, error=error, commands=rows))
        finally:
            for sig, handler in old.items():
                signal.signal(sig, handler)
    if failure:
        raise RunnerFailure(error or 'command failed; inspect ' + str(directory / 'process-manifest.json'), failure)
    return rows


def built_test_binary(path):
    messages = [harness.strict_json_loads(line) for line in Path(path).read_text().splitlines()]
    candidates = [Path(item['executable']) for item in messages
                  if item.get('reason') == 'compiler-artifact' and item.get('target', {}).get('name') == 'oxid'
                  and item.get('profile', {}).get('test') is True and item.get('executable')]
    if len(candidates) != 1 or not candidates[0].is_file():
        raise ValueError('build must identify exactly one actual oxid test binary')
    return candidates[0].resolve()


def source_inventory(repo):
    paths = [repo / name for name in ('Cargo.toml', 'Cargo.lock', 'build.rs')]
    def scan_root(name):
        directory = repo
        for part in Path(name).parts:
            directory /= part
            if directory.is_symlink():
                raise ValueError('source snapshots reject symlink directories: ' + str(directory))
        return directory
    # The Rust binary also embeds compiler/stdlib text and its owned RFC through
    # include_str!, so those inputs belong to the build identity as well.
    for name in ('src', 'native', 'compiler', 'stdlib', 'rfcs', '.cargo',
                 'fixtures/owned_source', 'tests/fixtures/owned_source'):
        for path in scan_root(name).rglob('*'):
            if path.is_symlink():
                raise ValueError('source snapshots reject symlinks: ' + str(path))
            if path.is_file():
                paths.append(path)
    paths.extend(path for path in scan_root('scripts').glob('*owned*.py'))
    paths = sorted(set(paths))
    if any(path.is_symlink() for path in paths):
        raise ValueError('source snapshots require regular input files')
    return {str(path.relative_to(repo)): digest(path) for path in paths}


def freeze_sources(repo, evidence, matcher):
    fixture = repo / 'tests/fixtures/owned_source/native-probe-sources.json'
    specification = read_json(fixture)
    expected = harness.native_probe_expectations(fixture)
    ids = [item['id'] for _, item in expected]
    require_inventory(ids, list(matcher.NATIVE_IDS), 'independent native cases')
    require_inventory([item['id'] for item in specification['cases']], ids, 'specification cases')
    frozen = fresh_directory(evidence / 'frozen')
    files = []
    for (source, item), specified in zip(expected, specification['cases']):
        identifier, fuel = item['id'], item['schedule']['total_fuel']
        if type(fuel) is not int or fuel < 2 or fuel >= 2048:
            raise ValueError('independent case exceeds bounded collector domain')
        require_inventory([row['budget'] for row in item['native_budget_expectations']], list(range(fuel + 1)), 'model budgets')
        (frozen / (identifier + '.ox')).write_bytes(source.encode())
        write_json(frozen / (identifier + '.json'), item)
        entry = dict(id=identifier, source_sha256=digest(frozen / (identifier + '.ox')),
                     expectation_sha256=digest(frozen / (identifier + '.json')), fuel=fuel, budget_count=fuel + 1)
        if any(entry[key] != specified[key] for key in entry):
            raise ValueError('new freeze differs from independently frozen specification: ' + identifier)
        files.append(entry)
    if len(files) != 8 or sum(row['budget_count'] for row in files) != 1932:
        raise ValueError('native corpus must include all eight cases and 1,932 budgets per profile')
    manifest = dict(schema='owned-source-native-freeze-v1', specification_sha256=digest(fixture),
                    model_sha256=digest(Path(harness.model.__file__)), files=files)
    write_json(frozen / 'manifest.json', manifest)
    return fixture, manifest


def tool_paths(repo, evidence, environment, llvm_bin, timeout):
    paths = {}
    for name in ('cargo', 'rustc'):
        selected = shutil.which(name, path=environment.get('PATH', os.environ.get('PATH')))
        if not selected:
            raise ValueError('required tool is missing: ' + name)
        path = Path(selected).resolve()
        if path.name == 'rustup':
            result = run_commands([Command('resolve-' + name, (str(path), 'which', name), repo, environment)],
                                  evidence / ('resolve-' + name), timeout=timeout)
            path = Path(Path(result[0]['stdout']).read_text().strip()).resolve()
        if not path.is_file():
            raise ValueError('resolved tool does not exist: ' + str(path))
        paths[name] = path
    paths['python'] = Path(sys.executable).resolve()
    for name in ('clang', 'opt', 'ld.lld', 'llvm-as'):
        path = llvm_bin / name
        if not path.is_file():
            raise ValueError('trusted LLVM installation lacks ' + str(path))
        # lld is a multicall binary: retain ld.lld as argv[0] while hashing the
        # symlink target bytes. Resolving its name changes the invoked program.
        paths[name] = path.absolute()
    commands = [Command(name.replace('.', '-'), (str(path), '--version'), repo, environment) for name, path in paths.items()]
    results = run_commands(commands, evidence / 'tool-versions', jobs=1, timeout=timeout)
    identities = {}
    for (name, path), result in zip(paths.items(), results):
        version = Path(result['stdout']).read_text() + Path(result['stderr']).read_text()
        if name not in ('rustc', 'cargo', 'python') and not re.search(r'(?<![\d.])19\.1\.7(?![\d.])', version):
            raise ValueError(name + ': trusted LLVM 19.1.7 is required')
        identities[name] = dict(path=str(path), sha256=digest(path), version=version,
                                version_command=result['command'])
    write_json(evidence / 'toolchains.json', identities)
    return identities


def require_one_test(result):
    text = Path(result['stdout']).read_text()
    if not re.search(r'^test result: ok\. 1 passed; 0 failed; 0 ignored;', text, re.M):
        raise ValueError('expected exactly one executed Rust test: ' + result['name'])


def collect_catalog(directory, frozen, item, compiler, command, manifest_hash):
    import owned_source_native_artifacts as artifacts
    identifier, fuel = item['id'], item['fuel']
    if not (directory / 'COMPLETE').is_file():
        raise ValueError('collector did not complete: ' + str(directory))
    source = frozen / (identifier + '.ox')
    if digest(directory / source.name) != item['source_sha256'] or digest(source) != item['source_sha256']:
        raise ValueError('collector consumed different source bytes')
    rows = [harness.strict_json_loads(line) for line in (directory / 'receipts.jsonl').read_text().splitlines()]
    names = ['default'] + ['budget-' + str(n) for n in range(fuel + 1)]
    names += ['fixed-' + str(n) for n in (0, fuel - 1, fuel)]
    names += ['probe-' + str(n) for n in range(fuel + 1)]
    require_inventory([row['name'] for row in rows], names, 'native receipt names')
    expected_budgets = [1000000] + list(range(fuel + 1)) + [0, fuel - 1, fuel] + list(range(fuel + 1))
    if any(type(row['budget']) is not int or row['budget'] != budget for row, budget in zip(rows, expected_budgets)):
        raise ValueError('native receipt budget identity differs')
    llvm_names = ['argv.ll', 'bounded.ll', 'default.ll', 'probe-original-stripped.ll',
                  'probe-original.ll', 'probe-stripped.ll', 'probe.ll']
    elf_names = ['argv.elf', 'default.elf', 'probe.elf']
    llvm_names += ['fixed-' + str(n) + '.ll' for n in (0, fuel - 1, fuel)]
    elf_names += ['fixed-' + str(n) + '.elf' for n in (0, fuel - 1, fuel)]
    require_inventory(sorted(path.name for path in directory.glob('*.ll')), sorted(llvm_names), 'LLVM modules')
    require_inventory(sorted(path.name for path in directory.glob('*.elf')), sorted(elf_names), 'compiled ELF files')
    files = {}
    for path in sorted(directory.iterdir()):
        if path.is_symlink():
            raise ValueError('symlink collector artifact: ' + str(path))
        if path.is_file():
            files[path.name] = digest(path)
    admission = read_json(directory / 'admission.json')
    catalog = dict(schema='unit4b-native-catalog-v1', source_path=str(source), source_sha256=digest(source),
                   guarded=admission['guarded'], entry=admission['entry'], admission=admission,
                   compiler=compiler, stores=read_json(directory / 'stores.json'), files=files,
                   freeze_manifest_sha256=manifest_hash, collection_command=command)
    for key, path in [('raw_witness', directory / 'raw-view.json'), ('llvm', directory / 'bounded.ll'),
                      ('argv_llvm', directory / 'argv.ll'), ('probe_llvm', directory / 'probe-original.ll'),
                      ('argv_probe_llvm', directory / 'probe.ll'), ('model', frozen / (identifier + '.json'))]:
        catalog[key + '_path'], catalog[key + '_sha256'] = str(path), digest(path)
    write_json(directory / 'catalog.json', catalog)
    artifacts.enrich(directory)
    return dict(id=identifier, profile=compiler['profile'], catalog_path=str(directory / 'catalog.json'),
                catalog_sha256=digest(directory / 'catalog.json'),
                payload_enrichment_sha256=digest(directory / 'payload-enrichment.json'),
                execution_counts=dict(budget_executions=fuel + 1, probe_executions=fuel + 1,
                                      fixed_executions=3, default_executions=1, total_executions=len(rows)))


def run_qualification(args, evidence):
    import owned_source_native as matcher
    import owned_source_native_artifacts as artifacts
    repo = Path(__file__).resolve().parents[1]
    fixture, freeze = freeze_sources(repo, evidence, matcher)
    frozen = evidence / 'frozen'
    frozen_hashes = {path.name: digest(path) for path in frozen.iterdir()}
    snapshot = fresh_directory(evidence / 'snapshot')
    sources = source_inventory(repo)
    for name in sources:
        destination = snapshot / 'inputs' / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(repo / name, destination)
    source_manifest = dict(schema='owned-source-native-inputs-v1', source_root=str(repo), files=sources,
                           collector=sources[COLLECTOR_SOURCE], frozen_manifest_sha256=digest(frozen / 'manifest.json'))
    write_json(snapshot / 'source-manifest.json', source_manifest)
    snapshot_identities = {'source-manifest.json': digest(snapshot / 'source-manifest.json')}
    tools, compilers = {}, []
    def check():
        if source_inventory(repo) != sources:
            raise ValueError('source inventory changed after snapshot')
        verify_hashes(snapshot / 'inputs', sources)
        verify_hashes(snapshot, snapshot_identities)
        verify_hashes(frozen, frozen_hashes)
        if set(path.name for path in frozen.iterdir()) != set(frozen_hashes):
            raise ValueError('frozen source inventory changed')
        for tool in tools.values():
            if digest(tool['path']) != tool['sha256']:
                raise ValueError('tool identity drift: ' + tool['path'])
        for compiler in compilers:
            if digest(compiler['binary']) != compiler['sha256']:
                raise ValueError('copied collector identity drift: ' + compiler['profile'])
    environment = {'OXID_LLVM_BIN': str(args.llvm_bin), 'CARGO_BUILD_JOBS': '1'}
    for key in ('CARGO_HOME', 'RUSTUP_HOME', 'LD_LIBRARY_PATH', 'PATH', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CC', 'CXX', 'AR'):
        if key in os.environ:
            environment[key] = os.environ[key]
    check()
    tools.update(tool_paths(repo, snapshot, environment, args.llvm_bin, args.timeout))
    snapshot_identities['toolchains.json'] = digest(snapshot / 'toolchains.json')
    python = tools['python']['path']
    environment['RUSTC'] = tools['rustc']['path']
    environment['CARGO_TARGET_DIR'] = str(evidence / 'build')
    for profile in PROFILES:
        command = [tools['cargo']['path'], 'test', '--locked', '--bin', 'oxid', '--no-run', '--message-format=json', '--jobs', '1']
        if args.offline:
            command.append('--offline')
        if profile == 'release':
            command.append('--release')
        result = run_commands([Command('build-' + profile, tuple(command), repo, environment)],
                              snapshot / ('build-' + profile), timeout=args.build_timeout, check=check)[0]
        built = built_test_binary(result['stdout'])
        binary = snapshot / ('oxid-test-' + profile)
        shutil.copy2(built, binary)
        binary.chmod(0o555)
        compiler = dict(profile=profile, command=command, binary=str(binary), sha256=digest(binary),
                        built_binary=str(built), source_manifest_sha256=digest(snapshot / 'source-manifest.json'),
                        build_record=result)
        compilers.append(compiler)
        listed = run_commands([Command('list-' + profile, (str(binary), '--list', '--format', 'terse'), repo, environment)],
                              snapshot / ('list-' + profile), timeout=args.timeout, check=check)[0]
        tests = [line.removesuffix(': test') for line in Path(listed['stdout']).read_text().splitlines() if line.endswith(': test')]
        if tests.count(COLLECTOR) != 1 or tests.count(CONTROL) != 1:
            raise ValueError('exact collector/control test identity is missing or ambiguous')
        focused = run_commands([Command('control-' + profile, (str(binary), CONTROL, '--exact', '--nocapture', '--test-threads=1'), repo, environment)],
                               snapshot / ('control-' + profile), timeout=args.timeout, check=check)[0]
        require_one_test(focused)
        compiler['control_record'] = focused
    write_json(snapshot / 'compiler-manifest.json', dict(schema='owned-source-native-compilers-v1', builds=compilers,
               source_manifest_path=str(snapshot / 'source-manifest.json'), source_manifest_sha256=digest(snapshot / 'source-manifest.json'),
               toolchain_manifest_path=str(snapshot / 'toolchains.json'), toolchain_manifest_sha256=digest(snapshot / 'toolchains.json')))
    snapshot_identities['compiler-manifest.json'] = digest(snapshot / 'compiler-manifest.json')
    root = fresh_directory(evidence / 'collection')
    commands, pairs = [], []
    for compiler in compilers:
        profile = compiler['profile']
        fresh_directory(root / profile)
        for item in freeze['files']:
            identifier, fuel = item['id'], item['fuel']
            directory = root / profile / identifier
            native_env = {**environment, 'OXID_UNIT4B_NATIVE_SOURCE': str(frozen / (identifier + '.ox')),
                          'OXID_UNIT4B_NATIVE_OUTPUT': str(directory), 'OXID_UNIT4B_NATIVE_BUDGETS': f'0..={fuel}',
                          'OXID_UNIT4B_NATIVE_FIXED_BUDGETS': f'0,{fuel-1},{fuel}', 'OXID_UNIT4B_NATIVE_PROBE_BUDGETS': f'0..={fuel}'}
            commands.append(Command(profile + '-' + identifier, (compiler['binary'], COLLECTOR, '--ignored', '--exact', '--nocapture', '--test-threads=1'), repo, native_env))
            pairs.append((directory, item, compiler))
    records = run_commands(commands, evidence / 'collector-logs', jobs=args.jobs, timeout=args.timeout, check=check)
    cases = []
    for (directory, item, compiler), command in zip(pairs, records):
        require_one_test(command)
        cases.append(collect_catalog(directory, frozen, item, compiler, command, digest(frozen / 'manifest.json')))
    collection = dict(schema='owned-source-native-collection-v1', specification_sha256=digest(fixture),
                      frozen_manifest_path=str(frozen / 'manifest.json'), frozen_manifest_sha256=digest(frozen / 'manifest.json'),
                      compiler_manifest_path=str(snapshot / 'compiler-manifest.json'), compiler_manifest_sha256=digest(snapshot / 'compiler-manifest.json'),
                      source_manifest_path=str(snapshot / 'source-manifest.json'), source_manifest_sha256=digest(snapshot / 'source-manifest.json'),
                      collector_source_sha256=sources[COLLECTOR_SOURCE], command_manifest_path=str(evidence / 'collector-logs/process-manifest.json'),
                      command_manifest_sha256=digest(evidence / 'collector-logs/process-manifest.json'), cases=cases)
    write_json(root / 'collection-manifest.json', collection)
    check()
    reports = fresh_directory(evidence / 'reports')
    commands = [Command('semantic-' + profile, (python, str(repo / 'scripts/owned_source_native.py'), str(root),
                    str(reports / ('semantic-' + profile + '.json')), '--profile', profile), repo, {}) for profile in PROFILES]
    run_commands(commands, evidence / 'semantic-logs', jobs=args.jobs, timeout=args.timeout, check=check)
    audit_commands = []
    for directory, item, compiler in pairs:
        name = compiler['profile'] + '-' + item['id']
        audit_commands.append(Command(name, (python, str(repo / 'scripts/owned_source_native_audit.py'),
                      str(directory / 'catalog.json'), '--llvm-bin', str(args.llvm_bin), '--output', str(reports / (name + '-audit.json'))), repo, environment))
    run_commands(audit_commands, evidence / 'audit-logs', jobs=args.jobs, timeout=args.timeout, check=check)
    controls = evidence / 'audit-controls'
    run_commands([Command('audit-controls', (python, str(repo / 'scripts/test_owned_source_native_audit.py'),
                     str(root / 'debug/batch/catalog.json'), str(controls), '--llvm-bin', str(args.llvm_bin)), repo, environment)],
                 evidence / 'audit-control-logs', timeout=args.timeout, check=check)
    control_report = read_json(controls / 'summary.json')
    require_inventory([row['case'] for row in control_report['controls']],
                      ['hoisted-borrow-store', 'delayed-borrow-store', 'removed-borrow-admission-branch',
                       'hoisted-overflow-result-store', 'renamed-all-ssa-values'], 'audit controls')
    if control_report['status'] != 'pass' or any(row['llvm_as_status'] != 0 for row in control_report['controls']):
        raise ValueError('audit controls did not verify')
    if [row['audit_status'] for row in control_report['controls']] != ['fail', 'fail', 'fail', 'fail', 'pass']:
        raise ValueError('audit controls have incorrect rejection results')
    artifact_reports = []
    for directory, item, compiler in pairs:
        report = artifacts.check(directory / 'catalog.json', frozen)
        expected_counts = {'ordinary_budget_executions': item['fuel'] + 1, 'probe_budget_executions': item['fuel'] + 1,
                           'production_default_executions': 1, 'production_fixed_executions': 3, 'source_free_receipts': 2 * (item['fuel'] + 1) + 4}
        if report['status'] != 'pass' or any(report['counts'].get(key) != value for key, value in expected_counts.items()):
            raise ValueError('incomplete independent artifact audit')
        name = compiler['profile'] + '-' + item['id']
        write_json(reports / (name + '-artifacts.json'), report)
        audit = read_json(reports / (name + '-audit.json'))
        if audit['status'] != 'pass' or audit['catalog_sha256'] != digest(directory / 'catalog.json') or not audit['model'] or not audit['probe']:
            raise ValueError('missing source-bound store/guard proof')
        artifact_reports.append(report)
    for profile in PROFILES:
        report = read_json(reports / ('semantic-' + profile + '.json'))
        require_inventory([row['id'] for row in report['cases']], list(matcher.NATIVE_IDS), 'semantic reports')
        if report['collection_manifest_sha256'] != digest(root / 'collection-manifest.json') or report['profile'] != profile:
            raise ValueError('semantic report collection/profile identity differs')
        if any(row['outcomes_and_committed_stores'] != 'MATCH' or row['full_qualification'] is not False for row in report['cases']):
            raise ValueError('semantic report contains an unqualified or mismatched case')
        if sum(row['actual_process_observations'] for row in report['cases']) != 3896:
            raise ValueError('incomplete semantic observation count')
    # Both compiler profiles must emit the same ordinary and instrumented bodies.
    for item in freeze['files']:
        debug = read_json(root / 'debug' / item['id'] / 'catalog.json')
        release = read_json(root / 'release' / item['id'] / 'catalog.json')
        names = sorted(name for name in debug['files'] if name.endswith(('.ll', '.elf')))
        if not names or names != sorted(name for name in release['files'] if name.endswith(('.ll', '.elf'))):
            raise ValueError('profile artifact inventory differs')
        if any(debug['files'][name] != release['files'][name] for name in names):
            raise ValueError('profile native artifact bytes differ')
    check()
    total = sum(report['counts']['source_free_receipts'] for report in artifact_reports)
    if len(cases) != 16 or total != 7792:
        raise ValueError('partial native collection cannot pass')
    summary = dict(schema='owned-source-native-result-v1', status='PASS', scope='bounded test-only owned-source native checks',
                   full_source_qualification=False, source_activation_changed=False, profiles=list(PROFILES), source_cases=8,
                   budgets_per_profile=1932, native_observations=total, jobs=args.jobs,
                   compiled_elf_files_per_profile=48, llvm_modules_per_profile=80,
                   collection_manifest_sha256=digest(root / 'collection-manifest.json'),
                   audit_controls_sha256=digest(controls / 'summary.json'),
                   reports={path.name: digest(path) for path in sorted(reports.iterdir())})
    write_json(evidence / 'result.json', summary)
    print(f'Owned-source native checks: 8 cases; 1,932 budgets/profile; {total} observations: PASS', flush=True)
    print('Evidence: ' + str(evidence), flush=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--evidence', type=Path, help='fresh output root; defaults beneath target/owned-source-native')
    parser.add_argument('--llvm-bin', type=Path, default=os.environ.get('OXID_LLVM_BIN'))
    parser.add_argument('--jobs', type=int, choices=(1, 2), default=1)
    parser.add_argument('--timeout', type=float, default=1800, help='timeout for each collector or audit command')
    parser.add_argument('--build-timeout', type=float, default=1800)
    parser.add_argument('--offline', action='store_true', help='require cached Cargo dependencies')
    args = parser.parse_args(argv)
    if sys.flags.optimize:
        parser.error('Python assertions must remain enabled for the preserved audit')
    if sys.platform != 'linux' or os.uname().machine != 'x86_64':
        parser.error('native qualification requires Linux x86_64')
    if any(not math.isfinite(value) or value <= 0 for value in (args.timeout, args.build_timeout)):
        parser.error('timeouts must be finite and positive')
    if args.llvm_bin is None:
        clang = shutil.which('clang-19') or shutil.which('clang')
        if not clang:
            parser.error('set --llvm-bin or OXID_LLVM_BIN to trusted LLVM 19.1.7')
        args.llvm_bin = Path(clang).resolve().parent
    args.llvm_bin = args.llvm_bin.resolve()
    evidence = None
    try:
        if args.evidence:
            evidence = fresh_directory(args.evidence.resolve())
        else:
            parent = Path(__file__).resolve().parents[1] / 'target/owned-source-native'
            parent.mkdir(parents=True, exist_ok=True)
            evidence = Path(tempfile.mkdtemp(prefix='run-', dir=parent))
        run_qualification(args, evidence)
        return 0
    except Exception as error:
        if evidence is not None:
            write_json(evidence / 'failure.json', dict(status='FAIL', error_type=type(error).__name__, error=str(error),
                       source_activation_changed=False, full_source_qualification=False))
        print('Owned-source native checks FAILED: ' + str(error), file=sys.stderr)
        return error.code if isinstance(error, RunnerFailure) else 1


if __name__ == '__main__':
    sys.exit(main())
