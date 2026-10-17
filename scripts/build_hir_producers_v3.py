#!/usr/bin/env python3
"""Experimental V3 test bundle from explicit frozen standalone closures.

This writes local test metadata only, never updates release proofs or pins.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


def digest(data):
    return hashlib.sha256(data).hexdigest()


def build(compiler, llvm_bin, prototype_root, output):
    compiler = compiler.resolve(strict=True)
    llvm_bin = llvm_bin.resolve(strict=True)
    prototype_root = prototype_root.resolve(strict=True)
    compiler_hash = digest(compiler.read_bytes())
    materializers = {name: digest((prototype_root / 'scripts' / name).read_bytes())
                     for name in ['build_v3_parser_feasibility.py', 'build_wide_span_parser.py', 'materialize_v3_static.py']}
    output = output.resolve()
    output.mkdir()
    bundle = output / 'bundle'
    bundle.mkdir()
    env = dict(os.environ, OXID_LLVM_BIN=str(llvm_bin))
    subprocess.run([sys.executable, str(prototype_root / 'scripts/build_v3_parser_feasibility.py'),
                    '--output', str(output / 'parser-source')], check=True, capture_output=True)
    subprocess.run([sys.executable, str(prototype_root / 'scripts/materialize_v3_static.py'),
                    str(output / 'static-source')], check=True, capture_output=True)
    receipts = []
    for role, root in [('parser', 'parser_main'), ('static', 'ast_static_main')]:
        sources = output / (role + '-source')
        argv = [str(compiler), 'compile', str(sources / (root + '.ox')),
                '--edition=typed-preview', '--backend=llvm', '--entry-mode=process',
                '--output', str(bundle / role)]
        result = subprocess.run(argv, env=env, capture_output=True, timeout=180)
        for stream in ['stdout', 'stderr']:
            (output / (role + '.' + stream)).write_bytes(getattr(result, stream))
        receipts.append(dict(role=role, argv=argv, status=result.returncode,
                             source_sha256={p.name: digest(p.read_bytes()) for p in sorted(sources.glob('*.ox'))},
                             stdout_sha256=digest(result.stdout), stderr_sha256=digest(result.stderr)))
        (output / 'experimental-build.json').write_text(json.dumps(dict(
            experimental=True, compiler_sha256=compiler_hash, materializer_sha256=materializers, receipts=receipts), indent=2)+'\n')
        if result.returncode:
            raise RuntimeError(f'{role} build failed; retained logs at {output}')
    hashes = {role: digest((bundle / role).read_bytes()) for role in ['parser', 'static']}
    (bundle / 'manifest.txt').write_text('OXID-HIR-PRODUCERS-3\n'+''.join(
        f'{role} {value}\n' for role, value in hashes.items()))
    if digest(compiler.read_bytes()) != compiler_hash:
        raise RuntimeError('compiler identity changed during bundle construction')
    (output / 'experimental-build.json').write_text(json.dumps(dict(
        experimental=True, compiler_sha256=compiler_hash, materializer_sha256=materializers,
        executable_sha256=hashes, manifest_sha256=digest((bundle / 'manifest.txt').read_bytes()),
        materialization_sha256={str(p.relative_to(output)): digest(p.read_bytes())
                               for role in ['parser', 'static']
                               for p in sorted((output / (role + '-source')).glob('*.json'))},
        receipts=receipts), indent=2)+'\n')
    return bundle


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['compiler', 'llvm-bin', 'prototype-root', 'output']:
        parser.add_argument('--'+name, type=Path, required=True)
    args = parser.parse_args()
    print(build(args.compiler, args.llvm_bin, args.prototype_root, args.output))
