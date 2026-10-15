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
OWNED_TEST = 'scripts/test_owned_array_native.py'
RETAINED_TESTS = ('tests/qualification/lexer_reservation_current/test_current_source.py',
                  'tests/qualification/unit4_parser_current/test_current.py',
                  'tests/qualification/unit4_parser_current/test_lexer_reservation.py')
PUBLIC_RUN = 'tests/qualification/unit4_public_v3/run.py'
ADAPTER_TEST = 'scripts/test_lexer_reservation_adapters_current.py'
SIMPLE = (*NATIVE, 'scripts/test_bounded_enum_native.py', 'scripts/verify_owned_array_native.py',
          '.github/workflows/ci.yml', UNIT2_TEST, OWNED_TEST, *RETAINED_TESTS, PUBLIC_RUN, ADAPTER_TEST)


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


def owned_array_test(raw):
    """Give synthetic receipt fixtures both authentic dependency descriptors."""
    start = b"        view_name = 'tests/qualification/published_formatter_current/predecessor-view-v1.json'\n"
    end = b"                    path.write_text('synthetic predecessor dependency, not executed\\n')\n"
    g.need(raw.count(start) == raw.count(end) == 1, 'exact owned fixture closure block')
    a = raw.index(start)
    b = raw.index(end, a) + len(end)
    block = raw[a:b]
    loop = (b"        for view_name in (\n"
            b"                'tests/qualification/published_formatter_current/predecessor-view-v1.json',\n"
            b"                'tests/qualification/package_json_current_v1/accepted-view-v1.json'):\n")
    loop += b''.join(b'    ' + line for line in block[len(start):].splitlines(True))
    raw = exact(raw, block, loop, 1)
    anchor = b'    def test_untracked_compiler_input_cannot_borrow_committed_source_identity(self):\n'
    control = b'''    def test_successor_fixture_closure_rejects_missing_extra_tamper_and_symlink(self):
        self.fixture()
        repo = self.base / 'repo'
        identity = admission.read_json(self.root / 'source/identity.json')
        _, tracked = admission.source_identity(repo, identity['head'])
        expected = admission.capture_sources(repo, tracked)
        descriptor = repo / 'tests/qualification/package_json_current_v1/accepted-view-v1.json'
        view = json.loads(descriptor.read_bytes())
        dependencies = {row['read_from'] for row in view['files'] if row['read_from'] is not None}
        self.assertTrue(dependencies <= set(expected['files']))
        dependency = repo / next(row['read_from'] for row in view['files']
                                 if row['path'] == 'scripts/test_owned_array_native.py')
        extra = descriptor.parent / 'unexpected.py'
        external = self.base / 'descriptor-copy.json'
        external.write_bytes(descriptor.read_bytes())
        for mutation in ('missing-descriptor', 'tampered-descriptor', 'symlink-descriptor',
                         'missing-dependency', 'tampered-dependency', 'extra-member'):
            target = extra if mutation == 'extra-member' else (
                dependency if mutation.endswith('dependency') else descriptor)
            original = None if mutation == 'extra-member' else target.read_bytes()
            try:
                if mutation.startswith('missing'): target.unlink()
                elif mutation == 'symlink-descriptor':
                    target.unlink()
                    target.symlink_to(external)
                else: target.write_bytes((original or b'') + b'changed')
                error = FileNotFoundError if mutation == 'missing-descriptor' else admission.AdmissionError
                with self.subTest(mutation=mutation), self.assertRaises(error):
                    admission.capture_sources(repo, tracked)
            finally:
                if target.exists() or target.is_symlink(): target.unlink()
                if original is not None: target.write_bytes(original)
        self.assertEqual(admission.capture_sources(repo, tracked), expected)

'''
    return exact(raw, anchor, control + anchor, 1)



def retained_test(name, raw, dispatcher_raw):
    """Execute the entire accepted control module in a checked accepted view."""
    count = sum(isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and
                n.name.startswith('test_') for n in ast.walk(ast.parse(raw)))
    header = ('NAME = ' + repr(name) + '\nCOUNT = ' + str(count) + '\nDISPATCHER = ' +
              repr(g.identity(g.PACKAGE + 'run.py', dispatcher_raw)) + '\n')
    return (header + RETAINED_FACADE).encode()


RETAINED_FACADE = r'''"""Retained controls run only after successor admission, at their genuine paths."""
import hashlib
import importlib.util
from pathlib import Path
import sys
import types
import unittest
sys.dont_write_bytecode = True
ROOT = Path(__file__).absolute().parents[3]


class AcceptedControls(unittest.TestSuite):
    def countTestCases(self):
        return COUNT

    def run(self, result, debug=False):
        # Close any preceding discovery module before entering the retained view.
        self._tearDownPreviousClass(None, result)
        self._handleModuleTearDown(result)
        result._previousTestClass = None
        entered = getattr(result, '_testRunEntered', False)
        result._testRunEntered = True
        try:
            path = ROOT / DISPATCHER['path']
            raw = path.read_bytes()
            if path.is_symlink() or len(raw) != DISPATCHER['bytes'] or hashlib.sha256(raw).hexdigest() != DISPATCHER['sha256']:
                raise RuntimeError('changed independently pinned successor dispatcher')
            source = types.ModuleType('accepted_controls_dispatcher')
            source.__file__ = str(path)
            exec(compile(raw, str(path), 'exec'), source.__dict__)
            captured = source.preflight(ROOT)
            view = source._modules['predecessor']
            with view.view(ROOT, captured['package_json_predecessor']['inputs'],
                           captured['package_json_seal']['predecessor_view']) as root:
                directory = (root / NAME).parent
                aliases = {p.stem for p in directory.glob('*.py')}
                saved = {n: sys.modules[n] for n in aliases if n in sys.modules}
                search = list(sys.path)
                try:
                    for n in aliases:
                        sys.modules.pop(n, None)
                    sys.path.insert(0, str(directory))
                    module_name = 'accepted_' + NAME.replace('/', '_').replace('.', '_')
                    module = view.load(root, NAME, module_name)
                    sys.modules[module_name] = module
                    suite = unittest.defaultTestLoader.loadTestsFromModule(module)
                    if suite.countTestCases() != COUNT:
                        raise RuntimeError('retained test roster changed')
                    try:
                        suite.run(result, debug)
                    finally:
                        suite._tearDownPreviousClass(None, result)
                        suite._handleModuleTearDown(result)
                        result._previousTestClass = None
                finally:
                    sys.path[:] = search
                    for n in aliases:
                        sys.modules.pop(n, None)
                    sys.modules.update(saved)
        except Exception:
            case = unittest.FunctionTestCase(lambda: None, description=NAME + ' accepted-view admission/closure')
            result.startTest(case)
            result.addError(case, sys.exc_info())
            result.stopTest(case)
        finally:
            result._testRunEntered = entered
        return result


def load_tests(loader, tests, pattern):
    return AcceptedControls()


if __name__ == '__main__':
    unittest.main()
'''



def adapter_timeout_test(raw):
    """Only the parser lexer test subprocess receives the measured 900s cap."""
    raw = exact(raw, b'def run_source_controls(self, relative):',
                b'def run_source_controls(self, relative, *, timeout=300):', 1)
    raw = exact(raw, b'text=True, timeout=300,', b'text=True, timeout=timeout,', 1)
    return exact(raw,
        b"self.run_source_controls('tests/qualification/unit4_parser_current/test_lexer_reservation.py')",
        b"self.run_source_controls('tests/qualification/unit4_parser_current/test_lexer_reservation.py', timeout=900)", 1)


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
        if name == ADAPTER_TEST:
            result[name] = adapter_timeout_test(body)
            continue
        if name in RETAINED_TESTS:
            result[name] = retained_test(name, body, dispatcher_raw)
            continue
        if name == PUBLIC_RUN:
            nodes = [n for n in ast.parse(body).body if isinstance(n, ast.Assign)
                     and isinstance(n.targets[0], ast.Name) and n.targets[0].id == 'PACKAGE_FILES']
            g.need(len(nodes) == 1, 'exact public adapter membership assignment')
            members = ast.literal_eval(nodes[0].value)
            g.need(not {'package_json_lifecycle.py', 'package-json-lifecycle-v1.json'} & members,
                   'successor adapter members are additions')
            # Sorted tuple makes generated bytes independent of hash seed.
            result[name] = assignment(body, 'PACKAGE_FILES',
                set())
            result[name] = exact(result[name], b'PACKAGE_FILES = set()\n',
                ('PACKAGE_FILES = set(' + repr(tuple(sorted(members | {
                    'package_json_lifecycle.py', 'package-json-lifecycle-v1.json'}))) + ')\n').encode(), 1)
            continue
        if name == UNIT2_TEST:
            result[name] = unit2_test(body)
            continue
        if name == OWNED_TEST:
            result[name] = owned_array_test(body)
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
