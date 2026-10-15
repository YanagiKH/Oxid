"""Pure, finite current-binding transport for the reviewed Windows binary-reader and roster fix.

Historical generators and generation receipts remain unchanged. This is source
transport only, not execution qualification. Caller supplies all bytes; no file,
network, compiler, target-module import or directory traversal occurs here.
"""
import ast
import copy
import hashlib
import json
import types

P = 'tests/qualification/published_formatter_current/'
S = 'tests/fixtures/typed_project_source_binding/'
R = 'tests/qualification/unit4_parser_current/'
U = 'tests/qualification/unit4_public_v3/'
L = 'tests/qualification/lexer_reservation_current/'
I = 'tests/qualification/unit4_ci/'
ENTRY = 'scripts/test_published_formatter_source.py'
RECIPE = P + 'derive_windows_bindings_v2.py'
RECEIPT = P + 'source-proof/windows-bindings-derivation-v2.json'
SEEDS = {'tests/fixtures/typed_project_source_binding/run.py': (6721, '5fe80abde86d7f00ace57cdeb87b0db8388eafb2861647fd7ad5558ec9e8baa5'), 'tests/qualification/unit4_ci/test_source_identity.py': (19556, '52b324aafe927d1fb9d552e726a022ba919e53b73a644f3313ff188d7b150436'), 'tests/qualification/unit4_public_v3/published_lifecycle.py': (46537, 'b2862acdf7846472393742f968840b6caefa0298035aa923b15eccec1e9dcda6'), 'tests/qualification/published_formatter_current/published_view.py': (12882, '11a381be0f642138663d46c928b48b434332cb8152cf5006dd452fd178076e9b'), 'tests/qualification/unit4_ci/test_integration.py': (165022, '4d8b505cab7e538293b2e97e970f7c71e1202d2b47a8b9360429c790d3b17583')}
# The exact PR-head manifest is independently pinned, never inferred from bodies.
BASE_INPUTS_PIN = (199544, '3e5f11a6ed3c86d87c5ef4702e7cb8f09dd3ac9e7998b080087eaafc03757e13')
BASE_COMMON_PIN = (17821, '0dbfc98ca5bab1c294bb58c7a50017debf4e87444ef984af7010b64fd6c158d7')


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def pin(name, raw):
    return dict(path=name, bytes=len(raw), sha256=sha(raw))


def encode(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode()


def declaration(raw, name):
    found = [n for n in ast.parse(raw).body if isinstance(n, ast.Assign)
             and any(isinstance(t, ast.Name) and t.id == name for t in n.targets)]
    need(len(found) == 1, 'unique literal seam: ' + name)
    return found[0]


def literal(raw, name):
    return ast.literal_eval(declaration(raw, name).value)


def assign(raw, name, value):
    """Change one top-level data assignment; verify all executable AST unchanged."""
    node = declaration(raw, name)
    lines = raw.decode().splitlines(keepends=True)
    lines[node.lineno - 1:node.end_lineno] = [name + ' = ' + repr(value) + '\n']
    result = ''.join(lines).encode()
    before, after = ast.parse(raw), ast.parse(result)
    a = declaration(result, name)
    for n in after.body:
        if isinstance(n, ast.Assign) and n.lineno == a.lineno:
            n.value = node.value
    need(ast.dump(before, include_attributes=False) == ast.dump(after, include_attributes=False),
         'executable syntax changed outside data assignment: ' + name)
    return result


def derive(bodies, *, recipe_raw):
    """Authenticate exact initial closure, then produce a deterministic successor."""
    need(type(bodies) is dict and all(type(v) is bytes for v in bodies.values()), 'byte buffers required')
    initial = dict(bodies)
    need((len(bodies[I + 'inputs.json']), sha(bodies[I + 'inputs.json'])) == BASE_INPUTS_PIN,
         'unreviewed predecessor input manifest')
    need(bodies[I + 'inputs.json'] == bodies[I + 'inputs-published-formatter-v1.json'], 'input aliases differ')
    need((len(bodies[I + 'common.py']), sha(bodies[I + 'common.py'])) == BASE_COMMON_PIN, 'changed common template')
    old = json.loads(bodies[I + 'inputs.json'])
    need(len(old['files']) == 977 and len(old['closed_roots']) == 16, 'changed predecessor closure')
    expected_names = {row['path'] for row in old['files']} | {I + n for n in (
        'inputs.json', 'inputs-published-formatter-v1.json', 'common.py', 'test_integration.py')}
    expected_names |= set(SEEDS)
    need(set(bodies) == expected_names, 'input body roster differs')
    for row in old['files']:
        n = row['path']
        expected = SEEDS.get(n, (row['bytes'], row['sha256']))
        need((len(bodies[n]), sha(bodies[n])) == expected, 'unreviewed current input: ' + n)
    for n, expected in SEEDS.items():
        need((len(bodies[n]), sha(bodies[n])) == expected, 'changed reviewed fix: ' + n)
    out = dict(bodies)
    # Current compiler/source closure and every historical view read stay exact.
    source = json.loads(out[S + 'current-source.json'])
    need(len(source['files']) == 376, 'changed current source closure')
    for row in source['files']:
        need(pin(row['path'], out[row['path']]) == row, 'changed current compiler/source body')
    view = json.loads(out[P + 'predecessor-view-v1.json'])
    need(len(view['files']) == 681, 'changed historical view closure')
    for row in view['files']:
        n = row['read_from']
        if n is not None:
            need(pin(n, out[n]) == dict(path=n, bytes=row['bytes'], sha256=row['sha256']),
                 'changed historical predecessor body: ' + n)
    def desc(n): return pin(n, out[n])
    def setjson(n, value): out[n] = encode(value)
    def update_rows(rows, prefix=''):
        return [dict(row, bytes=len(out[prefix + row['path']]), sha256=sha(out[prefix + row['path']]))
                for row in rows]
    # Dispatcher first, then its package row and consumers. Source seal is fixed.
    n = S + 'run.py'
    helpers = literal(out[n], '_HELPER_SPECS')
    for pair in helpers:
        pair[1] = desc(pair[1]['path'])
    out[n] = assign(out[n], '_HELPER_SPECS', helpers)
    manifest = json.loads(out[S + 'package-manifest.json'])
    manifest['files'] = update_rows(manifest['files'], S)
    setjson(S + 'package-manifest.json', manifest)
    n = L + 'unit1_current_support.py'
    out[n] = assign(out[n], 'DISPATCHER_SHA', sha(out[S + 'run.py']))
    out[n] = assign(out[n], 'SOURCE_PACKAGE_SHA', sha(out[S + 'package-manifest.json']))
    n = P + 'test_published_transition.py'
    pins = literal(out[n], 'HELPER_PINS')
    pins['published_view.py'] = sha(out[P + 'published_view.py'])
    out[n] = assign(out[n], 'HELPER_PINS', pins)
    # Parser authority's runtime binding changes, historical/source provenance does not.
    authority = json.loads(out[R + 'authority.json'])
    authority['source_binding_runner'] = desc(S + 'run.py')
    setjson(R + 'authority.json', authority)
    seal = json.loads(out[P + 'source-proof/parser-seal-v1.json'])
    for key in ('authority', 'source_dispatcher', 'view'):
        seal[key] = desc(seal[key]['path'])
    for item in seal['source_runtime_dependencies']:
        item['artifact'] = desc(item['artifact']['path'])
    setjson(P + 'source-proof/parser-seal-v1.json', seal)
    n = R + 'portable.py'
    out[n] = assign(out[n], 'AUTHORITY_SHA', sha(out[R + 'authority.json']))
    out[n] = assign(out[n], '_PUBLISHED_PARSER_SEAL', json.loads(json.dumps(seal, sort_keys=True)))
    # The unchanged original pure parser recipe must reproduce every new byte.
    recipe = out[P + 'derive_parser_successor.py']
    need(sha(recipe) == '2d5427b6f3bd4dfa0cfec6ad7723725c0122a70ccc033a34c41d728470d76bf6', 'changed parser recipe')
    module = types.ModuleType('reviewed_pure_parser_derivation')
    exec(compile(recipe, '<reviewed pure parser recipe>', 'exec'), module.__dict__)
    regenerated, _ = module.derive(out[P + 'predecessor-v1/' + R + 'portable.py'], seal)
    need(ast.dump(ast.parse(regenerated), include_attributes=False) == ast.dump(ast.parse(out[n]), include_attributes=False), 'current portable differs from original pure recipe')
    out[n] = regenerated
    # Existing public lifecycle predicates remain intact, only data pins move.
    n = U + 'published_lifecycle.py'
    out[n] = assign(out[n], 'DISPATCHER_PIN', desc(S + 'run.py'))
    out[n] = assign(out[n], 'PACKAGE_PIN', desc(S + 'package-manifest.json'))
    proof = json.loads(out[U + 'published-lifecycle-v1.json'])
    for key in ('source_dispatcher', 'source_package_manifest', 'transport_helper'):
        proof[key] = desc(proof[key]['path'])
    for pair in proof['source_helpers']:
        pair['body'] = desc(pair['body']['path'])
    setjson(U + 'published-lifecycle-v1.json', proof)
    n = U + 'authority.py'
    for key, target in (('PUBLISHED_LIFECYCLE_HELPER', 'published_lifecycle.py'),
                        ('PUBLISHED_LIFECYCLE_PROOF', 'published-lifecycle-v1.json')):
        out[n] = assign(out[n], key, dict(desc(U + target), path=target))
    # Exact finite fixture sizes are recomputed, with original retained overhead.
    fixture = json.loads(out[P + 'parser-control-fixture-v1.json'])
    fixture['files'] = update_rows(fixture['files'])
    fixture['logical_bytes'] = sum(r['bytes'] for r in fixture['files'])
    fixture['file_rounded_bytes'] = sum(((r['bytes'] + 4095) // 4096) * 4096 for r in fixture['files'])
    fixture['allocated_bytes'] = fixture['file_rounded_bytes'] + (len(fixture['directories']) + 1) * 4096
    fixture['maximum_lifetime_bytes'] = fixture['allocated_bytes'] + fixture['other_live_allocated_bytes']
    need(fixture['maximum_lifetime_bytes'] <= 128 * 1024 * 1024, 'fixture lifetime exceeds unchanged ceiling')
    setjson(P + 'parser-control-fixture-v1.json', fixture)
    for n in (R + 'test_current.py', R + 'test_lexer_reservation.py'):
        pins = literal(out[n], '_PARSER_CONTROL_PINS')
        for key in ('authority', 'portable', 'fixture'):
            pins[key] = desc(pins[key]['path'])
        out[n] = assign(out[n], '_PARSER_CONTROL_PINS', pins)
    # Preserve every old receipt. New receipt records only this transport stage;
    # downstream entrypoint/manifests are excluded to avoid self-hash cycles.
    changes = [dict(before=pin(n, initial[n]), after=desc(n))
               for n in sorted(initial) if out[n] != initial[n]]
    receipt = dict(schema='oxid-windows-current-binding-derivation-v2',
        source_only=True, execution_qualified=False, tests_executed=0,
        windows_execution_performed=False, historical_generation_records_modified=False,
        predecessor_git_head='1e68344e4f0481e458d4d7b0dfbf6429b03e4654',
        predecessor_inputs=pin(I + 'inputs.json', initial[I + 'inputs.json']),
        reviewed_seed_files=[pin(n, initial[n]) for n in sorted(SEEDS)],
        recipe=pin(RECIPE, recipe_raw), transported_bindings=changes,
        transport='Only named data assignments and current descriptor rows; executable consumer AST preserved.',
        runtime_changes='Separately reviewed binary-open and authoritative-roster fixes; full existing identity predicates retained.',
        downstream='Source-control input roster, both current CI aliases, common input pin, and exact inventory count are derived last.')
    out[RECIPE] = recipe_raw
    out[RECEIPT] = encode(receipt)
    config = literal(out[ENTRY], '_CONFIG')
    config['input_roster'] = update_rows(config['input_roster'])
    config['input_roster'] += [desc(RECIPE), desc(RECEIPT)]
    config['input_roster'].sort(key=lambda r: r['path'])
    out[ENTRY] = assign(out[ENTRY], '_CONFIG', config)
    # No exclusions are relaxed: old977 plus both new source-only artifacts.
    current = copy.deepcopy(old)
    current['files'] = update_rows(old['files']) + [desc(RECIPE), desc(RECEIPT)]
    current['files'].sort(key=lambda r: r['path'])
    need(len(current['files']) == 979, 'wrong complete current roster')
    current_raw = encode(current)
    out[I + 'inputs.json'] = out[I + 'inputs-published-formatter-v1.json'] = current_raw
    out[I + 'common.py'] = assign(out[I + 'common.py'], 'INPUTS_SHA', sha(current_raw))
    # Preserve reviewed metadata controls; only exact expected inventory grows.
    n = I + 'test_integration.py'
    seam = b"self.assertEqual((len(current['files']), len(current['closed_roots'])), (977, 16))"
    need(out[n].count(seam) == 1, 'missing unique exact inventory assertion')
    out[n] = out[n].replace(seam, seam.replace(b'(977, 16)', b'(979, 16)'))
    need(out[n].replace(seam.replace(b'(977, 16)', b'(979, 16)'), seam) == initial[n], 'other integration edits')
    need(len(out[n]) <= 172032, 'integration cap exceeded')
    for row in current['files']:
        need(desc(row['path']) == row, 'final descriptor mismatch')
    return {n: raw for n, raw in out.items() if n not in initial or raw != initial[n]}
