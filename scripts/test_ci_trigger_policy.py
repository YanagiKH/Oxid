"""Keep CI entry points explicit without adding a YAML dependency.

These source-contract checks intentionally follow the workflows' block style;
actionlint remains the YAML and GitHub Actions syntax validator.
"""
from pathlib import Path
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
        self.assertIn('scripts/verify_registry_compatibility.py', current)
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

    def test_private_registry_acceptance_is_mandatory_and_keeps_native_evidence(self):
        native = block(self.ci, 'native-numeric-and-packages', 2)
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


if __name__ == '__main__':
    unittest.main()
