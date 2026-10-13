"""Source-extracted original verifier/recipe with inert authority and path mocks.

These exercise the unchanged branch bodies; they do not issue any live authority.
"""
import ast
import copy
from pathlib import Path
import sys
import types
import unittest

ROOT=Path(__file__).resolve().parent
SOURCE=ROOT.parent/'package-unit4-consumers-v11/candidate/tests/qualification/package_dependency_unit4_current/preparation.py'
TREE=ast.parse(SOURCE.read_text())
NAMES={'verify_probe','_public_runtime_recipe'}
BODIES=[n for n in TREE.body if isinstance(n,ast.FunctionDef) and n.name in NAMES]
assert len(BODIES)==2

class OriginalProbeRoutes(unittest.TestCase):
    def test_original_generator_compiler_native_recipes_and_verification(self):
        calls=[];invocation=object();public=object();runtime_case={'section':'lifecycle','index':0,'variant':'ordinary'}
        class RuntimePermit:
            def __init__(self,document):self.value=document
            def document(self):return copy.deepcopy(self.value)
        native=RuntimePermit({'runtime_case':runtime_case,'body_pins':[],
                              'binary':{'path':'/virtual/root/native-source-free/program'}})
        def need(condition,message):
            if not condition:raise ValueError(message)
        empty='e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
        def identity(path):return {'path':str(path),'bytes':0,'sha256':empty}
        contract=types.SimpleNamespace(verified=[dict(identity('/source/generate_fixtures.py'),
            authority_path='/retained/typed_project_unit1_compatibility/generate_fixtures.py')])
        env={'LANG':'C.UTF-8'};fixture=Path('/virtual/root/fixture');evidence=Path('/virtual/root')
        compiler=['/virtual/compiler','check','example.ox']
        def selected(d,permit,case):
            need(permit is public and case==runtime_case,'original selected case')
            return ('lifecycle',{},compiler,fixture,evidence)
        def permit_owner(permit,inv,kind):
            need(permit is public and inv is invocation and kind=='public','original permit association')
            return {'root':'/virtual/root'}
        def recipe_environment(permit,inv,kind,case,selected):calls.append(('recipe',kind));return env
        namespace=dict(Path=Path,sys=sys,need=need,identity=identity,RuntimePermit=RuntimePermit,
            _PROBES={},_PUBLIC_REPLAY={'permit':public},_PUBLIC_CONTRACTS={public:{'contracts':contract}},
            _NATIVE={native:{'invocation':invocation,'runtime_permit':public}},_GUARD_LINKS={},
            _runtime_permit=permit_owner,_runtime_permit_owner=permit_owner,
            _replay_owner=lambda permit,inv:calls.append(('replay',permit,inv)),
            _public_case_verified=selected,_public_selected_pair=lambda *a:None,
            _public_environment_verified=recipe_environment,verify_invocation=lambda inv:need(inv is invocation,'invocation'),
            _probe_body_identity=lambda row,*a:row,_verify_view_rows=lambda *a,**k:None,
            _public_probe_view_owner=lambda *a:None,_native_terminal=lambda *a:None,
            _probe_stream=lambda record,name:calls.append(('stream',name)),
            _verify_link=lambda *a:None)
        exec(compile(ast.Module(body=BODIES,type_ignores=[]),str(SOURCE),'exec'),namespace)
        class Record:
            def __init__(self,document):self.value=document
            def document(self):return copy.deepcopy(self.value)
        for kind in ('generator','public-runtime','public-native'):
            with self.subTest(kind=kind):
                case=None if kind=='generator' else runtime_case
                n=native if kind=='public-native' else None
                argv=([sys.executable,'/virtual/root/public-runtime/original-generator/generate_fixtures.py'] if kind=='generator'
                      else compiler if kind=='public-runtime' else [native.document()['binary']['path']])
                cwd=(Path('/virtual/root/public-runtime/original-generator') if kind=='generator' else
                     fixture if kind=='public-runtime' else evidence/'native-source-free')
                doc=dict(argv=argv,cwd=str(cwd),environment=env,timeout_seconds=60 if kind=='generator' else 120,
                    stream_limit_bytes=8388608,operation_kind=kind,runtime_case=case,cleanup_complete=True,
                    cleanup_error=None,failure=None,timed_out=False,stream_limit_exceeded=False,
                    executable=identity(sys.executable),process_tree=identity('/virtual/tree.py'),
                    executable_route={'path':sys.executable,'target':str(Path(sys.executable).resolve())},helper=None)
                record=Record(doc)
                namespace['_PROBES'][record]=dict(invocation=invocation,runtime_permit=public,native_permit=n,
                    directory=types.SimpleNamespace(iterdir=lambda:[Path(name) for name in ('command.json','stdout','stderr','result.json')]),
                    result=identity('/virtual/result.json'),command=identity('/virtual/command.json'))
                args=dict(argv=argv,cwd=cwd,environment=env,timeout_seconds=doc['timeout_seconds'],stream_limit_bytes=8388608,
                          operation_kind=kind,runtime_permit=public,native_permit=n,runtime_case=case)
                self.assertEqual(namespace['verify_probe'](record,invocation,**args),doc)
                with self.assertRaisesRegex(ValueError,'permit'):
                    namespace['verify_probe'](record,invocation,**dict(args,runtime_permit=object()))
                with self.assertRaisesRegex(ValueError,'tuple/native permit'):
                    namespace['verify_probe'](record,invocation,**dict(args,native_permit=object()))
                record.value['cleanup_complete']=False
                with self.assertRaisesRegex(ValueError,'bounded completion'):
                    namespace['verify_probe'](record,invocation,**args)
        self.assertEqual([entry for entry in calls if entry[0]=='stream'],[('stream','stdout'),('stream','stderr')]*3)
        self.assertEqual(set(entry[1] for entry in calls if entry[0]=='recipe'),{'generator','public-runtime','public-native'})

    def test_consumer_parser_keeps_literal_full_sequence_and_638(self):
        path=SOURCE.parents[1]/'package_dependency_unit4_parser_v1/runtime.py'
        tree=ast.parse(path.read_text())
        sequence=next(n for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='SEQUENCE' for t in n.targets))
        self.assertEqual(ast.literal_eval(sequence.value),('passivity-debug','collect-debug','passivity-release','collect-release','u8','phase','compare'))
        compare=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='compare')
        self.assertTrue(any(isinstance(n,ast.Call) and isinstance(n.func,ast.Name) and n.func.id=='same' and len(n.args)>=2 and
                            ast.unparse(n.args[0])=='len(rows)' and isinstance(n.args[1],ast.Constant) and n.args[1].value==638
                            for n in ast.walk(compare)))
