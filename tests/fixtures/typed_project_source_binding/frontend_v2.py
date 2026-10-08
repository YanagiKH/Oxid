"""Closed, reversible source-capacity successor; never a semantic oracle."""
import hashlib
import json

# Exact reviewed values below are generated once at the frozen source checkpoint.
SOURCE_SHA = 'e9f92670a63edd0336bada42e03702f7e446706c9fe56fa80913c7428025da2c'
SOURCE_BYTES = 63969
AUTHORITY_SHA = '77be4c08d5db92c2fbfdfb9451c324252a6b9b350fdd5fc63238b58bfc938232'
AUTHORITY_BYTES = 136550
PATCH_SHA = '78438fb9a468ecfa5a31c304793080c4f2a4618493b3e8abbda8573609fa5cb4'
PATCH_BYTES = 61892
SOURCE_HEAD = '36b775e5f577bd2355f8b21f7af9c427d76e1f4d'
SOURCE_TREE = '12cb3d6d42aa5445e3f8d94ce564e942da910fa0'
BASE_TREE = '6a67309be29b6841f4776ed392853b36d7263e02'
PRODUCER_SHA = '17d7473695424f8ccf570b3cb4129b08d0291650eab45356c66f8b6864a96680'
PRODUCER_BYTES = 63220
PATHS = ('src/frontend/hir_producer.rs', 'src/frontend/hir_producer/bundle.rs', 'src/frontend/hir_producer/supervisor.rs', 'src/frontend/hir_protocol.rs', 'src/frontend/mod.rs', 'src/frontend/oir/native_emit_cost.rs', 'src/frontend/oir/source/hir_import.rs', 'src/frontend/oir/source/hir_import/ast_compare.rs', 'src/frontend/oir/source/hir_import/ast_compare/tests.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/emit_terminal.rs', 'src/frontend/oir/source/hir_import/candidate/verify_terminal.rs', 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'src/frontend/oir/source/hir_import/leaf.rs', 'src/frontend/oir/source/hir_import/public_facade.rs', 'src/frontend/oir/source/hir_import/tests.rs')
FIXTURES = ({'path': 'tests/fixtures/checked_hir_import_v2/source-255.txt', 'bytes': 255, 'sha256': 'f616a9ac1025210fa1d282b06aadb8c3088b653154c64afe01d64c8d9c838ee3'}, {'path': 'tests/fixtures/checked_hir_import_v2/success-255.bin', 'bytes': 2607, 'sha256': '09a5ec7761d7da0aed5f8c94d8e374dc577f4cd649aa964c43efa79d58f10f05'})
PRODUCER_CLOSURE = (
    {'path':'fixtures/typed-frontend-v2/sources.json','bytes':5461,
     'sha256':'08629aee51a25e05fd7bf4b121ff8a7d882683687425f4e5469f8b508618d608'},
    {'path':'scripts/build_hir_producers_v2.py','bytes':3986,
     'sha256':'32b382b037065919f2ce94e2de4393da49cb8a6ef1a6ffd439fae8d0f2bbeeb8'},
)
BANK_SUCCESSOR = {
    'version': 'frontend-v2-shared-bank-v1',
    'v1_old_fixed_bytes': 138865, 'shared_fixed_bytes': 139939,
    'v1_old_retained_bytes': 153190, 'v1_retained_bytes': 154264,
    'v1_work': 10069187, 'v1_llvm_bytes': 14325,
    'v2_fixture_fixed_bytes': 139939, 'v2_fixture_retained_bytes': 141475,
    'v2_fixture_work': 1004093, 'v2_fixture_llvm_bytes': 690,
    'scope': 'Shared import-leaf named storage successor; v1 byte admission tightens by 1074; no global limit increase.',
}


def inverse(inputs, patch, api):
    reduced = dict(inputs)
    for row in FIXTURES:
        name = row['path']
        api.require(name in reduced and api.entry(name, reduced[name]) == row,
                    'changed frontend v2 fixture: ' + name)
        del reduced[name]
    return api.apply_inverse_patch(reduced, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def admit(repo, package_bytes, api):
    api.require(api.digest(package_bytes['current-source.json']) == SOURCE_SHA
                and len(package_bytes['current-source.json']) == SOURCE_BYTES,
                'unapproved current source manifest')
    raw = package_bytes['hir-producer-source.json']
    api.require(api.digest(raw) == PRODUCER_SHA and len(raw) == PRODUCER_BYTES,
                'unapproved HIR producer source manifest')
    predecessor = json.loads(raw)
    current = json.loads(package_bytes['current-source.json'])
    raw = package_bytes['frontend-v2-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale frontend v2 authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-frontend-v2-source-transition-v1'
                and authority['base_tree'] == current['frontend_v2_base_tree'] == predecessor['source_only_tree'] == BASE_TREE
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['hir_producer_source_sha256'] == current['hir_producer_source_sha256'] == PRODUCER_SHA
                and authority['hir_producer_source_bytes'] == PRODUCER_BYTES
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['fixture_inputs'] == list(FIXTURES)
                and authority['compiler_additions'] == ['src/frontend/hir_protocol.rs']
                and authority['removed_paths'] == []
                and authority['resource_successor'] == BANK_SUCCESSOR
                and authority['producer_closure'] == list(PRODUCER_CLOSURE),
                'stale frontend v2 transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'frontend_v2_base_tree', 'hir_producer_source_sha256'}
    api.require({k:v for k,v in current.items() if k not in omit}
                == {k:v for k,v in predecessor.items() if k not in omit},
                'stale frontend v2 provenance')
    api.check_entries(repo, list(PRODUCER_CLOSURE))
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered frontend v2 source inventory')
    before = {row['path']:row for row in predecessor['files']}
    api.require(set(inputs) == set(before) | {'src/frontend/hir_protocol.rs'} | {row['path'] for row in FIXTURES},
                'unexpected frontend v2 source membership')
    api.require([row['path'] for row in current['files'] if row != before.get(row['path'])]
                == sorted(list(PATHS) + [row['path'] for row in FIXTURES]),
                'unexpected frontend v2 source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part+'/'+name for part in ('src','native') for name in api.members(repo/part)]
    api.require(len(expected) == 250 and sorted(actual) == expected,
                'missing or extra compiler source member')
    identities = []
    for name, data in inputs.items():
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0, 'changed input mode: ' + name)
        identities.append({**api.entry(name,data), 'mode':'100644',
            'git_blob':hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()})
    api.require(authority['current_input_identities'] == identities
                and authority['current_input_git_modes'] == [{'path':name,'mode':'100644'} for name in inputs]
                and authority['current_source_members'] == len(inputs) == 330
                and authority['hir_producer_source_members'] == len(before) == 327
                and authority['compiler_source_members'] == len(expected) == 250,
                'stale frontend v2 complete input identities')
    restored, touched = inverse(inputs, package_bytes['frontend-v2-transition.patch'], api)
    api.check_bytes(restored, predecessor['files'])
    changes=[]
    for name in PATHS:
        row={'path':name}
        for label, view in (('before',restored),('after',inputs)):
            data=view.get(name)
            row[label]=None if data is None else {**api.entry(name,data),'mode':'100644',
                'git_blob':hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()}
        changes.append(row)
    api.require(authority['transition_inputs'] == changes, 'stale frontend v2 transition input identities')
    return current, inputs, authority, restored, touched
