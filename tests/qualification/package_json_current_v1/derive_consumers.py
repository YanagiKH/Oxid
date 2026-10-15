"""Pure, finite consumer derivation from authenticated accepted-main bodies."""
import ast
import copy
import json

import generation as g

NATIVE = ('scripts/verify_bounded_byte_storage.py', 'scripts/verify_bounded_enum_native.py',
          'scripts/verify_bounded_stdin_native.py', 'scripts/verify_bounded_stdout_native.py')
SUPPORT = 'tests/qualification/lexer_reservation_current/unit1_current_support.py'
BINDINGS = 'tests/qualification/lexer_reservation_current/unit1_current_bindings.json'
UNIT2_TEST = 'tests/qualification/unit2_u8_current/test_current.py'
SIMPLE = (*NATIVE, 'scripts/test_bounded_enum_native.py', 'scripts/verify_owned_array_native.py',
          '.github/workflows/ci.yml', UNIT2_TEST)


def exact(raw, before, after, count):
    g.need(raw.count(before) == count, 'exact consumer replacement occurrence')
    return raw.replace(before, after)


def assignment(raw, name, value):
    lines = raw.decode().splitlines(True)
    nodes = [n for n in ast.parse(raw).body if isinstance(n, ast.Assign)
             and len(n.targets) == 1 and isinstance(n.targets[0], ast.Name) and n.targets[0].id == name]
    g.need(len(nodes) == 1, 'exact consumer constant: ' + name)
    n = nodes[0]; lines[n.lineno-1:n.end_lineno] = [name + ' = ' + repr(value) + '\n']
    return ''.join(lines).encode()


def unit2_test(raw):
    """Retain the published-layer checks and add the distinct current layer."""
    name = 'test_current_materialization_consumes_separately_named_successor'
    methods = [node for node in ast.walk(ast.parse(raw))
               if isinstance(node, ast.FunctionDef) and node.name == name]
    g.need(len(methods) == 1, 'exact Unit2 materialization control')
    node = methods[0]
    original = b''.join(raw.splitlines(True)[node.lineno-1:node.end_lineno])
    body = original.replace(b'transfer', b'published')
    body = exact(body, b"        published = captured['published_source_unit2_accounting_source_binding']\n",
                 b"        published = captured['published_source_unit2_accounting_source_binding']\n"
                 b"        published_source = (PACKAGE / 'published-formatter-source-v1.json').read_bytes()\n", 1)
    body = exact(body, b'self.current_source', b'published_source', 3)
    body = exact(body,
                 b'        # The lexer association remains historical; only the separately named\n'
                 b'        # published published binds the current execution source.\n',
                 b'        # Both lexer and published associations retain their historical sources.\n', 1)
    anchor = b'        with tempfile.TemporaryDirectory() as root:\n'
    checks = b'''        transfer = captured['package_json_unit2_accounting_source_binding']
        self.assertEqual(transfer['version'], 'unit2-package-json-identical-accounting-source-v1')
        self.assertEqual(transfer['predecessor_accounting_source_binding'], published)
        self.assertEqual(published, captured['package_json_predecessor']['published_source_unit2_accounting_source_binding'])
        self.assertNotEqual(transfer['current_source'], published['current_source'])
        self.assertEqual(transfer['current_source'], binding.entry('current-source.json', self.current_source))
        current = json.loads(self.current_source)
        self.assertEqual(transfer['reviewed_source_head'], current['reviewed_source_head'])
        self.assertEqual(transfer['source_only_tree'], current['source_only_tree'])
        self.assertEqual(transfer['package_json_provenance'], current['package_json_source'])
        for key in ('retained_accounting_source', 'retained_reviewed_source_head',
                    'retained_source_only_tree', 'retained_authority', 'retained_helper',
                    'derived_resource', 'source_dependencies'):
            self.assertEqual(transfer[key], published[key])
'''
    body = exact(body, anchor, checks + anchor, 1)
    anchor = b"            self.assertEqual(seam['published_source_unit2_accounting_source_binding'], published)\n"
    body = exact(body, anchor, anchor +
                 b"            self.assertEqual(seam['package_json_unit2_accounting_source_binding'], transfer)\n", 1)
    return exact(raw, original, body, 1)


def derive(originals, accepted_tree, source_raw, dispatcher_raw, package_raw, predecessor_view_raw):
    g.need(set(originals) == set(SIMPLE) | {SUPPORT, BINDINGS}, 'fixed consumer input roster')
    for name, raw in originals.items():
        g.need(g.gitid('blob', raw) == accepted_tree[name]['sha'], 'unreviewed accepted consumer body')
    manifest = json.loads(source_raw)
    g.need(manifest['reviewed_source_head'] == g.CHECKPOINT['head']
           and manifest['source_only_tree'] == g.CHECKPOINT['tree'], 'exact published source checkpoint')
    result = {}
    for name in SIMPLE:
        body = originals[name]
        if name == UNIT2_TEST:
            result[name] = unit2_test(body)
            continue
        if name == 'scripts/verify_owned_array_native.py':
            # This consumer obtains current identity through the sealed native
            # gate; its historical 227-member authority must remain unchanged.
            view = json.loads(predecessor_view_raw)
            g.need(view['accepted_main'] == g.MAIN and len(view['files']) == 2724,
                   'complete accepted-main native dependency view')
            insertion = ("    successor_view = \"tests/qualification/package_json_current_v1/accepted-view-v1.json\"\n"
                         "    successor_raw, _ = stable_bytes(no_symlink_path(repo / successor_view))\n"
                         "    require(len(successor_raw) == " + str(len(predecessor_view_raw)) +
                         " and sha256(successor_raw) == " + repr(g.sha(predecessor_view_raw)) +
                         ", \"changed fixed alias/Unicode predecessor view\")\n"
                         "    successor = strict_json(successor_raw)\n"
                         "    require(len(successor[\"files\"]) == 2724, \"accepted view membership\")\n"
                         "    names.update(row[\"read_from\"] for row in successor[\"files\"] if row[\"read_from\"] is not None)\n")
            anchor = b'    prefixes = ("src/", "native/",'
            body = exact(body, anchor, insertion.encode() + anchor, 1)
            body = exact(body, b'                "tests/qualification/published_formatter_current/",',
                         b'                "tests/qualification/published_formatter_current/",\n'
                         b'                "tests/qualification/package_json_current_v1/",', 1)
            result[name] = body
            continue
        count = 2 if name == '.github/workflows/ci.yml' else 1
        body = exact(body, g.PREDECESSOR_SHA.encode(), g.sha(source_raw).encode(), count)
        if name == 'scripts/test_bounded_enum_native.py':
            body = exact(body, g.FORMATTTER_HEAD.encode(), g.CHECKPOINT['head'].encode(), 1)
            body = exact(body, b'89e90c3ec83d0712d0037a0407fc6571883c5e9c',
                         g.CHECKPOINT['tree'].encode(), 1)
            body = exact(body,
                         b"self.assertEqual(sorted(name for name in current_map if current_map[name] != published_map[name]),\n"
                         b"                         ['src/main.rs'])",
                         b"self.assertEqual(sorted(name for name in current_map if current_map[name] != published_map[name]),\n"
                         b"                         ['src/main.rs', 'src/runtime/data.rs'])", 1)
        result[name] = body
    value = json.loads(originals[BINDINGS]);value['compiler_checkpoint'] = g.CHECKPOINT['head']
    result[BINDINGS] = g.encoded(value)
    support = originals[SUPPORT]
    for name, value in {'CHECKPOINT': g.CHECKPOINT['head'], 'CURRENT_SOURCE_SHA': g.sha(source_raw),
                        'CURRENT_SOURCE_BYTES': len(source_raw), 'DISPATCHER_SHA': g.sha(dispatcher_raw),
                        'SOURCE_PACKAGE_SHA': g.sha(package_raw), 'CURRENT_MAP_SHA': g.sha(result[BINDINGS])}.items():
        support = assignment(support, name, value)
    result[SUPPORT] = support
    receipt = {'schema': 'oxid-package-json-consumer-derivation-v1', 'source_only': True,
               'execution_qualified': False, 'accepted_main': g.MAIN, 'source_checkpoint': g.CHECKPOINT,
               'source_manifest': g.identity(g.CURRENT_NAME, source_raw),
               'consumers': [{'before': g.identity(n, originals[n]), 'after': g.identity(n, result[n])}
                             for n in sorted(result)]}
    return result, receipt
