#!/usr/bin/env python3
"""Check optimizer wire formats, malformed inputs, counters and allocation rollback."""
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
import optimizer_checkpoint_programs as programs
import optimizer_oracles as oracle

ROOT = Path(__file__).resolve().parents[1]
LIBRARY = ROOT / 'apps/cpu-training/tensor'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))


def controls():
    cases = []
    for kind in ['sgd', 'adam', 'adamw']:
        for scheduled in [False, True]:
            settings = oracle.config(kind, clip=.25 if scheduled else 0.,
                                     factor=.5 if scheduled else 0., every=2 if scheduled else 0)
            state = oracle.initial_state(settings, 3)
            if scheduled:
                parameter = [1., 2., 3.]
                for _ in range(3):
                    parameter, state = oracle.update(parameter, [.3, -.2, .4], state)
            body = oracle.encode_state(state)
            cases.append((kind + ('-scheduled' if scheduled else '-initial'), body, 0, body, False))
    for kind in ['sgd', 'adam']:
        state = oracle.initial_state(oracle.config(kind), 0)
        body = oracle.encode_state(state)
        cases.append((kind+'-empty', body, 0, body, False))
    cases += [(name, body, 15, b'', False) for name, body in oracle.invalid_states()]
    state = oracle.initial_state(oracle.config(), 3, steps=(1 << 64)-1)
    body = oracle.encode_state(state)
    cases.append(('maximum-step-roundtrip', body, 0, body, False))
    cases.append(('counter-overflow-is-atomic', body, 0, struct.pack('<3d', 1., 2., 3.) + body, True))
    return cases


def verify(args):
    if not __debug__:
        raise ValueError('qualification refuses optimized Python')
    host = target.required_host()
    output = args.output.resolve()
    if output.exists() or output == ROOT or ROOT in output.parents:
        raise ValueError('output must be a new directory outside the repository')
    compilers = [p.resolve(strict=True) for p in args.compilers]
    if len(set(compilers)) != len(compilers) or not all(p.is_file() and os.access(p, os.X_OK) for p in compilers):
        raise ValueError('distinct executable compilers are required')
    llvm = args.llvm_bin.resolve(strict=True)
    output.mkdir(parents=True)
    sources = {p.relative_to(LIBRARY).as_posix(): sha(p) for p in LIBRARY.rglob('*')
               if p.is_file() and p.suffix in {'.ox', '.toml', '.md'} and '.oxid' not in p.parts}
    controllers = {name: sha(ROOT/'scripts'/name) for name in
                   ['verify_optimizer_checkpoints.py', 'optimizer_checkpoint_programs.py',
                    'optimizer_programs.py', 'optimizer_oracles.py', 'cpu_training_oracles.py']}
    pins = [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]
    report = {'status': 'running', 'controllers': controllers, 'sources': sources, 'compilers': pins,
              'target': host.triple, 'commands': [], 'cases': [], 'deployments': [],
              'scope': 'Optimizer codec validation and round trips; step overflow; invalid configuration and OOM rollback.'}

    def save():
        (output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')

    def run(name, argv, cwd, expected=0, native=False):
        folder = output / 'commands' / name
        folder.mkdir(parents=True)
        environment = {} if native else dict(os.environ, OXID_LLVM_BIN=str(llvm),
                                             OXID_CACHE_DIR=str(output/'legacy-cache'))
        environment.pop('OXID_PATH', None)
        started = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=cwd, env=environment,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=240,
                                preexec_fn=native_limits if native else None)
        for stream in ['stdout', 'stderr']:
            (folder/stream).write_bytes(getattr(result, stream))
        report['commands'].append({'name': name, 'argv': [str(x) for x in argv], 'cwd': str(cwd),
                                   'exit_status': result.returncode, 'expected_status': expected,
                                   'stdout_sha256': sha(folder/'stdout'), 'stderr_sha256': sha(folder/'stderr'),
                                   'seconds': time.monotonic()-started,
                                   'address_space_limit': 33554432 if native else None,
                                   'open_file_limit': 32 if native else None})
        save()
        if result.returncode != expected or result.stderr:
            raise ValueError(name + ': unexpected optimizer codec/configuration result')
        return result.stdout

    def exercise(label, command, cwd, native):
        for name, body, status, expected, overflow in controls():
            (cwd/'input.bin').write_bytes(body)
            actual = run(label+'-'+name, command+(['overflow'] if overflow else []), cwd, status, native)
            if actual != expected:
                raise ValueError(label+'-'+name+': optimizer encoding or rollback differs')
            report['cases'].append({'name': label+'-'+name, 'input_sha256': hashlib.sha256(body).hexdigest(),
                                     'expected_status': status, 'native': native, 'status': 'passed'})
        save()

    try:
        save()
        for index, compiler in enumerate(compilers):
            base = output/f'compiler-{index}'
            library = base/'library'
            for name, digest in sources.items():
                destination = library/name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(LIBRARY/name, destination)
                if sha(destination) != digest:
                    raise ValueError('library snapshot changed')
            binaries = []
            for kind, text in [('codec', programs.codec()), ('constructors', programs.constructors())]:
                label = f'compiler-{index}-{kind}'
                project = base/'projects'/kind
                (project/'src').mkdir(parents=True)
                (project/'src/main.ox').write_text(text)
                (project/'oxid.toml').write_text('[project]\nname="optimizer-check"\nedition="typed-preview"\n'
                                                 '[build]\nentry-mode="process"\n[execution]\npolicy="hosted"\n'
                                                 '[dependencies]\ntensor="../../library"\n')
                run(label+'-check', [compiler, 'check'], project)
                if kind == 'codec':
                    exercise(label+'-reference', [compiler, 'run', '--'], project, False)
                else:
                    body = run(label+'-reference', [compiler, 'run'], project)
                    if body != oracle.encode_state(oracle.initial_state(oracle.config(), 3)):
                        raise ValueError('invalid setters changed the optimizer configuration')
                for profile in ['dev', 'release']:
                    run(label+'-'+profile+'-build', [compiler, 'build', '--profile', profile, '--reproducible'], project)
                    deployed = output/'deployed'/label/profile/'optimizer-check'
                    deployed.parent.mkdir(parents=True)
                    shutil.copy2(project/'.oxid/bin/optimizer-check', deployed)
                    content = deployed.read_bytes()
                    if content[:6] != b'\x7fELF\x02\x01' or int.from_bytes(content[18:20], 'little') != host.elf_machine:
                        raise ValueError('optimizer check has the wrong ELF architecture')
                    binaries.append((label, kind, profile, deployed))
            (base/'projects').rename(base/'retained-projects')
            library.rename(base/'retained-library')
            if (base/'projects').exists() or library.exists():
                raise ValueError('optimizer check sources remain at their build paths')
            for label, kind, profile, executable in binaries:
                digest = sha(executable)
                cwd = output/'runs'/label/profile
                cwd.mkdir(parents=True)
                if kind == 'codec':
                    exercise(label+'-'+profile, [executable], cwd, True)
                else:
                    body = run(label+'-'+profile+'-source-free', [executable], cwd, native=True)
                    if body != oracle.encode_state(oracle.initial_state(oracle.config(), 3)):
                        raise ValueError('native configuration or OOM rollback changed')
                if sha(executable) != digest or list(executable.parent.iterdir()) != [executable]:
                    raise ValueError('optimizer check executable changed or has adjacent inputs')
                report['deployments'].append({'path': str(executable), 'sha256': digest, 'profile': profile,
                                               'source_removed': True, 'elf_machine': host.elf_machine, 'kind': kind})
            print('passed compiler-'+str(index), flush=True)
        if sources != {p.relative_to(LIBRARY).as_posix(): sha(p) for p in LIBRARY.rglob('*')
                       if p.is_file() and p.suffix in {'.ox', '.toml', '.md'} and '.oxid' not in p.parts}:
            raise ValueError('optimizer library changed during qualification')
        if controllers != {name: sha(ROOT/'scripts'/name) for name in controllers}:
            raise ValueError('optimizer controller or oracle changed')
        if pins != [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]:
            raise ValueError('compiler changed during qualification')
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
