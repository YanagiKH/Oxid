#!/usr/bin/env python3
"""Prepare isolated additive observer and produce manifest-bound binary receipts.

Source toolchains/env.sh first. Run builds only while holding the coordinated
heavy-build slot. No corpus, expected-output or compiler semantics are observed.
"""
import sys
sys.dont_write_bytecode = True
import argparse
import difflib
import os
import shutil
import subprocess
import sys
from pathlib import Path, PurePosixPath
from contracts import need, sha, load, save, binding, verify
from runtime import source_manifest, process
from authority import CURRENT_SOURCE_SHA, CURRENT_FILES_SHA, LIFECYCLE_PATCH_SHA, OBSERVER_FILES_SHA

def observer_path_order(paths, root):
    # Preserve the approved POSIX component order on every actual host.
    return sorted(paths, key=lambda path: PurePosixPath(path.relative_to(root).as_posix()).parts)

def prepare(args):
    source = Path(args.source_root).resolve()
    manifest_path = Path(args.manifest).resolve()
    source_manifest(manifest_path, source)
    need(sha(manifest_path.read_bytes()) == CURRENT_SOURCE_SHA, 'unapproved observer base manifest')
    output = Path(args.out).resolve()
    output.mkdir(parents=True, exist_ok=False)
    dest = output / 'source'
    dest.mkdir()
    original = load(manifest_path)
    for row in original['files']:
        target = dest / row['path']
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(verify(source / row['path'], row))
    patch = Path(args.observer_patch).resolve()
    need(sha(patch.read_bytes()) == args.observer_patch_sha256 == LIFECYCLE_PATCH_SHA, 'observer approved patch binding')
    git = ['git', '-c', 'core.autocrlf=false', '-c', 'core.eol=lf', 'apply']
    p = subprocess.run([*git, '--check', str(patch)], cwd=dest, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    need(p.returncode == 0, 'observer patch precondition: ' + p.stderr.decode())
    p = subprocess.run([*git, str(patch)], cwd=dest, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    need(p.returncode == 0, 'observer patch apply: ' + p.stderr.decode())
    files = []
    changed = []
    before = {r['path']: r for r in original['files']}
    for path in observer_path_order(dest.rglob('*'), dest):
        if path.is_file():
            rel = path.relative_to(dest).as_posix()
            row = {'path': rel, 'bytes': path.stat().st_size, 'sha256': sha(path.read_bytes())}
            files.append(row)
            if before.get(rel) != row:
                changed.append(rel)
    expected = {'src/frontend/mod.rs', 'src/frontend/project.rs', 'src/frontend/lexer.rs', 'src/frontend/parser.rs',
                'src/frontend/oir/source/sealed.rs', 'src/frontend/declaration_index/sealed.rs',
                'src/frontend/driver.rs', 'src/frontend/lifecycle_observer.rs'}
    need(set(changed) == expected and len(files) == len(original['files']) + 1, 'observer patch source scope')
    save(output / 'observer-source.json', {'schema_version': 1, 'kind': 'unit4-public-v3-additive-observer',
         'base_source_manifest': binding(manifest_path), 'base_source_manifest_sha256': sha(manifest_path.read_bytes()),
         'observer_patch': binding(patch), 'counter_semantics': 'Logical stage events, not OS syscalls',
         'files': files, 'changed_paths': changed})
    source_manifest(output / 'observer-source.json', dest)
    save(output / 'prepared.json', {'source_root': str(dest), 'source_manifest': binding(output / 'observer-source.json'), 'base_source_manifest': binding(manifest_path), 'compiler_invocations': 0})
    print(output / 'prepared.json')

def build(args):
    source = Path(args.source_root).resolve()
    manifest = Path(args.manifest).resolve()
    source_manifest(manifest, source)
    source_data = load(manifest)
    files_sha = sha(__import__('json').dumps(source_data['files'], sort_keys=True, separators=(',', ':')).encode())
    if args.kind == 'unit4-public-v3-candidate':
        need(sha(manifest.read_bytes()) == CURRENT_SOURCE_SHA and files_sha == CURRENT_FILES_SHA, 'unapproved current build source')
    else:
        need(files_sha == OBSERVER_FILES_SHA and source_data['base_source_manifest_sha256'] == CURRENT_SOURCE_SHA and source_data['observer_patch']['sha256'] == LIFECYCLE_PATCH_SHA, 'unapproved observer build source')
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=False)
    target = Path(args.target).resolve()
    env = {**os.environ, 'CARGO_INCREMENTAL': '0', 'CARGO_BUILD_JOBS': '2', 'CARGO_TARGET_DIR': str(target)}
    need('CARGO_HOME' in env and 'RUSTUP_HOME' in env, 'source toolchains/env.sh first')
    rustc = subprocess.check_output(['rustc', '-Vv'], env=env).decode()
    need(rustc.startswith('rustc 1.99.0 '), 'expected Rust 1.99.0')
    binaries = {}
    receipts = {}
    for profile in ('debug', 'release'):
        source_manifest(manifest, source)
        argv = ['cargo', 'build', '--bin', 'oxid', '--locked', '--offline'] + (['--release'] if profile == 'release' else [])
        before = sha(manifest.read_bytes())
        p = process(argv, source, env, timeout=1800)
        streams = {}
        for name in ('stdout', 'stderr'):
            path = out / (profile + '.' + name)
            path.write_bytes(p[name].encode())
            streams[name] = binding(path)
        need(p['status'] == 0 and not p['timed_out'] and not p['stream_limit_exceeded'], 'build failed: ' + p['stderr'][-2000:])
        source_manifest(manifest, source)
        suffix = '.exe' if os.name == 'nt' else ''
        dest = out / ('oxid-' + profile + suffix)
        shutil.copy2(target / profile / ('oxid' + suffix), dest)
        binaries[profile] = binding(dest)
        receipt = {k: v for k, v in p.items() if k not in ('stdout', 'stderr')}
        receipt.update(profile=profile, source_manifest_sha256=before, source_before=before,
                       source_after=sha(manifest.read_bytes()), binary=binaries[profile], streams=streams,
                       rustc=rustc, environment={k: env[k] for k in ('CARGO_HOME', 'RUSTUP_HOME', 'CARGO_INCREMENTAL', 'CARGO_BUILD_JOBS', 'CARGO_TARGET_DIR')})
        path = out / (profile + '-build.json')
        save(path, receipt)
        receipts[profile] = binding(path)
        print(profile, 'source-bound binary built', flush=True)
    cfg = {'schema_version': 1, 'kind': args.kind, 'source_root': str(source), 'source_manifest': binding(manifest),
           'compiler_head': args.compiler_head, 'compiler_head_tree': args.compiler_head_tree, 'compiler_source_only_tree': args.compiler_source_only_tree, 'binaries': binaries, 'build_receipts': receipts}
    if args.kind == 'unit4-public-v3-lifecycle-observer':
        cfg['base_source_manifest_sha256'] = load(manifest)['base_source_manifest_sha256']
        cfg['observer_patch'] = load(manifest)['observer_patch']
    save(out / 'candidate-binding.json', cfg)
    print(out / 'candidate-binding.json')

def main():
    p = argparse.ArgumentParser()
    s = p.add_subparsers(dest='command', required=True)
    prepare_parser = s.add_parser('prepare-observer')
    prepare_parser.add_argument('--observer-patch', required=True)
    prepare_parser.add_argument('--observer-patch-sha256', required=True)
    build_parser = s.add_parser('build')
    build_parser.add_argument('--target', required=True)
    build_parser.add_argument('--kind', choices=('unit4-public-v3-candidate', 'unit4-public-v3-lifecycle-observer'), required=True)
    build_parser.add_argument('--compiler-head', required=True)
    build_parser.add_argument('--compiler-head-tree', required=True)
    build_parser.add_argument('--compiler-source-only-tree', required=True)
    for parser in (prepare_parser, build_parser):
        parser.add_argument('--source-root', required=True)
        parser.add_argument('--manifest', required=True)
        parser.add_argument('--out', required=True)
    args = p.parse_args()
    prepare(args) if args.command == 'prepare-observer' else build(args)

if __name__ == '__main__':
    main()
