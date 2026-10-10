"""Exact reversible bounded-u8 source admission; no language or resource oracle."""
import hashlib
import json
import re

SOURCE_SHA = '35ee91911bb62c38c831aecb97c918bd14d9516013f5da3e62f445a1153e1cc4'
SOURCE_BYTES = 71254
AUTHORITY_SHA = '407d3319bbc2dc404d65768cbe117d314dc486eb5b99169e0104c5536c029cda'
AUTHORITY_BYTES = 221718
PATCH_SHA = '133d3a599b7c95929d4b691b7390645d0fca44d8fe0fff40e1fbc79c11091d27'
PATCH_BYTES = 484704
SOURCE_HEAD = '5e4875d19961b4eba8e465c915ac676c54a9926e'
SOURCE_TREE = '4c687ed5ead4786e34cea154e3b263c00bd1fd1b'
BASE_HEAD = '98af42f3baa02f179c0437078ab1928e86f0c8f6'
BASE_TREE = '1d37d040150822d4358ec1c70c8e0f227f5eaf48'
CACHE_ADMISSION_SOURCE_SHA = '82cd3f0733ee6b457341e7607ff3883593138aa64b3763e217f0e53ef1662c67'
CACHE_ADMISSION_SOURCE_BYTES = 67820
PATHS = ('src/frontend/ast.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/enum_tests.rs', 'src/frontend/declaration_index/resource.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/declaration_index/source_owner.rs', 'src/frontend/declaration_index/tests.rs', 'src/frontend/declaration_index/u8_integration_tests.rs', 'src/frontend/declaration_index/u8_reservation.rs', 'src/frontend/driver.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/format/resource_tests.rs', 'src/frontend/hir.rs', 'src/frontend/oir/comparison_tests.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/execute_measurement.rs', 'src/frontend/oir/lower.rs', 'src/frontend/oir/lower_measurement.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_cost.rs', 'src/frontend/oir/native_emit_observation.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/native_private_emit.rs', 'src/frontend/oir/native_private_emit_tests.rs', 'src/frontend/oir/native_scalar_resource.rs', 'src/frontend/oir/native_scalar_resource_proof.md', 'src/frontend/oir/native_u8_tests.rs', 'src/frontend/oir/owned/array_native_resource_tests.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/array_reference_boundary_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/array_tests.rs', 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'src/frontend/oir/owned/enum_match_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/enum_query_allocation_tests.rs', 'src/frontend/oir/owned/enum_reference_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/mod.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_tests.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/array_type_controls.rs', 'src/frontend/oir/owned/source/array_types_tests.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/budget.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/candidate_native.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/hir_budget_tests.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/output_lower_tests.rs', 'src/frontend/oir/owned/source/program.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned/source/u8_resources.rs', 'src/frontend/oir/owned/source/u8_tests.rs', 'src/frontend/oir/owned/u8_tests.rs', 'src/frontend/oir/owned/verified.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/owned_types/u8_tests.rs', 'src/frontend/oir/project_execution_tests.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/conversion_seen.rs', 'src/frontend/oir/source/hir_import.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/emit_terminal.rs', 'src/frontend/oir/source/hir_import/candidate/run_entry_tests.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'src/frontend/oir/source/hir_import/emit_tests.rs', 'src/frontend/oir/source/hir_import/run_execution_tests.rs', 'src/frontend/oir/source/hir_import/tests.rs', 'src/frontend/oir/source/hir_import/u8_resource_successor.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/oir/source/u8_association_resource_proof.md', 'src/frontend/oir/u8_association_tests.rs', 'src/frontend/oir/u8_carrier_tests.rs', 'src/frontend/oir/u8_execute_tests.rs', 'src/frontend/oir/u8_tests.rs', 'src/frontend/oir/verify.rs', 'src/frontend/oir/verify_measurement.rs', 'src/frontend/owned_diagnostic.rs', 'src/frontend/parser.rs', 'src/frontend/parser/conversions.rs', 'src/frontend/parser/u8_syntax_tests.rs', 'src/frontend/project.rs', 'src/frontend/project/enum_carrier_tests.rs', 'src/frontend/typeck.rs', 'src/frontend/typeck_measurement.rs')
ADDITIONS = ('src/frontend/declaration_index/u8_integration_tests.rs', 'src/frontend/declaration_index/u8_reservation.rs', 'src/frontend/oir/native_scalar_resource.rs', 'src/frontend/oir/native_scalar_resource_proof.md', 'src/frontend/oir/native_u8_tests.rs', 'src/frontend/oir/owned/source/u8_resources.rs', 'src/frontend/oir/owned/source/u8_tests.rs', 'src/frontend/oir/owned/u8_tests.rs', 'src/frontend/oir/owned_types/u8_tests.rs', 'src/frontend/oir/source/conversion_seen.rs', 'src/frontend/oir/source/hir_import/u8_resource_successor.rs', 'src/frontend/oir/source/u8_association_resource_proof.md', 'src/frontend/oir/u8_association_tests.rs', 'src/frontend/oir/u8_carrier_tests.rs', 'src/frontend/oir/u8_execute_tests.rs', 'src/frontend/oir/u8_tests.rs', 'src/frontend/parser/conversions.rs', 'src/frontend/parser/u8_syntax_tests.rs')
INCLUDE_ADDITIONS = ({'expression': 'include_str!(concat!(\n        env!("CARGO_MANIFEST_DIR"),\n        "/tests/fixtures/checked_hir_import/rich-source.txt"\n    ))', 'ordinal': 0, 'source': 'src/frontend/oir/source/hir_import/u8_resource_successor.rs'}, {'expression': 'include_str!(concat!(\n        env!("CARGO_MANIFEST_DIR"),\n        "/tests/fixtures/checked_hir_import_v2/source-255.txt"\n    ))', 'ordinal': 1, 'source': 'src/frontend/oir/source/hir_import/u8_resource_successor.rs'})


def inverse(inputs, patch, api):
    """Restore the complete 345-member cache-admission view with exact contexts."""
    return api.apply_inverse_patch(inputs, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def identity(name, body, api):
    return {**api.entry(name, body), 'mode': '100644',
            'git_blob': hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest()}


def include_directives(inputs, api):
    """Enumerate exact include expressions; this is not a Rust macro evaluator.

    Predecessor expressions must all remain byte-identical. The only admitted
    additions are two literal references to already admitted fixture inputs.
    Inherited macro-generated fixture closure remains checked by its unchanged
    predecessor admission stages after exact inversion.
    """
    result = []
    for name, body in sorted(inputs.items()):
        if not name.endswith('.rs'):
            continue
        ordinal = 0
        for match in re.finditer(rb'include_(?:str|bytes)!\s*\(', body):
            at, depth, quoted, escaped = match.end(), 1, False, False
            while at < len(body) and depth:
                char = body[at]
                if quoted:
                    if escaped:
                        escaped = False
                    elif char == 92:
                        escaped = True
                    elif char == 34:
                        quoted = False
                elif char == 34:
                    quoted = True
                elif char == 40:
                    depth += 1
                elif char == 41:
                    depth -= 1
                at += 1
            api.require(not depth and not quoted, 'incomplete compile-time include: ' + name)
            result.append({'source': name, 'ordinal': ordinal,
                           'expression': body[match.start():at].decode('utf-8')})
            ordinal += 1
    return result


def admit(repo, package_bytes, api):
    raw = package_bytes['current-source.json']
    api.require(api.digest(raw) == SOURCE_SHA and len(raw) == SOURCE_BYTES,
                'unapproved current source manifest')
    current = json.loads(raw)
    raw = package_bytes['cache-admission-source.json']
    api.require(api.digest(raw) == CACHE_ADMISSION_SOURCE_SHA and len(raw) == CACHE_ADMISSION_SOURCE_BYTES,
                'unapproved cache admission predecessor source manifest')
    predecessor = json.loads(raw)
    raw = package_bytes['u8-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale u8 source authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-bounded-u8-source-transition-v1'
                and authority['recipe'] == 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
                and authority['base_head'] == current['u8_base_head'] == predecessor['reviewed_source_head'] == BASE_HEAD
                and authority['base_tree'] == current['u8_base_tree'] == predecessor['source_only_tree'] == BASE_TREE
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['cache_admission_source_sha256'] == current['cache_admission_source_sha256'] == CACHE_ADMISSION_SOURCE_SHA
                and authority['cache_admission_source_bytes'] == CACHE_ADMISSION_SOURCE_BYTES
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['compiler_additions'] == list(ADDITIONS)
                and authority['fixture_additions'] == [] and authority['removed_paths'] == []
                and (authority['current_source_members'], authority['cache_admission_source_members'],
                     authority['compiler_source_members'], authority['compiler_bodies']) == (363, 345, 275, 278),
                'stale u8 source transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'u8_base_head', 'u8_base_tree', 'cache_admission_source_sha256'}
    api.require({k: v for k, v in current.items() if k not in omit}
                == {k: v for k, v in predecessor.items() if k not in omit},
                'stale u8 source provenance')
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered u8 source inventory')
    before = {row['path']: row for row in predecessor['files']}
    api.require(len(inputs) == 363 and len(before) == 345
                and set(inputs) == set(before) | set(ADDITIONS),
                'unexpected u8 source membership')
    api.require([row['path'] for row in current['files'] if row != before.get(row['path'])] == list(PATHS),
                'unexpected u8 source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part + '/' + name for part in ('src', 'native') for name in api.members(repo / part)]
    api.require(len(expected) == 275 and sorted(actual) == expected,
                'missing or extra compiler source member')
    for name in inputs:
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
    api.require(authority['current_input_identities'] == [identity(name, body, api) for name, body in inputs.items()],
                'stale u8 complete input identities')
    fixtures = [identity(name, body, api) for name, body in inputs.items()
                if name.startswith(('tests/fixtures/', 'fixtures/'))]
    api.require(len(fixtures) == 78 and authority['compile_time_fixture_inputs'] == fixtures,
                'stale u8 compile-time fixture closure')
    includes = include_directives(inputs, api)
    api.require(len(includes) == 136 and authority['compile_time_include_directives'] == includes
                and authority['compile_time_include_additions'] == list(INCLUDE_ADDITIONS),
                'stale u8 compile-time include inventory')
    restored, touched = inverse(inputs, package_bytes['u8-transition.patch'], api)
    api.check_bytes(restored, predecessor['files'])
    old_includes = include_directives(restored, api)
    api.require(len(old_includes) == 134
                and [row for row in includes if row not in old_includes] == list(INCLUDE_ADDITIONS)
                and all(row in includes for row in old_includes),
                'changed predecessor compile-time include')
    api.require([identity(name, restored[name], api) for name in restored
                 if name.startswith(('tests/fixtures/', 'fixtures/'))] == fixtures,
                'changed predecessor compile-time fixture')
    api.require(authority['transition_inputs'] == [
        {'path': name, 'before': identity(name, restored[name], api) if name in restored else None,
         'after': identity(name, inputs[name], api)}
        for name in PATHS], 'stale u8 transition input identities')
    return current, inputs, authority, restored, touched
