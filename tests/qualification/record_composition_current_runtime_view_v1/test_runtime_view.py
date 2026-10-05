#!/usr/bin/env python3
import copy
import unittest
import runtime_view as v


def record(number=0): return {'tag': 'Record', 'items': [number]}
def carrier(tag, number=0): return {'tag': tag, 'items': [record(number)]}
def observation():
    field = {'tag': 'RawFieldDecl', 'id': [0, 0], 'span': [0, 1, 2], 'ty': {'tag': 'Value', 'items': [{'tag': 'Scalar', 'scalar': {'tag': 'I32'}}]}}
    raw = {'tag': 'RawOwnedProgram', 'records': [{'tag': 'RawRecordDecl', 'id': 0, 'span': [0, 1, 2], 'fields': [field]}],
           'functions': [{'tag': 'RawOwnedFunction', 'id': 0, 'span': [0, 1, 2], 'result': {'tag': 'Owned', 'items': [record()]}, 'parameters': [], 'locals': [], 'places': [],
                          'owners': [{'tag': 'OwnerDecl', 'aggregate': carrier('AggregateSlot'), 'kind': {'tag': 'Temporary'}, 'span': [0, 1, 2]}],
                          'references': [{'tag': 'ReferenceDecl', 'referent': carrier('BorrowedSlot'), 'kind': {'tag': 'Shared'}, 'position': 0, 'span': [0, 1, 2]}], 'calls': [],
                          'loans': [{'tag': 'LoanDecl', 'referent': carrier('BorrowedSlot'), 'kind': {'tag': 'Exclusive'}, 'call': 0, 'argument': 0, 'authority': {'tag': 'Owner', 'items': [0]}, 'span': [0, 1, 2]}],
                          'entry': 0, 'blocks': [{'tag': 'OwnedBlock', 'merge': None, 'span': [0, 1, 2], 'statements': [], 'terminator': None}]}]}
    return {'schema': 1, 'loaded': {'unchanged': True}, 'index': {}, 'namespace_work': {}, 'audit': {}, 'reference': {'reference-default': {'outcome': {'result': 7}, 'trace': []}}, 'native': [],
            'route': 'owned', 'raw': raw, 'raw_before_audit': copy.deepcopy(raw), 'checked_immutable': True, 'entry': 0}


def sites(raw):
    f = raw['functions'][0]
    return [f['owners'][0]['aggregate'], f['references'][0]['referent'], f['loans'][0]['referent']]


class RuntimeView(unittest.TestCase):
    def project(self, original): return v.project_bytes(v.canonical(original))
    def reject(self, original):
        with self.assertRaises(ValueError): self.project(original)

    def test_exact_all_sites_round_trip_and_nonraw_preservation(self):
        original = observation(); data = v.canonical(original); projected, receipt = v.project_bytes(data)
        self.assertEqual(receipt['original'], v.identity(data)); self.assertEqual(receipt['derived'], v.identity(projected))
        self.assertEqual(receipt['round_trip_sha256'], v.digest(data)); self.assertEqual(len(receipt['mapped_sites']), 8)
        self.assertEqual(v.reverse_bytes(projected), data)
        derived = v.decode(projected)
        for name in set(original) - {'raw', 'raw_before_audit'}: self.assertEqual(original[name], derived[name])
        self.assertEqual(original, observation(), 'caller must remain unmodified')

    def test_out_of_range_declaration_references_preserved(self):
        original = observation()
        for name in ('raw', 'raw_before_audit'):
            for site in sites(original[name]): site['items'][0]['items'][0] = v.U32_MAX
            original[name]['functions'][0]['result']['items'][0]['items'][0] = v.USIZE_MAX
        projected, receipt = self.project(original)
        self.assertEqual(v.reverse_bytes(projected), v.canonical(original))
        self.assertEqual({x['record'] for x in receipt['mapped_sites']}, {v.U32_MAX, v.USIZE_MAX})

    def test_bool_negative_string_and_overwide_slot_ids_rejected(self):
        for index in range(3):
            for invalid in (True, False, -1, '0', 0.0, 1 << 32):
                with self.subTest(index=index, invalid=invalid):
                    original=observation();sites(original['raw'])[index]['items'][0]['items'][0]=invalid;self.reject(original)

    def test_bool_negative_string_and_overwide_result_ids_rejected(self):
        for invalid in (True, -1, '0', 0.0, 1 << 64):
            original=observation();original['raw']['functions'][0]['result']['items'][0]['items'][0]=invalid;self.reject(original)

    def test_missing_extra_keys_and_tags_at_each_carrier_depth_rejected(self):
        for index in range(3):
            for depth in (0, 1):
                for mutation in ('missing', 'extra', 'tag', 'arity'):
                    with self.subTest(index=index, depth=depth, mutation=mutation):
                        original=observation();site=sites(original['raw'])[index];node=site if depth==0 else site['items'][0]
                        if mutation=='missing': del node['items']
                        elif mutation=='extra': node['extra']=0
                        elif mutation=='tag': node['tag']='Array'
                        else: node['items'].append(0)
                        self.reject(original)

    def test_missing_extra_descriptor_and_result_keys_rejected(self):
        for table in ('owners','references','loans'):
            for mutation in ('missing','extra'):
                original=observation();node=original['raw']['functions'][0][table][0]
                if mutation=='missing':del node['span']
                else:node['extra']=0
                self.reject(original)
        for mutation in ('missing','extra','tag','arity'):
            original=observation();node=original['raw']['functions'][0]['result']
            if mutation=='missing':del node['items']
            elif mutation=='extra':node['extra']=0
            elif mutation=='tag':node['tag']='Reference'
            else:node['items'].append(0)
            self.reject(original)

    def test_arrays_slices_and_aggregate_stored_fields_rejected(self):
        for tag in ('FixedArray','ScalarSlice','Exact'):
            for index in range(3):
                original=observation();sites(original['raw'])[index]['items'][0]['tag']=tag;self.reject(original)
        original=observation();original['raw']['records'][0]['fields'][0]['ty']={'tag':'Value','items':[{'tag':'Owned','items':[record()]}]};self.reject(original)

    def test_all_array_composition_instructions_rejected(self):
        for tag in ('ConstructComposite','ReadProjection','WriteProjection','ProjectionLength','ConstructArray','ReadIndex','WriteIndex','ArrayLength'):
            original=observation();original['raw']['functions'][0]['blocks'][0]['statements']=[{'tag':'OwnedStatement','span':[0,1,2],'diagnostic_origins':None,'kind':{'tag':tag}}];self.reject(original)

    def test_double_projection_of_mapped_view_rejected(self):
        projected,_=self.project(observation())
        with self.assertRaises(ValueError):v.project_bytes(projected)

    def test_scalar_route_and_preraw_rejections_identity(self):
        for mode in ('scalar','no-raw'):
            original=observation();del original['raw'];del original['raw_before_audit'];original.pop('route')
            if mode=='scalar': original.update(route='scalar',raw={'tag':'Program','functions':[]})
            projected,receipt=self.project(original)
            self.assertEqual(projected,v.canonical(original));self.assertEqual(receipt['mapped_sites'],[])

    def test_wrong_root_extra_keys_and_unknown_route_rejected(self):
        original=observation();original['extra']=1;self.reject(original)
        original=observation();original['route']='array';self.reject(original)

    def test_route_relabel_cannot_bypass_owned_domain(self):
        original=observation();original['route']='scalar'
        sites(original['raw'])[0]['items'][0]['tag']='FixedArray'
        self.reject(original)
        original=observation();del original['raw'];del original['raw_before_audit'];original['route']='array'
        self.reject(original)
        original=observation();original['raw']['tag']='Program';self.reject(original)

    def test_duplicate_noncanonical_and_nonfinite_json_rejected(self):
        for data in (b'{"schema":1,"schema":1}',b'{"a":NaN}',b'{ "a": 0 }'):
            with self.assertRaises(ValueError):v.project_bytes(data)

    def test_trace_semantics_preserved_exactly(self):
        original=observation();original['reference']['reference-default']['trace']=[{'tag':'Row','context':None,'kind':'charge_attempt','payload':[23,2,[0,1,2],False]}]
        projected,_=self.project(original)
        self.assertEqual(v.decode(projected)['reference'],original['reference'])

    def test_nonlegacy_instruction_in_trace_rejected(self):
        original=observation();original['reference']['reference-default']['trace']=[{'tag':'Row','kind':'operation_start','payload':[], 'context':{'operation':{'tag':'OwnedStatement','kind':{'tag':'ReadProjection'}}}}];self.reject(original)


if __name__=='__main__':unittest.main()
