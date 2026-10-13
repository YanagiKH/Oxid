"""Owned external Unit4 launcher. Import/validation never start a process.

An exact pinned invocation is required. The control file is data, not code.
This observes only its direct producer; operation-tree cleanup remains unverified.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import time

CAP = 8 * 1024 ** 2
CONTROL_CAP = 4096
TIMEOUT = 36300
GRACE = 10
REAP_GRACE = 2
SCHEMA = 'oxid-unit4-owned-launcher-invocation-v2'
CAPACITY_GROWTH_BYTES = 8378261504
CAPACITY_GROWTH_INODES = 232225
CAPACITY_ALARM_BYTES = 2281701376  # Sampled-risk policy; NOT an aggregate burst bound.
CAPACITY_CLEANUP_INODES = 1024
KEYS = {'schema', 'argv', 'cwd', 'environment', 'expected_head', 'expected_tree',
        'entrypoint_sha256', 'interpreter_sha256', 'launcher_sha256',
        'request_sha256', 'launch_nonce', 'launch_record_path',
        'terminal_record_path', 'stdout_path', 'stderr_path', 'cancel_path',
        'external_term_seconds', 'external_kill_grace_seconds',
        'shared_work_seconds', 'no_automatic_retry', 'clear_inherited_environment'}
ENV_KEYS = {'CARGO_HOME', 'HOME', 'LANG', 'LC_ALL', 'PATH',
            'PYTHONDONTWRITEBYTECODE', 'PYTHONNOUSERSITE', 'PYTHONOPTIMIZE',
            'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN', 'TMPDIR'}
OUTPUT_KEYS = ('launch_record_path', 'terminal_record_path', 'stdout_path',
               'stderr_path', 'cancel_path')


class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: need(False, 'nonfinite JSON'))


def real(path, exists=True):
    need(type(path) is str, 'string path required')
    value = Path(path)
    need(value.is_absolute() and value.resolve(strict=exists) == value and
         not any(p.is_symlink() for p in (value, *value.parents)),
         'canonical path without symlinks required')
    return value


def body(path, cap=CAP):
    path = real(str(path))
    before = path.lstat()
    need(stat.S_ISREG(before.st_mode) and before.st_size <= cap,
         'bounded regular file required')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        need((opened.st_dev, opened.st_ino) == (before.st_dev, before.st_ino),
             'input identity changed')
        raw = stream.read(cap + 1)
        after = os.fstat(stream.fileno())
    need(len(raw) <= cap and len(raw) == before.st_size == after.st_size and
         (opened.st_mtime_ns, opened.st_ctime_ns) ==
         (after.st_mtime_ns, after.st_ctime_ns), 'input body changed')
    need(path.lstat() == after, 'input path changed')
    return raw


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def identity(path, cap=CAP):
    raw = body(path, cap)
    return {'path': str(path), 'bytes': len(raw), 'sha256': digest(raw)}


def exclusive_json(path, value):
    raw = canonical(value)
    need(len(raw) <= CAP, 'bounded evidence record')
    with Path(path).open('xb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def validate(invocation_path, sha256):
    need(sys.platform.startswith('linux'), 'Linux owned-process identity required')
    need(sys.dont_write_bytecode and not sys.flags.optimize,
         'normal bytecode-disabled Python required')
    need(type(sha256) is str and re.fullmatch('[0-9a-f]{64}', sha256), 'exact invocation pin')
    raw = body(invocation_path)
    need(digest(raw) == sha256, 'invocation pin mismatch')
    value = decode(raw)
    need(type(value) is dict and set(value) == KEYS and value['schema'] == SCHEMA,
         'closed invocation schema')
    for key in ('entrypoint_sha256', 'interpreter_sha256', 'launcher_sha256', 'request_sha256'):
        need(type(value[key]) is str and re.fullmatch('[0-9a-f]{64}', value[key]), 'exact body pin')
    for key in ('expected_head', 'expected_tree'):
        need(type(value[key]) is str and re.fullmatch('[0-9a-f]{40}', value[key]), 'exact commit identity')
    need(type(value['launch_nonce']) is str and re.fullmatch('[0-9a-f]{32}', value['launch_nonce']), 'exact launch nonce')
    need(type(value['external_term_seconds']) is int and value['external_term_seconds'] == TIMEOUT and
         type(value['external_kill_grace_seconds']) is int and value['external_kill_grace_seconds'] == GRACE and
         type(value['shared_work_seconds']) is int and value['shared_work_seconds'] == 36000 and
         value['no_automatic_retry'] is True and value['clear_inherited_environment'] is True,
         'unchanged bounds and no retry')
    cwd = real(value['cwd'])
    need(cwd.is_dir(), 'canonical checkout directory')
    argv = value['argv']
    need(type(argv) is list and len(argv) == 6 and all(type(a) is str and '\0' not in a for a in argv) and
         argv[1] == '-B' and argv[3] == '--request' and argv[5] == '--execute', 'exact direct producer argv')
    interpreter, entrypoint, request_path = (real(argv[i]) for i in (0, 2, 4))
    need(entrypoint == cwd / 'tests/qualification/package_dependency_unit4_session_v1/run.py', 'fixed producer entrypoint')
    need(os.access(interpreter, os.X_OK), 'executable interpreter required')
    need(identity(interpreter, 64 * 1024 ** 2)['sha256'] == value['interpreter_sha256'], 'interpreter changed')
    need(identity(entrypoint)['sha256'] == value['entrypoint_sha256'], 'entrypoint changed')
    need(identity(Path(__file__).resolve())['sha256'] == value['launcher_sha256'], 'launcher changed')
    request_raw = body(request_path)
    need(digest(request_raw) == value['request_sha256'], 'request changed')
    request = decode(request_raw)
    need(type(request) is dict and request.get('schema') == 'oxid-unit4-local-session-request-v1' and
         request.get('expected_head') == request.get('event_sha') == value['expected_head'], 'bound request head')
    root = real(request['run_root'], False)
    need(not os.path.lexists(root) and root.parent.is_dir(), 'fresh producer root')
    env = value['environment']
    need(type(env) is dict and set(env) == ENV_KEYS and
         all(type(v) is str and '\0' not in v for v in env.values()), 'closed clean environment')
    need(env['PYTHONDONTWRITEBYTECODE'] == '1' and env['PYTHONNOUSERSITE'] == '1' and
         env['PYTHONOPTIMIZE'] == '0' and env['LANG'] == env['LC_ALL'] == 'C.UTF-8', 'fixed interpreter environment')
    outputs = [real(value[key], False) for key in OUTPUT_KEYS]
    need(len(set(outputs)) == len(outputs) and len({p.parent for p in outputs}) == 1,
         'distinct evidence and control paths in one directory')
    for path in outputs:
        need(path.parent.is_dir() and not os.path.lexists(path), 'fresh evidence/control path')
        need(path != root and root not in path.parents and path not in root.parents and
             path != cwd and cwd not in path.parents, 'external evidence/control paths')
    return value, request


def process_identity(pid):
    """Kernel identity of a direct child in its own session, never a name search."""
    raw = Path('/proc/' + str(pid) + '/stat').read_text()
    end = raw.rfind(')')
    need(end > 0 and raw.split(' ', 1)[0] == str(pid), 'process identity format')
    fields = raw[end + 2:].split()
    need(len(fields) >= 20, 'complete process identity')
    return {'pid': pid, 'ppid': int(fields[1]), 'pgid': int(fields[2]),
            'sid': int(fields[3]), 'start_ticks': int(fields[19])}


def read_control(path, invocation, observation=None):
    raw = body(path, CONTROL_CAP)
    if observation is not None:
        observation.update(bytes=len(raw), sha256=digest(raw), raw_hex=raw.hex())
    value = decode(raw)
    need(type(value) is dict and set(value) == {'schema', 'launch_nonce', 'request_sha256', 'reason'} and
         value['schema'] == 'oxid-unit4-local-session-cancel-v1', 'closed cancellation schema')
    need(value['launch_nonce'] == invocation['launch_nonce'] and
         value['request_sha256'] == invocation['request_sha256'], 'cancellation binding mismatch')
    need(type(value['reason']) is str and 0 < len(value['reason']) <= 512,
         'bounded cancellation reason')
    return {'path': str(path), 'bytes': len(raw), 'sha256': digest(raw), 'reason': value['reason']}


def signal_owned(child, owned, signum, record):
    row = {'signal': signal.Signals(signum).name, 'started_ns': time.time_ns(), 'sent': False}
    record['signals'].append(row)
    try:
        if child.poll() is not None:
            row['not_sent'] = 'producer already exited'
            return False
        need(owned is not None and process_identity(child.pid) == owned and
             owned['ppid'] == os.getpid() and owned['pid'] == owned['pgid'] == owned['sid'],
             'owned producer identity unavailable or changed')
        if signum == signal.SIGKILL:
            os.killpg(owned['pgid'], signum)
        else:
            os.kill(owned['pid'], signum)
        row['sent'] = True
        return True
    except BaseException as error:
        row['error'] = str(error)[:2048]
        return False
    finally:
        row['completed_ns'] = time.time_ns()


def receipt_status(root, invocation, request):
    path = root / 'unit4-local-session.json'
    if not os.path.lexists(path):
        return {'path': str(path), 'available': False}
    result = {'path': str(path), 'available': True, 'valid_binding': False}
    try:
        raw = body(path)
        result.update(bytes=len(raw), sha256=digest(raw))
        value = decode(raw)
        need(value['schema'] == 'oxid-unit4-local-session-result-v1' and
             value['request_sha256'] == invocation['request_sha256'] and value['request'] == request and
             value['entrypoint']['sha256'] == invocation['entrypoint_sha256'] and
             value['plan']['root'] == str(root), 'producer receipt binding mismatch')
        need(type(value['local_complete']) is bool and value['status'] == 'NotReady' and
             value['execution_qualified'] is False and value['ci_runtime_proof'] is False,
             'producer receipt completion fields')
        result.update(valid_binding=True, local_complete=value['local_complete'], failure=value['failure'])
    except BaseException as error:
        result['error'] = str(error)[:2048]
    return result


def capacity_identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid)


def capacity_open(request, invocation, report):
    """Anchor the actual output filesystem; no tree scan or new directory."""
    parent = real(str(Path(request['run_root']).parent))
    evidence = real(str(Path(invocation['terminal_record_path']).parent))
    before = parent.lstat()
    fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        opened = os.fstat(fd)
        need(stat.S_ISDIR(opened.st_mode) and capacity_identity(opened) == capacity_identity(before),
             'capacity parent changed while opening')
        output_identity = capacity_identity(opened)
        evidence_identity = capacity_identity(evidence.lstat())
        need(evidence_identity[0] == output_identity[0], 'output and evidence require one filesystem')
        report['capacity'] = {
            'schema': 'oxid-unit4-sampled-capacity-observation-v1',
            'parent': str(parent), 'parent_identity': list(output_identity),
            'evidence_parent': str(evidence), 'evidence_identity': list(evidence_identity),
            'sample_target_seconds': 0.1, 'samples': 0, 'maximum_observed_gap_seconds': 0.0,
            'first': None, 'last': None, 'minimum_bytes_sample': None,
            'minimum_inodes_sample': None, 'alarm_sample': None,
            'expected_growth_bytes': CAPACITY_GROWTH_BYTES,
            'expected_growth_inodes': CAPACITY_GROWTH_INODES,
            'alarm_bytes': CAPACITY_ALARM_BYTES, 'alarm_inodes': None,
            'hard_no_overshoot_guarantee': False,
            'risk': 'Sampled safeguard only; aggregate intersample/cleanup growth and other writers are unbounded.',
            'anchor_released': False}
        return {'fd': fd, 'parent': parent, 'evidence': evidence,
                'parent_identity': output_identity, 'evidence_identity': evidence_identity,
                'root': Path(request['run_root']), 'root_identity': None, 'filesystem': None}
    except BaseException as error:
        try:
            os.close(fd)
        except BaseException as close_error:
            error.add_note('capacity anchor close failed: ' + repr(close_error))
        raise


def capacity_sample(state, report, *, initial=False, phase='running'):
    try:
        return _capacity_sample(state, report, initial=initial)
    except BaseException as error:
        report['capacity'].setdefault('sample_failure', {
            'monotonic': time.monotonic(), 'wall_ns': time.time_ns(),
            'phase': 'prelaunch' if initial else phase,
            'error_type': type(error).__name__, 'reason': str(error)[:2048],
            'free_space_observation_available': False})
        raise


def _capacity_sample(state, report, *, initial=False):
    """One constant-cost sample. Equality is admitted; below either floor cancels."""
    observed = report['capacity']
    need(capacity_identity(os.fstat(state['fd'])) == state['parent_identity'] and
         capacity_identity(state['parent'].lstat()) == state['parent_identity'],
         'capacity output parent identity changed')
    need(capacity_identity(state['evidence'].lstat()) == state['evidence_identity'],
         'capacity evidence parent identity changed')
    if os.path.lexists(state['root']):
        info = state['root'].lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_dev == state['parent_identity'][0],
             'capacity output root filesystem/type changed')
        identity = capacity_identity(info)
        need(state['root_identity'] in (None, identity), 'capacity output root identity changed')
        state['root_identity'] = identity
    else:
        need(state['root_identity'] is None, 'capacity observed output root disappeared')
    disk = os.fstatvfs(state['fd'])
    filesystem = (getattr(disk, 'f_fsid', None), disk.f_frsize, disk.f_bsize, disk.f_files)
    need(disk.f_frsize == disk.f_bsize == 4096 and disk.f_files > 0,
         'capacity filesystem geometry changed')
    need(state['filesystem'] in (None, filesystem), 'capacity filesystem identity changed')
    state['filesystem'] = filesystem
    alarm_inodes = (disk.f_files + 4) // 5 + CAPACITY_CLEANUP_INODES
    sample = {'monotonic': time.monotonic(), 'wall_ns': time.time_ns(),
              'available_blocks': disk.f_bavail, 'available_bytes': disk.f_bavail * disk.f_frsize,
              'available_inodes': disk.f_favail, 'fragment_bytes': disk.f_frsize,
              'filesystem_id': filesystem[0], 'total_inodes': disk.f_files, 'initial': initial}
    previous = observed['last']
    if previous is not None:
        observed['maximum_observed_gap_seconds'] = max(observed['maximum_observed_gap_seconds'],
                                                       sample['monotonic'] - previous['monotonic'])
    observed['samples'] += 1
    observed['first'] = observed['first'] or sample
    observed['last'] = sample
    observed['alarm_inodes'] = alarm_inodes
    for key, field in (('minimum_bytes_sample', 'available_bytes'), ('minimum_inodes_sample', 'available_inodes')):
        if observed[key] is None or sample[field] < observed[key][field]:
            observed[key] = sample
    required_bytes = CAPACITY_ALARM_BYTES + (CAPACITY_GROWTH_BYTES if initial else 0)
    required_inodes = alarm_inodes + (CAPACITY_GROWTH_INODES if initial else 0)
    if sample['available_bytes'] < required_bytes or sample['available_inodes'] < required_inodes:
        observed['alarm_sample'] = observed['alarm_sample'] or sample
        reason = 'capacity ' + ('prelaunch reserve unavailable' if initial else 'sampled free-space/inode alarm')
        observed.setdefault('first_alarm_reason', reason)
        return reason
    return None


def capacity_close(state, report):
    os.close(state['fd'])
    report['capacity']['anchor_released'] = True


def run(invocation_path, sha256):
    started = time.monotonic()
    invocation, request = validate(invocation_path, sha256)
    report = {'schema': 'oxid-unit4-owned-launcher-terminal-v1', 'invocation_sha256': sha256,
              'request_sha256': invocation['request_sha256'], 'launch_nonce': invocation['launch_nonce'],
              'started_ns': time.time_ns(), 'pid': None, 'owned_identity': None,
              'returncode': None, 'reaped': False, 'signals': [], 'cancel_reason': None,
              'accepted_control': None, 'control_observation': None,
              'failure': None, 'finalization_errors': [],
              'operation_group_cleanup': 'unverified', 'status': 'NotReady',
              'execution_qualified': False, 'ci_runtime_proof': False}
    previous, streams = {}, {}
    child = owned = capacity = None
    pending = []
    control_seen = False

    def capture_signal(signum, frame):
        if not pending:
            pending.append('supervisor ' + signal.Signals(signum).name)

    def fail(error):
        if report['failure'] is None:
            report['failure'] = {'type': type(error).__name__, 'message': str(error)[:4096]}

    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.getsignal(sig)
            signal.signal(sig, capture_signal)
        capacity = capacity_open(request, invocation, report)
        reason = capacity_sample(capacity, report, initial=True)
        if reason:
            report['cancel_reason'] = reason
            raise Rejected(reason)
        for name in ('stdout', 'stderr'):
            streams[name] = Path(invocation[name + '_path']).open('xb')
        need(time.monotonic() - started < TIMEOUT, 'outer deadline exhausted before launch')
        child = subprocess.Popen(invocation['argv'], cwd=invocation['cwd'], env=invocation['environment'],
                                 stdin=subprocess.DEVNULL, stdout=streams['stdout'], stderr=streams['stderr'],
                                 shell=False, close_fds=True, start_new_session=True)
        report['pid'] = child.pid
        report['identity_window_started_ns'] = time.time_ns()
        if child.poll() is None:
            owned = process_identity(child.pid)
            need(owned['ppid'] == os.getpid() and owned['pid'] == owned['pgid'] == owned['sid'],
                 'producer did not enter owned session')
            report['owned_identity'] = owned
        report['identity_window_completed_ns'] = time.time_ns()
        exclusive_json(invocation['launch_record_path'], dict(report,
                       schema='oxid-unit4-owned-launcher-start-v1', argv=invocation['argv'],
                       cwd=invocation['cwd'], environment=invocation['environment'],
                       expected_head=invocation['expected_head'], expected_tree=invocation['expected_tree'],
                       stdout_path=invocation['stdout_path'], stderr_path=invocation['stderr_path'],
                       cancel_path=invocation['cancel_path']))
        while child.poll() is None:
            if pending:
                report['cancel_reason'] = pending[0]
                break
            if not control_seen and os.path.lexists(invocation['cancel_path']):
                control_seen = True
                report['control_observation'] = {'path': invocation['cancel_path'], 'observed_ns': time.time_ns()}
                report['accepted_control'] = read_control(invocation['cancel_path'], invocation,
                                                         report['control_observation'])
                report['cancel_reason'] = 'control file: ' + report['accepted_control']['reason']
                break
            if time.monotonic() - started >= TIMEOUT:
                report['cancel_reason'] = 'outer 36300-second deadline'
                break
            reason = capacity_sample(capacity, report)
            if reason:
                report['cancel_reason'] = report['cancel_reason'] or reason
                break
            for name in streams:
                need(os.fstat(streams[name].fileno()).st_size <= CAP, 'launcher ' + name + ' log bound exceeded')
            time.sleep(0.1)
    except BaseException as error:
        fail(error)
        report['cancel_reason'] = report['cancel_reason'] or 'launcher failure'
    finally:
        if pending:
            report['cancel_reason'] = report['cancel_reason'] or pending[0]
        if child is not None:
            try:
                if child.poll() is None:
                    report['cancel_reason'] = report['cancel_reason'] or 'launcher finalization'
                    signal_owned(child, owned, signal.SIGTERM, report)
                    try:
                        child.wait(timeout=GRACE)
                    except subprocess.TimeoutExpired:
                        signal_owned(child, owned, signal.SIGKILL, report)
                        child.wait(timeout=REAP_GRACE)
                else:
                    child.wait(timeout=0)
                report['returncode'] = child.returncode
                report['reaped'] = child.returncode is not None
            except BaseException as error:
                report['finalization_errors'].append('reap: ' + str(error)[:2048])
                report['returncode'] = child.returncode
        if capacity is not None:
            try:
                reason = capacity_sample(capacity, report, phase='finalization')
                report['cancel_reason'] = report['cancel_reason'] or reason
            except BaseException as error:
                report['finalization_errors'].append('capacity sample: ' + str(error)[:2048])
                fail(error)
            try:
                capacity_close(capacity, report)
            except BaseException as error:
                report['finalization_errors'].append('capacity close: ' + str(error)[:2048])
        for name, stream in streams.items():
            try:
                stream.flush()
                os.fsync(stream.fileno())
                stream.close()
                report[name] = identity(invocation[name + '_path'])
            except BaseException as error:
                report['finalization_errors'].append(name + ': ' + str(error)[:2048])
                try:
                    stream.close()
                except BaseException:
                    pass
        report['receipt'] = receipt_status(Path(request['run_root']), invocation, request)
        report['completed_ns'] = time.time_ns()
        report['producer_succeeded'] = (report['reaped'] and report['returncode'] == 0 and
            report['failure'] is None and report['cancel_reason'] is None and not report['finalization_errors'] and
            report['receipt'].get('valid_binding') is True and report['receipt'].get('local_complete') is True and
            report['receipt'].get('failure') is None)
        try:
            exclusive_json(invocation['terminal_record_path'], report)
        except BaseException as error:
            print(json.dumps({'terminal_capture_error': str(error)[:2048], 'observed_launch': report},
                             sort_keys=True), file=sys.stderr)
            raise
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
    return report


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument('--invocation', required=True)
    parser.add_argument('--sha256', required=True)
    args = parser.parse_args(argv)
    result = run(args.invocation, args.sha256)
    print(json.dumps(result, sort_keys=True))
    return 0 if result['producer_succeeded'] else 1


if __name__ == '__main__':
    sys.exit(main())
