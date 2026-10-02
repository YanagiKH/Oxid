"""Negative execution controls; these tests never invoke an Oxid compiler."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import protocol as p


class ProtocolControls(unittest.TestCase):
    def test_exact_unique_ids(self):
        p.exact_ids(["b", "a"], ["a", "b"])

    def test_zero_execution_is_rejected(self):
        with self.assertRaises(p.ProtocolError):
            p.exact_ids([], ["a"])

    def test_missing_id_is_rejected(self):
        with self.assertRaises(p.ProtocolError):
            p.exact_ids(["a"], ["a", "b"])

    def test_duplicate_cannot_replace_missing_id(self):
        with self.assertRaises(p.ProtocolError):
            p.exact_ids(["a", "a"], ["a", "b"])

    def test_unexpected_id_is_rejected(self):
        with self.assertRaises(p.ProtocolError):
            p.exact_ids(["a", "x"], ["a", "b"])

    def test_profile_pair(self):
        p.both_profiles([{"profile": "debug"}, {"profile": "release"}])
        for rows in ([], [{"profile": "debug"}], [{"profile": "debug"}] * 2):
            with self.assertRaises(p.ProtocolError):
                p.both_profiles(rows)

    def test_rust_names_and_terminal_are_both_required(self):
        good = ("running 2 tests\ntest a ... ok\ntest b ... ok\n"
                "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; "
                "900 filtered out; finished in 0.03s\n")
        self.assertEqual(p.rust_success(good, ["a", "b"])["filtered"], 900)
        for bad in (good.replace("test b ... ok\n", ""),
                    good.replace("test b ... ok", "test a ... ok"),
                    good.replace("test b ... ok", "test b ... ignored"),
                    good.split("test result:")[0],
                    good.replace("2 passed", "0 passed"),
                    good.replace("0 ignored", "1 ignored"),
                    good + good):
            with self.assertRaises(p.ProtocolError):
                p.rust_success(bad, ["a", "b"])

    def test_listing_requires_exact_names_and_terminal(self):
        p.rust_listing("a: test\nb: test\n\n2 tests, 0 benchmarks\n", ["a", "b"])
        for bad in ("a: test\n1 test, 0 benchmarks\n", "a: test\nb: test\n"):
            with self.assertRaises(p.ProtocolError):
                p.rust_listing(bad, ["a", "b"])

    def test_comparison_cannot_count_only(self):
        good = {"profile": "debug", "cases": 2, "counts": {"match": 2},
                "results": [{"case": name, "status": "match", "failures": [], "unavailable": []}
                            for name in ("a", "b")]}
        p.comparison(good, "debug", ["a", "b"])
        bad = copy.deepcopy(good)
        bad["results"][1]["case"] = "a"
        with self.assertRaises(p.ProtocolError):
            p.comparison(bad, "debug", ["a", "b"])

    def test_raw_and_normalized_streams_reject_partial_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "raw.jsonl"
            for rows in ([], [{"case": "a"}], [{"case": "a"}, {"case": "a"}],
                         [{"case": "a"}, {"case": "b", "normalization_error": "bad"}]):
                file.write_text("".join(json.dumps(row) + "\n" for row in rows))
                with self.assertRaises(p.ProtocolError):
                    p.observations(file, ["a", "b"])

    def test_stale_receipts_and_altered_artifacts_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "raw").write_bytes(b"observed")
            binding = {key: "current-" + key for key in p.BINDINGS}
            good = {**binding, "schema": 1, "status": "passed", "exit_status": 0,
                    "artifacts": [{"path": "raw", "bytes": 8, "sha256": p.digest(b"observed")}]}
            p.run_receipt(good, binding, root, ["raw"])
            for field in p.BINDINGS:
                bad = copy.deepcopy(good)
                bad[field] = "previous-run"
                with self.assertRaises(p.ProtocolError):
                    p.run_receipt(bad, binding, root, ["raw"])
            for field, value in (("status", "running"), ("exit_status", 1)):
                bad = copy.deepcopy(good)
                bad[field] = value
                with self.assertRaises(p.ProtocolError):
                    p.run_receipt(bad, binding, root, ["raw"])
            (root / "raw").write_bytes(b"modified")
            with self.assertRaises(p.ProtocolError):
                p.run_receipt(good, binding, root, ["raw"])

    def test_path_alias_and_traversal_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "good").write_text("okay")
            (root / "alias").symlink_to(root / "good")
            for name in ("../good", "/good", "./good", "alias", "absent", "x//y"):
                with self.assertRaises(p.ProtocolError):
                    p.relative_file(root, name)

    def test_regenerated_manifest_cannot_redefine_captured_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "manifest.json"
            file.write_bytes(b'{"files": ["original"]}')
            captured = p.digest(file.read_bytes())
            p.unchanged_file(file, captured)
            file.write_bytes(b'{"files": ["replacement"]}')
            with self.assertRaises(p.ProtocolError):
                p.unchanged_file(file, captured)


if __name__ == "__main__":
    unittest.main()
