"""Held-out RFC-derived tests. Runs model and synthetic harness only, never oxid/LLVM.

The frozen model modules live in scripts/. Set OWNED_SOURCE_MODEL_SCRIPTS to test
an integrated revision without changing these reviewer-authored expectations.
"""
import contextlib
import copy
import hashlib
import io
import itertools
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, os.environ.get('OWNED_SOURCE_MODEL_SCRIPTS', str(ROOT / 'scripts')))
import owned_source_model as m
import verify_owned_source as h


def program_for(b, statements, result='i32'):
    return m.Program((m.Record('S', (('value', 'i32'),)),),
                     (m.Function('main', (), result, b.block(*statements)),))


def expectation(program, identifier='heldout'):
    return m.case_expectation(m.Case(identifier, 'reviewer-heldout', program))


def operation(b, op):
    if op == 'R': return b.discard(b.f('x', 'value'))
    if op == 'M': return b.discard(b.v('x'))
    if op == 'I': return b.store('x', b.lit('S', ('value', b.i(7))))
    if op == 'C': return b.iff(b.b(False), b.block(b.discard(b.v('x'))))
    raise AssertionError(op)


def transition(states, word):
    """Small separate set model: A=available, M=moved, both branch outcomes."""
    states = set(states)
    for op in word:
        if op in ('R', 'M', 'C') and 'M' in states: return None
        if op == 'M': states = {'M'}
        elif op == 'I': states = {'A'}
        elif op == 'C': states.add('M')
    return states


def cycle_acceptance(word):
    entry = {'A'}
    while True:
        out = transition(entry, word)
        if out is None: return False
        joined = entry | out
        if joined == entry: return True
        entry = joined


class IndependentStaticStateTests(unittest.TestCase):
    def test_exhaustive_straight_line_choice_automaton(self):
        total = 0
        for n in range(6):
            for word in itertools.product('RMIC', repeat=n):
                b = m.Builder()
                p = program_for(b, [b.let('x', b.lit('S', ('value', b.i(0))), True)] +
                                [operation(b, op) for op in word] + [b.ret(b.i(0))])
                actual = m.static_check(m.NamesTypes(p).check())['accepted']
                self.assertEqual(actual, transition({'A'}, word) is not None, word)
                total += 1
        self.assertEqual(total, 1365)

    def test_exhaustive_loop_fixed_point_automaton(self):
        total = 0
        for n in range(5):
            for word in itertools.product('RMIC', repeat=n):
                b = m.Builder()
                p = program_for(b, [b.let('x', b.lit('S', ('value', b.i(0))), True),
                    b.loop(b.b(False), b.block(*[operation(b, op) for op in word])), b.ret(b.i(0))])
                actual = m.static_check(m.NamesTypes(p).check())['accepted']
                self.assertEqual(actual, cycle_acceptance(word), word)
                total += 1
        self.assertEqual(total, 341)

    def test_exact_state_ceiling_is_inclusive_and_one_below_fails_closed(self):
        b = m.Builder()
        p = program_for(b, [b.let('x', b.lit('S', ('value', b.i(0))), True),
              b.loop(b.b(False), b.block(operation(b, 'M'), operation(b, 'I'))), b.ret(b.i(0))])
        checked = m.NamesTypes(p).check()
        graph = m.SourceGraph(checked, checked.functions['main'])
        count = graph.saturate()['states']
        self.assertEqual(graph.saturate(state_ceiling=count)['states'], count)
        with self.assertRaisesRegex(ValueError, 'MODEL_INCOMPLETE'):
            graph.saturate(state_ceiling=count - 1)

    def test_independent_bell_and_mode_enumeration(self):
        counts = []
        for n in range(1, 5):
            partitions = set()
            for assignment in itertools.product(range(n), repeat=n):
                identity, normalized = {}, []
                for root in assignment:
                    identity.setdefault(root, len(identity))
                    normalized.append(identity[root])
                partitions.add(tuple(normalized))
            accepts = 0
            for assignment in partitions:
                for modes in itertools.product(('shared', 'exclusive'), repeat=n):
                    groups = [[modes[i] for i in range(n) if assignment[i] == root] for root in set(assignment)]
                    permitted = all(len(group) == 1 or all(mode == 'shared' for mode in group) for group in groups)
                    self.assertEqual(m.alias_contract(assignment, modes), permitted)
                    accepts += permitted
            self.assertEqual(set(m.alias_partitions(n)), partitions)
            counts.append((len(partitions) * 2 ** n, accepts))
        self.assertEqual(counts, [(2, 2), (8, 5), (40, 15), (240, 52)])

    def test_all_290_alias_sources_against_separate_alias_rule(self):
        total=accepted=0
        for case in m.alias_cases():
            call=case.program.functions[-1].body.data['statements'][-1].data['value']
            args=call.data['args'];roots={arg.data['name'] for arg in args}
            good=all(sum(arg.data['name']==root for arg in args)==1 or
                     not any(arg.data['name']==root and arg.data['mutable'] for arg in args) for root in roots)
            actual=m.static_check(m.NamesTypes(case.program).check())['accepted']
            self.assertEqual(actual,good,case.identifier)
            total+=1;accepted+=good
        self.assertEqual((total,accepted),(290,74))

    def test_lexical_loop_slot_has_fresh_dynamic_generations(self):
        b = m.Builder()
        p = program_for(b, [b.let('i', b.i(0), True),
            b.loop(b.op(b.v('i'), '<', b.i(3)), b.block(
                b.let('x', b.lit('S', ('value', b.v('i')))),
                b.discard(b.v('x')), b.store('i', b.op(b.v('i'), '+', b.i(1))), b.cont())), b.ret(b.v('i'))])
        result = expectation(p)
        self.assertEqual(result['expected']['result'], 3)
        creates = [e for e in result['dynamic']['events'] if e['kind'] == 'let-owned']
        ends = [e for e in result['dynamic']['events'] if e['kind'] == 'lexical-end']
        self.assertEqual([e['owner'][-1] for e in creates], [1, 2, 3])
        self.assertEqual([e['owner'] for e in ends], [e['owner'] for e in creates])

    def test_distinct_record_shared_parameters_cannot_alias(self):
        b = m.Builder()
        fn = m.Function('f', (('a', '&A'), ('z', '&Z')), 'i32', b.block(b.ret(b.i(0))))
        p = m.Program((m.Record('A'), m.Record('Z')), (fn,))
        checked = m.NamesTypes(p).check()
        graph = m.SourceGraph(checked, fn)
        self.assertEqual(len(list(graph.initial_states())), 1)


class HeldoutScalarContractTests(unittest.TestCase):
    def test_out_of_range_literals_are_resolution_failures(self):
        for number in (-(2 ** 31) - 1, 2 ** 31, 10 ** 100):
            with self.subTest(number=number):
                b = m.Builder(); lit = b.i(number)
                result = expectation(program_for(b, [b.ret(lit)]))
                self.assertEqual(result['expected']['status'], 'reject')
                self.assertEqual((result['expected']['stage'], result['expected']['code']), ('resolve', 'E0203'))
                self.assertEqual(result['expected']['span'], result['origins'][lit.key])

    def test_i32_boundaries_are_valid(self):
        for number in (-(2 ** 31), 2 ** 31 - 1):
            b = m.Builder(); self.assertEqual(expectation(program_for(b, [b.ret(b.i(number))]))['expected']['result'], number)

    def test_minus_is_literal_syntax_not_a_unary_operator(self):
        b = m.Builder()
        p = program_for(b, [b.let('x', b.i(3)), b.ret(b.unary('-', b.v('x')))])
        try: result = expectation(p)
        except ValueError: return  # Rejecting outside the tagged model domain is safe.
        self.assertEqual(result['expected']['status'], 'reject')
        self.assertEqual((result['expected']['stage'], result['expected']['code']), ('parse', 'E0101'))

    def test_not_binary_requires_explicit_group_or_source_semantics(self):
        b = m.Builder()
        p = program_for(b, [b.ret(b.unary('!', b.op(b.b(True), '&&', b.b(False))))], 'bool')
        try: result = expectation(p)
        except ValueError: return  # The existing binary renderer rejects such noncanonical ASTs.
        self.assertIn('!true && false', result['source'])
        self.assertEqual(result['expected']['result'], False)  # ! binds more tightly than &&.

    def test_not_grouped_binary_semantics_and_schedule(self):
        b = m.Builder()
        p = program_for(b, [b.ret(b.unary('!', b.group(b.op(b.b(True), '&&', b.b(False)))))], 'bool')
        result = expectation(p)
        self.assertEqual(result['expected']['result'], True)
        self.assertEqual([x['operation'] for x in result['schedule']['items']],
                         ['Root', 'Scalar', 'Branch', 'Scalar', 'Goto', 'BoolMerge', 'Scalar', 'Scalar', 'ReturnScalar'])
        self.assertEqual(result['schedule']['total_fuel'], 14)

    def test_deferred_division_and_remainder_are_parse_rejections(self):
        for op in ('/', '%'):
            with self.subTest(operator=op):
                b = m.Builder(); expression = b.op(b.i(7), op, b.i(3))
                try: result = expectation(program_for(b, [b.ret(expression)]))
                except ValueError: continue
                self.assertEqual(result['expected']['status'], 'reject')
                self.assertEqual((result['expected']['stage'],result['expected']['code']), ('parse','E0101'))
                self.assertEqual(result['expected']['span'], result['origins'][expression.key + '.operator'])

    def test_comparisons_are_one_nonassociative_tier(self):
        for inner_op, outer_op in (('<','=='), ('==','=='), ('!=','!=')):
            with self.subTest(operators=(inner_op,outer_op)):
                b = m.Builder()
                expression = b.op(b.op(b.i(1),inner_op,b.i(2)),outer_op,b.b(True))
                try: result = expectation(program_for(b, [b.ret(expression)], 'bool'))
                except ValueError: continue
                self.assertEqual(result['expected']['status'], 'reject')
                self.assertEqual((result['expected']['stage'],result['expected']['code']), ('parse','E0100'))
                self.assertEqual(result['expected']['span'], result['origins'][expression.key + '.operator'])

    def test_type_errors_use_first_invalid_operand_span(self):
        for op, bad_side in (('+','lhs'), ('+','rhs'), ('<','lhs'), ('<','rhs'), ('==','rhs')):
            with self.subTest(operator=op,bad_side=bad_side):
                b=m.Builder(); lhs=b.b(True) if bad_side=='lhs' else b.i(1)
                rhs=b.b(True) if bad_side=='rhs' else b.i(2)
                expression=b.op(lhs,op,rhs)
                result=expectation(program_for(b,[b.ret(expression)],'i32' if op=='+' else 'bool'))
                self.assertEqual((result['expected']['stage'],result['expected']['code']),('type','E0300'))
                self.assertEqual(result['expected']['span'],result['origins'][(lhs if bad_side=='lhs' else rhs).key])
        for identifier in ('aggregate-equality','condition-grouped-aggregate-comparison'):
            case=next(c for c in m.corpus() if c.identifier==identifier)
            result=expectation(case.program)
            source=result['source'].encode()
            start,end=result['expected']['span']
            self.assertNotIn(b'==',source[start:end])

    def test_overflow_occurs_before_right_operand_effects(self):
        b=m.Builder()
        mutate=m.Function('touch',(('p','&mut S'),),'i32',b.block(b.write('p','value',b.i(9)),b.ret(b.i(0))))
        left=b.group(b.op(b.i(m.MAX_I32),'+',b.i(1)))
        expression=b.op(left,'+',b.call('touch',b.borrow('x',True)))
        main=m.Function('main',(),'i32',b.block(b.let('x',b.lit('S',('value',b.i(0))),True),b.ret(expression)))
        result=expectation(m.Program((m.Record('S',(('value','i32'),)),),(mutate,main)))
        self.assertEqual(result['expected']['runtime_failure']['code'],'E0604')
        self.assertEqual(result['dynamic']['writes'],[])
        self.assertFalse(any(e['kind']=='call-open' for e in result['dynamic']['events']))

    def test_dynamic_model_bound_cannot_be_published_as_runtime_expectation(self):
        b = m.Builder()
        p = program_for(b, [b.loop(b.b(True), b.block(b.cont())), b.ret(b.i(0))])
        machine = m.Machine
        with mock.patch.object(m, 'Machine', lambda checked: machine(checked, step_bound=30)):
            try: result = expectation(p)
            except ValueError as error:
                self.assertIn('MODEL', str(error)); return
        self.assertNotEqual(result.get('expected', {}).get('runtime_failure', {}).get('code'), 'MODEL_BOUND',
                            'An incomplete interpreter run is not a compiler runtime failure oracle')

    def test_self_replacement_hand_fuel_and_commit_boundary(self):
        b=m.Builder();p=program_for(b,[b.let('x',b.lit('S',('value',b.i(1))),True),
                                        b.store('x',b.v('x')),b.ret(b.f('x','value'))])
        result=expectation(p);schedule=result['schedule'];dynamic=result['dynamic']
        wanted=[('Root',18),('Scalar',1),('StorageLive',1),('Construct',2),
                ('StorageLive',1),('MoveInitialize',2),('StorageEnd',2),
                ('StorageLive',1),('MoveInitialize',2),('Replace',3),('StorageEnd',2),
                ('ReadField',1),('StorageEnd',2),('ReturnScalar',4)]
        self.assertEqual([(i['operation'],i['cost']) for i in schedule['items']],wanted)
        self.assertEqual(schedule['total_fuel'],42)
        before=m.TemplateScheduler.at_budget(schedule,dynamic,32)
        after=m.TemplateScheduler.at_budget(schedule,dynamic,33)
        self.assertEqual((before['failure']['operation'],len(before['committed_writes'])),('Replace',0))
        self.assertEqual((after['failure']['operation'],len(after['committed_writes'])),('StorageEnd',1))

    def test_recursive_dynamic_depth_limit_is_explicit_incompleteness(self):
        b=m.Builder()
        recurse=m.Function('recurse',(),'i32',b.block(b.ret(b.call('recurse'))))
        main=m.Function('main',(),'i32',b.block(b.ret(b.call('recurse'))))
        p=m.Program((m.Record('S'),),(recurse,main))
        with self.assertRaisesRegex(ValueError,'MODEL_INCOMPLETE'):
            expectation(p)


class HeldoutRenderedDomainTests(unittest.TestCase):
    def reject_or_domain_error(self,program):
        try: result=expectation(program)
        except (ValueError,m.Rejected):return
        self.assertEqual(result['expected']['status'],'reject')

    def test_invalid_and_reserved_local_identifiers_fail_closed(self):
        for name in ('','1x','x y','x-y','雪','true','false','fn','return','mut','struct','while'):
            with self.subTest(name=name):
                b=m.Builder()
                self.reject_or_domain_error(program_for(b,[b.let(name,b.i(7)),b.ret(b.v(name))]))

    def test_identifier_validation_covers_declarations_fields_and_functions(self):
        for kind in ('record','field','function','parameter'):
            with self.subTest(kind=kind):
                b=m.Builder();records=(m.Record('S',(('value','i32'),)),)
                if kind=='record':records=(m.Record('bad-name'),)
                if kind=='field':records=(m.Record('S',(('return','i32'),)),)
                helpers=() if kind not in ('function','parameter') else (
                    m.Function('fn' if kind=='function' else 'helper',(('雪','i32'),) if kind=='parameter' else (),
                               'i32',b.block(b.ret(b.i(0)))),)
                p=m.Program(records,helpers+(m.Function('main',(),'i32',b.block(b.ret(b.i(0)))),))
                self.reject_or_domain_error(p)

    def test_ascii_identifiers_and_nominal_builtin_restrictions(self):
        for name in ('_','_x','x0','X','bool','i32'):
            b=m.Builder()
            self.assertEqual(expectation(program_for(b,[b.let(name,b.i(7)),b.ret(b.v(name))]))['expected']['result'],7)
        for name in ('bool','i32'):
            b=m.Builder();result=expectation(m.Program((m.Record(name),),(m.Function('main',(),'i32',b.block(b.ret(b.i(0)))),)))
            self.assertEqual((result['expected']['stage'],result['expected']['code']),('resolve','E0202'))

    def test_item_order_must_cover_each_item_exactly_once(self):
        b=m.Builder();main=m.Function('main',(),'i32',b.block(b.ret(b.i(7))))
        for order in ((('record',0),),(('function',0),),(('unknown',0),('record',0))):
            with self.subTest(order=order):
                with self.assertRaises(ValueError):m.render(m.Program((m.Record('S'),),(main,),order=order))
        p=m.Program((m.Record('S'),),(main,),order=(('function',0),('record',0)))
        self.assertEqual(expectation(p)['expected']['result'],7)

    def test_trivia_slots_cannot_inject_source_or_unclosed_comments(self):
        for text in ('fn injected()->i32{return 1;}\n','/* unclosed','x','/* outer /* inner */ */'):
            with self.subTest(preamble=text):
                b=m.Builder();p=m.Program((m.Record('S'),),(m.Function('main',(),'i32',b.block(b.ret(b.i(7)))),),preamble=text)
                with self.assertRaises(ValueError):m.render(p)
        for newline in ('x','/*'):
            b=m.Builder();p=m.Program((m.Record('S'),),(m.Function('main',(),'i32',b.block(b.ret(b.i(7)))),),newline=newline)
            with self.subTest(newline=newline):
                with self.assertRaises(ValueError):m.render(p)
        b=m.Builder();main=m.Function('main',(),'i32',b.block(b.let('x',b.lit('S',('value',b.i(1))),True),
                                   b.ret(b.call('read',b.borrow('x',False,trivia=' injected ')))))
        read=m.Function('read',(('p','&S'),),'i32',b.block(b.ret(b.f('p','value'))))
        with self.assertRaises(ValueError):m.render(m.Program((m.Record('S',(('value','i32'),)),),(read,main)))

    def test_scalar_payload_python_types_are_exact(self):
        for tag,value in (('int',True),('int',1.5),('bool',1),('bool','false')):
            with self.subTest(tag=tag,value=value):
                b=m.Builder();expr=b.node(tag,value=value)
                with self.assertRaises(ValueError):expectation(program_for(b,[b.ret(expr)],'i32' if tag=='int' else 'bool'))

    def test_condition_literal_requires_group_or_call_argument_boundary(self):
        b=m.Builder();p=program_for(b,[b.iff(b.lit('S',('value',b.i(1))),b.block(b.ret(b.i(1))),b.block(b.ret(b.i(0))))])
        try:result=expectation(p)
        except ValueError:return
        self.assertEqual(result['expected']['stage'],'parse')

    def test_exact_negative_token_codes_follow_the_ordinary_parser_contract(self):
        ordinary={'temporary-literal-borrow','temporary-call-borrow','parenthesized-place','arbitrary-deref',
                  'shorthand-literal','trailing-call-comma','condition-literal','mutable-parameter','trailing-parameter-comma'}
        for case in m.excluded_cases():
            identifier=case.identifier.removeprefix('excluded-')
            result=expectation(case.program) if case.program else m.case_expectation(case)
            with self.subTest(identifier=identifier):
                self.assertEqual(result['expected']['code'],'E0100' if identifier in ordinary else 'E0101')


class HeldoutHarnessTests(unittest.TestCase):
    def test_summary_must_belong_to_a_real_command(self):
        fake = {'schema_version': 1, 'kind': 'invented-summary', 'success': True}
        with self.assertRaises(ValueError):
            h.parse_records(subprocess.CompletedProcess([], 0, (json.dumps(fake) + '\n').encode(), b''))

    def test_runtime_overflow_must_not_accept_human_and_elf_success(self):
        """No compiler subprocess: inject command results for a one-source fixture."""
        with tempfile.TemporaryDirectory(prefix='owned-review-') as temporary:
            root = Path(temporary); binary = root / 'fake-compiler'; binary.write_bytes(b'fake')
            corpus = root / 'corpus'; corpus.mkdir(); evidence = root / 'evidence'
            (corpus / 'overflow.ox').write_text('struct S {}\nfn main() -> i32 { return 2147483647 + 1; }\n')
            item = {'expected': {'status': 'accept', 'result': None, 'result_type': 'i32',
                    'runtime_failure': {'code': 'E0604', 'origin': 'operator'}},
                    'origins': {'operator': [54,55]}, 'function_count': 1,
                    'source_sha256': hashlib.sha256((corpus/'overflow.ox').read_bytes()).hexdigest()}
            (corpus / 'overflow.json').write_text(json.dumps(item))
            def fake_run(argv, **kwargs):
                if argv[0] != str(binary): return subprocess.CompletedProcess(argv, 0, b'42\n', b'')
                operation = argv[1]
                if operation == 'run' and '--message-format=json' not in argv:
                    return subprocess.CompletedProcess(argv, 0, b'42\n', b'')
                records = []
                code = 1 if operation == 'run' else 0
                if code:
                    records.append({'schema_version':1,'kind':'diagnostic','stage':'oir-run','code':'E0604',
                                    'primary':{'start':54,'end':55}})
                if operation == 'compile': Path(argv[argv.index('--output')+1]).write_bytes(b'\x7fELFfake')
                records.append({'schema_version':1,'kind':operation+'-summary','success':not code,'functions':1})
                return subprocess.CompletedProcess(argv, code, b''.join((json.dumps(r)+'\n').encode() for r in records), b'')
            with mock.patch.object(h,'verify_frozen',return_value={'files':[{'id':'overflow'}]}), \
                 mock.patch.object(h.subprocess,'run',side_effect=fake_run), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(ValueError): h.profile_worker(binary, 'debug', corpus, evidence)

    def test_expected_success_rejects_matching_human_and_elf_failure(self):
        with tempfile.TemporaryDirectory(prefix='owned-review-success-') as temporary:
            root=Path(temporary); binary=root/'fake-compiler'; binary.write_bytes(b'fake')
            corpus=root/'corpus';corpus.mkdir();(corpus/'success.ox').write_text('struct S {} fn main()->i32{return 42;}')
            item={'expected':{'status':'accept','result':42,'result_type':'i32','runtime_failure':None},
                  'origins':{},'function_count':1,'source_sha256':hashlib.sha256((corpus/'success.ox').read_bytes()).hexdigest()}
            (corpus/'success.json').write_text(json.dumps(item))
            def fake_run(argv,**kwargs):
                if argv[0]!=str(binary) or '--message-format=json' not in argv:
                    return subprocess.CompletedProcess(argv,1,b'',b'wrong runtime failure\n')
                operation=argv[1]
                if operation=='compile':Path(argv[argv.index('--output')+1]).write_bytes(b'\x7fELFfake')
                summary={'schema_version':1,'kind':operation+'-summary','success':True,'functions':1,'result':{'type':'i32','value':42}}
                return subprocess.CompletedProcess(argv,0,(json.dumps(summary)+'\n').encode(),b'')
            with mock.patch.object(h,'verify_frozen',return_value={'files':[{'id':'success'}]}), \
                 mock.patch.object(h.subprocess,'run',side_effect=fake_run), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(ValueError):h.profile_worker(binary,'debug',corpus,root/'evidence')

    @unittest.skipUnless(os.name=='posix' and Path('/proc').is_dir(), 'Linux process liveness probe')
    def test_sigint_cleans_worker_and_descendant_and_records_failure(self):
        import time
        with tempfile.TemporaryDirectory(prefix='owned-review-interrupt-') as temporary:
            root=Path(temporary); worker=root/'worker.py'; launcher=root/'launch.py'
            worker.write_text("import os,pathlib,signal,subprocess,sys,time\n"
                "signal.signal(signal.SIGTERM,signal.SIG_IGN)\n"
                "child=subprocess.Popen([sys.executable,'-c','import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(30)'])\n"
                "pathlib.Path(sys.argv[1]).write_text(str(os.getpid())+' '+str(child.pid))\n"
                "time.sleep(30)\n")
            launcher.write_text("import pathlib,sys\nsys.path.insert(0,sys.argv[1])\nimport verify_owned_source as h\n"
                "root=pathlib.Path(sys.argv[2])\n"
                "sys.exit(h.run_jobs([h.Job('waiting',(sys.executable,str(root/'worker.py'),str(root/'pids')),'waiting: PASS')],root/'evidence',jobs=1,timeout=20))\n")
            runner=subprocess.Popen([sys.executable,str(launcher),str(Path(h.__file__).parent),str(root)],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            pids=[]
            try:
                deadline=time.monotonic()+5
                while not (root/'pids').exists() and time.monotonic()<deadline:time.sleep(.01)
                self.assertTrue((root/'pids').exists(),'synthetic worker failed to start')
                pids=list(map(int,(root/'pids').read_text().split()))
                os.kill(runner.pid,signal.SIGINT)
                stdout,stderr=runner.communicate(timeout=5)
                self.assertEqual(runner.returncode,130,(stdout,stderr))
                status=json.loads((root/'evidence/worker-status.json').read_text())
                self.assertEqual((status['exit'],status['passed']),(130,0))
                self.assertIn(b'CANCELLED',stdout)
                for pid in pids:
                    path=Path('/proc')/str(pid)/'stat'
                    self.assertTrue(not path.exists() or path.read_text().split()[2]=='Z',f'live process {pid}')
            finally:
                if runner.poll() is None:runner.kill();runner.communicate()
                if pids:
                    try:os.killpg(pids[0],signal.SIGKILL)
                    except ProcessLookupError:pass

    def test_alternative_causal_witness_is_not_an_unqualified_mismatch(self):
        """Same error site may be reached with either of two valid move causes."""
        case = next(c for c in m.corpus() if c.identifier == 'join-move-1-restore-0')
        result = expectation(case.program)
        alternatives = result['expected']['reachable_diagnostics']
        self.assertEqual(len(alternatives), 2)
        self.assertEqual({(a['stage'],a['code'],tuple(a['span'])) for a in alternatives},
                         {(result['expected']['stage'],result['expected']['code'],tuple(result['expected']['span']))})

    def test_correspondence_comparator_only_detects_supplied_facts(self):
        b = m.Builder(); source = expectation(program_for(b, [b.let('x', b.lit('S', ('value', b.i(1)))), b.ret(b.f('x', 'value'))]))
        facts = source['facts']
        self.assertEqual(h.model.correspondence_differences(facts, copy.deepcopy(facts)), [])
        # The model freezes declaration fields, but not each ReadField's chosen field identity.
        self.assertTrue(all(key in facts for key in ('records','bindings','calls','transfers')))


if __name__ == '__main__': unittest.main(verbosity=2)
