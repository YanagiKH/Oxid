"""Exact reversible RFC0031 source admission; no semantic/resource oracle.

Every retained authority is immutable. This successor admits one exact source
closure and reconstructs all cross-host scalar-u8 inputs before older admission.
"""
import hashlib
import json

# SOURCE_CONSTANTS: replaced only from the final reviewed compiler checkpoint.
SOURCE_SHA = '9e9e65c9ba3b034074ca22cb0967d6820ff5b5f8907613fcb36720b97519b0f8'
SOURCE_BYTES = 74192
AUTHORITY_SHA = '72b59997311d9198a0357376a938b9332fd7ef6808ec3c9949a5f2ffaf7c4518'
AUTHORITY_BYTES = 182657
PATCH_SHA = '8ed7bf4876066a9cce422ff53e995c4a8c6d0fc7a3a81baedaa5de32c047bcd9'
PATCH_BYTES = 274861
SOURCE_HEAD = 'b3371e6a383b526d25409f2d4b308b4993739466'
SOURCE_TREE = 'c168a3d6d6c3e3ba3d6b71b981319cbed9497525'
BASE_HEAD = 'd204fbc684b81c6e0deac04007182ebe2bb67b00'
BASE_TREE = '8c00492b24d56b842858b72da9b4442f057ab4de'
PREDECESSOR_SHA = '20f13e26a80cc55fd1e76ee76f8ec10988e9723d644dbc9a474fa7bcf6d4e0a4'
PREDECESSOR_BYTES = 71510
PATHS = ('src/frontend/ast.rs',
 'src/frontend/declaration_index.rs',
 'src/frontend/format/ast_tests.rs',
 'src/frontend/oir/owned/byte_storage_codec_tests.rs',
 'src/frontend/oir/owned/byte_storage_native_tests.rs',
 'src/frontend/oir/owned/byte_storage_reference_tests.rs',
 'src/frontend/oir/owned/execute.rs',
 'src/frontend/oir/owned/mod.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/source/association.rs',
 'src/frontend/oir/owned/source/byte_storage_association_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_authority_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_fuel_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_native_fixture.rs',
 'src/frontend/oir/owned/source/byte_storage_raw_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_resources.rs',
 'src/frontend/oir/owned/source/byte_storage_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_trust_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_type_tests.rs',
 'src/frontend/oir/owned/source/hir_budget.rs',
 'src/frontend/oir/owned/source/hir_budget_tests.rs',
 'src/frontend/oir/owned/source/mod.rs',
 'src/frontend/oir/owned/source/program.rs',
 'src/frontend/oir/owned/source/resolve.rs',
 'src/frontend/oir/owned/source/typeck.rs',
 'src/frontend/oir/owned/source/u8_tests.rs',
 'src/frontend/oir/owned_types.rs',
 'src/frontend/oir/owned_types/byte_storage_tests.rs',
 'src/frontend/oir/owned_types/u8_tests.rs',
 'src/frontend/oir/u8_association_tests.rs',
 'src/frontend/parser/array_syntax_tests.rs',
 'src/frontend/parser/arrays.rs',
 'src/frontend/parser/u8_syntax_tests.rs')
ADDITIONS = ('src/frontend/oir/owned/byte_storage_codec_tests.rs',
 'src/frontend/oir/owned/byte_storage_native_tests.rs',
 'src/frontend/oir/owned/byte_storage_reference_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_association_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_authority_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_fuel_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_native_fixture.rs',
 'src/frontend/oir/owned/source/byte_storage_raw_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_resources.rs',
 'src/frontend/oir/owned/source/byte_storage_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_trust_tests.rs',
 'src/frontend/oir/owned/source/byte_storage_type_tests.rs',
 'src/frontend/oir/owned_types/byte_storage_tests.rs')
FIXTURE_ADDITIONS = ()
INCLUDE_ADDITIONS = ()
CURRENT_MEMBERS = 376
COMPILER_MEMBERS = 288
FIXTURE_MEMBERS = 78
INCLUDE_MEMBERS = 136
# END_SOURCE_CONSTANTS


def inverse(inputs, patch, api):
    """Recover every immutable cross-host member with exact patch contexts."""
    return api.apply_inverse_patch(inputs, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def identity(name, body, api):
    return {**api.entry(name, body), 'mode': '100644',
            'git_blob': hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest()}


def admit(repo, package_bytes, api):
    raw = package_bytes['current-source.json']
    api.require(api.digest(raw) == SOURCE_SHA and len(raw) == SOURCE_BYTES,
                'unapproved current source manifest')
    current = json.loads(raw)
    raw = package_bytes['u8-cross-host-source.json']
    api.require(api.digest(raw) == PREDECESSOR_SHA and len(raw) == PREDECESSOR_BYTES,
                'unapproved cross-host predecessor source manifest')
    predecessor = json.loads(raw)
    raw = package_bytes['byte-storage-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale byte storage source authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-bounded-byte-storage-source-transition-v1'
                and authority['recipe'] == 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
                and authority['base_head'] == current['byte_storage_base_head'] == predecessor['reviewed_source_head'] == BASE_HEAD
                and authority['base_tree'] == current['byte_storage_base_tree'] == predecessor['source_only_tree'] == BASE_TREE
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['predecessor_source_sha256'] == current['u8_cross_host_source_sha256'] == PREDECESSOR_SHA
                and authority['predecessor_source_bytes'] == PREDECESSOR_BYTES
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['compiler_additions'] == list(ADDITIONS)
                and authority['fixture_additions'] == list(FIXTURE_ADDITIONS)
                and authority['removed_paths'] == []
                and (authority['current_source_members'], authority['predecessor_source_members'],
                     authority['compiler_source_members'], authority['compiler_bodies'])
                    == (CURRENT_MEMBERS, 363, COMPILER_MEMBERS, COMPILER_MEMBERS + 3),
                'stale byte storage source transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'byte_storage_base_head', 'byte_storage_base_tree', 'u8_cross_host_source_sha256'}
    api.require({k: v for k, v in current.items() if k not in omit}
                == {k: v for k, v in predecessor.items() if k not in omit},
                'stale byte storage source provenance')
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered byte storage source inventory')
    before = {row['path']: row for row in predecessor['files']}
    api.require(len(inputs) == CURRENT_MEMBERS and len(before) == 363
                and set(inputs) == set(before) | set(ADDITIONS) | set(FIXTURE_ADDITIONS),
                'unexpected byte storage source membership')
    api.require([row['path'] for row in current['files'] if row != before.get(row['path'])] == list(PATHS),
                'unexpected byte storage source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part + '/' + name for part in ('src', 'native') for name in api.members(repo / part)]
    api.require(len(expected) == COMPILER_MEMBERS and sorted(actual) == expected,
                'missing or extra compiler source member')
    for name in inputs:
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
    api.require(authority['current_input_identities'] == [identity(name, body, api) for name, body in inputs.items()],
                'stale byte storage complete input identities')
    fixtures = [identity(name, body, api) for name, body in inputs.items()
                if name.startswith(('tests/fixtures/', 'fixtures/'))]
    api.require(len(fixtures) == FIXTURE_MEMBERS and authority['compile_time_fixture_inputs'] == fixtures,
                'stale byte storage compile-time fixture closure')
    scanner = api.load_u8_cross_host(package_bytes)
    includes = scanner.include_directives(inputs, api)
    api.require(len(includes) == INCLUDE_MEMBERS and authority['compile_time_include_directives'] == includes
                and authority['compile_time_include_additions'] == list(INCLUDE_ADDITIONS),
                'stale byte storage compile-time include inventory')
    restored, touched = inverse(inputs, package_bytes['byte-storage-transition.patch'], api)
    api.check_bytes(restored, predecessor['files'])
    old_includes = scanner.include_directives(restored, api)
    api.require(len(old_includes) == 136
                and [row for row in includes if row not in old_includes] == list(INCLUDE_ADDITIONS)
                and all(row in includes for row in old_includes),
                'changed predecessor compile-time include')
    api.require([identity(name, restored[name], api) for name in restored
                 if name.startswith(('tests/fixtures/', 'fixtures/'))]
                == [row for row in fixtures if row['path'] not in FIXTURE_ADDITIONS],
                'changed predecessor compile-time fixture')
    api.require(authority['transition_inputs'] == [
        {'path': name, 'before': identity(name, restored[name], api) if name in restored else None,
         'after': identity(name, inputs[name], api)}
        for name in PATHS], 'stale byte storage transition input identities')
    return current, inputs, authority, restored, touched
