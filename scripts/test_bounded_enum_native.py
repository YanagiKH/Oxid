"""Fail-closed selection and subprocess evidence controls; never build Rust/LLVM."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

import verify_bounded_enum_native as gate


def listing(names):
    return ("\n".join(name + ": test" for name in names)
            + f"\n\n{len(names)} tests, 0 benchmarks\n").encode()


def execution(name, marker=""):
    output = marker + "\nok" if marker else "ok"
    return (f"\nrunning 1 test\ntest {name} ... {output}\n\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
            "987 filtered out; finished in 0.01s\n\n").encode()


class BoundedEnumNativeControls(unittest.TestCase):
    def test_frozen_sources_and_explicit_rosters(self):
        self.assertEqual(hashlib.sha256(gate.TINY).hexdigest(),
                         "95bbfb95b733bc272e2b11e6772b66a74bf14507f4a86a7f3e9258d4c88d2220")
        self.assertEqual(len(gate.RAW_NAMES), 6)
        self.assertEqual(len(gate.SOURCE_NAMES), 2)
        self.assertEqual(len(set((*gate.RAW_NAMES, *gate.SOURCE_NAMES))), 8)
        repo = Path(__file__).resolve().parents[1]
        for filename, expected in gate.SCANNER_HASHES.items():
            self.assertEqual(hashlib.sha256((repo / "tests/fixtures/bounded_enum_scanner" / filename)
                                           .read_bytes()).hexdigest(), expected)

    def test_ignored_discovery_accepts_only_the_complete_named_rosters(self):
        for names in (gate.RAW_NAMES, gate.SOURCE_NAMES):
            gate.admit_listing(listing(names), names)
            gate.admit_listing(listing(tuple(reversed(names))), names)
            for changed in ((), names[:-1], (*names, "unexpected"),
                            (*names[:-1], names[0]), (*names[:-1], names[-1] + "_renamed")):
                with self.subTest(names=len(names), changed=changed):
                    with self.assertRaises(ValueError):
                        gate.admit_listing(listing(changed), names)
            for changed in (listing(names).replace(b": test", b": benchmark", 1),
                            listing(names).replace(b"0 benchmarks", b"1 benchmarks"),
                            listing(names) + b"unexpected: test\n"):
                with self.assertRaises(ValueError):
                    gate.admit_listing(changed, names)

    def test_execution_rejects_zero_ignored_failed_wrong_or_duplicate_selection(self):
        name = gate.RAW_NAMES[0]
        good = execution(name)
        gate.admit_execution(good, name)
        for changed in (
            good.replace(b"running 1 test", b"running 0 tests"),
            good.replace(b"1 passed", b"0 passed"),
            good.replace(b"0 ignored", b"1 ignored"),
            good.replace(b"0 failed", b"1 failed"),
            good.replace(name.encode(), gate.RAW_NAMES[1].encode()),
            good.replace(b"test result: ok.", b"test result: FAILED."),
            good.replace(b" ... ok", b" ... ignored"),
            good.replace(b" ... ok", b" ... FAILED"),
            good + good,
            good.replace(b"test result:", b"missing summary:"),
        ):
            with self.subTest(changed=changed):
                with self.assertRaises(ValueError):
                    gate.admit_execution(changed, name)

    def test_tool_version_does_not_accept_adjacent_or_newer_versions(self):
        for marker in ("clang version", "LLVM version", "LLD"):
            gate.admit_tool_version("tool", marker, ("Debian " + marker + " 19.1.7\n").encode())
            for suffix in ("19.1.70", "19.1.8", "20.1.7", "19.1.7-extra"):
                with self.assertRaises(ValueError):
                    gate.admit_tool_version("tool", marker, (marker + " " + suffix + "\n").encode())

    def test_source_success_and_boundary_markers_keep_existing_counts(self):
        for name, marker in zip(gate.SOURCE_NAMES, (
            "ENUM_SOURCE_NATIVE_SUCCESS artifacts=2 runs=2",
            "ENUM_SOURCE_NATIVE_BOUNDARIES artifacts=14 runs=21",
        )):
            good = execution(name, marker)
            gate.admit_execution(good, name)
            for changed in (execution(name), good.replace(b"artifacts=", b"artifacts=0"),
                            good.replace(b"runs=", b"runs=0")):
                with self.assertRaises(ValueError):
                    gate.admit_execution(changed, name)

    def test_reviewed_source_identity_rejects_missing_extra_duplicate_and_changed_inputs(self):
        repo = Path("/repo")
        manifest = {"files": [{"path": "src/cli.rs", "bytes": 3, "sha256": "frozen"},
                              {"path": "rfcs/old.md", "bytes": 9, "sha256": "other"}]}
        observed = [{"path": "/repo/src/cli.rs", "bytes": 3, "sha256": "frozen"}]
        gate.admit_source_identity(repo, observed, manifest)
        for changed in ([], observed + observed,
                        observed + [{"path": "/repo/src/extra.rs", "bytes": 0, "sha256": "extra"}],
                        [dict(observed[0], sha256="changed")], [dict(observed[0], bytes=4)]):
            with self.assertRaises(ValueError):
                gate.admit_source_identity(repo, changed, manifest)
        with self.assertRaises(ValueError):
            gate.admit_source_identity(repo, observed, {"files": manifest["files"] * 2})

    def test_full_reviewed_identity_includes_retained_non_source_fixtures(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            path = repo / "fixtures/retained/main.ox"
            path.parent.mkdir(parents=True)
            path.write_bytes(b"frozen")
            manifest = {"files": [{"path": "fixtures/retained/main.ox", "bytes": 6,
                                   "sha256": hashlib.sha256(b"frozen").hexdigest()}]}
            before = gate.reviewed_input_identities(repo, manifest)
            self.assertEqual(before, [gate.identity(path)])
            path.write_bytes(b"edited")
            with self.assertRaises(ValueError):
                gate.reviewed_input_identities(repo, manifest)
            path.write_bytes(b"frozen")
            self.assertEqual(gate.reviewed_input_identities(repo, manifest), before)
            with self.assertRaises(ValueError):
                gate.reviewed_input_identities(repo, {"files": manifest["files"] * 2})
            path.unlink()
            with self.assertRaises(FileNotFoundError):
                gate.reviewed_input_identities(repo, manifest)

    def test_approved_manifest_rejects_coherent_omission_and_rehash(self):
        repo = Path(__file__).resolve().parents[1]
        original = repo / "tests/fixtures/typed_project_source_binding/current-source.json"
        manifest = gate.read_reviewed_manifest(original)
        self.assertEqual(len(manifest["files"]), 237)
        reduced = dict(manifest, files=[row for row in manifest["files"]
            if row["path"].startswith(("src/", "native/", "tests/fixtures/bounded_enum_scanner/"))
            or row["path"] in ("Cargo.toml", "Cargo.lock", "build.rs")])
        self.assertEqual(len(reduced["files"]), 184)
        self.assertEqual(reduced["reviewed_source_head"], "511df03975c2aa1a815f92155d673adb3571ff77")
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / "current-source.json"
            candidate.write_text(json.dumps(reduced, sort_keys=True, indent=2) + "\n")
            # All retained member hashes and the feature label remain coherent;
            # a freshly recomputed candidate receipt cannot authorize omission.
            rehashed_receipt = gate.identity(candidate)
            self.assertEqual(rehashed_receipt["sha256"], hashlib.sha256(candidate.read_bytes()).hexdigest())
            self.assertNotEqual(rehashed_receipt["sha256"], gate.REVIEWED_SOURCE_SHA256)
            with self.assertRaisesRegex(ValueError, "approved authority"):
                gate.read_reviewed_manifest(candidate)
            candidate.write_bytes(b"not JSON")
            with self.assertRaisesRegex(ValueError, "approved authority"):
                gate.read_reviewed_manifest(candidate)

    def test_cargo_artifact_is_unambiguous_and_standard_profile(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve()
            for profile in ("debug", "release"):
                for unit in (True, False):
                    executable = repo / "target" / profile / ("deps/oxid-test" if unit else "oxid")
                    executable.parent.mkdir(parents=True, exist_ok=True)
                    executable.write_bytes(b"artifact identity only; never executed")
                    artifact = {"reason": "compiler-artifact", "target": {"name": "oxid", "kind": ["bin"]},
                                "profile": {"test": unit, "opt_level": "0" if profile == "debug" else "3",
                                            "debug_assertions": profile == "debug",
                                            "overflow_checks": profile == "debug",
                                            "debuginfo": 2 if profile == "debug" else 0},
                                "executable": str(executable)}
                    finished = {"reason": "build-finished", "success": True}
                    def encoded(rows):
                        return b"\n".join(json.dumps(row).encode() for row in rows)
                    self.assertEqual(gate.oxid_artifact(encoded([artifact, finished]), repo, profile,
                                                        unit=unit), executable)
                    for rows in ([finished], [artifact, artifact, finished], [artifact],
                                 [artifact, {"reason": "build-finished", "success": False}],
                                 [artifact, finished, finished]):
                        with self.assertRaises(ValueError):
                            gate.oxid_artifact(encoded(rows), repo, profile, unit=unit)
                    for key, value in (("test", not unit), ("opt_level", "1"),
                                       ("debug_assertions", profile != "debug"),
                                       ("overflow_checks", profile != "debug"),
                                       ("debuginfo", 0 if profile == "debug" else 2),
                                       ("debuginfo", None), ("debuginfo", False)):
                        changed = dict(artifact, profile=dict(artifact["profile"], **{key: value}))
                        with self.assertRaises(ValueError):
                            gate.oxid_artifact(encoded([changed, finished]), repo, profile, unit=unit)
                    changed = dict(artifact, executable=str(Path(__file__).resolve()))
                    with self.assertRaises(ValueError):
                        gate.oxid_artifact(encoded([changed, finished]), repo, profile, unit=unit)

    def test_command_preserves_failure_streams_and_exact_source_free_environment(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "runtime"
            runtime.mkdir()
            stdout, stderr = gate.command(root, "environment", [sys.executable, "-c",
                "import json,os; print(json.dumps({'cwd':os.getcwd(),'env':dict(os.environ)}))"],
                cwd=runtime, env=gate.ELF_ENV)
            # CPython can add LC_CTYPE during startup; the launch receipt remains exact.
            result = json.loads(stdout)
            self.assertEqual(result["cwd"], str(runtime))
            self.assertEqual(result["env"].get("PATH"), "/no-tools")
            self.assertFalse(set(result["env"]) - {"PATH", "LC_CTYPE"})
            self.assertEqual(stderr, b"")
            self.assertEqual(json.loads((root / "environment.json").read_text())["environment"],
                             {"PATH": "/no-tools"})
            with self.assertRaises(ValueError):
                gate.command(root, "failed", [sys.executable, "-c",
                    "import sys; print('kept out'); print('kept err',file=sys.stderr); sys.exit(23)"],
                    cwd=runtime, env=dict(os.environ))
            self.assertEqual((root / "failed.stdout").read_bytes(), b"kept out\n")
            self.assertEqual((root / "failed.stderr").read_bytes(), b"kept err\n")
            self.assertEqual(json.loads((root / "failed.json").read_text())["status"], 23)
            with self.assertRaises(ValueError):
                gate.command(root, "missing", [root / "missing-tool"], cwd=runtime, env=gate.ELF_ENV)
            self.assertIsNone(json.loads((root / "missing.json").read_text())["status"])

    def test_timeout_retains_partial_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(ValueError):
                gate.command(root, "timeout", [sys.executable, "-c",
                    "import time; print('partial',flush=True); time.sleep(5)"],
                    cwd=root, env=dict(os.environ), timeout=0.2)
            self.assertEqual((root / "timeout.stdout").read_bytes(), b"partial\n")
            result = json.loads((root / "timeout.json").read_text())
            self.assertIsNone(result["status"])
            self.assertIn("timed out", result["error"])


if __name__ == "__main__":
    unittest.main()
