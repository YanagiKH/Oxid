"""Authenticated, inert Unit1 current-source derivation. No execution authority.

Retained controllers and generated controller bytes are data: never imported,
compiled, executed, or written by this module. Canonical A only registers bounded;
policy, plan, and run remain unconditional holds until a separate owner review.
"""
from __future__ import annotations

import ast
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import stat
import sys

PACKAGE_FILES = frozenset(('unit1.py', 'recipes.json', 'test_unit1.py', 'package-manifest.json'))
RECIPES_SHA256 = '057410d78fbe414f8d3c1daf2c90a08259e97369fc921916073f1489550eea5b'
MEMBERS = ('unit1-both',)
ORIGINAL = 'tests/fixtures/typed_project_unit1_independent/'
RETAINED = 'tests/qualification/lexer_reservation_current/'
RUNNER = RETAINED + 'unit1_run_current.py'
CURRENT_MANIFEST = 'tests/fixtures/package_dependency_source_v1/package-dependency-source-v1.json'
HISTORICAL_MANIFEST = 'tests/fixtures/typed_project_source_binding/current-source.json'
BARRIER = b"raise RuntimeError('Inert Unit1 derivation: original source/process/artifact owner release required')\n"
HOLD = 'Unit1 is unregistered: canonical A registration and actual PreparedSource/process/test-artifact owner review required'


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


def _json(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode()


def _relative(name):
    need(type(name) is str, 'exact string path required')
    path = Path(name)
    need(name and not path.is_absolute() and path.as_posix() == name
         and all(part not in ('', '.', '..') for part in path.parts), 'unsafe relative path')
    return path


def _read(path):
    path = Path(path).absolute()
    need(all(not part.is_symlink() for part in (path, *path.parents)), 'symlink input')
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and not before.st_mode & 0o111
         and before.st_size <= 8 * 1024 * 1024, 'nonregular, executable, or oversized input')
    with path.open('rb') as stream:
        body = stream.read(8 * 1024 * 1024 + 1)
    after = path.stat()
    need((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) ==
         (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
         and len(body) == before.st_size, 'changed input during read')
    return body


def authenticate():
    """Authenticate the closed new package, without importing retained code."""
    root = Path(__file__).absolute().parent
    need(not root.is_symlink() and {p.name for p in root.iterdir()} == PACKAGE_FILES,
         'Unit1 package membership differs')
    manifest = decode(_read(root / 'package-manifest.json'))
    need(set(manifest) == {'schema', 'files', 'operational_release', 'owner_registration'},
         'Unit1 manifest fields differ')
    need(manifest['schema'] == 'oxid-package-dependency-unit1-current-package-v1'
         and manifest['operational_release'] is None and manifest['owner_registration'] is None,
         'Unit1 owner/release is held')
    need(set(manifest['files']) == PACKAGE_FILES - {'package-manifest.json'}, 'Unit1 sealed roster differs')
    for name, expected in manifest['files'].items():
        need(identity(_read(root / name)) == expected, 'Unit1 package body changed: ' + name)
    raw = _read(root / 'recipes.json')
    need(identity(raw)['sha256'] == RECIPES_SHA256, 'Unit1 recipes changed')
    registry = decode(raw)
    need(registry['schema'] == 'oxid-package-dependency-unit1-inert-recipes-v1'
         and registry['members'] == list(MEMBERS) and set(registry['recipes']) == set(MEMBERS)
         and registry['operational_release'] is None and registry['owner_registration'] is None,
         'Unit1 fixed recipe/release differs')
    need(registry['ast_normalization'] == 'explicit-fields-no-locations-v1', 'AST normalization differs')
    return registry


def read_retained_bodies(repository):
    """Read exactly the fixed public Unit1 closure, with no controller imports."""
    registry = authenticate()
    root = Path(repository).absolute()
    bodies = {name: _read(root / _relative(name)) for name in registry['inputs']}
    _authenticate_retained(registry, bodies)
    return bodies


def _authenticate_retained(registry, bodies):
    # Every complete body is checked before any retained JSON or Python is parsed.
    need(type(bodies) is dict and set(bodies) == set(registry['inputs']), 'retained closure differs')
    for name, expected in registry['inputs'].items():
        need(identity(bodies[name]) == expected, 'retained body changed: ' + name)


def _normal(value):
    """Versioned structural AST form includes empty fields and excludes locations."""
    if isinstance(value, ast.AST):
        return [type(value).__name__, [[name, _normal(item)] for name, item in ast.iter_fields(value)]]
    if isinstance(value, list):
        return [_normal(item) for item in value]
    if isinstance(value, bytes):
        return ['bytes', value.hex()]
    if isinstance(value, complex):
        return ['complex', repr(value)]
    if value is Ellipsis:
        return ['ellipsis']
    return value


def _node_id(node):
    need(sys.version_info[:2] in ((3, 12), (3, 13)), 'AST proofs require CPython 3.12 or 3.13')
    return identity(_json(_normal(node)))


def _definitions(body):
    tree = ast.parse(body)
    result = []
    def visit(node, prefix):
        for child in ast.iter_child_nodes(node):
            child_prefix = prefix
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                child_prefix = prefix + [child.name]
                start = min([child.lineno] + [item.lineno for item in child.decorator_list])
                raw = b''.join(body.splitlines(keepends=True)[start - 1:child.end_lineno])
                result.append({'name': '.'.join(child_prefix), 'kind': type(child).__name__,
                               'body': identity(raw), 'ast': _node_id(child)})
            visit(child, child_prefix)
    visit(tree, [])
    return result


def _preserved_nodes(body):
    predicates, commands, constants = [], [], []
    tree = ast.parse(body)
    for node in ast.walk(tree):
        if isinstance(node, (ast.Assert, ast.If, ast.While, ast.IfExp)):
            predicates.append(_node_id(node.test)['sha256'])
        if isinstance(node, (ast.Compare, ast.BoolOp)):
            predicates.append(_node_id(node)['sha256'])
        if isinstance(node, ast.Call):
            name = node.func.id if isinstance(node.func, ast.Name) else (
                node.func.attr if isinstance(node.func, ast.Attribute) else '')
            if name in ('run', 'checked', 'Popen', 'check_call', 'check_output'):
                commands.append(_node_id(node)['sha256'])
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            if any(isinstance(item, ast.Name) and (item.id.isupper() or item.id == 'argv') for item in targets):
                constants.append(_node_id(node)['sha256'])
    return {'predicates': sorted(predicates), 'commands': sorted(commands), 'constants_and_argv': sorted(constants)}


def _transform(raw, record, inverse=False):
    need(identity(raw) == record['derived' if inverse else 'retained'], 'wrong complete derivation input')
    result = raw
    for edit in reversed(record['edits']) if inverse else record['edits']:
        before = edit['after' if inverse else 'before'].encode()
        after = edit['before' if inverse else 'after'].encode()
        need(before and before != after and type(edit['count']) is int and edit['count'] > 0
             and result.count(before) == edit['count'], 'exact occurrence differs: ' + edit['island'])
        result = result.replace(before, after)
    need(identity(result) == record['retained' if inverse else 'derived'], 'wrong complete derivation output')
    return result


def _literal_assignments(body, names):
    """Read selected literal assignments through AST only; no retained execution."""
    result = {}
    for node in ast.parse(body).body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
            name = node.targets[0].id
            if name in names:
                need(name not in result, 'duplicate retained literal')
                result[name] = ast.literal_eval(node.value)
    need(set(result) == set(names), 'missing retained literal')
    return result


def _historical_inverses(bodies):
    seams = _literal_assignments(bodies[RETAINED + 'unit1_derivation.py'], ('RUNNER_SEAMS', 'REVIEWER_SEAMS'))
    proofs = []
    for original, derived, key in (('run.py', 'unit1_run_current.py', 'RUNNER_SEAMS'),
                                    ('run_reviewer.py', 'unit1_run_reviewer_current.py', 'REVIEWER_SEAMS')):
        body = bodies[ORIGINAL + original]
        for before, after in seams[key]:
            need(body.count(before) == 1 and after not in body, 'historical derivation seam drift')
            body = body.replace(before, after, 1)
        need(body == bodies[RETAINED + derived], 'historical exact runner derivation differs')
        for before, after in reversed(seams[key]):
            need(body.count(after) == 1, 'historical inverse occurrence differs')
            body = body.replace(after, before, 1)
        need(body == bodies[ORIGINAL + original], 'historical complete inverse differs')
        proofs.append({'original': original, 'retained': derived, 'inverse': identity(body), 'seams': len(seams[key])})
    literals = _literal_assignments(bodies[RETAINED + 'adapters.py'], ('UNIT1_BEFORE', 'UNIT1_AFTER'))
    before, after = literals['UNIT1_BEFORE'], literals['UNIT1_AFTER']
    original, adapted = bodies[ORIGINAL + 'reviewer_additional.rs'], bodies[RETAINED + 'unit1_reviewer_additional.current.rs']
    need(original.count(before) == 1 and adapted.count(after) == 1
         and original.replace(before, after, 1) == adapted
         and adapted.replace(after, before, 1) == original, 'historical reviewer exact inverse differs')
    proofs.append({'original': 'reviewer_additional.rs', 'inverse': identity(original), 'seams': 1})
    return proofs


def _source_relationship(registry, bodies):
    old, current = decode(bodies[HISTORICAL_MANIFEST]), decode(bodies[CURRENT_MANIFEST])
    def mapping(document):
        result = {}
        need(len(document['files']) == 376, 'source member count differs')
        for row in document['files']:
            name = row['path']
            _relative(name)
            need(name not in result, 'duplicate source member')
            result[name] = {'bytes': row['bytes'], 'sha256': row['sha256']}
        need(len([name for name in result if name.startswith(('src/', 'native/'))
                  or name in ('Cargo.toml', 'Cargo.lock', 'build.rs')]) == 291, 'compiler body count differs')
        return result
    before, after = mapping(old), mapping(current)
    transition = registry['source_transition']
    need(set(before) == set(after) and [name for name in before if before[name] != after[name]] == ['src/main.rs'],
         'unexpected current source transition')
    for kind, found in (('historical_main', before['src/main.rs']), ('current_main', after['src/main.rs'])):
        expected = transition[kind]
        need(found == {'bytes': expected['bytes'], 'sha256': expected['sha256']}, 'main identity differs')
    need(after['src/main.rs'] == identity(bodies['src/main.rs'])
         and after['src/frontend/project.rs'] == identity(bodies['src/frontend/project.rs']), 'retained current source body differs')
    need(current['reviewed_source_head'] == transition['current_checkpoint']['head']
         and current['source_only_tree'] == transition['current_checkpoint']['tree'], 'current checkpoint differs')
    return {'projection_required': 'current-all376', 'manifest_members': 376, 'compiler_members': 291,
            'current_manifest': identity(bodies[CURRENT_MANIFEST]), 'historical_manifest': identity(bodies[HISTORICAL_MANIFEST]),
            'current_main': identity(bodies['src/main.rs']), 'changed_manifest_members': ['src/main.rs'],
            'live_checkout_admitted': False, 'all376_bodies_read': False,
            'actual_head_tree_and_controller_closure': 'held for original A context'}


def _coverage(registry, bodies):
    names = decode(bodies[ORIGINAL + 'expected-test-roster.json'])['tests']
    expected = registry['coverage']
    need(names == expected['original_names'] and len(names) == len(set(names)) == 69, 'original69 roster differs')
    controls = re.findall(rb'#\[test\]\s*fn ([A-Za-z0-9_]+)\(', bodies[RETAINED + 'unit1_lexer_controls.rs'])
    need(controls == [expected['control_name'].rsplit('::', 1)[1].encode()]
         and expected['control_name'] not in names, 'separate lexer control differs')
    old_map, new_map = decode(bodies[RETAINED + 'unit1_original_package.json']), decode(bodies[RETAINED + 'unit1_current_bindings.json'])
    need([row['path'] for row in old_map['files']] == expected['frozen_package_members'], 'original16 map differs')
    for row in old_map['files']:
        raw = bodies[ORIGINAL + row['path']]
        need(row == {'path': row['path'], **identity(raw), 'mode': '100644',
                     'git_blob': hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()}, 'original map binding differs')
    for row in new_map['files']:
        need(row == {'path': row['path'], **identity(bodies[RETAINED + row['path']])}, 'historical current map differs')
    correspondence = decode(bodies[RETAINED + 'unit1_correspondence.json'])
    need([row['name'] for row in correspondence['cases']] == names
         and correspondence['profiles'] == ['debug', 'release']
         and correspondence['execution_qualified'] is False, 'historical correspondence differs')
    fixtures = decode(bodies[ORIGINAL + 'expected-fixtures.json'])['fixtures']
    resources = decode(bodies[ORIGINAL + 'expected-resources.json'])['fixtures']
    projected_fixtures = [{'id': row['id'], 'expected': row['expected'], 'unavailable': row.get('unavailable', [])} for row in fixtures]
    projected_resources = [{'id': row['id'], 'expected': row['expected']} for row in resources]
    need(projected_fixtures == expected['fixture_expectations']
         and projected_resources == expected['resource_expectations'], 'frozen expectation semantics differ')
    inventories = [{'id': row['id'], 'files': len(row['files']), 'complete_files': identity(_json(row['files'])),
                    'complete_paths': identity(_json([item['path'] for item in row['files']]))} for row in resources]
    need(inventories == expected['resource_inventories'], 'full resource membership differs')
    need(len(fixtures) == 42 and sum(not row.get('unavailable') for row in fixtures) == 41
         and len(resources) == 12, 'fixture/resource cardinality differs')
    return {'profiles': ['debug', 'release'], 'original_names': names, 'control_names': [expected['control_name']],
            'original_executions_required': 138, 'control_executions_required': 2,
            'filesystem_authored': 42, 'filesystem_available': 41, 'resources': 12,
            'resource_inventories': inventories, 'execution_qualified': False}


def fixed_derivation(retained_bodies):
    """Return inert exact bytes and preservation proofs; never a live capability."""
    registry = authenticate()
    need(type(retained_bodies) is dict, 'exact retained mapping required')
    retained_bodies = retained_bodies.copy()
    _authenticate_retained(registry, retained_bodies)
    relationship = _source_relationship(registry, retained_bodies)
    coverage = _coverage(registry, retained_bodies)
    historical = _historical_inverses(retained_bodies)
    inventories = {name: _definitions(body) for name, body in retained_bodies.items() if name.endswith('.py')}
    need(inventories == registry['retained_definitions'], 'complete retained function/class inventory differs')
    retained_nodes = {name: _preserved_nodes(body) for name, body in retained_bodies.items() if name.endswith('.py')}
    need(retained_nodes == registry['retained_predicates_commands_constants'], 'retained predicate/command inventory differs')
    record = registry['recipes']['unit1-both']
    derived = _transform(retained_bodies[RUNNER], record)
    recovered = _transform(derived, record, inverse=True)
    need(recovered == retained_bodies[RUNNER], 'complete runner inverse differs')
    tree = ast.parse(derived)
    need(isinstance(tree.body[0], ast.Raise) and derived.startswith(BARRIER), 'first executable barrier missing')
    before_nodes, after_nodes = _preserved_nodes(recovered), _preserved_nodes(derived)
    for kind in before_nodes:
        need(not (Counter(before_nodes[kind]) - Counter(after_nodes[kind])), 'retained runner obligation removed: ' + kind)
    derived_definitions = _definitions(derived)
    need(derived_definitions == record['derived_definitions'], 'derived function/class inventory differs')
    before = {row['name']: row for row in inventories[RUNNER]}
    after = {row['name']: row for row in derived_definitions}
    need(set(before) == set(after), 'runner function/class roster differs')
    changed = [name for name in before if before[name] != after[name]]
    need(changed == ['main'], 'source-binding changes escaped main')
    return {'schema': 'oxid-package-dependency-unit1-inert-derivation-v1',
            'members': {'unit1-both': derived}, 'source_relationship': relationship, 'coverage': coverage,
            'proofs': {'historical_inverses': historical, 'retained_definitions': inventories,
                       'retained_predicates_commands_constants': retained_nodes,
                       'derived_definitions': derived_definitions, 'runner_inverse': identity(recovered),
                       'unchanged_runner_definitions': [name for name in before if name not in changed],
                       'changed_runner_definitions': changed, 'source_only_islands': record['edits'],
                       'all_original_runner_predicates_commands_constants_preserved': True},
            'owner_registration': None, 'operational_release': None, 'execution_qualified': False}


def inverse(member, derived_body):
    """Recover exactly the pinned retained runner, accepting no other member/body."""
    need(type(member) is str and member in MEMBERS, 'unknown fixed Unit1 member')
    registry = authenticate()
    return _transform(derived_body, registry['recipes'][member], inverse=True)


def current_policy(original_job_context):
    raise RuntimeError(HOLD)


def plan(original_job_context):
    raise RuntimeError(HOLD)


def run(*args, **kwargs):
    raise RuntimeError(HOLD)
