"""Synthetic controls only: these fixtures never qualify a Rust/LLVM run."""
import json
import os
from pathlib import Path
import struct
import signal
import subprocess
import sys
import time
from unittest import mock
import tempfile
import unittest

import verify_owned_array_native as admission


def stdout_fixture():
    rows = ["", "running 16 tests"]
    for name in admission.ROSTER:
        if name == admission.MULTILINE_NAME:
            rows.extend(["test " + name + " ... " + admission.MULTILINE_PAYLOADS[0],
                         admission.MULTILINE_PAYLOADS[1], "ok"])
        else:
            rows.append("test " + name + " ... ok")
    rows.extend(["", "test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 312 filtered out; finished in 0.03s", ""])
    return "\n".join(rows).encode()


def elf_fixture():
    data = bytearray(64)
    data[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HHI", data, 16, 2, 62, 1)
    struct.pack_into("<H", data, 52, 64)
    return bytes(data)


class AdmissionTests(unittest.TestCase):
    def test_exact_source_bound_member_count(self):
        members = admission.expected_array_members()
        self.assertEqual(len(members), 1307)
        self.assertEqual(sum(name.endswith(".elf") for name in members), 524)
        self.assertEqual(sum(name.endswith("-guarded-production.ll") for name in members), 259)
        self.assertEqual(sum(row["elf_processes"] for row in admission.FAMILIES), 7058)
        self.assertEqual(sum(row["reference_comparisons_derived"] for row in admission.FAMILIES), 6793)

    def test_exact_list_and_roster_rejections(self):
        names = admission.ROSTER
        def listing(values):
            return ("\n".join(name + ": test" for name in values) + "\n\n16 tests, 0 benchmarks\n").encode()
        self.assertEqual(admission.admit_list(listing(names)), list(names))
        for values in ((), names[:-1], names + (names[0],), names + ("unexpected",), names[:5]):
            with self.subTest(values=len(values)), self.assertRaises(admission.AdmissionError):
                admission.admit_list(listing(values))
        with self.assertRaises(admission.AdmissionError):
            admission.admit_list(b"")

    def test_full_stdout_exact_multiline_attribution_and_byte_spans(self):
        data = stdout_fixture()
        rows = admission.admit_stdout(data)
        self.assertEqual(len(rows), 16)
        for row in rows:
            span = data[row["byte_start"]:row["byte_end"]].decode()
            self.assertTrue(span.startswith("test " + row["name"] + " ... "))
            self.assertTrue(span.endswith("ok\n"))
        multiline = next(row for row in rows if row["name"] == admission.MULTILINE_NAME)
        self.assertEqual(multiline["line_end"] - multiline["line_start"], 2)

    def test_stdout_rejects_unknown_missing_duplicate_failed_and_ignored(self):
        data = stdout_fixture()
        row = ("test " + admission.ROSTER[0] + " ... ok\n").encode()
        for changed in (data.replace(row, b""), data.replace(row, row * 2),
                        data.replace(row, row.replace(b"ok\n", b"ignored\n")),
                        data.replace(row, row.replace(b"ok\n", b"FAILED\n")),
                        data.replace(row, b"test unknown ... ok\n"),
                        data.replace(row, b"unknown stdout\n" + row),
                        data.replace(b"running 16 tests", b"running 0 tests"),
                        data[:data.index(row)] + data[data.index(b"test result:"):]):
            with self.subTest(changed=changed), self.assertRaises(admission.AdmissionError):
                admission.admit_stdout(changed)

    def test_multiline_rejects_each_truncation_and_attribution_error(self):
        data = stdout_fixture()
        first = ("test " + admission.MULTILINE_NAME + " ... " + admission.MULTILINE_PAYLOADS[0]).encode()
        second = admission.MULTILINE_PAYLOADS[1].encode()
        block = first + b"\n" + second + b"\nok\n"
        other = ("test " + admission.ROSTER[0] + " ... ok\n").encode()
        footer = data[data.index(b"test result:"):]
        mutations = [data.replace(first, first[:-1]), data.replace(second, second[:-1]),
                     data.replace(block, first + b"\n" + second + b"\n"),
                     data.replace(first, first.replace(admission.MULTILINE_NAME.encode(), admission.ROSTER[0].encode())),
                     data.replace(block, first + b"\n" + other + second + b"\nok\n"),
                     data.replace(second, second + b"\n" + second),
                     data.replace(block, block * 2), data.replace(block, block + b"ok\n"),
                     data + b"ok\n", data.replace(second, footer + second),
                     data.replace(block, ("test " + admission.MULTILINE_NAME + " ... ok\n").encode())]
        for index, changed in enumerate(mutations):
            with self.subTest(index=index), self.assertRaises(admission.AdmissionError):
                admission.admit_stdout(changed)

    def test_stderr_family_summaries_are_exact_and_separate(self):
        data = ("inherited diagnostic\n" + "\n".join(admission.family_summary(row) for row in admission.FAMILIES) + "\n").encode()
        rows = admission.admit_stderr(data)
        self.assertEqual(len(rows), 6)
        self.assertTrue(all("derived" in row["reference_count_kind"] for row in rows))
        for changed in (data + admission.family_summary(admission.FAMILIES[0]).encode(),
                        data.replace(b"5037 ELF", b"5036 ELF"), b"", stdout_fixture()):
            with self.assertRaises(admission.AdmissionError):
                admission.admit_stderr(changed)

    def test_duplicate_json_keys_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "receipt.json"
            path.write_text('{"status": "FAIL", "status": "PASS"}')
            with self.assertRaises(admission.AdmissionError):
                admission.read_json(path)


class ArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "artifacts"
        self.root.mkdir()
        self.expected = {"array-core-0-acyclic.elf": {"family": "core", "role": "compiled-elf"},
                         "array-core-0-acyclic.ll": {"family": "core", "role": "compiled-ll"}}
        self.elf = self.root / "array-core-0-acyclic.elf"
        self.elf.write_bytes(elf_fixture())
        self.elf.chmod(0o755)
        self.llvm = self.root / "array-core-0-acyclic.ll"
        self.llvm.write_text("; synthetic LLVM member\n")
        (self.root / "inherited.ll").write_text("; retained inherited bytes\n")

    def test_positive_retains_inherited_members(self):
        rows = admission.admit_artifacts(self.root, self.expected)
        self.assertEqual(len(rows), 3)
        self.assertEqual(rows["inherited.ll"]["role"], "inherited")
        self.assertEqual(rows[self.elf.name]["elf"]["machine"], "x86_64")

    def test_missing_and_extra_and_nested_array_members_rejected(self):
        original = self.elf.read_bytes()
        self.elf.unlink()
        with self.assertRaises(admission.AdmissionError):
            admission.admit_artifacts(self.root, self.expected)
        self.elf.write_bytes(original)
        self.elf.chmod(0o755)
        extra = self.root / "array-unexpected.ll"
        extra.write_text("; extra")
        with self.assertRaises(admission.AdmissionError):
            admission.admit_artifacts(self.root, self.expected)
        extra.unlink()
        nested = self.root / "nested"
        nested.mkdir()
        self.elf.rename(nested / self.elf.name)
        with self.assertRaises(admission.AdmissionError):
            admission.admit_artifacts(self.root, self.expected)

    def test_elf_header_and_executable_mode_rejections(self):
        for data in (b"\x7fELF", elf_fixture().replace(b"\x7fELF", b"NOPE"),
                     elf_fixture()[:18] + b"\x03\x00" + elf_fixture()[20:]):
            self.elf.write_bytes(data)
            with self.assertRaises(admission.AdmissionError):
                admission.admit_artifacts(self.root, self.expected)
        self.elf.write_bytes(elf_fixture())
        self.elf.chmod(0o644)
        with self.assertRaises(admission.AdmissionError):
            admission.admit_artifacts(self.root, self.expected)

    def test_empty_or_invalid_utf8_module_rejected(self):
        for data in (b"", b"  \n", b"\xff"):
            self.llvm.write_bytes(data)
            with self.assertRaises((admission.AdmissionError, UnicodeError)):
                admission.admit_artifacts(self.root, self.expected)

    def test_symlink_member_ancestor_and_directory_rejected(self):
        target = Path(self.temporary.name) / "saved-elf"
        self.elf.rename(target)
        self.elf.symlink_to(target)
        with self.assertRaises(admission.AdmissionError):
            admission.admit_artifacts(self.root, self.expected)
        self.elf.unlink()
        self.elf.mkdir()
        with self.assertRaises(admission.AdmissionError):
            admission.admit_artifacts(self.root, self.expected)
        alias = Path(self.temporary.name) / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises((admission.AdmissionError, OSError)):
            admission.stable_bytes(alias / self.llvm.name)


    def test_traversal_io_error_cannot_drop_inherited_evidence(self):
        inherited = self.root / "inherited"
        inherited.mkdir()
        (inherited / "original.ll").write_text("; preserve this original")
        original_scandir = os.scandir
        def failing_scandir(path):
            if Path(path) == inherited:
                raise PermissionError("synthetic unreadable inherited directory")
            return original_scandir(path)
        with mock.patch.object(os, "scandir", side_effect=failing_scandir):
            with self.assertRaises(PermissionError):
                admission.regular_inventory(self.root)
            with self.assertRaises(PermissionError):
                admission.admit_artifacts(self.root, self.expected)

    def test_hashes_detect_same_size_member_and_log_changes(self):
        before = admission.regular_inventory(self.root)
        self.llvm.write_text("; synthetiX LLVM member\n")
        self.assertNotEqual(before, admission.regular_inventory(self.root))
        before = admission.file_record(self.elf)
        self.elf.write_bytes(elf_fixture()[:-1] + b"\x01")
        self.assertNotEqual(before, admission.file_record(self.elf))


class ProcessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.environment = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"]}

    def run_python(self, script, name="child", **kwargs):
        return admission.run_child([sys.executable, "-c", script], self.root / name,
                                   cwd=self.root, environment=self.environment, **kwargs)

    def test_original_exit7_and_full_logs_survive(self):
        with self.assertRaises(admission.ChildFailure) as failure:
            self.run_python("import sys; print('test result: ok. 16 passed'); print('sentinel', file=sys.stderr); sys.exit(7)")
        self.assertEqual(failure.exception.code, 7)
        row = admission.read_json(self.root / "child/command.json")
        self.assertEqual(row["exit_code"], 7)
        self.assertEqual(row["status"], "FAIL")
        self.assertEqual((self.root / "child/stderr").read_bytes(), b"sentinel\n")
        self.assertEqual(row["stdout"], admission.file_record(self.root / "child/stdout"))
        self.assertTrue(row["cleanup"]["stopped"])

    def test_zero_selection_real_child_is_not_admitted(self):
        row = self.run_python("print('running 0 tests\\n\\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.0s')")
        self.assertEqual(row["exit_code"], 0)
        with self.assertRaises(admission.AdmissionError):
            admission.admit_stdout((self.root / "child/stdout").read_bytes())

    def test_launch_signal_and_timeout_preserved(self):
        with self.assertRaises(admission.ChildFailure):
            admission.run_child([str(self.root / "missing")], self.root / "launch",
                                cwd=self.root, environment=self.environment)
        self.assertIsNone(admission.read_json(self.root / "launch/command.json")["exit_code"])
        with self.assertRaises(admission.ChildFailure) as signal_failure:
            self.run_python("import os,signal; os.kill(os.getpid(), signal.SIGTERM)", "signal")
        self.assertEqual(signal_failure.exception.code, 143)
        self.assertEqual(admission.read_json(self.root / "signal/command.json")["exit_code"], -signal.SIGTERM)
        with self.assertRaises(admission.ChildFailure) as timeout:
            self.run_python("import time; time.sleep(60)", "timeout", timeout=0.05)
        self.assertEqual(timeout.exception.code, 124)
        row = admission.read_json(self.root / "timeout/command.json")
        self.assertEqual(row["status"], "TIMEOUT")
        self.assertTrue(row["cleanup"]["stopped"])

    def descendant_script(self, exit_code):
        writer = ("import pathlib,time; p=pathlib.Path('ready'); p.write_text('ready'); "
                  "exec(\"while True:\\n print('writing',flush=True); open('artifact-marker','a').write('x'); time.sleep(.01)\")")
        return ("import pathlib,subprocess,sys,time; "
                "subprocess.Popen([sys.executable,'-c'," + repr(writer) + "]); "
                "exec(\"while not pathlib.Path('ready').exists(): time.sleep(.001)\"); "
                f"sys.exit({exit_code})")

    def test_surviving_descendants_after_exit0_and_exit7_are_reaped_before_seal(self):
        for code in (0, 7):
            name = "exit" + str(code)
            with self.subTest(code=code), self.assertRaises(admission.ChildFailure) as failure:
                self.run_python(self.descendant_script(code), name)
            self.assertEqual(failure.exception.code, code or 1)
            row = admission.read_json(self.root / name / "command.json")
            self.assertEqual(row["exit_code"], code)
            self.assertTrue(row["cleanup"]["surviving_group_detected"])
            self.assertTrue(row["cleanup"]["stopped"])
            before = (self.root / "artifact-marker").read_bytes()
            stdout = (self.root / name / "stdout").read_bytes()
            time.sleep(.06)
            self.assertEqual(before, (self.root / "artifact-marker").read_bytes())
            self.assertEqual(stdout, (self.root / name / "stdout").read_bytes())
            self.assertEqual(row["stdout"]["sha256"], admission.sha256(stdout))
            (self.root / "ready").unlink()

    def test_exit7_is_not_masked_by_receipt_write_failure(self):
        with mock.patch.object(admission, "write_json", side_effect=OSError("synthetic sealing failure")):
            with self.assertRaises(admission.ChildFailure) as failure:
                self.run_python("import sys; sys.exit(7)")
        self.assertEqual(failure.exception.code, 7)
        self.assertIn("sealing", str(failure.exception))


    def test_exception_after_launch_still_reaps_owned_group(self):
        def injected(process):
            deadline = time.monotonic() + 3
            while not (self.root / "ready").exists() and time.monotonic() < deadline:
                time.sleep(.005)
            raise RuntimeError("synthetic exception")
        with self.assertRaises(admission.ChildFailure):
            self.run_python(self.descendant_script(0), _after_launch=injected)
        row = admission.read_json(self.root / "child/command.json")
        self.assertIn("synthetic exception", row["error"])
        self.assertTrue(row["cleanup"]["stopped"])
        before = (self.root / "artifact-marker").read_bytes()
        time.sleep(.06)
        self.assertEqual(before, (self.root / "artifact-marker").read_bytes())


class EnvironmentTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.selected = self.root / "selected"
        self.trap = self.root / "trap"
        self.other = self.root / "other"
        for directory in (self.selected, self.trap, self.other):
            directory.mkdir()
            for name in admission.LLVM_TOOLS:
                suffix = "-19" if directory == self.trap else ""
                path = directory / (name + suffix)
                path.write_text("#!" + sys.executable + "\nimport os,pathlib\n"
                                + "pathlib.Path(" + repr(str(self.root / (directory.name + '-marker'))) + ").write_text(os.environ.get('OXID_LLVM_BIN','unset'))\nprint('LLVM version 19.1.7')\n")
                path.chmod(0o755)
        self.selection = {"llvm_directory": str(self.selected),
                          "tools": {name: admission.tool_identity(self.selected / name) for name in admission.LLVM_TOOLS}}
        self.environment = admission.child_environment(self.selection, cargo_home=self.root / "cargo-home",
            home=os.environ["HOME"], rustc=sys.executable, cargo=sys.executable,
            cc=sys.executable, cxx=sys.executable, ar=sys.executable, target=self.root / "target", temporary=self.root)
        self.environment["PATH"] = str(self.trap) + ":" + self.environment["PATH"]

    def test_constructed_environment_binds_exact_selected_tools_and_home(self):
        self.assertEqual(self.environment["HOME"], os.environ["HOME"])
        self.assertTrue(all(name not in self.environment for name in admission.UNSET_CONTROLS))
        for index, name in enumerate(admission.LLVM_TOOLS):
            admission.run_child([self.selection["tools"][name]["path"], "--version"], self.root / f"version-{index}",
                                cwd=self.root, environment=self.environment, selection=self.selection)
        self.assertEqual((self.root / "selected-marker").read_text(), str(self.selected))
        self.assertFalse((self.root / "trap-marker").exists())

    def test_missing_empty_relative_wrong_directory_rejected_before_any_tool(self):
        for index, value in enumerate((None, "", "selected", str(self.other))):
            environment = dict(self.environment)
            if value is None:
                environment.pop("OXID_LLVM_BIN")
            else:
                environment["OXID_LLVM_BIN"] = value
            with self.subTest(value=value), self.assertRaises(admission.ChildFailure):
                admission.run_child([str(self.selected / "llvm-as")], self.root / f"wrong-{index}",
                                    cwd=self.root, environment=environment, selection=self.selection)
        self.assertFalse((self.root / "selected-marker").exists())
        self.assertFalse((self.root / "other-marker").exists())
        self.assertFalse((self.root / "trap-marker").exists())

    def test_negative_fixture_demonstrates_same_version_rust_fallback_trap(self):
        # Reproduce unchanged Rust fallback outside operational admission.
        environment = dict(self.environment)
        environment.pop("OXID_LLVM_BIN")
        script = "import os,subprocess; directory=os.environ.get('OXID_LLVM_BIN'); subprocess.run([directory+'/llvm-as' if directory else 'llvm-as-19','--version'],check=True)"
        subprocess.run([sys.executable, "-c", script], cwd=self.root, env=environment, check=True, capture_output=True)
        self.assertEqual((self.root / "trap-marker").read_text(), "unset")
        self.assertFalse((self.root / "selected-marker").exists())

    def test_changed_tool_and_loader_injection_rejected(self):
        changed = self.selected / "llvm-as"
        changed.write_text(changed.read_text() + "# changed\n")
        with self.assertRaises(admission.AdmissionError):
            admission.validate_environment(self.environment, self.selection)
        for variable in ("LD_LIBRARY_PATH", "RUSTFLAGS", "CLANG_CONFIG_FILE_ANYTHING"):
            with self.subTest(variable=variable), self.assertRaises(admission.AdmissionError):
                admission.validate_environment({**self.environment, variable: "injected"}, self.selection)

    def test_cargo_binary_cannot_cross_profile_directory(self):
        debug = self.root / "target/debug"
        debug.mkdir(parents=True)
        binary = debug / "oxid-test"
        binary.write_bytes(elf_fixture())
        binary.chmod(0o755)
        data = (json.dumps({"reason": "compiler-artifact", "target": {"name": "oxid"},
                          "profile": {"test": True}, "executable": str(binary)}) + "\n").encode()
        self.assertEqual(admission.built_test_binary(data, debug)[0], binary)
        with self.assertRaises(admission.AdmissionError):
            admission.built_test_binary(data, self.root / "target/release")
        for changed in (b"", data + data):
            with self.assertRaises(admission.AdmissionError):
                admission.built_test_binary(changed, debug)


    def test_exact_rust_and_llvm_version_pins(self):
        data = (f"rustc {admission.RUST_RELEASE}\nrelease: {admission.RUST_RELEASE}\n"
                f"commit-hash: {admission.RUST_COMMIT}\nhost: {admission.RUST_HOST}\n").encode()
        admission.validate_rust_version(data)
        for old in (admission.RUST_RELEASE, admission.RUST_COMMIT, admission.RUST_HOST):
            with self.assertRaises(admission.AdmissionError):
                admission.validate_rust_version(data.replace(old.encode(), b"wrong"))
        admission.validate_llvm_version(b"Debian LLVM version 19.1.7\n")
        admission.validate_llvm_version(b"Debian clang version 19.1.7 (Debian)\n")
        admission.validate_llvm_version(b"Debian LLD 19.1.7 (compatible with GNU linkers)\n")
        with self.assertRaises(admission.AdmissionError):
            admission.validate_llvm_version(b"LLVM version 19.1.8\n")

    def test_config_metadata_only_and_credentials_never_opened(self):
        repo = self.root / "repo"
        cargo = self.root / "cargo"
        repo.mkdir()
        cargo.mkdir()
        (cargo / "credentials.toml").write_text("secret credential sentinel")
        baseline = admission.check_cargo_config(repo, cargo, set())
        self.assertTrue(all(not row["present"] for row in baseline))
        for path in (cargo / "config.toml", repo / ".cargo/config", self.root / ".cargo/config"):
            path.parent.mkdir(exist_ok=True)
            path.write_text("secret unsupported configuration")
            with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("must not read config contents")):
                with self.assertRaises(admission.AdmissionError) as failure:
                    admission.check_cargo_config(repo, cargo, set())
            self.assertNotIn("secret", str(failure.exception))
            path.unlink()
        config = repo / ".cargo/config.toml"
        config.write_text("[build]\njobs = 2\n")
        self.assertTrue(any(row["present"] for row in admission.check_cargo_config(repo, cargo, {".cargo/config.toml"})))
        config.unlink()
        config.symlink_to(cargo / "credentials.toml")
        with self.assertRaises(admission.AdmissionError):
            admission.check_cargo_config(repo, cargo, {".cargo/config.toml"})


class PackageTests(unittest.TestCase):
    """Constructed receipt fixtures, never an operational qualification run."""
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / "producer"
        self.root.mkdir()
        self.ci = admission.ci_identity("1" * 40, "2" * 40)

    def fixture(self):
        repo = self.base / "repo"
        repo.mkdir()
        required = ("Cargo.toml", "Cargo.lock", "build.rs", ".github/workflows/ci.yml",
                    "scripts/verify_owned_array_native.py", "scripts/test_owned_array_native.py",
                    "scripts/verify_owned_source_native.py", "scripts/verify_owned_source.py",
                    "scripts/preserve_unit3_ci_evidence.py", "docs/architecture/fixed-array-unit2e-native-ci.md")
        for name in required:
            path = repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("synthetic receipt fixture, not executed\n")
        binding_root = repo / admission.BINDING
        binding_root.mkdir(parents=True)
        admission.write_json(binding_root / "current-source.json", {"files": [], "reviewed_source_head": "3" * 40, "source_only_tree": "4" * 40})
        admission.write_json(binding_root / "authority.json", {"repository_inputs": []})
        admission.write_json(binding_root / "package-manifest.json", {"synthetic": True})
        environment = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"], "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1"}
        for arguments in (["init", "-q"], ["add", "."], ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "Synthetic fixture"]):
            subprocess.run(["/usr/bin/git", "-C", str(repo), *arguments], env=environment, check=True, capture_output=True)
        head = subprocess.check_output(["/usr/bin/git", "-C", str(repo), "rev-parse", "HEAD"], env=environment, text=True).strip()
        identity, tracked = admission.source_identity(repo, head)
        inventory = admission.capture_sources(repo, tracked)
        cargo_home = self.base / "cargo-home"
        cargo_home.mkdir()
        source = {**identity, **inventory, "cargo_config": admission.check_cargo_config(repo, cargo_home, tracked)}
        (self.root / "source/inputs").mkdir(parents=True)
        for name in inventory["files"]:
            destination = self.root / "source/inputs" / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((repo / name).read_bytes())
        admission.write_json(self.root / "source/identity.json", source)
        selected = self.base / "tools"
        selected.mkdir()
        tools = {}
        for name in (*admission.LLVM_TOOLS, "cargo", "rustc", "python", "cc", "cxx", "ar"):
            path = selected / name
            path.write_text("#!/bin/sh\n# Synthetic identity only, never invoked\nexit 99\n")
            path.chmod(0o755)
            tools[name] = admission.tool_identity(path)
        selection = {"llvm_directory": str(selected), "tools": {name: tools[name] for name in admission.LLVM_TOOLS}}
        environment = admission.child_environment(selection, cargo_home=cargo_home, home=os.environ["HOME"],
            cargo=tools["cargo"]["path"], rustc=tools["rustc"]["path"], cc=tools["cc"]["path"],
            cxx=tools["cxx"]["path"], ar=tools["ar"]["path"], target=repo / "target", temporary=self.base)
        self.ci = admission.ci_identity(head, "2" * 40)
        invocation = {"schema": "oxid-unit2e-producer-v1", "invocation_id": "synthetic-only",
                      "repository_path": str(repo), "output_root": str(self.root), "profiles": list(admission.PROFILES), "ci": self.ci}
        admission.write_json(self.root / "invocation.json", invocation)
        (self.root / "source-preflight").mkdir()
        admission.write_json(self.root / "source-preflight/prepared.json", {"status": "verified-inputs", "semantic_pass": False,
                             "current_source_sha256": inventory["current_source_sha256"]})
        self.command("preflight-command", [tools["python"]["path"], "-B", str(repo / admission.BINDING / "run.py"),
                     "preflight", "--repo", str(repo), "--output", str(self.root / "source-preflight")], environment, b"synthetic fixture\n")
        versions = {}
        for name in tools:
            version = b"LLVM version 19.1.7\n" if name in admission.LLVM_TOOLS else b"synthetic version\n"
            if name == "rustc":
                version = (f"release: {admission.RUST_RELEASE}\ncommit-hash: {admission.RUST_COMMIT}\nhost: {admission.RUST_HOST}\n").encode()
            self.command("tools/" + name, [tools[name]["path"], "--version"] + (["--verbose"] if name == "rustc" else []), environment, version)
            versions[name] = admission.file_record(self.root / "tools" / name / "command.json")
        stdlib = self.base / "libstd-synthetic.rlib"
        stdlib.write_bytes(b"synthetic standard library identity")
        self.command("tools/target-libdir", [tools["rustc"]["path"], "--print", "target-libdir"], environment, (str(self.base) + "\n").encode())
        toolchains = {**selection, "all_tools": tools, "version_receipts": versions,
                      "stdlib": [{"path": str(stdlib), **admission.file_record(stdlib)}],
                      "effective_environment": environment, "unset_controls": list(admission.UNSET_CONTROLS)}
        admission.write_json(self.root / "toolchains.json", toolchains)
        binding = {**identity, "invocation_id": invocation["invocation_id"],
                   "source_identity_sha256": admission.file_record(self.root / "source/identity.json")["sha256"],
                   "current_source_sha256": inventory["current_source_sha256"],
                   "toolchains_sha256": admission.file_record(self.root / "toolchains.json")["sha256"]}
        terminal = {"status": "PASS", "exit_code": 0, "producer_only": True, "invocation_id": invocation["invocation_id"],
                    "ci": self.ci, "binding": binding, "profiles": {}}
        for profile in admission.PROFILES:
            root = self.root / profile
            (root / "bin").mkdir(parents=True)
            (root / "artifacts").mkdir()
            binary = root / "bin/oxid-test"
            binary.write_bytes(elf_fixture())
            binary.chmod(0o755)
            binary_record = admission.file_record(binary)
            built_path = repo / "target" / profile / "oxid-test"
            child_env = {**environment, "OXID_OWNED_NATIVE_EVIDENCE": str(root / "artifacts")}
            build_argv = [tools["cargo"]["path"], "test", "--locked", "--offline", "--bin", "oxid", "--no-run", "--message-format=json", "--jobs", "2"]
            if profile == "release":
                build_argv.append("--release")
            build_stdout = json.dumps({"reason": "compiler-artifact", "target": {"name": "oxid"}, "profile": {"test": True}, "executable": str(built_path)}).encode() + b"\n"
            self.command(profile + "/build", build_argv, child_env, build_stdout)
            listing = ("\n".join(name + ": test" for name in admission.ROSTER) + "\n\n16 tests, 0 benchmarks\n").encode()
            self.command(profile + "/list", [str(binary), admission.PREFIX, "--list", "--ignored", "--format", "terse", "--color", "never"], child_env, listing)
            stderr = ("\n".join(admission.family_summary(row) for row in admission.FAMILIES) + "\n").encode()
            self.command(profile + "/run", [str(binary), admission.PREFIX, "--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"], child_env, stdout_fixture(), stderr)
            admission.write_json(root / "roster.json", {"profile": profile, "binding": binding, "names": list(admission.ROSTER), "test_binary": binary_record})
            for name in admission.expected_array_members():
                path = root / "artifacts" / name
                path.write_bytes(elf_fixture() if name.endswith(".elf") else b"; synthetic LLVM module\n")
                path.chmod(0o755 if name.endswith(".elf") else 0o644)
            artifacts = admission.admit_artifacts(root / "artifacts")
            admission.write_json(root / "artifact-manifest.json", artifacts)
            result = {"status": "PASS", "profile": profile, "binding": binding, "test_binary": binary_record,
                      "build_binary": {"path": str(built_path), **binary_record}, "outcomes": admission.admit_stdout(stdout_fixture()),
                      "families": admission.admit_stderr(stderr), "array_artifact_files": 1307,
                      "elf_executions_asserted": 7058, "reference_comparisons_derived": 6793,
                      "artifact_manifest_sha256": admission.file_record(root / "artifact-manifest.json")["sha256"],
                      "command_receipts": {name: admission.file_record(root / name / "command.json") for name in ("build", "list", "run")}}
            admission.write_json(root / "result.json", result)
            terminal["profiles"][profile] = {"status": "PASS", "result": admission.file_record(root / "result.json")}
        admission.write_json(self.root / "result.json", terminal)
        admission.write_json(self.root / "evidence-manifest.json", admission.regular_inventory(self.root))

    def command(self, name, argv, environment, stdout=b"", stderr=b""):
        root = self.root / name
        root.mkdir(parents=True)
        (root / "stdout").write_bytes(stdout)
        (root / "stderr").write_bytes(stderr)
        admission.write_json(root / "command.json", {"argv": argv, "cwd": str(self.base / "repo"),
            "environment": environment, "unset_controls": list(admission.UNSET_CONTROLS),
            "status": "PASS", "exit_code": 0, "failure_code": 0,
            "cleanup": {"stopped": True, "surviving_group_detected": False},
            "stdout": admission.file_record(root / "stdout"), "stderr": admission.file_record(root / "stderr")})

    def test_full_synthetic_receipt_admission_and_archive_bytes(self):
        self.fixture()
        output = self.base / "upload"
        self.assertEqual(admission.package_evidence(self.root, output, self.ci, "success"), 0)
        index = admission.read_json(output / admission.INDEX_NAME)
        self.assertEqual(index["status"], "COMPLETE")
        self.assertFalse(index["complete_unit2e_qualification"])
        self.assertLess(index["compact"]["bytes"], admission.COMPACT_LIMIT)
        with admission.tarfile.open(output / admission.ARCHIVE_NAME) as archive:
            manifest = json.load(archive.extractfile("membership.json"))
            self.assertEqual(set(archive.getnames()), set(manifest["members"]) | {"membership.json"})
            for name, row in manifest["members"].items():
                self.assertEqual(admission.sha256(archive.extractfile(name).read()), row["sha256"])
        with admission.tarfile.open(output / admission.COMPACT_NAME) as compact:
            for name in ("producer/result.json", "producer/source/identity.json", "producer/toolchains.json",
                         "producer/debug/run/stdout", "producer/release/run/stderr", "producer/debug/artifact-manifest.json",
                         "producer/release/result.json", "producer/evidence-manifest.json",
                         "producer/debug/build/stdout", "producer/release/build/stderr"):
                self.assertIn(name, compact.getnames())
                self.assertEqual(compact.extractfile(name).read(), (self.root / name.removeprefix("producer/")).read_bytes())

    def test_missing_output_and_failed_skipped_upstream_are_incomplete(self):
        for outcome in ("success", "failure", "skipped", "cancelled"):
            output = self.base / ("upload-" + outcome)
            self.assertEqual(admission.package_evidence(self.base / "missing", output, self.ci, outcome), 1)
            self.assertEqual(admission.read_json(output / admission.INDEX_NAME)["status"], "INCOMPLETE")
            self.assertTrue((output / admission.ARCHIVE_NAME).exists())

    def test_stale_head_event_attempt_profile_binary_and_manifest_rejected(self):
        self.fixture()
        for field in ("expected_head", "event_sha", "run_attempt"):
            with self.subTest(field=field), self.assertRaises(admission.AdmissionError):
                admission.verify_producer(self.root, {**self.ci, field: "stale"}, "success")
        path = self.root / "release/result.json"
        original = path.read_bytes()
        for mutation in ({"profile": "debug"}, {"test_binary": {}}, {"artifact_manifest_sha256": "0" * 64},
                         {"binding": {"current_source_sha256": "0" * 64}}):
            value = json.loads(original)
            value.update(mutation)
            path.write_bytes(admission.json_bytes(value))
            with self.subTest(mutation=mutation), self.assertRaises(admission.AdmissionError):
                admission.verify_producer(self.root, self.ci, "success")
        path.write_bytes(original)
        for outcome in ("failure", "skipped", "cancelled", ""):
            with self.assertRaises(admission.AdmissionError):
                admission.verify_producer(self.root, self.ci, outcome)
        path.unlink()
        self.assertEqual(admission.package_evidence(self.root, self.base / "missing-release", self.ci, "success"), 1)

    def test_untracked_compiler_input_cannot_borrow_committed_source_identity(self):
        self.fixture()
        repository = self.base / "repo"
        (repository / "src").mkdir()
        (repository / "src/untracked.rs").write_text("// uncommitted compiler input")
        with self.assertRaises(admission.AdmissionError) as failure:
            admission.verify_producer(self.root, self.ci, "success")
        self.assertIn("membership differs", str(failure.exception))


    def test_tracked_and_manifest_named_credentials_are_rejected_without_open(self):
        self.fixture()
        repository = self.base / "repo"
        credential = repository / ".cargo/credentials.toml"
        credential.parent.mkdir()
        credential.write_text("synthetic credential sentinel")
        subprocess.run(["/usr/bin/git", "-C", str(repository), "add", ".cargo/credentials.toml"], check=True, capture_output=True)
        subprocess.run(["/usr/bin/git", "-C", str(repository), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                        "commit", "-qm", "Synthetic credential fixture"], check=True, capture_output=True)
        head = subprocess.check_output(["/usr/bin/git", "-C", str(repository), "rev-parse", "HEAD"], text=True).strip()
        original_git = admission.git_bytes
        def guarded_git(repo, *arguments):
            self.assertNotEqual(arguments[0], "status", "credentials must be rejected before Git status")
            return original_git(repo, *arguments)
        original_open = os.open
        def guarded_open(path, *args, **kwargs):
            self.assertNotEqual(Path(path).name, "credentials.toml", "credential contents must not be opened")
            return original_open(path, *args, **kwargs)
        with mock.patch.object(admission, "git_bytes", side_effect=guarded_git), mock.patch.object(os, "open", side_effect=guarded_open):
            with self.assertRaises(admission.AdmissionError) as failure:
                admission.source_identity(repository, head)
        self.assertIn("credentials", str(failure.exception))
        # Even an authority/current-source row cannot authorize credential capture.
        current = repository / admission.BINDING / "current-source.json"
        value = json.loads(current.read_bytes())
        value["files"] = [{"path": ".cargo/credentials.toml"}]
        current.write_bytes(admission.json_bytes(value))
        tracked = {name: {} for name in admission.read_json(self.root / "source/identity.json")["files"]}
        tracked[".cargo/credentials.toml"] = {}
        with mock.patch.object(os, "open", side_effect=guarded_open):
            with self.assertRaises(admission.AdmissionError) as failure:
                admission.capture_sources(repository, tracked)
        self.assertIn("credential", str(failure.exception))

    def test_post_readback_archive_replacement_never_gets_complete(self):
        self.fixture()
        original_verify = admission.verify_archive
        for target in (admission.ARCHIVE_NAME, admission.COMPACT_NAME):
            def replacing_verify(path, entries):
                result = original_verify(path, entries)
                if path.name == target:
                    replacement = path.with_name("corrupt-replacement")
                    replacement.write_bytes(b"not a gzip archive")
                    replacement.replace(path)
                return result
            output = self.base / ("replaced-" + target)
            with mock.patch.object(admission, "verify_archive", side_effect=replacing_verify):
                self.assertEqual(admission.package_evidence(self.root, output, self.ci, "success"), 1)
            index = admission.read_json(output / admission.INDEX_NAME)
            self.assertEqual(index["status"], "INCOMPLETE")
            self.assertTrue(any("verified archive changed" in error for error in index["errors"]))


    def test_receipt_llvm_or_unset_runtime_tampering_rejected(self):
        self.fixture()
        path = self.root / "debug/run/command.json"
        original = path.read_bytes()
        for mutation in ("missing", "wrong", "loader", "unset"):
            value = json.loads(original)
            if mutation == "missing":
                value["environment"].pop("OXID_LLVM_BIN")
            elif mutation == "wrong":
                value["environment"]["OXID_LLVM_BIN"] = "/other"
            elif mutation == "loader":
                value["environment"]["LD_LIBRARY_PATH"] = "/ambient"
            else:
                value["unset_controls"].remove("LD_LIBRARY_PATH")
            path.write_bytes(admission.json_bytes(value))
            with self.subTest(mutation=mutation), self.assertRaises(admission.AdmissionError):
                admission.verify_producer(self.root, self.ci, "success")
        path.write_bytes(original)

    def test_snapshot_races_preserve_stable_members_and_fail(self):
        self.fixture()
        victim = self.root / "debug/artifacts/array-core-0-acyclic.ll"
        original = victim.read_bytes()
        for mutation in ("change", "delete", "add"):
            victim.write_bytes(original)
            def observe(phase, name):
                if phase == "inventory":
                    if mutation == "change":
                        victim.write_text("modified")
                    elif mutation == "delete":
                        victim.unlink()
                    else:
                        (self.root / "new.txt").write_text("unexpected")
            output = self.base / mutation
            self.assertEqual(admission.package_evidence(self.root, output, self.ci, "success", _observe=observe), 1)
            index = admission.read_json(output / admission.INDEX_NAME)
            self.assertEqual(index["status"], "INCOMPLETE")
            self.assertFalse(any(error.startswith("producer admission:") for error in index["errors"]))
            with admission.tarfile.open(output / admission.ARCHIVE_NAME) as archive:
                self.assertEqual(archive.extractfile("producer/debug/run/stdout").read(), stdout_fixture())

    def test_archive_compression_is_deterministic_for_same_captured_bytes(self):
        snapshot = self.base / "snapshot"
        snapshot.mkdir()
        (snapshot / "receipt.json").write_text('{"synthetic": true}\n')
        entries = {"receipt.json": admission.file_record(snapshot / "receipt.json")}
        first, second = self.base / "first.tar.gz", self.base / "second.tar.gz"
        admission.write_archive(snapshot, first, entries)
        admission.write_archive(snapshot, second, entries)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        self.assertEqual(first.read_bytes()[4:8], b"\x00" * 4)
        self.assertEqual(admission.verify_archive(first, entries), admission.file_record(first))


    def test_archive_error_checksum_error_and_existing_output_cannot_pass(self):
        (self.root / "stable.txt").write_text("original")
        for phase in ("archive", "verify-archive"):
            def observe(actual, name):
                if phase == actual:
                    if actual == "archive":
                        raise OSError("synthetic tar write failure")
                    archive = self.base / phase / admission.ARCHIVE_NAME
                    archive.write_bytes(b"corrupt" + archive.read_bytes()[7:])
            output = self.base / phase
            self.assertEqual(admission.package_evidence(self.root, output, self.ci, "failure", _observe=observe), 1)
            self.assertEqual(admission.read_json(output / admission.INDEX_NAME)["status"], "INCOMPLETE")
        output = self.base / "existing"
        output.mkdir()
        sentinel = output / "sentinel"
        sentinel.write_text("preserved")
        with self.assertRaises(admission.AdmissionError):
            admission.package_evidence(self.root, output, self.ci, "success")
        self.assertEqual(sentinel.read_text(), "preserved")


if __name__ == "__main__":
    unittest.main()
