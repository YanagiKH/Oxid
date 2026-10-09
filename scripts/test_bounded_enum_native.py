"""Fail-closed selection and subprocess evidence controls; never build Rust/LLVM."""
import hashlib
import importlib.util
import json
import os
import re
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import verify_bounded_enum_native as gate


def listing(names):
    return ("\n".join(name + ": test" for name in names)
            + f"\n\n{len(names)} tests, 0 benchmarks\n").encode()


def execution(name, marker=""):
    output = marker + "\nok" if marker else "ok"
    return (f"\nrunning 1 test\ntest {name} ... {output}\n\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
            "987 filtered out; finished in 0.01s\n\n").encode()


class GitCheckoutControls(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="bounded-enum-git-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.global_config = self.root / "global-config"
        self.global_config.write_text("[user]\n\tname = Untouched global config\n")
        self.env = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"], "LC_ALL": "C",
                    "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": str(self.global_config)}
        self.repo = self.root / "checkout"
        self.sibling = self.root / "sibling"
        for repo in (self.repo, self.sibling):
            repo.mkdir()
            (repo / "src").mkdir()
            (repo / "src/main.rs").write_text("// " + repo.name + "\nfn main() {}\n")
            for label, argv in (("init", ["init", "-q"]), ("add", ["add", "src/main.rs"]),
                                ("commit", ["-c", "user.name=Regression", "-c",
                                 "user.email=regression@example.invalid", "commit", "-qm", "fixture"])):
                gate.command(self.root, repo.name + "-" + label, ["/usr/bin/git", *argv],
                             cwd=repo, env=self.env)
        self.head, _ = gate.command(self.root, "fixture-head",
            ["/usr/bin/git", "rev-parse", "HEAD", "HEAD^{tree}"], cwd=self.repo, env=self.env)
        # Use Git's ownership regression hook at the real subprocess seam. No
        # production environment exception and no Git result is mocked.
        real_run = gate.subprocess.run
        def different_owner(*args, **kwargs):
            kwargs["env"] = dict(kwargs["env"], GIT_TEST_ASSUME_DIFFERENT_OWNER="1")
            return real_run(*args, **kwargs)
        patch = mock.patch.object(gate.subprocess, "run", side_effect=different_owner)
        patch.start()
        self.addCleanup(patch.stop)
        try:
            gate.command(self.root, "unscoped-head", ["/usr/bin/git", "rev-parse", "HEAD"],
                         cwd=self.repo, env=self.env)
        except ValueError:
            receipt = json.loads((self.root / "unscoped-head.json").read_text())
            self.assertEqual(receipt["status"], 128)
            self.assertIn(b"dubious ownership", (self.root / "unscoped-head.stderr").read_bytes())
        else:
            self.skipTest("Git does not support GIT_TEST_ASSUME_DIFFERENT_OWNER")

    def test_exact_repository_trust_is_read_only_and_scoped_to_each_command(self):
        alias = self.root / "checkout-alias"
        alias.symlink_to(self.repo, target_is_directory=True)
        before = {path: path.read_bytes() for repo in (self.repo, self.sibling)
                  for path in (repo / ".git").rglob("*") if path.is_file()}
        before[self.global_config] = self.global_config.read_bytes()
        # None of this inherited Git configuration may redirect the reads or
        # broaden the exact per-command ownership exception.
        hostile_env = dict(self.env, GIT_DIR=str(self.sibling / ".git"),
            GIT_WORK_TREE=str(self.sibling), GIT_CONFIG_COUNT="1",
            GIT_CONFIG_KEY_0="safe.directory", GIT_CONFIG_VALUE_0="*",
            GIT_CONFIG_PARAMETERS="'safe.directory=*'")
        paths = ("src", "native", "Cargo.toml", "Cargo.lock", "build.rs",
                 "tests/fixtures/bounded_enum_scanner")
        commands = (("git-head", ("rev-parse", "HEAD", "HEAD^{tree}"), self.head),
                    ("git-status", ("status", "--porcelain=v1"), b""),
                    ("source-files", ("ls-files", "-z", "--", *paths), b"src/main.rs\0"),
                    ("source-files-after", ("ls-files", "-z", "--", *paths), b"src/main.rs\0"))
        for label, arguments, expected in commands:
            with self.subTest(command=label):
                stdout, stderr = gate.git_command(self.root, label, alias, *arguments, env=hostile_env)
                self.assertEqual((stdout, stderr), (expected, b""))
                self.assertEqual((self.root / (label + ".stdout")).read_bytes(), expected)
                self.assertEqual((self.root / (label + ".stderr")).read_bytes(), b"")
                receipt = json.loads((self.root / (label + ".json")).read_text())
                self.assertEqual(receipt["argv"], ["/usr/bin/git", "-c",
                    "safe.directory=" + str(self.repo), "-C", str(self.repo), *arguments])
                self.assertEqual(receipt["cwd"], str(self.repo))
                self.assertEqual(receipt["status"], 0)
                self.assertEqual(receipt["environment"], {"mode": "isolated-git-read", "values": {
                    "PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"], "LC_ALL": "C",
                    "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                    "GIT_OPTIONAL_LOCKS": "0"}})
        # Reusing the exact checkout exception cannot admit its sibling.
        with self.assertRaisesRegex(ValueError, "status 128"):
            gate.command(self.root, "sibling-denied", ["/usr/bin/git", "-c",
                "safe.directory=" + str(self.repo), "-C", self.sibling, "rev-parse", "HEAD"],
                cwd=self.sibling, env=self.env)
        self.assertIn(b"dubious ownership", (self.root / "sibling-denied.stderr").read_bytes())
        with self.assertRaisesRegex(ValueError, "status 128"):
            gate.command(self.root, "checkout-still-denied", ["/usr/bin/git", "rev-parse", "HEAD"],
                         cwd=self.repo, env=self.env)
        after = {path: path.read_bytes() for repo in (self.repo, self.sibling)
                 for path in (repo / ".git").rglob("*") if path.is_file()}
        after[self.global_config] = self.global_config.read_bytes()
        self.assertEqual(after, before)

    def test_verify_still_rejects_wrong_head_and_dirty_checkout_before_building(self):
        args = SimpleNamespace(repo=self.repo, llvm_bin=self.root, expected_head="0" * 40)
        with mock.patch.dict(os.environ, self.env):
            wrong_head = self.root / "wrong-head"
            wrong_head.mkdir()
            with self.assertRaisesRegex(ValueError, "not the exact CI head"):
                gate.verify(args, wrong_head)
            self.assertFalse((wrong_head / "git-status.json").exists())
            args.expected_head = self.head.decode().splitlines()[0]
            for name, path in (("tracked", self.repo / "src/main.rs"),
                               ("untracked", self.repo / "unexpected")):
                with self.subTest(drift=name):
                    original = path.read_bytes() if path.exists() else None
                    path.write_bytes(b"checkout drift\n")
                    dirty = self.root / ("dirty-" + name)
                    dirty.mkdir()
                    with self.assertRaisesRegex(ValueError, "checkout must be clean"):
                        gate.verify(args, dirty)
                    self.assertEqual((dirty / "git-head.stdout").read_bytes(), self.head)
                    self.assertTrue((dirty / "git-status.stdout").read_bytes())
                    self.assertFalse((dirty / "source-files.json").exists())
                    if original is None:
                        path.unlink()
                    else:
                        path.write_bytes(original)

    def descendant_git_probe(self):
        probe = self.root / "descendant-git-probe"
        probe.write_text(f"#!{sys.executable}\n" + '''import json, os, pathlib, subprocess, sys
repo = pathlib.Path.cwd().resolve()
rows = []
for name, cwd, args in (
    ("head", repo, ["rev-parse", "HEAD"]),
    ("status", repo, ["status", "--porcelain=v1"]),
    ("diff", repo, ["diff", "--no-ext-diff", "--binary", "HEAD"]),
    ("sibling", repo.with_name("sibling"), ["rev-parse", "HEAD"]),
):
    output = subprocess.run(["git", "--no-optional-locks", *args], cwd=cwd,
                            capture_output=True, text=True)
    rows.append({"name": name, "status": output.returncode,
                 "stdout": output.stdout, "stderr": output.stderr})
print(json.dumps({"git": rows, "environment": dict(os.environ)}))
print("real descendant Git probe", file=sys.stderr)
sys.exit(next((row["status"] for row in rows[:-1] if row["status"]), 0))
''')
        probe.chmod(0o755)
        return probe

    def test_source_children_inherit_only_canonical_checkout_trust_and_preserve_native_environment(self):
        alias = self.root / "checkout-alias"
        alias.symlink_to(self.repo, target_is_directory=True)
        probe = self.descendant_git_probe()
        native_env = dict(self.env, OXID_LLVM_BIN="/retained/llvm/bin",
            OXID_OWNED_NATIVE_EVIDENCE=str(self.root / "artifacts"),
            CARGO_HOME="/retained/cargo", RUSTUP_HOME="/retained/rustup",
            LD_LIBRARY_PATH="/retained/native-libraries")
        hostile_env = dict(native_env, GIT_DIR=str(self.sibling / ".git"),
            GIT_WORK_TREE=str(self.sibling), GIT_COMMON_DIR=str(self.sibling / ".git"),
            GIT_CONFIG_SYSTEM=str(self.global_config), GIT_CONFIG_COUNT="2",
            GIT_CONFIG_KEY_0="safe.directory", GIT_CONFIG_VALUE_0="*",
            GIT_CONFIG_KEY_1="core.bare", GIT_CONFIG_VALUE_1="true",
            GIT_CONFIG_PARAMETERS="'safe.directory=*'", GIT_TEST_ASSUME_DIFFERENT_OWNER="0")
        original_env = dict(hostile_env)
        expected_git = {"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "safe.directory",
            "GIT_CONFIG_VALUE_0": str(self.repo), "GIT_OPTIONAL_LOCKS": "0"}
        before = {path: path.read_bytes() for repo in (self.repo, self.sibling)
                  for path in (repo / ".git").rglob("*") if path.is_file()}
        before[self.global_config] = self.global_config.read_bytes()
        supplied = gate.source_test_environment(alias, hostile_env)
        self.assertEqual({key: value for key, value in supplied.items()
                          if key.startswith("GIT_")}, expected_git)
        for index, name in enumerate(gate.SOURCE_NAMES):
            with self.subTest(name=name):
                evidence = self.root / f"source-child-{index}"
                evidence.mkdir()
                stdout, stderr = gate.native_test_command(evidence, probe, name,
                    repo=alias, env=hostile_env)
                self.assertEqual((evidence / "execution.stdout").read_bytes(), stdout)
                self.assertEqual((evidence / "execution.stderr").read_bytes(), stderr)
                self.assertEqual(stderr, b"real descendant Git probe\n")
                observed = json.loads(stdout)
                self.assertEqual(observed["git"][:3], [
                    {"name": "head", "status": 0, "stdout": self.head.decode().splitlines()[0] + "\n",
                     "stderr": ""},
                    {"name": "status", "status": 0, "stdout": "", "stderr": ""},
                    {"name": "diff", "status": 0, "stdout": "", "stderr": ""}])
                self.assertEqual(observed["git"][3]["status"], 128)
                self.assertIn("dubious ownership", observed["git"][3]["stderr"])
                for key, value in native_env.items():
                    if not key.startswith("GIT_"):
                        self.assertEqual(observed["environment"][key], value)
                self.assertEqual({key: value for key, value in observed["environment"].items()
                    if key.startswith("GIT_")},
                    dict(expected_git, GIT_TEST_ASSUME_DIFFERENT_OWNER="1"))
                receipt = json.loads((evidence / "execution.json").read_text())
                self.assertEqual(receipt["status"], 0)
                self.assertEqual(receipt["argv"], [str(probe), name, "--exact", "--ignored",
                    "--nocapture", "--test-threads=1", "--color=never"])
                self.assertEqual(receipt["environment"], {"mode": "source-test-scoped-git",
                    "git_environment": expected_git,
                    "OXID_LLVM_BIN": native_env["OXID_LLVM_BIN"],
                    "OXID_OWNED_NATIVE_EVIDENCE": native_env["OXID_OWNED_NATIVE_EVIDENCE"]})
        self.assertEqual(hostile_env, original_env)
        with self.assertRaisesRegex(ValueError, "status 128"):
            gate.command(self.root, "descendant-trust-not-persistent",
                ["git", "rev-parse", "HEAD"], cwd=self.repo, env=self.env)
        after = {path: path.read_bytes() for repo in (self.repo, self.sibling)
                 for path in (repo / ".git").rglob("*") if path.is_file()}
        after[self.global_config] = self.global_config.read_bytes()
        self.assertEqual(after, before)

    def test_all_six_raw_children_keep_their_original_environment_without_added_trust(self):
        probe = self.descendant_git_probe()
        for index, name in enumerate(gate.RAW_NAMES):
            with self.subTest(name=name):
                evidence = self.root / f"raw-child-{index}"
                evidence.mkdir()
                with self.assertRaisesRegex(ValueError, "status 128"):
                    gate.native_test_command(evidence, probe, name, repo=self.repo, env=self.env)
                observed = json.loads((evidence / "execution.stdout").read_bytes())
                self.assertEqual(observed["git"][0]["status"], 128)
                self.assertIn("dubious ownership", observed["git"][0]["stderr"])
                for key, value in self.env.items():
                    self.assertEqual(observed["environment"][key], value)
                self.assertNotIn("GIT_CONFIG_COUNT", observed["environment"])
                receipt = json.loads((evidence / "execution.json").read_text())
                self.assertEqual(receipt["status"], 128)
                self.assertEqual(receipt["environment"]["mode"], "inherited")
                self.assertNotIn("git_environment", receipt["environment"])


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
        binding_root = original.parent
        spec = importlib.util.spec_from_file_location("native_byte_source_authority", binding_root / "byte_storage.py")
        byte = importlib.util.module_from_spec(spec); spec.loader.exec_module(byte)
        self.assertEqual(len(manifest["files"]), byte.CURRENT_MEMBERS)
        self.assertEqual(hashlib.sha256(original.read_bytes()).hexdigest(), byte.SOURCE_SHA)
        reduced = dict(manifest, files=[row for row in manifest["files"]
            if row["path"].startswith(("src/", "native/", "tests/fixtures/bounded_enum_scanner/"))
            or row["path"] in ("Cargo.toml", "Cargo.lock", "build.rs")])
        self.assertEqual(len(reduced["files"]), byte.COMPILER_MEMBERS + 5)
        self.assertEqual(reduced["reviewed_source_head"], byte.SOURCE_HEAD)
        previous_raw = original.with_name("u8-source.json").read_bytes()
        self.assertEqual(hashlib.sha256(previous_raw).hexdigest(), manifest["u8_source_sha256"])
        previous = json.loads(previous_raw)
        self.assertEqual(previous["reviewed_source_head"], "5e4875d19961b4eba8e465c915ac676c54a9926e")
        before = {row["path"]: row for row in previous["files"]}
        cross_raw = original.with_name("u8-cross-host-source.json").read_bytes()
        self.assertEqual(hashlib.sha256(cross_raw).hexdigest(), byte.PREDECESSOR_SHA)
        cross = json.loads(cross_raw)
        after = {row["path"]: row for row in cross["files"]}
        current = {row["path"]: row for row in manifest["files"]}
        self.assertEqual(set(current) - set(after), set(byte.ADDITIONS) | set(byte.FIXTURE_ADDITIONS))
        self.assertEqual([row["path"] for row in manifest["files"] if after.get(row["path"]) != row], list(byte.PATHS))
        self.assertEqual(set(before), set(after))
        self.assertEqual({name for name in before if before[name] != after[name]},
                         {"src/frontend/project.rs", "src/frontend/declaration_index/u8_integration_tests.rs"})
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

    def test_native_entrypoints_and_workflow_share_the_exact_current_source_pin(self):
        import verify_bounded_stdin_native as stdin_gate
        import verify_bounded_stdout_native as stdout_gate
        expected = '402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa'
        self.assertEqual((gate.REVIEWED_SOURCE_SHA256, stdin_gate.REVIEWED_SOURCE_SHA256,
                          stdout_gate.REVIEWED_SOURCE_SHA256), (expected, expected, expected))
        repo = Path(__file__).resolve().parents[1]
        workflow = (repo / ".github/workflows/ci.yml").read_text()
        self.assertEqual(re.findall(r"--source-manifest-sha256 ([0-9a-f]{64})", workflow),
                         [expected, expected])
        for script in ("verify_bounded_stdin_native.py", "verify_bounded_stdout_native.py"):
            invocation = workflow.split("python3 -B scripts/" + script, 1)[1].split("\n      - name:", 1)[0]
            self.assertEqual(invocation.count("--source-manifest-sha256 " + expected), 1)

    def test_stale_predecessor_and_rehashed_same_count_manifest_remain_unapproved(self):
        repo = Path(__file__).resolve().parents[1]
        package = repo / "tests/fixtures/typed_project_source_binding"
        current = gate.read_reviewed_manifest(package / "current-source.json")
        stale = (package / "producer-diagnostic-source.json").read_bytes()
        self.assertEqual(hashlib.sha256(stale).hexdigest(),
                         "35e7e43cb1ef5de8be0c1a78d9e5ac70b1a2caf2efe1e37ba05b7445916c2e29")
        lexical = (package / "lexical-provider-source.json").read_bytes()
        self.assertEqual(hashlib.sha256(lexical).hexdigest(),
                         "952c7cf86d2be1036781155d38f81af8854c0fb26487c4dfa1dc24bc575309db")
        integrity = (package / "package-integrity-source.json").read_bytes()
        self.assertEqual(hashlib.sha256(integrity).hexdigest(),
                         "4d114bbb9b375de743bc18508ebcb48301604f5b417ca0b44d788bf22189c99f")
        forged_head = dict(current, reviewed_source_head="c15465acb90e9f8bb18f5291a8931f5d5bbc6edb")
        forged_rows = dict(current, files=[dict(row) for row in current["files"]])
        forged_rows["files"][0]["sha256"] = "0" * 64
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / "source.json"
            for label, raw in (("stale predecessor", stale),
                               ("retained lexical predecessor", lexical),
                               ("retained package predecessor", integrity),
                               ("same-count wrong head", (json.dumps(forged_head, sort_keys=True, indent=2) + "\n").encode()),
                               ("same-count wrong input", (json.dumps(forged_rows, sort_keys=True, indent=2) + "\n").encode())):
                candidate.write_bytes(raw)
                receipt = gate.identity(candidate)
                self.assertEqual(receipt["sha256"], hashlib.sha256(raw).hexdigest())
                self.assertNotEqual(receipt["sha256"], gate.REVIEWED_SOURCE_SHA256)
                with self.subTest(label=label), self.assertRaisesRegex(ValueError, "approved authority"):
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
