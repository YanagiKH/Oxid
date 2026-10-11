"""Pure source, provenance and synthetic closure controls. Never run a producer."""
import ast
import builtins
import contextlib
import copy
import dis
import hashlib
import importlib.util
import os
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('parser_runtime_controls_api',HERE/'portable.py')
api=importlib.util.module_from_spec(spec);spec.loader.exec_module(api)

class RuntimeControls(unittest.TestCase):
    def setUp(self):self.runtime=api._runtime_api()
    def retained(self):
        root=Path(os.environ.get('OXID_PARSER_TEST_RETAINED_ROOT',api.REPOSITORY))
        path=root/api.OLD_PATH
        if not path.is_file():self.skipTest('fixed-hash retained source fixture not installed')
        raw=path.read_bytes();self.assertEqual(api.sha(raw),api._manifest()['retained_portable']['sha256'])
        return path,raw
    def test_opaque_handles_only(self):
        for value in (None,{},object(),{'status':'pass'}):
            with self.assertRaises(self.runtime.Rejected):self.runtime._live(value)
        with self.assertRaises(TypeError):self.runtime.RuntimeSession()
        with self.assertRaises(api.Rejected):api.create_runtime(None,[],None,None)
    def test_consumer_binding_is_once(self):
        with self.assertRaises(self.runtime.Rejected):self.runtime.bind_consumer(object())
    def test_sealed_canonical_dependency_owners(self):
        if api.SHARED_MANIFEST_SHA256 is None or api.MACHINE_MANIFEST_SHA256 is None:
            self.skipTest('shared/machine source seal pending')
        preparation=api._preparation_api();machine=api._machine_api();context=api._context_api()
        self.assertIs(preparation,api._preparation_api())
        self.assertIs(machine,api._machine_api())
        self.assertIs(machine._shared(),preparation)
        self.assertIs(preparation.context_api(),context)
        self.assertIs(self.runtime,api._runtime_api())
        for name in ('capture_parser_runtime','verify_parser_runtime','verify_parser_runtime_selected'):
            self.assertTrue(callable(getattr(machine,name)))
        for name in ('register_parser_runtime','owned_probe','verify_probe'):
            self.assertTrue(callable(getattr(preparation,name)))

    def test_runtime_pin(self):
        row=api._manifest()['runtime_helper'];raw=(HERE/row['path']).read_bytes()
        self.assertEqual(row,dict(path='runtime.py',bytes=len(raw),sha256=api.sha(raw)))
    def test_exact_source_derivations(self):
        _,raw=self.retained();sources,proof=self.runtime.derive_functions(raw)
        self.assertEqual(proof,api._manifest()['runtime_derivations'])
        self.assertEqual(sources['project_enum_observations'],self.runtime._function(raw,'project_enum_observations'))
        for name,source in sources.items():compile(source,name,'exec')
        with self.assertRaises(self.runtime.Rejected):self.runtime.derive_functions(raw+b'\n')
    def test_derived_global_closure(self):
        path,raw=self.retained();old=api._module('pure_retained_source',path,raw)
        state={'old':old,'facade':types.SimpleNamespace(verify_build=lambda *a:None)}
        with mock.patch.object(self.runtime._API,'_record_at',return_value=raw):module=self.runtime._derived(state)
        def codes(code):
            yield code
            for value in code.co_consts:
                if isinstance(value,types.CodeType):yield from codes(value)
        for name in api._manifest()['runtime_derivations']:
            for code in codes(getattr(module,name).__code__):
                for op in dis.get_instructions(code):
                    if op.opname in ('LOAD_GLOBAL','LOAD_NAME'):
                        self.assertTrue(op.argval in module.__dict__ or hasattr(builtins,op.argval),(name,op.argval))
    @contextlib.contextmanager
    def fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            roster={'passivity-debug/a','lexer-phase/passivity-debug.json','collect-debug/later'}
            state=dict(root=root,failed=False,active='passivity-debug',invocation=object(),sealed={},sealed_dirs=set(),runtime_roster=roster,stages={},processes={str(root/'passivity-debug'):object()})
            with mock.patch.object(self.runtime,'_CURRENT',[state]),mock.patch.object(self.runtime._API,'_tick',return_value=None):
                token=self.runtime._ACTIVE.set(state)
                try:yield root,state
                finally:self.runtime._ACTIVE.reset(token)
    def test_phase_closure_and_seal(self):
        with self.fixture() as (root,state):
            for name in ('passivity-debug/a','lexer-phase/passivity-debug.json'):
                p=root/name;p.parent.mkdir(exist_ok=True);p.write_bytes(b'fixture')
            self.runtime._seal(state)
            token=self.runtime._ACTIVE.set(None)
            try:self.assertEqual(len(self.runtime.closure(root)[0]),2)
            finally:self.runtime._ACTIVE.reset(token)
            (root/'passivity-debug/a').write_bytes(b'drift')
            with self.assertRaises(self.runtime.Rejected):self.runtime.closure(root)
    def test_premature_and_extra_empty_directory_rejected(self):
        with self.fixture() as (root,state):
            (root/'collect-debug').mkdir()
            with self.assertRaises(self.runtime.Rejected):self.runtime.closure(root)
            (root/'collect-debug').rmdir();(root/'passivity-debug').mkdir();(root/'passivity-debug/empty').mkdir()
            with self.assertRaises(self.runtime.Rejected):self.runtime.closure(root)
    def test_incomplete_phase_cannot_seal(self):
        with self.fixture() as (root,state):
            with self.assertRaises(self.runtime.Rejected):self.runtime._seal(state)
    def test_symlink_cannot_enter_runtime(self):
        with self.fixture() as (root,state):
            (root/'passivity-debug').symlink_to(root/'missing')
            with self.assertRaises(self.runtime.Rejected):self.runtime.closure(root)
    def test_phase_sequence_is_explicit(self):
        expected=['passivity-debug','collect-debug','passivity-release','collect-release','u8','phase','compare']
        self.assertEqual(list(self.runtime.SEQUENCE),expected)
        state={'report':{'stages':[]},'stages':{}}
        self.runtime._order(state,expected[0])
        with self.assertRaises(self.runtime.Rejected):self.runtime._order(state,'collect-debug')
    def test_no_unsupervised_process_or_old_module_mutation(self):
        tree=ast.parse((HERE/'runtime.py').read_bytes())
        for node in ast.walk(tree):
            if isinstance(node,ast.Call) and isinstance(node.func,ast.Attribute):
                self.assertNotIn(node.func.attr,('Popen','check_output','bounded_capture'))
                if isinstance(node.func.value,ast.Name) and node.func.value.id=='old':
                    self.assertNotIn(node.func.attr,('invoke','verify_build','session_at','authority','prepare','build','compare'))
            if isinstance(node,ast.Assign):
                for target in node.targets:
                    self.assertFalse(isinstance(target,ast.Attribute) and isinstance(target.value,ast.Name) and target.value.id=='old')
    def test_original_phase_catalogue_and_full_roster(self):
        path,raw=self.retained();old=api._module('pure_retained_catalogue',path,raw)
        retained=api.load((path.parent/'authority.json').read_bytes())
        self.assertEqual(api.sha((path.parent/'authority.json').read_bytes()),api._manifest()['retained_authority']['sha256'])
        recipe=api.load((old.FROZEN/'authority.json').read_bytes());recipe['current']=retained
        driver=old.phase_module(recipe,controls=True)
        self.assertEqual(len(driver.roster()),54)
        self.assertEqual(sum(driver.expected(*entry)['exit_code']==101 for entry in driver.roster()),30)
        # Synthetic case names exercise closure cardinality only; no corpus admission.
        cases=[{'id':'pure-case-%03d'%i} for i in range(248)]
        state=dict(old=old,recipe=recipe,contract={'cases':cases},root=Path('/pure-root'))
        names,dirs=self.runtime._full_roster(state)
        self.assertEqual(sum(n.startswith('lexer-phase-controls/') for n in names),217)
        self.assertEqual(sum(n.startswith('lexer-phase/') for n in names),6)
        self.assertEqual(len(self.runtime._process_roster(state,'u8')),18)
        self.assertEqual(len(self.runtime._process_roster(state,'phase')),54)
        self.assertEqual(sum(n.endswith('/request.json') and n.startswith('collect-') for n in names),496)

    def test_shared_helper_order_and_command_grammar(self):
        # Pure policy test with no producer registry, capture or owned operation.
        path=HERE.parent/'package_dependency_unit4_current/preparation.py'
        policy=api._module('pure_shared_parser_policy',path,path.read_bytes())
        builds={(p,c):(p,c) for p in ('debug','release') for c in (False,True)}
        with mock.patch.object(self.runtime._API,'_result',side_effect=lambda value:{'helper':value}):
            ordered=self.runtime._helper_probes(builds)
        self.assertEqual(ordered,[('debug',False),('release',False),('debug',True),('release',True)])
        # Match the actual shared registration loop nesting, not a second
        # independently invented caller ordering.
        tree=ast.parse(path.read_bytes());fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='register_parser_runtime')
        outer=next(n for n in fn.body if isinstance(n,ast.For));inner=next(n for n in outer.body if isinstance(n,ast.For))
        self.assertEqual((outer.target.id,inner.target.id),('control','profile'))
        self.assertEqual(ast.literal_eval(outer.iter),(False,True));self.assertEqual(ast.literal_eval(inner.iter),('debug','release'))
        root=Path('/pure-policy/parser');state=dict(root=root,session_path=root/'package-source-session.json',contract_root=Path('/pure-contract'))
        permit=dict(root=str(root),checkpoint=str(state['session_path']),contract_root=str(state['contract_root']))
        env=dict(HOME=str(root/'home'),PATH='/usr/bin:/bin',LANG='C.UTF-8',LC_ALL='C.UTF-8',PYTHONDONTWRITEBYTECODE='1',PYTHONNOUSERSITE='1')
        with mock.patch.object(policy,'_runtime_permit',return_value=permit),mock.patch.object(policy,'identity',side_effect=lambda p:{'path':str(p)}):
            for kind,operation,timeout in (('collection','parser-collect',1800),('passivity','parser-passivity',900)):
                for profile in ('debug','release'):
                    out,argv=self.runtime._helper_args(state,kind,profile)
                    self.assertEqual(policy._parser_runtime_recipe(object(),object(),operation,argv,root,env,timeout,8*1024**2),{'path':argv[2]})
                    with self.assertRaises(policy.Rejected):policy._parser_runtime_recipe(object(),object(),operation,argv+['--subset'],root,env,timeout,8*1024**2)

    def test_all_phase_commands_match_shared_policy(self):
        path,raw=self.retained();old=api._module('pure_original_phase_recipe',path,raw)
        retained=api.load((path.parent/'authority.json').read_bytes())
        recipe={'current':retained};driver=old.phase_module(recipe,controls=True)
        path=HERE.parent/'package_dependency_unit4_current/preparation.py'
        policy=api._module('pure_phase_supervisor_policy',path,path.read_bytes())
        root=Path('/pure-policy/parser');checkpoint=root/'package-source-session.json'
        binaries={}
        for index,(p,c) in enumerate((p,c) for c in (False,True) for p in ('debug','release')):
            binaries[('control-' if c else 'observed-')+p]=dict(path=str(root/(('build-control-' if c else 'build-')+p)/'result/target/oxid'),bytes=1,sha256=str(index+1)*64)
        permit=dict(root=str(root),checkpoint=str(checkpoint),binaries=binaries,authority_sha256='b'*64)
        with mock.patch.object(policy,'_runtime_permit',return_value=permit),mock.patch.object(policy,'verify_invocation',return_value=({}, {'checkout':{'path':'/pure-repository'}})),mock.patch.object(policy,'_module',return_value=driver),mock.patch.object(policy,'identity',side_effect=lambda path:dict(path=str(path),sha256='a'*64)):
            for probe,profile,role in driver.roster():
                binary=binaries[('control-' if role=='not_observed' else 'observed-')+profile]
                request=dict(schema=driver.REQUEST_SCHEMA,nonce='e'*32,probe=probe,profile=profile,role=role,
                    session_sha256='a'*64,authority_sha256='b'*64,driver_sha256=retained['lexer_phase']['controls']['driver']['sha256'],
                    binary=dict(binary,path=str(Path(binary['path']).relative_to(root))),entrypoint=driver.ENTRYPOINT,expected=driver.expected(probe,profile,role))
                argv,cwd,env=driver.inputs(root,request)
                with mock.patch.object(policy,'body',return_value=driver.canonical_bytes(request)):
                    policy._parser_runtime_recipe(object(),object(),'parser-phase',argv,Path(cwd),env,30,4096)
                    with self.assertRaises(policy.Rejected):policy._parser_runtime_recipe(object(),object(),'parser-phase',argv,Path(cwd),env,31,4096)
                for field in ('authority_sha256','driver_sha256'):
                    with mock.patch.object(policy,'body',return_value=driver.canonical_bytes(dict(request,**{field:'f'*64}))):
                        with self.assertRaises(policy.Rejected):policy._parser_runtime_recipe(object(),object(),'parser-phase',argv,Path(cwd),env,30,4096)

    def test_phase_projection_flag_and_stage_limits(self):
        import inspect
        self.assertIn('project',inspect.signature(self.runtime._invoke).parameters)
        text=(HERE/'runtime.py').read_text()
        for token in ('248','319','638','524','four actual builds','parser-phase','parser-u8'):
            self.assertIn(token,text if token!='524' else self.retained()[1].decode())
    def test_failed_owned_process_retained_before_rejection(self):
        # Synthetic supervisor fixture only; no issued Invocation or operation.
        r=self.runtime;doc=dict(status=101,cleanup_complete=True,cleanup_error=None,failure=None,
            timed_out=False,stream_limit_exceeded=False,invocation_nonce='a'*32,started_ns=20,completed_ns=30)
        probe=types.SimpleNamespace(document=lambda:copy.deepcopy(doc))
        supervisor=types.SimpleNamespace(owned_probe=lambda *a,**k:probe,verify_probe=lambda *a,**k:copy.deepcopy(doc),checked_scope=lambda *a:contextlib.nullcontext())
        machine=types.SimpleNamespace(verify_parser_runtime=lambda *a:True)
        root=Path('/pure-runtime');state=dict(root=root,session={'host':{'fixture':True}},
            old=types.SimpleNamespace(measured_host=lambda:{'fixture':True}),runtime_inputs=object(),machine=object(),
            invocation=types.SimpleNamespace(document=lambda:{'nonce':'a'*32}),permit=object(),processes={},built_after_ns=10)
        token=r._ACTIVE.set(state)
        try:
            with mock.patch.object(r._API,'_preparation_api',return_value=supervisor),mock.patch.object(r._API,'_machine_api',return_value=machine),mock.patch.object(r,'_selected_operation',side_effect=[None,RuntimeError('post-readback drift')]):
                with self.assertRaisesRegex(r.Rejected,'exact admitted runtime exit') as caught:r._invoke(state,'parser-u8',['pure'],root,{},root/'u8',120,4096,(0,))
            self.assertIs(state['processes'][str(root/'u8')]['probe'],probe)
            self.assertIn('post-readback drift',caught.exception.__notes__[0])
            self.assertIn('post-readback drift',state['operation_preservation_errors'][0])
        finally:r._ACTIVE.reset(token)

    def test_command_derived_selected_build_roles(self):
        r=self.runtime;root=Path('/pure-selection/parser')
        binaries={(p,c):str(root/(("control-" if c else "observed-")+p)) for p in r.PROFILES for c in r.ROLES}
        state=dict(root=root,session_path=root/'package-source-session.json',contract_root=Path('/pure-contract'),
            builds={key:types.SimpleNamespace(document=lambda path=path:{'binary':{'path':path}}) for key,path in binaries.items()})
        for profile in r.PROFILES:
            for kind,semantic,wanted in (('parser-collect','collection',[(profile,False)]),('parser-passivity','passivity',[(profile,False),(profile,True)])):
                _,argv=r._helper_args(state,semantic,profile)
                self.assertEqual(r._operation_selection(state,kind,argv),wanted)
                with self.assertRaises(r.Rejected):r._operation_selection(state,kind,argv+['--subset'])
        public=['--exact','frontend::parser::u8_syntax_tests::u8_named_conversion_has_a_real_child_and_exact_trivia_origins_on_every_mode','--nocapture']
        self.assertEqual(r._operation_selection(state,'parser-u8',[binaries['debug',True]]+public),[('debug',True)])
        with self.assertRaises(r.Rejected):r._operation_selection(state,'parser-u8',[binaries['debug',False]]+public)
        with self.assertRaises(r.Rejected):r._operation_selection(state,'parser-phase',['/foreign','bad'])

    def test_failure_receipt_is_terminal(self):
        r=self.runtime
        @contextlib.contextmanager
        def exhausted(*args):raise RuntimeError('fixture exhausted');yield
        with tempfile.TemporaryDirectory() as directory,mock.patch.object(r._API,'_bounded_phase',exhausted):
            state=dict(root=Path(directory),invocation=types.SimpleNamespace(document=lambda:{'nonce':'a'*32}),processes={})
            r._failure_evidence(state,'u8',ValueError('original'))
            result=api.load((state['root']/'package-source-runtime-failure.json').read_bytes())
            self.assertFalse(result['execution_qualified']);self.assertEqual(len(result['preservation']),5)
            self.assertTrue(all(p['status']=='unverified' for p in result['preservation']))
            self.assertIn('original',result['original_error'])

if __name__=='__main__':unittest.main()
