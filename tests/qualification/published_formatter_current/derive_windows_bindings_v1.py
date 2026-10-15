"""Pure, finite current-binding transport for the reviewed Windows reader fix.

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
RECIPE = P + 'derive_windows_bindings_v1.py'
RECEIPT = P + 'source-proof/windows-bindings-derivation-v1.json'
SEEDS = {'tests/qualification/published_formatter_current/published_view.py': (12607, '629ca03dd3633df969f6ca50c6d3d675ce5f5acd70caa00a372ae6a9cb2a5aca'), 'tests/fixtures/typed_project_source_binding/run.py': (6625, '4cc52140267b55f1b5af8461ebfb97e414ff00c9ea28435de4137fdc13e39680'), 'tests/qualification/unit2_u8_current/test_current.py': (9180, '2766f0fb1cf336b34f3d166f4370e26e572b49ddfdea19cc2e9265af394d822d'), 'tests/qualification/unit4_public_v3/published_lifecycle.py': (46328, '2d206734581b99fac9c9d2ed5cde0942d37b5f22f00094a7af5334900e7f5bda'), 'tests/qualification/unit4_ci/test_source_identity.py': (13949, 'adfdffb2db8fed16c34276c3949d19297bfc390ed8a02952ea05be3fd8f19876'), 'tests/qualification/unit4_ci/test_integration.py': (165022, '5eeac325c5b20dc0563a29c4dce8be7fd33c988c1e651650cfa6a658100410a9'), 'scripts/test_owned_array_native.py': (147064, '8e2917490150fc60ca45e0cb8fdf5d84a1623ae6aa71898c636b5b63cf2fa86a'), 'scripts/test_lexer_reservation_source_current.py': (703, '2a2cfe16518034fd4576aea4c6867311f573befe8f0000a46b15f49bba4500ce'), 'tests/qualification/lexer_reservation_current/test_unit1_current.py': (17038, '0127972a2cf288529cbfc46302628b18db2c9bfbceaa35900658beb42f2713e0'), 'tests/qualification/lexer_reservation_current/test_current_source.py': (26772, '0c09d4776284d614a1b4423010358b9464b4c2e07b729ea685739b36a0d1085a'), 'tests/qualification/unit4_public_v3/run.py': (31003, 'c05785e7ccb061122d78bcdd1a3565ab15b371f315049a23972518e4c6d986fb')}
# The exact PR-head manifest is independently pinned, never inferred from bodies.
BASE_INPUTS_PIN = (199100, 'b242f65a5ddace7f402dfa51901bd4e81796fdbcb075e9f6ed2c581a4d7a5c4e')
BASE_COMMON_PIN = (17821, 'd0edb1cb3017b9828fd84493b6c1fe33e20a6b88dd55c56fe03e640ad22d4d11')


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
    need(len(old['files']) == 975 and len(old['closed_roots']) == 16, 'changed predecessor closure')
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
    receipt = dict(schema='oxid-windows-current-binding-derivation-v1',
        source_only=True, execution_qualified=False, tests_executed=0,
        windows_execution_performed=False, historical_generation_records_modified=False,
        predecessor_git_head='eb529f71f80126516fa50c26dddbf2e36ab78924',
        predecessor_inputs=pin(I + 'inputs.json', initial[I + 'inputs.json']),
        reviewed_seed_files=[pin(n, initial[n]) for n in sorted(SEEDS)],
        recipe=pin(RECIPE, recipe_raw), transported_bindings=changes,
        transport='Only named data assignments and current descriptor rows; executable consumer AST preserved.',
        runtime_changes='Separately reviewed reader/platform/schema fixes; no AST-equivalence claim for those fixes.',
        downstream='Source-control input roster, both current CI aliases, common input pin, and exact inventory count are derived last.')
    out[RECIPE] = recipe_raw
    out[RECEIPT] = encode(receipt)
    config = literal(out[ENTRY], '_CONFIG')
    config['input_roster'] = update_rows(config['input_roster'])
    config['input_roster'] += [desc(RECIPE), desc(RECEIPT)]
    config['input_roster'].sort(key=lambda r: r['path'])
    out[ENTRY] = assign(out[ENTRY], '_CONFIG', config)
    # No exclusions are relaxed: old975 plus both new source-only artifacts.
    current = copy.deepcopy(old)
    current['files'] = update_rows(old['files']) + [desc(RECIPE), desc(RECEIPT)]
    current['files'].sort(key=lambda r: r['path'])
    need(len(current['files']) == 977, 'wrong complete current roster')
    current_raw = encode(current)
    out[I + 'inputs.json'] = out[I + 'inputs-published-formatter-v1.json'] = current_raw
    out[I + 'common.py'] = assign(out[I + 'common.py'], 'INPUTS_SHA', sha(current_raw))
    # Preserve reviewed metadata controls; only exact expected inventory grows.
    n = I + 'test_integration.py'
    seam = b"self.assertEqual((len(current['files']), len(current['closed_roots'])), (975, 16))"
    need(out[n].count(seam) == 1, 'missing unique exact inventory assertion')
    out[n] = out[n].replace(seam, seam.replace(b'(975, 16)', b'(977, 16)'))
    need(out[n].replace(seam.replace(b'(975, 16)', b'(977, 16)'), seam) == initial[n], 'other integration edits')
    need(len(out[n]) <= 172032, 'integration cap exceeded')
    for row in current['files']:
        need(desc(row['path']) == row, 'final descriptor mismatch')
    return {n: raw for n, raw in out.items() if n not in initial or raw != initial[n]}
