"""Synthetic controls only: these fixtures never qualify a Rust/LLVM run."""
import contextlib
import json
import os
from pathlib import Path
import re
import shlex
import struct
import signal
import subprocess
import sys
import time
from unittest import mock
import tempfile
import unittest

import verify_owned_array_native as admission


SLICE_NAMES = (
    "frontend::oir::owned::native::tests::slices::native_slices_acyclic_phi_and_rhs_failure_use_source_free_llvm",
    "frontend::oir::owned::native::tests::slices::native_slices_checkpoint_and_mutation_use_source_free_llvm",
    "frontend::oir::owned::native::tests::slices::native_slices_signed_bounds_and_fuel_use_source_free_llvm",
)


COMPOSITION_NAMES = (
    "frontend::oir::owned::native::tests::composition::native_composition_projected_array_bounds_fuel_and_rhs_precedence",
    "frontend::oir::owned::native::tests::composition::native_composition_raw_empty_sentinels_source_free",
    "frontend::oir::owned::native::tests::composition::native_composition_raw_pilot_source_free_every_fuel",
    "frontend::oir::owned::native::tests::composition::native_composition_source_free_depth_sixty_four",
    "frontend::oir::owned::native::tests::composition::native_composition_source_free_pilot_sentinels_effects_and_phi",
)

def listing_fixture(names, count=None):
    count = len(names) if count is None else count
    return ("\n".join(name + ": test" for name in names)
            + f"\n\n{count} tests, 0 benchmarks\n").encode()


def discovery_fixture():
    return listing_fixture(sorted((*admission.ROSTER, *SLICE_NAMES, *COMPOSITION_NAMES)))


def selection_mutations(argv):
    filters_end = argv.index("--exact")
    return {
        "broad-prefix": [argv[0], admission.PREFIX, *argv[filters_end + 1:]],
        "missing-exact": [argument for argument in argv if argument != "--exact"],
        "missing-name": [argv[0], *argv[2:]],
        "duplicate-name": [argv[0], argv[1], *argv[1:]],
        "filter-drift": [argv[0], argv[1] + "_drift", *argv[2:]],
        "slice-substitution": [argv[0], SLICE_NAMES[0], *argv[2:]],
        "composition-substitution": [argv[0], COMPOSITION_NAMES[0], *argv[2:]],
        "missing-ignored": [argument for argument in argv if argument != "--ignored"],
    }


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

    def test_pinned_rust_actual_pretty_and_terse_listing_bytes(self):
        # Original Rust 1.99.0 list-only captures from the fresh 13cfd8a8
        # rehearsal: terse omits the footer required by the existing contract.
        terse = ("\n".join(name + ": test" for name in admission.ROSTER) + "\n").encode()
        pretty = terse + b"\n16 tests, 0 benchmarks\n"
        self.assertEqual(admission.sha256(terse), "52d8c44957c258d0e7b7dfaad90d4b6ec338e8bae07d50ef132864793e304856")
        self.assertEqual(admission.sha256(pretty), "29cc4b930f62b84bcf99af1c8aaec583db5832f4b51aff6d3de5bf6300d3b2f0")
        self.assertEqual(admission.admit_list(pretty), list(admission.ROSTER))
        with self.assertRaisesRegex(admission.AdmissionError, "missing listing footer"):
            admission.admit_list(terse)

    def test_broad_discovery_separates_twenty_four_names_from_sixteen_selected(self):
        self.assertEqual(tuple(admission.SLICE_ROSTER), SLICE_NAMES)
        self.assertEqual(len(admission.ROSTER), 16)
        self.assertTrue(set(admission.ROSTER).isdisjoint(SLICE_NAMES))
        names = sorted((*admission.ROSTER, *SLICE_NAMES, *COMPOSITION_NAMES))
        self.assertEqual(len(names), 24)
        # The immutable nineteen-name predecessor remains independently identified.
        predecessor = listing_fixture(sorted((*admission.ROSTER, *SLICE_NAMES)))
        self.assertEqual(admission.sha256(predecessor),
                         "cb886c3662c245bedfe4eb4ee8789f1a3acaf252d7406a31545357bce01f1ada")
        self.assertEqual(tuple(admission.COMPOSITION_ROSTER), COMPOSITION_NAMES)
        self.assertTrue(set(admission.ROSTER).isdisjoint(COMPOSITION_NAMES))
        with self.assertRaises(admission.AdmissionError):
            admission.admit_discovery(predecessor)
        self.assertEqual(admission.admit_discovery(discovery_fixture()), names)
        self.assertEqual(admission.admit_list(listing_fixture(admission.ROSTER)), list(admission.ROSTER))
        with self.assertRaises(admission.AdmissionError):
            admission.admit_list(discovery_fixture())
        with self.assertRaises(admission.AdmissionError):
            admission.admit_discovery(listing_fixture(admission.ROSTER))

    def test_broad_discovery_rejects_unknown_missing_duplicate_and_footer_drift(self):
        names = sorted((*admission.ROSTER, *SLICE_NAMES, *COMPOSITION_NAMES))
        changed_names = ([], names[:-1], names[1:], names + [names[0]],
                         names + [admission.PREFIX + "slices::native_slices_unreviewed"],
                         [name for name in names if name != SLICE_NAMES[0]],
                         *[[name for name in names if name != omitted] for omitted in COMPOSITION_NAMES],
                         ["unexpected", *names[1:]])
        for values in changed_names:
            with self.subTest(values=values), self.assertRaises(admission.AdmissionError):
                admission.admit_discovery(listing_fixture(values, 24))
        for changed in (b"", listing_fixture(names, 16), listing_fixture(names, 20),
                        discovery_fixture().replace(b"24 tests, 0 benchmarks\n", b""),
                        discovery_fixture() + b"24 tests, 0 benchmarks\n",
                        discovery_fixture() + b"unexpected: test\n"):
            with self.subTest(data=changed), self.assertRaises(admission.AdmissionError):
                admission.admit_discovery(changed)

    def test_explicit_slice_workflow_gates_cover_all_three_ignored_names_in_both_profiles(self):
        repo = Path(__file__).resolve().parents[1]
        workflow = (repo / ".github/workflows/ci.yml").read_text()
        block = workflow.split("      - name: Verify borrowed scalar slices native parity in both profiles\n", 1)[1].split("      - name:", 1)[0]
        commands = [shlex.split(line.strip()) for line in block.splitlines()
                    if line.strip().startswith("cargo ") and "--bin oxid" in line]
        self.assertEqual(commands, [
            ["cargo", "test", "--locked", "--bin", "oxid", "native_slices", "--", "--ignored"],
            ["cargo", "test", "--release", "--locked", "--bin", "oxid", "native_slices", "--", "--ignored"],
        ])
        source = (repo / "src/frontend/oir/owned/slice_native_tests.rs").read_text()
        ignored = re.findall(r'#\[test\]\s*#\[ignore[^\n]*\]\s*fn (native_slices_\w+)\(', source)
        expected = {name.rsplit("::", 1)[1] for name in SLICE_NAMES}
        self.assertEqual(set(ignored), expected)
        self.assertEqual(len(ignored), 3)
        for command in commands:
            selected = command[command.index("oxid") + 1]
            self.assertEqual({name for name in SLICE_NAMES if selected in name}, set(SLICE_NAMES))

    def test_explicit_composition_gate_covers_five_names_in_both_profiles(self):
        repo = Path(__file__).resolve().parents[1]
        workflow = (repo / ".github/workflows/ci.yml").read_text()
        block = workflow.split("      - name: Verify owned record composition native parity in both profiles\n", 1)[1].split("      - name:", 1)[0]
        commands = [shlex.split(line.strip()) for line in block.splitlines()
                    if line.strip().startswith("cargo ") and "--bin oxid" in line]
        self.assertEqual(commands, [
            ["cargo", "test", "--locked", "--bin", "oxid", "native_composition", "--", "--include-ignored", "--test-threads=1"],
            ["cargo", "test", "--release", "--locked", "--bin", "oxid", "native_composition", "--", "--include-ignored", "--test-threads=1"],
        ])
        source = (repo / "src/frontend/oir/owned/composition_native_tests.rs").read_text()
        ignored = re.findall(r'#\[test\]\s*#\[ignore[^\n]*\]\s*fn (native_composition_\w+)\(', source)
        self.assertEqual(set(ignored), {name.rsplit("::", 1)[1] for name in COMPOSITION_NAMES})
        self.assertEqual(len(ignored), 5)

    def test_container_upload_paths_cover_all_members_after_early_failure(self):
        # Observed on runner 2.337.0, runs 37169581641 and 37169578747.
        # ContainerInfo.TranslateToContainerPath translates one leading prefix
        # per whole INPUT_PATH value, not each line of that value.
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/ci.yml').read_text()
        package = workflow.split('      - name: Index and preserve combined Unit2E evidence on every terminal state\n', 1)[1].split('      - name:', 1)[0]
        self.assertIn('        id: unit2e_preservation\n', package)
        self.assertIn('        if: always()\n', package)
        shell = '\n'.join(line[10:] for line in package.split('        run: |\n', 1)[1].splitlines())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / 'step-output'
            environment = dict(os.environ, RUNNER_TEMP='/__w/_temp', GITHUB_OUTPUT=str(output))
            process = subprocess.run(['/bin/bash', '-e', '-o', 'pipefail', '-c', 'python3() { return 7; }\n' + shell],
                                     env=environment, capture_output=True)
            self.assertEqual(process.returncode, 7)
            self.assertEqual(output.read_text(), 'upload-root=/__w/_temp/fixed-array-unit2e-upload\n')
        def translate(value):
            prefix = '/home/runner/work'
            return '/__w' + value[len(prefix):] if value.startswith(prefix + '/') else value
        uploads = [('Retain compact Unit2E evidence and original receipt bodies',
                    [admission.INDEX_NAME, admission.COMPACT_NAME, admission.COMPACT_NAME + '.sha256']),
                   ('Retain complete or explicitly incomplete combined Unit2E evidence',
                    [admission.ARCHIVE_NAME, admission.ARCHIVE_NAME + '.sha256'])]
        for title, names in uploads:
            block = workflow.split('      - name: ' + title + '\n', 1)[1].split('\n      - name:', 1)[0].split('\n  unit4-linux:', 1)[0]
            self.assertIn('        if: always()\n', block)
            paths = [line.strip() for line in block.split('          path: |\n', 1)[1].splitlines() if line.strip()]
            self.assertEqual(paths, ['${{ steps.unit2e_preservation.outputs.upload-root }}/' + name for name in names])
            actual = translate('\n'.join(paths).replace('${{ steps.unit2e_preservation.outputs.upload-root }}',
                                                    '/__w/_temp/fixed-array-unit2e-upload')).splitlines()
            wanted = ['/__w/_temp/fixed-array-unit2e-upload/' + name for name in names]
            self.assertEqual(actual, wanted)
            old = translate('\n'.join('/home/runner/work/_temp/fixed-array-unit2e-upload/' + name for name in names)).splitlines()
            self.assertEqual([path for path in old if path in wanted], wanted[:1])

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
                    "scripts/preserve_unit3_ci_evidence.py", "docs/architecture/fixed-array-unit2e-native-ci.md",
                    "scripts/replay_fixed_array_unit2d.py", "scripts/replay_unit2d_tool_capture.py",
                    "scripts/test_replay_fixed_array_unit2d.py")
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
        admission.write_json(self.root / "independent-llvm-tools.json", {name: tools[name]["path"] for name in admission.INDEPENDENT_TOOLS})
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
            self.command(profile + "/discovery", [str(binary), admission.PREFIX, "--list", "--ignored", "--format", "pretty", "--color", "never"], child_env, discovery_fixture())
            listing = listing_fixture(admission.ROSTER)
            self.command(profile + "/list", [str(binary), *admission.ROSTER, "--exact", "--list", "--ignored", "--format", "pretty", "--color", "never"], child_env, listing)
            stderr = ("\n".join(admission.family_summary(row) for row in admission.FAMILIES) + "\n").encode()
            self.command(profile + "/run", [str(binary), *admission.ROSTER, "--exact", "--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"], child_env, stdout_fixture(), stderr)
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
                      "command_receipts": {name: admission.file_record(root / name / "command.json") for name in ("build", "discovery", "list", "run")}}
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

    @contextlib.contextmanager
    def changed_producer_command(self, profile, name, *, argv=None, stdout=None):
        """Rebind synthetic receipt hashes so semantic admission is exercised."""
        root = self.root / profile
        command_path = root / name / "command.json"
        paths = (command_path, root / name / "stdout", root / "result.json",
                 self.root / "result.json", self.root / "evidence-manifest.json")
        originals = {path: path.read_bytes() for path in paths}
        try:
            command = admission.read_json(command_path)
            if argv is not None:
                command["argv"] = argv
            if stdout is not None:
                (root / name / "stdout").write_bytes(stdout)
                command["stdout"] = admission.file_record(root / name / "stdout")
            command_path.write_bytes(admission.json_bytes(command))
            result = admission.read_json(root / "result.json")
            result["command_receipts"][name] = admission.file_record(command_path)
            (root / "result.json").write_bytes(admission.json_bytes(result))
            terminal = admission.read_json(self.root / "result.json")
            terminal["profiles"][profile]["result"] = admission.file_record(root / "result.json")
            (self.root / "result.json").write_bytes(admission.json_bytes(terminal))
            inventory = admission.regular_inventory(self.root)
            inventory.pop("evidence-manifest.json")
            (self.root / "evidence-manifest.json").write_bytes(admission.json_bytes(inventory))
            yield
        finally:
            for path, data in originals.items():
                path.write_bytes(data)

    def test_selected_list_and_run_agree_on_exact_sixteen_argv_in_both_profiles(self):
        self.fixture()
        self.assertEqual(set(admission.verify_producer(self.root, self.ci, "success")["profiles"]),
                         {"debug", "release"})
        for profile in admission.PROFILES:
            root = self.root / profile
            binary = str(root / "bin/oxid-test")
            listed = admission.read_json(root / "list/command.json")["argv"]
            executed = admission.read_json(root / "run/command.json")["argv"]
            expected_selection = [binary, *admission.ROSTER, "--exact"]
            self.assertEqual(listed[:18], expected_selection)
            self.assertEqual(executed[:18], expected_selection)
            self.assertEqual(listed[18:], ["--list", "--ignored", "--format", "pretty", "--color", "never"])
            self.assertEqual(executed[18:], ["--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"])
            discovery = admission.read_json(root / "discovery/command.json")
            self.assertEqual(discovery["argv"], [binary, admission.PREFIX, "--list", "--ignored", "--format", "pretty", "--color", "never"])
            result = admission.read_json(root / "result.json")
            self.assertEqual(set(result["command_receipts"]), {"build", "discovery", "list", "run"})
            self.assertEqual(len(result["outcomes"]), 16)
            self.assertEqual([result[name] for name in ("array_artifact_files", "elf_executions_asserted", "reference_comparisons_derived")],
                             [1307, 7058, 6793])

    def test_resealed_selected_argv_mutations_fail_with_successful_sixteen_stdout(self):
        self.fixture()
        for profile in admission.PROFILES:
            for name in ("list", "run"):
                argv = admission.read_json(self.root / profile / name / "command.json")["argv"]
                for label, changed in selection_mutations(argv).items():
                    with self.subTest(profile=profile, command=name, mutation=label), \
                         self.changed_producer_command(profile, name, argv=changed):
                        with self.assertRaisesRegex(admission.AdmissionError, "invocation differs"):
                            admission.verify_producer(self.root, self.ci, "success")

    def test_resealed_broad_discovery_names_and_invocation_are_required(self):
        self.fixture()
        names = sorted((*admission.ROSTER, *SLICE_NAMES, *COMPOSITION_NAMES))
        mutations = {"missing-slice": listing_fixture([name for name in names if name != SLICE_NAMES[0]], 24),
                     "duplicate": listing_fixture([*names, names[0]], 24),
                     "unknown": listing_fixture([*names, admission.PREFIX + "unknown"], 24),
                     "historical-only": listing_fixture(admission.ROSTER)}
        for profile in admission.PROFILES:
            for label, data in mutations.items():
                with self.subTest(profile=profile, mutation=label), \
                     self.changed_producer_command(profile, "discovery", stdout=data):
                    with self.assertRaisesRegex(admission.AdmissionError, "missing|extra|duplicate|discovery"):
                        admission.verify_producer(self.root, self.ci, "success")
            argv = admission.read_json(self.root / profile / "discovery/command.json")["argv"]
            for label, changed in (("narrow-filter", [argv[0], admission.ROSTER[0], *argv[2:]]),
                                   ("exact-filter", [*argv, "--exact"]),
                                   ("missing-ignored", [argument for argument in argv if argument != "--ignored"])):
                with self.subTest(profile=profile, mutation=label), \
                     self.changed_producer_command(profile, "discovery", argv=changed):
                    with self.assertRaisesRegex(admission.AdmissionError, "invocation differs"):
                        admission.verify_producer(self.root, self.ci, "success")

    def test_full_synthetic_receipt_admission_and_archive_bytes(self):
        self.fixture()
        output = self.base / "upload"
        self.assertEqual(admission.package_evidence(self.root, output, self.ci, "success"), 0)
        index = admission.read_json(output / admission.INDEX_NAME)
        self.assertEqual(index["status"], "COMPLETE")
        self.assertFalse(index["complete_unit2e_qualification"])
        (output / admission.COMPACT_NAME).chmod(0o600)  # download permissions need not match hosted mode
        audit = admission.audit_compact(output / admission.INDEX_NAME, output / admission.COMPACT_NAME,
                                        self.ci["expected_head"], self.ci["event_sha"])
        self.assertEqual(audit["status"], "PASS")
        self.assertFalse(audit["complete_unit2e_qualification"])
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
                         "producer/debug/build/stdout", "producer/release/build/stderr",
                         *(f"producer/{profile}/discovery/{leaf}" for profile in admission.PROFILES
                           for leaf in ("command.json", "stdout", "stderr"))):
                self.assertIn(name, compact.getnames())
                self.assertEqual(compact.extractfile(name).read(), (self.root / name.removeprefix("producer/")).read_bytes())


    def test_independent_prelaunch_admission_failure_retains_original_reason(self):
        self.fixture()
        output = self.base / "independent/debug"
        args = admission.argparse.Namespace(producer_root=self.root, output=output, profile="debug",
                    expected_head=self.ci["expected_head"], event_sha=self.ci["event_sha"])
        with mock.patch.object(admission, "verify_producer", side_effect=admission.AdmissionError("synthetic stale producer source")), \
             mock.patch.object(admission, "run_child", side_effect=AssertionError("expensive child must not launch")):
            self.assertEqual(admission.execute_independent(args), 1)
        receipt = admission.read_json(self.base / "independent/debug-invocation/failure.json")
        self.assertEqual(receipt["exit_code"], 1)
        self.assertIn("synthetic stale producer source", receipt["error"])
        self.assertFalse(output.exists())

    def test_initial_git_failure_retains_compact_command_bodies_and_sidecars(self):
        self.fixture()
        independent = self.base / 'early-independent'
        debug = independent / 'debug'
        commands = debug / 'evidence/commands'
        commands.mkdir(parents=True)
        # Same original failure shape as hosted d582: first Git read exits 128,
        # before state.json, inputs, any compiler or independent test exists.
        stderr = b"fatal: detected dubious ownership in repository at '/__w/Oxid/Oxid'\n"
        command = {'argv': ['git', '--no-optional-locks', '-C', '/__w/Oxid/Oxid', 'rev-parse', '--show-toplevel'],
                   'label': 'git-rev-parse', 'exit': 128, 'expected_exit': 0,
                   'stdout': '0001-git-rev-parse.stdout', 'stderr': '0001-git-rev-parse.stderr'}
        admission.write_json(commands / '0001-git-rev-parse.json', command)
        (commands / command['stdout']).write_bytes(b'')
        (commands / command['stderr']).write_bytes(stderr)
        admission.write_json(debug / 'evidence/failure.json', {'status': 'failed', 'error': 'retained Git command failure'})
        output = self.base / 'early-failure-package'
        code = admission.package_evidence(self.root, output, self.ci, 'success', independent={
            'debug': {'root': debug, 'outcome': 'failure'},
            'release': {'root': independent / 'release', 'outcome': 'skipped'}})
        self.assertEqual(code, 1)
        index = admission.read_json(output / admission.INDEX_NAME)
        self.assertEqual(index['status'], 'INCOMPLETE')
        self.assertFalse(index['complete_unit2e_qualification'])
        self.assertEqual({path.name for path in output.iterdir()}, {admission.INDEX_NAME, admission.ARCHIVE_NAME,
            admission.COMPACT_NAME, admission.ARCHIVE_NAME + '.sha256', admission.COMPACT_NAME + '.sha256'})
        for name in (admission.ARCHIVE_NAME, admission.COMPACT_NAME):
            with tarfile.open(output / name) as archive:
                self.assertEqual(archive.extractfile('independent/debug/evidence/commands/' + command['stderr']).read(), stderr)
                self.assertEqual(json.loads(archive.extractfile('independent/debug/evidence/commands/0001-git-rev-parse.json').read()), command)
                self.assertIn('producer/debug/run/stdout', archive.getnames())
                self.assertIn('producer/release/run/stdout', archive.getnames())

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


class IndependentReaderTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "run"
        (self.root / "evidence/logged-tools").mkdir(parents=True)
        (self.root / "evidence/frozen-loggers").mkdir()
        (self.root / "inputs/tools").mkdir(parents=True)
        (self.root / "evidence/replay_unit2d_tool_capture.py").write_text("# synthetic identity only\n")
        (self.root / "inputs/tools/native_tool_logger.py").write_text("# synthetic identity only\n")
        for relative, target in admission.independent_links(self.root).items():
            (self.root / "evidence" / relative).symlink_to(target)

    def test_exact_inventory_records_known_links_and_excludes_only_frozen_scratch_and_seals(self):
        (self.root / "evidence/temporary").mkdir()
        (self.root / "evidence/temporary/scratch").write_text("not in frozen closure")
        for name in admission.INDEPENDENT_SEALS:
            (self.root / "evidence" / name).write_text("[]\n")
        rows = admission.independent_inventory(self.root)
        links = {row["path"]: row["target"] for row in rows if row["kind"] == "symlink"}
        self.assertEqual(links, admission.independent_links(self.root))
        self.assertEqual(len(links), 8)
        self.assertFalse(any(row["path"].startswith("temporary") or row["path"] in admission.INDEPENDENT_SEALS for row in rows))
        self.assertTrue(any(row["kind"] == "directory" for row in rows))

    def test_unknown_symlink_and_wrong_known_target_fail_without_following(self):
        unknown = self.root / "evidence/unknown"
        unknown.symlink_to(self.root / "missing-secret-target")
        with self.assertRaises(admission.AdmissionError):
            admission.independent_inventory(self.root)
        unknown.unlink()
        known = self.root / "evidence/logged-tools/llvm-as"
        known.unlink()
        known.symlink_to(self.root / "missing-secret-target")
        with self.assertRaises(admission.AdmissionError):
            admission.independent_inventory(self.root)

    def test_special_member_and_unreadable_directory_fail(self):
        special = self.root / "evidence/fifo"
        os.mkfifo(special)
        with self.assertRaises(admission.AdmissionError):
            admission.independent_inventory(self.root)
        special.unlink()
        denied = (self.root / "evidence/logged-tools").stat().st_ino
        original = os.scandir
        def fail_scandir(fd):
            if isinstance(fd, int) and os.fstat(fd).st_ino == denied:
                raise PermissionError("synthetic independent directory failure")
            return original(fd)
        with mock.patch.object(os, "scandir", side_effect=fail_scandir):
            with self.assertRaises(PermissionError):
                admission.independent_inventory(self.root)

    def test_absent_incomplete_active_or_fake_zero_execution_state_is_rejected(self):
        states = [{}, {"schema": 1, "status": "passed", "completed": []},
                  {"schema": 1, "status": "passed", "completed": list(admission.INDEPENDENT_PHASES), "active": "native"},
                  {"schema": 1, "status": "passed", "completed": list(admission.INDEPENDENT_PHASES), "seals": {}},
                  {"schema": 1, "status": "failed", "completed": list(admission.INDEPENDENT_PHASES)}]
        for value in states:
            (self.root / "state.json").write_bytes(admission.json_bytes(value))
            with self.subTest(value=value), self.assertRaises(admission.AdmissionError):
                admission.independent_body(self.root, head="1" * 40, tree="2" * 40, profile="debug", tools={}, schema=None, checkout=self.root)


    def terminal_fixture(self):
        expected_inputs = admission.independent_regular_manifest(self.root / "inputs")
        expected_evidence = admission.independent_inventory(self.root)
        seals = {}
        for name in admission.INDEPENDENT_SEALS:
            (self.root / "evidence" / name).write_bytes(admission.json_bytes(expected_evidence))
            seals[name] = admission.file_record(self.root / "evidence" / name)["sha256"]
        state = {"seals": seals}
        (self.root / "state.json").write_bytes(admission.json_bytes(state))
        state_sha = admission.file_record(self.root / "state.json")["sha256"]
        return state, state_sha, expected_inputs, expected_evidence

    def test_every_fixed_seal_is_rechecked_at_terminal_admission(self):
        arguments = self.terminal_fixture()
        admission.independent_terminal_identity(self.root, *arguments)
        for name in admission.INDEPENDENT_SEALS:
            path = self.root / "evidence" / name
            original = path.read_bytes()
            path.write_bytes(original + b"\n")
            with self.subTest(name=name), self.assertRaises(admission.AdmissionError):
                admission.independent_terminal_identity(self.root, *arguments)
            path.write_bytes(original)

    def test_input_membership_and_bytes_are_rechecked_at_terminal_admission(self):
        arguments = self.terminal_fixture()
        path = self.root / "inputs/tools/native_tool_logger.py"
        original = path.read_bytes()
        for mutation in ("change", "delete", "add"):
            if mutation == "change":
                path.write_bytes(original + b"# changed\n")
            elif mutation == "delete":
                path.unlink()
            else:
                (self.root / "inputs/extra").write_text("extra")
            with self.subTest(mutation=mutation), self.assertRaises(admission.AdmissionError):
                admission.independent_terminal_identity(self.root, *arguments)
            if mutation == "add":
                (self.root / "inputs/extra").unlink()
            path.write_bytes(original)


    def test_unrelated_sibling_creation_does_not_invalidate_ancestor_identity(self):
        expected = admission.independent_inventory(self.root)
        original = admission.file_record
        changed = [False]
        def create_unrelated_siblings(path):
            if not changed[0]:
                changed[0] = True
                (self.root.parent / "unrelated-sibling").write_text("unrelated parent activity")
                (self.root / "unrelated-evidence-sibling").write_text("unrelated run-root activity")
            return original(path)
        with mock.patch.object(admission, "file_record", side_effect=create_unrelated_siblings):
            self.assertEqual(admission.independent_inventory(self.root), expected)

    def test_identical_bytes_under_replaced_run_or_evidence_ancestor_are_rejected(self):
        import shutil
        original_record = admission.file_record
        for parent in (self.root / "evidence", self.root):
            moved = parent.with_name("replaced-" + parent.name)
            changed = [False]
            def replace_after_first_capture(path):
                if not changed[0]:
                    changed[0] = True
                    parent.rename(moved)
                    shutil.copytree(moved, parent, symlinks=True)
                return original_record(path)
            with self.subTest(parent=parent.name), mock.patch.object(admission, "file_record", side_effect=replace_after_first_capture):
                with self.assertRaises(admission.AdmissionError):
                    admission.independent_inventory(self.root)
            shutil.rmtree(parent)
            moved.rename(parent)


    def test_captured_input_references_join_blob_path_hash_length_and_type(self):
        checksum = admission.sha256(b"synthetic input")
        relative = "tool-captures/blobs/" + checksum
        snapshot = {"sha256": checksum, "bytes": len(b"synthetic input"),
                    "blob": str(self.root / "evidence" / relative)}
        inventory = {relative: {"kind": "file", "sha256": checksum, "bytes": snapshot["bytes"]}}
        admission.validate_independent_capture({"snapshots": [snapshot]}, self.root, inventory)
        admission.validate_independent_capture({"snapshots": []}, self.root, inventory)
        changes = [{"sha256": "0" * 64}, {"bytes": snapshot["bytes"] + 1},
                   {"blob": str(self.root / "elsewhere" / checksum)}, {"bytes": -1}, {"sha256": "invalid"}]
        for change in changes:
            with self.subTest(change=change), self.assertRaises(admission.AdmissionError):
                admission.validate_independent_capture({"snapshots": [{**snapshot, **change}]}, self.root, inventory)
        for changed in ({}, {relative: {**inventory[relative], "kind": "symlink"}},
                        {relative: {**inventory[relative], "sha256": "0" * 64}},
                        {relative: {**inventory[relative], "bytes": snapshot["bytes"] + 1}}):
            with self.subTest(changed=changed), self.assertRaises(admission.AdmissionError):
                admission.validate_independent_capture({"snapshots": [snapshot]}, self.root, changed)

    def test_head_tree_profile_and_run_root_are_joined_before_later_admission(self):
        baseline = {"schema": 1, "commit": "1" * 40, "checkout_head": "1" * 40, "tree": "2" * 40,
                    "profile": "debug", "run_root": str(self.root), "input_checkout": str(self.root), "checkout_clean": True,
                    "runner_sha256": admission.INDEPENDENT_RUNNER_SHA}
        for field, value in (("commit", "3" * 40), ("tree", "3" * 40), ("profile", "release"),
                             ("run_root", str(self.root / "other")), ("runner_sha256", "0" * 64)):
            binding = {**baseline, field: value}
            (self.root / "evidence/source-binding.json").write_bytes(admission.json_bytes(binding))
            rows = admission.independent_inventory(self.root)
            seals = {}
            for name in admission.INDEPENDENT_SEALS:
                (self.root / "evidence" / name).write_bytes(admission.json_bytes(rows))
                seals[name] = admission.file_record(self.root / "evidence" / name)["sha256"]
            state = {"schema": 1, "status": "passed", "completed": list(admission.INDEPENDENT_PHASES), "seals": seals,
                     "source_binding_sha256": admission.file_record(self.root / "evidence/source-binding.json")["sha256"]}
            (self.root / "state.json").write_bytes(admission.json_bytes(state))
            with self.subTest(field=field), self.assertRaises(admission.AdmissionError) as failure:
                admission.independent_body(self.root, head="1" * 40, tree="2" * 40, profile="debug", tools={}, schema=None, checkout=self.root)
            self.assertIn("source/head/profile/run", str(failure.exception))



"""Bounded SYNTHETIC receipt tests; no Rust/LLVM/native execution occurs.

Portable adjunct: place beside test_owned_array_native.py and the admission
module, then set OXID_UNIT2E_SCHEMA_REPO to a checkout of the frozen runner.
The existing PackageTests.fixture supplies dummy producer ELF headers.  The
independent fixture has fabricated original-command receipts, 29 physical
outcomes, one supplement outcome, one native ELF/module and one LLVM receipt.
Only the frozen INPUT identity is privately patched to this small fixture's
manifest.  All schema/admission functions run unchanged; no production bypass
is introduced.  An auditor module-location patch supplies its frozen helper
from a separate downloaded checker directory, never a hosted evidence path.
These bytes and any COMPLETE/PASS strings are unit-test fixtures, not proof of
execution, semantic correctness or real Unit2E qualification.
"""
import contextlib
import copy
import gzip
import io
import json
import os
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest
from unittest import mock

import verify_owned_array_native as admission


def replace_json(path, value):
    path.write_bytes(admission.json_bytes(value))


class CombinedReceiptTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.host = PackageTests('test_full_synthetic_receipt_admission_and_archive_bytes')
        cls.host.setUp()
        cls.addClassCleanup(cls.host.doCleanups)
        cls.base = cls.host.base
        source_repo = Path(os.environ.get('OXID_UNIT2E_SCHEMA_REPO', str(Path(admission.__file__).resolve().parents[1])))
        # This verifies the actual frozen runner SHA before importing it.
        cls.schema = admission.independent_schema(source_repo)
        frozen_runner = (source_repo / 'scripts/replay_fixed_array_unit2d.py').read_bytes()
        original_write_text = Path.write_text
        def fixture_source(path, text, *args, **kwargs):
            if path.name == 'replay_fixed_array_unit2d.py' and text == 'synthetic receipt fixture, not executed\n':
                return path.write_bytes(frozen_runner)
            return original_write_text(path, text, *args, **kwargs)
        # Replace only the synthetic source file's contents before its Git
        # commit/source capture; source and runner hash checks stay real.
        with mock.patch.object(Path, 'write_text', fixture_source):
            cls.host.fixture()
        cls.producer = cls.host.root
        cls.ci = cls.host.ci
        cls.tools = admission.read_json(cls.producer / 'toolchains.json')
        cls.producer_admission = admission.verify_producer(cls.producer, cls.ci, 'success')
        cls.inputs = cls.small_inputs()
        cls.input_manifest = {
            'synthetic_only': True,
            'files': [{'path': name, 'bytes': len(data), 'sha256': admission.sha256(data)} for name, data in sorted(cls.inputs.items())],
            'replay_inventories': {'provenance_kind': 'historical-observed-inventory; not a semantic oracle',
                'structure_sidecars': ['synthetic.structure.tsv'], 'structure_sidecar_count': 1,
                'bounds_sites': 1, 'pointer_phis': 1},
        }
        cls.input_digest = admission.sha256(admission.json_bytes(cls.input_manifest))
        cls.identity_patch = mock.patch.object(admission, 'INDEPENDENT_INPUT_SHA', cls.input_digest)
        cls.identity_patch.start()
        cls.addClassCleanup(cls.identity_patch.stop)
        cls.independent = {}
        for profile in admission.PROFILES:
            root = cls.base / 'independent' / profile
            cls.make_profile(root, profile)
            cls.independent[profile] = {'root': root, 'outcome': 'success'}
        cls.auditor = cls.base / 'downloaded-checker' / 'scripts'
        cls.auditor.mkdir(parents=True)
        (cls.auditor / 'replay_fixed_array_unit2d.py').write_bytes(frozen_runner)
        cls.module_patch = mock.patch.object(admission, '__file__', str(cls.auditor / 'verify_owned_array_native.py'))
        cls.module_patch.start()
        cls.addClassCleanup(cls.module_patch.stop)
        cls.counter = 0
        cls.good_output = cls.base / 'good-upload'
        code = admission.package_evidence(cls.producer, cls.good_output, cls.ci, 'success', independent=cls.independent)
        cls.good_index = admission.read_json(cls.good_output / admission.INDEX_NAME)
        if code != 0:
            raise AssertionError('Synthetic combined positive rejected: ' + repr(cls.good_index['errors']))
        cls.archive_bytes = (cls.good_output / admission.COMPACT_NAME).read_bytes()
        with tarfile.open(fileobj=io.BytesIO(cls.archive_bytes), mode='r:gz') as archive:
            cls.members = [(copy.copy(member), archive.extractfile(member).read() if member.isfile() else None) for member in archive]
        cls.audit(cls.good_output)

    @classmethod
    def small_inputs(cls):
        module = b'; SYNTHETIC old IR, never compiled\n'
        old_manifest = [{'path': 'old-ir/synthetic.ll', 'length': len(module), 'sha256': admission.sha256(module)}]
        harness = 'case\tstatus\tstdout_hex\tstderr_hex\n' + ''.join(
            f'synthetic-{i:02d}\t{int(i >= 18)}\t\t\n' for i in range(29))
        return {
            'qualification-v2.json': admission.json_bytes({'synthetic_only': True, 'elf_artifacts': 1,
                'source_free_executions': 30, 'official_llvm_command_receipts': 1}),
            'expectations/old-ir-manifest.json': admission.json_bytes(old_manifest),
            'expectations/old-ir-inventory.tsv': b'synthetic\tnever-executed\n',
            'expectations/physical-harness.tsv': harness.encode(),
            'expectations/supplement-v1.json': admission.json_bytes({'synthetic_only': True,
                'positive_shared_alias': {'status': 0, 'stdout_hex': '', 'stderr_hex': ''}, 'extreme_cases': []}),
            'tools/native_tool_logger.py': b'# SYNTHETIC logger identity, never invoked\n',
        }

    @classmethod
    def make_profile(cls, root, profile):
        evidence = root / 'evidence'
        evidence.mkdir(parents=True)
        def put(name, value, executable=False):
            path = evidence / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(value if isinstance(value, bytes) else admission.json_bytes(value))
            if executable:
                path.chmod(0o755)
            return admission.file_record(path)
        for name, data in cls.inputs.items():
            path = root / 'inputs' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        put('original-source.json', {'synthetic_only': True})
        put('source.tar', b'SYNTHETIC source archive, never extracted\n')
        put('input-manifest.json', cls.input_manifest)
        put('replay_unit2d_tool_capture.py', b'# SYNTHETIC capture helper, never invoked\n')
        binding = {'schema': 1, 'synthetic_only': True, 'commit': cls.ci['expected_head'], 'checkout_head': cls.ci['expected_head'],
            'tree': cls.producer_admission['binding']['tree'], 'profile': profile, 'run_root': str(root), 'checkout_clean': True,
            'runner_sha256': admission.INDEPENDENT_RUNNER_SHA, 'input_checkout': str((cls.host.base / 'repo')),
            'public_array_activation': cls.schema.PUBLIC_ARRAY_ACTIVATION,
            'borrowed_slot_compatibility': cls.schema.BORROWED_SLOT_COMPATIBILITY,
            'module_sha256': cls.schema.BORROWED_SLOT_COMPATIBILITY['current_module_sha256'],
            'original_manifest_sha256': admission.file_record(evidence / 'original-source.json')['sha256'],
            'archive_sha256': admission.file_record(evidence / 'source.tar')['sha256'],
            'input_manifest_sha256': cls.input_digest, 'original_input_manifest_sha256': cls.input_digest,
            'inputs': admission.independent_regular_manifest(root / 'inputs'),
            'capture_runner_sha256': admission.file_record(evidence / 'replay_unit2d_tool_capture.py')['sha256']}
        content_id = admission.sha256(json.dumps(binding, sort_keys=True, separators=(',', ':')).encode())
        binding.update(content_id=content_id, marker='independent_unit2d_replay_marker_' + content_id)
        source_hash = put('source-binding.json', binding)['sha256']
        for name, target in admission.independent_links(root).items():
            link = evidence / name
            link.parent.mkdir(parents=True, exist_ok=True)
            link.symlink_to(target)
        selected = {name: cls.tools['all_tools'][name]['path'] for name in admission.INDEPENDENT_TOOLS}
        hashes = {name: cls.tools['all_tools'][name]['sha256'] for name in admission.INDEPENDENT_TOOLS}
        put('trusted-tools.json', selected)
        put('tool-hashes.json', hashes)
        put('expectation-provenance.json', cls.schema.PROVENANCE)
        test_binary = put('bin/oxid-unit2d-tests', elf_fixture(), True)
        binary = {'relative_path': 'evidence/bin/oxid-unit2d-tests', 'sha256': test_binary['sha256'], 'bytes': test_binary['bytes'],
            'source_binding_sha256': source_hash, 'cargo_artifact': {'reason': 'compiler-artifact', 'target': {'name': 'oxid'},
            'profile': {'test': True}, 'fresh': False, 'executable': str(root / 'target' / profile / 'synthetic-tests')}}
        binary_hash = put('binary.json', binary)['sha256']
        verified = {'status': 'passed', 'synthetic_only': True, 'source_commit': cls.ci['expected_head'],
            'source_tree': binding['tree'], 'profile': profile, 'source_binding_sha256': source_hash,
            'test_binary_sha256': test_binary['sha256'], 'historical_binary_is_comparison_only': True,
            'fixed_inventory_totals_are_not_semantic_oracles': True, 'ordinary': 8, 'native_families': 9,
            'elf_artifacts': 1, 'source_free_executions': 30}
        put('verified.json', verified)
        names = cls.schema.ORDINARY + [cls.schema.PREFIX + binding['marker']] + cls.schema.NATIVE + [cls.schema.PHYSICAL]
        put('test-inventory.json', {'all': names, 'ignored': cls.schema.NATIVE + [cls.schema.PHYSICAL], 'scope': names})
        labels = ['git-rev-parse', 'git-status', 'git-rev-parse', 'git-rev-parse', 'git-rev-parse',
            'git-ls-tree', 'git-archive', 'git-status', 'git-rev-parse']
        labels += [name + '-version' for name in ('rustc', 'cargo', *admission.INDEPENDENT_TOOLS)]
        labels += ['cargo-build', 'test-inventory', 'ignored-inventory', binding['marker']]
        labels += [name.rsplit('::', 1)[-1] for name in cls.schema.ORDINARY + cls.schema.NATIVE]
        labels += ['storage-text-tests', 'prepare-physical', 'verify-physical', cls.schema.PHYSICAL.rsplit('::', 1)[-1]]
        by_label = {name.rsplit('::', 1)[-1]: name for name in names}
        git_arguments = [('rev-parse', '--show-toplevel'), ('status', '--porcelain=v1', '--untracked-files=all'),
            ('rev-parse', 'HEAD'), ('rev-parse', cls.ci['expected_head'] + '^{commit}'),
            ('rev-parse', cls.ci['expected_head'] + '^{tree}'), ('ls-tree', '-r', '-z', cls.ci['expected_head']),
            ('archive', '--format=tar', '--output=' + str(evidence / 'source.tar'), cls.ci['expected_head']),
            ('status', '--porcelain=v1', '--untracked-files=all'), ('rev-parse', 'HEAD')]
        for i, label in enumerate(labels):
            stem = f'{i:03d}'
            test = by_label.get(label)
            argv = ['synthetic-unexecuted', label]
            if i < len(git_arguments):
                argv = cls.schema.git_argv((cls.host.base / 'repo'), *git_arguments[i])
            stdout, stderr = b'SYNTHETIC unexecuted original receipt\n', b''
            if test:
                argv = [str(root / binary['relative_path']), '--exact', test, '--nocapture', '--test-threads=1']
                if test in cls.schema.NATIVE + [cls.schema.PHYSICAL]:
                    argv.append('--ignored')
                stdout = f'test {test} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n'.encode()
                if label == binding['marker']:
                    stderr = ('OXID_UNIT2D_SOURCE_BINDING=' + content_id).encode()
            put('commands/' + stem + '.stdout', stdout)
            put('commands/' + stem + '.stderr', stderr)
            put('commands/' + stem + '.json', {'label': label, 'exit': 0, 'expected_exit': 0, 'argv': argv,
                'cwd': str(root if i < len(git_arguments) else root / 'source'), 'stdout': stem + '.stdout', 'stderr': stem + '.stderr', 'synthetic_only': True})
        native_record = put('native/synthetic.elf', elf_fixture(), True)
        native = {'path': 'native/synthetic.elf', 'sha256': native_record['sha256'], 'bytes': native_record['bytes']}
        llvm_record = put('native/synthetic.ll', b'; SYNTHETIC native IR, never compiled\n')
        put('artifact-manifest.json', {'elf': [native], 'llvm': [{'path': 'synthetic.ll', 'sha256': llvm_record['sha256'], 'bytes': llvm_record['bytes']}]})
        execution_binding = []
        for name, status in [(f'physical-synthetic-{i:02d}', int(i >= 18)) for i in range(29)] + [('shared-array-aliases', 0)]:
            receipt = name + '.json'
            original = {'env_clear': True, 'PATH': str(root / 'synthetic-runtime/no-tools'), 'cwd': str(root / 'synthetic-runtime'),
                'status': status, 'binary': str(evidence / 'native/synthetic'), 'synthetic_only': True}
            row = {'receipt': receipt, 'receipt_sha256': put('executions/' + receipt, original)['sha256'], 'status': status, 'binary': native}
            for stream in ('stdout', 'stderr'):
                row[stream + '_sha256'] = put('executions/' + name + '.' + stream, b'')['sha256']
            execution_binding.append(row)
        put('execution-binding.json', execution_binding)
        put('executions/synthetic.structure.tsv', b'SYNTHETIC structure sidecar\n')
        put('structure.json', {'bounds': [{'synthetic_only': True}], 'phis': [{'synthetic_only': True}], 'bounds_sites': 1, 'pointer_phis': 1})
        blob = b'; SYNTHETIC captured compiler input\n'
        blob_hash = admission.sha256(blob)
        put('tool-captures/blobs/' + blob_hash, blob)
        tool = {'tool': 'llvm-as', 'trusted_target': selected['llvm-as'], 'exit': 0,
            'stdout': '000.stdout', 'stderr': '000.stderr', 'snapshots': [{'sha256': blob_hash, 'bytes': len(blob), 'blob': str(evidence / 'tool-captures/blobs' / blob_hash)}], 'synthetic_only': True}
        put('tool-receipts/000.json', tool)
        put('tool-receipts/000.stdout', b'')
        put('tool-receipts/000.stderr', b'')
        put('tool-captures/000/receipt.json', tool)
        put('tool-captures/000/stdout', b'')
        put('tool-captures/000/stderr', b'')
        put('old-ir/synthetic.ll', b'; SYNTHETIC old IR, never compiled\n')
        put('old-ir/inventory.tsv', cls.inputs['expectations/old-ir-inventory.tsv'])
        old = cls.schema.compare_old_ir(evidence / 'old-ir', json.loads(cls.inputs['expectations/old-ir-manifest.json']), cls.inputs['expectations/old-ir-inventory.tsv'])
        put('old-ir-comparison.json', old)
        for name in ('harness.tsv', 'manifest.tsv'):
            put('physical/' + name, cls.inputs['expectations/physical-harness.tsv'])
        for name in ('input-binding.json', 'harness-receipt.json'):
            put('physical/' + name, {'source_commit': cls.ci['expected_head'], 'synthetic_only': True})
        put('physical/SHA256SUMS', b'SYNTHETIC fixture\n')
        put('physical-comparison.json', [{'case': f'synthetic-{i:02d}', 'status': int(i >= 18)} for i in range(29)])
        put('supplement-comparison.json', {'provenance': 'source-frozen semantic expected outputs from expectations/supplement-v1.json',
            'cases': ['shared-array-aliases'], 'exact_status_stdout_stderr_match': True})
        # The excluded temporary subtree and verified target cache must not leak
        # into either archive; a copied required binary must remain full-only.
        put('temporary/synthetic-scratch', b'UNSEALED temporary fixture bytes')
        (root / 'target').mkdir()
        (root / 'target/cache-only').write_bytes(b'SYNTHETIC regenerable target cache')
        seals = {}
        for name in admission.INDEPENDENT_SEALS:
            value = admission.independent_inventory(root) if name == 'phase-verify-artifacts.json' else {'synthetic_phase': name}
            seals[name] = put(name, value)['sha256']
        state = {'schema': 1, 'status': 'passed', 'active': None, 'completed': list(admission.INDEPENDENT_PHASES),
            'seals': seals, 'source_binding_sha256': source_hash, 'binary_binding_sha256': binary_hash,
            'tools': {'trusted': selected, 'hashes': hashes, 'rust_bin': str(Path(cls.tools['all_tools']['rustc']['path']).parent),
                'rust_hashes': {name: cls.tools['all_tools'][name]['sha256'] for name in ('cargo', 'rustc')},
                'cargo_home': cls.tools['effective_environment']['CARGO_HOME'], 'trusted_map_path': str(cls.producer / 'independent-llvm-tools.json'),
                'trusted_map_sha256': admission.file_record(cls.producer / 'independent-llvm-tools.json')['sha256']}, 'synthetic_only': True}
        replace_json(root / 'state.json', state)
        observed = admission.independent_body(root, head=cls.ci['expected_head'], tree=binding['tree'], profile=profile, tools=cls.tools, schema=cls.schema, checkout=(cls.host.base / 'repo'))
        launch = root.with_name(profile + '-invocation')
        launch.mkdir()
        invocation = admission.read_json(cls.producer / 'invocation.json')
        configs = admission.check_cargo_config(root / 'source', Path(cls.tools['effective_environment']['CARGO_HOME']), admission.read_json(cls.producer / 'source/identity.json')['files'])
        launch_binding = {'ci': cls.ci, 'profile': profile, 'producer_invocation_id': invocation['invocation_id'],
            'producer_result_sha256': admission.file_record(cls.producer / 'result.json')['sha256'], 'source': cls.producer_admission['binding'],
            'output_root': str(root), 'trusted_map_sha256': state['tools']['trusted_map_sha256'],
            'external_cargo_config': [row for row in configs if row['category'] != 'repository']}
        replace_json(launch / 'invocation.json', launch_binding)
        replace_json(launch / 'closure-manifest.json', observed.pop('closure'))
        # Reuse producer's original receipt helper, temporarily targeting launcher.
        original_root = cls.host.root
        try:
            cls.host.root = launch
            cls.host.command('command', admission.independent_command(Path(invocation['repository_path']), root, profile, cls.tools, cls.producer),
                cls.tools['effective_environment'], b'SYNTHETIC independent invocation, never executed\n')
        finally:
            cls.host.root = original_root
        replace_json(launch / 'result.json', {'status': 'PASS', 'exit_code': 0, 'binding': launch_binding, 'admission': observed,
            'closure_manifest_sha256': admission.file_record(launch / 'closure-manifest.json')['sha256'],
            'command_sha256': admission.file_record(launch / 'command/command.json')['sha256']})
        replace_json(launch / 'evidence-manifest.json', admission.regular_inventory(launch))

    @classmethod
    def audit(cls, output):
        return admission.audit_compact(output / admission.INDEX_NAME, output / admission.COMPACT_NAME, cls.ci['expected_head'], cls.ci['event_sha'])

    def fresh_output(self, label='negative'):
        type(self).counter += 1
        return self.base / (label + '-' + str(type(self).counter))

    @contextlib.contextmanager
    def changed(self, path, data=None, missing=False):
        original, mode = path.read_bytes(), path.stat().st_mode & 0o777
        try:
            if missing:
                path.unlink()
            else:
                path.write_bytes(data)
            yield
        finally:
            path.write_bytes(original)
            path.chmod(mode)

    def reject_package(self, selected=None):
        output = self.fresh_output()
        self.assertEqual(admission.package_evidence(self.producer, output, self.ci, 'success',
            independent=self.independent if selected is None else selected), 1)
        index = admission.read_json(output / admission.INDEX_NAME)
        self.assertEqual(index['status'], 'INCOMPLETE')
        self.assertFalse(index['complete_unit2e_qualification'])
        self.assertTrue(index['errors'])
        self.assertTrue((output / admission.ARCHIVE_NAME).exists())
        self.assertTrue((output / admission.COMPACT_NAME).exists())
        shutil.rmtree(output)
        return index

    @contextlib.contextmanager
    def resealed(self, root):
        """Rebind only a temporary fixture's terminal seal, never real evidence."""
        state = admission.read_json(root / 'state.json')
        closure = admission.json_bytes(admission.independent_inventory(root))
        state['seals']['phase-verify-artifacts.json'] = admission.sha256(closure)
        with self.changed(root / 'evidence/phase-verify-artifacts.json', closure), \
             self.changed(root / 'state.json', admission.json_bytes(state)):
            yield

    def read_body(self, root):
        return admission.independent_body(root, head=self.ci['expected_head'],
            tree=self.producer_admission['binding']['tree'], profile='debug', tools=self.tools, schema=self.schema, checkout=(self.host.base / 'repo'))

    def rewrite_download(self, output, transform):
        output.mkdir()
        target = output / admission.COMPACT_NAME
        with target.open('wb') as raw, gzip.GzipFile(fileobj=raw, mode='wb', filename='', mtime=0, compresslevel=1) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as archive:
                for member, data in self.members:
                    replacement = transform(member.name, data)
                    if replacement is False:
                        continue
                    info = copy.copy(member)
                    if info.isfile():
                        info.size = len(replacement)
                        archive.addfile(info, io.BytesIO(replacement))
                    else:
                        archive.addfile(info)
        index = copy.deepcopy(self.good_index)
        index['compact'] = admission.file_record(target)
        replace_json(output / admission.INDEX_NAME, index)

    def test_combined_positive_and_relocated_download_without_hosted_files(self):
        self.assertEqual(self.good_index['status'], 'COMPLETE')
        self.assertTrue(self.good_index['complete_unit2e_qualification'])
        self.assertFalse(self.good_index['producer_only'])
        output = self.fresh_output('download')
        output.mkdir()
        for name in (admission.COMPACT_NAME, admission.INDEX_NAME):
            shutil.copyfile(self.good_output / name, output / name)
        (output / admission.COMPACT_NAME).chmod(0o600)
        # Make every hosted producer, runner, repository, tool and Cargo path
        # unavailable. Only downloaded archive/index and checker remain readable.
        moved = []
        try:
            for name in ('producer', 'independent', 'repo', 'tools', 'cargo-home'):
                path = self.base / name
                hidden = self.base / ('offline-' + name)
                path.rename(hidden)
                moved.append((path, hidden))
            result = self.audit(output)
            self.assertEqual(result['status'], 'PASS')
            self.assertTrue(result['complete_unit2e_qualification'])
        finally:
            for path, hidden in reversed(moved):
                hidden.rename(path)
        names = {member.name for member, _ in self.members}
        for profile in admission.PROFILES:
            self.assertIn('independent/' + profile + '/evidence/physical/harness.tsv', names)
            self.assertIn('independent/' + profile + '/evidence/tool-captures/000/receipt.json', names)
            self.assertNotIn('independent/' + profile + '/evidence/bin/oxid-unit2d-tests', names)
            self.assertNotIn('independent/' + profile + '/evidence/native/synthetic.elf', names)
        with tarfile.open(self.good_output / admission.ARCHIVE_NAME) as archive:
            full = set(archive.getnames())
            self.assertFalse(any('/temporary/' in name or '/target/' in name for name in full))
            for profile in admission.PROFILES:
                self.assertIn('independent/' + profile + '/evidence/bin/oxid-unit2d-tests', full)
                for name, target in admission.independent_links(self.independent[profile]['root']).items():
                    member = archive.getmember('independent/' + profile + '/evidence/' + name)
                    self.assertTrue(member.issym())
                    self.assertEqual(member.linkname, target)

    def test_missing_profile_and_unsuccessful_independent_steps(self):
        self.reject_package({'debug': self.independent['debug']})
        for outcome in ('failure', 'skipped', 'cancelled'):
            with self.subTest(outcome=outcome):
                selected = copy.deepcopy(self.independent)
                selected['release']['outcome'] = outcome
                self.reject_package(selected)
        selected = copy.deepcopy(self.independent)
        selected['release']['root'] = self.base / 'absent' / 'release'
        self.reject_package(selected)

    def test_original_launcher_missing_stale_profile_head_tool_and_failure(self):
        root = self.independent['debug']['root']
        launch = root.with_name('debug-invocation')
        path = launch / 'result.json'
        original = admission.read_json(path)
        for key, value in (('producer_invocation_id', 'STALE-SYNTHETIC-RUN'), ('profile', 'release'),
                           ('ci', {**self.ci, 'expected_head': 'f' * 40}), ('trusted_map_sha256', '0' * 64)):
            with self.subTest(binding=key):
                result = copy.deepcopy(original)
                result['binding'][key] = value
                with self.changed(path, admission.json_bytes(result)):
                    self.assertIn('binding differs', ' '.join(self.reject_package()['errors']))
        for name in ('result.json', 'command/stdout', 'closure-manifest.json'):
            with self.subTest(missing=name), self.changed(launch / name, missing=True):
                self.reject_package()
        result = copy.deepcopy(original)
        result['exit_code'] = 1
        with self.changed(path, admission.json_bytes(result)):
            self.reject_package()

    def test_runner_binary_input_source_and_body_mutations_fail_closed(self):
        root = self.independent['debug']['root']
        for name in ('evidence/bin/oxid-unit2d-tests', 'inputs/qualification-v2.json',
                     'evidence/source-binding.json', 'evidence/verified.json', 'evidence/physical-comparison.json',
                     'evidence/commands/018.json', 'evidence/executions/physical-synthetic-00.stdout',
                     'evidence/tool-captures/000/receipt.json', 'evidence/phase-verify-artifacts.json'):
            with self.subTest(changed=name), self.changed(root / name, (root / name).read_bytes() + b'\nTAMPERED'):
                self.reject_package()
        for name in ('evidence/bin/oxid-unit2d-tests', 'inputs/expectations/physical-harness.tsv',
                     'evidence/commands/018.stdout', 'evidence/executions/physical-synthetic-00.json'):
            with self.subTest(missing=name), self.changed(root / name, missing=True):
                self.reject_package()

    def test_original_tool_map_and_command_argv_cannot_fallback(self):
        root = self.independent['debug']['root']
        for path, mutate in (
            (root / 'state.json', lambda state: state['tools']['trusted'].update({'clang': '/usr/bin/fallback-clang'})),
            (root.with_name('debug-invocation') / 'command/command.json', lambda receipt: receipt['argv'].__setitem__(0, '/usr/bin/fallback-python')),
        ):
            value = admission.read_json(path)
            mutate(value)
            with self.subTest(path=str(path)), self.changed(path, admission.json_bytes(value)):
                self.reject_package()

    def test_independent_git_receipts_require_exact_checkout_and_reset(self):
        root = self.independent['debug']['root']
        commands = [admission.read_json(path) for path in sorted((root / 'evidence/commands').glob('*.json'))]
        admission.admit_independent_git_commands(commands, (self.host.base / 'repo'), self.ci['expected_head'], root, self.schema)
        for field, value in [('safe', 'safe.directory=*'), ('checkout', '/different/checkout'),
                             ('reset', 'safe.directory=/inherited/other'), ('cwd', '/different/cwd')]:
            changed = copy.deepcopy(commands)
            if field == 'cwd':
                changed[0]['cwd'] = value
            else:
                changed[0]['argv'][{'safe': 4, 'checkout': 7, 'reset': 2}[field]] = value
            with self.subTest(field=field), self.assertRaisesRegex(admission.AdmissionError, 'exact-checkout Git invocation differs'):
                admission.admit_independent_git_commands(changed, (self.host.base / 'repo'), self.ci['expected_head'], root, self.schema)
        path = root / 'evidence/commands/000.json'
        row = admission.read_json(path)
        row['argv'][4] = 'safe.directory=*'
        with self.changed(path, admission.json_bytes(row)), self.resealed(root):
            with self.assertRaisesRegex(admission.AdmissionError, 'exact-checkout Git invocation differs'):
                self.read_body(root)

    def test_phase_completion_and_zero_execution_fake_success_rejected(self):
        root = self.independent['debug']['root']
        state = admission.read_json(root / 'state.json')
        for mutate in (lambda value: value.update(completed=list(admission.INDEPENDENT_PHASES[:-1])),
                       lambda value: value.update(active='verify'), lambda value: value.update(status='running')):
            changed = copy.deepcopy(state)
            mutate(changed)
            with self.changed(root / 'state.json', admission.json_bytes(changed)):
                self.assertIn('did not finish all phases', ' '.join(self.reject_package()['errors']))
        # Regenerate the synthetic closure around an empty execution inventory
        # so the real reader reaches its nonempty inventory guard rather than merely
        # detecting an old seal. Other receipts still claim successful execution.
        with contextlib.ExitStack() as changes:
            changes.enter_context(self.changed(root / 'evidence/execution-binding.json', admission.json_bytes([])))
            for path in sorted((root / 'evidence/executions').glob('*.json')):
                changes.enter_context(self.changed(path, missing=True))
            with self.resealed(root):
                with self.assertRaisesRegex(admission.AdmissionError, 'independent execution receipts: invalid expected inventory'):
                    self.read_body(root)

    def test_resealed_wrong_tool_and_physical_result_bodies_are_rejected(self):
        root = self.independent['debug']['root']
        path = root / 'evidence/tool-receipts/000.json'
        receipt = admission.read_json(path)
        receipt['trusted_target'] = '/usr/bin/fallback-llvm-as'
        with self.changed(path, admission.json_bytes(receipt)), self.resealed(root):
            with self.assertRaisesRegex(admission.AdmissionError, 'LLVM command used wrong tool or failed'):
                self.read_body(root)
        path = root / 'evidence/physical-comparison.json'
        result = admission.read_json(path)
        result[0]['status'] = 1
        with self.changed(path, admission.json_bytes(result)), self.resealed(root):
            with self.assertRaisesRegex(admission.AdmissionError, 'physical comparison body differs'):
                self.read_body(root)

    def test_every_independent_compact_body_missing_or_tampered(self):
        selected = [(member.name, data) for member, data in self.members
                    if member.isfile() and member.name.startswith('independent/')]
        self.assertGreater(len(selected), 100)
        for name, original in selected:
            for missing in (True, False):
                with self.subTest(member=name, missing=missing):
                    output = self.fresh_output('corrupt-download')
                    self.rewrite_download(output, lambda current, data: (False if missing else original + b'\nTAMPERED') if current == name else data)
                    try:
                        with self.assertRaisesRegex(admission.AdmissionError, 'compact body roster differs|compact original body hash mismatch'):
                            self.audit(output)
                    finally:
                        shutil.rmtree(output)
        print(f'SYNTHETIC compact body matrix: {len(selected)} members x 2 mutations = {2 * len(selected)} fail-closed checks')


    def coherent_download(self, label, mutate):
        bodies = {member.name: data for member, data in self.members}
        full = json.loads(bodies['membership.json'])
        compact = json.loads(bodies['compact-membership.json'])
        changes = {}
        mutate(bodies, full, changes)
        for name, data in changes.items():
            bodies[name] = data
            full['members'][name].update(bytes=len(data), sha256=admission.sha256(data))
            if name in compact['members']:
                compact['members'][name].update(bytes=len(data), sha256=admission.sha256(data))
        bodies['membership.json'] = admission.json_bytes(full)
        compact['members']['membership.json'].update(bytes=len(bodies['membership.json']), sha256=admission.sha256(bodies['membership.json']))
        expected = {name for name in full['members'] if (name.startswith('producer/') and admission.compact_member(name))
                    or admission.independent_compact_member(name)}
        compact['full_only_members'] = sorted(set(full['members']) - expected)
        bodies['compact-membership.json'] = admission.json_bytes(compact)
        output = self.fresh_output(label)
        self.rewrite_download(output, lambda name, data: bodies[name])
        return output

    def producer_command_download(self, label, profile, name, *, argv=None, stdout=None):
        def mutate(bodies, full, changes):
            prefix = "producer/" + profile
            command_name = prefix + "/" + name + "/command.json"
            command = json.loads(bodies[command_name])
            def record(path):
                data = changes.get(path, bodies[path])
                return {**full["members"][path], "bytes": len(data), "sha256": admission.sha256(data)}
            if argv is not None:
                command["argv"] = argv
            if stdout is not None:
                path = prefix + "/" + name + "/stdout"
                changes[path] = stdout
                command["stdout"] = record(path)
            changes[command_name] = admission.json_bytes(command)
            result_name = prefix + "/result.json"
            result = json.loads(bodies[result_name])
            result["command_receipts"][name] = record(command_name)
            changes[result_name] = admission.json_bytes(result)
            terminal = json.loads(bodies["producer/result.json"])
            terminal["profiles"][profile]["result"] = record(result_name)
            changes["producer/result.json"] = admission.json_bytes(terminal)
            manifest = json.loads(bodies["producer/evidence-manifest.json"])
            for path in changes:
                manifest[path.removeprefix("producer/")] = record(path)
            changes["producer/evidence-manifest.json"] = admission.json_bytes(manifest)
        return self.coherent_download(label, mutate)

    def test_offline_resealed_selected_argv_rejects_broad_missing_duplicate_and_filter_drift(self):
        for profile in admission.PROFILES:
            for name in ("list", "run"):
                argv = admission.read_json(self.producer / profile / name / "command.json")["argv"]
                for label, changed in selection_mutations(argv).items():
                    with self.subTest(profile=profile, command=name, mutation=label):
                        output = self.producer_command_download("selected-" + label, profile, name, argv=changed)
                        try:
                            with self.assertRaisesRegex(admission.AdmissionError, "original profile invocations differ"):
                                self.audit(output)
                        finally:
                            shutil.rmtree(output)

    def test_offline_resealed_discovery_admits_only_current_nineteen_and_broad_argv(self):
        names = sorted((*admission.ROSTER, *SLICE_NAMES, *COMPOSITION_NAMES))
        mutations = {"missing-slice": listing_fixture([name for name in names if name != SLICE_NAMES[0]], 24),
                     "duplicate": listing_fixture([*names, names[0]], 24),
                     "unknown": listing_fixture([*names, admission.PREFIX + "unknown"], 24),
                     "historical-only": listing_fixture(admission.ROSTER)}
        for profile in admission.PROFILES:
            for label, data in mutations.items():
                with self.subTest(profile=profile, mutation=label):
                    output = self.producer_command_download("discovery-" + label, profile, "discovery", stdout=data)
                    try:
                        with self.assertRaisesRegex(admission.AdmissionError, "missing|extra|duplicate|discovery"):
                            self.audit(output)
                    finally:
                        shutil.rmtree(output)
            argv = admission.read_json(self.producer / profile / "discovery/command.json")["argv"]
            for label, changed in (("narrow-filter", [argv[0], admission.ROSTER[0], *argv[2:]]),
                                   ("exact-filter", [*argv, "--exact"]),
                                   ("missing-ignored", [argument for argument in argv if argument != "--ignored"])):
                with self.subTest(profile=profile, mutation=label):
                    output = self.producer_command_download("discovery-" + label, profile, "discovery", argv=changed)
                    try:
                        with self.assertRaisesRegex(admission.AdmissionError, "original profile invocations differ"):
                            self.audit(output)
                    finally:
                        shutil.rmtree(output)

    def test_offline_exact_closure_and_original_launcher_and_known_links(self):
        def extra(bodies, full, changes):
            full['members']['independent/debug/evidence/undeclared-extra.elf'] = {
                'kind': 'file', 'mode': 0o755, 'bytes': 123, 'sha256': '1' * 64}
        def launcher(bodies, full, changes):
            name = 'independent/debug-invocation/closure-manifest.json'
            changes[name] = admission.json_bytes([])
            manifest_name = 'independent/debug-invocation/evidence-manifest.json'
            manifest = json.loads(bodies[manifest_name])
            manifest['closure-manifest.json'].update(bytes=len(changes[name]), sha256=admission.sha256(changes[name]))
            changes[manifest_name] = admission.json_bytes(manifest)
        def link(bodies, full, changes):
            name = 'independent/debug/evidence/phase-verify-artifacts.json'
            seal = json.loads(bodies[name])
            for row in seal:
                if row['path'] == 'logged-tools/clang':
                    row['target'] = '/synthetic-unknown-link-target'
            full['members']['independent/debug/evidence/logged-tools/clang']['target'] = '/synthetic-unknown-link-target'
            changes[name] = admission.json_bytes(seal)
            state_name = 'independent/debug/state.json'
            state = json.loads(bodies[state_name])
            state['seals']['phase-verify-artifacts.json'] = admission.sha256(changes[name])
            changes[state_name] = admission.json_bytes(state)
        for label, mutate in (('extra-full-member', extra), ('unbound-launcher-closure', launcher), ('unknown-link', link)):
            with self.subTest(label=label):
                output = self.coherent_download(label, mutate)
                with self.assertRaises(admission.AdmissionError):
                    self.audit(output)

    def test_offline_state_must_match_original_launcher_admission(self):
        def rust_selection(bodies, full, changes):
            name = 'independent/debug/state.json'
            state = json.loads(bodies[name])
            state['tools']['rust_bin'] = '/synthetic-wrong-rust-selection/bin'
            changes[name] = admission.json_bytes(state)
        def phase_seal(bodies, full, changes):
            name = 'independent/debug/state.json'
            state = json.loads(bodies[name])
            seal = 'independent/debug/evidence/phase-build-artifacts.json'
            changes[seal] = bodies[seal] + b'\n'
            state['seals']['phase-build-artifacts.json'] = admission.sha256(changes[seal])
            changes[name] = admission.json_bytes(state)
        for label, mutate in (('rust-selection', rust_selection), ('phase-seal', phase_seal)):
            with self.subTest(state_change=label):
                output = self.coherent_download('unbound-state', mutate)
                with self.assertRaises(admission.AdmissionError):
                    self.audit(output)

    def test_offline_whole_original_admission_and_index_identity(self):
        original = admission.read_json(self.independent['debug']['root'].with_name('debug-invocation') / 'result.json')['admission']
        cases = [('state_sha256', '0' * 64), ('source_binding_sha256', '0' * 64),
                 ('test_binary', {**original['test_binary'], 'mode': 0o644}),
                 ('phase_seals', {**original['phase_seals'], 'phase-build-artifacts.json': '0' * 64}),
                 ('verified', {**original['verified'], 'profile': 'release'}),
                 ('named_test_outcomes', list(reversed(original['named_test_outcomes'])))]
        for field, value in cases:
            committed = {}
            def mutate(bodies, full, changes):
                name = 'independent/debug-invocation/result.json'
                result = json.loads(bodies[name])
                result['admission'][field] = value
                changes[name] = admission.json_bytes(result)
                manifest_name = 'independent/debug-invocation/evidence-manifest.json'
                manifest = json.loads(bodies[manifest_name])
                manifest['result.json'].update(bytes=len(changes[name]), sha256=admission.sha256(changes[name]))
                changes[manifest_name] = admission.json_bytes(manifest)
                committed.update(result['admission'], launcher_result_sha256=admission.sha256(changes[name]),
                                 launcher_evidence_manifest_sha256=admission.sha256(changes[manifest_name]))
            with self.subTest(original_field=field):
                output = self.coherent_download('changed-original-admission', mutate)
                index = admission.read_json(output / admission.INDEX_NAME)
                index['independent']['debug'] = committed
                (output / admission.INDEX_NAME).write_bytes(admission.json_bytes(index))
                with self.assertRaises(admission.AdmissionError):
                    self.audit(output)
            with self.subTest(index_field=field):
                output = self.coherent_download('changed-index-admission', lambda bodies, full, changes: None)
                index = admission.read_json(output / admission.INDEX_NAME)
                index['independent']['debug'][field] = value
                (output / admission.INDEX_NAME).write_bytes(admission.json_bytes(index))
                with self.assertRaisesRegex(admission.AdmissionError, 'original admission differs from index'):
                    self.audit(output)
        for field in ('launcher_result_sha256', 'launcher_evidence_manifest_sha256'):
            with self.subTest(index_field=field):
                output = self.coherent_download('changed-index-launcher', lambda bodies, full, changes: None)
                index = admission.read_json(output / admission.INDEX_NAME)
                index['independent']['debug'][field] = '0' * 64
                (output / admission.INDEX_NAME).write_bytes(admission.json_bytes(index))
                with self.assertRaisesRegex(admission.AdmissionError, 'original admission differs from index'):
                    self.audit(output)

    def test_malformed_independent_receipts_preserve_raw_and_unaffected_evidence(self):
        root = self.independent['debug']['root']
        target = root / 'target/debug/partial-required.elf'
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b'SYNTHETIC surviving partial target')
        target.chmod(0o755)
        cases = [('binary-truncated', root / 'evidence/binary.json', b'{'),
                 ('binary-wrong-type', root / 'evidence/binary.json', b'[]\n'),
                 ('state-truncated', root / 'state.json', b'{'),
                 ('state-wrong-type', root / 'state.json', b'[]\n')]
        for label, path, malformed in cases:
            output = self.fresh_output(label)
            with self.subTest(label=label), self.changed(path, malformed):
                self.assertEqual(admission.package_evidence(self.producer, output, self.ci, 'success', independent=self.independent), 1)
                index = admission.read_json(output / admission.INDEX_NAME)
                self.assertEqual(index['status'], 'INCOMPLETE')
                self.assertFalse(index['complete_unit2e_qualification'])
                self.assertTrue((output / admission.COMPACT_NAME).is_file())
                with tarfile.open(output / admission.ARCHIVE_NAME) as archive:
                    self.assertEqual(archive.extractfile('independent/debug/' + path.relative_to(root).as_posix()).read(), malformed)
                    self.assertEqual(archive.extractfile('independent/debug/target/debug/partial-required.elf').read(), target.read_bytes())
                    self.assertIn('producer/debug/run/stdout', archive.getnames())
                    self.assertIn('independent/release/evidence/bin/oxid-unit2d-tests', archive.getnames())
            shutil.rmtree(output)
        target.unlink()

    def test_offline_archive_digest_and_parsing_share_one_identity_window(self):
        alternate_dir = self.fresh_output('alternate')
        marker = 'SYNTHETIC_ALTERNATE_ARCHIVE_NOT_BOUND_TO_INDEX'
        def alternate_body(name, data):
            if name == 'compact-membership.json':
                value = json.loads(data)
                value['full_only_scope'] = marker
                return admission.json_bytes(value)
            return data
        self.rewrite_download(alternate_dir, alternate_body)
        alternate = (alternate_dir / admission.COMPACT_NAME).read_bytes()
        for trigger in (1, 2):
            output = self.fresh_output('aba-audit')
            output.mkdir()
            archive = output / admission.COMPACT_NAME
            archive.write_bytes(self.archive_bytes)
            index = output / admission.INDEX_NAME
            index.write_bytes((self.good_output / admission.INDEX_NAME).read_bytes())
            real_stable, real_json = admission.stable_file, admission.strict_json
            opened, parsed = [], []
            @contextlib.contextmanager
            def scheduled(path):
                if Path(path) == archive:
                    opened.append(str(path))
                    if len(opened) == trigger:
                        archive.write_bytes(alternate)
                        try:
                            with real_stable(path) as value:
                                yield value
                        finally:
                            archive.write_bytes(self.archive_bytes)
                        return
                with real_stable(path) as value:
                    yield value
            def observe(data):
                value = real_json(data)
                if isinstance(value, dict) and value.get('full_only_scope') == marker:
                    parsed.append(True)
                return value
            with self.subTest(trigger=trigger), mock.patch.object(admission, 'stable_file', scheduled), \
                 mock.patch.object(admission, 'strict_json', observe):
                with self.assertRaises(admission.AdmissionError):
                    self.audit(output)
            self.assertGreaterEqual(len(opened), trigger)
            self.assertFalse(parsed)
            self.assertEqual(index.read_bytes(), (self.good_output / admission.INDEX_NAME).read_bytes())
            self.assertEqual(archive.read_bytes(), self.archive_bytes)

    def test_resealed_capture_snapshot_requires_matching_blob_hash_length_and_path(self):
        root = self.independent['debug']['root']
        path = root / 'evidence/tool-captures/000/receipt.json'
        original = admission.read_json(path)
        for field, value in (('sha256', '0' * 64), ('bytes', original['snapshots'][0]['bytes'] + 1),
                             ('blob', str(root / 'missing-blob'))):
            with self.subTest(field=field):
                receipt = copy.deepcopy(original)
                receipt['snapshots'][0][field] = value
                with self.changed(path, admission.json_bytes(receipt)), self.resealed(root):
                    with self.assertRaises(admission.AdmissionError):
                        self.read_body(root)

    def test_full_only_binary_commitment_mismatch_is_rejected(self):
        target = 'independent/debug/evidence/bin/oxid-unit2d-tests'
        full_data = next(data for member, data in self.members if member.name == 'membership.json')
        full = json.loads(full_data)
        full['members'][target]['sha256'] = '0' * 64
        changed_full = admission.json_bytes(full)
        compact = json.loads(next(data for member, data in self.members if member.name == 'compact-membership.json'))
        compact['members']['membership.json'].update(bytes=len(changed_full), sha256=admission.sha256(changed_full))
        replacements = {'membership.json': changed_full, 'compact-membership.json': admission.json_bytes(compact)}
        output = self.fresh_output('full-only-corrupt')
        self.rewrite_download(output, lambda name, data: replacements.get(name, data))
        with self.assertRaisesRegex(admission.AdmissionError, 'independent full-only binary commitment differs'):
            self.audit(output)



if __name__ == "__main__":
    unittest.main()
