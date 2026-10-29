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
REPLAY_ROUTES = {
    (SCALAR_REPLAY[0],): ('direct', SCALAR_REPLAY),
    (REGISTRY_REPLAY[0],): ('direct', REGISTRY_REPLAY),
    (MSVC_REPLAY, '--source-policy', 'scalar-library'): ('msvc', SCALAR_REPLAY),
    (MSVC_REPLAY, '--source-policy', 'registry'): ('msvc', REGISTRY_REPLAY),
    (OWNED_VECTOR_REPLAY,): ('owned-vector', REGISTRY_REPLAY),
    (OWNED_TEXT_REPLAY,): ('owned-text', OWNED_VECTOR_POLICY),
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
        self.assertIn('${{ runner.temp }}/current-seekable-streaming', native)
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
        self.assertIn('${{ runner.temp }}/current-native-libraries', native)
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

    def test_live_text_route_binds_the_pinned_owned_vector_wrapper_and_exact_source_membership(self):
        self.assertEqual(self.route, (OWNED_TEXT_REPLAY,))
        self.assertEqual(len(REPLAY_ROUTES), 6)
        self.assertEqual(REPLAY_ROUTES[self.route], ('owned-text', OWNED_VECTOR_POLICY))
        require_retained_replay_binding(self.route)

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
        parent = (b'0e8f23f7481c7ec4e24cd416ed30607f8eb75daf' if kind == 'owned-text'
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
