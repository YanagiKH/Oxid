"""Authentication and isolated staging for the separately named current Unit1 run.

No compiler is invoked by this module. Execution is owned by the exact derived
runner and continues to require a separate build authorization. Historical inputs
are read-only; only the isolated reviewer's one approved counter branch changes.
"""
import hashlib
import json
from pathlib import Path
import re
import stat
import sys
import types

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
CHECKPOINT = 'c8e9a72afd9866f32b96f98ae24f61390039f421'
CURRENT_SOURCE_SHA = 'aa021e6046786300d13b12c22e5cff3f2565b1ae8f739e0698bdad93fd33a633'
CURRENT_SOURCE_BYTES = 74327
DISPATCHER_SHA = 'a978de636e715d0b9283f9b0228b9d6825b2d6044bb114410f1cf1a6a54739ae'
SOURCE_PACKAGE_SHA = '0c201be199aff6819907419a0b09a54297026010d74f80efb406959cfef538db'
ORIGINAL_MAP_SHA = 'd0ccbbd78ff138f15791b55aed88bb33eaf6b68c69022ccb6bf0096f9ae069c3'
CURRENT_MAP_SHA = '05b5d227f05b63e98350c561a298a3d1362644a2cda7430ce6b029bd078b900d'
CONTROL = 'frontend::project::reviewer_unit1::current_lexer_storage_failure_full_span_and_eof'
ORIGINAL_NAMES = (
    'README.md', 'check_resource_inventory.py', 'expectation-provenance.json',
    'expected-fixtures.json', 'expected-resources.json', 'expected-test-roster.json',
    'generate_reviewer_tests.py', 'materialize_fixtures.py',
    'materialize_resource_fixtures.py', 'origin_oracle.py',
    'reviewer_additional.rs', 'reviewer_v2.rs', 'run.py', 'run_reviewer.py',
    'rust_strings.py', 'scalar_schedule_heldout.rs',
)


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def binding(name, body):
    return {'path': name, 'bytes': len(body), 'sha256': sha(body)}


def original_package():
    return REPO / 'tests/fixtures/typed_project_unit1_independent'


def regular(path):
    need(not path.is_symlink(), 'Symlink in Unit1 input: ' + str(path))
    mode = path.stat().st_mode
    need(stat.S_ISREG(mode) and not mode & 0o111,
         'Unit1 input must be a regular nonexecutable file: ' + str(path))
    return path.read_bytes()


def load_module(name, path, body=None):
    # Execute the exact authenticated bytes, never an existing bytecode cache.
    module = types.ModuleType(name)
    module.__file__ = str(path)
    raw = regular(path) if body is None else body
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


def authenticate(package=None, current_root=None):
    """Check complete frozen and derived bytes before staging or executing helpers."""
    package = original_package() if package is None else Path(package)
    root = ROOT if current_root is None else Path(current_root)
    original_map = regular(root / 'unit1_original_package.json')
    current_map = regular(root / 'unit1_current_bindings.json')
    need(sha(original_map) == ORIGINAL_MAP_SHA, 'Changed complete frozen Unit1 map')
    need(sha(current_map) == CURRENT_MAP_SHA, 'Changed current Unit1 body bindings')
    old = json.loads(original_map)
    new = json.loads(current_map)
    need(old['compiler_checkpoint'] == new['compiler_checkpoint'] == CHECKPOINT,
         'Wrong Unit1 compiler checkpoint')
    need(new['original_package'] == binding('unit1_original_package.json', original_map),
         'Current map does not bind complete original Unit1 package')
    need(not package.is_symlink() and package.is_dir(), 'Invalid original Unit1 directory')
    need(sorted(p.name for p in package.iterdir()) == sorted(ORIGINAL_NAMES),
         'Missing or extra original Unit1 package member')
    need([row['path'] for row in old['files']] == sorted(ORIGINAL_NAMES),
         'Wrong original Unit1 package map roster')
    originals = {}
    for row in old['files']:
        name = row['path']
        body = regular(package / name)
        blob = hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest()
        need(row == {**binding(name, body), 'mode': '100644', 'git_blob': blob},
             'Changed original Unit1 complete body: ' + name)
        originals[name] = body
    derived = {}
    names = [row['path'] for row in new['files']]
    need(names == sorted(set(names)), 'Duplicate or unordered Unit1 current bodies')
    for row in new['files']:
        body = regular(root / row['path'])
        need(row == binding(row['path'], body), 'Changed current Unit1 complete body: ' + row['path'])
        derived[row['path']] = body
    adapter = load_module('_unit1_current_adapter', root / 'adapters.py', derived['adapters.py'])
    recipe = load_module('_unit1_current_derivation', root / 'unit1_derivation.py', derived['unit1_derivation.py'])
    adapted, receipt = adapter.unit1_counter_domain(originals['reviewer_additional.rs'])
    receipt = adapter.unit1_correspondence(originals['expected-test-roster.json'], receipt)
    need(adapted == derived['unit1_reviewer_additional.current.rs'],
         'Unapproved adapted reviewer complete body')
    need(adapter.inverse_exact(adapted, adapter.UNIT1_SEAMS) == originals['reviewer_additional.rs'],
         'Adapted reviewer does not recover complete original body')
    full_receipt = json.loads(derived['unit1_correspondence.json'])
    need(all(full_receipt.get(key) == value for key, value in receipt.items()),
         'Original69 correspondence receipt mismatch')
    need(full_receipt['current_controls'] == {
        'count': 1, 'names': [CONTROL],
        'source': binding('unit1_lexer_controls.rs', derived['unit1_lexer_controls.rs'])},
        'New control overlaps or changes original69 accounting')
    control_names = re.findall(rb'#\[test\]\s*fn ([A-Za-z0-9_]+)\(', derived['unit1_lexer_controls.rs'])
    need(control_names == [CONTROL.rsplit('::', 1)[1].encode()], 'Wrong current control roster')
    runner_rows = []
    for original, generated, seams in (
        ('run.py', 'unit1_run_current.py', recipe.RUNNER_SEAMS),
        ('run_reviewer.py', 'unit1_run_reviewer_current.py', recipe.REVIEWER_SEAMS),
    ):
        need(recipe.transform(originals[original], seams) == derived[generated],
             'Current Unit1 runner differs from exact derivation: ' + generated)
        inverse = recipe.inverse(derived[generated], seams)
        need(inverse == originals[original], 'Current runner does not recover original: ' + original)
        runner_rows.append({'original': binding(original, originals[original]),
                            'derived': binding(generated, derived[generated]),
                            'inverse_sha256': sha(inverse),
                            'exact_single_occurrence_seams': len(seams)})
    need(full_receipt['runner_correspondence'] == runner_rows, 'Runner correspondence mismatch')
    need(full_receipt['profiles'] == ['debug', 'release'] and
         full_receipt['original_cases_per_profile'] == 69 and
         full_receipt['current_controls_per_profile'] == 1 and
         full_receipt['execution_qualified'] is False,
         'Incorrect source-only Unit1 accounting')
    return {'originals': originals, 'derived': derived, 'correspondence': full_receipt}


def rust_string_literal(value):
    authenticated = authenticate()
    namespace = {}
    exec(compile(authenticated['originals']['rust_strings.py'],
                 str(original_package() / 'rust_strings.py'), 'exec'), namespace)
    return namespace['rust_string_literal'](value)


def admit(repo):
    """Use the approved active public dispatcher; never restore old executable inputs."""
    repo = Path(repo).resolve()
    # The candidate repository is data. Import only our trusted public dispatcher.
    package = REPO / 'tests/fixtures/typed_project_source_binding'
    dispatcher_raw = regular(package / 'run.py')
    need(sha(dispatcher_raw) == DISPATCHER_SHA, 'Changed trusted current source dispatcher')
    need(sha(regular(package / 'package-manifest.json')) == SOURCE_PACKAGE_SHA,
         'Changed trusted current source-binding package manifest')
    dispatcher = load_module('_unit1_current_source_binding', package / 'run.py', dispatcher_raw)
    captured = dispatcher.preflight(repo, package=package)
    need('lexer_reservation_authority' in captured and 'lexer_reservation_touched' in captured,
         'Current Unit1 requires the active fallible-lexer outer admission')
    raw = captured['package_bytes']['current-source.json']
    need((len(raw), sha(raw)) == (CURRENT_SOURCE_BYTES, CURRENT_SOURCE_SHA),
         'Unit1 source admission is not the independently frozen current manifest')
    need(captured['current']['reviewed_source_head'] == CHECKPOINT,
         'Unit1 source admission has the wrong compiler head')
    rows = [binding(name, body) for name, body in sorted(captured['inputs'].items())]
    need(rows == captured['current']['files'], 'Current Unit1 input map differs from manifest')
    return {'schema': 'oxid-unit1-lexer-source-admission-v1',
            'compiler_checkpoint': CHECKPOINT,
            'current_source': binding('current-source.json', raw),
            'inputs': rows,
            'lexer_reservation_touched': captured['lexer_reservation_touched']}


def check_admission(repo, expected):
    need(admit(repo) == expected, 'Current Unit1 source admission changed during execution')


def stage(q):
    authenticated = authenticate()
    q = Path(q)
    staged_names = {p.name for p in q.iterdir()}
    need(set(ORIGINAL_NAMES) <= staged_names <= set(ORIGINAL_NAMES) | {'input-source-manifest.json'},
         'Stage must start from the complete frozen Unit1 package')
    for name, body in authenticated['originals'].items():
        need(regular(q / name) == body, 'Changed frozen staged helper before adaptation: ' + name)
    (q / 'reviewer_additional.rs').write_bytes(authenticated['derived']['unit1_reviewer_additional.current.rs'])
    for name in ('unit1_run_reviewer_current.py', 'unit1_lexer_controls.rs', 'unit1_correspondence.json'):
        (q / name).write_bytes(authenticated['derived'][name])
    check_stage(q, authenticated)


def check_stage(q, authenticated=None):
    authenticated = authenticate() if authenticated is None else authenticated
    q = Path(q)
    for name, original in authenticated['originals'].items():
        expected = authenticated['derived']['unit1_reviewer_additional.current.rs'] if name == 'reviewer_additional.rs' else original
        need(regular(q / name) == expected, 'Changed staged Unit1 helper or fixture: ' + name)
    for name in ('unit1_run_reviewer_current.py', 'unit1_lexer_controls.rs', 'unit1_correspondence.json'):
        need(regular(q / name) == authenticated['derived'][name], 'Changed current Unit1 stage body: ' + name)


def include_control(test):
    original = test.read_bytes()
    need(b'unit1_lexer_controls.rs' not in original, 'Current Unit1 control already included')
    test.write_bytes(original + b'\ninclude!("unit1_lexer_controls.rs");\n')


def profile_guard(repo, admission, source, compiled_sources, q):
    """Before/after source and roster admission supplements all retained guards."""
    check_admission(repo, admission)
    check_stage(q)
    for name, expected in compiled_sources.items():
        need(sha((source / name).read_bytes()) == expected, 'Compiled Unit1 source changed: ' + name)


def check_profile_results(results):
    need(results['schema'] == 'oxid-unit1-lexer-current-results-v1' and results['status'] == 'passed',
         'Wrong or failed current Unit1 result')
    need((results['total'], results['passed'], results['expected']) == (69, 69, 69),
         'Original69 Unit1 executions are incomplete')
    controls = results['current_controls']
    need((controls['total'], controls['passed'], controls['expected']) == (1, 1, 1),
         'The separate current Unit1 control did not pass exactly once')
    need([row['test'] for row in controls['tests']] == [CONTROL] and
         all(row['passed'] and row['one_named_test_verified'] for row in controls['tests']),
         'Current Unit1 control identity or execution proof changed')
    original_names = json.loads(authenticate()['originals']['expected-test-roster.json'])['tests']
    need(sorted(row['test'] for row in results['tests']) == sorted(original_names) and
         all(row['passed'] and row['one_named_test_verified'] for row in results['tests']),
         'Original69 Unit1 result identities or execution proofs changed')
    need((results['all_total'], results['all_passed'], results['all_expected']) == (70, 70, 70),
         'Combined accounting does not preserve original69 plus new1')


def summary_fields(admission):
    return {'schema': 'oxid-unit1-lexer-current-run-v1',
            'current_control_test_functions_per_profile': 1,
            'current_control_test_functions_executed': 0,
            'current_source_admission': admission,
            'unit1_correspondence_sha256': sha((ROOT / 'unit1_correspondence.json').read_bytes()),
            'original_package_manifest_sha256': ORIGINAL_MAP_SHA,
            'current_unit1_bindings_sha256': CURRENT_MAP_SHA}
