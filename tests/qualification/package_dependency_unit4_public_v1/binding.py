"""Source-only public successor binding. This module cannot admit execution.

The retained predecessor manifest identifies semantic authority only. No old
execution receipt is relabelled, copied into a new execution schema, or accepted
by this source binding. A later separately reviewed producer must authenticate
machine inputs before build evidence can be admitted.
"""
import hashlib
import json
import stat
import sys
import types
from pathlib import Path, PurePosixPath

sys.dont_write_bytecode = True
SHARED_MANIFEST_SHA256 = 'fa910b15e2008451da83f1cebc6452ff3617b34e1c5a04be85dfe558be8e93fa'
SHARED_NAME = 'package_dependency_unit4_current'
MACHINE_MANIFEST_SHA256 = '1661e0efc94e43410acf74d011f98cb52be76cac5f5904cb658e410e0bbec03a'
MACHINE_NAME = 'package_dependency_unit4_machine_v1'
CURRENT_SOURCE_SHA = 'f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
PREDECESSOR_SOURCE_SHA = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
SOURCE_CHECKPOINT = {'head': 'ce522e393646ce6478848cee31f307cb4b6ad5e8',
                     'tree': '8c3f4837e1fc89a6272ebee3552ce617ce584131'}
ROLE_MAPS = {
    'ordinary': ('33d82b779b82978d9744393d58148c6be6bb97817c62c73ab9aba077ba94661c', 376),
    'lifecycle': ('cfddb64018748b679f203fbd3a5c4e889df74d6703599cf5afb77a354c9e3ec6', 377),
}
KINDS = {role: 'package-dependency-unit4-public-' + role + '-v1' for role in ROLE_MAPS}


class Rejected(ValueError):
    """Malformed or substituted source/evidence."""


class NotReady(Rejected):
    """The source slice is not an execution authority."""


def need(value, message):
    if not value:
        raise Rejected(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    def nonfinite(_):
        raise Rejected('nonfinite JSON')
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=nonfinite)


def regular(path):
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=True) == path, 'canonical regular path required')
    need(not any(p.is_symlink() for p in (path, *path.parents)), 'symlink ancestry forbidden')
    need(stat.S_ISREG(path.lstat().st_mode), 'regular file required')
    return path.read_bytes()


def _shared_package():
    """Authenticate fixed installed shared package, never a caller helper path."""
    root = Path(__file__).absolute().parent.parent / SHARED_NAME
    manifest = regular(root / 'package-manifest.json')
    need(isinstance(SHARED_MANIFEST_SHA256, str) and sha(manifest) == SHARED_MANIFEST_SHA256,
         'shared package has no reviewed final manifest pin')
    manifest_data = decode(manifest)
    need(manifest_data['schema'] == 'oxid-package-dependency-unit4-current-package-v1', 'shared manifest schema')
    rows = manifest_data['files']
    need([r['path'] for r in rows] == ['context.py', 'preparation.py', 'prepare_worker.py', 'test_context.py', 'test_preparation.py'], 'exact shared package roster')
    names = []
    bodies = {}
    for row in rows:
        name = row['path']
        need(isinstance(name, str) and PurePosixPath(name).name == name and name not in ('.', '..'),
             'shared package must have a flat closed roster')
        need(name not in names and name != 'package-manifest.json', 'duplicate shared member')
        raw = regular(root / name)
        need(row == {'path': name, 'bytes': len(raw), 'sha256': sha(raw)}, 'shared helper body drift')
        need(not (root / name).stat().st_mode & 0o111, 'shared executable mode changed')
        names.append(name)
        bodies[name] = raw
    need(sorted(p.name for p in root.iterdir()) == sorted(names + ['package-manifest.json']),
         'shared package closed membership changed')
    return root, bodies


def preparation_api():
    """Load one authenticated producer owner, never a historical replacement."""
    root, bodies = _shared_package()
    name = '_oxid_package_dependency_unit4_preparation_v1'
    path = str(root / 'preparation.py')
    digest = sha(bodies['preparation.py'])
    if name in sys.modules:
        module = sys.modules[name]
        need(getattr(module, '__file__', None) == path and
             getattr(module, '__source_sha256__', None) == digest,
             'conflicting shared preparation module owner')
        return module
    module = types.ModuleType(name)
    module.__file__ = path
    module.__source_sha256__ = digest
    sys.modules[name] = module
    try:
        exec(compile(bodies['preparation.py'], path, 'exec'), module.__dict__)
    except BaseException:
        if sys.modules.get(name) is module:
            del sys.modules[name]
        raise
    return module


def machine_api():
    """Authenticate the distinct captured-machine owner; reports are not trust."""
    root = Path(__file__).absolute().parent.parent / MACHINE_NAME
    raw = regular(root / 'package-manifest.json')
    need(isinstance(MACHINE_MANIFEST_SHA256, str) and sha(raw) == MACHINE_MANIFEST_SHA256,
         'machine package has no reviewed final manifest pin')
    manifest = decode(raw)
    need(manifest['schema'] == 'oxid-package-dependency-unit4-machine-package-v1', 'machine package schema')
    need([r['path'] for r in manifest['files']] == ['machine.py', 'test_machine.py'], 'machine closed roster')
    bodies = {}
    for row in manifest['files']:
        body = regular(root / row['path'])
        need(row == {'path': row['path'], 'bytes': len(body), 'sha256': sha(body)}, 'machine helper body drift')
        need(not (root / row['path']).stat().st_mode & 0o111, 'machine helper executable mode changed')
        bodies[row['path']] = body
    need(sorted(p.name for p in root.iterdir()) == ['machine.py', 'package-manifest.json', 'test_machine.py'],
         'machine package extra/missing member')
    name = '_oxid_package_dependency_unit4_machine_v1'
    path, digest = str(root / 'machine.py'), sha(bodies['machine.py'])
    if name in sys.modules:
        module = sys.modules[name]
        need(getattr(module, '__file__', None) == path and
             getattr(module, '__source_sha256__', None) == digest, 'conflicting captured-machine owner')
        return module
    module = types.ModuleType(name)
    module.__file__, module.__source_sha256__ = path, digest
    sys.modules[name] = module
    try:
        exec(compile(bodies['machine.py'], path, 'exec'), module.__dict__)
    except BaseException:
        if sys.modules.get(name) is module:
            del sys.modules[name]
        raise
    return module


def context_api():
    """Reuse the canonical source/invocation owner after closure revalidation."""
    return preparation_api().context_api()


_PUBLIC_MODULES = {}


def _public_api(name):
    need(name in ('binding', 'build', 'runtime', 'hosted'), 'fixed public API name required')
    root = Path(__file__).absolute().parent
    manifest = decode(regular(root / 'package-manifest.json'))
    names = ['binding.py', 'build.py', 'hosted.py', 'runtime.py', 'test_binding.py', 'test_build.py', 'test_hosted.py', 'test_runtime.py']
    need(manifest['schema'] == 'oxid-package-dependency-unit4-public-package-v1' and
         [r['path'] for r in manifest['files']] == names, 'closed public runtime package')
    need(sorted(p.name for p in root.iterdir()) == sorted(names + ['package-manifest.json']), 'public package membership')
    bodies = {}
    for row in manifest['files']:
        raw = regular(root / row['path'])
        need(row == {'path': row['path'], 'bytes': len(raw), 'sha256': sha(raw)}, 'public helper drift')
        bodies[row['path']] = raw
    key = '_oxid_package_dependency_unit4_public_' + name + '_v1'
    path, digest = str(root / (name + '.py')), sha(bodies[name + '.py'])
    if key in sys.modules:
        module = sys.modules[key]
        need(getattr(module, '__file__', None) == path and getattr(module, '__source_sha256__', None) == digest,
             'conflicting canonical public API owner')
        return module
    module = types.ModuleType(key)
    module.__file__, module.__source_sha256__ = path, digest
    sys.modules[key] = module
    _PUBLIC_MODULES[key] = module
    try:
        exec(compile(bodies[name + '.py'], path, 'exec'), module.__dict__)
    except BaseException:
        if sys.modules.get(key) is module:
            del sys.modules[key]
        _PUBLIC_MODULES.pop(key, None)
        raise
    return module


def build_api():
    return _public_api('build')


def runtime_api():
    return _public_api('runtime')


def hosted_api():
    return _public_api('hosted')


def source_binding(context, role):
    """Re-admit real source context and return a defensive source-only carrier."""
    need(role in ROLE_MAPS, 'unsupported public role')
    context_api().verify_context(context)
    document = context.document()
    view = context.view(role)
    need(document['status'] == 'NotReady' and document['execution_qualified'] is False,
         'source context cannot assert execution readiness')
    need(document['source_checkpoint'] == SOURCE_CHECKPOINT, 'source checkpoint changed')
    need(document['current_source_manifest']['sha256'] == CURRENT_SOURCE_SHA,
         'new execution source must be successor, not retained predecessor')
    need(document['predecessor_source_manifest']['sha256'] == PREDECESSOR_SOURCE_SHA,
         'retained semantic source must remain genuine predecessor')
    digest, members = ROLE_MAPS[role]
    need(view['map_sha256'] == sha(canonical(view['files'])) == digest,
         'exact role map required')
    need(len(view['files']) == members, 'complete role membership required')
    need(view == document['views'][role], 'mixed source context/view')
    return decode(canonical({
        'schema': 'package-dependency-unit4-public-source-binding-v1',
        'kind': KINDS[role], 'role': role, 'status': 'NotReady',
        'execution_qualified': False, 'context_sha256': sha(canonical(document)),
        'checkout': document['checkout'], 'source_checkpoint': document['source_checkpoint'],
        'invocation': document['invocation'], 'source_manifest': document['current_source_manifest'],
        'view': view,
        'retained_semantic_basis': {
            'source_manifest': document['predecessor_source_manifest'],
            'meaning': 'unchanged predecessor semantic authority; not this candidate execution',
        },
    }))


def load_source_context(path, expected_invocation):
    """Loading requires the producer's actual invocation, not a receipt boolean."""
    return context_api().load_context(path, expected_invocation)


def read_build_binding(*args, **kwargs):
    raise NotReady('raw build envelopes cannot establish origin; use build.read_build_binding with its live producer result')


def runtime_admission(*args, **kwargs):
    raise NotReady('raw runtime claims cannot establish origin; use runtime_api().admit_runtime with live producer inputs')


def collect(*args, **kwargs):
    return runtime_admission(*args, **kwargs)
