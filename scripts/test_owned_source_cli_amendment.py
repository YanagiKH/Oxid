"""Current CLI expectations are a sealed, opt-in amendment to the frozen model."""
import contextlib
import copy
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import verify_owned_source as h


SELECTION = "checked-division-v1"
IDENTIFIERS = ("scalar-unsupported-division", "scalar-unsupported-remainder")
RESULTS = dict(zip(IDENTIFIERS, (2, 1)))


class CheckedDivisionCliAmendmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.frozen = Path(cls.temporary.name) / "frozen"
        cls.manifest = h.freeze(cls.frozen)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def amendment(self, directory=None):
        return h.load_cli_amendment(SELECTION, directory or self.frozen, mode="cli")

    def item(self, identifier):
        return h.strict_json_loads((self.frozen / (identifier + ".json")).read_text())

    def test_only_two_effective_cli_expectations_change_and_frozen_bytes_stay_identical(self):
        before = {path.name: h.digest(path) for path in self.frozen.iterdir()}
        amendment = self.amendment()
        self.assertEqual(amendment["amendment_id"], SELECTION)
        self.assertEqual(amendment["data_sha256"], h.digest(h.CLI_AMENDMENT_PATH))
        self.assertEqual(amendment["scope"], "current-production-cli-only")
        self.assertIs(amendment["full_qualification"], False)
        self.assertEqual([row["id"] for row in amendment["cases"]], list(IDENTIFIERS))
        changed = []
        for entry in self.manifest["files"]:
            item = self.item(entry["id"])
            old = copy.deepcopy(item)
            effective = h.apply_cli_amendment(item, amendment)
            self.assertEqual(item, old)
            if effective != item:
                changed.append(entry["id"])
                self.assertEqual(effective["expected"], {
                    "status": "accept", "result_type": "i32",
                    "result": RESULTS[entry["id"]], "runtime_failure": None,
                })
                self.assertEqual(effective["function_count"], 5)
                self.assertEqual(effective, {**item, "expected": effective["expected"]})
                self.assertEqual(item["expected"]["stage"], "parse")
                self.assertEqual(item["expected"]["code"], "E0101")
                self.assertEqual(item["expected"]["origin"], "n0018.operator")
                self.assertEqual(item["expected"]["span"], [264, 265])
            self.assertEqual(h.apply_cli_amendment(item, None), old)
        self.assertEqual(changed, list(IDENTIFIERS))
        self.assertEqual(len(self.manifest["files"]), 431)
        self.assertEqual(h.verify_frozen(self.frozen), self.manifest)
        self.assertEqual(before, {path.name: h.digest(path) for path in self.frozen.iterdir()})

    def test_missing_extra_changed_or_mistyped_amendment_data_is_rejected(self):
        original = h.strict_json_loads(h.CLI_AMENDMENT_PATH.read_text())
        variants = []
        for mutate in (
            lambda data: data.update(cases=[]),
            lambda data: data["cases"].pop(),
            lambda data: data["cases"].append(copy.deepcopy(data["cases"][0])),
            lambda data: data.update(extra="unreviewed"),
            lambda data: data["cases"][0].update(extra="unreviewed"),
            lambda data: data["cases"][0]["effective_expected"].update(result=3),
            lambda data: data["cases"][1]["effective_expected"].update(result=True),
            lambda data: data["cases"][0]["effective_expected"].update(result=2.0),
            lambda data: data["cases"][0]["effective_expected"].update(result_type="bool"),
            lambda data: data["cases"][0]["old_expected"].update(span=[263, 265]),
            lambda data: data["cases"][0].update(source_sha256="0" * 64),
            lambda data: data["cases"][0].update(old_expectation_sha256="0" * 64),
            lambda data: data.update(scope="candidate"),
            lambda data: data.update(full_qualification=True),
        ):
            data = copy.deepcopy(original)
            mutate(data)
            variants.append(json.dumps(data))
        variants.extend(("{}", '{"amendment_id":1,"amendment_id":2}', '{"result":NaN}'))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "amendment.json"
            with mock.patch.object(h, "CLI_AMENDMENT_PATH", path):
                with self.assertRaises(OSError):
                    self.amendment()
                for data in variants:
                    path.write_text(data)
                    with self.subTest(data=data), self.assertRaises(ValueError):
                        self.amendment()
        with self.assertRaises(ValueError):
            h.load_cli_amendment("unchecked-v2", self.frozen, mode="cli")

    def test_changed_source_old_expectation_and_function_count_are_rejected(self):
        amendment = self.amendment()
        for identifier in IDENTIFIERS:
            for mutate in (
                lambda item: item.update(source_sha256="0" * 64),
                lambda item: item.update(function_count=6),
                lambda item: item.update(function_count=5.0),
                lambda item: item["expected"].update(code="E9999"),
                lambda item: item["expected"].update(origin="other.operator"),
                lambda item: item["expected"].update(span=[264, 266]),
                lambda item: item["expected"].update(span=[264.0, 265]),
            ):
                item = self.item(identifier)
                mutate(item)
                with self.subTest(identifier=identifier, item=item), self.assertRaises(ValueError):
                    h.apply_cli_amendment(item, amendment)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for identifier in IDENTIFIERS:
                for suffix in (".ox", ".json"):
                    name = identifier + suffix
                    (root / name).write_bytes((self.frozen / name).read_bytes())
            for identifier in IDENTIFIERS:
                for suffix in (".ox", ".json"):
                    path = root / (identifier + suffix)
                    original = path.read_bytes()
                    path.write_bytes(original + b" ")
                    with self.subTest(path=path), self.assertRaises(ValueError):
                        self.amendment(root)
                    path.write_bytes(original)

    def test_amendment_is_rejected_outside_explicit_current_cli_mode(self):
        for mode in ("model", "freeze", "candidate", "production"):
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, "only.*cli"):
                h.load_cli_amendment(SELECTION, self.frozen, mode=mode)
            for worker in ([], ["--worker-profile", "debug"]):
                with mock.patch.object(h, "verify_frozen") as frozen, mock.patch.object(h, "run_jobs") as jobs, contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(h.main(["--mode", mode, "--cli-amendment", SELECTION, *worker]), 1)
                    frozen.assert_not_called()
                    jobs.assert_not_called()
        with mock.patch.object(h, "verify_frozen") as frozen, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(h.main(["--cli-amendment", SELECTION]), 1)
            frozen.assert_not_called()

    def run_profile(self, root, *, bad_path=None, bad_result=None, selection=SELECTION):
        binary = root / "compiler"
        binary.write_bytes(b"test compiler identity")
        manifest = {"files": [{"id": identifier} for identifier in IDENTIFIERS]}
        def invoke(argv, *, cwd, **kwargs):
            identifier = Path(cwd).name
            value = RESULTS[identifier]
            result = {"type": "i32", "value": value}
            if bad_path == "json" and identifier == IDENTIFIERS[0]:
                result = bad_result
            operation = argv[1] if argv[0] == str(binary) else "native"
            if operation == "native" or (operation == "run" and "--message-format=json" not in argv):
                if operation == "native":
                    self.assertFalse((Path(cwd) / "source.ox").exists())
                stdout = f"{value}\n".encode()
                if bad_path == operation and identifier == IDENTIFIERS[0]:
                    stdout = b"99\n"
            else:
                payload = {"check": "functions", "run": "result", "compile": "output"}[operation]
                payload_value = 5 if operation == "check" else result
                if operation == "compile":
                    payload_value = argv[argv.index("--output") + 1]
                    Path(payload_value).write_bytes(b"\x7fELF fake test artifact")
                if bad_path == "check" and operation == "check":
                    payload_value = 4
                stdout = (json.dumps({"schema_version": 1, "edition": "typed-preview", "kind": operation + "-summary", "success": True, "errors": 0, payload: payload_value}) + "\n").encode()
            return subprocess.CompletedProcess(argv, 0, stdout, b"")
        with mock.patch.object(h, "verify_frozen", return_value=manifest), mock.patch.object(h.subprocess, "run", side_effect=invoke), contextlib.redirect_stdout(io.StringIO()):
            h.profile_worker(binary, "debug", self.frozen, root / "profile", cli_amendment=selection, mode="cli")
        return h.strict_json_loads((root / "profile/profile-result.json").read_text())

    def test_profile_uses_amended_check_json_text_and_source_free_elf_results(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_profile(Path(directory))
        self.assertEqual(result["counts"], {"cli_invocations": 8, "native_executions": 2, "unique_elf_artifacts": 2, "source_programs": 2})
        self.assertEqual(result["cli_expectation_amendment"], self.amendment())
        self.assertIs(result["full_qualification"], False)

    def test_cli_without_explicit_selection_retains_historical_rejections(self):
        self.assertIsNone(h.load_cli_amendment(None, self.frozen, mode="cli"))
        with tempfile.TemporaryDirectory() as directory, self.assertRaisesRegex(ValueError, "expected ordinary source failure"):
            self.run_profile(Path(directory), selection=None)

    def test_profile_rejects_wrong_function_counts_results_and_scalar_types(self):
        variants = [(path, None) for path in ("check", "run", "native")]
        variants.extend(("json", result) for result in (
            {"type": "i32", "value": 1}, {"type": "i32", "value": True},
            {"type": "i32", "value": 2.0}, {"type": "bool", "value": True},
        ))
        for path, result in variants:
            with self.subTest(path=path, result=result), tempfile.TemporaryDirectory() as directory, self.assertRaises(ValueError):
                self.run_profile(Path(directory), bad_path=path, bad_result=result)

    def test_cli_propagates_selection_and_retains_both_expectations_in_aggregate(self):
        self.aggregate()

    def test_cli_rejects_missing_changed_or_overqualified_worker_amendment(self):
        for defect in ("missing", "changed", "qualified"):
            with self.subTest(defect=defect):
                self.aggregate(defect)

    def aggregate(self, defect=None):
        amendment = self.amendment()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "compiler"
            binary.write_bytes(b"test compiler identity")
            binary.chmod(0o700)
            evidence = root / "evidence"
            def jobs(items, *args, **kwargs):
                self.assertEqual(len(items), 2)
                for item in items:
                    self.assertEqual(item.command[item.command.index("--mode") + 1], "cli")
                    self.assertEqual(item.command[item.command.index("--cli-amendment") + 1], SELECTION)
                    dest = evidence / item.name
                    dest.mkdir(parents=True)
                    result = {"diagnostic_selections": {}, "elf": [], "full_qualification": False,
                              "cli_expectation_amendment": copy.deepcopy(amendment)}
                    if item.name == "release":
                        if defect == "missing": result.pop("cli_expectation_amendment")
                        if defect == "changed": result["cli_expectation_amendment"]["cases"][0]["effective_expected"]["result"] = 3
                        if defect == "qualified": result["full_qualification"] = True
                    h.write_json(dest / "profile-result.json", result)
                return 0
            with mock.patch.object(h, "run_jobs", side_effect=jobs), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                code = h.main([str(binary), str(binary), "--mode", "cli", "--cli-amendment", SELECTION,
                               "--frozen", str(self.frozen), "--evidence", str(evidence)])
            self.assertEqual(code, 1 if defect else 0)
            report = evidence / "production-cli-result.json"
            if defect:
                self.assertFalse(report.exists())
            else:
                result = h.strict_json_loads(report.read_text())
                self.assertEqual(result["cli_expectation_amendment"], amendment)
                self.assertEqual(result["status"], "CLI_ELF_PASS")
                self.assertIs(result["full_qualification"], False)
                for profile in result["profiles"]:
                    self.assertEqual(profile["cli_expectation_amendment"], amendment)




class RecordCompositionCliAmendmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.frozen = Path(cls.temporary.name) / 'frozen'
        cls.manifest = h.freeze(cls.frozen)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def test_exact_two_composition_changes_extend_unchanged_division_predecessor(self):
        before = {path.name: h.digest(path) for path in self.frozen.iterdir()}
        old = h.load_cli_amendment('checked-division-v1', self.frozen, mode='cli')
        new = h.load_cli_amendment('owned-record-composition-v1', self.frozen, mode='cli')
        self.assertEqual(new['cases'][:2], old['cases'])
        self.assertEqual(new['predecessor_amendment_sha256'], old['data_sha256'])
        self.assertEqual([r['id'] for r in new['cases'][2:]], ['excluded-chained-projection', 'nested-record-field'])
        changed = []
        for row in self.manifest['files']:
            item = h.strict_json_loads((self.frozen / (row['id'] + '.json')).read_text())
            if h.apply_cli_amendment(item, old) != h.apply_cli_amendment(item, new):
                changed.append(item['id'])
        self.assertEqual(changed, ['excluded-chained-projection', 'nested-record-field'])
        chained = new['cases'][2]
        self.assertEqual(chained['effective_expected'], {
            'status': 'reject', 'stage': 'type', 'code': 'E0305',
            'message': 'intermediate field access requires a record',
            'span': [75, 80], 'span_match': 'exact', 'related': [],
        })
        source = (self.frozen / 'excluded-chained-projection.ox').read_bytes()
        self.assertEqual(source[75:80], b'other')
        self.assertEqual(chained['old_expected']['span'], [74, 75])
        self.assertEqual(new['cases'][3]['effective_expected'], {
            'status': 'accept', 'result_type': 'i32', 'result': 0, 'runtime_failure': None})
        self.assertEqual(before, {path.name: h.digest(path) for path in self.frozen.iterdir()})
        self.assertIs(new['full_qualification'], False)

    def test_composition_amendment_pin_and_explicit_mode_cannot_be_relaxed(self):
        for mode in ('model', 'freeze', 'candidate', 'production'):
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                h.load_cli_amendment('owned-record-composition-v1', self.frozen, mode=mode)
        original = h.strict_json_loads(h.COMPOSITION_CLI_AMENDMENT_PATH.read_text())
        for mutate in (lambda a: a['cases'].pop(),
                       lambda a: a['cases'][2]['effective_expected'].update(status='accept'),
                       lambda a: a['cases'][2]['effective_expected'].update(span=[74, 75]),
                       lambda a: a['cases'][3]['effective_expected'].update(result=1),
                       lambda a: a.update(full_qualification=True)):
            value = copy.deepcopy(original); mutate(value)
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'amendment.json'; path.write_text(json.dumps(value))
                with mock.patch.object(h, 'COMPOSITION_CLI_AMENDMENT_PATH', path), self.assertRaises(ValueError):
                    h.load_cli_amendment('owned-record-composition-v1', self.frozen, mode='cli')

    def test_changed_historical_composition_cases_reject(self):
        amendment = h.load_cli_amendment('owned-record-composition-v1', self.frozen, mode='cli')
        for identifier in ('excluded-chained-projection', 'nested-record-field'):
            item = h.strict_json_loads((self.frozen / (identifier + '.json')).read_text())
            for key, value in (('source_sha256', '0' * 64), ('function_count', 2),
                               ('expected', {'status': 'accept'})):
                changed = {**item, key: value}
                with self.subTest(identifier=identifier, key=key), self.assertRaises(ValueError):
                    h.apply_cli_amendment(changed, amendment)


if __name__ == "__main__":
    unittest.main()
