#!/usr/bin/env python3
"""Verify native tensor gradients, training convergence and checkpoint recovery."""
import argparse
import hashlib
import json
import math
import os
import resource
from pathlib import Path
import shutil
import struct
import subprocess
import time
import zlib

import cpu_training_oracles as oracle
import linux_native_target as native_target

ROOT = Path(__file__).resolve().parents[1]
APPLICATION = ROOT / 'apps/cpu-training'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_NOFILE, (32,) * 2)


def main():
    if not __debug__:
        raise ValueError('qualification requires Python assertion checks')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compilers', nargs='+', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--llvm-bin', type=Path, required=True)
    args = parser.parse_args()
    host = native_target.required_host()
    output = args.output.resolve()
    if output == ROOT or ROOT in output.parents:
        raise ValueError('output must be outside the repository')
    output.mkdir(parents=True, exist_ok=False)
    sources = sorted(p for p in APPLICATION.rglob('*')
                     if p.is_file() and p.suffix in {'.ox', '.toml', '.md'}
                     and '.oxid' not in p.parts)
    pins = {str(p.relative_to(APPLICATION)): digest(p) for p in sources}
    compilers = [p.resolve(strict=True) for p in args.compilers]
    compiler_pins = [{'path': str(p), 'sha256': digest(p)} for p in compilers]
    record = {'status': 'running', 'controller_sha256': digest(__file__),
              'oracle_sha256': digest(oracle.__file__), 'sources': pins,
              'compilers': compiler_pins, 'target': host.triple,
              'commands': [], 'gradients': [], 'training': [], 'deployments': []}
    environment = dict(os.environ, OXID_LLVM_BIN=str(args.llvm_bin.resolve(strict=True)),
                       OXID_CACHE_DIR=str(output / 'cache'))
    environment.pop('OXID_PATH', None)

    def save():
        (output / 'result.json').write_text(json.dumps(record, indent=2, allow_nan=False) + '\n')

    def invoke(name, argv, cwd, expected=None, status=0, native=False, stderr=b''):
        directory = output / 'commands' / name
        directory.mkdir(parents=True)
        started = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=cwd,
                                env={} if native else environment,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=300,
                                preexec_fn=native_limits if native else None)
        (directory / 'stdout').write_bytes(result.stdout)
        (directory / 'stderr').write_bytes(result.stderr)
        record['commands'].append({'name': name, 'argv': [str(x) for x in argv],
                                   'cwd': str(cwd), 'exit_status': result.returncode,
                                   'seconds': time.monotonic() - started,
                                   'environment': {} if native else 'isolated compiler cache',
                                   'stdout_sha256': digest(directory / 'stdout'),
                                   'stderr_sha256': digest(directory / 'stderr')})
        save()
        if result.returncode != status or (stderr is not None and result.stderr != stderr) or (expected is not None and result.stdout != expected):
            raise ValueError('unexpected command result: ' + name)
        return result

    def checkpoint(path):
        body = path.read_bytes()
        return body, oracle.parse_checkpoint(body)

    def training(name, binary, directory):
        directory.mkdir(parents=True)
        invoke(name + '-help', [binary, '--help'], directory, native=True)
        invoke(name + '-initial', [binary, 'train', 'resume', '20'], directory, native=True)
        first = directory / 'resume.a'
        second = directory / 'resume.b'
        old = max((checkpoint(p) for p in (first, second)), key=lambda pair: pair[1]['updates'])
        if old[1]['updates'] != 20:
            raise ValueError('training did not save the requested update count')
        invoke(name + '-resume', [binary, 'resume', 'resume', '30'], directory, native=True)
        resumed = max((checkpoint(p) for p in (first, second)), key=lambda pair: pair[1]['updates'])
        invoke(name + '-continuous', [binary, 'train', 'continuous', '50'], directory, native=True)
        continuous = max((checkpoint(directory / ('continuous.' + suffix)) for suffix in ('a', 'b')),
                         key=lambda pair: pair[1]['updates'])
        if resumed[0] != continuous[0] or resumed[1]['updates'] != 50:
            raise ValueError('resumed training differs from continuous training')
        before = {p.name: p.read_bytes() for p in (first, second)}
        invoke(name + '-existing', [binary, 'train', 'resume', '1'], directory, status=2, native=True)
        if before != {p.name: p.read_bytes() for p in (first, second)}:
            raise ValueError('new training overwrote existing checkpoints')
        for count in ('0', '-1', '1000001', '99999999999999999999', '1x', ''):
            invoke(name + '-invalid-' + str(len(record['commands'])),
                   [binary, 'train', 'invalid', count], directory, status=2, native=True)
            if (directory / 'invalid.a').exists() or (directory / 'invalid.b').exists():
                raise ValueError('invalid arguments created checkpoint files')
        invoke(name + '-convergence', [binary, 'train', 'converged', '2000'], directory, native=True)
        converged = max((checkpoint(directory / ('converged.' + suffix)) for suffix in ('a', 'b')),
                        key=lambda pair: pair[1]['updates'])
        loss = oracle.forward_loss(converged[1]['parameters'])
        if converged[1]['updates'] != 2000 or not math.isfinite(loss) or loss >= 1e-12:
            raise ValueError('independently evaluated model did not converge')
        # Preserve a known older generation and corrupt the newer generation.
        mutations = [('truncated-' + str(cut), continuous[0][:cut])
                     for cut in (0, 1, 8, 31, 35, len(continuous[0]) - 1)]
        altered = bytearray(continuous[0]); altered[48] ^= 0x80
        mutations.append(('checksum', bytes(altered)))
        altered = bytearray(continuous[0]); struct.pack_into('<Q', altered, 32, 0x7ff8000000000000)
        struct.pack_into('<I', altered, len(altered) - 4, zlib.crc32(altered[:-4]))
        mutations.append(('non-finite', bytes(altered)))
        for label, offset, value in [('high-dimension', 8, 0xffffffff),
                                      ('overflow-shape', 12, 268435451), ('zero-rng', 20, 0)]:
            altered = bytearray(continuous[0]); struct.pack_into('<I', altered, offset, value)
            struct.pack_into('<I', altered, len(altered) - 4, zlib.crc32(altered[:-4]))
            mutations.append((label, bytes(altered)))
        for label, body in mutations:
            first.write_bytes(old[0]); second.write_bytes(body)
            invoke(name + '-recover-' + label, [binary, 'resume', 'resume', '30'], directory, native=True)
            recovered = max((checkpoint(p) for p in (first, second)), key=lambda pair: pair[1]['updates'])
            if recovered[0] != continuous[0]:
                raise ValueError('checkpoint fallback changed the training trajectory')
        for path in (first, second):
            path.write_bytes(b'broken')
        invoke(name + '-both-corrupt', [binary, 'resume', 'resume', '1'], directory, status=1, native=True)
        # An I/O error is fatal even when the other file is a valid model.
        first.unlink(); first.mkdir(); second.write_bytes(old[0])
        invoke(name + '-io-error', [binary, 'resume', 'resume', '1'], directory, status=1, native=True)
        if second.read_bytes() != old[0]:
            raise ValueError('fatal I/O failure modified the valid checkpoint')
        first.rmdir(); first.write_bytes(old[0])
        second.write_bytes(b'\0' * (17 * 1024 * 1024))
        invoke(name + '-read-memory-limit', [binary, 'resume', 'resume', '1'], directory, status=1, native=True)
        if first.read_bytes() != old[0]:
            raise ValueError('allocation failure modified the valid checkpoint')
        second.unlink()
        invoke(name + '-stress', [binary, 'train', 'stress', '20000'], directory, native=True)
        stress = max((checkpoint(directory / ('stress.' + suffix)) for suffix in ('a', 'b')),
                     key=lambda pair: pair[1]['updates'])
        if stress[1]['updates'] != 20000:
            raise ValueError('long training did not finish within native resource limits')
        record['training'].append({'name': name, 'resume_bit_equal': True,
                                   'updates': 2000, 'independent_loss': loss,
                                   'recovery_cases': len(mutations), 'fatal_io_preserved': True,
                                   'read_memory_failure_preserved': True, 'stress_updates': 20000,
                                   'address_space_limit': 32 * 1024 * 1024, 'open_file_limit': 32})
        save()

    try:
        save()
        for index, compiler in enumerate(compilers):
            label = 'compiler-' + str(index)
            project = output / 'projects' / label
            for source in sources:
                target = project / source.relative_to(APPLICATION)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)
            checked = invoke(label + '-check', [compiler, 'check', '--workspace'], project, stderr=None)
            if b'workspace check: 3 passed; 0 failed' not in checked.stderr:
                raise ValueError('workspace member check was incomplete')
            tested = invoke(label + '-tests', [compiler, 'test', '--workspace'], project, b'', stderr=None)
            if tested.stderr != (b'test trainer::optimizers: ok (exit status: 0)\n'
                                 b'test trainer::tensor: ok (exit status: 0)\n'
                                 b'workspace test: 2 passed; 0 failed\n'):
                raise ValueError('declared tensor and optimizer contract tests were not executed')
            invoke(label + '-lock', [compiler, 'lock'], project, stderr=b'workspace lock written\n')
            archive = output / (label + '.oxpkg')
            invoke(label + '-package', [compiler, 'package', '--frozen', '--output', archive], project, stderr=None)
            package_sha = digest(archive)
            invoke(label + '-verify-package', [compiler, 'package', '--verify', archive, '--sha256', package_sha], output, stderr=None)
            extracted = output / 'extracted' / label
            extracted.parent.mkdir(exist_ok=True)
            invoke(label + '-extract-package', [compiler, 'package', '--extract', archive, '--sha256', package_sha, '--output', extracted], output, stderr=None)
            invoke(label + '-extracted-check', [compiler, 'check', '--workspace', '--frozen'], extracted / 'source', stderr=None)
            invoke(label + '-extracted-tests', [compiler, 'test', '--workspace', '--frozen'], extracted / 'source', b'', stderr=None)
            reference = output / 'reference' / label
            reference.mkdir(parents=True)
            # Run from the project with an absolute checkpoint prefix.
            invoke(label + '-reference', [compiler, 'run', '--', 'train', reference / 'model', '1'], project)
            reference_state = max((checkpoint(reference / ('model.' + suffix)) for suffix in ('a', 'b')),
                                  key=lambda pair: pair[1]['updates'])[1]
            initial, rng = oracle.initial_parameters()
            expected = oracle.finite_difference_step(initial)
            error = max(abs(a-b) for a, b in zip(expected, reference_state['parameters']))
            if error >= 2e-8:
                raise ValueError('model update differs from independent numerical differentiation')
            record['gradients'].append({'name': label + '-model-update', 'maximum_error': error})
            binaries = []
            for profile in ('dev', 'release'):
                name = label + '-' + profile
                build = [compiler, 'build', '--package', 'cpu-training', '--profile', profile,
                         '--frozen', '--incremental', '--reproducible']
                invoke(name + '-cold', build, project, stderr=b'trainer: ok\nworkspace build: 1 passed; 0 failed\n')
                built = project / 'trainer/.oxid/bin/cpu-training'
                expected_hash = digest(built)
                warm = invoke(name + '-warm', build, project, stderr=b'trainer: ok\nworkspace build: 1 passed; 0 failed\n')
                if b'native object cache: hit' not in warm.stdout or digest(built) != expected_hash:
                    raise ValueError('warm application build changed executable bytes')
                binary = output / 'deployed' / name
                binary.parent.mkdir(exist_ok=True)
                shutil.copy2(built, binary)
                data = binary.read_bytes()
                if data[:6] != b'\x7fELF\x02\x01' or int.from_bytes(data[18:20], 'little') != host.elf_machine:
                    raise ValueError('application target does not match the native host')
                binaries.append((name, binary))
            for case in oracle.cases:
                body, expected = oracle.generate(case)
                query = output / 'oracles' / label / case['name']
                (query / 'src').mkdir(parents=True)
                (query / 'src/main.ox').write_text(body)
                (query / 'oxid.toml').write_text('[project]\nname="gradient-check"\nedition="typed-preview"\nentry="src/main.ox"\n[build]\nentry-mode="result"\n[execution]\npolicy="hosted"\n[dependencies]\ntensor=' + json.dumps(str(project / 'tensor')) + '\n')
                invoke(label + '-' + case['name'] + '-reference', [compiler, 'run'], query, b'0\n')
                for profile in ('dev', 'release'):
                    invoke(label + '-' + case['name'] + '-' + profile + '-build',
                           [compiler, 'build', '--profile', profile], query)
                    binary = output / 'deployed' / (label + '-' + case['name'] + '-' + profile)
                    shutil.copy2(query / '.oxid/bin/gradient-check', binary)
                    binaries.append((label + '-' + case['name'] + '-' + profile, binary))
                record['gradients'].append({'name': label + '-' + case['name'], 'oracle': expected})
            mutated, unused = oracle.generate(oracle.cases[0], True)
            for case_name, body, expected_output in [('mutated-gradient', mutated, b'101\n'),
                                                      ('atomic-update', oracle.atomic_update_program(), b'0\n')]:
                query = output / 'oracles' / label / case_name
                (query / 'src').mkdir(parents=True)
                (query / 'src/main.ox').write_text(body)
                (query / 'oxid.toml').write_text('[project]\nname="training-control"\nedition="typed-preview"\nentry="src/main.ox"\n[build]\nentry-mode="result"\n[execution]\npolicy="hosted"\n[dependencies]\ntensor=' + json.dumps(str(project / 'tensor')) + '\ncheckpoint=' + json.dumps(str(project / 'checkpoint')) + '\n')
                if case_name == 'atomic-update':
                    (query / 'before.bin').write_bytes(oracle.atomic_checkpoint())
                invoke(label + '-' + case_name + '-reference', [compiler, 'run'], query, expected_output)
                if case_name == 'atomic-update' and (query / 'after.bin').read_bytes() != oracle.atomic_checkpoint():
                    raise ValueError('failed model update changed parameters in the reference runtime')
                for profile in ('dev', 'release'):
                    invoke(label + '-' + case_name + '-' + profile + '-build', [compiler, 'build', '--profile', profile], query)
                    binary = output / 'deployed' / (label + '-' + case_name + '-' + profile)
                    shutil.copy2(query / '.oxid/bin/training-control', binary)
                    binaries.append((label + '-' + case_name + '-' + profile, binary))
            reporting, patterns = oracle.report_program()
            query = output / 'oracles' / label / 'report'
            (query / 'src').mkdir(parents=True)
            (query / 'src/main.ox').write_text(reporting)
            shutil.copy2(project / 'trainer/src/report.ox', query / 'src/report.ox')
            (query / 'oxid.toml').write_text('[project]\nname="report-check"\nedition="typed-preview"\nentry="src/main.ox"\n[build]\nentry-mode="process"\n[execution]\npolicy="hosted"\n')
            result = invoke(label + '-report-reference', [compiler, 'run'], query)
            oracle.verify_report_output(result.stdout, patterns)
            for profile in ('dev', 'release'):
                invoke(label + '-report-' + profile + '-build', [compiler, 'build', '--profile', profile], query)
                binary = output / 'deployed' / (label + '-report-' + profile)
                shutil.copy2(query / '.oxid/bin/report-check', binary)
                binaries.append((label + '-report-' + profile, binary))
            # Detach every build source before running native deployments.
            project.rename(project.with_name(label + '-retained'))
            (output / 'oracles' / label).rename(output / 'oracles' / (label + '-retained'))
            for name, binary in binaries:
                if name in {label + '-dev', label + '-release'}:
                    training(name, binary, output / 'runs' / name)
                elif '-report-' in name:
                    result = invoke(name + '-source-free', [binary], output, native=True)
                    oracle.verify_report_output(result.stdout, patterns)
                elif '-atomic-update-' in name:
                    directory = output / 'runs' / name
                    directory.mkdir(parents=True)
                    (directory / 'before.bin').write_bytes(oracle.atomic_checkpoint())
                    invoke(name + '-source-free', [binary], directory, b'0\n', native=True)
                    if (directory / 'after.bin').read_bytes() != oracle.atomic_checkpoint():
                        raise ValueError('failed native model update changed an earlier parameter tensor')
                else:
                    expected_output = b'101\n' if '-mutated-gradient-' in name else b'0\n'
                    invoke(name + '-source-free', [binary], output, expected_output, native=True)
                record['deployments'].append({'name': name, 'path': str(binary),
                                              'sha256': digest(binary), 'source_removed': True, 'elf_machine': host.elf_machine})
            print(label + ': passed', flush=True)
        if pins != {str(p.relative_to(APPLICATION)): digest(p) for p in sources}:
            raise ValueError('application sources changed during verification')
        for pin in compiler_pins:
            if digest(pin['path']) != pin['sha256']:
                raise ValueError('compiler changed during verification')
        record['status'] = 'passed'
    except BaseException as error:
        record.update(status='failed', failure=f'{type(error).__name__}: {error}')
        raise
    finally:
        save()
    print(json.dumps({'status': record['status'], 'output': str(output),
                      'training_runs': len(record['training']), 'gradients': len(record['gradients'])}))


if __name__ == '__main__':
    main()
