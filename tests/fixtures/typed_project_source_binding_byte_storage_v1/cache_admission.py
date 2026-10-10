"""Exact one-file cache-admission successor; no language or execution oracle."""
import hashlib
import json

SOURCE_SHA = '82cd3f0733ee6b457341e7607ff3883593138aa64b3763e217f0e53ef1662c67'
SOURCE_BYTES = 67820
AUTHORITY_SHA = '07ff6c8427fdc108a2d995a6d94a5788df77e0f85d12892446956eb9d6a43440'
AUTHORITY_BYTES = 94920
PATCH_SHA = '302fab79c45fb34a41a3a641e44fe0880bbfb765554dfe306bcc07cdf67cc01d'
PATCH_BYTES = 10191
SOURCE_HEAD = '98af42f3baa02f179c0437078ab1928e86f0c8f6'
SOURCE_TREE = '1d37d040150822d4358ec1c70c8e0f227f5eaf48'
BASE_HEAD = '20f8e494f872420e6eb53b518b92783c9a1c3539'
BASE_TREE = '26b2d2ed5eaf78b37e9933296f011f6536c8d1dd'
CACHE_PRESERVATION_SOURCE_SHA = '481bc1f3f7b68530151d2b0d1e9bed76467b744f9087197320dfe286cbe98102'
CACHE_PRESERVATION_SOURCE_BYTES = 67558
PATHS = ('src/runtime/packages.rs',)


def inverse(inputs, patch, api):
    """Recover all 345 cache-preservation-predecessor inputs before any older inverse."""
    return api.apply_inverse_patch(inputs, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def identity(name, body, api):
    return {**api.entry(name, body), 'mode': '100644',
            'git_blob': hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest()}


def admit(repo, package_bytes, api):
    raw = package_bytes['current-source.json']
    api.require(api.digest(raw) == SOURCE_SHA and len(raw) == SOURCE_BYTES,
                'unapproved current source manifest')
    current = json.loads(raw)
    raw = package_bytes['cache-preservation-source.json']
    api.require(api.digest(raw) == CACHE_PRESERVATION_SOURCE_SHA and len(raw) == CACHE_PRESERVATION_SOURCE_BYTES,
                'unapproved cache preservation predecessor source manifest')
    predecessor = json.loads(raw)
    raw = package_bytes['cache-admission-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale cache admission authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-cache-admission-source-transition-v1'
                and authority['recipe'] == 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
                and authority['base_head'] == current['cache_admission_base_head'] == BASE_HEAD
                and authority['base_tree'] == current['cache_admission_base_tree'] == BASE_TREE
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['cache_preservation_source_sha256'] == current['cache_preservation_source_sha256'] == CACHE_PRESERVATION_SOURCE_SHA
                and authority['cache_preservation_source_bytes'] == CACHE_PRESERVATION_SOURCE_BYTES
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['compiler_additions'] == [] and authority['removed_paths'] == []
                and (authority['current_source_members'], authority['compiler_source_members'],
                     authority['compiler_bodies']) == (345, 257, 260),
                'stale cache admission transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'cache_admission_base_head', 'cache_admission_base_tree', 'cache_preservation_source_sha256'}
    api.require({k: v for k, v in current.items() if k not in omit}
                == {k: v for k, v in predecessor.items() if k not in omit},
                'stale cache admission provenance')
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered cache admission source inventory')
    before = {row['path']: row for row in predecessor['files']}
    api.require(len(inputs) == len(before) == 345 and set(inputs) == set(before),
                'unexpected cache admission source membership')
    api.require([row['path'] for row in current['files'] if row != before[row['path']]] == list(PATHS),
                'unexpected cache admission source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part + '/' + name for part in ('src', 'native') for name in api.members(repo / part)]
    api.require(len(expected) == 257 and sorted(actual) == expected,
                'missing or extra compiler source member')
    for name in inputs:
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
    api.require(authority['current_input_identities'] == [identity(name, body, api) for name, body in inputs.items()],
                'stale cache admission complete input identities')
    restored, touched = inverse(inputs, package_bytes['cache-admission-transition.patch'], api)
    api.check_bytes(restored, predecessor['files'])
    api.require(authority['transition_inputs'] == [
        {'path': name, 'before': identity(name, restored[name], api), 'after': identity(name, inputs[name], api)}
        for name in PATHS], 'stale cache admission transition input identities')
    return current, inputs, authority, restored, touched
