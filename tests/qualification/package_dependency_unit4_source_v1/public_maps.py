"""Public package-source map derivation only; no preparation or execution API.

The old authorities and exact patch operations remain immutable. All four maps
are recomputed from authenticated actual current bodies, never relabelled.
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import stat
import sys
import types

PUBLIC = 'tests/qualification/unit4_public_v3/'
RUNNER = 'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py'
PINS = {
    PUBLIC + 'authority.py': '346485ecb8afed13bd784190f57f8e86a4d6d065437c2220e99928ade425aac7',
    PUBLIC + 'lexer_reservation_lifecycle.py': '155471428cc243fbb7845ec2dcd21fb982f51e7e8264ccf804f1df07338c91ec',
    PUBLIC + 'lexer-reservation-lifecycle-v2.json': 'f8e6e92eda7c0d07dea9a73301be9318f32c4a0eddb7126a581ad17e88d10e86',
    PUBLIC + 'observer-lexer-reservation-v2.patch': '826468198bfd2575d0d01713bb803f19f31b732d482dbf336b7e37567f890824',
    PUBLIC + 'observer-u8-v1.patch': 'c64b43cd0b630fcae46ac4cf73812ad0eb614696474e3af5d75d79b32f5047cf',
    PUBLIC + 'byte_storage_source_authority.py': '14629fd20362700e69c3aca0ce079748bf829533f1d2e5098f43b26ba3df0ba5',
    RUNNER: '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76',
}
CURRENT_SHA = 'f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
OLD_SHA = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
TRANSITION_SHA = '3b123cfa29136244e62038e885761e68858d2b9b472bc14231d2e7a60a13586d'
CHECKPOINT = {'head': 'ce522e393646ce6478848cee31f307cb4b6ad5e8', 'tree': '8c3f4837e1fc89a6272ebee3552ce617ce584131'}
OLD_MAP_SHA = '676b96270257ad5f9f793c5ab8c7872434906fef8353feabb6052e2a2cbd2e87'
OLD_OBSERVER_SHA = 'a6bf8f6a6c89cbe765d33c4351438749cc1e5e408a9ab002d8ad026cbc800790'
AUTHORITY_NAME = 'public-source-successor-authority.json'


class Rejected(ValueError):
    pass


class NotReady(RuntimeError):
    pass


def need(ok, message):
    if not ok:
        raise Rejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def serialize(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()


def row(name, raw):
    return {'path': name, 'bytes': len(raw), 'sha256': sha(raw)}


def rows(inputs, components=False):
    key = (lambda name: PurePosixPath(name).parts) if components else None
    return [row(name, inputs[name]) for name in sorted(inputs, key=key)]


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs)


def read_retained(repo):
    repo = Path(repo)
    need(repo.is_absolute() and repo == repo.resolve(), 'canonical repository required')
    result = {}
    for name in PINS:
        path = repo / name
        need(not any(p.is_symlink() for p in (path, *path.parents)), 'symlink retained input')
        need(path.is_file() and stat.S_ISREG(path.stat().st_mode)
             and not path.stat().st_mode & 0o111, 'nonregular retained input')
        result[name] = path.read_bytes()
    return result


def load(raw, name):
    module = types.ModuleType('package_public_retained_' + Path(name).stem)
    module.__file__ = '/authenticated-retained/' + name
    exec(compile(raw, module.__file__, 'exec'), module.__dict__)
    return module


def derive_bytes(context, retained):
    """Pure body/map proof. Input dicts and retained modules are never patched."""
    need(sys.flags.optimize == 0 and __debug__, 'normal Python required')
    need(set(retained) == set(PINS), 'retained closure differs')
    for name, digest in PINS.items():
        need(type(retained[name]) is bytes and sha(retained[name]) == digest, 'retained identity: ' + name)
    current_raw, old_raw = context['current_manifest'], context['predecessor_manifest']
    need(sha(current_raw) == CURRENT_SHA and sha(old_raw) == OLD_SHA, 'source manifest identity')
    current, old = decode(current_raw), decode(old_raw)
    need(context['current_manifest_binding'] == row('tests/fixtures/package_dependency_source_v1/package-dependency-source-v1.json', current_raw), 'current manifest binding')
    need(context['predecessor_manifest_binding'] == row('tests/fixtures/typed_project_source_binding/current-source.json', old_raw), 'predecessor manifest binding')
    need(context['status'] == 'NotReady' and context['execution_qualified'] is False, 'source-only context required')
    need(context['source_checkpoint'] == CHECKPOINT, 'source checkpoint association')
    need(current['reviewed_source_head'] == CHECKPOINT['head'] and current['source_only_tree'] == CHECKPOINT['tree'], 'manifest checkpoint association')
    transition = context['transition_patch']
    need(len(transition) == 752 and sha(transition) == TRANSITION_SHA, 'main inverse identity')
    new376 = dict(context['current_inputs'])
    need(len(new376) == 376 and all(type(n) is str and type(b) is bytes for n,b in new376.items()), 'current body shape')
    need(rows(new376) == current['files'], 'complete current376 body map')
    inverse = load(retained[RUNNER], RUNNER)
    old376, touched = inverse.apply_inverse_patch(new376, transition, TRANSITION_SHA, 752, ('src/main.rs',))
    need(touched == ['src/main.rs'] and old376 == context['predecessor_inputs'], 'complete main inverse')
    need(rows(old376) == old['files'] and len(old376) == 376, 'complete old376 body map')
    need([n for n in sorted(old376) if old376[n] != new376[n]] == ['src/main.rs'], 'ordinary delta scope')
    for wrong in (old376, dict(old376)):
        try:
            inverse.apply_inverse_patch(wrong, transition, TRANSITION_SHA, 752, ('src/main.rs',))
        except inverse.BindingError:
            pass
        else:
            raise Rejected('wrong-stage/double main inverse accepted')
    api = load(retained[PUBLIC + 'lexer_reservation_lifecycle.py'], PUBLIC + 'lexer_reservation_lifecycle.py')
    proof = decode(retained[PUBLIC + 'lexer-reservation-lifecycle-v2.json'])
    patch = retained[PUBLIC + 'observer-lexer-reservation-v2.patch']
    need(proof['base_files'] == rows(old376), 'old ordinary authority')
    need(proof['patch'] == row('observer-lexer-reservation-v2.patch', patch), 'lifecycle patch binding')
    need(proof['predecessor_authority'] == row('byte_storage_source_authority.py', retained[PUBLIC + 'byte_storage_source_authority.py']), 'lifecycle predecessor binding')
    need(proof['predecessor_patch'] == row('observer-u8-v1.patch', retained[PUBLIC + 'observer-u8-v1.patch']), 'lifecycle predecessor patch binding')
    roster = api.inserted_roster(patch)
    need(roster == proof['inserted_roster'], 'lifecycle inserted events')
    old377 = api.exact_patch(old376, patch)
    new377 = api.exact_patch(new376, patch)
    need(len(old377) == len(new377) == 377 and rows(old377, True) == proof['observer_files'], 'complete old377 authority')
    need(set(old377) == set(new377) and [n for n in sorted(old377) if old377[n] != new377[n]] == ['src/main.rs'], 'observer delta scope')
    need(api.exact_patch(old377, patch, True) == old376, 'complete old lifecycle inverse')
    need(api.exact_patch(new377, patch, True) == new376, 'complete new lifecycle inverse')
    restored377, touched377 = inverse.apply_inverse_patch(new377, transition, TRANSITION_SHA, 752, ('src/main.rs',))
    need(restored377 == old377 and touched377 == ['src/main.rs'], 'main inverse/lifecycle composition')
    need(api.exact_patch(restored377, patch, True) == old376, 'complete new377 reverse composition')
    maps = {'old_ordinary376': rows(old376), 'new_ordinary376': rows(new376),
            'old_lifecycle377': rows(old377, True), 'new_lifecycle377': rows(new377, True)}
    hashes = {name: sha(canonical(records)) for name, records in maps.items()}
    need(hashes['old_ordinary376'] == OLD_MAP_SHA == proof['complete_inverse_sha256'], 'old complete inverse digest')
    need(hashes['old_lifecycle377'] == OLD_OBSERVER_SHA, 'old complete observer digest')
    return {'schema': 'oxid-package-unit4-public-source-successor-v1', 'status': 'NotReady',
            'execution_qualified': False, 'physical_preparation': 'not-run', 'compiler_invocations': 0,
            'source_checkpoint': CHECKPOINT, 'current_source': context['current_manifest_binding'],
            'predecessor_source': context['predecessor_manifest_binding'],
            'transition_patch': row('package-dependency-transition-v1.patch', transition),
            'retained_identities': [row(n, retained[n]) for n in sorted(retained)],
            'maps': maps, 'canonical_map_sha256': hashes,
            'map_order': {'ordinary': 'lexicographic path', 'lifecycle': 'POSIX path components'},
            'changed_paths': ['src/main.rs'], 'inserted_roster': roster,
            'lexer_hook_correspondence': proof['lexer_hook_correspondence'],
            'composition': {'main_inverse_new376': hashes['old_ordinary376'],
                            'main_inverse_new377': hashes['old_lifecycle377'],
                            'lifecycle_inverse_old377': hashes['old_ordinary376'],
                            'lifecycle_inverse_new377': hashes['new_ordinary376'],
                            'both_inverses_new377': hashes['old_ordinary376']}}


def derive(context, retained):
    return derive_bytes(context, retained)


def validate_authority(raw, context, retained):
    expected = derive(context, retained)
    need(raw == serialize(expected), 'successor authority is not exact deterministic derivation')
    return expected


def execution_context(*args, **kwargs):
    raise NotReady('Public map derivation does not qualify or activate execution')
