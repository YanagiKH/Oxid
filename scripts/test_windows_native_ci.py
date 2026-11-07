"""Keep actual Windows native qualification mandatory without a YAML dependency.

These checks follow the workflow's block style, like test_ci_trigger_policy.py.
Source checks run on every host; Windows executes both temporary batch fixtures
and the installed Visual Studio SDK setup. These tests do not run a compiler.
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
$sdkStart = [System.Diagnostics.ProcessStartInfo]::new()
$sdkStart.FileName = $env:ComSpec
$sdkStart.UseShellExecute = $false
$sdkStart.CreateNoWindow = $true
$sdkStart.RedirectStandardInput = $true
$sdkStart.RedirectStandardOutput = $true
$sdkStart.RedirectStandardError = $true
$sdkStart.StandardOutputEncoding = [System.Text.Encoding]::Unicode
$sdkStart.StandardErrorEncoding = [System.Text.Encoding]::Unicode
$sdkStart.Arguments = '/d /u /s /c ""' + $devcmd + '" -no_logo -arch=x64 -host_arch=x64 >nul && set"'
$sdk = [System.Diagnostics.Process]::new()
$sdk.StartInfo = $sdkStart
$sdkStarted = $false
try {
  if (!$sdk.Start()) { throw 'Visual Studio environment initialization did not start' }
  $sdkStarted = $true
  $sdk.StandardInput.Close()
  $sdkOut = [System.Text.StringBuilder]::new()
  $sdkErr = [System.Text.StringBuilder]::new()
  $outBuffer = [char[]]::new(4096)
  $errBuffer = [char[]]::new(4096)
  $outRead = $sdk.StandardOutput.ReadAsync($outBuffer, 0, $outBuffer.Length)
  $errRead = $sdk.StandardError.ReadAsync($errBuffer, 0, $errBuffer.Length)
  $outDone = $false
  $errDone = $false
  $sdkTimer = [System.Diagnostics.Stopwatch]::StartNew()
  while (!$outDone -or !$errDone -or !$sdk.HasExited) {
    if ($sdkTimer.Elapsed.TotalSeconds -ge 120) { throw 'Visual Studio environment initialization timed out' }
    if (!$outDone -and $outRead.IsCompleted) {
      $count = $outRead.GetAwaiter().GetResult()
      if ($count -eq 0) { $outDone = $true } else {
        if ($sdkOut.Length + $count -gt 1048576) { throw 'Visual Studio environment output exceeded its limit' }
        [void]$sdkOut.Append($outBuffer, 0, $count)
        $outRead = $sdk.StandardOutput.ReadAsync($outBuffer, 0, $outBuffer.Length)
      }
    }
    if (!$errDone -and $errRead.IsCompleted) {
      $count = $errRead.GetAwaiter().GetResult()
      if ($count -eq 0) { $errDone = $true } else {
        if ($sdkErr.Length + $count -gt 65536) { throw 'Visual Studio environment error output exceeded its limit' }
        [void]$sdkErr.Append($errBuffer, 0, $count)
        $errRead = $sdk.StandardError.ReadAsync($errBuffer, 0, $errBuffer.Length)
      }
    }
    if (!$outDone -or !$errDone -or !$sdk.HasExited) { Start-Sleep -Milliseconds 10 }
  }
  $sdk.WaitForExit()
  if ($sdk.ExitCode -ne 0) {
    $sdkErrorText = $sdkErr.ToString().Replace("`r", " ").Replace("`n", " ")
    $sdkErrorText = $sdkErrorText.Substring(0, [Math]::Min(256, $sdkErrorText.Length))
    throw "Visual Studio environment initialization failed (cmd exit code $($sdk.ExitCode); captured stderr characters $($sdkErr.Length); first error: $sdkErrorText)"
  }
  $settings = $sdkOut.ToString() -split '\r?\n'
} finally {
  try {
    if ($sdkStarted -and !$sdk.HasExited) {
      try { $sdk.Kill($true) } catch [System.InvalidOperationException] {
        if (!$sdk.HasExited) { throw }
      }
      if (!$sdk.WaitForExit(5000)) { throw 'Visual Studio environment process did not terminate' }
    }
  } finally {
    $sdk.Dispose()
  }
}
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
Write-Host 'Downloading the pinned LLVM archive'
curl.exe --proto '=https' --tlsv1.2 --location --fail --silent --show-error --connect-timeout 30 --max-time 900 --retry 2 --retry-delay 5 --retry-max-time 900 --output $archive $url
if ($LASTEXITCODE -ne 0) { throw "LLVM download failed (curl exit code $LASTEXITCODE)" }
Write-Host 'Download complete; verifying archive length and SHA256'
if ((Get-Item -LiteralPath $archive).Length -ne 845236708) { throw 'LLVM archive length differs' }
$digest = 'b4557b4f012161f56a2f5d9e877ab9635cafd7a08f7affe14829bd60c9d357f0'
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $digest) {
  throw 'LLVM archive SHA256 differs'
}
$sevenZip = Join-Path $env:ProgramFiles '7-Zip/7z.exe'
if (!(Test-Path -LiteralPath $sevenZip -PathType Leaf)) { throw 'The preinstalled Windows 7-Zip is required' }
function Invoke-LlvmSevenZip {
  param(
    [string]$Stage,
    [ValidateSet('xz', 'tar')][string]$Format,
    [string]$InputArchive,
    [string]$Destination,
    [ValidateRange(1, 900)][int]$TimeoutSeconds
  )
  $start = [System.Diagnostics.ProcessStartInfo]::new()
  $start.FileName = $sevenZip
  $start.UseShellExecute = $false
  $start.CreateNoWindow = $true
  $start.RedirectStandardInput = $true
  foreach ($argument in @('x', "-t$Format", '-y', '-bsp0', '-bb0', "-o$Destination", $InputArchive)) {
    $start.ArgumentList.Add($argument)
  }
  $extraction = [System.Diagnostics.Process]::new()
  $extraction.StartInfo = $start
  $started = $false
  try {
    Write-Host "$Stage started (deadline $TimeoutSeconds seconds)"
    if (!$extraction.Start()) { throw "$Stage did not start" }
    $started = $true
    $extraction.StandardInput.Close()
    if (!$extraction.WaitForExit($TimeoutSeconds * 1000)) { throw "$Stage timed out" }
    if ($extraction.ExitCode -ne 0) { throw "$Stage failed (7-Zip exit code $($extraction.ExitCode))" }
    Write-Host "$Stage complete"
  } finally {
    try {
      if ($started -and !$extraction.HasExited) {
        try { $extraction.Kill($true) } catch [System.InvalidOperationException] {
          if (!$extraction.HasExited) { throw }
        }
        if (!$extraction.WaitForExit(5000)) { throw "$Stage process did not terminate" }
      }
    } finally {
      $extraction.Dispose()
    }
  }
}
$staging = Join-Path $env:RUNNER_TEMP ('oxid-llvm-msvc-' + [Guid]::NewGuid().ToString('N'))
$tarDirectory = Join-Path $staging 'tar'
$extract = Join-Path $staging 'toolchain'
New-Item -ItemType Directory -Path $staging | Out-Null
New-Item -ItemType Directory -Path $tarDirectory | Out-Null
New-Item -ItemType Directory -Path $extract | Out-Null
Write-Host 'Archive verified; decompressing the complete LLVM XZ stream'
Invoke-LlvmSevenZip -Stage 'LLVM XZ decompression' -Format xz -InputArchive $archive -Destination $tarDirectory -TimeoutSeconds 600
$tarArchive = Join-Path $tarDirectory 'clang+llvm-19.1.7-x86_64-pc-windows-msvc.tar'
if (!(Test-Path -LiteralPath $tarArchive -PathType Leaf) -or (Get-Item -LiteralPath $tarArchive).Length -le 0) { throw 'LLVM XZ decompression produced no TAR archive' }
Write-Host 'XZ decompression complete; extracting every LLVM TAR member'
Invoke-LlvmSevenZip -Stage 'LLVM TAR extraction' -Format tar -InputArchive $tarArchive -Destination $extract -TimeoutSeconds 900
Remove-Item -LiteralPath $tarArchive
Write-Host 'Extraction complete; verifying LLVM tools'
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
    scalar(named(LLVM_STEP), 'timeout-minutes', '45')
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
            ('quoted-sdk-switches', '-no_logo -arch=x64 -host_arch=x64',
             '"-no_logo" "-arch=x64" "-host_arch=x64"'),
            ('missing-sdk-invocation-regression', '          python -B scripts/test_windows_native_ci.py\n', ''),
            ('shell-execute-bypass', '$sdkStart.UseShellExecute = $false', '$sdkStart.UseShellExecute = $true'),
            ('double-wrapped-batch', "'/d /u /s /c \"\"'", "'/d /u /s /c \"\"\"'"),
            ('missing-outer-pair', "'/d /u /s /c \"\"'", "'/d /u /s /c \"'"),
            ('unicode-output-disabled', '/d /u /s /c ', '/d /s /c '),
            ('wrong-output-decoder', '$sdkStart.StandardOutputEncoding = [System.Text.Encoding]::Unicode',
             '$sdkStart.StandardOutputEncoding = [System.Text.Encoding]::UTF8'),
            ('sdk-exit-ignored', '$sdk.ExitCode -ne 0', '$sdk.ExitCode -ne 37'),
            ('sdk-deadline-removed', "    if ($sdkTimer.Elapsed.TotalSeconds -ge 120) { throw 'Visual Studio environment initialization timed out' }\n", ''),
            ('sdk-output-unbounded', '$sdkOut.Length + $count -gt 1048576', '$false'),
            ('sdk-errors-unbounded', '$sdkErr.Length + $count -gt 65536', '$false'),
            ('sdk-child-tree-not-killed', '$sdk.Kill($true)', '$sdk.Kill($false)'),
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
            ('download-replaced', 'curl.exe --proto', 'Write-Host --proto'),
            ('unbounded-connect', '--connect-timeout 30', '--connect-timeout 0'),
            ('unbounded-download', '--max-time 900', '--max-time 0'),
            ('unbounded-retries', '--retry-max-time 900', '--retry-max-time 0'),
            ('unbounded-toolchain-step', '        timeout-minutes: 45\n', ''),
            ('download-failure-ignored', 'if ($LASTEXITCODE -ne 0) { throw "LLVM download failed (curl exit code $LASTEXITCODE)" }', 'Write-Host "download accepted"'),
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

    def test_complete_llvm_extraction_requires_both_bounded_checked_stages(self):
        self.reject_mutations([
            ('wrong-preinstalled-extractor', "'7-Zip/7z.exe'", "'Other/7z.exe'"),
            ('missing-preinstalled-extractor-check',
             "if (!(Test-Path -LiteralPath $sevenZip -PathType Leaf)) { throw 'The preinstalled Windows 7-Zip is required' }", ''),
            ('not-extracting-archive', "@('x', \"-t$Format\"", "@('t', \"-t$Format\""),
            ('partial-extraction-filter', "@('x', \"-t$Format\"", "@('x', 'bin/*', \"-t$Format\""),
            ('unsafe-argument-joining', '$start.ArgumentList.Add($argument)', '$start.Arguments += $argument'),
            ('missing-xz-stage', "Invoke-LlvmSevenZip -Stage 'LLVM XZ decompression' -Format xz -InputArchive $archive -Destination $tarDirectory -TimeoutSeconds 600", ''),
            ('missing-tar-stage', "Invoke-LlvmSevenZip -Stage 'LLVM TAR extraction' -Format tar -InputArchive $tarArchive -Destination $extract -TimeoutSeconds 900", ''),
            ('unbounded-xz-stage', '-Destination $tarDirectory -TimeoutSeconds 600', '-Destination $tarDirectory -TimeoutSeconds 0'),
            ('unbounded-tar-stage', '-Destination $extract -TimeoutSeconds 900', '-Destination $extract -TimeoutSeconds 0'),
            ('stage-deadline-ignored', '$extraction.WaitForExit($TimeoutSeconds * 1000)', '$extraction.WaitForExit()'),
            ('stage-failure-accepted', '$extraction.ExitCode -ne 0', '$extraction.ExitCode -gt 1'),
            ('stage-input-not-closed', '$extraction.StandardInput.Close()', ''),
            ('stage-child-tree-not-killed', '$extraction.Kill($true)', '$extraction.Kill($false)'),
            ('stage-cleanup-wait-omitted',
             'if (!$extraction.WaitForExit(5000)) { throw "$Stage process did not terminate" }', ''),
            ('stage-owner-not-disposed', '$extraction.Dispose()', ''),
            ('reused-output-path', "[Guid]::NewGuid().ToString('N')", "'reused-output'"),
            ('tar-output-unchecked',
             "if (!(Test-Path -LiteralPath $tarArchive -PathType Leaf) -or (Get-Item -LiteralPath $tarArchive).Length -le 0) { throw 'LLVM XZ decompression produced no TAR archive' }", ''),
            ('missing-stage-start-message', 'Write-Host "$Stage started (deadline $TimeoutSeconds seconds)"', ''),
            ('missing-stage-complete-message', 'Write-Host "$Stage complete"', ''),
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
    def sdk_commands(self):
        workflow = (REPO / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        job = '    ' + block(workflow, JOB, 2) + '\n'
        match = re.search(r'^      - name: ' + re.escape(SDK_STEP) +
                          r'\n(.*?)(?=^      - |\Z)', job, re.MULTILINE | re.DOTALL)
        require(match is not None, 'SDK initialization step is required')
        return step_commands(textwrap.dedent(match[1]))

    def sdk_tail(self):
        commands = self.sdk_commands()
        start = commands.index("$devcmd = Join-Path $installation 'Common7/Tools/VsDevCmd.bat'")
        return '\n'.join(commands[start:])

    def test_cmd_s_removes_exactly_the_explicit_outer_quote_pair(self):
        # ProcessStartInfo.Arguments is the raw CreateProcess argument string,
        # with no PowerShell native binder or ArgumentList/CRT re-escaping.
        # cmd /s removes the explicit first and last quotes after /c.
        invocation = next(line for line in self.sdk_tail().splitlines()
                          if line.startswith('$sdkStart.Arguments = '))
        self.assertEqual(invocation, "$sdkStart.Arguments = '/d /u /s /c \"\"' + $devcmd + "
                         "'\" -no_logo -arch=x64 -host_arch=x64 >nul && set\"'")
        self.assertNotIn('$PSNativeCommandArgumentPassing', self.sdk_tail())
        for path in (r'C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat',
                     r'C:\Program Files (x86)\Visual Studio 雪 & tools\Common7\Tools\VsDevCmd.bat'):
            with self.subTest(path=path):
                raw_arguments = '/d /u /s /c ""' + path + '" -no_logo -arch=x64 -host_arch=x64 >nul && set"'
                raw_command = raw_arguments.split('/c ', 1)[1]
                self.assertEqual(raw_command, '""' + path +
                                 '" -no_logo -arch=x64 -host_arch=x64 >nul && set"')
                self.assertEqual(raw_command[1:-1], '"' + path +
                                 '" -no_logo -arch=x64 -host_arch=x64 >nul && set')

    @unittest.skipUnless(sys.platform == 'win32', 'requires actual Windows cmd.exe and PowerShell 7')
    def test_real_cmd_batch_preserves_spaced_paths_arguments_environment_and_errors(self):
        pwsh = shutil.which('pwsh')
        self.assertIsNotNone(pwsh, 'PowerShell 7 is mandatory on the Windows runner')
        tail = self.sdk_tail()
        for directory in ('VisualStudio',
                          'Program Files (x86)/Microsoft Visual Studio/2022/Enterprise',
                          'Program Files/Visual Studio 雪 & tools'):
            for status in (0, 37):
                with self.subTest(directory=directory, batch_exit=status), tempfile.TemporaryDirectory() as temp:
                    root = Path(temp)
                    installation = root / directory
                    batch = installation / 'Common7/Tools/VsDevCmd.bat'
                    batch.parent.mkdir(parents=True)
                    batch.write_text(
                        '@echo off\n'
                        '> "%OXID_SDK_TEST_TRACE%" echo entered\n'
                        '>> "%OXID_SDK_TEST_TRACE%" echo args:[%~1][%~2][%~3][%~4][%~5][%~6]\n'
                        '>> "%OXID_SDK_TEST_TRACE%" echo raw:%*\n'
                        # cmd separates '=' in batch argument slots while %*
                        # retains the original unquoted SDK command line.
                        'if not "%1"=="-no_logo" exit /b 21\n'
                        'if not "%~2"=="-arch" exit /b 22\n'
                        'if not "%~3"=="x64" exit /b 23\n'
                        'if not "%~4"=="-host_arch" exit /b 24\n'
                        'if not "%~5"=="x64" exit /b 26\n'
                        'if not "%~6"=="" exit /b 27\n'
                        'if not "%OXID_SDK_TEST_PARENT%"=="inherited unchanged" exit /b 25\n'
                        'set "INCLUDE=fixture headers=a;b"\n'
                        'set "LIB=fixture libraries=c;d"\n'
                        'set "LIBPATH=%OXID_SDK_TEST_UNICODE%"\n'
                        'set "VSCMD_ARG_TGT_ARCH=x64"\n'
                        'set "VSCMD_ARG_HOST_ARCH=x64"\n'
                        'set "OXID_SDK_TEST_UNLISTED=must not be exported"\n'
                        f'>> "%OXID_SDK_TEST_TRACE%" echo validated-exit-{status}\n'
                        f'exit /b {status}\n', encoding='ascii', newline='\r\n')
                    exported = root / 'github-env.txt'
                    trace = root / 'fixture-trace.txt'
                    script = root / 'initialize.ps1'
                    script.write_text("$ErrorActionPreference = 'Stop'\n"
                                      "$PSNativeCommandArgumentPassing = 'Standard'\n"
                                      '$installation = $env:OXID_SDK_TEST_INSTALLATION\n' + tail + '\n',
                                      encoding='utf-8')
                    environment = dict(os.environ, GITHUB_ENV=str(exported),
                                       OXID_SDK_TEST_INSTALLATION=str(installation),
                                       OXID_SDK_TEST_PARENT='inherited unchanged',
                                       OXID_SDK_TEST_UNICODE='fixture Unicode 雪 library path',
                                       OXID_SDK_TEST_TRACE=str(trace))
                    result = subprocess.run([pwsh, '-NoLogo', '-NoProfile', '-NonInteractive',
                                             '-File', str(script)], env=environment,
                                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                            timeout=30, check=False)
                    diagnostics = result.stderr.decode(errors='replace')
                    self.assertTrue(trace.is_file(), 'the actual batch fixture was not entered: ' + diagnostics)
                    self.assertLessEqual(trace.stat().st_size, 16384)
                    marks = trace.read_bytes().decode('utf-16-le').removeprefix('\ufeff').splitlines()
                    self.assertEqual(marks, [
                        'entered', 'args:[-no_logo][-arch][x64][-host_arch][x64][]',
                        'raw:-no_logo -arch=x64 -host_arch=x64',
                        f'validated-exit-{status}'], diagnostics)
                    if status:
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn(b'Visual Studio environment initialization failed', result.stderr)
                        self.assertIn(b'cmd exit code 37;', result.stderr)
                        self.assertFalse(exported.exists(), 'failed SDK setup must not export environment')
                    else:
                        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
                        self.assertEqual(result.stdout, b'')
                        values = exported.read_text(encoding='utf-8-sig').splitlines()
                        for value in ('INCLUDE=fixture headers=a;b', 'LIB=fixture libraries=c;d',
                                      'LIBPATH=fixture Unicode 雪 library path',
                                      'VSCMD_ARG_TGT_ARCH=x64', 'VSCMD_ARG_HOST_ARCH=x64'):
                            self.assertIn(value, values)
                        self.assertFalse(any(line.startswith('OXID_SDK_TEST_') for line in values))

    @unittest.skipUnless(sys.platform == 'win32', 'requires the installed Windows MSVC and SDK')
    def test_real_installed_visual_studio_exports_x64_tools_and_sdk(self):
        pwsh = shutil.which('pwsh')
        self.assertIsNotNone(pwsh, 'PowerShell 7 is mandatory on the Windows runner')
        commands = self.sdk_commands()
        first = next(index for index, command in enumerate(commands)
                     if command.startswith('$vswhere = '))
        # Use the current workflow body after its self-test invocation, so this
        # checks the installed SDK without recursively invoking this test file.
        with tempfile.TemporaryDirectory(prefix='oxid-installed-sdk-') as temporary:
            root = Path(temporary)
            exported = root / 'github-env.txt'
            script = root / 'initialize.ps1'
            script.write_text("$ErrorActionPreference = 'Stop'\n" +
                              '\n'.join(commands[first:]) + '\n', encoding='utf-8')
            result = subprocess.run([pwsh, '-NoLogo', '-NoProfile', '-NonInteractive',
                                     '-File', str(script)],
                                    env=dict(os.environ, GITHUB_ENV=str(exported)),
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    timeout=150, check=False)
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
            self.assertEqual(result.stdout, b'')
            self.assertTrue(exported.is_file(), 'installed SDK exported no environment')
            self.assertLessEqual(exported.stat().st_size, 1048576)
            values = dict(line.split('=', 1) for line in
                          exported.read_text(encoding='utf-8-sig').splitlines())
            values = {name.upper(): value for name, value in values.items()}
            self.assertLessEqual(set(values), {
                'PATH', 'INCLUDE', 'LIB', 'LIBPATH', 'VCINSTALLDIR', 'VCTOOLSINSTALLDIR',
                'VCTOOLSVERSION', 'WINDOWSSDKDIR', 'WINDOWSSDKVERSION', 'WINDOWSSDKLIBVERSION',
                'UNIVERSALCRTSDKDIR', 'UCRTVERSION', 'VSCMD_ARG_TGT_ARCH', 'VSCMD_ARG_HOST_ARCH',
            })
            for name in ('PATH', 'INCLUDE', 'LIB', 'LIBPATH', 'VCINSTALLDIR',
                         'VCTOOLSINSTALLDIR', 'WINDOWSSDKDIR', 'WINDOWSSDKVERSION'):
                self.assertTrue(values.get(name), name + ' is missing from installed SDK')
            self.assertEqual(values.get('VSCMD_ARG_TGT_ARCH'), 'x64')
            self.assertEqual(values.get('VSCMD_ARG_HOST_ARCH'), 'x64')
            self.assertTrue(Path(values['VCTOOLSINSTALLDIR']).is_dir())
            self.assertTrue(Path(values['WINDOWSSDKDIR']).is_dir())


if __name__ == '__main__':
    unittest.main()
