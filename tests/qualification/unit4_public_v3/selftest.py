#!/usr/bin/env python3
"""Deterministic source-only adapter controls. Never invokes the candidate."""
import sys
sys.dont_write_bytecode = True
import copy
import contextlib
import io
import json
import gzip
import importlib.util
import os
import sys
import tempfile
import time
import threading
from unittest import mock
from pathlib import Path
import unittest
from types import SimpleNamespace
from contracts import Reject, host, receipt_identity, check_inventory, sha, EXCLUDED_REASON, REMOTE_REASON
from compare import valid_origins, location, diagnostic_vector, policy_bounds, envelope, lifecycle

class SourceAuthorityControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import build
        cls.builder = build
        cls.repo = Path(__file__).resolve().parents[3]
        cls.package = Path(__file__).resolve().parent
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.output = Path(cls.temp.name) / 'prepared'
        with contextlib.redirect_stdout(io.StringIO()):
            build.prepare(SimpleNamespace(
                source_root=cls.repo,
                manifest=cls.repo / 'tests/fixtures/typed_project_source_binding/current-source.json',
                observer_patch=cls.package / 'observer-stdin-v1.patch',
                observer_patch_sha256=build.LIFECYCLE_PATCH_SHA,
                out=cls.output))
        cls.manifest = json.loads((cls.output / 'observer-source.json').read_bytes())

    def test_stdout_current_and_derived_maps_are_exact(self):
        import authority
        original = json.loads((self.repo / 'tests/fixtures/typed_project_source_binding/current-source.json').read_bytes())
        canonical = lambda value: json.dumps(value, sort_keys=True, separators=(',', ':')).encode()
        self.assertEqual(len(original['files']), 262)
        self.assertEqual(sha(canonical(original['files'])), authority.CURRENT_FILES_SHA)
        self.assertEqual(len(self.manifest['files']), 263)
        self.assertEqual(sha(canonical(self.manifest['files'])), authority.OBSERVER_FILES_SHA)
        self.assertEqual(set(self.manifest['changed_paths']), {
            'src/frontend/mod.rs', 'src/frontend/project.rs', 'src/frontend/lexer.rs',
            'src/frontend/parser.rs', 'src/frontend/oir/source/sealed.rs',
            'src/frontend/declaration_index/sealed.rs', 'src/frontend/driver.rs',
            'src/frontend/lifecycle_observer.rs'})
        self.assertEqual(json.loads((self.output / 'prepared.json').read_bytes())['compiler_invocations'], 0)

    def test_lifecycle_successor_restores_exact_historical_patch(self):
        current = (self.package / 'observer-stdin-v1.patch').read_bytes()
        projected = (self.package / 'observer-combined-v1.patch').read_bytes()
        self.assertEqual(sha(projected), self.builder.PROJECTED_LIFECYCLE_PATCH_SHA)
        historical = (self.repo / 'tests/fixtures/typed_project_unit4_independent/components/lifecycle/observer-additive-v1.patch').read_bytes()
        self.assertEqual(self.builder.verify_lifecycle_successor(current), historical)
        for changed in (current + b'\n', current.replace(b'EnumSyntaxPolicy', b'OtherSyntaxPolicy'),
                        current.replace(b'parse_attempt', b'other_attempt')):
            with self.subTest(changed=sha(changed)), self.assertRaises(Reject):
                self.builder.verify_lifecycle_successor(changed)

    def test_hooks_cover_current_execution_bodies_once(self):
        source = self.output / 'source/src/frontend'
        parser = (source / 'parser.rs').read_text()
        parser_body = parser.split('fn parse_counted_with_policies(', 1)[1]
        self.assertEqual(parser.count('event("parse_attempt", source.path())'), 1)
        self.assertEqual(parser.count('event("parse_complete", source.path())'), 1)
        self.assertIn('event("parse_attempt", source.path());\n    *storage = enums::SyntaxStorage::default();', parser_body)
        self.assertIn('if diagnostics.is_empty() {\n        crate::frontend::lifecycle_observer::event("parse_complete", source.path());', parser_body)
        index = (source / 'declaration_index/sealed.rs').read_text()
        self.assertEqual(index.count('event("index_attempts", "")'), 1)
        self.assertIn('event("index_attempts", "");\n    work.restrict(limits.work);', index.split("fn collect<'s>(", 1)[1])
        checked = (source / 'oir/source/sealed.rs').read_text()
        self.assertIn('if !ast.enums.is_empty() {\n        crate::frontend::lifecycle_observer::event("route_completions", "owned");', checked)
        self.assertIn('else if ast.uses_owned_syntax(source) {\n        crate::frontend::lifecycle_observer::event("route_completions", "owned");', checked)

class EnumQualifiedPathsControls(unittest.TestCase):
    repo = Path(__file__).resolve().parents[3]
    amendment_root = repo / 'tests/fixtures/typed_project_source_binding'

    @classmethod
    def setUpClass(cls):
        helper = cls.amendment_root / 'enum_enabled_qualified_values_v1.py'
        spec = importlib.util.spec_from_file_location('enum_qualified_paths_controls', helper)
        cls.helper = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.helper)
        cls.manifest = cls.repo / 'tests/fixtures/typed_project_source_binding/enum-source.json'
        cls.execution_manifest = cls.repo / 'tests/fixtures/typed_project_source_binding/current-source.json'
        cls.corpus = cls.repo / 'tests/fixtures/typed_project_unit2_independent/semantic/corpus.jsonl.gz'
        cls.descriptor = cls.amendment_root / 'enum-enabled-qualified-values-v1.json'
        transport_root = cls.repo / 'tests/fixtures/typed_project_unit4_contracts'
        spec = importlib.util.spec_from_file_location('enum_qualified_paths_transport', transport_root / 'transport.py')
        transport = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(transport)
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        destination = Path(cls.temp.name) / 'transport'
        manifest = transport.materialize(transport_root, destination)
        paths = {row['logical_path']: destination / row['archive_path'] for row in manifest['members']}
        public = next(path for name, path in paths.items() if name.endswith('/predecessor-public-projections-replacement-v3.json.gz'))
        cls.public_rows = json.loads(gzip.decompress(public.read_bytes()))['rows']
        cls.contracts = SimpleNamespace(root=public.parent, tables={'predecessors': {'rows': cls.public_rows}},
            verified=[{'path': str(path), 'authority_path': name} for name, path in paths.items()])

    def amendment(self):
        return self.helper.Amendment(self.manifest, self.corpus, self.descriptor)

    def item(self, amendment, case_id):
        return {'case': case_id, 'mode': 'project-parser',
                'result': copy.deepcopy(amendment.rows[case_id]['current_unit2_result']),
                'program_read_source_entries': [{'display_file': 'root.ox'}]}

    def tuple(self, case_id, profile='debug'):
        case = next(row for row in self.public_rows if row['family'] == 'Unit2' and row['id'] == case_id)
        return {'case': copy.deepcopy(case), 'profile': profile, 'host': 'Linux x86_64',
                'execution_host': 'Linux x86_64', 'scope': 'execute',
                'argv_template': ['{oxid}', 'check', 'root.ox', '--edition', 'typed-preview', '--message-format=json']}

    def public_process(self, amendment, case_id, *, historical=False):
        row = amendment.rows[case_id]
        expected = row['old_public_expected'] if historical else row['current_public_expected']
        origin = row['source_derived_public_diagnostic']['primary']
        primary = {'file_id': 0, **origin, 'line': 1, 'column': origin['start'] + 1,
                   'end_line': 1, 'end_column': origin['end'] + 1}
        diagnostic = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'diagnostic',
                      'severity': 'error', **expected['first_diagnostic'], 'message': 'arbitrary nonempty prose',
                      'primary': primary, 'secondary': [], 'notes': []}
        summary = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'check-summary',
                   'success': False, 'errors': 1, 'functions': None}
        stdout = ''.join(json.dumps(value) + '\n' for value in (diagnostic, summary))
        return {'status': 1, 'timed_out': False, 'stdout': stdout, 'stderr': '',
                'stdout_sha256': sha(stdout.encode()), 'stderr_sha256': sha(b'')}

    def test_four_loader_rows_are_closed_and_inputs_unchanged(self):
        amendment = self.amendment()
        before = copy.deepcopy(amendment.corpus)
        for case_id in self.helper.CASE_IDS:
            row = amendment.corpus[case_id]
            item = self.item(amendment, case_id)
            original = copy.deepcopy(item)
            failures = []
            got = amendment.project_unit2(row, copy.deepcopy(row['expected']), item, failures)
            self.assertEqual(failures, [])
            self.assertIs(got['parser_accepts'], True)
            self.assertIs(got['load_accepts'], True)
            self.assertNotIn('first_diagnostic', got)
            self.assertEqual(item, original)
        receipt = amendment.unit2_report([{'case': case_id} for case_id in amendment.corpus])
        self.assertEqual(receipt['cases'], list(self.helper.CASE_IDS))
        self.assertEqual(amendment.corpus, before)
        ordinary = next(row for row in amendment.corpus.values() if row['cohort'] == 'parser' and row['id'] not in self.helper.CASE_IDS)
        expected = copy.deepcopy(ordinary['expected'])
        self.assertEqual(amendment.project_unit2(ordinary, expected, {'case': ordinary['id']}, []), expected)

    def test_loader_diagnostics_route_reads_and_all_result_fields_fail_closed(self):
        changes = [lambda item: item['result'].__setitem__('diagnostics', [{'code': 'E0202', 'stage': 'resolve'}]),
                   lambda item: item['result'].__setitem__('actual_phases', ['collection']),
                   lambda item: item['result'].__setitem__('signature_starts', [{}]),
                   lambda item: item['result'].__setitem__('record_starts', [{}]),
                   lambda item: item['result'].__setitem__('load_ok', 1),
                   lambda item: item['result'].__setitem__('parser_ok', True),
                   lambda item: item['result'].__setitem__('syntax_flavor', 'ProjectSyntax'),
                   lambda item: item['result'].__setitem__('content_files_retained', ['other.ox']),
                   lambda item: item['result'].__setitem__('human', 'failure'),
                   lambda item: item.__setitem__('mode', 'project'),
                   lambda item: item.pop('program_read_source_entries'),
                   lambda item: item['program_read_source_entries'].append({'display_file': 'child.ox'}),
                   lambda item: item['program_read_source_entries'].append({'display_file': 'root.ox'}),
                   lambda item: item['result'].__setitem__('extra', True)]
        for index, change in enumerate(changes):
            with self.subTest(index=index):
                amendment = self.amendment()
                case_id = self.helper.CASE_IDS[0]
                row = amendment.corpus[case_id]
                item = self.item(amendment, case_id)
                change(item)
                failures = []
                amendment.project_unit2(row, row['expected'], item, failures)
                self.assertTrue(failures)

    def test_source_descriptor_corpus_and_whole_row_drift_reject(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, original in (('source', self.manifest), ('corpus', self.corpus), ('descriptor', self.descriptor)):
                changed = root / name
                changed.write_bytes(original.read_bytes() + b'\n')
                args = [self.manifest, self.corpus, self.descriptor]
                args[('source', 'corpus', 'descriptor').index(name)] = changed
                with self.subTest(name=name), self.assertRaises(ValueError): self.helper.Amendment(*args)
            coherent = json.loads(self.descriptor.read_bytes())
            coherent['rows'][0]['current_public_expected']['first_diagnostic']['code'] = 'E9999'
            changed = root / 'coherent.json'
            changed.write_text(json.dumps(coherent))
            with self.assertRaises(ValueError): self.helper.Amendment(self.manifest, self.corpus, changed)
        changes = [lambda row: row.__setitem__('mode', 'project'),
                   lambda row: row['sources'][0].__setitem__('base64', 'ZGlmZmVyZW50Cg=='),
                   lambda row: row['expected'].__setitem__('parser_accepts', True),
                   lambda row: row.__setitem__('id', 'parser/other'),
                   lambda row: row.__setitem__('extra', True)]
        for change in changes:
            amendment = self.amendment()
            case_id = self.helper.CASE_IDS[0]
            row = copy.deepcopy(amendment.corpus[case_id])
            change(row)
            with self.assertRaises(ValueError): amendment.project_unit2(row, row['expected'], self.item(amendment, case_id), [])

    def test_missing_duplicate_and_extra_rows_reject(self):
        amendment = self.amendment()
        case_id = self.helper.CASE_IDS[0]
        row = amendment.corpus[case_id]
        amendment.project_unit2(row, row['expected'], self.item(amendment, case_id), [])
        with self.assertRaises(ValueError): amendment.project_unit2(row, row['expected'], self.item(amendment, case_id), [])
        with self.assertRaises(ValueError): amendment.unit2_report([{'case': name} for name in amendment.corpus])
        amendment.admit_public_rows(self.public_rows)
        selected = [row for row in self.public_rows if row['family'] == 'Unit2' and row['id'] in self.helper.CASE_IDS]
        for rows in (selected[:-1], selected + [selected[0]], [{**selected[0], 'kind': 'complete-check-projection'}] + selected[1:]):
            with self.assertRaises(ValueError): amendment.admit_public_rows(rows)
        complete = self.amendment()
        for name in self.helper.CASE_IDS:
            row = complete.corpus[name]
            complete.project_unit2(row, row['expected'], self.item(complete, name), [])
        results = [{'case': name} for name in complete.corpus]
        for rows in (results[:-1], results + [results[0]], results + [{'case': 'invented'}]):
            with self.assertRaises(ValueError): complete.unit2_report(rows)

    def test_public_amendment_preserves_first_code_stage_domain(self):
        from predecessors import Predecessors
        current = Predecessors(self.contracts, self.execution_manifest, self.amendment_root)
        historical = Predecessors(self.contracts)
        policy = {'applies_to': []}
        before = copy.deepcopy(current.rows)
        for case_id in self.helper.CASE_IDS:
            for profile in ('debug', 'release'):
                row = self.tuple(case_id, profile)
                process = self.public_process(current._qualified_paths, case_id)
                original = copy.deepcopy(process)
                report = current.qualified_paths_comparison(row, process)
                self.assertEqual(report['historical']['status'], 'mismatch')
                self.assertEqual(report['current']['status'], 'match')
                self.assertEqual(current.compare(row, process, policy), report)
                self.assertEqual(process, original)
                with self.assertRaises(Reject): historical.compare(row, process, policy)
                with self.assertRaises(Reject): current.compare(row, process, policy, historical=True)
                old = self.public_process(current._qualified_paths, case_id, historical=True)
                historical.compare(row, old, policy)
                with self.assertRaises(Reject): current.compare(row, old, policy)
                # Prose and later sequence are still excluded; source origins remain validated.
                lines = [json.loads(line) for line in process['stdout'].splitlines()]
                lines[0]['message'] = 'different explanatory prose'
                lines[0]['primary'] = {**lines[0]['primary'], 'start': 0, 'end': 2, 'column': 1, 'end_column': 3}
                lines.insert(1, copy.deepcopy(lines[0]))
                lines[-1]['errors'] = 2
                process['stdout'] = ''.join(json.dumps(value) + '\n' for value in lines)
                process['stdout_sha256'] = sha(process['stdout'].encode())
                current.compare(row, process, policy)
        self.assertEqual(current.rows, before)
        self.assertIsNone(historical.qualified_paths_amendment)
        self.assertEqual(len(current.qualified_paths_amendment['rows']), 4)

    def test_semantic_and_execution_sources_have_distinct_exact_roles(self):
        from predecessors import Predecessors
        import authority
        current = Predecessors(self.contracts, self.execution_manifest, self.amendment_root)
        receipt = current.qualified_paths_amendment
        self.assertEqual(receipt['source_manifest']['sha256'], authority.ENUM_SOURCE_SHA)
        self.assertEqual(receipt['source_manifest']['members'], 237)
        self.assertEqual(receipt['execution_source_manifest']['sha256'], authority.CURRENT_SOURCE_SHA)
        self.assertEqual(receipt['execution_source_manifest']['members'], 262)
        self.assertEqual(self.helper.sha(self.manifest.read_bytes()), authority.ENUM_SOURCE_SHA)
        with self.assertRaisesRegex(Reject, 'current execution source identity'):
            Predecessors(self.contracts, self.manifest, self.amendment_root)
        with self.assertRaisesRegex(ValueError, 'source manifest'):
            self.helper.Amendment(self.execution_manifest, self.corpus, self.descriptor)

    def test_public_source_expected_host_profile_and_operation_drift_reject(self):
        amendment = self.amendment()
        case_id = self.helper.CASE_IDS[0]
        corpus = amendment.corpus[case_id]
        sources = {'root.ox': amendment.rows[case_id]['source']['text'].encode()}
        diagnostics = [{'code': 'E0202', 'stage': 'resolve'}]
        changes = [lambda row: row['case'].__setitem__('authority_id', 'other'),
                   lambda row: row.__setitem__('profile', 'other'),
                   lambda row: row.__setitem__('host', 'macOS arm64'),
                   lambda row: row.__setitem__('execution_host', 'macOS arm64'),
                   lambda row: row.__setitem__('scope', 'excluded-host'),
                   lambda row: row['argv_template'].__setitem__(1, 'run')]
        for change in changes:
            row = self.tuple(case_id)
            change(row)
            with self.assertRaises(ValueError): amendment.public_comparison(row, corpus, corpus['expected'], sources, 1, diagnostics)
        with self.assertRaises(ValueError): amendment.public_comparison(self.tuple(case_id), corpus, corpus['expected'], {'root.ox': b'changed'}, 1, diagnostics)
        with self.assertRaises(ValueError): amendment.public_comparison(self.tuple(case_id), corpus, {}, sources, 1, diagnostics)
        for status, first in ((0, []), (1, []), (1, [{'code': 'E0101', 'stage': 'parse'}]), (1, [{'code': 'E0202', 'stage': 'type'}])):
            self.assertEqual(amendment.public_comparison(self.tuple(case_id), corpus, corpus['expected'], sources, status, first)['current']['status'], 'mismatch')

def process_gone_or_zombie(info):
    try:
        return info.read_text().split()[2] == 'Z'
    except (FileNotFoundError, ProcessLookupError):
        return True

class ComparatorTests(unittest.TestCase):
    def test_source_inventory_uses_exact_path_key_order(self):
        from runtime import source_map
        rows = [{'path': 'm1', 'kind': 'directory'}, {'path': 'm1/m2.ox', 'kind': 'file', 'bytes': 1, 'sha256': 'nested'}, {'path': 'm1.ox', 'kind': 'file', 'bytes': 2, 'sha256': 'parent'}, {'path': 'root.ox', 'kind': 'file', 'bytes': 3, 'sha256': 'root'}]
        got = source_map(rows)
        self.assertEqual([r['path'] for r in got], ['m1.ox', 'm1/m2.ox', 'root.ox'])
        self.assertEqual({r['path']: (r['bytes'], r['sha256']) for r in got}, {'m1.ox': (2, 'parent'), 'm1/m2.ox': (1, 'nested'), 'root.ox': (3, 'root')})

    def test_original_manifest_platform_newline_transport_is_narrow(self):
        from predecessors import original_manifest_transport
        expected = b'{\n  "value": 1\n}\n'
        windows = expected.replace(b'\n', b'\r\n')
        self.assertEqual(original_manifest_transport(windows, expected, 'nt'), expected)
        self.assertEqual(original_manifest_transport(expected, expected, 'posix'), expected)
        for raw, platform_name in ((windows, 'posix'), (windows.replace(b'1', b'2'), 'nt'), (windows + b'\r', 'nt')):
            with self.assertRaises(Reject): original_manifest_transport(raw, expected, platform_name)

    def test_aliases_and_architecture_are_exact(self):
        self.assertEqual(host('Darwin', 'aarch64'), 'macOS arm64')
        self.assertEqual(host('Windows', 'AMD64'), 'Windows x86_64')
        self.assertNotEqual(host('Linux', 'arm64'), 'Linux x86_64')

    def test_location_end_fields_are_required_when_frozen(self):
        actual = {'file_id': 0, 'path': 'm.ox', 'start': 0, 'end': 2, 'line': 1, 'column': 1, 'end_line': 1, 'end_column': 3}
        expected = {'file': 0, 'path': 'm.ox', 'start': 0, 'end': 2, 'line': 1, 'scalar_column': 1, 'end_line': 1, 'end_scalar_column': 3}
        location(actual, expected)
        altered = dict(actual, end_column=2)
        with self.assertRaises(Reject): location(altered, expected)
        expected.pop('end_line'); expected.pop('end_scalar_column')
        location(altered, expected)

    def test_literal_message_is_not_model_prose(self):
        got = {'code': 'E0100', 'stage': 'parse', 'primary': None, 'secondary': [], 'message': 'known'}
        want = {'code': 'E0100', 'stage': 'parse', 'location': None, 'related_locations': [], 'model_message': 'not literal'}
        diagnostic_vector([got], [want])
        want['literal_message'] = 'different'
        with self.assertRaises(Reject): diagnostic_vector([got], [want])

    def test_utf8_byte_bounds(self):
        policy = {'applies_to': [{'code': 'E0400', 'stage': 'source-project'}], 'message_utf8_bytes_max': 1024, 'secondary_label_count_max': 2, 'secondary_label_message_utf8_bytes_max': 256, 'note_count_max': 2, 'note_utf8_bytes_max': 256}
        got = {'code': 'E0400', 'stage': 'source-project', 'message': 'é' * 512, 'secondary': [], 'notes': []}
        policy_bounds([got], policy)
        got['message'] += 'é'
        with self.assertRaises(Reject): policy_bounds([got], policy)

    def test_partial_rows_still_validate_every_source_origin(self):
        sources = {'main.ox': 'é\nfn'.encode()}
        origin = {'file_id': 0, 'path': 'main.ox', 'start': 3, 'end': 5, 'line': 2, 'column': 1, 'end_line': 2, 'end_column': 3}
        diagnostic = {'primary': origin, 'secondary': [], 'notes': []}
        valid_origins([diagnostic], sources)
        with self.assertRaises(Reject):
            valid_origins([{**diagnostic, 'secondary': [{'span': None, 'message': 'unbound'}]}], sources)
        for field, value in [('start', 1), ('path', 'absent.ox'), ('end', 99), ('end_column', 2), ('file_id', 2)]:
            bad = copy.deepcopy(diagnostic)
            bad['primary'][field] = value
            with self.assertRaises((Reject, UnicodeDecodeError)): valid_origins([bad], sources)

    def test_failed_predicate_cannot_become_aggregate_success(self):
        from run import aggregate
        row = {'key': 'K', 'contract_identity': 'v3', 'group': 'literal', 'case_id': 'x', 'observation_index': 0, 'profile': 'debug', 'host': 'Linux x86_64', 'execution_host': 'Linux x86_64', 'required_hosts': ['Linux x86_64'], 'scope': 'execute', 'format': 'json', 'argv_template': ['{oxid}', 'check'], 'required_capabilities': []}
        got = {**receipt_identity(row), 'executed': True, 'status': 'fail'}
        with self.assertRaises(Reject): aggregate([row], [got])

    def test_unavailable_required_capability_is_incomplete(self):
        from run import aggregate
        row = {'key': 'K', 'contract_identity': 'v3', 'group': 'literal', 'case_id': 'x', 'observation_index': 0, 'profile': 'debug', 'host': 'Linux x86_64', 'execution_host': 'Linux x86_64', 'required_hosts': ['Linux x86_64'], 'scope': 'execute', 'format': 'json', 'argv_template': ['{oxid}', 'check'], 'required_capabilities': []}
        passed = {**receipt_identity(row), 'executed': True, 'status': 'pass'}
        capability_row = {**row, 'key': 'C', 'case_id': 'child-unreadable', 'required_capabilities': ['posix_regular_file_mode_denies_open_for_effective_identity']}
        from contracts import CAPABILITY_REASON
        gap = {**receipt_identity(capability_row), 'executed': False, 'status': 'unsupported-capability', 'reason': CAPABILITY_REASON,
               'capability_proof': {'read_open_succeeded': True, 'mode_before_candidate': 0, 'source_restored': True}}
        got = aggregate([row, capability_row], [passed, gap])
        self.assertEqual(got['status'], 'INCOMPLETE_LOCAL_CAPABILITY')
        self.assertEqual(got['executed'], 1)
        self.assertEqual(got['unsupported_capability'], 1)
        self.assertFalse(got['global_host_qualification'])
        false_pass = dict(gap, status='pass')
        with self.assertRaises(Reject): aggregate([row, capability_row], [passed, false_pass])

    def test_inventory_mutations_fail_closed(self):
        row = {'key': 'K', 'contract_identity': 'v3', 'group': 'literal', 'case_id': 'x', 'observation_index': 0, 'profile': 'debug', 'host': 'Linux x86_64', 'execution_host': 'Linux x86_64', 'required_hosts': ['Linux x86_64'], 'scope': 'execute', 'format': 'json', 'argv_template': ['{oxid}', 'check'], 'required_capabilities': []}
        good = {**receipt_identity(row), 'executed': True, 'status': 'pass'}
        check_inventory([row], [good])
        variants = [[], [good, good], [{**good, 'key': 'other'}], [{**good, 'profile': 'release'}], [{**good, 'host': 'macOS arm64'}], [{**good, 'executed': False}], [{**good, 'status': 'unsupported-capability', 'capability_proof': {}}]]
        for variant in variants:
            with self.assertRaises(Reject): check_inventory([row], variant)

class ProcessControls(unittest.TestCase):
    def test_complete_capture_and_stream_limit(self):
        from runtime import process
        with tempfile.TemporaryDirectory() as temp:
            good = process([sys.executable, '-c', "print('ok')"], temp, os.environ, timeout=5, limit=64)
            self.assertEqual(good['stdout'], 'ok\n')
            self.assertEqual(good['status'], 0)
            bad = process([sys.executable, '-c', "import sys; sys.stdout.write('x'*1000000)"], temp, os.environ, timeout=5, limit=64)
            self.assertTrue(bad['stream_limit_exceeded'])
            self.assertEqual(len(bad['stdout']), 64)

    def test_timeout_when_child_closes_output(self):
        from runtime import process
        with tempfile.TemporaryDirectory() as temp:
            got = process([sys.executable, '-c', "import os,time; os.close(1); os.close(2); time.sleep(30)"], temp, os.environ, timeout=0.2)
            self.assertTrue(got['timed_out'])

    def test_reader_error_is_reported_and_cleanup_is_bounded(self):
        import runtime
        read = os.read
        def injected(fd, count):
            if threading.current_thread().name.startswith('unit4-pipe-reader-'):
                raise OSError(5, 'injected pipe read error')
            return read(fd, count)
        with tempfile.TemporaryDirectory() as temp:
            started = time.monotonic()
            with mock.patch('runtime.os.read', side_effect=injected):
                with self.assertRaisesRegex(OSError, 'reader failure'):
                    runtime.process([sys.executable, '-c', 'import time; time.sleep(30)'], temp, os.environ, timeout=2)
            self.assertLess(time.monotonic() - started, 5)

    def test_success_cleans_closed_pipe_descendant(self):
        from runtime import process
        with tempfile.TemporaryDirectory() as temp:
            script = "import subprocess,sys; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL); print(p.pid,flush=True)"
            got = process([sys.executable, '-c', script], temp, os.environ, timeout=3)
            self.assertEqual(got['status'], 0)
            self.assertFalse(got['timed_out'])
            pid = int(got['stdout'].strip())
            if os.name != 'nt' and Path('/proc').exists():
                info = Path('/proc') / str(pid) / 'stat'
                deadline = time.monotonic() + 2
                while not process_gone_or_zombie(info) and time.monotonic() < deadline:
                    time.sleep(0.02)
                self.assertTrue(process_gone_or_zombie(info))

    def test_timeout_cleans_descendant_pipe(self):
        from runtime import process
        with tempfile.TemporaryDirectory() as temp:
            script = "import subprocess,sys,time; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); print(p.pid,flush=True); time.sleep(30)"
            started = time.monotonic()
            got = process([sys.executable, '-c', script], temp, os.environ, timeout=0.3, limit=64)
            self.assertTrue(got['timed_out'])
            self.assertLess(time.monotonic() - started, 5)
            pid = int(got['stdout'].strip())
            if os.name != 'nt' and Path('/proc').exists():
                info = Path('/proc') / str(pid) / 'stat'
                self.assertTrue(process_gone_or_zombie(info))

if __name__ == '__main__':
    unittest.main()
