#!/usr/bin/env python3
"""Independent Unit4B LLVM store/guard audit; never imports compiler code.

Input JSON binds source_path/source_sha256, llvm_path/llvm_sha256, raw_view
(or raw_witness_path/raw_witness_sha256 containing raw_view), entry, stores.
Store rows bind function, llvm_line (one based), store_text, raw_block, and
raw_operation {kind: parameter|merge|statement|terminator, index: int|null}.
Names of SSA values and emitted guard/error blocks carry no audit semantics.
The explicit raw-block to emitted-block correspondence defaults to b<raw id>.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


class AuditFailure(Exception):
    pass


def require(condition, message):
    if not condition:
        raise AuditFailure(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_bound(catalog, key, base):
    path = Path(catalog[key + '_path'])
    if not path.is_absolute():
        path = base / path
    data = path.read_bytes()
    require(catalog[key + '_sha256'] == sha(data), f'{key} hash mismatch: {path}')
    return path, data


SSA = r'%[-A-Za-z$._0-9]+'
LABEL = r'[-A-Za-z$._0-9]+'


class Function:
    def __init__(self, name, header, start):
        self.name, self.header, self.start = name, header, start
        self.blocks, self.block_lines, self.instructions, self.defs = {}, {}, {}, {}
        self.edges = {}

    def finish(self):
        require(self.blocks, f'{self.name}: empty function')
        self.entry = next(iter(self.blocks.values()))[0]
        for block, nodes in self.blocks.items():
            require(nodes, f'{self.name}: empty block {block}')
            for a, b in zip(nodes, nodes[1:]):
                self.edges[a] = [b]
            last = nodes[-1]
            text = self.instructions[last]['text']
            labels = re.findall(r'label %(' + LABEL + ')', text)
            require(text.startswith(('br ', 'ret ', 'unreachable')), f'{self.name}:{last}: unsupported terminator {text}')
            require(all(label in self.blocks for label in labels), f'{self.name}: absent branch label')
            self.edges[last] = [self.blocks[label][0] for label in labels]
        self.reached = self.reachable(self.entry)

    def reachable(self, start, without_node=None, without_edge=None):
        todo, seen = [start], set()
        while todo:
            n = todo.pop()
            if n in seen or n == without_node:
                continue
            seen.add(n)
            todo.extend(v for v in self.edges[n] if (n, v) != without_edge)
        return seen

    def dominated_node(self, node, by):
        return node in self.reached and node not in self.reachable(self.entry, without_node=by)

    def dominated_edge(self, node, edge):
        return node in self.reached and node not in self.reachable(self.entry, without_edge=edge)


def parse_module(text):
    functions, constants, current, block = {}, {}, None, None
    for line_no, full in enumerate(text.splitlines(), 1):
        line = full.strip()
        m = re.fullmatch(r'(@[-A-Za-z$._0-9]+) = .*constant \[(\d+) x i8\] c"(.*)"', line)
        if m:
            encoded = m[3]
            require(re.fullmatch(r'(?:\\[0-9A-Fa-f]{2})*', encoded) is not None, 'non-byte diagnostic constant')
            constants[m[1]] = bytes(int(encoded[i + 1:i + 3], 16) for i in range(0, len(encoded), 3)).decode()
            require(len(constants[m[1]].encode()) == int(m[2]), 'diagnostic constant length differs')
        if line.startswith('define '):
            require(current is None, 'nested definition')
            m = re.search(r'@([-A-Za-z$._0-9]+)\(', line)
            require(m is not None, 'unsupported function definition')
            current = Function(m[1], line, line_no)
            functions[current.name] = current
            block = None
        elif line == '}' and current:
            current.end = line_no
            current.finish()
            current = None
        elif current and line and not line.startswith(';'):
            if re.fullmatch(LABEL + ':', line):
                block = line[:-1]
                require(block not in current.blocks, 'duplicate block')
                current.blocks[block] = []
                current.block_lines[block] = line_no
            else:
                require(block is not None, 'instruction before label')
                clean = line.split(' ;', 1)[0]
                current.blocks[block].append(line_no)
                current.instructions[line_no] = dict(text=clean, block=block)
                m = re.match('(' + SSA + r') = (.*)', clean)
                if m:
                    require(m[1] not in current.defs, 'duplicate SSA definition')
                    current.defs[m[1]] = (line_no, m[2])
    require(current is None, 'unterminated function')
    return functions, constants


def destination(text):
    m = re.fullmatch(r'store (i\d+|ptr) (.+), ptr (' + SSA + r')(?:, align \d+)?', text)
    require(m is not None, f'unsupported store: {text}')
    return m[1], m[2], m[3]


def guard_failure(f, first, constants, code, source, span):
    seen = f.reachable(first)
    require(len(seen) == 2, f'{f.name}:{first}: error path is not exactly failure call then unreachable')
    nodes = sorted(seen)
    require(f.instructions[nodes[-1]]['text'] == 'unreachable', f'{f.name}: returning error path')
    call = f.instructions[nodes[0]]['text']
    m = re.fullmatch(r'call void @__oxid_overflow\(ptr (@[-A-Za-z$._0-9]+), i64 (\d+)\)', call)
    require(m is not None and m[1] in constants, f'{f.name}: unsupported failure sink')
    message = constants[m[1]]
    require(len(message.encode()) == int(m[2]), 'failure length mismatch')
    require(f'error[{code}]' in message, f'{f.name}: wrong failure code')
    if span is not None:
        start, end = span
        require(0 <= start <= end <= len(source), 'span outside source bytes')
        line = source[:start].count(b'\n') + 1
        col = len(source[:start].split(b'\n')[-1].decode('utf-8')) + 1
        require(re.search(r':' + str(line) + ':' + str(col) + r'\n$', message) is not None,
                f'{f.name}:{first}: failure origin differs: want {line}:{col}, got {message!r}')
    return seen


def discover_guards(f, constants):
    guards = []
    for node, inst in f.instructions.items():
        m = re.fullmatch(r'br i1 (' + SSA + '), label %(' + LABEL + '), label %(' + LABEL + ')', inst['text'])
        if not m:
            continue
        cond = f.defs.get(m[1])
        if cond is None:
            continue
        fuel = re.fullmatch(r'icmp ult i64 (' + SSA + r'), (\d+)', cond[1])
        overflow = re.fullmatch(r'extractvalue \{ i32, i1 \} (' + SSA + r'), 1', cond[1])
        if fuel:
            remaining = f.defs.get(fuel[1])
            require(remaining is not None, 'fuel check missing load')
            load = re.fullmatch(r'load i64, ptr (' + SSA + r')(?:, align \d+)?', remaining[1])
            require(load is not None, 'fuel check does not use loaded shared fuel')
            success, error = f.blocks[m[3]][0], f.blocks[m[2]][0]
            debit_candidates = []
            for n in f.blocks[m[3]]:
                it = f.instructions[n]['text']
                if not it.startswith('store '):
                    continue
                typ, value, ptr = destination(it)
                if ptr != load[1]:
                    continue
                definition = f.defs.get(value)
                require(typ == 'i64' and definition and definition[1] == f'sub i64 {fuel[1]}, {fuel[2]}',
                        f'{f.name}:{n}: incorrect fuel subtraction')
                debit_candidates.append(n)
            require(len(debit_candidates) == 1, f'{f.name}:{node}: success must debit fuel exactly once')
            debit = debit_candidates[0]
            require(f.dominated_edge(debit, (node, success)), f'{f.name}:{node}: debit before admitted success')
            guards.append(dict(kind='fuel', node=node, success=success, error=error,
                               debit=debit, pointer=load[1], cost=int(fuel[2]), loaded=remaining[0]))
        elif overflow:
            intrinsic = f.defs.get(overflow[1])
            require(intrinsic is not None and re.fullmatch(r'call \{ i32, i1 \} @llvm\.s(?:add|sub|mul)\.with\.overflow\.i32\(i32 .*, i32 .*\)', intrinsic[1]),
                    'overflow flag is not from checked intrinsic')
            guards.append(dict(kind='overflow', node=node, success=f.blocks[m[3]][0], error=f.blocks[m[2]][0],
                               intrinsic=intrinsic[0], aggregate=overflow[1]))
    return guards


class RawModel:
    def __init__(self, raw):
        self.raw = raw
        self.functions = {f['id']: f for f in raw['functions']}
        self.records = {r['id']: r for r in raw['records']}
        self.usage = {}
        self.operations = {}
        for fid, f in self.functions.items():
            P = sum(self.width(o['record']) for o in f['owners'])
            S = len(f['locals']) + len(f['places'])
            A = sum(len(c['arguments']) for c in f['calls'])
            self.usage[fid] = dict(S=S, A=A, P=P, O=len(f['owners']), R=len(f['references']), L=len(f['loans']), C=len(f['calls']))
            u = self.usage[fid]
            u['X'] = S + A + P + 4*u['O'] + 8*u['R'] + 12*u['L'] + 2*u['C']
        for fid, f in self.functions.items():
            for p in f['parameters']:
                op = dict(kind='parameter', index=p['position'], raw=p, span=p['span'], cost=None, overflow_span=None)
                op['types'] = (['i64'] if p['mode'] == 'scalar' else ['ptr'] if p['mode'] == 'reference' else self.field_types(f['owners'][p['id']]['record']))
                self.operations[(fid, None, 'parameter', p['position'])] = op
            for b in f['blocks']:
                if b['merge'] is not None:
                    self.operations[(fid,b['id'],'merge',None)] = dict(kind='merge',index=None,raw=b['merge'],span=b['merge']['span'],cost=1,overflow_span=None,types=['i64'])
                for s in b['instructions']:
                    op = self.operation(fid, s['instruction'], s.get('charge_span',s['span']))
                    self.operations[(fid,b['id'],'statement',s['position'])] = op
                t = b['terminator']
                self.operations[(fid,b['id'],'terminator',None)] = self.operation(fid,t['instruction'],t['span'])

    def width(self, record):
        return max(1,len(self.records[record]['fields']))

    def field_types(self, record):
        return [{'i32':'i32','bool':'i1','()':'i8'}[v['type']] for v in self.records[record]['fields']] or ['i8']

    def operation(self, fid, inst, span):
        f, u = self.functions[fid], self.usage[fid]
        kind = inst['operation']
        cost, types, overflow_span, target = 1, [], None, None
        def own(o):
            return f['owners'][o]['record']
        if kind == 'Scalar':
            types = ['i64']
            s = inst['scalar']
            if s['kind'] == 'Assign' and s['value']['kind'] == 'CheckedI32':
                overflow_span = s['value']['operator_span']
        elif kind == 'Construct':
            types = [self.field_types(v['field']['record'])[v['field']['index']] for v in inst['fields']] or ['i8']
            cost += len(types)
        elif kind in ('MoveInitialize','Replace','PrepareOwned'):
            types = self.field_types(own(inst['source']))
            cost += len(types) * (2 if kind == 'Replace' else 1)
        elif kind in ('StorageEnd','Discard'):
            cost += self.width(own(inst['owner']))
        elif kind in ('ReadField','PrepareScalar'):
            types = ['i64']
        elif kind == 'WriteField':
            field = inst['field']
            types = [self.field_types(field['record'])[field['index']]]
        elif kind == 'PrepareBorrow':
            types = ['ptr']
        elif kind == 'OpenCall':
            cost += sum(a['mode'] == 'owned' for a in f['calls'][inst['call']]['arguments'])
        elif kind == 'Invoke':
            c = f['calls'][inst['call']]
            target = c['callee']
            borrow = sum(a['mode'] == 'borrow' for a in c['arguments'])
            widths = sum(self.width(own(a['id'])) for a in c['arguments'] if a['mode'] == 'owned')
            cost += len(c['arguments']) + self.usage[target]['X'] + widths + borrow*(borrow-1)//2
            types = ['i64'] if c['result']['mode'] == 'scalar' else []
        elif kind in ('ReturnScalar','ReturnOwned'):
            cost += u['P'] + u['L'] + u['C'] + u['R']
            if kind == 'ReturnOwned':
                types = self.field_types(own(inst['owner']))
                cost += len(types)
        elif kind not in ('StorageLive','Goto','Branch'):
            raise AuditFailure(f'unknown raw operation {kind}')
        return dict(kind=kind,raw=inst,span=span,cost=cost,types=types,overflow_span=overflow_span,target=target)

    def block_ops(self, fid, bid):
        return [(key,op) for key,op in self.operations.items() if key[0] == fid and key[1] == bid]


def has_cycle(edges):
    pending, done = set(), set()
    def visit(n):
        if n in pending:
            return True
        if n in done:
            return False
        pending.add(n)
        if any(visit(v) for v in edges[n]):
            return True
        pending.remove(n)
        done.add(n)
        return False
    return any(visit(n) for n in edges)


def raw_successors(inst):
    k = inst['operation']
    if k == 'Goto':
        return [inst['target']]
    if k == 'Branch':
        return [inst['then_block'],inst['else_block']]
    if k == 'Invoke':
        return [inst['continuation']]
    return []


def audit(catalog_path, llvm_bin=None, verify=True):
    catalog_path = Path(catalog_path).resolve()
    c = json.loads(catalog_path.read_text())
    base = catalog_path.parent
    source_path, source = read_bound(c,'source',base)
    llvm_path, llvm = read_bound(c,'llvm',base)
    if 'raw_view' in c:
        raw = c['raw_view']
    else:
        _, data = read_bound(c,'raw_witness',base)
        view = json.loads(data)
        raw = view.get('raw_view',view)
    if verify:
        require(llvm_bin is not None,'--llvm-bin or OXID_LLVM_BIN is required')
        p = subprocess.run([str(Path(llvm_bin)/'llvm-as'),str(llvm_path),'-o','/dev/null'],capture_output=True,text=True)
        require(p.returncode == 0,f'llvm-as rejects production module: {p.stderr}')
    functions, constants = parse_module(llvm.decode())
    model = RawModel(raw)
    model_report = None
    if 'model_path' in c:
        model_path, model_bytes = read_bound(c,'model',base)
        expected = json.loads(model_bytes)
        require(expected['source_sha256']==sha(source),'independent model/source binding differs')
        checked_frames = []
        for fid, f in model.functions.items():
            start,end = f['span']
            name = source[start:end].decode()
            require(name in expected['template_frames'],f'function {name} absent from independent model')
            want = expected['template_frames'][name]
            require(all(want[key]==value for key,value in model.usage[fid].items()),f'{name}: raw frame differs from independent source template')
            checked_frames.append(name)
        require(len(checked_frames)==len(expected['template_frames']),'independent frame inventory differs')
        model_report = dict(path=str(model_path),sha256=sha(model_bytes),frame_matches=checked_frames)
    owned = {int(name.rsplit('_',1)[1]):f for name,f in functions.items() if name.startswith('__oxid_owned_fn_')}
    require(set(owned) == set(model.functions),'raw/LLVM function inventory differs')
    guards = {fid:discover_guards(f,constants) for fid,f in owned.items()}
    guarded = any(g['kind']=='fuel' for gs in guards.values() for g in gs)
    require(c['guarded'] == guarded,'catalog guarded mode differs from actual guards')
    call_graph = {fid:[] for fid in owned}
    raw_cycles = []
    for fid, f in model.functions.items():
        edges = {b['id']:raw_successors(b['terminator']['instruction']) for b in f['blocks']}
        require(all(v in edges for vs in edges.values() for v in vs),'raw successor missing')
        if has_cycle(edges):
            raw_cycles.append(fid)
        call_graph[fid] = [op['target'] for key,op in model.operations.items() if key[0]==fid and op.get('target') is not None]
    require(not has_cycle(call_graph),'whole call graph is recursive')
    require(guarded == bool(raw_cycles),'fuel mode is not consistent with whole module cyclicity')
    static_bounds = {}
    def bound(fid):
        if fid in static_bounds:
            return static_bounds[fid]
        v = model.usage[fid]['X']
        for key,op in model.operations.items():
            if key[0] != fid or key[1] is None:
                continue
            v += op['cost']
            if op.get('target') is not None:
                target = op['target']
                v += bound(target)-model.usage[target]['X']
        static_bounds[fid] = v
        return v
    if not guarded:
        require(all(1+bound(fid) <= 100000 for fid in owned),'static unguarded conservative fuel admission exceeds 100000')
    op_guards, op_regions, error_nodes, all_fuel_stores = {}, {}, {}, set()
    for fid, f in owned.items():
        gs = guards[fid]
        fuel_parameter = re.search(r'\(ptr (' + SSA + r')(?:,|\))', f.header)
        require(not guarded or fuel_parameter is not None, f'{f.name}: missing fuel parameter')
        f.fuel_parameter = fuel_parameter[1] if guarded else None
        error_nodes[fid] = set()
        for b in model.functions[fid]['blocks']:
            label = b.get('llvm_label',f"b{b['id']}")
            require(label in f.blocks,f'{f.name}: missing raw block label {label}')
            start = f.block_lines[label]
            later = [f.block_lines[x.get('llvm_label',f"b{x['id']}")] for x in model.functions[fid]['blocks'] if f.block_lines[x.get('llvm_label',f"b{x['id']}")]>start]
            end = min(later,default=f.end)
            fuel = [g for g in gs if g['kind']=='fuel' and start<g['node']<end]
            ops = model.block_ops(fid,b['id'])
            require(len(fuel)==(len(ops) if guarded else 0),f'{f.name}:{label}: wrong number of operation fuel guards')
            for index,(key,op) in enumerate(ops):
                if guarded:
                    g = fuel[index]
                    require(g['cost']==op['cost'],f'{f.name}:{g["node"]}: charge {g["cost"]} != raw-independent cost {op["cost"]}')
                    require(g['pointer']==f.fuel_parameter,f'{f.name}: charge uses wrong shared fuel cell')
                    error_nodes[fid] |= guard_failure(f,g['error'],constants,'E0601',source,op['span'])
                    all_fuel_stores.add((fid,g['debit']))
                    next_guard = fuel[index+1]['loaded'] if index+1<len(fuel) else end
                    region = (g['loaded'],next_guard)
                    op_guards[key] = g
                else:
                    # Catalog gives physical store mapping; only arithmetic has a
                    # dynamic failure guard in statically admitted acyclic IR.
                    region = (start,end)
                op_regions[key] = region
            overflow = [g for g in gs if g['kind']=='overflow' and start<g['node']<end]
            expected = [(key,op) for key,op in ops if op['overflow_span'] is not None]
            require(len(overflow)==len(expected),f'{f.name}:{label}: missing or extra arithmetic overflow guards')
            for g,(key,op) in zip(overflow,expected):
                require(op_regions[key][0]<g['node']<op_regions[key][1],f'{f.name}: overflow outside charged operation')
                error_nodes[fid] |= guard_failure(f,g['error'],constants,'E0604',source,op['overflow_span'])
                op_guards[(key,'overflow')] = g
        require({g['node'] for g in gs}=={g['node'] for k,g in op_guards.items() if (isinstance(k[0],tuple) and k[0][0]==fid) or (isinstance(k[0],int) and k[0]==fid)},f'{f.name}: unbound guard')
    actual = {(fid,n):inst for fid,f in owned.items() for n,inst in f.instructions.items() if inst['text'].startswith('store ') and (fid,n) not in all_fuel_stores}
    catalog_rows = {}
    counts, store_reports, op_store_types = Counter(), [], defaultdict(list)
    for row in c['stores']:
        fid = row['function']
        if isinstance(fid,str):
            fid = int(fid.rsplit('_',1)[-1])
        node = row['llvm_line']
        key = (fid,node)
        require(key not in catalog_rows,'duplicate catalog physical store')
        catalog_rows[key] = row
        require(key in actual,f'catalog non-store/fuel/stale line {key}')
        inst = actual[key]
        require(inst['text']==row['store_text'].strip(),f'catalog store text differs at {key}')
        require(inst['block']==row['llvm_block'],f'catalog store block differs at {key}')
        r = row['raw_operation']
        opkey = (fid,row['raw_block'],r['kind'],None if r['kind'] in ('terminator','merge') else r.get('index'))
        require(opkey in model.operations,f'unknown raw operation {opkey}')
        op = model.operations[opkey]
        if 'charge_span' in row:
            require(row['charge_span']==op['span'],f'catalog span differs from raw source origin {opkey}')
        op_store_types[opkey].append(destination(inst['text'])[0])
        f = owned[fid]
        require(node in f.reached,f'{f.name}:{node}: unreachable store cannot count as execution proof')
        if r['kind']=='parameter':
            require(inst['block']==next(iter(f.blocks)),f'{f.name}:{node}: parameter copy moved outside entry')
            proof = 'caller-paid-frame' if guarded else 'static-frame-admission'
        elif guarded:
            g = op_guards[opkey]
            require(f.dominated_edge(node,(g['node'],g['success'])),f'{f.name}:{node}: STORE_BEFORE_ADMISSION: required fuel success edge does not dominate')
            require(f.dominated_node(node,g['debit']),f'{f.name}:{node}: STORE_BEFORE_DEBIT: paid charge does not dominate')
            require(op_regions[opkey][0]<node<op_regions[opkey][1],f'{f.name}:{node}: STORE_DELAYED_PAST_NEXT_OPERATION')
            proof = 'operation-paid-fuel'
        else:
            proof = 'acyclic-static-fuel-exemption'
        own_overflow = op_guards.get((opkey,'overflow'))
        if own_overflow:
            require(f.dominated_edge(node,(own_overflow['node'],own_overflow['success'])),f'{f.name}:{node}: STORE_BEFORE_OVERFLOW_SUCCESS')
            counts['overflow_result_stores'] += 1
        # Every earlier mandatory arithmetic check must pass before this store.
        # Conditional checks need not dominate globally, but their error edge is
        # still terminal and cannot reach ANY store (checked below).
        dominating_overflow = 0
        for g in guards[fid]:
            if g['kind']=='overflow' and f.dominated_node(node,g['node']):
                require(f.dominated_edge(node,(g['node'],g['success'])),f'{f.name}:{node}: prior overflow success does not dominate')
                dominating_overflow += 1
            require(node not in f.reachable(g['error']),f'{f.name}:{node}: error path reaches store')
        counts[proof] += 1
        counts['stores'] += 1
        counts['prior_overflow_store_dependencies'] += dominating_overflow
        store_reports.append(dict(function=fid,line=node,raw=list(opkey),proof=proof,overflow_dependencies=dominating_overflow))
    require(set(actual)==set(catalog_rows),f'catalog omits actual stores: {sorted(set(actual)-set(catalog_rows))}')
    for key,op in model.operations.items():
        require(op_store_types[key]==op['types'],f'raw operation {key}: physical store types/count {op_store_types[key]} != {op["types"]}')
    # Every internal invocation must pay the callee frame before it can execute
    # parameter copies. Root has its own 1+X admission; no function escapes.
    callers = defaultdict(list)
    for fid,f in owned.items():
        calls = [(n,re.search(r'call (?:i\d+|void) @__oxid_owned_fn_(\d+)\(',i['text'])) for n,i in f.instructions.items()]
        calls = [(n,int(m[1])) for n,m in calls if m]
        expected = [(key,op) for key,op in model.operations.items() if key[0]==fid and op.get('target') is not None]
        require(len(calls)==len(expected),f'{f.name}: raw/LLVM call inventory differs')
        for (node,target),(key,op) in zip(calls,expected):
            require(target==op['target'],f'{f.name}:{node}: call target differs')
            if guarded:
                g = op_guards[key]
                require(f.dominated_edge(node,(g['node'],g['success'])) and f.dominated_node(node,g['debit']),f'{f.name}:{node}: CALL_BEFORE_FRAME_ADMISSION')
                require(f'(ptr {f.fuel_parameter}' in f.instructions[node]['text'],f'{f.name}: callee fuel pointer differs')
            callers[target].append(dict(caller=fid,line=node,frame_cells=model.usage[target]['X'],paid_cost=op['cost'] if guarded else None))
    main = functions['main']
    root_calls = [(n,re.search(r'call i\d+ @__oxid_owned_fn_(\d+)\(',i['text'])) for n,i in main.instructions.items()]
    root_calls = [(n,int(m[1])) for n,m in root_calls if m]
    require(len(root_calls)==1,'wrapper must have one root invocation')
    root_node,entry = root_calls[0]
    require(entry==c['entry'],'root entry identity differs')
    require(not model.functions[entry]['parameters'],'root has parameters')
    root_guards = discover_guards(main,constants)
    require(len(root_guards)==int(guarded),'unexpected root guard inventory')
    if guarded:
        g = root_guards[0]
        require(g['kind']=='fuel' and g['cost']==1+model.usage[entry]['X'],'root admission cost differs')
        guard_failure(main,g['error'],constants,'E0601',source,model.functions[entry]['span'])
        require(main.dominated_edge(root_node,(g['node'],g['success'])) and main.dominated_node(root_node,g['debit']),'ROOT_CALL_BEFORE_FRAME_ADMISSION')
        require(f'(ptr {g["pointer"]})' in main.instructions[root_node]['text'],'root invocation does not share the admitted fuel cell')
    callers[entry].append(dict(caller='main',line=root_node,frame_cells=model.usage[entry]['X'],paid_cost=1+model.usage[entry]['X'] if guarded else None))
    for line in llvm.decode().splitlines():
        if '@__oxid_owned_fn_' in line:
            require(line.lstrip().startswith('define internal ') or re.search(r'\bcall (?:i\d+|void) @__oxid_owned_fn_\d+\(',line),'owned function address escapes direct internal calls')
    live = set()
    todo=[entry]
    while todo:
        fid=todo.pop()
        if fid not in live:
            live.add(fid)
            todo.extend(call_graph[fid])
    for fid,f in model.functions.items():
        if f['parameters']:
            require(callers[fid] or fid not in live,f'live parameter function {fid} has no paid caller')
    counts.update(functions=len(owned),fuel_guards=sum(g['kind']=='fuel' for gs in guards.values() for g in gs)+int(guarded),overflow_guards=sum(g['kind']=='overflow' for gs in guards.values() for g in gs),guarded_modules=int(guarded),static_modules=int(not guarded),parameter_stores=sum(len(op['types']) for k,op in model.operations.items() if k[2]=='parameter'))
    probe_report = None
    if 'probe_llvm_path' in c:
        probe_path,probe = read_bound(c,'probe_llvm',base)
        probe_lines = probe.splitlines(keepends=True)
        inserted = [l for l in probe_lines if l.rstrip().endswith(b'; UNIT4B_PROBE')]
        stripped = b''.join(l for l in probe_lines if not l.rstrip().endswith(b'; UNIT4B_PROBE'))
        require(inserted,'probe contains no marked insertion')
        require(stripped==llvm,'removing precisely marked probe lines does not restore entire production module')
        require(all(not l.lstrip().startswith((b'store ',b'br ',b'ret ',b'unreachable')) for l in inserted),'probe marker hides control flow or store edits')
        chunks, original_line = defaultdict(list), 0
        for line in probe_lines:
            if line.rstrip().endswith(b'; UNIT4B_PROBE'):
                chunks[original_line].append(line.decode().rsplit('; UNIT4B_PROBE',1)[0].strip())
            else:
                original_line += 1
        post_store_groups = 0
        for row in c['stores']:
            node = row['llvm_line']
            chunk = chunks.pop(node,[])
            typ,_,ptr = destination(row['store_text'].strip())
            site = row['site']
            require(len(chunk)==(1 if typ=='ptr' else 2 if typ=='i64' else 3),f'probe site {site}: missing/extra post-store instructions')
            if typ=='ptr':
                observed = '0'
            else:
                load = re.fullmatch('(' + SSA + f') = load {typ}, ptr {re.escape(ptr)}, align 1',chunk[0])
                require(load is not None,f'probe site {site}: not an immediate read of just-stored location/type')
                observed=load[1]
                if typ!='i64':
                    wide = re.fullmatch('(' + SSA + f') = zext {typ} {re.escape(observed)} to i64',chunk[1])
                    require(wide is not None,f'probe site {site}: incorrect unsigned value widening')
                    observed=wide[1]
            call = re.fullmatch(SSA + r' = call i32 \(i32, ptr, \.\.\.\) @dprintf\(i32 2, ptr @__unit4b_store_format, i64 '+str(site)+r', i64 '+re.escape(observed)+r'\)',chunk[-1])
            require(call is not None,f'probe site {site}: not the expected stderr occurrence/value observation')
            post_store_groups += 1
        remaining=[line for chunk in chunks.values() for line in chunk]
        require(len(remaining)==2 and remaining[0]=='declare i32 @dprintf(i32, ptr, ...)', 'probe contains unbound extra instructions')
        fmt = b'__UNIT4B_STORE %llu %llu\n\0'
        encoded=''.join('\\'+format(v,'02X') for v in fmt)
        require(remaining[1]==f'@__unit4b_store_format = private constant [{len(fmt)} x i8] c"{encoded}"','probe printf format differs')
        if verify:
            p=subprocess.run([str(Path(llvm_bin)/'llvm-as'),str(probe_path),'-o','/dev/null'],capture_output=True,text=True)
            require(p.returncode==0,f'llvm-as rejects observation probe: {p.stderr}')
        probe_report=dict(path=str(probe_path),sha256=sha(probe),inserted_lines=len(inserted),post_store_groups=post_store_groups,stripped_sha256=sha(stripped),byte_identical=True,pointer_values='occurrence only; no address or referent value claim',scalar_values='unsigned physical i1/i8/i32/i64 stored bits')
    return dict(status='pass',catalog=str(catalog_path),catalog_sha256=sha(catalog_path.read_bytes()),llvm=str(llvm_path),llvm_sha256=sha(llvm),source=str(source_path),source_sha256=sha(source),guarded=guarded,counts=dict(counts),raw_cycles=raw_cycles,static_bounds=static_bounds,frame_usage=model.usage,callers=dict(callers),unreachable_functions=sorted(set(owned)-live),stores=store_reports,probe=probe_report,model=model_report,
                limitation='CFG/static proof is over the bound emitted IR and raw witness. Probe execution is separate evidence; source/reference Event does not expose every physical scalar, whole, parameter, capability or empty store. Static acyclic modules intentionally have no per-budget fuel guards.')


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('catalog',type=Path)
    p.add_argument('--llvm-bin',default=os.environ.get('OXID_LLVM_BIN'))
    p.add_argument('--output',type=Path)
    args=p.parse_args()
    try:
        report=audit(args.catalog,args.llvm_bin)
    except (AuditFailure,KeyError,ValueError,OSError) as error:
        report=dict(status='fail',catalog=str(args.catalog),catalog_sha256=sha(args.catalog.read_bytes()) if args.catalog.exists() else None,error=str(error),error_type=type(error).__name__)
    output=json.dumps(report,indent=2,sort_keys=True)+'\n'
    if args.output:
        # Each invocation chooses a fresh report name. Never erase first failures.
        with args.output.open('x') as f:
            f.write(output)
    print(output)
    return report['status']!='pass'


if __name__=='__main__':
    sys.exit(main())
