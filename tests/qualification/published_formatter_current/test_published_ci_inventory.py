"""Inactive exact-workflow transport controls with explicit genuine inputs.

The bounded caller must authenticate this file, helper modules and all supplied
bodies. No discovery, imports of production targets, command execution, skipped
tests or synthetic replacement of the approved 173-entry inventory occurs here.
"""
import copy
import hashlib
import json
import unittest


def build_suite(ci, reject, *, audit, oldest, middle, old_dispositions,
                source_descriptor):
    for name, value in (("audit", audit), ("oldest", oldest), ("middle", middle),
                        ("old_dispositions", old_dispositions)):
        if type(value) is not bytes or not value:
            raise ValueError("missing genuine workflow fixture: " + name)
    if not isinstance(source_descriptor, dict):
        raise ValueError("missing genuine current source descriptor")

    def sha(raw):
        return hashlib.sha256(raw).hexdigest()

    def raw(value):
        return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()

    def binding(path, body):
        return {"path": path, "bytes": len(body), "sha256": sha(body)}

    class ExactWorkflowTransport(unittest.TestCase):
        def setUp(self):
            self.source = copy.deepcopy(source_descriptor)
            self.workflow = ci.workflow_successor(oldest, middle, self.source["sha256"])
            self.workflow_pin = binding(".github/workflows/ci.yml", self.workflow)
            self.old = json.loads(old_dispositions)
            self.plan = ci.disposition_successor(
                audit, oldest, middle, old_dispositions, self.source, self.workflow_pin)
            self.plan_raw = raw(self.plan)
            self.plan_pin = binding("published-direct-consumer-dispositions-v1.json",
                                    self.plan_raw)

        def validate(self, **changes):
            args = dict(audit=audit, oldest=oldest, middle=middle, current=self.workflow,
                        old_raw=old_dispositions, current_raw=self.plan_raw,
                        source_descriptor=self.source, workflow_descriptor=self.workflow_pin,
                        dispositions_descriptor=self.plan_pin)
            args.update(changes)
            return ci.validate(**args)

        def test_three_generations_have_genuine_fixed_bindings(self):
            self.assertEqual(sha(oldest), ci.retained.WORKFLOW_SHA)
            self.assertEqual((len(middle), sha(middle)),
                             (67408, "81dfdfe82d9e477c4b6535a10967fc8d6861bcbcea5f90d341c694797ced3269"))
            expected = oldest.replace(ci.BYTE_SOURCE_SHA.encode(), ci.M_SOURCE_SHA.encode())
            expected = expected.replace(ci.UNIT1_OLD, ci.UNIT1_CURRENT, 1)
            self.assertEqual(expected, middle)
            self.assertNotEqual(oldest, middle)
            self.assertNotEqual(middle, self.workflow)

        def test_only_two_physical_argument_lines_change(self):
            before = middle.splitlines(keepends=True)
            after = self.workflow.splitlines(keepends=True)
            self.assertEqual(len(before), len(after))
            self.assertEqual([i + 1 for i, pair in enumerate(zip(before, after))
                              if pair[0] != pair[1]], [755, 768])
            for number in (755, 768):
                self.assertEqual(
                    after[number - 1],
                    before[number - 1].replace(ci.M_SOURCE_SHA.encode(),
                                               self.source["sha256"].encode(), 1))
            self.assertEqual(
                self.workflow.replace(self.source["sha256"].encode(), ci.M_SOURCE_SHA.encode()),
                middle)

        def test_complete_173_roster_and_every_disposition_preserved(self):
            self.assertEqual(len(self.plan["entries"]), 173)
            self.assertEqual(self.plan["entry_count"], 173)
            self.assertEqual([r["id"] for r in self.plan["entries"]],
                             [r["id"] for r in self.old["entries"]])
            for new, old in zip(self.plan["entries"], self.old["entries"]):
                expected = copy.deepcopy(old)
                if old["id"] in ("ci.yml:L751", "ci.yml:L764"):
                    expected["active_command"] = old["active_command"].replace(
                        ci.M_SOURCE_SHA, self.source["sha256"])
                    expected["active_command_sha256"] = sha(
                        expected["active_command"].encode())
                self.assertEqual(new, expected, old["id"])
            self.assertEqual(self.plan["disposition_counts"], self.old["disposition_counts"])
            self.assertEqual(self.plan["retained_workflow_scope"],
                             self.old["retained_workflow_scope"])

        def test_new_provenance_names_real_source_checkpoint(self):
            self.assertEqual(self.plan["compiler_head"], ci.source.SOURCE_CHECKPOINT["head"])
            self.assertEqual(self.plan["compiler_tree"], ci.source.SOURCE_CHECKPOINT["tree"])
            self.assertEqual(self.old["compiler_head"], ci.source.HISTORICAL_SOURCE["head"])
            self.assertEqual(self.old["compiler_tree"], ci.source.HISTORICAL_SOURCE["tree"])
            self.assertEqual(self.plan["current_source_manifest_sha256"],
                             self.source["sha256"])
            self.assertEqual(self.plan["status"],
                             "source-only-successor-pending-independent-execution")

        def test_valid_complete_record_makes_no_execution_claim(self):
            self.assertEqual(self.validate(), {
                "entry_count": 173, "changed_commands": ["ci.yml:L751", "ci.yml:L764"],
                "argument_lines": [755, 768], "source_only": True,
                "execution_qualified": False})

        def test_oldest_workflow_tamper_rejected(self):
            with self.assertRaises(reject):
                ci.workflow_successor(oldest + b"\n", middle, self.source["sha256"])

        def test_published_middle_workflow_tamper_rejected(self):
            with self.assertRaises(reject):
                ci.workflow_successor(oldest, middle + b"\n", self.source["sha256"])

        def test_changed_audit_rejected(self):
            with self.assertRaises(reject):
                self.validate(audit=audit + b"\n")

        def test_changed_old_disposition_rejected(self):
            with self.assertRaises(reject):
                self.validate(old_raw=old_dispositions + b"\n")

        def test_bad_source_digest_rejected(self):
            for bad in ("", "A" * 64, "0" * 63, "0" * 65, "z" * 64,
                        ci.M_SOURCE_SHA, ci.BYTE_SOURCE_SHA):
                with self.subTest(digest=bad), self.assertRaises(reject):
                    ci.workflow_successor(oldest, middle, bad)

        def test_unapproved_current_workflow_rejected_even_if_rehashed(self):
            changed = self.workflow + b"\n"
            with self.assertRaises(reject):
                self.validate(current=changed,
                              workflow_descriptor=binding(".github/workflows/ci.yml", changed))

        def test_workflow_descriptor_fields_and_path_closed(self):
            for key, value in (("path", "elsewhere.yml"), ("bytes", len(self.workflow) + 1),
                               ("sha256", "0" * 64), ("extra", True)):
                changed = copy.deepcopy(self.workflow_pin)
                changed[key] = value
                with self.subTest(key=key), self.assertRaises(reject):
                    self.validate(workflow_descriptor=changed)

        def test_source_descriptor_fields_and_path_closed(self):
            for key, value in (("path", "current-source.json"), ("extra", True),
                               ("bytes", 0), ("bytes", -1), ("bytes", True),
                               ("bytes", float(self.source["bytes"])),
                               ("sha256", "A" * 64)):
                changed = copy.deepcopy(self.source)
                changed[key] = value
                with self.subTest(key=key), self.assertRaises(reject):
                    self.validate(source_descriptor=changed)

        def test_plan_descriptor_fields_and_path_closed(self):
            for key, value in (("path", "direct-consumer-dispositions.json"),
                               ("bytes", len(self.plan_raw) + 1),
                               ("sha256", "0" * 64), ("extra", True)):
                changed = copy.deepcopy(self.plan_pin)
                changed[key] = value
                with self.subTest(key=key), self.assertRaises(reject):
                    self.validate(dispositions_descriptor=changed)

        def test_equal_float_descriptor_sizes_are_rejected(self):
            for name, original in (("workflow_descriptor", self.workflow_pin),
                                   ("dispositions_descriptor", self.plan_pin)):
                changed = copy.deepcopy(original)
                changed["bytes"] = float(changed["bytes"])
                with self.subTest(descriptor=name), self.assertRaises(reject):
                    self.validate(**{name: changed})

        def test_rehashed_plan_rejects_equal_numeric_types(self):
            variants = []
            count = copy.deepcopy(self.plan)
            count["entry_count"] = float(count["entry_count"])
            variants.append(count)
            line = copy.deepcopy(self.plan)
            line["entries"][0]["line"] = float(line["entries"][0]["line"])
            variants.append(line)
            for changed in variants:
                body = raw(changed)
                with self.subTest(body_sha=sha(body)), self.assertRaises(reject):
                    self.validate(current_raw=body, dispositions_descriptor=binding(
                        "published-direct-consumer-dispositions-v1.json", body))

        def test_rehashed_plan_rejects_duplicate_keys(self):
            body = b'{"entry_count": 173,' + self.plan_raw[1:]
            with self.assertRaises(reject):
                self.validate(current_raw=body, dispositions_descriptor=binding(
                    "published-direct-consumer-dispositions-v1.json", body))

        def test_rehashed_plan_cannot_omit_reorder_or_duplicate_consumer(self):
            variants = []
            omitted = copy.deepcopy(self.plan)
            omitted["entries"].pop()
            variants.append(omitted)
            reordered = copy.deepcopy(self.plan)
            reordered["entries"][0:2] = reversed(reordered["entries"][0:2])
            variants.append(reordered)
            duplicated = copy.deepcopy(self.plan)
            duplicated["entries"][-1] = copy.deepcopy(duplicated["entries"][0])
            variants.append(duplicated)
            for changed in variants:
                body = raw(changed)
                with self.subTest(body_sha=sha(body)), self.assertRaises(reject):
                    self.validate(current_raw=body, dispositions_descriptor=binding(
                        "published-direct-consumer-dispositions-v1.json", body))

        def test_rehashed_plan_cannot_change_any_original_entry_field(self):
            # Enumerate every original field, including host/profile/failure
            # obligations, rather than maintaining a selective field allowlist.
            original = self.plan["entries"][0]
            for key in original:
                changed = copy.deepcopy(self.plan)
                changed["entries"][0][key] = None
                body = raw(changed)
                with self.subTest(field=key), self.assertRaises(reject):
                    self.validate(current_raw=body, dispositions_descriptor=binding(
                        "published-direct-consumer-dispositions-v1.json", body))

        def test_rehashed_plan_cannot_claim_execution_or_stale_source(self):
            for key, value in (
                ("status", "PASS"), ("compiler_head", ci.source.HISTORICAL_SOURCE["head"]),
                ("compiler_tree", ci.source.HISTORICAL_SOURCE["tree"]),
                ("current_source_manifest_sha256", ci.M_SOURCE_SHA),
                ("entry_count", 172), ("execution_qualified", True)):
                changed = copy.deepcopy(self.plan)
                changed[key] = value
                body = raw(changed)
                with self.subTest(field=key), self.assertRaises(reject):
                    self.validate(current_raw=body, dispositions_descriptor=binding(
                        "published-direct-consumer-dispositions-v1.json", body))

    return unittest.defaultTestLoader.loadTestsFromTestCase(ExactWorkflowTransport)

