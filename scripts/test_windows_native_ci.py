"""Keep actual Windows native qualification mandatory without a YAML dependency.

These checks follow the workflow's block style, like test_ci_trigger_policy.py.
Source checks run on every host; Windows also executes the SDK command against
a temporary batch fixture. No compiler or real SDK is executed by these tests.
"""
from pathlib import Path
import os
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest

from test_ci_trigger_policy import block


REPO = Path(__file__).resolve().parents[1]
JOB = 'current-windows-native'
SDK_STEP = 'Initialize the installed x64 Visual Studio and Windows SDK'
LLVM_STEP = 'Verify and extract the complete LLVM 19.1.7 toolchain'
BUILD_STEP = 'Build both compiler profiles'
TEST_STEP = 'Verify Windows diagnosis, capture limits and source admission'
VERIFY_STEP = 'Verify PE execution, publication and standalone deployment'
UPLOAD_STEP = 'Retain PE programs and raw transcripts'
EXIT_CHECK = 'if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }'
DOCTOR_PREFIX = 'frontend::native::doctor::windows::tests::'
DOCTOR_COMMAND = f'cargo test --locked --bin oxid {DOCTOR_PREFIX} -- --test-threads=1'
SCOPE_COMMAND = ('cargo test --locked --test native_doctor --test typed_windows_native '
                 '-- --include-ignored --test-threads=1')
VERIFIER_COMMAND = ('python -B scripts/verify_windows_native.py '
                    'target/debug/oxid.exe target/release/oxid.exe '
                    '--llvm-bin "$env:OXID_LLVM_BIN" '
                    '--output "$env:RUNNER_TEMP/current-windows-native"')
DOCTOR_TESTS = (
    'byte_pipe_capture_remains_nonblocking_in_a_multithreaded_process',
    'capture_keeps_the_inclusive_per_stream_limit',
    'a_sleeping_probe_has_a_hard_deadline_and_no_success_result',
    'environment_uses_unicode_ordinal_names_and_keeps_raw_values',
    'process_arguments_keep_empty_unicode_quotes_and_trailing_backslashes',
)

# Preserve the executable initialization and authentication checks, while allowing
# indentation, blank lines, and explanatory comments to change independently.
SDK_COMMANDS = r'''
$ErrorActionPreference = 'Stop'
python -B scripts/test_windows_native_ci.py
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or !$installation) { throw 'Installed MSVC x64 tools are required' }
$devcmd = Join-Path $installation 'Common7/Tools/VsDevCmd.bat'
$PSNativeCommandArgumentPassing = 'Legacy'
$settings = & $env:ComSpec /d /s /c "`"$devcmd`" -no_logo -arch=x64 -host_arch=x64 >nul && set"
if ($LASTEXITCODE -ne 0) { throw 'Visual Studio environment initialization failed' }
$names = @('Path', 'INCLUDE', 'LIB', 'LIBPATH', 'VCINSTALLDIR', 'VCToolsInstallDir',
  'VCToolsVersion', 'WindowsSdkDir', 'WindowsSDKVersion', 'WindowsSDKLibVersion',
  'UniversalCRTSdkDir', 'UCRTVersion', 'VSCMD_ARG_TGT_ARCH', 'VSCMD_ARG_HOST_ARCH')
foreach ($line in $settings) {
  if ($line -match '^([^=][^=]*)=(.*)$' -and $names -contains $Matches[1]) {
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "$($Matches[1])=$($Matches[2])" -Encoding utf8
  }
}
'''
LLVM_COMMANDS = r'''
$ErrorActionPreference = 'Stop'
$archive = Join-Path $env:RUNNER_TEMP 'clang+llvm-19.1.7-x86_64-pc-windows-msvc.tar.xz'
$url = 'https://github.com/llvm/llvm-project/releases/download/llvmorg-19.1.7/clang%2Bllvm-19.1.7-x86_64-pc-windows-msvc.tar.xz'
Invoke-WebRequest -Uri $url -OutFile $archive
if ((Get-Item -LiteralPath $archive).Length -ne 845236708) { throw 'LLVM archive length differs' }
$digest = 'b4557b4f012161f56a2f5d9e877ab9635cafd7a08f7affe14829bd60c9d357f0'
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $digest) {
  throw 'LLVM archive SHA256 differs'
}
$extract = Join-Path $env:RUNNER_TEMP 'oxid-llvm-msvc'
New-Item -ItemType Directory -Path $extract | Out-Null
tar -xJf $archive -C $extract
if ($LASTEXITCODE -ne 0) { throw 'LLVM extraction failed' }
$bin = Join-Path $extract 'clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin'
foreach ($tool in @('clang.exe', 'opt.exe', 'lld-link.exe')) {
  $version = & (Join-Path $bin $tool) --version
  if ($LASTEXITCODE -ne 0 -or ($version -join "`n") -notmatch '(?m)(clang version|LLVM version|LLD) 19\.1\.7(?:[ (]|$)') {
    throw "Wrong LLVM tool: $tool"
  }
  $version
}
Add-Content -LiteralPath $env:GITHUB_ENV -Value "OXID_LLVM_BIN=$bin" -Encoding utf8
Add-Content -LiteralPath $env:GITHUB_PATH -Value $bin -Encoding utf8
'''


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def scalar(source, key, expected):
    values = re.findall(rf'^{re.escape(key)}: (.*)$', source, re.MULTILINE)
    require(values == [expected], f'{key} must be exactly {expected!r}, found {values!r}')


def command_lines(source):
    """Read actual commands, excluding comments and joining pwsh continuations."""
    result, pending = [], ''
    for raw in source.splitlines():
        line = raw.strip()
        if not line or line.startswith('#'):
            continue
        line = pending + line
        if line.endswith('`'):
            pending = line[:-1].rstrip() + ' '
        else:
            result.append(line)
            pending = ''
    require(not pending, 'unterminated PowerShell continuation')
    return result


def step_commands(step):
    run = block(step, 'run')
    require(run.startswith('|\n'), 'Windows qualification needs an executable run block')
    return command_lines(textwrap.dedent(run[2:]))


def validate_windows_native_job(workflow):
    """Validate only this required job and its executable qualification steps."""
    job = '    ' + block(workflow, JOB, 2) + '\n'
    scalar(textwrap.dedent(job), 'runs-on', 'windows-2022')
    require(not re.search(r'^    if:', job, re.MULTILINE), 'Windows job must be unconditional')
    require(not re.search(r'^\s*(?:-\s+)?continue-on-error:', job, re.MULTILINE),
            'Windows qualification must propagate failure')
    steps = [textwrap.dedent('        ' + item)
             for item in re.findall(r'^      - (.*?)(?=^      - |\Z)',
                                    job, re.MULTILINE | re.DOTALL)]
    require(steps, 'Windows qualification steps are missing')

    def named(name):
        selected = [step for step in steps if re.search(
            rf'^name: {re.escape(name)}$', step, re.MULTILINE)]
        require(len(selected) == 1, f'expected one Windows step {name!r}')
        return selected[0]

    upload = named(UPLOAD_STEP)
    for step in steps:
        conditions = re.findall(r'^if: (.*)$', step, re.MULTILINE)
        require(conditions == (['always()'] if step == upload else []),
                'only retained Windows evidence may have an always() condition')
    checkouts = [step for step in steps if re.search(r'^uses: actions/checkout@v4$', step, re.MULTILINE)]
    require(len(checkouts) == 1, 'Windows qualification needs one current-source checkout')
    scalar(block(checkouts[0], 'with'), 'ref', '${{ github.event.pull_request.head.sha || github.sha }}')

    for name, expected in ((SDK_STEP, SDK_COMMANDS), (LLVM_STEP, LLVM_COMMANDS)):
        step = named(name)
        scalar(step, 'shell', 'pwsh')
        require(step_commands(step) == command_lines(expected),
                f'{name}: initialization and authenticated toolchain checks must execute intact')
    expected_commands = {
        BUILD_STEP: ['cargo build --locked', EXIT_CHECK, 'cargo build --release --locked', EXIT_CHECK],
        TEST_STEP: [DOCTOR_COMMAND, EXIT_CHECK, SCOPE_COMMAND, EXIT_CHECK],
        VERIFY_STEP: [VERIFIER_COMMAND, EXIT_CHECK],
    }
    for name, commands in expected_commands.items():
        require(step_commands(named(name)) == commands,
                f'{name}: required commands and native failure checks must execute intact')

    scalar(upload, 'uses', 'actions/upload-artifact@v4')
    options = textwrap.dedent('  ' + block(upload, 'with'))
    scalar(options, 'name', 'current-windows-native-msvc-x64')
    scalar(options, 'include-hidden-files', 'true')
    scalar(options, 'if-no-files-found', 'error')
    scalar(options, 'retention-days', '14')
    require(block(options, 'path') == (
        '|\n  ${{ runner.temp }}/current-windows-native\n'
        '  !${{ runner.temp }}/current-windows-native/tools/opt.exe'),
        'retain all Windows evidence except the copied opt tool')


def validate_doctor_test_roster(doctor_module, windows_source):
    require(re.search(r'#\[path = "doctor_windows\.rs"\]\s*mod windows;', doctor_module),
            'doctor prefix must address the actual doctor_windows.rs module')
    modules = re.findall(r'^((?:#\[[^\n]+\]\n)+)mod tests \{\n(.*)', windows_source,
                         re.MULTILINE | re.DOTALL)
    require(len(modules) == 1, 'expected one active Windows doctor tests module')
    attributes, tests = modules[0]
    require(attributes == '#[cfg(test)]\n', 'Windows doctor tests module must be active in test builds')
    names = re.findall(r'^    #\[test\]\n    fn ([a-z_]+)\(', tests, re.MULTILINE)
    require(names == list(DOCTOR_TESTS) and len(re.findall(r'#\[test\]', tests)) == 5,
            'doctor prefix must cover exactly the five distinct Windows regression tests')
    require(not re.search(r'^\s*#\[(?:ignore|cfg|cfg_attr)\b', tests, re.MULTILINE),
            'all five Windows doctor regression tests must be active')


class WindowsNativeCiTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ci = (REPO / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        cls.doctor = (REPO / 'src/frontend/native/doctor.rs').read_text(encoding='utf-8')
        cls.windows = (REPO / 'src/frontend/native/doctor_windows.rs').read_text(encoding='utf-8')

    def test_current_windows_native_job_executes_the_mandatory_contract(self):
        validate_windows_native_job(self.ci)

    def test_doctor_prefix_covers_exactly_five_active_unit_tests(self):
        validate_doctor_test_roster(self.doctor, self.windows)

    def reject_mutations(self, mutations):
        # First require the unmodified job to pass, so rejection cannot be caused
        # by an unrelated baseline failure. Every mutation must affect one seam.
        validate_windows_native_job(self.ci)
        job = '  ' + JOB + ':\n    ' + block(self.ci, JOB, 2) + '\n'
        validate_windows_native_job(job)
        for label, old, new in mutations:
            with self.subTest(mutation=label):
                require(old in job, f'mutation {label!r} does not address the current Windows job')
                changed = job.replace(old, new, 1)
                with self.assertRaises(AssertionError):
                    validate_windows_native_job(changed)

    def test_windows_host_and_current_source_checkout_cannot_be_replaced(self):
        self.reject_mutations([
            ('wrong-host', 'runs-on: windows-2022', 'runs-on: ubuntu-latest'),
            ('missing-checkout', 'uses: actions/checkout@v4', 'uses: actions/download-artifact@v4'),
            ('wrong-checkout', '${{ github.event.pull_request.head.sha || github.sha }}', 'main'),
        ])

    def test_removed_replaced_or_unlocked_profile_builds_are_rejected(self):
        self.reject_mutations([
            ('missing-debug', '          cargo build --locked\n', ''),
            ('missing-release', '          cargo build --release --locked\n', ''),
            ('unlocked-debug', 'cargo build --locked', 'cargo build'),
            ('unlocked-release', 'cargo build --release --locked', 'cargo build --release'),
            ('repeated-debug', 'cargo build --release --locked', 'cargo build --locked'),
            ('echo-build', 'cargo build --locked', 'Write-Host "cargo build --locked"'),
            ('comment-build', 'cargo build --locked', '# cargo build --locked'),
            ('build-failure-ignored', EXIT_CHECK, 'if ($LASTEXITCODE -ne 0) { exit 0 }'),
        ])

    def test_wrong_doctor_filter_and_public_integration_targets_are_rejected(self):
        self.reject_mutations([
            ('missing-doctor', DOCTOR_COMMAND, ''),
            ('doctor-integration-target', '--bin oxid ' + DOCTOR_PREFIX,
             '--test oxid ' + DOCTOR_PREFIX),
            ('missing-doctor-binary', '--bin oxid ' + DOCTOR_PREFIX, DOCTOR_PREFIX),
            ('wrong-doctor-binary', '--bin oxid ' + DOCTOR_PREFIX, '--bin another ' + DOCTOR_PREFIX),
            ('wrong-doctor-prefix', DOCTOR_PREFIX, 'frontend::native::doctor::other::tests::'),
            ('partial-doctor-filter', DOCTOR_PREFIX, DOCTOR_PREFIX + DOCTOR_TESTS[0]),
            ('doctor-listed-only', DOCTOR_COMMAND, DOCTOR_COMMAND + ' --list'),
            ('missing-native-doctor', '--test native_doctor ', ''),
            ('missing-public-scope', '--test typed_windows_native ', ''),
            ('wrong-public-target', '--test typed_windows_native', '--test typed_llvm'),
            ('ignored-public-scope', '--include-ignored ', ''),
            ('echo-public-scope', SCOPE_COMMAND, 'Write-Host "' + SCOPE_COMMAND + '"'),
        ])

    def test_sdk_headers_libraries_and_x64_setup_cannot_be_removed(self):
        self.reject_mutations([
            ('missing-sdk-step', SDK_STEP, 'SDK setup disabled'),
            ('wrong-sdk-shell', 'shell: pwsh', 'shell: cmd'),
            ('missing-vc-tools', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', 'Other.Component'),
            ('wrong-sdk-architecture', '-arch=x64 -host_arch=x64', '-arch=x86 -host_arch=x86'),
            ('missing-sdk-invocation-regression', '          python -B scripts/test_windows_native_ci.py\n', ''),
            ('wrong-native-binding', "$PSNativeCommandArgumentPassing = 'Legacy'", "$PSNativeCommandArgumentPassing = 'Standard'"),
            ('double-wrapped-batch', '"`"$devcmd`" -no_logo -arch=x64 -host_arch=x64 >nul && set"',
             '"`"`"$devcmd`" -no_logo -arch=x64 -host_arch=x64 >nul && set`""'),
            ('missing-include', "'INCLUDE', ", ''),
            ('missing-library-path', "'LIB', ", ''),
            ('missing-sdk-lib-version', "'WindowsSDKLibVersion',", ''),
            ('missing-sdk-export', 'Add-Content -LiteralPath $env:GITHUB_ENV -Value '
             '"$($Matches[1])=$($Matches[2])" -Encoding utf8', 'Write-Host "SDK initialized"'),
            ('sdk-errors-ignored', "$ErrorActionPreference = 'Stop'", "$ErrorActionPreference = 'Continue'"),
        ])

    def test_official_llvm_archive_length_sha_and_tools_must_be_verified(self):
        self.reject_mutations([
            ('wrong-download-host', 'https://github.com/llvm/llvm-project/', 'https://example.invalid/llvm/'),
            ('wrong-version', 'llvmorg-19.1.7/', 'llvmorg-19.1.6/'),
            ('download-replaced', 'Invoke-WebRequest -Uri $url -OutFile $archive', 'Write-Host $url'),
            ('wrong-size', '.Length -ne 845236708', '.Length -ne 1'),
            ('length-unchecked', "if ((Get-Item -LiteralPath $archive).Length -ne 845236708) { throw 'LLVM archive length differs' }", ''),
            ('wrong-digest', 'b4557b4f012161f56a2f5d9e877ab9635cafd7a08f7affe14829bd60c9d357f0', '0' * 64),
            ('wrong-hash-algorithm', 'Get-FileHash -Algorithm SHA256', 'Get-FileHash -Algorithm MD5'),
            ('missing-hash-check', '          if ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $digest) {\n'
             "            throw 'LLVM archive SHA256 differs'\n          }\n", ''),
            ('hash-bypassed', ').Hash.ToLowerInvariant() -ne $digest', ').Hash.ToLowerInvariant() -ne $digest -and $false'),
            ('linker-unverified', "'clang.exe', 'opt.exe', 'lld-link.exe'", "'clang.exe', 'opt.exe'"),
            ('wrong-tool-version', '(clang version|LLVM version|LLD) 19\\.1\\.7', '(clang version|LLVM version|LLD) 19\\.1\\.6'),
            ('missing-llvm-export', 'OXID_LLVM_BIN=$bin', 'UNUSED_LLVM_BIN=$bin'),
        ])

    def test_verifier_requires_both_compilers_toolchain_and_evidence_directory(self):
        self.reject_mutations([
            ('missing-debug-compiler', 'target/debug/oxid.exe ', ''),
            ('missing-release-compiler', 'target/release/oxid.exe ', ''),
            ('repeated-debug-compiler', 'target/release/oxid.exe', 'target/debug/oxid.exe'),
            ('wrong-verifier', 'scripts/verify_windows_native.py', 'scripts/verify_repo.py'),
            ('comment-verifier', 'python -B scripts/verify_windows_native.py', '# python -B scripts/verify_windows_native.py'),
            ('wrong-toolchain-argument', '--llvm-bin "$env:OXID_LLVM_BIN"', '--llvm-bin "missing-tools"'),
            ('missing-output', '--output "$env:RUNNER_TEMP/current-windows-native"', ''),
            ('wrong-output', '--output "$env:RUNNER_TEMP/current-windows-native"', '--output "$env:RUNNER_TEMP/elsewhere"'),
        ])

    def test_job_and_step_skips_soft_failures_and_command_bypasses_are_rejected(self):
        self.reject_mutations([
            ('job-condition', '    runs-on: windows-2022', '    if: false\n    runs-on: windows-2022'),
            ('job-soft-failure', '    runs-on: windows-2022', '    continue-on-error: true\n    runs-on: windows-2022'),
            ('step-condition', f'      - name: {BUILD_STEP}', f'      - name: {BUILD_STEP}\n        if: false'),
            ('step-soft-failure', f'      - name: {TEST_STEP}', f'      - name: {TEST_STEP}\n        continue-on-error: true'),
            ('early-success', '          cargo build --locked', '          exit 0\n          cargo build --locked'),
            ('conditional-build', 'cargo build --locked', 'if ($false) { cargo build --locked }'),
            ('masked-build-status', 'cargo build --locked', 'cargo build --locked; exit 0'),
        ])
        for command in ('cargo build --locked', 'cargo build --release --locked',
                        DOCTOR_COMMAND, SCOPE_COMMAND,
                        '--output "$env:RUNNER_TEMP/current-windows-native"'):
            with self.subTest(unchecked_command=command):
                self.reject_mutations([
                    ('missing-native-exit-check', command + '\n          ' + EXIT_CHECK, command),
                ])

    def test_retained_evidence_requires_always_upload_and_missing_files_failure(self):
        self.reject_mutations([
            ('missing-upload', 'uses: actions/upload-artifact@v4', 'uses: actions/download-artifact@v4'),
            ('missing-always', '        if: always()\n', ''),
            ('success-only-upload', 'if: always()', 'if: success()'),
            ('warn-on-missing-evidence', 'if-no-files-found: error', 'if-no-files-found: warn'),
            ('ignore-missing-evidence', 'if-no-files-found: error', 'if-no-files-found: ignore'),
            ('hidden-evidence-omitted', 'include-hidden-files: true', 'include-hidden-files: false'),
            ('wrong-evidence-directory', '            ${{ runner.temp }}/current-windows-native\n', '            ${{ runner.temp }}/unrelated\n'),
            ('all-executables-excluded', '!${{ runner.temp }}/current-windows-native/tools/opt.exe', '!${{ runner.temp }}/current-windows-native/**/*.exe'),
            ('missing-retention', '          retention-days: 14', ''),
        ])

    def test_removed_renamed_ignored_or_additional_doctor_tests_are_rejected(self):
        validate_doctor_test_roster(self.doctor, self.windows)
        for old, new in (
            (f'fn {DOCTOR_TESTS[0]}(', 'fn renamed_regression('),
            ('    #[test]\n', ''),
            ('    #[test]\n', '    #[ignore]\n    #[test]\n'),
            ('    #[test]\n', '    #[cfg(any())]\n    #[test]\n'),
            ('    #[test]\n', '    #[test]\n    fn extra_regression() {}\n    #[test]\n'),
            ('#[cfg(test)]\nmod tests', '#[cfg(any())]\n#[cfg(test)]\nmod tests'),
        ):
            with self.subTest(mutation=new):
                self.assertIn(old, self.windows)
                with self.assertRaises(AssertionError):
                    validate_doctor_test_roster(self.doctor, self.windows.replace(old, new, 1))
        with self.assertRaises(AssertionError):
            validate_doctor_test_roster(self.doctor.replace('doctor_windows.rs', 'other.rs'), self.windows)


class WindowsSdkInvocationTests(unittest.TestCase):
    def sdk_tail(self):
        workflow = (REPO / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        job = '    ' + block(workflow, JOB, 2) + '\n'
        match = re.search(r'^      - name: ' + re.escape(SDK_STEP) +
                          r'\n(.*?)(?=^      - |\Z)', job, re.MULTILINE | re.DOTALL)
        require(match is not None, 'SDK initialization step is required')
        commands = step_commands(textwrap.dedent(match[1]))
        start = commands.index("$devcmd = Join-Path $installation 'Common7/Tools/VsDevCmd.bat'")
        return '\n'.join(commands[start:])

    def test_cmd_s_removes_exactly_the_native_binders_outer_quote_pair(self):
        # PowerShell's Legacy binder wraps arguments with unquoted whitespace;
        # cmd /s then removes the first and last quotes after /c. Model those two
        # independent boundaries, keeping the path's own quotes intact.
        invocation = next(line for line in self.sdk_tail().splitlines()
                          if line.startswith('$settings = '))
        expression = invocation.removeprefix('$settings = & $env:ComSpec /d /s /c ')
        self.assertTrue(expression.startswith('"') and expression.endswith('"'))
        for path in (r'C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat',
                     r'C:\Program Files (x86)\Visual Studio 雪 & tools\Common7\Tools\VsDevCmd.bat'):
            with self.subTest(path=path):
                argument = expression[1:-1].replace('`"', '"').replace('$devcmd', path)
                quoted = False
                needs_outer_quotes = False
                previous = ''
                for char in argument:
                    if char == '"' and previous != '\\':
                        quoted = not quoted
                    elif char.isspace() and not quoted:
                        needs_outer_quotes = True
                    previous = char
                self.assertTrue(needs_outer_quotes)
                raw_command = '"' + argument + '"'
                self.assertEqual(raw_command, '""' + path +
                                 '" -no_logo -arch=x64 -host_arch=x64 >nul && set"')
                self.assertEqual(raw_command[1:-1], '"' + path +
                                 '" -no_logo -arch=x64 -host_arch=x64 >nul && set')

    @unittest.skipUnless(sys.platform == 'win32', 'requires actual Windows cmd.exe and PowerShell 7')
    def test_real_cmd_batch_preserves_spaced_paths_arguments_environment_and_errors(self):
        pwsh = shutil.which('pwsh')
        self.assertIsNotNone(pwsh, 'PowerShell 7 is mandatory on the Windows runner')
        tail = self.sdk_tail()
        for directory in ('Program Files (x86)/Microsoft Visual Studio/2022/Enterprise',
                          'Program Files/Visual Studio 雪 & tools'):
            for status in (0, 37):
                with self.subTest(directory=directory, batch_exit=status), tempfile.TemporaryDirectory() as temp:
                    root = Path(temp)
                    installation = root / directory
                    batch = installation / 'Common7/Tools/VsDevCmd.bat'
                    batch.parent.mkdir(parents=True)
                    batch.write_text(
                        '@echo off\n'
                        'if not "%~1"=="-no_logo" exit /b 21\n'
                        'if not "%~2"=="-arch=x64" exit /b 22\n'
                        'if not "%~3"=="-host_arch=x64" exit /b 23\n'
                        'if not "%~4"=="" exit /b 24\n'
                        'if not "%OXID_SDK_TEST_PARENT%"=="inherited unchanged" exit /b 25\n'
                        'set "INCLUDE=fixture headers=a;b"\n'
                        'set "LIB=fixture libraries=c;d"\n'
                        'set "VSCMD_ARG_TGT_ARCH=x64"\n'
                        'set "VSCMD_ARG_HOST_ARCH=x64"\n'
                        'set "OXID_SDK_TEST_UNLISTED=must not be exported"\n'
                        f'exit /b {status}\n', encoding='ascii', newline='\r\n')
                    exported = root / 'github-env.txt'
                    script = root / 'initialize.ps1'
                    script.write_text("$ErrorActionPreference = 'Stop'\n"
                                      "$PSNativeCommandArgumentPassing = 'Standard'\n"
                                      '$installation = $env:OXID_SDK_TEST_INSTALLATION\n' + tail + '\n',
                                      encoding='utf-8')
                    environment = dict(os.environ, GITHUB_ENV=str(exported),
                                       OXID_SDK_TEST_INSTALLATION=str(installation),
                                       OXID_SDK_TEST_PARENT='inherited unchanged')
                    result = subprocess.run([pwsh, '-NoLogo', '-NoProfile', '-NonInteractive',
                                             '-File', str(script)], env=environment,
                                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                            timeout=30, check=False)
                    if status:
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn(b'Visual Studio environment initialization failed', result.stderr)
                        self.assertFalse(exported.exists(), 'failed SDK setup must not export environment')
                    else:
                        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
                        self.assertEqual(result.stdout, b'')
                        values = exported.read_text(encoding='utf-8-sig').splitlines()
                        for value in ('INCLUDE=fixture headers=a;b', 'LIB=fixture libraries=c;d',
                                      'VSCMD_ARG_TGT_ARCH=x64', 'VSCMD_ARG_HOST_ARCH=x64'):
                            self.assertIn(value, values)
                        self.assertFalse(any(line.startswith('OXID_SDK_TEST_') for line in values))


if __name__ == '__main__':
    unittest.main()
