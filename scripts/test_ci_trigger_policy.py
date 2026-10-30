"""Keep CI entry points explicit without adding a YAML dependency.

These source-contract checks intentionally follow the workflows' block style;
actionlint remains the YAML and GitHub Actions syntax validator.
"""
from pathlib import Path
import ast
import hashlib
import re
import shlex
import shutil
import subprocess
import tempfile
import textwrap
import unittest


REPO = Path(__file__).resolve().parents[1]


def block(source, key, indent=0):
    """Return exactly one block at the requested indentation level."""
    prefix = ' ' * indent
    matches = re.findall(
        rf'^{prefix}{re.escape(key)}:(.*?)(?=^\S|^ {{1,{indent}}}\S|\Z)'
        if indent else rf'^{re.escape(key)}:(.*?)(?=^\S|\Z)',
        source, re.MULTILINE | re.DOTALL,
    )
    if len(matches) != 1:
        raise AssertionError(f'expected one {key} block, found {len(matches)}')
    return matches[0].strip()


SCALAR_REPLAY = ('scripts/verify_scalar_library_compatibility.py',
                 (63079, '29c1d76604adfeddf2006213922ceb59947f7a193454ffb542df2ba0f7597119'))
REGISTRY_REPLAY = ('scripts/verify_registry_compatibility.py',
                   (35050, 'd6bd23496f38bde3d89e7e97a2d724b73a0f233cfef99a6dc9fad752bdec9766'))
MSVC_REPLAY = 'scripts/verify_msvc_linker_compatibility.py'
OWNED_VECTOR_REPLAY = 'scripts/verify_owned_record_vector_compatibility.py'
OWNED_VECTOR_POLICY = (OWNED_VECTOR_REPLAY,
                       (110006, '16c7affd778deda3d61f9a4e4aebd66c9e01d842c6afe3e9c1a20de57caab6e0'))
OWNED_TEXT_REPLAY = 'scripts/verify_owned_text_compatibility.py'
OWNED_TEXT_POLICY = (OWNED_TEXT_REPLAY,
                     (16603, '61bf3bf55b9c1f958fdf48fcc89e747cdf61700109ab979b098851249d9fa109'))
OFFLINE_VENDOR_REPLAY = 'scripts/verify_offline_vendor_compatibility.py'
OFFLINE_VENDOR_PARENT = '7279208226a3d4185c0b996491332519971e916e'
OFFLINE_VENDOR_POLICY = (OFFLINE_VENDOR_REPLAY,
                         (27286, 'ee64cc7f63051764594a5866101cff9987ca7ddf20b764e7e1aa88f975c9b7a1'))
OWNED_ENUM_VECTOR_REPLAY = 'scripts/verify_owned_enum_vector_compatibility.py'
OWNED_ENUM_VECTOR_PARENT = '5917f581e69c0a0c69e4b5b059bb97dced55331d'
OWNED_ENUM_VECTOR_OBSERVER = ('scripts/owned_enum_vector_compatibility_observer.py',
    (7495, 'ff932fb13ae4db32fe15a389505be7a7ffa2fc0557b15f9b1b153c3602de3fd9'))
GENERIC_OWNED_VECTOR_REPLAY = 'scripts/verify_generic_owned_vector_compatibility.py'
GENERIC_OWNED_VECTOR_PARENT = '698043182916dda7276c363e20a98dfbb5b22ed3'
WINDOWS_LOCAL_PACKAGE_REPLAY = 'scripts/verify_windows_local_package_compatibility.py'
WINDOWS_LOCAL_PACKAGE_PARENT = '1ed9a4b9c373b62d115f31ef9abf1b84debd2e3e'
WINDOWS_LOCAL_PACKAGE_REVIEWED_HEAD = '0d72e4f621534a4eaf6e5495d0719e0400615673'
GENERIC_OWNED_VECTOR_POLICY = (GENERIC_OWNED_VECTOR_REPLAY,
    (43357, '8e80cbdb0b42e01dd4e125d36215dc94ff37422a0e10786216ec02ad66831ad0'))
OWNED_ENUM_VECTOR_POLICY = (OWNED_ENUM_VECTOR_REPLAY,
    (67623, '3505f8238f1d24754916075fb1ad46aa022c740cd71c9f10a5509656b7e9877c'))
REPLAY_ROUTES = {
    (SCALAR_REPLAY[0],): ('direct', SCALAR_REPLAY),
    (REGISTRY_REPLAY[0],): ('direct', REGISTRY_REPLAY),
    (MSVC_REPLAY, '--source-policy', 'scalar-library'): ('msvc', SCALAR_REPLAY),
    (MSVC_REPLAY, '--source-policy', 'registry'): ('msvc', REGISTRY_REPLAY),
    (OWNED_VECTOR_REPLAY,): ('owned-vector', REGISTRY_REPLAY),
    (OWNED_TEXT_REPLAY,): ('owned-text', OWNED_VECTOR_POLICY),
    (OFFLINE_VENDOR_REPLAY,): ('offline-vendor', OWNED_TEXT_POLICY),
    (OWNED_ENUM_VECTOR_REPLAY,): ('owned-enum-vector', OFFLINE_VENDOR_POLICY),
    (GENERIC_OWNED_VECTOR_REPLAY,): ('generic-owned-vector', OWNED_ENUM_VECTOR_POLICY),
    (WINDOWS_LOCAL_PACKAGE_REPLAY,): ('windows-local-package', GENERIC_OWNED_VECTOR_POLICY),
}


def retained_replay_route(current):
    """Admit one complete mandatory command, never a filename in a comment."""
    if re.search(r'(?m)^    if:|continue-on-error:', current):
        raise AssertionError('retained replay must be mandatory')
    checkout = ('      - uses: actions/checkout@v4\n'
                '        with:\n'
                '          ref: ${{ github.event.pull_request.head.sha || github.sha }}\n'
                '          fetch-depth: 0\n')
    if current.count('actions/checkout@') != 1 or checkout not in current:
        raise AssertionError('retained replay must use the current head and complete history')
    steps = re.findall(
        r'^      - name: Run retained semantic and resource assertions on current sources\n'
        r'(.*?)(?=^      - |\Z)', current, re.MULTILINE | re.DOTALL)
    if len(steps) != 1:
        raise AssertionError('expected exactly one retained replay step')
    prefix = ('        env:\n'
              '          EXPECTED_HEAD: ${{ github.event.pull_request.head.sha || github.sha }}\n'
              '        run: |\n')
    if not steps[0].startswith(prefix):
        raise AssertionError('retained replay step metadata changed')
    lines = steps[0][len(prefix):].splitlines()
    tail = ['            --repo . \\',
            '            --expected-head "$EXPECTED_HEAD" \\',
            '            --output "$RUNNER_TEMP/current-project-unit2"']
    if (len(lines) != 4 or lines[1:] != tail
            or not lines[0].startswith('          python3 -B ')
            or not lines[0].endswith(' \\')):
        raise AssertionError('retained replay command or failure propagation changed')
    try:
        command = tuple(shlex.split(lines[0][10:-2]))
    except ValueError as error:
        raise AssertionError('invalid retained replay command') from error
    route = command[2:]
    if command[:2] != ('python3', '-B') or route not in REPLAY_ROUTES:
        raise AssertionError('unknown retained replay route')
    return route


def _same_statement(node, source):
    return ast.dump(node) == ast.dump(ast.parse(source).body[0])


def _assignment(body, name):
    rows = [node for node in body if isinstance(node, ast.Assign)
            and any(isinstance(target, ast.Name) and target.id == name for target in node.targets)]
    if len(rows) != 1:
        raise AssertionError('expected one retained binding: ' + name)
    return rows[0]


def _literal(body, name):
    return ast.literal_eval(_assignment(body, name).value)


def _function(body, name):
    rows = [node for node in body if isinstance(node, ast.FunctionDef) and node.name == name]
    if len(rows) != 1:
        raise AssertionError('expected one retained function: ' + name)
    return rows[0]


def _require_statement(body, source):
    if sum(_same_statement(node, source) for node in body) != 1:
        raise AssertionError('retained runner binding changed: ' + source)


def _retained_delegate(tree, context, invocation, guard_inside=False):
    """Require the original call and its immediate failure guard on the live path."""
    run = _function(tree.body, 'run')
    attempts = [node for node in run.body if isinstance(node, ast.Try)]
    if len(attempts) != 1 or run.body[-1] is not attempts[0]:
        raise AssertionError('retained replay exception boundary changed')
    attempt = attempts[0]
    calls = [node for node in attempt.body if isinstance(node, ast.With)]
    if len(calls) != 1:
        raise AssertionError('expected one unconditional retained replay call')
    call = calls[0]
    expected = ast.parse('with ' + context + ':\n    status = ' + invocation).body[0]
    if (len(call.items) != 1 or ast.dump(call.items[0]) != ast.dump(expected.items[0])
            or not call.body or ast.dump(call.body[0]) != ast.dump(expected.body[0])):
        raise AssertionError('retained replay call changed')
    if guard_inside:
        guard = call.body[1] if len(call.body) > 1 else None
    else:
        if len(call.body) != 1:
            raise AssertionError('retained replay call scope changed')
        index = attempt.body.index(call)
        guard = attempt.body[index + 1] if index + 1 < len(attempt.body) else None
    if not (isinstance(guard, ast.Expr) and isinstance(guard.value, ast.Call)
            and isinstance(guard.value.func, ast.Name) and guard.value.func.id == 'require'
            and guard.value.args
            and ast.dump(guard.value.args[0]) == ast.dump(ast.parse('status == 0', mode='eval').body)):
        raise AssertionError('retained replay failure guard changed')


def _retained_bound(body, expected):
    bindings = _assignment(body, 'bindings').value
    if ast.dump(bindings) != ast.dump(ast.parse(expected, mode='eval').body):
        raise AssertionError('retained binding scope changed')


def _require_condition(body, condition):
    expected = ast.dump(ast.parse(condition, mode='eval').body)
    guards = [node for node in body if isinstance(node, ast.Expr)
              and isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name)
              and node.value.func.id == 'require' and node.value.args
              and ast.dump(node.value.args[0]) == expected]
    if len(guards) != 1:
        raise AssertionError('owned-text retained admission condition changed: ' + condition)


def _generic_vector_replay_binding(tree, read):
    for name, expected in (
        ('PREDECESSOR', GENERIC_OWNED_VECTOR_PARENT),
        ('PREDECESSOR_POLICY', OWNED_ENUM_VECTOR_POLICY[0]),
        ('PREDECESSOR_POLICY_PIN', OWNED_ENUM_VECTOR_POLICY[1]),
        ('OBSERVER_ADAPTER', OWNED_ENUM_VECTOR_OBSERVER[0]),
        ('OBSERVER_ADAPTER_PIN', OWNED_ENUM_VECTOR_OBSERVER[1]),
        ('OBSERVER_DERIVED_PIN', (34582, '73b30bca73d10126977744d50fb4cc42f6dc1cb876c5ee82b7335f3ffc14a3d3')),
        ('PUBLIC_TEST', 'tests/typed_owned_vector_wrappers.rs'),
    ):
        if _literal(tree.body, name) != expected:
            raise AssertionError('generic-vector retained parent/observer changed: ' + name)
    require_retained_replay_binding((OWNED_ENUM_VECTOR_REPLAY,), read)
    for statement in (
        'predecessor = load_predecessor(ROOT)', 'library = predecessor.library',
        'owned_policy = predecessor.owned_policy', 'RETAINED_RUN = predecessor.RETAINED_RUN',
        'PREDECESSOR_ADDED = predecessor.PARENT_ADDED',
        'PARENT_ADDED = PREDECESSOR_ADDED | GENERIC_ADDED | frozenset(SOURCE_ASSETS) | {PUBLIC_TEST}',
    ):
        _require_statement(tree.body, statement)
    inventories = {}
    for name, expected in (
        ('GENERIC_CHANGED', {
            'src/frontend/declaration_index/generics.rs',
            'src/frontend/declaration_index/generics/nominal.rs',
            'src/frontend/declaration_index/generics/owned_candidate.rs',
            'src/frontend/declaration_index/generics/owned_builtin_enum_key_tests.rs',
            'src/frontend/declaration_index/generics/vector_tests.rs',
            'src/frontend/oir/owned/source/program.rs', 'src/frontend/owned_vector_builtins.rs'}),
        ('GENERIC_ADDED', {
            'src/frontend/declaration_index/generics/owned_parameter_tests.rs',
            'src/frontend/oir/owned/source/owned_parameter_cases.rs',
            'src/frontend/oir/owned/source/owned_parameter_source_tests.rs'}),
    ):
        node = _assignment(tree.body, name).value
        if (not isinstance(node, ast.Call) or not isinstance(node.func, ast.Name)
                or node.func.id != 'frozenset' or len(node.args) != 1 or node.keywords):
            raise AssertionError('generic-vector source roster must be literal')
        values = ast.literal_eval(node.args[0])
        if not isinstance(values, tuple) or len(values) != len(expected) or set(values) != expected:
            raise AssertionError('generic-vector finite source roster changed')
        inventories[name] = set(values)
    expected_inputs = inventories['GENERIC_CHANGED'] | inventories['GENERIC_ADDED'] | {
        'tests/typed_owned_vector_wrappers.rs'}
    pins = _literal(tree.body, 'REVIEWED_INPUTS')
    assets = _literal(tree.body, 'SOURCE_ASSETS')
    if (set(pins) != expected_inputs or len(pins) != 11
            or set(assets) != {'packages/owned_vec/src/lib.ox', 'packages/owned_vec/oxid.toml',
                               'examples/owned-vector-results/src/main.ox'}
            or any(not isinstance(pin, tuple) or len(pin) != 2 or type(pin[0]) is not int or pin[0] <= 0
                   or not isinstance(pin[1], str) or re.fullmatch('[a-f0-9]{64}', pin[1]) is None
                   for pin in (*pins.values(), *assets.values()))):
        raise AssertionError('generic-vector literal source/asset pins changed')
    if set(_literal(tree.body, 'REVIEWED_DIRECTIVES')) != expected_inputs:
        raise AssertionError('generic-vector literal directive membership changed')
    if _literal(tree.body, 'SOURCE_ASSET_INCLUDE_BINDINGS') != {
        'packages/owned_vec/src/lib.ox': ('src/frontend/oir/owned/source/owned_parameter_cases.rs',
                                        '../../../../../packages/owned_vec/src/lib.ox', 1),
        'examples/owned-vector-results/src/main.ox': ('src/frontend/oir/owned/source/owned_parameter_cases.rs',
                                                   '../../../../../examples/owned-vector-results/src/main.ox', 1),
        'packages/owned_vec/oxid.toml': ('tests/typed_owned_vector_wrappers.rs', '../packages/owned_vec/oxid.toml', 1),
    }:
        raise AssertionError('generic-vector exact include binding changed')
    captured = _function(tree.body, 'capture_sources').body
    _require_statement(captured, 'expected_parent = set(baseline) | PREDECESSOR_ADDED')
    _require_statement(captured, 'expected = expected_parent | GENERIC_ADDED | set(SOURCE_ASSETS) | {PUBLIC_TEST}')
    _require_condition(captured, 'set(parent) == expected_parent and len(parent) == 587')
    _require_condition(captured, 'len(GENERIC_CHANGED) == 7 and len(GENERIC_ADDED) == 3 '
                       'and GENERIC_CHANGED <= expected_parent and not GENERIC_ADDED & expected_parent '
                       'and not set(SOURCE_ASSETS) & expected_parent and PUBLIC_TEST not in expected_parent '
                       'and len(expected) == 594')
    _require_condition(captured, 'set(REVIEWED_INPUTS) == GENERIC_CHANGED | GENERIC_ADDED | {PUBLIC_TEST} '
                       'and set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS)')
    _require_statement(captured, 'validate_source_assets(inputs)')
    bound = _function(tree.body, 'bound_predecessor')
    _retained_bound(bound.body,
                    "{'PREDECESSOR': PREDECESSOR, 'PREDECESSOR_ADDED': PREDECESSOR_ADDED, "
                    "'PREDECESSOR_POLICIES': PREDECESSOR_POLICIES, 'PINS_PROVISIONAL': PINS_PROVISIONAL, "
                    "'capture_sources': capture_sources, 'current_manifest': current_manifest}")
    _require_condition(bound.body, 'library.run is RETAINED_RUN and predecessor.RETAINED_RUN is RETAINED_RUN '
                       'and owned_policy.RETAINED_RUN is RETAINED_RUN')
    _require_condition(bound.body, 'predecessor.OBSERVER_ADAPTER == OBSERVER_ADAPTER '
                       'and predecessor.OBSERVER_ADAPTER_PIN == OBSERVER_ADAPTER_PIN')
    scopes = [node for node in bound.body if isinstance(node, ast.With)]
    expected = ast.parse('with predecessor.bound_predecessor() as context:\n    pass').body[0]
    if len(scopes) != 1 or [ast.dump(item) for item in scopes[0].items] != [ast.dump(item) for item in expected.items]:
        raise AssertionError('generic-vector source/observer delegation scope changed')
    restores = [node for node in scopes[0].body if isinstance(node, ast.Try)]
    if len(restores) != 1:
        raise AssertionError('generic-vector source restoration boundary changed')
    _require_statement(restores[0].body, 'for name, value in bindings.items():\n    setattr(library, name, value)')
    _require_condition(restores[0].body, 'context["adapter"].DERIVED_PIN == OBSERVER_DERIVED_PIN')
    _require_statement(restores[0].finalbody, 'for name, value in before.items():\n    setattr(library, name, value)')
    _retained_delegate(tree, 'bound_predecessor() as context', 'RETAINED_RUN(delegated)', guard_inside=True)
    run = _function(tree.body, 'run')
    attempt = next(node for node in run.body if isinstance(node, ast.Try))
    for condition in ('args.prepare_only or not PINS_PROVISIONAL',
                      'args.prepare_only or not OBSERVER_PINS_PROVISIONAL',
                      'args.prepare_only or not predecessor.PINS_PROVISIONAL'):
        _require_condition(attempt.body, condition)
    scope = next(node for node in attempt.body if isinstance(node, ast.With))
    controls = [node for node in scope.body if isinstance(node, ast.If)
                and ast.dump(node.test) == ast.dump(ast.parse('not args.prepare_only', mode='eval').body)]
    pipeline = ast.parse(
        'report.update(owned_policy.run_owned_vector_observer_controls('
        'repo, delegated.output, context["selected"], context["seam"], context["original"], '
        'context["adapter"], retained["test_function_executions"]))\n'
        'report.update(predecessor.run_enum_observer_controls('
        'repo, delegated.output, context["selected"], context["seam"], context["original"], '
        'context["adapter"], report["test_function_executions"]))').body
    if len(controls) != 1 or [ast.dump(node) for node in controls[0].body] != [ast.dump(node) for node in pipeline]:
        raise AssertionError('generic-vector inherited controls pipeline changed')
    manifest = _function(tree.body, 'current_manifest').body
    _require_statement(manifest, 'inherited = predecessor.current_manifest(inputs=parent, baseline=baseline, '
                       'checkout=checkout, parent={name: parent[name] for name in previous_names})')
    flags = _assignment(manifest, 'result').value
    false_names = {'generic_owned_vector_semantics_qualified_by_this_gate',
                   'generic_owned_vector_allocator_failures_qualified_by_this_gate',
                   'generic_owned_vector_platform_qualified_by_this_gate',
                   'general_heap_collections_qualified_by_this_gate', 'g2_complete_qualified_by_this_gate'}
    found = {key.value: value for key, value in zip(flags.keys, flags.values)
             if isinstance(key, ast.Constant) and isinstance(key.value, str) and key.value.endswith('qualified_by_this_gate')}
    if set(found) != false_names or any(not isinstance(value, ast.Constant) or value.value is not False for value in found.values()):
        raise AssertionError('generic-vector source admission cannot claim new feature qualification')
    _require_condition(manifest, 'all(value is False for name, value in result.items() '
                       'if name.endswith("qualified_by_this_gate"))')
    report = _assignment(run.body, 'report').value
    for name in ('semantic_pass', 'new_feature_qualification', 'platform_qualification'):
        values = [value for key, value in zip(report.keys, report.values)
                  if isinstance(key, ast.Constant) and key.value == name]
        if len(values) != 1 or not isinstance(values[0], ast.Constant) or values[0].value is not False:
            raise AssertionError('generic-vector preflight qualification claim changed')


def _windows_local_package_replay_binding(tree, read):
    for name, expected in (
        ('PREDECESSOR', WINDOWS_LOCAL_PACKAGE_PARENT),
        ('PREDECESSOR_POLICY', GENERIC_OWNED_VECTOR_POLICY[0]),
        ('PREDECESSOR_POLICY_PIN', GENERIC_OWNED_VECTOR_POLICY[1]),
        ('REVIEWED_SOURCE_HEAD', WINDOWS_LOCAL_PACKAGE_REVIEWED_HEAD),
        ('OBSERVER_PINS_PROVISIONAL', False),
        ('OBSERVER_ADAPTER', OWNED_ENUM_VECTOR_OBSERVER[0]),
        ('OBSERVER_ADAPTER_PIN', OWNED_ENUM_VECTOR_OBSERVER[1]),
        ('OBSERVER_DERIVED_PIN', (34582, '73b30bca73d10126977744d50fb4cc42f6dc1cb876c5ee82b7335f3ffc14a3d3')),
    ):
        if _literal(tree.body, name) != expected:
            raise AssertionError('Windows-package retained parent/source/observer changed: ' + name)
    if type(_literal(tree.body, 'PINS_PROVISIONAL')) is not bool:
        raise AssertionError('Windows-package preparation status must be explicit')
    require_retained_replay_binding((GENERIC_OWNED_VECTOR_REPLAY,), read)
    for statement in (
        'predecessor = load_predecessor(ROOT)', 'library = predecessor.library',
        'owned_policy = predecessor.owned_policy', 'enum_policy = predecessor.predecessor',
        'RETAINED_RUN = predecessor.RETAINED_RUN', 'PREDECESSOR_ADDED = predecessor.PARENT_ADDED',
        'PARENT_ADDED = PREDECESSOR_ADDED | WINDOWS_ADDED',
    ):
        _require_statement(tree.body, statement)
    inventories = {}
    for name, expected in (
        ('WINDOWS_CHANGED', {
            'src/frontend/driver.rs', 'src/frontend/mod.rs', 'src/frontend/native/doctor_windows.rs',
            'src/frontend/oir/owned/source/owned_text_source_resource_tests.rs',
            'src/frontend/oir/source/owned_vector_route_tests.rs',
            'src/frontend/project.rs', 'src/frontend/project/editor_sources.rs',
            'src/frontend/project/filesystem.rs', 'src/frontend/project/filesystem/windows.rs',
            'src/frontend/project/packages.rs', 'src/frontend/project/packages/tests.rs',
            'src/main.rs', 'src/project_cli/workspace.rs', 'src/project_lock.rs'}),
        ('WINDOWS_ADDED', {
            'tests/hosted_execution.rs', 'tests/owned_enum_payloads.rs',
            'tests/typed_documentation.rs', 'tests/typed_generics.rs',
            'tests/typed_language_server.rs', 'tests/typed_package_lock.rs',
            'tests/typed_path_dependencies.rs', 'tests/typed_windows_modules.rs',
            'tests/typed_workspace.rs'}),
    ):
        node = _assignment(tree.body, name).value
        if (not isinstance(node, ast.Call) or not isinstance(node.func, ast.Name)
                or node.func.id != 'frozenset' or len(node.args) != 1 or node.keywords):
            raise AssertionError('Windows-package source roster must be literal')
        values = ast.literal_eval(node.args[0])
        if not isinstance(values, tuple) or len(values) != len(expected) or set(values) != expected:
            raise AssertionError('Windows-package finite source roster changed')
        inventories[name] = set(values)
    expected_inputs = inventories['WINDOWS_CHANGED'] | inventories['WINDOWS_ADDED']
    pins = _literal(tree.body, 'REVIEWED_INPUTS')
    if (inventories['WINDOWS_CHANGED'] & inventories['WINDOWS_ADDED']
            or set(pins) != expected_inputs or len(pins) != 23
            or any(not isinstance(pin, tuple) or len(pin) != 2 or type(pin[0]) is not int or pin[0] <= 0
                   or not isinstance(pin[1], str) or re.fullmatch('[a-f0-9]{64}', pin[1]) is None
                   for pin in pins.values())):
        raise AssertionError('Windows-package literal source pins changed')
    if set(_literal(tree.body, 'REVIEWED_DIRECTIVES')) != expected_inputs:
        raise AssertionError('Windows-package literal directive membership changed')
    for name, pin in pins.items():
        body = read(name)
        if (len(body), hashlib.sha256(body).hexdigest()) != pin:
            raise AssertionError('Windows-package compiled input differs from source pin: ' + name)
    loader = _function(tree.body, 'load_predecessor').body
    _require_condition(loader, 'not path.is_symlink() and not any(parent.is_symlink() for parent in path.parents)')
    _require_condition(loader, 'stat.S_ISREG(before.st_mode) and before.st_nlink == 1 '
                       'and not before.st_mode & 0o111 and before.st_size == PREDECESSOR_POLICY_PIN[0]')
    _require_condition(loader, 'tuple(getattr(before, name) for name in fields) '
                       '== tuple(getattr(path.stat(), name) for name in fields) '
                       'and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN')
    _require_statement(loader, 'exec(compile(body, str(path), "exec"), module.__dict__)')
    captured = _function(tree.body, 'capture_sources').body
    _require_statement(captured, 'expected_parent = set(baseline) | PREDECESSOR_ADDED')
    _require_statement(captured, 'expected = expected_parent | WINDOWS_ADDED')
    _require_condition(captured, 'set(parent) == expected_parent and len(parent) == 594')
    _require_condition(captured, 'len(WINDOWS_CHANGED) == 14 and len(WINDOWS_ADDED) == 9 '
                       'and WINDOWS_CHANGED <= expected_parent and not WINDOWS_ADDED & expected_parent '
                       'and len(expected) == 603')
    _require_condition(captured, 'set(REVIEWED_INPUTS) == WINDOWS_CHANGED | WINDOWS_ADDED '
                       'and set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS)')
    _require_condition(captured, 'actual == compiler and len(compiler) == 500')
    _require_condition(captured, 'directories == expected_dirs')
    bound = _function(tree.body, 'bound_predecessor')
    _retained_bound(bound.body,
                    "{'PREDECESSOR': PREDECESSOR, 'PREDECESSOR_ADDED': PREDECESSOR_ADDED, "
                    "'PREDECESSOR_POLICIES': PREDECESSOR_POLICIES, 'PINS_PROVISIONAL': PINS_PROVISIONAL, "
                    "'capture_sources': capture_sources, 'current_manifest': current_manifest}")
    _require_condition(bound.body, 'library.run is RETAINED_RUN and predecessor.RETAINED_RUN is RETAINED_RUN '
                       'and owned_policy.RETAINED_RUN is RETAINED_RUN and enum_policy.RETAINED_RUN is RETAINED_RUN')
    _require_condition(bound.body, 'predecessor.OBSERVER_ADAPTER == OBSERVER_ADAPTER '
                       'and predecessor.OBSERVER_ADAPTER_PIN == OBSERVER_ADAPTER_PIN')
    scopes = [node for node in bound.body if isinstance(node, ast.With)]
    expected = ast.parse('with predecessor.bound_predecessor() as context:\n    pass').body[0]
    if len(scopes) != 1 or [ast.dump(item) for item in scopes[0].items] != [ast.dump(item) for item in expected.items]:
        raise AssertionError('Windows-package source/observer delegation scope changed')
    restores = [node for node in scopes[0].body if isinstance(node, ast.Try)]
    if len(restores) != 1:
        raise AssertionError('Windows-package source restoration boundary changed')
    _require_statement(restores[0].body, 'for name, value in bindings.items():\n    setattr(library, name, value)')
    _require_condition(restores[0].body, 'context["adapter"].DERIVED_PIN == OBSERVER_DERIVED_PIN')
    _require_condition(restores[0].body, 'library.run is RETAINED_RUN')
    _require_statement(restores[0].finalbody, 'for name, value in before.items():\n    setattr(library, name, value)')
    _retained_delegate(tree, 'bound_predecessor() as context', 'RETAINED_RUN(delegated)', guard_inside=True)
    run = _function(tree.body, 'run')
    attempt = next(node for node in run.body if isinstance(node, ast.Try))
    for condition in ('args.prepare_only or not PINS_PROVISIONAL',
                      'args.prepare_only or not OBSERVER_PINS_PROVISIONAL',
                      'args.prepare_only or not predecessor.PINS_PROVISIONAL'):
        _require_condition(attempt.body, condition)
    scope = next(node for node in attempt.body if isinstance(node, ast.With))
    _require_condition(scope.body, 'retained.get("status") == ("prepared" if args.prepare_only else "passed") '
                       'and retained.get("semantic_pass") is (not args.prepare_only) '
                       'and retained.get("current_input_members") == 603 '
                       'and retained.get("historical_results_apply_to_current") is False '
                       'and retained.get("controller") == identity(owned_policy.PREDECESSOR_POLICY, '
                       'regular(repo, owned_policy.PREDECESSOR_POLICY))')
    controls = [node for node in scope.body if isinstance(node, ast.If)
                and ast.dump(node.test) == ast.dump(ast.parse('not args.prepare_only', mode='eval').body)]
    pipeline = ast.parse(
        'report.update(owned_policy.run_owned_vector_observer_controls('
        'repo, delegated.output, context["selected"], context["seam"], context["original"], '
        'context["adapter"], retained["test_function_executions"]))\n'
        'report.update(enum_policy.run_enum_observer_controls('
        'repo, delegated.output, context["selected"], context["seam"], context["original"], '
        'context["adapter"], report["test_function_executions"]))').body
    if len(controls) != 1 or [ast.dump(node) for node in controls[0].body] != [ast.dump(node) for node in pipeline]:
        raise AssertionError('Windows-package inherited controls pipeline changed')
    manifest = _function(tree.body, 'current_manifest').body
    _require_statement(manifest, 'inherited = predecessor.current_manifest(inputs=parent, baseline=baseline, '
                       'checkout=checkout, parent={name: parent[name] for name in previous_names})')
    flags = _assignment(manifest, 'result').value
    false_names = {'windows_local_package_semantics_qualified_by_this_gate',
                   'windows_local_package_ntfs_qualified_by_this_gate',
                   'windows_local_package_races_qualified_by_this_gate',
                   'windows_local_package_platform_qualified_by_this_gate'}
    found = {key.value: value for key, value in zip(flags.keys, flags.values)
             if isinstance(key, ast.Constant) and isinstance(key.value, str) and key.value.endswith('qualified_by_this_gate')}
    if set(found) != false_names or any(not isinstance(value, ast.Constant) or value.value is not False for value in found.values()):
        raise AssertionError('Windows-package source admission cannot claim new feature qualification')
    _require_condition(manifest, 'all(value is False for name, value in result.items() '
                       'if name.endswith("qualified_by_this_gate"))')
    report = _assignment(run.body, 'report').value
    values = {key.value: value for key, value in zip(report.keys, report.values)
              if isinstance(key, ast.Constant)}
    for name, expected in (('semantic_pass', False), ('new_feature_qualification', False),
                           ('platform_qualification', False), ('inherited_input_members', 594),
                           ('expected_current_input_members', 603)):
        if (name not in values or not isinstance(values[name], ast.Constant)
                or type(values[name].value) is not type(expected) or values[name].value != expected):
            raise AssertionError('Windows-package preflight qualification/membership changed: ' + name)


def _enum_vector_replay_binding(tree, read):
    for name, expected in (
        ('PREDECESSOR', OWNED_ENUM_VECTOR_PARENT),
        ('PREDECESSOR_POLICY', OFFLINE_VENDOR_POLICY[0]),
        ('PREDECESSOR_POLICY_PIN', OFFLINE_VENDOR_POLICY[1]),
        ('OBSERVER_ADAPTER', OWNED_ENUM_VECTOR_OBSERVER[0]),
        ('OBSERVER_ADAPTER_PIN', OWNED_ENUM_VECTOR_OBSERVER[1]),
        ('V13_ADAPTER', 'scripts/owned_record_vector_compatibility_observer.py'),
        ('V13_ADAPTER_PIN', (7268, 'ac49c38f23b0d3a5929fe3d52edae1cb85341c72b16249522e30dc155e1fa8f3')),
        ('V13_DERIVED_PIN', (32684, '8e65804b8486ec2fda4bca21875c3c296f0b722bd6c40041ee1144b4c44e6042')),
    ):
        if _literal(tree.body, name) != expected:
            raise AssertionError('enum-vector retained parent/observer changed: ' + name)
    require_retained_replay_binding((OFFLINE_VENDOR_REPLAY,), read)
    observer = read(OWNED_ENUM_VECTOR_OBSERVER[0])
    if (len(observer), hashlib.sha256(observer).hexdigest()) != OWNED_ENUM_VECTOR_OBSERVER[1]:
        raise AssertionError('immutable enum-vector observer adapter changed')
    for statement in (
        'predecessor = load_predecessor(ROOT)', 'library = predecessor.library',
        'owned_policy = predecessor.predecessor.predecessor',
        'RETAINED_RUN = predecessor.RETAINED_RUN', 'PREDECESSOR_ADDED = predecessor.PARENT_ADDED',
        'PARENT_ADDED = PREDECESSOR_ADDED | ENUM_ADDED',
    ):
        _require_statement(tree.body, statement)
    inventories = {}
    for name, count in (('ENUM_CHANGED', 45), ('ENUM_ADDED', 11)):
        node = _assignment(tree.body, name).value
        if (not isinstance(node, ast.Call) or not isinstance(node.func, ast.Name)
                or node.func.id != 'frozenset' or len(node.args) != 1 or node.keywords):
            raise AssertionError('enum-vector source roster must be literal')
        values = ast.literal_eval(node.args[0])
        if (not isinstance(values, tuple) or len(values) != count or len(set(values)) != count
                or any(not isinstance(value, str) or not value.startswith(('src/', 'native/')) for value in values)):
            raise AssertionError('enum-vector finite source roster changed')
        inventories[name] = set(values)
    pins = _literal(tree.body, 'REVIEWED_INPUTS')
    expected_inputs = inventories['ENUM_CHANGED'] | inventories['ENUM_ADDED']
    if (inventories['ENUM_CHANGED'] & inventories['ENUM_ADDED'] or len(expected_inputs) != 56
            or set(pins) != expected_inputs
            or any(not isinstance(pin, tuple) or len(pin) != 2 or type(pin[0]) is not int or pin[0] <= 0
                   or not isinstance(pin[1], str) or re.fullmatch('[a-f0-9]{64}', pin[1]) is None
                   for pin in pins.values())):
        raise AssertionError('enum-vector56 literal source pins changed')
    directives = _literal(tree.body, 'REVIEWED_DIRECTIVES')
    if set(directives) != expected_inputs:
        raise AssertionError('enum-vector literal directive membership changed')
    captured = _function(tree.body, 'capture_sources').body
    _require_statement(captured, 'expected_parent = set(baseline) | PREDECESSOR_ADDED')
    _require_statement(captured, 'expected = expected_parent | ENUM_ADDED')
    _require_condition(captured, 'set(parent) == expected_parent and len(parent) == 576')
    _require_condition(captured, 'len(ENUM_CHANGED) == 45 and len(ENUM_ADDED) == 11 '
                       'and ENUM_CHANGED <= expected_parent and not ENUM_ADDED & expected_parent '
                       'and len(expected) == 587')
    _require_condition(captured, 'set(REVIEWED_INPUTS) == ENUM_CHANGED | ENUM_ADDED '
                       'and set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS)')
    bound = _function(tree.body, 'bound_predecessor')
    _retained_bound(bound.body,
                    "{'PREDECESSOR': PREDECESSOR, 'PREDECESSOR_ADDED': PREDECESSOR_ADDED, "
                    "'PREDECESSOR_POLICIES': PREDECESSOR_POLICIES, 'PINS_PROVISIONAL': PINS_PROVISIONAL, "
                    "'capture_sources': capture_sources, 'current_manifest': current_manifest}")
    _require_condition(bound.body, 'library.run is RETAINED_RUN and predecessor.RETAINED_RUN is RETAINED_RUN '
                       'and owned_policy.RETAINED_RUN is RETAINED_RUN')
    scopes = [node for node in bound.body if isinstance(node, ast.With)]
    expected = ast.parse('with bound_observer(), predecessor.bound_predecessor() as context:\n    pass').body[0]
    if (len(scopes) != 1 or [ast.dump(item) for item in scopes[0].items]
            != [ast.dump(item) for item in expected.items]):
        raise AssertionError('enum-vector source/observer delegation scope changed')
    restores = [node for node in scopes[0].body if isinstance(node, ast.Try)]
    if len(restores) != 1:
        raise AssertionError('enum-vector source restoration boundary changed')
    _require_statement(restores[0].body, 'for name, value in bindings.items():\n    setattr(library, name, value)')
    _require_statement(restores[0].finalbody, 'for name, value in before.items():\n    setattr(library, name, value)')
    observer_bound = _function(tree.body, 'bound_observer').body
    if _literal(observer_bound, 'names') != (
            'OWNED_VECTOR_OBSERVER_ADAPTER', 'OWNED_VECTOR_OBSERVER_ADAPTER_PIN',
            'authenticate_owned_vector_observer_adapter'):
        raise AssertionError('enum-vector observer must bind only the three existing G2 globals')
    _require_statement(observer_bound, 'prior, old_body = before["authenticate_owned_vector_observer_adapter"](ROOT)')
    _require_statement(observer_bound, 'authenticate_enum_observer(ROOT, prior)')
    _retained_bound(observer_bound,
                    "{'OWNED_VECTOR_OBSERVER_ADAPTER': OBSERVER_ADAPTER, "
                    "'OWNED_VECTOR_OBSERVER_ADAPTER_PIN': OBSERVER_ADAPTER_PIN, "
                    "'authenticate_owned_vector_observer_adapter': authenticate}")
    restores = [node for node in observer_bound if isinstance(node, ast.Try)]
    if len(restores) != 1:
        raise AssertionError('enum-vector observer restoration boundary changed')
    _require_statement(restores[0].body, 'for name, value in bindings.items():\n    setattr(owned_policy, name, value)')
    _require_statement(restores[0].finalbody, 'for name, value in before.items():\n    setattr(owned_policy, name, value)')
    authenticate = _function(tree.body, 'authenticate_enum_observer').body
    _require_condition(authenticate, '(len(old_body), sha(old_body)) == V13_ADAPTER_PIN '
                       'and prior.DERIVED_PIN == V13_DERIVED_PIN')
    _require_condition(authenticate, '(len(body), sha(body)) == OBSERVER_ADAPTER_PIN')
    _require_condition(authenticate, 'adapter.PARENT == Path(V13_ADAPTER).name '
                       'and adapter.PARENT_PIN == V13_ADAPTER_PIN '
                       'and adapter.predecessor.descriptor() == prior.descriptor() '
                       'and adapter.PARENT_DERIVED_PIN == prior.DERIVED_PIN')
    _retained_delegate(tree, 'bound_predecessor() as context', 'RETAINED_RUN(delegated)', guard_inside=True)
    run = _function(tree.body, 'run')
    attempt = next(node for node in run.body if isinstance(node, ast.Try))
    _require_condition(attempt.body, 'args.prepare_only or not PINS_PROVISIONAL')
    _require_condition(attempt.body, 'args.prepare_only or not OBSERVER_PINS_PROVISIONAL')
    _require_condition(attempt.body, 'args.prepare_only or not predecessor.PINS_PROVISIONAL')
    scope = next(node for node in attempt.body if isinstance(node, ast.With))
    controls = [node for node in scope.body if isinstance(node, ast.If)
                and ast.dump(node.test) == ast.dump(ast.parse('not args.prepare_only', mode='eval').body)]
    pipeline = ast.parse(
        'report.update(owned_policy.run_owned_vector_observer_controls('
        'repo, delegated.output, context["selected"], context["seam"], context["original"], '
        'context["adapter"], retained["test_function_executions"]))\n'
        'report.update(run_enum_observer_controls('
        'repo, delegated.output, context["selected"], context["seam"], context["original"], '
        'context["adapter"], report["test_function_executions"]))').body
    if len(controls) != 1 or [ast.dump(node) for node in controls[0].body] != [ast.dump(node) for node in pipeline]:
        raise AssertionError('enum-vector inherited/new controls pipeline changed')
    false_names = {'owned_enum_vector_semantics_qualified_by_this_gate',
                   'owned_enum_vector_allocator_failures_qualified_by_this_gate',
                   'owned_enum_vector_platform_qualified_by_this_gate',
                   'general_heap_collections_qualified_by_this_gate', 'g2_complete_qualified_by_this_gate'}
    manifest = _function(tree.body, 'current_manifest').body
    values = _assignment(manifest, 'result').value
    flags = {key.value: value for key, value in zip(values.keys, values.values)
             if isinstance(key, ast.Constant) and isinstance(key.value, str) and key.value.endswith('qualified_by_this_gate')}
    if set(flags) != false_names or any(not isinstance(value, ast.Constant) or value.value is not False for value in flags.values()):
        raise AssertionError('enum-vector source admission cannot claim new feature qualification')
    _require_condition(manifest, 'all(value is False for name, value in result.items() '
                       'if name.endswith("qualified_by_this_gate"))')
    report = _assignment(run.body, 'report').value
    for name in ('semantic_pass', 'new_feature_qualification', 'platform_qualification'):
        values = [value for key, value in zip(report.keys, report.values)
                  if isinstance(key, ast.Constant) and key.value == name]
        if len(values) != 1 or not isinstance(values[0], ast.Constant) or values[0].value is not False:
            raise AssertionError('enum-vector preflight qualification claim changed')
    control_body = _function(tree.body, 'run_enum_observer_controls').body
    profiles = [node for node in control_body if isinstance(node, ast.For)]
    if (len(profiles) != 1 or ast.dump(profiles[0].target) != ast.dump(ast.Name(id='profile', ctx=ast.Store()))
            or ast.literal_eval(profiles[0].iter) != ('debug', 'release') or profiles[0].orelse):
        raise AssertionError('enum-vector controls must execute both real Rust profiles')
    profile = profiles[0].body
    _require_condition(control_body, 'type(previous_executions) is int and previous_executions > 0')
    _require_condition(control_body, 'tuple(adapter.OWNED_ENUM_VECTOR_CONTROL_NAMES) == ('
                       '"frontend::oir::unit2_observer::current_unit2_owned_enum_vector_adapter_denies_values",'
                       '"frontend::oir::unit2_observer::current_unit2_owned_enum_vector_adapter_denies_shared_vectors",'
                       '"frontend::oir::unit2_observer::current_unit2_owned_enum_vector_adapter_denies_exclusive_vectors")')
    _require_statement(profile, 'names = list(adapter.OWNED_ENUM_VECTOR_CONTROL_NAMES)')
    _require_statement(profile, 'protocol.rust_listing(streams["list"], names)')
    _require_statement(profile, 'result = protocol.rust_success(streams["run"], names)')
    _require_statement(control_body, 'original.verify_unit2_result(output, captured, seam, False)')
    loops = [node for node in profile if isinstance(node, ast.For)]
    expected = ast.parse(
        '(("list", ["--list", adapter.OWNED_ENUM_VECTOR_CONTROL_FILTER]), '
        '("run", [adapter.OWNED_ENUM_VECTOR_CONTROL_FILTER, "--test-threads=1", "--color", "never"]))',
        mode='eval').body
    if len(loops) != 1 or ast.dump(loops[0].iter) != ast.dump(expected):
        raise AssertionError('enum-vector controls must list and execute the three exact controls')
    commands = [node for node in loops[0].body if isinstance(node, ast.With)]
    if len(commands) != 1:
        raise AssertionError('enum-vector raw command scope changed')
    _require_statement(commands[0].body, 'completed = library.subprocess.run('
                       'argv, cwd=run / "source", stdout=stdout, stderr=stderr, timeout=60)')
    _require_condition(loops[0].body, 'completed.returncode == 0 and err.read_bytes() == b""')
    _require_statement(control_body,
                       'return {"owned_enum_vector_observer_control_tests_per_profile": 3, '
                       '"owned_enum_vector_observer_denials_per_profile": 9, '
                       '"owned_enum_vector_observer_control_receipts": receipts, '
                       '"inherited_test_function_executions": previous_executions, '
                       '"test_function_executions": previous_executions + 6}')


def require_retained_replay_binding(route, read=None):
    """Check route ownership against immutable parents without importing controllers."""
    read = read or (lambda name: (REPO / name).read_bytes())
    kind, (policy, pin) = REPLAY_ROUTES[route]
    raw = read(policy)
    if (len(raw), hashlib.sha256(raw).hexdigest()) != pin:
        raise AssertionError('immutable retained policy changed: ' + policy)
    if kind == 'direct':
        return
    tree = ast.parse(read(route[0]))
    if kind == 'msvc':
        expected = {
            'scalar-library': {'parent': '260c816fb984391493df84e498511dd9fc013f42',
                               'controller': SCALAR_REPLAY[0], 'controller_pin': SCALAR_REPLAY[1],
                               'input_members': 522},
            'registry': {'parent': 'af0fe5e1fbaf4fb49b62e894a53ad4b5ed307ee7',
                         'controller': REGISTRY_REPLAY[0], 'controller_pin': REGISTRY_REPLAY[1],
                         'input_members': 527},
        }
        if _literal(tree.body, 'SOURCE_POLICIES') != expected:
            raise AssertionError('MSVC retained source-policy delegation changed')
        classes = [node for node in tree.body if isinstance(node, ast.ClassDef)
                   and node.name == 'RetainedPolicy']
        if len(classes) != 1:
            raise AssertionError('missing retained policy owner')
        _require_statement(_function(classes[0].body, '__init__').body,
                           'self.original_run, self.original_manifest = library.run, library.current_manifest')
        _retained_bound(_function(classes[0].body, 'bound').body,
                        "{'PREDECESSOR': self.spec['parent'], 'PREDECESSOR_ADDED': self.parent_added, "
                        "'PREDECESSOR_POLICIES': self.policies, 'PINS_PROVISIONAL': PINS_PROVISIONAL, "
                        "'capture_sources': self.capture_sources, 'current_manifest': self.current_manifest}")
        _require_statement(_function(tree.body, 'load_policy').body, 'body = read_policy(repo, spec)')
        _require_statement(_function(tree.body, 'load_policy').body, 'return RetainedPolicy(name, module)')
        _require_statement(next(node for node in _function(tree.body, 'run').body
                                if isinstance(node, ast.Try)).body,
                           'selected = load_policy(repo, args.source_policy)')
        _retained_delegate(tree, 'selected.bound()', 'selected.original_run(delegated)')
    elif kind == 'windows-local-package':
        _windows_local_package_replay_binding(tree, read)
    elif kind == 'generic-owned-vector':
        _generic_vector_replay_binding(tree, read)
    elif kind == 'owned-enum-vector':
        _enum_vector_replay_binding(tree, read)
    elif kind == 'offline-vendor':
        for name, expected in (
            ('PREDECESSOR', OFFLINE_VENDOR_PARENT),
            ('PREDECESSOR_POLICY', OWNED_TEXT_POLICY[0]),
            ('PREDECESSOR_POLICY_PIN', OWNED_TEXT_POLICY[1]),
            ('CLI', 'src/cli.rs'), ('MAIN', 'src/main.rs'),
            ('PACKAGE', 'src/project_package.rs'), ('COMMAND', 'src/project_vendor.rs'),
            ('TEST', 'src/project_package/vendor_tests.rs'),
            ('TEXT_RESOURCE_TEST', 'src/frontend/oir/owned/source/owned_text_source_resource_tests.rs'),
            ('VECTOR_ROUTE_TEST', 'src/frontend/oir/source/owned_vector_route_tests.rs'),
        ):
            if _literal(tree.body, name) != expected:
                raise AssertionError('offline-vendor retained parent/input changed: ' + name)
        require_retained_replay_binding((OWNED_TEXT_REPLAY,), read)
        for statement in (
            'predecessor = load_predecessor(ROOT)', 'library = predecessor.library',
            'RETAINED_RUN = predecessor.RETAINED_RUN', 'PREDECESSOR_ADDED = predecessor.PARENT_ADDED',
            'VENDOR_CHANGED = frozenset((CLI, MAIN, PACKAGE, TEXT_RESOURCE_TEST, VECTOR_ROUTE_TEST))',
            'VENDOR_ADDED = frozenset((COMMAND, TEST))',
            'PARENT_ADDED = PREDECESSOR_ADDED | VENDOR_ADDED',
        ):
            _require_statement(tree.body, statement)
        captured = _function(tree.body, 'capture_sources').body
        _require_statement(captured, 'expected_parent = set(baseline) | PREDECESSOR_ADDED')
        _require_statement(captured, 'expected = expected_parent | VENDOR_ADDED')
        _require_condition(captured, 'set(parent) == expected_parent and len(parent) == 574')
        _require_condition(captured,
                           'VENDOR_CHANGED == {CLI, MAIN, PACKAGE, TEXT_RESOURCE_TEST, VECTOR_ROUTE_TEST} '
                           'and VENDOR_ADDED == {COMMAND, TEST} '
                           'and VENDOR_CHANGED <= expected_parent and not VENDOR_ADDED & expected_parent '
                           'and len(expected) == 576')
        _require_condition(captured, 'inputs[CLI] == approved_cli(parent[CLI])')
        _require_condition(captured, 'inputs[MAIN] == approved_main(parent[MAIN])')
        _require_condition(captured, 'inputs[TEXT_RESOURCE_TEST] == approved_text_resource_test(parent[TEXT_RESOURCE_TEST])')
        _require_condition(captured, 'inputs[VECTOR_ROUTE_TEST] == approved_vector_route_test(parent[VECTOR_ROUTE_TEST])')
        _require_statement(captured, 'insertion = unchanged_package(parent[PACKAGE], inputs[PACKAGE])')
        bound = _function(tree.body, 'bound_predecessor')
        _retained_bound(bound.body,
                        "{'PREDECESSOR': PREDECESSOR, 'PREDECESSOR_ADDED': PREDECESSOR_ADDED, "
                        "'PREDECESSOR_POLICIES': PREDECESSOR_POLICIES, 'PINS_PROVISIONAL': PINS_PROVISIONAL, "
                        "'capture_sources': capture_sources, 'current_manifest': current_manifest}")
        scopes = [node for node in bound.body if isinstance(node, ast.With)]
        expected = ast.parse('with predecessor.bound_predecessor() as context:\n    pass').body[0]
        if (len(scopes) != 1 or len(scopes[0].items) != 1
                or ast.dump(scopes[0].items[0]) != ast.dump(expected.items[0])):
            raise AssertionError('offline-vendor predecessor observer scope changed')
        _retained_delegate(tree, 'bound_predecessor() as context', 'RETAINED_RUN(delegated)', guard_inside=True)
        attempt = next(node for node in _function(tree.body, 'run').body if isinstance(node, ast.Try))
        _require_condition(attempt.body, 'args.prepare_only or not PINS_PROVISIONAL')
        scope = next(node for node in attempt.body if isinstance(node, ast.With))
        controls = [node for node in scope.body if isinstance(node, ast.If)
                    and ast.dump(node.test) == ast.dump(ast.parse('not args.prepare_only', mode='eval').body)]
        if len(controls) != 1:
            raise AssertionError('offline-vendor full observer replay changed')
        _require_statement(controls[0].body,
                           'report.update(predecessor.predecessor.run_owned_vector_observer_controls('
                           'repo, delegated.output, context["selected"], context["seam"], '
                           'context["original"], context["adapter"], retained["test_function_executions"]))')
    elif kind == 'owned-text':
        for name, expected in (
            ('PREDECESSOR', '0e8f23f7481c7ec4e24cd416ed30607f8eb75daf'),
            ('PREDECESSOR_POLICY', OWNED_VECTOR_POLICY[0]),
            ('PREDECESSOR_POLICY_PIN', OWNED_VECTOR_POLICY[1]),
            ('PROGRAM', 'src/frontend/oir/owned/source/program.rs'),
            ('TEST', 'src/frontend/oir/owned/source/owned_text_source_resource_tests.rs'),
            ('ASSET', 'packages/utf8_text/src/lib.ox'),
        ):
            if _literal(tree.body, name) != expected:
                raise AssertionError('owned-text retained parent/input changed: ' + name)
        require_retained_replay_binding((OWNED_VECTOR_REPLAY,), read)
        for statement in (
            'predecessor = load_predecessor(ROOT)', 'library = predecessor.library',
            'RETAINED_RUN = predecessor.RETAINED_RUN', 'PREDECESSOR_ADDED = predecessor.PARENT_ADDED',
            'TEXT_CHANGED = frozenset((PROGRAM,))', 'TEXT_ADDED = frozenset((TEST,))',
            'PARENT_ADDED = PREDECESSOR_ADDED | TEXT_ADDED | frozenset(TEXT_SOURCE_ASSETS)',
        ):
            _require_statement(tree.body, statement)
        captured = _function(tree.body, 'capture_sources').body
        _require_statement(captured, 'expected_parent = set(baseline) | PREDECESSOR_ADDED')
        _require_statement(captured, 'expected = expected_parent | TEXT_ADDED | set(TEXT_SOURCE_ASSETS)')
        _require_condition(captured, 'set(parent) == expected_parent and len(parent) == 572')
        _require_condition(captured,
                           'TEXT_CHANGED == {PROGRAM} and TEXT_ADDED == {TEST} '
                           'and TEXT_CHANGED <= expected_parent and not TEXT_ADDED & expected_parent '
                           'and not set(TEXT_SOURCE_ASSETS) & expected_parent and len(expected) == 574')
        _require_condition(captured, 'inputs[PROGRAM] == approved_program(parent[PROGRAM])')
        bound = _function(tree.body, 'bound_predecessor')
        _retained_bound(bound.body,
                        "{'PREDECESSOR': PREDECESSOR, 'PREDECESSOR_ADDED': PREDECESSOR_ADDED, "
                        "'PREDECESSOR_POLICIES': PREDECESSOR_POLICIES, 'PINS_PROVISIONAL': PINS_PROVISIONAL, "
                        "'capture_sources': capture_sources, 'current_manifest': current_manifest}")
        scopes = [node for node in bound.body if isinstance(node, ast.With)]
        expected = ast.parse('with predecessor.bound_registry() as context:\n    pass').body[0]
        if (len(scopes) != 1 or len(scopes[0].items) != 1
                or ast.dump(scopes[0].items[0]) != ast.dump(expected.items[0])):
            raise AssertionError('owned-text predecessor observer scope changed')
        _retained_delegate(tree, 'bound_predecessor() as context', 'RETAINED_RUN(delegated)', guard_inside=True)
        attempt = next(node for node in _function(tree.body, 'run').body if isinstance(node, ast.Try))
        _require_condition(attempt.body, 'args.prepare_only or not PINS_PROVISIONAL')
        scope = next(node for node in attempt.body if isinstance(node, ast.With))
        controls = [node for node in scope.body if isinstance(node, ast.If)
                    and ast.dump(node.test) == ast.dump(ast.parse('not args.prepare_only', mode='eval').body)]
        if len(controls) != 1:
            raise AssertionError('owned-text full observer replay changed')
        _require_statement(controls[0].body,
                           'report.update(predecessor.run_owned_vector_observer_controls('
                           'repo, delegated.output, context["selected"], context["seam"], '
                           'context["original"], context["adapter"], retained["test_function_executions"]))')
    else:
        for name, expected in (
            ('PREDECESSOR', 'af0fe5e1fbaf4fb49b62e894a53ad4b5ed307ee7'),
            ('PREDECESSOR_POLICY', REGISTRY_REPLAY[0]),
            ('PREDECESSOR_POLICY_PIN', REGISTRY_REPLAY[1]),
        ):
            if _literal(tree.body, name) != expected:
                raise AssertionError('owned-vector retained parent changed: ' + name)
        _require_statement(tree.body, 'library = load_predecessor(ROOT)')
        _require_statement(tree.body, 'RETAINED_RUN = library.run')
        _retained_bound(_function(tree.body, 'bound_registry').body,
                        "{'PREDECESSOR': PREDECESSOR, 'PREDECESSOR_ADDED': PREDECESSOR_ADDED, "
                        "'PREDECESSOR_POLICIES': PREDECESSOR_POLICIES, 'PINS_PROVISIONAL': PINS_PROVISIONAL, "
                        "'capture_sources': capture_sources, 'current_manifest': current_manifest, "
                        "'OBSERVER_ADAPTER': OWNED_VECTOR_OBSERVER_ADAPTER, "
                        "'authenticate_observer_adapter': authenticate_owned_vector_observer_adapter, "
                        "'prepare_current_observer': prepare}")
        _retained_delegate(tree, 'bound_registry() as context', 'RETAINED_RUN(delegated)', guard_inside=True)


class CiTriggerPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ci = (REPO / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        cls.release = (REPO / '.github/workflows/release.yml').read_text(encoding='utf-8')
        cls.historical = (REPO / '.github/workflows/historical-qualification.yml').read_text(encoding='utf-8')

    def test_push_runs_main_and_all_tags_only(self):
        self.assertEqual(block(self.ci, 'push', 2),
                         "branches: [main]\n    tags: ['**']")

    def test_pull_requests_manual_reusable_and_weekly_entry_points(self):
        for event in ('pull_request', 'workflow_dispatch', 'workflow_call'):
            with self.subTest(event=event):
                self.assertEqual(block(self.ci, event, 2), '')
        self.assertEqual(block(self.ci, 'schedule', 2), '- cron: "17 3 * * 1"')
        events = re.findall(r'^(?:  )?([a-z_]+):', block(self.ci, 'on'), re.MULTILINE)
        self.assertEqual(events, ['push', 'pull_request', 'workflow_dispatch', 'workflow_call', 'schedule'])

    def test_no_workflow_or_job_concurrency_cancellation(self):
        for workflow in (self.ci, self.release, self.historical):
            self.assertNotRegex(workflow, r'(?m)^\s*(?:concurrency|cancel-in-progress):')

    def test_release_requires_readiness_then_full_reusable_ci_before_packaging(self):
        readiness = block(self.release, 'readiness', 2)
        self.assertIn('scripts/check_release_gates.py --release-tag "$RELEASE_TAG"', readiness)
        self.assertIn("if: github.ref_type == 'tag'", readiness)
        self.assertNotRegex(readiness, r'(?m)^    if:|continue-on-error:')
        verify = block(self.release, 'verify', 2)
        self.assertEqual(re.findall(r'^(?:    )?needs: (.+)$', verify, re.MULTILINE), ['readiness'])
        self.assertIn('uses: ./.github/workflows/ci.yml', verify)
        self.assertNotRegex(verify, r'(?m)^    if:|continue-on-error:')
        package = block(self.release, 'package', 2)
        self.assertEqual(re.findall(r'^    needs: (.+)$', package, re.MULTILINE), ['verify'])
        self.assertNotRegex(package, r'(?m)^    if:')

    def test_copy_record_debugging_executes_on_both_linux_architectures_and_retains_binaries(self):
        for job in ('native-projects', 'linux-arm64-native'):
            native = block(self.ci, job, 2)
            self.assertIn('for profile in debug release; do', native)
            self.assertIn('--test native_copy_record_debug', native)
            self.assertIn('-- --include-ignored --test-threads=1 --nocapture', native)
            self.assertIn('OXID_TEST_KEEP_ARTIFACTS=1', native)
            self.assertIn('TMPDIR=', native)
            self.assertIn('include-hidden-files: true', native)
            self.assertIn('if-no-files-found: error', native)
            self.assertNotRegex(native, r'(?m)^    if:|continue-on-error:')

    def test_current_discovery_and_historical_replay_are_both_mandatory(self):
        current = block(self.ci, 'quality-and-repository', 2)
        self.assertIn('python3 -B scripts/run_current_script_tests.py', current)
        require_retained_replay_binding(retained_replay_route(current))
        self.assertIn('scripts/verify_floats.py', current)
        self.assertIn('scripts/verify_float_roots.py', current)
        self.assertIn('scripts/verify_characters.py', current)
        self.assertIn('scripts/verify_float_vector.py', current)
        self.assertIn('scripts/verify_typed_vectors.py', current)
        self.assertIn('scripts/verify_copy_record_vectors.py', current)
        self.assertIn('scripts/verify_matrices.py', current)
        self.assertIn('scripts/verify_i64.py', current)
        self.assertIn('scripts/verify_integer_widths.py', current)
        self.assertIn('scripts/verify_byte_integers.py', current)
        self.assertIn('scripts/verify_integer128.py', current)
        self.assertIn('scripts/verify_integer_bits.py', current)
        self.assertNotRegex(current, r'(?m)^    if:|continue-on-error:')

        qualified = block(self.ci, 'qualified-completion-source-policy-predecessor', 2)
        self.assertIn('ref: 20ea8ba102975d2e8ac0c667b96ce835a1bc6ef9', qualified)
        self.assertIn('scripts/run_qualified_completion_predecessor_tests.py', qualified)
        self.assertIn('"$RUNNER_TEMP/qualified-completion-source-policy-replay.py" --repo . --output', qualified)
        self.assertIn('if-no-files-found: error', qualified)
        self.assertNotRegex(qualified, r'(?m)^    if:|continue-on-error:')

        replay = block(self.ci, 'historical-qualification', 2)
        self.assertIn('uses: ./.github/workflows/historical-qualification.yml', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')
        historical = block(self.historical, 'quality-and-repository', 2)
        self.assertIn('ref: 9ff8ef90a26f003f4616838dfc186fa3bfb6845c', historical)
        self.assertIn('historical-script-controller/run_historical_script_tests.py', historical)
        self.assertIn('--archive "$RUNNER_TEMP/historical-script-controller/typed-project-unit2-evidence.zip"', historical)
        self.assertIn('name: historical-script-replay', historical)
        self.assertNotRegex(historical, r'(?m)^    if:|continue-on-error:')

    def test_copy_debug_source_policy_replay_is_mandatory_and_keeps_evidence(self):
        replay = block(self.ci, 'copy-record-debug-source-policy-predecessor', 2)
        self.assertIn('runs-on: ubuntu-latest', replay)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('cp scripts/run_copy_record_debug_predecessor_tests.py '
                      '"$RUNNER_TEMP/copy-record-debug-source-policy-replay.py"', replay)
        self.assertIn('ref: 9a5ecaaeb5fa50aff2296bf990d63167143cf312', replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/copy-record-debug-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/copy-record-debug-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/copy-record-debug-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')

    def test_windows_native_source_policy_replay_is_mandatory_and_keeps_evidence(self):
        replay = block(self.ci, 'windows-native-source-policy-predecessor', 2)
        self.assertIn('runs-on: ubuntu-latest', replay)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('cp scripts/run_windows_native_predecessor_tests.py '
                      '"$RUNNER_TEMP/windows-native-source-policy-replay.py"', replay)
        self.assertIn('ref: 8180b1bbcbcf9ccd4d648ff5ba0ec9ea3d5b971f', replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/windows-native-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/windows-native-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/windows-native-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')

    def test_record_field_source_policy_replay_is_mandatory_and_keeps_evidence(self):
        replay = block(self.ci, 'record-field-completion-source-policy-predecessor', 2)
        self.assertIn('runs-on: ubuntu-latest', replay)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('cp scripts/run_record_field_completion_predecessor_tests.py '
                      '"$RUNNER_TEMP/record-field-completion-source-policy-replay.py"', replay)
        self.assertIn('ref: b273b71237cc6a1d2db76dfe7c971e073d1d5cb3', replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/record-field-completion-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/record-field-completion-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/record-field-completion-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')

    def test_seekable_streaming_native_oracle_and_sanitizers_are_required(self):
        native = block(self.ci, 'native-projects', 2)
        self.assertIn('python3 -B scripts/verify_seekable_streaming.py target/debug/oxid target/release/oxid', native)
        self.assertIn('--output "$RUNNER_TEMP/current-seekable-streaming" --llvm-bin "$OXID_LLVM_BIN"', native)
        self.assertIn('${{ env.OXID_NATIVE_EVIDENCE_ROOT }}/current-seekable-streaming', native)
        self.assertIn('          python3 -B scripts/test_native_file_seek_runtime.py\n', native)
        self.assertIn('          OXID_BYTEBUF_SANITIZE=1 python3 -B scripts/test_native_file_seek_runtime.py\n', native)
        self.assertNotRegex(native, r'(?m)^    (?:if|needs):|continue-on-error:')

    def test_file_seek_source_policy_replay_is_mandatory_and_keeps_evidence(self):
        replay = block(self.ci, 'file-seek-source-policy-predecessor', 2)
        self.assertIn('runs-on: ubuntu-latest', replay)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('cp scripts/run_file_seek_predecessor_tests.py '
                      '"$RUNNER_TEMP/file-seek-source-policy-replay.py"', replay)
        self.assertIn('ref: 104c900222ead9d9177025102a90520b08e761a4', replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/file-seek-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/file-seek-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/file-seek-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')

    def test_returning_native_clients_run_in_both_profiles_and_keep_evidence(self):
        native = block(self.ci, 'native-projects', 2)
        self.assertIn('      - name: Verify returning native libraries from C Rust and Python\n'
                      '        env:\n'
                      '          TMPDIR: ${{ runner.temp }}/current-native-libraries\n'
                      "          OXID_TEST_KEEP_ARTIFACTS: '1'\n"
                      '        run: |\n'
                      '          mkdir -p "$TMPDIR"\n', native)
        for profile in ('', '--release '):
            command = ('cargo test ' + profile + '--locked --test native_library '
                       'actual_same_library -- --ignored --nocapture')
            self.assertEqual(native.count(command), 1)
        self.assertIn('${{ env.OXID_NATIVE_EVIDENCE_ROOT }}/current-native-libraries', native)
        self.assertNotRegex(native, r'(?m)^    (?:if|needs):|continue-on-error:')

    def test_scalar_library_source_policy_replay_is_mandatory_and_keeps_evidence(self):
        replay = block(self.ci, 'scalar-library-source-policy-predecessor', 2)
        self.assertIn('runs-on: ubuntu-latest', replay)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('cp scripts/replay_scalar_library_source_policy.py '
                      '"$RUNNER_TEMP/scalar-library-source-policy-replay.py"', replay)
        self.assertIn('ref: 8f2c29eaffabbd90c591bbdc440ae6e23acf1353', replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/scalar-library-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/scalar-library-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/scalar-library-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')

    def test_owned_vector_source_policy_replay_is_mandatory_on_the_exact_published_source(self):
        replay = block(self.ci, 'owned-record-vector-source-policy-predecessor', 2)
        self.assertIn('runs-on: ubuntu-latest', replay)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('cp scripts/replay_owned_record_vector_source_policy.py '
                      '"$RUNNER_TEMP/owned-vector-source-policy-replay.py"', replay)
        self.assertIn('ref: 0e8f23f7481c7ec4e24cd416ed30607f8eb75daf', replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/owned-vector-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/owned-vector-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/owned-vector-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')

    def test_offline_vendor_source_policy_replay_keeps_all_fourteen_frozen_controls_mandatory(self):
        replay = block(self.ci, 'offline-vendor-source-policy-predecessor', 2)
        self.assertIn('runs-on: ubuntu-latest', replay)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('cp scripts/replay_offline_vendor_source_policy.py '
                      '"$RUNNER_TEMP/offline-vendor-source-policy-replay.py"', replay)
        self.assertIn('ref: ' + OWNED_ENUM_VECTOR_PARENT, replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/offline-vendor-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/offline-vendor-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/offline-vendor-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')
        source = ast.parse((REPO / 'scripts/replay_offline_vendor_source_policy.py').read_bytes())
        self.assertEqual(_literal(source.body, 'REVISION'), OWNED_ENUM_VECTOR_PARENT)
        self.assertEqual(_literal(source.body, 'MODULES'), {'test_offline_vendor_compatibility': (
            '4446e8981ac90152f156fb4231268fa7bbe86b2dc6e4beab88e7aedd45f01bc9', 14)})
        _require_statement(source.body, '''POLICIES = {
            "verify_offline_vendor_compatibility.py": "ee64cc7f63051764594a5866101cff9987ca7ddf20b764e7e1aa88f975c9b7a1",
            "verify_owned_text_compatibility.py": "61bf3bf55b9c1f958fdf48fcc89e747cdf61700109ab979b098851249d9fa109",
            "verify_owned_record_vector_compatibility.py": "16c7affd778deda3d61f9a4e4aebd66c9e01d842c6afe3e9c1a20de57caab6e0",
            "replay_owned_text_source_policy.py": "341673ed92fdc3b2f3269ad0396411a061397110b663d067cc7d92def65b01b6",
            ENGINE: ENGINE_SHA,
        }''')
        self.assertEqual(_literal(source.body, 'ENGINE'), 'run_record_vector_predecessor_tests.py')
        self.assertEqual(_literal(source.body, 'ENGINE_SHA'),
                         '6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c')

    def test_enum_vector_source_policy_replay_keeps_all_sixteen_frozen_controls_mandatory(self):
        replay = block(self.ci, 'owned-enum-vector-source-policy-predecessor', 2)
        self.assertEqual(replay.count('actions/checkout@v4'), 2)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('ref: ' + GENERIC_OWNED_VECTOR_PARENT, replay)
        self.assertIn('cp scripts/replay_owned_enum_vector_source_policy.py '
                      '"$RUNNER_TEMP/owned-enum-vector-source-policy-replay.py"', replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/owned-enum-vector-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/owned-enum-vector-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/owned-enum-vector-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')
        source = ast.parse((REPO / 'scripts/replay_owned_enum_vector_source_policy.py').read_bytes())
        self.assertEqual(_literal(source.body, 'REVISION'), GENERIC_OWNED_VECTOR_PARENT)
        self.assertEqual(_literal(source.body, 'MODULES'), {'test_owned_enum_vector_compatibility': (
            '6a4bf9d412a9ae94acb7c01ae29633764bd56f39aeb27e117d55a60ed7374a0f', 16)})
        self.assertEqual(_literal(source.body, 'ENGINE'), 'run_record_vector_predecessor_tests.py')
        self.assertEqual(_literal(source.body, 'ENGINE_SHA'),
                         '6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c')
        _require_statement(source.body, '''POLICIES = {
            "verify_owned_enum_vector_compatibility.py": "3505f8238f1d24754916075fb1ad46aa022c740cd71c9f10a5509656b7e9877c",
            "verify_offline_vendor_compatibility.py": "ee64cc7f63051764594a5866101cff9987ca7ddf20b764e7e1aa88f975c9b7a1",
            "verify_owned_text_compatibility.py": "61bf3bf55b9c1f958fdf48fcc89e747cdf61700109ab979b098851249d9fa109",
            "verify_owned_record_vector_compatibility.py": "16c7affd778deda3d61f9a4e4aebd66c9e01d842c6afe3e9c1a20de57caab6e0",
            "replay_owned_text_source_policy.py": "341673ed92fdc3b2f3269ad0396411a061397110b663d067cc7d92def65b01b6",
            ENGINE: ENGINE_SHA,
        }''')

    def test_generic_source_policy_replay_keeps_all_sixteen_frozen_controls_mandatory(self):
        replay = block(self.ci, 'generic-owned-vector-source-policy-predecessor', 2)
        self.assertEqual(replay.count('actions/checkout@v4'), 2)
        self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', replay)
        self.assertIn('ref: ' + WINDOWS_LOCAL_PACKAGE_PARENT, replay)
        self.assertIn('cp scripts/replay_generic_owned_vector_source_policy.py '
                      '"$RUNNER_TEMP/generic-owned-vector-source-policy-replay.py"', replay)
        self.assertIn('fetch-depth: 0', replay)
        self.assertIn('python3 -B "$RUNNER_TEMP/generic-owned-vector-source-policy-replay.py" '
                      '--repo . --output "$RUNNER_TEMP/generic-owned-vector-source-policy-replay"', replay)
        self.assertIn('if: always()', replay)
        self.assertIn('path: ${{ runner.temp }}/generic-owned-vector-source-policy-replay', replay)
        self.assertIn('if-no-files-found: error', replay)
        self.assertIn('retention-days: 14', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')
        source = ast.parse((REPO / 'scripts/replay_generic_owned_vector_source_policy.py').read_bytes())
        self.assertEqual(_literal(source.body, 'REVISION'), WINDOWS_LOCAL_PACKAGE_PARENT)
        self.assertFalse(_literal(source.body, 'PINS_PROVISIONAL'))
        self.assertEqual(_literal(source.body, 'MODULES'), {'test_generic_owned_vector_compatibility': (
            '916fcbd204b221fabf1286eb48e08149a9b47b2cca40aa10a2d2ee0e69959be2', 16)})
        self.assertEqual(_literal(source.body, 'ENGINE'), 'run_record_vector_predecessor_tests.py')
        self.assertEqual(_literal(source.body, 'ENGINE_SHA'),
                         '6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c')
        _require_statement(source.body, '''POLICIES = {
            "verify_generic_owned_vector_compatibility.py": "8e80cbdb0b42e01dd4e125d36215dc94ff37422a0e10786216ec02ad66831ad0",
            "replay_owned_enum_vector_source_policy.py": "11a1d669c636c43b9163ed0e86a73785827628b93a83f4608bebfb262e2abbb1",
            "verify_owned_enum_vector_compatibility.py": "3505f8238f1d24754916075fb1ad46aa022c740cd71c9f10a5509656b7e9877c",
            "verify_offline_vendor_compatibility.py": "ee64cc7f63051764594a5866101cff9987ca7ddf20b764e7e1aa88f975c9b7a1",
            "verify_owned_text_compatibility.py": "61bf3bf55b9c1f958fdf48fcc89e747cdf61700109ab979b098851249d9fa109",
            "verify_owned_record_vector_compatibility.py": "16c7affd778deda3d61f9a4e4aebd66c9e01d842c6afe3e9c1a20de57caab6e0",
            "replay_owned_text_source_policy.py": "341673ed92fdc3b2f3269ad0396411a061397110b663d067cc7d92def65b01b6",
            ENGINE: ENGINE_SHA,
        }''')
        # No steps or predicates from the Enum replay are replaced by this job.
        enum = block(self.ci, 'owned-enum-vector-source-policy-predecessor', 2)
        self.assertIn('ref: ' + GENERIC_OWNED_VECTOR_PARENT, enum)
        self.assertIn('cp scripts/replay_owned_enum_vector_source_policy.py ', enum)

    def test_generic_native_packages_execute_both_real_compiler_profiles_on_all_posix_runners(self):
        command = ('            OXID_TEST_LLVM_BIN="$OXID_LLVM_BIN" cargo test --locked "${profile_flags[@]}" \\\n'
                   '              --test typed_owned_vector_wrappers -- --ignored --test-threads=1\n')
        step = 'Verify generic vector source-free packages in both compiler profiles'
        for job in ('native-projects', 'linux-arm64-native', 'macos-native'):
            with self.subTest(job=job):
                current = block(self.ci, job, 2)
                selected = re.findall(r'^      - name: ' + re.escape(step)
                                      + r'\n(.*?)(?=^      - |\Z)', current, re.M | re.S)
                self.assertEqual(len(selected), 1)
                self.assertIn('        shell: bash\n        run: |\n          set -euo pipefail\n', selected[0])
                self.assertIn('          for profile in debug release; do\n', selected[0])
                self.assertIn('            if test "$profile" = release; then profile_flags=(--release); fi\n', selected[0])
                self.assertEqual(selected[0].count(command), 1)
                self.assertNotRegex(selected[0], r'(?m)^        if:|continue-on-error:|\|\| true|--no-run|--list')

    def test_arm_owned_text_driver_binds_expected_head_from_the_same_step_environment(self):
        arm = block(self.ci, 'linux-arm64-native', 2)
        steps = re.findall(
            r'^      - name: Verify native ARM64 builds and source-free execution\n'
            r'(.*?)(?=^      - |\Z)', arm, re.MULTILINE | re.DOTALL)
        self.assertEqual(len(steps), 1)
        self.assertIn('        env:\n', steps[0])
        self.assertIn('          EXPECTED_HEAD: ${{ github.event.pull_request.head.sha || github.sha }}\n', steps[0])
        command = ('          python3 -B scripts/verify_owned_text.py \\\n'
                   '            --repo . --expected-head "$EXPECTED_HEAD" \\\n'
                   '            --oxid-debug target/debug/oxid --oxid-release target/release/oxid \\\n'
                   '            --output "$RUNNER_TEMP/linux-arm64-native/owned-text" \\\n'
                   '            --llvm-bin "$OXID_LLVM_BIN" --native\n')
        self.assertEqual(steps[0].count(command), 1)
        self.assertNotRegex(steps[0], r'(?m)^        if:|continue-on-error:')

    def test_private_registry_acceptance_is_mandatory_and_keeps_native_evidence(self):
        native = block(self.ci, 'native-numeric-and-packages', 2) + '\n'
        self.assertIn('      - name: Verify private registry publication offline builds and deployment\n'
                      '        run: |\n'
                      '          python3 -B scripts/verify_typed_registry.py '
                      'target/debug/oxid target/release/oxid \\\n'
                      '            --output "$RUNNER_TEMP/current-typed-registry" --native\n', native)
        self.assertIn('          cargo build --locked\n', native)
        self.assertIn('          cargo build --release --locked\n', native)
        self.assertIn('      - name: Retain private registry evidence\n'
                      '        if: always()\n'
                      '        uses: actions/upload-artifact@v4\n'
                      '        with:\n'
                      '          name: current-typed-registry\n'
                      '          path: ${{ runner.temp }}/current-typed-registry\n'
                      '          include-hidden-files: true\n'
                      '          if-no-files-found: error\n'
                      '          retention-days: 14\n', native)
        self.assertNotRegex(native, r'(?m)^    (?:if|needs):|continue-on-error:')

    def test_native_numeric_shard_preserves_the_complete_workload_and_evidence(self):
        primary = block(self.ci, 'native-projects', 2)
        shard = block(self.ci, 'native-numeric-and-packages', 2)
        step = 'Verify real package application and source-free deployment'
        self.assertNotIn(step, primary)
        match = re.search(
            rf'^      - name: {re.escape(step)}\n        run: \|\n(.*?)(?=^      - |\Z)',
            shard, re.MULTILINE | re.DOTALL)
        self.assertIsNotNone(match)
        commands = textwrap.dedent(match.group(1)).strip() + '\n'
        # Frozen complete command body from the previously executed native job.
        self.assertEqual(hashlib.sha256(commands.encode()).hexdigest(),
                         '1bd38ea49faf0c575f2b1a7900ae07c9f62dbeb0dac151a30374e480c8438c31')
        outputs = re.findall(r'--output "\$RUNNER_TEMP/([^"]+)"', commands)
        self.assertEqual(len(outputs), 14)
        self.assertEqual(len(set(outputs)), 14)
        for name in outputs:
            self.assertIn('${{ runner.temp }}/' + name, shard)
        for job in (primary, shard):
            self.assertIn('timeout-minutes: 120', job)
            self.assertNotRegex(job, r'(?m)^    (?:if|needs):|continue-on-error:')
        self.assertIn('image: debian:trixie-slim', shard)
        self.assertIn('uses: dtolnay/rust-toolchain@1.99.0', shard)
        self.assertIn('cargo build --locked\n', shard)
        self.assertIn('cargo build --release --locked\n', shard)
        self.assertIn('if: always()', shard)
        self.assertIn('name: current-native-numeric-and-packages', shard)
        self.assertIn('Run current independent native oracles and private LLVM regressions', primary)

        predecessor = block(self.ci, 'source-policy-predecessor', 2)
        self.assertIn('ref: 2e3fe21f76d0027d2b24cb5c7f02ba38ad354809', predecessor)
        self.assertIn('scripts/run_predecessor_script_tests.py', predecessor)
        self.assertIn('source-policy-replay.py" --repo .', predecessor)
        self.assertIn('name: byte-buffer-source-policy-predecessor', predecessor)
        self.assertNotRegex(predecessor, r'(?m)^    if:|continue-on-error:')

        hosted = block(self.ci, 'hosted-source-policy-predecessor', 2)
        self.assertIn('ref: 559a6e6aa0b2ab2022d8fa37546ac62aeac9954e', hosted)
        self.assertIn('scripts/run_hosted_predecessor_tests.py', hosted)
        self.assertIn('hosted-source-policy-replay.py" --repo .', hosted)
        self.assertIn('name: hosted-source-policy-predecessor', hosted)
        self.assertNotRegex(hosted, r'(?m)^    if:|continue-on-error:')

        transfer = block(self.ci, 'buffer-transfer-source-policy-predecessor', 2)
        self.assertIn('ref: c45d519127d314f522762c35879c0c485994ea42', transfer)
        self.assertIn('scripts/run_buffer_transfer_predecessor_tests.py', transfer)
        self.assertIn('buffer-transfer-source-policy-replay.py" --repo .', transfer)
        self.assertIn('name: buffer-transfer-source-policy-predecessor', transfer)
        self.assertNotRegex(transfer, r'(?m)^    if:|continue-on-error:')

        literals = block(self.ci, 'byte-string-source-policy-predecessor', 2)
        self.assertIn('ref: 22d04a015a444edcb600a6011cdb0e2c8529cb34', literals)
        self.assertIn('scripts/run_byte_string_predecessor_tests.py', literals)
        self.assertIn('byte-string-source-policy-replay.py" --repo .', literals)
        self.assertIn('name: byte-string-source-policy-predecessor', literals)
        self.assertNotRegex(literals, r'(?m)^    if:|continue-on-error:')

        arm64 = block(self.ci, 'arm64-host-source-policy-predecessor', 2)
        self.assertIn('ref: 18314e95d6afa3d8a4da7970dbddc828f4f6cbac', arm64)
        self.assertIn('fetch-depth: 0', arm64)
        self.assertIn('scripts/run_arm64_host_predecessor_tests.py', arm64)
        self.assertIn('arm64-host-source-policy-replay.py" --repo .', arm64)
        self.assertIn('name: arm64-host-source-policy-predecessor', arm64)
        self.assertNotRegex(arm64, r'(?m)^    if:|continue-on-error:')

    def test_historical_provenance_reads_are_authorized_without_event_spoofing(self):
        for workflow in (self.ci, self.release, self.historical):
            self.assertIn('actions: read', block(workflow, 'permissions'))
        self.assertNotRegex(self.historical, r'(?m)^\s*GITHUB_SHA:|GITHUB_SHA=|GITHUB_EVENT_PATH=')


    def exercise_cross_platform_exit_propagation(self, name):
        ci = block(self.ci, 'cross-platform', 2)
        steps = re.findall(
            rf'^      - name: {re.escape(name)}\n(.*?)(?=^      - |\Z)',
            ci, re.MULTILINE | re.DOTALL,
        )
        self.assertEqual(len(steps), 1, name)
        self.assertRegex(steps[0], r'(?m)^        shell: bash$')
        run = re.search(r'^        run: \|\n((?:          .*\n?)+)',
                        steps[0], re.MULTILINE)
        self.assertIsNotNone(run, name)
        body = '\n'.join(line[10:] for line in run.group(1).splitlines()) + '\n'
        lines = body.splitlines()
        self.assertEqual(lines[0], 'set -euo pipefail')
        commands = [shlex.split(line) for line in lines[1:]]
        self.assertGreater(len(commands), 1)
        self.assertTrue(all(command and command[0] in ('cargo', 'python')
                            for command in commands))
        bash = shutil.which('bash')
        self.assertIsNotNone(bash, 'CI exit controls require Bash')
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            tools = directory / 'bin'
            tools.mkdir()
            trace = directory / 'commands.tsv'
            stub = f'#!{bash}\n' + r'''
set -u
count=0
while IFS= read -r previous; do count=$((count + 1)); done < "$CI_TRACE"
printf '%s\t' "${0##*/}" "$@" >> "$CI_TRACE"
printf '\n' >> "$CI_TRACE"
if test "$((count + 1))" = "$CI_FAIL_AT"; then exit 47; fi
exit 0
'''
            for command in ('cargo', 'python'):
                executable = tools / command
                executable.write_text(stub, encoding='utf-8')
                executable.chmod(0o755)
            script = directory / 'actual-ci-step.bash'
            script.write_text(body, encoding='utf-8')
            for failed in range(len(commands) + 1):
                with self.subTest(step=name, failed_command=failed):
                    trace.write_text('', encoding='utf-8')
                    result = subprocess.run(
                        [bash, '--noprofile', '--norc', str(script)],
                        cwd=directory,
                        env={'PATH': str(tools), 'CI_TRACE': str(trace),
                             'CI_FAIL_AT': str(failed)},
                        capture_output=True, timeout=10,
                    )
                    self.assertEqual(result.returncode, 47 if failed else 0,
                                     (result.stdout, result.stderr))
                    observed = [line.split('\t')[:-1]
                                for line in trace.read_text(encoding='utf-8').splitlines()]
                    self.assertEqual(observed, commands[:failed] if failed else commands)

    def test_cross_platform_compiler_step_propagates_every_failed_command(self):
        self.exercise_cross_platform_exit_propagation('Build and test current compiler')

    def test_cross_platform_benchmark_step_propagates_every_failed_command(self):
        self.exercise_cross_platform_exit_propagation('Exercise current benchmark reporting')


NATIVE_EVIDENCE_DIRECTORIES = (
    'current-native', 'current-reproducible-native', 'current-native-profiles',
    'current-owned-enum-application', 'current-local-references-debug',
    'current-local-references-release', 'current-local-reference-bindings',
    'current-generic-field-borrows', 'current-native-reachability',
    'current-byte-storage-debug', 'current-byte-storage-release',
    'current-byte-fields-debug', 'current-byte-fields-release',
    'current-byte-buffers-debug', 'current-byte-buffers-release',
    'current-byte-reverse-application', 'current-typed-vector-native',
    'current-copy-record-native', 'current-copy-record-vector-native',
    'current-owned-record-vector-native', 'current-owned-text-native',
    'current-utf8-characters-native', 'current-utf8-text-application',
    'current-native-files', 'current-cpu-training', 'current-optimizer-math',
    'current-optimizer-checkpoints', 'current-optimizer-training',
    'current-data-pipeline', 'current-minibatch-training',
    'current-streaming-training', 'current-seekable-streaming',
    'current-native-libraries',
)


def require_native_project_evidence(native):
    checkout = ('      - uses: actions/checkout@v4\n'
                '        with:\n'
                '          ref: ${{ github.event.pull_request.head.sha || github.sha }}\n'
                '          fetch-depth: 0\n')
    if native.count('actions/checkout@') != 1 or checkout not in native:
        raise AssertionError('native policy reads require current-head complete history')
    capture = ('      - name: Record the native evidence root inside the container\n'
               '        run: printf \'OXID_NATIVE_EVIDENCE_ROOT=%s\\n\' "$RUNNER_TEMP" >> "$GITHUB_ENV"\n')
    if (native.count(capture) != 1
            or native.index(capture) > native.index('      - name: Install pinned LLVM and host tools\n')):
        raise AssertionError('native evidence must bind the actual container root before fallible work')
    steps = re.findall(r'^      - name: Retain current native evidence\n(.*?)(?=^      - |\Z)',
                       native, re.MULTILINE | re.DOTALL)
    if len(steps) != 1:
        raise AssertionError('expected one current native evidence upload')
    upload = steps[0]
    paths = re.findall(r'^            (.+)$', upload, re.MULTILINE)
    if tuple(paths) != tuple('${{ env.OXID_NATIVE_EVIDENCE_ROOT }}/' + name
                             for name in NATIVE_EVIDENCE_DIRECTORIES):
        raise AssertionError('native evidence directory roster or container root changed')
    expected = ('        if: always()\n'
                '        uses: actions/upload-artifact@v4\n'
                '        with:\n'
                '          name: current-native\n'
                '          path: |\n'
                + ''.join('            ' + path + '\n' for path in paths)
                + '          include-hidden-files: true\n'
                  '          if-no-files-found: error\n'
                  '          retention-days: 14\n')
    if upload.strip() != expected.strip():
        raise AssertionError('native upload must retain failure evidence and hidden inputs without suppressing missing files')
    debug = ('      - name: Retain Copy record debugger binaries and transcripts\n'
             '        if: always()\n'
             '        uses: actions/upload-artifact@v4\n'
             '        with:\n'
             '          name: current-copy-record-debug\n'
             '          path: ${{ env.OXID_NATIVE_EVIDENCE_ROOT }}/current-copy-record-debug-*\n'
             '          include-hidden-files: true\n'
             '          if-no-files-found: error\n'
             '          retention-days: 14')
    if native.count(debug) != 1:
        raise AssertionError('debugger upload must use the same actual root and retain failure evidence')
    return capture.split('        run: ', 1)[1].strip(), paths


class NativeProjectEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        ci = (REPO / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        cls.native = block(ci, 'native-projects', 2)

    def test_native_checkout_and_complete_explicit_container_evidence_roster(self):
        _, paths = require_native_project_evidence(self.native)
        self.assertEqual(len(paths), 33)
        self.assertEqual(len(set(paths)), 33)
        self.assertNotIn('${{ runner.temp }}', '\n'.join(paths))
        self.assertFalse(any('*' in path or '..' in path for path in paths))

    def test_shallow_local_git_cannot_read_a_policy_ancestor_until_history_is_fetched(self):
        git = shutil.which('git')
        self.assertIsNotNone(git, 'native history controls require Git')
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            upstream, shallow = root / 'upstream', root / 'shallow'
            upstream.mkdir()
            hooks = root / 'empty-hooks'
            hooks.mkdir()

            def run(directory, *arguments, check=True):
                command = [git, '-c', 'user.name=Oxid CI', '-c', 'user.email=ci@example.invalid',
                           '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=' + str(hooks),
                           '-c', 'protocol.file.allow=always', *arguments]
                result = subprocess.run(command, cwd=directory, capture_output=True, timeout=10)
                if check:
                    self.assertEqual(result.returncode, 0, (command, result.stdout, result.stderr))
                return result

            run(upstream, 'init', '--initial-branch=main')
            policy = upstream / 'policy.py'
            policy.write_bytes(b'original frozen policy\n')
            run(upstream, 'add', 'policy.py')
            run(upstream, 'commit', '-m', 'Frozen policy')
            parent = run(upstream, 'rev-parse', 'HEAD').stdout.decode().strip()
            policy.write_bytes(b'current policy\n')
            run(upstream, 'commit', '-am', 'Current source')
            run(root, 'clone', '--depth=1', '--no-local', upstream.as_uri(), str(shallow))
            self.assertEqual(run(shallow, 'rev-parse', '--is-shallow-repository').stdout, b'true\n')
            missing = run(shallow, 'show', parent + ':policy.py', check=False)
            self.assertNotEqual(missing.returncode, 0)
            self.assertEqual(missing.stdout, b'')
            run(shallow, 'fetch', '--unshallow', 'origin')
            self.assertEqual(run(shallow, 'rev-parse', '--is-shallow-repository').stdout, b'false\n')
            run(shallow, 'merge-base', '--is-ancestor', parent, 'HEAD')
            self.assertEqual(run(shallow, 'show', parent + ':policy.py').stdout,
                             b'original frozen policy\n')
            require_native_project_evidence(self.native)

    def test_actual_root_capture_survives_a_later_failure_and_resolves_every_upload_line(self):
        command, paths = require_native_project_evidence(self.native)
        bash = shutil.which('bash')
        self.assertIsNotNone(bash, 'native container evidence controls require Bash')
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            container = root / 'container temporary directory'
            container.mkdir()
            environment = root / 'GITHUB_ENV'
            result = subprocess.run([bash, '--noprofile', '--norc', '-c', command + '\nexit 47\n'],
                                    env={'RUNNER_TEMP': str(container), 'GITHUB_ENV': str(environment)},
                                    capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 47, (result.stdout, result.stderr))
            self.assertEqual(environment.read_text(), 'OXID_NATIVE_EVIDENCE_ROOT=' + str(container) + '\n')
            for name, expression in zip(NATIVE_EVIDENCE_DIRECTORIES, paths, strict=True):
                directory = container / name
                directory.mkdir()
                hidden = directory / '.oxid'
                hidden.mkdir()
                (hidden / 'receipt.json').write_text('{}\n')
                resolved = Path(expression.replace('${{ env.OXID_NATIVE_EVIDENCE_ROOT }}', str(container)))
                self.assertEqual(resolved, directory)
                self.assertEqual((resolved / '.oxid/receipt.json').read_text(), '{}\n')

    def test_shallow_history_or_host_path_regressions_are_refused(self):
        mutations = [
            self.native.replace('          fetch-depth: 0\n', '', 1),
            self.native.replace('          fetch-depth: 0\n', '          fetch-depth: 1\n', 1),
            self.native.replace('"$RUNNER_TEMP" >> "$GITHUB_ENV"', '"/host/tmp" >> "$GITHUB_ENV"', 1),
            self.native.replace('${{ env.OXID_NATIVE_EVIDENCE_ROOT }}/current-seekable-streaming',
                                '${{ runner.temp }}/current-seekable-streaming', 1),
            self.native.replace('            ${{ env.OXID_NATIVE_EVIDENCE_ROOT }}/current-owned-text-native\n', '', 1),
            self.native.replace('${{ env.OXID_NATIVE_EVIDENCE_ROOT }}/current-native\n',
                                '${{ env.OXID_NATIVE_EVIDENCE_ROOT }}/**\n', 1),
        ]
        for changed in mutations:
            with self.subTest(changed=changed):
                self.assertNotEqual(changed, self.native)
                with self.assertRaises(AssertionError):
                    require_native_project_evidence(changed)

    def test_native_upload_cannot_skip_failures_hidden_inputs_or_missing_evidence(self):
        original = '      - name: Retain current native evidence\n'
        selected = re.findall(r'^' + re.escape(original)
                              + r'.*?(?=^      - |\Z)',
                              self.native, re.MULTILINE | re.DOTALL)
        self.assertEqual(len(selected), 1)
        upload = selected[0]
        self.assertEqual(self.native.count(upload), 1)
        for before, after in [
            (original + '        if: always()\n', original + '        if: success()\n'),
            ('          include-hidden-files: true\n', '          include-hidden-files: false\n'),
            ('          if-no-files-found: error\n', '          if-no-files-found: warn\n'),
            (original, original + '        continue-on-error: true\n'),
        ]:
            self.assertEqual(upload.count(before), 1)
            changed_upload = upload.replace(before, after, 1)
            self.assertNotEqual(changed_upload, upload)
            changed = self.native.replace(upload, changed_upload, 1)
            self.assertNotEqual(changed, self.native)
            # Earlier uploads may use the same hidden/error settings. Mutate
            # only the selected current-native step and retain every other byte.
            self.assertEqual(self.native.replace(upload, '', 1),
                             changed.replace(changed_upload, '', 1))
            with self.subTest(before=before), self.assertRaises(AssertionError):
                require_native_project_evidence(changed)


class RetainedReplayRouteTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.current = block((REPO / '.github/workflows/ci.yml').read_text(encoding='utf-8'),
                            'quality-and-repository', 2)
        cls.route = retained_replay_route(cls.current)
        cls.command = '          python3 -B ' + ' '.join(cls.route) + ' \\'

    def test_all_five_known_routes_keep_the_same_complete_current_head_contract(self):
        self.assertEqual(self.current.count(self.command), 1)
        for route in REPLAY_ROUTES:
            with self.subTest(route=route):
                changed = self.current.replace(
                    self.command, '          python3 -B ' + ' '.join(route) + ' \\', 1)
                self.assertEqual(retained_replay_route(changed), route)

    def test_text_route_still_binds_the_pinned_owned_vector_wrapper_and_exact_source_membership(self):
        route = (OWNED_TEXT_REPLAY,)
        self.assertEqual(len(REPLAY_ROUTES), 10)
        self.assertEqual(REPLAY_ROUTES[route], ('owned-text', OWNED_VECTOR_POLICY))
        require_retained_replay_binding(route)

    def test_vendor_route_still_binds_the_pinned_text_wrapper_and_exact_source_membership(self):
        route = (OFFLINE_VENDOR_REPLAY,)
        self.assertEqual(len(REPLAY_ROUTES), 10)
        self.assertEqual(REPLAY_ROUTES[route], ('offline-vendor', OWNED_TEXT_POLICY))
        require_retained_replay_binding(route)

    def test_live_enum_route_binds_the_public_vendor_policy_and_full_source_and_observer_pipeline(self):
        route = (OWNED_ENUM_VECTOR_REPLAY,)
        self.assertEqual(len(REPLAY_ROUTES), 10)
        self.assertEqual(REPLAY_ROUTES[route], ('owned-enum-vector', OFFLINE_VENDOR_POLICY))
        require_retained_replay_binding(route)

    def test_generic_route_keeps_the_complete_enum_controller_and_original_observer(self):
        route = (GENERIC_OWNED_VECTOR_REPLAY,)
        self.assertEqual(len(REPLAY_ROUTES), 10)
        self.assertEqual(REPLAY_ROUTES[route], ('generic-owned-vector', OWNED_ENUM_VECTOR_POLICY))
        require_retained_replay_binding(route)

    def test_live_windows_route_keeps_the_complete_generic_and_original_observer_pipeline(self):
        self.assertEqual(self.route, (WINDOWS_LOCAL_PACKAGE_REPLAY,))
        self.assertEqual(REPLAY_ROUTES[self.route], ('windows-local-package', GENERIC_OWNED_VECTOR_POLICY))
        require_retained_replay_binding(self.route)

    def test_windows_source_roster_loader_observer_pipeline_and_refusals_cannot_drift(self):
        wrapper = WINDOWS_LOCAL_PACKAGE_REPLAY
        original = (REPO / wrapper).read_bytes()
        mutations = (
            (WINDOWS_LOCAL_PACKAGE_PARENT.encode(), b'f' * 40),
            (WINDOWS_LOCAL_PACKAGE_REVIEWED_HEAD.encode(), b'e' * 40),
            (b'PREDECESSOR_POLICY_PIN = (43357,', b'PREDECESSOR_POLICY_PIN = (43358,'),
            (b'PINS_PROVISIONAL = False', b'PINS_PROVISIONAL = 1'),
            (b'OBSERVER_PINS_PROVISIONAL = False', b'OBSERVER_PINS_PROVISIONAL = True'),
            (b'len(parent) == 594', b'len(parent) == 593'),
            (b'len(WINDOWS_CHANGED) == 14', b'len(WINDOWS_CHANGED) == 13'),
            (b'len(WINDOWS_ADDED) == 9', b'len(WINDOWS_ADDED) == 8'),
            (b'len(expected) == 603', b'len(expected) == 602'),
            (b'len(compiler) == 500', b'len(compiler) == 499'),
            (b'actual == compiler and len(compiler) == 500', b'True'),
            (b'directories == expected_dirs', b'True'),
            (b"'tests/typed_windows_modules.rs'", b"'tests/unreviewed.rs'"),
            (b'not path.is_symlink() and not any(parent.is_symlink() for parent in path.parents)', b'True'),
            (b'before.st_nlink == 1', b'before.st_nlink >= 1'),
            (b'(len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN', b'True'),
            (b'RETAINED_RUN = predecessor.RETAINED_RUN', b'RETAINED_RUN = library.run'),
            (b'owned_policy = predecessor.owned_policy', b'owned_policy = predecessor'),
            (b'enum_policy = predecessor.predecessor', b'enum_policy = predecessor'),
            (b'"capture_sources":', b'"run":'),
            (b'with predecessor.bound_predecessor() as context:', b'with unreviewed_parent() as context:'),
            (b'context["adapter"].DERIVED_PIN == OBSERVER_DERIVED_PIN', b'True'),
            (b'setattr(library, name, value)', b'setattr(predecessor, name, value)'),
            (b'report.update(owned_policy.run_owned_vector_observer_controls(',
             b'report.update(owned_policy.unreviewed_controls('),
            (b'report.update(enum_policy.run_enum_observer_controls(',
             b'report.update(enum_policy.unreviewed_controls('),
            (b'context["adapter"], report["test_function_executions"]))',
             b'context["adapter"], retained["test_function_executions"]))'),
            (b'retained.get("current_input_members") == 603', b'retained.get("current_input_members") == 594'),
            (b'"windows_local_package_semantics_qualified_by_this_gate": False',
             b'"windows_local_package_semantics_qualified_by_this_gate": True'),
            (b'"windows_local_package_ntfs_qualified_by_this_gate": False',
             b'"windows_local_package_ntfs_qualified_by_this_gate": True'),
            (b'"windows_local_package_races_qualified_by_this_gate": False',
             b'"windows_local_package_races_qualified_by_this_gate": True'),
            (b'"windows_local_package_platform_qualified_by_this_gate": False',
             b'"windows_local_package_platform_qualified_by_this_gate": True'),
            (b'"new_feature_qualification": False', b'"new_feature_qualification": True'),
            (b'"inherited_input_members": 594', b'"inherited_input_members": 587'),
            (b'"expected_current_input_members": 603', b'"expected_current_input_members": 594'),
            (b'args.prepare_only or not PINS_PROVISIONAL', b'True'),
            (b'args.prepare_only or not OBSERVER_PINS_PROVISIONAL', b'True'),
            (b'args.prepare_only or not predecessor.PINS_PROVISIONAL', b'True'),
        )
        for before, after in mutations:
            with self.subTest(before=before):
                changed = original.replace(before, after, 1)
                self.assertNotEqual(changed, original, 'mutation must reach the real Windows-package controller')
                with self.assertRaises(AssertionError):
                    require_retained_replay_binding(
                        (wrapper,), lambda name: changed if name == wrapper else (REPO / name).read_bytes())

    def test_windows_route_rejects_drift_in_all_twenty_three_compiled_input_pins(self):
        source = ast.parse((REPO / WINDOWS_LOCAL_PACKAGE_REPLAY).read_bytes())
        pins = _literal(source.body, 'REVIEWED_INPUTS')
        self.assertEqual(len(pins), 23)
        for member in pins:
            with self.subTest(member=member), self.assertRaisesRegex(AssertionError, 'compiled input differs'):
                require_retained_replay_binding(
                    (WINDOWS_LOCAL_PACKAGE_REPLAY,),
                    lambda name: (REPO / name).read_bytes() + (b'\n' if name == member else b''))

    def test_generic_inventory_assets_observer_pipeline_and_source_refusals_cannot_drift(self):
        wrapper = GENERIC_OWNED_VECTOR_REPLAY
        original = (REPO / wrapper).read_bytes()
        mutations = (
            (GENERIC_OWNED_VECTOR_PARENT.encode(), b'f' * 40),
            (b"PREDECESSOR_POLICY_PIN = (67623,", b"PREDECESSOR_POLICY_PIN = (67624,"),
            (b'len(parent) == 587', b'len(parent) == 586'),
            (b'len(GENERIC_CHANGED) == 7', b'len(GENERIC_CHANGED) == 6'),
            (b'len(GENERIC_ADDED) == 3', b'len(GENERIC_ADDED) == 4'),
            (b'len(expected) == 594', b'len(expected) == 595'),
            (b"PUBLIC_TEST = 'tests/typed_owned_vector_wrappers.rs'", b"PUBLIC_TEST = 'tests/unreviewed.rs'"),
            (b"'../packages/owned_vec/oxid.toml'", b"'../packages/unreviewed/oxid.toml'"),
            (b'RETAINED_RUN = predecessor.RETAINED_RUN', b'RETAINED_RUN = library.run'),
            (b'owned_policy = predecessor.owned_policy', b'owned_policy = predecessor'),
            (b'"capture_sources":', b'"run":'),
            (b'with predecessor.bound_predecessor() as context:', b'with unreviewed_parent() as context:'),
            (b'context["adapter"].DERIVED_PIN == OBSERVER_DERIVED_PIN', b'True'),
            (b'setattr(library, name, value)', b'setattr(predecessor, name, value)'),
            (b'report.update(owned_policy.run_owned_vector_observer_controls(',
             b'report.update(owned_policy.unreviewed_controls('),
            (b'report.update(predecessor.run_enum_observer_controls(',
             b'report.update(predecessor.unreviewed_controls('),
            (b'context["adapter"], report["test_function_executions"]))',
             b'context["adapter"], retained["test_function_executions"]))'),
            (b'"g2_complete_qualified_by_this_gate": False', b'"g2_complete_qualified_by_this_gate": True'),
            (b'"new_feature_qualification": False', b'"new_feature_qualification": True'),
            (b'args.prepare_only or not PINS_PROVISIONAL', b'True'),
            (b'args.prepare_only or not OBSERVER_PINS_PROVISIONAL', b'True'),
        )
        for before, after in mutations:
            with self.subTest(before=before):
                changed = original.replace(before, after, 1)
                self.assertNotEqual(changed, original, 'mutation must reach the real generic-vector controller')
                with self.assertRaises(AssertionError):
                    require_retained_replay_binding(
                        (wrapper,), lambda name: changed if name == wrapper else (REPO / name).read_bytes())

    def test_enum_parent_inventory_binding_observer_controls_and_provisional_refusal_cannot_drift(self):
        wrapper = OWNED_ENUM_VECTOR_REPLAY
        original = (REPO / wrapper).read_bytes()
        mutations = (
            (OWNED_ENUM_VECTOR_PARENT.encode(), b'f' * 40),
            (b'PREDECESSOR_POLICY_PIN = (27286,', b'PREDECESSOR_POLICY_PIN = (27287,'),
            (b'len(parent) == 576', b'len(parent) == 575'),
            (b'len(ENUM_CHANGED) == 45', b'len(ENUM_CHANGED) == 44'),
            (b'len(ENUM_ADDED) == 11', b'len(ENUM_ADDED) == 12'),
            (b'len(expected) == 587', b'len(expected) == 588'),
            (b'"native/experimental_owned_record.c": (11933,', b'"native/unreviewed.c": (11933,'),
            (b'RETAINED_RUN = predecessor.RETAINED_RUN', b'RETAINED_RUN = library.run'),
            (b'owned_policy = predecessor.predecessor.predecessor', b'owned_policy = predecessor'),
            (b'"capture_sources":', b'"run":'),
            (b'with bound_observer(), predecessor.bound_predecessor() as context:',
             b'with predecessor.bound_predecessor() as context:'),
            (b'"OWNED_VECTOR_OBSERVER_ADAPTER_PIN": OBSERVER_ADAPTER_PIN,',
             b'"OWNED_VECTOR_OBSERVER_ADAPTER_PIN": V13_ADAPTER_PIN,'),
            (b'setattr(owned_policy, name, value)', b'setattr(library, name, value)'),
            (b'report.update(owned_policy.run_owned_vector_observer_controls(',
             b'report.update(owned_policy.unreviewed_controls('),
            (b'report.update(run_enum_observer_controls(', b'report.update(unreviewed_enum_controls('),
            (b'context["adapter"], report["test_function_executions"]))',
             b'context["adapter"], retained["test_function_executions"]))'),
            (b'for profile in ("debug", "release"):', b'for profile in ("debug",):'),
            (b'protocol.rust_listing(streams["list"], names)', b'pass'),
            (b'result = protocol.rust_success(streams["run"], names)', b'result = {"passed": 3}'),
            (b'"test_function_executions": previous_executions + 6',
             b'"test_function_executions": previous_executions'),
            (b'"g2_complete_qualified_by_this_gate": False', b'"g2_complete_qualified_by_this_gate": True'),
            (b'"new_feature_qualification": False', b'"new_feature_qualification": True'),
            (b'args.prepare_only or not PINS_PROVISIONAL', b'True'),
            (b'args.prepare_only or not OBSERVER_PINS_PROVISIONAL', b'True'),
        )
        for before, after in mutations:
            with self.subTest(before=before):
                changed = original.replace(before, after, 1)
                self.assertNotEqual(changed, original, 'mutation must reach the real enum-vector controller')
                with self.assertRaises(AssertionError):
                    require_retained_replay_binding(
                        (wrapper,), lambda name: changed if name == wrapper else (REPO / name).read_bytes())

    def test_enum_observer_cannot_replace_the_reversible_pinned_adapter(self):
        observer = OWNED_ENUM_VECTOR_OBSERVER[0]
        original = (REPO / observer).read_bytes()
        for before, after in ((b'def reverse(derived):', b'def unreviewed_reverse(derived):'),
                              (b'inherited = predecessor.adapt(original)', b'inherited = original'),
                              (b'return result', b'return derived')):
            with self.subTest(before=before):
                changed = original.replace(before, after, 1)
                self.assertNotEqual(changed, original)
                with self.assertRaisesRegex(AssertionError, 'immutable enum-vector observer adapter changed'):
                    require_retained_replay_binding(
                        self.route, lambda name: changed if name == observer else (REPO / name).read_bytes())

    def test_vendor_parent_membership_dispatch_package_scope_and_observer_cannot_drift(self):
        wrapper = OFFLINE_VENDOR_REPLAY
        original = (REPO / wrapper).read_bytes()
        mutations = (
            (OFFLINE_VENDOR_PARENT.encode(), b'f' * 40),
            (b'PREDECESSOR_POLICY_PIN = (16603,', b'PREDECESSOR_POLICY_PIN = (16604,'),
            (b'len(parent) == 574', b'len(parent) == 573'),
            (b'len(expected) == 576', b'len(expected) == 577'),
            (b'RETAINED_RUN = predecessor.RETAINED_RUN', b'RETAINED_RUN = library.run'),
            (b'inputs[CLI] == approved_cli(parent[CLI])', b'True'),
            (b'inputs[MAIN] == approved_main(parent[MAIN])', b'True'),
            (b'inputs[TEXT_RESOURCE_TEST] == approved_text_resource_test(parent[TEXT_RESOURCE_TEST])', b'True'),
            (b'inputs[VECTOR_ROUTE_TEST] == approved_vector_route_test(parent[VECTOR_ROUTE_TEST])', b'True'),
            (b'VENDOR_CHANGED = frozenset((CLI, MAIN, PACKAGE, TEXT_RESOURCE_TEST, VECTOR_ROUTE_TEST))',
             b'VENDOR_CHANGED = frozenset((CLI, MAIN, PACKAGE))'),
            (b'insertion = unchanged_package(parent[PACKAGE], inputs[PACKAGE])', b'insertion = b""'),
            (b'with predecessor.bound_predecessor() as context:', b'with unreviewed_parent() as context:'),
            (b'predecessor.predecessor.run_owned_vector_observer_controls(', b'predecessor.unreviewed_controls('),
            (b'args.prepare_only or not PINS_PROVISIONAL', b'True'),
        )
        for before, after in mutations:
            with self.subTest(before=before):
                changed = original.replace(before, after, 1)
                self.assertNotEqual(changed, original, 'mutation must reach the real vendor controller')
                with self.assertRaises(AssertionError):
                    require_retained_replay_binding(
                        (wrapper,), lambda name: changed if name == wrapper else (REPO / name).read_bytes())

    def test_text_parent_cardinality_hooks_observer_and_provisional_guard_cannot_drift(self):
        wrapper = OWNED_TEXT_REPLAY
        original = (REPO / wrapper).read_bytes()
        mutations = (
            (b'0e8f23f7481c7ec4e24cd416ed30607f8eb75daf', b'f' * 40),
            (b'PREDECESSOR_POLICY_PIN = (110006,', b'PREDECESSOR_POLICY_PIN = (110007,'),
            (b'len(parent) == 572', b'len(parent) == 571'),
            (b'len(expected) == 574', b'len(expected) == 575'),
            (b'RETAINED_RUN = predecessor.RETAINED_RUN', b'RETAINED_RUN = library.run'),
            (b'inputs[PROGRAM] == approved_program(parent[PROGRAM])', b'True'),
            (b'with predecessor.bound_registry() as context:', b'with unreviewed_registry() as context:'),
            (b'predecessor.run_owned_vector_observer_controls(', b'predecessor.unreviewed_controls('),
            (b'args.prepare_only or not PINS_PROVISIONAL', b'True'),
        )
        for before, after in mutations:
            with self.subTest(before=before):
                changed = original.replace(before, after, 1)
                self.assertNotEqual(changed, original, 'mutation must reach the real text controller')
                with self.assertRaises(AssertionError):
                    require_retained_replay_binding(
                        (wrapper,), lambda name: changed if name == wrapper else (REPO / name).read_bytes())

    def test_missing_duplicate_unknown_or_bypassed_replay_commands_are_refused(self):
        malformed = [
            self.current.replace(self.command, '          # ' + self.command.strip(), 1),
            self.current.replace(self.command, self.command + '\n' + self.command, 1),
            self.current.replace(self.command, self.command.replace(self.route[0],
                                 'scripts/verify_unreviewed_compatibility.py'), 1),
            self.current.replace(self.command, self.command[:-2] + ' --prepare-only \\', 1),
            self.current.replace(self.command, self.command[:-2] + ' --allow-dirty \\', 1),
            self.current.replace(self.command, self.command[:-2] + ' --source-policy unreviewed \\', 1),
            self.current.replace(self.command, self.command.replace('python3 -B', 'echo python3 -B'), 1),
            self.current.replace(self.command, self.command.replace('python3 -B', 'if ! python3 -B'), 1),
            self.current.replace('--output "$RUNNER_TEMP/current-project-unit2"',
                                 '--output "$RUNNER_TEMP/current-project-unit2" || true', 1),
            self.current.replace('--output "$RUNNER_TEMP/current-project-unit2"',
                                 '--output "$RUNNER_TEMP/current-project-unit2"; exit 0', 1),
            self.current.replace('            --expected-head "$EXPECTED_HEAD" \\\n', '', 1),
            self.current.replace('          ref: ${{ github.event.pull_request.head.sha || github.sha }}',
                                 '          ref: main', 1),
            self.current.replace('        env:\n          EXPECTED_HEAD:',
                                 '        if: false\n        env:\n          EXPECTED_HEAD:', 1),
            self.current + '\n      - run: true\n        continue-on-error: true\n',
        ]
        for changed in malformed:
            with self.subTest(source=changed):
                self.assertNotEqual(changed, self.current, 'mutation must reach the real workflow')
                with self.assertRaises(AssertionError):
                    retained_replay_route(changed)

    def test_selected_immutable_policy_cannot_be_replaced_even_with_the_same_route(self):
        _, (policy, _) = REPLAY_ROUTES[self.route]
        original = (REPO / policy).read_bytes()
        with self.assertRaisesRegex(AssertionError, 'immutable retained policy changed'):
            require_retained_replay_binding(
                self.route, lambda name: original + b'\n' if name == policy else (REPO / name).read_bytes())

    def test_wrapper_parent_runner_scope_and_failure_refusal_cannot_drift(self):
        kind, _ = REPLAY_ROUTES[self.route]
        if kind == 'direct':
            # Direct routes already bind the complete immutable original runner.
            require_retained_replay_binding(self.route)
            return
        wrapper = self.route[0]
        original = (REPO / wrapper).read_bytes()
        invocation = ('selected.original_run(delegated)' if kind == 'msvc'
                      else 'RETAINED_RUN(delegated)')
        parent = (WINDOWS_LOCAL_PACKAGE_PARENT.encode() if kind == 'windows-local-package'
                  else GENERIC_OWNED_VECTOR_PARENT.encode() if kind == 'generic-owned-vector'
                  else OWNED_ENUM_VECTOR_PARENT.encode() if kind == 'owned-enum-vector'
                  else OFFLINE_VENDOR_PARENT.encode() if kind == 'offline-vendor'
                  else b'0e8f23f7481c7ec4e24cd416ed30607f8eb75daf' if kind == 'owned-text'
                  else b'af0fe5e1fbaf4fb49b62e894a53ad4b5ed307ee7')
        mutations = [
            (parent, b'f' * 40),
            (invocation.encode(), b'0'),
            (b'status == 0', b'status == 0 or True'),
            (b'"capture_sources":', b'"run":'),
        ]
        for before, after in mutations:
            with self.subTest(before=before):
                changed = original.replace(before, after, 1)
                self.assertNotEqual(changed, original, 'mutation must reach the real controller')
                with self.assertRaises(AssertionError):
                    require_retained_replay_binding(
                        self.route, lambda name: changed if name == wrapper else (REPO / name).read_bytes())


if __name__ == '__main__':
    unittest.main()
