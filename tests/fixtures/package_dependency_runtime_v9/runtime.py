"""Current-source public runtime collection; import never starts a child.

Only live producer BuildResults and RuntimeTools issue a parent admission.
Transported child contexts are deliberately separate, bounded capability shards.
Frozen comparison bodies and old semantic-source associations remain unchanged.
"""
import copy
import contextlib
import threading
import gzip
import hashlib
import importlib.util
import json
import os
import stat
import sys
import time
import types
import weakref
import traceback
from pathlib import Path

sys.dont_write_bytecode = True
BINDING_SHA256 = 'f942422346805131e92ad0dd8f08f7d004d337c0b2f89a432f6a23e9c829ecd8'
INPUTS_SHA256 = 'e65fbe5018b90bb9cbc3abe1941c391e3678b19d0b2bbe8f00b38e70b4099445'
PUBLIC = 'tests/qualification/unit4_public_v3'
HOSTED = 'tests/qualification/unit4_hosted_capability'
SOURCE = 'tests/fixtures/typed_project_source_binding'
COMPAT = 'tests/fixtures/typed_project_unit1_compatibility'
SECTIONS = ('public', 'original', 'predecessors', 'lifecycle', 'guards')
PROFILES = ('debug', 'release')
SEMANTIC_FILES = ('current-source.json', 'enum-source.json', 'enum_enabled_qualified_values_v1.py', 'enum-enabled-qualified-values-v1.json')
_ISSUED_HELPERS = {}
_ADMISSIONS = weakref.WeakKeyDictionary()
_COLLECTIONS = weakref.WeakKeyDictionary()
_READ_BOUNDARIES = weakref.WeakKeyDictionary()
_READ_LOCAL = threading.local()


class Rejected(ValueError):
    pass


class InfrastructureFailure(RuntimeError):
    """A readback service failure can never satisfy a semantic control."""
    def __init__(self, phase, error):
        self.phase = phase
        self.original = error
        super().__init__(phase + ': ' + _safe_error_text(error))


def _infrastructure(phase, function, *args, **kwargs):
    try:
        return function(*args, **kwargs)
    except InfrastructureFailure:
        raise
    except Exception as error:
        raise InfrastructureFailure(phase, error) from error


def _safe_error_text(error, limit=2048):
    try:
        return str(error)[:limit]
    except BaseException:
        return '<exception text unavailable>'


def _is_semantic_error(error):
    retained = globals().get('Reject')
    return isinstance(error, Rejected) or (isinstance(retained, type) and isinstance(error, retained))


def _failure_details(error):
    """Bounded runtime-side traceback only; diagnostics never replace failure."""
    try:
        original = error.original if isinstance(error, InfrastructureFailure) else error
        semantic = getattr(error, 'preceding_semantic', None)
        return {'phase': getattr(error, 'phase', None), 'type': type(original).__name__,
                'error': _safe_error_text(original),
                'preceding_semantic': None if semantic is None else
                    {'type': type(semantic).__name__, 'error': _safe_error_text(semantic, 500)},
                'stack': [{'file': frame.filename[:1024], 'function': frame.name[:128], 'line': frame.lineno}
                          for frame in traceback.StackSummary.extract(
                              traceback.walk_tb(original.__traceback__ or error.__traceback__),
                              limit=32, lookup_lines=False)]}
    except BaseException:
        return {'phase': None, 'type': type(error).__name__, 'error': '<failure diagnostics unavailable>',
                'stack': [], 'diagnostic_error': 'runtime failure diagnostic capture failed'}


def need(value, message):
    if not value:
        raise Rejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def _binding_uncached():
    path = Path(__file__).absolute().with_name('binding.py')
    raw = path.read_bytes()
    need(isinstance(BINDING_SHA256, str) and sha(raw) == BINDING_SHA256, 'unsealed public binding helper')
    key = '_oxid_package_dependency_unit4_public_binding_v1'
    if key in sys.modules:
        module = sys.modules[key]
        need(module.__file__ == str(path) and module.__source_sha256__ == BINDING_SHA256, 'conflicting binding owner')
        return module
    module = types.ModuleType(key)
    module.__file__, module.__source_sha256__ = str(path), BINDING_SHA256
    sys.modules[key] = module
    try:
        exec(compile(raw, str(path), 'exec'), module.__dict__)
    except BaseException:
        if sys.modules.get(key) is module:
            del sys.modules[key]
        raise
    return module


class _PublicReadBoundary:
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('public read boundaries are privately issued')


def _active_boundary():
    boundary = getattr(_READ_LOCAL, 'boundary', None)
    if boundary is None:
        return None
    need(type(boundary) is _PublicReadBoundary and boundary in _READ_BOUNDARIES,
         'unissued public read boundary')
    owned = _READ_BOUNDARIES[boundary]
    need(owned['thread'] == threading.get_ident(), 'public read boundary thread changed')
    return owned


def _binding():
    active = _infrastructure('read-owner', _active_boundary)
    return active['binding'] if active is not None else _infrastructure('read-owner', _binding_uncached)


def _build():
    active = _active_boundary()
    return active['build'] if active is not None else _binding().build_api()


def _preparation():
    active = _active_boundary()
    return active['preparation'] if active is not None else _binding().preparation_api()


def _machine():
    active = _active_boundary()
    return active['machine'] if active is not None else _binding().machine_api()


def _tick(invocation):
    active = _infrastructure('read-owner', _active_boundary)
    if active is not None:
        if invocation is not None and invocation is not active['invocation']:
            raise InfrastructureFailure('read-owner', RuntimeError('mixed read-boundary invocation'))
        _infrastructure('checkpoint', active['preparation'].check_deadline, active['invocation'])
        _infrastructure('output-audit', active['preparation'].public_output_tick, active['invocation'])
    else:
        api = _infrastructure('read-owner', _preparation)
        _infrastructure('checkpoint', api.check_deadline, invocation)
        _infrastructure('output-audit', api.public_output_tick, invocation)


def _authenticate_read_owners(state, expected=None):
    binding_api = _binding_uncached()
    owners = {'binding': binding_api, 'build': binding_api.build_api(),
              'preparation': binding_api.preparation_api(), 'machine': binding_api.machine_api()}
    if expected is not None:
        need(all(owners[key] is expected[key] for key in owners), 'canonical public read owner changed')
    need(owners['build']._consumer_identity(None) == state['consumer'], 'public consumer closure changed')
    return owners


@contextlib.contextmanager
def _read_boundary(state):
    """One synchronous read-only boundary; cache owners, never input hashes."""
    need(any(issued is state for issued in _ADMISSIONS.values()), 'issued runtime state required')
    existing = _active_boundary()
    if existing is not None:
        need(existing['state'] is state, 'mixed nested public read state')
        yield
        return
    need(not state.get('invalidated'), 'invalidated runtime read state')
    try:
        invocation = state['invocation']
        with state['_read_owner_seed'].public_inputs(invocation, state['runtime_permit']):
            owners = _authenticate_read_owners(state)
            need(owners['preparation'] is state['_read_owner_seed'], 'read scope original preparation owner changed')
            boundary = object.__new__(_PublicReadBoundary)
            _READ_BOUNDARIES[boundary] = {**owners, 'invocation': invocation, 'state': state,
                                         'thread': threading.get_ident()}
            _READ_LOCAL.boundary = boundary
            original_error = None
            try:
                need(retained_helper_inputs(state['input_paths']['repository']) == state['helper_input_files'],
                     'retained helper closure changed at read entry')
                yield
            except BaseException as error:
                original_error = error
                raise
            finally:
                try:
                    need(retained_helper_inputs(state['input_paths']['repository']) == state['helper_input_files'],
                         'retained helper closure changed at read exit')
                    _authenticate_read_owners(state, owners)
                    owners['preparation'].check_deadline(invocation)
                except BaseException as preservation:
                    if original_error is None or _is_semantic_error(original_error):
                        failure = InfrastructureFailure('read-boundary-preservation', preservation)
                        if original_error is not None:
                            failure.preceding_semantic = original_error
                            failure.add_note('preceding semantic rejection: ' + _safe_error_text(original_error, 500))
                        raise failure from preservation
                    original_error.add_note('public read-boundary preservation failed: ' + _safe_error_text(preservation, 1000))
                finally:
                    del _READ_LOCAL.boundary
                    del _READ_BOUNDARIES[boundary]

    except BaseException:
        state['invalidated'] = True
        raise


def _scoped_read_file(path, invocation=None, *, header_only=False, maximum=1024 ** 3):
    """Bounded streaming identity, checked ancestry and stable before/after stat."""
    need(_active_boundary() is not None, 'scoped reader requires issued boundary')
    _tick(invocation)
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=True) == path and
         not any(p.is_symlink() for p in (path, *path.parents)), 'canonical regular evidence path')
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and before.st_size <= maximum, 'regular evidence byte bound')
    keys = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
    digest, captured, total = hashlib.sha256(), bytearray(), 0
    with path.open('rb') as stream:
        while True:
            _tick(invocation)
            raw = stream.read(1024 * 1024)
            if not raw:
                break
            total += len(raw)
            need(total <= maximum, 'evidence grew beyond byte bound')
            digest.update(raw)
            if not header_only or not captured:
                captured.extend(raw)
    _tick(invocation)
    after = path.stat()
    need(all(getattr(before, k) == getattr(after, k) for k in keys) and total == before.st_size,
         'evidence changed while being read')
    return {'path': str(path), 'bytes': total, 'sha256': digest.hexdigest(),
            'stat': {k: getattr(after, k) for k in keys}}, bytes(captured)


def _identity(path, invocation=None):
    if _infrastructure('read-owner', _active_boundary) is not None:
        row, _ = _infrastructure('evidence-identity', _scoped_read_file, Path(path), invocation, header_only=True)
        del row['stat']
        return row
    return _infrastructure('evidence-identity', _infrastructure('read-owner', _build)._identity, Path(path), invocation=invocation)


def _read(path, invocation=None):
    if _infrastructure('read-owner', _active_boundary) is not None:
        return _infrastructure('evidence-read', _scoped_read_file, Path(path), invocation, maximum=128 * 1024 ** 2)[1]
    return _infrastructure('evidence-read', _infrastructure('read-owner', _build)._read_file, Path(path), invocation, maximum=128 * 1024 ** 2)[1]


def _write(path, value, invocation=None):
    return _build()._write(Path(path), value, invocation)


def retained_helper_inputs(repository_root):
    """Fixed exact retained closure, derived from the immutable old inventory."""
    repo = Path(repository_root)
    need(repo.is_absolute() and repo.resolve(strict=True) == repo, 'canonical retained repository')
    inventory = repo / 'tests/qualification/unit4_ci/inputs.json'
    raw = _read(inventory)
    need(sha(raw) == INPUTS_SHA256, 'retained Unit4 input authority')
    entries = _binding().decode(raw)['files']
    chosen = [r for r in entries if r['path'].startswith((PUBLIC + '/', HOSTED + '/', COMPAT + '/'))
              or r['path'] in {SOURCE + '/' + name for name in SEMANTIC_FILES}]
    need(len({r['path'] for r in chosen}) == len(chosen), 'duplicate retained helper row')
    for root, count in ((PUBLIC, 31), (HOSTED, 4), (COMPAT, 7)):
        expected = {r['path'][len(root) + 1:] for r in chosen if r['path'].startswith(root + '/')}
        need(len(expected) == count, 'complete retained helper root')
        files, dirs = set(), set()
        for path in (repo / root).rglob('*'):
            mode = path.lstat().st_mode
            relative = path.relative_to(repo / root).as_posix()
            if stat.S_ISDIR(mode):
                dirs.add(relative)
            else:
                need(stat.S_ISREG(mode), 'retained helper special or symlink member')
                files.add(relative)
        wanted_dirs = {str(parent) for name in expected for parent in Path(name).parents if str(parent) != '.'}
        need(files == expected and dirs == wanted_dirs, 'retained closed helper membership')
    result = [_identity(inventory)]
    for row in chosen:
        identity = _identity(repo / row['path'])
        need({**identity, 'path': row['path']} == row, 'retained helper body changed')
        result.append(identity)
    return result


def retained_helpers(repository_root):
    """Authenticate all imports first; never replace an old module or globals."""
    repo = Path(repository_root)
    identities = retained_helper_inputs(repo)
    lookup = {r['path']: r for r in identities}
    order = ('contracts', 'authority', 'compare', 'process_tree', 'runtime', 'predecessors', 'run', 'guards')
    # No member executes until all transitive names and full package were read.
    bodies = {name: _read(repo / PUBLIC / (name + '.py')) for name in order}
    for name in order:
        path = str(repo / PUBLIC / (name + '.py'))
        need(sha(bodies[name]) == lookup[path]['sha256'], 'retained import closure drift')
        if name in sys.modules:
            need(name in _ISSUED_HELPERS and sys.modules[name] is _ISSUED_HELPERS[name] and
                 sys.modules[name].__file__ == path and sys.modules[name].__source_sha256__ == sha(bodies[name]),
                 'unissued/conflicting retained short-name module')
    for name in order:
        if name in _ISSUED_HELPERS:
            continue
        path = str(repo / PUBLIC / (name + '.py'))
        module = types.ModuleType(name)
        module.__file__, module.__source_sha256__ = path, sha(bodies[name])
        _ISSUED_HELPERS[name] = module
        sys.modules[name] = module
        try:
            exec(compile(bodies[name], path, 'exec'), module.__dict__)
        except BaseException:
            if sys.modules.get(name) is module:
                del sys.modules[name]
            del _ISSUED_HELPERS[name]
            raise
    global rt, compare, FREEZE_SHA, receipt_identity, load, save, binding, verify, check_inventory, Reject
    rt, compare = _ISSUED_HELPERS['runtime'], _ISSUED_HELPERS['compare']
    c = _ISSUED_HELPERS['contracts']
    FREEZE_SHA, receipt_identity, load, save, binding, verify = c.FREEZE_SHA, c.receipt_identity, c.load, c.save, c.binding, c.verify
    check_inventory, Reject = c.check_inventory, c.Reject
    return {name: _ISSUED_HELPERS[name] for name in order}


def capability_rows(contracts):
    result = {}
    for section in ('public', 'lifecycle'):
        result[section] = [row for row in contracts.roster(section, 'Linux x86_64')
                           if row['scope'] == 'execute' and row['required_capabilities'] ==
                           ['posix_regular_file_mode_denies_open_for_effective_identity']]
    need({key: len(value) for key, value in result.items()} == {'public': 6, 'lifecycle': 6}, 'exact twelve capability rows')
    return result


class RuntimeAdmission:
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('RuntimeAdmission requires live producer inputs')

    def document(self):
        return json.loads(canonical(_state(self)['document']))


class CollectionResult:
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('CollectionResult is issued by complete collection')

    def document(self):
        need(type(self) is CollectionResult and self in _COLLECTIONS, 'live collection result required')
        return json.loads(_COLLECTIONS[self]['raw'])


def _state(admission):
    need(type(admission) is RuntimeAdmission and admission in _ADMISSIONS, 'live parent runtime admission required')
    state = _ADMISSIONS[admission]
    need(not state.get('invalidated'), 'runtime admission was invalidated')
    return state


def _cfg(result):
    doc = _build().read_build_binding(result)
    return {'kind': doc['kind'], 'role': doc['role'], '_sha256': _build()._result(result)['receipt']['sha256'],
            'binaries': {p: {k: doc['profiles'][p]['binary'][k] for k in ('path', 'bytes', 'sha256')} for p in PROFILES},
            '_binary_stats': {p: doc['profiles'][p]['binary']['stat'] for p in PROFILES}, 'build_binding': doc}


def _contract_snapshot(contracts):
    return {'root': str(contracts.root), 'path_map': contracts.path_map, 'verified': contracts.verified,
            'effective_identity': contracts.effective_identity, 'tables': contracts.tables}


def _verify_view(view, invocation):
    _tick(invocation)
    root = Path(view['path'])
    expected = {r['path']: r for r in view['files'] + view['metadata']}
    need(len(expected) == len(view['files']) + len(view['metadata']), 'source/metadata duplicate')
    files, directories = {}, set()
    for path in root.rglob('*'):
        _tick(invocation)
        mode = path.lstat().st_mode
        relative = path.relative_to(root).as_posix()
        if stat.S_ISDIR(mode):
            directories.add(relative)
        else:
            need(stat.S_ISREG(mode), 'source special/symlink member')
            row = _identity(path, invocation)
            files[relative] = dict(row, path=relative)
    wanted_dirs = {str(p) for name in expected for p in Path(name).parents if str(p) != '.'}
    need(files == expected and directories == wanted_dirs, 'sealed prepared view drift')


def _selected_case(state):
    case = state['active_case']
    need(type(case) is dict and set(case) == {'section', 'index', 'variant'}, 'closed selected runtime case')
    section, index, variant = case['section'], case['index'], case['variant']
    need(section in SECTIONS and type(index) is int and 0 <= index < len(state['rosters'][section]) and
         variant in ('ordinary', 'observer'), 'frozen selected runtime case')
    row = state['rosters'][section][index]
    need(row['scope'] == 'execute' and row['profile'] in PROFILES, 'selected local runtime tuple required')
    need(variant == 'ordinary' or section in ('lifecycle', 'guards'), 'observer variant outside frozen pair sections')
    return ('ordinary' if variant == 'ordinary' else 'lifecycle'), row['profile']


def _checkpoint(state, full=False):
    need(not state.get('invalidated'), 'runtime admission was invalidated')
    try:
        # Full capture admission stays outside cheap read-only association scopes.
        if full:
            need(_active_boundary() is None, 'full checkpoint cannot hide inside a selected read scope')
            _preparation().full_verify_invocation(state['invocation'])
        with _read_boundary(state):
            _checkpoint_reads(state, full=full)
        if full:
            _preparation().full_verify_invocation(state['invocation'])
        return True

    except BaseException:
        state['invalidated'] = True
        raise

def _checkpoint_reads(state, full=False):
    invocation = state['invocation']
    _tick(invocation)
    if full:
        for role in ('ordinary', 'lifecycle'):
            need(_cfg(state[role]) == {k: v for k, v in state['cfgs'][role].items() if k not in ('_execution_context', '_consumer')}, 'current build context drift')
        _machine().verify_runtime_tools(state['runtime_tools'], state['machine'], invocation)
        c = state['helpers']['contracts']
        current = c.Contracts(state['input_paths']['contract_root'],
                              _binding().decode(_read(state['input_paths']['path_map'], invocation)),
                              state['input_paths']['amendment_root'])
        need(canonical(_contract_snapshot(current)) == state['contract_snapshot'] ==
             canonical(_contract_snapshot(state['contracts'])), 'contract/roster/corpus drift')
        expected_rosters = {section: current.roster(section, state['execution_context']['host']) for section in SECTIONS}
        need(state['rosters'] == expected_rosters and state['row_indices'] ==
             {section: {row['key']: index for index, row in enumerate(rows)} for section, rows in expected_rosters.items()},
             'in-memory frozen roster changed')
    selected_role, selected_profile = _selected_case(state) if not full else (None, None)
    for role in (('ordinary', 'lifecycle') if full else (selected_role,)):
        _verify_view(state['cfgs'][role]['build_binding']['source_binding']['view'], invocation)
        for profile, binary in state['cfgs'][role]['binaries'].items():
            if not full and profile != selected_profile:
                continue
            need(_identity(binary['path'], invocation) == binary and
                 state['helpers']['runtime'].file_identity(binary['path']) == state['cfgs'][role]['_binary_stats'][profile],
                 'runtime compiler bytes/stat changed')
    # This selected runtime closure excludes repeated Rust/native build sysroots.
    _machine().verify_runtime_inputs(state['runtime_tools'], state['machine'], invocation)
    return True


def admit_runtime(ordinary, lifecycle, *, contract_root, path_map, amendment_root, runtime_tools):
    api = _build()
    live = {role: api.runtime_inputs(value) for role, value in (('ordinary', ordinary), ('lifecycle', lifecycle))}
    need(live['ordinary']['role'] == 'ordinary' and live['lifecycle']['role'] == 'lifecycle', 'exact paired public builds')
    invocation, machine = live['ordinary']['invocation'], live['ordinary']['machine']
    need(live['lifecycle']['invocation'] is invocation and live['lifecycle']['machine'] is machine, 'mixed public build producer')
    _machine().verify_runtime_tools(runtime_tools, machine, invocation)
    need(runtime_tools.purpose() == 'main' and runtime_tools.invocation() is invocation, 'main runtime tools required')
    repo = Path(live['ordinary']['context'].document()['checkout']['path'])
    helpers = retained_helpers(repo)
    c = helpers['contracts']
    contract_root, path_map, amendment_root = map(lambda p: str(Path(p).resolve(strict=True)), (contract_root, path_map, amendment_root))
    contracts = c.Contracts(contract_root, _binding().decode(_read(path_map, invocation)), amendment_root)
    cfgs = {'ordinary': _cfg(ordinary), 'lifecycle': _cfg(lifecycle)}
    need(cfgs['ordinary']['build_binding']['source_binding']['source_manifest'] ==
         cfgs['lifecycle']['build_binding']['source_binding']['source_manifest'], 'role source manifest differs')
    predecessors = helpers['predecessors'].Predecessors(contracts, source_manifest=repo / SOURCE / 'current-source.json',
                                                       amendment_root=repo / SOURCE)
    root = Path(invocation.document()['run_root']) / 'public-runtime'
    need(root.is_dir(), 'runtime tools must create the dedicated runtime root')
    need(not (root / 'collection').exists(), 'fresh full collection required')
    rosters = {section: contracts.roster(section, c.host()) for section in SECTIONS}
    needed_native = c.host() == 'Linux x86_64'
    need(runtime_tools.mode() == ('real-native' if needed_native else 'no-native-traps'), 'host runtime domain cannot be skipped')
    consumer = api._consumer_identity(invocation)
    outer = {'schema': 'package-unit4-public-runtime-context-v1', 'source_manifest_sha256': _binding().CURRENT_SOURCE_SHA,
             'invocation': invocation.document(), 'checkout': cfgs['ordinary']['build_binding']['source_binding']['checkout'],
             'source_context_sha256': cfgs['ordinary']['build_binding']['source_binding']['context_sha256'],
             'consumer': consumer, 'runtime_tools_sha256': sha(canonical(runtime_tools.document())),
             'ordinary_build_sha256': cfgs['ordinary']['_sha256'], 'lifecycle_build_sha256': cfgs['lifecycle']['_sha256'],
             'retained_semantic_source_manifest_sha256': _binding().PREDECESSOR_SOURCE_SHA,
             'host': c.host(), 'global_host_qualification': False}
    for cfg in cfgs.values():
        cfg['_execution_context'] = dict(outer, role=cfg['role'])
        cfg['_consumer'] = {'schema': 'package-unit4-public-consumer-v2', 'bytes': consumer['bytes'], 'sha256': consumer['sha256']}
    state = {'ordinary': ordinary, 'lifecycle': lifecycle, 'invocation': invocation, 'machine': machine,
             'runtime_tools': runtime_tools, 'helpers': helpers, 'contracts': contracts, 'predecessors': predecessors,
             'contract_snapshot': canonical(_contract_snapshot(contracts)), 'cfgs': cfgs, 'root': root,
             'rosters': rosters, 'row_indices': {section: {row['key']: index for index, row in enumerate(rows)} for section, rows in rosters.items()}, 'consumer': consumer, 'execution_context': outer, 'probes': {}, 'native_probes': {}, 'native_permits': {}, 'process_cases': {},
             'helper_input_files': retained_helper_inputs(repo),
             'input_paths': {'contract_root': contract_root, 'path_map': path_map, 'amendment_root': amendment_root,
                             'repository': str(repo)},
             'document': {'schema': 'package-unit4-public-runtime-admission-v1', 'status': 'NotReady',
                          'execution_context': outer, 'execution_qualified': False}}
    permit = _preparation().register_public_runtime(
        invocation, [live[role]['build_probes'][profile] for role in ('ordinary', 'lifecycle') for profile in PROFILES],
        [live[role]['metadata_probe'] for role in ('ordinary', 'lifecycle')])
    _preparation().register_public_contracts(invocation, permit, contract_root, contracts.path_map, amendment_root)
    _preparation().register_public_tools(invocation, permit, runtime_tools.setup_probes())
    state['runtime_permit'] = permit
    # The inner amendment is deliberately left exactly as originally issued.
    need(predecessors.qualified_paths_amendment['execution_source_manifest']['sha256'] == _binding().PREDECESSOR_SOURCE_SHA,
         'retained semantic amendment relabelled')
    admission = object.__new__(RuntimeAdmission)
    state['_read_owner_seed'] = _preparation()
    _ADMISSIONS[admission] = state
    _checkpoint(state, full=True)
    return admission


def _outer(row, cfg, **values):
    return {**receipt_identity(row), 'execution_context': copy.deepcopy(cfg['_execution_context']),
            'candidate_binding_sha256': cfg['_sha256'], 'freeze_sha256': FREEZE_SHA,
            'adapter': copy.deepcopy(cfg['_consumer']), **values}


def _check_outer(row, got, cfg):
    need(all(got.get(k) == v for k, v in receipt_identity(row).items()), 'current tuple identity')
    need(got['execution_context'] == cfg['_execution_context'] and got['adapter'] == cfg['_consumer'],
         'current-source consumer/host/session context')
    need(got['candidate_binding_sha256'] == cfg['_sha256'] and got['freeze_sha256'] == FREEZE_SHA,
         'current build/contract binding')
    need(got['execution_context']['source_manifest_sha256'] == _binding().CURRENT_SOURCE_SHA and
         got['execution_context']['retained_semantic_source_manifest_sha256'] == _binding().PREDECESSOR_SOURCE_SHA,
         'new execution and retained semantic identities swapped')


def _owned_process(owner, argv, cwd, env, *, native=False):
    if owner.get('worker') is not None:
        _binding().hosted_api().verify_worker(owner['worker'])
        return owner['worker'].process(argv, cwd, env, timeout=120, limit=8 * 1024 ** 2)
    state = owner
    need(_active_boundary() is None, 'target launch forbidden inside read-only boundary')
    _checkpoint(state)
    api = _preparation()
    record = api.owned_probe(state['invocation'], argv, cwd, env, timeout_seconds=120,
                             stream_limit_bytes=8 * 1024 ** 2,
                             operation_kind='public-native' if native else 'public-runtime',
                             runtime_permit=state['runtime_permit'], runtime_case=state['active_case'],
                             native_permit=state['native_permits'].get(str(argv[0])) if native else None)
    # Retain an issued failure outcome before any validation can raise.
    document = record.document()
    state['probes'][document['pid'], document['started_ns']] = record
    state['process_cases'][document['pid'], document['started_ns']] = copy.deepcopy(state['active_case'])
    with _read_boundary(state):
        checked = api.verify_probe(record, state['invocation'], argv=argv, cwd=cwd, environment=env,
                                    timeout_seconds=120, stream_limit_bytes=8 * 1024 ** 2,
                                    operation_kind='public-native' if native else 'public-runtime',
                                 runtime_permit=state['runtime_permit'], runtime_case=state['active_case'],
                                 native_permit=state['native_permits'].get(str(argv[0])) if native else None)
        process = dict(checked)
        for stream in ('stdout', 'stderr'):
            raw = getattr(record, stream)()
            process[stream] = raw.decode('utf-8', errors='strict')
            process[stream + '_sha256'] = sha(raw)
    _checkpoint(state)
    return process


def _verify_process(owner, process, expected_case=None):
    if owner.get('worker') is not None:
        # The outer parent independently validates the whole worker process table.
        _infrastructure('worker-owner', lambda: _binding().hosted_api().verify_worker(owner['worker']))
        return
    key = process['pid'], process['started_ns']
    def readmit_original():
        need(key in owner['probes'], 'unissued public runtime process')
        if expected_case is not None:
            need(owner['process_cases'][key] == expected_case, 'process belongs to another frozen tuple/variant')
        record = owner['probes'][key]
        original = record.document()
        if not original['cleanup_complete'] or original['cleanup_error'] is not None:
            raise InfrastructureFailure('producer-cleanup', RuntimeError('producer cleanup incomplete'))
        # Only original issued arguments enter the producer. Candidate mutations
        # are compared after owner, evidence, completion and cleanup readmission.
        doc = _preparation().verify_probe(
            record, owner['invocation'], argv=original['argv'], cwd=original['cwd'],
            environment=original['environment'], timeout_seconds=120, stream_limit_bytes=8 * 1024 ** 2,
            operation_kind=original['operation_kind'], runtime_permit=owner['runtime_permit'],
            runtime_case=owner['process_cases'][key],
            native_permit=owner['native_permits'].get(original['argv'][0]))
        expected = dict(doc)
        for stream in ('stdout', 'stderr'):
            raw = getattr(record, stream)()
            expected[stream], expected[stream + '_sha256'] = raw.decode('utf-8'), sha(raw)
        return expected
    expected = _infrastructure('producer-readmission', readmit_original)
    need(process == expected, 'runtime raw process changed')


def _native(fixture, evidence, sources, env, owner, compiler_process):
    original = fixture / 'out.bin'
    data = _read(original, owner.get('invocation'))
    need(data[:6] == b'\x7fELF\x02\x01' and len(data) >= 20 and int.from_bytes(data[18:20], 'little') == 62,
         'native output is not x86_64 ELF')
    dest = evidence / 'native-source-free'
    dest.mkdir()
    copied = dest / 'program'
    with copied.open('xb') as stream:
        stream.write(data)
    copied.chmod(stat.S_IMODE(original.stat().st_mode))
    before = rt.inventory(dest)
    need(before == [{'path': 'program', 'kind': 'file', 'bytes': len(data), 'sha256': sha(data)}], 'native ELF-only directory')
    result = {'original_elf': binding(original), 'copied_elf': binding(copied), 'directory_before': before}
    originals = {name: (fixture / name).read_bytes() for name in sources}
    modes = {name: stat.S_IMODE((fixture / name).stat().st_mode) for name in sources}
    key = compiler_process['pid'], compiler_process['started_ns']
    owner['native_permits'][str(copied)] = _preparation().register_public_native(
        owner['invocation'], owner['probes'][key], runtime_permit=owner['runtime_permit'])
    try:
        for name in originals:
            (fixture / name).unlink()
        result['sources_unavailable'] = [name for name in sources if not os.path.lexists(fixture / name)]
        need(result['sources_unavailable'] == list(sources), 'source-free removal roster')
        result['process'] = _owned_process(owner, [str(copied)], dest, env, native=True)
        result['directory_after'] = rt.inventory(dest)
    finally:
        for name, raw in originals.items():
            (fixture / name).write_bytes(raw)
            (fixture / name).chmod(modes[name])
        result['restored_sources'] = [{'path': name, 'bytes': len(raw), 'sha256': sha((fixture / name).read_bytes())}
                                      for name, raw in sorted(originals.items())]
        result['sources_restored_exactly'] = all((fixture / name).read_bytes() == raw and
                                                stat.S_IMODE((fixture / name).stat().st_mode) == modes[name]
                                                for name, raw in originals.items())
    return result


def _collect_one(row, cfg, evidence, sources, env, owner):
    evidence.mkdir(parents=True, exist_ok=False)
    fixture = evidence / 'fixture'
    wanted = rt.materialize(sources, fixture)
    got = _outer(row, cfg, executed=True, status='fail', source_files=wanted,
                 fixture_directory=str(fixture), directory_before=rt.inventory(fixture))
    if row['group'] in ('Unit2', 'Unit3'):
        got['projection_kind'] = row['case']['kind']
        for key in ('expected_authority_line_sha256', 'expected_projection_sha256', 'source_line_sha256'):
            if key in row['case']:
                got[key] = row['case'][key]
    binary = cfg['binaries'][row['profile']]
    got['binary'], got['binary_stat_before'] = binary, rt.file_identity(binary['path'])
    need(got['binary_stat_before'] == cfg['_binary_stats'][row['profile']], 'compiler changed before runtime')
    setup = row.get('_filesystem_setup', row['case'].get('filesystem_setup'))
    if setup:
        got['capability_proof'] = rt.capability(fixture, setup)
        if got['capability_proof']['read_open_succeeded']:
            rt.restore_capability(fixture, got['capability_proof'])
            got.update(executed=False, status='unsupported-capability', reason=owner['helpers']['contracts'].CAPABILITY_REASON)
            got['directory_after'] = rt.inventory(fixture)
            if owner.get('worker') is None:
                _checkpoint(owner)
            return got
    log, trace = evidence / 'tools.jsonl', evidence / 'lifecycle.jsonl'
    per_env = {**env, 'UNIT4_TOOL_LOG': str(log), 'UNIT4_TOOL_EVIDENCE': str(evidence),
               'UNIT4_ADAPTER_ROOT': str(Path(owner['helpers']['runtime'].__file__).parent)}
    if cfg['role'] == 'lifecycle':
        per_env['UNIT4_LIFECYCLE_LOG'] = str(trace)
    try:
        got['process'] = _owned_process(owner, [binary['path']] + row['argv_template'][1:], fixture, per_env)
    finally:
        if setup:
            rt.restore_capability(fixture, got['capability_proof'])
    save(evidence / 'process.json', got['process'])
    for stream in ('stdout', 'stderr'):
        (evidence / stream).write_bytes(got['process'][stream].encode())
    got['tool_invocations'], got['trace'] = rt.read_events(log), rt.read_events(trace)
    got['emitted_llvm'] = binding(evidence / 'program.ll') if (evidence / 'program.ll').exists() else None
    if got['process']['status'] == 0 and row['argv_template'][1] == 'compile':
        got['native_execution'] = _native(fixture, evidence, sources, per_env, owner, got['process'])
    got['directory_after'], got['binary_stat_after'] = rt.inventory(fixture), rt.file_identity(binary['path'])
    return got


# Retained semantic predicates with the separately named current execution boundary.
def check_common(row, got, cfg, sources, tools, expected_native=None, native_required=None):
    _check_outer(row, got, cfg)
    if not got['executed']:
        need(got['status'] == 'unsupported-capability', 'nonexecuted candidate receipt')
        need(got['directory_before'] == got['directory_after'], 'capability skip source preservation')
        wanted = [{'path': name, 'bytes': len(data), 'sha256': sha(data)} for name, data in sorted(sources.items())]
        need(got['source_files'] == wanted and rt.source_map(got['directory_before']) == wanted, 'capability skip source map')
        proof = got['capability_proof']
        need(proof['source_restored'] and proof['content_before_sha256'] == proof['content_after_sha256'] == sha(sources[proof['target']]), 'capability skipped source bytes')
        return
    need(got['binary'] == cfg['binaries'][row['profile']], 'wrong executable identity')
    need(got['binary_stat_before'] == got['binary_stat_after'] == cfg['_binary_stats'][row['profile']], 'executable mutation')
    wanted = [{'path': name, 'bytes': len(data), 'sha256': sha(data)} for name, data in sorted(sources.items())]
    need(got['source_files'] == wanted and rt.source_map(got['directory_before']) == wanted, 'input source identity')
    p = got['process']
    need(not p['timed_out'] and not p['stream_limit_exceeded'] and p['completed_ns'] >= p['started_ns'], 'process incomplete')
    need(p['argv'] == [got['binary']['path']] + row['argv_template'][1:] and p['cwd'] == got['fixture_directory'], 'invocation argv/cwd')
    for stream in ('stdout', 'stderr'):
        need(sha(p[stream].encode()) == p[stream + '_sha256'], 'process stream bytes/hash')
    native_required = p['status'] == 0 and row['argv_template'][1] == 'compile' if native_required is None else native_required
    after = got['directory_after']
    delta = [r['path'] for r in after if r not in got['directory_before']]
    need(delta == (['out.bin'] if native_required else []), 'unexpected directory delta')
    need(all(r in after for r in got['directory_before']), 'changed/removed source')
    need(rt.source_map(after, exclude=('out.bin',)) == wanted, 'post-run source identity')
    if 'capability_proof' in got:
        proof = got['capability_proof']
        need(not proof['read_open_succeeded'] and proof['source_restored'] and proof['mode_before_candidate'] == proof['mode_after_candidate'] == 0, 'permission control proof')
    rt.check_tools(got['tool_invocations'], tools, native_required, got['emitted_llvm'])
    if native_required:
        need(expected_native is not None and 'native_execution' in got, 'successful compile missing frozen/native envelope')
        rt.check_native(got['native_execution'], expected_native, sources)
    else:
        need('native_execution' not in got, 'orphan native receipt')

def core_compare(section, row, got, cfg, contracts, sources, tools, predecessors):
    policy = contracts.tables['public']['new_policy_diagnostic_bounds']
    expected_native = row['case'].get('native_execution')
    check_common(row, got, cfg, sources, tools, expected_native)
    if not got['executed']:
        return
    if section in ('public', 'original'):
        compare.public(row, got['process'], policy)
        if row['format'] == 'json':
            _, diagnostics = compare.envelope(got['process'], row['argv_template'][1])
            compare.valid_origins(diagnostics, sources)
        if row['group'] == 'literal_cases':
            delta = [r['path'] for r in got['directory_after'] if r not in got['directory_before']]
            need(delta == row['observation']['directory_delta'], 'literal directory delta')
    elif section == 'predecessors':
        need(got['projection_kind'] == row['case']['kind'], 'predecessor projection classification')
        for field in ('expected_authority_line_sha256', 'expected_projection_sha256', 'source_line_sha256'):
            if field in row['case']:
                need(got[field] == row['case'][field], 'predecessor authority identity: ' + field)
        check_predecessor_amendment(row, got, predecessors)
        predecessors.compare(row, got['process'], policy)

def check_predecessor_amendment(row, got, predecessors):
    """Derive the extra receipt from unchanged process bytes, never its metadata."""
    expected = predecessors.qualified_paths_comparison(row, got['process']) if got['executed'] else None
    fields = {key for key in got if key.startswith('qualified_paths_')}
    need(fields == ({'qualified_paths_comparison'} if expected is not None else set()),
         'missing/extra predecessor semantic comparison')
    if expected is not None:
        need(json.dumps(got['qualified_paths_comparison'], sort_keys=True, allow_nan=False) ==
             json.dumps(expected, sort_keys=True, allow_nan=False), 'altered predecessor semantic comparison')
    return expected

def predecessor_history(rows, receipts, predecessors, policy, observations, *, checkpoint=None):
    """Replay the frozen comparator separately; an amended pass is not a historical pass."""
    need(len(rows) == len(receipts), 'historical predecessor receipt count')
    mismatches, comparisons = [], []
    executed = 0
    for row, got in zip(_CheckpointedRows(rows, checkpoint), _CheckpointedRows(receipts, checkpoint)):
        need(got['key'] == row['key'], 'historical predecessor tuple order')
        if not got['executed']:
            continue
        executed += 1
        try:
            comparison = predecessors.qualified_paths_comparison(row, got['process'])
            if comparison is not None:
                comparisons.append({'key': row['key'], 'comparison': comparison})
            predecessors.compare(row, got['process'], policy, historical=True)
        except (Rejected, Reject, KeyError, TypeError, ValueError) as error:
            mismatches.append({'key': row['key'], 'failure': str(error)})
    return {'schema': 'oxid-unit4-public-predecessor-historical-comparison-v1',
            'comparison_basis': 'unchanged-frozen-predecessor-public-projections',
            'freeze_sha256': FREEZE_SHA, 'observations': observations,
            'status': 'MISMATCH' if mismatches else 'MATCH', 'executed': executed,
            'skipped': len(rows) - executed, 'mismatches': mismatches,
            'qualified_paths_comparisons': comparisons, 'historical_qualification_claim': False,
            'raw_observations_unchanged': True}

def predecessor_report_fields(predecessors, history, authority_binding, history_binding):
    return {'comparison_basis': 'frozen-predecessor-projections-plus-enum-enabled-qualified-values-v1',
            'qualified_paths_amendment': predecessors.qualified_paths_amendment,
            'qualified_paths_amendment_receipt': authority_binding,
            'qualified_paths_amended_keys': [row['key'] for row in history['qualified_paths_comparisons']],
            'historical_comparison': history_binding}

def lifecycle_compare(row, ordinary, observed, cfg, observer_cfg, contracts, sources, tools):
    if not observed['executed']:
        need(not ordinary['executed'], 'observer capability inconsistent')
        check_common(row, ordinary, cfg, sources, tools)
        check_common(row, observed, observer_cfg, sources, tools)
        return
    case = row['case']
    source_case = contracts.cases.get(row['case_id'])
    native_expected = source_case.get('native_execution') if source_case else None
    for got, config in ((ordinary, cfg), (observed, observer_cfg)):
        check_common(row, got, config, sources, tools, native_expected)
    for field in ('status', 'stdout', 'stderr'):
        need(observed['process'][field] == ordinary['process'][field], 'observer passivity: ' + field)
    need(observed['directory_after'] == ordinary['directory_after'], 'observer directory/ELF passivity')
    if row['group'] == 'public':
        mainrow = next(r for r in contracts.roster('public', row['execution_host']) if r['case_id'] == row['case_id'] and r['observation_index'] == row['observation_index'] and r['profile'] == row['profile'] and r['host'] == row['host'])
        compare.public(mainrow, ordinary['process'], contracts.tables['public']['new_policy_diagnostic_bounds'])
    else:
        need(ordinary['process']['status'] == case['expected_status'], 'heldout expected process status')
    if row['format'] == 'json' and ordinary['process']['stdout']:
        # Invalid CLI combinations can intentionally report only stderr, so only
        # successfully framed typed-preview streams have source origins here.
        if ordinary['process']['stdout'].startswith('{"schema_version":'):
            _, diagnostics = compare.envelope(ordinary['process'], row['argv_template'][1])
            compare.valid_origins(diagnostics, sources)
    table = contracts.tables['lifecycle']
    observed['logical_lifecycle'] = compare.lifecycle(observed['trace'], case, table['counter_order'], table['file_counter_order'])
    need(ordinary['trace'] == [], 'ordinary binary emitted observer events')

class _CheckpointedRows:
    """Re-iterable bool/len/iteration adapter for the unchanged retained checker."""
    def __init__(self, rows, checkpoint):
        self.rows, self.checkpoint = rows, checkpoint

    def __bool__(self):
        return bool(self.rows)

    def __len__(self):
        return len(self.rows)

    def __iter__(self):
        for row in self.rows:
            _work_checkpoint(self.checkpoint)
            yield row
            _work_checkpoint(self.checkpoint)


def _work_checkpoint(checkpoint):
    if checkpoint is not None:
        _infrastructure('readback-checkpoint', checkpoint)


def _copy_receipts(receipts, checkpoint):
    copied = []
    memo = {id(receipts): copied}
    for got in _CheckpointedRows(receipts, checkpoint):
        copied.append(copy.deepcopy(got, memo))
    return copied


def _replay_receipts(rows, receipts, checker, checkpoint, *, executed_only=False):
    for row, got in zip(_CheckpointedRows(rows, checkpoint), _CheckpointedRows(receipts, checkpoint)):
        if not executed_only or got['executed']:
            checker(row, got)
    _work_checkpoint(checkpoint)


def aggregate(rows, receipts, *, checkpoint=None):
    rows, receipts = _CheckpointedRows(rows, checkpoint), _CheckpointedRows(receipts, checkpoint)
    counts = check_inventory(rows, receipts)
    failures = [{'key': r['key'], 'failure': r.get('failure', 'predicate failure')} for r in receipts if r['status'] == 'fail']
    need(not failures, json.dumps(failures[:25]))
    return {**counts, 'status': ('NOT_APPLICABLE_THIS_HOST' if counts['required_local'] == 0 else ('INCOMPLETE_LOCAL_CAPABILITY' if counts['unsupported_capability'] else 'QUALIFIED_LOCAL')),
            'global_host_qualification': counts['unavailable_host'] == 0 and counts['unsupported_capability'] == 0}

def negative_controls(rows, receipts, checker, *, checkpoint=None):
    aggregate(rows, receipts, checkpoint=checkpoint)
    index = next((i for i, r in enumerate(_CheckpointedRows(receipts, checkpoint)) if r['executed']), None)
    controls = []
    def run(name, change):
        mutated = _copy_receipts(receipts, checkpoint)
        change(mutated)
        try:
            aggregate(rows, mutated, checkpoint=checkpoint)
            _replay_receipts(rows, mutated, checker, checkpoint, executed_only=True)
        except (Rejected, Reject) as error:
            rejected = {'control': name, 'status': 'REJECTED', 'reason': str(error)[:500]}
        else:
            raise Rejected('negative control accepted: ' + name)
        # A poisoned producer or failed service is not an expected semantic result.
        _work_checkpoint(checkpoint)
        controls.append(rejected)
    if index is None:
        need(not any(row['scope'] == 'execute' for row in rows), 'unexecuted required local domain')
        index = 0
        run('zero', lambda x: x.clear())
        run('missing', lambda x: x.pop(0))
        run('duplicate', lambda x: x.append(copy.deepcopy(x[0])))
        run('extra', lambda x: x.append({**copy.deepcopy(x[0]), 'key': 'extra'}))
        run('fabricated-execution', lambda x: x[0].update(executed=True, status='pass'))
        return controls
    run('zero', lambda x: x.clear())
    run('missing', lambda x: x.pop(index))
    run('duplicate', lambda x: x.append(copy.deepcopy(x[index])))
    run('extra', lambda x: x.append({**copy.deepcopy(x[index]), 'key': 'extra'}))
    for name, field, value in (('false-skip', 'executed', False), ('failed-predicate', 'status', 'fail'), ('wrong-host', 'host', 'other'), ('wrong-profile', 'profile', 'other'), ('wrong-format', 'format', 'other'), ('stale-contract', 'freeze_sha256', '0' * 64), ('stale-candidate', 'candidate_binding_sha256', '0' * 64)):
        run(name, lambda x, k=field, v=value: x[index].__setitem__(k, v))
    run('wrong-argv', lambda x: x[index]['process']['argv'].append('--invented'))
    run('changed-source', lambda x: x[index]['source_files'][0].__setitem__('sha256', '0' * 64))
    run('wrong-stream', lambda x: x[index]['process'].__setitem__('stdout', 'wrong'))
    native_index = next((i for i, r in enumerate(_CheckpointedRows(receipts, checkpoint)) if r.get('native_execution')), None)
    if native_index is not None:
        run('missing-native', lambda x: x[native_index].pop('native_execution'))
        run('sources-available', lambda x: x[native_index]['native_execution'].__setitem__('sources_unavailable', []))
        run('changed-ELF', lambda x: x[native_index]['native_execution']['copied_elf'].__setitem__('sha256', '0' * 64))
        run('missing-tools', lambda x: x[native_index].__setitem__('tool_invocations', []))
    traced = next((i for i, r in enumerate(_CheckpointedRows(receipts, checkpoint)) if r.get('trace')), None)
    if traced is not None:
        run('missing-passivity-binding', lambda x: x[traced].pop('ordinary_receipt'))
        run('missing-trace', lambda x: x[traced].__setitem__('trace', []))
        run('duplicate-checker', lambda x: x[traced]['trace'].append({'event': 'checker_attempts', 'subject': 'project'}))
        run('source-reopen', lambda x: x[traced]['trace'].append({'event': 'source_open_attempt', 'subject': 'main.ox'}))
    race_index = next((i for i, r in enumerate(_CheckpointedRows(receipts, checkpoint)) if r['group'] == 'race' and r['executed']), None)
    if race_index is not None:
        run('changed-competitor', lambda x: x[race_index]['output_after'].__setitem__('sha256', '0' * 64))
        run('missing-race-barrier', lambda x: next(c for c in x[race_index]['tool_invocations'] if 'race' in c).pop('race'))
    return controls


def output_state(output, target=None):
    return {'kind': 'symlink' if output.is_symlink() else 'regular', 'sha256': sha(output.read_bytes()),
            'identity': rt.file_identity(output, follow=False), 'link_target': os.readlink(output) if output.is_symlink() else None,
            'target_sha256': sha(target.read_bytes()) if target else None,
            'target_identity': rt.file_identity(target) if target else None}

def substitution(text, root, output):
    return text.replace('${FIXTURE_ROOT}', str(root)).replace('${OUTPUT}', str(output))

def check_guard(row, got, cfg, contracts, sources, tools):
    _check_outer(row, got, cfg)
    need(got['binary'] == cfg['binaries'][row['profile']] and got['binary_stat_before'] == got['binary_stat_after'] == cfg['_binary_stats'][row['profile']], 'guard binary')
    wanted = [{'path': k, 'bytes': len(v), 'sha256': sha(v)} for k, v in sorted(sources.items())]
    need(got['source_files'] == wanted and rt.source_map(got['directory_before']) == wanted, 'guard sources')
    need(got['directory_after'] == got['directory_before'], 'guard source/unrelated mutation')
    need(got['native_workspaces_after'] == [], 'guard scratch cleanup')
    p = got['process']
    expected_argv = [cfg['binaries'][row['profile']]['path']] + [substitution(a, got['fixture_root'], got['output']) for a in row['argv_template'][1:]]
    need(p['argv'] == expected_argv and p['cwd'] == got['fixture_directory'], 'guard argv/cwd')
    need(not p['timed_out'] and not p['stream_limit_exceeded'], 'guard incomplete process')
    for stream in ('stdout', 'stderr'):
        need(sha(p[stream].encode()) == p[stream + '_sha256'], 'guard stream hash')
    case = row['case']
    if row['group'] == 'occupied':
        need(got['output_before'] == got['output_after'] and got['output_before']['kind'] == case['output_kind'], 'occupied output/target identity mutation')
        need(p['status'] == case['frozen_envelope']['exit_status'], 'occupied status')
        for stream in ('stdout', 'stderr'):
            need(p[stream] == substitution(case['frozen_envelope'][stream], got['fixture_root'], got['output']), 'occupied literal ' + stream)
        rt.check_tools(got['tool_invocations'], tools, False, got['emitted_llvm'])
    else:
        need(got['output_initially_absent'] and p['status'] == case['expected_status'], 'race initial/status')
        summary, diagnostics = compare.envelope(p, 'compile')
        need(summary == case['summary'], 'race complete summary')
        exact_summary = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'compile-summary', 'success': False, 'errors': 1, 'output': None}
        need(p['stdout'].splitlines(keepends=True)[-1] == json.dumps(exact_summary, separators=(',', ':')) + '\n', 'race exact summary line')
        compare.diagnostic_vector(diagnostics, [case['diagnostic_projection']])
        compare.valid_origins(diagnostics, sources)
        rt.check_tools(got['tool_invocations'], tools, True, got['emitted_llvm'])
        calls = got['tool_invocations']
        injected = [call for call in calls if 'race' in call]
        need(len(injected) == 1, 'race injection count')
        final = injected[0]
        need(final['tool'] == 'clang' and final['status'] == 0, 'race successful link')
        race = final['race']
        need(final['completed_ns'] <= race['linked_verified_ns'] <= race['absent_verified_ns'] <= race['created_ns'] <= race['closed_ns'] <= race['readback_verified_ns'] <= final['wrapper_return_ready_ns'] <= p['completed_ns'], 'race causal order')
        need(race['created_exclusively'] and race['output'] == got['output'], 'race exclusive competitor')
        need(race['sha256'] == case['payload_sha256'] == got['output_after']['sha256'], 'race competitor bytes')
        need(got['output_after']['kind'] == 'regular' and race['file_identity'] == got['output_after']['identity'], 'race competitor inode preservation')
        need(race['linked_elf_sha256'] == got['linked_artifact']['sha256'] and got['linked_artifact_is_elf'] and got['linked_artifact_executed'] is False, 'race linked artifact')
        direct = sorted([c for c in calls if c['wrapper_ppid'] == p['pid']], key=lambda c: c['started_ns'])
        need(len(direct) == 7 and direct[-1] is final, 'race direct phase inventory')
        names = [c['tool'] for c in direct]
        need(names == ['clang', 'opt', 'ld.lld', 'opt', 'clang', 'clang', 'clang'], 'race tool phase order')
        need(all(c['argv'] == ['--version'] for c in direct[:3]), 'race version argv')
        need(direct[3]['argv'] == ['-passes=verify', '-disable-output', 'program.ll'], 'race verify argv')
        nested = [c for c in calls if c['wrapper_ppid'] != p['pid']]
        need(nested and all(c['tool'] == 'ld.lld' and c['wrapper_ppid'] == final['real_tool_pid'] and final['started_ns'] <= c['started_ns'] <= c['wrapper_return_ready_ns'] <= final['completed_ns'] for c in nested), 'real nested linker evidence')
        need(any('program.o' in c['argv'] and 'runtime.o' in c['argv'] for c in nested), 'real object linking')


def _guard_execute(row, config, evidence, fixture, fixture_root, output, target, sources, env, owner, before_output=None):
    evidence.mkdir(parents=True, exist_ok=True)
    binary = config['binaries'][row['profile']]
    log = evidence / 'tools.jsonl'
    trace = evidence / 'lifecycle.jsonl'
    per_env = {**env, 'UNIT4_TOOL_LOG': str(log), 'UNIT4_TOOL_EVIDENCE': str(evidence),
               'UNIT4_ADAPTER_ROOT': str(Path(owner['helpers']['runtime'].__file__).parent)}
    if config['role'] == 'lifecycle':
        per_env['UNIT4_LIFECYCLE_LOG'] = str(trace)
    if row['group'] == 'race':
        per_env['UNIT4_RACE_OUTPUT'] = str(output)
    got = {**receipt_identity(row), 'executed': True, 'status': 'fail', 'candidate_binding_sha256': config['_sha256'],
           'freeze_sha256': FREEZE_SHA, 'adapter': config['_consumer'], 'execution_context': config['_execution_context'], 'binary': binary, 'binary_stat_before': rt.file_identity(binary['path']),
           'source_files': [{'path': k, 'bytes': len(v), 'sha256': sha(v)} for k, v in sorted(sources.items())],
           'directory_before': rt.inventory(fixture), 'fixture_directory': str(fixture), 'fixture_root': str(fixture_root),
           'output': str(output), 'output_before': before_output, 'output_initially_absent': not os.path.lexists(output)}
    need(got['binary_stat_before'] == config['_binary_stats'][row['profile']], 'guard stale binary before invocation')
    argv = [binary['path']] + [substitution(a, fixture_root, output) for a in row['argv_template'][1:]]
    got['process'] = _owned_process(owner, argv, fixture, per_env)
    save(evidence / 'process.json', got['process'])
    got['binary_stat_after'] = rt.file_identity(binary['path'])
    # Race output is explicitly permitted; retain complete directory change too.
    got['complete_directory_after'] = rt.inventory(fixture)
    got['directory_after'] = [r for r in got['complete_directory_after'] if not (row['group'] == 'race' and r['path'] == 'out.bin')]
    got['output_after'] = output_state(output, target) if os.path.lexists(output) else None
    got['tool_invocations'] = rt.read_events(log)
    got['trace'] = rt.read_events(trace)
    got['native_workspaces_after'] = [str(p.relative_to(evidence.parent)) for p in evidence.parent.rglob('.oxid-native-*')]
    got['emitted_llvm'] = binding(evidence / 'program.ll') if (evidence / 'program.ll').exists() else None
    if row['group'] == 'race':
        linked = evidence / 'linked-before-publication.elf'
        got['linked_artifact'] = binding(linked) if linked.exists() else None
        got['linked_artifact_is_elf'] = linked.exists() and linked.read_bytes()[:6] == b'\x7fELF\x02\x01'
        got['linked_artifact_executed'] = False
        need([r['path'] for r in got['complete_directory_after'] if r not in got['directory_before']] == row['case']['expected_directory_delta'], 'race complete directory delta')
    for stream in ('stdout', 'stderr'):
        (evidence / stream).write_text(got['process'][stream])
    return got

def _run_guards(rows, output_root, cfg, observer_cfg, contracts, env, tools, owner):
    receipts = []
    for index, row in enumerate(rows):
        if row['scope'] != 'execute':
            receipts.append(_skipped(row, cfg))
            continue
        with _preparation().public_case_scope(owner['invocation'], owner['runtime_permit'], 'guards', index):
            owner['active_case'] = {'section': 'guards', 'index': index, 'variant': 'ordinary'}
            case = row['case']
            source_case = contracts.cases[case['source_case_id']]
            sources = contracts.source_bytes(source_case)
            evidence = output_root / ('%03d' % index)
            fixture_root = evidence / 'fixtures'
            fixture = fixture_root / case['source_case_id']
            rt.materialize(sources, fixture)
            target = evidence / 'preserved-target' if row['group'] == 'occupied' else None
            output = evidence / 'occupied-output' if row['group'] == 'occupied' else fixture / 'out.bin'
            if target:
                target.write_bytes(b'Oxid Unit3 no-clobber sentinel\x00\xff\n')
                if case['output_kind'] == 'symlink':
                    output.symlink_to(target.name)
                    _preparation().register_public_guard(owner['invocation'], owner['runtime_permit'], owner['active_case'])
                else:
                    output.write_bytes(b'Oxid Unit3 occupied output sentinel\x00\xfe\n')
            before = output_state(output, target) if target else None
            got = {**receipt_identity(row), 'executed': True, 'status': 'fail'}
            try:
                ordinary = _guard_execute(row, cfg, evidence / 'ordinary', fixture, fixture_root, output, target, sources, env, owner, before)
                check_guard(row, ordinary, cfg, contracts, sources, tools)
                with _read_boundary(owner):
                    _verify_process(owner, ordinary['process'])
                need(ordinary['trace'] == [], 'ordinary guard observer trace')
                ordinary['status'] = 'pass'
                save(evidence / 'ordinary' / 'receipt.json', ordinary)
                if row['group'] == 'race':
                    # Remove only this run's proven generated competitor, after saving
                    # its bytes and identity. Both runs start with the frozen absent state.
                    (evidence / 'ordinary' / 'competitor.bytes').write_bytes(output.read_bytes())
                    output.unlink()
                owner['active_case'] = {'section': 'guards', 'index': index, 'variant': 'observer'}
                got = _guard_execute(row, observer_cfg, evidence / 'observer', fixture, fixture_root, output, target, sources, env, owner, before)
                check_guard(row, got, observer_cfg, contracts, sources, tools)
                for field in ('status', 'stdout', 'stderr'):
                    need(got['process'][field] == ordinary['process'][field], 'guard observer passivity ' + field)
                need(got['directory_after'] == ordinary['directory_after'], 'guard observer source passivity')
                expected = next(c for c in contracts.tables['lifecycle']['public_cases'] if c['id'] == case['source_case_id'])
                need(expected['expected_lifecycle'] == case['expected_lifecycle'], 'guard declared lifecycle binding')
                expected = copy.deepcopy(expected)
                if row['group'] == 'occupied':
                    expected['file_counts'] = {str(fixture / name): value for name, value in expected['file_counts'].items()}
                got['logical_lifecycle'] = compare.lifecycle(got['trace'], expected, contracts.tables['lifecycle']['counter_order'], contracts.tables['lifecycle']['file_counter_order'])
                got['ordinary_receipt'] = binding(evidence / 'ordinary' / 'receipt.json')
                with _read_boundary(owner):
                    _verify_process(owner, got['process'])
                got['status'] = 'pass'
                save(evidence / 'observer' / 'receipt.json', got)
            except (Exception,) as error:
                got.update(status='fail', failure=str(error))
            save(evidence / 'receipt.json', got)
            receipts.append(got)
            with _read_boundary(owner):
                _validate_case_evidence(owner, 'guards', index, got)
    return receipts

def _verify_guard_complete(row, got, observer_cfg, contracts, sources, tools, ordinary_cfg):
    if not got['executed']:
        return
    check_guard(row, got, observer_cfg, contracts, sources, tools)
    need('ordinary_receipt' in got, 'missing ordinary passivity binding')
    bound = got['ordinary_receipt']
    _infrastructure('ordinary-evidence', verify, bound['path'], bound)
    ordinary = _infrastructure('ordinary-evidence', load, bound['path'])
    check_guard(row, ordinary, ordinary_cfg, contracts, sources, tools)
    need(all(got['process'][field] == ordinary['process'][field] for field in ('status', 'stdout', 'stderr')), 'saved guard passivity envelope')
    need(got['directory_after'] == ordinary['directory_after'], 'saved guard passivity directory')
    expected = copy.deepcopy(next(c for c in contracts.tables['lifecycle']['public_cases'] if c['id'] == row['case']['source_case_id']))
    if row['group'] == 'occupied':
        expected['file_counts'] = {str(Path(got['fixture_directory']) / name): value for name, value in expected['file_counts'].items()}
    compare.lifecycle(got['trace'], expected, contracts.tables['lifecycle']['counter_order'], contracts.tables['lifecycle']['file_counter_order'])
    need('native_execution' not in got, 'guard orphan successful-native receipt')

    need(ordinary['trace'] == [], 'saved ordinary guard observer trace')


def _skipped(row, cfg):
    got = _ISSUED_HELPERS['run'].skipped(row)
    return {**_outer(row, cfg), **got}


def hosted_context(admission):
    state = _state(admission)
    _checkpoint(state, full=True)
    need(state['execution_context']['host'] == 'Linux x86_64', 'hosted capability is Linux only')
    rows = capability_rows(state['contracts'])
    sources = {row['key']: state['contracts'].source_bytes(state['contracts'].cases[row['case_id']])
               for section in ('public', 'lifecycle') for row in rows[section]}
    return {**{key: state[key] for key in ('invocation', 'ordinary', 'lifecycle', 'machine', 'runtime_tools',
                                          'contracts', 'helpers', 'root', 'runtime_permit')},
            'input_paths': copy.deepcopy(state['input_paths']),
            'helper_input_files': copy.deepcopy(state['helper_input_files']),
            'capability_rows': copy.deepcopy(rows), 'capability_sources': sources,
            'cfgs': copy.deepcopy(state['cfgs']), 'execution_context': copy.deepcopy(state['execution_context'])}


def collect_capability_shard(worker_admission):
    """Only a child-issued readonly WorkerAdmission can collect this partial shard."""
    api = _binding().hosted_api()
    inputs = api.verify_worker(worker_admission)
    owner = {**inputs, 'worker': worker_admission}
    cfgs, contracts = inputs['cfgs'], inputs['contracts']
    rows = capability_rows(contracts)
    need(rows == inputs['capability_rows'], 'worker exact capability rows')
    result = {'schema': 'package-unit4-public-capability-shard-v1', 'status': 'PARTIAL_CAPABILITY_SHARD',
              'complete_qualification': False, 'execution_context': inputs['execution_context'],
              'identity': inputs['identity'], 'sections': {}, 'ordinary_counterparts': {}}
    for section in ('public', 'lifecycle'):
        receipts = []
        for index, row in enumerate(rows[section]):
            api.verify_worker(worker_admission)
            sources = inputs['capability_sources'][row['key']]
            row = copy.deepcopy(row)
            if section == 'lifecycle':
                row['_filesystem_setup'] = contracts.cases[row['case_id']].get('filesystem_setup')
            evidence = Path(inputs['output']) / section / str(index)
            ordinary = _collect_one(row, cfgs['ordinary'], evidence / 'ordinary', sources, inputs['environment'], owner)
            api.require_denial_proof(ordinary, inputs['identity'])
            if section == 'lifecycle':
                got = _collect_one(row, cfgs['lifecycle'], evidence / 'observer', sources, inputs['environment'], owner)
                api.require_denial_proof(got, inputs['identity'])
                lifecycle_compare(row, ordinary, got, cfgs['ordinary'], cfgs['lifecycle'], contracts, sources, inputs['tools'])
                ordinary['status'] = got['status'] = 'pass'
                save(evidence / 'ordinary' / 'receipt.json', ordinary)
                got['ordinary_receipt'] = binding(evidence / 'ordinary' / 'receipt.json')
                save(evidence / 'observer' / 'receipt.json', got)
                result['ordinary_counterparts'][row['key']] = ordinary
            else:
                core_compare(section, row, ordinary, cfgs['ordinary'], contracts, sources, inputs['tools'], None)
                ordinary['status'] = 'pass'
                got = ordinary
                save(evidence / 'ordinary' / 'receipt.json', got)
            save(evidence / 'receipt.json', got)
            receipts.append(got)
        result['sections'][section] = receipts
    api.verify_worker(worker_admission)
    return result


def verify_capability_shard(admission, worker_inputs, raw_result, worker_process):
    """Parent replay; transported dictionaries never issue a parent admission."""
    state = _state(admission)
    _checkpoint(state, full=True)
    need(worker_process['cleanup_complete'] and worker_process['status'] == 0, 'owned hosted worker failed cleanup')
    need(raw_result['schema'] == 'package-unit4-public-capability-shard-v1' and
         raw_result['status'] == 'PARTIAL_CAPABILITY_SHARD' and raw_result['complete_qualification'] is False,
         'hosted shard cannot claim complete qualification')
    need(raw_result['execution_context'] == worker_inputs['execution_context'] and
         raw_result['identity'] == worker_inputs['identity'], 'hosted shard source/identity context')
    rows = capability_rows(state['contracts'])
    need(set(raw_result['sections']) == {'public', 'lifecycle'} and
         set(raw_result['ordinary_counterparts']) == {r['key'] for r in rows['lifecycle']}, 'exact hosted sections/counterparts')
    records = worker_inputs['process_records']
    need(len(records) == 21, 'hosted exact process roster')
    used = set()
    api = _binding().hosted_api()
    def replay_process(got):
        api.require_denial_proof(got, worker_inputs['identity'])
        process = got['process']
        key = process['pid'], process['started_ns']
        need(key in records and key not in used and process == records[key], 'unissued/reused hosted compiler process')
        api.inspect_nested_process(process, worker_process, worker_inputs['identity'])
        used.add(key)
        need('native_execution' not in got, 'hosted capability cannot run native output')
        evidence = Path(got['fixture_directory']).parent
        need(evidence.is_relative_to(Path(worker_inputs['output'])) and evidence.resolve() == evidence,
             'hosted row evidence escaped owned output')
        _verify_row_files(got)
    cfgs = worker_inputs['cfgs']
    for section in ('public', 'lifecycle'):
        receipts = raw_result['sections'][section]
        need(len(receipts) == len(rows[section]), 'hosted section cardinality')
        for row, got in zip(rows[section], receipts):
            need(got['status'] == 'pass' and got['executed'], 'mandatory hosted row did not execute')
            sources = worker_inputs['capability_sources'][row['key']]
            replay_process(got)
            if section == 'lifecycle':
                ordinary = raw_result['ordinary_counterparts'][row['key']]
                replay_process(ordinary)
                bound = got['ordinary_receipt']
                need(_binding().decode(_read(bound['path'])) == ordinary and _identity(bound['path']) == bound,
                     'hosted ordinary receipt bytes')
                _replay_lifecycle(row, ordinary, got, cfgs['ordinary'], cfgs['lifecycle'],
                                  state['contracts'], sources, worker_inputs['tools'])
            else:
                core_compare(section, row, got, cfgs['ordinary'], state['contracts'], sources, worker_inputs['tools'], None)
    need(len(used) == 18, 'all eighteen hosted compiler observations required')
    remaining = [value for key, value in records.items() if key not in used]
    need(len(remaining) == 3 and {Path(p['argv'][0]).name for p in remaining} == {'clang', 'opt', 'ld.lld'} and
         all(p['argv'][1:] == ['--version'] and p['status'] == 121 for p in remaining), 'hosted exact three direct trap controls')
    _checkpoint(state, full=True)
    return True


def _verify_row_files(got, resolver=None):
    if not got.get('executed'):
        return
    resolve = resolver or (lambda path: path)
    evidence = Path(got['fixture_directory']).parent
    # Guard fixtures are shared siblings, so their owned log directory is explicit.
    if 'fixture_root' in got:
        evidence = Path(got['process']['environment']['UNIT4_TOOL_EVIDENCE'])
    for stream in ('stdout', 'stderr'):
        need(_read(resolve(str(evidence / stream))) == got['process'][stream].encode(), 'saved process stream differs')
    need(_binding().decode(_read(resolve(str(evidence / 'process.json')))) == got['process'], 'saved process document differs')
    need(_infrastructure('tool-evidence', rt.read_events, Path(resolve(str(evidence / 'tools.jsonl')))) == got['tool_invocations'] and
         _infrastructure('lifecycle-evidence', rt.read_events, Path(resolve(str(evidence / 'lifecycle.jsonl')))) == got['trace'], 'saved tool/lifecycle events differ')
    if got.get('emitted_llvm'):
        need(_identity(resolve(got['emitted_llvm']['path']))['sha256'] == got['emitted_llvm']['sha256'], 'emitted LLVM differs')
    if got.get('native_execution'):
        for key in ('original_elf', 'copied_elf'):
            bound = got['native_execution'][key]
            need(_identity(resolve(bound['path'])) == bound, 'native emitted/copied bytes changed')


def _generator_check(state):
    """Run the unchanged generator through the current owned process boundary."""
    _checkpoint(state, full=True)
    contracts, helpers = state['contracts'], state['helpers']
    old = helpers['predecessors']
    generator = old.authority_path(contracts, '/typed_project_unit1_compatibility/generate_fixtures.py')
    package_root = Path(state['input_paths']['repository']) / COMPAT
    manifest_path = package_root / 'package-manifest.json'
    need(sha(_read(manifest_path, state['invocation'])) == '0f331a077a7e67b8e8943dfd463273f61ebbb64f08e2a60e0545b391479d3215',
         'unchanged Unit1 compatibility manifest')
    for name, row in load(manifest_path)['files'].items():
        verify(package_root / name, row)
    intent_path = old.authority_path(contracts, '/typed_project_unit1_compatibility/suite-intent.json')
    output = state['root'] / 'original-generator'
    output.mkdir()
    (output / 'fixtures').mkdir()
    copied = output / generator.name
    with copied.open('xb') as sink:
        sink.write(_read(generator, state['invocation']))
    api = _preparation()
    argv = [sys.executable, str(copied)]
    environment = state['runtime_tools'].environment()
    record = api.owned_probe(state['invocation'], argv, output, environment, timeout_seconds=60,
                             stream_limit_bytes=8 * 1024 ** 2, operation_kind='generator',
                             runtime_permit=state['runtime_permit'])
    state['generator_probe'] = record
    doc = api.verify_probe(record, state['invocation'], argv=argv, cwd=output, environment=environment,
                           timeout_seconds=60, stream_limit_bytes=8 * 1024 ** 2, operation_kind='generator',
                           runtime_permit=state['runtime_permit'])
    need(doc['status'] == 0 and record.stderr() == b'', 'original generator failed')
    raw_intent, projected_intent = output / 'suite-intent.json', output / 'suite-intent.frozen-newline.json'
    projected_intent.write_bytes(old.original_manifest_transport(_read(raw_intent), _read(intent_path), os.name))
    intent = load(intent_path)
    need(len(intent['cases']) == 35, 'complete original fixture count')
    for case in intent['cases']:
        _tick(state['invocation'])
        verify(output / case['path'], case)
    need({p.relative_to(output).as_posix() for p in (output / 'fixtures').rglob('*') if p.is_file()} ==
         {c['path'] for c in intent['cases']}, 'complete original source membership')
    for case in contracts.tables['original']['literal_cases']:
        for source in case['source_files']:
            need(_read(output / source['path']) == contracts.source_bytes(case)[source['path']], 'original generated source binding')
    receipt = {'generator': binding(generator), 'package_manifest': binding(manifest_path), 'intent': binding(intent_path),
               'sources': [{'path': c['path'], 'bytes': c['bytes'], 'sha256': c['sha256']} for c in intent['cases']],
               'manifest_newline_transport': {'platform': os.name, 'raw_generated': binding(raw_intent),
                   'frozen_newline_projection': binding(projected_intent),
                   'scope': 'Only generated suite-intent JSON platform newlines; no compiler stream/source normalization'},
               'compiler_invocations': 0, 'status': 'SOURCE_IDENTITIES_VERIFIED',
               'execution_context': state['execution_context'], 'owned_process': doc}
    _write(output / 'verification.json', receipt, state['invocation'])
    _checkpoint(state, full=True)
    return receipt


def _row_sources(state, section, row):
    contracts = state['contracts']
    if section == 'guards':
        return contracts.source_bytes(contracts.cases[row['case']['source_case_id']])
    return state['helpers']['run'].tuple_sources(row, contracts, state['predecessors'])


def _check_receipt(state, section, row, got):
    cfgs, contracts = state['cfgs'], state['contracts']
    tools = _infrastructure('runtime-tools', state['runtime_tools'].tools)
    if row['scope'] != 'execute':
        need(got == _skipped(row, cfgs['ordinary']), 'changed remote/excluded receipt')
        return
    cfg = cfgs['lifecycle'] if section in ('guards', 'lifecycle') else cfgs['ordinary']
    _check_outer(row, got, cfg)
    sources = _infrastructure('row-sources', _row_sources, state, section, row)
    case = {'section': section, 'index': state['row_indices'][section][row['key']],
            'variant': 'observer' if section in ('guards', 'lifecycle') else 'ordinary'}
    ordinary_case = dict(case, variant='ordinary')
    if got['executed']:
        _verify_process(state, got['process'], case)
        _verify_row_files(got)
        if 'native_execution' in got:
            _verify_process(state, got['native_execution']['process'], case)
    if section == 'lifecycle':
        need('ordinary_receipt' in got, 'missing ordinary passivity binding')
        bound = got['ordinary_receipt']
        need(_identity(bound['path'], state['invocation']) == bound, 'ordinary saved receipt changed')
        ordinary = _binding().decode(_read(bound['path'], state['invocation']))
        if ordinary['executed']:
            _verify_process(state, ordinary['process'], ordinary_case)
            _verify_row_files(ordinary)
            if 'native_execution' in ordinary:
                _verify_process(state, ordinary['native_execution']['process'], ordinary_case)
        _replay_lifecycle(row, ordinary, got, cfgs['ordinary'], cfgs['lifecycle'], contracts, sources, tools)
    elif section == 'guards':
        _verify_guard_complete(row, got, cfgs['lifecycle'], contracts, sources, tools, cfgs['ordinary'])
        ordinary = _infrastructure('ordinary-evidence', load, got['ordinary_receipt']['path'])
        _verify_process(state, ordinary['process'], ordinary_case)
        _verify_row_files(ordinary)
    else:
        core_compare(section, row, got, cfgs['ordinary'], contracts, sources, tools, state['predecessors'])


def _tree_snapshot(root, invocation):
    """Close generated evidence including exactly registered guard links."""
    rows = []
    for path in sorted(Path(root).rglob('*')):
        _tick(invocation)
        relative, mode = path.relative_to(root).as_posix(), path.lstat().st_mode
        if stat.S_ISDIR(mode):
            rows.append({'path': relative, 'kind': 'directory'})
        elif stat.S_ISREG(mode):
            rows.append({**_identity(path, invocation), 'path': relative, 'kind': 'file'})
        elif stat.S_ISLNK(mode):
            parts = Path(relative).parts
            need(len(parts) == 3 and parts[0] == 'guards' and parts[1].isdigit() and
                 parts[2] == 'occupied-output' and os.readlink(path) == 'preserved-target', 'unlisted evidence symlink')
            rows.append({'path': relative, 'kind': 'symlink', 'target': 'preserved-target',
                         'target_identity': _identity(path.parent / 'preserved-target', invocation)})
        else:
            raise Rejected('special runtime evidence file')
    return rows


def collect(admission):
    """Collect the complete frozen roster, with explicit unavailable host rows."""
    state = _state(admission)
    need(not state.get('collection_started'), 'runtime admission is single-use')
    _checkpoint(state, full=True)
    state['collection_started'] = True
    out = state['root'] / 'collection'
    out.mkdir()
    state['collection_root'] = out
    cfgs, contracts = state['cfgs'], state['contracts']
    env, tools = state['runtime_tools'].environment(), state['runtime_tools'].tools()
    receipts_by_section, reports = {}, {}
    try:
        _write(out / 'input-integrity.json', {'execution_context': state['execution_context'],
               'verified': contracts.verified, 'effective_authority': contracts.effective_identity}, state['invocation'])
        _write(out / 'tools.json', state['runtime_tools'].document(), state['invocation'])
        _write(out / 'qualified-paths-amendment.json', state['predecessors'].qualified_paths_amendment, state['invocation'])
        generator = _generator_check(state)
        _preparation().public_output_start(state['invocation'], state['runtime_permit'])
        for section in SECTIONS:
            _checkpoint(state, full=True)
            rows = state['rosters'][section]
            section_out = out / section
            with _preparation().public_output_boundary(state['invocation'], state['runtime_permit'], 'section-open', section=section):
                section_out.mkdir()
                _write(section_out / 'roster.json', [receipt_identity(row) for row in rows], state['invocation'])
            if section == 'guards':
                receipts = _run_guards(rows, section_out, cfgs['ordinary'], cfgs['lifecycle'], contracts, env, tools, state)
            else:
                receipts = []
                for index, row in enumerate(rows):
                    _tick(state['invocation'])
                    if row['scope'] != 'execute':
                        receipts.append(_skipped(row, cfgs['ordinary']))
                        continue
                    with _preparation().public_case_scope(state['invocation'], state['runtime_permit'], section, index):
                        row = copy.deepcopy(row)
                        sources = _row_sources(state, section, row)
                        if section == 'lifecycle' and row['group'] == 'public':
                            row['_filesystem_setup'] = contracts.cases[row['case_id']].get('filesystem_setup')
                        evidence = section_out / ('%05d' % index)
                        state['active_case'] = {'section': section, 'index': index, 'variant': 'ordinary'}
                        got = _collect_one(row, cfgs['ordinary'], evidence / 'ordinary', sources, env, state)
                        if section == 'lifecycle':
                            ordinary = got
                            state['active_case'] = {'section': section, 'index': index, 'variant': 'observer'}
                            got = _collect_one(row, cfgs['lifecycle'], evidence / 'observer', sources, env, state)
                            lifecycle_compare(row, ordinary, got, cfgs['ordinary'], cfgs['lifecycle'], contracts, sources, tools)
                            if ordinary['executed']:
                                ordinary['status'] = got['status'] = 'pass'
                            _write(evidence / 'ordinary' / 'receipt.json', ordinary, state['invocation'])
                            got['ordinary_receipt'] = binding(evidence / 'ordinary' / 'receipt.json')
                            _write(evidence / 'observer' / 'receipt.json', got, state['invocation'])
                        else:
                            if section == 'predecessors' and got['executed']:
                                amendment = state['predecessors'].qualified_paths_comparison(row, got['process'])
                                if amendment is not None:
                                    got['qualified_paths_comparison'] = amendment
                            core_compare(section, row, got, cfgs['ordinary'], contracts, sources, tools, state['predecessors'])
                            if got['executed']:
                                got['status'] = 'pass'
                            _write(evidence / 'ordinary' / 'receipt.json', got, state['invocation'])
                        _write(evidence / 'receipt.json', got, state['invocation'])
                        receipts.append(got)
                        with _read_boundary(state):
                            _validate_case_evidence(state, section, index, got)
            _checkpoint(state, full=True)
            with _read_boundary(state), _preparation().public_replay(state['invocation'], state['runtime_permit']):
                checkpoint = lambda: _tick(state['invocation'])
                checker = lambda row, got: _check_receipt(state, section, row, got)
                _replay_receipts(rows, receipts, checker, checkpoint)
                report = aggregate(rows, receipts, checkpoint=checkpoint)
                report['negative_controls'] = negative_controls(rows, receipts, checker, checkpoint=checkpoint)
            with _preparation().public_output_boundary(state['invocation'], state['runtime_permit'], 'section-close', section=section):
                observation_path = section_out / 'observations.jsonl.gz'
                with gzip.open(observation_path, 'wb') as sink:
                    for got in receipts:
                        _tick(state['invocation'])
                        sink.write(canonical(got) + b'\n')
                if section == 'predecessors':
                    history = predecessor_history(rows, receipts, state['predecessors'],
                        contracts.tables['public']['new_policy_diagnostic_bounds'], binding(observation_path),
                        checkpoint=lambda: _tick(state['invocation']))
                    _write(section_out / 'historical-comparison.json', history, state['invocation'])
                    report.update(predecessor_report_fields(state['predecessors'], history,
                        binding(out / 'qualified-paths-amendment.json'), binding(section_out / 'historical-comparison.json')))
                    report['executed_projection_kind_counts'] = {kind: sum(got.get('projection_kind') == kind and got['executed'] for got in receipts) for kind in sorted({got['projection_kind'] for got in receipts if got['executed'] and 'projection_kind' in got})}
                    report['projection_boundary'] = 'Complete public check/run projections only where declared. First-error, frontend-prefix and schema/source-only rows remain partial. No private legacy-scalar/raw/event/fuel claim.'
                    report['frozen_case_kind_counts'] = contracts.tables[section]['kind_counts']
                if contracts.effective_identity:
                    report.update(contracts.effective_identity)
                report.update(contract_identity=contracts.tables[section]['identity'], historical_qualification_claim=False,
                              private_raw_event_fuel_claim=False, execution_context=state['execution_context'])
                _write(section_out / 'comparison.json', report, state['invocation'])
                receipts_by_section[section], reports[section] = receipts, report
            _checkpoint(state, full=True)
        _checkpoint(state, full=True)
        incomplete = any(r['status'] == 'INCOMPLETE_LOCAL_CAPABILITY' for r in reports.values())
        document = {'schema': 'package-unit4-public-collection-v1', 'execution_context': state['execution_context'],
                    'status': 'INCOMPLETE_LOCAL_CAPABILITY' if incomplete else 'COMPLETE_LOCAL_OBSERVATIONS',
                    'execution_qualified': False, 'global_host_qualification': False,
                    'hosted_staging_admitted': False, 'reports': reports, 'generator': generator,
                    'mandatory_hosted_join_required': incomplete}
        with _preparation().public_output_boundary(state['invocation'], state['runtime_permit'], 'collection-close'):
            bound = _write(state['root'] / 'collection.json', document, state['invocation'])
        _validate_evidence_roster(state, receipts_by_section)
        result = object.__new__(CollectionResult)
        _COLLECTIONS[result] = {'admission': admission, 'raw': canonical(document), 'binding': bound,
                                'receipts': receipts_by_section, 'snapshot': _tree_snapshot(out, state['invocation'])}
        verify_collection(result)
        return result
    except BaseException as error:
        failure = {'status': 'FAILED', 'error_type': type(error).__name__, 'error': _safe_error_text(error),
                   'owned_operations': [r.document() for r in state['probes'].values()],
                   'preservation': 'not-attempted', 'execution_qualified': False,
                   'original_failure': _failure_details(error)}
        try:
            failure['public_output_preservation'] = state['_read_owner_seed'].public_output_failure(
                state['invocation'], state['runtime_permit'])
        except BaseException as output_preservation:
            failure['public_output_preservation'] = {'status': 'unavailable'}
            failure['public_output_preservation_error'] = _safe_error_text(output_preservation)
        try:
            _checkpoint(state, full=True)
            failure['preservation'] = 'verified'
        except BaseException as preservation:
            failure['preservation'] = 'failed'
            failure['preservation_error'] = _safe_error_text(preservation)
        # One bounded emergency attempt; never erase the original process failure.
        try:
            raw = canonical(failure)
            if len(raw) > 8 * 1024 ** 2:
                failure['owned_operation_count'] = len(failure.pop('owned_operations'))
                failure['owned_operations_omitted'] = 'full envelopes remain in original operation evidence'
                failure['owned_operation_evidence_root'] = str(state['root'].parent / 'operations')
                raw = canonical(failure)
            need(len(raw) <= 8 * 1024 ** 2, 'failure evidence bound')
            with (state['root'] / 'collection-failed.json').open('xb') as stream:
                stream.write(raw)
        except BaseException:
            pass
        raise


def verify_collection(result):
    need(type(result) is CollectionResult and result in _COLLECTIONS, 'live collection result required')
    owned = _COLLECTIONS[result]
    state = _state(owned['admission'])
    _checkpoint(state, full=True)
    with _read_boundary(state):
        need(_identity(owned['binding']['path'], state['invocation']) == owned['binding'] and
             canonical(_binding().decode(_read(owned['binding']['path'], state['invocation']))) == owned['raw'],
             'saved collection carrier changed')
        _validate_evidence_roster(state, owned['receipts'])
        _verify_generator(state, json.loads(owned['raw'])['generator'])
        need(_tree_snapshot(state['collection_root'], state['invocation']) == owned['snapshot'], 'runtime evidence tree changed')
        need(set(owned['receipts']) == set(SECTIONS), 'complete five-section collection required')
    processes = []
    for section in SECTIONS:
        _checkpoint(state, full=True)
        with _read_boundary(state), _preparation().public_replay(state['invocation'], state['runtime_permit']):
            rows, receipts = state['rosters'][section], owned['receipts'][section]
            aggregate(rows, receipts, checkpoint=lambda: _tick(state['invocation']))
            for row, got in zip(rows, receipts):
                _tick(state['invocation'])
                _check_receipt(state, section, row, got)
                observations = [got]
                if got.get('ordinary_receipt'):
                    observations.append(load(got['ordinary_receipt']['path']))
                for observation in observations:
                    if observation['executed']:
                        processes.append((observation['process']['pid'], observation['process']['started_ns']))
                        if observation.get('native_execution'):
                            p = observation['native_execution']['process']
                            processes.append((p['pid'], p['started_ns']))
        _checkpoint(state, full=True)
    need(len(processes) == len(set(processes)) and set(processes) == set(state['probes']),
         'exact unique compiler/native process coverage')
    _checkpoint(state, full=True)
    return True


def _replay_lifecycle(row, ordinary, observed, cfg, observer_cfg, contracts, sources, tools):
    replay = copy.deepcopy(observed)
    lifecycle_compare(row, ordinary, replay, cfg, observer_cfg, contracts, sources, tools)
    need(replay == observed, 'saved derived lifecycle value changed')


def _case_evidence_roster(state, section, index, row, got):
    root = state['collection_root']
    required, optional, links = set(), set(), set()
    def role_files(prefix, got, sources, *, fixture_prefix=None):
        required.add(prefix + '/receipt.json')
        fixture_prefix = fixture_prefix or prefix + '/fixture'
        required.update(fixture_prefix + '/' + name for name in sources)
        if not got['executed']:
            return
        required.update(prefix + '/' + name for name in ('process.json', 'stdout', 'stderr'))
        for name, field in (('tools.jsonl', 'tool_invocations'), ('lifecycle.jsonl', 'trace')):
            (required if got[field] else optional).add(prefix + '/' + name)
        if got.get('emitted_llvm'):
            required.add(prefix + '/program.ll')
        if got.get('native_execution'):
            required.update((fixture_prefix + '/out.bin', prefix + '/native-source-free/program'))
    prefix = section + '/' + (('%03d' if section == 'guards' else '%05d') % index)
    required.add(prefix + '/receipt.json')
    sources = _row_sources(state, section, row)
    if section in ('guards', 'lifecycle'):
        ordinary_path = root / prefix / 'ordinary/receipt.json'
        need(got['ordinary_receipt']['path'] == str(ordinary_path), 'ordinary evidence exact indexed path')
        ordinary = load(ordinary_path)
        fixture_prefix = prefix + '/fixtures/' + row['case']['source_case_id'] if section == 'guards' else None
        role_files(prefix + '/ordinary', ordinary, sources, fixture_prefix=fixture_prefix)
        role_files(prefix + '/observer', got, sources, fixture_prefix=fixture_prefix)
        if section == 'guards':
            if row['group'] == 'race':
                required.update((fixture_prefix + '/out.bin', prefix + '/ordinary/competitor.bytes',
                                 prefix + '/ordinary/linked-before-publication.elf',
                                 prefix + '/observer/linked-before-publication.elf'))
            else:
                required.add(prefix + '/preserved-target')
                if row['case']['output_kind'] == 'symlink':
                    links.add(prefix + '/occupied-output')
                else:
                    required.add(prefix + '/occupied-output')
    else:
        role_files(prefix + '/ordinary', got, sources)
    return required, optional, links


def _validate_evidence_members(root, required, optional, links, invocation):
    observed_files, observed_dirs, observed_links = set(), set(), set()
    for path in root.rglob('*'):
        _tick(invocation)
        relative, mode = path.relative_to(root).as_posix(), path.lstat().st_mode
        if stat.S_ISREG(mode):
            observed_files.add(relative)
        elif stat.S_ISDIR(mode):
            observed_dirs.add(relative)
        elif stat.S_ISLNK(mode):
            need(relative in links and os.readlink(path) == 'preserved-target', 'unlisted runtime evidence link')
            observed_links.add(relative)
        else:
            raise Rejected('special runtime evidence entry')
    need(required <= observed_files <= required | optional and observed_links == links,
         'missing or unlisted runtime evidence file')
    wanted_dirs = {str(parent) for name in observed_files | links for parent in Path(name).parents if str(parent) != '.'}
    need(observed_dirs == wanted_dirs, 'unlisted or missing runtime evidence directory')
    return True



def _validate_case_evidence(state, section, index, got):
    row = state['rosters'][section][index]
    need(row['scope'] == 'execute', 'only local cases create evidence')
    prefix = section + '/' + (('%03d' if section == 'guards' else '%05d') % index)
    root = state['collection_root'] / prefix
    required, optional, links = _case_evidence_roster(state, section, index, row, got)
    projected = [{name[len(prefix) + 1:] for name in names} for names in (required, optional, links)]
    _validate_evidence_members(root, *projected, state['invocation'])
    sources = _row_sources(state, section, row)
    fixtures = ([root / 'fixtures' / row['case']['source_case_id']] if section == 'guards' else
                [root / role / 'fixture' for role in (('ordinary', 'observer') if section == 'lifecycle' else ('ordinary',))])
    for fixture in fixtures:
        for name, raw in sources.items():
            need(_read(fixture / name, state['invocation']) == raw, 'case source changed before monitor close')
    _verify_row_files(got)
    if section in ('guards', 'lifecycle'):
        _verify_row_files(load(root / 'ordinary/receipt.json'))
    return True


def _validate_evidence_roster(state, receipts_by_section):
    required = {'input-integrity.json', 'tools.json', 'qualified-paths-amendment.json'}
    optional, links = set(), set()
    for section in SECTIONS:
        required.update(section + '/' + name for name in ('roster.json', 'observations.jsonl.gz', 'comparison.json'))
        if section == 'predecessors':
            required.add(section + '/historical-comparison.json')
        rows, receipts = state['rosters'][section], receipts_by_section[section]
        need(len(rows) == len(receipts), 'evidence complete row count')
        for index, (row, got) in enumerate(zip(rows, receipts)):
            _tick(state['invocation'])
            if row['scope'] != 'execute':
                continue
            current = _case_evidence_roster(state, section, index, row, got)
            for target, members in zip((required, optional, links), current):
                target.update(members)
    return _validate_evidence_members(state['collection_root'], required, optional, links, state['invocation'])


def _verify_generator(state, receipt):
    record = state['generator_probe']
    output = state['root'] / 'original-generator'
    old, contracts = state['helpers']['predecessors'], state['contracts']
    generator = old.authority_path(contracts, '/typed_project_unit1_compatibility/generate_fixtures.py')
    intent_path = old.authority_path(contracts, '/typed_project_unit1_compatibility/suite-intent.json')
    expected = _preparation().verify_probe(record, state['invocation'],
        argv=[sys.executable, str(output / generator.name)], cwd=output,
        environment=state['runtime_tools'].environment(), timeout_seconds=60, stream_limit_bytes=8 * 1024 ** 2,
        operation_kind='generator', runtime_permit=state['runtime_permit'])
    need(expected == receipt['owned_process'] and expected['status'] == 0 and record.stderr() == b'', 'generator owned process changed')
    need(load(output / 'verification.json') == receipt and receipt['generator'] == binding(generator), 'generator receipt/source changed')
    need(_read(output / generator.name) == _read(generator), 'copied generator changed')
    intent = load(intent_path)
    need(len(intent['cases']) == 35 and receipt['intent'] == binding(intent_path), 'original generator frozen intent')
    for case in intent['cases']:
        _tick(state['invocation'])
        verify(output / case['path'], case)
    raw, projected = output / 'suite-intent.json', output / 'suite-intent.frozen-newline.json'
    need(_read(projected) == old.original_manifest_transport(_read(raw), _read(intent_path), os.name), 'generated manifest transport differs')
    transport = receipt['manifest_newline_transport']
    need(transport['raw_generated'] == binding(raw) and transport['frozen_newline_projection'] == binding(projected), 'generated intent evidence drift')
    wanted = {c['path'] for c in intent['cases']} | {generator.name, raw.name, projected.name, 'verification.json'}
    actual, dirs = set(), set()
    for path in output.rglob('*'):
        _tick(state['invocation'])
        mode, name = path.lstat().st_mode, path.relative_to(output).as_posix()
        if stat.S_ISDIR(mode):
            dirs.add(name)
        else:
            need(stat.S_ISREG(mode), 'generator special file')
            actual.add(name)
    need(actual == wanted and dirs == {str(p) for name in wanted for p in Path(name).parents if str(p) != '.'},
         'generator exact closed output membership')
    return True


_JOINS = weakref.WeakKeyDictionary()


class JoinedResult:
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('JoinedResult requires live complete primary and hosted results')

    def document(self):
        need(type(self) is JoinedResult and self in _JOINS, 'live joined result required')
        return json.loads(_JOINS[self]['raw'])


def join_hosted(collection, hosted):
    """Complete mandatory local capability coverage without rewriting primary skips."""
    verify_collection(collection)
    primary = _COLLECTIONS[collection]
    inputs = _binding().hosted_api().result_inputs(hosted)
    need(inputs['admission'] is primary['admission'], 'hosted/primary live runtime owner mismatch')
    state = _state(primary['admission'])
    need(state['execution_context']['host'] == 'Linux x86_64', 'hosted join is Linux only')
    rosters = capability_rows(state['contracts'])
    replacements, reports = {}, {}
    for section in SECTIONS:
        receipts = _copy_receipts(primary['receipts'][section], lambda: _tick(state['invocation']))
        missing = {got['key'] for got in receipts if got['status'] == 'unsupported-capability'}
        wanted = {row['key'] for row in rosters.get(section, [])}
        need(missing == wanted, 'hosted replacement must cover exactly the twelve primary capability skips')
        shard = {got['key']: got for got in inputs['raw_result']['sections'].get(section, [])}
        need(set(shard) == wanted, 'hosted shard replacement membership')
        replacements[section] = sorted(wanted)
        for index, got in enumerate(receipts):
            if got['key'] in shard:
                receipts[index] = copy.deepcopy(shard[got['key']])
        report = aggregate(state['rosters'][section], receipts, checkpoint=lambda: _tick(state['invocation']))
        need(report['status'] != 'INCOMPLETE_LOCAL_CAPABILITY', 'mandatory hosted coverage remains incomplete')
        reports[section] = report
    document = {'schema': 'package-unit4-public-hosted-join-v1', 'status': 'COMPLETE_LOCAL_OBSERVATIONS',
                'execution_context': state['execution_context'], 'primary': primary['binding'],
                'hosted': inputs['report_binding'], 'readonly_transport': inputs['transport_binding'],
                'replaced_capability_keys': replacements, 'reports': reports,
                'primary_receipts_unchanged': True, 'execution_qualified': False,
                'global_host_qualification': False, 'hosted_staging_admitted': False}
    bound = _write(state['root'] / 'joined-collection.json', document, state['invocation'])
    result = object.__new__(JoinedResult)
    _JOINS[result] = {'collection': collection, 'hosted': hosted, 'raw': canonical(document), 'binding': bound}
    verify_join(result)
    return result


def verify_join(result):
    need(type(result) is JoinedResult and result in _JOINS, 'live hosted join required')
    owned = _JOINS[result]
    verify_collection(owned['collection'])
    primary = _COLLECTIONS[owned['collection']]
    hosted = _binding().hosted_api().result_inputs(owned['hosted'])
    need(hosted['admission'] is primary['admission'], 'joined live parent changed')
    state = _state(primary['admission'])
    need(_identity(owned['binding']['path'], state['invocation']) == owned['binding'] and
         canonical(load(owned['binding']['path'])) == owned['raw'], 'joined carrier changed')
    document = json.loads(owned['raw'])
    need(document['primary'] == primary['binding'] and document['hosted'] == hosted['report_binding'] and
         document['readonly_transport'] == hosted['transport_binding'], 'joined evidence changed')
    for section in SECTIONS:
        rows = state['rosters'][section]
        receipts = primary['receipts'][section]
        shard = {got['key']: got for got in hosted['raw_result']['sections'].get(section, [])}
        missing = {got['key'] for got in receipts if got['status'] == 'unsupported-capability'}
        need(missing == set(shard) == set(document['replaced_capability_keys'][section]), 'joined missing/extra coverage')
        merged = [shard.get(got['key'], got) for got in receipts]
        need(aggregate(rows, merged, checkpoint=lambda: _tick(state['invocation'])) == document['reports'][section], 'joined coverage report changed')
    _checkpoint(state, full=True)
    return True
