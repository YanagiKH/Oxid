"""Synthetic controls only: these fixtures never qualify a Rust/LLVM run."""
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

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


if __name__ == "__main__":
    unittest.main()
