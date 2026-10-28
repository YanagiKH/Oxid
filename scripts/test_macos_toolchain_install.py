"""Exercise the actual macOS installer shell against isolated local Git inputs."""
from pathlib import Path
import json
import os
import re
import shutil
import signal
import time
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
REVISION = '6a00b97b93bfc245d7083e4a90c7459ff5325222'
REMOTE = 'https://github.com/Homebrew/homebrew-core.git'


def install_script(workflow=None):
    if workflow is None:
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
    match = re.search(
        r'^      - name: Install LLVM 19 and the Mach-O linker\n(.*?)(?=^      - )',
        workflow, re.MULTILINE | re.DOTALL)
    if match is None:
        raise AssertionError('missing macOS LLVM install step')
    body = match[1].split('        run: |\n', 1)[1]
    lines = body.splitlines()
    if not all(not line or line.startswith('          ') for line in lines):
        raise AssertionError('unexpected installer block indentation')
    return '\n'.join(line[10:] if line else '' for line in lines) + '\n'


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], stderr=subprocess.PIPE)


class InstallerFixture:
    def __init__(self, root, llvm_version='19.1.7', install_status=0):
        self.root = root
        self.origin = root / 'origin'
        self.origin.mkdir()
        git(self.origin, 'init', '--quiet')
        formula = self.origin / 'Formula/l'
        formula.mkdir(parents=True)
        self.llvm = b'class LlvmAT19 < Formula\n  bottle do\n    sha256 cellar: :any, sonoma: "llvm-bottle"\n  end\nend\n'
        self.lld = (b'class LldAT19 < Formula\n  bottle do\n    sha256 cellar: :any, sonoma: "lld-bottle"\n  end\n'
                    + b'  depends_on "llvm@19"\nend\n')
        (formula / 'llvm@19.rb').write_bytes(self.llvm)
        (formula / 'lld@19.rb').write_bytes(self.lld)
        git(self.origin, 'add', '.')
        git(self.origin, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
            'commit', '--quiet', '-m', 'Pinned fixture')
        self.revision = git(self.origin, 'rev-parse', 'HEAD').decode().strip()
        self.existing_core = root / 'brew/Library/Taps/homebrew/homebrew-core'
        self.existing_core.parent.mkdir(parents=True)
        subprocess.run(['git', 'clone', '--quiet', str(self.origin), str(self.existing_core)],
                       check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        (self.existing_core / 'Formula/r').mkdir()
        tracked = self.existing_core / 'Formula/r/rustup.rb'
        tracked.write_text('tracked runner formula\n')
        git(self.existing_core, 'add', '.')
        git(self.existing_core, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
            'commit', '--quiet', '-m', 'Runner formula')
        tracked.write_text('runner local modification\n')
        (self.existing_core / 'Formula/o').mkdir()
        (self.existing_core / 'Formula/o/openssl@1.1.rb').write_text('runner untracked formula\n')
        self.before = self.core_snapshot()
        self.bin = root / 'bin'
        self.bin.mkdir()
        self.log = root / 'brew.jsonl'
        self.temp = root / 'runner-temp'
        self.temp.mkdir()
        self.environment = root / 'github-env'
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ['PATH'],
                        INSTALL_FIXTURE=str(root), LLVM_FIXTURE_VERSION=llvm_version, INSTALL_FIXTURE_STATUS=str(install_status),
                        RUNNER_TEMP=str(self.temp), GITHUB_ENV=str(self.environment))
        self.command('brew', '''#!/usr/bin/env python3
from pathlib import Path
import json, os, sys, time
root = Path(os.environ['INSTALL_FIXTURE'])
args = sys.argv[1:]
with (root / 'brew.jsonl').open('a') as log:
    log.write(json.dumps(args) + '\\n')
if args == ['--repository']:
    print(root / 'brew')
elif args == ['install', '--force-bottle', 'llvm@19', 'lld@19']:
    if os.environ.get('PAUSE_FIXTURE_INSTALL'):
        (root / 'install-active').touch()
        while True:
            time.sleep(0.01)
    if int(os.environ['INSTALL_FIXTURE_STATUS']):
        raise SystemExit(int(os.environ['INSTALL_FIXTURE_STATUS']))
    for name, programs in [('llvm@19', ['clang', 'opt', 'llvm-config']), ('lld@19', ['ld64.lld'])]:
        directory = root / 'prefix' / name / 'bin'
        directory.mkdir(parents=True)
        for program in programs:
            path = directory / program
            path.write_text('#!/bin/sh\\nprintf "%s\\\\n" "' + os.environ['LLVM_FIXTURE_VERSION'] + '"\\n')
            path.chmod(0o755)
elif args[:1] == ['--prefix'] and len(args) == 2 and args[1] in ['llvm@19', 'lld@19']:
    print(root / 'prefix' / args[1])
else:
    raise SystemExit('unexpected brew command: ' + repr(args))
''')
        self.command('xcrun', '#!/bin/sh\nprintf "%s\\n" "/fixture/macos-sdk"\n')

    def command(self, name, body):
        path = self.bin / name
        path.write_text(body)
        path.chmod(0o755)

    def core_snapshot(self):
        return {'head': git(self.existing_core, 'rev-parse', 'HEAD'),
                'status': git(self.existing_core, 'status', '--porcelain=v1', '--untracked-files=all'),
                'files': {str(path.relative_to(self.existing_core)): path.read_bytes()
                          for path in self.existing_core.rglob('*')
                          if path.is_file() and '.git' not in path.relative_to(self.existing_core).parts}}

    def run(self, script=None, revision=None):
        script = install_script() if script is None else script
        script = script.replace(REMOTE, self.origin.as_uri()).replace(REVISION, revision or self.revision)
        return subprocess.run(['bash', '--noprofile', '--norc', '-c', script], env=self.env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)

    def staged_core(self):
        workspaces = list(self.existing_core.parent.glob('.oxid-llvm19.*'))
        if len(workspaces) != 1:
            raise AssertionError('expected one isolated core workspace')
        return workspaces[0] / 'pinned'

    def calls(self):
        return [] if not self.log.exists() else [json.loads(row) for row in self.log.read_text().splitlines()]


class MacosToolchainInstallTests(unittest.TestCase):
    def fixture(self, **kwargs):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        return InstallerFixture(Path(temporary.name), **kwargs)

    def test_pinned_install_leaves_existing_dirty_core_unchanged(self):
        self.assertEqual(install_script().count(REVISION), 1)
        fixture = self.fixture()
        result = fixture.run()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(fixture.core_snapshot(), fixture.before)
        used = fixture.staged_core()
        self.assertEqual((used / 'Formula/l/llvm@19.rb').read_bytes(), fixture.llvm)
        self.assertEqual((used / 'Formula/l/lld@19.rb').read_bytes(), fixture.lld)
        self.assertEqual(git(used, 'rev-parse', 'HEAD').decode().strip(), fixture.revision)
        self.assertIn(['install', '--force-bottle', 'llvm@19', 'lld@19'], fixture.calls())
        self.assertEqual(fixture.environment.read_text().splitlines(),
                         ['OXID_LLVM_BIN=' + str(fixture.temp / 'oxid-llvm-19'),
                          'SDKROOT=/fixture/macos-sdk'])

    def test_install_failure_restores_the_original_tap_and_failure_status(self):
        fixture = self.fixture(install_status=37)
        result = fixture.run()
        self.assertEqual(result.returncode, 37, result.stderr.decode())
        self.assertFalse(fixture.environment.exists())
        self.assertEqual(fixture.core_snapshot(), fixture.before)
        self.assertEqual(git(fixture.staged_core(),
                             'rev-parse', 'HEAD').decode().strip(), fixture.revision)

    def test_absent_original_tap_is_not_left_installed(self):
        fixture = self.fixture()
        moved = fixture.root / 'untouched-original'
        fixture.existing_core.rename(moved)
        result = fixture.run()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertFalse(fixture.existing_core.exists())
        fixture.existing_core = moved
        self.assertEqual(fixture.core_snapshot(), fixture.before)

    def test_fetch_failure_stops_before_touching_the_existing_tap(self):
        fixture = self.fixture()
        result = fixture.run(revision='0' * 40)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(fixture.calls(), [['--repository']])
        self.assertFalse(fixture.environment.exists())
        self.assertEqual(fixture.core_snapshot(), fixture.before)

    def test_checkout_failure_leaves_the_existing_tap_unchanged(self):
        fixture = self.fixture()
        executable = shutil.which('git')
        fixture.command('git', '#!/usr/bin/env python3\nimport os, sys\n'
                        + 'if sys.argv[1:][-3:] == ["checkout", "--detach", "FETCH_HEAD"]: raise SystemExit(71)\n'
                        + 'os.execv(' + repr(executable) + ', [' + repr(executable) + ', *sys.argv[1:]])\n')
        result = fixture.run()
        self.assertEqual(result.returncode, 71, result.stderr.decode())
        self.assertEqual(fixture.calls(), [['--repository']])
        self.assertFalse(fixture.environment.exists())
        self.assertEqual(fixture.core_snapshot(), fixture.before)

    def test_interrupt_and_termination_restore_the_original_tap(self):
        for interruption, expected in [(signal.SIGINT, 130), (signal.SIGTERM, 143)]:
            with self.subTest(signal=interruption):
                fixture = self.fixture()
                script = install_script().replace(REMOTE, fixture.origin.as_uri()).replace(REVISION, fixture.revision)
                process = subprocess.Popen(['bash', '--noprofile', '--norc', '-c', script],
                                           env=dict(fixture.env, PAUSE_FIXTURE_INSTALL='1'),
                                           stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                           start_new_session=True)
                try:
                    deadline = time.monotonic() + 10
                    while not (fixture.root / 'install-active').exists():
                        if process.poll() is not None or time.monotonic() > deadline:
                            self.fail('installer did not reach the paused install')
                        time.sleep(0.01)
                    os.killpg(process.pid, interruption)
                    stdout, stderr = process.communicate(timeout=10)
                finally:
                    if process.poll() is None:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.communicate()
                self.assertEqual(process.returncode, expected, stderr.decode())
                self.assertFalse(fixture.environment.exists())
                self.assertEqual(fixture.core_snapshot(), fixture.before)
                self.assertEqual(git(fixture.staged_core(),
                                     'rev-parse', 'HEAD').decode().strip(), fixture.revision)

    def test_staging_failure_leaves_the_existing_tap_unchanged(self):
        fixture = self.fixture()
        fixture.command('mktemp', '#!/bin/sh\nexit 53\n')
        result = fixture.run()
        self.assertEqual(result.returncode, 53, result.stderr.decode())
        self.assertEqual(fixture.calls(), [['--repository']])
        self.assertFalse(fixture.environment.exists())
        self.assertEqual(fixture.core_snapshot(), fixture.before)

    def test_sdk_failure_restores_the_original_tap_without_publishing_environment(self):
        fixture = self.fixture()
        fixture.command('xcrun', '#!/bin/sh\nexit 29\n')
        result = fixture.run()
        self.assertEqual(result.returncode, 29, result.stderr.decode())
        self.assertFalse(fixture.environment.exists())
        self.assertEqual(fixture.core_snapshot(), fixture.before)

    def test_wrong_installed_version_does_not_publish_environment(self):
        fixture = self.fixture(llvm_version='20.0.0')
        result = fixture.run()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(fixture.environment.exists())
        self.assertEqual(fixture.core_snapshot(), fixture.before)


if __name__ == '__main__':
    unittest.main()
