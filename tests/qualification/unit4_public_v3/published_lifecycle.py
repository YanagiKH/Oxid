"""INACTIVE published-main public observer transport. Source-only, no CLI.

Install only through an independently reviewed exact-head consumer closure.
The unchanged public build calls admit(source, patch) twice, sequentially.
This stable module instance rejects overlapping admissions and remains poisoned
on preservation/cleanup failure. That is NOT cross-module/process exclusion,
resource admission, a host/runtime result, or permission to materialize a view.
The execution caller owns global exclusion and all C/M maps, package buffers,
compiled globals, temporary-prefix, memory, inode and allocation overlap.

The accepted legacy functions are unmodified. Hardened reads bracket their
content-relevant identities and their outputs are compared with those reads.
Legacy Path.read_bytes calls retain their original atime behavior. A source
path already resolved by build.prepare cannot reveal erased symlink spelling.
"""
import ast
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import threading
import types

PUBLIC = 'tests/qualification/unit4_public_v3'
PACKAGE = 'tests/fixtures/typed_project_source_binding'
SAVED = 'tests/fixtures/typed_project_source_binding_byte_storage_v1'
PROOF_NAME = 'published-lifecycle-v1.json'
HELPER_NAME = 'published_lifecycle.py'
MAIN = 'src/main.rs'
DISPATCHER_PIN = {'path': PACKAGE + '/run.py', 'bytes': 5824,
    'sha256': '695ea921fa6c263fd22bb8bd11a8ac01046002ab2feb8943c3831e289ac78b98'}
PACKAGE_PIN = {'path': PACKAGE + '/package-manifest.json', 'bytes': 16025,
    'sha256': '9a36d668a53e763bc06cb8e38b32d9808152537bafe14c2dcd0fd4d9a9b64ec0'}
OLD_HELPER_PIN = {'path': PUBLIC + '/lexer_reservation_lifecycle.py', 'bytes': 9393,
    'sha256': '155471428cc243fbb7845ec2dcd21fb982f51e7e8264ccf804f1df07338c91ec'}
OLD_PROOF_PIN = {'path': PUBLIC + '/lexer-reservation-lifecycle-v2.json', 'bytes': 282549,
    'sha256': 'f8e6e92eda7c0d07dea9a73301be9318f32c4a0eddb7126a581ad17e88d10e86'}
PATCH_PIN = {'path': 'observer-lexer-reservation-v2.patch', 'bytes': 7337,
    'sha256': '826468198bfd2575d0d01713bb803f19f31b732d482dbf336b7e37567f890824'}
PREVIOUS_PATCH_PIN = {'path': 'observer-u8-v1.patch', 'bytes': 7541,
    'sha256': 'c64b43cd0b630fcae46ac4cf73812ad0eb614696474e3af5d75d79b32f5047cf'}
PREVIOUS_AUTHORITY_PIN = {'path': 'byte_storage_source_authority.py', 'bytes': 1404,
    'sha256': '14629fd20362700e69c3aca0ce079748bf829533f1d2e5098f43b26ba3df0ba5'}
OLD_AUTHORITY_PIN = {'path': 'authority.py', 'bytes': 1600,
    'sha256': '346485ecb8afed13bd784190f57f8e86a4d6d065437c2220e99928ade425aac7'}
OLD_BUILD_PIN = {'path': 'build.py', 'bytes': 14651,
    'sha256': 'df3d748ee033761a15b3142a4d77f6a5f34dcd6cf10463a87d9757020e21baa7'}
OLD_TEST_PIN = {'path': 'test_lexer_reservation.py', 'bytes': 4442,
    'sha256': '5afea775ea089e577487c3de71ed5a803707d50d176a3d8e52a99aa4a7e1e132'}
RETAINED_PINS = [
    {'path': SAVED + '/run.py', 'bytes': 216279,
     'sha256': '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76'},
    {'path': SAVED + '/u8_cross_host.py', 'bytes': 7911,
     'sha256': '7664dc9ff6597c3c586154af3b3a38f095147d025b8fd6e544f6c7a93f250ff8'},
    {'path': SAVED + '/byte_storage.py', 'bytes': 9896,
     'sha256': 'e9eef8a6475d6c79c5e93a127af8ed99e46eb029de4da7909371b44dcd6f6084'},
    {'path': SAVED + '/package-manifest.json', 'bytes': 14146,
     'sha256': 'c68a80705e43c2711d492d68fdbf63213e7d7b7f02dd1288c12403382ad3208c'},
]
HISTORICAL_PACKAGE_PIN = {
    'path': 'tests/qualification/published_formatter_current/predecessor-v1/'
            'tests/fixtures/typed_project_source_binding/package-manifest.json',
    'bytes': 15335,
    'sha256': '5fb5ee78a42609dab51f8e8bb23c325305954f49c52f0d18df6bbe47e57548bc'}
PUBLISHED_MAIN = {'head': '15eac633e167c261ba0246ced49cfdc1c6cef434',
                  'tree': 'f66d5ae31272268c29f520ec523ee1fa31c15ee9'}
CHECKPOINT = {'head': 'fbff527c895e6cd4ec9350bd8b5d460bc20bb80d',
              'tree': '89e90c3ec83d0712d0037a0407fc6571883c5e9c',
              'parent': '15eac633e167c261ba0246ced49cfdc1c6cef434'}
HISTORICAL = {'head': 'b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489',
              'tree': '7f5c9aa08c569c4d0b5a27391d8fc68337075d36'}
OLD_CURRENT_VALUES = {
    'CURRENT_SOURCE_SHA': '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261',
    'CURRENT_FILES_SHA': '676b96270257ad5f9f793c5ab8c7872434906fef8353feabb6052e2a2cbd2e87',
    'OBSERVER_FILES_SHA': 'a6bf8f6a6c89cbe765d33c4351438749cc1e5e408a9ab002d8ad026cbc800790'}
HELPER_ORDER = ('adapters', 'source_transition', 'seal', 'current',
                'published_transition', 'published_view', 'published_current')
RETAINED_ALIASES = ('retained_lexer_reservation_run',
                    'retained_lexer_reservation_u8_cross_host',
                    'retained_lexer_reservation_byte_storage',
                    'published_formatter_retained_execution')
SEAL_KEYS = {'schema', 'source_manifest', 'authority', 'patch',
             'source_proof', 'predecessor_view'}
PROOF_KEYS = {
    'schema', 'published_main', 'source_checkpoint', 'retained_historical_source',
    'source_seal', 'source_dispatcher', 'source_package_manifest',
    'source_helpers', 'retained_public_api', 'transport_helper',
    'historical_lifecycle_helper', 'historical_lifecycle_proof',
    'source_manifest', 'transport_paths', 'base_files', 'observer_files',
    'complete_inverse_sha256', 'patch', 'predecessor_patch',
    'predecessor_authority', 'inserted_roster', 'lexer_hook_correspondence',
    'compiler_invocations', 'source_only', 'execution_qualified',
    'admission_route'}
MAX_BODY = 2 * 1024 * 1024


def _need(ok, message):
    if not ok:
        raise Rejected(message)


def _digest(raw):
    return hashlib.sha256(raw).hexdigest()


def _binding(name, raw):
    return {'path': name, 'bytes': len(raw), 'sha256': _digest(raw)}


def _same(left, right):
    if type(left) is not type(right):
        return False
    if type(left) is dict:
        return left.keys() == right.keys() and all(_same(left[k], right[k]) for k in left)
    if type(left) in (list, tuple):
        return len(left) == len(right) and all(_same(a, b) for a, b in zip(left, right))
    return left == right


def _json(raw):
    _need(type(raw) is bytes, 'JSON input must be authenticated bytes')
    def pairs(items):
        value = {}
        for key, item in items:
            _need(key not in value, 'duplicate JSON key: ' + key)
            value[key] = item
        return value
    def reject_number(value):
        raise Rejected('noninteger or nonfinite JSON number: ' + value)
    try:
        return json.loads(raw, object_pairs_hook=pairs,
                          parse_float=reject_number, parse_constant=reject_number)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise Rejected('invalid UTF-8 JSON') from error


def _canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'),
                      ensure_ascii=True, allow_nan=False).encode()


def _relative(name):
    _need(type(name) is str and name and '\\' not in name
          and not PurePosixPath(name).is_absolute()
          and str(PurePosixPath(name)) == name
          and all(p not in ('.', '..') for p in PurePosixPath(name).parts),
          'unsafe relative path')
    return name


def _descriptor(value):
    _need(type(value) is dict and set(value) == {'path', 'bytes', 'sha256'},
          'descriptor fields differ')
    _relative(value['path'])
    _need(type(value['bytes']) is int and 0 <= value['bytes'] <= MAX_BODY
          and type(value['sha256']) is str
          and re.fullmatch('[0-9a-f]{64}', value['sha256']) is not None,
          'descriptor types differ')


def _verify(raw, descriptor):
    _descriptor(descriptor)
    _need(type(raw) is bytes and len(raw) == descriptor['bytes']
          and _digest(raw) == descriptor['sha256'],
          'changed pinned body: ' + descriptor['path'])


def _rows_schema(value, count, components=False):
    _need(type(value) is list and len(value) == count, 'map cardinality differs')
    for row in value:
        _descriptor(row)
    names = [row['path'] for row in value]
    key = (lambda name: PurePosixPath(name).parts) if components else None
    _need(names == sorted(set(names), key=key), 'unordered or duplicate map member')


def _literal(raw, name):
    tree = ast.parse(raw)
    found = [node.value for node in tree.body
             if isinstance(node, ast.Assign) and len(node.targets) == 1
             and isinstance(node.targets[0], ast.Name) and node.targets[0].id == name]
    _need(len(found) == 1, 'missing or duplicate authenticated literal: ' + name)
    return ast.literal_eval(found[0])


def _dispatcher(raw):
    # These pins are independent of the dispatcher and its manifest. Never
    # import/execute the dispatcher: its top level installs a retained runtime.
    _verify(raw, DISPATCHER_PIN)
    specs = _literal(raw, '_HELPER_SPECS')
    seal = _literal(raw, '_SOURCE_SEAL')
    _need(type(specs) is list and len(specs) == 7, 'wrong helper closure')
    for pair in specs:
        _need(type(pair) is list and len(pair) == 2 and type(pair[0]) is str,
              'wrong helper pair')
        _descriptor(pair[1])
    _need(tuple(pair[0] for pair in specs) == HELPER_ORDER
          and len({pair[1]['path'] for pair in specs}) == 7,
          'wrong helper dependency order or duplicate helper')
    _need(type(seal) is dict and set(seal) == SEAL_KEYS
          and type(seal['schema']) is str
          and seal['schema'] == 'oxid-published-main-source-seal-v1',
          'wrong source seal')
    for key in SEAL_KEYS - {'schema'}:
        _descriptor(seal[key])
    return specs, seal


def _proof_schema(proof):
    _need(type(proof) is dict and set(proof) == PROOF_KEYS,
          'published lifecycle proof has unknown or missing fields')
    _need(type(proof['schema']) is str
          and proof['schema'] == 'oxid-published-public-lifecycle-v1'
          and type(proof['admission_route']) is str
          and proof['admission_route'] == 'authenticated-pure-source-then-one-original-M-admission'
          and _same(proof['published_main'], PUBLISHED_MAIN)
          and _same(proof['source_checkpoint'], CHECKPOINT)
          and _same(proof['retained_historical_source'], HISTORICAL)
          and _same(proof['transport_paths'], [MAIN])
          and type(proof['compiler_invocations']) is int and proof['compiler_invocations'] == 0
          and proof['source_only'] is True and proof['execution_qualified'] is False,
          'wrong source-only provenance or recursive types')
    for name in ('source_dispatcher', 'source_package_manifest', 'transport_helper',
                 'historical_lifecycle_helper', 'historical_lifecycle_proof',
                 'source_manifest', 'patch', 'predecessor_patch', 'predecessor_authority'):
        _descriptor(proof[name])
    seal = proof['source_seal']
    _need(type(seal) is dict and set(seal) == SEAL_KEYS
          and type(seal['schema']) is str
          and seal['schema'] == 'oxid-published-main-source-seal-v1', 'wrong proof seal')
    for key in SEAL_KEYS - {'schema'}:
        _descriptor(seal[key])
    _need(type(proof['source_helpers']) is list and len(proof['source_helpers']) == 7,
          'wrong proof helper count')
    for pair in proof['source_helpers']:
        _need(type(pair) is dict and set(pair) == {'module', 'body'}
              and type(pair['module']) is str, 'wrong proof helper fields')
        _descriptor(pair['body'])
    _need(tuple(pair['module'] for pair in proof['source_helpers']) == HELPER_ORDER,
          'wrong proof helper order')
    _need(type(proof['retained_public_api']) is list
          and len(proof['retained_public_api']) == 4, 'wrong retained API closure')
    for row in proof['retained_public_api']:
        _descriptor(row)
    _rows_schema(proof['base_files'], 376)
    _rows_schema(proof['observer_files'], 377, True)
    _need(type(proof['complete_inverse_sha256']) is str
          and re.fullmatch('[0-9a-f]{64}', proof['complete_inverse_sha256']) is not None,
          'wrong complete inverse identity')
    roster = proof['inserted_roster']
    _need(type(roster) is list and len(roster) == 8, 'wrong inserted roster')
    for row in roster:
        _need(type(row) is dict and set(row) == {'path', 'inserted'}
              and type(row['inserted']) is str, 'wrong inserted row')
        _relative(row['path'])
    names = [row['path'] for row in roster]
    _need(names == sorted(set(names)), 'wrong inserted roster ordering')
    hook = proof['lexer_hook_correspondence']
    counts = {'actual_core_attempt_sites': 1, 'eof_append_sites': 1,
              'ordinary_append_sites': 1, 'successful_append_hook_sites': 0,
              'successful_eof_completion_sites': 1, 'wrapper_hook_sites': 0}
    _need(type(hook) is dict
          and set(hook) == set(counts) | {'source', 'derived', 'inverse', 'kind', 'schema'}
          and type(hook['kind']) is str and hook['kind'] == 'lifecycle'
          and type(hook['schema']) is str
          and hook['schema'] == 'oxid-fallible-lexer-hook-correspondence-v1',
          'wrong hook correspondence fields')
    for name, count in counts.items():
        _need(type(hook[name]) is int and hook[name] == count, 'wrong hook count type/value')
    for name in ('source', 'derived', 'inverse'):
        _descriptor(hook[name])


def make_proof(historical_raw, current_raw, dispatcher_raw, package_raw, helper_raw):
    """Pure deterministic recipe, with no filesystem/materialization/exec call.

    The proof deliberately does not pin authority.py, build.py, the transported
    tests, or any other consumer embedding the proof's digest. External CI must
    authenticate those consumers and the generator separately.
    """
    _verify(historical_raw, OLD_PROOF_PIN)
    _verify(package_raw, PACKAGE_PIN)
    specs, seal = _dispatcher(dispatcher_raw)
    _verify(current_raw, seal['source_manifest'])
    historical, current = _json(historical_raw), _json(current_raw)
    _need(type(helper_raw) is bytes and 0 < len(helper_raw) <= 65536,
          'missing finite transport helper body')
    _need(historical['reviewed_source_head'] == HISTORICAL['head']
          and historical['source_only_tree'] == HISTORICAL['tree']
          and current['reviewed_source_head'] == CHECKPOINT['head']
          and current['source_only_tree'] == CHECKPOINT['tree']
          and current['published_main_head'] == PUBLISHED_MAIN['head']
          and current['published_main_tree'] == PUBLISHED_MAIN['tree'],
          'relabelled historical or current identity')
    before, after = historical['base_files'], current['files']
    old_observer = historical['observer_files']
    _rows_schema(before, 376)
    _rows_schema(after, 376)
    _rows_schema(old_observer, 377, True)
    bmap, amap = ({row['path']: row for row in rows} for rows in (before, after))
    _need(list(bmap) == list(amap)
          and [name for name in amap if not _same(amap[name], bmap[name])] == [MAIN],
          'not the exact complete one-main source transition')
    omap = {row['path']: row for row in old_observer}
    _need(_same(omap[MAIN], bmap[MAIN]) and MAIN not in
          [row['path'] for row in historical['inserted_roster']],
          'transported main participates in observer patch')
    omap[MAIN] = amap[MAIN]
    observer = [omap[name] for name in sorted(omap, key=lambda n: PurePosixPath(n).parts)]
    result = {
        'schema': 'oxid-published-public-lifecycle-v1',
        'published_main': PUBLISHED_MAIN, 'source_checkpoint': CHECKPOINT,
        'retained_historical_source': HISTORICAL, 'source_seal': seal,
        'source_dispatcher': DISPATCHER_PIN, 'source_package_manifest': PACKAGE_PIN,
        'source_helpers': [{'module': name, 'body': body} for name, body in specs],
        'retained_public_api': RETAINED_PINS,
        'transport_helper': _binding(PUBLIC + '/' + HELPER_NAME, helper_raw),
        'historical_lifecycle_helper': OLD_HELPER_PIN,
        'historical_lifecycle_proof': OLD_PROOF_PIN,
        'source_manifest': _binding(PACKAGE + '/current-source.json', current_raw),
        'transport_paths': [MAIN], 'base_files': after, 'observer_files': observer,
        'complete_inverse_sha256': _digest(_canonical(after)),
        'patch': historical['patch'], 'predecessor_patch': historical['predecessor_patch'],
        'predecessor_authority': historical['predecessor_authority'],
        'inserted_roster': historical['inserted_roster'],
        'lexer_hook_correspondence': historical['lexer_hook_correspondence'],
        'compiler_invocations': 0, 'source_only': True, 'execution_qualified': False,
        'admission_route': 'authenticated-pure-source-then-one-original-M-admission'}
    _proof_schema(result)
    _need(_same(result['patch'], PATCH_PIN)
          and _same(result['predecessor_patch'], PREVIOUS_PATCH_PIN)
          and _same(result['predecessor_authority'], PREVIOUS_AUTHORITY_PIN),
          'historical lifecycle descriptors changed')
    # Fresh JSON data only; callers cannot mutate our constant pin dictionaries.
    return _json(_canonical(result))


def _authority_values(proof):
    return {'CURRENT_SOURCE_SHA': proof['source_manifest']['sha256'],
            'CURRENT_FILES_SHA': _digest(_canonical(proof['base_files'])),
            'OBSERVER_FILES_SHA': _digest(_canonical(proof['observer_files']))}


def _authority_suffix(proof_raw, helper_raw):
    helper = _binding(HELPER_NAME, helper_raw)
    proof = _binding(PROOF_NAME, proof_raw)
    return ('\n# Separate published-source descriptors; historical LEXER_* pins stay exact.\n'
            + 'PUBLISHED_LIFECYCLE_HELPER = ' + repr(helper) + '\n'
            + 'PUBLISHED_LIFECYCLE_PROOF = ' + repr(proof) + '\n').encode()


def render_authority(original, proof_raw, helper_raw):
    _verify(original, OLD_AUTHORITY_PIN)
    proof = _json(proof_raw)
    _proof_schema(proof)
    result = original
    for key, after in _authority_values(proof).items():
        before = (key + " = '" + OLD_CURRENT_VALUES[key] + "'\n").encode()
        _need(result.count(before) == 1, 'missing exact original current authority line')
        result = result.replace(before, (key + " = '" + after + "'\n").encode(), 1)
    return result + _authority_suffix(proof_raw, helper_raw)


def render_build(original):
    _verify(original, OLD_BUILD_PIN)
    before = b'    import lexer_reservation_lifecycle as current\n'
    _need(original.count(before) == 2, 'original build import count changed')
    return original.replace(before, b'    import published_lifecycle as current\n')


def render_tests(original):
    _verify(original, OLD_TEST_PIN)
    before = b'import lexer_reservation_lifecycle as m\n'
    name = b"'lexer-reservation-lifecycle-v2.json'"
    _need(original.count(before) == original.count(name) == 1,
          'original lifecycle test transport seam changed')
    return original.replace(before, b'import published_lifecycle as m\n', 1).replace(
        name, b"'published-lifecycle-v1.json'", 1)


def _stamp(value):
    # Deliberately omit atime: unchanged legacy readers may update it.
    return tuple(getattr(value, field) for field in (
        'st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size', 'st_mtime_ns', 'st_ctime_ns'))


def _safe_ancestors(path):
    records = []
    for parent in reversed(path.parents):
        info = parent.lstat()
        _need(stat.S_ISDIR(info.st_mode), 'aliased or special input ancestor')
        records.append((parent, (info.st_dev, info.st_ino, info.st_mode)))
    return records


def _read_regular(path, limit=MAX_BODY):
    path = Path(path).absolute()
    _need('..' not in path.parts, 'noncanonical input path')
    ancestors = _safe_ancestors(path)
    before = path.lstat()
    _need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
          and not before.st_mode & 0o111 and 0 <= before.st_size <= limit,
          'unsafe, aliased, executable or oversized input: ' + str(path))
    # No permissive fallback: a host lacking either flag needs separate review.
    _need(hasattr(os, 'O_NOFOLLOW') and hasattr(os, 'O_NOATIME'),
          'hardened read flags unavailable on this host')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NOATIME | os.O_NONBLOCK)
    try:
        _need(_stamp(os.fstat(fd)) == _stamp(before), 'input changed before read')
        chunks, remaining = [], before.st_size
        while remaining:
            piece = os.read(fd, min(remaining, 65536))
            _need(bool(piece), 'short stable input read')
            chunks.append(piece)
            remaining -= len(piece)
        _need(os.read(fd, 1) == b'' and _stamp(os.fstat(fd)) == _stamp(before),
              'input changed during read')
    finally:
        os.close(fd)
    _need(_stamp(path.lstat()) == _stamp(before), 'input name changed during read')
    for parent, expected in ancestors:
        actual = parent.lstat()
        _need((actual.st_dev, actual.st_ino, actual.st_mode) == expected,
              'input ancestor changed during read')
    return b''.join(chunks), _stamp(before)


# Bootstrap one stable genuine original class/function namespace. There is no
# dispatcher import and no predecessor view at import time. Original __file__,
# HERE and REPOSITORY always refer to the authentic current checkout.
Rejected = ValueError
HERE = Path(__file__).absolute().parent
REPOSITORY = HERE.parents[2]
_bootstrap_raw, _bootstrap_stamp = _read_regular(REPOSITORY / OLD_HELPER_PIN['path'])
_verify(_bootstrap_raw, OLD_HELPER_PIN)
_old = types.ModuleType('authenticated_original_public_lifecycle')
_old.__file__ = str(REPOSITORY / OLD_HELPER_PIN['path'])
exec(compile(_bootstrap_raw, _old.__file__, 'exec'), _old.__dict__)
Rejected = _old.Rejected
for _export in ('rows', 'exact_patch', 'inserted_roster', 'sections', 'adapter',
                'sha', 'need', 'LEXER'):
    globals()[_export] = getattr(_old, _export)
_need(_old.HERE == HERE and _old.REPOSITORY == REPOSITORY,
      'original lifecycle helper is not beside authentic authorities')
_OLD_GLOBALS = dict(vars(_old))
_OLD_CODES = {name: value.__code__ for name, value in vars(_old).items()
              if isinstance(value, types.FunctionType)}
del _bootstrap_raw, _bootstrap_stamp, _export
_ADMISSION = threading.Lock()
_POISONED = False


class _Snapshot:
    """Exact known-file brackets, not a recursive workspace scan or cache."""
    def __init__(self):
        self.files = {}
        self.rosters = []

    def read(self, path, descriptor=None):
        path = Path(path).absolute()
        raw, stamp = _read_regular(path)
        if descriptor is not None:
            _verify(raw, descriptor)
        record = (stamp, _digest(raw))
        _need(path not in self.files or self.files[path] == record,
              'input changed between authenticated reads')
        self.files[path] = record
        return raw

    def closed(self, root, names):
        root = Path(root).absolute()
        names = frozenset(_relative(n) for n in names)
        _closed_roster(root, names)
        self.rosters.append((root, names))

    def verify(self):
        for path, expected in self.files.items():
            raw, stamp = _read_regular(path)
            _need((stamp, _digest(raw)) == expected,
                  'input preservation failed: ' + str(path))
        for root, names in self.rosters:
            _closed_roster(root, names)


def _closed_roster(root, files):
    directories = {str(parent) for name in files for parent in PurePosixPath(name).parents
                   if str(parent) != '.'}
    _safe_ancestors(root / '_roster_sentinel')
    pending, seen_files, seen_dirs = [''], set(), set()
    while pending:
        parent = pending.pop()
        with os.scandir(root / parent) as entries:
            for entry in entries:
                name = parent + '/' + entry.name if parent else entry.name
                _need(name in files or name in directories, 'unknown file or empty directory: ' + name)
                info = entry.stat(follow_symlinks=False)
                if name in directories:
                    _need(stat.S_ISDIR(info.st_mode) and name not in seen_dirs,
                          'aliased or special source directory')
                    seen_dirs.add(name)
                    pending.append(name)
                else:
                    _need(stat.S_ISREG(info.st_mode) and info.st_nlink == 1
                          and not info.st_mode & 0o111 and name not in seen_files,
                          'aliased, special or executable source file')
                    seen_files.add(name)
    _need(seen_files == set(files) and seen_dirs == directories, 'incomplete exact roster')


def _compiler_rosters(snapshot, root, names):
    for prefix in ('src', 'native'):
        selected = {name[len(prefix) + 1:] for name in names if name.startswith(prefix + '/')}
        _need(bool(selected), 'missing complete compiler root')
        snapshot.closed(root / prefix, selected)


def _old_intact():
    _need(set(vars(_old)) == set(_OLD_GLOBALS)
          and all(vars(_old)[name] is value for name, value in _OLD_GLOBALS.items())
          and all(vars(_old)[name].__code__ is code for name, code in _OLD_CODES.items())
          and _old.HERE == HERE and _old.REPOSITORY == REPOSITORY,
          'original lifecycle namespace or function body changed')


def _decode_authority(raw, proof_raw, helper_raw):
    proof = _json(proof_raw)
    _proof_schema(proof)
    suffix = _authority_suffix(proof_raw, helper_raw)
    _need(raw.endswith(suffix), 'missing exact separate published descriptors')
    original = raw[:-len(suffix)]
    for key, current in _authority_values(proof).items():
        after = (key + " = '" + current + "'\n").encode()
        _need(original.count(after) == 1, 'wrong current authority identity')
        original = original.replace(after,
            (key + " = '" + OLD_CURRENT_VALUES[key] + "'\n").encode(), 1)
    _verify(original, OLD_AUTHORITY_PIN)
    _need(raw == render_authority(original, proof_raw, helper_raw),
          'noncanonical public authority transport')
    return proof


def _aggregate(message, failures):
    if len(failures) == 1:
        raise failures[0]
    raise BaseExceptionGroup(message, failures) from None


@contextmanager
def _aliases(specs, bodies, authority_raw):
    """Temporary aliases restored by absence/identity, even on interrupts.

    No M module is loaded. Retained public API modules are intentionally never
    registered. Unexpected aliases to a newly loaded helper are rejected and
    removed; discovery or failed restoration poisons subsequent admissions.
    """
    global _POISONED
    names = tuple(name for name, _ in specs) + ('authority',)
    _need(len(names) == len(set(names)), 'duplicate temporary alias')
    _need(not any(name in sys.modules for name in RETAINED_ALIASES),
          'unexpected retained runtime alias already installed')
    absent = object()
    saved = {name: sys.modules.get(name, absent) for name in names}
    modules, primary = {}, None
    bytecode = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        for name, row in specs:
            module = types.ModuleType(name)
            module.__file__ = str(REPOSITORY / row['path'])
            modules[name] = module
            sys.modules[name] = module
            exec(compile(bodies[name], module.__file__, 'exec'), module.__dict__)
        module = types.ModuleType('authority')
        module.__file__ = str(HERE / 'authority.py')
        modules['authority'] = module
        sys.modules['authority'] = module
        exec(compile(authority_raw, module.__file__, 'exec'), module.__dict__)
        _need(module.LEXER_LIFECYCLE_HELPER_SHA == OLD_HELPER_PIN['sha256']
              and module.LEXER_LIFECYCLE_AUTHORITY_SHA == OLD_PROOF_PIN['sha256']
              and module.LIFECYCLE_PATCH_SHA == PATCH_PIN['sha256'],
              'historical public authority pins were relabelled')
        yield modules
    except BaseException as error:
        primary = error
    failures = [] if primary is None else [primary]
    try:
        unexpected = [name for name, value in list(sys.modules.items())
                      if name not in names and any(value is item for item in modules.values())]
        unexpected += [name for name in RETAINED_ALIASES if name in sys.modules]
        changed = [name for name in modules if sys.modules.get(name, absent) is not modules[name]]
        for name in unexpected:
            sys.modules.pop(name, None)
        if unexpected or changed:
            _POISONED = True
            failures.append(Rejected('unexpected retained/rebound helper aliases: '
                                     + ', '.join(unexpected + changed)))
    except BaseException as error:
        _POISONED = True
        failures.append(error)
    # Attempt every restoration even if an earlier cleanup action failed.
    for name, value in saved.items():
        try:
            if value is absent:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = value
        except BaseException as error:
            _POISONED = True
            failures.append(error)
    try:
        _need(all((name not in sys.modules) if value is absent
                  else sys.modules.get(name) is value for name, value in saved.items()),
              'temporary alias restoration failed')
    except BaseException as error:
        _POISONED = True
        failures.append(error)
    try:
        sys.dont_write_bytecode = bytecode
    except BaseException as error:
        _POISONED = True
        failures.append(error)
    # Remove scoped imports' references to source captures and compiled runner
    # globals. The original public namespace remains stable in C. A cleanup
    # interrupt/error joins earlier failures rather than replacing them.
    for module in list(modules.values()):
        try:
            module.__dict__.clear()
        except BaseException as error:
            _POISONED = True
            failures.append(error)
    modules.clear()
    if failures:
        _aggregate('public observer operation and alias cleanup failed', failures)


def _view_manifest(raw, seal):
    _verify(raw, seal)
    value = _json(raw)
    _need(type(value) is dict and set(value) == {
        'schema', 'published_main', 'selected_source_manifest', 'files', 'directories', 'logical_bytes'}
        and value['schema'] == 'oxid-published-main-predecessor-view-v1'
        and _same(value['published_main'], PUBLISHED_MAIN)
        and type(value['logical_bytes']) is int and value['logical_bytes'] == 38003640,
        'wrong predecessor view schema or identity')
    _descriptor(value['selected_source_manifest'])
    _need(type(value['files']) is list and len(value['files']) == 681,
          'wrong predecessor view file count')
    names = []
    for row in value['files']:
        _need(type(row) is dict and set(row) == {
            'path', 'bytes', 'sha256', 'mode', 'git_blob', 'read_from'}, 'wrong view row fields')
        _descriptor({key: row[key] for key in ('path', 'bytes', 'sha256')})
        _need(type(row['mode']) is str and row['mode'] == '100644'
              and type(row['git_blob']) is str and re.fullmatch('[0-9a-f]{40}', row['git_blob'])
              and (row['read_from'] is None or type(row['read_from']) is str),
              'wrong view row types')
        if row['read_from'] is not None:
            _relative(row['read_from'])
        names.append(row['path'])
    directories = sorted({str(parent) for name in names for parent in PurePosixPath(name).parents
                          if str(parent) != '.'})
    _need(names == sorted(set(names)) and type(value['directories']) is list
          and all(type(item) is str for item in value['directories'])
          and value['directories'] == directories and len(directories) == 111
          and sum(row['bytes'] for row in value['files']) == value['logical_bytes'],
          'wrong complete predecessor roster')
    return value


def _capture_view(snapshot, value, admitted):
    old_inputs = admitted['predecessor_inputs']
    prefix = 'tests/qualification/published_formatter_current/predecessor-v1/'
    selected = []
    for row in value['files']:
        descriptor = {key: row[key] for key in ('path', 'bytes', 'sha256')}
        name = row['path']
        if name in old_inputs:
            _need(row['read_from'] is None, 'inverse selected input must not reopen C')
            body = old_inputs[name]
            selected.append(name)
        else:
            _need(row['read_from'] in (name, prefix + name), 'unexpected predecessor alias')
            body = snapshot.read(REPOSITORY / row['read_from'], descriptor)
        _verify(body, descriptor)
        _need(hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest()
              == row['git_blob'], 'wrong predecessor Git blob')
    _need(selected == sorted(old_inputs), 'incomplete exact M inverse')


def _materialized_snapshot(root, value):
    result = _Snapshot()
    result.closed(root, {row['path'] for row in value['files']})
    for row in value['files']:
        result.read(root / row['path'], {key: row[key] for key in ('path', 'bytes', 'sha256')})
    return result


def _one_original_admission(modules, admitted, seal, view_value, patch):
    global _POISONED
    view, directory, operation_error = None, None, None
    try:
        with modules['published_view'].predecessor_view(REPOSITORY, admitted, seal) as view:
            directory = view.parent
            _need(view != REPOSITORY and view != Path(admitted['_source_root'])
                  and view != REPOSITORY / PACKAGE, 'M and C role confusion')
            preserved = _materialized_snapshot(view, view_value)
            try:
                # Actual original helper, actual M source root, authentic C
                # __file__/HERE/REPOSITORY and unchanged historical authorities.
                _old_intact()
                derived, proof = _old.admit(view, patch)
            except BaseException as error:
                operation_error = error
                try:
                    preserved.verify()
                except BaseException as preservation:
                    _POISONED = True
                    raise BaseExceptionGroup('original admission and M preservation failed',
                                             [error, preservation]) from None
                raise
            else:
                preserved.verify()
                _old_intact()
        _need(directory is not None and not os.path.lexists(directory),
              'predecessor cleanup retained its directory')
    except BaseException as error:
        # Do not collapse an ExceptionGroup or replace an interrupt. The exact
        # accepted context manager can attach an earlier operation/preservation
        # failure to cleanup_error.__context__; retain that whole original chain.
        removed = directory is not None and not os.path.lexists(directory)
        if error is not operation_error or not removed:
            _POISONED = True
        raise
    # M is fully verified and gone before any main transport or C map checks.
    return derived, proof


def _transport(admitted, historical_observer, historical, patch, proof):
    current, predecessor = admitted['inputs'], admitted['predecessor_inputs']
    _need(type(current) is dict and type(predecessor) is dict
          and len(current) == len(predecessor) == 376
          and list(current) == list(predecessor)
          and all(type(name) is str and type(body) is bytes for name, body in current.items())
          and all(type(name) is str and type(body) is bytes for name, body in predecessor.items()),
          'wrong complete C/M input maps')
    _need([name for name in current if current[name] != predecessor[name]] == [MAIN],
          'unexpected current source transport body')
    _need(_same(rows(predecessor), historical['base_files'])
          and _same(rows(historical_observer, True), historical['observer_files']),
          'original admission did not return its genuine M maps')
    _need(MAIN not in [name for name, _, _ in sections(patch)],
          'transported main is in the observer patch')
    _need(historical_observer[MAIN] == predecessor[MAIN], 'M observer main changed')
    result = dict(historical_observer)
    result[MAIN] = current[MAIN]
    _need(len(result) == 377 and set(result) == set(historical_observer)
          and all(result[name] == historical_observer[name] for name in result if name != MAIN),
          'non-main observer transport changed')
    _need(_same(rows(current), proof['base_files'])
          and _same(rows(result, True), proof['observer_files'])
          and exact_patch(current, patch) == result
          and exact_patch(result, patch, True) == current,
          'complete current observer forward/inverse maps differ')
    roster = inserted_roster(patch)
    _need(len(roster) == 8 and _same(roster, proof['inserted_roster'])
          and _same(roster, historical['inserted_roster'])
          and _same(proof['lexer_hook_correspondence'], historical['lexer_hook_correspondence'])
          and sum(len(hunks) for _, added, hunks in sections(patch) if not added) == 16,
          'changed exact hook correspondence or context count')
    _need(_same(_json(_canonical(proof)), proof), 'non-data observer proof escaped')
    return result, proof


def _admit(root, patch, snapshot):
    _need(type(patch) is bytes, 'observer patch must be bytes')
    _verify(patch, PATCH_PIN)
    source_root = Path(root).absolute()
    _need('..' not in source_root.parts, 'noncanonical source root')
    _safe_ancestors(source_root / '_source_sentinel')
    authentic_checkout = REPOSITORY
    package = authentic_checkout / PACKAGE
    dispatcher_raw = snapshot.read(authentic_checkout / DISPATCHER_PIN['path'], DISPATCHER_PIN)
    package_raw = snapshot.read(authentic_checkout / PACKAGE_PIN['path'], PACKAGE_PIN)
    specs, seal = _dispatcher(dispatcher_raw)
    manifest = _json(package_raw)
    _need(type(manifest) is dict and set(manifest) == {'schema', 'files'}
          and type(manifest['schema']) is str, 'wrong current package schema')
    _rows_schema(manifest['files'], 99)
    package_rows = {row['path']: row for row in manifest['files']}
    _need('package-manifest.json' not in package_rows, 'cyclic self-listed package manifest')
    snapshot.closed(package, set(package_rows) | {'package-manifest.json'})
    for name, row in package_rows.items():
        snapshot.read(package / name, row)
    historical_package_raw = snapshot.read(authentic_checkout / HISTORICAL_PACKAGE_PIN['path'],
                                           HISTORICAL_PACKAGE_PIN)
    _json(historical_package_raw)
    bodies = {name: snapshot.read(authentic_checkout / row['path'], row) for name, row in specs}
    retained = {Path(row['path']).name: snapshot.read(authentic_checkout / row['path'], row)
                for row in RETAINED_PINS}
    _json(retained['package-manifest.json'])
    historical_raw = snapshot.read(authentic_checkout / OLD_PROOF_PIN['path'], OLD_PROOF_PIN)
    snapshot.read(authentic_checkout / OLD_HELPER_PIN['path'], OLD_HELPER_PIN)
    current_raw = snapshot.read(package / seal['source_manifest']['path'], seal['source_manifest'])
    helper_raw = snapshot.read(HERE / HELPER_NAME)
    _need(0 < len(helper_raw) <= 65536, 'oversized public observer helper')
    proof_raw = snapshot.read(HERE / PROOF_NAME)
    _need(len(proof_raw) <= 524288, 'oversized public observer proof')
    authority_raw = snapshot.read(HERE / 'authority.py')
    proof = _decode_authority(authority_raw, proof_raw, helper_raw)
    expected = make_proof(historical_raw, current_raw, dispatcher_raw, package_raw, helper_raw)
    _need(_same(proof, expected), 'changed typed closed published lifecycle proof')
    historical = _json(historical_raw)
    for row in (PATCH_PIN, PREVIOUS_PATCH_PIN, PREVIOUS_AUTHORITY_PIN):
        snapshot.read(HERE / row['path'], row)
    # The independent consumer closure must also include the exact unchanged
    # runtime/gate files, 173-consumer inventory and every generated output.
    # These two retained consumers have a local exact transport check as well.
    build_raw = snapshot.read(HERE / 'build.py')
    test_raw = snapshot.read(HERE / 'test_lexer_reservation.py')
    old_build = build_raw.replace(b'    import published_lifecycle as current\n',
                                 b'    import lexer_reservation_lifecycle as current\n')
    old_test = test_raw.replace(b'import published_lifecycle as m\n',
                               b'import lexer_reservation_lifecycle as m\n').replace(
                               b"'published-lifecycle-v1.json'", b"'lexer-reservation-lifecycle-v2.json'")
    _need(build_raw == render_build(old_build) and test_raw == render_tests(old_test),
          'public consumer changed outside exact import/proof-name seams')
    _compiler_rosters(snapshot, source_root, {row['path'] for row in proof['base_files']})
    for row in proof['base_files']:
        snapshot.read(source_root / row['path'], row)
    view_raw = snapshot.read(authentic_checkout / seal['predecessor_view']['path'],
                              seal['predecessor_view'])
    view_value = _view_manifest(view_raw, seal['predecessor_view'])
    with _aliases(specs, bodies, authority_raw) as modules:
        known_rejects = (modules['adapters'].Reject,)
        try:
            admitted_package_raw, package_bytes = modules['published_current']._current_package(
                authentic_checkout, package, seal)
            _need(admitted_package_raw == package_raw and set(package_bytes) == set(package_rows),
                  'legacy package admission returned a different closure')
            for name, body in package_bytes.items():
                _verify(body, package_rows[name])
                if name.endswith('.json'):
                    _json(body)  # reject duplicate keys before any legacy JSON consumer
            api, byte_helper = modules['source_transition'].retained_public_api(
                authentic_checkout / SAVED, retained)
            known_rejects = known_rejects + (api.BindingError,)
            # Only these two original pure API operations are called indirectly.
            # No capture_retained, install, preflight or historical runtime call.
            admitted = modules['published_transition'].admit(source_root, package_bytes, seal, api)
            _need(_same(admitted['current']['files'], proof['base_files'])
                  and _same(rows(admitted['inputs']), proof['base_files'])
                  and _same(rows(admitted['predecessor_inputs']), historical['base_files']),
                  'wrong current or inverse source admission')
            admitted['_source_root'] = str(source_root)
            _capture_view(snapshot, view_value, admitted)
            snapshot.verify()
            historical_observer, actual_historical = _one_original_admission(
                modules, admitted, seal['predecessor_view'], view_value, patch)
            _need(_same(actual_historical, historical), 'original helper returned relabelled proof')
            result = _transport(admitted, historical_observer, actual_historical, patch, proof)
            # Do not expose retained modules, M roots, C/M source captures or API
            # objects. Only complete current observer bytes and JSON proof leave.
            del api, byte_helper, admitted, package_bytes, historical_observer, actual_historical
            return result
        except known_rejects as error:
            # Convert only the specific authenticated source/API rejection types.
            # ExceptionGroups, interrupts, OS errors and cleanup failures retain
            # their original types, nested exceptions, causes and contexts.
            raise Rejected('published source admission rejected: ' + str(error)) from error


def admit(root, patch):
    """Compatible public API; authentic checkout and caller source stay distinct."""
    global _POISONED
    _need(not _POISONED, 'observer admission is poisoned; caller reconciliation required')
    _need(_ADMISSION.acquire(blocking=False), 'overlapping public observer admission')
    snapshot = _Snapshot()
    primary, result = None, None
    try:
        _need(not _POISONED, 'observer admission is poisoned')
        _old_intact()
        try:
            result = _admit(root, patch, snapshot)
        except BaseException as error:
            primary = error
        failures = [] if primary is None else [primary]
        try:
            snapshot.verify()
            _old_intact()
        except BaseException as preservation:
            _POISONED = True
            failures.append(preservation)
        if failures:
            _aggregate('public observer operation and input preservation failed', failures)
        return result
    finally:
        _ADMISSION.release()
