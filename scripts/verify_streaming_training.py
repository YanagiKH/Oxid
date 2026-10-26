#!/usr/bin/env python3
"""Verify sequential dataset reads, bounded native deployment and analytic training."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import struct
import subprocess
import threading
import time

from data_pipeline_verification import ROOT, Verification, native_limits, sha
import streaming_training_oracle as oracle

DATA = Path('tests/data/reader.oxds')


def require(value, message):
    if not value:
        raise ValueError(message)


def case_directory(check, label, payload):
    directory = check.output/'runs'/label
    directory.mkdir(parents=True)
    path = directory/DATA
    path.parent.mkdir(parents=True)
    if payload is not None:
        path.write_bytes(payload)
    return directory


def reader_case(check, label, command, directory, expected, rows, size, native):
    result = check.command(label, [*command, str(expected), str(rows), str(size)], directory, native=native)
    require(result.stdout == b'', label+': reader did not validate every row and terminal state')
    path = directory/DATA
    check.report['cases'].append({'name': label, 'expected_error': expected, 'rows': rows,
                                  'batch_size': size, 'native': native,
                                  'data_sha256': sha(path) if path.is_file() else None})
    check.save()


def inspect_closed(check, label, binary, payload, expected, rows, size=2):
    directory = case_directory(check, label, payload)
    folder = check.output/'commands'/label
    folder.mkdir(parents=True)
    command = [str(binary), str(expected), str(rows), str(size), 'inspect']
    with subprocess.Popen(command, cwd=directory, env={}, stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, preexec_fn=native_limits) as child:
        try:
            selector = selectors.DefaultSelector()
            selector.register(child.stdout, selectors.EVENT_READ)
            with selector:
                require(selector.select(30), label+': no live-reader marker')
                marker = os.read(child.stdout.fileno(), 6)
            require(marker == b'READY\n', label+': wrong live-reader marker')
            links = []
            for path in (Path('/proc')/str(child.pid)/'fd').iterdir():
                try:
                    links.append(os.readlink(path))
                except FileNotFoundError:
                    pass
            dataset = str((directory/DATA).resolve())
            closed = dataset not in links
            stdout, stderr = child.communicate(b'x', timeout=30)
            (folder/'stdout').write_bytes(marker+stdout)
            (folder/'stderr').write_bytes(stderr)
            check.report['cases'].append({'name': label, 'pid': child.pid, 'open_descriptors': links,
                                          'dataset': dataset, 'closed_while_reader_live': closed,
                                          'exit_status': child.returncode})
            check.save()
            require(closed and child.returncode == 0 and stdout == b'' and not stderr,
                    label+': descriptor cleanup or subsequent reader state failed')
        finally:
            if child.poll() is None:
                child.kill()
                stdout, stderr = child.communicate()
                (folder/'interrupted-stdout').write_bytes(stdout)
                (folder/'interrupted-stderr').write_bytes(stderr)


def short_reads(check, label, binary):
    directory = case_directory(check, label, None)
    os.mkfifo(directory/DATA, 0o600)
    payload = oracle.grid(5)
    failures = []

    def feed():
        try:
            with (directory/DATA).open('wb', buffering=0) as target:
                at = 0
                widths = [1, 7, 3, 13]
                while at < len(payload):
                    count = target.write(payload[at:at+widths[at % 4]])
                    at += count
                    time.sleep(.001)
        except BaseException as error:
            failures.append(str(error))

    writer = threading.Thread(target=feed, daemon=True)
    writer.start()
    reader_case(check, label, [binary], directory, 0, 5, 2, True)
    writer.join(timeout=10)
    require(not writer.is_alive() and not failures, label+': short-read writer failed')
    (directory/'retained-pipe-input').write_bytes(payload)


def app_case(check, label, command, directory, epochs, size, native):
    result = check.command(label, [*command, 'input.oxds', str(epochs), str(size)], directory, native=native)
    losses = oracle.check_summary(result.stdout, oracle.SAMPLE, epochs, size)
    check.report['cases'].append({'name': label, 'epochs': epochs, 'batch_size': size,
                                  'independent_initial_loss': losses[0], 'independent_final_loss': losses[1]})
    check.save()
    return result.stdout


def verify(args):
    check = Verification(args, ['verify_streaming_training.py', 'streaming_training_oracle.py',
                               'data_pipeline_verification.py', 'linux_native_target.py'],
                         ['apps/cpu-training/tensor', 'apps/streaming-training'],
                         'Sequential finite f64 dataset batches, terminal cleanup, analytic SGD and source-free native execution.')
    # Dataset bytes are part of the source snapshot, rather than compiler input.
    for name in ['data/sample.oxds', 'tests/data/reader.oxds']:
        relative = 'apps/streaming-training/'+name
        check.sources[relative] = sha(ROOT/relative)
    check.save()
    try:
        require((ROOT/'apps/streaming-training/data/sample.oxds').read_bytes() == oracle.encode(oracle.SAMPLE),
                'sample differs from independent wire fixture')
        require((ROOT/'apps/streaming-training'/DATA).read_bytes() == oracle.grid(5),
                'reader fixture differs from independent wire fixture')
        for index, compiler in enumerate(check.compilers):
            prefix = f'compiler-{index}'
            snapshot = check.output/prefix
            check.snapshot(snapshot)
            project = snapshot/'apps/streaming-training'
            check.command(prefix+'-check', [compiler, 'check', '--message-format=json'], project)
            tested = check.command(prefix+'-test', [compiler, 'test', '--message-format=json'], project)
            results = [json.loads(line) for line in tested.stdout.splitlines()]
            actual = [row for row in results if row.get('kind') == 'test-result']
            require(len(actual) == 1 and actual[0].get('name') == 'reader'
                    and actual[0].get('success') is True and actual[0].get('exit_status') == 0,
                    'public reader test did not run')
            (project/'input.oxds').write_bytes(oracle.encode(oracle.SAMPLE))
            reference = {}
            for epochs, size in [(30, 4), (3, 3), (1, 128)]:
                reference[epochs, size] = app_case(check, f'{prefix}-reference-{epochs}-{size}',
                                                  [compiler, 'run', '--'], project, epochs, size, False)
            binaries = []
            for profile in ['dev', 'release']:
                check.command(f'{prefix}-{profile}-build', [compiler, 'build', '--profile', profile,
                                                            '--reproducible', '--message-format=json'], project)
                binary = check.deploy(project/'.oxid/bin/streaming-training', prefix, profile)
                binaries.append((profile, binary))
            # The probe is the same public reader test, built as a result program.
            probe = snapshot/'apps/reader-probe'
            probe.mkdir()
            shutil.copy2(project/'tests/reader.ox', probe/'main.ox')
            readers = []
            for limit in [16384, 1024]:
                (probe/'oxid.toml').write_text('[project]\nname="reader-probe"\nedition="typed-preview"\nentry="main.ox"\n'
                    '[build]\nentry-mode="process"\n[execution]\npolicy="hosted"\nheap-limit='+str(limit)+'\n'
                    '[dependencies]\ntensor="../cpu-training/tensor"\n')
                if limit == 16384:
                    for name, payload, expected, rows, size in oracle.reader_cases():
                        if name == 'larger-than-heap':
                            continue  # The native cases establish the heap bound.
                        directory = probe
                        path = directory/DATA
                        path.parent.mkdir(parents=True, exist_ok=True)
                        if path.exists():
                            path.rename(directory/('retained-'+name+'-previous.oxds'))
                        if payload is not None:
                            path.write_bytes(payload)
                        reader_case(check, f'{prefix}-reference-reader-'+name, [compiler, 'run', '--'],
                                    directory, expected, rows, size, False)
                for profile in ['dev', 'release']:
                    check.command(f'{prefix}-reader-{limit}-{profile}-build',
                                  [compiler, 'build', '--profile', profile, '--reproducible', '--message-format=json'], probe)
                    binary = check.deploy(probe/'.oxid/bin/reader-probe', prefix+'-reader-'+str(limit), profile)
                    readers.append((limit, profile, binary))
                shutil.copy2(probe/'oxid.toml', probe/f'manifest-{limit}.toml')
            retained = check.output/('retained-'+prefix)
            snapshot.rename(retained)
            require(not snapshot.exists(), 'build source paths remain available')
            check.report.setdefault('source_moves', []).append({'from': str(snapshot), 'to': str(retained),
                                                                 'restoration': 'Rename the retained directory to its original path.'})
            check.save()
            for limit, profile, binary in readers:
                label = f'{prefix}-reader-{limit}-{profile}'
                cases = oracle.reader_cases() if limit == 16384 else [('heap-limit', oracle.grid(5), 8, 0, 2)]
                for name, payload, expected, rows, size in cases:
                    directory = case_directory(check, label+'-'+name, payload)
                    reader_case(check, label+'-'+name, [binary], directory, expected, rows, size, True)
                if limit == 16384:
                    directory = case_directory(check, label+'-read-error', None)
                    (directory/DATA).mkdir()
                    reader_case(check, label+'-read-error', [binary], directory, 1021, 0, 2, True)
                    short_reads(check, label+'-short-reads', binary)
                    for name, payload, expected, rows, size in [('eof-closed', oracle.grid(5), 0, 5, 2),
                            ('truncated-closed', oracle.grid(5)[:-1], 2, 0, 2),
                            ('trailing-closed', oracle.grid(5)+b'x', 3, 0, 2),
                            ('memory-closed', oracle.grid(4099), 8, 0, 1024),
                            ('overflow-closed', b'OXDS0001'+struct.pack('<IIII', 2147483647, 2, 1, 0),
                             10, 0, 2147483647)]:
                        inspect_closed(check, label+'-'+name, binary, payload, expected, rows, size)
                check.deployed(binary, prefix+'-reader-'+str(limit), profile)
            for profile, binary in binaries:
                label = prefix+'-'+profile
                directory = check.output/'runs'/label
                directory.mkdir(parents=True)
                (directory/'input.oxds').write_bytes(oracle.encode(oracle.SAMPLE))
                for epochs, size in reference:
                    actual = app_case(check, f'{label}-train-{epochs}-{size}', [binary], directory, epochs, size, True)
                    require(actual == reference[epochs, size], 'native and reference summaries differ')
                for number, arguments in enumerate([[], ['--help'], ['-h']]):
                    result = check.command(f'{label}-help-{number}', [binary, *arguments], directory, native=True)
                    require(b'streaming-training <dataset> <epochs> <batch-size>' in result.stdout, 'missing usage')
                for number, arguments in enumerate([['input.oxds'], ['input.oxds', '0', '4'],
                        ['input.oxds', '1000001', '4'], ['input.oxds', '1', '0'],
                        ['input.oxds', '1', '2147483648'], ['input.oxds', '-1', '4'],
                        ['input.oxds', '1', 'x'], ['input.oxds', '1', '4', 'extra']]):
                    check.command(f'{label}-invalid-{number}', [binary, *arguments], directory, 2, native=True)
                for number, payload in enumerate([b'', oracle.encode(oracle.SAMPLE)[:-1],
                                                  oracle.encode(oracle.SAMPLE)+b'x']):
                    (directory/f'invalid-{number}.oxds').write_bytes(payload)
                    result = check.command(f'{label}-bad-data-{number}',
                                           [binary, f'invalid-{number}.oxds', '1', '4'], directory, 1, native=True)
                    require(b'Training failed.' in result.stdout and b'Final MSE:' not in result.stdout,
                            'failed training reported success')
                check.command(label+'-missing', [binary, 'missing.oxds', '1', '4'], directory, 1, native=True)
                check.deployed(binary, prefix, profile)
        check.finish()
    except BaseException as error:
        check.failed(error)
        raise
    print('Streaming training verification passed', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compilers', nargs='+', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--llvm-bin', required=True, type=Path)
    verify(parser.parse_args())
