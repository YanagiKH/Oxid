"""Inactive, explicitly constructed controls for the published-source adapter.

This file neither imports production helpers nor discovers fixtures. A bounded,
effects-blocked caller must authenticate/load the helper bodies below, supply
the genuine retained bytes, and run BOTH factories. Unit controls use named
boundary traps and in-memory runtime doubles; they are not production admission.
Integration controls require genuine complete inputs and real consumer/verifier
callbacks. Missing integration inputs raise; there is no skip or mock fallback.

The caller must record unit/integration results and counts separately. A unit
pass alone, this test file, or a coherent test-local descriptor is never an
authority, seal, independent review, compiler result, or qualification receipt.
All test-local descriptor changes stay in memory. Only the caller's explicitly
isolated negative-case contexts may contain filesystem mutations.
"""
import copy
from contextlib import contextmanager, ExitStack, redirect_stderr
import hashlib
import io
import json
from pathlib import Path
import types
import unittest
from unittest import mock


HELPER_PINS = {'published_transition.py': '069f7610fe2ffd3e51c47b1d2ed8f6db61f774dccb1dabf6977b24472e973a45', 'published_current.py': '11f96d500198406d29e1a1e0880ade13dd8e2afd799315a08d5febcf5e889393', 'published_view.py': '629ca03dd3633df969f6ca50c6d3d675ce5f5acd70caa00a372ae6a9cb2a5aca'}

UNIT_CONTROLS = (
    "test_source_positive_prefix_stops_before_source_read",
    "test_source_seal_schema_is_exact_and_noncyclic",
    "test_source_named_descriptors_and_alias_are_exact",
    "test_source_authority_schema_is_exact",
    "test_source_published_and_historical_provenance_are_distinct",
    "test_source_manifest_provenance_and_metadata_are_preserved",
    "test_source_membership_requires_all_376_ordered_rows",
    "test_source_transition_is_exactly_main",
    "test_source_fixed_predecessor_bytes_cannot_be_rehashed",
    "test_package_positive_roster_stops_before_body_acceptance",
    "test_package_requires_old95_plus_exact_four_names",
    "test_package_duplicate_and_self_manifest_rows_rejected",
    "test_package_manifest_metadata_is_preserved",
    "test_exact_capture_equality_rejects_bool_int_and_float_aliases",
    "test_exact_descriptor_and_receipt_overlap_types_are_preserved",
    "test_assert_unchanged_rejects_equal_valued_type_mutation",
    "test_view_prefers_published_predecessor_over_older_map",
    "test_view_accepts_only_explicit_source_only_inverse_stage",
    "test_view_checks_complete_predecessor_bodies",
    "test_capture_retains_complete_old_record_and_closes_view",
    "test_capture_rejects_live_objects_and_temporary_paths",
    "test_capture_cannot_call_old_preflight_on_current_root",
    "test_unit2_uses_actual_instrumented_assembly_map",
    "test_unit2_keeps_all_original_verifier_result_fields",
    "test_unit2_does_not_suppress_original_verifier_rejection",
    "test_unit2_rejects_stale_main_lexer_or_accounting_body",
    "test_unit2_rejects_changed_assembly_or_current_manifest",
    "test_unit2_rejects_changed_prepared_accounting_binding",
    "test_failure_primary_is_written_before_validation",
    "test_failure_invalid_capture_preserves_primary_error",
    "test_failure_sidecar_write_error_cannot_mask_primary",
    "test_failure_sidecar_keyboard_interrupt_cannot_mask_primary",
    "test_failure_without_capture_keeps_original_writer",
    "test_terminal_result_provenance_rejection_writes_failure",
    "test_terminal_prepared_provenance_rejection_writes_failure",
    "test_planned_provenance_rejection_uses_original_failure_path",
    "test_successful_extension_keeps_original_receipt_fields",
)

INTEGRATION_CONTROLS = (
    "test_genuine_complete_376_inverse_and_provenance",
    "test_genuine_proof_schema_and_evidence_pins_reject_rehashes",
    "test_genuine_identity_modes_blobs_fixtures_and_includes_are_fixed",
    "test_genuine_inverse_rejects_wrong_stage_tail_and_membership",
    "test_genuine_source_body_mutations_rejected",
    "test_genuine_complete_source_membership_and_modes_rejected",
    "test_genuine_package99_preserves_old95_boundary",
    "test_genuine_shared_package_body_coherent_rehash_rejected",
    "test_genuine_capture_preserves_historical_and_accounting_chain",
    "test_genuine_capture_rejects_post_admission_type_confusion",
    "test_genuine_paired_capture_mutations_reject_canonical_mismatch",
    "test_genuine_prepared_unit2_uses_actual_assembly_and_old_verifier",
    "test_genuine_prepared_unit2_rejects_changed_main_and_assembly",
    "test_independent_consumer_accepts_exact_original_snapshot",
    "test_independent_consumer_rejects_coherent_dispatcher_manifest_tamper",
)


class SourceReadBoundary(RuntimeError):
    """Expected unit-test stop, explicitly not a successful source admission."""


class PackageBodyBoundary(RuntimeError):
    """Expected unit-test stop before authenticating unavailable member bodies."""


def _encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()


def _entry(name, raw):
    return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}


def _require(condition, reason):
    if not condition:
        raise ValueError(reason)


def _suite(case, controls, kind):
    suite = unittest.TestSuite(case(name) for name in controls)
    suite.control_kind = kind
    suite.control_names = tuple(controls)
    suite.required_control_count = len(controls)
    return suite


def _check_retained_bytes(t, c, source_raw, package_raw, runner_raw):
    _require(all(type(raw) is bytes for raw in (source_raw, package_raw, runner_raw)),
             "explicit genuine retained byte arguments are required")
    c.adapters.verify(source_raw, {"path": t.PREDECESSOR_NAME,
                                  "bytes": t.PREDECESSOR_BYTES,
                                  "sha256": t.PREDECESSOR_SHA}, t.PREDECESSOR_NAME)
    c.adapters.verify(package_raw, c.HISTORICAL_PACKAGE_MANIFEST, c.PACKAGE_MANIFEST)
    _require(hashlib.sha256(runner_raw).hexdigest()
             == c.retained_current.source_transition.RETAINED_RUNNER_SHA,
             "changed genuine retained runtime bytes")


def _test_seal(t):
    # Test-only descriptors. This object is never written or called a fixed
    # production seal; each unit boundary being isolated is stated explicitly.
    fields = {
        "source_manifest": t.CURRENT_NAME, "authority": t.AUTHORITY_NAME,
        "patch": t.PATCH_NAME, "source_proof": t.PROOF_NAME,
        "predecessor_view": "tests/qualification/published_formatter_current/predecessor-view-v1.json",
    }
    return {"schema": "oxid-published-main-source-seal-v1", **{
        field: _entry(name, b"explicit unit-test descriptor") for field, name in fields.items()}}


def _source_prefix(t, source_raw):
    """Valid only up to the trapped read_inputs call; no source proof is faked."""
    predecessor = json.loads(source_raw)
    current = copy.deepcopy(predecessor)
    current.update(reviewed_source_head=t.SOURCE_CHECKPOINT["head"],
                   source_only_tree=t.SOURCE_CHECKPOINT["tree"],
                   published_main_head=t.PUBLISHED_MAIN["head"],
                   published_main_tree=t.PUBLISHED_MAIN["tree"],
                   published_source_predecessor_sha256=t.PREDECESSOR_SHA)
    current["files"] = [_entry("src/main.rs", b"synthetic unread current main")
                        if row["path"] == "src/main.rs" else row for row in current["files"]]
    proof = {"unavailable": "unit control stops before source read"}
    bodies = {t.PREDECESSOR_NAME: source_raw, t.CURRENT_NAME: _encoded(current),
              t.PROOF_NAME: _encoded(proof), t.PATCH_NAME: b"not an executable fixture patch\n"}
    bodies["current-source.json"] = bodies[t.CURRENT_NAME]
    seal = _test_seal(t)
    for field, name in (("source_manifest", t.CURRENT_NAME), ("source_proof", t.PROOF_NAME),
                        ("patch", t.PATCH_NAME)):
        seal[field] = _entry(name, bodies[name])
    authority = {key: [] for key in t.AUTHORITY_FIELDS}
    authority.update(
        schema="oxid-published-main-source-transition-v1", recipe=t.RECIPE,
        published_main=copy.deepcopy(t.PUBLISHED_MAIN),
        source_checkpoint=copy.deepcopy(t.SOURCE_CHECKPOINT),
        retained_historical_source=copy.deepcopy(t.HISTORICAL_SOURCE),
        predecessor_source_manifest={"path": t.PREDECESSOR_NAME,
                                     "bytes": t.PREDECESSOR_BYTES, "sha256": t.PREDECESSOR_SHA},
        current_source_manifest=seal["source_manifest"], transition_patch=seal["patch"],
        source_proof=seal["source_proof"], transition_paths=["src/main.rs"],
        current_source_members=376, predecessor_source_members=376,
        compiler_source_members=288, compiler_bodies=291,
        source_only=True, execution_qualified=False)
    bodies[t.AUTHORITY_NAME] = _encoded(authority)
    seal["authority"] = _entry(t.AUTHORITY_NAME, bodies[t.AUTHORITY_NAME])
    return bodies, seal


def _proposal(t, bodies, seal, field, mutate):
    """Coherent in-memory negative proposal; never generates a production seal."""
    bodies, seal = copy.deepcopy(bodies), copy.deepcopy(seal)
    names = {"source_manifest": t.CURRENT_NAME, "authority": t.AUTHORITY_NAME,
             "source_proof": t.PROOF_NAME}
    name = names[field]
    value = json.loads(bodies[name])
    mutate(value)
    bodies[name] = _encoded(value)
    seal[field] = _entry(name, bodies[name])
    if field == "source_manifest":
        bodies["current-source.json"] = bodies[name]
    if field != "authority":
        authority = json.loads(bodies[t.AUTHORITY_NAME])
        authority["current_source_manifest" if field == "source_manifest" else "source_proof"] = seal[field]
        bodies[t.AUTHORITY_NAME] = _encoded(authority)
        seal["authority"] = _entry(t.AUTHORITY_NAME, bodies[t.AUTHORITY_NAME])
    return bodies, seal


def _package_prefix(t, c, package_raw):
    old = json.loads(package_raw)
    manifest = copy.deepcopy(old)
    additions = {name: b"unread synthetic artifact: " + name.encode()
                 for _, name in c.CURRENT_SOURCE_ARTIFACTS}
    manifest["files"] = sorted(old["files"] + [_entry(n, b) for n, b in additions.items()],
                               key=lambda row: row["path"])
    bodies = {row["path"]: b"body not supplied to this roster-only unit control"
              for row in old["files"]}
    bodies.update(additions)
    seal = _test_seal(t)
    for field, name in c.CURRENT_SOURCE_ARTIFACTS:
        seal[field] = _entry(name, bodies[name])
    return bodies, _encoded(manifest), seal


def _synthetic_source(old_source_raw):
    names = [row["path"] for row in json.loads(old_source_raw)["files"]]
    inputs = {name: ("synthetic source for " + name + "\n").encode() for name in names}
    source = {"files": [_entry(name, body) for name, body in sorted(inputs.items())]}
    return inputs, source


class _BytesFile:
    def __init__(self, raw):
        self.raw = raw

    def read_bytes(self):
        return self.raw


def _runtime_preflight(repo, package=None):
    return _dispatch("old_preflight", repo, package)


def _runtime_archived(output, captured):
    return _dispatch("old_archived", output, captured)


def _runtime_prepare(output, captured):
    return _dispatch("old_prepare", output, captured)


def _runtime_verify(output, captured, seam, prepare_only):
    return _dispatch("old_verify", output, captured, seam, prepare_only)


def _runtime_assert(repo, captured, package=None):
    return _dispatch("old_assert", repo, captured, package)


def _runtime_write(path, value):
    return _dispatch("old_write", path, value)


def _runtime_main():
    return _dispatch("old_main")


def _runtime_controls(repo, output, captured, seam):
    return _dispatch("old_controls", repo, output, captured, seam)


class _RuntimeDouble:
    """In-memory call-order/association double, never the retained verifier."""
    def __init__(self, c, package):
        self.events, self.writes, self.files, self.json_files = [], {}, {}, {}
        self.sidecar_error = None
        self.verifier_error = None
        self.verifier_result = {"status": "prepared-current-unit2", "semantic_pass": False,
                                "original_evidence": {"untouched": [1, 2, 3]}}
        self.c = c
        self.runtime = types.ModuleType("explicit_test_byte_runtime_double")
        runtime = self.runtime
        runtime.PACKAGE = c.retained_current.retained_package(package)
        runtime.CURRENT_SOURCE_SHA = c.retained_current.source_transition.PREDECESSOR_SHA
        runtime.CURRENT_SOURCE_BYTES = c.retained_current.source_transition.PREDECESSOR_BYTES
        runtime._dispatch = self.dispatch
        for name, function in {
            "preflight": _runtime_preflight, "prepare_archived": _runtime_archived,
            "prepare_unit2": _runtime_prepare, "verify_unit2_result": _runtime_verify,
            "assert_unchanged": _runtime_assert, "write_json": _runtime_write,
            "main": _runtime_main, "run_unit2_observer_controls": _runtime_controls,
        }.items():
            setattr(runtime, name, types.FunctionType(function.__code__, vars(runtime), name))
        runtime.require = c.adapters.need
        runtime.regular = lambda root, name: _BytesFile(self.files[str(Path(root) / name)])
        runtime.read_json = lambda path: copy.deepcopy(self.json_files[str(path)])
        runtime.check_entries = self.check_entries

    def dispatch(self, name, *args):
        self.events.append(name if name != "old_write" else "write:" + Path(args[0]).name)
        if name == "old_write":
            path, value = args
            if str(path).endswith(".published-provenance.json") and self.sidecar_error:
                raise self.sidecar_error
            self.writes[str(path)] = copy.deepcopy(value)
            return
        if name == "old_verify":
            if self.verifier_error:
                raise self.verifier_error
            return copy.deepcopy(self.verifier_result)
        if name == "old_prepare":
            return {self.c.RETAINED_ACCOUNTING_KEY: copy.deepcopy(args[1][self.c.RETAINED_ACCOUNTING_KEY])}
        return {"original_call": name}

    def check_entries(self, root, rows, exact=False):
        self.events.append("check_actual_assembly")
        root = str(root) + "/"
        actual = {name[len(root):]: body for name, body in self.files.items() if name.startswith(root)}
        if exact:
            self.c.adapters.need(set(actual) == {row["path"] for row in rows}, "assembly membership")
        for row in rows:
            self.c.adapters.verify(actual[row["path"]], row, row["path"])


def _unit2_capture(c):
    inputs = {"src/main.rs": b"current main", "src/frontend/lexer.rs": b"current lexer",
              "src/frontend/parser.rs": b"ordinary parser"}
    for name in c.retained_current.source_transition.ACCOUNTING_PATHS:
        inputs[name] = name.encode()
    prior = {**inputs, "src/main.rs": b"published predecessor main"}
    byte = {**prior, "src/frontend/lexer.rs": b"byte-storage lexer"}
    manifest = _encoded({"files": [_entry(n, b) for n, b in sorted(inputs.items())]})
    binding = {"current_source": _entry("current-source.json", manifest),
               "source_dependencies": [_entry(n, inputs[n]) for n in
                                       c.retained_current.source_transition.ACCOUNTING_PATHS]}
    return {"inputs": inputs, "package_bytes": {"current-source.json": manifest},
            "published_predecessor": {"inputs": prior, "byte_storage_inputs": byte},
            c.RETAINED_ACCOUNTING_KEY: {"genuine_stage_in_real_fixture": "byte"},
            c.LEXER_ACCOUNTING_KEY: {"genuine_stage_in_real_fixture": "lexer"},
            c.CURRENT_ACCOUNTING_KEY: binding}


@contextmanager
def _installed_double(t, c, runner_raw, *, capture=None, validation_error=None,
                      sidecar_error=None, verifier_error=None):
    package, repo = Path("/explicit-unit-package/current"), Path("/explicit-unit-repo")
    capture = copy.deepcopy(capture if capture is not None else _unit2_capture(c))
    double = _RuntimeDouble(c, package)
    double.sidecar_error, double.verifier_error = sidecar_error, verifier_error
    extension = {"published_source_provenance": {"unit_double_only": True}}

    def validate(*args):
        double.events.append("validate")
        if validation_error is not None:
            raise validation_error
        return copy.deepcopy(capture[c.CURRENT_ACCOUNTING_KEY])

    with ExitStack() as stack:
        stack.enter_context(mock.patch.object(c, "_regular", return_value=_BytesFile(runner_raw)))
        stack.enter_context(mock.patch.object(c, "preflight", return_value=capture))
        stack.enter_context(mock.patch.object(c, "validate_unit2_accounting", side_effect=validate))
        stack.enter_context(mock.patch.object(c, "outer_receipt", return_value=extension))
        runtime = c.install(double.runtime, package, _test_seal(t))
        runtime.preflight(repo)
        yield double, runtime, capture


def _populate_assembly(double, capture, output):
    c = double.c
    source = output / "unit2/run/source"
    materialized = dict(capture["inputs"])
    materialized["src/frontend/parser.rs"] += b"\nactual admitted instrumentation"
    for name, raw in materialized.items():
        double.files[str(source / name)] = raw
    selected = capture["package_bytes"]["current-source.json"]
    double.files[str(output / "unit2/derived-package/source-inputs.json")] = selected
    double.json_files[str(output / "unit2/run/assembly.json")] = {
        "source_inputs_sha256": hashlib.sha256(selected).hexdigest(),
        "files": [_entry(n, b) for n, b in sorted(materialized.items())],
    }
    return {key: copy.deepcopy(capture[key]) for key in
            (c.RETAINED_ACCOUNTING_KEY, c.LEXER_ACCOUNTING_KEY, c.CURRENT_ACCOUNTING_KEY)}


def build_unit_suite(transition, current, view, *, retained_source_manifest_bytes,
                     retained_package_manifest_bytes, retained_runner_bytes):
    """Build synthetic/mechanical controls with explicit authentic retained pins."""
    t, c, v = transition, current, view
    source_raw, package_raw, runner_raw = (retained_source_manifest_bytes,
                                         retained_package_manifest_bytes, retained_runner_bytes)
    _check_retained_bytes(t, c, source_raw, package_raw, runner_raw)

    class UnitControls(unittest.TestCase):
        def source_call(self, bodies, seal, rejected=True):
            with mock.patch.object(t.retained, "read_inputs", side_effect=SourceReadBoundary()) as read:
                with self.assertRaises(c.adapters.Reject if rejected else SourceReadBoundary):
                    t.admit(Path("/source-read-must-be-trapped"), bodies, seal, object())
                self.assertEqual(read.call_count, 0 if rejected else 1)

        def package_call(self, bodies, raw, seal, rejected=True):
            real_verify = c._verify

            def boundary(body, descriptor, name):
                if name == c.PACKAGE_MANIFEST:
                    return real_verify(body, descriptor, name)
                raise PackageBodyBoundary()

            with mock.patch.object(c, "_verify", side_effect=boundary):
                with self.assertRaises(c.adapters.Reject if rejected else PackageBodyBoundary):
                    c._current_package_map(bodies, raw, package_raw, seal)

        def test_source_positive_prefix_stops_before_source_read(self):
            self.source_call(*_source_prefix(t, source_raw), rejected=False)

        def test_source_seal_schema_is_exact_and_noncyclic(self):
            bodies, baseline = _source_prefix(t, source_raw)
            variants = [dict(baseline, schema="wrong"), dict(baseline, extra=True),
                        dict(baseline, package_manifest=_entry("package-manifest.json", b"cycle"))]
            for key in baseline:
                changed = copy.deepcopy(baseline)
                del changed[key]
                variants.append(changed)
            for seal in variants:
                with self.subTest(seal_fields=sorted(seal)):
                    self.source_call(bodies, seal)

        def test_source_named_descriptors_and_alias_are_exact(self):
            bodies, seal = _source_prefix(t, source_raw)
            changed = copy.deepcopy(seal)
            changed["source_manifest"]["path"] = "wrong-source.json"
            self.source_call(bodies, changed)
            changed_bodies = dict(bodies, **{"current-source.json": b"wrong alias"})
            self.source_call(changed_bodies, seal)
            changed_bodies = dict(bodies)
            changed_bodies[t.CURRENT_NAME] += b" "
            self.source_call(changed_bodies, seal)

        def test_source_authority_schema_is_exact(self):
            bodies, seal = _source_prefix(t, source_raw)
            mutations = [lambda value: value.update(extra=True),
                         lambda value: value.pop("recipe"),
                         lambda value: value.update(schema="wrong", recipe="wrong")]
            for mutate in mutations:
                self.source_call(*_proposal(t, bodies, seal, "authority", mutate))

        def test_source_published_and_historical_provenance_are_distinct(self):
            bodies, seal = _source_prefix(t, source_raw)
            mutations = [
                lambda x: x.update(published_main=copy.deepcopy(t.HISTORICAL_SOURCE)),
                lambda x: x.update(retained_historical_source=copy.deepcopy(t.PUBLISHED_MAIN)),
                lambda x: x["source_checkpoint"].update(parent="0" * 40),
                lambda x: x.update(source_only=False),
                lambda x: x.update(execution_qualified=True),
                lambda x: x["predecessor_source_manifest"].update(sha256="0" * 64),
                lambda x: x["transition_patch"].update(bytes=1),
            ]
            for index, mutate in enumerate(mutations):
                with self.subTest(index=index):
                    self.source_call(*_proposal(t, bodies, seal, "authority", mutate))

        def test_source_manifest_provenance_and_metadata_are_preserved(self):
            bodies, seal = _source_prefix(t, source_raw)
            mutations = [
                lambda x: x.update(reviewed_source_head=t.PUBLISHED_MAIN["head"]),
                lambda x: x.update(source_only_tree=t.PUBLISHED_MAIN["tree"]),
                lambda x: x.update(published_main_tree="0" * 40),
                lambda x: x.update(published_source_predecessor_sha256="0" * 64),
                lambda x: x.update(unreviewed_field=True),
                lambda x: x.pop("published_main_head"),
            ]
            for index, mutate in enumerate(mutations):
                with self.subTest(index=index):
                    # Missing mandatory fields fail closed as KeyError; they
                    # must not be counted as a successful source-read boundary.
                    changed = _proposal(t, bodies, seal, "source_manifest", mutate)
                    with mock.patch.object(t.retained, "read_inputs", side_effect=SourceReadBoundary()) as read:
                        with self.assertRaises((c.adapters.Reject, KeyError)):
                            t.admit(Path("/trapped"), *changed, object())
                        read.assert_not_called()

        def test_source_membership_requires_all_376_ordered_rows(self):
            bodies, seal = _source_prefix(t, source_raw)
            def rename(x):
                x["files"][0]["path"] = "a-renamed-input"
                x["files"].sort(key=lambda row: row["path"])
            mutations = [lambda x: x["files"].pop(),
                         lambda x: x["files"].append(_entry("zz-extra", b"x")),
                         lambda x: x["files"].reverse(),
                         lambda x: x["files"].append(copy.deepcopy(x["files"][-1])), rename]
            for index, mutate in enumerate(mutations):
                with self.subTest(index=index):
                    self.source_call(*_proposal(t, bodies, seal, "source_manifest", mutate))

        def test_source_transition_is_exactly_main(self):
            bodies, seal = _source_prefix(t, source_raw)
            for paths in ([], ["src/frontend/lexer.rs"], ["src/main.rs", "src/main.rs"]):
                self.source_call(*_proposal(t, bodies, seal, "authority",
                                            lambda x, paths=paths: x.update(transition_paths=paths)))

        def test_source_fixed_predecessor_bytes_cannot_be_rehashed(self):
            bodies, seal = _source_prefix(t, source_raw)
            bodies[t.PREDECESSOR_NAME] += b" "
            self.source_call(bodies, seal)

        def test_package_positive_roster_stops_before_body_acceptance(self):
            bodies, raw, seal = _package_prefix(t, c, package_raw)
            self.assertEqual(len(json.loads(raw)["files"]), 99)
            self.package_call(bodies, raw, seal, rejected=False)

        def test_package_requires_old95_plus_exact_four_names(self):
            bodies, raw, seal = _package_prefix(t, c, package_raw)
            for kind in ("missing", "extra", "rename"):
                changed, manifest = dict(bodies), json.loads(raw)
                if kind == "missing":
                    name = c.CURRENT_SOURCE_ARTIFACTS[0][1]
                    del changed[name]
                    manifest["files"] = [r for r in manifest["files"] if r["path"] != name]
                else:
                    if kind == "rename":
                        old = manifest["files"].pop(0)["path"]
                        del changed[old]
                    changed["unapproved.py"] = b"arbitrary executable"
                    manifest["files"].append(_entry("unapproved.py", changed["unapproved.py"]))
                    manifest["files"].sort(key=lambda row: row["path"])
                self.package_call(changed, _encoded(manifest), seal)

        def test_package_duplicate_and_self_manifest_rows_rejected(self):
            bodies, raw, seal = _package_prefix(t, c, package_raw)
            for row in (json.loads(raw)["files"][0], _entry(c.PACKAGE_MANIFEST, raw)):
                manifest = json.loads(raw)
                manifest["files"].append(row)
                manifest["files"].sort(key=lambda item: item["path"])
                self.package_call(bodies, _encoded(manifest), seal)

        def test_package_manifest_metadata_is_preserved(self):
            bodies, raw, seal = _package_prefix(t, c, package_raw)
            manifest = json.loads(raw)
            manifest["unreviewed_metadata"] = True
            self.package_call(bodies, _encoded(manifest), seal)

        def test_exact_capture_equality_rejects_bool_int_and_float_aliases(self):
            for actual, expected in ((1, True), (0, False), (1.0, 1),
                                     ({"nested": [1]}, {"nested": [True]}),
                                     ({"nested": [0]}, {"nested": [False]}),
                                     ({"nested": [{"bytes": 1.0}]}, {"nested": [{"bytes": 1}]}),
                                     ((1,), (True,)), ([1], (1,))):
                with self.subTest(actual=actual, expected=expected):
                    self.assertFalse(c.same(actual, expected))
            self.assertTrue(c.same({"nested": [True, False, 1, 1.0]},
                                   {"nested": [True, False, 1, 1.0]}))

        def test_exact_descriptor_and_receipt_overlap_types_are_preserved(self):
            descriptor = _entry("one-byte", b"x")
            descriptor["bytes"] = 1.0
            with self.assertRaises(c.adapters.Reject):
                c._verify(b"x", descriptor, "one-byte")
            for actual, expected in ((1, True), (0, False), (1.0, 1)):
                with self.assertRaises(c.adapters.Reject):
                    c._extend_receipt({"identity": actual}, {"identity": expected})

        def test_assert_unchanged_rejects_equal_valued_type_mutation(self):
            capture = _unit2_capture(c)
            capture["exact_bool"] = True
            capture["exact_zero"] = False
            capture["exact_integer"] = 7
            for key, replacement in (("exact_bool", 1), ("exact_zero", 0), ("exact_integer", 7.0)):
                with _installed_double(t, c, runner_raw, capture=capture) as (d, runtime, admitted):
                    changed = copy.deepcopy(admitted)
                    changed[key] = replacement
                    with self.assertRaises(c.adapters.Reject):
                        runtime.assert_unchanged(Path("/explicit-unit-repo"), changed)

        def test_view_prefers_published_predecessor_over_older_map(self):
            inputs, source = _synthetic_source(source_raw)
            capture = {"published_predecessor": {"inputs": inputs, "current": source},
                       "predecessor_inputs": {"wrong-old-stage": b"old"}}
            self.assertEqual(v._old_inputs(capture), inputs)

        def test_view_accepts_only_explicit_source_only_inverse_stage(self):
            inputs, source = _synthetic_source(source_raw)
            admitted = {"predecessor_inputs": inputs, "predecessor": source,
                        "source_only": True, "execution_qualified": False}
            self.assertEqual(v._old_inputs(admitted), inputs)
            for field, value in (("source_only", False), ("execution_qualified", True)):
                changed = dict(admitted, **{field: value})
                with self.assertRaises(c.adapters.Reject):
                    v._old_inputs(changed)

        def test_view_checks_complete_predecessor_bodies(self):
            inputs, source = _synthetic_source(source_raw)
            for kind in ("missing", "changed", "nonbytes"):
                changed = dict(inputs)
                first = sorted(changed)[0]
                if kind == "missing":
                    del changed[first]
                else:
                    changed[first] = b"changed" if kind == "changed" else "not bytes"
                with self.assertRaises(c.adapters.Reject):
                    v._old_inputs({"published_predecessor": {"inputs": changed, "current": source}})

        def capture_case(self):
            inputs, source = _synthetic_source(source_raw)
            historical = {"current": source, "inputs": inputs,
                          "package_bytes": {"old": b"complete historical test record"},
                          "package_manifest": package_raw,
                          "predecessor_inputs": {"older-stage": b"must remain older"},
                          "byte_storage_source": {"identity": "byte-stage"},
                          "lexer_reservation_source": {"identity": "lexer-stage"},
                          "archived": {str(i): b"archived" for i in range(117)},
                          "extra_old_receipt": {"untouched": [1, 2, 3]}}
            admitted = {"current": {"identity": "C"}, "inputs": dict(inputs, **{"src/main.rs": b"C"}),
                        "predecessor": source, "predecessor_inputs": inputs,
                        "authority": {"unit_boundary_double": True}, "source_proof": {},
                        "transition_touched": ["src/main.rs"]}
            events = []
            @contextmanager
            def predecessor(repo, admitted=None, seal=None):
                events.append("view-open")
                try:
                    yield Path("/explicit-temporary-M-view")
                finally:
                    events.append("view-closed")
            def old_preflight(repo, package):
                self.assertEqual(repo, Path("/explicit-temporary-M-view"))
                self.assertEqual(package, repo / c.PACKAGE_PATH)
                events.append("old-preflight")
                return historical
            with ExitStack() as stack:
                stack.enter_context(mock.patch.object(c, "_current_package", return_value=(b"current manifest", {"new": b"package"})))
                stack.enter_context(mock.patch.object(c.retained_current, "capture_retained", return_value=(None, None, object())))
                stack.enter_context(mock.patch.object(c.published_transition, "admit", return_value=admitted))
                stack.enter_context(mock.patch.object(c, "predecessor_view", side_effect=predecessor))
                stack.enter_context(mock.patch.object(c, "_regular", return_value=_BytesFile(package_raw)))
                stack.enter_context(mock.patch.object(c.retained_current, "preflight", side_effect=old_preflight))
                def binding(captured, package, seal):
                    self.assertEqual(events[-1], "view-closed")
                    return {"unit_double_only": True}
                stack.enter_context(mock.patch.object(c, "unit2_accounting_binding", side_effect=binding))
                result = c.preflight(Path("/explicit-current-C-root"), Path("/explicit-package"), _test_seal(t))
            return historical, admitted, result, events

        def test_capture_retains_complete_old_record_and_closes_view(self):
            historical, admitted, result, events = self.capture_case()
            self.assertEqual(result["published_predecessor"], historical)
            self.assertEqual(result["current"], admitted["current"])
            self.assertEqual(result["inputs"], admitted["inputs"])
            for name in set(historical) - c.REPLACED_CAPTURE_FIELDS:
                self.assertEqual(result[name], historical[name])
            self.assertEqual(events, ["view-open", "old-preflight", "view-closed"])
            self.assertEqual(result["published_package_manifest"], _entry(c.PACKAGE_MANIFEST, b"current manifest"))

        def test_capture_rejects_live_objects_and_temporary_paths(self):
            for value in (types.ModuleType("live"), Path("/temporary"), lambda: None):
                with self.assertRaises(c.adapters.Reject):
                    c._pure({"bad": value})
            with self.assertRaises(c.adapters.Reject):
                c._pure({"path": "/explicit-temporary-M-view/leak"}, "/explicit-temporary-M-view")

        def test_capture_cannot_call_old_preflight_on_current_root(self):
            self.capture_case()  # The old-preflight double checks both exact M paths.

        def unit2_case(self, mutate=None, verifier_error=None):
            output = Path("/explicit-unit-output")
            with _installed_double(t, c, runner_raw, verifier_error=verifier_error) as (double, runtime, captured):
                seam = _populate_assembly(double, captured, output)
                if mutate:
                    mutate(double, captured, seam, output)
                result = runtime.verify_unit2_result(output, captured, seam, True)
                return double, captured, result

        def test_unit2_uses_actual_instrumented_assembly_map(self):
            double, captured, result = self.unit2_case()
            path = "/explicit-unit-output/unit2/run/source/src/frontend/parser.rs"
            self.assertNotEqual(double.files[path], captured["inputs"]["src/frontend/parser.rs"])
            self.assertIn("check_actual_assembly", double.events)
            self.assertLess(double.events.index("old_verify"), double.events.index("check_actual_assembly"))
            self.assertEqual(result[c.CURRENT_ACCOUNTING_KEY], captured[c.CURRENT_ACCOUNTING_KEY])

        def test_unit2_keeps_all_original_verifier_result_fields(self):
            double, captured, result = self.unit2_case()
            for name, value in double.verifier_result.items():
                self.assertEqual(result[name], value)
            self.assertEqual(result[c.LEXER_ACCOUNTING_KEY], captured[c.LEXER_ACCOUNTING_KEY])

        def test_unit2_does_not_suppress_original_verifier_rejection(self):
            with self.assertRaisesRegex(c.adapters.Reject, "original verifier rejected"):
                self.unit2_case(verifier_error=c.adapters.Reject("original verifier rejected"))

        def test_unit2_rejects_stale_main_lexer_or_accounting_body(self):
            names = ["src/main.rs", "src/frontend/lexer.rs", *c.retained_current.source_transition.ACCOUNTING_PATHS]
            for name in names:
                def mutate(d, captured, seam, output, name=name):
                    key = str(output / "unit2/run/source" / name)
                    d.files[key] = b"coherently rehashed stale materialized body"
                    assembly = d.json_files[str(output / "unit2/run/assembly.json")]
                    assembly["files"] = [_entry(name, d.files[key]) if row["path"] == name else row
                                         for row in assembly["files"]]
                with self.subTest(path=name), self.assertRaises(c.adapters.Reject):
                    self.unit2_case(mutate)

        def test_unit2_rejects_changed_assembly_or_current_manifest(self):
            def assembly(d, captured, seam, output):
                d.files[str(output / "unit2/run/source/src/frontend/parser.rs")] += b"tamper"
            def manifest(d, captured, seam, output):
                d.files[str(output / "unit2/derived-package/source-inputs.json")] += b" "
            def identity(d, captured, seam, output):
                d.json_files[str(output / "unit2/run/assembly.json")]["source_inputs_sha256"] = "0" * 64
            for mutate in (assembly, manifest, identity):
                with self.assertRaises(c.adapters.Reject):
                    self.unit2_case(mutate)

        def test_unit2_rejects_changed_prepared_accounting_binding(self):
            for key in (c.RETAINED_ACCOUNTING_KEY, c.LEXER_ACCOUNTING_KEY, c.CURRENT_ACCOUNTING_KEY):
                def mutate(d, captured, seam, output, key=key):
                    seam[key] = {"changed": True}
                with self.assertRaises(c.adapters.Reject):
                    self.unit2_case(mutate)
            def retype(d, captured, seam, output):
                row = seam[c.CURRENT_ACCOUNTING_KEY]["source_dependencies"][0]
                row["bytes"] = float(row["bytes"])
            with self.assertRaises(c.adapters.Reject):
                self.unit2_case(retype)

        def failure(self, **kwargs):
            return _installed_double(t, c, runner_raw, **kwargs)

        def test_failure_primary_is_written_before_validation(self):
            value = {"schema": "oxid-current-archive-binding-v1", "status": "failed",
                     "semantic_pass": False, "error": "original child exited 17"}
            with self.failure() as (d, runtime, captured):
                runtime.write_json(Path("/output/failure.json"), value)
                self.assertEqual(d.events[:2], ["write:failure.json", "validate"])
                self.assertEqual(d.writes["/output/failure.json"], value)
                self.assertEqual(d.writes["/output/failure.json.published-provenance.json"]["provenance_status"], "validated")

        def test_failure_invalid_capture_preserves_primary_error(self):
            value = {"schema": "oxid-current-archive-binding-v1", "status": "failed",
                     "semantic_pass": False, "error": "original child failure"}
            with self.failure(validation_error=c.adapters.Reject("invalid captured identity")) as (d, runtime, captured):
                runtime.write_json(Path("/output/failure.json"), value)
                self.assertEqual(d.writes["/output/failure.json"], value)
                sidecar = d.writes["/output/failure.json.published-provenance.json"]
                self.assertEqual(sidecar["primary_error"], value["error"])
                self.assertEqual(sidecar["provenance_status"], "rejected")
                self.assertIn("invalid captured identity", sidecar["provenance_error"])

        def failed_sidecar(self, error):
            value = {"schema": "oxid-current-archive-binding-v1", "status": "failed", "error": "original"}
            with self.failure(validation_error=c.adapters.Reject("secondary"), sidecar_error=error) as (d, runtime, captured):
                with redirect_stderr(io.StringIO()):
                    runtime.write_json(Path("/output/failure.json"), value)
                self.assertEqual(d.writes, {"/output/failure.json": value})
                self.assertEqual(d.events[0], "write:failure.json")

        def test_failure_sidecar_write_error_cannot_mask_primary(self):
            self.failed_sidecar(OSError("sidecar write denied"))

        def test_failure_sidecar_keyboard_interrupt_cannot_mask_primary(self):
            self.failed_sidecar(KeyboardInterrupt())

        def test_failure_without_capture_keeps_original_writer(self):
            value = {"schema": "oxid-current-archive-binding-v1", "status": "failed", "error": "preflight"}
            with self.failure() as (d, runtime, captured):
                with mock.patch.object(c, "preflight", side_effect=c.adapters.Reject("preflight")):
                    with self.assertRaises(c.adapters.Reject):
                        runtime.preflight(Path("/repo"))
                runtime.write_json(Path("/output/failure.json"), value)
                self.assertNotIn("validate", d.events)
                self.assertEqual(d.writes, {"/output/failure.json": value})

        def terminal_rejection(self, name, status, semantic_pass):
            value = {"schema": "oxid-current-archive-binding-v1", "status": status,
                     "semantic_pass": semantic_pass, "original_evidence": {"kept": True}}
            with self.failure(validation_error=c.adapters.Reject("terminal provenance")) as (d, runtime, captured):
                with self.assertRaisesRegex(c.adapters.Reject, "terminal provenance"):
                    runtime.write_json(Path("/output") / name, value)
                self.assertNotIn(str(Path("/output") / name), d.writes)
                failed = d.writes["/output/failure.json"]
                self.assertEqual(failed["status"], "failed")
                self.assertIs(failed["semantic_pass"], False)
                self.assertEqual(failed["attempted_terminal_receipt"], value)
                self.assertEqual(failed["attempted_terminal_receipt_name"], name)
                self.assertIn("terminal provenance", failed["published_source_provenance_error"])

        def test_terminal_result_provenance_rejection_writes_failure(self):
            self.terminal_rejection("result.json", "passed-current-unit2", True)

        def test_terminal_prepared_provenance_rejection_writes_failure(self):
            self.terminal_rejection("prepared.json", "prepared-current-unit2", False)

        def test_planned_provenance_rejection_uses_original_failure_path(self):
            with self.failure(validation_error=c.adapters.Reject("planned provenance")) as (d, runtime, captured):
                with self.assertRaises(c.adapters.Reject):
                    runtime.write_json(Path("/output/plan.json"),
                                       {"schema": "oxid-current-archive-binding-v1", "status": "planned"})
                self.assertEqual(d.writes, {})

        def test_successful_extension_keeps_original_receipt_fields(self):
            value = {"schema": "oxid-current-archive-binding-v1", "status": "verified-inputs",
                     "semantic_pass": False, "compiler_executions": 0, "evidence": ["original"]}
            with self.failure() as (d, runtime, captured):
                runtime.write_json(Path("/output/prepared.json"), value)
                actual = d.writes["/output/prepared.json"]
                self.assertEqual({key: actual[key] for key in value}, value)
                self.assertIn("published_source_provenance", actual)
                self.assertNotIn("/output/failure.json", d.writes)

    return _suite(UnitControls, UNIT_CONTROLS, "unit-boundary-controls-only")


def build_integration_suite(transition, current, view, *, fixture,
                            isolated_repo_case, prepared_unit2_case, consumer_boundary):
    """Require genuine fixtures and separately authenticated executable callbacks.

    fixture requires repo/package Paths, fixed seal, exact package_bytes and raw
    package_manifest, complete current_inputs/predecessor_inputs, and public_api.
    Its provenance must be independently admitted by the bounded outer caller.

    isolated_repo_case(spec) is a context manager yielding a NEW isolated repo.
    It supports only replace/remove/add/symlink/directory/mode mutations listed
    below, starting from the genuine current fixture. It must never mutate repo.

    prepared_unit2_case() yields NEW isolated preparation evidence with a fresh,
    pinned byte runtime, output, full capture, and seam. It must use genuine
    unchanged preparation/assembly code. The pinned --prepare-only target path
    contains no compiler or network calls. This is structural control-flow
    exclusion in ordinary trusted CI, not an OS sandbox or startup attestation.

    consumer_boundary requires verify(package_bytes, package_manifest), a narrow
    rejection_type, implementation_bytes, and independently reviewed identity.
    verify must invoke the REAL final CI/adapter pre-import boundary, not a mock
    or these helpers' manifest consistency checks. Missing boundary is a blocker.
    """
    t, c, v = transition, current, view
    required = {"repo", "package", "seal", "package_bytes", "package_manifest",
                "current_inputs", "predecessor_inputs", "public_api"}
    _require(type(fixture) is dict and required <= set(fixture), "missing genuine integration fixture fields")
    _require(callable(isolated_repo_case) and callable(prepared_unit2_case),
             "genuine isolated repo and prepared Unit2 factories are required")
    boundary_fields = {"verify", "rejection_type", "implementation_bytes", "identity"}
    _require(type(consumer_boundary) is dict and boundary_fields <= set(consumer_boundary),
             "independent final consumer boundary is required; no mock fallback")
    _require(callable(consumer_boundary["verify"])
             and isinstance(consumer_boundary["rejection_type"], type)
             and issubclass(consumer_boundary["rejection_type"], Exception)
             and consumer_boundary["rejection_type"] is not Exception,
             "consumer boundary must provide a narrow rejection exception")
    body, identity = consumer_boundary["implementation_bytes"], consumer_boundary["identity"]
    _require(type(body) is bytes and type(identity) is dict
             and set(identity) == {"path", "bytes", "sha256"}
             and c.same(_entry(identity["path"], body), identity),
             "independent consumer implementation identity is missing or inconsistent")
    _require(type(fixture["package_manifest"]) is bytes
             and len(fixture["current_inputs"]) == len(fixture["predecessor_inputs"]) == 376,
             "complete genuine integration bytes are required")
    repo, package = Path(fixture["repo"]), Path(fixture["package"])
    bodies, seal, api = fixture["package_bytes"], fixture["seal"], fixture["public_api"]
    before, after = fixture["predecessor_inputs"], fixture["current_inputs"]

    class IntegrationControls(unittest.TestCase):
        def admit(self, package_bytes=None, source_seal=None, source_repo=None, public_api=None):
            return t.admit(source_repo if source_repo is not None else repo,
                           package_bytes if package_bytes is not None else bodies,
                           source_seal if source_seal is not None else seal,
                           public_api if public_api is not None else api)

        @contextmanager
        def negative_repo(self, specification):
            with isolated_repo_case(copy.deepcopy(specification)) as isolated:
                isolated = Path(isolated)
                self.assertNotEqual(isolated, repo)
                self.assertNotIn(repo, isolated.parents)
                self.assertNotIn(isolated, repo.parents)
                yield isolated

        @contextmanager
        def prepared(self):
            with prepared_unit2_case() as case:
                self.assertTrue({"runtime", "output", "captured", "seam"} <= set(case))
                output = Path(case["output"])
                self.assertNotEqual(output, repo)
                self.assertNotIn(repo, output.parents)
                self.assertNotIn(output, repo.parents)
                self.assertNotEqual(output, package)
                self.assertNotIn(package, output.parents)
                self.assertEqual(case["captured"]["inputs"], after)
                yield case, output

        def test_genuine_complete_376_inverse_and_provenance(self):
            result = self.admit()
            self.assertEqual(result["inputs"], after)
            self.assertEqual(result["predecessor_inputs"], before)
            self.assertEqual(len(result["inputs"]), 376)
            self.assertEqual(result["transition_touched"], ["src/main.rs"])
            self.assertEqual([n for n in sorted(after) if before[n] != after[n]], ["src/main.rs"])
            self.assertEqual(result["current"]["reviewed_source_head"], t.SOURCE_CHECKPOINT["head"])
            self.assertEqual(result["predecessor"]["reviewed_source_head"], t.HISTORICAL_SOURCE["head"])
            self.assertIs(result["source_only"], True)
            self.assertIs(result["execution_qualified"], False)

        def test_genuine_proof_schema_and_evidence_pins_reject_rehashes(self):
            mutations = [lambda x: x.update(extra=True), lambda x: x.pop("schema"),
                         lambda x: x.update(execution_qualified=True),
                         lambda x: x.update(published_main=copy.deepcopy(t.HISTORICAL_SOURCE)),
                         lambda x: x["git_object_evidence"]["published_main"].update(sha256="0" * 64),
                         lambda x: x["git_object_evidence"]["verifier"].update(bytes=65537),
                         lambda x: x["git_object_evidence"]["generation_receipt"].update(bytes=32769),
                         lambda x: x["git_object_evidence"].update(extra={})]
            messages = ["changed sealed complete published-source proof"] * 4 + [
                "changed accepted published Git readback", "oversized source generation evidence",
                "oversized source generation evidence", "missing exact Git object evidence pins"]
            for index, (mutate, message) in enumerate(zip(mutations, messages)):
                with self.subTest(index=index), self.assertRaisesRegex(c.adapters.Reject, "^" + message + "$"):
                    changed_bodies, changed_seal = _proposal(t, bodies, seal, "source_proof", mutate)
                    self.admit(changed_bodies, changed_seal)

        def test_genuine_identity_modes_blobs_fixtures_and_includes_are_fixed(self):
            mutations = [lambda x: x["current_input_identities"][0].update(mode="100755"),
                         lambda x: x["current_input_identities"][0].update(git_blob="0" * 40),
                         lambda x: x["compile_time_fixture_inputs"].pop(),
                         lambda x: x["compile_time_include_directives"].reverse(),
                         lambda x: x["compile_time_include_directives"].append(copy.deepcopy(x["compile_time_include_directives"][-1])),
                         lambda x: x["unit2_accounting_dependencies"][0].update(sha256="0" * 64)]
            for index, mutate in enumerate(mutations):
                with self.subTest(index=index), self.assertRaises(c.adapters.Reject):
                    changed_bodies, changed_seal = _proposal(t, bodies, seal, "authority", mutate)
                    self.admit(changed_bodies, changed_seal)

        def test_genuine_inverse_rejects_wrong_stage_tail_and_membership(self):
            restored, touched = api.apply_inverse_patch(after, bodies[t.PATCH_NAME],
                                                         seal["patch"]["sha256"], seal["patch"]["bytes"], t.TRANSITION_PATHS)
            self.assertEqual(restored, before)
            variants = [(dict(after), list(touched)), (dict(before), []),
                        ({**before, "src/main.rs": before["src/main.rs"] + b"tail"}, list(touched))]
            removed = dict(before)
            del removed[sorted(removed)[0]]
            variants.append((removed, list(touched)))
            for index, inverse in enumerate(variants):
                proxy = types.SimpleNamespace(**vars(api))
                proxy.apply_inverse_patch = lambda *args, inverse=inverse: copy.deepcopy(inverse)
                with self.subTest(index=index), self.assertRaises(c.adapters.Reject):
                    self.admit(public_api=proxy)

        def test_genuine_source_body_mutations_rejected(self):
            fixture_name = next(name for name in after if name.startswith(("tests/fixtures/", "fixtures/")))
            names = ("src/main.rs", fixture_name, *t.retained.ACCOUNTING_PATHS)
            for name in names:
                with self.subTest(path=name), self.negative_repo({"replace": {name: after[name] + b"tamper"}}) as isolated:
                    with self.assertRaisesRegex(c.adapters.Reject, "^unapproved complete body: " + name + "$"):
                        self.admit(source_repo=isolated)

        def test_genuine_complete_source_membership_and_modes_rejected(self):
            specifications = [
                {"remove": ["src/main.rs"]},
                {"add": {"src/unreviewed-extra.rs": b"extra"}},
                {"directory": ["src/unreviewed-empty-directory"]},
                {"symlink": {"src/main.rs": "../Cargo.toml"}},
                {"mode": {"src/main.rs": 0o755}},
            ]
            messages = [None, "missing or extra complete compiler member/directory",
                        "missing or extra complete compiler member/directory",
                        "symlink in selected input: src/main.rs", "nonregular/executable source: src/main.rs"]
            for specification, message in zip(specifications, messages):
                with self.subTest(mutation=tuple(specification)), self.negative_repo(specification) as isolated:
                    if message is None:
                        with self.assertRaises(FileNotFoundError) as failure:
                            self.admit(source_repo=isolated)
                        self.assertEqual(Path(failure.exception.filename), isolated / "src/main.rs")
                    else:
                        with self.assertRaisesRegex(c.adapters.Reject, "^" + message + "$"):
                            self.admit(source_repo=isolated)

        def test_genuine_package99_preserves_old95_boundary(self):
            raw, actual = c._current_package(repo, package, seal)
            self.assertEqual(raw, fixture["package_manifest"])
            self.assertEqual(actual, bodies)
            self.assertEqual(len(json.loads(raw)["files"]), 99)
            capture = c.preflight(repo, package, seal)
            old = capture["published_predecessor"]["package_bytes"]
            self.assertEqual(len(old), 95)
            self.assertEqual(set(actual), set(old) | {name for _, name in c.CURRENT_SOURCE_ARTIFACTS})
            for name in set(old) - {"run.py", "current-source.json"}:
                self.assertEqual(actual[name], old[name])

        def test_genuine_shared_package_body_coherent_rehash_rejected(self):
            capture = c.preflight(repo, package, seal)
            old_raw = capture["published_predecessor"]["package_manifest"]
            name = "unit2_u8_resource.py"
            changed = dict(bodies)
            changed[name] += b"\n# coherently rehashed negative control\n"
            manifest = json.loads(fixture["package_manifest"])
            manifest["files"] = [_entry(name, changed[name]) if row["path"] == name else row
                                 for row in manifest["files"]]
            with self.assertRaises(c.adapters.Reject):
                c._current_package_map(changed, _encoded(manifest), old_raw, seal)

        def test_genuine_capture_preserves_historical_and_accounting_chain(self):
            capture = c.preflight(repo, package, seal)
            historical = capture["published_predecessor"]
            self.assertEqual(capture["inputs"], after)
            self.assertEqual(historical["inputs"], before)
            for name in set(historical) - c.REPLACED_CAPTURE_FIELDS:
                self.assertEqual(capture[name], historical[name])
            self.assertEqual(v._old_inputs(capture), before)
            c._pure(capture)
            c._source_capture(capture, seal)
            binding = c.validate_unit2_accounting(capture, package, seal)
            old = c.retained_current.unit2_accounting_binding(historical, package)
            self.assertEqual(binding["predecessor_accounting_source_binding"], old)
            self.assertEqual(old["predecessor_accounting_source_binding"], historical[c.RETAINED_ACCOUNTING_KEY])
            for row in binding["source_dependencies"]:
                self.assertEqual(after[row["path"]], before[row["path"]])
            changed = copy.deepcopy(capture)
            changed["byte_storage_source"] = changed["current"]
            with self.assertRaises(c.adapters.Reject):
                c._source_capture(changed, seal)

        def test_genuine_capture_rejects_post_admission_type_confusion(self):
            # These are post-admission mutations of captured data, not changes
            # to sealed source bytes. Python's 0 == False, 1 == True and integral
            # float equality must not turn them into exact identity matches.
            capture = c.preflight(repo, package, seal)
            mutations = [
                lambda x: x["published_source_authority"].update(source_only=1),
                lambda x: x["published_source_authority"].update(execution_qualified=0),
                lambda x: x["published_source_proof"].update(source_only=1),
                lambda x: x["published_source_proof"].update(execution_qualified=0),
                lambda x: x["current"]["files"][0].update(bytes=float(x["current"]["files"][0]["bytes"])),
                lambda x: x["published_package_manifest"].update(bytes=float(x["published_package_manifest"]["bytes"])),
                lambda x: x["published_source_seal"]["source_manifest"].update(bytes=float(x["published_source_seal"]["source_manifest"]["bytes"])),
            ]
            for index, mutate in enumerate(mutations):
                changed = copy.deepcopy(capture)
                mutate(changed)
                with self.subTest(captured_identity=index), self.assertRaises(c.adapters.Reject):
                    c._source_capture(changed, seal)
            changed = copy.deepcopy(capture)
            row = changed[c.CURRENT_ACCOUNTING_KEY]["source_dependencies"][0]
            row["bytes"] = float(row["bytes"])
            with self.assertRaises(c.adapters.Reject):
                c.validate_unit2_accounting(changed, package, seal)

        def test_genuine_paired_capture_mutations_reject_canonical_mismatch(self):
            capture = c.preflight(repo, package, seal)
            cases = [
                ("u8_index_resource_authority", ("source_dependencies", 0, "bytes"),
                 "changed retained accounting authority"),
                ("byte_storage_source", ("files", 0, "bytes"),
                 "wrong retained accounting source identity"),
                ("lexer_reservation_seal", ("source_manifest", "bytes"),
                 "changed current or historical source association"),
                ("lexer_reservation_authority", ("current_source_members",),
                 "changed current or historical source association"),
            ]
            for field, path, message in cases:
                changed = copy.deepcopy(capture)
                value = changed[field]
                for part in path[:-1]:
                    value = value[part]
                value[path[-1]] = float(value[path[-1]])
                changed["published_predecessor"][field] = copy.deepcopy(changed[field])
                with self.subTest(field=field), self.assertRaisesRegex(c.adapters.Reject, "^" + message + "$"):
                    c.validate_unit2_accounting(changed, package, seal)

        def test_genuine_prepared_unit2_uses_actual_assembly_and_old_verifier(self):
            with self.prepared() as (case, output):
                runtime = case["runtime"]
                original_verify = runtime.verify_unit2_result
                original = original_verify(output, case["captured"], case["seam"], True)
                c.install(runtime, package, seal)
                actual = runtime.verify_unit2_result(output, case["captured"], case["seam"], True)
                self.assertEqual({key: actual[key] for key in original}, original)
                parser = runtime.regular(output / "unit2/run/source", "src/frontend/parser.rs").read_bytes()
                self.assertNotEqual(parser, after["src/frontend/parser.rs"])
                self.assertIs(actual["semantic_pass"], False)
                self.assertEqual(actual["compiler_executions"], 0)

        def test_genuine_prepared_unit2_rejects_changed_main_and_assembly(self):
            for name in ("src/main.rs", "src/frontend/parser.rs"):
                with self.subTest(path=name), self.prepared() as (case, output):
                    runtime = c.install(case["runtime"], package, seal)
                    path = output / "unit2/run/source" / name
                    self.assertFalse(path.is_symlink())
                    original = path.read_bytes()
                    try:
                        path.write_bytes(original + b"\nnegative integration mutation\n")
                        with self.assertRaisesRegex(runtime.BindingError, "^changed input: " + name + "$"):
                            runtime.verify_unit2_result(output, case["captured"], case["seam"], True)
                    finally:
                        path.write_bytes(original)

        def test_independent_consumer_accepts_exact_original_snapshot(self):
            consumer_boundary["verify"](copy.deepcopy(bodies), fixture["package_manifest"])

        def test_independent_consumer_rejects_coherent_dispatcher_manifest_tamper(self):
            capture = c.preflight(repo, package, seal)
            changed = dict(bodies)
            changed["run.py"] += b"\n# coherently rehashed dispatcher negative control\n"
            manifest = json.loads(fixture["package_manifest"])
            manifest["files"] = [_entry("run.py", changed["run.py"]) if row["path"] == "run.py" else row
                                 for row in manifest["files"]]
            raw = _encoded(manifest)
            # Intentionally demonstrate this helper's noncyclic trust boundary:
            # its package consistency check is not dispatcher authentication.
            c._current_package_map(changed, raw, capture["published_predecessor"]["package_manifest"], seal)
            with self.assertRaises(consumer_boundary["rejection_type"]):
                consumer_boundary["verify"](changed, raw)

    return _suite(IntegrationControls, INTEGRATION_CONTROLS, "genuine-fixture-integration-required")


if __name__ == "__main__":
    raise SystemExit("No implicit fixture discovery: use an authenticated effects-blocked caller and run both explicit suites.")
