"""Reject missing, changed and falsely qualified original-event evidence."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

import verify_historical_event_evidence as evidence

ARCHIVE = Path(__file__).resolve().parents[1] / evidence.ARCHIVE_RELATIVE


class HistoricalEventEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with zipfile.ZipFile(ARCHIVE) as archive:
            cls.members = {name: archive.read(evidence.PREFIX + name) for name in evidence.REQUIRED_MEMBERS}

    def change_receipt(self, name, update):
        members = copy.deepcopy(self.members)
        def replace(path, value):
            # Preserve lengths so malformed typed fields reach their semantic
            # checks instead of failing only the aggregate byte envelope.
            raw = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
            self.assertLessEqual(len(raw), len(members[path]))
            members[path] = raw + b" " * (len(members[path]) - len(raw))

        value = json.loads(members[name])
        update(value)
        replace(name, value)
        if name in ("plan.json", "command.json", "resource-seam.json"):
            result = json.loads(members["result.json"])
            if name == "plan.json":
                command = json.loads(members["command.json"])
                command["plan_sha256"] = hashlib.sha256(members[name]).hexdigest()
                replace("command.json", command)
                result["plan_sha256"] = hashlib.sha256(members[name]).hexdigest()
            if name == "resource-seam.json":
                result["resource_seam_sha256"] = hashlib.sha256(members[name]).hexdigest()
            result["command_sha256"] = hashlib.sha256(members["command.json"]).hexdigest()
            replace("result.json", result)
        index_name = evidence.flat("index.json")
        index = json.loads(members[index_name])
        descriptors = [row["flat_file"] for row in index["entries"]] + index["original_unit2_inputs"]
        for row in descriptors:
            body = members[row["path"]]
            row.update(bytes=len(body), sha256=hashlib.sha256(body).hexdigest())
        index["aggregate_bytes_before_index"] = sum(len(members[evidence.flat(path)]) for path in evidence.ORIGINAL_PATHS)
        members[index_name] = json.dumps(index).encode()
        return members

    def test_exact_retained_archive_and_all_receipts(self):
        result = evidence.verify_archive(ARCHIVE)
        self.assertEqual(result["tests"], 92)
        self.assertEqual(result["indexed_entries"], 53)
        self.assertEqual(result["old_unit2_inputs"], 6)
        self.assertEqual(result["archive"]["sha256"], evidence.ARCHIVE_SHA256)
        self.assertIs(result["counts"]["execution_qualified"], False)
        self.assertEqual(result["counts"]["compiler_executions"], 0)

    def test_mutated_truncated_and_wrong_archives_fail(self):
        raw = ARCHIVE.read_bytes()
        mutated = bytearray(raw)
        mutated[len(mutated) // 2] ^= 1
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "evidence.zip"
            for body in (raw[:-1], bytes(mutated), bytes(len(raw))):
                with self.subTest(bytes=len(body)):
                    path.write_bytes(body)
                    with self.assertRaises(ValueError):
                        evidence.verify_archive(path)

    def test_directory_and_aliased_archive_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(ValueError):
                evidence.verify_archive(Path(directory))
            link = Path(directory) / "archive.zip"
            try:
                link.symlink_to(ARCHIVE)
            except (OSError, NotImplementedError):
                return  # Windows hosts may disallow creation of symlinks.
            with self.assertRaises(ValueError):
                evidence.verify_archive(link)

    def test_every_indexed_or_preserved_member_is_required(self):
        for name in sorted(self.members):
            with self.subTest(name=name):
                members = dict(self.members)
                del members[name]
                with self.assertRaises(ValueError):
                    evidence.verify_members(members)
        with self.assertRaises(ValueError):
            evidence.verify_members({**self.members, evidence.flat("error.json"): b"{}"})

    def test_changed_old_input_or_preparation_stream_fails(self):
        for name in ("runner.stderr", evidence.flat("case-2/unit2/run/materialize.stdout")):
            with self.subTest(name=name), self.assertRaises(ValueError):
                evidence.verify_members({**self.members, name: b"changed"})

    def test_wrong_typed_completion_or_checkout_cannot_claim_success(self):
        for key, value in (("execution_qualified", 0), ("execution_qualified", True),
                           ("compiler_executions", False), ("compiler_executions", 1),
                           ("source_unit", 36), ("source_unit", 37.0),
                           ("status", "passed"), ("additional_public_lifecycle_pending", 0)):
            with self.subTest(key=key, value=value), self.assertRaisesRegex(ValueError, "historical completion mismatch: " + key):
                evidence.verify_members(self.change_receipt(evidence.flat("complete.json"),
                                                           lambda item: item.update({key: value})))
        for key, value in (("checkout_head", "0" * 40), ("event_name", "pull_request"),
                           ("full_checkpoint_reconstruction", 0)):
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "historical checkout mismatch: " + key):
                evidence.verify_members(self.change_receipt(evidence.flat("checkout.json"),
                                                           lambda item: item.update({key: value})))

    def test_rehashed_failed_old_execution_is_rejected(self):
        for name, key, value in (("result.json", "semantic_pass", False),
                                 ("result.json", "direct_current_execution", 1),
                                 ("command.json", "exit_status", False),
                                 ("plan.json", "current_source_members", 376.0)):
            label = name.removesuffix(".json")
            with self.subTest(name=name, key=key), self.assertRaisesRegex(ValueError, "original Unit2 " + label + " mismatch: " + key):
                evidence.verify_members(self.change_receipt(name, lambda item: item.update({key: value})))

    def test_duplicate_json_and_typed_descriptor_rejected(self):
        name = evidence.flat("index.json")
        with self.assertRaises(ValueError):
            evidence.verify_members({**self.members, name: b'{"status":"failed","status":"passed-source-controls-only"}'})
        index = json.loads(self.members[name])
        index["entries"][1]["flat_file"]["bytes"] = False
        with self.assertRaises(ValueError):
            evidence.verify_members({**self.members, name: json.dumps(index).encode()})


if __name__ == "__main__":
    unittest.main()
