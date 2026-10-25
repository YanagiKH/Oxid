#!/usr/bin/env python3
"""Verify user-data training, complete batch continuation and source-package deployment."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import tarfile
import tomllib

import data_pipeline_oracles as oracle
import optimizer_oracles as optimizer
from data_pipeline_verification import Verification

INPUTS = [[0., 0.], [0., 1.], [1., 0.], [1., 1.]]
TARGETS = [[0.], [1.], [1.], [0.]]


def newest(directory, prefix):
    choices = []
    for suffix in ['a', 'b']:
        path = directory/(prefix+'.'+suffix)
        if not path.is_file():
            continue
        body = path.read_bytes()
        try:
            decoded = oracle.parse_session(body)
        except ValueError:
            continue
        choices.append((decoded['sampler']['batches'], body, decoded, path))
    if not choices:
        raise ValueError('no valid pipeline session')
    return max(choices, key=lambda row: row[0])


def independent(kind, updates):
    parameters, _ = optimizer.model_oracle.initial_parameters()
    setting = optimizer.config(kind)
    if kind == 'adamw':
        setting['decay'] = .01
    state = optimizer.initial_state(setting, 33)
    sampling = oracle.sampler(4, 3, True, 1)
    for _ in range(updates):
        selected = oracle.indices(sampling)
        gradient = oracle.loss_gradient(parameters, [INPUTS[i] for i in selected], [TARGETS[i] for i in selected])[1]
        parameters, state = optimizer.update(parameters, gradient, state)
        sampling = oracle.advance(sampling)
    return parameters, state, sampling


def corruptions(body):
    result = [('empty', b''), ('truncated', body[:-1]), ('checksum', body[:-1]+bytes([body[-1] ^ 1]))]
    first, second, third = struct.unpack_from('<QQQ', body, 8)
    training, source, sampling = body[32:32+first], body[32+first:32+first+second], body[-4-third:-4]
    for label, component, offset, form, value in [
            ('dataset-reserved', source, 20, '<I', 1), ('dataset-nan', source, 32, '<d', float('nan')),
            ('sampler-duplicate', sampling, 56, '<I', struct.unpack_from('<I', sampling, 60)[0]),
            ('sampler-progress', sampling, 24, '<I', 1)]:
        changed = bytearray(component[:-4])
        struct.pack_into(form, changed, offset, value)
        payload = training+(oracle.crc(changed) if label.startswith('dataset') else source)
        payload += sampling if label.startswith('dataset') else oracle.crc(changed)
        result.append((label, oracle.crc(b'OXDP0001'+struct.pack('<QQQ', first, second, third)+payload)))
    for label, value in result:
        try:
            oracle.parse_session(value)
        except ValueError:
            continue
        raise ValueError('corruption is valid: '+label)
    return result


def train(check, label, binary, directory, kind, reference):
    directory.mkdir(parents=True)
    dataset = oracle.dataset(INPUTS, TARGETS)
    (directory/'input.oxdata').write_bytes(dataset)

    def run(name, arguments, status=0):
        return check.command(label+'-'+name, [binary, *arguments], directory, status, native=True)

    run('start', ['train', 'input.oxdata', 'interrupted', '20', '3', kind, '1'])
    (directory/'input.oxdata').rename(directory/'retained-input.oxdata')
    run('resume', ['resume', 'interrupted', '30'])
    (directory/'retained-input.oxdata').rename(directory/'input.oxdata')
    run('continuous', ['train', 'input.oxdata', 'continuous', '50', '3', kind, '1'])
    completed = newest(directory, 'interrupted')
    if completed[:2] != newest(directory, 'continuous')[:2] or completed[0] != 50:
        raise ValueError('interrupted training is not bit identical')
    parameters, state, sampling = independent(kind, 50)
    actual = completed[2]
    error = max(abs(a-b) for a, b in zip(parameters, actual['training']['model']['parameters']))
    if error > 1e-12 or actual['sampler'] != sampling or actual['dataset'] != oracle.parse_dataset(dataset):
        raise ValueError('minibatch result differs from independent equations')
    for key in ['first', 'second']:
        if any(abs(a-b) > 1e-12 for a, b in zip(state[key], actual['training']['optimizer'][key])):
            raise ValueError('optimizer history differs')
    if actual['training']['model']['updates'] != 50 or actual['training']['optimizer']['steps'] != 50:
        raise ValueError('training counters differ')
    run('reference-two', ['train', 'input.oxdata', 'two', '2', '3', kind, '1'])
    if newest(directory, 'two')[1] != reference:
        raise ValueError('native and reference sessions differ')
    saved = {path.name: path.read_bytes() for path in directory.glob('interrupted.*')}
    for name, changed in corruptions(completed[1]):
        for filename, body in saved.items():
            (directory/filename).write_bytes(body)
        completed[3].write_bytes(changed)
        run('recover-'+name, ['resume', 'interrupted', '5'])
        if newest(directory, 'interrupted')[:2] != completed[:2]:
            raise ValueError('recovery changed the next batch or optimizer history')
    for filename, body in saved.items():
        (directory/filename).write_bytes(body)
    fatal = completed[3]
    fatal.unlink()
    fatal.mkdir()
    run('fatal-io', ['resume', 'interrupted', '5'], 1)
    if not fatal.is_dir() or any((directory/name).read_bytes() != body for name, body in saved.items() if name != fatal.name):
        raise ValueError('fatal I/O changed a checkpoint')
    fatal.rmdir()
    fatal.write_bytes(b'\0'*(17*1024*1024))
    digest = hashlib.sha256(fatal.read_bytes()).digest()
    run('fatal-memory', ['resume', 'interrupted', '5'], 1)
    if hashlib.sha256(fatal.read_bytes()).digest() != digest or any((directory/name).read_bytes() != body for name, body in saved.items() if name != fatal.name):
        raise ValueError('memory failure changed a checkpoint')
    fatal.write_bytes(saved[fatal.name])
    for index, arguments in enumerate([
            ['train', 'input.oxdata', 'interrupted', '1', '3', kind, '1'],
            ['resume', 'interrupted', '0'], ['resume', 'interrupted', '1000001'],
            ['resume', 'interrupted', '1', 'adam'],
            ['train', 'input.oxdata', 'invalid', '1', '0'],
            ['train', 'input.oxdata', 'invalid', '1', '2147483648'],
            ['train', 'input.oxdata', 'invalid', '1', '3', 'invalid'],
            ['train', 'input.oxdata', 'invalid', '1', '3', kind, '4294967296']]):
        run('invalid-'+str(index), arguments, 2)
        if any((directory/name).read_bytes() != body for name, body in saved.items()) or list(directory.glob('invalid.*')):
            raise ValueError('invalid command changed checkpoints')
    run('converge', ['train', 'input.oxdata', 'converged', '2000', '3', kind, '1'])
    final = newest(directory, 'converged')
    loss = oracle.loss_gradient(final[2]['training']['model']['parameters'], INPUTS, TARGETS)[0]
    if final[0] != 2000 or loss >= .001:
        raise ValueError('minibatch model did not converge')
    run('stress', ['train', 'input.oxdata', 'stress', '20000', '3', kind, '1'])
    if newest(directory, 'stress')[0] != 20000:
        raise ValueError('stress counter differs')
    check.report['cases'].append({'name': label, 'optimizer': kind, 'resume_bit_equal': True,
                                  'reference_bit_equal': True, 'recovery_cases': len(corruptions(completed[1])),
                                  'fatal_io_preserved': True, 'memory_failure_preserved': True,
                                  'maximum_parameter_error': error, 'updates': 2000,
                                  'independent_loss': loss, 'stress_updates': 20000})
    check.save()
    print('passed '+label, flush=True)


def verify(args):
    check = Verification(args, ['verify_minibatch_training.py', 'data_pipeline_verification.py',
                               'data_pipeline_oracles.py', 'optimizer_oracles.py', 'cpu_training_oracles.py'],
                         ['apps/cpu-training', 'apps/minibatch-training'],
                         'Public source-packaged minibatch CLI with user data, exact resume/recovery and native resource bounds.')
    try:
        for index, compiler in enumerate(check.compilers):
            snapshot = check.output/f'compiler-{index}'
            check.snapshot(snapshot)
            project = snapshot/'apps/minibatch-training'
            check.command(f'compiler-{index}-check', [compiler, 'check', '--message-format=json'], project)
            tested = check.command(f'compiler-{index}-test', [compiler, 'test', '--message-format=json'], project)
            results = [json.loads(line) for line in tested.stdout.splitlines()]
            expected_tests = tomllib.loads((project/'oxid.toml').read_text())['tests']
            actual = [row for row in results if row.get('kind') == 'test-result']
            if len(actual) != len(expected_tests) or any(row.get('name') not in expected_tests or row.get('success') is not True or row.get('exit_status') != 0 for row in actual):
                raise ValueError('declared tests did not execute')
            (project/'input.oxdata').write_bytes(oracle.dataset(INPUTS, TARGETS))
            references = {}
            for kind in ['sgd', 'adam', 'adamw']:
                check.command(f'compiler-{index}-reference-'+kind,
                              [compiler, 'run', '--', 'train', 'input.oxdata', 'reference-'+kind, '2', '3', kind, '1'], project)
                references[kind] = newest(project, 'reference-'+kind)[1]
            check.command(f'compiler-{index}-lock', [compiler, 'lock', '--offline', '--message-format=json'], project)
            package = check.output/f'compiler-{index}.oxpkg.tar'
            check.command(f'compiler-{index}-package', [compiler, 'package', '--frozen', '--output', package, '--message-format=json'], project)
            check.command(f'compiler-{index}-verify-package', [compiler, 'package', '--verify', package,
                                                             '--sha256', hashlib.sha256(package.read_bytes()).hexdigest(), '--message-format=json'], project)
            with tarfile.open(package) as tar:
                metadata = tomllib.loads(tar.extractfile('OXID-PACKAGE.toml').read().decode())
                payload = {member.name: tar.extractfile(member).read() for member in tar.getmembers() if member.name != 'OXID-PACKAGE.toml'}
                wanted = [{'path': name, 'size': len(body), 'sha256': hashlib.sha256(body).hexdigest()}
                          for name, body in sorted(payload.items(), key=lambda row: row[0].encode())]
                if metadata['files'] != wanted:
                    raise ValueError('package inventory mismatch')
            if index and package.read_bytes() != (check.output/'compiler-0.oxpkg.tar').read_bytes():
                raise ValueError('compiler profiles produced different source packages')
            extracted = check.output/f'extracted-{index}'
            check.command(f'compiler-{index}-extract', [compiler, 'package', '--extract', package, '--output', extracted,
                                                      '--sha256', hashlib.sha256(package.read_bytes()).hexdigest(), '--message-format=json'], project)
            project = extracted/metadata['root']
            check.command(f'compiler-{index}-extracted-check', [compiler, 'check', '--frozen', '--message-format=json'], project)
            binaries = []
            for profile in ['dev', 'release']:
                check.command(f'compiler-{index}-{profile}-build', [compiler, 'build', '--profile', profile,
                                                                  '--reproducible', '--frozen', '--message-format=json'], project)
                binary = check.deploy(project/'.oxid/bin/minibatch-training', f'compiler-{index}', profile)
                binaries.append((profile, binary))
            snapshot.rename(check.output/f'retained-compiler-{index}')
            extracted.rename(check.output/f'retained-extracted-{index}')
            if snapshot.exists() or extracted.exists():
                raise ValueError('source paths remain available')
            for profile, binary in binaries:
                for kind in ['sgd', 'adam', 'adamw']:
                    label = f'compiler-{index}-{profile}-'+kind
                    train(check, label, binary, check.output/'runs'/label, kind, references[kind])
                check.deployed(binary, f'compiler-{index}', profile)
            check.report['source_packages'].append({'path': str(package), 'sha256': hashlib.sha256(package.read_bytes()).hexdigest(),
                                                    'payload_files': len(payload), 'extracted_and_built': True})
            check.save()
        check.finish()
    except BaseException as error:
        check.failed(error)
        raise
    print('Minibatch training verification passed', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compilers', nargs='+', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--llvm-bin', required=True, type=Path)
    verify(parser.parse_args())
