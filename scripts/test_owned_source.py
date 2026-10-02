"""Independent source model and process-harness tests; no LLVM processes."""
import hashlib
import os
import sys
from pathlib import Path
import unittest
import owned_source_model as m

ROOT = Path(__file__).resolve().parents[1]


class SourceRendererTests(unittest.TestCase):
    def test_batch_exact_rfc_source(self):
        rfc = (ROOT / "rfcs/0014-owned-structs-call-borrows.md").read_text()
        section = rfc.split("## 5. Integrated pilot:", 1)[1]
        expected = section.split("```text\n", 1)[1].split("```", 1)[0]
        rendered = m.render(m.batch_program())
        self.assertEqual(rendered.source, expected)
        self.assertEqual((ROOT / "fixtures/owned_source/batch.ox").read_text(), expected)
        self.assertEqual(rendered.sha256, hashlib.sha256(expected.encode()).hexdigest())

    def test_batch_arithmetic_is_independent(self):
        ledger = m.batch_ledger()
        self.assertEqual((ledger["retries"], ledger["commits"], ledger["checksum"], ledger["result"]), (6, 6, 210, 816))
        self.assertEqual(len(ledger["state_transitions"]), 12)
        self.assertIsNone(ledger["source_fuel"])

    def test_byte_origins_crlf_unicode_and_borrow_trivia(self):
        b = m.Builder()
        loan = b.borrow("x", True, trivia=" /*雪🦀*/ ")
        call = b.call("f", loan)
        program = m.Program((), (m.Function("main", (), "()", b.block(b.discard(call), b.ret())),), "// 雪🦀\r\n", "\r\n")
        rendered = m.render(program)
        encoded = rendered.source.encode()
        start, end = rendered.origins[loan.key]
        self.assertEqual(encoded[start:end].decode(), "&mut /*雪🦀*/ x")
        self.assertEqual(encoded[slice(*rendered.origins[loan.key + ".name"])], b"x")
        self.assertEqual(start, len(rendered.source[:rendered.source.index("&mut")].encode()))

    def test_shared_ast_nodes_rejected(self):
        b = m.Builder()
        value = b.i(0)
        with self.assertRaisesRegex(ValueError, "shared or label duplicated"):
            m.render(m.Program((), (m.Function("main", (), "i32", b.block(b.discard(value), b.ret(value))),)))


class SourceSemanticsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases = {case.identifier: case for case in m.corpus()}
        cls.expectations = {key: m.case_expectation(case) for key, case in cls.cases.items()}

    def test_every_case_is_unique_deterministic_and_finite(self):
        self.assertEqual(len(self.cases), 431)
        self.assertEqual(len({value['source_sha256'] for value in self.expectations.values()}), len(self.cases))
        for key, case in self.cases.items():
            self.assertEqual(self.expectations[key], m.case_expectation(case))

    def test_alias_partition_cardinality_and_modes_are_not_permutations(self):
        self.assertEqual([len(list(m.alias_partitions(n))) for n in range(1, 5)], [1, 2, 5, 15])
        aliases = [value for value in self.expectations.values() if value['family'] == '07-alias-partitions']
        self.assertEqual(len(aliases), 290)
        self.assertEqual(sum(value['expected']['status'] == 'accept' for value in aliases), 74)
        for value in aliases:
            if value['expected']['status'] == 'reject':
                self.assertEqual((value['expected']['stage'], value['expected']['code']), ('ownership', 'E0311'))

    def test_availability_saturates_backedges_and_checks_constant_arms(self):
        for name in ('continue-move-restore-0', 'join-move-0-restore-0', 'join-move-1-restore-0'):
            self.assertEqual(self.expectations[name]['expected']['code'], 'E0310')
        for name in ('continue-move-restore-1', 'join-move-0-restore-1', 'join-move-1-restore-1', 'moved-terminating-arm'):
            self.assertEqual(self.expectations[name]['expected']['status'], 'accept')
        value = self.expectations['continue-move-restore-0']['static']['functions']['main']
        self.assertGreater(value['states'], value['points'])

    def test_saturation_ceiling_is_incomplete_not_acceptance(self):
        c = m.NamesTypes(m.batch_program()).check()
        graph = m.SourceGraph(c, c.functions['main'])
        with self.assertRaisesRegex(ValueError, 'MODEL_INCOMPLETE'):
            graph.saturate(state_ceiling=1)

    def test_snapshot_and_move_acquire_order(self):
        self.assertEqual(self.expectations['snapshot-order-0']['expected']['result'], 3)
        self.assertEqual(self.expectations['snapshot-order-1']['expected']['code'], 'E0311')
        self.assertEqual(self.expectations['move-loan-order-0']['expected']['code'], 'E0310')
        self.assertEqual(self.expectations['move-loan-order-1']['expected']['code'], 'E0311')

    def test_capability_tree_shared_children_and_permission(self):
        self.assertEqual(self.expectations['shared-children-exclusive-parent']['expected']['result'], 8)
        self.assertEqual(self.expectations['reborrow-shared-exclusive']['expected']['code'], 'E0313')
        self.assertEqual(self.expectations['reference-write-shared']['expected']['code'], 'E0313')
        for name in ('reborrow-shared-shared', 'reborrow-exclusive-shared', 'reborrow-exclusive-exclusive', 'reference-write-exclusive'):
            self.assertEqual(self.expectations[name]['expected']['status'], 'accept')

    def test_lazy_control_points_keep_outer_loans_static_but_skip_dynamic_rhs(self):
        for flag in (0, 1):
            self.assertEqual(self.expectations[f'lazy-nested-conflict-{flag}']['expected']['code'], 'E0311')
            value = self.expectations[f'lazy-dynamic-effects-{flag}']
            self.assertEqual(value['expected']['result'], flag)
            self.assertEqual(len(value['dynamic']['writes']), flag)

    def test_rhs_before_field_destination(self):
        failed = self.expectations['field-target-after-rhs-move']
        self.assertEqual(failed['expected']['code'], 'E0310')
        start, end = failed['expected']['span']
        self.assertEqual(failed['source'].encode()[start:end], b'x.value')
        self.assertEqual(self.expectations['field-target-after-inner-mutation']['expected']['result'], 2)
        self.assertEqual(self.expectations['whole-replace-nested-constructor']['expected']['result'], 1)

    def test_generation_reuse_and_prefix_cleanup(self):
        value = self.expectations['cleanup-prefix-continue']
        events = value['dynamic']['events']
        first = [event for event in events if event['kind'] == 'let-owned']
        self.assertEqual(len(first), 2)
        self.assertEqual([event['owner'][-1] for event in first], [1, 2])
        ended = [event for event in events if event['kind'] == 'lexical-end' and event['owner'][1] == first[0]['owner'][1]]
        self.assertEqual(len(ended), 2)
        self.assertFalse(any('later' in str(event) for event in ended))
        self.assertEqual(self.expectations['cleanup-prefix-return']['expected']['result'], 17)

    def test_batch_matches_hand_state_and_template_ledgers(self):
        value = self.expectations['batch']
        self.assertEqual(value['expected']['result'], 816)
        writes = value['dynamic']['writes']
        self.assertEqual([event['value'] for event in writes if event['field'] == 'retries'], [1, 2, 3, 4, 5, 6])
        self.assertEqual([event['value'] for event in writes if event['field'] == 'checksum'], [10, 30, 60, 100, 150, 210])
        self.assertEqual(len(writes), 19)
        self.assertEqual(value['dynamic']['activations'], 27)
        expected_frames = {'retry': (4, 0, 0, 0, 1, 0, 0, 12), 'commit': (10, 0, 0, 0, 1, 0, 0, 18),
                           'dispatch': (4, 2, 0, 0, 1, 1, 1, 28), 'done': (3, 0, 0, 0, 1, 0, 0, 11),
                           'relay': (0, 0, 8, 2, 0, 0, 0, 16), 'finish': (7, 0, 4, 1, 0, 0, 0, 15),
                           'main': (26, 6, 32, 8, 0, 3, 5, 142)}
        for fn, expected in expected_frames.items():
            self.assertEqual(tuple(value['schedule']['frames'][fn][key] for key in ('S', 'A', 'P', 'O', 'R', 'L', 'C', 'X')), expected)
        self.assertEqual(value['schedule']['total_fuel'], 1297)
        self.assertEqual(len(value['schedule']['items']), 506)
        operations = m.Counter(item['operation'] for item in value['schedule']['items'])
        self.assertEqual(dict(operations), {'Root': 1, 'Scalar': 249, 'StorageLive': 6, 'Construct': 1,
            'MoveInitialize': 5, 'StorageEnd': 8, 'Goto': 25, 'ReadField': 39, 'Branch': 36,
            'OpenCall': 26, 'PrepareBorrow': 24, 'Invoke': 26, 'WriteField': 19,
            'ReturnScalar': 26, 'PrepareScalar': 12, 'PrepareOwned': 2, 'ReturnOwned': 1})

    def test_every_lower_batch_budget_has_exact_next_operation_and_no_unpaid_write(self):
        value = self.expectations['batch']
        schedule, dynamic = value['schedule'], value['dynamic']
        for budget in range(schedule['total_fuel']):
            result = m.TemplateScheduler.at_budget(schedule, dynamic, budget)
            next_event = next(item for item in schedule['items'] if item['end_fuel'] > budget)
            self.assertEqual(result['failure']['code'], 'E0601')
            self.assertEqual(result['failure']['origin'], next_event['origin'])
            self.assertEqual(result['failure']['operation'], next_event['operation'])
            self.assertTrue(all(write['index'] < next_event['semantic_event'] for write in result['writes']))
        self.assertEqual(m.TemplateScheduler.at_budget(schedule, dynamic, 1297)['result'], 816)

    def test_replacement_commits_after_replace_charge_before_cleanup_charge(self):
        value = self.expectations['replace-self']
        schedule, dynamic = value['schedule'], value['dynamic']
        operation = next(item for item in schedule['items'] if item['operation'] == 'Replace')
        before = m.TemplateScheduler.at_budget(schedule, dynamic, operation['start_fuel'])
        after = m.TemplateScheduler.at_budget(schedule, dynamic, operation['end_fuel'])
        self.assertEqual(before['committed_writes'], [])
        self.assertEqual([event['kind'] for event in after['committed_writes']], ['replace-owned'])
        self.assertEqual(after['failure']['operation'], 'StorageEnd')

    def test_overflow_keeps_earlier_write_and_omits_later_store(self):
        value = self.expectations['overflow-after-write']
        self.assertEqual(value['expected']['runtime_failure']['code'], 'E0604')
        self.assertEqual([(event['field'], event['value']) for event in value['dynamic']['writes']], [('value', 1)])
        self.assertFalse(value['schedule']['complete_execution'])

    def test_source_mutability_precedes_ownership(self):
        for name in ('immutable-field-write', 'immutable-exclusive-borrow'):
            self.assertEqual((self.expectations[name]['expected']['stage'], self.expectations[name]['expected']['code']), ('type', 'E0304'))

    def test_phase_precedence_name_errors_win_over_earlier_type_error(self):
        b = m.Builder()
        program = m.Program((m.Record('Empty'),), (m.Function('main', (), 'i32', b.block(
            b.let('wrong', b.b(True), annotation='i32'), b.ret(b.v('absent')))),))
        with self.assertRaises(m.Rejected) as error: m.NamesTypes(program).check()
        self.assertEqual((error.exception.diagnostic.stage, error.exception.diagnostic.code), ('resolve', 'E0200'))

    def test_raw_valid_producer_mutations_detected_by_correspondence(self):
        import copy
        facts = self.expectations['batch']['facts']
        mutants = []
        changed = copy.deepcopy(facts)
        immutable = next(binding for binding in changed['bindings'] if not binding['mutable'] and binding['type'] == 'Batch')
        immutable['mutable'] = True; mutants.append(('binding mutability', changed))
        changed = copy.deepcopy(facts)
        changed['records'][0]['identity'] = 'other-same-shaped-record'; mutants.append(('nominal identity', changed))
        changed = copy.deepcopy(facts)
        changed['records'][0]['fields'][0]['identity'] = 'record.0.field.1'; mutants.append(('field identity', changed))
        changed = copy.deepcopy(facts)
        changed['bindings'][0]['type'] = '&Batch'; mutants.append(('parameter mode', changed))
        changed = copy.deepcopy(facts)
        changed['bindings'][0]['position'] = 1; mutants.append(('parameter position', changed))
        changed = copy.deepcopy(facts)
        scalar = next(arg for call in changed['calls'] for arg in call['arguments'] if arg['mode'] == 'scalar')
        scalar['snapshot_expression'] = 'reread-after-later-argument'; mutants.append(('scalar snapshot', changed))
        for name, changed in mutants:
            with self.subTest(name=name): self.assertTrue(m.correspondence_differences(facts, changed))
        self.assertEqual(m.correspondence_differences(facts, copy.deepcopy(facts)), [])

    def test_origins_and_causes_survive_bounded_display_names(self):
        for length in (64, 65, 900):
            b = m.Builder(); name = 'x' * length
            literal = b.lit('S')
            program = m.Program((m.Record('S', ((name, 'i32'),)),), (m.Function('main', (), 'i32', b.block(b.let('s', literal), b.ret(b.i(0)))),))
            value = m.case_expectation(m.Case('long', '14-diagnostics', program))
            self.assertEqual(value['expected']['span'], value['origins'][literal.key])
            self.assertLessEqual(len(value['expected']['message'].encode()), 1024)
        for character in ('é', '雪', '🦀'):
            for count in range(20, 70):
                text = character * count
                bounded = m.bounded_text(text)
                self.assertLessEqual(len(bounded.encode()), 64)
                self.assertEqual(bounded, text if len(text.encode()) <= 64 else text.encode()[:61].decode(errors='ignore') + '...')


class ExtendedSemanticsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls): cls.values = {case.identifier: m.case_expectation(case) for case in m.corpus()}

    def test_written_field_order_and_construction_after_every_field(self):
        self.assertEqual(self.values['written-field-effects-0']['expected']['result'], 12)
        self.assertEqual(self.values['written-field-effects-1']['expected']['result'], 21)
        value = self.values['literal-overflow-before-construction']
        self.assertEqual([write['value'] for write in value['dynamic']['writes']], [1])
        self.assertFalse(any(event['kind'] == 'construct' and event['record'] == 'Pair' for event in value['dynamic']['events']))

    def test_parent_access_and_nested_children(self):
        self.assertEqual(self.values['parent-access-shared-read']['expected']['result'], 5)
        self.assertEqual(self.values['parent-access-exclusive-read']['expected']['code'], 'E0311')
        self.assertEqual(self.values['parent-access-shared-exclusive-child']['expected']['code'], 'E0311')

    def test_batch_variants_do_not_replace_the_exact_pilot(self):
        for suffix, code in (('conditional-move', 'E0310'), ('continue-carried-move', 'E0310'), ('alias-conflict', 'E0311'),
                             ('nested-reborrow-conflict', 'E0311'), ('use-after-relay', 'E0310')):
            self.assertEqual(self.values['batch-' + suffix]['expected']['code'], code)
        self.assertEqual(self.values['batch-shared-after-reinitialization']['expected']['result'], 816)
        failure = self.values['batch-overflow-after-earlier-write']
        self.assertEqual(failure['expected']['runtime_failure']['code'], 'E0604')
        self.assertEqual([(write['field'], write['value']) for write in failure['dynamic']['writes']], [('retries', 1), ('completed', 1)])

    def test_interleaved_parameters_keep_original_positions_and_empty_identity(self):
        value = self.values['interleaved-parameter-positions']
        bindings = [binding for binding in value['facts']['bindings'] if binding['function'] == 'interleave']
        self.assertEqual([(binding['type'], binding['position']) for binding in bindings], [('i32', 0), ('S', 1), ('&S', 2), ('bool', 3)])
        self.assertEqual(value['expected']['result'], 6)
        self.assertEqual(self.values['empty-between-scalars']['expected']['result'], 8)

    def test_reject_unparenthesized_ast_with_different_source_meaning(self):
        b = m.Builder()
        expr = b.op(b.op(b.i(1), '+', b.i(2)), '*', b.i(3))
        with self.assertRaisesRegex(ValueError, 'explicit Group'):
            m.render(m.Program((), (m.Function('main', (), 'i32', b.block(b.ret(expr))),)))

    def test_source_state_bound_never_proves_infinite_execution(self):
        b = m.Builder()
        program = m.Program((m.Record('Unused'),), (m.Function('main', (), 'i32', b.block(b.loop(b.b(True), b.block(b.cont())), b.ret(b.i(0)))),))
        checked = m.NamesTypes(program).check()
        self.assertTrue(m.static_check(checked)['accepted'])
        result = m.Machine(checked, step_bound=30).run()
        self.assertEqual(result['failure']['code'], 'MODEL_BOUND')


@unittest.skipUnless(os.name == "posix", "source native harness requires POSIX process groups")
class SourceHarnessTests(unittest.TestCase):
    def setUp(self):
        import tempfile
        import io
        self.temporary = tempfile.TemporaryDirectory(prefix='oxid-owned-source-harness-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = io.BytesIO()
        self.worker = self.root / 'worker.py'
        self.worker.write_text('''import fcntl,json,os,pathlib,signal,subprocess,sys,time
root,name,mode=pathlib.Path(sys.argv[1]),sys.argv[2],sys.argv[3]
with (root/'events').open('a') as f:
 fcntl.flock(f,fcntl.LOCK_EX);f.write(json.dumps(['start',name,os.getpid()])+'\\n');f.flush()
print(name+' stdout',flush=True);print(name+' stderr',file=sys.stderr,flush=True)
if mode=='wait':
 signal.signal(signal.SIGTERM,signal.SIG_IGN)
 child=subprocess.Popen([sys.executable,'-c','import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);time.sleep(60)'])
 (root/(name+'.pids')).write_text(str(os.getpid())+' '+str(child.pid))
 time.sleep(60)
if mode=='fail': print(name+': PASS',flush=True);sys.exit(23)
if mode=='empty': sys.exit(0)
if mode=='signal': os.kill(os.getpid(),signal.SIGTERM)
if mode=='large': print('x'*200000)
if mode=='overlap':
 while len((root/'events').read_text().splitlines())<2:time.sleep(.01)
 time.sleep(.15 if name=='first' else .02)
with (root/'events').open('a') as f:
 fcntl.flock(f,fcntl.LOCK_EX);f.write(json.dumps(['end',name,os.getpid()])+'\\n');f.flush()
print(name+': PASS',flush=True)
''')

    def job(self, name, mode='ok'):
        import sys
        import verify_owned_source as h
        return h.Job(name, (sys.executable, str(self.worker), str(self.root), name, mode), name + ': PASS')

    def run_jobs(self, jobs, workers=2, timeout=5):
        import verify_owned_source as h
        return h.run_jobs(jobs, self.root / 'evidence', jobs=workers, timeout=timeout, output=self.output)

    def test_two_worker_bound_preserves_full_ordered_logs(self):
        import json
        jobs = [self.job('first', 'overlap'), self.job('second', 'overlap'), self.job('third', 'large')]
        self.assertEqual(self.run_jobs(jobs), 0)
        active, peak = set(), 0
        for action, name, _ in map(json.loads, (self.root / 'events').read_text().splitlines()):
            if action == 'start': active.add(name); peak = max(peak, len(active))
            else: active.remove(name)
        self.assertEqual(peak, 2)
        self.assertFalse(active)
        text = self.output.getvalue()
        self.assertLess(text.index(b'=== first ==='), text.index(b'=== second ==='))
        self.assertLess(text.index(b'=== second ==='), text.index(b'=== third ==='))
        self.assertIn(b'x' * 200000, text)
        self.assertIn(b'first stdout\nfirst stderr', text)
        self.assertEqual(len(list((self.root / 'evidence').glob('*.log'))), 3)

    def test_terminal_status_beats_false_pass_string(self):
        self.assertEqual(self.run_jobs([self.job('bad', 'fail'), self.job('pending')], workers=1), 23)
        self.assertIn(b'pending: NOT RUN', self.output.getvalue())

    def test_zero_exit_without_summary_fails(self):
        self.assertEqual(self.run_jobs([self.job('empty', 'empty')]), 1)
        self.assertIn(b'missing checked terminal summary', self.output.getvalue())

    @unittest.skipUnless(sys.platform.startswith("linux") and Path("/proc").is_dir(), "descendant liveness proof requires Linux /proc")
    def test_timeout_kills_worker_and_descendant(self):
        import time
        self.assertEqual(self.run_jobs([self.job('wait', 'wait')], timeout=.2), 124)
        for pid in map(int, (self.root / 'wait.pids').read_text().split()):
            for _ in range(100):
                try: content = Path(f'/proc/{pid}/stat').read_text()
                except (FileNotFoundError, ProcessLookupError): break
                if content.split()[2] == 'Z': break
                time.sleep(.01)
            else: self.fail('descendant remained running')

    def test_worker_signal_and_invalid_parallelism_are_failures(self):
        import signal
        self.assertEqual(self.run_jobs([self.job('signal', 'signal')]), 128 + signal.SIGTERM)
        for workers in (0, 3):
            with self.assertRaises(ValueError): self.run_jobs([self.job('x')], workers=workers)

    def test_launch_failure_retains_checked_failure(self):
        import verify_owned_source as h
        self.assertEqual(self.run_jobs([h.Job('missing', ('/definitely-not-an-executable',), 'missing: PASS')]), 1)
        self.assertIn(b'orchestration failure', self.output.getvalue())

    def test_json_summary_and_diagnostic_contract(self):
        import subprocess
        import json
        import verify_owned_source as h
        records = [{'schema_version': 1, 'kind': 'diagnostic', 'code': 'E0310', 'stage': 'ownership', 'primary': {'start': 7, 'end': 8}},
                   {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'run-summary', 'success': False, 'errors': 1, 'result': None}]
        output = b'\n'.join(json.dumps(item).encode() for item in records) + b'\n'
        completed = subprocess.CompletedProcess([], 1, output, b'')
        self.assertEqual(h.parse_records(completed), records)
        h.match_diagnostic(records, {'code': 'E0310', 'stage': 'ownership', 'span': [7, 8]})
        with self.assertRaises(ValueError): h.match_diagnostic(records, {'code': 'E0311', 'stage': 'ownership', 'span': [7, 8]})
        with self.assertRaises(ValueError): h.parse_records(subprocess.CompletedProcess([], 0, output, b''))
        with self.assertRaises(ValueError): h.parse_records(subprocess.CompletedProcess([], 1, output, b'error'))

    def test_freeze_binds_model_source_and_expected_facts_before_observation(self):
        import verify_owned_source as h
        directory = self.root / 'frozen'
        frozen = h.freeze(directory)
        self.assertEqual(frozen['unique_source_programs'], 431)
        self.assertEqual(h.verify_frozen(directory), frozen)
        source = directory / 'batch.ox'
        source.write_text(source.read_text() + '\n')
        with self.assertRaisesRegex(ValueError, 'source changed'): h.verify_frozen(directory)


class DiagnosticBoundaryTests(unittest.TestCase):
    def test_origin_only_mutation_does_not_change_classification(self):
        pairs = (
            (m.Denial('Unavailable', 'MoveInitialize', 'source', 'local', 'moved', 'temporary'), 'E0310'),
            (m.Denial('Unavailable', 'MoveInitialize', 'source', 'local', 'moved', 'local'), 'E0500'),
            (m.Denial('Unavailable', 'MoveInitialize', 'destination', 'local', 'moved', 'temporary'), 'E0500'),
            (m.Denial('Unavailable', 'ReadField', 'base', 'parameter', 'moved'), 'E0310'),
            (m.Denial('Unavailable', 'ReadField', 'base', 'local', 'dead'), 'E0500'),
            (m.Denial('Unavailable', 'PrepareOwned', 'staged-input', 'temporary', 'moved'), 'E0500'),
            (m.Denial('Unavailable', 'ReturnOwned', 'return-value', 'call-result', 'moved'), 'E0500'),
            (m.Denial('LoanConflict', 'MoveInitialize', 'source', 'local', counterpart='temporary'), 'E0311'),
            (m.Denial('LoanConflict', 'StorageEnd', 'storage', 'local'), 'E0500'),
            (m.Denial('LoanConflict', 'MoveInitialize', 'destination', 'local'), 'E0500'),
            (m.Denial('LoanConflict', 'Construct', 'destination', 'temporary'), 'E0500'),
            (m.Denial('LoanConflict', 'ReadField', 'base', 'local', permission_validated=True), 'E0311'),
            (m.Denial('LoanConflict', 'ReadField', 'base', 'reference-parameter', granted='exclusive', permission_validated=True), 'E0311'),
            (m.Denial('LoanConflict', 'WriteField', 'base', 'reference-parameter', granted='shared'), 'E0500'),
            (m.Denial('LoanConflict', 'Replace', 'destination', 'local', mutable=True, permission_validated=True), 'E0311'),
            (m.Denial('LoanConflict', 'Replace', 'source', 'temporary', mutable=True, permission_validated=True), 'E0500'),
            (m.Denial('LoanConflict', 'PrepareBorrow', 'authority', 'local', requested='exclusive', permission_validated=True), 'E0311'),
            (m.Denial('Permission', 'WriteField', 'base', 'reference-parameter', granted='shared'), 'E0313'),
            (m.Denial('Permission', 'WriteField', 'base', 'local', mutable=False), 'E0500'),
            (m.Denial('Permission', 'WriteField', 'base', 'reference-parameter', granted='exclusive'), 'E0500'),
            (m.Denial('Permission', 'PrepareBorrow', 'authority', 'reference-parameter', granted='shared', requested='exclusive'), 'E0313'),
            (m.Denial('Permission', 'PrepareBorrow', 'authority', 'reference-parameter', granted='shared', requested='shared'), 'E0500'),
            (m.Denial('Lifetime', 'StorageLive', 'storage', 'local'), 'E0500'),
            (m.Denial('CallRegion', 'Invoke', 'call', 'call'), 'E0500'),
        )
        for denial, expected in pairs:
            self.assertEqual(m.classify_denial(denial), expected)
            # Origin spans are deliberately absent from classification inputs.
            self.assertEqual(m.classify_denial(m.replace(denial)), expected)

    def test_retained_component_inclusive_boundaries_and_amplification(self):
        for bound in (3, 16, 64, 256, 1024):
            self.assertEqual(m.bounded_text('x' * bound, bound), 'x' * bound)
            self.assertEqual(m.bounded_text('x' * (bound + 1), bound), 'x' * (bound - 3) + '...')
        payload = m.diagnostic_payload('x' * 2000, ['y' * 1000] * 3, ['z' * 1000] * 3)
        self.assertEqual((len(payload['labels']), len(payload['notes']), payload['bytes']), (2, 2, 2048))
        source, fields = m.amplification_source(100)
        self.assertLess(len(source.encode()), 1048576)
        message = m.missing_field_message(fields)
        self.assertIn('1016 more omitted', message['message'])
        self.assertLessEqual(message['bytes'], 1024)
        self.assertLessEqual(message['bytes'] * 100, 204800)
        self.assertEqual(len(m.missing_field_message(fields[:8])['message'].split(', ')), 8)
        self.assertIn('1 more omitted', m.missing_field_message(fields[:9])['message'])
        self.assertNotIn('omitted', m.missing_field_message(fields[:8])['message'])
        self.assertEqual(m.amplification_source(101)[0].count(' = Wide {};'), 101)

    def test_exact_excluded_token_spans_are_frozen(self):
        wanted = {'reference-local': '&', 'temporary-literal-borrow': '{', 'temporary-call-borrow': '(',
                  'field-borrow': '.', 'parenthesized-borrow': '&', 'parenthesized-place': '(',
                  'arbitrary-deref': '*', 'group-projection': '.', 'chained-projection': '.',
                  'shorthand-literal': '}', 'update-literal': '.', 'trailing-call-comma': ')',
                  'condition-literal': ':', 'reference-field': '&', 'reference-result': '&',
                  'mutable-parameter': 'mut', 'trailing-parameter-comma': ')'}
        for case in m.excluded_cases():
            value = m.case_expectation(case)
            self.assertEqual(value['expected']['span_match'], 'exact')
            start, end = value['expected']['span']
            self.assertEqual(value['source'].encode()[start:end].decode(), wanted[case.identifier.removeprefix('excluded-')])

    def test_parameter_modes_have_no_implicit_coercion(self):
        for case in m.extended_cases():
            if case.identifier.startswith('borrow-mode-'):
                value = m.case_expectation(case)
                self.assertEqual((value['expected']['stage'], value['expected']['code']), ('type', 'E0300'))


class ReviewedScalarRegressionTests(unittest.TestCase):
    def case(self, b, body, result='i32'):
        return m.case_expectation(m.Case('held-out', 'scalar-contract', m.Program((m.Record('Unused'),), (m.Function('main', (), result, b.block(*body)),))))

    def test_signed_range_resolution_is_distinct_from_runtime_overflow(self):
        for number in (m.MIN_I32 - 1, m.MAX_I32 + 1, 10 ** 100):
            b = m.Builder(); literal = b.i(number)
            value = self.case(b, [b.ret(literal)])
            self.assertEqual((value['expected']['stage'], value['expected']['code'], value['expected']['span']), ('resolve', 'E0203', value['origins'][literal.key]))
        for number in (m.MIN_I32, m.MAX_I32):
            b = m.Builder(); self.assertEqual(self.case(b, [b.ret(b.i(number))])['expected']['result'], number)

    def test_unsupported_scalar_syntax_cannot_become_dynamic_semantics(self):
        for operator in ('-', '+'):
            b = m.Builder()
            value = self.case(b, [b.let('x', b.i(1)), b.ret(b.unary(operator, b.v('x')))])
            self.assertEqual((value['expected']['stage'], value['expected']['code']), ('parse', 'E0101'))
        for operator in ('/', '%'):
            b = m.Builder(); value = self.case(b, [b.ret(b.op(b.i(7), operator, b.i(3)))])
            self.assertEqual((value['expected']['stage'], value['expected']['code']), ('parse', 'E0101'))

    def test_noncanonical_not_and_comparison_ast_require_explicit_groups(self):
        b = m.Builder()
        with self.assertRaisesRegex(ValueError, 'Group'):
            self.case(b, [b.ret(b.unary('!', b.op(b.b(True), '&&', b.b(False))))], 'bool')
        for left_operator, outer in (('<', '=='), ('==', '<'), ('<', '<')):
            b = m.Builder()
            with self.assertRaisesRegex(ValueError, 'Group'):
                self.case(b, [b.ret(b.op(b.op(b.i(1), left_operator, b.i(2)), outer, b.i(3)))], 'bool')
        b = m.Builder(); value = self.case(b, [b.ret(b.unary('!', b.group(b.op(b.b(True), '&&', b.b(False)))))], 'bool')
        self.assertEqual(value['expected']['result'], True)
        self.assertEqual(value['schedule']['total_fuel'], 14)

    def test_type_mismatch_points_at_first_wrong_operand(self):
        b = m.Builder(); wrong = b.b(True)
        value = self.case(b, [b.ret(b.op(wrong, '+', b.i(1)))])
        self.assertEqual(value['expected']['span'], value['origins'][wrong.key])
        b = m.Builder(); wrong = b.b(True)
        value = self.case(b, [b.ret(b.op(b.i(1), '<', wrong))], 'bool')
        self.assertEqual(value['expected']['span'], value['origins'][wrong.key])

    def test_incomplete_machine_run_cannot_enter_expected_compiler_results(self):
        from unittest import mock
        b = m.Builder()
        original = m.Machine
        with mock.patch.object(m, 'Machine', lambda checked: original(checked, step_bound=20)):
            with self.assertRaisesRegex(ValueError, 'MODEL_INCOMPLETE'):
                self.case(b, [b.loop(b.b(True), b.block(b.cont())), b.ret(b.i(0))])

    def test_human_and_elf_must_each_match_expected_error(self):
        import subprocess
        import tempfile
        import verify_owned_source as h
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'source.ox'; source.write_text('fn main() -> i32 { return 2147483647 + 1; }\n')
            start = source.read_bytes().index(b'+')
            item = {'expected': {'result': None, 'result_type': 'i32', 'runtime_failure': {'code': 'E0604', 'origin': 'plus'}}, 'origins': {'plus': [start, start + 1]}}
            success = subprocess.CompletedProcess([], 0, b'42\n', b'')
            with self.assertRaises(ValueError): h.verify_text_result(success, item, source)
            correct = subprocess.CompletedProcess([], 1, b'', f'error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> source.ox:1:{start+1}\n'.encode())
            h.verify_text_result(correct, item, source)
            with self.assertRaises(ValueError): h.verify_text_result(subprocess.CompletedProcess([], 1, b'42\n', correct.stderr), item, source)
            with self.assertRaises(ValueError): h.verify_text_result(subprocess.CompletedProcess([], 1, b'', correct.stderr.replace(b'E0604', b'E0601')), item, source)

    def test_only_real_matching_command_summary_can_pass(self):
        import json
        import subprocess
        import verify_owned_source as h
        for kind, operation in (('invented-summary', None), ('check-summary', 'run')):
            value = {'schema_version': 1, 'kind': kind, 'success': True}
            with self.assertRaises(ValueError): h.parse_records(subprocess.CompletedProcess([], 0, (json.dumps(value) + '\n').encode(), b''), operation)


class RenderDomainAndFactTests(unittest.TestCase):
    def test_identifiers_and_complete_item_order_are_model_invariants(self):
        for name in ('雪', 'true', 'x-y', '1x', ''):
            b = m.Builder(); p = m.Program((m.Record('S'),), (m.Function('main', (), 'i32', b.block(b.let(name, b.i(7)), b.ret(b.v(name)))),))
            with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.render(p)
            with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.NamesTypes(p).check()
        b = m.Builder(); p = m.Program((m.Record('S'),), (m.Function('main', (), 'i32', b.block(b.ret(b.i(7)))),))
        for order in ((('record', 0),), (('record', 0), ('function', 2)), (('function', 0), ('function', 0))):
            with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.render(m.replace(p, order=order))
        self.assertIn('fn main', m.render(m.replace(p, order=(('function', 0), ('record', 0)))).source)

    def test_trivia_cannot_inject_or_comment_out_source(self):
        b = m.Builder(); p = m.Program((m.Record('S'),), (m.Function('main', (), 'i32', b.block(b.ret(b.i(7)))),))
        for preamble in ('fn fake() -> i32 { return 9; }', '// comment without terminator', '/* unterminated'):
            with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.render(m.replace(p, preamble=preamble))
        for newline in ('', '; fn injected', '\nreturn 1;'):
            with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.render(m.replace(p, newline=newline))
        self.assertIn('// 雪🦀\r\n', m.render(m.replace(p, preamble='// 雪🦀\r\n')).source)

    def test_borrow_trivia_preserves_keyword_and_place_token_boundaries(self):
        for trivia in ("", "// hidden", "/* unterminated"):
            b = m.Builder(); call = b.call("f", b.borrow("x", True, trivia=trivia))
            p = m.Program((), (m.Function("main", (), "()", b.block(b.discard(call), b.ret())),))
            with self.assertRaisesRegex(ValueError, "MODEL_DOMAIN"): m.render(p)
        for trivia in (" ", "/*雪*/", "//comment\n"):
            b = m.Builder(); call = b.call("f", b.borrow("x", True, trivia=trivia))
            p = m.Program((), (m.Function("main", (), "()", b.block(b.discard(call), b.ret())),))
            self.assertIn("&mut" + trivia + "x", m.render(p).source)
        b = m.Builder(); call = b.call("f", b.borrow("p", True, True, trivia=""))
        p = m.Program((), (m.Function("main", (), "()", b.block(b.discard(call), b.ret())),))
        self.assertIn("&mut*p", m.render(p).source)

    def test_condition_literals_require_explicit_group_or_call_argument(self):
        b = m.Builder(); condition = b.lit('S')
        p = m.Program((m.Record('S'),), (m.Function('main', (), 'i32', b.block(b.iff(condition, b.block(b.ret(b.i(1)))), b.ret(b.i(0)))),))
        with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.render(p)
        b = m.Builder(); p = m.Program((m.Record('S'),), (m.Function('main', (), 'i32', b.block(b.iff(b.group(b.lit('S')), b.block(b.ret(b.i(1)))), b.ret(b.i(0)))),))
        result = m.case_expectation(m.Case('grouped', 'domain', p))
        self.assertEqual((result['expected']['stage'], result['expected']['code']), ('type', 'E0300'))

    def test_recursive_execution_depth_is_incomplete(self):
        b = m.Builder(); p = m.Program((m.Record('S'),), (m.Function('main', (), 'i32', b.block(b.ret(b.call('main')))),))
        with self.assertRaisesRegex(ValueError, 'MODEL_INCOMPLETE'): m.case_expectation(m.Case('recursive', 'domain', p))

    def test_projection_literal_and_unused_store_facts_are_frozen(self):
        import copy
        b = m.Builder()
        unused = m.Function('unused', (('p', '&mut S'),), '()', b.block(b.write('p', 'a', b.i(8)), b.ret()))
        main = m.Function('main', (), 'i32', b.block(b.let('x', b.lit('S', ('b', b.i(2)), ('a', b.i(1)))), b.ret(b.f('x', 'a'))))
        value = m.case_expectation(m.Case('facts', 'facts', m.Program((m.Record('S', (('a', 'i32'), ('b', 'i32'))),), (unused, main))))
        facts = value['facts']
        self.assertEqual(facts['projections'][0]['field_identity'], 'record.0.field.0')
        self.assertEqual([field['field_identity'] for field in facts['literals'][0]['written_fields']], ['record.0.field.1', 'record.0.field.0'])
        store = next(store for store in facts['stores'] if store['function'] == 'unused')
        self.assertEqual(store['projection']['field_identity'], 'record.0.field.0')
        for category in ('projections', 'literals', 'stores'):
            changed = copy.deepcopy(facts)
            if category == 'projections': changed[category][0]['field_identity'] = 'record.0.field.1'
            elif category == 'literals': changed[category][0]['written_fields'].reverse()
            else: changed[category][0]['projection']['field_identity'] = 'record.0.field.1'
            self.assertTrue(m.correspondence_differences(facts, changed))

    def test_ordinary_vs_unsupported_syntax_codes_follow_token_contract(self):
        unsupported = {'reference-local', 'field-borrow', 'parenthesized-borrow', 'group-projection', 'chained-projection', 'update-literal', 'reference-field', 'reference-result'}
        for case in m.excluded_cases():
            expected = m.case_expectation(case)['expected']
            suffix = case.identifier.removeprefix('excluded-')
            self.assertEqual(expected['code'], 'E0101' if suffix in unsupported else 'E0100', suffix)


class IndependentPathOutcomeTests(unittest.TestCase):
    def test_profile_rejects_each_bad_text_path_after_valid_json_failure(self):
        import contextlib
        import hashlib
        import io
        import json
        import subprocess
        import tempfile
        from unittest import mock
        import verify_owned_source as h
        for bad_path in ('human', 'native', None):
            with self.subTest(path=bad_path), tempfile.TemporaryDirectory() as directory:
                root = Path(directory); binary = root / 'compiler'; binary.write_bytes(b'fake-test-only')
                corpus = root / 'corpus'; corpus.mkdir()
                source = 'struct S {}\nfn main() -> i32 { return 2147483647 + 1; }\n'
                start = source.encode().index(b'+'); line = 2; column = len(source[:source.index('+')].rsplit('\n', 1)[-1]) + 1
                wanted_stderr = f'error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> source.ox:{line}:{column}\n'.encode()
                (corpus / 'overflow.ox').write_text(source)
                item = {'function_count': 1, 'source_sha256': hashlib.sha256(source.encode()).hexdigest(),
                        'expected': {'status': 'accept', 'result': None, 'result_type': 'i32', 'runtime_failure': {'code': 'E0604', 'origin': 'plus'}},
                        'origins': {'plus': [start, start + 1]}}
                (corpus / 'overflow.json').write_text(json.dumps(item))
                calls = []
                def fake_run(argv, **kwargs):
                    if argv[0] != str(binary):
                        calls.append('native')
                        return subprocess.CompletedProcess(argv, 0, b'42\n', b'') if bad_path == 'native' else subprocess.CompletedProcess(argv, 1, b'', wanted_stderr)
                    operation = argv[1]
                    if operation == 'run' and '--message-format=json' not in argv:
                        calls.append('human')
                        return subprocess.CompletedProcess(argv, 0, b'42\n', b'') if bad_path == 'human' else subprocess.CompletedProcess(argv, 1, b'', wanted_stderr)
                    calls.append(operation)
                    records = []; code = 1 if operation == 'run' else 0
                    if code: records.append({'schema_version': 1, 'kind': 'diagnostic', 'stage': 'oir-run', 'code': 'E0604', 'primary': {'start': start, 'end': start + 1}})
                    if operation == 'compile': Path(argv[argv.index('--output') + 1]).write_bytes(b'\x7fELFsynthetic-test-artifact')
                    summary = {'schema_version': 1, 'edition': 'typed-preview', 'kind': operation + '-summary', 'success': not code, 'errors': code}
                    if operation == 'check': summary['functions'] = 1
                    if operation == 'run': summary['result'] = None
                    if operation == 'compile': summary['output'] = argv[argv.index('--output') + 1]
                    records.append(summary)
                    return subprocess.CompletedProcess(argv, code, b''.join((json.dumps(value) + '\n').encode() for value in records), b'')
                with mock.patch.object(h, 'verify_frozen', return_value={'files': [{'id': 'overflow'}]}), mock.patch.object(h.subprocess, 'run', side_effect=fake_run), contextlib.redirect_stdout(io.StringIO()):
                    if bad_path is None: h.profile_worker(binary, 'debug', corpus, root / 'evidence')
                    else:
                        with self.assertRaisesRegex(ValueError, 'runtime failure must exit 1'):
                            h.profile_worker(binary, 'debug', corpus, root / 'evidence')
                self.assertEqual('native' in calls, bad_path != 'human')


class FinalSummaryContractTests(unittest.TestCase):
    def test_summary_sequence_and_json_types_are_exact(self):
        import copy
        import json
        import subprocess
        import verify_owned_source as h
        success = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'run-summary', 'success': True, 'errors': 0, 'result': {'type': 'i32', 'value': 7}}
        diagnostic = {'schema_version': 1, 'kind': 'diagnostic', 'code': 'E0604', 'stage': 'oir-run', 'primary': {'start': 1, 'end': 2}}
        def parse(records, code=0):
            return h.parse_records(subprocess.CompletedProcess([], code, b''.join((json.dumps(value) + '\n').encode() for value in records), b''), 'run')
        parse([success])
        for records in ([success, success], [diagnostic, success], [{'schema_version': 1, 'kind': 'progress'}, success]):
            with self.assertRaises(ValueError): parse(records)
        for key, invalid in (('schema_version', True), ('success', 1), ('errors', False)):
            mutant = copy.deepcopy(success); mutant[key] = invalid
            with self.assertRaises(ValueError): parse([mutant])
        for result in ({'type': 'i32', 'value': True}, {'type': 'bool', 'value': 1}, {'type': 'unit', 'value': None}):
            mutant = copy.deepcopy(success); mutant['result'] = result
            with self.assertRaises(ValueError): parse([mutant])
        failure = {**success, 'success': False, 'errors': 1, 'result': None}
        parse([diagnostic, failure], 1)
        for key in ('start', 'end'):
            mutant = copy.deepcopy(diagnostic); mutant['primary'][key] = True
            with self.assertRaises(ValueError): parse([mutant, failure], 1)

    def test_unknown_reference_referent_uses_exact_name_origin(self):
        b = m.Builder(); p = m.Program((m.Record("S"),), (m.Function("f", (("p", "&mut Missing"),), "()", b.block(b.ret())),))
        value = m.case_expectation(m.Case("unknown", "names", p))
        self.assertEqual((value["expected"]["stage"], value["expected"]["code"]), ("resolve", "E0202"))
        start, end = value["expected"]["span"]
        self.assertEqual(value["source"].encode()[start:end], b"Missing")

    def test_context_invalid_reference_types_are_outside_tagged_domain(self):
        b = m.Builder()
        fn = m.Function('main', (), 'i32', b.block(b.ret(b.i(0))))
        for records in ((m.Record('S'), m.Record('T', (('p', '&S'),))),
                        (m.Record('S'), m.Record('S'), m.Record('T', (('p', '&S'),)))):
            p = m.Program(records, (fn,))
            with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.case_expectation(m.Case('refs', 'domain', p))
        b = m.Builder(); p = m.Program((m.Record('S'),), (m.Function('bad', (), '&S', b.block(b.ret())),))
        with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.render(p)
        b = m.Builder(); p = m.Program((m.Record('S'),), (m.Function('main', (), 'i32', b.block(b.let('x', b.lit('S'), annotation='&S'), b.ret(b.i(0)))),))
        with self.assertRaisesRegex(ValueError, 'MODEL_DOMAIN'): m.render(p)


class CandidateReceiptTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import json
        import verify_owned_source as h
        cls.h = h
        cls.fixture_path = ROOT / 'tests/fixtures/owned_source/producer-translations.json'
        cls.data = json.loads(cls.fixture_path.read_text())
        cls.row = cls.data['cases'][0]

    def test_actual_nine_raw_valid_translations_are_detected(self):
        report = self.h.compare_translation_controls(self.fixture_path)
        self.assertEqual((report['mutations'],report['unique_original_sources']), (9,6))
        self.assertTrue(all(row['failed_facts'] and row['debug_release_identical'] for row in report['cases']))

    def test_candidate_protocol_rejects_scalar_type_confusion_and_lost_fields(self):
        import copy,json
        base=json.loads(self.row['receipt_json'])['original'];item=self.row['expected'];source=self.row['source'].encode()
        variants=[]
        for key,value in [('accepted',1),('accepted',1.0),('diagnostics',{})]:
            v=copy.deepcopy(base);v['check'][key]=value;variants.append(v)
        for key,value in [('result',True),('result',6.0),('result_type','invented')]:
            v=copy.deepcopy(base);v['reference'][key]=value;variants.append(v)
        v=copy.deepcopy(base);v['frames']['main']['S']=True;variants.append(v)
        v=copy.deepcopy(base);v['charge_events'][0]['cost']=True;variants.append(v)
        v=copy.deepcopy(base);v['raw_view']['functions'][0]['owners'][0]['id']=False;variants.append(v)
        v=copy.deepcopy(base);v['raw_view']['functions'][0]['owners'][0]['record']=0.0;variants.append(v)
        v=copy.deepcopy(base);v['raw_view']['records'][0]['span'][0]=True;variants.append(v)
        v=copy.deepcopy(base);del v['raw_view'];variants.append(v)
        self.h.validate_candidate_receipt(base,item,source)
        for value in variants:
            with self.subTest(value=value):
                with self.assertRaises(ValueError):self.h.validate_candidate_receipt(value,item,source)

    def test_candidate_protocol_rejects_outcome_inconsistency(self):
        import copy,json
        base=json.loads(self.row['receipt_json'])['original']
        for mutate in (lambda v:v['check'].update(accepted=False),
                       lambda v:v['reference'].update(failure={'code':'E0604','span':[0,1]})):
            v=copy.deepcopy(base);mutate(v)
            with self.assertRaises(ValueError):self.h.validate_candidate_receipt(v,self.row['expected'],self.row['source'].encode())

    def test_raw_authority_schema_rejects_invalid_ids_and_operation_roles(self):
        import copy,json
        base=json.loads(self.row['receipt_json'])['original']
        variants=[]
        for identity in (-1,False,0.0,1):
            value=copy.deepcopy(base);value['raw_view']['functions'][0]['blocks'][0]['id']=identity;variants.append(value)
        for operation in ('UNRECOGNIZED','Scalar'):
            value=copy.deepcopy(base);value['raw_view']['functions'][0]['blocks'][0]['terminator']['instruction']['operation']=operation;variants.append(value)
        value=copy.deepcopy(base);del value['raw_view']['functions'][0]['blocks'][0]['terminator']['instruction']['value'];variants.append(value)
        for kind,constant in [('UNKNOWN',6),('I32',2**31),('I32',-(2**31)-1)]:
            value=copy.deepcopy(base);value['raw_view']['functions'][0]['blocks'][0]['instructions'][0]['instruction']['scalar']['value'].update(kind=kind,value=constant);variants.append(value)
        for value in variants:
            with self.assertRaises(ValueError):self.h.validate_candidate_receipt(value,self.row['expected'],self.row['source'].encode())

    def test_json_decoding_rejects_duplicate_keys_and_nonfinite_numbers(self):
        for value in ('{"id":1,"id":2}','{"x":{"id":0,"id":1}}','{"x":NaN}','[Infinity]','[-Infinity]','[1e999]','[-1e999]'):
            with self.assertRaises(ValueError):self.h.strict_json_loads(value)

    def test_empty_raw_arrays_cannot_be_replaced_by_objects(self):
        import copy,json
        row=next(row for row in self.data['cases'] if row['id']=='producer_nested_snapshot')
        base=json.loads(row['receipt_json'])['original']
        paths=[('records',0,'fields'),('functions',0,'parameters'),('functions',0,'locals'),
               ('functions',0,'blocks',0,'instructions'),('functions',2,'calls',0,'arguments')]
        for path in paths:
            value=copy.deepcopy(base);target=value['raw_view']
            for component in path[:-1]:target=target[component]
            target[path[-1]]={}
            with self.assertRaises(ValueError):self.h.validate_candidate_receipt(value,row['expected'],row['source'].encode())

    def test_parameter_mode_matches_its_authority_type_and_declaration(self):
        import copy,json
        row=next(row for row in self.data['cases'] if row['id']=='producer_reference_mode')
        base=json.loads(row['receipt_json'])['original']
        for mutate in (lambda p:p.update(mode='scalar'),lambda p:p.update(type={'mode':'scalar','type':'i32'}),
                       lambda p:p.update(span=[0,0]),lambda p:p.update(position=1)):
            value=copy.deepcopy(base);mutate(value['raw_view']['functions'][0]['parameters'][0])
            with self.assertRaises(ValueError):self.h.validate_candidate_receipt(value,row['expected'],row['source'].encode())

    def test_explicit_cli_mode_has_scoped_success_and_preserves_production_partial(self):
        import contextlib,io,json,tempfile
        from unittest import mock
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);binary=root/'fake';binary.write_bytes(b'fake');binary.chmod(0o700)
            for mode,wanted in [('production',2),('cli',0)]:
                evidence=root/mode
                def jobs(*args,**kwargs):
                    for profile in ('debug','release'):
                        dest=evidence/profile;dest.mkdir(parents=True)
                        self.h.write_json(dest/'profile-result.json',{'diagnostic_selections':{},'elf':[], 'full_qualification':False})
                    return 0
                with mock.patch.object(self.h,'verify_frozen',return_value={'files':[]}),mock.patch.object(self.h,'run_jobs',side_effect=jobs),contextlib.redirect_stdout(io.StringIO()):
                    result=self.h.main([str(binary),str(binary),'--mode',mode,'--frozen',str(root/'frozen'),'--evidence',str(evidence)])
                self.assertEqual(result,wanted)
                report=json.loads((evidence/'production-cli-result.json').read_text());self.assertIs(report['full_qualification'],False)
                self.assertEqual(report['status'],'CLI_ELF_PASS' if mode=='cli' else 'PARTIAL')
            with mock.patch.object(self.h,'verify_frozen',return_value={'files':[]}),mock.patch.object(self.h,'run_jobs',return_value=124):
                self.assertEqual(self.h.main([str(binary),str(binary),'--mode','cli','--frozen',str(root/'frozen'),'--evidence',str(root/'timeout')]),124)

    def test_complete_source_inventory_and_expectations_are_reproduced(self):
        import copy,json,tempfile
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory);manifest=self.h.freeze(path);self.h.verify_frozen(path)
            for entries in ([],manifest['files'][:1],manifest['files'][:1]*2):
                modified={**manifest,'files':entries};self.h.write_json(path/'manifest.json',modified)
                with self.assertRaisesRegex(ValueError,'complete unique ordered'):self.h.verify_frozen(path)
            modified=copy.deepcopy(manifest);entry=modified['files'][0];expectation=path/(entry['id']+'.json')
            value=json.loads(expectation.read_text());value['expected']['code']='E9999';self.h.write_json(expectation,value)
            entry['expectation_sha256']=self.h.digest(expectation);self.h.write_json(path/'manifest.json',modified)
            with self.assertRaisesRegex(ValueError,'differs from independent model'):self.h.verify_frozen(path)

    def test_raw_inspector_deep_scalar_types_are_exact(self):
        self.assertFalse(self.h.RawInspector.equal({'values':[True]}, {'values':[1]}))
        self.assertFalse(self.h.RawInspector.equal({'values':[1]}, {'values':[1.0]}))
        self.assertTrue(self.h.RawInspector.equal({'values':[1]}, {'values':[1]}))

    def test_scalar_constant_substitution_is_detected_without_execution(self):
        import json
        row = next(row for row in self.data['cases'] if row['id'] == 'producer_parameter_positions')
        observed = json.loads(row['receipt_json'])['original']
        function = observed['raw_view']['functions'][0]
        constant = next(event['instruction']['scalar']['value'] for block in function['blocks'] for event in block['instructions']
                        if event['instruction']['operation'] == 'Scalar' and event['instruction']['scalar'].get('value', {}).get('kind') == 'I32')
        constant['value'] += 1
        checked = m.NamesTypes(self.h.decode_source_program(row['program'])).check()
        report = self.h.RawInspector(row['expected'], row['source'], observed['raw_view'], checked).inspect()
        self.assertTrue(any(issue['fact'].endswith('.scalar-literal') for issue in report['issues']))

    def test_native_probe_sources_reproduce_pre_observation_expectations(self):
        rows = self.h.native_probe_expectations(ROOT/'tests/fixtures/owned_source/native-probe-sources.json')
        self.assertEqual(len(rows), 8)
        self.assertEqual(sum(len(item['native_budget_expectations']) for _, item in rows), 1932)
        batch = next(item for _,item in rows if item['id']=='batch')
        self.assertEqual((batch['schedule']['total_fuel'],batch['expected']['result']), (1297,816))

    def test_raw_valid_control_does_not_pass_if_mutant_is_replaced_by_baseline(self):
        import copy,json,tempfile
        value=copy.deepcopy(self.data);row=value['cases'][0];receipt=json.loads(row['receipt_json']);receipt['mutant']=receipt['original']
        row['receipt_json']=json.dumps(receipt);row['receipt_sha256']=hashlib.sha256(row['receipt_json'].encode()).hexdigest();row['release_receipt_sha256']=row['receipt_sha256']
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'controls.json';path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError,'escaped correspondence'):self.h.compare_translation_controls(path)

    def test_translation_required_cause_is_used(self):
        import copy,json,tempfile
        value=copy.deepcopy(self.data);row=next(r for r in value['cases'] if r['id']=='producer_borrow_weakening');receipt=json.loads(row['receipt_json'])
        receipt['original']['check']['diagnostics'][0]['related']=receipt['original']['check']['diagnostics'][0]['related'][1:]
        row['receipt_json']=json.dumps(receipt);row['receipt_sha256']=hashlib.sha256(row['receipt_json'].encode()).hexdigest();row['release_receipt_sha256']=row['receipt_sha256']
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'controls.json';path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError,'frozen cause'):self.h.compare_translation_controls(path)

    def test_budget_prefix_matcher_requires_complete_ordered_inventory(self):
        import copy
        item=m.case_expectation(next(c for c in m.corpus() if c.identifier=='replace-self'))
        rows=[self.h.expected_budget_row(item,n) for n in range(item['schedule']['total_fuel']+1)]
        self.assertEqual(self.h.compare_budget_receipts(item,rows,require_stores=True)['issues'],[])
        for variant in (rows[:-1],rows+[rows[-1]],list(reversed(rows))):
            with self.assertRaises(ValueError):self.h.compare_budget_receipts(item,variant)
        altered=copy.deepcopy(rows);altered[0]['budget']=False
        with self.assertRaises(ValueError):self.h.compare_budget_receipts(item,altered)

    def test_budget_prefix_matcher_detects_early_store_and_wrong_failure(self):
        import copy
        item=m.case_expectation(next(c for c in m.corpus() if c.identifier=='replace-self'))
        rows=[self.h.expected_budget_row(item,n) for n in range(item['schedule']['total_fuel']+1)]
        first=next(n for n,row in enumerate(rows) if row['committed_stores'])
        altered=copy.deepcopy(rows);altered[first-1]['committed_stores']=altered[first]['committed_stores']
        self.assertEqual(self.h.compare_budget_receipts(item,altered)['issues'][0]['fact'],'committed-store-prefix')
        self.assertEqual(self.h.compare_budget_receipts(item,altered,require_stores=True)['store_prefix_gate'],'MISMATCH')
        altered=copy.deepcopy(rows);altered[0]['reference']['failure']['span']=[0,0]
        self.assertTrue(self.h.compare_budget_receipts(item,altered)['issues'])
        altered=copy.deepcopy(rows);del altered[0]['committed_stores']
        self.assertEqual(self.h.compare_budget_receipts(item,altered)['store_prefix_gate'],'PENDING')
        with self.assertRaisesRegex(ValueError,'missing actual'):self.h.compare_budget_receipts(item,altered,require_stores=True)

    def test_candidate_collection_identity_changes_fail_closed(self):
        import json,tempfile
        from unittest import mock
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);frozen=root/'frozen';observed=root/'observed';frozen.mkdir();observed.mkdir()
            (frozen/'manifest.json').write_text('{}');(observed/'x.json').write_text('{}');binary=root/'binary';binary.write_text('pinned')
            manifest={'files':[{'id':'x','source_sha256':'source'}]}
            collection={'exit_code':0,'source_manifest_sha256':self.h.digest(frozen/'manifest.json'),'source_files':{'x.ox':'source'},'receipt_files':{'x.json':self.h.digest(observed/'x.json')},'command':[str(binary)],'binary_sha256':self.h.digest(binary)}
            path=root/'collection.json';path.write_text(json.dumps(collection))
            with mock.patch.object(self.h,'verify_frozen',return_value=manifest):
                self.h.verify_collection_manifest(frozen,observed,path)
                for key,value in [('exit_code',False),('source_manifest_sha256','changed'),('source_files',{}),('receipt_files',{}),('binary_sha256','changed')]:
                    changed={**collection,key:value};path.write_text(json.dumps(changed))
                    with self.assertRaises(ValueError):self.h.verify_collection_manifest(frozen,observed,path)

    def test_unchanged_reviewer_handwritten_batch_ledger(self):
        import json,shutil,subprocess,tempfile
        fixture=ROOT/'tests/fixtures/owned_source/reviewer/review_batch_ledger.py'
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);review=root/'review';review.mkdir();shutil.copyfile(fixture,review/fixture.name)
            frozen=root/'ownership-unit4-source-model/freeze-v3/frozen';frozen.mkdir(parents=True)
            (frozen/'batch.json').write_text(json.dumps(m.case_expectation(next(c for c in m.corpus() if c.identifier=='batch'))))
            process=subprocess.run([sys.executable,str(review/fixture.name)],capture_output=True,timeout=30)
            self.assertEqual(process.returncode,0,process.stderr.decode())
            report=json.loads((review/'independent-batch-ledger.json').read_text())
            self.assertEqual((report['result'],report['fuel'],report['charged_operations']), (816,1297,506))


def load_tests(loader, tests, pattern):
    """Retain unchanged reviewer controls; repeated cases are regression counts."""
    import importlib
    from unittest import mock
    fixture=ROOT/'tests/fixtures/owned_source/reviewer'
    sys.path.insert(0,str(fixture))
    modules=('test_reviewer_owned_source','test_reviewer_lexical_boundaries','test_reviewer_final_contract_edges',
             'test_alias_amendment','test_alias_amendment_edges')
    with mock.patch.dict(os.environ, {'OWNED_SOURCE_MODEL_SCRIPTS':str(Path(__file__).resolve().parent)}):
        for name in modules:tests.addTests(loader.loadTestsFromModule(importlib.import_module(name)))
    return tests


if __name__ == '__main__':
    unittest.main()
