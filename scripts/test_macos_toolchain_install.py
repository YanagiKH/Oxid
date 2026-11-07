"""Exercise the actual macOS installer shell against isolated local Git inputs."""
from pathlib import Path
import ast
import hashlib
import json
import os
import re
import shutil
import signal
import time
import subprocess
import tempfile
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
REVISION = '6a00b97b93bfc245d7083e4a90c7459ff5325222'
REMOTE = 'https://github.com/Homebrew/homebrew-core.git'
OBSOLETE = b'  no_autobump! because: :requires_manual_review\n'
FORMULA_PINS = {
    'llvm@19.rb': ('14c7ae5879dff4c4d3fab077671d924a56605c24e2fef252ca343d49029efa59',
                   '9672f3b5b565b7b5e310bd0aa4698540712ff5b74955d1716b8127825517eb95'),
    'lld@19.rb': ('bda99a28eec9b9e4986220751dd0c622566619d5071b9df9dcb3f315a92358a1',
                  '5fa2731e9bf2027ecb569817b3934cfb07e2684f7d41bd0e3310eda0de3d84fe'),
}

FORMULA_TREE = '072eb5593e65cd9374940a2a183e18b656f57543'
FORMULA_FILE_COUNT = 7674
FORMULA_INVENTORY_SHA = 'd54485054853a163d3b5835b3403131eece818c7509fae4a570a7a35861e35b3'
ADAPTATION_FILE_COUNT = 1156
ADAPTATION_INVENTORY_SHA = 'df40f1369bb73ae9e9f489877dbd9137558d12f3048af29945cd91bdb304252a'
METADATA_STANZAS = {
    OBSOLETE: 1125,
    b'  no_autobump! because: :incompatible_version_format\n': 17,
    b'  no_autobump! because: :bumped_by_upstream\n': 10,
    b'  no_autobump! because: "some resources have to be updated manually"\n': 1,
    b'  no_autobump! because: "unable to get versions"\n': 1,
    b'  no_autobump! because: "resources cannot be updated automatically"\n': 1,
    b'  no_autobump! because: "\'playwright\' resource lacks PyPI sdist"\n': 1,
}
NESTED_LIVECHECKS = ('Formula/b/bash.rb', 'Formula/b/blastem.rb', 'Formula/c/calabash.rb',
                    'Formula/f/frege.rb', 'Formula/h/http_load.rb')
NESTED_STANZA = b'      no_autobump! because: :requires_manual_review\n'


def encoded(value):
    return json.dumps(value, ensure_ascii=True, separators=(',', ':')).encode() + b'\n'


def adapted_formula(body):
    return b''.join(line for line in body.splitlines(keepends=True) if line not in METADATA_STANZAS)


def fixture_inventory(root):
    rows, changes = [], []
    counts = dict.fromkeys(METADATA_STANZAS, 0)
    raw = git(root, 'ls-tree', '-rlz', 'HEAD', '--', 'Formula')
    for entry in raw.split(b'\0'):
        if not entry:
            continue
        details, name = entry.split(b'\t', 1)
        mode, kind, object_id, length = details.split()
        name = name.decode()
        body = (root / name).read_bytes()
        rows.append([name, mode.decode(), object_id.decode(), len(body)])
        after = adapted_formula(body)
        found = [line for line in body.splitlines(keepends=True) if line in METADATA_STANZAS]
        if len(found) == 1:
            counts[found[0]] += 1
            changes.append([name, mode.decode(), object_id.decode(), len(body), digest(body),
                            len(after), digest(after), digest(found[0])])
    return rows, changes, counts


def adapter_assignments(code):
    wanted = {'formula_tree', 'formula_file_count', 'formula_inventory_sha', 'adaptation_file_count',
              'adaptation_inventory_sha', 'formulas', 'metadata_stanzas', 'nested_livechecks', 'nested_stanza'}
    return {node.targets[0].id: ast.literal_eval(node.value)
            for node in ast.parse(code).body if isinstance(node, ast.Assign)
            and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
            and node.targets[0].id in wanted}


def replace_adapter_assignments(script, values):
    code = formula_adapter(script)
    lines = code.splitlines(keepends=True)
    nodes = {node.targets[0].id: node for node in ast.parse(code).body
             if isinstance(node, ast.Assign) and len(node.targets) == 1
             and isinstance(node.targets[0], ast.Name) and node.targets[0].id in values}
    if set(nodes) != set(values):
        raise AssertionError('closed adapter assignment inventory changed')
    for name, node in sorted(nodes.items(), key=lambda item: item[1].lineno, reverse=True):
        lines[node.lineno - 1:node.end_lineno] = [name + ' = ' + repr(values[name]) + '\n']
    return script.replace(code, ''.join(lines), 1)


def digest(body):
    return hashlib.sha256(body).hexdigest()


def formula_adapter(script):
    match = re.search(r'^python3 - "\$pinned_core" <<\'PY\'\n(.*?)^PY$',
                      script, re.MULTILINE | re.DOTALL)
    if match is None:
        raise AssertionError('missing closed formula metadata adaptation')
    return match[1]


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
    def __init__(self, root, llvm_version='19.1.7', install_status=0, extra_formulas=None):
        self.root = root
        self.origin = root / 'origin'
        self.origin.mkdir()
        git(self.origin, 'init', '--quiet')
        formula = self.origin / 'Formula/l'
        formula.mkdir(parents=True)
        self.llvm = (b'class LlvmAT19 < Formula\n'
                     + b'  url "https://example.invalid/llvmorg-19.1.7.tar.xz"\n'
                     + OBSOLETE + b'  bottle do\n    sha256 cellar: :any, sonoma: "llvm-bottle"\n  end\n'
                     + b'  depends_on "zstd"\n  def install\n    system "cmake", "-DLLVM_ENABLE_LTO=ON"\n  end\nend\n')
        self.lld = (b'class LldAT19 < Formula\n' + OBSOLETE
                    + b'  bottle do\n    sha256 cellar: :any, sonoma: "lld-bottle"\n  end\n'
                    + b'  depends_on "llvm@19"\n  def install\n    system "cmake", "-DLLD_BUILT_STANDALONE=ON"\n  end\nend\n')
        self.formulas = {'Formula/l/llvm@19.rb': self.llvm, 'Formula/l/lld@19.rb': self.lld}
        for name in ['llvm@14', 'llvm@15', 'llvm@17', 'llvm@18']:
            self.formulas['Formula/l/' + name + '.rb'] = b'class Conflict < Formula\n' + OBSOLETE + b'end\n'
        self.formulas['Formula/c/cmake.rb'] = b'class Cmake < Formula\n' + OBSOLETE + b'end\n'
        for index, stanza in enumerate(METADATA_STANZAS):
            self.formulas['Formula/m/metadata-' + str(index) + '.rb'] = b'class Metadata < Formula\n' + stanza + b'end\n'
        for name in NESTED_LIVECHECKS:
            self.formulas[name] = (b'class Deferred < Formula\n  livecheck do\n    strategy :json do |value|\n'
                                   + NESTED_STANZA + b'      value\n    end\n  end\nend\n')
        self.formulas['Formula/z/zero-metadata.rb'] = b'class Plain < Formula\n  depends_on "zstd"\nend\n'
        self.formulas.update(extra_formulas or {})
        for name, body in self.formulas.items():
            path = self.origin / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(body)
        (self.origin / 'Formula/z/zero-metadata.rb').chmod(0o755)
        git(self.origin, 'add', '.')
        git(self.origin, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
            'commit', '--quiet', '-m', 'Pinned fixture')
        self.revision = git(self.origin, 'rev-parse', 'HEAD').decode().strip()
        self.formula_tree = git(self.origin, 'rev-parse', 'HEAD:Formula').decode().strip()
        self.rows, self.changes, self.counts = fixture_inventory(self.origin)
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
import json, os, signal, sys, time
root = Path(os.environ['INSTALL_FIXTURE'])
args = sys.argv[1:]
with (root / 'brew.jsonl').open('a') as log:
    log.write(json.dumps(args) + '\\n')
if args == ['--repository']:
    print(root / 'brew')
elif args == ['install', '--force-bottle', 'llvm@19', 'lld@19']:
    for path in (root / 'brew/Library/Taps/homebrew/homebrew-core/Formula').rglob('*.rb'):
        if any(line.startswith(b'  no_autobump!') for line in path.read_bytes().splitlines()):
            raise SystemExit('unsupported update-bot metadata reached Homebrew')
    if os.environ.get('PAUSE_FIXTURE_INSTALL'):
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        def stop_install(signum, frame):
            original = root / 'brew/Library/Taps/homebrew/homebrew-core/Formula/r/rustup.rb'
            (root / 'install-stopped').write_text(json.dumps({
                'signal': signum, 'private_tap_active': not original.exists()}))
            raise SystemExit(0)
        signal.signal(signal.SIGTERM, signal.SIG_IGN if os.environ.get('IGNORE_FIXTURE_TERM') else stop_install)
        (root / 'install-starting').write_text(json.dumps({'pid': os.getpid(), 'group': os.getpgrp()}))
        (root / 'install-starting').rename(root / 'install-active')
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

    def script(self, script=None, revision=None):
        script = install_script() if script is None else script
        script = script.replace(REMOTE, self.origin.as_uri()).replace(REVISION, revision or self.revision)
        # Replace only the closed audit descriptors for this synthetic Git tree.
        # The transformation and every authentication predicate remain actual CI code.
        values = {'formula_tree': self.formula_tree, 'formula_file_count': len(self.rows),
                  'formula_inventory_sha': digest(encoded(self.rows)),
                  'adaptation_file_count': len(self.changes),
                  'adaptation_inventory_sha': digest(encoded(self.changes)),
                  'metadata_stanzas': self.counts}
        script = replace_adapter_assignments(script, values)
        for name, body in [('llvm@19.rb', self.llvm), ('lld@19.rb', self.lld)]:
            before, after = FORMULA_PINS[name]
            script = script.replace(before, digest(body)).replace(after, digest(adapted_formula(body)))
        return script

    def after_checkout(self, code):
        executable = shutil.which('git')
        self.command('git', '#!/usr/bin/env python3\nimport subprocess, sys\nfrom pathlib import Path\n'
                     + 'result = subprocess.run([' + repr(executable) + ', *sys.argv[1:]])\n'
                     + 'if result.returncode == 0 and sys.argv[1:][-3:] == ["checkout", "--detach", "FETCH_HEAD"]:\n'
                     + '    root = Path(sys.argv[2])\n'
                     + ''.join('    ' + line + '\n' for line in code.splitlines())
                     + 'raise SystemExit(result.returncode)\n')

    def run(self, script=None, revision=None):
        script = self.script(script, revision)
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
        for name, original in [('llvm@19.rb', fixture.llvm), ('lld@19.rb', fixture.lld)]:
            adapted = (used / 'Formula/l' / name).read_bytes()
            self.assertEqual(adapted, original.replace(OBSOLETE, b'', 1))
            offset = original.index(OBSOLETE)
            self.assertEqual(adapted[:offset] + OBSOLETE + adapted[offset:], original)
        self.assertEqual(git(used, 'rev-parse', 'HEAD').decode().strip(), fixture.revision)
        self.assertIn(['install', '--force-bottle', 'llvm@19', 'lld@19'], fixture.calls())
        self.assertEqual(fixture.environment.read_text().splitlines(),
                         ['OXID_LLVM_BIN=' + str(fixture.temp / 'oxid-llvm-19'),
                          'SDKROOT=/fixture/macos-sdk'])

    def test_formula_adaptation_is_limited_to_the_exact_pinned_tree(self):
        self.assertEqual(adapter_assignments(formula_adapter(install_script())), {
            'formula_tree': FORMULA_TREE, 'formula_file_count': FORMULA_FILE_COUNT,
            'formula_inventory_sha': FORMULA_INVENTORY_SHA,
            'adaptation_file_count': ADAPTATION_FILE_COUNT,
            'adaptation_inventory_sha': ADAPTATION_INVENTORY_SHA,
            'formulas': FORMULA_PINS, 'metadata_stanzas': METADATA_STANZAS,
            'nested_livechecks': NESTED_LIVECHECKS, 'nested_stanza': NESTED_STANZA})
        self.assertEqual(sum(METADATA_STANZAS.values()), 1156)
        self.assertEqual(len(OBSOLETE), 48)

    def test_changed_formula_fails_before_swapping_or_installing(self):
        for name in ['Formula/l/llvm@19.rb', 'Formula/l/llvm@14.rb', 'Formula/c/cmake.rb',
                     'Formula/z/zero-metadata.rb']:
            with self.subTest(formula=name):
                fixture = self.fixture()
                fixture.after_checkout('p = root / ' + repr(name) + '\np.write_bytes(p.read_bytes() + b"# changed\\n")')
                result = fixture.run()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(('changed pinned formula: ' + name).encode(), result.stderr)
                self.assertEqual(fixture.calls(), [['--repository']])
                self.assertFalse(fixture.environment.exists())
                self.assertEqual(fixture.core_snapshot(), fixture.before)

    def test_wrong_adapted_digest_leaves_all_formulas_unmodified(self):
        fixture = self.fixture()
        script = install_script().replace(FORMULA_PINS['lld@19.rb'][1], '0' * 64)
        result = fixture.run(script)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'invalid target formula adaptation: Formula/l/lld@19.rb', result.stderr)
        self.assertEqual(fixture.calls(), [['--repository']])
        self.assertEqual(fixture.core_snapshot(), fixture.before)
        for name, original in fixture.formulas.items():
            self.assertEqual((fixture.staged_core() / name).read_bytes(), original)

    def test_missing_formula_is_not_installed_and_original_tap_is_unchanged(self):
        fixture = self.fixture()
        fixture.after_checkout('(root / "Formula/l/lld@19.rb").unlink()')
        result = fixture.run()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(fixture.calls(), [['--repository']])
        self.assertEqual(fixture.core_snapshot(), fixture.before)
        self.assertEqual((fixture.staged_core() / 'Formula/l/llvm@19.rb').read_bytes(), fixture.llvm)

    def test_conflicts_dependencies_and_other_known_reasons_are_adapted(self):
        fixture = self.fixture()
        result = fixture.run()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        for name, before in fixture.formulas.items():
            after = (fixture.staged_core() / name).read_bytes()
            self.assertEqual(after, adapted_formula(before), name)
        report = json.loads((fixture.staged_core().parent / 'formula-adaptation.json').read_text())
        self.assertEqual(report['adapted_formulas'], len(fixture.changes))
        self.assertEqual(report['changes'], fixture.changes)
        self.assertEqual(report['unchanged_nested_livechecks'], list(NESTED_LIVECHECKS))
        self.assertTrue((fixture.staged_core() / 'Formula/z/zero-metadata.rb').stat().st_mode & 0o111)

    def test_nested_livecheck_bodies_keep_all_bytes_and_return_expressions(self):
        fixture = self.fixture()
        result = fixture.run()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        for name in NESTED_LIVECHECKS:
            body = (fixture.staged_core() / name).read_bytes()
            self.assertEqual(body, fixture.formulas[name])
            self.assertIn(NESTED_STANZA, body)
            self.assertIn(OBSOLETE, body)  # Substring replacement would corrupt this body.
            self.assertIn(b'      value\n', body)

    def test_unknown_reason_shape_and_repeated_stanzas_fail_closed(self):
        for body in [b'class Invalid < Formula\n  no_autobump! because: :unknown\nend\n',
                     b'class Invalid < Formula\n   no_autobump! because: :requires_manual_review\nend\n',
                     b'class Invalid < Formula\n' + OBSOLETE + OBSOLETE + b'end\n']:
            with self.subTest(body=body):
                fixture = self.fixture(extra_formulas={'Formula/u/unknown.rb': body})
                result = fixture.run()
                self.assertNotEqual(result.returncode, 0)
                self.assertRegex(result.stderr.decode(), 'unknown formula update metadata|repeated formula update metadata')
                self.assertEqual(fixture.calls(), [['--repository']])
                self.assertEqual(fixture.core_snapshot(), fixture.before)
                for name, before in fixture.formulas.items():
                    self.assertEqual((fixture.staged_core() / name).read_bytes(), before)

    def test_tree_inventory_and_change_count_drift_fail_before_writes(self):
        for name, replacement, message in [
            ('formula_tree', '0' * 40, 'changed pinned Formula tree'),
            ('formula_file_count', 0, 'changed pinned formula inventory'),
            ('formula_inventory_sha', '0' * 64, 'changed pinned formula inventory'),
            ('adaptation_file_count', 0, 'changed formula adaptation inventory'),
            ('adaptation_inventory_sha', '0' * 64, 'changed formula adaptation inventory'),
        ]:
            with self.subTest(field=name):
                fixture = self.fixture()
                script = fixture.script()
                script = replace_adapter_assignments(script, {name: replacement})
                # run() normally installs this fixture's descriptors, so invoke
                # the already translated shell directly for deliberate drift.
                result = subprocess.run(['bash', '--noprofile', '--norc', '-c', script],
                                        env=fixture.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message.encode(), result.stderr)
                self.assertEqual(fixture.core_snapshot(), fixture.before)
                for path, before in fixture.formulas.items():
                    self.assertEqual((fixture.staged_core() / path).read_bytes(), before)

    def test_stanza_count_drift_fails_before_any_write(self):
        fixture = self.fixture()
        counts = dict(fixture.counts)
        counts[OBSOLETE] += 1
        script = replace_adapter_assignments(fixture.script(), {'metadata_stanzas': counts})
        result = subprocess.run(['bash', '--noprofile', '--norc', '-c', script], env=fixture.env,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'changed formula adaptation inventory', result.stderr)
        self.assertEqual(fixture.calls(), [['--repository']])
        self.assertEqual(fixture.core_snapshot(), fixture.before)
        for name, before in fixture.formulas.items():
            self.assertEqual((fixture.staged_core() / name).read_bytes(), before)

    def test_untracked_formula_cannot_enter_the_private_tap(self):
        fixture = self.fixture()
        fixture.after_checkout('(root / "Formula/l/unknown.rb").write_bytes(b"class Unknown < Formula\\nend\\n")')
        result = fixture.run()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'untracked or missing pinned formula member', result.stderr)
        self.assertEqual(fixture.calls(), [['--repository']])
        self.assertEqual(fixture.core_snapshot(), fixture.before)

    def test_linked_and_changed_mode_formulas_are_rejected_before_writes(self):
        for mutation in [
            'p = root / "Formula/c/cmake.rb"\np.unlink()\np.symlink_to(root / "Formula/l/llvm@19.rb")',
            'p = root / "Formula/c/cmake.rb"\np.chmod(0o755)',
            'p = root / "Formula/c/cmake.rb"\nimport os\nos.link(p, root.parent / "linked-formula")',
        ]:
            with self.subTest(mutation=mutation):
                fixture = self.fixture()
                fixture.after_checkout(mutation)
                result = fixture.run()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b'nonregular or changed-mode pinned formula: Formula/c/cmake.rb', result.stderr)
                self.assertEqual(fixture.calls(), [['--repository']])
                self.assertEqual(fixture.core_snapshot(), fixture.before)

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
                script = fixture.script()
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

    def signal_only_the_installer_shell(self, interruption, expected, ignore_term=False,
                                       before_pid_assignment=False):
        fixture = self.fixture()
        env = dict(fixture.env, PAUSE_FIXTURE_INSTALL='1')
        if ignore_term:
            env['IGNORE_FIXTURE_TERM'] = '1'
        script = fixture.script()
        if before_pid_assignment:
            # Widen the launch/assignment boundary without changing the actual
            # signal handler, cleanup, child job or wait implementation.
            script = script.replace('install_pid=$!\n',
                                    'while ! test -e "$INSTALL_FIXTURE/release-pid"; do sleep 0.01; done\n'
                                    'install_pid=$!\n', 1)
        process = subprocess.Popen(['bash', '--noprofile', '--norc', '-c', script],
                                   env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
        child = None
        try:
            deadline = time.monotonic() + 10
            active = fixture.root / 'install-active'
            while not active.exists():
                if process.poll() is not None or time.monotonic() > deadline:
                    self.fail('installer did not reach the paused install')
                time.sleep(0.01)
            child = json.loads(active.read_text())
            # Actions signals the shell PID first. Signalling the whole group
            # here would hide a trap deferred until foreground brew completes.
            os.kill(process.pid, interruption)
            try:
                stdout, stderr = process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                raise AssertionError('shell-PID cancellation did not stop the active installer within five seconds') from None
            self.assertEqual(process.returncode, expected, stderr.decode())
            self.assertFalse(fixture.environment.exists())
            self.assertFalse((fixture.temp / 'oxid-llvm-19').exists())
            self.assertEqual(fixture.calls(), [['--repository'],
                                              ['install', '--force-bottle', 'llvm@19', 'lld@19']])
            self.assertEqual(fixture.core_snapshot(), fixture.before)
            used = fixture.staged_core()
            self.assertEqual(git(used, 'rev-parse', 'HEAD').decode().strip(), fixture.revision)
            for name, original in fixture.formulas.items():
                self.assertEqual((used / name).read_bytes(), adapted_formula(original), name)
            with self.assertRaises(ProcessLookupError, msg='brew child was not stopped and reaped'):
                os.kill(child['pid'], 0)
            if not ignore_term:
                self.assertEqual(json.loads((fixture.root / 'install-stopped').read_text()),
                                 {'signal': signal.SIGTERM, 'private_tap_active': True})
        finally:
            # Emergency cleanup only; an earlier assertion has already failed
            # if either process remains active. It cannot turn a timeout green.
            if child is not None:
                try:
                    os.kill(child['pid'], 0)
                except ProcessLookupError:
                    pass
                else:
                    os.killpg(child['group'], signal.SIGKILL)
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
            process.communicate()

    def test_shell_pid_interrupt_and_termination_stop_brew_before_restoring_the_tap(self):
        for interruption, expected in [(signal.SIGINT, 130), (signal.SIGTERM, 143)]:
            with self.subTest(signal=interruption):
                self.signal_only_the_installer_shell(interruption, expected)

    def test_shell_pid_cancellation_reaps_a_child_that_ignores_int_and_term(self):
        self.signal_only_the_installer_shell(signal.SIGINT, 130, ignore_term=True)

    def test_shell_pid_cancellation_between_launching_brew_and_assigning_its_pid(self):
        for interruption, expected in [(signal.SIGINT, 130), (signal.SIGTERM, 143)]:
            with self.subTest(signal=interruption):
                self.signal_only_the_installer_shell(interruption, expected, before_pid_assignment=True)

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
