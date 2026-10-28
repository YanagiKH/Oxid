#!/usr/bin/env python3
"""Verify absolute dataset starts, selected-row SGD and source-free native use."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct

from data_pipeline_verification import ROOT, Verification, sha
import streaming_training_oracle as oracle


def require(value, message):
    if not value:
        raise ValueError(message)


def directory(check, label, payload=None):
    result = check.output/'runs'/label
    result.mkdir(parents=True)
    if payload is not None:
        (result/'input.oxds').write_bytes(payload)
        check.report.setdefault('inputs', []).append({
            'path': str(result/'input.oxds'), 'bytes': len(payload),
            'sha256': hashlib.sha256(payload).hexdigest()})
        check.save()
    return result


def sparse(check, target, rows, start, values):
    """A bounded, independently reconstructible sparse fixture; no 4 GiB hash pass."""
    require(0 < start < rows and len(values) == rows-start, 'invalid sparse oracle')
    header = b'OXDS0001'+struct.pack('<IIII', rows, 2, 1, 0)
    offset = 24 + start*24
    require(offset > 2**32, 'sparse case does not cross the 32-bit offset boundary')
    tail = b''.join(struct.pack('<ddd', *row) for row in values)
    # A NaN in the skipped prefix must not be parsed as a selected value.
    prefix = struct.pack('<Q', 0x7ff8000000000042)
    segments = [(0, header), (24, prefix), (offset, tail)]
    with target.open('xb') as output:
        for at, body in segments:
            output.seek(at)
            output.write(body)
        output.truncate(24+rows*24)
    actual = target.stat()
    require(actual.st_size == 24+rows*24 and actual.st_blocks*512 < 1024*1024,
            'sparse fixture was not retained with bounded physical storage')
    receipt = {'path': str(target), 'bytes': actual.st_size,
               'allocated_bytes': actual.st_blocks*512, 'zero_filled_holes': True,
               'start_row': start, 'absolute_offset': offset,
               'segments': [{'offset': at, 'hex': body.hex(),
                             'sha256': hashlib.sha256(body).hexdigest()} for at, body in segments]}
    receipt['definition_sha256'] = hashlib.sha256(
        json.dumps(receipt, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    check.report.setdefault('sparse_inputs', []).append(receipt)
    check.save()


def train(check, label, command, cwd, start, selected, native, explicit=True):
    arguments = ['input.oxds', '3', '2'] + ([str(start)] if explicit else [])
    completed = check.command(label, [*command, *arguments], cwd, native=native)
    losses = oracle.check_summary(completed.stdout, selected, 3, 2)
    check.report['cases'].append({'name': label, 'start_row': start,
                                  'selected_rows': len(selected), 'native': native,
                                  'independent_initial_loss': losses[0],
                                  'independent_final_loss': losses[1]})
    check.save()
    return completed.stdout


def failed_training(check, label, command, cwd, start, native):
    result = check.command(label, [*command, 'input.oxds', '3', '2', str(start)],
                           cwd, status=1, native=native)
    require(result.stdout.startswith(b'Training failed.') and b'Final MSE:' not in result.stdout,
            label+': selected invalid input reported success')


def app_cases(check, prefix, command, native):
    results = {}
    base = oracle.encode(oracle.SAMPLE)
    working = directory(check, prefix+'-sample', base)
    default = train(check, prefix+'-default', command, working, 0, oracle.SAMPLE, native, False)
    explicit = train(check, prefix+'-zero', command, working, 0, oracle.SAMPLE, native)
    require(default == explicit, 'explicit zero changed the default invocation')
    results['default'] = default
    for start in [1, 3, 7]:
        results[str(start)] = train(check, f'{prefix}-suffix-{start}', command, working,
                                   start, oracle.SAMPLE[start:], native)
    prefix_nan = bytearray(base)
    struct.pack_into('<Q', prefix_nan, 24, 0x7ff8000000000042)
    skipped = directory(check, prefix+'-skipped-nan', prefix_nan)
    result = train(check, prefix+'-skip-invalid-prefix', command, skipped, 3,
                   oracle.SAMPLE[3:], native)
    require(result == results['3'], 'skipped prefix influenced selected-row training')
    failed_training(check, prefix+'-read-invalid-prefix', command, skipped, 0, native)
    for label, payload in [('selected-nan', bytes(prefix_nan[:24+3*24])+struct.pack('<Q', 0x7ff8000000000042)+base[24+3*24+8:]),
                           ('truncated', base[:-1]), ('trailing', base+b'x')]:
        bad = directory(check, prefix+'-'+label, payload)
        failed_training(check, prefix+'-reject-'+label, command, bad, 3, native)
    for start in [8, 2147483647]:
        failed_training(check, f'{prefix}-invalid-start-{start}', command, working, start, native)
    for number, value in enumerate(['-1', '2147483648', 'x', '']):
        check.command(f'{prefix}-invalid-number-{number}', [*command, 'input.oxds', '1', '2', value],
                      working, status=2, native=native)
    rows, start = 178956974, 178956972
    large = directory(check, prefix+'-sparse')
    sparse(check, large/'input.oxds', rows, start, oracle.SAMPLE[-2:])
    results['sparse'] = train(check, prefix+'-over-4gib', command, large, start,
                              oracle.SAMPLE[-2:], native)
    return results


def reader_cases(check, prefix, command, native):
    base = oracle.grid(257)
    cases = [(f'start-{start}', base, start, 257, 0, 17) for start in [0, 1, 169, 170, 256]]
    cases += [('invalid-start', base, 257, 257, 7, 17),
              ('invalid-size', base, 1, 257, 4, 0),
              ('truncated', base[:-1], 250, 257, 2, 17),
              ('trailing', base+b'x', 250, 257, 3, 17)]
    for name, payload, start, rows, error, size in cases:
        cwd = directory(check, prefix+'-'+name, payload)
        result = check.command(prefix+'-'+name, [*command, str(start), str(rows), str(error), str(size)],
                               cwd, native=native)
        require(result.stdout == b'', name+': row/state oracle failed')
    rows, start = 178956974, 178956972
    large = directory(check, prefix+'-sparse')
    sparse(check, large/'input.oxds', rows, start,
           [(float(row), row+.5, -2.*row) for row in range(start, rows)])
    result = check.command(prefix+'-over-4gib', [*command, str(start), str(rows), '0', '1'],
                           large, native=native)
    require(result.stdout == b'', 'sparse row/state oracle failed')
    pipe = directory(check, prefix+'-nonseekable')
    path = pipe/'input.oxds'
    os.mkfifo(path, 0o600)
    # An already-open read/write endpoint prevents fixture deadlocks if the
    # child refuses the program before opening this FIFO.
    descriptor = os.open(path, os.O_RDWR | os.O_NONBLOCK)
    payload = oracle.grid(5)
    try:
        require(os.write(descriptor, payload) == len(payload), 'short FIFO fixture write')
        (pipe/'retained-fifo-input').write_bytes(payload)
        result = check.command(prefix+'-nonseekable', [*command, '1', '5', '1029', '2'],
                               pipe, native=native)
        require(result.stdout == b'', 'nonseekable reader failed to return its OS error')
    finally:
        os.close(descriptor)
    check.report.setdefault('inputs', []).append({'path': str(pipe/'retained-fifo-input'),
        'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest(),
        'transport': 'named FIFO', 'expected_seek_error': 29})
    check.save()


def verify(args):
    check = Verification(args, ['verify_seekable_streaming.py', 'data_pipeline_verification.py',
                               'streaming_training_oracle.py', 'linux_native_target.py'],
                         ['apps/cpu-training/tensor', 'apps/streaming-training'],
                         'Absolute file seek and suffix-only dataset validation, independent SGD, '
                         'O0/O2 and source-free Linux native deployment; no throughput claim.')
    for name in ['data/sample.oxds', 'tests/data/reader.oxds']:
        relative = 'apps/streaming-training/'+name
        check.sources[relative] = sha(ROOT/relative)
    check.report['tools'] = []
    check.save()
    try:
        for name in ['clang', 'opt', 'ld.lld']:
            tool = (check.llvm/name).resolve(strict=True)
            version = check.command('tool-'+name, [tool, '--version'], check.output)
            check.report['tools'].append({'path': str(tool), 'sha256': sha(tool),
                                           'version_stdout_sha256': hashlib.sha256(version.stdout).hexdigest()})
        check.save()
        for number, compiler in enumerate(check.compilers):
            prefix = f'compiler-{number}'
            snapshot = check.output/prefix
            check.snapshot(snapshot)
            project = snapshot/'apps/streaming-training'
            check.command(prefix+'-check', [compiler, 'check', '--message-format=json'], project)
            tested = check.command(prefix+'-test', [compiler, 'test', '--message-format=json'], project)
            tests = [json.loads(line) for line in tested.stdout.splitlines()]
            require(any(row.get('kind') == 'test-result' and row.get('name') == 'reader'
                        and row.get('success') is True and row.get('exit_status') == 0 for row in tests),
                    'reader default/start-row test did not execute')
            # Run reference cases from project copies with their own data.
            reference_root = check.output/(prefix+'-reference')
            check.snapshot(reference_root)
            reference_project = reference_root/'apps/streaming-training'
            sample = oracle.encode(oracle.SAMPLE)
            (reference_project/'input.oxds').write_bytes(sample)
            reference = {}
            for start in [0, 1, 3, 7]:
                reference[str(start)] = train(check, f'{prefix}-reference-{start}',
                    [compiler, 'run', '--'], reference_project, start, oracle.SAMPLE[start:], False)
            check.command(prefix+'-lock', [compiler, 'lock', '--message-format=json'], project)
            check.report.setdefault('locks', []).append({'path': str(project/'oxid.lock'), 'sha256': sha(project/'oxid.lock')})
            probe = snapshot/'apps/seek-reader'
            (probe/'src').mkdir(parents=True)
            shutil.copy2(project/'tests/seek.ox', probe/'src/main.ox')
            (probe/'oxid.toml').write_text('[project]\nname="seek-reader"\nedition="typed-preview"\nentry="src/main.ox"\n'
                '[build]\nentry-mode="process"\n[execution]\npolicy="hosted"\nheap-limit=16384\n'
                '[dependencies]\ntensor="../cpu-training/tensor"\nreader="../streaming-training/reader"\n')
            check.command(prefix+'-probe-lock', [compiler, 'lock', '--message-format=json'], probe)
            check.report['locks'].append({'path': str(probe/'oxid.lock'), 'sha256': sha(probe/'oxid.lock')})
            binaries = []
            for profile in ['dev', 'release']:
                for kind, source in [('app', project), ('reader', probe)]:
                    label = f'{prefix}-{profile}-{kind}'
                    check.command(label+'-build', [compiler, 'build', '--profile', profile,
                                  '--reproducible', '--frozen', '--message-format=json'], source)
                    name = 'streaming-training' if kind == 'app' else 'seek-reader'
                    binary = check.deploy(source/'.oxid/bin'/name, prefix+'-'+kind, profile)
                    binaries.append((kind, profile, binary))
            artifact = project/'.oxid/bin/streaming-training'
            before = sha(artifact)
            original = (project/'src/main.ox').read_bytes()
            (project/'src/main.ox').write_text('fn main()->i32{return true;}')
            check.command(prefix+'-source-refused', [compiler, 'build', '--frozen', '--message-format=json'], project, status=1)
            require(sha(artifact) == before, 'source refusal replaced the last good artifact')
            (project/'src/main.ox').write_bytes(original)
            dependency = project/'reader/src/lib.ox'
            old = dependency.read_bytes()
            dependency.write_bytes(old+b'\n')
            check.command(prefix+'-lock-refused', [compiler, 'build', '--frozen', '--message-format=json'], project, status=1)
            require(sha(artifact) == before, 'stale lock replaced the last good artifact')
            dependency.write_bytes(old)
            for source in [snapshot, reference_root]:
                retained = source.with_name('retained-'+source.name)
                source.rename(retained)
                require(not source.exists(), 'original source path remains accessible')
                check.report.setdefault('source_moves', []).append({'from': str(source), 'to': str(retained)})
            # Resolve retained lock paths after source relocation, keeping exact bytes.
            for pin in check.report['locks']:
                path = Path(pin['path'])
                if path.is_relative_to(snapshot):
                    pin['retained_path'] = str(snapshot.with_name('retained-'+snapshot.name)/path.relative_to(snapshot))
                    require(sha(pin['retained_path']) == pin['sha256'], 'lock changed during retention')
            check.save()
            native_results = None
            for kind, profile, binary in binaries:
                label = f'{prefix}-{profile}-{kind}'
                if kind == 'reader':
                    reader_cases(check, label, [binary], True)
                else:
                    current = app_cases(check, label, [binary], True)
                    for start in [1, 3, 7]:
                        require(current[str(start)] == reference[str(start)], 'native/reference selected summaries differ')
                    require(current['default'] == reference['0'], 'native/reference zero summaries differ')
                    if native_results is not None:
                        require(current == native_results, 'O0/O2 changed observable application results')
                    native_results = current
                check.deployed(binary, prefix+'-'+kind, profile)
        for tool in check.report['tools']:
            require(sha(tool['path']) == tool['sha256'], 'native tool changed during verification')
        for pin in check.report['locks']:
            require(sha(pin['retained_path']) == pin['sha256'], 'retained lock changed')
        check.finish()
    except BaseException as error:
        check.failed(error)
        raise
    print('Seekable streaming verification passed', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compilers', nargs='+', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--llvm-bin', required=True, type=Path)
    verify(parser.parse_args())
