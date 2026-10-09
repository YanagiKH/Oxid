"""Exact one-file cache-preservation successor; no language or execution oracle."""
import hashlib
import json

SOURCE_SHA = '481bc1f3f7b68530151d2b0d1e9bed76467b744f9087197320dfe286cbe98102'
SOURCE_BYTES = 67558
AUTHORITY_SHA = 'e9f62aaa5d6544c999dc857427f6c5c52abb8308502f980be822e2ebf55eae99'
AUTHORITY_BYTES = 94920
PATCH_SHA = 'ea517427d0fa75bf5fbc09aaa0c5ad504cb9e66b747895953716a1137e75f1c1'
PATCH_BYTES = 6541
SOURCE_HEAD = 'e8a4d357c18fa7f4ca0f722b8fcf123dbb0bc55b'
SOURCE_TREE = 'cf4dbd4fb02a219795b366b6be76d53e8e77ee20'
BASE_HEAD = '7f4e37707ad1c46476d2bf1e96575cb7d7e7724d'
BASE_TREE = '532822dab2c2b118b773ade53bb902fa1440bbbb'
PACKAGE_INTEGRITY_SOURCE_SHA = '4d114bbb9b375de743bc18508ebcb48301604f5b417ca0b44d788bf22189c99f'
PACKAGE_INTEGRITY_SOURCE_BYTES = 67336
PATHS = ('src/runtime/packages.rs',)


def inverse(inputs, patch, api):
    """Recover all 345 package-integrity-predecessor inputs before any older inverse."""
    return api.apply_inverse_patch(inputs, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def identity(name, body, api):
    return {**api.entry(name, body), 'mode': '100644',
            'git_blob': hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest()}


def admit(repo, package_bytes, api):
    raw = package_bytes['current-source.json']
    api.require(api.digest(raw) == SOURCE_SHA and len(raw) == SOURCE_BYTES,
                'unapproved current source manifest')
    current = json.loads(raw)
    raw = package_bytes['package-integrity-source.json']
    api.require(api.digest(raw) == PACKAGE_INTEGRITY_SOURCE_SHA and len(raw) == PACKAGE_INTEGRITY_SOURCE_BYTES,
                'unapproved package integrity predecessor source manifest')
    predecessor = json.loads(raw)
    raw = package_bytes['cache-preservation-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale cache preservation authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-cache-preservation-source-transition-v1'
                and authority['recipe'] == 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
                and authority['base_head'] == current['cache_preservation_base_head'] == BASE_HEAD
                and authority['base_tree'] == current['cache_preservation_base_tree'] == BASE_TREE
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['package_integrity_source_sha256'] == current['package_integrity_source_sha256'] == PACKAGE_INTEGRITY_SOURCE_SHA
                and authority['package_integrity_source_bytes'] == PACKAGE_INTEGRITY_SOURCE_BYTES
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['compiler_additions'] == [] and authority['removed_paths'] == []
                and (authority['current_source_members'], authority['compiler_source_members'],
                     authority['compiler_bodies']) == (345, 257, 260),
                'stale cache preservation transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'cache_preservation_base_head', 'cache_preservation_base_tree', 'package_integrity_source_sha256'}
    api.require({k: v for k, v in current.items() if k not in omit}
                == {k: v for k, v in predecessor.items() if k not in omit},
                'stale cache preservation provenance')
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered cache preservation source inventory')
    before = {row['path']: row for row in predecessor['files']}
    api.require(len(inputs) == len(before) == 345 and set(inputs) == set(before),
                'unexpected cache preservation source membership')
    api.require([row['path'] for row in current['files'] if row != before[row['path']]] == list(PATHS),
                'unexpected cache preservation source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part + '/' + name for part in ('src', 'native') for name in api.members(repo / part)]
    api.require(len(expected) == 257 and sorted(actual) == expected,
                'missing or extra compiler source member')
    for name in inputs:
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
    api.require(authority['current_input_identities'] == [identity(name, body, api) for name, body in inputs.items()],
                'stale cache preservation complete input identities')
    restored, touched = inverse(inputs, package_bytes['cache-preservation-transition.patch'], api)
    api.check_bytes(restored, predecessor['files'])
    api.require(authority['transition_inputs'] == [
        {'path': name, 'before': identity(name, restored[name], api), 'after': identity(name, inputs[name], api)}
        for name in PATHS], 'stale cache preservation transition input identities')
    return current, inputs, authority, restored, touched
