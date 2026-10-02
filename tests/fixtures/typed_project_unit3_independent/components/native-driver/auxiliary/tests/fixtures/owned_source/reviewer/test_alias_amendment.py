"""Source-defined post-observation amendment tests; no compiler processes.

The inventory was derived from original source text before reading candidate
receipts. Select the immutable amended model with OWNED_SOURCE_MODEL_SCRIPTS.
"""
import contextlib,copy,hashlib,io,json,os,pathlib,subprocess,sys,tempfile,unittest
from unittest import mock
ROOT=pathlib.Path(__file__).resolve().parent
sys.path.insert(0,os.environ['OWNED_SOURCE_MODEL_SCRIPTS'])
import owned_source_model as m
import verify_owned_source as h
INVENTORY={x['id']:x for x in json.loads((ROOT/'source-only-alias-pair-inventory.json').read_text())['cases']}


def canonical(pair):
    return (pair['stage'],pair['code'],tuple(pair.get('span',pair.get('primary'))),tuple(pair.get('cause_span',pair.get('cause'))),tuple(pair.get('declaration_span',pair.get('declaration'))))


def diagnostic(pair):
    _,_,primary,cause,declaration=canonical(pair)
    return {'schema_version':1,'edition':'typed-preview','kind':'diagnostic','severity':'error','message':'conflicting live loan',
            'stage':'ownership','code':'E0311','primary':{'start':primary[0],'end':primary[1]},
            'secondary':[{'span':{'start':span[0],'end':span[1]},'message':text}
                         for span,text in ((cause,'loan acquired here'),(declaration,'owner declared here'))],'notes':[]}


class AliasAmendmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases={c.identifier:c for c in m.corpus()}
        cls.expected={name:m.case_expectation(cls.cases[name]) for name in INVENTORY}

    def test_inventory_is_source_defined_and_uniform(self):
        counts={0:0,1:0,2:0}
        for name,wanted in INVENTORY.items():
            actual=self.expected[name]
            self.assertEqual(actual['source_sha256'],wanted['source_sha256'])
            pairs=wanted['first_conflict_per_root'];counts[len(pairs)]+=1
            if not pairs:
                self.assertEqual(actual['expected']['status'],'accept')
                self.assertFalse(actual['expected'].get('diagnostic_alternatives'))
                continue
            alternatives=actual['expected'].get('diagnostic_alternatives',[])
            self.assertEqual({canonical(p) for p in alternatives},{canonical(p) for p in pairs},name)
            self.assertEqual(len(alternatives),len(pairs),name)
        self.assertEqual(counts,{0:74,1:189,2:27})

    def test_every_legal_primary_cause_declaration_tuple_matches(self):
        total=0
        for name,case in self.expected.items():
            for pair in INVENTORY[name]['first_conflict_per_root']:
                chosen=h.match_diagnostic([diagnostic(pair)],case['expected'])
                self.assertIsNotNone(chosen,'matcher must return the observed tuple for determinism checks')
                total+=1
        self.assertEqual(total,243)

    def test_root_alternatives_do_not_cross_pair_causes_or_declarations(self):
        count=0
        for name,entry in INVENTORY.items():
            pairs=entry['first_conflict_per_root']
            if len(pairs)!=2:continue
            expected=self.expected[name]['expected']
            for own,other in (pairs,pairs[::-1]):
                for label in (0,1):
                    bad=diagnostic(own);bad['secondary'][label]=diagnostic(other)['secondary'][label]
                    with self.subTest(case=name,label=label):
                        with self.assertRaises(ValueError):h.match_diagnostic([bad],expected)
                    count+=1
        self.assertEqual(count,108)

    def test_missing_unrelated_partial_or_reordered_evidence_cannot_match(self):
        name='alias-4-0110-ssxx';expected=self.expected[name]['expected']
        good=diagnostic(INVENTORY[name]['first_conflict_per_root'][0])
        mutants=[]
        for primary in (None,{'start':0,'end':1},{'start':good['primary']['start']+1,'end':good['primary']['end']}):
            changed=copy.deepcopy(good);changed['primary']=primary;mutants.append(changed)
        for labels in ([],good['secondary'][:1],good['secondary'][1:],good['secondary'][::-1],good['secondary']+[{'span':{'start':0,'end':1},'message':'unrelated'}]):
            changed=copy.deepcopy(good);changed['secondary']=labels;mutants.append(changed)
        for key,value in (('stage','oir-run'),('code','E0310')):
            changed=copy.deepcopy(good);changed[key]=value;mutants.append(changed)
        for changed in mutants:
            with self.subTest(diagnostic=changed):
                with self.assertRaises(ValueError):h.match_diagnostic([changed],expected)

    def test_later_already_invalid_request_and_later_same_root_cause_are_rejected(self):
        for name,position,change in (('alias-4-0000-ssxx',3,'primary'),('alias-4-0000-sssx',1,'cause')):
            entry=INVENTORY[name];expected=self.expected[name]['expected'];bad=diagnostic(entry['first_conflict_per_root'][0])
            span=entry['arguments'][position]['span'];where={'start':span[0],'end':span[1]}
            if change=='primary':bad['primary']=where
            else:bad['secondary'][0]['span']=where
            with self.subTest(case=name):
                with self.assertRaises(ValueError):h.match_diagnostic([bad],expected)

    def test_unrelated_prior_site_is_not_a_live_cause(self):
        name='alias-4-0110-ssxx';entry=INVENTORY[name]
        bad=diagnostic(entry['first_conflict_per_root'][0]);bad['secondary'][0]['span']={'start':0,'end':6}
        with self.assertRaises(ValueError):h.match_diagnostic([bad],self.expected[name]['expected'])

    def test_standalone_blocks_fail_closed_and_original_sources_remain_negatives(self):
        b=m.Builder();p=m.Program((m.Record('S'),),(m.Function('main',(),'i32',b.block(b.block(b.let('x',b.i(1))),b.ret(b.i(0)))),))
        with self.assertRaisesRegex(ValueError,'MODEL_DOMAIN'):m.render(p)
        byhash={m.case_expectation(c)['source_sha256']:m.case_expectation(c) for c in self.cases.values()}
        for name in ('lexical-reuse','active-shadow'):
            original=json.loads((ROOT/('original-'+name+'-standalone-block.json')).read_text())
            actual=byhash[original['source_sha256']]
            self.assertEqual(actual['source'],original['source'])
            for field in ('status','stage','code','span'):
                self.assertEqual(actual['expected'][field],original['expected'][field])
        reuse=m.case_expectation(self.cases['lexical-reuse']);shadow=m.case_expectation(self.cases['active-shadow'])
        self.assertIn('if true',reuse['source']);self.assertEqual(reuse['expected']['result'],2)
        self.assertIn('if true',shadow['source']);self.assertEqual((shadow['expected']['stage'],shadow['expected']['code']),('resolve','E0201'))
        self.assertEqual(len(self.cases),431)

    def test_permitted_alternative_cannot_change_between_cli_commands(self):
        name='alias-4-0110-ssxx';item=copy.deepcopy(self.expected[name]);source=item.pop('source');pairs=INVENTORY[name]['first_conflict_per_root']
        with tempfile.TemporaryDirectory(prefix='alias-amendment-') as directory:
            root=pathlib.Path(directory);binary=root/'fake';binary.write_bytes(b'fake')
            corpus=root/'corpus';corpus.mkdir();(corpus/(name+'.ox')).write_text(source);(corpus/(name+'.json')).write_text(json.dumps(item))
            calls=[]
            def fake_run(argv,**kwargs):
                operation=argv[1];calls.append(operation)
                selected=pairs[1] if operation=='run' else pairs[0]
                summary={'schema_version':1,'edition':'typed-preview','kind':operation+'-summary','success':False,'errors':1,
                         {'check':'functions','run':'result','compile':'output'}[operation]:None}
                output=b''.join((json.dumps(x)+'\n').encode() for x in (diagnostic(selected),summary))
                return subprocess.CompletedProcess(argv,1,output,b'')
            with mock.patch.object(h,'verify_frozen',return_value={'files':[{'id':name}]}),mock.patch.object(h.subprocess,'run',side_effect=fake_run),contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(ValueError):h.profile_worker(binary,'debug',corpus,root/'evidence')
            self.assertIn('run',calls,'failure must occur after two permitted alternative observations')

    def test_cross_profile_selection_is_deterministic_without_qualifying_synthetic_work(self):
        name='alias-4-0110-ssxx';item=copy.deepcopy(self.expected[name]);source=item.pop('source');pairs=INVENTORY[name]['first_conflict_per_root']
        for drift in (False,True):
            with self.subTest(drift=drift),tempfile.TemporaryDirectory(prefix='alias-profile-amendment-') as directory:
                root=pathlib.Path(directory);binary=root/'fake';binary.write_bytes(b'fake');binary.chmod(0o700)
                corpus=root/'corpus';corpus.mkdir();(corpus/(name+'.ox')).write_text(source);(corpus/(name+'.json')).write_text(json.dumps(item))
                evidence=root/'evidence'
                def fake_run(argv,**kwargs):
                    operation=argv[1]
                    selected=pairs[1] if drift and 'release' in pathlib.Path(kwargs['cwd']).parts else pairs[0]
                    summary={'schema_version':1,'edition':'typed-preview','kind':operation+'-summary','success':False,'errors':1,
                             {'check':'functions','run':'result','compile':'output'}[operation]:None}
                    output=b''.join((json.dumps(x)+'\n').encode() for x in (diagnostic(selected),summary))
                    return subprocess.CompletedProcess(argv,1,output,b'')
                def fake_jobs(*args,**kwargs):
                    for profile in ('debug','release'):h.profile_worker(binary,profile,corpus,evidence/profile)
                    return 0
                with mock.patch.object(h,'verify_frozen',return_value={'files':[{'id':name}]}), \
                     mock.patch.object(h.subprocess,'run',side_effect=fake_run),mock.patch.object(h,'run_jobs',side_effect=fake_jobs), \
                     contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()):
                    result=h.main([str(binary),str(binary),'--frozen',str(corpus),'--evidence',str(evidence)])
                self.assertEqual(result,1 if drift else 2)


if __name__=='__main__':unittest.main(verbosity=2)
