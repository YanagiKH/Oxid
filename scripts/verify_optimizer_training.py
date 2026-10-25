#!/usr/bin/env python3
"""Verify native optimizer training, exact resume, malformed-session recovery and failure preservation."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import resource
import shutil
import struct
import subprocess
import time
import tomllib
import tarfile

import linux_native_target as target
import optimizer_oracles as oracle

ROOT = Path(__file__).resolve().parents[1]
APPLICATION = ROOT / 'apps/cpu-training'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))


def newest(directory, name):
    choices = []
    for suffix in ['a', 'b']:
        path = directory/(name+'.'+suffix)
        body = path.read_bytes()
        try:
            decoded = oracle.parse_session(body)
        except ValueError:
            continue
        choices.append((decoded['model']['updates'], body, decoded, path))
    if not choices:
        raise ValueError('no valid training checkpoint')
    return max(choices, key=lambda row: row[0])


def corruptions(body, adaptive):
    result = [('empty', b''), ('truncated', body[:-1])]
    changed = bytearray(body); changed[-1] ^= 1
    result.append(('checksum', bytes(changed)))
    if adaptive:
        ml, sl = struct.unpack_from('<QQ', body, 8)
        model, state = body[24:24+ml], body[24+ml:-4]
        for name, offset, fmt, value in [('nan-beta', 40, '<d', math.nan),
                                         ('negative-variance', 128+33*8, '<d', -1.)]:
            changed = bytearray(state[:-4]); struct.pack_into(fmt, changed, offset, value)
            result.append((name, oracle.crc(b'OXTR0001'+struct.pack('<QQ', ml, sl)+model+oracle.crc(changed))))
        changed = bytearray(state[:-4])
        struct.pack_into('<II', changed, 12, 3, 11)
        result.append(('optimizer-shape', oracle.crc(b'OXTR0001'+struct.pack('<QQ', ml, sl)+model+oracle.crc(changed))))
        changed = bytearray(body[:-4]); struct.pack_into('<Q', changed, 16, (1 << 64)-1)
        result.append(('length-overflow', oracle.crc(changed)))
    for name, data in result:
        try:
            oracle.parse_session(data)
        except ValueError:
            continue
        raise ValueError('invalid-session control is valid: ' + name)
    return result


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
    sources = {p.relative_to(APPLICATION).as_posix(): sha(p) for p in APPLICATION.rglob('*')
               if p.is_file() and p.suffix in {'.ox', '.toml', '.md'} and '.oxid' not in p.parts}
    controllers = {name: sha(ROOT/'scripts'/name) for name in
                   ['verify_optimizer_training.py', 'optimizer_oracles.py', 'cpu_training_oracles.py']}
    pins = [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]
    report = {'status': 'running', 'controllers': controllers, 'sources': sources, 'compilers': pins,
              'target': host.triple, 'commands': [], 'training': [], 'deployments': [],
              'scope': 'Whole Oxid training application with SGD, Adam and AdamW; exact resume/recovery and native resource bounds.'}

    def save():
        (output/'result.json').write_text(json.dumps(report, indent=2, allow_nan=False)+'\n')

    def run(name, argv, cwd, status=0, native=False):
        folder = output/'commands'/name
        folder.mkdir(parents=True)
        environment = {} if native else dict(os.environ, OXID_LLVM_BIN=str(llvm), OXID_CACHE_DIR=str(output/'legacy-cache'))
        environment.pop('OXID_PATH', None)
        started = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=cwd, env=environment,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=300,
                                preexec_fn=native_limits if native else None)
        for stream in ['stdout', 'stderr']:
            (folder/stream).write_bytes(getattr(result, stream))
        report['commands'].append({'name': name, 'argv': [str(x) for x in argv], 'cwd': str(cwd),
                                   'exit_status': result.returncode, 'expected_status': status,
                                   'stdout_sha256': sha(folder/'stdout'), 'stderr_sha256': sha(folder/'stderr'),
                                   'seconds': time.monotonic()-started,
                                   'address_space_limit': 33554432 if native else None,
                                   'open_file_limit': 32 if native else None})
        save()
        if result.returncode != status or result.stderr:
            raise ValueError(name+': training command failed')
        return result

    def train(label, binary, directory, kind):
        directory.mkdir(parents=True)
        command = [binary]
        run(label+'-continuous', command+['train', 'continuous', '50', kind], directory, native=True)
        continuous = newest(directory, 'continuous')
        if continuous[0] != 50:
            raise ValueError('continuous training update count changed')
        wanted, state = oracle.trained(kind, 50, decay=.01 if kind == 'adamw' else None)
        observed = continuous[2]['model']['parameters']
        if any(not math.isclose(a, b, rel_tol=2e-12, abs_tol=2e-14) for a, b in zip(wanted, observed)):
            raise ValueError('trained model differs from independent analytic gradients')
        stored = continuous[2]['optimizer']
        if stored['steps'] != 50 or stored['kind'] != state['kind']:
            raise ValueError('training reset the optimizer state')
        for key in ['first', 'second']:
            if len(stored[key]) != len(state[key]) or any(not math.isclose(a, b, rel_tol=2e-12, abs_tol=2e-14)
                                                        for a, b in zip(stored[key], state[key])):
                raise ValueError('trained optimizer moments differ from independent equations')
        run(label+'-first', command+['train', 'resumed', '20', kind], directory, native=True)
        old = newest(directory, 'resumed')[1]
        run(label+'-resume', command+['resume', 'resumed', '30'], directory, native=True)
        if newest(directory, 'resumed')[1] != continuous[1]:
            raise ValueError('resumed training differs bitwise from continuous training')
        before = {p.name: sha(p) for p in directory.glob('resumed.*')}
        run(label+'-refuse-reset', command+['resume', 'resumed', '1', 'sgd'], directory, status=2, native=True)
        run(label+'-refuse-overwrite', command+['train', 'resumed', '1', kind], directory, status=2, native=True)
        if before != {p.name: sha(p) for p in directory.glob('resumed.*')}:
            raise ValueError('invalid invocation changed checkpoints')
        for name, damaged in corruptions(continuous[1], kind != 'sgd'):
            (directory/'recover.a').write_bytes(old); (directory/'recover.b').write_bytes(damaged)
            run(label+'-recover-'+name, command+['resume', 'recover', '30'], directory, native=True)
            if newest(directory, 'recover')[1] != continuous[1]:
                raise ValueError('invalid-session recovery changed the optimizer trajectory')
        first, second = directory/'fatal.a', directory/'fatal.b'
        first.mkdir(); second.write_bytes(old)
        run(label+'-fatal-io', command+['resume', 'fatal', '1'], directory, status=1, native=True)
        if second.read_bytes() != old:
            raise ValueError('fatal I/O failure changed the recovery checkpoint')
        first.rmdir(); first.write_bytes(old); second.write_bytes(b'\0'*(17*1024*1024))
        run(label+'-fatal-memory', command+['resume', 'fatal', '1'], directory, status=1, native=True)
        if first.read_bytes() != old:
            raise ValueError('memory failure changed the recovery checkpoint')
        second.unlink()
        run(label+'-convergence', command+['train', 'converged', '2000', kind], directory, native=True)
        converged = newest(directory, 'converged')
        loss = oracle.model_oracle.forward_loss(converged[2]['model']['parameters'])
        if converged[0] != 2000 or not math.isfinite(loss) or loss >= .001:
            raise ValueError('independent model evaluation did not converge')
        run(label+'-stress', command+['train', 'stress', '20000', kind], directory, native=True)
        if newest(directory, 'stress')[0] != 20000:
            raise ValueError('training exceeded native resource bounds')
        report['training'].append({'name': label, 'optimizer': kind, 'resume_bit_equal': True,
                                    'recovery_cases': len(corruptions(continuous[1], kind != 'sgd')),
                                    'fatal_io_preserved': True, 'memory_failure_preserved': True,
                                    'updates': 2000, 'independent_loss': loss, 'stress_updates': 20000,
                                    'maximum_parameter_error': max(abs(a-b) for a, b in zip(wanted, observed))})
        save()
        print('passed '+label, flush=True)

    try:
        save()
        for index, compiler in enumerate(compilers):
            project = output/'projects'/f'compiler-{index}'
            for name, digest in sources.items():
                destination = project/name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(APPLICATION/name, destination)
                if sha(destination) != digest:
                    raise ValueError('application snapshot changed')
            for operation in ['check', 'test']:
                completed = run(f'compiler-{index}-workspace-'+operation,
                                [compiler, operation, '--workspace', '--message-format=json'], project)
                rows = [json.loads(line) for line in completed.stdout.splitlines()]
                summaries = [row for row in rows if row.get('kind') == 'workspace-summary']
                workspace = tomllib.loads((project/'oxid.toml').read_text())['workspace']
                expected_total = len(workspace['members']) if operation == 'check' else sum(
                    len(tomllib.loads((project/member/'oxid.toml').read_text()).get('tests', {}))
                    for member in workspace['members'])
                if operation == 'test':
                    results = [row for row in rows if row.get('kind') == 'test-result']
                    if len(results) != expected_total or any(row.get('success') is not True or row.get('exit_status') != 0 for row in results):
                        raise ValueError('workspace did not execute every declared test')
                if (len(summaries) != 1 or summaries[0].get('operation') != operation
                        or summaries[0].get('success') is not True or summaries[0].get('total') != expected_total
                        or summaries[0].get('failed') != 0):
                    raise ValueError('workspace summary did not confirm all members')
            run(f'compiler-{index}-lock', [compiler, 'lock', '--offline', '--message-format=json'], project)
            package = output/f'compiler-{index}.oxpkg.tar'
            run(f'compiler-{index}-package', [compiler, 'package', '--workspace', '--frozen',
                '--output', package, '--message-format=json'], project)
            with tarfile.open(package, 'r:') as archive:
                members = archive.getmembers()
                if not all(m.isfile() and m.type == tarfile.REGTYPE and m.uid == m.gid == m.mtime == 0
                           and m.mode == 0o644 and not m.pax_headers for m in members):
                    raise ValueError('source package contains unexpected metadata')
                inventory = tomllib.loads(archive.extractfile('OXID-PACKAGE.toml').read().decode())
                payload = {m.name: archive.extractfile(m).read() for m in members if m.name != 'OXID-PACKAGE.toml'}
                expected = [{'path': name, 'size': len(body), 'sha256': hashlib.sha256(body).hexdigest()}
                            for name, body in sorted(payload.items(), key=lambda pair: pair[0].encode())]
                if inventory['files'] != expected or [m.name for m in members] != ['OXID-PACKAGE.toml', *[row['path'] for row in expected]]:
                    raise ValueError('source package roster or content hash differs')
                for name, digest in sources.items():
                    if name.endswith('.ox') and hashlib.sha256(payload['source/'+name]).hexdigest() != digest:
                        raise ValueError('source package changed an Oxid input')
            run(f'compiler-{index}-package-verify', [compiler, 'package', '--verify', package,
                '--sha256', sha(package), '--message-format=json'], project)
            extracted = output/f'extracted-{index}'
            run(f'compiler-{index}-package-extract', [compiler, 'package', '--extract', package,
                '--output', extracted, '--sha256', sha(package), '--message-format=json'], project)
            project.rename(project.with_name(f'compiler-{index}-retained-source'))
            project = extracted/'source'
            run(f'compiler-{index}-extracted-check', [compiler, 'check', '--workspace', '--frozen', '--message-format=json'], project)
            report.setdefault('source_packages', []).append({'path': str(package), 'sha256': sha(package),
                    'payload_files': len(payload), 'extracted_and_built': True})
            if index and package.read_bytes() != (output/'compiler-0.oxpkg.tar').read_bytes():
                raise ValueError('compiler profiles produced different source packages')
            binaries = []
            for profile in ['dev', 'release']:
                completed = run(f'compiler-{index}-{profile}-build',
                                [compiler, 'build', '--package', 'cpu-training', '--profile', profile,
                                 '--reproducible', '--message-format=json'], project)
                summaries = [row for row in (json.loads(line) for line in completed.stdout.splitlines())
                             if row.get('kind') == 'workspace-summary']
                if len(summaries) != 1 or summaries[0] != {'schema_version': 1, 'edition': 'typed-preview',
                        'kind': 'workspace-summary', 'operation': 'build', 'success': True, 'total': 1, 'failed': 0}:
                    raise ValueError('selected workspace binary was not built successfully')
                binary = output/'deployed'/f'compiler-{index}'/profile/'cpu-training'
                binary.parent.mkdir(parents=True)
                shutil.copy2(project/'trainer/.oxid/bin/cpu-training', binary)
                content = binary.read_bytes()
                if content[:6] != b'\x7fELF\x02\x01' or int.from_bytes(content[18:20], 'little') != host.elf_machine:
                    raise ValueError('training executable has the wrong architecture')
                binaries.append((profile, binary))
            extracted.rename(output/f'retained-extracted-{index}')
            if extracted.exists() or project.exists():
                raise ValueError('training source remains at its build path')
            for profile, binary in binaries:
                digest = sha(binary)
                for kind in ['sgd', 'adam', 'adamw']:
                    label = f'compiler-{index}-{profile}-{kind}'
                    train(label, binary, output/'runs'/label, kind)
                if sha(binary) != digest or list(binary.parent.iterdir()) != [binary]:
                    raise ValueError('training executable changed or has adjacent inputs')
                report['deployments'].append({'path': str(binary), 'sha256': digest, 'profile': profile,
                                               'source_removed': True, 'elf_machine': host.elf_machine})
        if sources != {p.relative_to(APPLICATION).as_posix(): sha(p) for p in APPLICATION.rglob('*')
                       if p.is_file() and p.suffix in {'.ox', '.toml', '.md'} and '.oxid' not in p.parts}:
            raise ValueError('training application changed during qualification')
        if controllers != {name: sha(ROOT/'scripts'/name) for name in controllers}:
            raise ValueError('training controller or oracle changed')
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
