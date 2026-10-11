"""Registry-issued package-source contexts. Never build or runtime authority.

A canonical producer captures source and invocation identity before preparation.
Only its authenticated, immutable PreparationRecord can associate a completed
operation with that invocation. Serialized receipts cannot create provenance.
Every admission independently rehashes source, helpers and prepared output.
"""
import hashlib
import json
import math
import os
import re
import shutil
import stat
import subprocess
import sys
import time
import types
import uuid
import weakref
from pathlib import Path, PurePosixPath

STAGE = 'tests/qualification/package_dependency_unit4_source_v1'
SELF = 'tests/qualification/package_dependency_unit4_current'
STAGE_MANIFEST = '83460b8bddaeb59d2af8416ffe0d5548f51d3f36a7b3700611e3871c0ae47118'
OLD = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
NEW = 'f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
PARSER = 'tests/qualification/unit4_parser_current/'
FROZEN = 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/'
OLD_DRIVER = '3366c49f267ab98bd792a1cd1b01ebbaaca548bc19685d841eb688cf39f99162'
OLD_HEAD = '0fef9bf4c664c52f441e1c8b842edfcb57fa15d0'
OLD_TREE = 'fc5515d09dce7e8cdb67a43c890272683f7b4755'
HISTORY = 'd9e6b9bf172abd5e15da7212c9e6224e29ccc768'
ROLES = {'ordinary': ('public/ordinary', 'public', 'new_ordinary376', 376),
         'lifecycle': ('public/lifecycle', 'public', 'new_lifecycle377', 377),
         'parser_observed': ('parser/observed', 'parser', 'current_derived_files', 542),
         'parser_control': ('parser/control', 'parser', 'current_control_derived_files', 542)}
SCOPES = {'public': ('ordinary', 'lifecycle'),
          'parser': ('parser_observed', 'parser_control'),
          'combined': tuple(ROLES)}
SHARED_FILES = ('context.py', 'preparation.py', 'prepare_worker.py', 'test_context.py', 'test_preparation.py')
CONTEXT_NAME = '_oxid_package_dependency_unit4_context_v1'
PRODUCER_NAME = '_oxid_package_dependency_unit4_preparation_v1'
WORKER_NAME = '_oxid_package_dependency_unit4_prepare_worker_v1'
MAX = 128 * 1024 ** 2
_LOCAL_DEADLINE = None
_CLOCK_BINDING = None


class Rejected(ValueError):
    pass


class NotReady(RuntimeError):
    pass


def need(ok, message):
    if not ok:
        raise Rejected(message)


def sha(raw):
    digest = hashlib.sha256()
    for offset in range(0, len(raw), 1024 ** 2):
        _tick()
        digest.update(raw[offset:offset + 1024 ** 2])
    _tick()
    return digest.hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def serial(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    def reject(_):
        raise Rejected('nonfinite JSON')
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=reject)


def safe(name):
    need(type(name) is str and name and ':' not in name and '\\' not in name and
         str(PurePosixPath(name)) == name and not name.startswith('/') and
         all(part not in ('', '.', '..') for part in PurePosixPath(name).parts), 'unsafe member')
    return name


def real(path, exists=True):
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=exists) == path and
         not any(p.is_symlink() for p in (path, *path.parents)), 'canonical path required')
    return path


def _clock_owner():
    """Bind the already authenticated canonical producer clock once per owner.

    Bootstrap reads are deliberately primitive and bounded so they cannot
    recurse through body/_tick. The full shared closure is independently
    authenticated by the canonical loader and on every context admission.
    """
    global _CLOCK_BINDING
    producer = sys.modules.get(PRODUCER_NAME)
    if producer is None:
        return None
    path = Path(__file__).with_name('preparation.py')
    marker = getattr(producer, '__source_sha256__', None)
    need(type(producer) is types.ModuleType and producer.__name__ == PRODUCER_NAME and
         getattr(producer, '__file__', None) == str(path) and type(marker) is str and
         re.fullmatch('[0-9a-f]{64}', marker), 'canonical producer deadline owner required')
    binding = (producer, str(path), marker)
    if _CLOCK_BINDING != binding:
        manifest_path = real(path.with_name('package-manifest.json'))
        info = manifest_path.lstat()
        need(stat.S_ISREG(info.st_mode) and info.st_size <= 2 * 1024 ** 2, 'bounded shared clock manifest required')
        with manifest_path.open('rb') as stream:
            raw = stream.read(2 * 1024 ** 2 + 1)
        need(len(raw) == info.st_size, 'shared clock manifest changed while reading')
        manifest = decode(raw)
        need(type(manifest) is dict and manifest.get('schema') == 'oxid-package-dependency-unit4-current-package-v1',
             'shared clock manifest schema')
        members = rows(manifest.get('files'))
        need(tuple(members) == SHARED_FILES and marker == members['preparation.py']['sha256'],
             'producer clock differs from installed shared row')
        need(callable(getattr(producer, 'tick', None)) and callable(getattr(producer, 'remaining_seconds', None)),
             'producer deadline API required')
        _CLOCK_BINDING = binding
    return producer


def _tick():
    if _LOCAL_DEADLINE is not None:
        need(time.monotonic() < _LOCAL_DEADLINE, 'source capture deadline')
    producer = _clock_owner()
    if producer is not None:
        producer.tick()


def _remaining_seconds(maximum):
    _tick()
    remaining = maximum
    if _LOCAL_DEADLINE is not None:
        remaining = min(remaining, _LOCAL_DEADLINE - time.monotonic())
    producer = _clock_owner()
    if producer is not None:
        value = producer.remaining_seconds()
        need(type(value) in (int, float) and math.isfinite(value) and value > 0, 'producer deadline exhausted')
        remaining = min(remaining, value)
    need(remaining > 0, 'source capture deadline exhausted')
    return remaining


def body(path, limit=64 * 1024 ** 2):
    _tick()
    deadline = time.monotonic() + 30
    path = real(path)
    info = path.lstat()
    need(stat.S_ISREG(info.st_mode) and info.st_size <= limit, 'regular bounded input')
    chunks, total = [], 0
    with path.open('rb') as stream:
        while True:
            _tick()
            need(time.monotonic() < deadline, 'bounded source read deadline')
            chunk = stream.read(1024 ** 2)
            if not chunk:
                break
            total += len(chunk)
            need(total <= info.st_size and total <= limit, 'input grew beyond bounded size while reading')
            chunks.append(chunk)
    raw = b''.join(chunks)
    need(len(raw) == info.st_size, 'input size drift')
    _tick()
    return raw


def row(name, raw):
    return {'path': str(name), 'bytes': len(raw), 'sha256': sha(raw)}


def identity(path):
    return row(real(path), body(path))


def rows(records):
    need(type(records) is list, 'file rows required')
    result = {}
    for item in records:
        need(type(item) is dict and set(item) == {'path', 'bytes', 'sha256'}, 'closed member row')
        name = safe(item['path'])
        need(name not in result and type(item['bytes']) is int and 0 <= item['bytes'] <= 64 * 1024 ** 2 and
             type(item['sha256']) is str and re.fullmatch('[0-9a-f]{64}', item['sha256']), 'invalid/duplicate row')
        result[name] = item
    need(not any(str(p) in result for name in result for p in PurePosixPath(name).parents), 'file/directory collision')
    return result


def scan(root):
    _tick()
    deadline = time.monotonic() + 120
    root = real(root)
    need(root.is_dir(), 'input root directory')
    files, dirs, total = {}, set(), 0
    for path in root.rglob('*'):
        _tick()
        need(time.monotonic() < deadline, 'bounded source scan deadline')
        mode = path.lstat().st_mode
        need(stat.S_ISREG(mode) or stat.S_ISDIR(mode), 'symlink/special member')
        name = path.relative_to(root).as_posix()
        safe(name)
        if stat.S_ISDIR(mode):
            dirs.add(name)
        else:
            raw = body(path)
            total += len(raw)
            need(total <= MAX, 'closure byte bound')
            files[name] = row(name, raw)
    _tick()
    return files, dirs


def directories(names):
    return {str(p) for name in names for p in PurePosixPath(name).parents if str(p) != '.'}


def module(path, raw, name):
    result = types.ModuleType(name)
    result.__file__ = str(path)
    exec(compile(raw, str(path), 'exec'), result.__dict__)
    return result


def _git(repo, *args):
    """Bounded read-only Git identity using one measured canonical executable."""
    _tick()
    found = shutil.which('git')
    need(found is not None, 'Git unavailable for source identity')
    route = Path(found)
    need(route.is_absolute(), 'absolute Git executable route required')
    git = real(route.resolve(strict=True))
    before = identity(git)
    env = {'PATH': str(git.parent) + os.pathsep + os.defpath,
           'HOME': '/nonexistent/package-source-git', 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8',
           'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull,
           'GIT_NO_REPLACE_OBJECTS': '1', 'GIT_TERMINAL_PROMPT': '0'}
    if os.name == 'nt':
        need(bool(os.environ.get('SYSTEMROOT')), 'Windows system context required')
        for name in ('SYSTEMROOT', 'WINDIR'):
            if name in os.environ:
                env[name] = os.environ[name]
    command = [str(git), '--no-optional-locks', '-c', 'core.hooksPath=' + os.devnull,
               '-c', 'core.fsmonitor=false', '-C', str(repo), *args]
    try:
        result = subprocess.run(command, env=env, stdin=subprocess.DEVNULL,
                                capture_output=True, timeout=_remaining_seconds(10))
    finally:
        _tick()
        need(route.resolve(strict=True) == git and identity(git) == before, 'Git executable identity drift')
    need(result.returncode == 0 and not result.stderr and len(result.stdout) <= 2 * 1024 ** 2,
         'read-only Git identity failure')
    _tick()
    return result.stdout.decode().strip()


def _shared(repo):
    root = repo / SELF
    manifest_raw = body(root / 'package-manifest.json')
    manifest = decode(manifest_raw)
    need(type(manifest) is dict and manifest.get('schema') == 'oxid-package-dependency-unit4-current-package-v1',
         'shared package schema')
    wanted = rows(manifest.get('files'))
    need(tuple(wanted) == SHARED_FILES, 'all five ordered shared helper rows required')
    actual, dirs = scan(root)
    need(set(actual) == set(wanted) | {'package-manifest.json'} and not dirs, 'closed shared package')
    for name, expected in wanted.items():
        need(actual[name] == expected and not (root / name).stat().st_mode & 0o111, 'shared helper body/mode drift')
    return root, wanted, manifest_raw


def _stage(repo, scope='combined'):
    need(type(scope) is str and scope in SCOPES, 'exact source scope required')
    root = repo / STAGE
    raw = body(root / 'package-manifest.json')
    need(sha(raw) == STAGE_MANIFEST, 'installed Stage1 package pin')
    manifest = decode(raw)
    wanted = rows(manifest['files'])
    actual, dirs = scan(root)
    need(len(wanted) == 9 and len(actual) == 10 and
         set(actual) == set(wanted) | {'package-manifest.json'} and dirs == directories(actual), 'closed installed Stage1 package')
    for name, expected in wanted.items():
        need(actual[name] == expected and not (root / name).stat().st_mode & 0o111, 'Stage1 helper drift')
    names = ['source']
    if scope != 'parser':
        names.append('public_maps')
    if scope != 'public':
        names.append('parser_maps')
    mods = {name: module(root / (name + '.py'), body(root / (name + '.py')), 'package_source_' + name)
            for name in names}
    return root, mods, actual


def _snapshot(repo, expected_head, event_sha, scope='combined'):
    global _LOCAL_DEADLINE
    previous = _LOCAL_DEADLINE
    if previous is None:
        _LOCAL_DEADLINE = time.monotonic() + 120
    try:
        _tick()
        result = _snapshot_body(repo, expected_head, event_sha, scope)
        _tick()
        return result
    finally:
        _LOCAL_DEADLINE = previous


def _snapshot_body(repo, expected_head, event_sha, scope='combined'):
    _tick()
    need(not sys.flags.optimize and sys.dont_write_bytecode, 'normal bytecode-disabled Python required')
    need(type(scope) is str and scope in SCOPES, 'exact source scope required')
    repo = real(repo)
    need(real(__file__) == repo / SELF / 'context.py', 'shared context must be installed at exact repository path')
    need(type(expected_head) is str and re.fullmatch('[0-9a-f]{40}', expected_head) and
         type(event_sha) is str and re.fullmatch('[0-9a-f]{40}', event_sha), 'explicit actual/event Git identities')
    head, tree = _git(repo, 'rev-parse', 'HEAD'), _git(repo, 'rev-parse', 'HEAD^{tree}')
    need(head == expected_head, 'actual checkout head differs')
    shared, shared_rows, manifest_raw = _shared(repo)
    stage, mods, stage_rows = _stage(repo, scope)
    _tick()
    source = mods['source'].capture(repo)
    _tick()
    proofs = {}
    if scope != 'parser':
        public = mods['public_maps']
        proofs['public'] = public.validate_authority(body(stage / 'public-source-successor-authority.json'),
                                                     source, public.read_retained(repo))
        _tick()
    if scope != 'public':
        parser = mods['parser_maps']
        proofs['parser'] = parser.verify({name: body(stage / name) for name in parser.OUTPUTS},
                                        source, parser.retained_inputs(repo))
        _tick()
    maps = {role: proofs[ROLES[role][1]]['maps'][ROLES[role][2]] for role in SCOPES[scope]}
    for role, items in maps.items():
        _tick()
        need(len(rows(items)) == ROLES[role][3], 'complete role body count')
    relevant = set(source['current_inputs']) | {(Path(STAGE) / name).as_posix() for name in stage_rows}
    relevant |= {(Path(SELF) / name).as_posix() for name in (*shared_rows, 'package-manifest.json')}
    relevant |= {source['current_manifest_binding']['path'], source['predecessor_manifest_binding']['path'],
                 source['bridge_helper']['path'], mods['source'].PATCH}
    # capture has already authenticated these complete, closed source packages.
    checkout = repo / mods['source'].CHECKOUT
    for package in (checkout.parent, (repo / mods['source'].CURRENT).parent):
        _tick()
        relevant |= {path.relative_to(repo).as_posix() for path in package.iterdir() if path.is_file()}
    layout = decode(body(checkout.parent / 'predecessor-layout-v1.json'))
    relevant |= {safe(record['path']) for record in layout['files']}
    for name in ('public_maps', 'parser_maps'):
        if name in mods:
            relevant |= set(mods[name].PINS)
    tracked = set(_git(repo, 'ls-tree', '-r', '--name-only', head).splitlines())
    need(relevant <= tracked, 'uncommitted qualification input')
    need(not _git(repo, 'diff', '--name-only', 'HEAD', '--', *sorted(relevant)), 'dirty qualification body')
    need(_git(repo, 'rev-parse', 'HEAD') == head, 'checkout head changed')
    value = {'scope': scope, 'checkout': {'path': str(repo), 'head': head, 'tree': tree, 'event_sha': event_sha},
             'source_checkpoint': source['source_checkpoint'],
             'current_source_manifest': source['current_manifest_binding'],
             'predecessor_source_manifest': source['predecessor_manifest_binding'],
             'authorities': {'public': row(STAGE + '/public-source-successor-authority.json', body(stage / 'public-source-successor-authority.json')),
                             'parser': row(STAGE + '/parser-source-successor-authority.json', body(stage / 'parser-source-successor-authority.json')),
                             'candidate': row(STAGE + '/parser-candidate-source.json', body(stage / 'parser-candidate-source.json'))},
             'maps': maps, 'retained': {'source_root': str(repo / 'tests/fixtures/typed_project_source_binding')},
             'shared_helper': identity(shared / 'context.py'),
             'shared_package': row(shared / 'package-manifest.json', manifest_raw)}
    need(value['current_source_manifest']['sha256'] == NEW and value['predecessor_source_manifest']['sha256'] == OLD,
         'distinct fixed source identities')
    return value


_ISSUE_TOKEN = object()
_CAPTURES = weakref.WeakKeyDictionary()
_INVOCATIONS = weakref.WeakKeyDictionary()
_CONTEXTS = weakref.WeakKeyDictionary()
_PRODUCERS = {}
_PRODUCER_TOKENS = {}


class _Issued:
    __slots__ = ('__weakref__',)
    def __new__(cls, token=None):
        need(token is _ISSUE_TOKEN, 'use the authenticated context factory')
        return super().__new__(cls)
    def __init__(self, token=None):
        pass
    def __setattr__(self, *args):
        raise TypeError('immutable registry-issued context object')


class CapturedSource(_Issued):
    __slots__ = ()
    def document(self):
        return decode(_entry(self, CapturedSource, _CAPTURES))


class Invocation(_Issued):
    __slots__ = ()
    def document(self):
        return decode(_entry(self, Invocation, _INVOCATIONS)['raw'])
    def captured(self):
        return _entry(self, Invocation, _INVOCATIONS)['capture']


class AdmittedContext(_Issued):
    __slots__ = ()
    def document(self):
        return decode(_entry(self, AdmittedContext, _CONTEXTS)['raw'])
    def invocation(self):
        return _entry(self, AdmittedContext, _CONTEXTS)['invocation']
    def view(self, role):
        views = self.document()['views']
        need(role in ROLES and role in views, 'source role is outside admitted scope')
        return views[role]


def _entry(value, cls, registry):
    need(type(value) is cls and value in registry, 'registry-issued ' + cls.__name__ + ' required')
    return registry[value]


def _issue(cls, registry, value):
    result = cls(_ISSUE_TOKEN)
    registry[result] = value
    return result


def capture_source(repo, expected_head, event_sha, scope='combined'):
    return _issue(CapturedSource, _CAPTURES, canonical(_snapshot(repo, expected_head, event_sha, scope)))


def _capture_check(captured):
    value = decode(_entry(captured, CapturedSource, _CAPTURES))
    checkout = value['checkout']
    need(value == _snapshot(Path(checkout['path']), checkout['head'], checkout['event_sha'], value['scope']),
         'captured source/helper drift')
    return value


def begin_invocation(captured, fresh_run_root):
    value = _capture_check(captured)
    root = real(fresh_run_root, False)
    repo = Path(value['checkout']['path'])
    need(root.parent.is_dir() and not root.exists() and repo not in root.parents and root not in repo.parents and root != repo,
         'fresh external invocation root')
    document = {'nonce': uuid.uuid4().hex, 'started_ns': time.time_ns(), 'run_root': str(root)}
    return _issue(Invocation, _INVOCATIONS, {'raw': canonical(document), 'capture': captured,
                                           'record': None, 'issuer': None})


def _invocation_association(invocation):
    """Validate original live registry association, without rereading checkout bodies."""
    entry = _entry(invocation, Invocation, _INVOCATIONS)
    value = decode(entry['raw'])
    need(set(value) == {'nonce', 'started_ns', 'run_root'} and type(value['nonce']) is str and
         re.fullmatch('[0-9a-f]{32}', value['nonce']) and type(value['started_ns']) is int and
         0 < value['started_ns'] <= time.time_ns(), 'invocation shape/window')
    real(value['run_root'], False)
    return value, decode(_entry(entry['capture'], CapturedSource, _CAPTURES))


def _invocation(invocation):
    value, _ = _invocation_association(invocation)
    entry = _entry(invocation, Invocation, _INVOCATIONS)
    return value, _capture_check(entry['capture'])


def _owner(module_value, name, path, expected):
    need(type(module_value) is types.ModuleType and sys.modules.get(name) is module_value and
         module_value.__name__ == name and getattr(module_value, '__file__', None) == str(path) and
         getattr(module_value, '__source_sha256__', None) == expected['sha256'] and
         row(path.name, body(path)) == expected, 'canonical authenticated module owner required')
    return module_value


def _register_producer(producer):
    """Issue only to the installed canonical producer after body authentication."""
    here = real(__file__)
    repo = here.parents[3]
    root, members, _ = _shared(repo)
    _owner(sys.modules.get(CONTEXT_NAME), CONTEXT_NAME, root / 'context.py', members['context.py'])
    need(sys.modules[CONTEXT_NAME].__dict__ is globals(), 'different canonical context registry owner')
    _owner(producer, PRODUCER_NAME, root / 'preparation.py', members['preparation.py'])
    if producer not in _PRODUCER_TOKENS:
        token = object()
        _PRODUCER_TOKENS[producer] = token
        _PRODUCERS[token] = producer
    return _PRODUCER_TOKENS[producer]


def _producer(issuer):
    need(type(issuer) is object and issuer in _PRODUCERS, 'unregistered preparation issuer')
    producer = _PRODUCERS[issuer]
    need(_register_producer(producer) is issuer, 'changed preparation issuer')
    return producer


def _record_preparation(invocation, record, issuer_token):
    """Associate one trusted completed producer operation; never a receipt dict."""
    entry = _entry(invocation, Invocation, _INVOCATIONS)
    need(entry['record'] is None and entry['issuer'] is None, 'preparation already associated with invocation')
    producer = _producer(issuer_token)
    verified = producer.verify_record(record, invocation)
    need(type(verified) is dict and set(verified) == {'config', 'plan', 'provenance'}, 'closed producer verification result')
    entry['record'], entry['issuer'] = record, issuer_token


def _preparation(invocation):
    entry = _entry(invocation, Invocation, _INVOCATIONS)
    need(entry['record'] is not None and entry['issuer'] is not None, 'trusted preparation record missing')
    verified = _producer(entry['issuer']).verify_record(entry['record'], invocation)
    need(type(verified) is dict and set(verified) == {'config', 'plan', 'provenance'} and
         all(type(verified[key]) is dict for key in verified), 'closed producer verification result')
    return verified


def _worker_api(repo):
    root, members, _ = _shared(repo)
    path = root / 'prepare_worker.py'
    expected = members['prepare_worker.py']
    if WORKER_NAME in sys.modules:
        return _owner(sys.modules[WORKER_NAME], WORKER_NAME, path, expected)
    worker = types.ModuleType(WORKER_NAME)
    worker.__file__ = str(path)
    worker.__source_sha256__ = expected['sha256']
    sys.modules[WORKER_NAME] = worker
    try:
        exec(compile(body(path), str(path), 'exec'), worker.__dict__)
    except BaseException:
        if sys.modules.get(WORKER_NAME) is worker:
            del sys.modules[WORKER_NAME]
        raise
    return worker


def _admit(invocation, prepared_root):
    # Complete input snapshots bracket one synchronous admission; nested record
    # ownership checks do not recursively repeat that same checkout traversal.
    inv, captured = _invocation(invocation)
    root = real(prepared_root)
    need(root == Path(inv['run_root']) / 'source-preparation', 'preparation outside fresh invocation')
    entry = _entry(invocation, Invocation, _INVOCATIONS)
    need(entry['record'] is not None and entry['issuer'] is not None, 'trusted preparation record missing')
    producer = _producer(entry['issuer'])
    with producer.checked_scope(invocation):
        value = _admit_readonly(invocation, root, inv, captured)
    need(_invocation(invocation) == (inv, captured), 'full admission boundary source drift')
    return value


def _admit_readonly(invocation, prepared_root, inv, captured):
    repo = Path(captured['checkout']['path'])
    root = real(prepared_root)
    need(root == Path(inv['run_root']) / 'source-preparation', 'preparation outside fresh invocation')
    verified = _preparation(invocation)
    config, proposed, provenance = (verified[name] for name in ('config', 'plan', 'provenance'))
    scope = captured['scope']
    need(scope in SCOPES and config.get('scope') == scope and config.get('current_repo') == str(repo) and
         config.get('stage1_root') == str(repo) and config.get('output') == str(root) and
         config.get('run_root') == inv['run_root'], 'trusted preparation source/scope/invocation association')
    worker = _worker_api(repo)
    need(proposed == worker.plan(config, tick=_tick), 'sealed preparation plan differs from current derivation')
    report = worker.verify_output(config, proposed, tick=_tick)
    need(report['scope'] == scope and report['status'] == 'NotReady' and report['execution_qualified'] is False and
         report['old_source'] == captured['predecessor_source_manifest'] and
         report['new_source'] == captured['current_source_manifest'] and
         report['source_checkpoint'] == captured['source_checkpoint'], 'prepared report source association')
    need(set(proposed['views']) == {ROLES[role][0] for role in SCOPES[scope]} and
         set(captured['maps']) == set(SCOPES[scope]), 'exact preparation role scope')
    exact = rows(proposed['expected_files'])
    actual = rows(report['files'])
    views = {}
    for role in SCOPES[scope]:
        sub = ROLES[role][0]
        records = proposed['views'][sub]
        need(records == captured['maps'][role] and len(rows(records)) == ROLES[role][3], 'prepared complete source role map')
        name = sub + '/package-preparation-overlay.json'
        need(actual.get(name) == exact[name], 'prepared exact overlay identity')
        metadata = dict(exact[name], path='package-preparation-overlay.json')
        views[role] = {'path': str(root / sub), 'files': records,
                       'map_sha256': sha(canonical(records)), 'metadata': [metadata]}
    report_identity = row(root / 'report.json', serial(report))
    need(identity(root / 'report.json') == report_identity, 'verified report changed before context issuance')
    preparation = {'root': str(root), 'scope': scope, 'report': report_identity,
                   'plan_sha256': sha(serial(proposed)), 'provenance_sha256': sha(canonical(provenance))}
    if scope == 'public':
        proof = dict(report['genuine_public_admission'], path=str(root / 'predecessor/public-admission.json'))
        need(identity(proof['path']) == proof, 'verified public admission changed')
        preparation['public_admission'] = proof
    else:
        session = dict(report['genuine_predecessor_session'], path=str(root / 'predecessor/session.json'))
        need(identity(session['path']) == session, 'verified predecessor session changed')
        preparation['predecessor_session'] = session
    result = {key: value for key, value in captured.items() if key not in ('maps', 'shared_helper', 'shared_package')}
    result.update(schema='oxid-package-unit4-admitted-source-context-v1', status='NotReady',
                  execution_qualified=False, invocation=inv, views=views, preparation=preparation)
    return result


def admit_prepared(invocation, prepared_root):
    value = _admit(invocation, prepared_root)
    return _issue(AdmittedContext, _CONTEXTS, {'raw': canonical(value), 'invocation': invocation})


def verify_context(context):
    entry = _entry(context, AdmittedContext, _CONTEXTS)
    value = decode(entry['raw'])
    need(value == _admit(entry['invocation'], value['preparation']['root']), 'context actual-input drift')
    return True


def load_context(path, expected_invocation):
    _entry(expected_invocation, Invocation, _INVOCATIONS)
    value = decode(body(path))
    actual = _admit(expected_invocation, value['preparation']['root'])
    need(value == actual, 'serialized context differs from fresh admission')
    return _issue(AdmittedContext, _CONTEXTS, {'raw': canonical(actual), 'invocation': expected_invocation})


def build(*args, **kwargs):
    raise NotReady('producer machine admission and execution supervisor are not implemented')


def runtime_admission(*args, **kwargs):
    raise NotReady('full public/parser qualification and CI join remain incomplete')
