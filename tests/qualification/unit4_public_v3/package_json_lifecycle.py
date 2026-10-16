"""Named two-source successor for the unchanged public lifecycle observer.

Runs the genuine formatter admission in a fresh accepted-main view, transports
only the two ordinary source bodies, and checks the complete observer inverse.
No compiler is invoked and no execution qualification is granted.
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import threading
import types

from published_lifecycle import exact_patch, inserted_roster, sections, adapter, LEXER, Rejected, _read_regular

HERE = Path(__file__).absolute().parent
REPOSITORY = HERE.parents[2]
_ADMISSION = threading.Lock()
_POISONED = False
# Fixed by the pure consumer derivation after the dispatcher is finalized.
DISPATCHER = {'path': 'tests/fixtures/typed_project_source_binding/run.py', 'bytes': 6774, 'sha256': '581c9b8949b9725cffc4931504ae59c7c4e98312e629234d16ea6c0018e6ee9d'}


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def need(value, reason):
    if not value:
        raise Rejected(reason)


def rows(inputs, components=False):
    names = sorted(inputs, key=(lambda n: PurePosixPath(n).parts) if components else None)
    return [{'path': n, 'bytes': len(inputs[n]), 'sha256': sha(inputs[n])} for n in names]


def _source():
    # The public CI closure authenticates this adapter and dispatcher before
    # import. Local pin checking additionally prevents coherent package rehashes.
    path = REPOSITORY / DISPATCHER['path']
    raw, _ = _read_regular(path)
    need(len(raw) == DISPATCHER['bytes'] and sha(raw) == DISPATCHER['sha256'], 'fixed source dispatcher')
    module = types.ModuleType('package_json_public_source')
    module.__file__ = str(path)
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


def make_proof(old, current, dispatcher, seal):
    before = {r['path']: r for r in old['base_files']}
    after = {r['path']: r for r in current['files']}
    paths = ('src/main.rs', 'src/runtime/data.rs')
    need(list(before) == list(after) and len(after) == 376
         and [n for n in before if before[n] != after[n]] == list(paths), 'exact lifecycle two-source delta')
    need(all(n not in {r['path'] for r in old['inserted_roster']} for n in paths),
         'ordinary source in observer inserted_roster')
    observer = []
    for row in old['observer_files']:
        n = row['path']
        if n in paths:
            need(row == before[n], 'ordinary source has observer instrumentation')
        observer.append(after[n] if n in paths else row)
    return {'schema': 'oxid-package-json-public-lifecycle-v1',
            'predecessor_proof': {'path': 'published-lifecycle-v1.json',
                                 'sha256': sha((json.dumps(old, sort_keys=True, indent=2)+'\n').encode())},
            'source_provenance': current['package_json_source'],
            'source_dispatcher': dispatcher, 'source_seal': seal,
            'base_files': current['files'], 'observer_files': observer,
            'inserted_roster': old['inserted_roster'],
            'lexer_hook_correspondence': old['lexer_hook_correspondence'],
            'patch': old['patch'], 'source_only': True, 'execution_qualified': False}


def admit(root, patch):
    global _POISONED
    need(type(patch) is bytes and len(patch) == 7337 and sha(patch) ==
         "826468198bfd2575d0d01713bb803f19f31b732d482dbf336b7e37567f890824", "changed pinned body: observer-lexer-reservation-v2.patch")
    need(not _POISONED and _ADMISSION.acquire(blocking=False), 'overlapping or poisoned lifecycle admission')
    source = None
    try:
        source = _source()
        g = source._modules['generation']
        module = source._modules['package_current']
        view_helper = source._modules['predecessor']
        admitted = module.admit(REPOSITORY, source._SOURCE_SEAL)
        source._modules['source_transition'].complete_compiler_members(Path(root), admitted['inputs'])
        for name, raw in admitted['inputs'].items():
            need(g.regular(root, name) == raw, 'caller source differs from admitted checkpoint')
        with view_helper.view(REPOSITORY, admitted['predecessor_inputs'], source._SOURCE_SEAL['predecessor_view']) as view:
            old = view_helper.load(view, 'tests/qualification/unit4_public_v3/published_lifecycle.py',
                                   'package_json_original_lifecycle')
            observer, old_proof = old.admit(view, patch)
            need(old_proof['base_files'] == rows(admitted['predecessor_inputs']), 'genuine predecessor observer base')
            for name in g.PATHS:
                need(observer[name] == admitted['predecessor_inputs'][name], 'ordinary source instrumented')
                observer[name] = admitted['inputs'][name]
            proof = make_proof(old_proof, admitted['current'], DISPATCHER, source._SOURCE_SEAL)
            need(rows(observer, True) == proof['observer_files']
                 and old.exact_patch(admitted['inputs'], patch) == observer
                 and old.exact_patch(observer, patch, True) == admitted['inputs'],
                 'complete current observer forward/inverse')
        authority = source._modules['generation'].regular(HERE, 'package-json-lifecycle-v1.json')
        need(json.loads(authority) == proof, 'exact named lifecycle proof')
        for name, raw in admitted['inputs'].items():
            need(g.regular(root, name) == raw, 'caller source changed during admission')
        return observer, proof
    except BaseException:
        # A failed operation cannot be reused as a successful admission.
        _POISONED = True
        raise
    finally:
        _ADMISSION.release()
