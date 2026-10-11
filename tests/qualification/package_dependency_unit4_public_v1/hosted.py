"""Explicit current-source Linux capability transport; import performs no work.

A child receipt is a partial observation, never an execution admission. Only the
original live public admission plus the original owned worker probe can validate
it. No historical controller, binding projection, or monkeypatch is used.
The dormant process entrypoints require a separately released execution route.
"""
import argparse
import copy
import contextvars
import errno
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import stat
import sys
import time
import types
import weakref

sys.dont_write_bytecode = True
CAPABILITY = 'posix_regular_file_mode_denies_open_for_effective_identity'
SOURCE_SHA = 'f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
HOST = 'Linux x86_64'
SECTIONS = ('public', 'lifecycle')
PROFILES = ('debug', 'release')
STREAM_LIMIT = 8 * 1024 ** 2
WORKER_TIMEOUT = 600
MAX_LEDGER_MEMBERS = 4096
MAX_LEDGER_BYTES = 512 * 1024 ** 2
MAX_FILE_BYTES = 512 * 1024 ** 2
MAX_OUTPUT_BYTES = 128 * 1024 ** 2
RETAINED_HOSTED = {
    'README.md': (4108, '610136e600feb491a07804af54e4ab7518633bb71c341b1261f8d0cfaa0c6c3b'),
    'hosted.py': (40016, '67ac926f39fa4d01430e6ac5940117aeb8d9b3c352260843d305d33955a8164f'),
    'integration_controls.py': (14631, '02309821e2d7ee79f20cabc0c832e1de34f14802a9ae3b116eefeb050afcb590'),
    'selftest.py': (9175, 'cf88c30d66c7863b7643073a7ea104bbd95b246e96afb1aff48c0524e448a4ba'),
}
_TRANSPORTS = weakref.WeakKeyDictionary()
_WORKERS = weakref.WeakKeyDictionary()
_RESULTS = weakref.WeakKeyDictionary()
_CHECKPOINT = contextvars.ContextVar("hosted_checkpoint", default=None)
_WORKER_DEADLINE = contextvars.ContextVar("hosted_worker_deadline", default=None)


class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def _tick():
    check = _CHECKPOINT.get()
    if check is not None:
        check()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate hosted JSON key')
            result[key] = value
        return result
    def invalid(_):
        raise Rejected('nonfinite hosted JSON')
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=invalid)


def clone(value):
    return decode(canonical(value))


def exact_path(path, *, exists=True):
    path = Path(path)
    need(path.is_absolute() and str(path) == str(path.absolute()) and
         path.resolve(strict=exists) == path, 'canonical absolute hosted path required')
    need(not any(part.is_symlink() for part in (path, *path.parents)), 'hosted symlink ancestry')
    return path


def file_identity(path):
    _tick()
    path = exact_path(path)
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and before.st_size <= MAX_FILE_BYTES, 'bounded regular hosted input')
    digest, count = hashlib.sha256(), 0
    with path.open('rb') as stream:
        while True:
            _tick()
            chunk = stream.read(1024 * 1024)
            if not chunk:
                break
            count += len(chunk)
            need(count <= MAX_FILE_BYTES, 'hosted file grew beyond limit')
            digest.update(chunk)
    after = path.stat()
    need(before == after and count == before.st_size, 'hosted input changed while reading')
    return {'path': str(path), 'bytes': count, 'sha256': digest.hexdigest()}


def _stat(path):
    value = exact_path(path).stat()
    return {name: getattr(value, name) for name in
            ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')}


def _read(path, maximum=MAX_OUTPUT_BYTES):
    identity = file_identity(path)
    need(identity['bytes'] <= maximum, 'hosted document bound')
    raw = Path(path).read_bytes()
    need(len(raw) == identity['bytes'] and sha(raw) == identity['sha256'], 'hosted document changed')
    return decode(raw)


def _write(path, value, *, readonly=False):
    path = exact_path(path, exists=False)
    raw = canonical(value)
    need(len(raw) <= MAX_OUTPUT_BYTES and not path.exists(), 'fresh bounded hosted carrier')
    with path.open('xb') as stream:
        stream.write(raw)
    if readonly:
        path.chmod(0o444)
    return file_identity(path)


def _runtime():
    """Load through the new package owner; never alias a historical module."""
    root = Path(__file__).absolute().parent
    manifest = _read(root / 'package-manifest.json')
    rows = {row['path']: row for row in manifest['files']}
    for name in ('binding.py', 'hosted.py', 'runtime.py'):
        actual = file_identity(root / name)
        need(rows[name] == {**actual, 'path': name}, 'new hosted package body changed')
    body = (root / 'binding.py').read_bytes()
    module = types.ModuleType('_package_public_hosted_binding_loader')
    module.__file__ = str(root / 'binding.py')
    exec(compile(body, module.__file__, 'exec'), module.__dict__)
    return module.runtime_api()


def _entry(value, cls, registry):
    need(type(value) is cls and value in registry, 'original live ' + cls.__name__ + ' required')
    return registry[value]


class TransportAdmission:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise TypeError('TransportAdmission is issued only by prepare_transport')
    def document(self):
        return clone(_entry(self, TransportAdmission, _TRANSPORTS)['ledger'])


class WorkerAdmission:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise TypeError('WorkerAdmission is issued only by the bounded worker entrypoint')
    def document(self):
        return clone(_entry(self, WorkerAdmission, _WORKERS)['ledger'])
    def inputs(self):
        return verify_worker(self)
    def process(self, argv, cwd, environment, timeout=120, limit=STREAM_LIMIT):
        return _worker_process(self, argv, cwd, environment, timeout, limit)


class HostedResult:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise TypeError('HostedResult is issued only after an owned worker process')
    def document(self):
        return clone(_entry(self, HostedResult, _RESULTS)['document'])


def selected_rosters(contracts):
    rows = {section: [row for row in contracts.roster(section, HOST)
                      if row['scope'] == 'execute' and row['required_capabilities'] == [CAPABILITY]]
            for section in SECTIONS}
    need({section: len(values) for section, values in rows.items()} == {'public': 6, 'lifecycle': 6},
         'exact frozen six-public six-lifecycle capability rows required')
    for section, values in rows.items():
        need(len({row['key'] for row in values}) == 6, 'duplicate frozen capability tuple')
        need({row['profile'] for row in values} == set(PROFILES) and
             all(row['execution_host'] == row['host'] == HOST for row in values), 'actual Linux capability roster')
        for row in values:
            setup = contracts.cases[row['case_id']]['filesystem_setup']
            need(int(setup['temporary_mode_octal'], 8) == 0, 'original mode-zero filesystem control required')
    return rows


def replacement_map(required_keys, candidates):
    need(len(required_keys) == 6 and len(set(required_keys)) == 6, 'exact six-row section domain')
    keys = [row['key'] for row in candidates]
    need(len(keys) == 6 and len(set(keys)) == 6 and set(keys) == set(required_keys),
         'missing/extra/duplicate capability replacement')
    need(all(row.get('executed') is True and row.get('status') == 'pass' for row in candidates),
         'capability replacement did not execute and pass')
    return {row['key']: row for row in candidates}


def require_denial_proof(receipt, identity):
    proof = receipt.get('capability_proof', {})
    need(type(identity.get('effective_uid')) is int and identity['effective_uid'] > 0 and
         type(identity.get('effective_gid')) is int and identity['effective_gid'] >= 0 and
         identity.get('supplementary_groups') == [], 'genuine nonroot worker identity')
    need(proof.get('read_open_succeeded') is False and
         type(proof.get('effective_uid')) is int and proof['effective_uid'] == identity['effective_uid'] and
         type(proof.get('effective_gid')) is int and proof['effective_gid'] == identity['effective_gid'] and
         proof.get('groups') == [] and type(proof.get('error_errno')) is int and
         proof['error_errno'] in (errno.EACCES, errno.EPERM) and
         proof.get('mode_before_candidate') == proof.get('mode_after_candidate') == 0 and
         proof.get('source_restored') is True, 'actual nonroot read-denial and restoration proof required')


def namespace_path(root, logical):
    logical = Path(logical)
    need(logical.is_absolute() and '..' not in logical.parts and str(logical) != '/', 'exact logical input path')
    return Path(root) / 'readonly' / str(logical).lstrip('/')


def verify_ledger(ledger, *, physical=True):
    need(ledger['schema'] == 'package-dependency-unit4-public-hosted-transport-v1' and
         ledger['status'] == 'NotReady' and ledger['execution_qualified'] is False,
         'hosted transport is a nonqualifying input carrier')
    need(ledger['source_manifest_sha256'] == SOURCE_SHA and ledger['host'] == HOST,
         'current-source Linux hosted transport required')
    root = exact_path(ledger['root'])
    need(ledger['output'] == str(root / 'observations'), 'fixed hosted output route')
    entries = ledger['entries']
    logical = [row['logical_path'] for row in entries]
    need(0 < len(entries) <= MAX_LEDGER_MEMBERS and logical == sorted(set(logical)),
         'closed sorted unique hosted input ledger')
    need(all(type(row['bytes']) is int and 0 <= row['bytes'] <= MAX_FILE_BYTES for row in entries) and
         sum(row['bytes'] for row in entries) <= MAX_LEDGER_BYTES, 'readonly hosted byte bound')
    files, directories = set(), {str(root / 'readonly')}
    for row in entries:
        _tick()
        destination = namespace_path(root, row['logical_path'])
        need(row['physical_path'] == str(destination) and row['mode'] in (0o444, 0o555),
             'exact readonly transport path and mode')
        need(isinstance(row['roles'], list) and row['roles'] == sorted(set(row['roles'])) and row['roles'],
             'exact nonempty input roles')
        files.add(str(destination))
        directories.update(str(parent) for parent in destination.parents if parent == root / 'readonly' or root / 'readonly' in parent.parents)
        if physical:
            observed = file_identity(destination)
            need(observed == {'path': str(destination), 'bytes': row['bytes'], 'sha256': row['sha256']} and
                 stat.S_IMODE(destination.stat().st_mode) == row['mode'], 'readonly body or mode changed')
    if physical:
        observed_files, observed_directories = set(), {str(root / 'readonly')}
        for path in (root / 'readonly').rglob('*'):
            _tick()
            need(not path.is_symlink(), 'readonly transport symlink')
            if path.is_file():
                observed_files.add(str(path))
            else:
                need(path.is_dir(), 'special readonly transport member')
                observed_directories.add(str(path))
        need(observed_files == files and observed_directories == directories, 'exact readonly file and directory membership')
        need(all(stat.S_IMODE(Path(path).stat().st_mode) == 0o555 for path in directories),
             'readonly directory permissions changed')
    return {row['logical_path']: row for row in entries}


def _resolver(ledger):
    mapping = verify_ledger(ledger)
    def resolve(logical):
        need(str(logical) in mapping, 'unlisted readonly input')
        return mapping[str(logical)]['physical_path']
    return resolve


def transported_configs(cfgs, resolve, ledger_binding):
    need(set(cfgs) == {'ordinary', 'lifecycle'}, 'complete hosted role pair')
    result = clone(cfgs)
    for role, cfg in result.items():
        need(cfg['role'] == role and cfg['kind'] == 'package-dependency-unit4-public-' + role + '-v1',
             'new source role identity, never historical kind')
        build = cfg['build_binding']
        need(build['source_binding']['source_manifest']['sha256'] == SOURCE_SHA and
             build['role'] == role and build['host'] == HOST, 'actual current source build association')
        need(set(cfg['binaries']) == set(PROFILES), 'both hosted profiles')
        cfg['transport'] = {'logical_binding_sha256': cfg['_sha256'],
                            'logical_binaries': clone(cfg['binaries']), 'readonly_ledger': ledger_binding}
        for profile, binary in cfg['binaries'].items():
            physical = resolve(binary['path'])
            observed = file_identity(physical)
            need((observed['bytes'], observed['sha256']) == (binary['bytes'], binary['sha256']),
                 'transported compiler bytes changed')
            cfg['binaries'][profile] = observed
            cfg['_binary_stats'][profile] = _stat(physical)
    return result



def outer_environment(output):
    return {'HOME': str(output), 'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8',
            'PYTHONDONTWRITEBYTECODE': '1', 'PYTHONNOUSERSITE': '1'}


def worker_argv(setpriv, python, helper, ledger, ledger_sha, output, uid, gid):
    need(type(uid) is int and uid > 0 and type(gid) is int and gid >= 0, 'existing nonroot identity required')
    need(isinstance(ledger_sha, str) and len(ledger_sha) == 64 and all(c in '0123456789abcdef' for c in ledger_sha),
         'exact readonly ledger hash')
    return [str(setpriv), '--reuid=' + str(uid), '--regid=' + str(gid), '--clear-groups', '--no-new-privs',
            str(python), '-B', str(helper), 'worker', '--ledger', str(ledger), '--ledger-sha256', ledger_sha,
            '--out', str(output), '--expected-uid', str(uid), '--expected-gid', str(gid)]


def inspect_worker_process(process, argv, cwd, environment, invocation):
    need(process['argv'] == argv and process['cwd'] == str(cwd) and process['environment'] == environment,
         'exact hosted worker command and environment')
    need(type(process['pid']) is int and process['pid'] > 0 and type(process['status']) is int and process['status'] == 0,
         'successful real hosted worker pid/status')
    need(process['timed_out'] is False and process['stream_limit_exceeded'] is False and
         process['cleanup_complete'] is True and process['timeout_seconds'] == WORKER_TIMEOUT and
         process['stream_limit_bytes'] == STREAM_LIMIT, 'complete bounded owned hosted worker')
    need(invocation['started_ns'] <= process['started_ns'] <= process['completed_ns'] and
         process['completed_ns'] - process['started_ns'] <= WORKER_TIMEOUT * 10 ** 9,
         'hosted process outside live invocation window')
    for stream in ('stdout', 'stderr'):
        need(isinstance(process[stream], str) and len(process[stream].encode()) <= STREAM_LIMIT and
             sha(process[stream].encode()) == process[stream + '_sha256'], 'complete hosted outer streams')
    return True


def inspect_nested_process(process, worker_process, identity):
    group = identity['process_group_policy']
    need(group == {'identity': 'outer-owned-session-shared-nested-v1', 'worker_pid': worker_process['pid'],
                   'process_group': worker_process['pid'], 'session_id': worker_process['pid'],
                   'nested_new_sessions': False}, 'outer-owned hosted process group')
    need(process['worker_pid'] == process['process_group'] == process['session_id'] == worker_process['pid'] and
         process['nested_new_session'] is False and process['pipe_capture_complete'] is True and
         process['outer_cleanup_required'] is True and 'cleanup_complete' not in process and
         worker_process['cleanup_complete'] is True,
         'nested process escaped owned worker group')
    need(type(process['pid']) is int and process['pid'] > 0 and type(process['status']) is int and
         process['pid'] != worker_process['pid'] and
         worker_process['started_ns'] <= process['started_ns'] <= process['completed_ns'] <= worker_process['completed_ns'],
         'nested process outside actual worker window')
    need(process['timed_out'] is False and process['stream_limit_exceeded'] is False and
         process['stream_limit_bytes'] == STREAM_LIMIT, 'complete bounded nested process')
    for stream in ('stdout', 'stderr'):
        need(isinstance(process[stream], str) and len(process[stream].encode()) <= STREAM_LIMIT and
             sha(process[stream].encode()) == process[stream + '_sha256'], 'nested raw stream identity')
    return True


def _input_inventory(state, traps):
    """Derive the closed copy set from live owners, never a caller file list."""
    paths = {}
    def add(path, role, bound=None):
        path = exact_path(path)
        actual = file_identity(path)
        if bound is not None:
            need(actual['bytes'] == bound['bytes'] and actual['sha256'] == bound['sha256'], 'hosted original input drift')
        value = paths.setdefault(str(path), {'identity': actual, 'roles': set()})
        need(value['identity'] == actual, 'hosted duplicate input drift')
        value['roles'].add(role)
    repository = Path(state['input_paths']['repository'])
    runtime = _runtime()
    for row in runtime.retained_helper_inputs(repository):
        add(row['path'], 'retained-helper', row)
    old_hosted = repository / 'tests/qualification/unit4_hosted_capability'
    need(sorted(p.name for p in old_hosted.iterdir()) == sorted(RETAINED_HOSTED), 'all four retained hosted members')
    for name, (size, digest) in RETAINED_HOSTED.items():
        add(old_hosted / name, 'retained-hosted-authority', {'bytes': size, 'sha256': digest})
    consumer = Path(__file__).absolute().parent
    manifest = _read(consumer / 'package-manifest.json')
    add(consumer / 'package-manifest.json', 'current-consumer-manifest')
    need(sorted(p.name for p in consumer.iterdir()) == sorted([row['path'] for row in manifest['files']] + ['package-manifest.json']),
         'closed current consumer for hosted transport')
    for row in manifest['files']:
        add(consumer / row['path'], 'current-consumer', row)
    for role, cfg in state['cfgs'].items():
        document = cfg['build_binding']
        source = document['source_binding']
        need(source['source_manifest']['sha256'] == SOURCE_SHA, 'current source required in live build')
        view = source['view']
        for row in view['files']:
            add(Path(view['path']) / row['path'], role + '-source', row)
        add(Path(view['path']) / 'package-preparation-overlay.json', role + '-overlay')
        for key in ('plan', 'source_carrier', 'machine_carrier'):
            add(document[key]['path'], role + '-' + key, document[key])
        add(Path(document['plan']['path']).parent / 'candidate-binding.json', role + '-build-binding')
        for profile, binary in cfg['binaries'].items():
            add(binary['path'], role + '-binary-' + profile, binary)
        for key in ('source_manifest',):
            value = source[key]
            path = Path(value['path'])
            add(path if path.is_absolute() else repository / path, role + '-source-manifest', value)
    contracts = state['contracts']
    add(contracts.root / 'PUBLIC-FREEZE-v3.json', 'contract-freeze')
    for bound in contracts.verified:
        add(bound['path'], 'contract-authority', bound)
    for case in contracts.cases.values():
        for row in case['source_files']:
            add(contracts.root / case['source_root'] / row['path'], 'contract-fixture', row)
    # All immutable external locations needed by Contracts were verified above.
    # A path-map key does not authorize copying an unrelated arbitrary file.
    for name in ('public-cli-contract-replacement-v3.json', 'original-compatibility-contract-replacement-v3.json.gz',
                 'predecessor-public-projections-replacement-v3.json.gz', 'lifecycle-contract-replacement-v3.json.gz',
                 'output-guards-contract-replacement-v3.json', 'authority-bindings-replacement-v3.json',
                 'pilot-semantic-derivations-v3.json'):
        add(contracts.root / name, 'contract-document')
    trap = traps.document()['trap_build']
    for key in ('source', 'binary'):
        add(trap[key]['path'], 'capability-trap-' + key, trap[key])
    need(len(paths) <= MAX_LEDGER_MEMBERS and sum(v['identity']['bytes'] for v in paths.values()) <= MAX_LEDGER_BYTES,
         'finite readonly input inventory bound')
    return paths


def _tools_api(runtime):
    return runtime._binding().machine_api()


def _prepare_transport(admission, *, existing_user='nobody'):
    """Dormant producer action. Never called by source-only tests or import."""
    runtime = _runtime()
    state = runtime.hosted_context(admission)
    need(platform.system() == 'Linux' and platform.machine() == 'x86_64' and os.geteuid() == 0,
         'genuine root Linux capability controller required')
    import pwd
    import shutil
    account = pwd.getpwnam(existing_user)
    need(account.pw_uid > 0, 'existing nonroot account required')
    setpriv = shutil.which('setpriv')
    need(setpriv is not None, 'existing setpriv required; no setup fallback')
    setpriv, python = exact_path(Path(setpriv).resolve()), exact_path(Path(sys.executable).resolve())
    tool_api = _tools_api(runtime)
    traps = tool_api.capture_capability_traps(state['machine'])
    tool_api.verify_runtime_tools(traps, state['machine'], state['invocation'])
    need(traps.purpose() == 'capability' and traps.mode() == 'no-native-traps', 'dedicated live capability trap tools')
    rosters = selected_rosters(state['contracts'])
    need(rosters == state['capability_rows'], 'live capability roster changed')
    source_files = _input_inventory(state, traps)
    root = exact_path(Path(state['root']) / 'hosted', exists=False)
    need(not root.exists(), 'fresh hosted root required')
    root.mkdir(mode=0o755)
    output = root / 'observations'
    output.mkdir(mode=0o700)
    # Only the newly created worker output is handed to the existing account.
    # Originals, checkout, source views and readonly copies remain untouched.
    os.chown(output, account.pw_uid, account.pw_gid)
    entries = []
    for logical, value in sorted(source_files.items()):
        _tick()
        target = namespace_path(root, logical)
        target.parent.mkdir(parents=True, exist_ok=True)
        with Path(logical).open('rb') as source, target.open('xb') as destination:
            count = 0
            while True:
                _tick()
                raw = source.read(1024 * 1024)
                if not raw:
                    break
                count += len(raw)
                need(count <= value['identity']['bytes'], 'original input grew during copy')
                destination.write(raw)
        mode = 0o555 if Path(logical).stat().st_mode & 0o111 else 0o444
        target.chmod(mode)
        identity = file_identity(target)
        need(identity['bytes'] == value['identity']['bytes'] and identity['sha256'] == value['identity']['sha256'] and
             file_identity(logical) == value['identity'], 'readonly copy changed original bytes')
        entries.append({'logical_path': logical, 'physical_path': str(target), 'bytes': identity['bytes'],
                        'sha256': identity['sha256'], 'mode': mode, 'roles': sorted(value['roles'])})
    for path in sorted((root / 'readonly').rglob('*'), reverse=True):
        if path.is_dir():
            path.chmod(0o555)
    (root / 'readonly').chmod(0o555)
    contracts = state['contracts']
    map_values = {row['authority_path']: row['path'] for row in contracts.verified if 'authority_path' in row}
    ledger = {'schema': 'package-dependency-unit4-public-hosted-transport-v1', 'status': 'NotReady',
              'execution_qualified': False, 'source_manifest_sha256': SOURCE_SHA, 'host': HOST,
              'root': str(root), 'output': str(output), 'entries': entries,
              'invocation': state['invocation'].document(), 'execution_context': state['execution_context'],
              'cfgs': state['cfgs'], 'capability_rows': rosters,
              'capability_sources_hex': {key: {name: raw.hex() for name, raw in sources.items()}
                                         for key, sources in state['capability_sources'].items()},
              'input_paths': {**state['input_paths'], 'contract_root': str(contracts.root), 'path_map': map_values},
              'trap_build': traps.document()['trap_build'],
              'parent_tools_sha256': sha(canonical(traps.document())),
              'identity': {'effective_uid': account.pw_uid, 'effective_gid': account.pw_gid, 'supplementary_groups': []},
              'controller': {'setpriv': file_identity(setpriv), 'python': file_identity(python),
                             'helper': str(Path(__file__).absolute())},
              'rust_build_invocations_in_child': 0, 'candidate_invocations_in_child': 18, 'native_executions_in_child': 0,
              'qualification_claim': False}
    verify_ledger(ledger)
    bound = _write(root / 'transport.json', ledger, readonly=True)
    helper = namespace_path(root, ledger['controller']['helper'])
    argv = worker_argv(setpriv, python, helper, bound['path'], bound['sha256'], output, account.pw_uid, account.pw_gid)
    env = outer_environment(output)
    transport = object.__new__(TransportAdmission)
    _TRANSPORTS[transport] = {'admission': admission, 'traps': traps, 'ledger': ledger, 'binding': bound,
                               'argv': argv, 'environment': env, 'state': state, 'inputs': source_files,
                               'started': False, 'probe': None, 'hosted_permit': None}
    verify_transport(transport)
    return transport


def _verify_transport(transport):
    owned = _entry(transport, TransportAdmission, _TRANSPORTS)
    runtime = _runtime()
    state = runtime.hosted_context(owned['admission'])
    ledger = owned['ledger']
    need(file_identity(owned['binding']['path']) == owned['binding'] and _read(owned['binding']['path']) == ledger,
         'original live hosted ledger changed')
    need(state['invocation'] is owned['state']['invocation'] and
         state['execution_context'] == ledger['execution_context'] and state['cfgs'] == ledger['cfgs'],
         'hosted live parent build/session changed')
    _tools_api(runtime).verify_runtime_tools(owned['traps'], state['machine'], state['invocation'])
    need(sha(canonical(owned['traps'].document())) == ledger['parent_tools_sha256'], 'hosted parent tools changed')
    inputs = _input_inventory(state, owned['traps'])
    need(inputs == owned['inputs'], 'hosted full live original input closure changed')
    mapping = verify_ledger(ledger)
    need(set(mapping) == set(inputs) and all(mapping[path]['roles'] == sorted(value['roles']) and
         (mapping[path]['bytes'], mapping[path]['sha256']) == (value['identity']['bytes'], value['identity']['sha256'])
         for path, value in inputs.items()), 'hosted ledger omitted/substituted original inputs')
    for key in ('setpriv', 'python'):
        bound = ledger['controller'][key]
        need(file_identity(bound['path']) == bound, 'hosted controller executable changed')
    return owned


def _worker_operations(inputs):
    operations = []
    root = Path(inputs['output'])
    env = inputs['environment']
    for name in ('clang', 'opt', 'ld.lld'):
        operations.append({'argv': [str(root / 'tool-traps' / name), '--version'], 'cwd': str(root),
                           'environment': {**env, 'UNIT4_TOOL_LOG': str(root / (name + '-direct-control.jsonl'))},
                           'timeout_seconds': 10, 'kind': 'trap-control'})
    adapter = inputs.get('adapter_root', str(Path(inputs['helpers']['runtime'].__file__).parent))
    for section, rows in inputs['capability_rows'].items():
        for index, row in enumerate(rows):
            roles = ('ordinary',) if section == 'public' else ('ordinary', 'lifecycle')
            for role in roles:
                evidence = root / section / str(index) / ('observer' if role == 'lifecycle' else 'ordinary')
                per_env = {**env, 'UNIT4_TOOL_LOG': str(evidence / 'tools.jsonl'),
                           'UNIT4_TOOL_EVIDENCE': str(evidence), 'UNIT4_ADAPTER_ROOT': adapter}
                if role == 'lifecycle':
                    per_env['UNIT4_LIFECYCLE_LOG'] = str(evidence / 'lifecycle.jsonl')
                operations.append({'argv': [inputs['cfgs'][role]['binaries'][row['profile']]['path']] + row['argv_template'][1:],
                                   'cwd': str(evidence / 'fixture'), 'environment': per_env,
                                   'timeout_seconds': 120, 'kind': 'capability-observation'})
    need(len(operations) == 21, 'three trap controls plus eighteen mandatory candidate processes')
    return operations


def verify_worker(worker):
    owned = _entry(worker, WorkerAdmission, _WORKERS)
    ledger = owned['ledger']
    need(file_identity(owned['ledger_binding']['path']) == owned['ledger_binding'], 'worker ledger drift')
    verify_ledger(ledger)
    identity = owned['inputs']['identity']
    need(os.geteuid() == identity['effective_uid'] > 0 and os.getegid() == identity['effective_gid'] and
         os.getgroups() == [] and os.getpid() == os.getpgrp() == os.getsid(0) == identity['process_group_policy']['worker_pid'],
         'worker identity or owned process group changed')
    need(time.monotonic() < owned['deadline'], 'hosted child shared deadline exhausted')
    return owned['inputs']


def _worker_process(worker, argv, cwd, environment, timeout, limit):
    inputs = verify_worker(worker)
    owned = _entry(worker, WorkerAdmission, _WORKERS)
    need(type(timeout) is int and limit == STREAM_LIMIT, 'fixed hosted process limits')
    claim = {'argv': list(argv), 'cwd': str(cwd), 'environment': dict(environment), 'timeout_seconds': timeout}
    matches = [index for index, allowed in enumerate(owned['operations'])
               if all(claim[key] == allowed[key] for key in claim)]
    need(len(matches) == 1 and matches[0] not in owned['completed'], 'unlisted or repeated hosted operation')
    index = matches[0]
    need(index == len(owned['completed']), 'frozen hosted operation order')
    remaining = owned['deadline'] - time.monotonic()
    need(remaining > 0, 'hosted process shared deadline')
    effective_timeout = min(timeout, remaining)
    before_group = (os.getpid(), os.getpgrp(), os.getsid(0))
    # No helper mutation: the unchanged process API has an explicit session flag.
    result = inputs['helpers']['runtime'].process(argv, cwd, environment, timeout=effective_timeout,
                                                  limit=limit, new_session=False)
    need(before_group == (os.getpid(), os.getpgrp(), os.getsid(0)), 'worker group changed during nested operation')
    result.update(environment=dict(environment), timeout_seconds=effective_timeout, requested_timeout_seconds=timeout,
                  pipe_capture_complete=True, outer_cleanup_required=True,
                  worker_pid=before_group[0], process_group=before_group[1], session_id=before_group[2], nested_new_session=False)
    need(not result['timed_out'] and not result['stream_limit_exceeded'], 'incomplete hosted nested process')
    owned['completed'][index] = clone(result)
    verify_worker(worker)
    return result


def _worker_traps(worker):
    import shutil
    inputs = verify_worker(worker)
    owned = _entry(worker, WorkerAdmission, _WORKERS)
    root = Path(inputs['output'])
    wrappers = root / 'tool-traps'
    wrappers.mkdir()
    binary = inputs['trap_binary']
    tools = {}
    for name in ('clang', 'opt', 'ld.lld'):
        target = wrappers / name
        shutil.copyfile(binary, target)
        target.chmod(0o555)
        need(file_identity(target)['sha256'] == owned['ledger']['trap_build']['binary']['sha256'], 'child trap copy bytes')
        log = root / (name + '-direct-control.jsonl')
        process = worker.process([str(target), '--version'], root,
                                 {**inputs['environment'], 'UNIT4_TOOL_LOG': str(log)}, timeout=10)
        need(process['status'] == 121 and process['stdout'] == '' and
             process['stderr'] == 'unexpected native tool invocation\n', 'child direct no-native trap control')
        need(inputs['helpers']['runtime'].read_events(log) == [{'unexpected_native_tool': True}], 'child direct trap event')
        tools[name] = {'mode': 'no-native-trap', 'executable': file_identity(target), 'wrapper': file_identity(target),
                       'control': process, 'control_log': file_identity(log)}
    inputs['tools'] = tools
    _write(root / 'tools.json', {'tools': tools, 'parent_trap_build': owned['ledger']['trap_build'],
                                'rust_build_invocations_in_child': 0})


def _worker_entry(ledger_path, ledger_sha, output, expected_uid, expected_gid):
    """Issue a distinct partial child owner, not a reconstructed BuildResult."""
    need(platform.system() == 'Linux' and platform.machine() == 'x86_64' and
         os.geteuid() == expected_uid > 0 and os.getegid() == expected_gid and os.getgroups() == [],
         'actual nonroot Linux identity and cleared groups required')
    need(os.getpid() == os.getpgrp() == os.getsid(0), 'hosted worker must lead controller-owned group')
    bound = file_identity(ledger_path)
    need(bound['sha256'] == ledger_sha, 'parent argument and actual readonly ledger differ')
    ledger = _read(ledger_path)
    resolve = _resolver(ledger)
    need(ledger['output'] == str(exact_path(output)) and ledger['identity'] ==
         {'effective_uid': expected_uid, 'effective_gid': expected_gid, 'supplementary_groups': []},
         'worker exact output and delegated identity')
    need(Path(output).stat().st_uid == expected_uid and not list(Path(output).iterdir()), 'fresh child-owned output')
    need(str(Path(__file__).absolute()) == resolve(ledger['controller']['helper']), 'worker exact transported helper')
    runtime = _runtime()
    repository = namespace_path(ledger['root'], ledger['input_paths']['repository'])
    helpers = runtime.retained_helpers(repository)
    source_map = {logical: resolve(physical) for logical, physical in ledger['input_paths']['path_map'].items()}
    amendment = ledger['input_paths']['amendment_root']
    amendment = str(namespace_path(ledger['root'], amendment)) if amendment else None
    contracts = helpers['contracts'].Contracts(namespace_path(ledger['root'], ledger['input_paths']['contract_root']),
                                                source_map, amendment)
    rows = selected_rosters(contracts)
    need(rows == ledger['capability_rows'], 'transport changed frozen capability roster')
    sources = {row['key']: contracts.source_bytes(contracts.cases[row['case_id']])
               for section in SECTIONS for row in rows[section]}
    need({key: {name: raw.hex() for name, raw in value.items()} for key, value in sources.items()} == ledger['capability_sources_hex'],
         'transport changed immutable capability fixture bytes')
    cfgs = transported_configs(ledger['cfgs'], resolve, bound)
    identity = {**ledger['identity'], 'process_group_policy': {
        'identity': 'outer-owned-session-shared-nested-v1', 'worker_pid': os.getpid(),
        'process_group': os.getpgrp(), 'session_id': os.getsid(0), 'nested_new_sessions': False}}
    env = dict(os.environ)
    for key in ('UNIT4_LIFECYCLE_LOG', 'UNIT4_TOOL_LOG', 'UNIT4_TOOL_EVIDENCE', 'UNIT4_RACE_OUTPUT',
                'LD_LIBRARY_PATH', 'LD_PRELOAD', 'PYTHONPATH'):
        env.pop(key, None)
    env.update(PYTHONDONTWRITEBYTECODE='1', PYTHONNOUSERSITE='1', OXID_LLVM_BIN=str(Path(output) / 'tool-traps'))
    execution_context = {**ledger['execution_context'], 'hosted_transport': {
        'readonly_ledger': bound, 'logical_builds': {role: cfg['_sha256'] for role, cfg in ledger['cfgs'].items()},
        'identity': identity, 'qualification_claim': False}}
    for role, cfg in cfgs.items():
        cfg['_execution_context'] = {**execution_context, 'role': role}
    inputs = {'cfgs': cfgs, 'logical_cfgs': clone(ledger['cfgs']), 'execution_context': execution_context,
              'contracts': contracts, 'helpers': helpers, 'capability_rows': rows,
              'capability_sources': sources, 'tools': {}, 'environment': env, 'env': env,
              'output': Path(output), 'root': Path(output), 'identity': identity,
              'transport': bound, 'trap_binary': resolve(ledger['trap_build']['binary']['path']),
              'window': {'started_ns': time.time_ns()},
              'input_paths': {'repository': str(repository), 'contract_root': str(contracts.root),
                              'path_map': source_map, 'amendment_root': amendment}}
    worker = object.__new__(WorkerAdmission)
    _WORKERS[worker] = {'ledger': ledger, 'ledger_binding': bound, 'inputs': inputs,
                       'deadline': _WORKER_DEADLINE.get(), 'operations': _worker_operations(inputs), 'completed': {}}
    _worker_traps(worker)
    raw = runtime.collect_capability_shard(worker)
    verify_worker(worker)
    owned = _WORKERS[worker]
    need(len(owned['completed']) == 21, 'complete exact child process roster required')
    report = {'schema': 'package-dependency-unit4-public-hosted-shard-v1', 'status': 'PARTIAL_CAPABILITY_SHARD',
              'execution_qualified': False, 'global_host_qualification': False, 'complete_qualification': False,
              'source_manifest_sha256': SOURCE_SHA, 'invocation': ledger['invocation'],
              'readonly_transport': bound, 'execution_context': execution_context, 'identity': identity,
              'cfgs': cfgs, 'tools': inputs['tools'], 'environment': env, 'raw_result': raw,
              'processes': [owned['completed'][index] for index in range(21)],
              'tuple_keys': {section: [row['key'] for row in values] for section, values in rows.items()},
              'rust_build_invocations_in_child': 0, 'candidate_invocations_in_child': 18, 'native_executions_in_child': 0}
    report['output_inventory'] = output_inventory(output)
    verify_output_roster(report['output_inventory'], rows, sources)
    _write(Path(output) / 'shard.json', report)
    return 0


def output_inventory(root, exclude=()):
    root = exact_path(root)
    rows, total = [], 0
    for path in sorted(root.rglob('*')):
        _tick()
        relative = path.relative_to(root).as_posix()
        need(not path.is_symlink(), 'hosted output symlink forbidden')
        if relative in exclude:
            need(path.is_file(), 'hosted excluded carrier must be regular')
            continue
        if path.is_dir():
            rows.append({'path': relative, 'kind': 'directory'})
        else:
            identity = file_identity(path)
            total += identity['bytes']
            need(total <= MAX_OUTPUT_BYTES, 'hosted exact output byte bound')
            rows.append({'path': relative, 'kind': 'file', 'bytes': identity['bytes'], 'sha256': identity['sha256']})
        need(len(rows) <= 2048, 'hosted exact output member bound')
    return rows



def expected_output_paths(rows, sources):
    """Derive every allowed child output from the original finite tuple domain."""
    required = {'tools.json'}
    optional = set()
    for name in ('clang', 'opt', 'ld.lld'):
        required.update({'tool-traps/' + name, name + '-direct-control.jsonl'})
    for section in SECTIONS:
        need(len(rows[section]) == 6, 'exact output section tuple domain')
        for index, row in enumerate(rows[section]):
            prefix = section + '/' + str(index)
            required.add(prefix + '/receipt.json')
            for role in (('ordinary',) if section == 'public' else ('ordinary', 'observer')):
                evidence = prefix + '/' + role
                required.update(evidence + '/' + name for name in ('process.json', 'stdout', 'stderr', 'receipt.json'))
                optional.update(evidence + '/' + name for name in ('tools.jsonl', 'lifecycle.jsonl'))
                for name in sources[row['key']]:
                    source = PurePosixPath(name)
                    need(not source.is_absolute() and '..' not in source.parts and source.parts,
                         'safe frozen fixture output path')
                    required.add(evidence + '/fixture/' + name)
    directories = {str(parent) for name in required for parent in PurePosixPath(name).parents if str(parent) != '.'}
    return required, optional, directories


def verify_output_roster(inventory, rows, sources):
    required, optional, directories = expected_output_paths(rows, sources)
    names = [row['path'] for row in inventory]
    need(names == sorted(set(names)), 'sorted unique hosted output inventory')
    files = {row['path'] for row in inventory if row['kind'] == 'file'}
    dirs = {row['path'] for row in inventory if row['kind'] == 'directory'}
    need(len(files) + len(dirs) == len(inventory) and required <= files <= required | optional and dirs == directories,
         'unlisted/missing hosted output file or directory')
    return True


def _expected_child_environment(parent_environment, output):
    env = dict(parent_environment)
    for key in ('UNIT4_LIFECYCLE_LOG', 'UNIT4_TOOL_LOG', 'UNIT4_TOOL_EVIDENCE', 'UNIT4_RACE_OUTPUT',
                'LD_LIBRARY_PATH', 'LD_PRELOAD', 'PYTHONPATH'):
        env.pop(key, None)
    env.update(PYTHONDONTWRITEBYTECODE='1', PYTHONNOUSERSITE='1', OXID_LLVM_BIN=str(Path(output) / 'tool-traps'))
    return env


def execute_transport(transport):
    """Capture an owned operation; this alone is never qualification."""
    owned = verify_transport(transport)
    runtime = _runtime()
    state = runtime.hosted_context(owned['admission'])
    api = runtime._binding().preparation_api()
    need(owned['started'] is False, 'hosted transport can launch only one worker attempt')
    owned['started'] = True
    identity = owned['ledger']['identity']
    permit = api.register_public_hosted(state['invocation'], state['runtime_permit'],
        Path(owned['binding']['path']), Path(owned['ledger']['output']), identity['effective_uid'],
        identity['effective_gid'], Path(owned['ledger']['controller']['setpriv']['path']))
    owned['hosted_permit'] = permit
    probe = api.owned_probe(state['invocation'], owned['argv'], Path(owned['ledger']['output']), owned['environment'],
                            timeout_seconds=WORKER_TIMEOUT, stream_limit_bytes=STREAM_LIMIT,
                            operation_kind='public-hosted-worker', runtime_permit=owned['hosted_permit'])
    owned['probe'] = probe
    process = api.verify_probe(probe, state['invocation'], argv=owned['argv'], cwd=Path(owned['ledger']['output']),
                               environment=owned['environment'], timeout_seconds=WORKER_TIMEOUT,
                               stream_limit_bytes=STREAM_LIMIT, operation_kind='public-hosted-worker', runtime_permit=owned['hosted_permit'])
    process = dict(process)
    for stream in ('stdout', 'stderr'):
        raw = getattr(probe, stream)()
        process[stream], process[stream + '_sha256'] = raw.decode('utf-8', errors='strict'), sha(raw)
    inspect_worker_process(process, owned['argv'], owned['ledger']['output'], owned['environment'], state['invocation'].document())
    verify_transport(transport)
    report_path = Path(owned['ledger']['output']) / 'shard.json'
    report = _read(report_path)
    result = object.__new__(HostedResult)
    _RESULTS[result] = {'transport': transport, 'probe': probe, 'process': process,
                        'report_binding': file_identity(report_path), 'document': report}
    verify_hosted(result)
    return result


def verify_hosted(result):
    """Replay the exact partial shard against original parent-owned evidence."""
    record = _entry(result, HostedResult, _RESULTS)
    owned = verify_transport(record['transport'])
    runtime = _runtime()
    state = runtime.hosted_context(owned['admission'])
    api = runtime._binding().preparation_api()
    actual = api.verify_probe(record['probe'], state['invocation'], argv=owned['argv'],
                             cwd=Path(owned['ledger']['output']), environment=owned['environment'],
                             timeout_seconds=WORKER_TIMEOUT, stream_limit_bytes=STREAM_LIMIT,
                             operation_kind='public-hosted-worker', runtime_permit=owned['hosted_permit'])
    process = dict(actual)
    for stream in ('stdout', 'stderr'):
        raw = getattr(record['probe'], stream)()
        process[stream], process[stream + '_sha256'] = raw.decode('utf-8', errors='strict'), sha(raw)
    need(process == record['process'], 'hosted original owned process changed')
    inspect_worker_process(process, owned['argv'], owned['ledger']['output'], owned['environment'], state['invocation'].document())
    report = record['document']
    need(file_identity(record['report_binding']['path']) == record['report_binding'] and
         _read(record['report_binding']['path']) == report, 'hosted raw shard body changed')
    need(report['schema'] == 'package-dependency-unit4-public-hosted-shard-v1' and
         report['status'] == 'PARTIAL_CAPABILITY_SHARD' and report['execution_qualified'] is False and
         report['global_host_qualification'] is False and report['complete_qualification'] is False and
         report['source_manifest_sha256'] == SOURCE_SHA and report['readonly_transport'] == owned['binding'] and
         report['invocation'] == state['invocation'].document(), 'partial new-source shard exact parent association')
    expected_identity = owned['ledger']['identity']
    need(all(report['identity'][key] == value for key, value in expected_identity.items()), 'worker identity differs from delegated account')
    need(report['tuple_keys'] == {section: [row['key'] for row in values]
                                 for section, values in state['capability_rows'].items()}, 'exact all twelve shard tuples')
    need(len(report['processes']) == 21 and len({(p['pid'], p['started_ns']) for p in report['processes']}) == 21,
         'complete distinct child process evidence')
    for nested in report['processes']:
        inspect_nested_process(nested, process, report['identity'])
    need(report['output_inventory'] == output_inventory(owned['ledger']['output'], exclude=('shard.json',)),
         'raw hosted output exact file/directory closure changed')
    verify_output_roster(report['output_inventory'], state['capability_rows'], state['capability_sources'])
    resolve = _resolver(owned['ledger'])
    cfgs = transported_configs(state['cfgs'], resolve, owned['binding'])
    expected_outer = {**state['execution_context'], 'hosted_transport': {
        'readonly_ledger': owned['binding'], 'logical_builds': {role: cfg['_sha256'] for role, cfg in state['cfgs'].items()},
        'identity': report['identity'], 'qualification_claim': False}}
    for role, cfg in cfgs.items():
        cfg['_execution_context'] = {**expected_outer, 'role': role}
    need(report['cfgs'] == cfgs, 'hosted copied compiler bindings differ')
    worker_inputs = {'cfgs': cfgs, 'logical_cfgs': state['cfgs'], 'execution_context': report['execution_context'],
                     'contracts': state['contracts'], 'helpers': state['helpers'], 'capability_rows': state['capability_rows'],
                     'capability_sources': state['capability_sources'], 'tools': report['tools'],
                     'output': Path(owned['ledger']['output']), 'root': Path(owned['ledger']['output']),
                     'identity': report['identity'], 'transport': owned['binding'],
                     'environment': _expected_child_environment(owned['environment'], owned['ledger']['output']),
                     'adapter_root': str(namespace_path(owned['ledger']['root'],
                         Path(state['input_paths']['repository']) / 'tests/qualification/unit4_public_v3')),
                     'process_records': {(p['pid'], p['started_ns']): p for p in report['processes']},
                     'resolver': resolve}
    need(report['environment'] == worker_inputs['environment'], 'exact cleaned worker environment')
    operations = _worker_operations(worker_inputs)
    previous_end = process['started_ns']
    for expected, actual in zip(operations, report['processes']):
        need(all(actual[key] == expected[key] for key in ('argv', 'cwd', 'environment')) and
             actual['requested_timeout_seconds'] == expected['timeout_seconds'] and
             type(actual['timeout_seconds']) in (int, float) and 0 < actual['timeout_seconds'] <= expected['timeout_seconds'] and
             previous_end <= actual['started_ns'], 'unlisted/reordered hosted child operation')
        previous_end = actual['completed_ns']
    need(report['execution_context'] == expected_outer, 'hosted source/build/session outer context differs')
    for name in ('clang', 'opt', 'ld.lld'):
        tool = report['tools'][name]
        need(file_identity(tool['executable']['path']) == tool['executable'] == tool['wrapper'] and
             tool['executable']['sha256'] == owned['ledger']['trap_build']['binary']['sha256'] and
             tool['executable']['path'] == str(Path(owned['ledger']['output']) / 'tool-traps' / name),
             'child tool differs from actual captured trap')
        need(file_identity(tool['control_log']['path']) == tool['control_log'] and
             state['helpers']['runtime'].read_events(Path(tool['control_log']['path'])) == [{'unexpected_native_tool': True}],
             'actual child trap direct-control log')
        need(tool['control'] in report['processes'][:3] and tool['control']['status'] == 121 and
             tool['control']['stdout'] == '' and tool['control']['stderr'] == 'unexpected native tool invocation\n',
             'actual child trap direct-control process')
    raw_before = canonical(report)
    runtime.verify_capability_shard(owned['admission'], worker_inputs, report['raw_result'], process)
    need(canonical(report) == raw_before, 'hosted semantic replay cannot normalize supplied raw evidence')
    return clone(report)




def result_inputs(result):
    """Join access retains the original parent owner; reports alone are inert."""
    report = verify_hosted(result)
    record = _entry(result, HostedResult, _RESULTS)
    transport = _entry(record['transport'], TransportAdmission, _TRANSPORTS)
    return {'admission': transport['admission'], 'raw_result': clone(report['raw_result']),
            'report': report, 'report_binding': clone(record['report_binding']),
            'worker_process': clone(record['process']), 'transport_binding': clone(transport['binding'])}


def prepare_transport(admission, *, existing_user='nobody'):
    runtime = _runtime()
    state = runtime.hosted_context(admission)
    api = runtime._binding().preparation_api()
    token = _CHECKPOINT.set(lambda: api.check_deadline(state['invocation']))
    try:
        return _prepare_transport(admission, existing_user=existing_user)
    finally:
        _CHECKPOINT.reset(token)


def verify_transport(transport):
    owned = _entry(transport, TransportAdmission, _TRANSPORTS)
    state = owned['state']
    api = _runtime()._binding().preparation_api()
    token = _CHECKPOINT.set(lambda: api.check_deadline(state['invocation']))
    try:
        return _verify_transport(transport)
    finally:
        _CHECKPOINT.reset(token)


def worker_entry(ledger_path, ledger_sha, output, expected_uid, expected_gid):
    deadline = time.monotonic() + WORKER_TIMEOUT
    def tick():
        need(time.monotonic() < deadline, 'hosted worker whole-entry deadline')
    token = _CHECKPOINT.set(tick)
    deadline_token = _WORKER_DEADLINE.set(deadline)
    try:
        return _worker_entry(ledger_path, ledger_sha, output, expected_uid, expected_gid)
    finally:
        _WORKER_DEADLINE.reset(deadline_token)
        _CHECKPOINT.reset(token)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=('worker',))
    parser.add_argument('--ledger', required=True)
    parser.add_argument('--ledger-sha256', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--expected-uid', type=int, required=True)
    parser.add_argument('--expected-gid', type=int, required=True)
    args = parser.parse_args()
    # The canonical module owns WorkerAdmission even when this file is a script.
    owner = _runtime()._binding().hosted_api()
    return owner.worker_entry(args.ledger, args.ledger_sha256, args.out, args.expected_uid, args.expected_gid)


if __name__ == '__main__':
    sys.exit(main())
