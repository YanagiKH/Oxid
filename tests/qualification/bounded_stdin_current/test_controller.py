"""Controller failure controls; stdlib only, no Rust or LLVM compilation."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import verify_bounded_stdin_native as gate

c = gate.controls


def child_output(name, markers, passed=1):
    body = "\n".join(key + "=" + value for key, value in markers.items())
    return (f"\nrunning 1 test\ntest {name} ... {body}\nok\n\n"
            f"test result: ok. {passed} passed; 0 failed; 0 ignored; 0 measured; 1374 filtered out; finished in 0.01s\n").encode()


class SelectionTests(unittest.TestCase):
    def test_exact_discovery_rejects_zero_extra_duplicate_and_wrong_names(self):
        valid = (c.RAW_TEST + ": test\n\n1 test, 0 benchmarks\n").encode()
        c.admit_listing(valid, c.RAW_TEST)
        for invalid in (b"0 tests, 0 benchmarks\n", valid + valid,
                        valid.replace(b"1 test,", b"2 tests,"), valid.replace(b"builtin_input_subprocess", b"other")):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                c.admit_listing(invalid, c.RAW_TEST)

    def test_execution_requires_one_actual_child_and_unambiguous_markers(self):
        markers = {"OXID_RAW_STDIN_RESULT": "-1", "OXID_RAW_STDIN_REMAINING_FUEL": "0"}
        valid = child_output(c.RAW_TEST, markers)
        result = subprocess.CompletedProcess([], 0, valid, b"")
        self.assertEqual(c.admit_child(result, c.RAW_TEST, markers), markers)
        variants = (valid.replace(b"1 passed", b"0 passed"),
                    valid.replace(b"running 1 test", b"running 0 tests"),
                    valid.replace(b"OXID_RAW_STDIN_RESULT=-1", b"OXID_RAW_STDIN_RESULT=-1\nOXID_RAW_STDIN_RESULT=-1"),
                    valid.replace(b"OXID_RAW_STDIN_RESULT=-1", b""),
                    valid.replace(b"builtin_input_subprocess_child", b"other"), valid + b"unexpected\n")
        for data in variants:
            with self.subTest(data=data), self.assertRaises(ValueError):
                c.admit_child(subprocess.CompletedProcess([], 0, data, b""), c.RAW_TEST, markers)

    def test_fixed_roster_includes_reviewed_boundaries(self):
        self.assertEqual(c.ROSTER, {"raw": 24, "fuel": 25, "shapes": 15, "private-source": 6, "public-source": 7})
        self.assertEqual((len(c.RAW_CASES), len(c.FUEL_CASES), len(c.SHAPE_CASES), len(c.SOURCE_CASES)), (12, 25, 5, 6))
        self.assertEqual({row[1] for row in c.RAW_CASES}, {0, 3, 129, 1024})
        self.assertEqual(sum(row[-1] for row in c.FUEL_CASES), 11)
        self.assertEqual(len({row[0] for row in c.FUEL_CASES}), 25)

    def test_fixture_selectors_and_preloads_do_not_leak(self):
        base = {"PATH": "a", "HOME": "b", "LD_PRELOAD": "bad", "LD_AUDIT": "bad",
                "OXID_RAW_STDIN_FUEL": "0", "OXID_PRIVATE_SOURCE_STDIN": "1", "OXID_LLVM_BIN": "/llvm"}
        env = c.child_env(base, 3, "status", 75)
        self.assertEqual(env, {"PATH": "a", "HOME": "b", "OXID_LLVM_BIN": "/llvm",
            "OXID_RAW_STDIN_CAPACITY": "3", "OXID_RAW_STDIN_MODE": "status", "OXID_RAW_STDIN_FUEL": "75"})
        self.assertEqual(base["LD_PRELOAD"], "bad")


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="bounded-stdin-controls-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_fresh_output_and_regular_inputs_reject_symlinks_and_oversize(self):
        c.fresh(self.root / "fresh")
        with self.assertRaises(FileExistsError):
            c.fresh(self.root / "fresh")
        link = self.root / "alias"
        link.symlink_to(self.root / "fresh", target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            c.fresh(link / "new")
        source = self.root / "source"
        source.write_bytes(b"1234")
        with self.assertRaisesRegex(ValueError, "oversized"):
            c.read_file(source, 3)
        with self.assertRaisesRegex(ValueError, "nonregular"):
            c.read_file(self.root)

    def test_pipe_and_file_unread_bytes_are_observed_without_overread(self):
        command = [sys.executable, "-c", "import os; print(os.read(0,3).hex())"]
        for kind in ("pipe", "file", "nonblocking"):
            result, left = c.run(self.root / kind, command, c.ELF_ENV, b"ABCZ", kind)
            self.assertEqual((result.returncode, result.stdout, result.stderr, left), (0, b"414243\n", b"", b"Z"))
            receipt = json.loads((self.root / kind / "receipt.json").read_text())
            self.assertEqual(receipt["stdin_kind"], kind)
            self.assertEqual(receipt["environment"]["mode"], "cleared-runtime")

    def test_real_nonblocking_partial_error_and_directory_error(self):
        source = "import os\ntry:\n a=os.read(0,1); os.read(0,1)\nexcept OSError as e: print(e.errno)"
        result, left = c.run(self.root / "partial", [sys.executable, "-c", source], c.ELF_ENV, b"A", "nonblocking")
        self.assertEqual((result.stdout, left), (b"11\n", b""))
        # Python rejects directory stdin during its own startup. Use a native
        # ordinary reader to observe the controller's actual EISDIR fd instead.
        result, left = c.run(self.root / "directory", ["/bin/cat"], c.ELF_ENV, kind="directory")
        self.assertEqual((result.returncode, result.stdout, left), (1, b"", b""))
        self.assertIn(b"Is a directory", result.stderr)

    def test_failure_and_timeout_always_retain_status_streams_and_invocation(self):
        result, _ = c.run(self.root / "failed", [sys.executable, "-c",
            "import sys; print('out'); print('err',file=sys.stderr); sys.exit(9)"], c.ELF_ENV)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (9, b"out\n", b"err\n"))
        with self.assertRaisesRegex(ValueError, "process failure"):
            c.run(self.root / "timeout", [sys.executable, "-c", "import time; time.sleep(5)"], c.ELF_ENV, timeout=0.05)
        receipt = json.loads((self.root / "timeout/receipt.json").read_text())
        self.assertEqual(receipt["status"], -9)
        self.assertIn("timed out", receipt["error"])
        self.assertTrue((self.root / "timeout/stdout").is_file())
        self.assertTrue((self.root / "timeout/stderr").is_file())

    def test_source_authority_rejects_rehashed_reduced_manifest(self):
        with self.assertRaisesRegex(ValueError, "reviewed current authority"):
            gate.source_manifest(self.root, "0" * 64)
        manifest_dir = self.root / Path(gate.MANIFEST_PATH).parent
        manifest_dir.mkdir(parents=True)
        (self.root / gate.MANIFEST_PATH).write_text('{"files": []}')
        with self.assertRaisesRegex(ValueError, "seal differs"):
            gate.source_manifest(self.root, gate.REVIEWED_SOURCE_SHA256)

    def test_real_current_source_authority_and_full_core_closure(self):
        manifest = gate.source_manifest(ROOT, gate.REVIEWED_SOURCE_SHA256)
        core = [row["path"] for row in manifest["files"] if row["path"].startswith(("src/", "native/"))
                or row["path"] in ("Cargo.toml", "Cargo.lock", "build.rs")]
        listing = b"\0".join(name.encode() for name in core) + b"\0"
        self.assertEqual(len(gate.source_identity(ROOT, manifest, listing)), 252)
        for invalid in (listing + b"src/extra.rs\0", listing.split(b"\0", 1)[1], listing + b"src/cli.rs\0"):
            with self.subTest(invalid=invalid[-30:]), self.assertRaisesRegex(ValueError, "closure"):
                gate.source_identity(ROOT, manifest, invalid)

    def test_ordinary_prebuilt_profiles_reject_wrong_profile_head_and_artifact(self):
        repo, target = self.root / "checkout", self.root / "target"
        executable = target / "debug/deps/oxid-0123456789abcdef"
        executable.parent.mkdir(parents=True)
        executable.write_bytes(b"\x7fELFfixture")
        row = {"reason": "compiler-artifact", "target": {"name": "oxid", "kind": ["bin"], "src_path": str(repo / "src/cli.rs")},
            "profile": {"test": True, "opt_level": "0", "debug_assertions": True, "overflow_checks": True, "debuginfo": 2},
            "executable": str(executable)}
        data = (json.dumps(row) + '\n{"reason":"build-finished","success":true}\n').encode()
        self.assertEqual(gate.admit_build_artifact(data, repo, target, "debug", True)[0], executable)
        for bad_data, bad_repo, profile in ((data, repo, "release"), (data, repo / "other", "debug"),
                                           (data.replace(b'"success":true', b'"success":false'), repo, "debug")):
            with self.subTest(profile=profile), self.assertRaises(ValueError):
                gate.admit_build_artifact(bad_data, bad_repo, target, profile, True)

    def test_calibration_literals_have_exact_two_injections_and_other_fd_parity(self):
        clean = c.calibration_expected("clean")
        injected = c.calibration_expected("injected")
        empty = c.calibration_expected("empty-clean")
        self.assertEqual(injected.count(b"result=-1 errno=4 "), 2)
        self.assertNotIn(b"errno=4 ", clean + empty)
        self.assertEqual([line for line in clean.splitlines() if line.startswith(b"other-")],
                         [line for line in injected.splitlines() if line.startswith(b"other-")])
        self.assertIn(b"stdin-one-3 count=1 result=1 errno=0 buffer=435a", injected)


if __name__ == "__main__":
    unittest.main()
