"""Exact producer-diagnostic source successor; no semantic or execution oracle."""
import hashlib
import json
import re

# Generated once from the reviewed, immutable source checkpoint.
SOURCE_SHA = '35e7e43cb1ef5de8be0c1a78d9e5ac70b1a2caf2efe1e37ba05b7445916c2e29'
SOURCE_BYTES = 66021
AUTHORITY_SHA = 'fa52793cd0717dbf2bf44b819afc32258e257cb0522b0f503d8d7cefbfea0dfb'
AUTHORITY_BYTES = 135620
PATCH_SHA = 'c344823854314d8569d04055cd14031d2b6cd48cd95b8ea6a8301027af0ffe7e'
PATCH_BYTES = 37700
SOURCE_HEAD = 'c15465acb90e9f8bb18f5291a8931f5d5bbc6edb'
SOURCE_TREE = '0d843f9141f153d4c008ac860786c138165d7de3'
BASE_TREE = '534dbf42fafabc0a889b91ad351d5a11dd9ef8f4'
FRONTEND_V2_SHA = 'd294963af70126d6415e035f5ef1652c00953a22e6d26d586e0e1c1352cd6c8a'
FRONTEND_V2_BYTES = 63969
FRONTEND_V2_HEAD = '5f5a6639db9f779bb2453695f64ea980f7ea1790'
PATHS = ('src/frontend/driver.rs', 'src/frontend/hir_producer.rs', 'src/frontend/oir/source/hir_import.rs', 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'src/frontend/oir/source/hir_import/leaf.rs', 'src/frontend/oir/source/hir_import/producer_diagnostic.rs', 'src/frontend/oir/source/hir_import/producer_diagnostic/tests.rs', 'src/frontend/oir/source/hir_import/public_facade.rs')
ADDITIONS = ('src/frontend/oir/source/hir_import/producer_diagnostic.rs', 'src/frontend/oir/source/hir_import/producer_diagnostic/tests.rs')
FIXTURES = ({'path': 'tests/fixtures/producer_diagnostic/duplicate-source.txt', 'bytes': 31, 'sha256': 'b62fbb273ad1e0a884f4c1e5ddbc45886bc4e962e94ab03761ea9d0e4b40cef2'}, {'path': 'tests/fixtures/producer_diagnostic/duplicate.bin', 'bytes': 1575, 'sha256': 'd6a0707e9f0ed1351602a830166ea4a18823653457962f016913493b61fca38c'}, {'path': 'tests/fixtures/producer_diagnostic/end255-source.txt', 'bytes': 255, 'sha256': '0ea89521240db1e81600391e78d2888f3f13d8c3aa945a2eb9b40577f90ecc73'}, {'path': 'tests/fixtures/producer_diagnostic/end255.bin', 'bytes': 1575, 'sha256': '4f1b818fb3c6036c7e9d7df32dab4a7b176f0d425aeab2652a372ad3b13bbb38'}, {'path': 'tests/fixtures/producer_diagnostic/multiple-source.txt', 'bytes': 48, 'sha256': '25016a9cacca43132e7c5d29f4d2f11a9719d49427b8a5a24c209e5f50cb138c'}, {'path': 'tests/fixtures/producer_diagnostic/multiple.bin', 'bytes': 1575, 'sha256': 'b3fdb49bf2f5da837121d6cdb80ac4a541e71416c6096e73d8e419c84e0e5fd0'}, {'path': 'tests/fixtures/producer_diagnostic/unknown-type-source.txt', 'bytes': 35, 'sha256': 'a40af9465791100a701df35e87a9f23902eb4a2cfd14985aba05cdf1532c1ff0'}, {'path': 'tests/fixtures/producer_diagnostic/unknown-type.bin', 'bytes': 1575, 'sha256': 'a9445cbcfdf920b50e960c730aa33aa2d74379d93c7f21f04d3348ce6dd7a840'})
DIAGNOSTIC_CLOSURE = ({'path': 'tests/fixtures/producer_diagnostic/provenance.json', 'bytes': 2584, 'sha256': 'ddf68e898be3895717461388368122f00eb31c3b3b27fd8677e652b263438155'},)
PREDECESSOR_BINDINGS = ({'path': 'frontend-v2-authority.json', 'bytes': 137275, 'sha256': '597982c5f195a304bcc7bad4c7afba776db8fc40c7286e5ed3f8da6fa55c16f6'}, {'path': 'frontend-v2-transition.patch', 'bytes': 67003, 'sha256': 'b0fbd3b34584d76c7ff3bc074732b62c9c1b6b381ff1ca4cc88f99f93d964d32'}, {'path': 'frontend_v2.py', 'bytes': 8042, 'sha256': 'c5b514f24e0dc4be32117198f4604d8e1f36b29cecdd17962666ebf3d43a62d7'})
INCLUDE_PATHS = ('tests/fixtures/checked_hir_import/rich-source.txt', 'tests/fixtures/checked_hir_import/rich-success.bin', 'tests/fixtures/producer_diagnostic/duplicate-source.txt', 'tests/fixtures/producer_diagnostic/duplicate.bin', 'tests/fixtures/producer_diagnostic/end255-source.txt', 'tests/fixtures/producer_diagnostic/end255.bin', 'tests/fixtures/producer_diagnostic/multiple-source.txt', 'tests/fixtures/producer_diagnostic/multiple.bin', 'tests/fixtures/producer_diagnostic/unknown-type-source.txt', 'tests/fixtures/producer_diagnostic/unknown-type.bin')


def inverse(inputs, patch, api):
    """Remove only the eight exact fixture inputs, then restore the v2 source."""
    reduced = dict(inputs)
    for row in FIXTURES:
        name = row['path']
        api.require(name in reduced and api.entry(name, reduced[name]) == row,
                    'changed producer diagnostic fixture: ' + name)
        del reduced[name]
    return api.apply_inverse_patch(reduced, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def admit(repo, package_bytes, api):
    """Admit every current input before any unchanged historical inverse runs."""
    raw = package_bytes['current-source.json']
    api.require(api.digest(raw) == SOURCE_SHA and len(raw) == SOURCE_BYTES,
                'unapproved current source manifest')
    current = json.loads(raw)
    raw = package_bytes['frontend-v2-source.json']
    api.require(api.digest(raw) == FRONTEND_V2_SHA and len(raw) == FRONTEND_V2_BYTES,
                'unapproved frontend v2 source manifest')
    predecessor = json.loads(raw)
    for row in PREDECESSOR_BINDINGS:
        data = package_bytes[row['path']]
        api.require(api.entry(row['path'], data) == row,
                    'changed retained frontend v2 binding: ' + row['path'])
    raw = package_bytes['producer-diagnostic-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale producer diagnostic authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-producer-diagnostic-source-transition-v1'
                and authority['recipe'] == 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
                and authority['base_tree'] == current['producer_diagnostic_base_tree'] == predecessor['source_only_tree'] == BASE_TREE
                and authority['predecessor_source_head'] == predecessor['reviewed_source_head'] == FRONTEND_V2_HEAD
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['frontend_v2_source_sha256'] == current['frontend_v2_source_sha256'] == FRONTEND_V2_SHA
                and authority['frontend_v2_source_bytes'] == FRONTEND_V2_BYTES
                and authority['predecessor_bindings'] == list(PREDECESSOR_BINDINGS)
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['compiler_additions'] == list(ADDITIONS)
                and authority['fixture_inputs'] == list(FIXTURES)
                and authority['diagnostic_closure'] == list(DIAGNOSTIC_CLOSURE)
                and authority['removed_paths'] == [],
                'stale producer diagnostic transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'producer_diagnostic_base_tree', 'frontend_v2_source_sha256'}
    api.require({k: v for k, v in current.items() if k not in omit}
                == {k: v for k, v in predecessor.items() if k not in omit},
                'stale producer diagnostic provenance')
    api.check_entries(repo, list(DIAGNOSTIC_CLOSURE))
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered producer diagnostic source inventory')
    before = {row['path']: row for row in predecessor['files']}
    api.require(set(inputs) == set(before) | set(ADDITIONS) | {row['path'] for row in FIXTURES},
                'unexpected producer diagnostic source membership')
    api.require([row['path'] for row in current['files'] if row != before.get(row['path'])]
                == sorted(list(PATHS) + [row['path'] for row in FIXTURES]),
                'unexpected producer diagnostic source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part + '/' + name for part in ('src', 'native') for name in api.members(repo / part)]
    api.require(len(expected) == 252 and sorted(actual) == expected,
                'missing or extra compiler source member')
    fixture_references = re.findall(rb'"/(tests/fixtures/[^"\n]+)"',
        inputs['src/frontend/oir/source/hir_import/producer_diagnostic/tests.rs'])
    api.require(len(fixture_references) == 10
                and sorted(name.decode('ascii') for name in fixture_references) == list(INCLUDE_PATHS)
                and set(INCLUDE_PATHS) <= inputs.keys(),
                'producer diagnostic compile-time fixture closure differs')
    identities = []
    for name, data in inputs.items():
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
        identities.append({**api.entry(name, data), 'mode': '100644',
            'git_blob': hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()})
    api.require(authority['current_input_identities'] == identities
                and authority['current_input_git_modes'] == [{'path': name, 'mode': '100644'} for name in inputs]
                and authority['current_source_members'] == len(inputs) == 340
                and authority['frontend_v2_source_members'] == len(before) == 330
                and authority['compiler_source_members'] == len(expected) == 252
                and authority['compiler_bodies'] == len(expected) + 3 == 255,
                'stale producer diagnostic complete input identities')
    restored, touched = inverse(inputs, package_bytes['producer-diagnostic-transition.patch'], api)
    api.check_bytes(restored, predecessor['files'])
    changes = []
    for name in PATHS:
        row = {'path': name}
        for label, view in (('before', restored), ('after', inputs)):
            data = view.get(name)
            row[label] = None if data is None else {**api.entry(name, data), 'mode': '100644',
                'git_blob': hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()}
        changes.append(row)
    api.require(authority['transition_inputs'] == changes,
                'stale producer diagnostic transition input identities')
    return current, inputs, authority, restored, touched
