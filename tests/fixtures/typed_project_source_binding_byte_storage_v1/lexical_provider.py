"""Exact lexical-provider source successor; no semantic or execution oracle."""
import hashlib
import json
import re

# Generated once from the reviewed immutable source-only checkpoint.
SOURCE_SHA = '952c7cf86d2be1036781155d38f81af8854c0fb26487c4dfa1dc24bc575309db'
SOURCE_BYTES = 67100
AUTHORITY_SHA = '69ea522493ab027b8dfcb3b1fb158f8dde14e7ff283e85d8e1ae6c29dc256a02'
AUTHORITY_BYTES = 136523
PATCH_SHA = '400270e547f1214abd9885d724641427fb8b0871f69f25176818446218d0d458'
PATCH_BYTES = 156583
SOURCE_HEAD = '41c73d527f5518e09877544fa5820f3129f55b42'
SOURCE_TREE = 'ea05a2c15672bdef5b596b4f9d4494e1134d6e76'
BASE_TREE = '0d843f9141f153d4c008ac860786c138165d7de3'
PRODUCER_DIAGNOSTIC_SHA = '35e7e43cb1ef5de8be0c1a78d9e5ac70b1a2caf2efe1e37ba05b7445916c2e29'
PRODUCER_DIAGNOSTIC_BYTES = 66021
PRODUCER_DIAGNOSTIC_HEAD = 'c15465acb90e9f8bb18f5291a8931f5d5bbc6edb'
PATHS = ('src/frontend/driver.rs', 'src/frontend/lexical_provider.rs', 'src/frontend/lexical_provider/bundle.rs', 'src/frontend/lexical_provider/supervisor.rs', 'src/frontend/lexical_provider/tests.rs', 'src/frontend/lexical_provider/wire.rs', 'src/frontend/mod.rs', 'src/frontend/options.rs', 'src/frontend/project.rs')
ADDITIONS = ('src/frontend/lexical_provider.rs', 'src/frontend/lexical_provider/bundle.rs', 'src/frontend/lexical_provider/supervisor.rs', 'src/frontend/lexical_provider/tests.rs', 'src/frontend/lexical_provider/wire.rs')
FIXTURES = ()
PRODUCER_CLOSURE = ({'path': 'fixtures/typed-streaming-lexer/data.ox', 'bytes': 934, 'sha256': 'fcdcbb04a6a82d241655d04aa2a11fe56720d9e243dddb8dafaab9376a98d7b0'}, {'path': 'fixtures/typed-streaming-lexer/frame.ox', 'bytes': 1690, 'sha256': 'abead8c5ab6cd0207b32b20ee00a724a060a3652a1224ad3fbec6545bac8d636'}, {'path': 'fixtures/typed-streaming-lexer/keywords.ox', 'bytes': 2066, 'sha256': '73a0070f751f74c076708cc8458c09ed1d7f8cbd30220c0f2d2c3038a1249e11'}, {'path': 'fixtures/typed-streaming-lexer/main.ox', 'bytes': 1604, 'sha256': '5fb8fba3182c364e0e00048270a30c6315161505b18cbf312026952bd81514b3'}, {'path': 'fixtures/typed-streaming-lexer/scanner.ox', 'bytes': 4176, 'sha256': 'b30888d125fd4a560369828b365812b27a34f993caefc22fe1cbb17e4f49d555'}, {'path': 'fixtures/typed-streaming-lexer/sources.json', 'bytes': 984, 'sha256': 'c591fab7de5fc47578566f9b85dfd088506f07a6e2f6202af0c05646142063a4'}, {'path': 'scripts/build_streaming_lexer.py', 'bytes': 4505, 'sha256': '2a5f6c5eef9fd95bfdda90bd888d5496ac1833d4f831b5a5af0c480d3c01adcd'})
PREDECESSOR_BINDINGS = ({'path': 'producer-diagnostic-authority.json', 'bytes': 135620, 'sha256': 'fa52793cd0717dbf2bf44b819afc32258e257cb0522b0f503d8d7cefbfea0dfb'}, {'path': 'producer-diagnostic-transition.patch', 'bytes': 37700, 'sha256': 'c344823854314d8569d04055cd14031d2b6cd48cd95b8ea6a8301027af0ffe7e'}, {'path': 'producer_diagnostic.py', 'bytes': 10369, 'sha256': '15afd2ed29427665fbbad206efc00d1abf8ba0fd214bb98913d7f2d414db5823'})


def inverse(inputs, patch, api):
    """Recover the exact producer-diagnostic view, removing all five additions."""
    return api.apply_inverse_patch(inputs, patch, PATCH_SHA, PATCH_BYTES, PATHS)


def admit(repo, package_bytes, api):
    """Admit the closed current view before any historical inverse executes."""
    raw = package_bytes['current-source.json']
    api.require(api.digest(raw) == SOURCE_SHA and len(raw) == SOURCE_BYTES,
                'unapproved current source manifest')
    current = json.loads(raw)
    raw = package_bytes['producer-diagnostic-source.json']
    api.require(api.digest(raw) == PRODUCER_DIAGNOSTIC_SHA and len(raw) == PRODUCER_DIAGNOSTIC_BYTES,
                'unapproved producer diagnostic source manifest')
    predecessor = json.loads(raw)
    for row in PREDECESSOR_BINDINGS:
        api.require(api.entry(row['path'], package_bytes[row['path']]) == row,
                    'changed retained producer diagnostic binding: ' + row['path'])
    raw = package_bytes['lexical-provider-authority.json']
    api.require(api.digest(raw) == AUTHORITY_SHA and len(raw) == AUTHORITY_BYTES,
                'stale lexical provider authority')
    authority = json.loads(raw)
    api.require(authority['schema'] == 'oxid-lexical-provider-source-transition-v1'
                and authority['recipe'] == 'git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS'
                and authority['base_tree'] == current['lexical_provider_base_tree'] == predecessor['source_only_tree'] == BASE_TREE
                and authority['predecessor_source_head'] == predecessor['reviewed_source_head'] == PRODUCER_DIAGNOSTIC_HEAD
                and authority['reviewed_source_head'] == current['reviewed_source_head'] == SOURCE_HEAD
                and authority['source_only_tree'] == current['source_only_tree'] == SOURCE_TREE
                and authority['current_source_sha256'] == SOURCE_SHA
                and authority['current_source_bytes'] == SOURCE_BYTES
                and authority['producer_diagnostic_source_sha256'] == current['producer_diagnostic_source_sha256'] == PRODUCER_DIAGNOSTIC_SHA
                and authority['producer_diagnostic_source_bytes'] == PRODUCER_DIAGNOSTIC_BYTES
                and authority['predecessor_bindings'] == list(PREDECESSOR_BINDINGS)
                and authority['transition_patch_sha256'] == PATCH_SHA
                and authority['transition_patch_bytes'] == PATCH_BYTES
                and authority['transition_paths'] == list(PATHS)
                and authority['compiler_additions'] == list(ADDITIONS)
                and authority['fixture_inputs'] == list(FIXTURES)
                and authority['producer_closure'] == list(PRODUCER_CLOSURE)
                and authority['removed_paths'] == [],
                'stale lexical provider transition authority')
    omit = {'files', 'purpose', 'reviewed_source_head', 'source_only_tree',
            'lexical_provider_base_tree', 'producer_diagnostic_source_sha256'}
    api.require({k: v for k, v in current.items() if k not in omit}
                == {k: v for k, v in predecessor.items() if k not in omit},
                'stale lexical provider provenance')
    closure = api.check_entries(repo, list(PRODUCER_CLOSURE))
    manifest = json.loads(closure['fixtures/typed-streaming-lexer/sources.json'])
    sources = {row['path']: row for row in PRODUCER_CLOSURE if row['path'].endswith('.ox')}
    api.require(manifest['schema_version'] == 1 and manifest['entry'] == 'main'
                and {row['path']: row for row in manifest['sources'].values()} == sources
                and sorted(path.name for path in (repo / 'fixtures/typed-streaming-lexer').glob('*.ox'))
                    == sorted(name.rsplit('/', 1)[1] for name in sources),
                'lexical producer source closure differs')
    for name in closure:
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
    inputs = api.check_entries(repo, current['files'])
    api.require([row['path'] for row in current['files']] == sorted(inputs),
                'unordered lexical provider source inventory')
    before = {row['path']: row for row in predecessor['files']}
    api.require(set(inputs) == set(before) | set(ADDITIONS),
                'unexpected lexical provider source membership')
    api.require([row['path'] for row in current['files'] if row != before.get(row['path'])] == list(PATHS),
                'unexpected lexical provider source delta')
    expected = [name for name in inputs if name.startswith(('src/', 'native/'))]
    actual = [part + '/' + name for part in ('src', 'native') for name in api.members(repo / part)]
    api.require(len(expected) == 257 and sorted(actual) == expected,
                'missing or extra compiler source member')
    api.require(not any(re.search(rb'include(?:_str|_bytes)?!\s*\(', inputs[name]) for name in ADDITIONS),
                'lexical provider compile-time fixture closure differs')
    identities = []
    for name, data in inputs.items():
        api.require(api.regular(repo, name).stat().st_mode & 0o111 == 0,
                    'changed input mode: ' + name)
        identities.append({**api.entry(name, data), 'mode': '100644',
            'git_blob': hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()})
    api.require(authority['current_input_identities'] == identities
                and authority['current_input_git_modes'] == [{'path': name, 'mode': '100644'} for name in inputs]
                and authority['current_source_members'] == len(inputs) == 345
                and authority['producer_diagnostic_source_members'] == len(before) == 340
                and authority['compiler_source_members'] == len(expected) == 257
                and authority['compiler_bodies'] == len(expected) + 3 == 260,
                'stale lexical provider complete input identities')
    restored, touched = inverse(inputs, package_bytes['lexical-provider-transition.patch'], api)
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
                'stale lexical provider transition input identities')
    return current, inputs, authority, restored, touched
