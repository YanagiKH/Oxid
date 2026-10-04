#!/usr/bin/env python3
"""External, source-authority-only comparator. Never invokes or imports Oxid."""
import argparse
import collections
import hashlib
import json
import pathlib
import re
import sys

AUTHORITY_SHA = '7cec518fdc38c02f8cfc6c3cf467239423621bf242e44c3e838be490a3060039'
REVIEW_SHA = '0d70789953ebb1ac6ad05a453bd5428fe9c17b8f6ce17153edcd5e56ac3cde48'
ADAPTER_FREEZE_SHA = 'efcef27051260a591fad67b74f1bfeec0f66534207c84c4298711d94f523f58d'
PROJECT_LEDGER_SHA = 'b49d52901fe25cb190701cf228dd8cb6c1fd7f3375a0543a27e06fcaa156a67b'
PROTOCOL = 'oxid-array-source-typing-observation-v1'
MAX_BYTES = 16 * 1024 * 1024
MAX_UNITS = 200000
CATEGORIES = 'header source phase selector ast_inventory record field signature parameter function binding block expression statement typed_expression typed_binding typed_block reservation diagnostic summary'.split()
PHASES = 'load lex parse provenance index select resolve type'.split()
DEFERRED = ['rhs-snapshot-success', 'rhs-snapshot-final-bounds-failure']

class Rejected(Exception):
    pass

def need(ok, where, message):
    if not ok:
        raise Rejected(f'{where}: {message}')

def equal(actual, expected, where):
    def same(a,e):
        if type(a) is not type(e):return False
        if isinstance(e,dict):return a.keys()==e.keys() and all(same(a[k],v) for k,v in e.items())
        if isinstance(e,(list,tuple)):return len(a)==len(e) and all(same(x,y) for x,y in zip(a,e))
        return a==e
    need(same(actual, expected), where,
         f'expected {expected!r}; got {actual!r}')

def sha(data):
    return hashlib.sha256(data).hexdigest()

def pairs(items):
    out = {}
    for key, value in items:
        need(key not in out, 'json', f'duplicate key {key!r}')
        out[key] = value
    return out

def loads(data):
    try:
        return json.loads(data, object_pairs_hook=pairs,
                          parse_constant=lambda x: (_ for _ in ()).throw(Rejected('json: nonfinite number')))
    except (UnicodeError, ValueError, RecursionError) as e:
        raise Rejected(f'json: {e}') from e

def read_json(path, bound=MAX_BYTES):
    data = pathlib.Path(path).read_bytes()
    need(len(data) <= bound, str(path), 'input byte limit')
    return loads(data)

def keys(obj, required, optional=(), where='shape'):
    need(isinstance(obj, dict), where, 'object required')
    equal(set(obj), set(required) | (set(optional) & set(obj)), where + '.keys')

def uint(v, where):
    need(type(v) is int and 0 <= v <= 2**64-1, where, 'unsigned integer required')
    return v

def units(obj):
    if isinstance(obj, dict):
        return 1 + len(obj) + sum(units(v) for v in obj.values())
    if isinstance(obj, list):
        return 1 + sum(units(v) for v in obj)
    return 1

def span(s):
    return [s['file'], s['start'], s['end']]

def ident_span(s):
    return tuple(span(s) if isinstance(s, dict) else s)

def exact_subset(actual, expected, where):
    """Only explicit source facts use subsets; lengths and generic schema stay exact."""
    if isinstance(expected, dict):
        need(isinstance(actual, dict), where, 'expected object')
        for k, v in expected.items():
            need(k in actual, where, f'missing {k}')
            exact_subset(actual[k], v, where + '.' + k)
    elif isinstance(expected, list):
        equal(len(actual), len(expected), where + '.length')
        for i, (a, e) in enumerate(zip(actual, expected)):
            exact_subset(a, e, f'{where}[{i}]')
    else:
        equal(actual, expected, where)

def normalize_expected(obj):
    if isinstance(obj, dict):
        if {'file', 'start', 'end', 'text'} <= obj.keys():
            return span(obj)
        return {k: normalize_expected(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [normalize_expected(v) for v in obj]
    return obj

class Authority:
    def __init__(self, root, review=None):
        self.root = pathlib.Path(root)
        raw = (self.root / 'freeze-manifest.json').read_bytes()
        equal(sha(raw), AUTHORITY_SHA, 'authority.freeze')
        self.manifest = loads(raw)
        for name, spec in self.manifest['files'].items():
            path = self.root / name
            need(not path.is_symlink(), 'authority', f'symlink {name}')
            data = path.read_bytes()
            equal(len(data), spec['bytes'], 'authority.' + name + '.bytes')
            equal(sha(data), spec['sha256'], 'authority.' + name + '.sha256')
        if review is not None:
            equal(sha(pathlib.Path(review).read_bytes()), REVIEW_SHA, 'authority.review')
        self.cases = self.read('source-manifest.json')['cases']
        self.by_id = {c['id']: c for c in self.cases}
        equal(len(self.by_id), 54, 'authority.case-count')
        equal(sum(len(c['files']) for c in self.cases), 65, 'authority.source-count')
        self.positive = {c['id']: c for c in self.read('positive-facts.json')['cases']}
        reuse = self.read('reused-authority.json')
        self.reused = {c['id']: c['authority_case']['expected'] for c in reuse['selected_cases']}
        self.diagnostics = {c['id']: c for c in reuse['language_diagnostics']}
        self.diagnostics.update({c['id']: c for c in self.read('child-diagnostics.json')['cases']})
        self.resources = {c['id']: c for c in self.read('resource-controls.json')['cases']}
        self.fences = {c['id']: c for c in self.read('fence-controls.json')['cases']}
        ledger=self.root.parent/'typing-independent-review'/'project-work-ledger-source-v1.json'
        equal(sha(ledger.read_bytes()),PROJECT_LEDGER_SHA,'authority.project-work-ledger')
        self.source = {}
        for c in self.cases:
            fs = []
            for f in c['files']:
                b = (self.root / 'fixtures' / c['id'] / f['path']).read_bytes()
                equal(sha(b), f['sha256'], 'authority.fixture')
                b.decode('utf-8')
                fs.append(b)
            self.source[c['id']] = fs

    def read(self, name):
        return read_json(self.root / name)

class Case:
    def __init__(self, authority, case_id, raw, request, binding):
        self.a, self.id, self.request, self.binding = authority, case_id, request, binding
        self.where = case_id
        need(0 < len(raw) <= MAX_BYTES, case_id, 'empty or oversized artifact')
        need(raw.endswith(b'\n'), case_id, 'missing final newline or truncated artifact')
        self.lines = raw.splitlines(keepends=True)
        need(all(line.strip() for line in self.lines), case_id, 'empty JSONL row')
        self.rows = [loads(line) for line in self.lines]
        need(all(isinstance(r, dict) and 'row' in r for r in self.rows), case_id, 'row discriminator required')
        self.g = collections.defaultdict(list)
        for r in self.rows:
            need(r['row'] in CATEGORIES + ['typed_statement', 'complete'], case_id, f'unknown row {r["row"]}')
            numeric={
                'source':['file','identity','bytes'],'phase':['attempted_files','completed_files'],
                'selector':['module','file'],'ast_inventory':['ast_expression_count','E_ast','target_wrappers','modules'],
                'record':['id','fields'],'field':['record','id'],'signature':['function','module','local','parameters'],
                'parameter':['function','position'],'function':['id','body','bindings','expressions','blocks'],
                'binding':['function','id','scope','parameter_position'],'block':['function','id','statements'],
                'expression':['function','id','binding','child','base','index','operand','left','right','target','record'],
                'statement':['function','block','position','binding','init','base','value','index','target','loop_id','condition','body','then','else'],
                'typed_expression':['function','id'],'typed_binding':['function','id'],'typed_block':['function','id'],
                'typed_statement':['function','block','position'],
                'reservation':['attempt','length','element_bytes','cumulative_hir_entries'],
                'summary':['entry','hir_expression_count','E_hir','ast_expression_count','E_ast','target_wrappers','work','allocator_attempts','trace_rows','trace_row_bytes','work_observation_rows','functions','signatures','records'],
                'complete':['rows_before_footer','bytes_before_footer','semantic_units_before_footer','diagnostics','auxiliary_requested_bytes','auxiliary_peak_requested_bytes','diagnostic_render_bound_bytes','descriptor_overlap_bound_bytes'],
                'header':['row_limit','byte_limit','retained_limit','scratch_limit','work_limit'],
            }.get(r['row'],[])
            nullable={'parameter_position','entry','else'}
            if r['row']=='statement' and r.get('kind')=='Return':nullable.add('value')
            for key in numeric:
                if key in r and not (r[key] is None and key in nullable):uint(r[key],case_id+'.'+r['row']+'.'+key)
            for key in ['elements','roots']:
                if key in r:
                    need(isinstance(r[key],list),case_id,key+' must be list')
                    for value in r[key]:uint(value,case_id+'.'+key)
            if r['row']=='expression':
                for argument in r.get('arguments',[]):
                    for key in ['expression','binding']:
                        if key in argument:uint(argument[key],case_id+'.argument.'+key)
                for field in r.get('fields',[]):
                    for key in ['record','field','value']:uint(field[key],case_id+'.field-init.'+key)
            if r['row']=='complete':
                for value in r['category_counts'].values():uint(value,case_id+'.category-count')
            self.g[r['row']].append(r)
        self.raw = raw
        self.sources = authority.source[case_id]
        self.fns = {}

    def one(self, category):
        equal(len(self.g[category]), 1, f'{self.id}.{category}.count')
        return self.g[category][0]

    def origin(self, value, where):
        need(isinstance(value, list) and len(value) == 3, where, 'span [file,start,end] required')
        f, start, end = [uint(v, where) for v in value]
        need(f < len(self.sources), where, 'unknown source file')
        raw = self.sources[f]
        need(start <= end <= len(raw), where, 'out-of-range origin')
        try:
            raw[:start].decode('utf-8'); raw[start:end].decode('utf-8'); raw[end:].decode('utf-8')
        except UnicodeError as e:
            raise Rejected(f'{where}: UTF-8 boundary') from e
        return raw[start:end].decode('utf-8')

    def loc(self, f, start, end):
        self.origin([f, start, end], self.id + '.diagnostic-span')
        raw = self.sources[f]
        def lc(n):
            s = raw[:n].decode('utf-8')
            return s.count('\n') + 1, len(s.rsplit('\n', 1)[-1]) + 1
        l, c = lc(start); el, ec = lc(end)
        return dict(file_id=f, path=self.a.by_id[self.id]['files'][f]['path'], start=start, end=end,
                    line=l, column=c, end_line=el, end_column=ec)

    def ty(self, t, parameter=False, where='type'):
        need(isinstance(t, dict) and 'tag' in t, where, 'canonical type required')
        tag = t['tag']
        if parameter:
            if tag == 'value':
                keys(t, ['tag', 'value'], where=where); self.ty(t['value'], where=where)
            elif tag == 'reference':
                keys(t, ['tag', 'kind', 'aggregate'], where=where)
                need(t['kind'] in ['shared', 'exclusive'], where, 'unknown reference kind')
                self.ty(t['aggregate'], where=where)
                need(t['aggregate']['tag'] in ['fixed_array', 'record'], where, 'reference to nonaggregate')
            else:
                raise Rejected(where + ': ParameterTy required')
        elif tag == 'scalar':
            keys(t, ['tag', 'scalar'], where=where)
            need(t['scalar'] in ['bool', 'i32', 'unit'], where, 'unknown scalar')
        elif tag == 'fixed_array':
            keys(t, ['tag', 'element', 'length'], where=where)
            need(t['element'] in ['bool', 'i32', 'unit'], where, 'nonscalar array')
            need(uint(t['length'], where) <= 1024, where, 'array length ceiling')
        elif tag == 'record':
            keys(t, ['tag', 'record_id', 'declaration'], where=where)
            uint(t['record_id'],where+'.record_id')
            need(t['record_id'] in self.records, where, 'unknown nominal RecordId')
            equal(t['declaration'], self.records[t['record_id']]['declaration'], where + '.record-origin')
        else:
            raise Rejected(where + ': unknown ValueTy tag')

    def envelope(self):
        h, end = self.one('header'), self.one('complete')
        equal(self.rows[0], h, self.id + '.header-first')
        equal(self.rows[-1], end, self.id + '.footer-last')
        keys(h, ['row','schema','case','base','core_manifest_sha256','scope','path','admission','row_limit','byte_limit','retained_limit','scratch_limit','work_limit','semantic_metric','work_observation_enabled'], where=self.id + '.header')
        for k, v in dict(schema=PROTOCOL, case=self.id, base=self.binding.get('base',self.a.manifest['selected_base']),
                         core_manifest_sha256=self.binding['core_manifest_sha256'], scope=self.request['scope'],
                         path='private-owned-test-seam', admission='ObserveArrayTypes',
                         semantic_metric='json-values-and-keys', work_observation_enabled=False).items():
            equal(h[k], v, self.id + '.header.' + k)
        for k in ['row','byte','retained','scratch','work']:
            equal(h[k + '_limit'], self.request[k + '_limit'], self.id + '.limit.' + k)
        need(0 < h['row_limit'] <= MAX_UNITS and 0 < h['byte_limit'] <= MAX_BYTES, self.id, 'invalid observation limits')
        need(len(self.raw) <= h['byte_limit'] and sum(units(r) for r in self.rows) <= h['row_limit'], self.id, 'observer limit exceeded')
        keys(end, ['row','complete','outcome','rows_before_footer','bytes_before_footer','semantic_units_before_footer','category_counts','diagnostics','auxiliary_requested_bytes','auxiliary_peak_requested_bytes','diagnostic_render_bound_bytes','descriptor_overlap_bound_bytes'], where=self.id + '.footer')
        equal(end['complete'], True, self.id + '.completion')
        need(end['outcome'] in ['typed-success','diagnostic-complete'], self.id, 'incomplete observation')
        equal(end['rows_before_footer'], len(self.rows)-1, self.id + '.row-count')
        equal(end['bytes_before_footer'], sum(map(len,self.lines[:-1])), self.id + '.byte-count')
        equal(end['semantic_units_before_footer'], sum(units(r) for r in self.rows[:-1]), self.id + '.nested-units')
        counts = {k:len(self.g[k]) for k in CATEGORIES}
        counts['typed_block'] += len(self.g['typed_statement'])
        equal(end['category_counts'], counts, self.id + '.category-counts')
        equal(end['diagnostics'], len(self.g['diagnostic']), self.id + '.diagnostic-count')
        need(uint(end['auxiliary_requested_bytes'], self.id) <= MAX_BYTES, self.id, 'auxiliary bound')
        peak=uint(end['auxiliary_peak_requested_bytes'],self.id)
        need(end['auxiliary_requested_bytes']<=peak<=MAX_BYTES,self.id,'auxiliary peak bound')
        equal(peak,end['auxiliary_requested_bytes']+end['diagnostic_render_bound_bytes'],self.id+'.render-overlap-bound')
        need(0<uint(end['descriptor_overlap_bound_bytes'],self.id)<=end['auxiliary_requested_bytes'],self.id,'descriptor allowance')
        need(end['descriptor_overlap_bound_bytes']>=256*1024+1+2*4096+128,self.id+'.descriptor-overlap','below admitted request/path/label bound before String headers')
        render_bound=0
        for d in self.g['diagnostic']:
            v=d['diagnostic']
            texts=[v['code'],v['stage'],v['message']]+v['notes']+[s['message'] for s in v['secondary']]+[f['path'] for f in self.a.by_id[self.id]['files']]
            render_bound=max(render_bound,16384+128*sum(len(t.encode('utf-8')) for t in texts))
        equal(end['diagnostic_render_bound_bytes'],render_bound,self.id+'.rendering-bound-formula')
        fs = self.a.by_id[self.id]['files']
        equal(len(self.g['source']), len(fs), self.id + '.source-count')
        identities = []
        for r, f, raw in zip(self.g['source'], fs, self.sources):
            keys(r, ['row','file','identity','role','path','original_path','bytes','text'], where=self.id + '.source')
            for k in ['file','path','bytes']: equal(r[k], f[k], self.id + '.source.' + k)
            equal(r['role'], 'root' if f['file']==0 else 'child', self.id + '.source.role')
            equal(r['text'].encode('utf-8'), raw, self.id + '.source.utf8')
            equal(sha(r['text'].encode('utf-8')), f['sha256'], self.id + '.source.hash')
            original = str(pathlib.Path(self.request['fixture_root']) / f['path'])
            equal(r['original_path'], original, self.id + '.explicit-path-relocation')
            identities.append(uint(r['identity'], self.id + '.source.identity'))
        equal(len(set(identities)), len(identities), self.id + '.source.identity-unique')
        return end['outcome']

    def phases(self, outcome):
        p = collections.defaultdict(list)
        for r in self.g['phase']:
            fields=['row','phase','status']+(['attempted_files','completed_files'] if r['phase'] in ['lex','parse'] else [])
            keys(r, fields, where=self.id + '.phase')
            if r['phase'] in ['lex','parse']:
                equal(r['attempted_files'],len(self.sources),self.id+'.phase.files-attempted')
                equal(r['completed_files'],len(self.sources),self.id+'.phase.files-completed')
            need(r['phase'] in PHASES, self.id, 'unknown phase')
            p[r['phase']].append(r['status'])
        for n in ['load','select','provenance']:
            equal(p[n], ['attempted','completed'], self.id + '.phase.' + n)
        for n in ['lex','parse']: equal(p[n], ['completed'], self.id + '.phase.' + n)
        expected = self.a.diagnostics.get(self.id)
        resolve_error = bool(expected and expected['diagnostic']['stage']=='resolve')
        equal(p['resolve'], ['attempted','failed' if resolve_error else 'completed'], self.id + '.phase.resolve')
        if resolve_error:
            equal(p['index'],['attempted','completed'],self.id+'.index-completed-before-language-resolution-error')
            equal(p['type'], ['not-attempted'], self.id + '.phase.type')
            need(not any(self.g[k] for k in ['record','field','signature','parameter','function','binding','block','expression','statement']), self.id, 'exported partial resolved inventory')
        else:
            equal(p['index'], ['attempted','completed'], self.id + '.phase.index')
            equal(p['type'], ['attempted','failed' if expected else 'completed'], self.id + '.phase.type')
        typed = outcome == 'typed-success'
        equal(typed, expected is None, self.id + '.expected-success-or-diagnostic')
        if not typed:
            need(not any(self.g[k] for k in ['typed_expression','typed_binding','typed_block','typed_statement']), self.id, 'partial typed/error overlap')
        if typed: equal(self.g['diagnostic'], [], self.id + '.success-has-no-diagnostics')
        # Require observed completion before dependent inventory, not merely the same marker set.
        def pos(n, status):
            return next(i for i,r in enumerate(self.rows) if r.get('row')=='phase' and r.get('phase')==n and r.get('status')==status)
        need(pos('load','completed') < pos('select','attempted') < pos('provenance','attempted') < pos('resolve','attempted'), self.id, 'phase ordering')
        if not resolve_error:
            need(pos('resolve','completed') < pos('type','attempted'), self.id, 'type began before resolution completed')
            for i,r in enumerate(self.rows):
                if r['row'] in ['record','field','signature','parameter','function','binding','block','expression','statement']:
                    need(pos('index','completed') < i < pos('resolve','completed') < pos('type','attempted'),self.id+'.resolved-inventory-order','resolved row outside completed index/active resolution interval')
                if r['row'].startswith('typed_'):
                    need(pos('type','attempted') < i < pos('type','completed'), self.id, 'typed inventory outside successful type phase')
        return typed, not resolve_error

    def diagnostics(self):
        wanted = self.a.diagnostics.get(self.id)
        equal(len(self.g['diagnostic']), 1 if wanted else 0, self.id + '.exact-diagnostic-list')
        if not wanted: return
        self.diagnostic(self.g['diagnostic'][0], wanted, self.id + '.diagnostic')

    def diagnostic(self, actual, wanted, where):
        keys(actual, ['row','human','json_line','diagnostic'], where=where)
        rendered = wanted['rendered']
        equal(actual['human'], rendered['human'], where + '.human-bytes')
        equal(actual['json_line'], rendered['json_line'], where + '.json-bytes')
        equal(actual['diagnostic'], loads(actual['json_line']), where + '.json-object')
        equal(actual['diagnostic'], rendered['json_object'], where + '.ordered-diagnostic')
        for s in [actual['diagnostic']['primary']] + [x['span'] for x in actual['diagnostic']['secondary']]:
            equal(s, self.loc(s['file_id'], s['start'], s['end']), where + '.source-coordinates')

    def selectors(self):
        inventory = self.one('ast_inventory')
        keys(inventory, ['row','ast_expression_count','E_ast','target_wrappers','owned','modules'], where=self.id + '.ast')
        for k in ['ast_expression_count','E_ast','target_wrappers','modules']: uint(inventory[k], self.id + '.ast.' + k)
        equal(inventory['modules'], len(self.sources), self.id + '.modules')
        ss = self.g['selector']
        equal(len(ss), len(self.sources), self.id + '.selector-count')
        for i,s in enumerate(ss):
            keys(s, ['row','module','file','owned'], where=self.id + '.selector')
            equal(s['module'], i, self.id + '.module-order'); equal(s['file'], i, self.id + '.module-file')
            need(type(s['owned']) is bool, self.id, 'boolean selector required')
        equal(inventory['owned'], any(s['owned'] for s in ss), self.id + '.whole-project-selector')
        expected = self.a.reused.get(self.id, {})
        if 'selectors' in expected:
            equal([s['owned'] for s in ss], expected['selectors']['ast'], self.id + '.source-selector')
            equal(inventory['owned'], expected['selectors']['source_owner'], self.id + '.source-owner-selector')
        if self.id in self.a.fences:
            equal(inventory['owned'], self.a.fences[self.id]['production_route']=='owned', self.id + '.production-route')

    def indexed(self, rows, count, key, where):
        uint(count,where+'.vector-length')
        equal([r[key] for r in rows], list(range(count)), where + '.complete-id-order')
        return {r[key]:r for r in rows}

    def graph(self, typed, resolved):
        self.records = self.indexed(self.g['record'],len(self.g['record']),'id', self.id + '.records')
        for r in self.g['record']:
            keys(r, ['row','id','declaration','origin','end','fields'], where=self.id + '.record')
            for k in ['declaration','origin','end']: self.origin(r[k], self.id + '.record.' + k)
            fs = [f for f in self.g['field'] if f['record']==r['id']]
            self.indexed(fs, r['fields'], 'id', self.id + '.fields')
        for f in self.g['field']:
            keys(f, ['row','record','id','declaration','origin','scalar'], where=self.id + '.field')
            need(f['record'] in self.records, self.id, 'orphan field')
            self.origin(f['declaration'], self.id + '.field.name'); self.origin(f['origin'], self.id + '.field.origin')
            need(f['scalar'] in ['bool','i32','unit'], self.id, 'unknown field scalar')
        self.functions = self.indexed(self.g['function'],len(self.g['function']),'id', self.id + '.functions')
        self.signatures = self.indexed(self.g['signature'],len(self.functions),'function', self.id + '.signatures')
        if resolved:
            # These exact fixtures have no strings or comments containing declaration tokens.
            # Byte matching is solely an independent roster/origin check, never an Oxid parser.
            for keyword,actual,key in [(b'fn',self.g['signature'],'declaration'),(b'struct',self.g['record'],'declaration')]:
                wanted=[]
                for file,raw in enumerate(self.sources):
                    wanted.extend([file,m.start(1),m.end(1)] for m in re.finditer(rb'\b'+keyword+rb'\s+([A-Za-z_][A-Za-z_0-9]*)',raw))
                equal([r[key] for r in actual],wanted,self.id+'.source-declaration-roster-'+keyword.decode())
        for s in self.g['signature']:
            keys(s, ['row','function','module','local','declaration','origin','result','parameters'], where=self.id + '.signature')
            self.origin(s['declaration'], self.id + '.declaration'); self.origin(s['origin'], self.id + '.signature.origin')
            equal(s['origin'], s['declaration'], self.id + '.signature-name-origin')
            equal(s['module'], s['declaration'][0], self.id + '.module-major')
            self.ty(s['result'], where=self.id + '.signature.result')
            ps = [r for r in self.g['parameter'] if r['function']==s['function']]
            self.indexed(ps,s['parameters'],'position',self.id+'.parameters')
            for p in ps:
                keys(p,['row','function','position','type'],where=self.id+'.parameter'); self.ty(p['type'],True,self.id+'.parameter.type')
        equal([(s['module'],s['local']) for s in self.g['signature']], sorted((s['module'],s['local']) for s in self.g['signature']), self.id+'.declaration-order')
        for module in range(len(self.sources)):
            local=[s['local'] for s in self.g['signature'] if s['module']==module]
            equal(local,list(range(len(local))),self.id+'.complete-module-local-declaration-ids')
        for category in ['parameter','binding','block','expression','statement','typed_expression','typed_binding','typed_block','typed_statement']:
            for r in self.g[category]: need(r['function'] in self.functions, self.id+'.'+category, 'orphan function')
        for fid,f in self.functions.items():
            keys(f,['row','id','body','end','bindings','expressions','blocks'],where=self.id+'.function')
            self.origin(f['end'],self.id+'.function.end')
            by = {k:[r for r in self.g[k] if r.get('function')==fid] for k in ['binding','block','expression','statement','typed_expression','typed_binding','typed_block','typed_statement','parameter']}
            bs = self.indexed(by['binding'], f['bindings'], 'id', self.id+'.bindings')
            bl = self.indexed(by['block'], f['blocks'], 'id', self.id+'.blocks')
            es = self.indexed(by['expression'], f['expressions'], 'id', self.id+'.expressions')
            need(f['body'] in bl, self.id, 'missing function body')
            body_origin=bl[f['body']]['origin'];module=self.signatures[fid]['module']
            equal(body_origin[0],module,self.id+'.body-module')
            for b in bs.values():
                keys(b,['row','function','id','origin','annotation','mutable','scope','parameter_position'],where=self.id+'.binding')
                self.origin(b['origin'],self.id+'.binding.origin')
                equal(b['origin'][0],module,self.id+'.binding-module')
                need(self.signatures[fid]['declaration'][2] <= b['origin'][1] <= b['origin'][2] <= f['end'][2],self.id+'.binding-origin','outside function')
                need(b['scope'] in bl,self.id,'unknown binding scope')
                need(type(b['mutable']) is bool,self.id,'binding mutability')
                if b['annotation'] is not None:self.ty(b['annotation'],where=self.id+'.binding.annotation')
                if b['parameter_position'] is not None:
                    need(0 <= uint(b['parameter_position'],self.id) < len(by['parameter']), self.id,'parameter position')
            equal(sorted(b['parameter_position'] for b in bs.values() if b['parameter_position'] is not None), list(range(len(by['parameter']))), self.id+'.parameter-bindings')
            for b in bl.values():
                keys(b,['row','function','id','origin','end','statements'],where=self.id+'.block')
                self.origin(b['origin'],self.id+'.block.origin');self.origin(b['end'],self.id+'.block.end')
                self.indexed([r for r in by['statement'] if r['block']==b['id']], b['statements'],'position',self.id+'.statements')
            used = collections.Counter()
            for e in es.values():
                equal(e['origin'][0],module,self.id+'.expression-module')
                need(body_origin[1] <= e['origin'][1] <= e['origin'][2] <= body_origin[2],self.id+'.expression-origin','outside function body')
                children = self.expression(e,es,bs)
                for child in children:
                    need(child in es and child < e['id'],self.id+'.expression.edges','missing/nonpostorder child')
                    used[child]+=1
            for s in by['statement']:
                need(s['block'] in bl,self.id,'orphan statement')
                block_origin=bl[s['block']]['origin']
                equal(s['origin'][0],block_origin[0],self.id+'.statement-module')
                need(block_origin[1] <= s['origin'][1] <= s['origin'][2] <= block_origin[2],self.id+'.statement-origin','outside block')
                self.statement(s,es,bs,bl)
                used.update(s['roots'])
            equal(set(used),set(es),self.id+'.retained-expression-reachability')
            need(all(n==1 for n in used.values()),self.id,'expression occurs in multiple parents/roots')
            if typed:
                te=self.indexed(by['typed_expression'],len(es),'id',self.id+'.all-expression-slots')
                tb=self.indexed(by['typed_binding'],len(bs),'id',self.id+'.all-binding-slots')
                tf=self.indexed(by['typed_block'],len(bl),'id',self.id+'.all-block-slots')
                equal([(r['block'],r['position']) for r in by['typed_statement']],[(r['block'],r['position']) for r in by['statement']],self.id+'.all-statement-slots')
                for r in te.values():
                    keys(r,['row','function','id','type','projection'],where=self.id+'.typed-expression')
                    self.ty(r['type'],where=self.id+'.expression.type');self.projection(r['projection'],bs,tb)
                    equal(r['projection'] is not None,es[r['id']]['kind']=='FieldRead',self.id+'.expression-projection-kind')
                for r in tb.values():
                    keys(r,['row','function','id','type'],where=self.id+'.typed-binding');self.ty(r['type'],True,self.id+'.binding.type')
                for r in tf.values():
                    keys(r,['row','function','id','flow'],where=self.id+'.typed-block')
                    need(isinstance(r['flow'],str) and bool(re.fullmatch(r'FlowSummary \{ fallthrough: (true|false), returns: (true|false), breaks: (true|false), continues: (true|false) \}',r['flow'])),self.id,'unknown block flow')
                for r in by['typed_statement']:
                    keys(r,['row','function','block','position','projection'],where=self.id+'.typed-statement');self.projection(r['projection'],bs,tb)
                    statement=next(s for s in by['statement'] if (s['block'],s['position'])==(r['block'],r['position']))
                    equal(r['projection'] is not None,statement['kind']=='FieldAssign',self.id+'.statement-projection-kind')
                for b in bs.values():
                    if b['parameter_position'] is not None:
                        equal(tb[b['id']]['type'],by['parameter'][b['parameter_position']]['type'],self.id+'.parameter-slot-type')
                for e in es.values():
                    t=te[e['id']]['type'];kind=e['kind']
                    if kind in ['Bool','I32','Unit']:equal(t,dict(tag='scalar',scalar={'Bool':'bool','I32':'i32','Unit':'unit'}[kind]),self.id+'.scalar-constant-type')
                    elif kind=='Group':equal(t,te[e['child']]['type'],self.id+'.group-type-slot')
                    elif kind=='Call':equal(t,self.signatures[e['target']]['result'],self.id+'.callee-result-type')
                    elif kind=='ArrayLength':equal(t,dict(tag='scalar',scalar='i32'),self.id+'.length-type')
                    elif kind=='ArrayLiteral':
                        equal(t['tag'],'fixed_array',self.id+'.literal-type');equal(t['length'],len(e['elements']),self.id+'.literal-length')
                        for child in e['elements']:equal(te[child]['type'],dict(tag='scalar',scalar=t['element']),self.id+'.literal-element-type')
                        if not e['elements']:
                            root=e['id']
                            while True:
                                parents=[x['id'] for x in es.values() if x['kind']=='Group' and x['child']==root]
                                if not parents:break
                                equal(len(parents),1,self.id+'.empty-group-parent');root=parents[0]
                            lets=[s for s in by['statement'] if s['kind']=='Let' and s['init']==root]
                            equal(len(lets),1,self.id+'.empty-local-initializer-context')
                            equal(bs[lets[0]['binding']]['annotation'],t,self.id+'.empty-explicit-annotation')
                    if kind in ['IndexRead','ArrayLength']:
                        bt=tb[e['base']]['type'];base=bt['value'] if bt['tag']=='value' else bt['aggregate']
                        equal(base['tag'],'fixed_array',self.id+'.access-base-kind')
                        if kind=='IndexRead':equal(t,dict(tag='scalar',scalar=base['element']),self.id+'.read-element-type')
                for s in by['statement']:
                    if s['kind']=='Let':
                        t=te[s['init']]['type'];equal(tb[s['binding']]['type'],dict(tag='value',value=t),self.id+'.local-initializer-binding-type')
                        if bs[s['binding']]['annotation'] is not None:equal(bs[s['binding']]['annotation'],t,self.id+'.local-annotation-type')
                    elif s['kind']=='Return':
                        t=dict(tag='scalar',scalar='unit') if s['value'] is None else te[s['value']]['type']
                        equal(t,self.signatures[fid]['result'],self.id+'.return-result-type')
            self.fns[fid] = dict(bindings=bs, blocks=bl, expressions=es, statements=by['statement'], types={r['id']:r['type'] for r in by['typed_expression']}, binding_types={r['id']:r['type'] for r in by['typed_binding']}, parameters=by['parameter'])
        summary=self.one('summary')
        if resolved:
            keys(summary,['row','entry','hir_expression_count','E_hir','ast_expression_count','E_ast','target_wrappers','work','allocator_attempts','trace_rows','trace_row_bytes','work_observation_rows','resolved_executable','typed_executable','functions','signatures','records'],where=self.id+'.summary')
            equal(summary['resolved_executable'],False,self.id+'.private-resolved-admission')
            equal(summary['typed_executable'],False if typed else None,self.id+'.private-typed-admission')
            for name in ['functions','signatures','records']:
                equal(summary[name],len(self.g[{'functions':'function','signatures':'signature','records':'record'}[name]]),self.id+'.actual-vector-'+name)
            ast=self.one('ast_inventory')
            for k in ['ast_expression_count','E_ast','target_wrappers']:equal(summary[k],ast[k],self.id+'.inventory.'+k)
            equal(summary['hir_expression_count'],len(self.g['expression']),self.id+'.full-hir-count')
            equal(summary['E_hir'],sum(len(r['elements']) for r in self.g['expression'] if r['kind']=='ArrayLiteral'),self.id+'.independent-hir-elements')
            equal(summary['E_hir'],summary['E_ast'],self.id+'.ast-hir-edges')
            equal(summary['hir_expression_count'],summary['ast_expression_count']-summary['target_wrappers'],self.id+'.ast-hir-expressions')
            equal(summary['target_wrappers'],sum(r['kind']=='IndexAssign' for r in self.g['statement']),self.id+'.ast-only-target-count')
            equal(summary['work_observation_rows'],0,self.id+'.broad-work-log-disabled')
            for k in ['trace_rows','trace_row_bytes']:uint(summary[k],self.id+'.'+k)
            need(summary['trace_row_bytes']>0,self.id+'.trace-layout','zero row width')
            equal(summary['trace_rows'],summary['allocator_attempts'],self.id+'.complete-allocator-trace')
            need(summary['trace_rows']<=200000,self.id+'.trace-bound','trace rows exceeded')
            footer=self.one('complete')
            source_lower=sum(len(raw)+len(f['path'].encode('utf-8'))+(raw.count(b'\n')+1) for raw,f in zip(self.sources,self.a.by_id[self.id]['files']))
            lower=footer['descriptor_overlap_bound_bytes']+200000*summary['trace_row_bytes']+source_lower
            need(footer['auxiliary_requested_bytes']>=lower,self.id+'.retained-auxiliary-bound','below admitted descriptor + reserved trace + source copy lower bound')
            if self.id=='reserve-across-modules':
                equal(summary['work'],350,self.id+'.source-ledger-nonresetting-project-work')
            need(summary['entry'] is None or summary['entry'] in self.functions,self.id,'unknown entry')
            if summary['entry'] is not None:
                s=self.signatures[summary['entry']]
                equal(s['module'],0,self.id+'.original-root-entry-file')
                equal(self.origin(s['declaration'],self.id),'main',self.id+'.original-root-entry-name')
        else:
            keys(summary,['row','work','work_phase','allocator_attempts'],where=self.id+'.failed-summary')
        for k in ['work','allocator_attempts']:uint(summary[k],self.id+'.summary.'+k)

    def expression(self,e,es,bs):
        common=['row','function','id','origin','kind']; k=e['kind']; children=[]
        fields={'Bool':['value'],'I32':['value'],'Unit':[],'Binding':['binding'],'Group':['child'],'ArrayLiteral':['elements'],
                'IndexRead':['base','base_origin','index'],'ArrayLength':['base','base_origin'],'FieldRead':['base','base_origin','field_origin'],
                'Not':['operand','operator_origin'],'Logical':['operator','left','right','operator_origin'],'Comparison':['operator','left','right','operator_origin'],
                'Arithmetic':['operator','left','right','operator_origin'],'Call':['target','declaration','arguments'],'StructLiteral':['record','fields']}
        need(k in fields,self.id+'.expression','unknown kind');keys(e,common+fields[k],where=self.id+'.expression')
        text=self.origin(e['origin'],self.id+'.expression.origin')
        for n in ['base_origin','field_origin','operator_origin','declaration']:
            if n in e:self.origin(e[n],self.id+'.expression.'+n)
        for n in ['base','binding']:
            if n in e:need(e[n] in bs,self.id,'unknown BindingId')
        if k=='Bool':equal(e['value'],text=='true',self.id+'.bool-value');need(text in ['true','false'],self.id,'boolean token')
        if k=='I32':need(type(e['value']) is int and -2**31<=e['value']<2**31,self.id,'i32 constant');equal(e['value'],int(text),self.id+'.i32-value')
        if k=='Unit':equal(text,'()',self.id+'.unit-value')
        if k=='Binding':equal(text,self.origin(bs[e['binding']]['origin'],self.id),self.id+'.binding-use-name')
        if 'base' in e:equal(self.origin(e['base_origin'],self.id),self.origin(bs[e['base']]['origin'],self.id),self.id+'.base-use-name')
        if k=='Group':children=[e['child']]
        elif k=='ArrayLiteral':children=e['elements'];need(isinstance(children,list) and len(children)<=1024,self.id,'literal edge cap')
        elif k=='IndexRead':children=[e['index']]
        elif k=='Not':children=[e['operand']]
        elif k in ['Arithmetic','Logical','Comparison']:
            children=[e['left'],e['right']]
            ops={'Arithmetic':['Add','Sub','Mul','Div','Rem'],'Logical':['And','Or'],'Comparison':['Eq','Ne','Lt','Le','Gt','Ge']}
            need(e['operator'] in ops[k],self.id,'unknown operator')
        elif k=='Call':
            need(e['target'] in self.signatures,self.id,'unknown callee DefId')
            equal(e['declaration'],self.signatures[e['target']]['declaration'],self.id+'.actual-callee-declaration')
            equal(len(e['arguments']),self.signatures[e['target']]['parameters'],self.id+'.call-arity')
            for arg in e['arguments']:
                if arg.get('kind')=='value':keys(arg,['kind','expression'],where=self.id+'.argument');children.append(arg['expression'])
                else:
                    keys(arg,['kind','permission','binding','forwarded','origin','name_origin','star_origin'],where=self.id+'.borrow')
                    equal(arg['kind'],'borrow',self.id);need(arg['permission'] in ['shared','exclusive'],self.id,'borrow mode');need(arg['binding'] in bs,self.id,'borrow binding')
                    need(type(arg['forwarded']) is bool,self.id,'forwarded flag')
                    for n in ['origin','name_origin','star_origin']:
                        if arg[n] is not None:self.origin(arg[n],self.id+'.borrow.'+n)
        elif k=='StructLiteral':
            need(e['record'] in self.records,self.id,'unknown struct record')
            seen=[]
            for f in e['fields']:
                keys(f,['record','field','value','origin'],where=self.id+'.field-init')
                equal(f['record'],e['record'],self.id+'.field-record');need(0<=f['field']<self.records[e['record']]['fields'],self.id,'unknown field')
                self.origin(f['origin'],self.id+'.field-init.origin');seen.append(f['field']);children.append(f['value'])
            equal(sorted(seen),list(range(self.records[e['record']]['fields'])),self.id+'.all-field-inits')
        return children

    def statement(self,s,es,bs,bl):
        common=['row','function','block','position','origin','kind','roots'];k=s['kind']
        fields={'Let':['binding','init'],'Assign':['binding','target_origin','operator_origin','value'],
                'IndexAssign':['base','base_origin','target_origin','operator_origin','value','index'],
                'FieldAssign':['base','base_origin','field_origin','target_origin','operator_origin','value'],
                'Expr':['value'],'Return':['value'],'Break':['target'],'Continue':['target'],'While':['loop_id','condition','body'],'If':['condition','then','else']}
        need(k in fields,self.id+'.statement','unknown kind');keys(s,common+fields[k],where=self.id+'.statement')
        self.origin(s['origin'],self.id+'.statement.origin')
        for n in ['base_origin','target_origin','operator_origin','field_origin']:
            if n in s:self.origin(s[n],self.id+'.statement.'+n)
        for n in ['binding','base']:
            if n in s:need(s[n] in bs,self.id,'statement binding')
        roots=[]
        if k=='Let':roots=[s['init']]
        elif k in ['Assign','FieldAssign','Expr']:roots=[s['value']]
        elif k=='Return':roots=[] if s['value'] is None else [s['value']]
        elif k=='IndexAssign':
            roots=[s['value'],s['index']]
            need(s['value']<s['index'],self.id+'.rhs-before-index','root IDs reversed')
            need(not any(e['origin']==s['target_origin'] for e in es.values()),self.id+'.no-target-wrapper','ghost HIR target read')
        elif k in ['If','While']:
            roots=[s['condition']]
            for n in ['body','then','else']:
                if n in s and s[n] is not None:need(s[n] in bl,self.id,'unknown child block')
        equal(s['roots'],roots,self.id+'.statement-root-order')
        need(all(r in es for r in roots),self.id,'missing statement expression')

    def projection(self,p,bs,typed_bindings):
        if p is None:return
        keys(p,['binding','mode','record','field'],where=self.id+'.projection')
        for key in ['binding','record','field']:uint(p[key],self.id+'.projection.'+key)
        need(p['binding'] in bs and p['record'] in self.records,self.id,'projection identity')
        need(p['mode'] in ['owner','shared','exclusive'],self.id,'projection mode')
        need(0<=p['field']<self.records[p['record']]['fields'],self.id,'projection field')
        ty=typed_bindings[p['binding']]['type']
        equal(p['mode'],'owner' if ty['tag']=='value' else ty['kind'],self.id+'.projection-reference-mode')
        aggregate=ty['value'] if ty['tag']=='value' else ty['aggregate']
        equal(aggregate['tag'],'record',self.id+'.projection-record-type')
        equal(aggregate['record_id'],p['record'],self.id+'.projection-nominal-record-id')

    def reservations(self):
        cumulative=0;attempt=0
        for r in self.g['reservation']:
            keys(r,['row','attempt','kind','length','element_bytes','success','cumulative_hir_entries'],where=self.id+'.reservation')
            need(uint(r['attempt'],self.id)>attempt,self.id,'reservation order');attempt=r['attempt']
            need(r['kind'] in ['array HIR elements','array literal elements'],self.id,'unexpected reservation kind')
            need(uint(r['element_bytes'],self.id)>0,self.id,'unmeasured element width');need(uint(r['length'],self.id)<=1024,self.id,'literal N')
            equal(r['success'],True,self.id+'.semantic-run-reservation-success')
            if r['kind']=='array HIR elements':cumulative+=r['length']
            equal(r['cumulative_hir_entries'],cumulative,self.id+'.nonresetting-cumulative-entries')
        need(attempt<=self.one('summary')['allocator_attempts'],self.id,'allocator attempt outside receipt')
        if any(r['phase']=='resolve' and r['status']=='completed' for r in self.g['phase']):
            # Each retained literal has exactly one pre-child construction request,
            # including zero and programs that subsequently fail type checking.
            literals=self.literal_preorder()
            actual=[r for r in self.g['reservation'] if r['kind']=='array HIR elements']
            equal([r['length'] for r in actual],[r['N'] for r in literals],self.id+'.complete-literal-request-preorder')
            equal(sum(r['length'] for r in actual),self.one('summary')['E_hir'],self.id+'.all-requested-hir-elements')
        expected=self.a.resources.get(self.id)
        if expected:
            actual=[r for r in self.g['reservation'] if r['kind']=='array HIR elements']
            equal(len(actual),len(expected['requests']),self.id+'.exact-request-count')
            for r,e in zip(actual,expected['requests']):
                equal(r['length'],e['N'],self.id+'.exact-N');equal(r['cumulative_hir_entries'],e['cumulative_entries'],self.id+'.exact-cumulative')

    def literal_preorder(self):
        result=[]
        for fid,fn in self.fns.items():
            seen_blocks=set();seen_exprs=set()
            def expr(eid):
                need(eid not in seen_exprs,self.id+'.literal-preorder','duplicate expression visit');seen_exprs.add(eid)
                e=fn['expressions'][eid];kind=e['kind']
                if kind=='ArrayLiteral':
                    result.append(dict(N=len(e['elements']),file=e['origin'][0],function=fid));children=e['elements']
                elif kind=='Group':children=[e['child']]
                elif kind=='IndexRead':children=[e['index']]
                elif kind=='Not':children=[e['operand']]
                elif kind in ['Arithmetic','Logical','Comparison']:children=[e['left'],e['right']]
                elif kind=='Call':children=[a['expression'] for a in e['arguments'] if a['kind']=='value']
                elif kind=='StructLiteral':children=[a['value'] for a in e['fields']]
                else:children=[]
                for child in children:expr(child)
            def block(bid):
                need(bid not in seen_blocks,self.id+'.literal-preorder','cycle/duplicate block visit');seen_blocks.add(bid)
                for stmt in [s for s in fn['statements'] if s['block']==bid]:
                    for root in stmt['roots']:expr(root)
                    if stmt['kind']=='While':block(stmt['body'])
                    elif stmt['kind']=='If':
                        block(stmt['then'])
                        if stmt['else'] is not None:block(stmt['else'])
            block(self.functions[fid]['body'])
            equal(seen_blocks,set(fn['blocks']),self.id+'.literal-preorder-full-block-coverage')
            equal(seen_exprs,set(fn['expressions']),self.id+'.literal-preorder-full-expression-coverage')
        return result

    def expected_expressions(self,fid,expected):
        fn=self.fns[fid]
        equal(len(fn['expressions']),len(expected),self.id+'.source-expression-count')
        for e in expected:
            d=normalize_expected(e); ty=d.pop('type');eid=d['id']
            if 'op' in d:d['operator']=d.pop('op')
            exact_subset(fn['expressions'][eid],d,self.id+f'.expression[{fid},{eid}]')
            exact_subset(fn['types'][eid],ty,self.id+f'.type[{fid},{eid}]')

    def expected_store(self,fid,wanted):
        stores=[s for s in self.fns[fid]['statements'] if s['kind']=='IndexAssign']
        equal(len(stores),1,self.id+'.store-count')
        d=normalize_expected(wanted.copy());d.pop('no_expression_at_target_origin',None)
        wrappers=d.pop('AST_target_wrapper_count',None)
        if wrappers is not None:equal(self.one('summary')['target_wrappers'],wrappers,self.id+'.target-wrappers')
        d['roots']=d.pop('root_order')
        exact_subset(stores[0],d,self.id+'.expected-store')

    def semantic(self):
        p=self.a.positive.get(self.id)
        summary=self.one('summary')
        reused=self.a.reused.get(self.id,{})
        if self.id in ['child-route-parameter','child-route-shared-parameter','child-route-exclusive-parameter']:
            t=normalize_expected(reused['type_queries'][0]['value'])
            if t['tag']!='reference':t=dict(tag='value',value=t)
            equal(self.fns[1]['parameters'][0]['type'],t,self.id+'.frozen-child-parameter-type')
        if self.id=='record-len-field-stays-field':
            equal(sum(r['kind'] in ['ArrayLiteral','ArrayLength','IndexRead'] for r in self.g['expression']),reused['array_node_count'],self.id+'.array-free-record-compatibility')
            expected=reused['ast'][0]
            rows=[r for r in self.g['expression'] if r['origin']==span(expected['origin'])]
            equal(len(rows),1,self.id+'.record-len-origin');equal(rows[0]['kind'],expected['kind'],self.id+'.record-len-stays-field')
        if not p:return
        for k in ['E_ast','E_hir','ast_expression_count','hir_expression_count','entry']:
            if k in p:equal(summary[k],p[k],self.id+'.expected.'+k)
        if 'expressions' in p:self.expected_expressions(p['function'],p['expressions'])
        if 'expressions_compact' in p:
            es=[]
            for e in p['expressions_compact']:
                if 'ids_inclusive' in e:
                    lo,hi=e['ids_inclusive'];equal(len(e['ordered_origins']),hi-lo+1,self.id+'.authority.compact')
                    es.extend(dict(id=i,kind=e['kind'],origin=o,type=e['type'],value=e['value']) for i,o in zip(range(lo,hi+1),e['ordered_origins']))
                else:
                    d=e.copy()
                    if 'elements_inclusive' in d:lo,hi=d.pop('elements_inclusive');d['elements']=list(range(lo,hi+1))
                    es.append(d)
            self.expected_expressions(p['function'],es)
        if 'binding' in p:
            f=self.fns[p['function']];b=p['binding']
            exact_subset(f['binding_types'][b],normalize_expected(p['binding_type']),self.id+'.expected-binding-type')
            if 'binding_origin' in p:equal(f['bindings'][b]['origin'],span(p['binding_origin']),self.id+'.expected-binding-origin')
        if 'store' in p:self.expected_store(p['function'],p['store'])
        req=[r for r in self.g['reservation'] if r['kind']=='array HIR elements']
        expected_req=p['literal_requests']
        equal([r['length'] for r in req],[r['N'] if isinstance(r,dict) else r for r in expected_req],self.id+'.literal-request-sequence')
        for r,e in zip(req,expected_req):
            if isinstance(e,dict):equal(r['cumulative_hir_entries'],e['cumulative_entries'],self.id+'.requested-cumulative')
        if any(isinstance(e,dict) for e in expected_req):
            equal(self.literal_preorder(),[{k:e[k] for k in ['N','file','function']} for e in expected_req],self.id+'.source-request-owner-sequence')
        names=[self.origin(s['declaration'],self.id) for s in self.g['signature']]
        if 'function_order' in p:equal(names,p['function_order'],self.id+'.function-order')
        if 'record_order' in p:equal([self.origin(r['declaration'],self.id) for r in self.g['record']],p['record_order'],self.id+'.record-order')
        if 'nominal_array_rows' in p:
            # Record rows must name real record declarations; arrays never synthesize one.
            equal(sum(self.origin(r['declaration'],self.id).startswith('[') for r in self.g['record']),p['nominal_array_rows'],self.id+'.nominal-array-rows')
        if 'probe_parameter_types' in p:
            actual=[r['type'] for r in self.fns[0]['parameters']]
            equal([r['position'] for r in self.fns[0]['parameters']],p['probe_parameter_positions'],self.id+'.probe-positions')
            exact_subset(actual,normalize_expected(p['probe_parameter_types']),self.id+'.distinct-parameter-types')
            desc=[t['value'] for t in actual]
            exact_subset(desc,normalize_expected(p['identity']['descriptors']),self.id+'.identity-descriptors')
            equal([[x==y for y in desc] for x in desc],p['identity']['equality_matrix'],self.id+'.identity-matrix')
        if 'functions' in p:
            for f in p['functions']:
                fid=f['id'];sig=self.signatures[fid]
                equal(self.origin(sig['declaration'],self.id),f['name'],self.id+'.function-name')
                if 'file' in f:equal(sig['module'],f['file'],self.id+'.function-file')
                if 'expressions' in f:
                    self.expected_expressions(fid,f['expressions']);self.expected_store(fid,f['store'])
                    wanted=dict(tag='reference',kind=f['parameter_mode'],aggregate=f['parameter_type'])
                    equal(self.fns[fid]['parameters'][f['parameter']]['type'],wanted,self.id+'.reference-parameter-mode')
                    equal(self.fns[fid]['binding_types'][f['access_base']['binding']],wanted,self.id+'.reference-binding-mode')
        if 'main' in p:
            f=p['main'];equal(len(self.fns[f['id']]['expressions']),f['expression_count'],self.id+'.main-expression-count')
            equal(self.signatures[f['id']]['result'],f['result'],self.id+'.main-result')
        if 'expression_kinds_by_function' in p:
            equal([[r['kind'] for r in self.fns[f]['expressions'].values()] for f in self.functions],p['expression_kinds_by_function'],self.id+'.cross-file-kinds')
            equal([[self.fns[f]['types'][i] for i in self.fns[f]['expressions']] for f in self.functions],p['expression_types_by_function'],self.id+'.cross-file-types')
        if 'calls' in p:
            for c in p['calls']:
                e=self.fns[c['function']]['expressions'][c['expression']]
                equal(e['target'],c['target'],self.id+'.callee-id')
                equal(e['arguments'],[dict(kind=a['tag'],expression=a['expression']) for a in c['args']],self.id+'.call-args')
            calls=[self.fns[c['function']]['expressions'][c['expression']] for c in p['calls']]
            for call,link in zip(calls,p['declaration_links']):
                equal(call['declaration'],span(link['declaration']),self.id+'.callee-declaration-origin')
                use=span(link['use']);origin=call['origin'];equal(origin[:2],use[:2],self.id+'.call-use-start')
                equal(self.sources[use[0]][use[1]:use[2]].decode('utf-8'),link['use']['text'],self.id+'.call-use-text')
            uses=p['identical_type_uses']
            actual=[self.fns[0]['bindings'][0]['annotation'],self.signatures[1]['result'],self.fns[2]['parameters'][0]['type']['value']]
            equal(actual,[x['value'] for x in uses],self.id+'.structural-types-across-files')
        if 'main_call' in p:
            c=p['main_call'];e=self.fns[0]['expressions'][c['expression']]
            equal(e['kind'],'Call',self.id);equal(e['target'],c['target'],self.id+'.chosen-not-decoy');equal(e['origin'],span(c['origin']),self.id+'.chosen-call-origin')
            for fid in p['equal_signature_functions']:
                equal(self.signatures[fid]['result'],p['result'],self.id+'.equal-signature-result');equal(self.signatures[fid]['parameters'],0,self.id+'.equal-signature-parameters')

    def compare(self):
        outcome=self.envelope();typed,resolved=self.phases(outcome)
        self.selectors();self.graph(typed,resolved);self.diagnostics();self.reservations();self.semantic()
        return {'case':self.id,'outcome':outcome,'rows':len(self.rows),'bytes':len(self.raw),
                'semantic_units':sum(units(r) for r in self.rows),'sha256':sha(self.raw)}

TEST_NAME='frontend::oir::owned::source::array_typing_observer::array_typing_observe_requests'

def digest_file(path):
    with pathlib.Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def execution_witness(log,command,returncode,binary):
    equal(returncode,0,'run.exit-status')
    equal(command,[str(pathlib.Path(binary).resolve()),'--exact',TEST_NAME,'--ignored','--nocapture','--test-threads=1'],'run.exact-command')
    need(len(re.findall(r'^running 1 test$',log,re.M))==1,'run','missing single-test execution witness')
    need(len(re.findall(r'^test '+re.escape(TEST_NAME)+r' \.\.\. ok$',log,re.M))==1,'run','named ignored test did not execute and pass exactly once')
    matches=re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',log,re.M)
    need(len(matches)==1 and matches[0][:4]==('1','0','0','0'),'run','zero or unexpected test execution count')

def roster_check(authority,requests,outputs,names):
    roster=[x['id'] for x in authority.cases]
    equal([r['id'] for r in requests],roster,'requests.exact-ordered-54-roster')
    equal(set(outputs),set(roster),'run.output-case-roster')
    equal(sorted(names),sorted(x+'.jsonl' for x in roster),'artifacts.exact-roster-no-error-or-extra')

def compare_driver(authority,run_dir,adapter_dir,build_path,observer_review,admission):
    """Admission is reviewer-written, not generated from observed results by this tool."""
    equal(admission['schema'],'oxid-array-typing-comparator-admission-v1','admission.schema')
    equal(admission['status'],'ADMITTED_FOR_CANDIDATE_COMPARISON','admission.status')
    equal(admission['authority_sha256'],AUTHORITY_SHA,'admission.authority')
    equal(admission['comparator_sha256'],digest_file(__file__),'admission.comparator-code')
    equal(admission['project_work_ledger_sha256'],PROJECT_LEDGER_SHA,'admission.work-ledger')
    adapter=pathlib.Path(adapter_dir);root=pathlib.Path(run_dir)
    freeze_path=adapter/'freeze-manifest.json'
    equal(digest_file(freeze_path),admission['adapter_manifest_sha256'],'admission.adapter-freeze')
    freeze=read_json(freeze_path)
    for name,want in freeze['files'].items():
        path=adapter/name
        need(path.is_file() and not path.is_symlink(),'adapter','missing/nonordinary frozen file '+name)
        equal(path.stat().st_size,want['bytes'],'adapter.'+name+'.bytes')
        equal(digest_file(path),want['sha256'],'adapter.'+name+'.sha256')
    equal(freeze['authority_sha256'],AUTHORITY_SHA,'adapter.authority')
    equal(digest_file(build_path),admission['build_manifest_sha256'],'admission.build')
    equal(digest_file(observer_review),admission['observer_review_sha256'],'admission.observer-review')
    build=read_json(build_path)
    equal(build['schema'],'oxid-array-typing-build-v1','build.schema')
    equal(build['base'],freeze['base'],'build.selected-base')
    equal(build['base_tree'],freeze['base_tree'],'build.selected-tree')
    identity=build['candidate_identity']
    equal(identity['kind'],'immutable-source-overlay','build.candidate-kind')
    equal(identity['adapter_freeze_sha256'],admission['adapter_manifest_sha256'],'build.adapter-binding')
    equal(identity['core_manifest_sha256'],freeze['core_manifest_sha256'],'build.core-binding')
    equal(identity['archive_sha256'],freeze['files']['candidate-source.tar.gz']['sha256'],'build.source-archive')
    equal(identity['full_source_closure_sha256'],freeze['files']['full-source-closure.json']['sha256'],'build.source-closure')
    equal(build['mapping_sha256'],freeze['files']['WIRE-MAPPING.md']['sha256'],'build.mapping')
    equal(build['observer_sha256'],freeze['files']['observer.rs']['sha256'],'build.observer')
    binary=pathlib.Path(build['binary']['path'])
    equal(binary.stat().st_size,build['binary']['bytes'],'build.binary-size')
    equal(digest_file(binary),build['binary']['sha256'],'build.binary-hash')
    need(build['profile'] in ['debug','release'],'build','unadmitted build profile')
    equal(binary.name,build['profile']+'-test','build.binary-profile-name')
    equal(build['binary']['sha256'],freeze['files'][build['profile']+'-test']['sha256'],'build.frozen-binary')
    for k in ['profile','target','rustc_vV','cargo_vV']:
        need(isinstance(build[k],str) and build[k],'build','missing target/profile/toolchain '+k)
    request_path=root/'request-manifest.json';request=read_json(request_path)
    equal(digest_file(request_path),admission['request_manifest_sha256'],'admission.requests')
    equal(request['schema'],'oxid-array-typing-requests-v1','requests.schema')
    equal(request['authority_sha256'],AUTHORITY_SHA,'requests.authority')
    tsv=(root/'requests.tsv').read_bytes();equal(sha(tsv),request['request_sha256'],'requests.tsv-hash');equal(len(tsv),request['request_bytes'],'requests.tsv-bytes')
    expected_tsv=[];normalized=[]
    for r in request['requests']:
        equal(r['files'],authority.by_id[r['id']]['files'],'requests.fixture-roster')
        equal(r['scope'],authority.by_id[r['id']]['scope'],'requests.scope')
        equal(len(r['limits']),5,'requests.limits')
        need(all(type(v) is int and v>=0 for v in r['limits']),'requests','invalid downward limits')
        for v,cap in zip(r['limits'],[MAX_UNITS,MAX_BYTES,32*1024*1024,16*1024*1024,256000000]):need(v<=cap,'requests','upward resource override')
        source_root=pathlib.Path(r['source_root'])
        equal(sorted(p.relative_to(source_root).as_posix() for p in source_root.rglob('*') if p.is_file()),sorted(f['path'] for f in r['files']),'requests.exact-source-files')
        for f in r['files']:
            path=source_root/f['path'];equal(digest_file(path),f['sha256'],'requests.source-bytes')
        expected_tsv.append('\t'.join([r['id'],r['scope'],r['source_root'],*map(str,r['limits'])]))
        normalized.append(dict(case=r['id'],scope=r['scope'],fixture_root=r['source_root'],**dict(zip(['row_limit','byte_limit','retained_limit','scratch_limit','work_limit'],r['limits']))))
    equal(tsv,('\n'.join(expected_tsv)+'\n').encode(),'requests.exact-tsv')
    # Only now open candidate run metadata and observations, after every prospective binding.
    run=read_json(root/'run-manifest.json')
    equal(run['schema'],'oxid-array-typing-run-v1','run.schema')
    equal(run['base'],build['base'],'run.selected-base')
    equal(run['request_manifest_sha256'],admission['request_manifest_sha256'],'run.requests')
    equal(run['request_sha256'],sha(tsv),'run.tsv')
    equal(run['build_manifest_sha256'],admission['build_manifest_sha256'],'run.build')
    equal(run['admission_sha256'],admission['observer_review_sha256'],'run.observer-admission')
    equal(run['binary_sha256'],build['binary']['sha256'],'run.binary')
    log=(root/'run.log').read_bytes();equal(sha(log),run['run_log_sha256'],'run.log-hash')
    execution_witness(log.decode('utf-8'),run['command'],run['returncode'],binary)
    equal(run['observer_errors'],[],'run.observer-errors')
    observations=root/'observations'
    roster_check(authority,request['requests'],run['outputs'],[p.name for p in observations.iterdir()])
    out=[]
    binding=dict(core_manifest_sha256=freeze['core_manifest_sha256'],base=build['base'])
    for req in normalized:
        item=run['outputs'][req['case']];equal(item['file'],req['case']+'.jsonl','artifact.filename')
        path=observations/item['file'];need(path.is_file() and not path.is_symlink(),'artifact','ordinary file required')
        need(path.stat().st_size<=MAX_BYTES,'artifact','oversized observation')
        raw=path.read_bytes();equal(sha(raw),item['sha256'],'artifact.hash');equal(len(raw),item['bytes'],'artifact.bytes')
        out.append(Case(authority,req['case'],raw,req,binding).compare())
    return {'status':'NORMAL_SOURCE_OBSERVATIONS_PASS','candidate_semantic_qualification':'partial; separate control coverage required',
            'cases':out,'authority_sha256':AUTHORITY_SHA,'deferred_effect_cases':DEFERRED,
            'not_claimed':['ownership','lowering','runtime','native','fuel','pilot','activation','CI','resource-and-fence-control-completion']}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--authority',required=True);p.add_argument('--authority-review',required=True)
    p.add_argument('--run-dir',required=True);p.add_argument('--adapter-dir',required=True);p.add_argument('--build-manifest',required=True)
    p.add_argument('--observer-review',required=True);p.add_argument('--admission',required=True);p.add_argument('--admission-sha256',required=True)
    p.add_argument('--comparator-review',required=True);p.add_argument('--comparator-review-sha256',required=True)
    p.add_argument('--out',required=True)
    args=p.parse_args()
    # The report file is new-only. A failed attempt is never overwritten by a rerun.
    out=pathlib.Path(args.out)
    with out.open('x',encoding='utf-8') as report:
        try:
            a=Authority(args.authority,args.authority_review)
            equal(digest_file(args.admission),args.admission_sha256,'admission.exact-file')
            equal(digest_file(args.comparator_review),args.comparator_review_sha256,'admission.exact-review')
            review=pathlib.Path(args.comparator_review).read_text()
            need(args.admission_sha256 in review and 'PASS' in review,'admission','review does not admit exact binding JSON')
            result=compare_driver(a,args.run_dir,args.adapter_dir,args.build_manifest,args.observer_review,read_json(args.admission))
            code=0
        except (Rejected,KeyError,IndexError,TypeError,ValueError,OSError,AttributeError,RecursionError) as e:
            result={'status':'REJECTED','first_failure':str(e),'candidate_qualification_credit':0};code=1
        json.dump(result,report,ensure_ascii=False,indent=2);report.write('\n')
    print(json.dumps({'status':result['status'],'report':str(out),'first_failure':result.get('first_failure')},ensure_ascii=False))
    return code

if __name__=='__main__':sys.exit(main())
