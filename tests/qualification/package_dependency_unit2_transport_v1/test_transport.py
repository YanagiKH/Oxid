"""Pure metadata/derivation controls only; no preflight, prepare or assemble runs."""
import copy
import os
from pathlib import Path
import sys
sys.dont_write_bytecode=True
import tempfile
import unittest
import transport as t

HERE=Path(__file__).resolve().parent
REPO=Path(os.environ.get('OXID_PACKAGE_TRANSPORT_TEST_REPOSITORY',str(HERE.parents[2]))).resolve()
FROZEN=Path(os.environ.get('OXID_PACKAGE_TRANSPORT_TEST_FROZEN',str(HERE.parent/'package_dependency_current'))).resolve()

class Controls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.gate,cls.seal=t.anchor(FROZEN)
        cls.context,cls.refs=cls.gate.capture(REPO)
        _,_,raw=cls.gate.anchored_data();cls.new=raw['package-dependency-source-v1.json']
        # Original package is an unqualified resource fixture for pure assembly
        # mechanics. It is NOT the adapted historical prepared resource package.
        prefix=t.U2
        original={n[len(prefix):]:b for n,b in cls.refs.items() if n.startswith(prefix)}
        cls.fixture=original
        cls.package=t.derive_current_package(original,cls.new)
        cls.assembled,cls.changes,cls.added=t.expected_assembly(cls.context['current_inputs'],cls.package)
        cls.history=t.historical_bindings_data(cls.refs,cls.gate)
    def test_pure_assembly_shape_and_new_main(self):
        self.assertEqual((len(self.assembled),len(self.changes),len(self.added)),(380,5,4))
        self.assertEqual(self.assembled['src/main.rs'],self.context['current_inputs']['src/main.rs'])
        self.assertNotEqual(self.assembled['src/main.rs'],self.context['predecessor_inputs']['src/main.rs'])
        t.validate_transport(self.context['current_inputs'],self.package,self.assembled,self.changes,self.added)
    def test_exact_two_file_package_change(self):
        self.assertEqual({n for n in self.package if self.package[n]!=self.fixture[n]},{'source-inputs.json','package-inputs.json'})
        self.assertEqual(self.package['source-inputs.json'],self.new)
    def test_old_main_replay_in_assembled_is_rejected(self):
        bodies=dict(self.assembled);bodies['src/main.rs']=self.context['predecessor_inputs']['src/main.rs']
        with self.assertRaises(ValueError):t.validate_transport(self.context['current_inputs'],self.package,bodies,self.changes,self.added)
    def test_added_file_missing_extra_mutated(self):
        for kind in ('missing','extra','changed'):
            bodies=dict(self.assembled)
            if kind=='missing':bodies.pop(self.added[0])
            elif kind=='extra':bodies['src/extra.rs']=b''
            else:bodies[self.added[0]]+=b' '
            with self.subTest(kind=kind),self.assertRaises(ValueError):t.validate_transport(self.context['current_inputs'],self.package,bodies,self.changes,self.added)
    def test_transcript_changed_missing_or_addition_reordered(self):
        for changes,added in ((self.changes[:-1],self.added),(list(reversed(self.changes)),self.added),(self.changes,list(reversed(self.added)))):
            with self.assertRaises(ValueError):t.validate_transport(self.context['current_inputs'],self.package,self.assembled,changes,added)
    def test_untouched_fixture_change_rejected(self):
        bodies=dict(self.assembled);name=next(n for n in bodies if n.startswith('tests/fixtures/'));bodies[name]+=b' '
        with self.assertRaises(ValueError):t.validate_transport(self.context['current_inputs'],self.package,bodies,self.changes,self.added)
    def test_source_package_corruption(self):
        package=dict(self.fixture);package['source-inputs.json']+=b' '
        with self.assertRaises(ValueError):t.derive_current_package(package,self.new)
        package=dict(self.fixture);package['extra']=b''
        with self.assertRaises(ValueError):t.derive_current_package(package,self.new)
    def test_original_fixture_is_not_admitted_as_current_resource(self):
        with self.assertRaises(ValueError):t.verify_resource_authorities(self.fixture,self.refs,self.gate)
    def test_both_historical_versions_distinct_and_nested(self):
        older=self.history[t.OLD_KEY];direct=self.history[t.DIRECT_KEY]
        self.assertEqual(direct['predecessor_accounting_source_binding'],older)
        self.assertNotEqual(older['current_source'],direct['current_source'])
        self.assertEqual(direct['current_source']['sha256'],self.gate.OLD_SHA)
        self.assertEqual(direct['version'],'unit2-lexer-reservation-identical-accounting-source-v1')
        self.assertEqual(older['version'],'unit2-byte-storage-identical-accounting-source-v1')
    def test_each_preserved_accounting_field_rejects_mutation(self):
        for key in (t.OLD_KEY,t.DIRECT_KEY):
            for field in self.history[key]:
                changed=copy.deepcopy(self.history);changed[key][field]='mutated'
                with self.subTest(key=key,field=field),self.assertRaises(ValueError):t.require_exact_accounting(changed,self.history)
        changed=copy.deepcopy(self.history);changed[t.DIRECT_KEY]['predecessor_accounting_source_binding']=changed[t.DIRECT_KEY].copy()
        with self.assertRaises(ValueError):t.require_exact_accounting(changed,self.history)
    def test_accounting_resource_replay_rejects(self):
        with self.assertRaises(ValueError):t.historical_bindings(self.refs,b'wrong resource',self.gate)
    def test_no_execution_route(self):
        with self.assertRaisesRegex(RuntimeError,'NotReady'):t.execute(self.context)
    def test_invalid_repository_creates_no_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);out=root/'out'
            with self.assertRaises(ValueError):t.prepare(root/'missing',out,FROZEN)
            self.assertFalse(out.exists())
    def test_existing_output_untouched(self):
        with tempfile.TemporaryDirectory() as directory:
            out=Path(directory);(out/'sentinel').write_bytes(b'safe')
            with self.assertRaises(ValueError):t.prepare(REPO,out,FROZEN)
            self.assertEqual(list(out.iterdir()),[out/'sentinel'])
            self.assertEqual((out/'sentinel').read_bytes(),b'safe')
    def test_exact_assembler_path_order_regression(self):
        # Actual attempt1 had identical380bodies but91positions differed because
        # Path sorts components while strings compare slash versus dot bytes.
        producer=t.assembler_inventory_rows(self.assembled)
        string_order=[t.row(n,b) for n,b in sorted(self.assembled.items())]
        independent=[t.row(str(n),self.assembled[str(n)]) for n in sorted(Path(n) for n in self.assembled)]
        self.assertEqual(producer,independent)
        self.assertNotEqual(producer,string_order)
        self.assertEqual(sum(a!=b for a,b in zip(producer,string_order)),91)
        self.assertEqual(sorted(producer,key=lambda r:r['path']),string_order)
    def test_assembler_receipt_strict_rejection_controls(self):
        valid=dict(files=t.assembler_inventory_rows(self.assembled),instrumentation=self.changes,source_inputs_sha256=t.sha(self.new))
        t.verify_assembler_receipt(valid,self.assembled,self.new,self.changes)
        for kind in ('string-order','reorder','duplicate','missing','extra','changed-row','wrong-source','unknown-field'):
            value=copy.deepcopy(valid)
            if kind=='string-order':value['files'].sort(key=lambda r:r['path'])
            elif kind=='reorder':value['files'][0],value['files'][1]=value['files'][1],value['files'][0]
            elif kind=='duplicate':value['files'][1]=value['files'][0]
            elif kind=='missing':value['files'].pop()
            elif kind=='extra':value['files'].append(dict(path='extra',bytes=0,sha256=t.sha(b'')))
            elif kind=='changed-row':value['files'][0]['sha256']='0'*64
            elif kind=='wrong-source':value['source_inputs_sha256']='0'*64
            else:value['extra']=True
            with self.subTest(kind=kind),self.assertRaises(ValueError):t.verify_assembler_receipt(value,self.assembled,self.new,self.changes)
    def test_forged_protocol_and_assembler_report_rejected(self):
        declared=dict(protocol_identity=t.row('protocol.py',self.refs[t.U2+'protocol.py']),assembler_identity=t.row('assemble.py',self.refs[t.U2+'assemble.py']))
        t.verify_declared_modules(declared,self.refs)
        for key in declared:
            changed=copy.deepcopy(declared);changed[key]['sha256']='0'*64
            with self.subTest(key=key),self.assertRaises(ValueError):t.verify_declared_modules(changed,self.refs)
    def test_wrong_relative_frozen_helper_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            fake=Path(directory)/'other/folder/helper'
            with self.assertRaisesRegex(ValueError,'relative path'):t.anchor(fake)
    def test_execution_cli_and_corpus_materializer_absent(self):
        import ast
        tree=ast.parse((HERE/'historical_prepare_worker.py').read_text())
        self.assertFalse(any(isinstance(n,ast.Attribute) and n.attr in ('main','run_unit2','run_unit2_observer_controls') for n in ast.walk(tree)))
        self.assertNotIn('semantic/materialize.py',(HERE/'historical_prepare_worker.py').read_text())

if __name__=='__main__':unittest.main()
