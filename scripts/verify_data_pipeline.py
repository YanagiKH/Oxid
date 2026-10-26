#!/usr/bin/env python3
"""Verify paired batches, exact epochs, sampler codecs and rejected-operation rollback."""
import argparse
from pathlib import Path
import struct

import data_pipeline_oracles as oracle
import data_pipeline_programs as programs
import data_pipeline_failure_programs as failures
from data_pipeline_verification import ROOT, Verification


def expected_sampling(case):
    rows = case['rows']
    source = oracle.dataset([[float(row), -float(row)] for row in range(rows)],
                            [[3.*row+.25, -.5*row] for row in range(rows)])
    state = oracle.sampler(rows, case['size'], case['shuffle'], case['seed'])
    result = source+oracle.encode_sampler(state)
    for _ in range(case['batches']):
        selected = oracle.indices(state)
        values = [value for row in selected for value in (float(row), -float(row))]
        values += [value for row in selected for value in (3.*row+.25, -.5*row)]
        result += struct.pack(f'<{len(values)}d', *values)
        state = oracle.advance(state)
        result += oracle.encode_sampler(state)
    return result


def codec_controls(kind):
    if kind == 'dataset':
        body = oracle.dataset([[0., -0.], [1., -2.]], [[.25], [.5]])
        fields = [(8, '<I', 0), (12, '<I', 0xffffffff), (16, '<I', 0), (20, '<I', 1),
                  (24, '<Q', oracle.MAXIMUM), (32, '<d', float('nan')), (40, '<d', float('inf'))]
        parse = oracle.parse_dataset
    elif kind == 'sampler':
        body = oracle.encode_sampler(oracle.sampler(7, 3, True, 1))
        fields = [(8, '<I', 0), (8, '<I', 0xffffffff), (12, '<I', 0), (12, '<I', 0xffffffff),
                  (16, '<I', 2), (20, '<I', 0), (24, '<I', 1), (28, '<I', 1),
                  (32, '<Q', 1), (40, '<Q', 1), (48, '<Q', oracle.MAXIMUM),
                  (56, '<I', 7), (56, '<I', 3), (16, '<I', 0)]
        parse = oracle.parse_sampler
    elif kind == 'session':
        body = oracle.initial_session()
        fields = [(8, '<Q', oracle.MAXIMUM), (16, '<Q', oracle.MAXIMUM), (24, '<Q', oracle.MAXIMUM)]
        parse = oracle.parse_session
    else:
        raise ValueError(kind)
    result = [('roundtrip', body, 0, body)]
    if kind == 'sampler':
        state = oracle.sampler(7, 3, False, 0)
        state.update(batches=oracle.MAXIMUM, epoch=oracle.MAXIMUM//3, cursor=(oracle.MAXIMUM % 3)*3)
        extreme = oracle.encode_sampler(state)
        result.append(('maximum-counter', extreme, 0, extreme))
        for case in oracle.sampling_cases():
            state = oracle.sampler(case['rows'], case['size'], case['shuffle'], case['seed'])
            for _ in range(2):
                state = oracle.advance(state)
            encoded = oracle.encode_sampler(state)
            result.append((case['name'], encoded, 0, encoded))
    invalid = [(f'truncated-{cut}', body[:cut]) for cut in [0, 1, 8, 31, len(body)-1]]
    invalid += [('trailing', body+b'\0'), ('checksum', body[:-1]+bytes([body[-1] ^ 1]))]
    if kind == 'session':
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
            invalid.append((label, oracle.crc(b'OXDP0001'+struct.pack('<QQQ', first, second, third)+payload)))
        other = oracle.dataset([[0., 0., 0.]]*4, [[0.]]*4)
        invalid.append(('feature-mismatch', oracle.crc(b'OXDP0001'+struct.pack('<QQQ', first, len(other), third)+training+other+sampling)))
        other = oracle.encode_sampler(oracle.sampler(3, 2, True, 42))
        invalid.append(('sampler-row-mismatch', oracle.crc(b'OXDP0001'+struct.pack('<QQQ', first, second, len(other))+training+source+other)))
    for index, (offset, form, value) in enumerate(fields):
        changed = bytearray(body[:-4])
        struct.pack_into(form, changed, offset, value)
        invalid.append((f'field-{index}', oracle.crc(changed)))
    for name, encoded in invalid:
        try:
            parse(encoded)
        except ValueError:
            result.append((name, encoded, 15, b''))
        else:
            raise ValueError('negative control is valid: '+kind+'-'+name)
    return result


def verify(args):
    check = Verification(args, ['verify_data_pipeline.py', 'data_pipeline_verification.py',
                               'data_pipeline_oracles.py', 'data_pipeline_programs.py', 'data_pipeline_failure_programs.py',
                               'optimizer_programs.py', 'optimizer_checkpoint_programs.py',
                               'optimizer_oracles.py', 'cpu_training_oracles.py'],
                         ['apps/cpu-training/tensor'], 'Paired rows, deterministic shuffled batches, validated data/sampler formats and failure atomicity.')
    try:
        for compiler_index, compiler in enumerate(check.compilers):
            snapshot = check.output/f'compiler-{compiler_index}'
            check.snapshot(snapshot)
            pending = []

            def project(name, text):
                root = snapshot/'probes'/name
                (root/'src').mkdir(parents=True)
                (root/'src/main.ox').write_text(text)
                (root/'oxid.toml').write_text('[project]\nname="data-probe"\nedition="typed-preview"\n[build]\nentry-mode="process"\n[execution]\npolicy="hosted"\n[dependencies]\ntensor="../../apps/cpu-training/tensor"\n')
                return root

            def build(name, root):
                binaries = []
                for profile in ['dev', 'release']:
                    check.command(name+'-'+profile+'-build', [compiler, 'build', '--profile', profile,
                                                            '--reproducible', '--message-format=json'], root)
                    binary = check.deploy(root/'.oxid/bin/data-probe', name, profile)
                    binaries.append((profile, binary))
                return binaries

            for case in oracle.sampling_cases():
                name = f'compiler-{compiler_index}-'+case['name']
                root = project(case['name'], programs.sampling(case))
                check.command(name+'-check', [compiler, 'check', '--message-format=json'], root)
                expected = expected_sampling(case)
                actual = check.command(name+'-reference', [compiler, 'run'], root).stdout
                if actual != expected:
                    raise ValueError('sampling differs from independent bytes: '+name)
                pending.append((name, expected, build(name, root)))
                check.report['cases'].append({'name': name, 'batches': case['batches'], 'status': 'passed'})
                check.save()
                print('passed '+name+' reference', flush=True)
            codecs = []
            for kind in ['dataset', 'sampler', 'session']:
                name = f'compiler-{compiler_index}-'+kind
                root = project(kind, programs.codec(kind))
                check.command(name+'-check', [compiler, 'check', '--message-format=json'], root)
                controls = codec_controls(kind)
                for label, body, status, expected in controls:
                    (root/'input.bin').write_bytes(body)
                    actual = check.command(name+'-reference-'+label, [compiler, 'run'], root, status).stdout
                    if actual != expected:
                        raise ValueError('reference codec result differs: '+name+'-'+label)
                codecs.append((name, controls, build(name, root)))
            for label, text in [('failure-rollback', failures.failures()), ('counter-overflow', failures.maximum())]:
                name = f'compiler-{compiler_index}-'+label
                root = project(label, text)
                check.command(name+'-check', [compiler, 'check', '--message-format=json'], root)
                actual = check.command(name+'-reference', [compiler, 'run'], root).stdout
                if actual:
                    raise ValueError('failure probe emitted unexpected bytes')
                pending.append((name, b'', build(name, root)))
            snapshot.rename(check.output/f'retained-compiler-{compiler_index}')
            if snapshot.exists():
                raise ValueError('source snapshot remains')
            for name, expected, binaries in pending:
                for profile, binary in binaries:
                    actual = check.command(name+'-'+profile+'-source-free', [binary], binary.parent, native=True).stdout
                    if actual != expected:
                        raise ValueError('native sampling differs: '+name)
                    check.deployed(binary, name, profile)
            for name, controls, binaries in codecs:
                for profile, binary in binaries:
                    directory = check.output/'runs'/name/profile
                    directory.mkdir(parents=True)
                    for label, body, status, expected in controls:
                        (directory/'input.bin').write_bytes(body)
                        actual = check.command(name+'-'+profile+'-'+label, [binary], directory, status, native=True).stdout
                        if actual != expected:
                            raise ValueError('native codec differs: '+name+'-'+label)
                        check.report['cases'].append({'name': name+'-'+profile+'-'+label, 'status': 'passed'})
                    check.deployed(binary, name, profile)
            print('passed compiler '+str(compiler_index)+' data pipeline', flush=True)
        check.finish()
    except BaseException as error:
        check.failed(error)
        raise
    print('Data pipeline verification passed', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compilers', nargs='+', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--llvm-bin', required=True, type=Path)
    verify(parser.parse_args())
