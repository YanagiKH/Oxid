#!/usr/bin/env python3
"""Check square-root bits with an integer oracle and source-free native programs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import resource
import shutil
import subprocess
import time

from float_root_oracle import sqrt_bits
import linux_native_target as target
import verify_floats as previous

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def patterns(width):
    fraction, exponent_bits = (23, 8) if width == 32 else (52, 11)
    infinity = ((1 << exponent_bits) - 1) << fraction
    one = ((1 << (exponent_bits - 1)) - 1) << fraction
    sign = 1 << (width - 1)
    values = [0, sign, 1, (1 << fraction) - 1, 1 << fraction,
              one - 1, one, one + 1, one + (1 << fraction) - 1,
              one + (1 << fraction), one + (1 << fraction) + 1,
              one + 2 * (1 << fraction), infinity - 1, infinity,
              infinity | sign, infinity | 1, infinity | (1 << (fraction - 1)),
              infinity | sign | 17, sign | one, sign | 1]
    rng = random.Random(104729 + width)
    for _ in range(48):
        bits = rng.randrange(infinity)
        values.extend([bits, bits | sign])
    return sorted(set(values))


def source(width):
    text, result, _ = previous.source('f' + str(width), 'identity')
    before = 'let value=a;return value.to_bits();'
    after = 'let value=a.sqrt();return value.to_bits();'
    if text.count(before) != 1:
        raise ValueError('changed argument-parser source boundary')
    return text.replace(before, after, 1), result


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))


def verify_json(result, width, bits, result_type):
    if result.returncode or result.stderr:
        raise ValueError('JSON reference execution failed')
    value = json.loads(result.stdout)
    expected = sqrt_bits(bits, width)
    expected = str(expected) if width == 64 else expected
    if (value.get('schema_version') != 1 or value.get('kind') != 'run-summary'
            or value.get('edition') != 'typed-preview' or value.get('errors') != 0
            or value.get('success') is not True
            or value.get('result') != {'type': result_type, 'value': expected}
            or type(value['result']['value']) is not type(expected)):
        raise ValueError('JSON square-root result differs from oracle')


def verify(args):
    if not __debug__:
        raise ValueError('verification refuses optimized Python')
    host = target.required_host()
    if not args.reference_only and args.llvm_bin is None:
        raise ValueError('--llvm-bin is required for native verification')
    output = args.output.resolve()
    if output.exists() or output == ROOT or ROOT in output.parents:
        raise ValueError('output must be a new directory outside the repository')
    compilers = [p.resolve(strict=True) for p in args.compilers]
    if len(compilers) != len(set(compilers)) or not all(p.is_file() and os.access(p, os.X_OK) for p in compilers):
        raise ValueError('compilers must be distinct executable files')
    output.mkdir(parents=True)
    pins = [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]
    report = {'status': 'running', 'controller_sha256': sha(Path(__file__)),
              'oracle_sha256': sha(ROOT / 'scripts/float_root_oracle.py'),
              'source_parser_sha256': sha(ROOT / 'scripts/verify_floats.py'),
              'target': host.triple, 'elf_machine': host.elf_machine,
              'reference_only': args.reference_only, 'compilers': pins,
              'commands': [], 'deployments': [], 'reference_cases': 0, 'native_cases': 0,
              'scope': 'f32/f64 square-root rounding and IEEE classes; no fast math'}
    def save():
        (output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    def invoke(name, argv, cwd, native=False):
        directory = output / 'commands' / name
        directory.mkdir(parents=True)
        environment = {} if native else {**os.environ, 'OXID_CACHE_DIR': str(output / 'legacy-cache')}
        if not native and args.llvm_bin is not None:
            environment['OXID_LLVM_BIN'] = str(args.llvm_bin.resolve(strict=True))
        environment.pop('OXID_PATH', None)
        started = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=cwd, env=environment,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=90,
                                preexec_fn=native_limits if native else None)
        for stream in ['stdout', 'stderr']:
            (directory / stream).write_bytes(getattr(result, stream))
        report['commands'].append({'name': name, 'argv': [str(x) for x in argv], 'cwd': str(cwd),
                                   'exit_status': result.returncode, 'stdout_sha256': sha(directory / 'stdout'),
                                   'stderr_sha256': sha(directory / 'stderr'), 'seconds': time.monotonic() - started,
                                   'address_space_limit': 33554432 if native else None, 'open_file_limit': 32 if native else None})
        return result
    def bits_result(result, expected):
        if (result.returncode, result.stdout, result.stderr) != (0, str(expected).encode() + b'\n', b''):
            raise ValueError('square-root result differs from exact integer oracle: ' + repr(result))
    try:
        save()
        for index, compiler in enumerate(compilers):
            for policy in ['bounded', 'hosted']:
                for width in [32, 64]:
                    label = f'compiler-{index}-{policy}-f{width}'
                    project = output / 'projects' / label
                    (project / 'src').mkdir(parents=True)
                    text, result_type = source(width)
                    (project / 'src/main.ox').write_text(text)
                    (project / 'oxid.toml').write_text(f'[project]\nname="roots"\nedition="typed-preview"\n[build]\nentry-mode="result"\n[execution]\npolicy="{policy}"\n')
                    checked = invoke(label + '-check', [compiler, 'check'], project)
                    if checked.returncode:
                        raise ValueError('square-root source rejected: ' + repr(checked))
                    rows = patterns(width)
                    for case, bits in enumerate(rows):
                        expected = sqrt_bits(bits, width)
                        bits_result(invoke(label + f'-reference-{case}', [compiler, 'run', '--', bits, 0], project), expected)
                        report['reference_cases'] += 1
                    for case, bits in enumerate(rows[:20]):
                        result = invoke(label + f'-json-{case}', [compiler, 'run', '--message-format=json', '--', bits, 0], project)
                        verify_json(result, width, bits, result_type)
                    deployments = []
                    if not args.reference_only:
                        for profile in ['dev', 'release']:
                            built = invoke(label + '-' + profile + '-build', [compiler, 'build', '--profile', profile, '--reproducible'], project)
                            if built.returncode or built.stderr:
                                raise ValueError('native square-root build failed: ' + repr(built))
                            deployed = output / 'deployed' / label / profile / 'roots'
                            deployed.parent.mkdir(parents=True)
                            shutil.copy2(project / '.oxid/bin/roots', deployed)
                            data = deployed.read_bytes()
                            if data[:6] != b'\x7fELF\x02\x01' or int.from_bytes(data[18:20], 'little') != host.elf_machine:
                                raise ValueError('native program has the wrong ELF architecture')
                            deployments.append((profile, deployed))
                    project.rename(project.with_name(label + '-retained-source'))
                    if project.exists():
                        raise ValueError('source project remains available')
                    for profile, deployed in deployments:
                        digest = sha(deployed)
                        for case, bits in enumerate(rows):
                            bits_result(invoke(label + f'-{profile}-{case}', [deployed, bits, 0], deployed.parent, True), sqrt_bits(bits, width))
                            report['native_cases'] += 1
                        if sha(deployed) != digest or list(deployed.parent.iterdir()) != [deployed]:
                            raise ValueError('deployed executable changed or depends on colocated inputs')
                        report['deployments'].append({'path': str(deployed), 'sha256': digest, 'profile': profile,
                                                       'policy': policy, 'width': width, 'source_removed': True,
                                                       'elf_machine': host.elf_machine, 'cases': len(rows)})
                    save()
                    print('passed ' + label, flush=True)
        if pins != [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]:
            raise ValueError('compiler changed during qualification')
        if report['controller_sha256'] != sha(Path(__file__)) or report['oracle_sha256'] != sha(ROOT / 'scripts/float_root_oracle.py') or report['source_parser_sha256'] != sha(ROOT / 'scripts/verify_floats.py'):
            raise ValueError('verification inputs changed')
        report['status'] = 'passed'
    except BaseException as error:
        report.update(status='failed', failure=f'{type(error).__name__}: {error}')
        raise
    finally:
        save()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compilers', type=Path, nargs='+')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--llvm-bin', type=Path)
    parser.add_argument('--reference-only', action='store_true')
    verify(parser.parse_args())
