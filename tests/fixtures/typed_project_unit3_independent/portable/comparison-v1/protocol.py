"""Path and identity admission for unchanged independent Unit3 comparators."""
import os, sys
if not __debug__ or sys.flags.optimize or os.environ.get('PYTHONOPTIMIZE', '0') not in ('', '0'):
    raise SystemExit('PROTOCOL_REFUSED: Python assertions must be enabled')
import hashlib, importlib, json, shutil
from pathlib import Path

COMPONENT_SHA = '920114e750c228e1524c8dc77ba3dfec44f4a902860c00ec02413f6e39e22ca2'
REQUEST_SHA = 'f405bf46d8945027cd1d79d3c125692b4c00b8fe57b97c0d55c68b83b9795113'
PARSER_SHA = '638fb39b814f637bf21902738edb35bbe775956d2c00a0e307957668e38c4264'
OBSERVER_SHA = 'ff51288e0bf90733eafe4690c02feacac06e61ab0608642423fd700e7fca42fc'
SCHEMA = 'unit3-independent-portable'

def require(value, message):
    if not value: raise ValueError(message)
def strict_json(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON key: ' + key); result[key] = value
        return result
    return json.loads(data, object_pairs_hook=pairs, parse_constant=lambda v: (_ for _ in ()).throw(ValueError(v)))
def read(path): return strict_json(Path(path).read_bytes())
def sha(data): return hashlib.sha256(data).hexdigest()
def identity(path):
    path = Path(path); digest = hashlib.sha256(); size = 0
    require(path.is_file() and not path.is_symlink() and path.resolve()==path.absolute(), 'missing/nonregular/aliased file before read')
    with path.open('rb') as source:
        while chunk := source.read(1024 * 1024): digest.update(chunk); size += len(chunk)
    return dict(path=str(path), bytes=size, sha256=digest.hexdigest())
def keys(value, expected, label): require(isinstance(value, dict) and set(value) == set(expected), 'unknown/missing ' + label + ' fields')
def absolute(value):
    require(isinstance(value, str) and value and '\x00' not in value and '\n' not in value and '\r' not in value, 'invalid absolute path')
    path = Path(value); require(path.is_absolute() and str(path) == value and '..' not in path.parts, 'noncanonical absolute path')
    return path
def relative(value):
    require(isinstance(value, str) and value and not any(c in value for c in '\x00\r\n\\'), 'invalid relative path')
    path = Path(value)
    require(not path.is_absolute() and all(p not in ('', '.', '..') for p in value.split('/')) and str(path) == value, 'unsafe relative path')
    return path
def verified(row):
    keys(row, ('path', 'bytes', 'sha256'), 'file identity'); path = absolute(row['path'])
    require(type(row['bytes']) is int and row['bytes'] >= 0, 'invalid byte size')
    require(path.is_file() and not path.is_symlink() and path.resolve() == path, 'missing/aliased bound artifact')
    require(path.stat().st_size == row['bytes'], 'stale artifact size before hashing: ' + str(path))
    require(identity(path) == row, 'stale file identity: ' + str(path)); return path
def read_bound(row): return read(verified(row))
def regular_rows(root, rows, exact=False):
    root = Path(root); seen = set()
    for row in rows:
        keys(row, ('path', 'bytes', 'sha256'), 'member'); rel = relative(row['path'])
        require(row['path'] not in seen, 'duplicate member'); seen.add(row['path'])
        verified({**row, 'path': str(root / rel)})
    if exact:
        actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file() or p.is_symlink()}
        require(actual == seen, 'unknown/missing component member')
def assertions(mode):
    require(mode.get('__debug__') is True and mode.get('optimize') == 0 and mode.get('PYTHONOPTIMIZE', '0') in ('', '0'), 'unqualified assertion mode')
def write(path, value): Path(path).write_text(json.dumps(value, sort_keys=True, indent=2) + '\n')
def component(path):
    path = Path(path); require(identity(path)['sha256'] == COMPONENT_SHA, 'unapproved component manifest')
    data = read(path)
    require(data['schema'] == SCHEMA + '-oracle-component-v1', 'component schema')
    require(len(data['files']) == data['non_source_count'] == 101 and len(data['source_members']) == data['source_count'] == 445, 'component cardinality')
    return data

def materialization(row, manifest):
    path = verified(row); data = read(path)
    require(data['schema'] == 'oxid-unit3-portable-v1-materialized' and data['status'] == 'materialized', 'wrong materialization')
    assertions(data['assertion_mode']); root = path.parent
    require(data['source_root'] == str(root), 'moved materialization')
    require(data['members'] == manifest['source_members'], 'shared source identity/membership differs from original freeze')
    regular_rows(root, data['members'])
    expected = {x['path'] for x in manifest['source_members']}
    actual = {p.relative_to(root / 'sources').as_posix() for p in (root / 'sources').rglob('*') if p.is_file() or p.is_symlink()}
    require(actual == {str(Path(x).relative_to('sources')) for x in expected}, 'unknown/missing shared source member')
    require(verified(data['requests']) == root / 'requests.jsonl' and data['requests']['sha256'] == REQUEST_SHA, 'source requests relocated/changed')
    requests = [strict_json(line) for line in (root / 'requests.jsonl').read_bytes().splitlines()]
    require(len(requests) == len({x['id'] for x in requests}) == 152, 'source request cardinality')
    return root, data, {x['id']: x for x in requests}

def prepare(component_root, component_manifest, materialization_path, parser_path, output):
    manifest = component(component_manifest); root = absolute(str(Path(component_root).resolve()))
    regular_rows(root, manifest['files'], exact=True)
    material_row = identity(Path(materialization_path).resolve()); sources, _, _ = materialization(material_row, manifest)
    parser_path = Path(parser_path).resolve(); parser_identity = identity(parser_path)
    require({k:parser_identity[k] for k in ('bytes', 'sha256')} == manifest['observer_parser'], 'wrong mechanical parser')
    verified(parser_identity)
    out = Path(output).resolve(); require(not out.exists(), 'fresh comparator view required')
    # All input checks precede creation. Only this declared directory alias is
    # permitted; candidate invokers continue to use the physical material root.
    oracle = out / 'typed-project-unit3-oracles'; oracle.mkdir(parents=True)
    for row in manifest['files']:
        dst = oracle / row['path']; dst.parent.mkdir(parents=True, exist_ok=True); shutil.copyfile(root / row['path'], dst)
    (oracle / 'sources').symlink_to(sources / 'sources', target_is_directory=True)
    parser_target = out / 'typed-project-unit3-observer/parse_debug.py'; parser_target.parent.mkdir(); shutil.copyfile(parser_path, parser_target)
    shutil.copyfile(component_manifest, out / 'component-manifest.json')
    result = dict(schema=SCHEMA + '-prepared-v1', status='prepared',
                  component=identity(out / 'component-manifest.json'), materialization=material_row,
                  oracle_root=str(oracle), source_alias=dict(path=str(oracle / 'sources'), target=str(sources / 'sources')),
                  parser=identity(parser_target), assertion_mode={'__debug__':__debug__, 'optimize':sys.flags.optimize, 'PYTHONOPTIMIZE':os.environ.get('PYTHONOPTIMIZE', '')})
    write(out / 'prepared-comparison.json', result); return result

def open_prepared(path):
    path = Path(path).resolve(); prepared = read(path)
    keys(prepared, ('schema','status','component','materialization','oracle_root','source_alias','parser','assertion_mode'), 'prepared comparison')
    require(prepared['schema'] == SCHEMA + '-prepared-v1' and prepared['status'] == 'prepared', 'wrong comparison preparation')
    assertions(prepared['assertion_mode']); manifest = component(verified(prepared['component']))
    require(verified(prepared['component']) == path.parent / 'component-manifest.json', 'component moved')
    sources, material, requests = materialization(prepared['materialization'], manifest)
    oracle = absolute(prepared['oracle_root']); require(oracle == path.parent / 'typed-project-unit3-oracles', 'oracle view moved')
    regular_rows(oracle, manifest['files'])
    alias = oracle / 'sources'; require(prepared['source_alias'] == dict(path=str(alias), target=str(sources / 'sources')), 'undeclared source alias')
    require(alias.is_symlink() and alias.readlink() == sources / 'sources' and alias.resolve() == sources / 'sources', 'source alias changed')
    files = set(); aliases = set()
    for base, dirs, names in os.walk(oracle, followlinks=False):
        for name in dirs + names:
            p = Path(base) / name
            if p.is_symlink(): aliases.add(p.relative_to(oracle).as_posix())
        for name in names:
            p = Path(base) / name
            if not p.is_symlink(): files.add(p.relative_to(oracle).as_posix())
    require(files == {x['path'] for x in manifest['files']} and aliases == {'sources'}, 'extra/missing/aliased oracle view member')
    require(verified(prepared['parser']) == path.parent / 'typed-project-unit3-observer/parse_debug.py' and prepared['parser']['sha256'] == PARSER_SHA, 'parser relocation')
    # Import only after every component member and the single alias is checked.
    for folder in (oracle, oracle / 'comparison-v1', oracle / 'mutation-comparison-v1'):
        sys.path.insert(0, str(folder))
    projection = importlib.import_module('expected_projection')
    require(Path(projection.__file__).resolve() == oracle / 'comparison-v1/expected_projection.py', 'unexpected cached semantic module')
    for row in manifest['manifests']:
        projection.verify_manifest(oracle / row['path'], row['sha256'])
    cases = projection.load()
    return dict(prepared=prepared, manifest=manifest, oracle=oracle, sources=sources, material=material, requests=requests, cases=cases)

def plan(path, digest, ctx):
    path = Path(path).resolve(); require(identity(path)['sha256'] == digest, 'unapproved source-only plan')
    data = read(path); keys(data, ('schema','kind','scope','materialization','builds','rows'), 'plan')
    require(data['schema'] == SCHEMA + '-plan-v1' and data['kind'] in ('source','native','mutation') and data['scope'] in ('full','bounded'), 'unknown plan mode')
    require(data['materialization'] == ctx['prepared']['materialization'], 'plan materialization differs')
    require(isinstance(data['rows'], list) and data['rows'], 'empty plan')
    require(isinstance(data['builds'], dict) and data['builds'], 'missing builds')
    receipts = set(); wrappers = set(); used = set()
    common = {'id','profile','build','receipt','wrapper_receipt'}
    native = {'group','operation','format','fuel','noclobber','source_root','entry','output','input_request_sha256'}
    seen = set()
    for row in data['rows']:
        keys(row, common | (native if data['kind'] == 'native' else set()), 'plan row')
        require(row['profile'] in ('debug','release'), 'unknown profile')
        absolute(row['receipt']); absolute(row['wrapper_receipt'])
        require(row['receipt'] not in receipts and row['wrapper_receipt'] not in wrappers, 'duplicate receipt/wrapper path')
        receipts.add(row['receipt']); wrappers.add(row['wrapper_receipt']); used.add(row['build'])
        require(row['build'] in data['builds'], 'unknown build key')
        binding = data['builds'][row['build']]
        require(binding['profile'] == row['profile'], 'wrong-profile build')
        if data['kind'] == 'source': require(binding['family'] == 'source' and row['id'] in ctx['cases'], 'unknown source/family')
        elif data['kind'] == 'native': require(binding['family'] == 'native', 'wrong native family')
        else:
            require(binding['family'] == ('mutation-v3' if row['id'] == 'source-association-stale_parser_generation' else 'mutation-v2'), 'wrong mutation family')
        if data['kind'] != 'native':
            key = (row['id'], row['profile']); require(key not in seen, 'duplicate case/profile'); seen.add(key)
    require(used == set(data['builds']), 'unused build binding')
    if data['kind'] == 'source' and data['scope'] == 'full':
        require(seen == {(id,p) for id in ctx['cases'] for p in ('debug','release')}, 'incomplete full source roster')
    if data['kind'] == 'mutation':
        effective = read(ctx['oracle'] / 'supplements/mutation-applicability-v1/expectations.json')
        ids = {x['id'] for x in effective['mutations'] if x['expected']['authority'] != 'driver-consumer-boundary'}
        require(len(ids) == 110 and seen <= {(id,p) for id in ids for p in ('debug','release')}, 'unknown internal mutation')
        if data['scope'] == 'full': require(seen == {(id,p) for id in ids for p in ('debug','release')}, 'incomplete full internal mutation roster')
    return data

def recheck_plan(path, digest, original):
    require(identity(Path(path).resolve())['sha256']==digest and read(path)==original,'source-only plan changed during comparison')
