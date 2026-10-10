"""Closed, bounded public lexical-phase probes; not ordinary parser evidence.

This current-only driver does not modify or reinterpret frozen helper failures.
Its executable path requires an authenticated outer authority and independently
reviewed actual layouts. Importing or testing this module launches no processes.
There is intentionally no command-line override, subset, or resume mode.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import subprocess
import time
import uuid


REQUEST_SCHEMA = 'oxid-unit4-lexer-phase-probe-request-v1'
RECEIPT_SCHEMA = 'oxid-unit4-lexer-phase-probe-receipt-v1'
INDEX_SCHEMA = 'oxid-unit4-lexer-phase-probe-index-v1'
ENTRYPOINT = 'frontend::parser::unit4_observer::lexer_phase::phase_controls'
ROOT_NAME = 'lexer-phase-controls'
DRIVER_PATH = 'tests/qualification/unit4_parser_current/lexer_phase_controls.py'
REQUEST_LIMIT = 2048
LAYOUT_LIMIT = 1024
RECEIPT_LIMIT = 4096
STREAM_LIMIT = 4096
INDEX_LIMIT = 16384
TIMEOUT_SECONDS = 30
PROCESS_COUNT = 54
FILE_COUNT = 217
EVIDENCE_LIMIT = 790528
PROFILES = ('debug', 'release')
ROLES = ('observed', 'not_observed')
OK_PREFIX = b'UNIT4_PHASE_PROBE_OK_V1 '
LAYOUT_PREFIX = b'UNIT4_PHASE_PROBE_LAYOUT_V1 '
ARMED_PREFIX = b'UNIT4_PHASE_PROBE_ARMED_V1 '
RESERVED_PREFIX = b'UNIT4_PHASE_PROBE_'
COUNTS = {'measured': 4, 'passed': 20, 'rejected_as_expected': 30}

# Catalogue order, then debug/release, then observed/not_observed, is normative.
# Entries are (probe, eligible roles, actual exit, receipt status, boundary).
CATALOGUE = (
    ('layout', ROLES, 0, 'measured', 'layout'),
    ('inactive_noop', ROLES, 0, 'passed', 'inactive'),
    ('lex_ok_seal', ROLES[:1], 0, 'passed', 'lex.ok'),
    ('lex_err_empty', ROLES[:1], 0, 'passed', 'lex.err'),
    ('lex_failed_terminal', ROLES[:1], 0, 'passed', 'lex.failed'),
    ('parser_forward', ROLES[:1], 0, 'passed', 'parser.forward'),
    ('parser_missing_position', ROLES[:1], 101, 'rejected_as_expected', 'parser.position'),
    ('nonlexer_in_lex', ROLES[:1], 101, 'rejected_as_expected', 'phase.nonlexer_lex'),
    ('lexer_in_parse', ROLES[:1], 101, 'rejected_as_expected', 'phase.lexer_parse'),
    ('unowned_active', ROLES[:1], 101, 'rejected_as_expected', 'phase.unowned'),
    ('nested_lex', ROLES[:1], 101, 'rejected_as_expected', 'phase.nested_lex'),
    ('parse_before_close', ROLES[:1], 101, 'rejected_as_expected', 'phase.unclosed_lex'),
    ('after_fail_reserve', ROLES[:1], 101, 'rejected_as_expected', 'phase.reserve_after_fail'),
    ('after_fail_token', ROLES[:1], 101, 'rejected_as_expected', 'phase.token_after_fail'),
    ('failed_then_ok', ROLES[:1], 101, 'rejected_as_expected', 'phase.failed_ok'),
    ('event_cap', ROLES[:1], 101, 'rejected_as_expected', 'cap.events'),
    ('byte_cap', ROLES[:1], 101, 'rejected_as_expected', 'cap.bytes'),
    ('serialize_max', ROLES[:1], 0, 'passed', 'serialize.max'),
    ('mode_order', ROLES[:1], 101, 'rejected_as_expected', 'phase.mode_order'),
    ('double_close', ROLES[:1], 101, 'rejected_as_expected', 'phase.double_close'),
    ('finished_callback', ROLES[:1], 101, 'rejected_as_expected', 'phase.finished'),
    ('control_not_observed', ROLES[1:], 0, 'passed', 'control.empty'),
    ('control_callback_reject', ROLES[1:], 101, 'rejected_as_expected', 'phase.control'),
    ('metadata_bounds', ROLES, 0, 'passed', 'metadata.bounds'),
)
LAYOUT_KEYS = (
    'row_bytes', 'row_align', 'rows_bytes', 'rows_align', 'ledger_bytes', 'ledger_align',
    'refcell_bytes', 'refcell_align', 'ref_bytes', 'ref_align', 'refmut_bytes', 'refmut_align',
    'token_bytes', 'token_align', 'capture_bytes', 'capture_align', 'serialize_bytes',
    'serialize_align', 'token_forward_bytes', 'token_forward_align', 'stdout_lock_bytes',
    'stdout_lock_align', 'frame_bytes', 'frame_align', 'named_total_bytes',
)
REQUEST_KEYS = ('schema', 'nonce', 'probe', 'profile', 'role', 'session_sha256',
                'authority_sha256', 'driver_sha256', 'binary', 'entrypoint', 'expected')
RECEIPT_KEYS = ('schema', 'nonce', 'probe', 'profile', 'role', 'request', 'status', 'exit_code',
                'boundary', 'started_ns', 'finished_ns', 'timed_out', 'stream_limit_exceeded',
                'argv_sha256', 'environment_sha256', 'cwd_sha256', 'stdout', 'stderr',
                'marker_count', 'layout', 'failure')
INDEX_KEYS = ('schema', 'session_sha256', 'authority_sha256', 'driver_sha256', 'counts', 'receipts')
FAILURES = ('spawn', 'timeout', 'stream_limit', 'exit', 'marker', 'rejection_text',
            'input_binding', 'layout', 'capture')


class Reject(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Reject(message)


def keys(value, expected, label):
    require(type(value) is dict and tuple(value) == tuple(expected), label + ' exact keys/order')


def integer(value, low, high, label):
    require(type(value) is int and low <= value <= high, label + ' integer range')


def hex_string(value, width, label):
    require(type(value) is str and re.fullmatch('[0-9a-f]{' + str(width) + '}', value) is not None,
            label + ' lowercase hexadecimal')


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical_bytes(value):
    return (json.dumps(value, ensure_ascii=True, separators=(',', ':'), allow_nan=False) + '\n').encode('ascii')


def projection_sha256(value):
    return sha256(json.dumps(value, sort_keys=True, ensure_ascii=False,
                            separators=(',', ':'), allow_nan=False).encode('utf-8'))


def decode(raw, cap):
    require(type(raw) is bytes and 1 <= len(raw) <= cap, 'bounded canonical JSON bytes')
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    try:
        value = json.loads(raw.decode('ascii'), object_pairs_hook=pairs,
                           parse_constant=lambda _: (_ for _ in ()).throw(Reject('nonfinite JSON')))
        require(canonical_bytes(value) == raw, 'noncanonical JSON bytes')
    except (UnicodeError, json.JSONDecodeError, TypeError, OverflowError) as error:
        raise Reject('invalid canonical JSON') from error
    return value


def relative_path(value, cap=128):
    require(type(value) is str and 1 <= len(value) <= cap and
            re.fullmatch('[A-Za-z0-9_./-]+', value) is not None and
            all(part not in ('', '.', '..') for part in value.split('/')), 'canonical relative path')
    return value


def validate_identity(value, *, path=None, cap=(1 << 64) - 1):
    keys(value, ('path', 'bytes', 'sha256'), 'identity')
    relative_path(value['path'])
    integer(value['bytes'], 0, cap, 'identity bytes')
    hex_string(value['sha256'], 64, 'identity sha256')
    if path is not None:
        require(value['path'] == path, 'identity at exact prescribed path')
    return value


def roster():
    return [(probe, profile, role) for probe, roles, _, _, _ in CATALOGUE
            for profile in PROFILES for role in roles]


def expected(probe, profile, role):
    require(profile in PROFILES and role in ROLES, 'unknown profile/role')
    matches = [row for row in CATALOGUE if row[0] == probe]
    require(len(matches) == 1 and role in matches[0][1], 'unknown/ineligible probe')
    row = matches[0]
    return {'exit_code': row[2], 'status': row[3], 'boundary': row[4]}


def process_dir(request):
    return ROOT_NAME + '/' + request['profile'] + '-' + request['role'] + '-' + request['probe']


def validate_request(raw):
    value = decode(raw, REQUEST_LIMIT)
    keys(value, REQUEST_KEYS, 'request')
    require(value['schema'] == REQUEST_SCHEMA, 'request schema')
    hex_string(value['nonce'], 32, 'request nonce')
    require(type(value['probe']) is str and len(value['probe']) <= 24, 'probe token bound')
    wanted = expected(value['probe'], value['profile'], value['role'])
    for name in ('session_sha256', 'authority_sha256', 'driver_sha256'):
        hex_string(value[name], 64, name)
    validate_identity(value['binary'])
    require(value['entrypoint'] == ENTRYPOINT, 'exact dedicated entrypoint')
    keys(value['expected'], ('exit_code', 'status', 'boundary'), 'expected')
    integer(value['expected']['exit_code'], 0, 101, 'expected exit')
    require(value['expected'] == wanted, 'fixed expected outcome')
    return value


def validate_catalogue(requests, *, session_sha256, authority_sha256, driver_sha256, binaries):
    require(type(requests) is list and len(requests) == PROCESS_COUNT, 'complete 54-request catalogue')
    for request in requests:
        validate_request(canonical_bytes(request))
    require([(r['probe'], r['profile'], r['role']) for r in requests] == roster(),
            'exact ordered catalogue, without duplicate/subset/extra requests')
    require(len({r['nonce'] for r in requests}) == PROCESS_COUNT, 'unique probe nonces')
    require(set(binaries) == {(p, r) for p in PROFILES for r in ROLES}, 'four verified binaries')
    for request in requests:
        for name, wanted in (('session_sha256', session_sha256),
                             ('authority_sha256', authority_sha256), ('driver_sha256', driver_sha256)):
            require(request[name] == wanted, 'request trusted ' + name)
        require(request['binary'] == binaries[(request['profile'], request['role'])],
                'request exact verified role/profile binary')


def inputs(root, request):
    root = Path(root)
    require(root.is_absolute() and str(root) == os.path.normpath(str(root)), 'absolute session root')
    validate_request(canonical_bytes(request))
    argv = [str(root / request['binary']['path']), ENTRYPOINT,
            '--exact', '--ignored', '--nocapture', '--test-threads=1']
    environment = {
        'HOME': str(root / 'home'), 'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8',
        'LC_ALL': 'C.UTF-8', 'PYTHONDONTWRITEBYTECODE': '1', 'PYTHONNOUSERSITE': '1',
        'UNIT4_PHASE_PROBE': request['probe'], 'UNIT4_PHASE_NONCE': request['nonce'],
        'UNIT4_PHASE_ROLE': request['role'], 'RUST_BACKTRACE': '0',
        'RUST_LIB_BACKTRACE': '0', 'RUST_TEST_THREADS': '1',
    }
    return argv, str(root), environment


def input_hashes(argv, cwd, environment):
    return {'argv_sha256': projection_sha256(argv),
            'environment_sha256': projection_sha256(environment),
            'cwd_sha256': sha256(cwd.encode('utf-8'))}


def validate_layout(value, approved=None):
    keys(value, LAYOUT_KEYS, 'layout')
    require(len(canonical_bytes(value)) <= LAYOUT_LIMIT, 'layout bytes bound')
    for name in LAYOUT_KEYS:
        integer(value[name], 0, 65535, 'layout ' + name)
        if name.endswith('_align'):
            require(value[name] != 0 and value[name] & (value[name] - 1) == 0,
                    'layout nonzero power-of-two alignment')
            require(value[name[:-6] + '_bytes'] % value[name] == 0, 'layout size/alignment')
    fixed = {'row_bytes': 8, 'row_align': 4, 'rows_bytes': 128, 'rows_align': 4,
             'ledger_bytes': 248, 'ledger_align': 4, 'refcell_bytes': 256, 'refcell_align': 8,
             'ref_bytes': 16, 'ref_align': 8, 'refmut_bytes': 16, 'refmut_align': 8,
             'token_bytes': 32, 'capture_bytes': 72, 'serialize_bytes': 120,
             'token_forward_bytes': 88}
    require(all(value[name] == want for name, want in fixed.items()), 'reviewed fixed layout sizes')
    alignment = max(8, value['stdout_lock_align'])
    frame = ((32 + value['stdout_lock_bytes'] + alignment - 1) // alignment) * alignment
    require(value['frame_align'] == alignment and value['frame_bytes'] == frame, 'typed frame layout')
    # Approved Token-argument accounting amendment prices the by-value input
    # independently of both TokenForwardCarriers fields; no optimizer discount.
    require(value['named_total_bytes'] == 2464 + frame + value['token_bytes'], 'named carrier accounting')
    if approved is not None:
        validate_layout(approved)
        require(value == approved, 'independently reviewed actual typed layout')
    return value


def validate_probe_output(request, stdout, stderr, exit_code, *, approved_layout=None):
    """Return (status, boundary, count, layout, failure), without manufacturing evidence."""
    require(type(stdout) is bytes and type(stderr) is bytes and
            len(stdout) <= STREAM_LIMIT and len(stderr) <= STREAM_LIMIT, 'retained stream limits')
    wanted = expected(request['probe'], request['profile'], request['role'])
    count = stdout.count(RESERVED_PREFIX)
    if exit_code != wanted['exit_code'] or type(exit_code) is not int:
        return 'failed', None, count, None, 'exit'
    forbidden = (b'UNIT4_EXECUTED', b'UNIT4_LEXER_PHASE_V1')
    if (count != 1 or RESERVED_PREFIX in stderr or
            any(word in stream for stream in (stdout, stderr) for word in forbidden)):
        return 'failed', None, count, None, 'marker'
    prefix = (LAYOUT_PREFIX if wanted['status'] == 'measured' else
              ARMED_PREFIX if wanted['status'] == 'rejected_as_expected' else OK_PREFIX)
    marker = prefix + request['nonce'].encode() + b' ' + request['probe'].encode() + b' ' + wanted['boundary'].encode()
    start = stdout.find(RESERVED_PREFIX)
    stop = stdout.find(b'\n', start)
    if stop == -1:
        return 'failed', None, count, None, 'marker'
    line = stdout[start:stop]
    layout = None
    if wanted['status'] == 'measured':
        if not line.startswith(marker + b' '):
            return 'failed', None, count, None, 'marker'
        try:
            layout = validate_layout(decode(line[len(marker) + 1:] + b'\n', LAYOUT_LIMIT), approved_layout)
        except Reject:
            return 'failed', None, count, None, 'layout'
    elif line != marker:
        return 'failed', None, count, None, 'marker'
    summaries = list(re.finditer(rb'test result: (?:ok|FAILED)\.[^\r\n]*', stdout))
    summary = (b'test result: FAILED. 0 passed; 1 failed; 0 ignored;' if wanted['exit_code'] == 101
               else b'test result: ok. 1 passed; 0 failed; 0 ignored;')
    if len(summaries) != 1 or not summaries[0].group().startswith(summary) or summaries[0].start() <= stop:
        return 'failed', None, count, None, 'exit'
    if wanted['status'] == 'rejected_as_expected':
        panic = (b'allocator event lacks actual parser position' if wanted['boundary'] == 'parser.position'
                 else b'UNIT4_PHASE_V1:' + wanted['boundary'].encode())
        if stderr.count(panic) != 1 or re.search(rb'(?:^|\n)' + re.escape(panic) + rb'(?:\n|$)', stderr) is None:
            return 'failed', None, count, None, 'rejection_text'
    return wanted['status'], wanted['boundary'], count, layout, None


def bounded_capture(argv, cwd, environment):
    """Read at most remaining allowance plus one detection byte from each pipe.

    The detection byte is never retained. A failed capture is forensic only;
    neither truncation, exit 101, nor an armed marker can make it admissible.
    """
    started = time.time_ns()
    deadline = time.monotonic() + TIMEOUT_SECONDS
    result = {'started_ns': started, 'finished_ns': started, 'exit_code': None,
              'timed_out': False, 'stream_limit_exceeded': False,
              'stdout': b'', 'stderr': b'', 'failure': None}
    buffers = {'stdout': bytearray(), 'stderr': bytearray()}
    child = None
    selector = selectors.DefaultSelector()
    try:
        try:
            child = subprocess.Popen(argv, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     start_new_session=True, close_fds=True)
        except OSError:
            result['failure'] = 'spawn'
            return result
        for name, stream in (('stdout', child.stdout), ('stderr', child.stderr)):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                result.update(timed_out=True, failure='timeout')
                break
            for key, _ in selector.select(remaining):
                target = buffers[key.data]
                allowance = STREAM_LIMIT - len(target)
                chunk = os.read(key.fd, allowance + 1)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                target.extend(chunk[:allowance])
                if len(chunk) > allowance:
                    result.update(stream_limit_exceeded=True, failure='stream_limit')
                    break
            if result['failure'] is not None:
                break
        if result['failure'] is None:
            try:
                child.wait(timeout=max(0, deadline - time.monotonic()))
            except subprocess.TimeoutExpired:
                result.update(timed_out=True, failure='timeout')
    except (OSError, ValueError):
        result['failure'] = 'capture'
    finally:
        selector.close()
        if child is not None:
            if result['failure'] is not None or child.returncode is None:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            child.wait()
            result['exit_code'] = child.returncode
            child.stdout.close()
            child.stderr.close()
        result['stdout'] = bytes(buffers['stdout'])
        result['stderr'] = bytes(buffers['stderr'])
        result['finished_ns'] = time.time_ns()
    return result


def regular(path):
    path = Path(path)
    require(path.is_absolute() and not any(p.is_symlink() for p in (path, *path.parents)) and
            path.is_file(), 'regular file with nonsymlink ancestry')


def read_bounded(path, cap):
    regular(path)
    with Path(path).open('rb') as stream:
        raw = stream.read(cap + 1)
    require(len(raw) <= cap, 'file exceeds protocol cap')
    return raw


def identity(root, relative, raw):
    relative_path(relative)
    return {'path': relative, 'bytes': len(raw), 'sha256': sha256(raw)}


def write_new(root, relative, raw, cap):
    relative_path(relative)
    require(type(raw) is bytes and len(raw) <= cap, 'write exceeds protocol cap')
    path = Path(root) / relative
    require(not any(p.is_symlink() for p in (path, *path.parents)), 'no symlink output')
    with path.open('xb') as stream:
        require(stream.write(raw) == len(raw), 'complete bounded file write')
    return identity(root, relative, raw)


def validate_receipt(raw, request, stdout, stderr, root, *, approved_layout, prepared_ns=0):
    value = decode(raw, RECEIPT_LIMIT)
    keys(value, RECEIPT_KEYS, 'receipt')
    require(value['schema'] == RECEIPT_SCHEMA, 'receipt schema')
    require(type(value['status']) is str and value['status'] in (*COUNTS, 'failed'), 'closed receipt status')
    for name in ('nonce', 'probe', 'profile', 'role'):
        require(value[name] == request[name], 'receipt/request ' + name)
    directory = process_dir(request)
    validate_identity(value['request'], path=directory + '/request.json', cap=REQUEST_LIMIT)
    require(value['request'] == identity(root, directory + '/request.json', canonical_bytes(request)),
            'exact request bytes identity')
    for name in ('stdout', 'stderr'):
        validate_identity(value[name], path=directory + '/' + name, cap=STREAM_LIMIT)
        raw_stream = stdout if name == 'stdout' else stderr
        require(value[name] == identity(root, directory + '/' + name, raw_stream), 'retained stream identity')
    for name in ('started_ns', 'finished_ns'):
        integer(value[name], 0, (1 << 64) - 1, name)
    require(prepared_ns <= value['started_ns'] <= value['finished_ns'], 'probe timestamp ordering')
    for name in ('timed_out', 'stream_limit_exceeded'):
        require(type(value[name]) is bool, name + ' boolean')
    if value['exit_code'] is not None:
        integer(value['exit_code'], -255, 255, 'actual exit')
    for name, wanted in input_hashes(*inputs(root, request)).items():
        hex_string(value[name], 64, name)
        require(value[name] == wanted, 'actual supplied input binding')
    integer(value['marker_count'], 0, 256, 'marker count')
    require(value['marker_count'] == stdout.count(RESERVED_PREFIX), 'actual marker count')
    if value['status'] == 'failed':
        require(value['failure'] in FAILURES and value['boundary'] is None and value['layout'] is None,
                'failed receipt retains failure, no successful claim')
        require(not value['timed_out'] or value['failure'] == 'timeout', 'timeout failure reason')
        require(not value['stream_limit_exceeded'] or value['failure'] == 'stream_limit', 'stream failure reason')
        return value
    require(not value['timed_out'] and not value['stream_limit_exceeded'] and value['failure'] is None,
            'admission requires complete bounded capture')
    outcome = validate_probe_output(request, stdout, stderr, value['exit_code'], approved_layout=approved_layout)
    require(outcome[0] != 'failed' and
            (value['status'], value['boundary'], value['marker_count'], value['layout'], value['failure']) == outcome,
            'receipt exact actual protocol outcome')
    return value


def verify(root, *, session_sha256, authority_sha256, driver_sha256, binaries,
           approved_layouts, prepared_ns=0, resolve=None, inventory=None):
    """Verify full 217-file evidence, using exact transported identities if supplied.

    The caller authenticates session/authority/build receipts before this extra
    conjunct. ``resolve`` receives an absolute path/bytes/sha256 identity.
    ``inventory`` is the exact absolute identity mapping for this control root.
    """
    root = Path(root)
    require(root.is_absolute(), 'absolute verified session root')
    require(type(binaries) is dict and set(binaries) == {(p, r) for p in PROFILES for r in ROLES},
            'four verified binary roles/profiles')
    for binary in binaries.values():
        validate_identity(binary)
    require(len({row['sha256'] for row in binaries.values()}) == 4, 'four distinct actual build binaries')
    for digest in (session_sha256, authority_sha256, driver_sha256):
        hex_string(digest, 64, 'trusted digest')
    require(type(approved_layouts) is dict and
            set(approved_layouts) == {row['sha256'] for row in binaries.values()},
            'reviewed layouts bind every actual binary identity')
    for layout in approved_layouts.values():
        validate_layout(layout)
    control_root = root / ROOT_NAME
    names = {ROOT_NAME + '/probe-index.json'}
    for probe, profile, role in roster():
        directory = ROOT_NAME + '/' + profile + '-' + role + '-' + probe
        names.update(directory + '/' + filename for filename in ('request.json', 'receipt.json', 'stdout', 'stderr'))
    absolute_names = {str(root / name) for name in names}
    if inventory is None:
        require(resolve is None, 'transport resolver requires exact inventory')
        require(control_root.is_dir() and not control_root.is_symlink(), 'control evidence root')
        actual = set()
        directories = {str(control_root)}
        directories.update(str((root / name).parent) for name in names)
        for path in control_root.rglob('*'):
            require(not path.is_symlink(), 'symlink control evidence')
            if path.is_dir():
                require(str(path) in directories, 'extra control directory')
            else:
                regular(path)
                actual.add(str(path))
        require(actual == absolute_names, 'exact 217-file control inventory')
    else:
        require(type(inventory) is dict and set(inventory) == absolute_names and resolve is not None,
                'exact transported 217-file inventory')

    def fetch(name, cap, record=None):
        absolute = str(root / name)
        if inventory is None:
            data = read_bounded(absolute, cap)
        else:
            entry = inventory[absolute]
            keys(entry, ('path', 'bytes', 'sha256'), 'transport identity')
            require(entry['path'] == absolute, 'transport exact absolute identity path')
            integer(entry['bytes'], 0, cap, 'transport bytes')
            hex_string(entry['sha256'], 64, 'transport digest')
            data = resolve(entry)
            require(type(data) is bytes and len(data) == entry['bytes'] and sha256(data) == entry['sha256'],
                    'authenticated transported bytes')
        require(len(data) <= cap, 'bound transported data')
        if record is not None:
            validate_identity(record, path=name, cap=cap)
            require(identity(root, name, data) == record, 'artifact exact bytes/hash')
        return data

    index = decode(fetch(ROOT_NAME + '/probe-index.json', INDEX_LIMIT), INDEX_LIMIT)
    keys(index, INDEX_KEYS, 'index')
    require(index['schema'] == INDEX_SCHEMA, 'index schema')
    for name, wanted in (('session_sha256', session_sha256), ('authority_sha256', authority_sha256),
                         ('driver_sha256', driver_sha256)):
        require(index[name] == wanted, 'index trusted ' + name)
    keys(index['counts'], COUNTS, 'index counts')
    for name, count in COUNTS.items():
        integer(index['counts'][name], 0, PROCESS_COUNT, 'index count')
        require(index['counts'][name] == count, 'index complete status counts')
    require(type(index['receipts']) is list and len(index['receipts']) == PROCESS_COUNT, 'index exact 54 receipts')
    requests, counts, total = [], dict.fromkeys(COUNTS, 0), len(canonical_bytes(index))
    previous_finish = prepared_ns
    for (probe, profile, role), record in zip(roster(), index['receipts']):
        directory = ROOT_NAME + '/' + profile + '-' + role + '-' + probe
        request_raw = fetch(directory + '/request.json', REQUEST_LIMIT)
        request = validate_request(request_raw)
        require((request['probe'], request['profile'], request['role']) == (probe, profile, role),
                'request at exact catalogue directory')
        requests.append(request)
        receipt_raw = fetch(directory + '/receipt.json', RECEIPT_LIMIT, record)
        stdout = fetch(directory + '/stdout', STREAM_LIMIT)
        stderr = fetch(directory + '/stderr', STREAM_LIMIT)
        receipt = validate_receipt(receipt_raw, request, stdout, stderr, root,
                                   approved_layout=approved_layouts[binaries[(profile, role)]['sha256']],
                                   prepared_ns=previous_finish)
        require(receipt['status'] in COUNTS, 'failed receipt cannot enter success index')
        counts[receipt['status']] += 1
        previous_finish = receipt['finished_ns']
        total += sum(map(len, (request_raw, receipt_raw, stdout, stderr)))
    validate_catalogue(requests, session_sha256=session_sha256, authority_sha256=authority_sha256,
                       driver_sha256=driver_sha256, binaries=binaries)
    require(counts == COUNTS and len(names) == FILE_COUNT and total <= EVIDENCE_LIMIT,
            'complete bounded control evidence')
    return index


def execute(api, session_path, a, binding):
    """Execute a fresh full catalogue only under the trusted outer binding.

    ``api.phase_control_binding(a)`` must authenticate the reviewed outer
    authority and expose exactly authority_sha256, driver, approved_layouts.
    Measurement-only work is separate and cannot create this success index.
    """
    keys(binding, ('authority_sha256', 'driver', 'approved_layouts'), 'trusted control binding')
    require(binding == api.phase_control_binding(a), 'outer authority control binding')
    hex_string(binding['authority_sha256'], 64, 'outer authority digest')
    own = Path(__file__).resolve()
    driver = binding['driver']
    validate_identity(driver, path=DRIVER_PATH)
    require(Path(api.REPOSITORY) / DRIVER_PATH == own, 'reviewed repository driver path')
    regular(own)
    own_bytes = own.read_bytes()
    require(driver['bytes'] == len(own_bytes) and driver['sha256'] == sha256(own_bytes),
            'reviewed actual driver bytes')
    session, root = api.session_at(session_path, a)
    root = Path(root)
    regular(root / 'session.json')
    session_sha = sha256((root / 'session.json').read_bytes())
    binaries = {}
    prepared_ns = session['prepared_ns']
    for profile in PROFILES:
        for role in ROLES:
            build, native = api.verify_build(root, session_path, profile, a, control=role == 'not_observed')
            binary = build['binary']
            api.artifact(binary)
            try:
                relative = Path(binary['path']).relative_to(root).as_posix()
            except ValueError as error:
                raise Reject('binary outside verified session') from error
            entry = {'path': relative, 'bytes': binary['bytes'], 'sha256': binary['sha256']}
            validate_identity(entry)
            binaries[(profile, role)] = entry
            invocation = api.load(api.artifact(native['invocation']))
            prepared_ns = max(prepared_ns, invocation['finished_ns'])
    require(len({row['sha256'] for row in binaries.values()}) == 4, 'four distinct actual build binaries')
    approved = binding['approved_layouts']
    require(type(approved) is dict and set(approved) ==
            {profile + '-' + role for profile in PROFILES for role in ROLES},
            'four independently reviewed profile-role layout values required')
    for layout in approved.values():
        validate_layout(layout)
    layouts = {binary['sha256']: approved[profile + '-' + role]
               for (profile, role), binary in binaries.items()}
    requests = []
    for probe, profile, role in roster():
        request = {'schema': REQUEST_SCHEMA, 'nonce': uuid.uuid4().hex, 'probe': probe,
                   'profile': profile, 'role': role, 'session_sha256': session_sha,
                   'authority_sha256': binding['authority_sha256'], 'driver_sha256': driver['sha256'],
                   'binary': binaries[(profile, role)], 'entrypoint': ENTRYPOINT,
                   'expected': expected(probe, profile, role)}
        validate_request(canonical_bytes(request))
        requests.append(request)
    validate_catalogue(requests, session_sha256=session_sha, authority_sha256=binding['authority_sha256'],
                       driver_sha256=driver['sha256'], binaries=binaries)
    out = root / ROOT_NAME
    require(not out.exists() and not out.is_symlink() and
            not any(path.is_symlink() for path in (root, *root.parents)), 'fresh safe control root')
    out.mkdir()
    receipts = []
    for request in requests:
        directory = process_dir(request)
        (root / directory).mkdir()
        request_identity = write_new(root, directory + '/request.json', canonical_bytes(request), REQUEST_LIMIT)
        argv, cwd, environment = inputs(root, request)
        supplied_hashes = input_hashes(argv, cwd, environment)
        result = bounded_capture(argv, cwd, environment)
        stdout, stderr = result['stdout'], result['stderr']
        stdout_identity = write_new(root, directory + '/stdout', stdout, STREAM_LIMIT)
        stderr_identity = write_new(root, directory + '/stderr', stderr, STREAM_LIMIT)
        failure = result['failure']
        outcome = ('failed', None, stdout.count(RESERVED_PREFIX), None, failure)
        if failure is None:
            outcome = validate_probe_output(request, stdout, stderr, result['exit_code'],
                                            approved_layout=layouts[request['binary']['sha256']])
        if supplied_hashes != input_hashes(*inputs(root, request)):
            outcome = ('failed', None, stdout.count(RESERVED_PREFIX), None, 'input_binding')
        status, boundary, count, layout, failure = outcome
        receipt = {'schema': RECEIPT_SCHEMA, **{name: request[name] for name in ('nonce', 'probe', 'profile', 'role')},
                   'request': request_identity, 'status': status, 'exit_code': result['exit_code'],
                   'boundary': boundary, 'started_ns': result['started_ns'], 'finished_ns': result['finished_ns'],
                   'timed_out': result['timed_out'], 'stream_limit_exceeded': result['stream_limit_exceeded'],
                   **supplied_hashes, 'stdout': stdout_identity, 'stderr': stderr_identity,
                   'marker_count': count, 'layout': layout, 'failure': failure}
        raw = canonical_bytes(receipt)
        validate_receipt(raw, request, stdout, stderr, root,
                         approved_layout=layouts[request['binary']['sha256']], prepared_ns=prepared_ns)
        receipts.append(write_new(root, directory + '/receipt.json', raw, RECEIPT_LIMIT))
        require(status != 'failed', 'probe failed; bounded forensic files retained, no success index')
        prepared_ns = result['finished_ns']
    index = {'schema': INDEX_SCHEMA, 'session_sha256': session_sha,
             'authority_sha256': binding['authority_sha256'], 'driver_sha256': driver['sha256'],
             'counts': dict(COUNTS), 'receipts': receipts}
    write_new(root, ROOT_NAME + '/probe-index.json', canonical_bytes(index), INDEX_LIMIT)
    return verify(root, session_sha256=session_sha, authority_sha256=binding['authority_sha256'],
                  driver_sha256=driver['sha256'], binaries=binaries,
                  approved_layouts=layouts, prepared_ns=session['prepared_ns'])
