"""Wholly mocked orchestration tests. No producer/archive module is imported."""
import builtins
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import socket
import subprocess
import sys
import types
import unittest
from unittest import mock

PATH=Path(__file__).with_name('worker.py')
spec=importlib.util.spec_from_file_location('held_fresh_worker_tests',PATH)
w=importlib.util.module_from_spec(spec);spec.loader.exec_module(w)


class Sink:
    def __init__(self):self.rows=[];self.failure=None
    def emit(self,kind,row):
        if self.failure:self.failure(kind,row)
        self.rows.append((kind,w.detached(row,262144)))


class Cancellation:
    def __init__(self):self.error=None;self.owner=None;self.finalizing=False;self.sealed=False;self.report=None
    def check(self):
        if self.error:raise self.error


class Clock:
    def __init__(self):self.now=1000.0
    def monotonic(self):return self.now
    def sleep(self,n):self.now+=n


class MemoryPath:
    members={}
    writes=[]
    def __init__(self,value):self.value=str(value)
    def __truediv__(self,name):return MemoryPath(self.value+'/'+str(name))
    def __str__(self):return self.value
    @property
    def name(self):return self.value.rsplit('/',1)[-1]
    def mkdir(self):self.writes.append(self.value)
    def iterdir(self):return iter(self.members.get(self.value,[]))
    def is_dir(self):return True
    def is_symlink(self):return False


class Record:
    def __init__(self,doc):self.doc=doc
    def document(self):return w.detached(self.doc)
    def stdout(self):return b''
    def stderr(self):return b''


class FakeSession:
    def __init__(self,clock):
        self.clock=clock;self.log=[];self.poisoned=False;self.failure_audits=0
        self.invocation=object();self.machine=object();self.context=object();self.permit=object()
        self.REPOSITORY=MemoryPath('/installed')
        self.runs={};self.probes={};self.monitors={};self.views={}
        self.request=dict(scope='combined',expected_head='a'*40,event_sha='a'*40,
            original_repo='/original',historical_repo='/history',public=dict(contract_root='/contracts',path_map='/map',
            amendment_root='/amendment',llvm_root='/llvm',library_root='/libs',runtime_receipt='/receipt'),parser={'contract_root':'/parser'})
        self.installed=dict(repository='/installed',head='a'*40,tree='b'*40)
        self.tools=types.SimpleNamespace(document=lambda:{},environment=lambda:{},tools=lambda:{})
        self.state=dict(invocation=self.invocation,runtime_permit=self.permit,root=MemoryPath('/live/public-runtime'),
            cfgs={'ordinary':{}},runtime_tools=self.tools,contracts=types.SimpleNamespace(verified=[],effective_identity={}),
            predecessors=types.SimpleNamespace(qualified_paths_amendment={}),execution_context={},
            rosters={section:[{'scope':scope} for scope,count in expected.items() for _ in range(count)] for section,expected in w.SELECTED_ROWS.items()},probes={},process_cases={})
        self.prep=types.SimpleNamespace(_RUNS=self.runs,_PROBES=self.probes,_OUTPUT_MONITORS=self.monitors,_VIEWS=self.views,
            check_deadline=self.check_deadline,public_output_tick=self.tick,produce=self.produce,
            checked_scope=lambda *a:self.scope('checked-scope'),
            full_verify_invocation=lambda inv:self.log.append('full-verify'),public_replay=lambda *a:self.scope('replay'),
            public_output_start=self.monitor_start,_public_output_report=self.monitor_report,
            public_output_boundary=self.section,public_case_scope=lambda *a:self.scope('case'),
            verify_probe=self.verify_probe,public_output_failure=self.failure_audit,
            cancel_invocation=self.poison)
        self.runtime=types.SimpleNamespace(_state=lambda a:self.state,_checkpoint=lambda *a,**k:self.log.append('checkpoint'),
            _read_boundary=lambda *a:self.scope('boundary'),_write=self.write,_generator_check=lambda s:{'sources':35},
            issue_downstream_diagnostic=self.issue_selection, collect_downstream_diagnostic=self.collect_selected,
            verify_downstream_diagnostic=self.verify_selected,
            core_compare=lambda *a:self.log.append('compare'),_validate_case_evidence=lambda *a:self.log.append('validate-case'),
            _verify_process=lambda *a:self.log.append('verify-process'),_check_receipt=lambda *a:self.log.append('check-receipt'),
            admit_runtime=self.admit_runtime,InfrastructureFailure=type('Infra',(RuntimeError,),{}),
            collect=self.forbidden,verify_collection=self.forbidden)
        self.build=types.SimpleNamespace(build=self.public_build)
        self.parser=types.SimpleNamespace(authority=lambda c:object(),prepare=lambda a:{'path':'/live/parser/checkpoint'},
            build=self.parser_build,verify_build=lambda result:self.log.append('verify-parser-build'),
            create_runtime=self.create_parser_runtime,complete_runtime=self.complete_parser_runtime,verify_runtime=self.verify_parser_runtime)
        self.api=dict(preparation=self.prep,parser=self.parser,
            context=types.SimpleNamespace(capture_source=lambda *a,**k:types.SimpleNamespace(document=lambda:{'checkout':{'tree':'b'*40}}),
                begin_invocation=lambda *a:self.invocation,verify_context=lambda c:True),
            machine=types.SimpleNamespace(capture_machine=lambda inv:self.machine,
                capture_parser_runtime=self.capture_parser_runtime,capture_runtime_tools=lambda *a,**k:self.tools,verify_runtime_tools=lambda *a:True,verify_machine=lambda *a:True),
            binding=types.SimpleNamespace(build_api=lambda:self.build,runtime_api=lambda:self.runtime))
    def forbidden(self,*a,**k):raise AssertionError('forbidden full collection/runtime')
    def own_package(self):return {'own':'fixed'}
    def dependencies(self):return {'deps':'fixed'}
    def apis(self):return self.api
    def plan_request(self,request):return {'root':'/live'}
    def verify_committed(self,*a):return True
    def check_deadline(self,inv):self.runs.setdefault(inv,{'root':MemoryPath('/live'),'deadline':37000,'next':0})
    def produce(self,*a,**k):self.log.append('produce-combined');return self.context
    def public_build(self,context,role,*a):self.log.append('public-'+role);return object()
    def parser_build(self,path,profile,authority,machine,control):
        self.log.append('parser-'+str(control)+'-'+profile)
        views=self.views.setdefault(self.invocation,{})
        for tool in ('cargo','rustc'):views[str(len(views))]={'target':'/toolchain/'+tool}
        return object()
    @contextlib.contextmanager
    def scope(self,name):
        self.log.append(name+'-enter')
        try:yield
        finally:self.log.append(name+'-exit')
    def section(self,inv,permit,phase,section):
        if phase!='section-open' or section!='public':self.forbidden()
        return self.scope('section-open')
    def write(self,path,value,inv):self.log.append('write:'+str(path))
    def monitor_start(self,inv,permit):
        self.monitors[inv]={'invocation':inv,'permit':permit,'last_audit':self.clock.now,'audits':[],
            'run':self.runs[inv],'count':1}
    def tick(self,inv):
        m=self.monitors.get(inv)
        if m and self.clock.now-m['last_audit']>=3:
            m['last_audit']=self.clock.now;m['count']+=1
            m['audits'].append({'started':self.clock.now,'completed':self.clock.now,'reason':'scheduled'})
            del m['audits'][:-16]
    def monitor_report(self,m):return {'timing':{'attempts':m['count']},'full_audit_count':m['count']}
    def failure_audit(self,*a):self.failure_audits+=1;return {'verified':True}
    def poison(self,*a):self.poisoned=True
    def add_operations(self,kinds):
        for kind,count in kinds.items():
            for _ in range(count):
                index=len(self.probes)+1;directory=MemoryPath('/live/operations/'+f'{index:04d}')
                pin=lambda name:{'path':str(directory)+'/'+name,'bytes':0,'sha256':w.hashlib.sha256(b'').hexdigest()}
                case={'section':'lifecycle','index':index,'variant':'ordinary'} if kind in ('public-runtime','public-native') else None
                doc=dict(pid=100+index,started_ns=index,completed_ns=index+1,operation_kind=kind,runtime_case=case,
                    argv=['/fresh/compiler'],cwd='/live',environment={},timeout_seconds=120,stream_limit_bytes=8388608,
                    stdout=pin('stdout'),stderr=pin('stderr'),status=0,cleanup_complete=True,cleanup_error=None,
                    failure=None,timed_out=False,stream_limit_exceeded=False,observed_peaks={'processes':1,'rss_bytes':0,'output_bytes':0})
                record=Record(doc);public=kind in ('generator','public-runtime','public-native')
                self.probes[record]={'invocation':self.invocation,'directory':directory,'result':pin('result.json'),'command':pin('command.json'),
                    'runtime_permit':self.permit if public else object() if kind.startswith('parser-') and kind!='parser-helper' else None,
                    'native_permit':object() if kind=='public-native' else None}
                self.runs[self.invocation]['next']=index
                MemoryPath.members.setdefault('/live/operations',[]).append(directory)
    def admit_runtime(self,*a,**k):
        self.add_operations({k:v for k,v in w.EXPECTED_KINDS.items() if k in ('probe','public-build','parser-helper','runtime-tool-version')})
        return object()
    def issue_selection(self,admission):self.log.append('issue-selected');return object()
    def collect_selected(self,selection):
        self.log.append('collect-selected');self.monitor_start(self.invocation,self.permit)
        self.add_operations({'generator':1,'public-runtime':680,'public-native':68})
        self.selected_document=dict(status='COMPLETE_SELECTED_OBSERVATIONS',selected_sections_complete=True,
            selected_sections=['lifecycle','guards'],collection_complete=False,local_complete=False,execution_qualified=False,
            reports={section:{'required_local':expected['execute'],'unsupported_capability':0} for section,expected in w.SELECTED_ROWS.items()})
        return types.SimpleNamespace(document=lambda:w.detached(self.selected_document))
    def verify_selected(self,result):self.log.append('verify-selected');return True
    def capture_parser_runtime(self,*a):self.log.append('capture-parser-runtime');return object()
    def create_parser_runtime(self,authority,builds,contract,inputs):
        assert len(builds)==4
        self.log.append('create-parser-runtime');return object()
    def complete_parser_runtime(self,handle):
        self.log.extend(w.PARSER_SEQUENCE)
        self.add_operations({'parser-passivity':2,'parser-collect':2,'parser-u8':18,'parser-phase':54})
        return {'local_semantics_status':'pass'}
    def verify_parser_runtime(self,handle):
        self.log.append('verify-parser-runtime')
        return dict(schema='oxid-package-source-parser-runtime-v1',stages=list(w.PARSER_SEQUENCE),local_semantics_status='pass',
                    execution_qualified=False,comparison={'path':'/live/parser/comparison.json','bytes':0,'sha256':'0'*64})
    def verify_probe(self,record,*a,**k):
        self.log.append('verify-probe:'+record.doc['operation_kind']);return record.document()


class WorkerTests(unittest.TestCase):
    def setUp(self):
        self.clock=Clock();self.sink=Sink();self.cancel=Cancellation()
        MemoryPath.members={};MemoryPath.writes=[]
        self.stack=contextlib.ExitStack();self.addCleanup(self.stack.close)
        self.stack.enter_context(mock.patch.object(w.time,'monotonic',self.clock.monotonic))
        self.stack.enter_context(mock.patch.object(w.time,'sleep',self.clock.sleep))
        self.stack.enter_context(mock.patch.object(w,'Path',MemoryPath))
        # Any accidental external effect is a test failure, including from helpers.
        for name in ('Popen','run','call','check_call','check_output'):
            self.stack.enter_context(mock.patch.object(subprocess,name,side_effect=AssertionError('process forbidden')))
        self.stack.enter_context(mock.patch.object(socket,'socket',side_effect=AssertionError('network forbidden')))
        self.stack.enter_context(mock.patch.object(builtins,'open',side_effect=AssertionError('filesystem write/read forbidden')))
        self.stack.enter_context(mock.patch.object(io,'open',side_effect=AssertionError('filesystem forbidden')))
        for name in ('open','mkdir','makedirs','unlink','remove','rename','replace','symlink','link','chmod','system','fork','forkpty','posix_spawn','posix_spawnp','execv','execve'):
            if hasattr(os,name):self.stack.enter_context(mock.patch.object(os,name,side_effect=AssertionError('OS effect forbidden')))
        self.session=FakeSession(self.clock)
    def run_worker(self):return w.run(self.session,self.session.request,self.sink,started_monotonic=1000,cancellation=self.cancel,installed=self.session.installed)
    def measurements(self):
        m=w.Measurements(self.sink,1000,self.cancel);m.phase_name='selected-public';m.deadline=1300
        return m
    def test_exact_mocked_route_eight_builds_selected_and_full_parser(self):
        report=self.run_worker()
        self.assertEqual(report['status'],'PARTIAL_DOWNSTREAM_DIAGNOSTIC_COMPLETE')
        self.assertEqual(report['build_results_retained'],8)
        self.assertEqual(report['public_operations_completed'],680)
        self.assertEqual(report['native_operations_completed'],68)
        self.assertEqual(report['parser']['observations'],638)
        self.assertEqual(sum(x.startswith('parser-True-') or x.startswith('parser-False-') for x in self.session.log),4)
        self.assertEqual(self.session.log.count('verify-selected'),3)
        self.assertEqual(self.session.log.count('verify-parser-runtime'),3)
        self.assertEqual(self.session.log.count('verify-parser-build'),4)
        self.assertEqual(report['original_record_preservation']['original_records_readmitted'],869)
        self.assertEqual(report['original_record_preservation']['operation_kinds'],w.EXPECTED_KINDS)
        self.assertFalse(report['local_complete']);self.assertFalse(report['execution_qualified'])
        self.assertEqual(report['operation_cleanup']['status'],'VERIFIED')
        self.assertEqual(report['operation_cleanup']['operation_carriers'],869)
        self.assertTrue(self.cancel.sealed)
        starts=[r for k,r in self.sink.rows if k=='phase' and r['event']=='start']
        self.assertEqual([r['name'] for r in starts],list(w.PHASES))
        self.assertEqual(starts[0]['deadline_monotonic'],3400)
        calls=[r for k,r in self.sink.rows if k=='call' and not r['name'].endswith('.body')]
        self.assertEqual(len(calls),w.MAX_TIMED_CALLS)
        carriers=[r for k,r in self.sink.rows if k=='operation']
        self.assertEqual(len(carriers),869)
        self.assertEqual([r['name'] for r in carriers],[f'{i:04d}' for i in range(1,870)])
        self.assertTrue(all(len(w.json.dumps(r).encode())<=w.OPERATION_BYTES for r in carriers))
    def test_wrong_installed_identity_stops_before_builds(self):
        self.session.installed['head']=None
        with self.assertRaises(w.DiagnosticFailure):self.run_worker()
        self.assertEqual(self.session.log,[])
    def test_incomplete_capability_blocks_parser(self):
        original=self.session.collect_selected
        def incomplete(selection):
            result=original(selection);self.session.selected_document['selected_sections_complete']=False;return result
        self.session.runtime.collect_downstream_diagnostic=incomplete
        with self.assertRaisesRegex(w.DiagnosticFailure,'complete original selected'):self.run_worker()
        self.assertNotIn('capture-parser-runtime',self.session.log)
        self.assertTrue(self.session.poisoned)
    def test_wrong_selected_roster_blocks_parser(self):
        self.session.state['rosters']['lifecycle'].pop()
        with self.assertRaisesRegex(w.DiagnosticFailure,'exact original selected host roster'):self.run_worker()
        self.assertNotIn('capture-parser-runtime',self.session.log)
    def test_parser_omission_and_reordering_rejected(self):
        for stages in (list(w.PARSER_SEQUENCE[:-1]),list(reversed(w.PARSER_SEQUENCE)),list(w.PARSER_SEQUENCE)+['compare']):
            row=self.session.verify_parser_runtime(None);row['stages']=stages
            with self.assertRaises(w.DiagnosticFailure):w.parser_summary(row)
    def test_post_parser_public_drift_is_first_failure(self):
        original=self.session.verify_selected;fault=OSError('post parser selected drift')
        def verify(result):
            if 'compare' in self.session.log:raise fault
            return original(result)
        self.session.runtime.verify_downstream_diagnostic=verify
        with self.assertRaises(OSError) as caught:self.run_worker()
        self.assertIs(caught.exception,fault);self.assertTrue(self.session.poisoned)
    def test_primary_call_error_beats_sink_and_malformed_metrics(self):
        m=self.measurements();fault=ValueError('first call')
        m.telemetry=lambda:{'monitor':{'bad':'missing timing'},'audit_attempts':[]}
        self.sink.failure=lambda *a:(_ for _ in ()).throw(OSError('sink secondary'))
        with self.assertRaises(ValueError) as caught:m.call('bad',lambda:(_ for _ in ()).throw(fault))
        self.assertIs(caught.exception,fault)
        self.assertTrue(fault.__notes__)
    def test_enter_success_then_post_tick_failure_exits(self):
        m=self.measurements();fault=OSError('post entry tick');ticks=[0]
        def tick():
            ticks[0]+=1
            if ticks[0]==2:raise fault
        m.tick=tick
        manager=mock.Mock();manager.__enter__=mock.Mock();manager.__exit__=mock.Mock(return_value=False)
        with self.assertRaises(OSError) as caught:
            with m.context('test',manager):self.fail('body must not run')
        self.assertIs(caught.exception,fault);manager.__exit__.assert_called_once()
    def test_enter_success_then_sink_failure_exits(self):
        m=self.measurements();fault=OSError('entry sink')
        self.sink.failure=lambda kind,row:(_ for _ in ()).throw(fault) if row.get('name')=='test.enter' else None
        manager=mock.Mock();manager.__enter__=mock.Mock();manager.__exit__=mock.Mock(return_value=False)
        with self.assertRaises(OSError):
            with m.context('test',manager):self.fail('body must not run')
        manager.__exit__.assert_called_once()
    def test_failed_enter_does_not_exit_but_records_failure(self):
        m=self.measurements();fault=OSError('enter first')
        manager=mock.Mock();manager.__enter__=mock.Mock(side_effect=fault);manager.__exit__=mock.Mock(return_value=False)
        with self.assertRaises(OSError):
            with m.context('test',manager):pass
        manager.__exit__.assert_not_called()
        self.assertFalse(self.sink.rows[-1][1]['succeeded'])
    def test_body_exit_sink_multi_failure_preserves_body(self):
        m=self.measurements();fault=ValueError('body first')
        manager=mock.Mock();manager.__enter__=mock.Mock();manager.__exit__=mock.Mock(side_effect=OSError('exit second'))
        self.sink.failure=lambda kind,row:(_ for _ in ()).throw(RuntimeError('sink third')) if row.get('name')!='test.enter' else None
        with self.assertRaises(ValueError) as caught:
            with m.context('test',manager):raise fault
        self.assertIs(caught.exception,fault);manager.__exit__.assert_called_once()
        self.assertTrue(any('exit' in s for s in fault.__notes__))
    def test_cleanup_telemetry_failure_does_not_skip_exit(self):
        m=self.measurements();fault=ValueError('body');calls=[0]
        def telemetry():
            calls[0]+=1
            if calls[0]>=3:raise OSError('telemetry')
            return {'monitor':None}
        m.telemetry=telemetry
        manager=mock.Mock();manager.__enter__=mock.Mock();manager.__exit__=mock.Mock(return_value=False)
        with self.assertRaises(ValueError):
            with m.context('test',manager):raise fault
        manager.__exit__.assert_called_once()
    def test_documented_canonical_priority_replacement(self):
        m=self.measurements();body=ValueError('semantic');klass=type('InfrastructureFailure',(RuntimeError,),{})
        replacement=klass('preservation');replacement.phase='read-boundary-preservation';replacement.preceding_semantic=body
        m.runtime=types.SimpleNamespace(InfrastructureFailure=klass)
        manager=mock.Mock();manager.__enter__=mock.Mock();manager.__exit__=mock.Mock(side_effect=replacement)
        with self.assertRaises(klass) as caught:
            with m.context('test',manager):raise body
        self.assertIs(caught.exception,replacement)
    def test_preservation_marker_failure_still_poison_and_failure_audit(self):
        fault=OSError('case first')
        self.session.runtime.collect_downstream_diagnostic=lambda *a:(_ for _ in ()).throw(fault)
        self.sink.failure=lambda kind,row:(_ for _ in ()).throw(RuntimeError('phase sink')) if kind=='phase' and row['group']=='preservation' else None
        with self.assertRaises(OSError) as caught:self.run_worker()
        self.assertIs(caught.exception,fault);self.assertTrue(self.session.poisoned)
        self.assertEqual(self.session.failure_audits,1)
    def test_unissued_operation_is_unverified(self):
        self.session.check_deadline(self.session.invocation)
        self.session.runs[self.session.invocation]['next']=1
        MemoryPath.members['/live/operations']=[MemoryPath('/live/operations/0001')]
        report=w.operation_evidence(self.session.prep,self.session.invocation)
        self.assertEqual(report['status'],'UNVERIFIED');self.assertEqual(report['unissued_directories'],['0001'])
    def test_producer_deadline_and_telemetry_are_not_mutated(self):
        self.run_worker()
        self.assertEqual(self.session.runs[self.session.invocation]['deadline'],37000)
        terminal=[r for k,r in self.sink.rows if k=='terminal'][0]
        terminal['final_telemetry']['monitor']['timing']['attempts']=-1
        self.assertGreater(self.session.monitors[self.session.invocation]['count'],0)
    def test_original_setup_failure_remains_primary(self):
        fault=OSError('first public build')
        self.session.build.build=lambda *a:(_ for _ in ()).throw(fault)
        with self.assertRaises(OSError) as caught:self.run_worker()
        self.assertIs(caught.exception,fault);self.assertTrue(self.session.poisoned)
        self.assertFalse(any(s.startswith('parser-') for s in self.session.log))
    def test_late_operation_evidence_failure_poisons_original(self):
        fault=OSError('late operation evidence')
        with mock.patch.object(w,'operation_evidence',side_effect=fault):
            with self.assertRaises(OSError) as caught:self.run_worker()
        self.assertIs(caught.exception,fault);self.assertTrue(self.session.poisoned)
    def test_terminal_sink_failure_poisons_original(self):
        fault=OSError('terminal sink')
        self.sink.failure=lambda kind,row:(_ for _ in ()).throw(fault) if kind=='terminal' else None
        with self.assertRaises(OSError) as caught:self.run_worker()
        self.assertIs(caught.exception,fault);self.assertTrue(self.session.poisoned)
        self.assertTrue(self.cancel.sealed)
    def test_operation_inventory_checks_each_item_deadline(self):
        self.session.check_deadline(self.session.invocation)
        self.session.runs[self.session.invocation]['next']=1
        MemoryPath.members['/live/operations']=[MemoryPath('/live/operations/0001')]
        calls=[0];fault=TimeoutError('budget')
        def check():
            calls[0]+=1
            if calls[0]==2:raise fault
        with self.assertRaises(TimeoutError) as caught:
            w.operation_evidence(self.session.prep,self.session.invocation,check)
        self.assertIs(caught.exception,fault);self.assertEqual(calls,[2])

    def test_setup_parser_outside_public_replay_generator_native_inside(self):
        active=[];original_scope=self.session.scope
        @contextlib.contextmanager
        def replay(*a):
            active.append(True)
            try:yield
            finally:active.pop()
        self.session.prep.public_replay=replay
        original=self.session.verify_probe
        def verify(record,*a,**k):
            kind=record.doc['operation_kind']
            self.assertEqual(bool(active),kind in ('generator','public-runtime','public-native'))
            return original(record,*a,**k)
        self.session.prep.verify_probe=verify
        self.run_worker()
    def test_original_stream_drift_rejects(self):
        original=self.session.complete_parser_runtime
        def complete(handle):
            result=original(handle);next(iter(self.session.probes)).doc['stdout']['sha256']='0'*64;return result
        self.session.parser.complete_runtime=complete
        with self.assertRaisesRegex(w.DiagnosticFailure,'stream preservation'):self.run_worker()
    def test_duplicate_original_process_rejected(self):
        original=self.session.complete_parser_runtime
        def complete(handle):
            result=original(handle);records=list(self.session.probes)
            records[1].doc.update(pid=records[0].doc['pid'],started_ns=records[0].doc['started_ns']);return result
        self.session.parser.complete_runtime=complete
        with self.assertRaisesRegex(w.DiagnosticFailure,'unique original'):self.run_worker()
    def test_operation_carrier_overflow_never_truncates(self):
        self.session.check_deadline(self.session.invocation);self.session.add_operations({'probe':1})
        record=next(iter(self.session.probes));record.doc['failure']='x'*20000
        with self.assertRaisesRegex(w.DiagnosticFailure,'bounded detached'):
            w.operation_evidence(self.session.prep,self.session.invocation)
    def test_record_roster_overflow_rejected(self):
        self.session.check_deadline(self.session.invocation);self.session.add_operations({'probe':870})
        with self.assertRaisesRegex(w.DiagnosticFailure,'bounded original operation count'):
            w.operation_evidence(self.session.prep,self.session.invocation)
    def test_cancellation_is_original_and_no_work_resumes(self):
        first=BaseException('original cancellation')
        actual=self.session.produce
        def produce(*a,**k):
            value=actual(*a,**k);self.cancel.error=first;raise first
        self.session.prep.produce=produce
        with self.assertRaises(BaseException) as caught:self.run_worker()
        self.assertIs(caught.exception,first);self.assertTrue(self.session.poisoned)
        self.assertFalse(any(s.startswith('public-') or s.startswith('parser-') for s in self.session.log))
    def test_malformed_metrics_reject_success_with_bounded_fallback(self):
        m=self.measurements();m.telemetry=lambda:{'monitor':{'bad':1},'audit_attempts':[]}
        with self.assertRaises(KeyError):m.call('good-body-bad-metrics',lambda:None)
        self.assertFalse(self.sink.rows[-1][1]['telemetry_verified'])

    def test_no_deadline_extension_between_phases(self):
        m=self.measurements()
        with m.phase('selected-public'):pass
        origin=m.work_started;self.clock.now=1000+10000
        with m.phase('parser'):self.assertEqual(m.deadline,1000+20400)
        self.assertEqual(m.work_started,origin)
        self.clock.now=1000+20401
        with self.assertRaises(w.DiagnosticFailure):
            with m.phase('parser'):pass
    def test_original_failure_beats_late_record_overflow(self):
        fault=OSError('selected first')
        def fail(selection):
            self.session.add_operations({'probe':826});raise fault
        self.session.runtime.collect_downstream_diagnostic=fail
        with self.assertRaises(OSError) as caught:self.run_worker()
        self.assertIs(caught.exception,fault)


if __name__=='__main__':unittest.main(verbosity=2)
