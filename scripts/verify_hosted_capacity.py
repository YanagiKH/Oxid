#!/usr/bin/env python3
"""Build larger hosted programs and check source-free native results and size refusals."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import time

import linux_native_target as target

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def functions(count, owned=False, padding=0):
    source = 'struct Marker{value:i32}' if owned else ''
    for index in range(count):
        name = 'main' if index == 0 else f'f{index}'
        source += f'fn {name}()->i32{{' + '0;' * padding + 'return 7;}'
    return source


def inventory(callers, calls=100):
    source = ('struct Value{number:i32}fn read(p:&Value,q:&Value)->i32{return p.number;}'
              'fn main()->i32{f0();return 7;}')
    for index in range(callers):
        source += (f'fn f{index}()->i32{{let value=Value{{number:7}};'
                   + 'read(&value,&value);' * calls + 'return 0;}')
    return source


def blocks(owned=False):
    source = 'struct Marker{value:i32}' if owned else ''
    for index in range(136):
        name = 'main' if index == 0 else f'f{index}'
        branches = 16 if index == 135 else 40
        source += f'fn {name}()->i32{{' + 'if true{}else{}' * branches + 'return 7;}'
    return source


def inventory_maximum():
    parameters = ','.join(f'p{i}:&Value' for i in range(64))
    arguments = ','.join('&v' for _ in range(64))
    source = (f'struct Value{{number:i32}}fn read({parameters})->i32{{return p0.number;}}'
              'fn main()->i32{f0();return 7;}')
    for index, count in enumerate([25] * 20 + [5]):
        source += (f'fn f{index}()->i32{{let v=Value{{number:7}};'
                   + f'read({arguments});' * count + 'return 0;}')
    return source


def rejected(result, message):
    if result.returncode != 1 or result.stderr:
        raise ValueError('size refusal must use the structured diagnostic contract')
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    diagnostics = [row for row in rows if row.get('kind') == 'diagnostic']
    summaries = [row for row in rows if row.get('kind') == 'compile-summary']
    if (len(diagnostics) != 1 or diagnostics[0].get('code') != 'E0700'
            or diagnostics[0].get('stage') != 'native-admission'
            or message not in diagnostics[0].get('message', '')
            or len(summaries) != 1 or summaries[0].get('success') is not False):
        raise ValueError('unexpected compiler refusal: ' + repr(rows))


def verification_refused(result):
    if result.returncode != 1 or result.stderr:
        raise ValueError('source-work guard did not return a structured failure')
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    diagnostics = [row for row in rows if row.get('kind') == 'diagnostic']
    summaries = [row for row in rows if row.get('kind') == 'compile-summary']
    if (len(diagnostics) != 1 or diagnostics[0].get('code') != 'E0400'
            or diagnostics[0].get('stage') != 'oir-owned-lower'
            or diagnostics[0].get('notes') != ['resource: ownership work']
            or len(summaries) != 1 or summaries[0].get('success') is not False):
        raise ValueError('source-work verification guard changed: ' + repr(rows))


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))


def verify(args):
    if not __debug__:
        raise ValueError('qualification refuses optimized Python')
    host = target.required_host()
    output = args.output.resolve()
    if output.exists() or output == ROOT or ROOT in output.parents:
        raise ValueError('output must be a new directory outside the repository')
    compilers = [path.resolve(strict=True) for path in args.compilers]
    if len(set(compilers)) != len(compilers) or not all(p.is_file() and os.access(p, os.X_OK) for p in compilers):
        raise ValueError('distinct executable compilers are required')
    llvm = args.llvm_bin.resolve(strict=True)
    output.mkdir(parents=True)
    pins = [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]
    report = {'status': 'running', 'controller_sha256': sha(Path(__file__)),
              'compilers': pins, 'target': host.triple, 'commands': [], 'deployments': [],
              'scope': 'Hosted program-size transitions, bounded-policy refusals and source-free native execution.'}

    def save():
        (output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')

    def run(name, argv, cwd, native=False, expected=0):
        folder = output / 'commands' / name
        folder.mkdir(parents=True)
        environment = {} if native else dict(os.environ, OXID_LLVM_BIN=str(llvm),
                                             OXID_CACHE_DIR=str(output / 'legacy-cache'))
        environment.pop('OXID_PATH', None)
        start = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=cwd, env=environment,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=240,
                                preexec_fn=native_limits if native else None)
        for stream in ['stdout', 'stderr']:
            (folder / stream).write_bytes(getattr(result, stream))
        report['commands'].append({'name': name, 'argv': [str(x) for x in argv],
                                   'cwd': str(cwd), 'exit_status': result.returncode,
                                   'expected_status': expected, 'stdout_sha256': sha(folder / 'stdout'),
                                   'stderr_sha256': sha(folder / 'stderr'), 'seconds': time.monotonic() - start,
                                   'address_space_limit': 33554432 if native else None,
                                   'open_file_limit': 32 if native else None})
        save()
        if result.returncode != expected:
            raise ValueError(f'{name}: unexpected exit status {result.returncode}')
        return result

    def project(name, text, policy, process=False):
        directory = output / 'projects' / name
        (directory / 'src').mkdir(parents=True)
        (directory / 'src/main.ox').write_text(text)
        mode = 'process' if process else 'result'
        (directory / 'oxid.toml').write_text(f'[project]\nname="capacity"\nedition="typed-preview"\n'
                                           f'[build]\nentry-mode="{mode}"\n[execution]\npolicy="{policy}"\n')
        return directory

    try:
        save()
        for index, compiler in enumerate(compilers):
            cases = [('functions-257-scalar', functions(257), False, False, 'function count limit exceeded (256)'),
                     ('functions-257-record', functions(257, True), False, False, 'function count limit exceeded (256)'),
                     ('functions-1024-scalar', functions(1024), False, False, 'function count limit exceeded (256)'),
                     ('functions-1024-record-process', functions(1024, True), True, True, 'function count limit exceeded (256)'),
                     ('slots-32768-scalar', functions(128, padding=255), False, False, 'aggregate locals limit exceeded (8192)'),
                     ('slots-32768-record', functions(128, True, 255), False, False, 'aggregate scalar slots limit exceeded (8192)'),
                     ('blocks-16384-scalar', blocks(), False, False, 'aggregate blocks limit exceeded (4096)'),
                     ('blocks-16384-record', blocks(True), False, False, 'aggregate blocks limit exceeded (4096)'),
                     ('borrow-inventory', inventory(16), False, False, 'compiler inventory items limit exceeded (8192)')]
            for name, text, process, debug_info, refusal in cases:
                label = f'compiler-{index}-{name}'
                bounded = project(label + '-bounded', text, 'bounded', process)
                checked = run(label + '-bounded-check', [compiler, 'check'], bounded)
                if checked.stderr:
                    raise ValueError('bounded type checking emitted errors')
                result = run(label + '-bounded-refusal', [compiler, 'build', '--message-format=json'], bounded, expected=1)
                rejected(result, refusal)
                if (bounded / '.oxid/bin/capacity').exists():
                    raise ValueError('rejected bounded program published an executable')
                hosted = project(label + '-hosted', text, 'hosted', process)
                run(label + '-hosted-check', [compiler, 'check'], hosted)
                result = run(label + '-reference', [compiler, 'run'], hosted, expected=7 if process else 0)
                if result.stderr or result.stdout != (b'' if process else b'7\n'):
                    raise ValueError('reference result differs from the independent constant result')
                binaries = []
                for profile in ['dev', 'release']:
                    argv = [compiler, 'build', '--profile', profile, '--reproducible']
                    if debug_info:
                        argv.append('--debug-info')
                    result = run(label + '-' + profile + '-build', argv, hosted)
                    if result.stderr:
                        raise ValueError('native build emitted errors')
                    deployed = output / 'deployed' / label / profile / 'capacity'
                    deployed.parent.mkdir(parents=True)
                    shutil.copy2(hosted / '.oxid/bin/capacity', deployed)
                    body = deployed.read_bytes()
                    if body[:6] != b'\x7fELF\x02\x01' or int.from_bytes(body[18:20], 'little') != host.elf_machine:
                        raise ValueError('native build produced the wrong architecture')
                    binaries.append((profile, deployed))
                hosted.rename(hosted.with_name(label + '-retained-source'))
                if hosted.exists():
                    raise ValueError('source project remains available')
                for profile, deployed in binaries:
                    digest = sha(deployed)
                    result = run(label + '-' + profile + '-source-free', [deployed], deployed.parent,
                                 native=True, expected=7 if process else 0)
                    if result.stderr or result.stdout != (b'' if process else b'7\n') or sha(deployed) != digest:
                        raise ValueError('source-free native result differs')
                    if list(deployed.parent.iterdir()) != [deployed]:
                        raise ValueError('deployed program has colocated inputs')
                    report['deployments'].append({'path': str(deployed), 'sha256': digest, 'profile': profile,
                                                   'source_removed': True, 'case': name, 'elf_machine': host.elf_machine,
                                                   'process_entry': process, 'debug_info': debug_info})
                save()
                print('passed ' + label, flush=True)
            negative = [('functions-1025-scalar', functions(1025), 'function count limit exceeded (1024)'),
                        ('functions-1025-record', functions(1025, True), 'function count limit exceeded (1024)'),
                        ('slots-32769-scalar', functions(128, padding=255) + 'fn extra()->i32{return 0;}', 'aggregate locals limit exceeded (32768)'),
                        ('slots-32769-record', functions(128, True, 255) + 'fn extra()->i32{return 0;}', 'aggregate scalar slots limit exceeded (32768)'),
                        ('function-slots-257-scalar', functions(1, padding=256), 'locals per function limit exceeded (256)'),
                        ('function-slots-257-record', functions(1, True, 256), 'scalar slots per function limit exceeded (256)')]
            negative += [('blocks-16385-scalar', blocks() + 'fn extra()->i32{return 0;}', 'aggregate blocks limit exceeded (16384)'),
                         ('blocks-16385-record', blocks(True) + 'fn extra()->i32{return 0;}', 'aggregate blocks limit exceeded (16384)')]
            for name, text, refusal in negative:
                label = f'compiler-{index}-{name}'
                directory = project(label, text, 'hosted')
                run(label + '-check', [compiler, 'check'], directory)
                result = run(label + '-refusal', [compiler, 'build', '--message-format=json'], directory, expected=1)
                rejected(result, refusal)
                if (directory / '.oxid/bin/capacity').exists():
                    raise ValueError('size refusal published an executable')
            # This source reaches the earlier ownership-work budget. Exact native
            # I=65536/65537 is checked independently with verified raw programs;
            # larger native caps must never bypass source verification.
            label = f'compiler-{index}-source-verification-work'
            directory = project(label, inventory_maximum(), 'hosted')
            result = run(label + '-refusal', [compiler, 'build', '--message-format=json'], directory, expected=1)
            verification_refused(result)
            if (directory / '.oxid/bin/capacity').exists():
                raise ValueError('source verification refusal published an executable')
        if pins != [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]:
            raise ValueError('compiler changed during qualification')
        if report['controller_sha256'] != sha(Path(__file__)):
            raise ValueError('controller changed during qualification')
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
