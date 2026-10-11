#!/usr/bin/env python3
"""Finite, inert current-source derivations for the seven bounded-native routes.

This package authenticates data and proves exact source transformations only.
It does not import retained/derived controllers, run Git, build, prepare, stage,
execute targets, or grant qualification. The actual producer/import transport
requires a separate review. No data flag can remove its operational barrier.
"""
from __future__ import annotations

import ast
import hashlib
import json
from pathlib import Path
import stat
import sys
import types

PACKAGE_FILES = frozenset(('bounded.py', 'recipes.json', 'test_bounded.py', 'package-manifest.json'))
RECIPES_SHA256 = 'a7bf8c780508c8d8dbb21fc8e4a9e804dd3ff735f17deeebc2404414d6357cce'
SOURCE_OWNER_KEY = '_oxid_package_dependency_ci_source_v1'
SOURCE_DEPENDENCY_SHA256 = 'a202686c177c128628420c4f208aff67cf33830878af2df6132c739ef9cf081d'
MEMBERS = ('enum', 'u8', 'byte-storage-public-build', 'byte-storage', 'stdin', 'stdout', 'owned-source-collector')
CURRENT_MANIFEST_PATH = 'tests/fixtures/package_dependency_source_v1/package-dependency-source-v1.json'
CURRENT_MANIFEST_SHA256 = 'f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
HISTORICAL_MANIFEST_PATH = 'tests/fixtures/typed_project_source_binding/current-source.json'
HISTORICAL_MANIFEST_SHA256 = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'


def need(condition, message):
    if not condition:
        raise ValueError(message)


def identity(body):
    need(type(body) is bytes, 'exact bytes required')
    return {'bytes': len(body), 'sha256': hashlib.sha256(body).hexdigest()}


def _unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def decode(body):
    return json.loads(body, object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def _safe_relative(name):
    need(type(name) is str, 'string relative path required')
    path = Path(name)
    need(name and not path.is_absolute() and path.as_posix() == name
         and all(part not in ('', '.', '..') for part in path.parts), 'unsafe relative path')
    return path


def _read(path, limit=8 * 1024 * 1024):
    path = Path(path)
    for part in (path, *path.parents):
        need(not part.is_symlink(), 'symlink input path')
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and before.st_size <= limit, 'nonregular or oversized input')
    with path.open('rb') as handle:
        body = handle.read(limit + 1)
    after = path.stat()
    need((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) ==
         (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
         and len(body) == before.st_size, 'input changed during read')
    return body


def authenticate():
    """Authenticate this closed four-file package; never load controller code."""
    root = Path(__file__).absolute().parent
    need(not root.is_symlink() and {path.name for path in root.iterdir()} == PACKAGE_FILES,
         'bounded package closed membership differs')
    manifest = decode(_read(root / 'package-manifest.json'))
    need(set(manifest) == {'schema', 'files', 'source_dependency_sha256', 'operational_release'},
         'bounded package manifest fields differ')
    need(manifest['schema'] == 'oxid-package-dependency-bounded-current-package-v1'
         and manifest['operational_release'] is None
         and manifest['source_dependency_sha256'] == SOURCE_DEPENDENCY_SHA256,
         'bounded package authority differs')
    need(set(manifest['files']) == PACKAGE_FILES - {'package-manifest.json'}, 'package file roster differs')
    for name, expected in manifest['files'].items():
        need(identity(_read(root / name)) == expected, 'bounded package body changed: ' + name)
    raw = _read(root / 'recipes.json')
    need(identity(raw)['sha256'] == RECIPES_SHA256, 'bounded recipe authority differs')
    registry = decode(raw)
    need(registry['members'] == list(MEMBERS) and set(registry['recipes']) == set(MEMBERS),
         'bounded member roster differs')
    need(registry['operational_release'] is None
         and registry['source_dependency_sha256'] == SOURCE_DEPENDENCY_SHA256
         and registry['source_owner_key'] == SOURCE_OWNER_KEY,
         'bounded dependency/release policy differs')
    need((registry['current_manifest_path'], registry['current_manifest_sha256']) ==
         (CURRENT_MANIFEST_PATH, CURRENT_MANIFEST_SHA256)
         and (registry['historical_manifest_path'], registry['historical_manifest_sha256']) ==
         (HISTORICAL_MANIFEST_PATH, HISTORICAL_MANIFEST_SHA256), 'source authorities changed')
    need(registry['ast_normalization'] == 'cpython-3.12-compatible-show-empty-v1'
         and registry['source_projection'] == 'current-all376', 'AST/projection policy differs')
    return registry


def read_retained_bodies(repository):
    """Read the finite retained inputs, without imports or source execution."""
    registry = authenticate()
    root = Path(repository).absolute()
    need(root.is_dir() and not root.is_symlink(), 'regular retained repository required')
    bodies = {name: _read(root / _safe_relative(name)) for name in registry['inputs']}
    _authenticate_retained(registry, bodies)
    return bodies


def _authenticate_retained(registry, bodies):
    need(type(bodies) is dict and set(bodies) == set(registry['inputs']),
         'retained input closure differs')
    for name, expected in registry['inputs'].items():
        need(identity(bodies[name]) == expected, 'retained input changed: ' + name)
    old = decode(bodies[HISTORICAL_MANIFEST_PATH])
    current = decode(bodies[CURRENT_MANIFEST_PATH])
    def source_map(document):
        rows = document['files']
        need(type(rows) is list and len(rows) == 376, 'source membership count differs')
        found = {}
        for row in rows:
            _safe_relative(row['path'])
            need(row['path'] not in found, 'duplicate source member')
            found[row['path']] = {'bytes': row['bytes'], 'sha256': row['sha256']}
        core = [name for name in found if name.startswith(('src/', 'native/'))
                or name in ('Cargo.toml', 'Cargo.lock', 'build.rs')]
        need(len(core) == 291, 'compiler source closure differs')
        return found
    old_map, current_map = source_map(old), source_map(current)
    need(set(old_map) == set(current_map), 'current source membership changed')
    need([name for name in old_map if old_map[name] != current_map[name]] == ['src/main.rs'],
         'unexpected current source delta')
    need(old_map['src/main.rs'] == registry['historical_main']
         and current_map['src/main.rs'] == registry['current_main'], 'main source transition differs')
    return {'historical_manifest_sha256': HISTORICAL_MANIFEST_SHA256,
            'current_manifest_sha256': CURRENT_MANIFEST_SHA256,
            'source_members': 376, 'compiler_bodies': 291,
            'changed_bodies': ['src/main.rs'], 'current_checkpoint': current['reviewed_source_head'],
            'current_source_tree': current['source_only_tree']}


def _transform(raw, record, *, inverse=False):
    expected = record['current_identity'] if inverse else record['historical_identity']
    need(identity(raw) == expected, 'derivation body identity differs')
    text = raw.decode('utf-8')
    edits = reversed(record['edits']) if inverse else record['edits']
    for edit in edits:
        before, after = (edit['after'], edit['before']) if inverse else (edit['before'], edit['after'])
        need(type(edit['count']) is int and edit['count'] > 0
             and text.count(before) == edit['count'], 'exact edit occurrence differs: ' + edit['label'])
        text = text.replace(before, after)
    result = text.encode('utf-8')
    need(identity(result) == (record['historical_identity'] if inverse else record['current_identity']),
         'derived output identity differs')
    return result


def _dump(node):
    # CPython 3.13 omits empty fields by default; the 3.12 representation is
    # our explicit versioned canonical form, with all empty fields included.
    need(sys.version_info[:2] in ((3, 12), (3, 13)), 'canonical AST proof requires CPython 3.12 or 3.13')
    options = {'show_empty': True} if sys.version_info[:2] >= (3, 13) else {}
    return ast.dump(node, include_attributes=False, **options)


def _functions(tree):
    return {node.name: node for node in tree.body if isinstance(node, (ast.FunctionDef, ast.ClassDef))}


def _process_calls(tree):
    """Pin actual commands, flags, environment arguments and native instrumentation."""
    names = {'command', 'invoke', 'run_commands', 'native_test_command', 'test_command', 'Popen'}
    calls = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        function = node.func
        selected = isinstance(function, ast.Name) and function.id in names
        selected |= (isinstance(function, ast.Attribute) and function.attr in {'invoke', 'run', 'Popen', 'test_command'})
        if selected:
            calls.append(_dump(node))
    return sorted(calls)


def _roster_assignments(tree):
    """All original uppercase module constants, including fixture/roster seals."""
    return {name.id: _dump(node) for node in tree.body if isinstance(node, ast.Assign)
            for name in node.targets if isinstance(name, ast.Name) and name.id.isupper()}


def _prove_module(raw, derived, record, registry):
    old_tree, new_tree = ast.parse(raw), ast.parse(derived)
    old, new = _functions(old_tree), _functions(new_tree)
    need(set(old) == set(new), 'function/class roster changed')
    need(set(old) == set(record['unchanged_functions']) | set(record['changed_functions']),
         'incomplete semantic proof inventory')
    for name, digest in record['unchanged_functions'].items():
        need(_dump(old[name]) == _dump(new[name])
             and identity(_dump(old[name]).encode())['sha256'] == digest,
             'unchanged semantic function drift: ' + name)
    for name, pins in record['changed_functions'].items():
        need(identity(_dump(old[name]).encode())['sha256'] == pins['historical_ast_sha256']
             and identity(_dump(new[name]).encode())['sha256'] == pins['current_ast_sha256'],
             'enumerated source/import seam drift: ' + name)
    need(_roster_assignments(old_tree) == _roster_assignments(new_tree), 'fixed roster/fixture constants changed')
    need(_process_calls(old_tree) == _process_calls(new_tree), 'process command/profile/instrumentation changed')
    need(_transform(derived, record, inverse=True) == raw, 'exact inverse failed')
    # The first non-docstring/future executable statement unconditionally raises.
    executable = [node for node in new_tree.body
                  if not (isinstance(node, ast.ImportFrom) and node.module == '__future__')
                  and not (isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant)
                           and isinstance(node.value.value, str))]
    need(executable and isinstance(executable[0], ast.Raise)
         and _dump(executable[0]) == _dump(ast.parse('raise RuntimeError("bounded current execution is not reviewed")').body[0]),
         'derived module operational barrier missing')
    repo_imports = {Path(row['historical_path']).stem for row in registry['modules'].values()}
    for node in ast.walk(new_tree):
        if isinstance(node, ast.Import):
            need(all(alias.name not in repo_imports for alias in node.names), 'historical ambient import remains')
        if isinstance(node, ast.ImportFrom):
            need(node.module not in repo_imports, 'historical ambient from-import remains')
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == '_load_current':
            need(len(node.args) == 1 and isinstance(node.args[0], ast.Constant)
                 and node.args[0].value in registry['modules'] and not node.keywords,
                 'nonclosed current import edge')
    return {'exact_inverse': True, 'full_original_ast_recovered': True,
            'unchanged_function_count': len(record['unchanged_functions']),
            'changed_functions': sorted(record['changed_functions']),
            'unchanged_rosters': sorted(_roster_assignments(old_tree)),
            'unchanged_process_call_count': len(_process_calls(old_tree)),
            'historical_identity': identity(raw), 'current_identity': identity(derived),
            'operational': False}


def fixed_derivation(member, retained_bodies):
    """Return inert module bytes and proofs for the fixed source transition.

    This is deliberately NOT a SourcePolicy or a live-context positive control.
    The complete closed sibling bundle is shared across the seven fixed members.
    """
    registry = authenticate()
    need(type(member) is str and member in MEMBERS, 'unknown bounded member')
    source_proof = _authenticate_retained(registry, retained_bodies)
    modules, proofs = {}, {}
    for name, record in registry['modules'].items():
        raw = retained_bodies[record['historical_path']]
        derived = _transform(raw, record)
        proofs[name] = _prove_module(raw, derived, record, registry)
        modules[record['current_path']] = derived
    return {'schema': 'oxid-bounded-current-inert-derivation-v1', 'inert': True,
            'operational': False, 'member': member, 'recipe': registry['recipes'][member],
            'source_relationship': source_proof, 'modules': modules, 'proofs': proofs,
            'source_dependency_sha256': SOURCE_DEPENDENCY_SHA256}


def inverse(module, derived_body):
    registry = authenticate()
    need(type(module) is str and module in registry['modules'], 'unknown derived module')
    return _transform(derived_body, registry['modules'][module], inverse=True)


def _source_owner(context):
    authenticate()
    owner = globals().get('_SOURCE_OWNER')
    need(type(owner) is types.ModuleType and sys.modules.get(SOURCE_OWNER_KEY) is owner,
         'original A-issued bounded adapter required; ambient owner adoption is forbidden')
    need(owner.identity(Path(owner.__file__))['sha256'] == SOURCE_DEPENDENCY_SHA256,
         'bounded source owner body drift')
    need(owner.owner() is owner and owner.owns_adapter(sys.modules.get(__name__), context),
         'bounded adapter was not issued for this original context')
    return owner


_POLICIES = {}


def _canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def _context_view(owner, context):
    """Only the canonical owner can re-admit the actual current CI context."""
    document = owner.verify_job(context)
    projection = owner.projection(context, 'current-all376')
    registry = authenticate()
    root = Path(document['repository_path'])
    need(root.is_absolute() and root.resolve(strict=True) == root, 'canonical repository path required')
    raw = _read(root / CURRENT_MANIFEST_PATH)
    need(identity(raw) == registry['inputs'][CURRENT_MANIFEST_PATH], 'current source manifest identity differs')
    manifest = decode(raw)
    rows = sorted(manifest['files'], key=lambda row: row['path'])
    expected_identity = dict(identity(raw), path=str(root / CURRENT_MANIFEST_PATH))
    expected_checkpoint = {'head': manifest['reviewed_source_head'], 'tree': manifest['source_only_tree']}
    need(document['source_files'] == rows
         and document['source_manifest_identity'] == expected_identity
         and document['source_checkpoint'] == expected_checkpoint,
         'job context does not bind the complete fixed current source')
    need(projection == {'id': 'current-all376', 'files': rows,
                        'manifest_identity': expected_identity, 'source_checkpoint': expected_checkpoint},
         'fixed current source projection differs')
    return document, manifest


class CurrentPolicy:
    """Immutable original-owner registry object; documents never recreate it."""
    __slots__ = ()

    def __new__(cls, *args, **kwargs):
        raise TypeError('current policies are issued only from original canonical job contexts')

    @property
    def manifest_path(self):
        _policy_record(self)
        return CURRENT_MANIFEST_PATH

    @property
    def manifest_sha256(self):
        _policy_record(self)
        return CURRENT_MANIFEST_SHA256

    @property
    def controller_members(self):
        _policy_record(self)
        registry = authenticate()
        current_modules = [record['current_path'] for record in registry['modules'].values()]
        package = 'tests/qualification/package_dependency_bounded_current_v1/'
        source_package = 'tests/qualification/package_dependency_ci_v1/'
        source_files = ('source.py', 'jobs.py', 'artifacts.py', 'test_source.py', 'test_jobs.py',
                        'test_artifacts.py', 'routes.json', 'inputs.json', 'package-manifest.json', 'README.md')
        production_files = [
            'tests/qualification/package_dependency_production_process_v1/' + name
            for name in ('preparation.py', 'test_preparation.py', 'package-manifest.json')]
        production_files += [
            'tests/qualification/package_dependency_production_machine_v1/' + name
            for name in ('machine.py', 'test_machine.py', 'package-manifest.json')]
        return tuple(sorted(current_modules + [package + name for name in PACKAGE_FILES]
                            + [source_package + name for name in source_files] + production_files))

    def require_manifest_path(self, path):
        record = _policy_record(self)
        need(Path(path) == Path(record['document']['repository_path']) / CURRENT_MANIFEST_PATH,
             'current policy manifest path differs')

    def require_manifest_document(self, document):
        record = _policy_record(self)
        need(_canonical(document) == record['manifest'], 'collector requires exact current manifest document')

    def document(self):
        record = _policy_record(self)
        return decode(record['document_bytes'])


def _policy_record(policy):
    need(type(policy) is CurrentPolicy and policy in _POLICIES, 'original bounded CurrentPolicy required')
    record = _POLICIES[policy]
    need(_source_owner(record['context']) is record['owner'], 'current policy canonical source owner changed')
    return record


def current_policy(context):
    """Issue only from canonical Package A; unresolved dependency pins reject."""
    owner = _source_owner(context)
    document, manifest = _context_view(owner, context)
    policy = object.__new__(CurrentPolicy)
    _POLICIES[policy] = {'owner': owner, 'context': context, 'document': document,
                         'document_bytes': _canonical(document), 'manifest': _canonical(manifest)}
    return policy


def admit_policy(policy):
    # The private record lookup does not invoke caller attributes or callbacks.
    record = _policy_record(policy)
    document, manifest = _context_view(record['owner'], record['context'])
    need(_canonical(document) == record['document_bytes'] and _canonical(manifest) == record['manifest'],
         'current policy source context changed')
    return policy


def derive(member, retained_bodies, current_policy):
    policy = admit_policy(current_policy)
    need(type(member) is str and member in MEMBERS, 'unknown bounded member')
    registry = authenticate()
    record = _policy_record(policy)
    # The original live job must own this particular fixed route.
    record['owner'].route(record['context'], registry['recipes'][member]['original_id'])
    result = fixed_derivation(member, retained_bodies)
    result['current_job_source'] = policy.document()
    return result


def plan(context, member):
    need(type(member) is str and member in MEMBERS, 'unknown bounded member')
    policy = current_policy(context)
    record = _policy_record(policy)
    bodies = read_retained_bodies(record['document']['repository_path'])
    result = derive(member, bodies, policy)
    return {'schema': 'oxid-bounded-current-inert-plan-v1', 'inert': True, 'operational': False,
            'recipe': result['recipe'], 'current_job_source': result['current_job_source'],
            'source_relationship': result['source_relationship'], 'proofs': result['proofs'],
            'derived_module_identities': {name: identity(body) for name, body in result['modules'].items()}}


def current_cli_policy():
    raise ValueError('current CLI policy requires the separately reviewed actual producer seam')


def load_derived_sibling(name):
    """No import or eval is available until actual process/import review."""
    raise ValueError('derived controller loading requires a separately reviewed operational release')


def run(context, member, admitted_builds, fresh_output):
    """Always fail before reading objects, creating output, or causing effects."""
    raise ValueError('bounded actual build/process seam is not reviewed; execution is blocked')
