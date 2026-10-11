"""Source/pure controls only; never preparation, compiler, runtime, or CI."""
import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
import tempfile
import types
from unittest import mock

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('package_parser_consumer_controls',HERE/'portable.py')
api=importlib.util.module_from_spec(spec)
spec.loader.exec_module(api)

class ConsumerControls(unittest.TestCase):
    def test_closed_roster(self):
        self.assertEqual(sorted(p.name for p in HERE.iterdir()),['authority.json','portable.py','runtime.py','test_consumer.py','test_runtime.py'])

    def test_shared_loader_fail_closed_or_exact(self):
        if api.SHARED_MANIFEST_SHA256 is None:
            with self.assertRaises(api.Rejected):api._context_api()
        else:
            module=api._context_api()
            for name in ('verify_context','load_context','capture_source','begin_invocation'):
                self.assertTrue(callable(getattr(module,name)))

    def test_exact_authority(self):
        raw=(HERE/'authority.json').read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(),api.AUTHORITY_SHA)
        self.assertEqual(api._manifest()['schema'],'oxid-package-source-parser-consumer-authority-v1')

    def test_distinct_source_schema(self):
        self.assertEqual(api.CHECKPOINT,'package-source-session.json')
        self.assertNotEqual(api.SESSION_SCHEMA,'oxid-unit4-current-parser-session-v1')
        a=api._manifest()
        self.assertFalse(a['execution_qualified'])
        self.assertEqual(a['status'],'NotReady')

    def test_no_caller_recipe_authority(self):
        for bad in (None,{}, {'status':'ready'}, object()):
            with self.assertRaises(api.Rejected):api._admit(bad)

    def test_constructor_requires_issuer(self):
        with self.assertRaises(api.Rejected):api.AdmittedAuthority(None,{}, {},None,object())

    def test_execution_held(self):
        for name in ('verify_passivity','admit_execution',
                     'comparator','phase_control_binding','phase_scope','phase_evidence',
                     'project_enum_observations','u8_policy_module','phase_module'):
            with self.subTest(name=name),self.assertRaises(api.NotReady):
                getattr(api,name)(None,None,None,None)

    def test_old_comparator_rejected(self):
        with self.assertRaises(api.Rejected):api.derive_comparator(b'old results')

    def test_derivation_pin_complete(self):
        p=api._manifest()['comparator_derivation']
        self.assertEqual(p['semantic_predicate_handlers_unchanged'],22)
        self.assertEqual(p['derived_sha256'],'a302d6bd079f87d2be3b4b497adb0240bd80db3242a3bfb5859b1193ae833cf4')
        replacement=b'    manifest, host, build, profile = package_source_admission(path, contract, approval)\n'
        self.assertEqual(hashlib.sha256(replacement).hexdigest(),p['replacement_sha256'])

    def test_closed_projection(self):
        self.assertEqual(set(api._manifest()['projection_fields']),{'current_base_files','current_derived_files',
            'current_control_derived_files','current_candidate_source_manifest_sha256',
            'current_source_manifest','reviewed_source_head','source_only_tree'})

    def test_duplicate_json_rejected(self):
        with self.assertRaises(api.Rejected):api.load(b'{"x":1,"x":2}')
        with self.assertRaises(api.Rejected):api.load(b'{"x":NaN}')

    def test_serialization_defensive(self):
        # This exercises the value carrier only, never authenticates a session.
        value={'list':[1]}
        obj=api.AdmittedAuthority(None,value,value,None,api._AUTHORITY_TOKEN)
        value['list'].append(2); copy=obj.document();copy['list'].append(3)
        self.assertEqual(obj.document(),{'list':[1]})
        with self.assertRaises(Exception):api._admit(obj)

    def test_no_old_global_mutation(self):
        tree=ast.parse((HERE/'portable.py').read_text())
        for node in ast.walk(tree):
            if isinstance(node,(ast.Assign,ast.AnnAssign,ast.AugAssign)):
                targets=node.targets if isinstance(node,ast.Assign) else [node.target]
                for target in targets:
                    if isinstance(target,ast.Attribute):
                        self.assertFalse(isinstance(target.value,ast.Name) and target.value.id=='old')
            if isinstance(node,ast.Call) and isinstance(node.func,ast.Attribute):
                if isinstance(node.func.value,ast.Name) and node.func.value.id=='old':
                    self.assertNotIn(node.func.attr,('authority','session_at','prepare','build','collect','compare'))

    def test_no_subprocess_execution_in_build_api(self):
        tree=ast.parse((HERE/'portable.py').read_text())
        for name in ('build','verify_build'):
            fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name==name)
            self.assertFalse(any(isinstance(n,ast.Attribute) and n.attr in ('invoke','check_output','run','Popen') for n in ast.walk(fn)))

    def test_directory_closure(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); (root/'home').mkdir(); (root/'source').mkdir()
            (root/'source/body').write_bytes(b'pure closure fixture')
            api._close_tree(root,['source/body'],('home',))
            (root/'extra-empty').mkdir()
            with self.assertRaises(api.Rejected):api._close_tree(root,['source/body'],('home',))
            (root/'extra-empty').rmdir(); (root/'home/extra').write_bytes(b'x')
            with self.assertRaises(api.Rejected):api._close_tree(root,['source/body'],('home',))

    def test_symlink_closure(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); (root/'dangling').symlink_to(root/'missing')
            with self.assertRaises(api.Rejected):api._close_tree(root,[])

    def test_live_build_result_required(self):
        for value in (None,{}, {'schema':'oxid-package-source-parser-build-v1'},object()):
            with self.assertRaises(api.Rejected):api.verify_build(value)
        with self.assertRaises(TypeError):api.BuildResult()
        with self.assertRaises(api.Rejected):api.build(None,'debug',None,None)

    def test_build_uses_real_helper_and_pure_verifier(self):
        tree=ast.parse((HERE/'portable.py').read_text())
        calls=[n for n in ast.walk(tree) if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute)]
        self.assertTrue(any(n.func.attr=='owned_probe' and any(k.arg=='operation_kind' and isinstance(k.value,ast.Constant) and k.value.value=='parser-helper' for k in n.keywords) for n in calls))
        self.assertTrue(any(isinstance(n.func.value,ast.Name) and n.func.value.id=='old' and n.func.attr=='verify_cargo' for n in calls))
        self.assertFalse(any(isinstance(n.func.value,ast.Name) and n.func.value.id=='old' and n.func.attr in ('verify_build','verify_sysroot','process_tools','invoke') for n in calls))

    def test_exact_link_closure(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);target=root/'body';target.write_bytes(b'pure test')
            (root/'link').symlink_to(target)
            api._close_tree(root,['body','link'],links={'link':str(target)})
            with self.assertRaises(api.Rejected):api._close_tree(root,['body','link'],links={'link':str(root/'wrong')})

    def test_expired_phase_rejected_without_child(self):
        supervisor=types.SimpleNamespace(check_deadline=lambda invocation:True)
        with mock.patch.object(api,'_preparation_api',return_value=supervisor):
            with self.assertRaises(api.Rejected):
                with api._bounded_phase(object(),-1):pass
            with api._bounded_phase(object(),1):api._tick(object())

    def test_terminal_failure_reports_unverified_budget(self):
        # Pure supervisor refusal fixture. No capture, source preparation,
        # machine admission, child, or compiler is invoked by this control.
        def stopped(_):raise api.Rejected('fixture budget exhausted')
        supervisor=types.SimpleNamespace(check_deadline=stopped)
        invocation=types.SimpleNamespace(document=lambda:{'nonce':'a'*32})
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(api,'_preparation_api',return_value=supervisor):
            root=Path(directory)
            api._failure_receipt(RuntimeError('original fixture failure'),root,None,None,invocation,None,None,[],None,None)
            receipt=api.load((root/'package-source-build-failure.json').read_bytes())
            self.assertEqual(receipt['status'],'NotReady')
            self.assertFalse(receipt['execution_qualified'])
            self.assertEqual(len(receipt['preservation']),4)
            self.assertTrue(all(row['status']=='unverified' for row in receipt['preservation']))
            self.assertIn('original fixture failure',receipt['original_error'])
            with self.assertRaises(api.Rejected):
                api._failure_receipt(RuntimeError('second'),root,None,None,invocation,None,None,[],None,None)

    def test_owned_process_semantic_mutations(self):
        # This unit-test verifier stub cannot manufacture a production
        # ProbeRecord. The real API always performs registry verification first.
        now=api.time.time_ns()
        invocation=types.SimpleNamespace(document=lambda:{'nonce':'b'*32,'started_ns':now-100})
        original=dict(status=0,pid=7,started_ns=now-50,completed_ns=now,
            cleanup_complete=True,cleanup_error=None,failure=None,timed_out=False,
            stream_limit_exceeded=False,invocation_nonce='b'*32)
        mutations=({'status':1},{'status':False},{'pid':0},{'cleanup_complete':False},
                   {'failure':'failed'},{'timed_out':True},{'stream_limit_exceeded':True},
                   {'invocation_nonce':'c'*32},{'started_ns':now-1000},{'completed_ns':now+10**12})
        for mutation in mutations:
            supervisor=types.SimpleNamespace(verify_probe=lambda *args,**kwargs:dict(original,**mutation))
            with self.subTest(mutation=mutation),mock.patch.object(api,'_preparation_api',return_value=supervisor):
                with self.assertRaises(api.Rejected):
                    api._verify_probe(object(),invocation,['fixture'],Path('/fixture'),{},30,'probe')

    def test_failure_and_success_schemas_are_distinct(self):
        text=(HERE/'portable.py').read_text()
        self.assertIn('oxid-package-source-parser-build-failure-v1',text)
        self.assertIn('oxid-package-source-parser-build-v1',text)
        self.assertNotIn("schema='oxid-unit4-parser-build-v1'",text)

    def test_fixed_retained_comparison_authority(self):
        a=api._manifest()
        self.assertEqual(a['retained_portable']['sha256'],'3366c49f267ab98bd792a1cd1b01ebbaaca548bc19685d841eb688cf39f99162')
        self.assertEqual(a['retained_authority']['sha256'],'d8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c')
        self.assertEqual(a['source_authority']['sha256'],'8473ef19379f0c12995d4e8ac10b71abc5af99023f68af1add493ae852adc0be')

class ReadLifetimeControls(unittest.TestCase):
    def fixture(self):
        from contextlib import contextmanager
        events=[]
        @contextmanager
        def checked(invocation):
            events.append('enter')
            try:yield
            finally:events.append('exit')
        context=types.SimpleNamespace(invocation=lambda:invocation)
        invocation=object();old=object()
        a=api.AdmittedAuthority(context,{'source':[1]},{'recipe':[2]},old,api._AUTHORITY_TOKEN)
        preparation=types.SimpleNamespace(checked_scope=checked,check_deadline=lambda i:True,
            full_verify_invocation=lambda i:events.append('full-invocation'),
            context_api=lambda:types.SimpleNamespace(verify_context=lambda c:events.append('full-context')))
        return a,preparation,object(),events
    def test_nested_read_scope_reuses_only_defensive_metadata(self):
        a,p,m,events=self.fixture()
        with mock.patch.object(api,'_preparation_api',return_value=p),mock.patch.object(api,'_machine_api',return_value=m),mock.patch.object(api,'authority',return_value=a) as verify:
            with api._read_scope(a):
                retained=api._READ_OWNER.get();_,recipe,doc=api._admit(a)
                recipe['recipe'].append(99);doc['source'].append(99)
                with api._read_scope(a):
                    _,again,document=api._admit(a)
                    self.assertEqual(again,{'recipe':[2]});self.assertEqual(document,{'source':[1]})
                self.assertEqual(verify.call_count,1)
            self.assertEqual(events,['enter','full-context','full-invocation','exit'])
            self.assertFalse(retained['active']);self.assertIsNone(retained['admitted']);self.assertIsNone(api._READ_OWNER.get())
            token=api._READ_OWNER.set(retained)
            try:
                with self.assertRaises(api.Rejected):api._read_owner(a)
            finally:api._READ_OWNER.reset(token)
    def test_read_failure_poisons_future_lifetimes(self):
        a,p,m,events=self.fixture()
        with mock.patch.object(api,'_preparation_api',return_value=p),mock.patch.object(api,'_machine_api',return_value=m):
            with self.assertRaisesRegex(RuntimeError,'fixture drift'):
                with api._read_scope(a):raise RuntimeError('fixture drift')
            self.assertIsNone(api._READ_OWNER.get())
            with self.assertRaises(api.Rejected):
                with api._read_scope(a):pass
    def test_read_scope_rejects_other_authority_and_thread(self):
        for change in ('authority','thread'):
            a,p,m,events=self.fixture();other=self.fixture()[0]
            with mock.patch.object(api,'_preparation_api',return_value=p),mock.patch.object(api,'_machine_api',return_value=m):
                with self.assertRaises(api.Rejected):
                    with api._read_scope(a):
                        if change=='authority':api._read_owner(other)
                        else:
                            with mock.patch.object(api.threading,'get_ident',return_value=-1):api._read_owner(a)
    def test_read_owner_rejects_mutated_authority_metadata(self):
        a,p,m,events=self.fixture()
        with mock.patch.object(api,'_preparation_api',return_value=p),mock.patch.object(api,'_machine_api',return_value=m):
            with self.assertRaises(api.Rejected):
                with api._read_scope(a):
                    object.__setattr__(a,'_recipe',b'{"changed":true}')
                    api._read_owner(a)

    def test_cached_context_is_fully_rechecked_on_exit(self):
        a,p,m,events=self.fixture()
        p.context_api=lambda:types.SimpleNamespace(verify_context=lambda c:(_ for _ in ()).throw(api.Rejected('prepared context drift')))
        with mock.patch.object(api,'_preparation_api',return_value=p),mock.patch.object(api,'_machine_api',return_value=m),mock.patch.object(api,'authority',return_value=a):
            with self.assertRaisesRegex(api.Rejected,'prepared context drift'):
                with api._read_scope(a):api._admit(a)
            self.assertIn(a,api._POISONED);self.assertIsNone(api._READ_OWNER.get())
    def test_cached_machine_is_fully_rechecked_on_exit(self):
        a,p,_,events=self.fixture();calls=[]
        machine=types.SimpleNamespace(document=lambda:{'machine':'same'})
        m=types.SimpleNamespace(verify_machine=lambda *args:calls.append(args))
        with mock.patch.object(api,'_preparation_api',return_value=p),mock.patch.object(api,'_machine_api',return_value=m):
            with api._read_scope(a):
                owner=api._read_owner(a);owner['machine']=machine;owner['machine_document']={'machine':'same'}
            self.assertEqual(len(calls),1);self.assertIs(calls[0][0],machine)

    def test_read_owner_rechecks_canonical_modules(self):
        a,p,m,events=self.fixture()
        with mock.patch.object(api,'_preparation_api',return_value=p),mock.patch.object(api,'_machine_api',return_value=m):
            with self.assertRaises(api.Rejected):
                with api._read_scope(a):
                    with mock.patch.object(api,'_machine_api',return_value=object()):api._read_owner(a)

if __name__=='__main__':unittest.main()
