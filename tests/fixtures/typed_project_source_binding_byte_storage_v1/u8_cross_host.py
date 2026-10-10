"""Exact reversible test-only u8 cross-host source binding; no execution oracle."""
import hashlib
import json
import re

SOURCE_SHA = '20f13e26a80cc55fd1e76ee76f8ec10988e9723d644dbc9a474fa7bcf6d4e0a4'
SOURCE_BYTES = 71510
AUTHORITY_SHA = 'c602ed97acae3437929069a2aa01244de9d54a9611167e0abefa289f6aa3c26c'
AUTHORITY_BYTES = 160226
PATCH_SHA = '8f62ca183f15292ef1cf043e5799a5e36d5a7c9b16fa95eb5a0b710e833e35a6'
PATCH_BYTES = 17278
SOURCE_HEAD = 'd204fbc684b81c6e0deac04007182ebe2bb67b00'
SOURCE_TREE = '8c00492b24d56b842858b72da9b4442f057ab4de'
BASE_HEAD = '5e4875d19961b4eba8e465c915ac676c54a9926e'
BASE_TREE = '4c687ed5ead4786e34cea154e3b263c00bd1fd1b'
U8_SOURCE_SHA = '35ee91911bb62c38c831aecb97c918bd14d9516013f5da3e62f445a1153e1cc4'
U8_SOURCE_BYTES = 71254
PATHS = ('src/frontend/declaration_index/u8_integration_tests.rs', 'src/frontend/project.rs')


def inverse(inputs, patch, api):
    """Recover all 363 immutable u8-source inputs before the original u8 inverse."""
    return api.apply_inverse_patch(inputs, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def identity(name, body, api):
    return {**api.entry(name, body), 'mode': '100644',
            'git_blob': hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest()}


def include_directives(inputs, api):
    """Enumerate exact include expressions; this is not a Rust macro evaluator.

    All 136 predecessor expressions must remain byte-identical; this successor
    admits no added include expression or fixture. Inherited macro-generated
    fixture closure is checked again by the unchanged predecessor stages after
    exact inversion.
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
    raw = package_bytes['u8-source.json']
    api.require(api.digest(raw) == U8_SOURCE_SHA and len(raw) == U8_SOURCE_BYTES,
                'unapproved u8 predecessor source manifest')
    predecessor = json.loads(raw)
    raw = package_bytes['u8-cross-host-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale u8 cross-host source authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-u8-cross-host-test-source-transition-v1'
                and authority['recipe'] == 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
                and authority['base_head'] == current['u8_cross_host_base_head'] == predecessor['reviewed_source_head'] == BASE_HEAD
                and authority['base_tree'] == current['u8_cross_host_base_tree'] == predecessor['source_only_tree'] == BASE_TREE
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['u8_source_sha256'] == current['u8_source_sha256'] == U8_SOURCE_SHA
                and authority['u8_source_bytes'] == U8_SOURCE_BYTES
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['compiler_additions'] == authority['fixture_additions'] == authority['removed_paths'] == []
                and (authority['current_source_members'], authority['u8_source_members'],
                     authority['compiler_source_members'], authority['compiler_bodies']) == (363, 363, 275, 278),
                'stale u8 cross-host source transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'u8_cross_host_base_head', 'u8_cross_host_base_tree', 'u8_source_sha256'}
    api.require({k: v for k, v in current.items() if k not in omit}
                == {k: v for k, v in predecessor.items() if k not in omit},
                'stale u8 cross-host source provenance')
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered u8 cross-host source inventory')
    before = {row['path']: row for row in predecessor['files']}
    api.require(len(inputs) == len(before) == 363 and set(inputs) == set(before),
                'unexpected u8 cross-host source membership')
    api.require([row['path'] for row in current['files'] if row != before[row['path']]] == list(PATHS),
                'unexpected u8 cross-host source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part + '/' + name for part in ('src', 'native') for name in api.members(repo / part)]
    api.require(len(expected) == 275 and sorted(actual) == expected,
                'missing or extra compiler source member')
    for name in inputs:
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
    api.require(authority['current_input_identities'] == [identity(name, body, api) for name, body in inputs.items()],
                'stale u8 cross-host complete input identities')
    fixtures = [identity(name, body, api) for name, body in inputs.items()
                if name.startswith(('tests/fixtures/', 'fixtures/'))]
    api.require(len(fixtures) == 78 and authority['compile_time_fixture_inputs'] == fixtures,
                'stale u8 cross-host compile-time fixture closure')
    includes = include_directives(inputs, api)
    api.require(len(includes) == 136 and authority['compile_time_include_directives'] == includes,
                'stale u8 cross-host compile-time include inventory')
    restored, touched = inverse(inputs, package_bytes['u8-cross-host-transition.patch'], api)
    api.check_bytes(restored, predecessor['files'])
    api.require(include_directives(restored, api) == includes,
                'changed u8 predecessor compile-time include')
    api.require([identity(name, restored[name], api) for name in restored
                 if name.startswith(('tests/fixtures/', 'fixtures/'))] == fixtures,
                'changed u8 predecessor compile-time fixture')
    api.require(authority['transition_inputs'] == [
        {'path': name, 'before': identity(name, restored[name], api),
         'after': identity(name, inputs[name], api)}
        for name in PATHS], 'stale u8 cross-host transition input identities')
    return current, inputs, authority, restored, touched
