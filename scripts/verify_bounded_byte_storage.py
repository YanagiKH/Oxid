#!/usr/bin/env python3
"""RFC0031 exact-source gate; immutable oracle, closed rosters, ordinary profiles.

The qualification mode reuses coordinated executable build receipts and invokes
the genuine dpkg LLVM stager. --prepare-public-build performs only the separate
ordinary integration no-run builds. Privacy probes compile private source
variations using the explicitly coordinated Cargo target directory.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import re
import shutil
import sys


def reject_bytecode(repo):
    for directory in (repo / 'scripts', repo / 'tests'):
        for path in directory.rglob('*'):
            if path.name == '__pycache__' or path.suffix in ('.pyc', '.pyo'):
                raise ValueError('source-only gate rejects bytecode contamination: ' + str(path))


# -B prevents writes, but does not prevent loading an existing cache. Check
# before importing repository controllers, including on failed hosted runs.
if __name__ == '__main__':
    reject_bytecode(Path(__file__).resolve().parents[1])

import qualify_hir_byte_storage_compatibility as provider
import run_bounded_u8_corpus as common
import verify_bounded_enum_native as enum_gate
import verify_bounded_stdin_native as builds
import verify_bounded_u8_native as u8_gate
import verify_byte_storage_source_privacy as privacy
from verify_fixture_data import _regular_path, _unique_object

require, identity, save, sha = common.require, common.identity, common.save, common.sha
PACKAGE = Path('tests/qualification/byte_storage_current')
DATA_SHA256 = '85c8908a2bf94e69e352ddb2399e1a3c0eb80319ab0a82f0ccdb3ed8c617e979'
ORACLE_SHA256 = 'de6d63aba53edfe1af802536d044ffbad345509556e804b293381ea5c39ed3a7'
ORIGINAL_SEAL_SHA256 = '91f99de6d2632f3d6972017c7af55596490ddd00e7b16c9e6bd1abecae5b01c3'
SOURCE_SHA256 = '9e9e65c9ba3b034074ca22cb0967d6820ff5b5f8907613fcb36720b97519b0f8'
STAGER = u8_gate.STAGER
STAGER_SHA256 = '055dd46f2c24f3a0496486ec89570ef533d7a016a5bc85d834447e8a59204def'
PUBLIC_SOURCE = 'tests/typed_byte_storage_native.rs'
TEST_PATTERN = re.compile(r'((?:#\[[^\n]+\]\s*)+)fn\s+(\w*byte_storage\w*)\s*\(')



def invoke(directory, label, argv, *, cwd, env, **options):
    try:
        return common.invoke(directory, label, argv, cwd=cwd, env=env, **options)
    finally:
        path = directory / (label + '.json')
        if path.is_file():
            receipt = read_json(path)
            receipt['selected_environment'] = {key: env.get(key) for key in (
                'PATH', 'LD_LIBRARY_PATH', 'CARGO_TARGET_DIR', 'OXID_LLVM_BIN',
                'OXID_OWNED_NATIVE_EVIDENCE', 'OXID_CACHE_DIR', 'PYTHONDONTWRITEBYTECODE')}
            save(path, receipt)


def read_json(path):
    return json.loads(path.read_bytes(), object_pairs_hook=_unique_object)



def admit_tree_membership(root, files):
    """Close directories as well as files, including empty cache directories."""
    directories = {parent.as_posix() for name in files for parent in Path(name).parents if parent != Path('.')}
    require(not root.is_symlink(), 'closed tree root is a symlink')
    found_files, found_directories = set(), set()
    for path in root.rglob('*'):
        require(not path.is_symlink(), 'symlink in closed tree: ' + str(path))
        relative = path.relative_to(root).as_posix()
        if path.is_file(): found_files.add(relative)
        elif path.is_dir(): found_directories.add(relative)
        else: raise ValueError('nonregular object in closed tree: ' + str(path))
    require(found_files == set(files) and found_directories == directories, 'closed tree directory/file membership differs')


def admit_package(repo):
    root = repo / PACKAGE
    raw = _regular_path(root, 'source-data-manifest.json').read_bytes()
    require(sha(raw) == DATA_SHA256, 'byte-storage fixture-data authority changed')
    manifest = json.loads(raw, object_pairs_hook=_unique_object)
    files = manifest['files']
    expected = {name for name in files if name.startswith('oracle-v3/')}
    actual = {p.relative_to(root).as_posix() for p in (root / 'oracle-v3').rglob('*') if p.is_file() or p.is_symlink()}
    require(actual == expected, 'oracle closed membership differs')
    admit_tree_membership(root / 'oracle-v3', {name.removeprefix('oracle-v3/') for name in expected})
    require(set(p.name for p in root.iterdir()) == {'predecessor', 'oracle-v3', 'registry.json', 'source-data-manifest.json', 'README.md'},
            'byte-storage package closed membership differs')
    for name, record in files.items():
        path = _regular_path(root, name)
        require(set(record) == {'bytes', 'sha256'} and type(record['bytes']) is int,
                'malformed registered identity')
        body = path.read_bytes()
        require((len(body), sha(body)) == (record['bytes'], record['sha256']), 'registered body changed: ' + name)
    historical = {**u8_gate.BYTE_STORAGE_ARCHIVES,
                  'identity.json': '77b3d06ef7c4def465fa95a07ca702823b6282b636ee7fbe58e00814039f8dea'}
    archived = root / 'predecessor'
    require({p.relative_to(archived).as_posix() for p in archived.rglob('*') if p.is_file() or p.is_symlink()} == set(historical),
            'historical archive closed membership differs')
    admit_tree_membership(archived, historical)
    for name, digest in historical.items():
        require(sha(_regular_path(archived, name).read_bytes()) == digest, 'historical archive changed: ' + name)
    original = root / 'oracle-v3/original-seal.json'
    require(sha(original.read_bytes()) == ORIGINAL_SEAL_SHA256, 'original oracle seal changed')
    seal = read_json(original)
    corpus = root / 'oracle-v3/corpus'
    require(sha((corpus / 'manifest.json').read_bytes()) == ORACLE_SHA256, 'original corpus manifest changed')
    retained = {name.removeprefix('oracle-v3/'): record['sha256'] for name, record in files.items() if name.startswith('oracle-v3/corpus/')}
    require(retained == {name: digest for name, digest in seal.items() if name.startswith('corpus/')},
            'extracted corpus is not the entire original sealed corpus')
    oracle = read_json(corpus / 'manifest.json')
    cases = oracle['cases']
    require(len(cases) == 323 and len({c['id'] for c in cases}) == 323, 'reference case roster differs')
    names = {'manifest.json'}
    for case in cases:
        require(case['entry'] == 'main.ox' and case['entry'] in case['files'], 'unexpected corpus entry')
        for name, digest in case['files'].items():
            relative = case['id'] + '/' + name
            path = _regular_path(corpus, relative)
            require(relative not in names and sha(path.read_bytes()) == digest, 'duplicate or changed oracle source')
            names.add(relative)
        exp = case['expected']
        require(exp['kind'] in ('value', 'diagnostic'), 'unknown expected object')
        if exp['kind'] == 'diagnostic':
            require(exp.get('exact_diagnostic_sealed') is True and all(k in exp for k in ('code', 'stage', 'message', 'primary', 'secondary')),
                    'partial diagnostic golden is forbidden')
    require(names == {p.relative_to(corpus).as_posix() for p in corpus.rglob('*') if p.is_file()}, 'unregistered corpus file')
    transport = [c for c in cases if c['group'] == 'all256']
    require([c['id'] for c in transport] == [f'transport-{v:03}' for v in range(256)], 'all256 exact case roster differs')
    for value, case in enumerate(transport):
        require(case['expected'] == {'kind': 'value', 'type': 'i32', 'value': value,
                'native': {'exit': 0, 'stdout': str(value) + '\n', 'stderr': ''}}, 'independent byte integer oracle differs')
    return manifest, read_json(root / 'registry.json'), cases


def fixture_data_sources(repo):
    manifest, _, _ = admit_package(repo)
    return {repo / PACKAGE / name for name in manifest['files'] if name.endswith('.ox')}


def source_tests(repo):
    rows = []
    for path in sorted((repo / 'src').rglob('*.rs')):
        for attrs, name in TEST_PATTERN.findall(path.read_text()):
            if '#[test]' in attrs:
                rows.append((path.relative_to(repo).as_posix(), name, '#[ignore' in attrs))
    return sorted(rows)


def admit_registry(repo, registry):
    rows = registry['unit_tests']
    require(len(rows) == len({r['name'] for r in rows}) == 77 and sum(r['ignored'] for r in rows) == 4,
            'unit roster must contain 73 normal and four ignored tests')
    require(sorted((r['source'], r['function'], r['ignored']) for r in rows) == source_tests(repo),
            'source-derived byte-storage test membership differs')
    require(all(r['name'].endswith('::' + r['function']) for r in rows), 'full test name/function mismatch')
    public = [name for attrs, name in TEST_PATTERN.findall((repo / PUBLIC_SOURCE).read_text()) if '#[test]' in attrs]
    require(registry['public_source'] == PUBLIC_SOURCE and sorted(public) == registry['public_tests'] and len(public) == len(set(public)) == 10,
            'public test source membership differs')
    require(registry['native_transport'] == [f'transport-{v:03}' for v in range(256)] and registry['reference_cases'] == 323,
            'reference/native corpus boundaries differ')
    ignored = {r['name'] for r in rows if r['ignored']}
    native = registry['native_tests']
    require(len(native) == 4 and {r['test'] for r in native} == ignored,
            'native roster must execute every ignored test exactly once')
    names = [name for row in native for name in row['artifacts']]
    require(len(names) == len(set(names)) == 93, 'native artifact roster differs')
    require(registry['privacy_probes'] == [p[0] for p in privacy.PROBES] and len(privacy.PROBES) == 11,
            'source privacy roster differs')


def admit_public_build(evidence, destination, repo, target, profile, source_receipt):
    """Admit a separately coordinated integration no-run build, never rebuild."""
    previous = evidence / profile
    require(read_json(evidence / 'source-identity.json') == source_receipt,
            'public build source receipt differs from exact ordinary builds')
    data = (previous / 'public-build.stdout').read_bytes()
    rows = [json.loads(line, object_pairs_hook=_unique_object) for line in data.splitlines() if line]
    require([r.get('success') for r in rows if r.get('reason') == 'build-finished'] == [True], 'public Cargo build did not finish')
    selected = [r for r in rows if r.get('reason') == 'compiler-artifact' and r.get('target', {}).get('name') == 'typed_byte_storage_native'
                and r['target'].get('kind') == ['test'] and r.get('executable')]
    require(len(selected) == 1, 'public Cargo artifact missing or ambiguous')
    row = selected[0]
    flags = row['profile']
    require(flags.get('test') is True and flags.get('opt_level') == ('0' if profile == 'debug' else '3')
            and flags.get('debug_assertions') is (profile == 'debug') and flags.get('overflow_checks') is (profile == 'debug')
            and flags.get('debuginfo') == (2 if profile == 'debug' else 0), 'public build is not an ordinary profile')
    binary = builds.controls.no_symlinks(Path(row['executable']))
    require(Path(row['target']['src_path']) == repo / PUBLIC_SOURCE and binary.parent == target / profile / 'deps'
            and re.fullmatch(r'typed_byte_storage_native-[a-f0-9]{16}', binary.name), 'unexpected public artifact location')
    require(binary.read_bytes().startswith(b'\x7fELF'), 'public test binary is not ELF')
    receipt = read_json(previous / 'public-build.json')
    expected = ['test', '--locked', '--test', 'typed_byte_storage_native', '--no-run', '--message-format=json']
    if profile == 'release': expected.append('--release')
    require(receipt.get('argv', [])[1:] == expected and receipt.get('status') == 0 and receipt.get('cwd') == str(repo),
            'public Cargo command was nonstandard or from another checkout')
    require(receipt.get('stdout_sha256') == sha(data) and receipt.get('stderr_sha256') == sha((previous / 'public-build.stderr').read_bytes()),
            'public build streams differ from original receipt')
    binding = read_json(previous / 'public-binary.json')
    require(binding == {'public': identity(binary), 'cli': identity(target / profile / 'oxid'),
                        'source': identity(repo / PUBLIC_SOURCE), 'profile': profile,
                        'head': source_receipt['head'], 'tree': source_receipt['tree']},
            'public prebuilt binary, embedded CLI or test source changed')
    cli, _ = builds.admit_build_artifact(data, repo, target, profile, False)
    require(cli == target / profile / 'oxid', 'public build selected another CLI')
    for name in ('public-build.stdout', 'public-build.stderr', 'public-build.json', 'public-binary.json'):
        shutil.copy2(previous / name, destination / name)
    return binary


def admit_observation(stdout, stderr, expected, operation, failed):
    require(not stderr, 'corpus command wrote stderr')
    rows = [json.loads(line, object_pairs_hook=_unique_object) for line in stdout.splitlines()]
    diagnostics = [r for r in rows if r.get('kind') == 'diagnostic']
    if failed:
        require(len(diagnostics) == 1, 'expected exactly one diagnostic')
        d = diagnostics[0]
        require(all(d.get(k) == expected[k] for k in ('code', 'stage', 'message', 'secondary')) and d.get('notes') == [],
                'exact diagnostic contract differs')
        require(all(d.get('primary', {}).get(k) == value for k, value in expected['primary'].items()), 'exact diagnostic origin differs')
    else:
        require(not diagnostics and len(rows) == 1, 'unexpected diagnostic, extra record or empty result')
        if operation == 'run':
            require(rows[-1].get('result') == {'type': 'i32', 'value': expected['value']}, 'reference value differs')
        else:
            require(rows[-1].get('kind') == operation + '-summary' and rows[-1].get('success') is True,
                    'missing successful command summary')


def run_oracle(repo, root, compiler, cases, native, env):
    root.mkdir()
    selected = [c for c in cases if c['group'] == 'all256'] if native else cases
    require(len(selected) == (256 if native else 323), 'corpus selection differs')
    records = []
    for case in selected:
        work = root / case['id']
        shutil.copytree(repo / PACKAGE / 'oracle-v3/corpus' / case['id'], work)
        exp = case['expected']
        runtime = exp.get('code', '').startswith('E06')
        commands = ['compile'] if native else ['check', 'run'] + (['compile'] if exp['kind'] == 'diagnostic' and not runtime else [])
        for operation in commands:
            failed = exp['kind'] == 'diagnostic' and (not runtime or operation == 'run')
            argv = [compiler, operation, 'main.ox', '--edition=typed-preview', '--message-format=json']
            if operation == 'compile': argv += ['--backend=llvm', '--output=program']
            stdout, stderr = invoke(work, operation, argv, cwd=work, env=env, expected_status=1 if failed else 0)
            admit_observation(stdout, stderr, exp, operation, failed)
            if failed and operation == 'compile': require(not (work / 'program').exists(), 'failed corpus compilation published output')
        if native:
            elf = work / 'program'
            require(elf.read_bytes().startswith(b'\x7fELF'), 'transport executable is not ELF')
            runtime_dir = work / 'source-free'; runtime_dir.mkdir()
            target = runtime_dir / 'program'; shutil.copy2(elf, target)
            expected = exp['native']
            stdout, stderr = invoke(work, 'native', [target], cwd=runtime_dir, env=common.ELF_ENV, expected_status=expected['exit'])
            require((stdout, stderr) == (expected['stdout'].encode(), expected['stderr'].encode())
                    and list(runtime_dir.iterdir()) == [target] and sha(target.read_bytes()) == sha(elf.read_bytes()),
                    'source-free native integer observation differs')
        records.append({'id': case['id'], 'status': 'passed'})
        save(root / 'progress.json', records)
    save(root / 'result.json', {'status': 'passed', 'compiler': identity(compiler), 'manifest_sha256': ORACLE_SHA256,
                              'route': 'native-transport' if native else 'reference-full', 'cases': records})


def native_artifacts(root, expected):
    suffixes = ('.elf', '.ll', '.ox', '.stdout', '.stderr', '.status', '.source-sha256', '.run.txt')
    require({p.name for p in root.iterdir()} == {name + suffix for name in expected for suffix in suffixes},
            'native evidence closed membership differs')
    for name in expected:
        require((root / (name + '.elf')).read_bytes().startswith(b'\x7fELF'), 'native evidence is not ELF')
        require((root / (name + '.source-sha256')).read_text().strip() == sha((root / (name + '.ox')).read_bytes()),
                'native source identity differs')
        require((root / (name + '.status')).read_text().strip() in ('0', '1'), 'native status missing or signaled')
        require((root / (name + '.ll')).stat().st_size > 0, 'native IR is empty')
    return [identity(path) for path in sorted(root.iterdir())]




def admit_toolchain(repo, root, env, tools, evidence, cargo_name='cargo'):
    """Bind PATH dispatch and actual current versions, not a stale version log."""
    require(not any(k in env for k in ('RUSTC', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_RUSTC',
                'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER',
                'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER')), 'toolchain override is forbidden')
    found = {}
    for name, spelling in (('cargo', cargo_name), ('rustc', 'rustc')):
        selected = shutil.which(spelling, path=env.get('PATH'))
        require(selected is not None and identity(Path(selected).resolve()) == tools[name],
                'resolved toolchain differs from admitted tools: ' + name)
        stdout, stderr = invoke(root, 'current-tool-' + name, [selected, '--version', '--verbose'], cwd=repo, env=env)
        require(not stderr and stdout == (evidence / ('tool-' + name + '.stdout')).read_bytes(),
                'current tool version differs from original ordinary build: ' + name)
        found[name] = selected
    require(re.search(rb'^release: 1\.99\.0$', (evidence / 'tool-rustc.stdout').read_bytes(), re.MULTILINE),
            'actual qualified Rust 1.99.0 required')
    return found


def run_privacy(repo, root, env, expected, cargo):
    """Retain all eleven registered probe compiler streams, including refusals."""
    root.mkdir()
    checkout = root / 'checkout'
    privacy.materialize_checkout(repo, checkout)
    module = checkout / 'src/frontend/oir/owned/source/mod.rs'
    original = module.read_text()
    reports = []
    require([p[0] for p in privacy.PROBES] == expected, 'privacy source roster changed')
    for name, succeeds, code, body in privacy.PROBES:
        directory = root / name; directory.mkdir()
        module.write_text(original + '\nmod byte_storage_privacy_probe {\nuse super::*;\n' + body + '\n}\n')
        shutil.copy2(module, directory / 'probed-module.rs')
        stdout, _ = invoke(directory, 'cargo-check', [cargo, 'check', '--locked', '--bin', 'oxid', '--message-format=json'],
                           cwd=checkout, env=env, expected_status=0 if succeeds else 101, timeout=180)
        errors = [r['message'] for r in (json.loads(line, object_pairs_hook=_unique_object) for line in stdout.splitlines())
                  if r.get('reason') == 'compiler-message' and r['message']['level'] == 'error']
        codes = [e['code']['code'] for e in errors if e.get('code')]
        require((not errors) if succeeds else (code in codes and not set(codes) & {'E0412', 'E0422', 'E0432', 'E0433'}),
                'privacy probe failed for an unrelated reason: ' + name)
        reports.append({'probe': name, 'expected_success': succeeds, 'error_codes': codes})
    module.write_text(original)
    save(root / 'result.json', {'status': 'passed', 'probes': reports})


def validate_provider(summary, compiler, head, tree):
    parities, refusals, captures = provider.expected_results()
    require(len(parities) == 12 and len(refusals) == 216 and summary.get('schema') == 'rfc0031-byte-storage-provider-boundary-v1'
            and summary.get('status') == 'passed' and summary.get('compiler_sha256') == compiler['sha256']
            and summary.get('source_head') == head and summary.get('source_tree') == tree, 'provider identity or counts differ')
    require(summary.get('recipe') == provider.recipe() and summary.get('unchanged_invalid_parities') == parities
            and summary.get('closed_boundary_refusals') == refusals
            and summary.get('pre_provider_parse_refusals') == [r for r in refusals if '/byte-enum-payload/' in r]
            and summary.get('protocol_or_import_refusals') == [r for r in refusals if '/byte-enum-payload/' not in r]
            and [(r['version'], r['name'], r['source_sha256'], r['kind']) for r in summary.get('captures', [])] == captures,
            'provider exact recipe differs')
    require(summary.get('frozen_protocols') == ['OPA1/AST1/STF1', 'OPA2/AST2/STF2'] and summary.get('caps') == [128, 255]
            and summary.get('frame_bytes') == [1559, 2607, 1575] and summary.get('invalid_source_llvm_invocations') == 0,
            'provider frozen boundaries differ')


def controller_inputs(repo):
    paths = [p for directory in ('scripts', 'tests/qualification/byte_storage_current') for p in (repo / directory).rglob('*') if p.is_file()]
    paths += [repo / '.github/workflows/ci.yml', repo / 'oxid.toml', repo / STAGER, repo / PUBLIC_SOURCE]
    return [identity(p) for p in sorted(paths)]


def verify(args, root):
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'Linux x86_64 required')
    repo = args.repo.resolve(strict=True); llvm = args.llvm_bin.resolve(strict=True)
    require(not root.is_relative_to(repo), 'evidence must be outside checkout')
    reject_bytecode(repo)
    env = enum_gate.source_test_environment(repo, dict(os.environ, PYTHONDONTWRITEBYTECODE='1', OXID_LLVM_BIN=str(llvm)))
    forbidden = [k for k in env if k.startswith('CARGO_PROFILE_') or k in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_RUSTC', 'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'PYTHONOPTIMIZE')]
    require(not forbidden, 'ordinary source-only profiles forbid overrides: ' + ', '.join(forbidden))
    require(env.get('CARGO_TARGET_DIR') and Path(env['CARGO_TARGET_DIR']).resolve() == repo / 'target',
            'privacy probes require coordinated CARGO_TARGET_DIR at this checkout target')
    env.pop('OXID_PATH', None)
    env['OXID_CACHE_DIR'] = str(root / 'cache')
    head_raw, _ = enum_gate.git_command(root, 'git-head', repo, 'rev-parse', 'HEAD', 'HEAD^{tree}', env=env)
    head, tree = head_raw.decode().splitlines()
    require(head == args.expected_head, 'checkout is not the exact requested head')
    status, _ = enum_gate.git_command(root, 'git-status', repo, 'status', '--porcelain=v1', '--untracked-files=all', env=env)
    require(not status, 'qualification requires clean checkout')
    source_manifest = builds.source_manifest(repo, SOURCE_SHA256)
    core, _ = enum_gate.git_command(root, 'source-files', repo, 'ls-files', '-z', '--', *builds.CORE_PATHS, env=env)
    source = builds.source_identity(repo, source_manifest, core)
    receipt = read_json(args.build_evidence / 'source-identity.json')
    require(receipt.get('head') == head and receipt.get('tree') == tree and receipt.get('reviewed_source_manifest', {}).get('sha256') == SOURCE_SHA256,
            'ordinary build receipt is not exact-head/tree/source')
    rust_version = (args.build_evidence / 'tool-rustc.stdout').read_text()
    require(re.search(r'^release: 1\.99\.0$', rust_version, re.MULTILINE), 'qualified Rust 1.99.0 required')
    manifest, registry, cases = admit_package(repo); admit_registry(repo, registry)
    inputs = controller_inputs(repo)
    require(identity(repo / STAGER)['sha256'] == STAGER_SHA256, 'genuine stager source changed')
    tools = read_json(args.build_evidence / 'tools.json')
    for name, record in tools.items(): require(identity(Path(record['path'])) == record, 'prebuilt tool changed: ' + name)
    selected_toolchain = admit_toolchain(repo, root, env, tools, args.build_evidence)
    save(root / 'source-identity.json', {'head': head, 'tree': tree, 'event_sha': args.event_sha, 'source_manifest_sha256': SOURCE_SHA256,
        'reviewed_inputs': source, 'controller_inputs': inputs, 'build_receipt': identity(args.build_evidence / 'source-identity.json'), 'tools': tools})
    stage = root / 'llvm-runtime'
    invoke(root, 'stage-llvm-runtime', [sys.executable, '-B', repo / STAGER, '--output', stage], cwd=repo, env=env)
    require(read_json(stage / 'llvm-runtime-stage.json').get('status') == 'verified', 'genuine dpkg LLVM staging did not verify')
    libraries = [identity(p) for p in sorted((stage / 'libraries').iterdir()) if not p.is_symlink()]
    save(root / 'staged-libraries.json', libraries); env['LD_LIBRARY_PATH'] = str(stage / 'libraries')
    for name, marker in (('clang', 'clang version'), ('llvm-as', 'LLVM version'), ('opt', 'LLVM version'), ('ld.lld', 'LLD')):
        require(identity(llvm / name) == tools[name], 'LLVM executable differs from ordinary build evidence')
        stdout, stderr = invoke(root, 'tool-' + name, [llvm / name, '--version'], cwd=repo, env=env)
        require(not stderr, 'LLVM version stderr'); enum_gate.admit_tool_version(name, marker, stdout)
    run_privacy(repo, root / 'source-privacy', env, registry['privacy_probes'], selected_toolchain['cargo'])
    roster = [r['name'] for r in registry['unit_tests']]; ignored = [r['test'] for r in registry['native_tests']]
    for profile in ('debug', 'release'):
        directory = root / profile; directory.mkdir()
        binaries = builds.reused_binaries(args.build_evidence, directory, repo, repo / 'target', profile)
        binaries['public'] = admit_public_build(args.public_build_evidence, directory, repo, repo / 'target', profile, receipt)
        before = {name: identity(binary) for name, binary in binaries.items()}
        for label, binary, filters, names in (
            ('unit-discovery', binaries['unit'], ['byte_storage', '--list'], roster),
            ('ignored-discovery', binaries['unit'], ['byte_storage', '--list', '--ignored'], ignored),
            ('public-discovery', binaries['public'], ['--list'], registry['public_tests'])):
            stdout, stderr = invoke(directory, label, [binary, *filters, '--color=never'], cwd=repo, env=env)
            require(not stderr, 'test discovery stderr'); u8_gate.admit_listing(stdout, names)
        skips = [x for name in ignored for x in ('--skip', name)]
        stdout, _ = invoke(directory, 'unit-resources-and-semantics', [binaries['unit'], 'byte_storage', '--nocapture', '--test-threads=1', '--color=never', *skips], cwd=repo, env=env, timeout=1800)
        u8_gate.admit_execution(stdout, 73)
        stdout, _ = invoke(directory, 'public-boundaries', [binaries['public'], '--nocapture', '--test-threads=1', '--color=never'], cwd=repo, env=env, timeout=1800)
        u8_gate.admit_execution(stdout, 10)
        for index, row in enumerate(registry['native_tests']):
            child = directory / f'native-{index + 1:02}'; child.mkdir(); artifacts = child / 'artifacts'; artifacts.mkdir()
            stdout, _ = invoke(child, 'execution', [binaries['unit'], row['test'], '--exact', '--ignored', '--nocapture', '--test-threads=1', '--color=never'],
                               cwd=repo, env=dict(env, OXID_OWNED_NATIVE_EVIDENCE=str(artifacts)), timeout=1800)
            u8_gate.admit_execution(stdout, 1)
            save(child / 'artifacts.json', {'test': row['test'], 'elf_count': len(row['artifacts']), 'files': native_artifacts(artifacts, row['artifacts'])})
        run_oracle(repo, directory / 'reference', binaries['cli'], cases, False, env)
        run_oracle(repo, directory / 'native-transport', binaries['cli'], cases, True, env)
        invoke(directory, 'provider', [sys.executable, '-B', repo / 'scripts/qualify_hir_byte_storage_compatibility.py', '--compiler', binaries['cli'], '--llvm-bin', llvm, '--output', directory / 'provider'], cwd=repo, env=env, timeout=3600)
        validate_provider(read_json(directory / 'provider/byte-storage-summary.json'), before['cli'], head, tree)
        require(before == {name: identity(binary) for name, binary in binaries.items()}, 'ordinary binary changed during execution')
        save(directory / 'result.json', {'status': 'passed', 'unit_tests': 73, 'ignored_tests': 4, 'native_elf_cases': 93,
             'reference_cases': 323, 'transport_elf_cases': 256, 'public_boundary_tests': 10, 'provider_refusals': 216})
    require(libraries == [identity(p) for p in sorted((stage / 'libraries').iterdir()) if not p.is_symlink()], 'staged libraries changed')
    for name, record in tools.items(): require(identity(Path(record['path'])) == record, 'tool changed during execution: ' + name)
    require(controller_inputs(repo) == inputs and admit_package(repo)[0] == manifest, 'controller inputs changed')
    core_after, _ = enum_gate.git_command(root, 'source-files-after', repo, 'ls-files', '-z', '--', *builds.CORE_PATHS, env=env)
    require(builds.source_identity(repo, source_manifest, core_after) == source, 'source changed')
    after, _ = enum_gate.git_command(root, 'git-head-after', repo, 'rev-parse', 'HEAD', 'HEAD^{tree}', env=env)
    status, _ = enum_gate.git_command(root, 'git-status-after', repo, 'status', '--porcelain=v1', '--untracked-files=all', env=env)
    require(after == head_raw and not status, 'checkout changed'); reject_bytecode(repo)



def prepare_public_build(args, root):
    """One coordinated integration build per ordinary profile, with real receipts."""
    repo = args.repo.resolve(strict=True)
    reject_bytecode(repo)
    require(not root.is_relative_to(repo), 'public build evidence must be outside checkout')
    env = enum_gate.source_test_environment(repo, dict(os.environ, PYTHONDONTWRITEBYTECODE='1'))
    forbidden = [k for k in env if k.startswith('CARGO_PROFILE_') or k.startswith('CARGO_BIN_EXE_')
                 or k in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_RUSTC', 'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER')]
    require(not forbidden, 'ordinary public build forbids overrides')
    require(not env.get('CARGO_TARGET_DIR') or Path(env['CARGO_TARGET_DIR']).resolve() == repo / 'target',
            'public build Cargo target must match ordinary build receipts')
    head_raw, _ = enum_gate.git_command(root, 'git-head', repo, 'rev-parse', 'HEAD', 'HEAD^{tree}', env=env)
    head, tree = head_raw.decode().splitlines()
    require(head == args.expected_head, 'public build is not requested head')
    status, _ = enum_gate.git_command(root, 'git-status', repo, 'status', '--porcelain=v1', '--untracked-files=all', env=env)
    require(not status, 'public build requires clean checkout')
    source_receipt = read_json(args.build_evidence / 'source-identity.json')
    require(source_receipt.get('head') == head and source_receipt.get('tree') == tree
            and source_receipt.get('reviewed_source_manifest', {}).get('sha256') == SOURCE_SHA256,
            'public build requires exact ordinary source receipt')
    manifest = builds.source_manifest(repo, SOURCE_SHA256)
    core, _ = enum_gate.git_command(root, 'source-files', repo, 'ls-files', '-z', '--', *builds.CORE_PATHS, env=env)
    source = builds.source_identity(repo, manifest, core)
    _, registry, _ = admit_package(repo); admit_registry(repo, registry)
    public_source = identity(repo / PUBLIC_SOURCE)
    tools = read_json(args.build_evidence / 'tools.json')
    selected_toolchain = admit_toolchain(repo, root, env, tools, args.build_evidence, args.cargo)
    shutil.copy2(args.build_evidence / 'source-identity.json', root / 'source-identity.json')
    for profile in ('debug', 'release'):
        directory = root / profile; directory.mkdir()
        binaries = builds.reused_binaries(args.build_evidence, directory, repo, repo / 'target', profile)
        before = {name: identity(binary) for name, binary in binaries.items()}
        argv = [selected_toolchain['cargo'], 'test', '--locked', '--test', 'typed_byte_storage_native', '--no-run', '--message-format=json']
        if profile == 'release': argv.append('--release')
        stdout, _ = invoke(directory, 'public-build', argv, cwd=repo, env=env, timeout=1800)
        rows = [json.loads(line, object_pairs_hook=_unique_object) for line in stdout.splitlines() if line]
        matches = [r for r in rows if r.get('reason') == 'compiler-artifact' and r.get('target', {}).get('name') == 'typed_byte_storage_native'
                   and r['target'].get('kind') == ['test'] and r.get('executable')]
        require(len(matches) == 1, 'public build artifact missing or duplicated')
        binary = Path(matches[0]['executable'])
        require(before == {name: identity(path) for name, path in binaries.items()}, 'integration build changed the already-qualified CLI/unit')
        save(directory / 'public-binary.json', {'public': identity(binary), 'cli': before['cli'], 'source': public_source,
             'profile': profile, 'head': head, 'tree': tree})
        checked = directory / 'admitted'; checked.mkdir()
        admit_public_build(root, checked, repo, repo / 'target', profile, source_receipt)
    require(identity(repo / PUBLIC_SOURCE) == public_source and builds.source_identity(repo, manifest, core) == source,
            'source changed during public build')
    after, _ = enum_gate.git_command(root, 'git-head-after', repo, 'rev-parse', 'HEAD', 'HEAD^{tree}', env=env)
    status, _ = enum_gate.git_command(root, 'git-status-after', repo, 'status', '--porcelain=v1', '--untracked-files=all', env=env)
    require(after == head_raw and not status, 'checkout changed during public build')
    reject_bytecode(repo)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare-public-build', action='store_true')
    parser.add_argument('--cargo', default='cargo')
    for name in ('repo', 'output', 'build-evidence'):
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ('public-build-evidence', 'llvm-bin'):
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--expected-head', required=True)
    parser.add_argument('--event-sha')
    args = parser.parse_args()
    args.repo = args.repo.resolve(strict=True); args.build_evidence = args.build_evidence.resolve(strict=True)
    if not args.prepare_public_build:
        require(args.public_build_evidence is not None and args.llvm_bin is not None and args.event_sha, 'gate requires public-build-evidence, llvm-bin and event-sha')
        args.public_build_evidence = args.public_build_evidence.resolve(strict=True)
    root = args.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    result = {'status': 'failed', 'profiles': ['debug', 'release'], 'scope': 'RFC0031 current source; predecessors remain separate'}
    try:
        if args.prepare_public_build:
            prepare_public_build(args, root)
        else:
            verify(args, root)
        result['status'] = 'passed'
    except Exception as error:
        result['error'] = str(error); raise
    finally:
        save(root / 'result.json', result)
    if args.prepare_public_build:
        print('bounded byte storage public integration binaries: exact ordinary profiles prepared')
        return
    print('bounded byte storage: both ordinary profiles, full 323 reference, 256 native transport, 77 unit, 93 gate ELFs, 10 public, 11 privacy and 216 provider refusals: PASS')


if __name__ == '__main__':
    main()
