"""Current lexer source admission around the immutable byte-storage dispatcher.

This stage authenticates real inputs before making an isolated predecessor view.
No compiler, private parser package, source mutation or qualification claim is made
by admission. Historical execution retains its original selected-byte identity.
"""
import importlib.util
import json
import os
import stat
from pathlib import Path
import sys
import tempfile
import types

import adapters
import source_transition
from seal import SEAL

HERE = Path(__file__).resolve().parent
RETAINED_NAME = 'typed_project_source_binding_byte_storage_v1'


def retained_package(package):
    return Path(package).parent / RETAINED_NAME


def closed_package(root, expected):
    """Exact files and directories, with no cache-name or empty-dir exclusions."""
    adapters.need(root.is_dir() and not root.is_symlink(), 'nonregular package root')
    files, directories = set(), set()
    for parent, ds, fs in os.walk(root, followlinks=False):
        for name in ds + fs:
            path = Path(parent) / name
            adapters.need(not path.is_symlink(), 'symlink package member')
            relative = path.relative_to(root).as_posix()
            if name in ds:
                directories.add(relative)
            else:
                mode = path.stat().st_mode
                adapters.need(stat.S_ISREG(mode) and not mode & 0o111,
                              'nonregular or executable package member')
                files.add(relative)
    wanted_dirs = {str(parent) for name in expected for parent in Path(name).parents
                   if str(parent) != '.'}
    adapters.need(files == set(expected) and directories == wanted_dirs,
                  'missing or extra closed package member/directory')


def capture_retained(package):
    saved = retained_package(package)
    raw = (saved / 'package-manifest.json').read_bytes()
    adapters.need(adapters.sha(raw) == source_transition.RETAINED_PACKAGE_SHA,
                  'changed immutable predecessor package manifest')
    rows = json.loads(raw)['files']
    expected = {row['path'] for row in rows} | {'package-manifest.json'}
    closed_package(saved, expected)
    bodies = {'package-manifest.json': raw}
    for row in rows:
        name = row['path']
        body = (saved / name).read_bytes()
        adapters.verify(body, row, name)
        bodies[name] = body
    api, byte = source_transition.retained_public_api(saved, bodies)
    return saved, bodies, api


def public_reference_closure(repo, api, package_bytes, package):
    """Read only the original authenticated public preflight dependencies."""
    authority = json.loads(package_bytes['authority.json'])
    references = api.check_entries(repo, authority['repository_inputs'])
    historical = json.loads(references[api.U2 + '/package-inputs.json'])
    historical_bytes = api.check_entries(repo / api.U2, historical['files'])
    api.require(api.members(repo / api.U2) == sorted(
        [row['path'] for row in historical['files']] + ['package-inputs.json']),
        'missing or extra historical Unit2 member')
    references.update({api.U2 + '/' + name: body for name, body in historical_bytes.items()})
    lexical = api.load_lexical_provider(package_bytes, package)
    diagnostic = api.load_producer_diagnostic(package_bytes, package)
    frontend = api.load_frontend_v2(package_bytes, package)
    for rows in (lexical.PRODUCER_CLOSURE, diagnostic.DIAGNOSTIC_CLOSURE,
                 frontend.PRODUCER_CLOSURE):
        closure = api.check_entries(repo, list(rows))
        for name in closure:
            api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                        'changed input mode: ' + name)
        references.update(closure)
    lexical_sources = {row['path'] for row in lexical.PRODUCER_CLOSURE
                       if row['path'].endswith('.ox')}
    api.require(sorted(path.name for path in (repo / 'fixtures/typed-streaming-lexer').glob('*.ox'))
                == sorted(name.rsplit('/', 1)[1] for name in lexical_sources),
                'lexical producer source closure differs')
    return references


def preflight(repo, package):
    adapters.need(sys.flags.optimize == 0 and __debug__, 'optimized Python is not supported')
    repo, package = Path(repo), Path(package)
    saved, retained, api = capture_retained(package)
    raw = api.regular(package, 'package-manifest.json').read_bytes()
    rows = json.loads(raw)['files']
    api.require(api.members(package) == sorted([row['path'] for row in rows] + ['package-manifest.json']),
                'missing or extra current adapter member')
    closed_package(package, [row['path'] for row in rows] + ['package-manifest.json'])
    package_bytes = api.check_entries(package, rows)
    adapters.need(package_bytes['current-source.json'] == package_bytes[source_transition.CURRENT_NAME],
                  'active source manifest differs from named lexer successor')
    admitted = source_transition.admit(repo, package_bytes, SEAL, api)
    references = public_reference_closure(repo, api, retained, saved)
    restored = admitted['predecessor_inputs']
    overlap = set(restored) & set(references)
    adapters.need(all(restored[name] == references[name] for name in overlap),
                  'predecessor public closure overlap differs')
    # Retained admission executes unchanged, against only the exact old source.
    # Its manifest name, package bytes, fixture macros and entire inverse chain
    # are preserved. No old helper is ever handed new lexer bodies.
    with tempfile.TemporaryDirectory(prefix='oxid-lexer-retained-source-') as directory:
        target = Path(directory) / 'repository'
        api.materialize(target, {**restored, **references})
        historical = api.preflight(target, saved)
    adapters.need(historical['inputs'] == restored and len(historical['archived']) == 117,
                  'retained inverse chain did not preserve complete source identities')
    return {**historical,
            'current': admitted['current'], 'inputs': admitted['inputs'],
            'package_bytes': package_bytes, 'package_manifest': raw,
            'byte_storage_source': historical['current'], 'byte_storage_inputs': restored,
            'lexer_reservation_source': admitted['current'],
            'lexer_reservation_authority': admitted['authority'],
            'lexer_reservation_touched': admitted['transition_touched'],
            'lexer_reservation_seal': SEAL,
            'lexer_reservation_retained_package_manifest': retained['package-manifest.json'],
            'lexer_reservation_references': references}


def outer_receipt(captured):
    seal = captured['lexer_reservation_seal']
    return {'lexer_reservation_source_sha256': seal['source_manifest']['sha256'],
            'lexer_reservation_authority_sha256': seal['authority']['sha256'],
            'lexer_reservation_inverse_patch_sha256': seal['patch']['sha256'],
            'lexer_reservation_inverse_touched': captured['lexer_reservation_touched'],
            'lexer_reservation_compiler_head': seal['compiler_head'],
            'lexer_reservation_compiler_tree': seal['compiler_tree'],
            'lexer_reservation_adapter_head': seal['adapter_head'],
            'lexer_reservation_predecessor_source_sha256': source_transition.PREDECESSOR_SHA}


def install(runtime, package):
    """Transport the unchanged Unit2 execution/verification functions.

    Only current admission and receipt associations change; semantic/resource
    fixtures and the original verifier functions remain exact retained bodies.
    """
    old_archived = runtime.prepare_archived
    old_write_json = runtime.write_json
    runtime.PACKAGE = Path(package)
    runtime.__file__ = str(Path(package) / 'run.py')
    runtime.CURRENT_SOURCE_SHA = SEAL['source_manifest']['sha256']
    runtime.CURRENT_SOURCE_BYTES = SEAL['source_manifest']['bytes']
    last = {}

    def admitted(repo, package=runtime.PACKAGE):
        result = preflight(repo, package)
        last['captured'] = result
        return result

    def assert_unchanged(repo, captured, package=runtime.PACKAGE):
        fresh = admitted(repo, package)
        runtime.require(fresh == captured, 'input identity changed during operation')

    def archived(output, captured):
        result = old_archived(output, captured)
        result.update(outer_receipt(captured))
        return result

    def write_json(path, value):
        if isinstance(value, dict) and value.get('schema') == 'oxid-current-archive-binding-v1' and last:
            value = {**value, **outer_receipt(last['captured'])}
        old_write_json(path, value)

    runtime.preflight = admitted
    runtime.prepare_archived = archived
    runtime.assert_unchanged = assert_unchanged
    runtime.write_json = write_json
    return runtime
