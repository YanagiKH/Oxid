#!/usr/bin/env python3
"""Compare optimizer updates with independent mathematics and source-free native execution."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import struct
import subprocess
import time

import linux_native_target as target
import optimizer_oracles as oracle
import optimizer_programs as programs

ROOT = Path(__file__).resolve().parents[1]
LIBRARY = ROOT / 'apps/cpu-training/tensor'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))


def expected(case):
    parameters = list(case['parameters'])
    state = oracle.initial_state(case['config'], 3)
    for derivative in case['gradients']:
        parameters, state = oracle.update(parameters, derivative, state)
    return parameters, state


def validate(body, case):
    if len(body) < 24:
        raise ValueError('optimizer probe omitted its parameter values')
    actual = list(struct.unpack_from('<3d', body))
    stored = oracle.parse_state(body[24:])
    parameters, state = expected(case)
    largest = 0
    for wanted, observed in [(parameters, actual), (state['first'], stored['first']),
                             (state['second'], stored['second'])]:
        if len(wanted) != len(observed):
            raise ValueError('optimizer moment dimensions changed')
        for left, right in zip(wanted, observed):
            distance = oracle.ulp_distance(left, right)
            largest = max(largest, distance)
            if distance > 16 or (left == 0 and oracle.bits(left) != oracle.bits(right)):
                raise ValueError(f'{case["name"]}: update differs from mathematical oracle by {distance} ULP')
    for key in set(state) - {'first', 'second'}:
        if stored[key] != state[key]:
            raise ValueError('optimizer metadata differs: ' + key)
    return largest


def verify(args):
    if not __debug__:
        raise ValueError('qualification refuses optimized Python')
    host = target.required_host()
    output = args.output.resolve()
    if output.exists() or output == ROOT or ROOT in output.parents:
        raise ValueError('output must be a new directory outside the repository')
    compilers = [p.resolve(strict=True) for p in args.compilers]
    if len(compilers) != len(set(compilers)) or not all(p.is_file() and os.access(p, os.X_OK) for p in compilers):
        raise ValueError('distinct executable compilers are required')
    llvm = args.llvm_bin.resolve(strict=True)
    output.mkdir(parents=True)
    sources = {p.relative_to(LIBRARY).as_posix(): sha(p) for p in LIBRARY.rglob('*')
               if p.is_file() and p.suffix in {'.ox', '.toml', '.md'} and '.oxid' not in p.parts}
    controllers = {Path(__file__).name: sha(Path(__file__)), 'optimizer_oracles.py': sha(Path(oracle.__file__)),
                   'optimizer_programs.py': sha(Path(programs.__file__)),
                   'cpu_training_oracles.py': sha(ROOT / 'scripts/cpu_training_oracles.py')}
    pins = [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]
    report = {'status': 'running', 'controllers': controllers, 'sources': sources, 'compilers': pins,
              'target': host.triple, 'commands': [], 'cases': [], 'deployments': [],
              'scope': 'SGD/Adam/AdamW equations, clipping, schedules, IEEE edge cases and failure atomicity.'}

    def save():
        (output / 'result.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')

    def run(name, argv, cwd, native=False):
        folder = output / 'commands' / name
        folder.mkdir(parents=True)
        environment = {} if native else dict(os.environ, OXID_LLVM_BIN=str(llvm),
                                             OXID_CACHE_DIR=str(output / 'legacy-cache'))
        environment.pop('OXID_PATH', None)
        started = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=cwd, env=environment,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=240,
                                preexec_fn=native_limits if native else None)
        for stream in ['stdout', 'stderr']:
            (folder / stream).write_bytes(getattr(result, stream))
        report['commands'].append({'name': name, 'argv': [str(x) for x in argv], 'cwd': str(cwd),
                                   'exit_status': result.returncode, 'stdout_sha256': sha(folder / 'stdout'),
                                   'stderr_sha256': sha(folder / 'stderr'), 'seconds': time.monotonic()-started,
                                   'address_space_limit': 33554432 if native else None,
                                   'open_file_limit': 32 if native else None})
        save()
        if result.returncode or result.stderr:
            raise ValueError(name + ': optimizer program failed')
        return result.stdout

    try:
        save()
        for index, compiler in enumerate(compilers):
            base = output / f'compiler-{index}'
            library = base / 'library'
            for name, digest in sources.items():
                path = library / name
                path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(LIBRARY / name, path)
                if sha(path) != digest:
                    raise ValueError('library snapshot changed')
            deployments = []
            for case in programs.cases():
                label = f'compiler-{index}-{case["name"]}'
                project = base / 'projects' / case['name']
                (project / 'src').mkdir(parents=True)
                (project / 'src/main.ox').write_text(programs.generate(case))
                (project / 'oxid.toml').write_text('[project]\nname="optimizer-probe"\nedition="typed-preview"\n'
                                                 '[build]\nentry-mode="process"\n[execution]\npolicy="hosted"\n'
                                                 '[dependencies]\ntensor="../../library"\n')
                run(label + '-check', [compiler, 'check'], project)
                reference = run(label + '-reference', [compiler, 'run'], project)
                maximum = validate(reference, case)
                report['cases'].append({'name': label, 'maximum_ulp_error': maximum,
                                        'parameter_count': 3, 'steps': len(case['gradients']),
                                        'failed_update_preserved': 'rejected' in case})
                for profile in ['dev', 'release']:
                    run(label + '-' + profile + '-build', [compiler, 'build', '--profile', profile, '--reproducible'], project)
                    executable = output / 'deployed' / label / profile / 'optimizer-probe'
                    executable.parent.mkdir(parents=True)
                    shutil.copy2(project / '.oxid/bin/optimizer-probe', executable)
                    body = executable.read_bytes()
                    if body[:6] != b'\x7fELF\x02\x01' or int.from_bytes(body[18:20], 'little') != host.elf_machine:
                        raise ValueError('optimizer probe has the wrong native architecture')
                    deployments.append((label, case, profile, executable, reference))
                print('built ' + label, flush=True)
            # All manifest paths and every library input used by the builds move.
            (base / 'projects').rename(base / 'retained-projects')
            library.rename(base / 'retained-library')
            if (base / 'projects').exists() or library.exists():
                raise ValueError('optimizer sources remain at their build paths')
            for label, case, profile, executable, reference in deployments:
                digest = sha(executable)
                actual = run(label + '-' + profile + '-source-free', [executable], executable.parent, native=True)
                if actual != reference or sha(executable) != digest or list(executable.parent.iterdir()) != [executable]:
                    raise ValueError('native optimizer result differs from reference or depends on adjacent inputs')
                maximum = validate(actual, case)
                report['deployments'].append({'case': label, 'path': str(executable), 'sha256': digest,
                                               'profile': profile, 'source_removed': True,
                                               'elf_machine': host.elf_machine, 'maximum_ulp_error': maximum})
            save()
        if sources != {p.relative_to(LIBRARY).as_posix(): sha(p) for p in LIBRARY.rglob('*')
                       if p.is_file() and p.suffix in {'.ox', '.toml', '.md'} and '.oxid' not in p.parts}:
            raise ValueError('optimizer library changed during qualification')
        if controllers != {name: sha(ROOT / 'scripts' / name) for name in controllers}:
            raise ValueError('optimizer oracle or controller changed')
        if pins != [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]:
            raise ValueError('compiler changed during optimizer qualification')
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
    parser.add_argument('--llvm-bin', type=Path, required=True)
    verify(parser.parse_args())
