"""Exact one-file package-integrity successor; no language or execution oracle."""
import hashlib
import json

SOURCE_SHA = '4d114bbb9b375de743bc18508ebcb48301604f5b417ca0b44d788bf22189c99f'
SOURCE_BYTES = 67336
AUTHORITY_SHA = 'f5a44432927c0e84d58226fcb988860098b2c44d9fe01e4874a771632f89f724'
AUTHORITY_BYTES = 94900
PATCH_SHA = '79c53c652528fe770939ba42dd7a478a309b04555e398d33298bc96644316a49'
PATCH_BYTES = 16227
SOURCE_HEAD = '3315ad42a98cbd033f88fbec676793dec5e0be4a'
SOURCE_TREE = '243e6e55d179a362569ab353aa207eaa95e5ba1b'
BASE_HEAD = 'b50ecd3c1a8de7513f44311e85491c2ee3b2e0b0'
BASE_TREE = '4a9ba347757c7f7e44cb3a4a8fac23300b5c586f'
LEXICAL_SOURCE_SHA = '952c7cf86d2be1036781155d38f81af8854c0fb26487c4dfa1dc24bc575309db'
LEXICAL_SOURCE_BYTES = 67100
PATHS = ('src/runtime/packages.rs',)


def inverse(inputs, patch, api):
    """Recover all 345 lexical-predecessor inputs before any older inverse."""
    return api.apply_inverse_patch(inputs, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def identity(name, body, api):
    return {**api.entry(name, body), 'mode': '100644',
            'git_blob': hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest()}


def admit(repo, package_bytes, api):
    raw = package_bytes['current-source.json']
    api.require(api.digest(raw) == SOURCE_SHA and len(raw) == SOURCE_BYTES,
                'unapproved current source manifest')
    current = json.loads(raw)
    raw = package_bytes['lexical-provider-source.json']
    api.require(api.digest(raw) == LEXICAL_SOURCE_SHA and len(raw) == LEXICAL_SOURCE_BYTES,
                'unapproved lexical predecessor source manifest')
    predecessor = json.loads(raw)
    raw = package_bytes['package-integrity-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale package integrity authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-package-integrity-source-transition-v1'
                and authority['recipe'] == 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
                and authority['base_head'] == current['package_integrity_base_head'] == BASE_HEAD
                and authority['base_tree'] == current['package_integrity_base_tree'] == BASE_TREE
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['lexical_source_sha256'] == current['lexical_provider_source_sha256'] == LEXICAL_SOURCE_SHA
                and authority['lexical_source_bytes'] == LEXICAL_SOURCE_BYTES
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['compiler_additions'] == [] and authority['removed_paths'] == []
                and (authority['current_source_members'], authority['compiler_source_members'],
                     authority['compiler_bodies']) == (345, 257, 260),
                'stale package integrity transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'package_integrity_base_head', 'package_integrity_base_tree', 'lexical_provider_source_sha256'}
    api.require({k: v for k, v in current.items() if k not in omit}
                == {k: v for k, v in predecessor.items() if k not in omit},
                'stale package integrity provenance')
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered package integrity source inventory')
    before = {row['path']: row for row in predecessor['files']}
    api.require(len(inputs) == len(before) == 345 and set(inputs) == set(before),
                'unexpected package integrity source membership')
    api.require([row['path'] for row in current['files'] if row != before[row['path']]] == list(PATHS),
                'unexpected package integrity source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part + '/' + name for part in ('src', 'native') for name in api.members(repo / part)]
    api.require(len(expected) == 257 and sorted(actual) == expected,
                'missing or extra compiler source member')
    for name in inputs:
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
    api.require(authority['current_input_identities'] == [identity(name, body, api) for name, body in inputs.items()],
                'stale package integrity complete input identities')
    restored, touched = inverse(inputs, package_bytes['package-integrity-transition.patch'], api)
    api.check_bytes(restored, predecessor['files'])
    api.require(authority['transition_inputs'] == [
        {'path': name, 'before': identity(name, restored[name], api), 'after': identity(name, inputs[name], api)}
        for name in PATHS], 'stale package integrity transition input identities')
    return current, inputs, authority, restored, touched
