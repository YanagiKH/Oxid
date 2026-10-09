#!/usr/bin/env python3
"""Prescribed RFC0030 gate: exact-source builds, resources, native and providers.

Reuses the real bounded-enum ordinary-profile Cargo receipts without rebuilding
or weakening predecessors. Calls the unchanged Debian dpkg LLVM stager itself.
Every ignored u8 test is discovered and executed, with counted retained ELFs.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import re
import sys

import qualify_hir_u8_compatibility as provider
import run_bounded_u8_corpus as corpus
import verify_bounded_enum_native as enum_gate
import verify_bounded_stdin_native as builds

require, save, identity = corpus.require, corpus.save, corpus.identity
PACKAGE = Path('tests/qualification/bounded_u8_current')
STAGER = Path('tests/fixtures/typed_project_unit3_independent/portable/native-v1/stage_llvm_runtime.py')
IGNORED = {
    'frontend::oir::native::u8_tests::u8_scalar_native_pinned_boundary_and_fuel_proof': ('scalar', 16),
    'frontend::oir::owned::source::u8_tests::owned_u8_real_native_helpers_and_all_roundtrips': ('owned', 3),
    'frontend::oir::owned::u8_tests::owned_u8_real_native_range_and_fuel_match_reference': ('owned', 48),
    'frontend::oir::owned::u8_tests::owned_u8_real_native_unsigned_comparisons': ('owned', 24),
}


def admit_listing(data, expected):
    lines = [line for line in data.decode().splitlines() if line]
    require(lines and lines[-1] == f'{len(expected)} tests, 0 benchmarks', 'test discovery count differs')
    require(sorted(lines[:-1]) == sorted(name + ': test' for name in expected),
            'missing, duplicate or unexpected u8 test')


def admit_execution(data, count):
    summaries = [line for line in data.decode().splitlines() if line.startswith('test result:')]
    require(len(summaries) == 1 and re.fullmatch(
        rf'test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; \d+ filtered out; finished in \d+(?:\.\d+)?s',
        summaries[0]), 'selected tests did not all execute successfully')


def native_artifacts(root, route, count):
    if route == 'scalar':
        elfs = sorted(root.glob('*/source-free/program'))
        require(len(elfs) == count and (root / 'receipts.txt').is_file(), 'incomplete scalar native evidence')
        for elf in elfs:
            directory = elf.parent.parent
            require(all((directory / name).is_file() for name in (
                'input.ox', 'module.ll', 'stdout.bin', 'stderr.bin', 'status.txt')),
                'missing scalar IR/source/streams/status')
    else:
        elfs = sorted(root.glob('*.elf'))
        require(len(elfs) == count, 'incomplete owned native evidence; OXID_UNARY_NATIVE_EVIDENCE required')
        for elf in elfs:
            require(all(elf.with_suffix(suffix).is_file() for suffix in ('.ll', '.stdout', '.stderr', '.status')),
                    'missing owned IR/streams/status')
    require(all(path.read_bytes()[:4] == b'\x7fELF' for path in elfs), 'retained native artifact is not ELF')
    return [identity(path) for path in sorted(root.rglob('*')) if path.is_file()]


def controller_inputs(repo):
    names = [Path('scripts') / name for name in (
        'verify_bounded_u8_native.py', 'run_bounded_u8_corpus.py', 'test_bounded_u8_native.py',
        'qualify_hir_u8_compatibility.py', 'qualify_hir_producer_diagnostics.py',
        'build_hir_producers_v2.py', 'verify_bounded_enum_native.py', 'verify_bounded_stdin_native.py')]
    names += [Path('.github/workflows/ci.yml'), STAGER]
    names += sorted(path.relative_to(repo) for path in (repo / PACKAGE).rglob('*') if path.is_file())
    return [identity(repo / path) for path in names]


def validate_provider_summary(summary, compiler, head, tree):
    parities, refusals, captures = provider.expected_results()
    require(summary.get('schema') == 'rfc0030-frozen-provider-successor-1'
            and summary.get('status') == 'passed' and summary.get('compiler_sha256') == compiler['sha256']
            and summary.get('source_head') == head and summary.get('source_tree') == tree,
            'provider summary not bound to the selected compiler/source')
    require(summary.get('recipe') == provider.recipe() and summary.get('legacy_parities') == parities
            and summary.get('changed_domain_refusals') == refusals
            and [(row['version'], row['name'], row['source_sha256'], row['kind'])
                 for row in summary.get('captures', [])] == captures,
            'provider recipe incomplete')
    require(summary.get('frozen_protocols') == ['OPA1/AST1/STF1', 'OPA2/AST2/STF2']
            and summary.get('caps') == [128, 255] and summary.get('frame_bytes') == [1559, 2607, 1575]
            and summary.get('invalid_source_llvm_invocations') == 0, 'provider boundary changed')


def verify(args, root):
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'Linux x86_64 required')
    repo = args.repo.resolve(strict=True)
    require(not root.is_relative_to(repo), 'evidence must be outside checkout')
    env = enum_gate.source_test_environment(repo, dict(os.environ, PYTHONDONTWRITEBYTECODE='1',
                                                     OXID_LLVM_BIN=str(args.llvm_bin.resolve(strict=True))))
    forbidden = [name for name in env if name.startswith('CARGO_PROFILE_') or name in (
        'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER')]
    require(not forbidden, 'ordinary profiles forbid Rust/Cargo overrides: ' + ', '.join(forbidden))
    head, _ = enum_gate.git_command(root, 'git-head', repo, 'rev-parse', 'HEAD', 'HEAD^{tree}', env=env)
    observed_head, tree = head.decode().splitlines()
    require(observed_head == args.expected_head, 'checkout differs from exact expected CI head')
    status, _ = enum_gate.git_command(root, 'git-status', repo, 'status', '--porcelain=v1', '--untracked-files=all', env=env)
    require(not status, 'qualification requires a clean source checkout')
    manifest = builds.source_manifest(repo, builds.REVIEWED_SOURCE_SHA256)
    core, _ = enum_gate.git_command(root, 'source-files', repo, 'ls-files', '-z', '--', *builds.CORE_PATHS, env=env)
    source = builds.source_identity(repo, manifest, core)
    previous = json.loads((args.build_evidence / 'source-identity.json').read_text())
    require(previous.get('head') == observed_head and previous.get('tree') == tree
            and previous.get('reviewed_source_manifest', {}).get('sha256') == builds.REVIEWED_SOURCE_SHA256,
            'build evidence is not for this exact head/tree/source authority')
    rust_version = (args.build_evidence / 'tool-rustc.stdout').read_text()
    require(re.search(r'^release: 1\.99\.0$', rust_version, re.MULTILINE),
            'prebuilt compiler requires qualified Rust 1.99.0')
    (root / 'prebuilt-rustc-version.stdout').write_text(rust_version)
    inputs = controller_inputs(repo)
    save(root / 'source-identity.json', {'head': observed_head, 'tree': tree, 'event_sha': args.event_sha,
        'reviewed_source_manifest_sha256': builds.REVIEWED_SOURCE_SHA256, 'reviewed_inputs': source,
        'controller_inputs': inputs, 'prebuilt_source_receipt': identity(args.build_evidence / 'source-identity.json')})
    # This invokes the original dpkg-query ownership/version/hash stager. There
    # is no stage-receipt override or local extracted-package substitute.
    stage = root / 'llvm-runtime'
    corpus.invoke(root, 'stage-llvm-runtime', [sys.executable, '-B', repo / STAGER, '--output', stage],
                  cwd=repo, env=env)
    staged = json.loads((stage / 'llvm-runtime-stage.json').read_text())
    require(staged.get('status') == 'verified', 'real LLVM runtime staging did not verify')
    staged_libraries = [identity(path) for path in sorted((stage / 'libraries').iterdir()) if not path.is_symlink()]
    save(root / 'staged-library-identities.json', staged_libraries)
    env['LD_LIBRARY_PATH'] = str(stage / 'libraries')
    for name, marker in (('clang', 'clang version'), ('llvm-as', 'LLVM version'), ('opt', 'LLVM version'), ('ld.lld', 'LLD')):
        stdout, stderr = corpus.invoke(root, 'tool-' + name, [args.llvm_bin / name, '--version'], cwd=repo, env=env)
        require(not stderr, 'native tool version stderr')
        enum_gate.admit_tool_version(name, marker, stdout)
    roster = json.loads((repo / PACKAGE / 'tests.json').read_text())
    require(len(roster) == 115 and len(set(roster)) == 115 and set(IGNORED) <= set(roster),
            'reviewed u8 test roster changed')
    for profile in ('debug', 'release'):
        directory = root / profile
        directory.mkdir()
        binaries = builds.reused_binaries(args.build_evidence, directory, repo, repo / 'target', profile)
        before = {name: identity(binary) for name, binary in binaries.items()}
        unit = binaries['unit']
        stdout, stderr = corpus.invoke(directory, 'all-discovery', [unit, 'u8', '--list', '--color=never'], cwd=repo, env=env)
        require(not stderr, 'test discovery stderr')
        admit_listing(stdout, roster)
        stdout, stderr = corpus.invoke(directory, 'ignored-discovery', [unit, 'u8', '--list', '--ignored', '--color=never'], cwd=repo, env=env)
        require(not stderr, 'ignored discovery stderr')
        admit_listing(stdout, list(IGNORED))
        skips = [value for name in IGNORED for value in ('--skip', name)]
        stdout, _ = corpus.invoke(directory, 'resources-and-semantics',
            [unit, 'u8', '--nocapture', '--test-threads=1', '--color=never', *skips], cwd=repo, env=env, timeout=1800)
        admit_execution(stdout, len(roster) - len(IGNORED))
        for index, (name, (route, count)) in enumerate(IGNORED.items()):
            child = directory / f'native-{index + 1:02}'
            child.mkdir()
            artifacts = child / 'artifacts'
            artifacts.mkdir()
            key = 'OXID_U8_NATIVE_EVIDENCE' if route == 'scalar' else 'OXID_UNARY_NATIVE_EVIDENCE'
            stdout, _ = corpus.invoke(child, 'execution', [unit, name, '--exact', '--ignored', '--nocapture',
                '--test-threads=1', '--color=never'], cwd=repo, env=dict(env, **{key: str(artifacts)}), timeout=1800)
            admit_execution(stdout, 1)
            save(child / 'artifacts.json', {'test': name, 'route': route, 'elf_count': count,
                                          'files': native_artifacts(artifacts, route, count)})
        for route in ('scalar', 'owned'):
            corpus.invoke(directory, 'corpus-' + route, [sys.executable, '-B', repo / 'scripts/run_bounded_u8_corpus.py',
                '--compiler', binaries['cli'], '--output', directory / ('corpus-' + route), '--route', route],
                cwd=repo, env=env, timeout=1800)
            summary = json.loads((directory / ('corpus-' + route) / 'summary.json').read_text())
            require(summary['status'] == 'passed' and summary['route'] == route
                    and summary['pair_operator_keys'] == corpus.KEY_COUNT and summary['roundtrip_values'] == 256
                    and summary['compiler'] == before['cli'], 'exhaustive corpus summary incomplete or stale')
        corpus.invoke(directory, 'frozen-provider', [sys.executable, '-B', repo / 'scripts/qualify_hir_u8_compatibility.py',
            '--compiler', binaries['cli'], '--llvm-bin', args.llvm_bin, '--output', directory / 'provider'],
            cwd=repo, env=env, timeout=3600)
        provider_summary = json.loads((directory / 'provider/u8-summary.json').read_text())
        validate_provider_summary(provider_summary, before['cli'], observed_head, tree)
        require(before == {name: identity(binary) for name, binary in binaries.items()}, 'prebuilt binary changed')
        save(directory / 'result.json', {'status': 'passed', 'profile': profile, 'unit_tests': len(roster),
            'native_elf_cases': 91, 'exhaustive_pair_operator_keys_per_route': corpus.KEY_COUNT,
            'roundtrip_values_per_route': 256, 'provider_summary': identity(directory / 'provider/u8-summary.json')})
    require(staged_libraries == [identity(path) for path in sorted((stage / 'libraries').iterdir()) if not path.is_symlink()],
            'staged LLVM libraries changed during execution')
    core_after, _ = enum_gate.git_command(root, 'source-files-after', repo, 'ls-files', '-z', '--', *builds.CORE_PATHS, env=env)
    require(builds.source_manifest(repo, builds.REVIEWED_SOURCE_SHA256) == manifest
            and builds.source_identity(repo, manifest, core_after) == source
            and controller_inputs(repo) == inputs, 'source/controller inputs changed during execution')
    after, _ = enum_gate.git_command(root, 'git-head-after', repo, 'rev-parse', 'HEAD', 'HEAD^{tree}', env=env)
    status, _ = enum_gate.git_command(root, 'git-status-after', repo, 'status', '--porcelain=v1', '--untracked-files=all', env=env)
    require(after == head and not status, 'checkout changed during execution')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('repo', 'output', 'build-evidence', 'llvm-bin'):
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ('expected-head', 'event-sha'):
        parser.add_argument('--' + name, required=True)
    args = parser.parse_args()
    args.build_evidence = args.build_evidence.resolve(strict=True)
    args.llvm_bin = args.llvm_bin.resolve(strict=True)
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    result = {'status': 'failed', 'profiles': ['debug', 'release'],
              'scope': 'Exact-source RFC0030 native, resource, exhaustive integer oracle and frozen-provider compatibility; predecessor gates remain separate.'}
    try:
        verify(args, root)
        result['status'] = 'passed'
    except Exception as error:
        result['error'] = str(error)
        raise
    finally:
        save(root / 'result.json', result)
    print('bounded u8: both ordinary profiles, 115 tests, 91 boundary ELFs, both exhaustive routes, v1/v2 provider controls: PASS')


if __name__ == '__main__':
    main()
