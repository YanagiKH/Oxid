"""HELD fresh live-owner diagnostic orchestration. Import starts no work.

The separately reviewed driver authenticates the canonical installed session and
supplies its original cancellation owner plus a bounded external record sink.
This file never loads a package, fabricates authority, or launches independently.
"""
import contextlib
import collections
import hashlib
import json
import re
from pathlib import Path
import time

FLAGS = dict(diagnostic_only=True, local_complete=False, execution_qualified=False,
             collection_complete=False, section_complete=False)
EXPECTED_RECORDS = 869
EXPECTED_KINDS = {'probe':33, 'public-build':4, 'parser-helper':4,
                  'runtime-tool-version':3, 'generator':1, 'public-runtime':680,
                  'public-native':68, 'parser-passivity':2, 'parser-collect':2,
                  'parser-u8':18, 'parser-phase':54}
SELECTED_ROWS = {'lifecycle':{'execute':318, 'unavailable-host':462, 'excluded-host':548},
                 'guards':{'execute':22, 'unavailable-host':0, 'excluded-host':66}}
PARSER_SEQUENCE = ('passivity-debug','collect-debug','passivity-release','collect-release','u8','phase','compare')
PHASES = {'setup':'setup', 'selected-public':'selected-public', 'parser':'parser',
          'preservation':'preservation'}
MAX_TIMED_CALLS = 46  # 18 setup + 3 selected + 6 parser + 13 preservation + 6 scope enter/exit.
OPERATION_BYTES = 16384


class DiagnosticFailure(RuntimeError):
    pass


def need(value, message):
    if not value:
        raise DiagnosticFailure(message)


def error_row(error):
    try: message = str(error)[:2048]
    except BaseException: message = '<error text unavailable>'
    return {'type':type(error).__name__, 'message':message}


def note(primary, label, secondary):
    try: primary.add_note(label + ': ' + error_row(secondary)['message'])
    except BaseException: pass


def detached(value, maximum=131072):
    raw = json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    need(len(raw) <= maximum, 'bounded detached diagnostic metadata')
    return json.loads(raw)


class Measurements:
    def __init__(self, sink, started, cancellation):
        self.sink, self.started, self.cancellation = sink, started, cancellation
        self.api = self.invocation = self.state = self.runtime = None
        self.phase_name = None
        self.deadline = started + 2400
        self.work_started = None
        self.producer_deadline = None
        self.call_count = 0

    def emit(self, kind, value, primary=None):
        try: self.sink.emit(kind, detached({**FLAGS, **value}, OPERATION_BYTES if kind == 'operation' else 131072))
        except BaseException as error:
            if primary is None: raise
            note(primary, 'diagnostic '+kind+' preservation failed', error)

    def check(self):
        if self.phase_name != 'preservation': self.cancellation.check()
        need(time.monotonic() <= self.deadline, 'diagnostic original phase deadline')

    def tick(self):
        self.check()
        if self.invocation is not None:
            self.api['preparation'].check_deadline(self.invocation)
            self.api['preparation'].public_output_tick(self.invocation)

    def telemetry(self):
        if self.invocation is None:
            return {'producer_deadline':None, 'monitor':None, 'audit_attempts':[]}
        prep = self.api['preparation']
        owner = prep._RUNS.get(self.invocation)
        need(owner is not None, 'original active producer run missing')
        deadline = owner['deadline']
        if self.producer_deadline is None: self.producer_deadline = deadline
        need(deadline == self.producer_deadline, 'original producer deadline changed')
        monitor = prep._OUTPUT_MONITORS.get(self.invocation)
        if monitor is None:
            return {'producer_deadline':deadline, 'monitor':None, 'audit_attempts':[]}
        need(monitor['invocation'] is self.invocation and
             monitor['permit'] is self.state['runtime_permit'], 'original monitor association')
        attempts = monitor['audits']
        need(len(attempts) <= 16, 'original bounded audit history')
        return detached({'producer_deadline':deadline,
            'monitor':prep._public_output_report(monitor), 'audit_attempts':attempts,
            'history_limit':16, 'complete_intermediate_history_claim':False})

    def call(self, name, function, *, cleanup=False, pending=None):
        need(self.call_count < MAX_TIMED_CALLS and len(name) <= 160, 'bounded diagnostic call roster')
        self.call_count += 1
        before = after = None
        primary = metric_error = None
        result = None
        started = time.monotonic()
        try:
            if not cleanup: self.tick()
            try: before = self.telemetry()
            except BaseException as error:
                if not cleanup: raise
                metric_error = error  # Context exit must still be attempted.
            started = time.monotonic()
            result = function()
        except BaseException as error:
            primary = error
        try: ended = time.monotonic()
        except BaseException as error:
            ended = started
            if metric_error is None: metric_error = error
        try:
            if not cleanup: self.tick()
            after = self.telemetry()
        except BaseException as error:
            if metric_error is None: metric_error = error
            elif primary or pending: note(primary or pending,'additional post-call readback failed',error)
        if metric_error is not None:
            if primary or pending: note(primary or pending,'timing/service/readback failed',metric_error)
            else: primary = metric_error
        try:
            growth = truncated = None
            if before and after and before['monitor'] and after['monitor']:
                growth = after['monitor']['timing']['attempts'] - before['monitor']['timing']['attempts']
                truncated = growth > len(after['audit_attempts'])
            row = dict(name=name, phase=self.phase_name, monotonic=started,
                completed_monotonic=ended, seconds=ended-started, succeeded=primary is None,
                error=None if primary is None else error_row(primary), before=before, after=after,
                telemetry_verified=metric_error is None, audit_attempt_count_delta=growth, intermediate_audit_history_truncated=truncated)
            self.emit('call', row, primary or pending)
        except BaseException as error:
            if primary or pending:note(primary or pending,'post-call metric pipeline failed',error)
            else:primary=error
            self.emit('call',dict(name=name,phase=self.phase_name,monotonic=started,
                completed_monotonic=ended,succeeded=False,telemetry_verified=False,
                error=error_row(primary or pending),diagnostic_error=error_row(error)),primary or pending)
        if primary is not None: raise primary
        return result

    @contextlib.contextmanager
    def context(self, name, manager):
        entered=False
        primary=None
        body_started=time.monotonic()
        def enter():
            nonlocal entered
            value=manager.__enter__()
            entered=True  # Before any post-entry diagnostic tick or sink call.
            return value
        try:
            self.call(name+'.enter',enter)
            body_started=time.monotonic()
            yield
        except BaseException as error:
            primary=error
        finally:
            if entered:
                try:
                    self.emit('call',dict(name=name+'.body',phase=self.phase_name,
                        monotonic=body_started,completed_monotonic=time.monotonic(),
                        succeeded=primary is None,error=None if primary is None else error_row(primary)),primary)
                except BaseException as error:
                    if primary is None:primary=error
                    else:note(primary,'context body telemetry failed',error)
                args=(None,None,None) if primary is None else (type(primary),primary,primary.__traceback__)
                attempted=False
                def leave():
                    nonlocal attempted
                    attempted=True
                    result=manager.__exit__(*args)
                    need(not result,'canonical context must not suppress diagnostic failure')
                    return result
                exit_error=None
                try:self.call(name+'.exit',leave,cleanup=True,pending=primary)
                except BaseException as error:exit_error=error
                finally:
                    if not attempted:
                        try:leave()  # Diagnostics must never prevent canonical exit.
                        except BaseException as error:
                            if exit_error is not None:note(error,'exit instrumentation failed',exit_error)
                            exit_error=error
                if exit_error is not None:
                    replacement=(primary is not None and self.runtime is not None and
                        isinstance(exit_error,self.runtime.InfrastructureFailure) and
                        getattr(exit_error,'phase',None)=='read-boundary-preservation' and
                        getattr(exit_error,'preceding_semantic',None) is primary)
                    if primary is None or replacement:primary=exit_error
                    else:note(primary,'canonical context exit failed',exit_error)
        if primary is not None:raise primary

    @contextlib.contextmanager
    def phase(self, name):
        group = PHASES[name]
        now = time.monotonic()
        if group == 'setup': deadline = min(self.started+2400, self.started+20400)
        elif group in ('selected-public', 'parser'):
            if self.work_started is None: self.work_started = now
            deadline = min(now+(5400 if group == 'selected-public' else 12600), self.started+20400)
        else: deadline = min(now+1140, self.started+21540)
        self.phase_name, self.deadline = name, deadline
        base = dict(name=name,group=group,deadline_monotonic=deadline,
            aggregate_started_monotonic=self.started,
            work_started_monotonic=self.work_started)
        self.emit('phase',dict(base,event='start',monotonic=now,error=None))
        primary = None
        try:
            self.check()
            yield
            self.check()
        except BaseException as error:
            primary=error
            raise
        finally:
            self.emit('phase',dict(base,event='end',monotonic=time.monotonic(),
                error=None if primary is None else error_row(primary)),primary)


def retain_records(prep, invocation, handles):
    records = []
    for record, owner in prep._PROBES.items():
        need(len(records) < EXPECTED_RECORDS, 'bounded original live record roster')
        need(owner['invocation'] is invocation, 'foreign original probe in isolated producer')
        records.append(record)
    handles['original_records'] = records
    return records


def selected_summary(runtime, result, state):
    document = result.document()
    need(document['status'] == 'COMPLETE_SELECTED_OBSERVATIONS' and
         document['selected_sections_complete'] is True and
         document['selected_sections'] == ['lifecycle', 'guards'] and
         all(document[key] is False for key in ('collection_complete', 'local_complete', 'execution_qualified')),
         'complete original selected result required before parser')
    for section, expected in SELECTED_ROWS.items():
        counts = collections.Counter(row['scope'] for row in state['rosters'][section])
        need(dict(counts) == {k:v for k,v in expected.items() if v}, 'exact original selected host roster')
        report = document['reports'][section]
        need(report['required_local'] == expected['execute'] and report['unsupported_capability'] == 0,
             'complete selected local capability required')
    return {'status':document['status'], 'selected_sections_complete':True,
            'selected_sections':['lifecycle','guards'], 'local_cases':{'lifecycle':318,'guards':22}}


def parser_summary(document):
    need(type(document) is dict and document.get('schema') == 'oxid-package-source-parser-runtime-v1' and
         document.get('stages') == list(PARSER_SEQUENCE) and document.get('local_semantics_status') == 'pass' and
         document.get('execution_qualified') is False, 'complete unchanged parser runtime required')
    # The original compare stage enforces exactly 638 observations, not a runner-supplied subset.
    return {'stages':list(PARSER_SEQUENCE), 'observations':638, 'local_semantics_status':'pass',
            'comparison':document['comparison']}


def reverify_operations(m, runtime, state):
    prep,invocation=m.api['preparation'],m.invocation
    records=[]
    for record,owner in prep._PROBES.items():
        m.check()
        need(len(records)<EXPECTED_RECORDS and owner['invocation'] is invocation,'bounded isolated original probe roster')
        records.append((record,owner))
    count=0; kinds=collections.Counter(); processes=set()
    def verify_group(public):
        nonlocal count
        for record,owner in records:
            m.tick()
            original=record.document(); kind=original['operation_kind']
            if (kind in ('generator','public-runtime','public-native')) is not public:continue
            if public:
                need(state is not None and owner.get('runtime_permit') is state['runtime_permit'],
                     'original public record permit association')
            observed=prep.verify_probe(record,invocation,argv=original['argv'],cwd=original['cwd'],
                environment=original['environment'],timeout_seconds=original['timeout_seconds'],
                stream_limit_bytes=original['stream_limit_bytes'],operation_kind=kind,
                runtime_permit=owner.get('runtime_permit'),native_permit=owner.get('native_permit'),
                runtime_case=original.get('runtime_case'))
            m.tick()
            need(observed==original,'original issued operation document drift')
            for name in ('stdout','stderr'):
                m.tick(); raw=getattr(record,name)(); m.tick()
                need(len(raw)==original[name]['bytes'] and hashlib.sha256(raw).hexdigest()==original[name]['sha256'],
                     'original setup/runtime stream preservation')
            process=(original['pid'],original['started_ns'])
            need(process not in processes,'unique original operation process identity')
            processes.add(process); kinds[kind]+=1; count+=1
    with m.context('original-operation-checked-scope',prep.checked_scope(invocation)):
        verify_group(False)
        if any(record.document()['operation_kind'] in ('generator','public-runtime','public-native') for record,_ in records):
            with m.context('preserve-public-record-boundary',runtime._read_boundary(state)):
                with m.context('preserve-public-record-replay',prep.public_replay(invocation,state['runtime_permit'])):
                    verify_group(True)
    need(count==prep._RUNS[invocation]['next'],'every original operation issued and readmitted')
    return {'original_records_readmitted':count,'operation_kinds':dict(kinds),
            'public_runtime_records':kinds['public-runtime'],'public_native_records':kinds['public-native'],
            'streams_verified_per_record':2, 'unique_original_processes':len(processes)}


def operation_evidence(prep, invocation, checkpoint=lambda:None, emit=lambda kind,row:None):
    """Separate bounded per-record carriers; original artifacts remain the evidence."""
    checkpoint(); owner=prep._RUNS.get(invocation)
    if owner is None:return {'status':'NOT_CREATED','original_next':0,'operation_carriers':0}
    count=owner['next']
    need(type(count) is int and 0<=count<=EXPECTED_RECORDS,'bounded original operation count')
    directory=owner['root']/'operations'; names=[]
    for path in directory.iterdir():
        checkpoint()
        need(len(names)<EXPECTED_RECORDS and path.is_dir() and not path.is_symlink(),'bounded ordinary operation directories')
        names.append(path.name)
    expected={f'{i:04d}' for i in range(1,count+1)}
    issued_names=[]; kinds=collections.Counter(); cleaned=True
    for record,issued in prep._PROBES.items():
        checkpoint()
        need(len(issued_names)<EXPECTED_RECORDS and issued['invocation'] is invocation,'bounded isolated issued probes')
        document=record.document(); name=issued['directory'].name
        row=dict(directory=str(issued['directory']),name=name,result_pin=issued['result'],command_pin=issued['command'],
            stdout_pin=document['stdout'],stderr_pin=document['stderr'],
            **{key:document.get(key) for key in ('pid','started_ns','completed_ns','status','operation_kind','runtime_case',
                'cleanup_complete','cleanup_error','failure','timed_out','stream_limit_exceeded','observed_peaks')})
        for pin in (row['result_pin'],row['command_pin'],row['stdout_pin'],row['stderr_pin']):
            need(type(pin) is dict and set(pin)=={'path','bytes','sha256'} and
                 type(pin['path']) is str and len(pin['path'].encode())<=1024 and
                 type(pin['bytes']) is int and 0<=pin['bytes']<2**63 and
                 type(pin['sha256']) is str and re.fullmatch('[0-9a-f]{64}',pin['sha256']),
                 'closed bounded original artifact pin')
        need(row['directory']==str(directory/name) and re.fullmatch('[0-9]{4}',name), 'original operation directory route')
        emit('operation',detached(row,OPERATION_BYTES))
        issued_names.append(name);kinds[document['operation_kind']]+=1
        cleaned = cleaned and all(document[key] is value for key,value in
            {'cleanup_complete':True,'cleanup_error':None,'failure':None,'timed_out':False,'stream_limit_exceeded':False}.items())
    verified=(len(names)==len(set(names)) and set(names)==expected and len(issued_names)==len(set(issued_names)) and
        set(issued_names)==expected and cleaned)
    checkpoint()
    return detached(dict(status='VERIFIED' if verified else 'UNVERIFIED',original_next=count,
        operation_carriers=len(issued_names),operation_kinds=dict(kinds),carrier_file='operation.jsonl',
        actual_directory_names=sorted(names),unissued_directories=sorted(set(names)-set(issued_names)),
        missing_directories=sorted(expected-set(names)),failed_terminal=owner.get('failed_terminal'),
        outer_descendant_cleanup_claim=False))


def run(session, request, sink, *, started_monotonic, cancellation, installed):
    """One held orchestration; only the separately released driver may call it."""
    m=Measurements(sink,started_monotonic,cancellation)
    report=dict(FLAGS,schema='oxid-partial-downstream-diagnostic-v1',status='RUNNING',
        failure=None,preservation={},public_operations_completed=0,build_results_retained=0,
        limitation='Fresh lifecycle/guards and original full parser only; no full public collection, hosted join, session or E14 qualification')
    cancellation.report=report
    primary=None
    handles={}
    before=own_before=api=runtime=state=None
    try:
        with m.phase('setup'):
            need(type(installed) is dict and set(installed)=={'repository','head','tree'} and
                 all(type(installed[k]) is str and re.fullmatch('[0-9a-f]{40}',installed[k]) for k in ('head','tree')) and
                 request['scope']=='combined' and request['expected_head']==request['event_sha']==installed['head'] and
                 str(session.REPOSITORY)==installed['repository'], 'exact reviewed fresh installed combined request')
            own_before=m.call('session-package',session.own_package)
            plan=m.call('session-request-plan',lambda:session.plan_request(request))
            before=m.call('session-dependencies',session.dependencies)
            api=m.call('canonical-session-apis',session.apis);m.api=api
            m.call('verify-committed-inputs',lambda:session.verify_committed(api,request,before))
            root=Path(plan['root'])
            handles['captured']=m.call('capture-current-checkout',lambda:api['context'].capture_source(
                session.REPOSITORY,request['expected_head'],request['event_sha'],scope='combined'))
            need(handles['captured'].document()['checkout']['tree']==installed['tree'],'exact reviewed installed tree')
            invocation=api['context'].begin_invocation(handles['captured'],root)
            handles['invocation']=m.invocation=invocation
            cancellation.owner=(api['preparation'],invocation)
            context=m.call('prepare-source',lambda:api['preparation'].produce(invocation,'combined',
                original_repo=request['original_repo'],historical_repo=request['historical_repo']))
            handles['context']=context
            machine=handles['machine']=m.call('capture-machine',lambda:api['machine'].capture_machine(invocation))
            build=api['binding'].build_api()
            public={}
            handles['public_builds']=public
            for role in ('ordinary','lifecycle'):
                public[role]=m.call('build-public-'+role,lambda role=role:build.build(context,role,
                    root/'targets'/('public-'+role),root/'public-build'/role,machine))
            handles['public_builds']=public
            parser=api['parser']
            authority=handles['parser_authority']=m.call('admit-parser-source',lambda:parser.authority(context))
            parser_session=handles['parser_session']=m.call('prepare-parser-layout',lambda:parser.prepare(authority))
            parser_builds=[]
            handles['parser_builds']=parser_builds
            for control in (False,True):
                for profile in ('debug','release'):
                    parser_builds.append(m.call('build-parser-'+('control-' if control else 'observed-')+profile,
                        lambda profile=profile,control=control:parser.build(parser_session['path'],profile,authority,machine,control=control)))
            p=request['public'];runtime=api['binding'].runtime_api();m.runtime=runtime
            tools=handles['tools']=m.call('capture-public-runtime-tools',lambda:api['machine'].capture_runtime_tools(machine,
                llvm_root=p['llvm_root'],library_root=p['library_root'],runtime_receipt=p['runtime_receipt']))
            admission=handles['admission']=m.call('admit-public-runtime',lambda:runtime.admit_runtime(public['ordinary'],public['lifecycle'],
                contract_root=p['contract_root'],path_map=p['path_map'],amendment_root=p['amendment_root'],runtime_tools=tools))
            state=m.state=runtime._state(admission)
            need(state['invocation'] is invocation,'original admitted invocation')
            views=api['preparation']._VIEWS.get(invocation,{})
            need(len(views)==8 and len({r['target'] for r in views.values()})==2,'eight original links and two genuine target paths')
            report['build_results_retained']=len(public)*2+len(parser_builds)
            retain_records(api['preparation'],invocation,handles)
        with m.phase('selected-public'):
            selection=handles['selection']=m.call('issue-downstream-diagnostic',lambda:runtime.issue_downstream_diagnostic(admission))
            result=handles['selected_result']=m.call('collect-downstream-diagnostic',lambda:runtime.collect_downstream_diagnostic(selection))
            m.call('verify-selected-before-parser',lambda:runtime.verify_downstream_diagnostic(result))
            report['selected']=selected_summary(runtime,result,state)
            retain_records(api['preparation'],invocation,handles)
        with m.phase('parser'):
            inputs=handles['parser_inputs']=m.call('capture-parser-runtime-inputs',lambda:api['machine'].capture_parser_runtime(machine,invocation))
            parser_handle=handles['parser_runtime']=m.call('admit-parser-runtime',lambda:parser.create_runtime(
                authority,parser_builds,request['parser']['contract_root'],inputs))
            handles['parser_comparison']=m.call('complete-parser-runtime',lambda:parser.complete_runtime(parser_handle))
            report['parser']=parser_summary(m.call('verify-parser-runtime',lambda:parser.verify_runtime(parser_handle)))
            m.call('verify-selected-after-parser',lambda:runtime.verify_downstream_diagnostic(result))
            report['selected']=selected_summary(runtime,result,state)
            report['parser']=parser_summary(m.call('verify-parser-after-public',lambda:parser.verify_runtime(parser_handle)))
            retain_records(api['preparation'],invocation,handles)

    except BaseException as error:
        primary=error;report['failure']=error_row(error)
    finally:
        cancellation.finalizing=True
        try:
            with m.phase('preservation'):
                checks=[]
                if api is not None and before is not None:
                    checks += [('committed-inputs',lambda:session.verify_committed(api,request,before)),
                        ('consumer-code',lambda:need(session.dependencies()==before and session.own_package()==own_before,'original session/dependency closure'))]
                if 'context' in handles:checks.append(('source',lambda:api['context'].verify_context(handles['context'])))
                if 'machine' in handles:checks.append(('machine',lambda:api['machine'].verify_machine(handles['machine'],m.invocation)))
                for index,result in enumerate(handles.get('parser_builds',())):
                    checks.append(('parser-build-'+str(index),lambda result=result:api['parser'].verify_build(result)))
                if state is not None:
                    checks += [('runtime-tools',lambda:api['machine'].verify_runtime_tools(handles['tools'],handles['machine'],m.invocation)),
                        ('full-readmission',lambda:runtime._checkpoint(state,full=True))]
                if 'selected_result' in handles:
                    checks.append(('selected-result',lambda:runtime.verify_downstream_diagnostic(handles['selected_result'])))
                if 'parser_runtime' in handles:
                    checks.append(('parser-runtime',lambda:parser_summary(api['parser'].verify_runtime(handles['parser_runtime']))))
                if api is not None and m.invocation is not None:
                    checks.append(('original-operation-records',lambda:reverify_operations(m,runtime,state)))
                for name,function in checks:
                    try:
                        result=m.call('preserve-'+name,function)
                        report['preservation'][name]='VERIFIED'
                        if name=='original-operation-records':report['original_record_preservation']=result
                    except BaseException as error:
                        report['preservation'][name]={'status':'UNVERIFIED','error':error_row(error)}
                        if primary is None:primary=error;report['failure']=error_row(error)
                        else:note(primary,'final '+name+' preservation failed',error)
                    m.emit('preservation',dict(name=name,monotonic=time.monotonic(),result=report['preservation'][name]),primary)
                if primary is None:
                    observed=report.get('original_record_preservation',{})
                    need(observed.get('original_records_readmitted')==EXPECTED_RECORDS and
                         observed.get('unique_original_processes')==EXPECTED_RECORDS and
                         observed.get('operation_kinds')==EXPECTED_KINDS,'exact reviewed 869 original operations')
                    report['public_operations_completed']=680
                    report['native_operations_completed']=68
                report['final_telemetry']=m.telemetry()
        except BaseException as error:
            if primary is None:primary=error;report['failure']=error_row(error)
            else:note(primary,'preservation phase failed',error)
        # Independent of phase entry/sink/check success: unwind already happened.
        if primary is not None and state is not None:
            try:
                need(time.monotonic()<=min(m.deadline,started_monotonic+21540),'failure audit preservation budget')
                report['failure_audit']=api['preparation'].public_output_failure(m.invocation,state['runtime_permit'])
            except BaseException as error:
                report['failure_audit']={'verified':False,'error':error_row(error)}
                note(primary,'failure audit unavailable',error)
        if api is not None and m.invocation is not None:
            try:
                need(time.monotonic()<=min(m.deadline,started_monotonic+21540),'operation evidence preservation budget')
                retain_records(api['preparation'],m.invocation,handles)
                report['operation_cleanup']=operation_evidence(api['preparation'],m.invocation,m.check,m.emit)
                need(report['operation_cleanup']['status']=='VERIFIED','original operation evidence/cleanup incomplete')
            except BaseException as error:
                if primary is None:primary=error;report['failure']=error_row(error)
                else:note(primary,'operation cleanup evidence unverified',error)
        def poison_late():
            if primary is not None and api is not None and m.invocation is not None:
                try:api['preparation'].cancel_invocation(m.invocation)
                except BaseException as error:note(primary,'original invocation poison failed',error)
        poison_late()
        try:
            report.update(status='PARTIAL_DOWNSTREAM_DIAGNOSTIC_COMPLETE' if primary is None else 'FAILED_OR_UNVERIFIED',
                monotonic=time.monotonic(),elapsed_seconds=time.monotonic()-started_monotonic,
                strong_handle_names=sorted(handles),failure=None if primary is None else error_row(primary))
            m.emit('terminal',report,primary)
        except BaseException as error:
            if primary is None:primary=error;report['failure']=error_row(error)
            else:note(primary,'terminal retention failed',error)
            poison_late()
        finally:cancellation.sealed=True
    if primary is not None:raise primary
    return detached(report)
