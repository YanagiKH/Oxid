"""INACTIVE pure-byte recipe for the current consumer/CI-input successor.

This module has no filesystem, import-of-target, execution or materialization API.
All original buffers have fixed complete fingerprints. Final body rosters and
their pins must be independently reviewed before either entrypoint is invoked.
Returned buffers are source generation only, never qualification evidence.
"""
import ast
import copy
import hashlib
import json
import re
from pathlib import PurePosixPath

SOURCE = 'tests/fixtures/typed_project_source_binding/'
LEXER = 'tests/qualification/lexer_reservation_current/'
PUBLISHED = 'tests/qualification/published_formatter_current/'
INTEGRATION = 'tests/qualification/unit4_ci/'
SOURCE_HEAD = 'fbff527c895e6cd4ec9350bd8b5d460bc20bb80d'
SOURCE_TREE = '89e90c3ec83d0712d0037a0407fc6571883c5e9c'
PUBLISHED_MAIN_HEAD = '15eac633e167c261ba0246ced49cfdc1c6cef434'
HISTORICAL_HEAD = 'b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489'
HISTORICAL_TREE = '7f5c9aa08c569c4d0b5a27391d8fc68337075d36'
M_SOURCE_SHA = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
C_SOURCE_SHA = 'd83edc200b9b900668ccc7ee9679a7972e1d497d10acf5f82ccd602680dd04e9'
VIEW_SHA = 'e262dd17ebae52f7d662a1408420b795607e3ef6ae4612b1fe0a1a33b15081c8'
INPUTS_SHA = 'e65fbe5018b90bb9cbc3abe1941c391e3678b19d0b2bbe8f00b38e70b4099445'
NATIVE_CONSTANTS = {
    'scripts/verify_bounded_byte_storage.py': 'SOURCE_SHA256',
    'scripts/verify_bounded_enum_native.py': 'REVIEWED_SOURCE_SHA256',
    'scripts/verify_bounded_stdin_native.py': 'REVIEWED_SOURCE_SHA256',
    'scripts/verify_bounded_stdout_native.py': 'REVIEWED_SOURCE_SHA256',
}
ORIGINALS = {
    'scripts/verify_bounded_byte_storage.py': (37410, '700a01821e74ebb1e00e54023854ff1abbe9200ca27e964a3c7ec6ef09155d68'),
    'scripts/verify_bounded_enum_native.py': (19628, 'c55c746259a7f79c1546ed73229964f2d3c8159313291eb1f4707e823a9e06b2'),
    'scripts/verify_bounded_stdin_native.py': (14373, '1949dd86aaa8e39b13073448281b60e85b21ceeda90407847afd8c9ca5bbef75'),
    'scripts/verify_bounded_stdout_native.py': (10116, '7b63090477b116f41dd54546c1a560354b89facc99e8a5ec8a88eb82c034c39c'),
    'scripts/test_bounded_enum_native.py': (33256, '803713ee9fad73f3d11f62849ca6514b0eda8414c6bd36215195b6fb738cbd0c'),
    LEXER + 'unit1_current_support.py': (13587, '87a2d19a05366e886b431f92ec3202a56f62e36e13665e77d1b6e11fdc803579'),
    LEXER + 'unit1_current_bindings.json': (1578, '9e467aaf5f19850845840c727e3fa95f59393ac88fc2efc46e74e3f6af8e1d32'),
    'scripts/verify_owned_array_native.py': (151470, '7333e863889033ac7e600e03bee94363809f36d0abbf71b8960c6a3658bac257'),
    'scripts/test_lexer_current_ci.py': (15803, '342fe0fd5f9947da082891130b419fd4906436fcdd8417efe1ea31c7579eb4d9'),
    '.github/workflows/ci.yml': (67408, '81dfdfe82d9e477c4b6535a10967fc8d6861bcbcea5f90d341c694797ced3269'),
    LEXER + 'direct-consumer-dispositions.json': (191055, 'b69f0585ffd35d889023e3bcefe668db142b0d3adbd7a18cfa3823c88ffa96f0'),
    'scripts/predecessors/byte_storage_ci_v1/.github/workflows/ci.yml': (67395, '7ae94a3f70dd543a28b780912d7ca48fced54d39e0ba0cfdcd2152b5d5b10c57'),
    INTEGRATION + 'inputs.json': (119529, INPUTS_SHA),
    INTEGRATION + 'common.py': (17821, '2e16e81a9f93efc92dd5e4605b01ec8b9231396d94f9adad50afc06f703f59b9'),
    INTEGRATION + 'test_integration.py': (160608, '8eb56af8920b15c76eadfdd6bf4560ee50a1a210d11e41fd603daa14c7990cef'),
}
ACCEPTED = {
    SOURCE + 'current-source.json': (74567, C_SOURCE_SHA),
    SOURCE + 'run.py': (5824, '695ea921fa6c263fd22bb8bd11a8ac01046002ab2feb8943c3831e289ac78b98'),
    SOURCE + 'package-manifest.json': (16025, '9a36d668a53e763bc06cb8e38b32d9808152537bafe14c2dcd0fd4d9a9b64ec0'),
    PUBLISHED + 'predecessor-view-v1.json': (241022, VIEW_SHA),
    '.github/workflows/ci.yml': (67408, '8af4c6810e332afcaebb692087238b598800db70f7198890b50a7f99f531fb96'),
    PUBLISHED + 'published-direct-consumer-dispositions-v1.json': (191975, 'ebdae13dee1a8bcfc5a62fba03e8088e9662cea0c676c44428975433b94707d1'),
}
CONSUMER_ORIGINAL_NAMES = frozenset(ORIGINALS) - {
    INTEGRATION + 'inputs.json', INTEGRATION + 'common.py', INTEGRATION + 'test_integration.py',
}
FINAL_ORIGINAL_NAMES = frozenset((
    INTEGRATION + 'inputs.json', INTEGRATION + 'common.py', INTEGRATION + 'test_integration.py',
))
# These roles must be present in the externally reviewed finite roster. Other
# additions are accepted only by that independently pinned whole roster, never
# by traversing a directory or assigning a placeholder/newly inferred digest.
ESSENTIAL_ROLES = (
    PUBLISHED + 'published_transition.py',
    PUBLISHED + 'published_current.py',
    PUBLISHED + 'published_view.py',
    PUBLISHED + 'published_ci_inventory.py',
    PUBLISHED + 'test_published_transition.py',
    PUBLISHED + 'test_published_view.py',
    PUBLISHED + 'test_published_ci_inventory.py',
    PUBLISHED + 'predecessor-view-v1.json',
    PUBLISHED + 'published-direct-consumer-dispositions-v1.json',
    PUBLISHED + 'source-proof/published-main-ci.yml',
    'scripts/test_published_formatter_source.py',
    LEXER + 'unit1_current_support.py',
    LEXER + 'unit1_current_bindings.json',
)


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def descriptor(name, raw):
    return {'path': name, 'bytes': len(raw), 'sha256': sha(raw)}


def relative(name):
    need(type(name) is str and name and '\\' not in name and not name.startswith('/')
         and str(PurePosixPath(name)) == name
         and all(part not in ('', '.', '..') for part in name.split('/'))
         and all(ord(character) >= 32 and ord(character) != 127 for character in name),
         'unsafe finite-roster path')
    return name


def pin(raw, expected, name):
    relative(name)
    need(type(raw) is bytes and type(expected) is dict
         and set(expected) == {'path', 'bytes', 'sha256'}
         and expected['path'] == name and type(expected['bytes']) is int
         and 0 <= expected['bytes'] <= 8 * 1024 * 1024
         and type(expected['sha256']) is str
         and re.fullmatch('[0-9a-f]{64}', expected['sha256']) is not None
         and descriptor(name, raw) == expected,
         'changed independently reviewed complete buffer: ' + name)


def fixed(raw, name, table):
    size, digest = table[name]
    pin(raw, {'path': name, 'bytes': size, 'sha256': digest}, name)


def unique(pairs):
    result = {}
    for key, item in pairs:
        need(key not in result, 'duplicate JSON key')
        result[key] = item
    return result


def loads(raw):
    return json.loads(raw, object_pairs_hook=unique)


def canonical(value):
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False,
                       allow_nan=False) + '\n').encode()


def seam(raw, before, after, label):
    need(type(before) is bytes and before and type(after) is bytes
         and raw.count(before) == 1 and before != after,
         'missing or ambiguous exact source seam: ' + label)
    result = raw.replace(before, after, 1)
    need(result.count(after) == 1 and result.replace(after, before, 1) == raw,
         'noninvertible exact source seam: ' + label)
    return result


def literal(raw, name, old, new):
    """Replace one authenticated top-level literal, not equal text elsewhere."""
    tree = ast.parse(raw)
    found = [node for node in tree.body if isinstance(node, ast.Assign)
             and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
             and node.targets[0].id == name]
    need(len(found) == 1 and isinstance(found[0].value, ast.Constant)
         and type(found[0].value.value) is type(old) and found[0].value.value == old,
         'changed exact top-level literal: ' + name)
    node = found[0].value
    lines = raw.splitlines(keepends=True)
    start = sum(map(len, lines[:node.lineno - 1])) + node.col_offset
    end = sum(map(len, lines[:node.end_lineno - 1])) + node.end_col_offset
    replacement = repr(new).encode()
    result = raw[:start] + replacement + raw[end:]
    parsed = ast.parse(result)
    updated = [n.value for n in parsed.body if isinstance(n, ast.Assign)
               and len(n.targets) == 1 and isinstance(n.targets[0], ast.Name)
               and n.targets[0].id == name]
    need(len(updated) == 1 and isinstance(updated[0], ast.Constant)
         and type(updated[0].value) is type(new) and updated[0].value == new,
         'derived top-level literal mismatch: ' + name)
    return result


def function_sources(raw):
    text = raw.decode('utf-8')
    result = {}
    def visit(nodes, prefix):
        for node in nodes:
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                name = '.'.join((*prefix, node.name))
                if not isinstance(node, ast.ClassDef):
                    need(name not in result, 'duplicate function identity')
                    result[name] = ast.get_source_segment(text, node).encode()
                visit(node.body, (*prefix, node.name))
    visit(ast.parse(raw).body, ())
    return result


def preservation(before, after, changed=(), added=()):
    old, new = function_sources(before), function_sources(after)
    need(set(new) == set(old) | set(added) and not (set(old) & set(added)),
         'function/method membership changed outside approved scope')
    need(set(changed) <= set(old), 'unknown changed function/method scope')
    preserved = []
    changed_rows = []
    for name, raw in old.items():
        if name in changed:
            need(raw != new[name], 'declared function did not change: ' + name)
            changed_rows.append({'name': name, 'original_sha256': sha(raw),
                                 'derived_sha256': sha(new[name])})
        else:
            need(new[name] == raw, 'unapproved function/method body change: ' + name)
            preserved.append({'name': name, 'bytes': len(raw), 'sha256': sha(raw)})
    return {'preserved_functions_and_methods': preserved,
            'changed_functions_and_methods': changed_rows,
            'added_functions': [{'name': name, 'bytes': len(new[name]), 'sha256': sha(new[name])}
                                for name in sorted(added)]}


def validate_buffers(buffers, names, table):
    need(type(buffers) is dict and set(buffers) == set(names),
         'missing/extra finite original buffer roster')
    for name in sorted(names):
        fixed(buffers[name], name, table)


def source_only_receipt(kind, originals, outputs, preservation_rows):
    return {
        'schema': kind, 'status': 'source-generation-only-pending-independent-review',
        'originals': [descriptor(name, originals[name]) for name in sorted(originals)],
        'outputs': [descriptor(name, outputs[name]) for name in sorted(outputs)],
        'changed_file_names': sorted(outputs), 'function_preservation': preservation_rows,
        'source_checkpoint': {'head': SOURCE_HEAD, 'tree': SOURCE_TREE},
        'actual_published_main_head': PUBLISHED_MAIN_HEAD,
        'historical_source': {'head': HISTORICAL_HEAD, 'tree': HISTORICAL_TREE},
        'target_modules_imported': 0, 'tests_executed': 0, 'children_launched': 0,
        'views_materialized': 0, 'compiler_executions': 0, 'native_executions': 0,
        'execution_qualified': False,
        'pending_gates': ['independent-exact-generated-byte-review', 'source-control-execution',
                          'compiler', 'runtime', 'host', 'native', 'permissions',
                          'profiles', 'failure-preservation', 'exact-final-CI-head-checks'],
    }


def derive_consumers(originals, accepted):
    """Return changed consumer buffers and receipt; never read or write paths."""
    validate_buffers(originals, CONSUMER_ORIGINAL_NAMES, ORIGINALS)
    validate_buffers(accepted, ACCEPTED, ACCEPTED)
    current = loads(accepted[SOURCE + 'current-source.json'])
    need(len(current['files']) == 376 and current['reviewed_source_head'] == SOURCE_HEAD
         and current['source_only_tree'] == SOURCE_TREE
         and current['published_main_head'] == PUBLISHED_MAIN_HEAD,
         'accepted current source identity/count mismatch')
    view = loads(accepted[PUBLISHED + 'predecessor-view-v1.json'])
    need(len(view['files']) == 681 and len(view['directories']) == 111
         and view['logical_bytes'] == 38003640
         and view['published_main']['head'] == PUBLISHED_MAIN_HEAD,
         'accepted genuine predecessor closure mismatch')
    for row in view['files']:
        if row['read_from'] is not None:
            relative(row['read_from'])
    outputs, identities = {}, {}
    for name, constant in NATIVE_CONSTANTS.items():
        raw = literal(originals[name], constant, M_SOURCE_SHA, C_SOURCE_SHA)
        outputs[name] = raw
        identities[name] = preservation(originals[name], raw)

    name = 'scripts/test_bounded_enum_native.py'
    raw = originals[name]
    for before, after, label in (
        ('self.assertEqual(manifest["reviewed_source_head"], "' + HISTORICAL_HEAD + '")',
         'self.assertEqual(manifest["reviewed_source_head"], "' + SOURCE_HEAD + '")', 'enum current head'),
        ('self.assertEqual(manifest["source_only_tree"], "' + HISTORICAL_TREE + '")',
         'self.assertEqual(manifest["source_only_tree"], "' + SOURCE_TREE + '")', 'enum current tree'),
        ("        expected = '" + M_SOURCE_SHA + "'\n",
         "        expected = '" + C_SOURCE_SHA + "'\n", 'enum current source'),
    ):
        raw = seam(raw, before.encode(), after.encode(), label)
    before = b'''        changed = sorted(name for name in current_map if current_map[name] != predecessor_map[name])
        self.assertEqual(changed, transition["transition_paths"])
        self.assertEqual(len(changed), 15)
'''
    after = b'''        published_raw = (repo / 'tests/qualification/published_formatter_current/predecessor-v1/tests/fixtures/typed_project_source_binding/current-source.json').read_bytes()
        self.assertEqual((len(published_raw), hashlib.sha256(published_raw).hexdigest()),
                         (74328, '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'))
        published = json.loads(published_raw)
        self.assertIs(type(published), dict)
        self.assertIs(type(published['files']), list)
        for row in published['files']:
            self.assertIs(type(row), dict)
            self.assertIs(type(row['path']), str)
            self.assertIs(type(row['bytes']), int)
            self.assertIs(type(row['sha256']), str)
        self.assertEqual((len(published['files']), published['reviewed_source_head'], published['source_only_tree']),
                         (376, 'b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489', '7f5c9aa08c569c4d0b5a27391d8fc68337075d36'))
        published_map = {row['path']: row for row in published['files']}
        self.assertEqual(len(published_map), 376)
        self.assertEqual(set(current_map), set(published_map))
        self.assertEqual(sorted(name for name in current_map if current_map[name] != published_map[name]),
                         ['src/main.rs'])
        changed = sorted(name for name in published_map if published_map[name] != predecessor_map[name])
        self.assertEqual(changed, transition["transition_paths"])
        self.assertEqual(len(changed), 15)
'''
    raw = seam(raw, before, after, 'enum retain historical15 and separate exact current delta')
    outputs[name] = raw
    identities[name] = preservation(originals[name], raw, (
        'BoundedEnumNativeControls.test_approved_manifest_rejects_coherent_omission_and_rehash',
        'BoundedEnumNativeControls.test_native_entrypoints_and_workflow_share_the_exact_current_source_pin',
    ))

    map_name = LEXER + 'unit1_current_bindings.json'
    old_map = loads(originals[map_name])
    need(old_map['compiler_checkpoint'] == HISTORICAL_HEAD and len(old_map['files']) == 7
         and old_map['original_cases_per_profile'] == 69
         and old_map['current_controls_per_profile'] == 1
         and old_map['profiles'] == ['debug', 'release']
         and old_map['execution_qualified'] is False,
         'changed fixed Unit1 original69/control1 map')
    # Preserve every byte except the one exact checkpoint literal, including all
    # seven body rows, original-package pin, profiles and zero-execution fields.
    new_map = seam(originals[map_name], ('"compiler_checkpoint": "' + HISTORICAL_HEAD + '"').encode(),
                   ('"compiler_checkpoint": "' + SOURCE_HEAD + '"').encode(), 'Unit1 checkpoint')
    need(loads(new_map) == dict(old_map, compiler_checkpoint=SOURCE_HEAD),
         'Unit1 map changed outside compiler checkpoint')
    outputs[map_name] = new_map
    name = LEXER + 'unit1_current_support.py'
    raw = originals[name]
    for constant, old, new in (
        ('CHECKPOINT', HISTORICAL_HEAD, SOURCE_HEAD),
        ('CURRENT_SOURCE_SHA', M_SOURCE_SHA, C_SOURCE_SHA),
        ('CURRENT_SOURCE_BYTES', 74328, 74567),
        ('DISPATCHER_SHA', '164d1e6213e140aca02f8f4a29c716f8af6c4af26e166e18fc0777251c0c2e1e', ACCEPTED[SOURCE + 'run.py'][1]),
        ('SOURCE_PACKAGE_SHA', '5fb5ee78a42609dab51f8e8bb23c325305954f49c52f0d18df6bbe47e57548bc', ACCEPTED[SOURCE + 'package-manifest.json'][1]),
        ('CURRENT_MAP_SHA', ORIGINALS[map_name][1], sha(new_map)),
    ):
        raw = literal(raw, constant, old, new)
    helper = b'''def verify_source_entrypoint(dispatcher_raw, manifest_raw) -> None:
    """Authenticate genuine source entrypoint buffers before target execution."""
    need(sha(dispatcher_raw) == DISPATCHER_SHA, 'Changed trusted current source dispatcher')
    need(sha(manifest_raw) == SOURCE_PACKAGE_SHA,
         'Changed trusted current source-binding package manifest')


'''
    raw = seam(raw, b'def admit(repo):\n', helper + b'def admit(repo):\n', 'Unit1 genuine buffer boundary')
    before = b"    need(sha(regular(package / 'package-manifest.json')) == SOURCE_PACKAGE_SHA,\n         'Changed trusted current source-binding package manifest')\n"
    after = b"    manifest_raw = regular(package / 'package-manifest.json')\n    verify_source_entrypoint(dispatcher_raw, manifest_raw)\n"
    raw = seam(raw, before, after, 'Unit1 use authenticated buffer boundary')
    early_order = b"    dispatcher_raw = regular(package / 'run.py')\n    need(sha(dispatcher_raw) == DISPATCHER_SHA, 'Changed trusted current source dispatcher')\n" + after
    need(raw.count(early_order) == 1
         and raw.index(early_order) < raw.index(b"    dispatcher = load_module('_unit1_current_source_binding'"),
         'Unit1 early dispatcher failure/read/execute order changed')
    outputs[name] = raw
    identities[name] = preservation(originals[name], raw, ('admit',), ('verify_source_entrypoint',))

    name = 'scripts/verify_owned_array_native.py'
    raw = originals[name]
    before = b'                "tests/qualification/lexer_reservation_current/",\n'
    after = before + b'                "tests/qualification/published_formatter_current/",\n'
    raw = seam(raw, before, after, 'native new closed helper namespace')
    before = b'    names.update(row["path"] for row in authority["repository_inputs"])\n'
    after = before + b'''    view_path = "tests/qualification/published_formatter_current/predecessor-view-v1.json"
    view_raw, _ = stable_bytes(no_symlink_path(repo / view_path))
    require(len(view_raw) == 241022 and sha256(view_raw) ==
            "e262dd17ebae52f7d662a1408420b795607e3ef6ae4612b1fe0a1a33b15081c8",
            "changed fixed published-main predecessor view")
    predecessor_view = strict_json(view_raw)
    require(len(predecessor_view["files"]) == 681,
            "changed published-main predecessor dependency count")
    for row in predecessor_view["files"]:
        dependency = row["read_from"]
        if dependency is not None:
            require(type(dependency) is str and dependency and "\\\\" not in dependency
                    and not dependency.startswith("/")
                    and all(part not in ("", ".", "..") for part in dependency.split("/")),
                    "unsafe published-main predecessor dependency")
            names.add(dependency)
'''
    raw = seam(raw, before, after, 'native complete fixed predecessor dependency closure')
    outputs[name] = raw
    identities[name] = preservation(originals[name], raw, ('source_members',))

    name = 'scripts/test_lexer_current_ci.py'
    raw = originals[name]
    for constant, old, new in (
        ('CURRENT_SHA', M_SOURCE_SHA, C_SOURCE_SHA),
        ('COMPILER_HEAD', HISTORICAL_HEAD, SOURCE_HEAD),
        ('COMPILER_TREE', HISTORICAL_TREE, SOURCE_TREE),
    ):
        raw = literal(raw, constant, old, new)
    before = b'import ci_inventory\n'
    after = before + ("\nPUBLISHED = REPO / 'tests/qualification/published_formatter_current'\n"
        "PUBLISHED_MAIN_SOURCE_SHA = " + repr(M_SOURCE_SHA) + "\n"
        "PUBLISHED_MAIN_WORKFLOW_SHA = " + repr(ORIGINALS['.github/workflows/ci.yml'][1]) + "\n"
        "PUBLISHED_VIEW_SHA = " + repr(VIEW_SHA) + "\n"
        "CURRENT_SOURCE_DESCRIPTOR = " + repr(descriptor('published-formatter-source-v1.json', accepted[SOURCE + 'current-source.json'])) + "\n"
        "CURRENT_WORKFLOW_DESCRIPTOR = " + repr(descriptor('.github/workflows/ci.yml', accepted['.github/workflows/ci.yml'])) + "\n"
        "CURRENT_DISPOSITIONS_DESCRIPTOR = " + repr(descriptor('published-direct-consumer-dispositions-v1.json', accepted[PUBLISHED + 'published-direct-consumer-dispositions-v1.json'])) + "\n"
        "sys.path.insert(0, str(PUBLISHED))\nimport published_ci_inventory\n").encode()
    raw = seam(raw, before, after, 'CI fixed source successor inventory dependency')
    before = b"        plan = read(HELPER / 'direct-consumer-dispositions.json')\n"
    after = b'''        current_raw = (PUBLISHED / 'published-direct-consumer-dispositions-v1.json').read_bytes()
        plan = json.loads(current_raw)
        complete = published_ci_inventory.validate(
            (REPO / 'docs/architecture/fallible-lexer-source-consumer-audit.md').read_bytes(),
            (PRESERVED / '.github/workflows/ci.yml').read_bytes(),
            (PUBLISHED / 'source-proof/published-main-ci.yml').read_bytes(),
            (REPO / '.github/workflows/ci.yml').read_bytes(),
            (HELPER / 'direct-consumer-dispositions.json').read_bytes(), current_raw,
            CURRENT_SOURCE_DESCRIPTOR, CURRENT_WORKFLOW_DESCRIPTOR, CURRENT_DISPOSITIONS_DESCRIPTOR)
        self.assertEqual(complete, {'entry_count': 173,
            'changed_commands': ['ci.yml:L751', 'ci.yml:L764'],
            'argument_lines': [755, 768], 'source_only': True, 'execution_qualified': False})
'''
    raw = seam(raw, before, after, 'CI full current disposition validator')
    before = b"        expected = raw.replace(PREDECESSOR_SHA.encode(), CURRENT_SHA.encode()).replace(old, new)\n        self.assertEqual((REPO / '.github/workflows/ci.yml').read_bytes(), expected)\n"
    after = b'''        middle = (PUBLISHED / 'source-proof/published-main-ci.yml').read_bytes()
        self.assertEqual(sha(middle), PUBLISHED_MAIN_WORKFLOW_SHA)
        expected_middle = raw.replace(PREDECESSOR_SHA.encode(), PUBLISHED_MAIN_SOURCE_SHA.encode()).replace(old, new)
        self.assertEqual(middle, expected_middle)
        old_argument = b'--source-manifest-sha256 ' + PUBLISHED_MAIN_SOURCE_SHA.encode()
        new_argument = b'--source-manifest-sha256 ' + CURRENT_SHA.encode()
        self.assertEqual(middle.count(old_argument), 2)
        self.assertEqual(middle.count(PUBLISHED_MAIN_SOURCE_SHA.encode()), 2)
        expected = middle.replace(old_argument, new_argument)
        current = (REPO / '.github/workflows/ci.yml').read_bytes()
        self.assertEqual(current, expected)
        self.assertEqual(current.count(new_argument), 2)
        self.assertEqual(current.replace(new_argument, old_argument), middle)
        self.assertEqual(published_ci_inventory.workflow_successor(raw, middle, CURRENT_SHA), current)
'''
    raw = seam(raw, before, after, 'CI explicit oldest-to-M3 and M-to-C2 substitutions')
    before = b"                 'tests/qualification/lexer_reservation_current',\n"
    after = before + b"                 'tests/qualification/published_formatter_current',\n"
    raw = seam(raw, before, after, 'CI native new helper closed-root census')
    before = b"        manifest, names = producer.source_members(REPO, tracked)\n"
    after = b'''        view_raw = (PUBLISHED / 'predecessor-view-v1.json').read_bytes()
        self.assertEqual((len(view_raw), sha(view_raw)), (241022, PUBLISHED_VIEW_SHA))
        view = json.loads(view_raw)
        self.assertEqual(len(view['files']), 681)
        dependencies = {row['read_from'] for row in view['files'] if row['read_from'] is not None}
        tracked.update({name: {} for name in dependencies})
''' + before + b'''        self.assertTrue(dependencies <= set(names))
        self.assertTrue({p.relative_to(REPO).as_posix() for p in PUBLISHED.rglob('*') if p.is_file()} <= set(names))
'''
    raw = seam(raw, before, after, 'CI all declared predecessor dependencies captured')
    outputs[name] = raw
    identities[name] = preservation(originals[name], raw, (
        'CurrentCIActivationControls.test_all_173_direct_entries_have_exact_individual_dispositions',
        'CurrentCIActivationControls.test_workflow_has_only_three_approved_active_substitutions',
        'CurrentCIActivationControls.test_native_producer_closes_current_and_retained_adapter_roots',
    ))
    outputs[PUBLISHED + 'source-proof/published-main-ci.yml'] = originals['.github/workflows/ci.yml']
    receipt = source_only_receipt('oxid-published-current-consumer-derivation-v1', originals, outputs, identities)
    receipt.update(accepted_inputs=[descriptor(name, accepted[name]) for name in sorted(accepted)],
                   original_unit1_cases_per_profile=69, separate_unit1_controls_per_profile=1,
                   original_unit1_body_rows=7, original_ci_dispositions=173,
                   workflow_jobs_added=0, workflow_commands_added=0,
                   exact_native_source_constant_replacements=4,
                   native_predecessor_dependency_count=sum(row['read_from'] is not None for row in view['files']))
    return outputs, receipt


def final_roster(raw, reviewed_pin, bodies, old):
    """Authenticate the independently reviewed FINAL finite body roster."""
    pin(raw, reviewed_pin, 'reviewed/current-consumer-final-body-roster-v1.json')
    value = loads(raw)
    need(type(value) is dict and set(value) == {'schema', 'files'}
         and value['schema'] == 'oxid-published-current-ci-final-roster-v1'
         and type(value['files']) is list and 556 < len(value['files']) <= 4096,
         'wrong final-body-roster schema or exact finite count')
    names = [row['path'] for row in value['files']]
    need(names == sorted(set(names)) and type(bodies) is dict and set(names) == set(bodies),
         'missing/extra/duplicate/unordered final body roster')
    need(not any(name.startswith(INTEGRATION) for name in names),
         'live integration-root inputs must be independently captured, never self-pinned')
    need(not any('__pycache__' in name.split('/') or name.endswith(('.pyc', '.pyo')) for name in names),
         'generated cache cannot enter final input closure')
    old_names = {row['path'] for row in old['files']}
    need(old_names <= set(names) and set(ESSENTIAL_ROLES) <= set(names),
         'final roster omitted old556 or essential successor input')
    for row in value['files']:
        pin(bodies[row['path']], row, row['path'])
    for name in ACCEPTED:
        if name != '.github/workflows/ci.yml':
            need(name in bodies, 'final roster missing fixed accepted artifact')
            fixed(bodies[name], name, ACCEPTED)
    package = loads(bodies[SOURCE + 'package-manifest.json'])
    need(len(package['files']) == 99, 'wrong final source package body roster')
    for row in package['files']:
        name = SOURCE + relative(row['path'])
        need(name in bodies, 'final closure omitted accepted source package body: ' + name)
        pin(bodies[name], dict(row, path=name), name)
    view = loads(bodies[PUBLISHED + 'predecessor-view-v1.json'])
    need(len(view['files']) == 681, 'wrong final genuine predecessor dependency count')
    for row in view['files']:
        name = row['read_from']
        if name is not None:
            relative(name)
            need(name in bodies, 'final closure omitted genuine predecessor dependency: ' + name)
            pin(bodies[name], {key: name if key == 'path' else row[key]
                               for key in ('path', 'bytes', 'sha256')}, name)
    fixed(bodies[PUBLISHED + 'source-proof/published-main-ci.yml'], '.github/workflows/ci.yml', ORIGINALS)
    return value


def derive_integration_test(original, current):
    """Patch only the two specified closure tests; all other methods stay exact."""
    name = INTEGRATION + 'test_integration.py'
    fixed(original, name, ORIGINALS)
    raw = original
    raw = seam(raw,
        b"        self.assertEqual(raw, (q.HERE / 'inputs-lexer-reservation-v1.json').read_bytes())\n",
        b"        self.assertEqual(raw, (q.HERE / 'inputs-published-formatter-v1.json').read_bytes())\n",
        'integration current named alias')
    before = b"        current = q.loads(raw)\n"
    after = before + b'''        predecessor_raw = (q.HERE / 'inputs-lexer-reservation-v1.json').read_bytes()
        self.assertEqual((len(predecessor_raw), q.sha(predecessor_raw)),
                         (119529, 'e65fbe5018b90bb9cbc3abe1941c391e3678b19d0b2bbe8f00b38e70b4099445'))
        predecessor = q.loads(predecessor_raw)
        self.assertEqual((len(predecessor['files']), len(predecessor['closed_roots'])), (556, 15))
        self.assertEqual(current['published_formatter_predecessor_inputs'], {
            'path': 'tests/qualification/unit4_ci/inputs-lexer-reservation-v1.json',
            'bytes': 119529, 'sha256': 'e65fbe5018b90bb9cbc3abe1941c391e3678b19d0b2bbe8f00b38e70b4099445'})
        self.assertTrue({row['path'] for row in predecessor['files']} <=
                        {row['path'] for row in current['files']})
        self.assertEqual(current['closed_roots'], predecessor['closed_roots'] +
                         ['tests/qualification/published_formatter_current'])
'''
    # The exact snippet appears only in this authenticated first closure test.
    raw = seam(raw, before, after, 'integration genuine M556/15 named predecessor')
    before = b"            'scripts/predecessors/lexer_reservation_source_v1'])\n"
    after = b"            'scripts/predecessors/lexer_reservation_source_v1',\n            'tests/qualification/published_formatter_current'])\n"
    raw = seam(raw, before, after, 'integration preserve old271 root assertions plus one root')
    before = b"        self.assertEqual((len(current['files']), len(current['closed_roots'])), (556, 15))\n"
    after = ("        self.assertEqual((len(current['files']), len(current['closed_roots'])), ("
             + str(len(current['files'])) + ", 16))\n"
             + "        self.assertEqual(current['published_formatter_current_source'], "
             + repr(current['published_formatter_current_source']) + ")\n"
             + "        self.assertEqual(current['published_formatter_source_checkpoint'], "
             + repr(current['published_formatter_source_checkpoint']) + ")\n"
             + "        self.assertEqual(current['published_formatter_published_main_head'], "
             + repr(PUBLISHED_MAIN_HEAD) + ")\n"
             + "        self.assertTrue(set(" + repr(ESSENTIAL_ROLES)
             + ") <= {row['path'] for row in current['files']})\n").encode()
    raw = seam(raw, before, after, 'integration exact current finite names/count and identities')
    before = b"                     'scripts/predecessors/lexer_reservation_source_v1/tests/qualification/unit4_ci/common.py'):\n"
    omission_names = tuple(name for name in ESSENTIAL_ROLES if name.startswith(PUBLISHED))
    after = b"                     'scripts/predecessors/lexer_reservation_source_v1/tests/qualification/unit4_ci/common.py',\n" + b''.join(
        ('                     ' + repr(name) + ('):\n' if i == len(omission_names) - 1 else ',\n')).encode()
        for i, name in enumerate(omission_names))
    raw = seam(raw, before, after, 'integration new executable/helper/proof/alias omission controls')
    identities = preservation(original, raw, (
        'LexerInputClosureControls.test_current_complete_inventory_and_named_predecessor',
        'LexerInputClosureControls.test_complete_inventory_cannot_omit_new_or_saved_adapters',
    ))
    need(b'        q.verify_package(REPO, current)\n' in raw,
         'current physical package verification was removed')
    return raw, identities


def finalize_ci_inputs(originals, *, final_roster_raw, reviewed_roster_pin, final_bodies):
    """Generate current inputs LAST from complete reviewed final body buffers.

    The independent review must freeze the whole roster and every full body pin
    after consumers, helpers, proof aliases and the CI source entrypoint are final.
    This function does not fill missing paths, copy trees, verify M against C,
    execute candidate modules, or assign a freshly observed identity as authority.
    The two current Git manifest paths deliberately share one returned bytes value.
    """
    validate_buffers(originals, FINAL_ORIGINAL_NAMES, ORIGINALS)
    old_raw = originals[INTEGRATION + 'inputs.json']
    old = loads(old_raw)
    need(len(old['files']) == 556 and len(old['closed_roots']) == 15
         and old['lexer_reservation_current_source_sha256'] == M_SOURCE_SHA
         and PUBLISHED.rstrip('/') not in old['closed_roots'],
         'changed genuine M556/15 integration predecessor')
    roster = final_roster(final_roster_raw, reviewed_roster_pin, final_bodies, old)
    current = copy.deepcopy(old)
    current.update(
        schema='oxid-unit4-ci-published-formatter-inputs-v1',
        files=copy.deepcopy(roster['files']),
        closed_roots=old['closed_roots'] + [PUBLISHED.rstrip('/')],
        published_formatter_predecessor_inputs=descriptor(INTEGRATION + 'inputs-lexer-reservation-v1.json', old_raw),
        published_formatter_current_source={'path': SOURCE + 'current-source.json',
                                            'bytes': 74567, 'sha256': C_SOURCE_SHA},
        published_formatter_source_checkpoint={'head': SOURCE_HEAD, 'tree': SOURCE_TREE},
        published_formatter_published_main_head=PUBLISHED_MAIN_HEAD,
        published_formatter_execution_qualified=False,
    )
    need(len(current['closed_roots']) == 16 and len(set(current['closed_roots'])) == 16,
         'wrong exact final closed-root roster')
    current_raw = canonical(current)
    name = INTEGRATION + 'common.py'
    common = literal(originals[name], 'INPUTS_SHA', INPUTS_SHA, sha(current_raw))
    common = literal(common, 'CURRENT_SHA', M_SOURCE_SHA, C_SOURCE_SHA)
    common_identities = preservation(originals[name], common)
    test, test_identities = derive_integration_test(
        originals[INTEGRATION + 'test_integration.py'], current)
    need(len(test) <= 172032, 'derived integration test exceeds reviewed 168-KiB output cap')
    outputs = {
        INTEGRATION + 'inputs-lexer-reservation-v1.json': old_raw,
        INTEGRATION + 'inputs.json': current_raw,
        INTEGRATION + 'inputs-published-formatter-v1.json': current_raw,
        INTEGRATION + 'common.py': common,
        INTEGRATION + 'test_integration.py': test,
    }
    receipt = source_only_receipt('oxid-published-current-ci-closure-derivation-v1', originals, outputs, {
        INTEGRATION + 'common.py': common_identities,
        INTEGRATION + 'test_integration.py': test_identities,
    })
    before = {row['path']: row for row in old['files']}
    after = {row['path']: row for row in current['files']}
    receipt.update(
        reviewed_final_roster=copy.deepcopy(reviewed_roster_pin),
        predecessor_files=556, predecessor_closed_roots=15,
        current_files=len(after), current_closed_roots=16,
        all_old_names_retained=True,
        added_input_names=sorted(set(after) - set(before)),
        changed_existing_input_names=sorted(name for name in before if before[name] != after[name]),
        unchanged_existing_input_names=sorted(name for name in before if before[name] == after[name]),
        current_manifests_identical=True,
        self_pinned_live_integration_files=False,
        common_function_bodies_unchanged=True,
        actual_Git_head_integration_directory_capture_unchanged=True,
        integration_test_bytes=len(test),
        original_271_history_assertions_preserved=True,
        current_physical_package_verification_preserved=True,
    )
    return outputs, receipt
