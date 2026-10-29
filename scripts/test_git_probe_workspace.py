"""Real Git ownership checks under the native drivers' existing minimal env."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import verify_owned_record_vectors as public
import verify_owned_text as text


@unittest.skipUnless(os.name == "posix" and shutil.which("git"),
                     "native-driver Git process controls require real Git on a POSIX host")
class GitProbeWorkspaceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-git-probe-workspace-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repo = self.root / "bound repository"
        self.other = self.root / "different repository"
        self.real_git = str(Path(shutil.which("git")).resolve(strict=True))
        trusted = self.root / "unrelated global trust"
        trusted.mkdir()
        configuration = self.root / "unrelated.gitconfig"
        configuration.write_text("[safe]\n\tdirectory = " + json.dumps(str(trusted)) + "\n")
        setup_environment = {"PATH": str(Path(self.real_git).parent), "LANG": "C.UTF-8",
                             "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": str(configuration)}
        for repo in (self.repo, self.other):
            initialized = subprocess.run([self.real_git, "init", "--quiet", repo],
                env=setup_environment, capture_output=True, timeout=10)
            self.assertEqual(initialized.returncode, 0, initialized.stderr)
            committed = subprocess.run([self.real_git, "-C", repo, "-c", "user.name=Oxid",
                "-c", "user.email=oxid@example.invalid", "commit", "--quiet", "--allow-empty",
                "-m", "local ownership fixture"], env=setup_environment, capture_output=True, timeout=10)
            self.assertEqual(committed.returncode, 0, committed.stderr)
        expected = subprocess.run([self.real_git, "-C", self.repo, "rev-parse", "HEAD"],
            env=setup_environment, capture_output=True, timeout=10)
        self.assertEqual(expected.returncode, 0, expected.stderr)
        self.expected_head = expected.stdout
        self.capture = self.root / "actual-git-invocations.jsonl"
        binaries = self.root / "bin"
        binaries.mkdir()
        wrapper = binaries / "git"
        wrapper.write_text("#!" + sys.executable + "\n"
            "import json, os, sys\n"
            f"with open({str(self.capture)!r}, 'a') as output:\n"
            "    output.write(json.dumps({'argv': sys.argv[1:], 'environment': dict(os.environ)}) + '\\n')\n"
            "environment = dict(os.environ)\n"
            # This is Git's actual ownership-policy test seam, not a fake Git
            # result. It permits non-root test workers to exercise the real
            # different-owner refusal without chown or trusting any user config.
            "environment['GIT_TEST_ASSUME_DIFFERENT_OWNER'] = '1'\n"
            "environment['GIT_CONFIG_NOSYSTEM'] = '1'\n"
            f"environment['GIT_CONFIG_GLOBAL'] = {str(configuration)!r}\n"
            f"os.execve({self.real_git!r}, [{self.real_git!r}, *sys.argv[1:]], environment)\n")
        wrapper.chmod(0o755)
        self.environment = {"PATH": str(binaries) + os.pathsep + str(Path(self.real_git).parent),
                            "LANG": "C.UTF-8"}

    def test_both_native_drivers_trust_only_the_bound_repo_without_home_or_global_configuration(self):
        arguments = ["git", "--no-replace-objects", "rev-parse", "HEAD"]
        refused = subprocess.run(arguments, cwd=self.repo, env=self.environment,
                                 capture_output=True, timeout=10)
        self.assertEqual(refused.returncode, 128, refused.stderr)
        self.assertIn(b"detected dubious ownership", refused.stderr)
        for label, driver_type in (("records", public.Driver), ("text", text.Driver)):
            with self.subTest(driver=label):
                output = self.root / label
                output.mkdir()
                report = {}
                with patch.object(public, "ROOT", self.repo):
                    driver = driver_type(output, self.environment, report)
                result = driver.command("head", arguments, self.repo)
                self.assertEqual(result.stdout, self.expected_head)
                self.assertEqual(result.stderr, b"")
                self.assertEqual(driver.repo, self.repo.resolve(strict=True))
                self.assertEqual(report["commands"][0]["argv"],
                    ["git", "-c", "safe.directory=" + str(self.repo.resolve(strict=True)), *arguments[1:]])
                observed = json.loads(self.capture.read_text().splitlines()[-1])
                self.assertEqual(observed["argv"], report["commands"][0]["argv"][1:])
                self.assertNotIn("HOME", observed["environment"])
                self.assertNotIn("GIT_CONFIG_GLOBAL", observed["environment"])
                self.assertEqual(report["commands"][0]["environment"], self.environment)
                self.assertEqual(arguments, ["git", "--no-replace-objects", "rev-parse", "HEAD"])
                with self.assertRaisesRegex(ValueError, "unexpected status"):
                    driver.command("different-repository", arguments, self.other)
                failure = report["commands"][-1]
                self.assertEqual((failure["status"], failure["exit_status"]), ("failed", 128))
                self.assertIn(b"detected dubious ownership",
                    (output / "commands/0001-different-repository/stderr.bin").read_bytes())

    def test_explicit_repository_is_resolved_and_non_git_commands_keep_the_minimal_environment(self):
        for label, driver_type in (("records-explicit", public.Driver), ("text-explicit", text.Driver)):
            with self.subTest(driver=label):
                output = self.root / label
                output.mkdir()
                report = {}
                driver = driver_type(output, self.environment, report, repo=self.repo / ".." / self.repo.name)
                self.assertEqual(driver.repo, self.repo.resolve(strict=True))
                arguments = [sys.executable, "-c", "import os; print('HOME' in os.environ)"]
                result = driver.command("non-git", arguments, self.repo)
                self.assertEqual((result.returncode, result.stdout, result.stderr), (0, b"False\n", b""))
                self.assertEqual(report["commands"][0]["argv"], arguments)
                self.assertEqual(report["commands"][0]["environment"], self.environment)
                for name in ("HOME", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_COUNT"):
                    with self.subTest(forbidden_environment=name), patch.object(public.subprocess, "Popen") as spawn:
                        with self.assertRaisesRegex(ValueError, "allowlisted name"):
                            driver.command("forbidden-" + name, arguments, self.repo,
                                           overrides={name: str(self.root)})
                        spawn.assert_not_called()
        self.assertNotIn("HOME", public.ENV_NAMES)
        self.assertNotIn("GIT_CONFIG_GLOBAL", public.ENV_NAMES)
        self.assertNotIn("GIT_CONFIG_COUNT", public.ENV_NAMES)


if __name__ == "__main__":
    unittest.main()
