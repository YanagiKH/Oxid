#!/usr/bin/env python3
"""Bounded current stdout qualification of prebuilt ordinary debug/release binaries.

No Rust build occurs here. The enum gate supplies exact-head Cargo receipts;
stdin's admission helpers enforce the same compiler closure and profile rules.
Reference child captures, emitted ELF execution, and injected controls retain
separate evidence. Historical enum and stdin qualification remain independent.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import sys

import verify_bounded_enum_native as enum_gate
import verify_bounded_stdin_native as stdin_gate

HELPERS = Path(__file__).resolve().parents[1] / 'tests/qualification/bounded_stdout_current'
spec = importlib.util.spec_from_file_location('bounded_stdout_controls', HELPERS / 'controls.py')
controls = importlib.util.module_from_spec(spec)
spec.loader.exec_module(controls)
c = stdin_gate.controls
require = c.require
REVIEWED_SOURCE_SHA256 = 'd5d1492a4873a40a53867d81062eec81f35aeecfa3468b70ceb8ce1f353fbadb'


def admit_source_receipt(previous, head, tree, event_sha, manifest_id, reviewed):
    require(previous.get('head') == head and previous.get('tree') == tree
            and previous.get('event_sha') == event_sha, 'prebuilt evidence is not bound to this exact head/tree/event')
    require(previous.get('reviewed_source_manifest') == manifest_id
            and previous.get('reviewed_inputs') == reviewed,
            'prebuilt evidence lacks the complete current source identity')


def controller_inputs(repo):
    paths = [Path(__file__).resolve(), repo / 'scripts/verify_bounded_enum_native.py',
             repo / 'scripts/verify_bounded_stdin_native.py',
             repo / 'tests/qualification/bounded_stdin_current/controls.py',
             repo / 'scripts/verify_bounded_stack_artifact.py', repo / '.github/workflows/ci.yml']
    paths.extend(sorted(path for path in HELPERS.iterdir() if path.is_file()))
    return [c.identity(path) for path in paths]


def verify(args, root):
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'Linux x86_64 required')
    repo = c.no_symlinks(args.repo).resolve(strict=True)
    llvm = c.no_symlinks(args.llvm_bin).resolve(strict=True)
    evidence = c.no_symlinks(args.build_evidence).resolve(strict=True)
    target = c.no_symlinks(args.target_dir or repo / 'target').resolve(strict=True)
    require(not os.environ.get('RUSTFLAGS') and not os.environ.get('CARGO_ENCODED_RUSTFLAGS'),
            'ordinary profiles require no additional Rust flags')
    require(args.source_manifest_sha256 == REVIEWED_SOURCE_SHA256, 'unreviewed stdout source seal')
    env = enum_gate.source_test_environment(repo, controls.clean_env(dict(os.environ, OXID_LLVM_BIN=str(llvm))))
    head, _ = enum_gate.git_command(root, 'git-head', repo, 'rev-parse', 'HEAD', 'HEAD^{tree}', env=env)
    observed_head, tree = head.decode().splitlines()
    require(observed_head == args.expected_head, 'checkout is not the exact requested head')
    status, _ = enum_gate.git_command(root, 'git-status', repo, 'status', '--porcelain=v1', env=env)
    require(not status, 'current checkout must be clean')
    manifest = stdin_gate.source_manifest(repo, args.source_manifest_sha256)
    core, _ = enum_gate.git_command(root, 'source-files', repo, 'ls-files', '-z', '--', *stdin_gate.CORE_PATHS, env=env)
    reviewed = stdin_gate.source_identity(repo, manifest, core)
    manifest_id = c.identity(repo / stdin_gate.MANIFEST_PATH)
    source_receipt = evidence / 'source-identity.json'
    previous = json.loads(c.read_file(source_receipt, c.MAX_STREAM))
    admit_source_receipt(previous, observed_head, tree, args.event_sha, manifest_id, reviewed)
    inputs = controller_inputs(repo)
    (root / 'reviewed-source.json').write_bytes(c.read_file(repo / stdin_gate.MANIFEST_PATH))
    c.save_json(root / 'source-identity.json', {'head': observed_head, 'tree': tree, 'event_sha': args.event_sha,
        'profiles': args.profiles, 'reviewed_source_manifest': manifest_id, 'reviewed_inputs': reviewed,
        'controller_inputs': inputs, 'prebuilt_source_receipt': c.identity(source_receipt)})
    build_inputs = [c.identity(evidence / name) for name in ('source-identity.json', 'tools.json')]
    build_inputs += [c.identity(evidence / profile / name) for profile in args.profiles
        for name in ('binaries.json', 'unit-build.stdout', 'unit-build.stderr', 'unit-build.json',
                     'cli-build.stdout', 'cli-build.stderr', 'cli-build.json')]
    tools = {}
    for name, marker in (('clang', 'clang version'), ('llvm-as', 'LLVM version'), ('opt', 'LLVM version'), ('ld.lld', 'LLD')):
        tool = llvm / name
        result, _ = c.run(root / ('tool-' + name), [tool, '--version'], env)
        require(result.returncode == 0 and not result.stderr, 'native tool version failed')
        enum_gate.admit_tool_version(name, marker, result.stdout)
        tools[name] = c.identity(tool.resolve(strict=True))
    prior_tools = json.loads(c.read_file(evidence / 'tools.json'))
    require(all(prior_tools.get(name, {}).get('sha256') == identity['sha256']
                and prior_tools[name].get('bytes') == identity['bytes']
                and Path(prior_tools[name]['path']).resolve(strict=True) == Path(identity['path'])
                for name, identity in tools.items()),
            'LLVM tool identities differ from the admitted enum build')
    c.save_json(root / 'tools.json', tools)
    c.save_json(root / 'build-inputs.json', build_inputs)
    instrumentation = controls.build_instrumentation(root, llvm / 'clang', env)
    instrument_ids = [c.identity(path) for path in instrumentation.values()]
    profiles = []
    for profile in args.profiles:
        directory = c.fresh(root / profile)
        binaries = stdin_gate.reused_binaries(evidence, directory, repo, target, profile)
        before = {name: c.identity(binary) for name, binary in binaries.items()}
        suite = controls.Suite(directory, binaries['unit'], binaries['cli'], env, instrumentation)
        suite.selection()
        suite.raw()
        suite.sources()
        suite.public()
        suite.finish()
        # One dispatch per admitted ordinary CLI; its independent decoder and
        # 80-byte artifact protocol remain owned by the artifact controller.
        require(c.identity(binaries['cli']) == before['cli'], 'CLI changed before artifact dispatch')
        result, _ = c.run(directory / 'artifact-controller', [sys.executable, '-B',
            repo / 'scripts/verify_bounded_stack_artifact.py', '--oxid', binaries['cli'],
            '--output', directory / 'stack-artifact', '--native'], env, timeout=1200)
        require(result.returncode == 0 and not result.stderr, 'public producer/loader artifact controller failed')
        require(before == {name: c.identity(binary) for name, binary in binaries.items()},
                'prebuilt binaries changed during execution')
        c.save_json(directory / 'binary-linkage.json', {'profile': profile, 'before': before,
            'after': {name: c.identity(binary) for name, binary in binaries.items()},
            'artifact_controller': c.identity(repo / 'scripts/verify_bounded_stack_artifact.py'),
            'artifact_result': c.identity(directory / 'stack-artifact/report.json')})
        profiles.append({'profile': profile, 'case_counts': suite.counts})
    core, _ = enum_gate.git_command(root, 'source-files-after', repo, 'ls-files', '-z', '--', *stdin_gate.CORE_PATHS, env=env)
    require(stdin_gate.source_identity(repo, manifest, core) == reviewed
            and c.identity(repo / stdin_gate.MANIFEST_PATH) == manifest_id, 'source authority changed during execution')
    require(controller_inputs(repo) == inputs, 'controller inputs changed during execution')
    require([c.identity(Path(row['path'])) for row in build_inputs] == build_inputs, 'Cargo build receipts changed during execution')
    require([c.identity(path) for path in instrumentation.values()] == instrument_ids, 'injected instrumentation changed during execution')
    require(all(c.identity(Path(value['path'])) == value for value in tools.values()), 'LLVM tool identity changed during execution')
    require(c.identity(source_receipt) == json.loads(c.read_file(root / 'source-identity.json'))['prebuilt_source_receipt'],
            'prebuilt source receipt changed during execution')
    after, _ = enum_gate.git_command(root, 'git-head-after', repo, 'rev-parse', 'HEAD', 'HEAD^{tree}', env=env)
    status, _ = enum_gate.git_command(root, 'git-status-after', repo, 'status', '--porcelain=v1', env=env)
    require(after == head and not status, 'checkout changed during execution')
    c.save_json(root / 'source-identity-after.json', {'head': observed_head, 'tree': tree,
        'reviewed_source_manifest': manifest_id, 'reviewed_inputs': reviewed, 'controller_inputs': inputs})
    return profiles


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('repo', 'output', 'build-evidence', 'llvm-bin'):
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ('expected-head', 'event-sha', 'source-manifest-sha256'):
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--target-dir', type=Path)
    parser.add_argument('--profiles', choices=('debug', 'release', 'debug,release'), default='debug,release')
    args = parser.parse_args()
    args.profiles = args.profiles.split(',')
    root = c.fresh(args.output)
    result = {'status': 'failed', 'profiles': args.profiles,
              'scope': 'current bounded stdout; historical enum eight groups and stdin 77 cases remain separate'}
    try:
        result['executed'] = verify(args, root)
        result['status'] = 'passed'
    except Exception as error:
        result['error'] = str(error)
        raise
    finally:
        c.save_json(root / 'result.json', result)
    print('bounded stdout native: ' + ','.join(args.profiles) + ': PASS')


if __name__ == '__main__':
    main()
