#!/usr/bin/env python3
"""Verify sequential dataset reads, bounded native deployment and analytic training."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import struct
import subprocess
import threading
import time
import zlib

from data_pipeline_verification import ROOT, STREAMING_TRAINING_SOURCES, Verification, native_limits, sha
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


def app_case(check, label, command, directory, epochs, size, native, shuffle=None):
    filename = 'input.oxds'
    rows = oracle.SAMPLE
    extra = []
    seed = None
    if shuffle is not None:
        start, seed = shuffle
        filename = 'shuffle.oxds'
        rows = oracle.SHUFFLE_ROWS[start:]
        if start != 0:
            extra.append(str(start))
        extra.extend(['--shuffle', str(seed)])
    result = check.command(label, [*command, filename, str(epochs), str(size), *extra],
                           directory, native=native)
    losses = oracle.check_summary(result.stdout, rows, epochs, size, seed)
    case = {'name': label, 'epochs': epochs, 'batch_size': size,
            'independent_initial_loss': losses[0], 'independent_final_loss': losses[1]}
    if shuffle is not None:
        case.update(start_row=start, shuffle_seed=seed)
    check.report['cases'].append(case)
    check.save()
    return result.stdout


def checkpoint_cases(check, label, command, directory, native):
    directory.mkdir(parents=True)
    source = directory/'input.oxds'
    payload = oracle.encode(oracle.SHUFFLE_ROWS)
    source.write_bytes(payload)
    batch_size = 2
    start = 2
    seed = 42
    flags = [str(start), '--shuffle', str(seed)]

    def run(name, epochs, mode, prefix, status=0, size=batch_size):
        return check.command(label+'-'+name,
                             [*command, str(source), str(epochs), str(size), *flags, mode, directory/prefix],
                             directory, status, native=native)

    whole = run('uninterrupted', 3, '--checkpoint', 'whole')
    rows = oracle.SHUFFLE_ROWS[start:]
    oracle.check_summary(whole.stdout, rows, 3, batch_size, seed)
    first = run('first-epoch', 1, '--checkpoint', 'split')
    oracle.check_summary(first.stdout, rows, 1, batch_size, seed)
    resumed = run('resume-two-epochs', 2, '--resume', 'split')
    require(resumed.stdout.splitlines()[1:3] == whole.stdout.splitlines()[1:3]
            and resumed.stdout.splitlines()[-1:] == whole.stdout.splitlines()[-1:],
            label+': resumed counts or final loss differ')

    def slots(prefix):
        return tuple((directory/f'{prefix}.{slot}').read_bytes() for slot in ('a', 'b'))

    expected = slots('whole')
    require(slots('split') == expected, label+': resumed model or shuffle state differ')
    selected_start = 24 + start * 24
    selected_digest = zlib.crc32(payload[selected_start:])
    for epoch, raw in zip((2, 3), expected):
        require(raw.startswith(b'OXSTR001') and struct.unpack_from('<I', raw, 8)[0] == epoch
                and zlib.crc32(raw[:-4]) == struct.unpack_from('<I', raw, len(raw)-4)[0],
                label+': wrong retained epoch or checksum')
        require(selected_digest == struct.unpack_from('<I', raw, 36)[0],
                label+': saved checksum does not cover the selected rows')

    second = directory/'split.b'
    second.write_bytes(expected[1][:37])
    run('recover-truncated-latest', 1, '--resume', 'split')
    require(slots('split') == expected, label+': recovery overwrote or diverged from the last good model')
    run('refuse-existing-prefix', 1, '--checkpoint', 'split', 1)
    run('refuse-changed-batch-size', 1, '--resume', 'split', 1, size=3)
    changed = bytearray(payload)
    changed[selected_start] ^= 1
    source.write_bytes(changed)
    run('refuse-changed-dataset', 1, '--resume', 'split', 1)
    require(slots('split') == expected, label+': failed restore changed a checkpoint')
    check.report['cases'].append({'name': label, 'epochs': 3, 'native': native,
                                  'checkpoint_sha256': [hashlib.sha256(raw).hexdigest() for raw in expected]})
    check.save()


def interrupted_checkpoint(check, label, command, directory, native):
    directory.mkdir(parents=True)
    source = directory / "input.oxds"
    source.write_bytes(oracle.encode(oracle.SHUFFLE_ROWS))
    prefix = directory / "interrupted"
    flags = ["2", "--shuffle", "42"]
    args = [*command, str(source), "1000000", *flags, "--checkpoint", str(prefix)]
    folder = check.output / "commands" / label
    folder.mkdir(parents=True)

    def latest(base):
        candidates = []
        for suffix in ("a", "b"):
            path = base.with_suffix("." + suffix)
            if not path.exists():
                continue
            raw = path.read_bytes()
            if (len(raw) >= 44 and raw.startswith(b"OXSTR001")
                    and zlib.crc32(raw[:-4]) == struct.unpack_from("<I", raw, len(raw) - 4)[0]):
                candidates.append((struct.unpack_from("<I", raw, 8)[0], raw))
        return max(candidates, default=None)

    with subprocess.Popen(args, cwd=directory, env=check.environment(native),
                          stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          preexec_fn=native_limits if native else None) as child:
        try:
            deadline = time.monotonic() + 30
            saved = latest(prefix)
            while (saved is None or saved[0] == 0) and child.poll() is None:
                require(time.monotonic() < deadline, label + ": no completed epoch before cancellation")
                time.sleep(0.005)
                saved = latest(prefix)
            require(child.poll() is None, label + ": worker ended before cancellation")
            child.send_signal(signal.SIGTERM)
            stdout, stderr = child.communicate(timeout=20)
            (folder / "stdout").write_bytes(stdout)
            (folder / "stderr").write_bytes(stderr)
            require(child.returncode == 130 and not stdout and not stderr,
                    label + ": interrupted training did not exit cleanly")
        finally:
            if child.poll() is None:
                child.kill()
                child.communicate()
    saved = latest(prefix)
    require(saved is not None and saved[0] > 0, label + ": cancellation erased the last completed epoch")
    completed = saved[0]
    resumed = check.command(label + "-resume", [
        *command, str(source), "1", *flags, "--resume", str(prefix),
    ], directory, native=native)
    whole = directory / "uninterrupted"
    baseline = check.command(label + "-baseline", [
        *command, str(source), str(completed + 1), *flags, "--checkpoint", str(whole),
    ], directory, native=native)
    require(resumed.stdout.splitlines()[-1] == baseline.stdout.splitlines()[-1]
            and latest(prefix) == latest(whole),
            label + ": resumed checkpoint differs from uninterrupted training")
    check.report["cases"].append({
        "name": label, "native": native, "exit_status": 130, "signal": "SIGTERM",
        "saved_epochs": completed, "resumed_epochs": completed + 1,
        "checkpoint_sha256": hashlib.sha256(latest(prefix)[1]).hexdigest(),
    })
    check.save()


def verify(args):
    check = Verification(args, ['verify_streaming_training.py', 'streaming_training_oracle.py',
                               'data_pipeline_verification.py', 'linux_native_target.py'],
                         STREAMING_TRAINING_SOURCES,
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
            (project/'shuffle.oxds').write_bytes(oracle.encode(oracle.SHUFFLE_ROWS))
            shuffled = {}
            for epochs, size, start, seed in [(5, 2, 2, 42), (5, 2, 2, 43), (4, 3, 1, 0), (2, 2147483647, 0, 4294967295)]:
                key = epochs, size, start, seed
                label = '-'.join(map(str, key))
                shuffled[key] = app_case(check, f'{prefix}-reference-shuffle-{label}', [compiler, 'run', '--'],
                                         project, epochs, size, False, (start, seed))
            require(shuffled[5, 2, 2, 42] != shuffled[5, 2, 2, 43],
                    'distinct seeds produced the same measured training result')
            checkpoint_cases(check, prefix+'-checkpoint-reference', [compiler, 'run', '--'],
                             project/'checkpoint-test', False)
            interrupted_checkpoint(check, prefix+'-interrupt-reference', [compiler, 'run', '--'],
                                   project/'interrupt-test', False)
            binaries = []
            for profile in ['dev', 'release']:
                check.command(f'{prefix}-{profile}-build', [compiler, 'build', '--profile', profile,
                                                            '--reproducible', '--message-format=json'], project)
                binary = check.deploy(project/'.oxid/bin/streaming-training', prefix, profile)
                binaries.append((profile, binary))
            limited = []
            heap_limit = 131072
            manifest = project/'oxid.toml'
            original = manifest.read_text()
            hosted = 'policy = "hosted"\n'
            require(original.count(hosted) == 1 and 'heap-limit' not in original,
                    'cannot apply the training heap limit')
            try:
                manifest.write_text(original.replace(hosted, hosted + f'heap-limit = {heap_limit}\n'))
                for profile in ['dev', 'release']:
                    check.command(f'{prefix}-{profile}-limited-build',
                                  [compiler, 'build', '--profile', profile, '--reproducible'], project)
                    binary = check.deploy(project/'.oxid/bin/streaming-training', prefix+'-limited', profile)
                    limited.append((profile, binary))
            finally:
                manifest.write_text(original)
            # The probe is the same public reader test, built as a result program.
            probe = snapshot/'apps/reader-probe'
            probe.mkdir()
            shutil.copy2(project/'tests/reader.ox', probe/'main.ox')
            readers = []
            for limit in [16384, 1024]:
                (probe/'oxid.toml').write_text('[project]\nname="reader-probe"\nedition="typed-preview"\nentry="main.ox"\n'
                    '[build]\nentry-mode="process"\n[execution]\npolicy="hosted"\nheap-limit='+str(limit)+'\n'
                    '[dependencies]\ntensor="../cpu-training/tensor"\nreader="../streaming-training/reader"\n')
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
                    seek_label = label + '-seek-open-reader'
                    directory = case_directory(check, seek_label, oracle.grid(5))
                    observed = check.command(seek_label, [binary], directory, native=True)
                    require(observed.stdout == b'', seek_label + ': unexpected reader output')
                    check.report['cases'].append({'name': seek_label, 'native': True, 'rows': 5,
                                                  'data_sha256': sha(directory / DATA)})
                    check.save()
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
                (directory/'shuffle.oxds').write_bytes(oracle.encode(oracle.SHUFFLE_ROWS))
                for (epochs, size, start, seed), expected in shuffled.items():
                    case_label = f'{label}-shuffle-{epochs}-{size}-{start}-{seed}'
                    actual = app_case(check, case_label, [binary], directory, epochs, size, True, (start, seed))
                    require(actual == expected, 'native and reference shuffled summaries differ')
                repeated = app_case(check, f'{label}-shuffle-repeated', [binary], directory, 5, 2, True, (2, 42))
                require(repeated == shuffled[5, 2, 2, 42], 'identical seeds changed shuffled training')
                checkpoint_cases(check, label+'-checkpoint', [binary], directory/'checkpoint-test', True)
                interrupted_checkpoint(check, label+'-interrupt', [binary], directory/'interrupt-test', True)
                for number, arguments in enumerate([[], ['--help'], ['-h']]):
                    result = check.command(f'{label}-help-{number}', [binary, *arguments], directory, native=True)
                    require(b'streaming-training <dataset> <epochs> <batch-size>' in result.stdout, 'missing usage')
                for number, arguments in enumerate([['input.oxds'], ['input.oxds', '0', '4'],
                        ['input.oxds', '1000001', '4'], ['input.oxds', '1', '0'],
                        ['input.oxds', '1', '2147483648'], ['input.oxds', '-1', '4'],
                        ['input.oxds', '1', 'x'], ['input.oxds', '1', '4', 'extra'],
                        ['input.oxds', '1', '4', '--shuffle'],
                        ['input.oxds', '1', '4', '--shuffle', '-1'],
                        ['input.oxds', '1', '4', '--shuffle', '4294967296'],
                        ['input.oxds', '1', '4', '--unknown', '42'],
                        ['input.oxds', '1', '4', '128', '--shuffle', '42', 'extra']]):
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
            count = 8193
            repeats = (count + len(oracle.SHUFFLE_ROWS) - 1) // len(oracle.SHUFFLE_ROWS)
            rows = (oracle.SHUFFLE_ROWS * repeats)[:count]
            payload = oracle.encode(rows)
            require(len(payload) > heap_limit, 'test dataset fits in the execution heap')
            directory = check.output/'runs'/(prefix+'-limited')
            directory.mkdir(parents=True)
            dataset = directory/'large.oxds'
            dataset.write_bytes(payload)
            for profile, binary in limited:
                label = f'{prefix}-{profile}-shuffle-larger-than-heap'
                result = check.command(label, [binary, dataset.name, '1', '8', '--shuffle', '2026'],
                                       directory, native=True)
                losses = oracle.check_summary(result.stdout, rows, 1, 8, 2026)
                check.report['cases'].append({
                    'name': label, 'dataset_bytes': len(payload), 'heap_limit': heap_limit,
                    'data_sha256': sha(dataset), 'rows': count, 'batches': (count-1)//8+1,
                    'independent_initial_loss': losses[0], 'independent_final_loss': losses[1],
                })
                check.deployed(binary, prefix+'-limited', profile)
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
