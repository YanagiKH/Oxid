"""Wholly mocked watchdog controls. No child, live owner, signal or build runs."""
import ast
import contextlib
import json
from pathlib import Path
import signal
import sys
import types
import unittest
from unittest import mock

sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent
SOURCE=(HERE/'launch.py').read_bytes()
q=types.ModuleType('fresh_owner_launch_controls'); q.__file__=str(HERE/'launch.py')
exec(compile(SOURCE,q.__file__,'exec'),q.__dict__)


def row(pid,ppid=10,start=None,state='S',group=None):
    return dict(pid=pid,ppid=ppid,start_ticks=pid if start is None else start,
                pgid=pid if group is None else group,sid=pid if group is None else group,state=state)


def report():
    return dict(first_error=None,secondary_errors=[],signals=[],returncode=None)


def ownership():
    state=q.ownership_state(row(10,1,start=1))
    state.update(producer=(20,20),owned={(20,20):dict(identity=row(20),fd=120,reaped=False,signals=[])},identity_count=1)
    return state


def marker(name,event,t,started=10,work=None,phase_start=None,error=None):
    group=dict(q.PHASES)[name]
    return dict(q.FLAGS,event=event,name=name,group=group,monotonic=t,
                aggregate_started_monotonic=started,work_started_monotonic=work,
                deadline_monotonic=q.phase_deadline(name,group,t if phase_start is None else phase_start,started,work),
                error=error)


def lines(*items):
    return b''.join(json.dumps(item).encode()+b'\n' for item in items)


class SourceAndPhases(unittest.TestCase):
    def test_capacity_functions_exact_accepted_source(self):
        source=(HERE.parent/'package-unit4-launcher-capacity-v1'/'launch.py').read_text()
        old={n.name:ast.dump(n,include_attributes=False) for n in ast.parse(source).body if isinstance(n,ast.FunctionDef)}
        new={n.name:ast.dump(n,include_attributes=False) for n in ast.parse(SOURCE).body if isinstance(n,ast.FunctionDef)}
        for name in ('capacity_identity','capacity_open','capacity_sample','_capacity_sample','capacity_close'):
            self.assertEqual(old[name],new[name],name)
        self.assertEqual(q.CAPACITY_GROWTH_BYTES+q.CAPACITY_ALARM_BYTES,5502926848)
        self.assertEqual(q.CAPACITY_GROWTH_INODES,40960)

    def test_no_numeric_pid_or_group_signals_or_early_poll(self):
        tree=ast.parse(SOURCE)
        names=[ast.unparse(n.func) for n in ast.walk(tree) if isinstance(n,ast.Call)]
        self.assertNotIn('os.kill',names); self.assertNotIn('os.killpg',names)
        self.assertNotIn('child.poll',names); self.assertNotIn('child.wait',names)
        self.assertEqual(names.count('subprocess.Popen'),1)

    def test_execute_absence_is_effects_blocked(self):
        with mock.patch.object(q,'validate') as validate, mock.patch.object(q.subprocess,'Popen') as popen:
            with self.assertRaises(q.Rejected):q.run('/held','0'*64)
        validate.assert_not_called(); popen.assert_not_called()

    def test_held_review_is_effects_blocked(self):
        value=dict(schema='oxid-fresh-downstream-diagnostic-review-v1',status='HELD',manifest={},launcher={},
                   limits=q.LIMITS,capacity=q.CAPACITY,environment={},ownership_limits=q.OWNERSHIP_LIMITS,no_automatic_retry=True)
        raw=q.canonical(value)
        with mock.patch.object(q,'body',return_value=raw),mock.patch.object(q,'pinned') as pin:
            with self.assertRaisesRegex(q.Rejected,'HELD'):q.validate('/held',q.digest(raw))
        pin.assert_not_called()

    def test_complete_phase_and_remaining_work_budget(self):
        raw=lines(marker('setup','start',10),marker('setup','end',100,phase_start=10),
                  marker('selected-public','start',100,work=100),
                  marker('selected-public','end',110,work=100,phase_start=100))
        result=q.phase_state(raw,10,111)
        self.assertEqual(result['deadline'],5500)
        self.assertIsNone(result['active'])

    def test_synchronous_phase_deadline(self):
        raw=lines(marker('setup','start',10),marker('setup','end',100,phase_start=10),
                  marker('selected-public','start',100,work=100))
        self.assertEqual(q.phase_state(raw,10,401)['deadline'],5500)

    def test_parser_and_preservation_cannot_extend_absolute_caps(self):
        self.assertEqual(q.phase_deadline('parser','parser',10000,10,100),20410)
        self.assertEqual(q.phase_deadline('preservation','preservation',21000,10,100),21550)
        self.assertEqual(q.LIMITS['aggregate_seconds'],21600)
        self.assertEqual(q.OWNERSHIP_LIMITS,{'live_pidfds':128,'lifetime_identities':65536})
        self.assertEqual(q.TOMBSTONE_MEMORY_BUDGET,32*1024**2)

    def test_duplicate_start_cannot_reset(self):
        with self.assertRaises(q.Rejected):q.phase_state(lines(marker('setup','start',10),marker('setup','start',20)),10,20)

    def test_phase_end_cannot_claim_expired_success(self):
        with self.assertRaises(q.Rejected):q.phase_state(lines(marker('setup','start',10),marker('setup','end',2411,phase_start=10)),10,2411)

    def test_failure_can_go_directly_to_preservation(self):
        raw=lines(marker('setup','start',10),marker('setup','end',20,phase_start=10,error={'message':'first'}),
                  marker('preservation','start',20))
        self.assertEqual(q.phase_state(raw,10,21)['deadline'],1160)

    def test_deadline_clock_and_unknown_field_changes_reject(self):
        for changes in ({'aggregate_started_monotonic':9},{'deadline_monotonic':9999},
                        {'work_started_monotonic':20},{'extra':True},{'monotonic':float('nan')}):
            with self.subTest(changes=changes),self.assertRaises((q.Rejected,ValueError)):
                q.phase_state(lines({**marker('setup','start',10),**changes}),10,20)

    def test_actual_worker_phase_through_actual_driver_sink(self):
        modules=[]
        for filename in ('driver.py','worker.py'):
            path=HERE/filename; module=types.ModuleType('test_cross_'+filename[:-3]);module.__file__=str(path)
            exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__);modules.append(module)
        driver,worker=modules
        sink=driver.Sink.__new__(driver.Sink);sink.terminal=False;written=[]
        sink._write=lambda name,body,**kwargs:written.append((name,body))
        clock=[10.0]
        with mock.patch.object(worker.time,'monotonic',side_effect=lambda:clock[0]):
            work=worker.Measurements(sink,10,mock.Mock())
            with self.assertRaisesRegex(ValueError,'original setup'):
                with work.phase('setup'):
                    clock[0]=11;raise ValueError('original setup')
            with work.phase('preservation'):clock[0]=12
        raw=b''.join(body for name,body in written if name=='phase.jsonl')
        state=q.phase_state(raw,10,12)
        self.assertEqual(state['completed'],['setup','preservation'])
        self.assertEqual(state['deadline'],1151)

    def test_body_open_is_nonblocking_and_fifo_substitution_rejected(self):
        before=types.SimpleNamespace(st_mode=0o100600,st_size=0)
        opened=types.SimpleNamespace(st_mode=0o010600)
        path=mock.Mock();path.lstat.return_value=before
        with mock.patch.object(q,'real',return_value=path),mock.patch.object(q.os,'open',return_value=30) as opened_file,\
             mock.patch.object(q.os,'fstat',return_value=opened),mock.patch.object(q.os,'read') as read,\
             mock.patch.object(q.os,'close') as close:
            with self.assertRaisesRegex(q.Rejected,'descriptor'):q.body('/synthetic')
        self.assertTrue(opened_file.call_args.args[1]&q.os.O_NONBLOCK)
        read.assert_not_called();close.assert_called_once_with(30)

    def test_body_close_failure_preserves_read_failure(self):
        info=types.SimpleNamespace(st_mode=0o100600,st_size=0)
        path=mock.Mock();path.lstat.return_value=info;first=OSError('body read first')
        with mock.patch.object(q,'real',return_value=path),mock.patch.object(q,'input_identity',return_value=()),\
             mock.patch.object(q.os,'open',return_value=30),mock.patch.object(q.os,'fstat',return_value=info),\
             mock.patch.object(q.os,'read',side_effect=first),mock.patch.object(q.os,'close',side_effect=OSError('body close second')):
            with self.assertRaises(OSError) as caught:q.body('/synthetic')
        self.assertIs(caught.exception,first);self.assertIn('body close second',first.__notes__[0])

    def test_phase_reader_ignores_partial_suffix(self):
        data=b'{"x":1}\n{"event":"end"'
        info=types.SimpleNamespace(st_mode=0o100600,st_size=len(data),st_dev=1,st_ino=2)
        path=mock.Mock(); path.lstat.return_value=info
        with mock.patch.object(q.os.path,'lexists',return_value=True),mock.patch.object(q.os,'open',return_value=30),\
             mock.patch.object(q.os,'fstat',return_value=info),mock.patch.object(q.os,'read',return_value=data),\
             mock.patch.object(q.os,'close'):
            self.assertEqual(q.phase_bytes(path),b'{"x":1}\n')
            with self.assertRaises(q.Rejected):q.phase_bytes(path,b'previous\n')

    def test_phase_close_failure_preserves_original(self):
        path=mock.Mock(); first=OSError('read first')
        with mock.patch.object(q.os.path,'lexists',return_value=True),mock.patch.object(q.os,'open',return_value=30),\
             mock.patch.object(q.os,'fstat',side_effect=first),mock.patch.object(q.os,'close',side_effect=OSError('close second')):
            with self.assertRaises(OSError) as caught:q.phase_bytes(path)
        self.assertIs(caught.exception,first); self.assertIn('close second',first.__notes__[0])


class OwnershipControls(unittest.TestCase):
    def setUp(self):
        self.state=ownership(); self.report=report(); self.clock=[0.0]
        self.enterContext(mock.patch.object(q.time,'monotonic',side_effect=lambda:self.clock[0]))
        self.enterContext(mock.patch.object(q.time,'sleep',side_effect=lambda amount:self.clock.__setitem__(0,self.clock[0]+amount)))
        self.pidfd=self.enterContext(mock.patch.object(q.os,'pidfd_open',return_value=121))
        self.signal=self.enterContext(mock.patch.object(q.signal,'pidfd_send_signal'))
        self.close=self.enterContext(mock.patch.object(q.os,'close'))
        self.wait=self.enterContext(mock.patch.object(q.os,'waitid'))
        self.exited=self.enterContext(mock.patch.object(q,'pidfd_exited',return_value=False))

    def test_acquire_rechecks_birth_and_parent(self):
        with mock.patch.object(q,'process_identity',side_effect=[row(21,20),row(20)]):
            q.acquire_owned(self.state,row(21,20),row(20))
        self.assertEqual(self.state['owned'][(21,21)]['fd'],121)
        self.close.assert_not_called()

    def test_reused_pid_rejected_and_descriptor_closed(self):
        with mock.patch.object(q,'process_identity',return_value=row(21,20,start=999)):
            with self.assertRaises(q.Rejected):q.acquire_owned(self.state,row(21,20),row(20))
        self.close.assert_called_once_with(121)
        self.assertNotIn((21,21),self.state['owned'])

    def test_producer_descriptor_survives_initial_proc_failure(self):
        self.state['producer']=None
        with mock.patch.object(q,'process_identity',side_effect=OSError('proc unavailable')):
            with self.assertRaises(OSError):q.retain_producer(self.state,20)
        self.assertEqual(self.state['bootstrap_pidfd'],121)
        self.close.assert_not_called()

    def test_orphan_adoption_independent_nested_session(self):
        supervisor=self.state['supervisor']; child=row(21,10,group=21)
        with mock.patch.object(q,'process_table',return_value={10:supervisor,20:row(20),21:child}),\
             mock.patch.object(q,'process_identity',side_effect=[child,supervisor]):
            members=q.discover_owned(self.state,1)
        self.assertEqual({m['pid'] for m in members},{20,21})
        self.assertIn((21,21),self.state['owned'])

    def test_term_targets_only_original_producer(self):
        self.state['owned'][(21,21)]=dict(identity=row(21,20),fd=121,reaped=False,signals=[])
        q.signal_owned(self.state,signal.SIGTERM,self.report,producer_only=True)
        self.signal.assert_called_once_with(120,signal.SIGTERM,None,0)

    def test_no_signal_after_reap_even_if_pid_reused(self):
        self.state['owned'][(20,20)]['reaped']=True
        q.signal_owned(self.state,signal.SIGKILL,self.report)
        self.signal.assert_not_called(); self.exited.assert_not_called()

    def test_signal_failure_does_not_skip_other_owned_identities(self):
        self.state['owned'][(21,21)]=dict(identity=row(21,20),fd=121,reaped=False,signals=[])
        self.signal.side_effect=[OSError('first signal'),None]
        q.signal_owned(self.state,signal.SIGKILL,self.report)
        self.assertEqual(self.signal.call_count,2)
        self.assertIn('(20, 20)',self.report['first_error']['phase'])

    def test_peek_is_wnowait_by_kernel_identity(self):
        q.peek_producer(self.state)
        self.wait.assert_called_once_with(q.os.P_PIDFD,120,q.os.WEXITED|q.os.WNOHANG|q.os.WNOWAIT)

    def test_discovery_failure_still_kills_retained_identity(self):
        with mock.patch.object(q,'discover_owned',side_effect=OSError('discovery first')),\
             mock.patch.object(q,'process_table',return_value={}):
            result=q.cleanup_owned(self.state,self.report,.1)
        self.assertFalse(result['complete'])
        self.assertTrue(any(call.args[1]==signal.SIGKILL for call in self.signal.call_args_list))
        self.assertEqual(self.report['first_error']['message'],'discovery first')
        self.wait.assert_not_called()

    def test_fixed_point_then_kill_before_any_reap(self):
        events=[]; snapshots=iter([[row(20,state='S')],[row(20,state='T')],[row(20,state='T')],[row(20,state='Z')],[]])
        def discover(*args):return next(snapshots)
        def sig(fd,signum,*_):
            events.append(('signal',signum))
            if signum==signal.SIGKILL:self.exited.return_value=True
        self.signal.side_effect=sig
        def wait(*args):events.append(('wait',args[2])); return types.SimpleNamespace(si_pid=20,si_status=0,si_code=q.os.CLD_EXITED)
        self.wait.side_effect=wait
        with mock.patch.object(q,'discover_owned',side_effect=discover),mock.patch.object(q,'process_table',return_value={10:self.state['supervisor']}):
            result=q.cleanup_owned(self.state,self.report,1)
        self.assertTrue(result['complete']); self.assertTrue(result['descendant_absence_verified'])
        self.assertLess(events.index(('signal',signal.SIGKILL)),next(i for i,e in enumerate(events) if e[0]=='wait'))
        self.assertTrue(self.state['owned'][(20,20)]['reaped'])
        before=self.signal.call_count;q.signal_owned(self.state,signal.SIGKILL,self.report)
        self.assertEqual(before,self.signal.call_count)

    def test_late_fork_discovers_before_fixed_point(self):
        count=[0]
        def discover(*_):
            count[0]+=1
            if count[0]==1:return [row(20,state='S')]
            if count[0]==2:
                self.state['owned'][(21,21)]=dict(identity=row(21,20),fd=121,reaped=False,signals=[])
                return [row(20,state='T'),row(21,20,state='S')]
            if count[0] in (3,4):return [row(20,state='T'),row(21,20,state='T')]
            if count[0]==5:return [row(20,state='Z'),row(21,10,state='Z')]
            return []
        dead=set()
        self.exited.side_effect=lambda fd:fd in dead
        def sig(fd,signum,*_):
            if signum==signal.SIGKILL:dead.add(fd)
        self.signal.side_effect=sig
        self.wait.side_effect=lambda mode,fd,flags:types.SimpleNamespace(si_pid=fd-100,si_status=0,si_code=q.os.CLD_EXITED)
        with mock.patch.object(q,'discover_owned',side_effect=discover),mock.patch.object(q,'process_table',return_value={10:self.state['supervisor']}):
            result=q.cleanup_owned(self.state,self.report,1)
        self.assertTrue(result['complete'])
        self.assertIn(mock.call(121,signal.SIGSTOP,None,0),self.signal.call_args_list)
        self.assertIn(mock.call(121,signal.SIGKILL,None,0),self.signal.call_args_list)

    def test_terminal_absence_retirement_is_compact_and_keeps_producer(self):
        self.state['owned'][(21,21)]=dict(identity=row(21,20),fd=121,reaped=False,signals=[])
        self.state['identity_count']=2; self.exited.return_value=True
        q.retire_exited(self.state,{})
        self.assertIn((20,20),self.state['owned']);self.assertNotIn((21,21),self.state['owned'])
        self.assertEqual(self.state['retired'],{(21,21)})
        self.assertEqual(self.state['retired_digest'].hexdigest(),q.digest(b'21:21\n'))
        self.close.assert_called_once_with(121)
        q.signal_owned(self.state,signal.SIGKILL,self.report)
        self.signal.assert_not_called()

    def test_exit_without_exact_absence_does_not_retire(self):
        self.state['owned'][(21,21)]=dict(identity=row(21,20),fd=121,reaped=False,signals=[])
        self.exited.return_value=True
        q.retire_exited(self.state,{21:row(21,20,state='Z')})
        self.assertIn((21,21),self.state['owned']);self.close.assert_not_called()

    def test_retirement_close_failure_removes_signal_eligibility(self):
        self.state['owned'][(21,21)]=dict(identity=row(21,20),fd=121,reaped=False,signals=[])
        self.exited.return_value=True;self.close.side_effect=OSError('uncertain close')
        with self.assertRaises(OSError):q.retire_exited(self.state,{})
        self.assertIn((21,21),self.state['retired']);self.assertNotIn((21,21),self.state['owned'])
        self.assertEqual(self.state['retirement_close_errors'],1)
        self.exited.return_value=False;q.signal_owned(self.state,signal.SIGKILL,self.report)
        self.assertNotIn(mock.call(121,signal.SIGKILL,None,0),self.signal.call_args_list)
        with self.assertRaises(q.Rejected):q.acquire_owned(self.state,row(21,20),row(20))

    def test_retired_pid_new_birth_needs_new_kernel_capture(self):
        self.state['retired'].add((21,21))
        with mock.patch.object(q,'process_identity',side_effect=[row(21,20,start=22),row(20)]):
            q.acquire_owned(self.state,row(21,20,start=22),row(20))
        self.assertIn((21,22),self.state['owned']);self.assertNotIn((21,21),self.state['owned'])
        self.pidfd.assert_called_once_with(21,0)

    def test_exit_with_live_descendant_adoption(self):
        self.state['owned'][(21,21)]=dict(identity=row(21,20),fd=121,reaped=False,signals=[])
        self.state['identity_count']=2;self.exited.side_effect=lambda fd:fd==121
        supervisor=self.state['supervisor'];grandchild=row(22,10)
        with mock.patch.object(q,'process_table',return_value={10:supervisor,20:row(20),22:grandchild}),\
             mock.patch.object(q,'process_identity',side_effect=[grandchild,supervisor]):
            q.discover_owned(self.state,1)
        self.assertIn((21,21),self.state['retired']);self.assertIn((22,22),self.state['owned'])

    def test_separate_live_and_lifetime_resource_ceiling(self):
        self.state['identity_count']=65536
        with self.assertRaisesRegex(q.Rejected,'lifetime'):q.acquire_owned(self.state,row(21,20),row(20))
        self.pidfd.assert_not_called()
        self.state['identity_count']=128
        for pid in range(100,227):self.state['owned'][(pid,pid)]=dict(identity=row(pid),fd=pid,reaped=False,signals=[])
        with self.assertRaisesRegex(q.Rejected,'live pidfd'):q.acquire_owned(self.state,row(21,20),row(20))
        self.pidfd.assert_not_called()

    def test_original_failure_survives_cleanup_errors(self):
        q.remember(self.report,'worker',ValueError('original work'))
        with mock.patch.object(q,'discover_owned',side_effect=OSError('cleanup')):
            q.cleanup_owned(self.state,self.report,.1)
        self.assertEqual(self.report['first_error']['message'],'original work')
        self.assertTrue(self.report['secondary_errors'])


class RunControls(unittest.TestCase):
    def setUp(self):
        self.clock=[10.0]
        self.enterContext(mock.patch.object(q.time,'monotonic',side_effect=lambda:self.clock[0]))
        self.sleep=self.enterContext(mock.patch.object(q.time,'sleep',side_effect=lambda amount:self.clock.__setitem__(0,self.clock[0]+amount)))
        pin=dict(path='/synthetic/input',bytes=0,sha256='0'*64)
        pins={name:dict(pin) for name in ('driver','worker','session','python','request','installed_admission','capacity_launcher','capacity_policy')}
        self.manifest=dict(pins=pins,paths=dict(evidence_root='/synthetic/evidence',run_root='/synthetic/run'),
                           installed=dict(repository='/synthetic/repo'),environment={})
        self.auth=dict(manifest=self.manifest,request=dict(run_root='/synthetic/run'),driver=b'# inert',
                       review=dict(manifest=pin,launcher=pin),review_path='/synthetic/review',review_sha256='0'*64)
        self.enterContext(mock.patch.object(q,'validate',return_value=self.auth))
        self.enterContext(mock.patch.object(q,'pinned',side_effect=lambda p,keep_fd=False,checkpoint=lambda:None:(b'',99) if keep_fd else b''))
        self.enterContext(mock.patch.object(q.os,'access',return_value=True))
        self.enterContext(mock.patch.object(q,'become_subreaper'))
        self.enterContext(mock.patch.object(q,'process_identity',return_value=row(10,1,start=1)))
        self.enterContext(mock.patch.object(q.signal,'signal',return_value=signal.SIG_DFL))
        self.enterContext(mock.patch.object(q.Path,'mkdir'))
        self.stream=mock.Mock()
        self.enterContext(mock.patch.object(q.Path,'open',return_value=self.stream))
        self.enterContext(mock.patch.object(q.os,'fsync'))
        self.enterContext(mock.patch.object(q.os,'close'))
        self.enterContext(mock.patch.object(q,'capacity_open',return_value={}))
        self.capacity=self.enterContext(mock.patch.object(q,'capacity_sample',return_value=None))
        self.enterContext(mock.patch.object(q,'capacity_close'))
        self.enterContext(mock.patch.object(q,'command',return_value=['/synthetic/python']))
        self.child=types.SimpleNamespace(pid=20,returncode=None)
        self.popen=self.enterContext(mock.patch.object(q.subprocess,'Popen',return_value=self.child))
        def retain(state,pid):
            state.update(ownership());return row(20)
        self.enterContext(mock.patch.object(q,'retain_producer',side_effect=retain))
        self.write=self.enterContext(mock.patch.object(q,'exclusive_json'))
        self.peek=self.enterContext(mock.patch.object(q,'peek_producer',return_value=types.SimpleNamespace(si_status=0)))
        self.phase=self.enterContext(mock.patch.object(q,'phase_bytes',return_value=b''))
        self.discover=self.enterContext(mock.patch.object(q,'discover_owned',return_value=[]))
        self.signal=self.enterContext(mock.patch.object(q,'signal_owned'))
        self.census=self.enterContext(mock.patch.object(q,'output_bytes',return_value=0))
        self.cleanup_time=[]
        def cleanup(state,report,deadline):
            self.cleanup_time.append((self.clock[0],deadline)); report['returncode']=0
            return dict(complete=True)
        self.cleanup=self.enterContext(mock.patch.object(q,'cleanup_owned',side_effect=cleanup))
        def receipt(root,report):
            report['worker_terminal']=dict(q.FLAGS,failure=None,status='PARTIAL_DOWNSTREAM_DIAGNOSTIC_COMPLETE',
                public_operations_completed=680,native_operations_completed=68,build_results_retained=8,operation_cleanup={'status':'VERIFIED','original_next':869,'operation_carriers':869},
                original_record_preservation={'original_records_readmitted':869,'unique_original_processes':869,'streams_verified_per_record':2},
                selected={'selected_sections_complete':True,'selected_sections':['lifecycle','guards'],'local_cases':{'lifecycle':318,'guards':22}},
                parser={'observations':638,'stages':['passivity-debug','collect-debug','passivity-release','collect-release','u8','phase','compare']},
                preservation={'source':'VERIFIED'})
        self.receipt=self.enterContext(mock.patch.object(q,'worker_result',side_effect=receipt))
        self.enterContext(mock.patch.object(q,'body',return_value=b''))
        self.enterContext(mock.patch.object(q,'digest',return_value='0'*64))

    def run_fake(self):return q.run('/synthetic/review','0'*64,execute=True)

    def test_normal_exit_exactly_one_child_and_no_cancellation(self):
        result=self.run_fake()
        self.assertTrue(result['producer_succeeded']);self.assertEqual(self.popen.call_count,1)
        self.signal.assert_not_called();self.assertEqual(self.cleanup_time,[(10,70)])
        self.assertEqual(self.child.returncode,0)

    def test_partial_worker_coverage_cannot_pass(self):
        original=self.receipt.side_effect
        for key in ('selected','parser','original_record_preservation','operation_cleanup'):
            def incomplete(root,report,key=key):
                original(root,report);report['worker_terminal'].pop(key)
            self.receipt.side_effect=incomplete
            self.assertFalse(self.run_fake()['producer_succeeded'])

    def test_setup_deadline_interrupts_while_no_progress(self):
        self.peek.side_effect=[None,None,None,types.SimpleNamespace(si_status=1)]
        self.clock[0]=10
        def phase(*_):self.clock[0]=2411;return b''
        self.phase.side_effect=phase
        result=self.run_fake()
        self.assertFalse(result['producer_succeeded'])
        self.assertIn('deadline',result['first_error']['message'])
        self.assertEqual(self.signal.call_args_list[0].args[1],signal.SIGTERM)
        self.assertTrue(self.signal.call_args_list[0].kwargs['producer_only'])

    def test_capacity_alarm_triggers_original_producer_unwind(self):
        self.peek.side_effect=[None,None,None,types.SimpleNamespace(si_status=1)]
        self.capacity.side_effect=[None,'capacity alarm',None,None]
        result=self.run_fake()
        self.assertEqual(result['first_error']['message'],'capacity alarm')
        self.assertEqual(self.signal.call_args_list[0].args[1],signal.SIGTERM)
        self.assertEqual(self.popen.call_count,1)

    def test_cancellation_window_never_extends_aggregate21540(self):
        self.peek.return_value=None
        self.phase.side_effect=lambda *_:(_ for _ in ()).throw(q.Rejected('first marker failure'))
        def trigger(*_,**kwargs):self.clock[0]=21551
        self.signal.side_effect=trigger
        result=self.run_fake()
        self.assertEqual(self.cleanup_time,[(21551,21610)])
        self.assertEqual(result['first_error']['message'],'first marker failure')

    def test_log_flush_failure_still_fsyncs_and_closes(self):
        self.stream.flush.side_effect=OSError('flush first')
        result=self.run_fake()
        self.assertFalse(result['producer_succeeded'])
        self.assertGreaterEqual(self.stream.close.call_count,2)
        self.assertGreaterEqual(self.stream.fileno.call_count,2)
        self.assertEqual(result['first_error']['message'],'flush first')

    def test_terminal_write_deadline_retains_failed_correction(self):
        writes=[]
        def write(path,value):
            writes.append((path.name,dict(value)))
            if path.name=='watchdog-terminal.json':self.clock[0]=21611
        self.write.side_effect=write
        with mock.patch('sys.stderr',new=mock.Mock()):result=self.run_fake()
        self.assertFalse(result['producer_succeeded'])
        correction=[value for name,value in writes if name=='watchdog-terminal-failure.json']
        self.assertEqual(len(correction),1)
        self.assertFalse(correction[0]['producer_succeeded'])
        self.assertIn('deadline',correction[0]['first_error']['message'])

    def test_terminal_census_deadline_retains_failed_correction(self):
        def census(root,*,terminal=False):
            if terminal:self.clock[0]=21611
            return 0
        self.census.side_effect=census
        with mock.patch('sys.stderr',new=mock.Mock()):result=self.run_fake()
        self.assertFalse(result['producer_succeeded'])
        self.assertIn('census',result['first_error']['message'])
        self.assertTrue(any(c.args[0].name=='watchdog-terminal-failure.json' for c in self.write.call_args_list))

    def test_cleanup_failure_preserves_initial_deadline_error(self):
        self.peek.side_effect=[None,None,types.SimpleNamespace(si_status=1)]
        self.phase.side_effect=ValueError('initial phase failure')
        self.cleanup.side_effect=RuntimeError('cleanup failure')
        result=self.run_fake()
        self.assertEqual(result['first_error']['message'],'initial phase failure')
        self.assertTrue(any(e['message']=='cleanup failure' for e in result['secondary_errors']))
        self.assertTrue(any(c.args[1]==signal.SIGKILL for c in self.signal.call_args_list))


if __name__=='__main__':unittest.main()
