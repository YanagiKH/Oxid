"""Driver controls only; fake commands do not qualify LLVM or native execution."""
from pathlib import Path
import sys
import tempfile
import unittest

import verify_checked_hir_import_native as gate


@unittest.skipUnless(sys.platform == "linux", "Linux-only process-group controls")
class DriverTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def run_python(self, text, name="probe", seconds=5):
        return gate.run(self.root, name, [sys.executable, "-c", text], cwd=self.root,
                        env={"PATH": "/no-tools"}, seconds=seconds)

    def test_exact_streams_and_nonzero_exit_are_preserved(self):
        result = self.run_python("import sys; print('out'); print('err', file=sys.stderr); sys.exit(7)")
        self.assertEqual(result, (7, b"out\n", b"err\n"))
        with self.assertRaisesRegex(RuntimeError, "status 7"):
            gate.success(result, "probe")
        self.assertTrue((self.root / "probe.json").is_file())

    def test_timeout_kills_process_group_and_retains_receipt(self):
        with self.assertRaisesRegex(RuntimeError, "exceeded"):
            self.run_python("import time; time.sleep(20)", seconds=0.05)
        import json
        receipt = json.loads((self.root / "probe.json").read_text())
        self.assertTrue(receipt["timed_out"])
        self.assertEqual(receipt["status"], -9)

    def test_timeout_kills_spawned_descendant(self):
        import time
        child = "import os, pathlib, time; pathlib.Path('descendant.pid').write_text(str(os.getpid())); time.sleep(30)"
        leader = f"import subprocess, sys, time; subprocess.Popen([sys.executable, '-c', {child!r}]); time.sleep(30)"
        with self.assertRaisesRegex(RuntimeError, "exceeded"):
            self.run_python(leader, seconds=0.5)
        pid = int((self.root / "descendant.pid").read_text())
        state = Path(f"/proc/{pid}/stat")
        for _ in range(100):
            try:
                # A killed orphan can briefly remain a zombie for host init.
                if state.read_text().split(") ", 1)[1].split()[0] == "Z":
                    break
            except FileNotFoundError:
                break
            time.sleep(0.01)
        else:
            self.fail("timed-out descendant is still running")

    def test_evidence_rejects_even_ignored_in_tree_destinations(self):
        with self.assertRaisesRegex(RuntimeError, "outside the checkout"):
            gate.evidence_destination(gate.ROOT / "target" / "native-gate-evidence")
        self.assertEqual(gate.evidence_destination(self.root), self.root.resolve())

    def test_no_stdin_or_ambient_environment(self):
        result = self.run_python("import os, sys; assert sys.stdin.read() == ''; assert 'HOME' not in os.environ")
        self.assertEqual(result, (0, b"", b""))

    def test_evidence_cannot_be_replaced(self):
        self.run_python("print('original')")
        with self.assertRaises(FileExistsError):
            self.run_python("print('replacement')")
        self.assertEqual((self.root / "probe.stdout").read_bytes(), b"original\n")

    def test_output_read_is_bounded(self):
        with self.assertRaisesRegex(RuntimeError, "1 MiB"):
            self.run_python("import sys; sys.stdout.write('x' * (2 << 20))")
        self.assertEqual((self.root / "probe.stdout").stat().st_size, 2 << 20)

    def test_elf_requires_pie_architecture(self):
        path = self.root / "program.elf"
        header = bytearray(20)
        header[:6] = b"\x7fELF\x02\x01"
        header[16:20] = b"\x03\x00\x3e\x00"
        path.write_bytes(header)
        gate.verify_elf(path)
        header[16] = 2
        path.write_bytes(header)
        with self.assertRaisesRegex(RuntimeError, "ELF64 PIE"):
            gate.verify_elf(path)


@unittest.skipUnless(sys.platform == "linux", "Linux-only Git/native driver")
class GitIdentityTests(unittest.TestCase):
    def setUp(self):
        import subprocess
        from unittest.mock import patch
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.repo = self.base / "repo"
        self.repo.mkdir()
        self.git_env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C",
                        "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
        subprocess.run(["/usr/bin/git", "init", "-q", str(self.repo)], env=self.git_env, check=True)
        (self.repo / "source").write_text("original")
        self.git("add", "source")
        self.git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                 "commit", "-qm", "initial")
        self.patch = patch.object(gate, "ROOT", self.repo)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.proof = self.base / "proof"
        self.proof.mkdir()

    def git(self, *args):
        import subprocess
        return subprocess.run(["/usr/bin/git", "-C", str(self.repo), *args],
                              env=self.git_env, check=True, capture_output=True)

    def test_exact_checkout_trust_under_simulated_ownership_mismatch(self):
        import json
        from unittest.mock import patch
        actual_run = gate.run
        def different_owner(root, name, argv, **kwargs):
            kwargs["env"] = {**kwargs["env"], "GIT_TEST_ASSUME_DIFFERENT_OWNER": "1"}
            return actual_run(root, name, argv, **kwargs)
        with patch.object(gate, "run", side_effect=different_owner):
            old = gate.run(self.proof, "old", ["git", "rev-parse", "HEAD"],
                           cwd=self.repo, env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
            self.assertEqual(old[0], 128)
            self.assertIn(b"dubious ownership", old[2])
            for name, args in (("git-head", ("rev-parse", "HEAD")),
                               ("git-status", ("status", "--porcelain=v1")),
                               ("git-head-after", ("rev-parse", "HEAD")),
                               ("git-status-after", ("status", "--porcelain=v1"))):
                self.assertEqual(gate.git_read(self.proof, name, *args)[0], 0)
                receipt = json.loads((self.proof / (name + ".json")).read_text())
                self.assertEqual(receipt["argv"], ["/usr/bin/git", "-c", "safe.directory=" + str(self.repo),
                                                   "-C", str(self.repo), *args])
                self.assertEqual(receipt["cwd"], str(self.repo))
                self.assertNotIn("HOME", receipt["env"])
            sibling = self.base / "sibling"
            import subprocess
            subprocess.run(["/usr/bin/git", "init", "-q", str(sibling)], env=self.git_env, check=True)
            denied = gate.run(self.proof, "sibling", ["/usr/bin/git", "-c", "safe.directory=" + str(self.repo),
                              "-C", str(sibling), "status", "--porcelain=v1"], cwd=sibling, env=self.git_env)
            self.assertEqual(denied[0], 128)
            self.assertIn(b"dubious ownership", denied[2])

    def test_stale_or_nested_root_is_rejected(self):
        from unittest.mock import patch
        with patch.object(gate, "ROOT", self.repo / "missing"):
            with self.assertRaises(FileNotFoundError):
                gate.git_read(self.proof, "missing", "rev-parse", "HEAD")
        nested = self.repo / "nested"
        nested.mkdir()
        with patch.object(gate, "ROOT", nested):
            with self.assertRaisesRegex(RuntimeError, "checkout root"):
                gate.git_read(self.proof, "nested", "rev-parse", "HEAD")

    def qualify_control(self, mutation=None):
        from unittest.mock import patch
        binary = self.base / "binary"
        binary.write_bytes(b"test binary")
        llvm = self.base / "llvm"
        llvm.mkdir()
        for tool in ("clang", "opt", "llvm-as", "ld.lld"):
            (llvm / tool).write_bytes(b"test tool")
        actual_run = gate.run
        def controlled(root, name, argv, **kwargs):
            if name.startswith("git-"):
                return actual_run(root, name, argv, **kwargs)
            self.assertEqual(set(kwargs["env"]), {"PATH", "LC_ALL"} |
                             ({"OXID_HIR_IMPORT_NATIVE_EVIDENCE"} if name == "export" else set()))
            if name == "export":
                if mutation:
                    mutation(binary)
                return 0, b"1 passed; 0 failed", b""
            marker = "clang version" if name == "version-clang" else "LLD" if name == "version-ld.lld" else "LLVM version"
            return 0, (marker + " 19.1.7\n").encode(), b""
        self.result = self.base / "qualification"
        with patch.object(gate, "run", side_effect=controlled), patch.object(gate, "CASES", ()):
            gate.qualify(binary, llvm, self.result)

    def test_all_four_reads_and_native_environment_stay_isolated(self):
        import json
        from unittest.mock import patch
        with patch.dict("os.environ", {"HOME": "/ambient", "GIT_DIR": "/wrong", "GIT_CONFIG_COUNT": "1",
                                       "GIT_CONFIG_KEY_0": "safe.directory", "GIT_CONFIG_VALUE_0": "*"}):
            self.qualify_control()
        for name in ("git-head", "git-status", "git-head-after", "git-status-after"):
            receipt = json.loads((self.result / (name + ".json")).read_text())
            self.assertEqual(receipt["env"], {**self.git_env, "GIT_OPTIONAL_LOCKS": "0"})
        self.assertEqual(json.loads((self.result / "manifest.json").read_text())["result"], "passed")

    def test_dirty_checkout_rejects_and_retains_failure(self):
        import json
        (self.repo / "source").write_text("dirty")
        with self.assertRaisesRegex(RuntimeError, "clean checkout"):
            self.qualify_control()
        self.assertEqual(json.loads((self.result / "manifest.json").read_text())["result"], "failed")
        self.assertIn(b"source", (self.result / "git-status.stdout").read_bytes())

    def test_changed_source_rejects(self):
        with self.assertRaisesRegex(RuntimeError, "changed during qualification"):
            self.qualify_control(lambda binary: (self.repo / "source").write_text("changed"))

    def test_changed_head_rejects(self):
        def commit(binary):
            self.git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                     "commit", "--allow-empty", "-qm", "changed")
        with self.assertRaisesRegex(RuntimeError, "changed during qualification"):
            self.qualify_control(commit)

    def test_changed_binary_rejects(self):
        with self.assertRaisesRegex(RuntimeError, "changed during qualification"):
            self.qualify_control(lambda binary: binary.write_bytes(b"changed"))


if __name__ == "__main__":
    unittest.main()
