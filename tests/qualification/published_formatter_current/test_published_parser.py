"""Inactive source controls. No test in this file has been executed.

Pure transport tests use synthetic identity maps, never synthetic success
receipts. The exact-source test requires the retained public predecessor file;
absence is a failure, not a skip. Effects-blocked full admission is a separate
reviewed fixture obligation. The lifetime tests below require that future exact
fixture and an effects audit; their presence is not evidence they have run.
"""
import copy
from contextlib import contextmanager
import importlib.abc
import importlib.util
from pathlib import Path, PurePosixPath
import tempfile
import unittest
from unittest.mock import Mock, patch
import sys
import types

import published_parser as transport
import derive_parser_successor as recipe


def row(name, text):
    return transport.identity(name, text.encode())


def fixture(stage="phase"):
    shared = ["src/main.rs"] + ["src/shared%03d.rs" % i for i in range(119)]
    historical = shared + ["tests/historical%03d.txt" % i for i in range(163)]
    selected = shared + ["src/added%03d.rs" % i for i in range(256)]
    original = {n: row(n, "historical:" + n) for n in sorted(historical)}
    unchanged = set(shared[1:36])
    before = {n: original[n] if n in unchanged else row(n, "M:" + n)
              for n in sorted(selected)}
    after = copy.deepcopy(before)
    after["src/main.rs"] = row("src/main.rs", "C:exact uninstrumented main")
    old_source = {"files": list(before.values()), "reviewed_source_head": "old-head",
                  "source_only_tree": "old-tree"}
    current = {"files": list(after.values()),
               "reviewed_source_head": transport.SOURCE_CHECKPOINT["head"],
               "source_only_tree": transport.SOURCE_CHECKPOINT["tree"]}
    base = original | before
    derived = dict(base)
    for name in ("candidate-source-manifest.json", "src/frontend/parser/unit4_observer.rs",
                 "tests/observer-helper.txt"):
        derived[name] = row(name, "unchanged:" + name)
    ordered = [derived[n] for n in sorted(derived, key=lambda n: PurePosixPath(n).parts)]
    active = {
        "schema": "oxid-unit4-current-parser-authority-v1",
        "reviewed_source_head": "old-head", "source_only_tree": "old-tree",
        "current_source_manifest": {"path": transport.PACKAGE + "current-source.json",
                                    "bytes": 74328, "sha256": transport.OLD_SOURCE_SHA},
        "source_binding_runner": row(transport.PACKAGE + "run.py", "old-dispatcher"),
        "source_delta": [{"path": n, "before": original.get(n), "after": r}
                         for n, r in before.items() if original.get(n) != r],
        "current_base_files": [base[n] for n in sorted(base)],
        "current_derived_files": copy.deepcopy(ordered),
        "current_control_derived_files": copy.deepcopy(ordered),
        "current_candidate_source_manifest_sha256": derived["candidate-source-manifest.json"]["sha256"],
        "unchanged_constraint": {"strict": True, "count": 54},
    }
    if stage == "phase":
        active["lexer_phase"] = {"unchanged_layouts": True}
    old = {"base_commit": "historical-283", "original_files": list(original.values()),
           "current_source": old_source, "current": active,
           "instrumentation": [], "control_instrumentation": []}
    seal = {"source_manifest": row(transport.PACKAGE + "current-source.json", "new-manifest"),
            "source_dispatcher": row(transport.PACKAGE + "run.py", "new-dispatcher"),
            "source_seal": {
                "authority": row(transport.SOURCE_AUTHORITY_NAME, "new-source-authority"),
                "source_proof": row(transport.SOURCE_PROOF_NAME, "new-source-proof")}}
    admitted = {"current": current, "published_predecessor": {"current": old_source},
                "published_source_authority": {
                    "retained_historical_source": {"head": "old-head", "tree": "old-tree"}}}
    return old, seal, admitted


class MapTransportControls(unittest.TestCase):
    def test_recursive_same_rejects_equal_valued_different_numeric_types(self):
        for expected, changed in ((0, False), (1, True), (0, 0.0), (1, 1.0), (55, 55.0)):
            for actual, wanted in ((changed, expected),
                    ({"current": [{"value": changed}]}, {"current": [{"value": expected}]})):
                self.assertEqual(actual, wanted)  # Ordinary equality is insufficient.
                with self.assertRaisesRegex(transport.Rejected, "exact typed value"):
                    transport.same(actual, wanted, "exact typed value")
                transport.same(wanted, copy.deepcopy(wanted), "positive exact typed value")

    def test_exact_cardinality_delta_and_unchanged_bodies(self):
        old, seal, admitted = fixture()
        result = transport._transport(old, seal, admitted, "phase")
        self.assertEqual(result["current_source"], admitted["current"])
        self.assertNotEqual(result["current_source"], old["current_source"])
        for field, count in (("current_base_files", 539), ("current_derived_files", 542),
                             ("current_control_derived_files", 542)):
            before = transport.rows_map(old["current"][field], count)
            after = transport.rows_map(result["current"][field], count)
            changed = [n for n in before if before[n] != after[n]]
            self.assertEqual(set(changed), {"src/main.rs"} if count == 539 else
                             {"src/main.rs", "candidate-source-manifest.json"})
        self.assertEqual(len(result["current"]["source_delta"]), 341)
        main = next(r for r in result["current"]["source_delta"] if r["path"] == "src/main.rs")
        prior = next(r for r in old["current"]["source_delta"] if r["path"] == "src/main.rs")
        self.assertEqual(main["before"], prior["before"])
        self.assertEqual(result["current"]["unchanged_constraint"], old["current"]["unchanged_constraint"])
        self.assertNotIn(transport.BINDING_KEY, old["current"])

    def test_prephase_and_phase_share_candidate(self):
        old, seal, admitted = fixture()
        phase = transport._transport(old, seal, admitted, "phase")
        pre = copy.deepcopy(old)
        del pre["current"]["lexer_phase"]
        prephase = transport._transport(pre, seal, admitted, "prephase")
        self.assertEqual(transport.current_candidate(phase), transport.current_candidate(prephase))
        self.assertEqual(prephase["current"][transport.BINDING_KEY]["stage"], "prephase")
        self.assertEqual(phase["current"][transport.BINDING_KEY]["stage"], "phase")

    def test_wrong_stage_and_double_transport_rejected(self):
        old, seal, admitted = fixture()
        with self.assertRaises(transport.Rejected):
            transport._transport(old, seal, admitted, "prephase")
        result = transport._transport(old, seal, admitted, "phase")
        with self.assertRaises(transport.Rejected):
            transport._transport(result, seal, admitted, "phase")

    def test_second_source_change_rejected_even_with_coherent_identity(self):
        old, seal, admitted = fixture()
        current = admitted["current"]["files"]
        target = next(i for i, r in enumerate(current) if r["path"] != "src/main.rs")
        current[target] = row(current[target]["path"], "coherently rehashed other body")
        with self.assertRaises(transport.Rejected):
            transport._transport(old, seal, admitted, "phase")

    def test_added_deleted_renamed_reordered_duplicate_members_rejected(self):
        for mutation in ("add", "delete", "rename", "reorder", "duplicate"):
            with self.subTest(mutation=mutation):
                old, seal, admitted = fixture()
                rows = admitted["current"]["files"]
                if mutation == "add": rows.append(row("src/extra.rs", "extra"))
                elif mutation == "delete": rows.pop()
                elif mutation == "rename": rows[-1] = row("src/renamed.rs", "renamed")
                elif mutation == "reorder": rows[0], rows[1] = rows[1], rows[0]
                else: rows[-1] = copy.deepcopy(rows[0])
                with self.assertRaises(transport.Rejected):
                    transport._transport(old, seal, admitted, "phase")

    def test_main_instrumentation_rejected(self):
        for field in ("instrumentation", "control_instrumentation"):
            old, seal, admitted = fixture()
            old[field] = [{"path": "src/main.rs"}]
            with self.assertRaises(transport.Rejected):
                transport._transport(old, seal, admitted, "phase")

    def test_stale_base_role_and_delta_rejected(self):
        for target in ("current_base_files", "current_derived_files", "source_delta"):
            old, seal, admitted = fixture()
            if target == "source_delta":
                old["current"][target][0]["after"] = row(old["current"][target][0]["path"], "stale")
            else:
                main = next(r for r in old["current"][target] if r["path"] == "src/main.rs")
                main["sha256"] = "0" * 64
            with self.assertRaises(transport.Rejected):
                transport._transport(old, seal, admitted, "phase")

    def test_closed_composer_routes_reject_before_any_io(self):
        for name, control, operation in (("src/main.rs", False, "lexer"),
                ("src/frontend/project/budget.rs", True, "array"),
                ("src/frontend/lexer.rs", 0, "lexer"),
                ("src/frontend/lexer.rs", False, "unknown")):
            with self.assertRaises(transport.Rejected):
                transport.compose(None, {}, name, b"", control, operation, {}, {})


class ExactSourceRecipeControls(unittest.TestCase):
    def test_wrong_predecessor_rejected(self):
        with self.assertRaises(recipe.Rejected):
            recipe.derive(b"unreviewed source", {})

    def source_fixture(self):
        repo = Path(__file__).resolve().parents[3]
        retained = repo / ("tests/qualification/published_formatter_current/predecessor-v1/"
                           "tests/qualification/unit4_parser_current/portable.py")
        original = retained.read_bytes()
        artifact = lambda path: row(path, "synthetic reviewed-descriptor fixture")
        dependencies = [{"module": name, "artifact": artifact(
                            ("tests/qualification/lexer_reservation_current/" if index < 4 else
                             transport.SUPPORT_DIRECTORY) + name + ".py")}
                        for index, name in enumerate(recipe.IMPORT_ORDER)]
        # Pure source-generation fixture only. These pins cannot admit runtime.
        seal = {"schema": "oxid-unit4-published-main-parser-seal-v1",
                "authority": {"path": transport.DIRECTORY + "authority.json", "bytes": 1,
                              "sha256": "1" * 64},
                "source_runtime_dependencies": dependencies,
                "view": dependencies[-1]["artifact"],
                "support": artifact(transport.SUPPORT_DIRECTORY + "published_parser.py"),
                "source_runtime": artifact(transport.SUPPORT_DIRECTORY + "published_current.py"),
                "source_dispatcher": artifact(transport.PACKAGE + "run.py"),
                "source_manifest": artifact(transport.PACKAGE + "current-source.json"),
                "source_seal": {"synthetic_source_recipe_fixture": True}}
        return original, seal

    def test_all_other_functions_remain_complete_identical_bodies(self):
        original, seal = self.source_fixture()
        derived, receipt = recipe.derive(original, seal)
        old_functions, new_functions = recipe._functions(original), recipe._functions(derived)
        self.assertEqual(set(r["name"] for r in receipt["changed_functions"]), set(recipe.FUNCTIONS))
        for name in old_functions:
            if name not in recipe.FUNCTIONS:
                self.assertEqual(new_functions[name][2], old_functions[name][2], name)
        self.assertEqual(new_functions["prepare"][2], old_functions["prepare"][2])
        self.assertEqual(new_functions["phase_evidence"][2], old_functions["phase_evidence"][2])
        self.assertFalse(receipt["execution_qualified"])

    def test_registered_and_unregistered_loader_use_exact_current_api(self):
        original, seal = self.source_fixture()
        derived, _ = recipe.derive(original, seal)

        class MemorySourceLoader(importlib.abc.Loader):
            def create_module(self, spec):
                return None

            def exec_module(self, module):
                module.__file__ = ("/tmp/oxid-parser-loader-test-no-materialization/"
                                   "tests/qualification/unit4_parser_current/portable.py")
                exec(compile(derived, module.__file__, "exec"), module.__dict__)

        for registered in (False, True):
            with self.subTest(registered=registered):
                name = "_unit4_portable_admission_regression_" + str(registered)
                self.assertNotIn(name, sys.modules)
                loader = MemorySourceLoader()
                spec = importlib.util.spec_from_loader(name, loader)
                module = importlib.util.module_from_spec(spec)
                if registered:
                    sys.modules[name] = module
                try:
                    # Mirrors module_from_spec/exec_module without registration
                    # when registered is false; no files or M views are created.
                    loader.exec_module(module)
                    admitted = {"sentinel": "source-admission test double"}
                    seen = []

                    def check_api(api):
                        self.assertEqual(set(vars(api)),
                                         {"REPOSITORY", "AUTHORITY_SHA", "verify_map",
                                          "regular", "read", "load"})
                        for key in vars(api):
                            self.assertIs(getattr(api, key), getattr(module, key))
                        seen.append(api)

                    def preflight(repo, package, source_seal):
                        self.assertEqual(repo, module.REPOSITORY)
                        self.assertEqual(package, repo / "tests/fixtures/typed_project_source_binding")
                        self.assertEqual(source_seal, seal["source_seal"])
                        return admitted

                    def authority(api, fixed, capture):
                        check_api(api)
                        self.assertEqual(fixed, seal)
                        self.assertIs(capture, admitted)
                        return "authority reached"

                    def phase_previous(api, value, fixed, capture):
                        authority(api, fixed, capture)
                        return value

                    def compose(api, value, path, raw, control, operation, fixed, capture):
                        authority(api, fixed, capture)
                        return raw

                    class SourceRejected(ValueError):
                        pass

                    support = types.SimpleNamespace(authority=authority,
                        phase_previous=phase_previous, compose=compose,
                        current_candidate=lambda value: value,
                        _seal=lambda api, fixed: None, Rejected=transport.Rejected)
                    current = types.SimpleNamespace(preflight=preflight,
                        adapters=types.SimpleNamespace(Reject=SourceRejected))
                    # Only the effectful pinned-import boundary is doubled.
                    # The emitted context/API construction and all seven new
                    # entry-point bodies execute unchanged in this seam test.
                    with patch.object(module, "_published_parser_modules", return_value=(support, current)):
                        self.assertEqual(module.authority(), "authority reached")
                        marker = {"test": "prephase"}
                        self.assertIs(module.phase_previous(marker), marker)
                        self.assertIs(module.current_candidate(marker), marker)
                        self.assertEqual(module.compose_current_lexer(marker, b"lexer"), b"lexer")
                        self.assertEqual(module.compose_array_instrumentation(marker, "src/frontend/ast.rs", b"ast"), b"ast")
                        self.assertEqual(module.compose_namespace_resource(marker, b"resource"), b"resource")
                        self.assertEqual(module.compose_source_read(marker, b"source"), b"source")
                        calls = (
                            ("authority", lambda: module.authority()),
                            ("phase_previous", lambda: module.phase_previous(marker)),
                            ("current_candidate", lambda: module.current_candidate(marker)),
                            ("compose", lambda: module.compose_current_lexer(marker, b"lexer")),
                            ("compose", lambda: module.compose_array_instrumentation(marker, "src/frontend/ast.rs", b"ast")),
                            ("compose", lambda: module.compose_namespace_resource(marker, b"resource")),
                            ("compose", lambda: module.compose_source_read(marker, b"source")))
                        for target, call in calls:
                            original_error = transport.Rejected("exact support rejection")
                            with patch.object(support, target, side_effect=original_error):
                                with self.assertRaises(module.Rejected) as caught:
                                    call()
                            self.assertIs(caught.exception.__cause__, original_error)
                            self.assertEqual(str(caught.exception), str(original_error))
                        source_error = SourceRejected("exact source rejection")
                        with patch.object(current, "preflight", side_effect=source_error):
                            with self.assertRaises(module.Rejected) as caught:
                                module.authority()
                        self.assertIs(caught.exception.__cause__, source_error)
                        with patch.object(support, "authority", side_effect=source_error):
                            with self.assertRaises(module.Rejected) as caught:
                                module.authority()
                        self.assertIs(caught.exception.__cause__, source_error)
                        pin_error = transport.Rejected("exact current seal rejection")
                        with patch.object(support, "_seal", side_effect=pin_error), \
                             patch.object(current, "preflight", side_effect=AssertionError("preflight reached")) as preflight_call:
                            with self.assertRaises(module.Rejected) as caught:
                                module.authority()
                        self.assertIs(caught.exception.__cause__, pin_error)
                        preflight_call.assert_not_called()
                        for unrelated in (ValueError("unclassified failure"),
                                BaseExceptionGroup("preserve both failures",
                                    [RuntimeError("operation"), transport.Rejected("cleanup")])):
                            with patch.object(support, "authority", side_effect=unrelated):
                                with self.assertRaises(type(unrelated)) as caught:
                                    module.authority()
                            self.assertIs(caught.exception, unrelated)
                    self.assertEqual(len(seen), 6)
                    if registered:
                        self.assertIs(sys.modules[name], module)
                    else:
                        self.assertNotIn(name, sys.modules)
                finally:
                    if registered:
                        sys.modules.pop(name, None)

    def test_bad_current_parser_pin_rejects_before_source_preflight_or_view(self):
        original, seal = self.source_fixture()
        derived, _ = recipe.derive(original, seal)
        module = types.ModuleType("_unit4_bad_current_pin")
        module.__file__ = ("/tmp/oxid-parser-pin-test-no-materialization/"
                           "tests/qualification/unit4_parser_current/portable.py")
        exec(compile(derived, module.__file__, "exec"), module.__dict__)

        class SourceRejected(ValueError):
            pass

        current = types.SimpleNamespace(adapters=types.SimpleNamespace(Reject=SourceRejected),
            preflight=Mock(side_effect=AssertionError("source preflight reached")),
            predecessor_view=Mock(side_effect=AssertionError("source M view reached")))
        bad_path = module.REPOSITORY / seal["authority"]["path"]

        def virtual_read(path):
            self.assertEqual(path, bad_path)
            return b"corrupt current parser authority"

        # Only metadata/file reads and the preauthenticated import boundary are
        # doubled. Actual emitted context, support._seal, verify_map, identity and
        # digest predicates run; no temporary fixture or M view is materialized.
        with patch.object(module, "_published_parser_modules", return_value=(transport, current)), \
             patch.object(module, "regular"), \
             patch.object(Path, "read_bytes", autospec=True, side_effect=virtual_read), \
             patch.object(transport, "_view", side_effect=AssertionError("parser M view reached")) as view:
            with self.assertRaisesRegex(module.Rejected, "file bytes differ: .*authority.json"):
                module.authority()
        current.preflight.assert_not_called()
        current.predecessor_view.assert_not_called()
        view.assert_not_called()

    def test_module_second_read_is_reauthenticated_before_compile_or_exec(self):
        descriptor = row(transport.SUPPORT_DIRECTORY + "published_view.py", "approved body")
        api = types.SimpleNamespace(REPOSITORY=Path("/unmaterialized"), verify_map=Mock())
        with patch.object(Path, "read_bytes", return_value=b"changed after initial verification"), \
             patch("builtins.compile", side_effect=AssertionError("unverified compile reached")) as compiler:
            with self.assertRaisesRegex(transport.Rejected, "exact second-read publication module body"):
                transport._module(api, descriptor, "_unverified_module")
        api.verify_map.assert_called_once_with(api.REPOSITORY, [descriptor])
        compiler.assert_not_called()


class RetainedImportScopeControls(unittest.TestCase):
    def test_retained_rejection_translation_preserves_cause_and_cleanup_group(self):
        view = Path("/tmp/oxid-parser-rejection-test-no-materialization")

        class OldRejected(ValueError):
            pass

        old = types.SimpleNamespace(Rejected=OldRejected)

        @contextmanager
        def view_double(*args):
            yield view

        name = "oxid_parser_retained_rejection_alias_test"
        self.assertNotIn(name, sys.modules)
        with patch.object(transport, "_view", side_effect=view_double), \
             patch.object(transport, "_retained", return_value=old):
            for leak in (False, True):
                original_error = OldRejected("unchanged retained predicate failure")
                expected = BaseExceptionGroup if leak else transport.Rejected
                with self.assertRaises(expected) as caught:
                    with transport._retained_scope(None, {}, {}):
                        if leak:
                            module = types.ModuleType(name)
                            module.__file__ = str(view / "unexpected.py")
                            sys.modules[name] = module
                        raise original_error
                if leak:
                    self.assertIs(caught.exception.exceptions[0], original_error)
                    self.assertEqual(caught.exception.exceptions[1].aliases, (name,))
                else:
                    self.assertIs(caught.exception.__cause__, original_error)
                self.assertNotIn(name, sys.modules)

    def test_original_alias_objects_restored_on_success_and_failure(self):
        view = Path("/tmp/oxid-parser-import-test-no-materialization")
        missing = object()
        before = {name: sys.modules.get(name, missing) for name in transport.RETAINED_IMPORT_ALIASES}
        for fail in (False, True):
            try:
                with transport._import_scope(view):
                    for name in transport.RETAINED_IMPORT_ALIASES:
                        module = types.ModuleType(name)
                        module.__file__ = str(view / (name + ".py"))
                        sys.modules[name] = module
                    if fail:
                        raise RuntimeError("prescribed retained failure")
            except RuntimeError:
                self.assertTrue(fail)
            for name, module in before.items():
                if module is missing:
                    self.assertNotIn(name, sys.modules)
                else:
                    self.assertIs(sys.modules[name], module)

    def test_unexpected_alias_cleanup_restores_only_exact_names_and_always_rejects(self):
        view = Path("/tmp/oxid-parser-import-test-no-materialization")
        name = "oxid_parser_unexpected_retained_alias_test"
        unrelated_name = "oxid_parser_unrelated_import_test"
        self.assertNotIn(name, sys.modules)
        self.assertNotIn(unrelated_name, sys.modules)
        for had_prior in (False, True):
            for fail in (False, True):
                with self.subTest(had_prior=had_prior, fail=fail):
                    prior = types.ModuleType(name)
                    prior.__file__ = "/outside-predecessor/prior.py"
                    leaked = types.ModuleType(name)
                    leaked.__file__ = str(view / "unexpected.py")
                    unrelated = types.ModuleType(unrelated_name)
                    unrelated.__file__ = "/outside-predecessor/unrelated.py"
                    original_error = RuntimeError("prescribed retained operation failure")
                    if had_prior:
                        sys.modules[name] = prior
                    try:
                        expected = BaseExceptionGroup if fail else transport.Rejected
                        with self.assertRaises(expected) as caught:
                            with transport._import_scope(view):
                                sys.modules[name] = leaked
                                sys.modules[unrelated_name] = unrelated
                                if fail:
                                    raise original_error
                        violation = caught.exception
                        if fail:
                            self.assertEqual(len(violation.exceptions), 2)
                            self.assertIs(violation.exceptions[0], original_error)
                            violation = violation.exceptions[1]
                        self.assertIs(type(violation), transport.Rejected)
                        self.assertEqual(violation.aliases, (name,))
                        if had_prior:
                            self.assertIs(sys.modules[name], prior)
                        else:
                            self.assertNotIn(name, sys.modules)
                        self.assertIs(sys.modules[unrelated_name], unrelated)
                    finally:
                        sys.modules.pop(name, None)
                        sys.modules.pop(unrelated_name, None)


class _AuditOverlap(RuntimeError):
    pass


class _FullViewEffectAudit:
    """Test-only observation of the actual directory effect across instances.

    This is an effects-audit rejection boundary, not a production view lock or
    admission bypass. No production predicate, capture or lease is replaced.
    """
    def __init__(self):
        self.original = tempfile.TemporaryDirectory
        self.active = 0
        self.maximum = 0
        self.created = 0
        self.rejected = 0

    @contextmanager
    def directory(self, *args, **kwargs):
        prefix = kwargs.get("prefix", args[1] if len(args) > 1 else None)
        if prefix != "oxid-published-main-":
            with self.original(*args, **kwargs) as directory:
                yield directory
            return
        if self.active:
            self.rejected += 1
            raise _AuditOverlap("effects audit rejected overlapping full M view")
        self.active += 1
        self.maximum = max(self.maximum, self.active)
        try:
            with self.original(*args, **kwargs) as directory:
                self.created += 1
                yield directory
        finally:
            self.active -= 1


class PredecessorLifetimeEffectsControls(unittest.TestCase):
    """Unexecuted; needs the independently authenticated current fixture.

    The normal entrypoint/import closure must authenticate this test and the
    current adapter before execution. Missing final fixture/seals must fail;
    these tests do not substitute synthetic descriptors or skip admission.
    """
    def current_adapter(self):
        repo = Path(__file__).resolve().parents[3]
        path = repo / (transport.DIRECTORY + "portable.py")
        spec = importlib.util.spec_from_file_location("_unit4_lifetime_admission", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assertNotIn(spec.name, sys.modules)
        return module

    def instances(self, p):
        _, admitted, api, _ = p._published_parser_context()
        row = p._PUBLISHED_PARSER_SEAL["view"]
        first = transport._module(api, row, "_unit4_view_instance_one")
        second = transport._module(api, row, "_unit4_view_instance_two")
        self.assertIsNot(first, second)
        return first, second, admitted

    def test_real_current_authority_and_inverse_reject_numeric_type_confusion(self):
        p = self.current_adapter()
        outer = p.authority()
        prephase = p.phase_previous(outer)
        support, admitted, api, _ = p._published_parser_context()
        seal = p._PUBLISHED_PARSER_SEAL
        lexer = (p.REPOSITORY / "src/frontend/lexer.rs").read_bytes()
        active_raw = (p.REPOSITORY / seal["authority"]["path"]).read_bytes()
        original_load = api.load
        cases = (
            (("current", "u8_closed_policy", "state_fields_added"), 0, False),
            (("current", "observer_initializer_adapter", "substitutions"), 1, True),
            (("current", "u8_closed_policy", "state_fields_added"), 0, 0.0),
            (("current", "observer_initializer_adapter", "substitutions"), 1, 1.0),
            (("original_files", 0, "bytes"), 55, 55.0))

        def changed_copy(value, path, expected, replacement):
            changed = copy.deepcopy(value)
            target = changed
            for key in path[:-1]:
                target = target[key]
            self.assertIs(type(target[path[-1]]), int)
            self.assertEqual(target[path[-1]], expected)
            target[path[-1]] = replacement
            self.assertEqual(changed, value)  # The rejected previous == accepted this.
            return changed

        # Positive calls above use the real current adapter, sealed C authority
        # and genuine M view. No synthetic authority replaces this fixture.
        for path, expected, replacement in cases:
            with self.subTest(path=path, replacement_type=type(replacement).__name__):
                changed = changed_copy(outer, path, expected, replacement)
                with self.assertRaisesRegex(p.Rejected, "exact phase input; no double inversion"):
                    p.phase_previous(changed)
                with self.assertRaisesRegex(support.Rejected, "exact full outer inverse input"):
                    support.provenance_previous(api, changed, seal, admitted)
                changed_prephase = changed_copy(prephase, path, expected, replacement)
                with self.assertRaisesRegex(p.Rejected, "composer requires exact current prephase stage"):
                    p.compose_current_lexer(changed_prephase, lexer)
                if path[0] == "current":
                    # Deliberate below-pin parser-load injection tests the full
                    # authority equivalence predicate independently of its pin.
                    # The fixed artifact and all old predicates stay unchanged.
                    with patch.object(api, "load", side_effect=lambda raw:
                            changed["current"] if raw == active_raw else original_load(raw)):
                        with self.assertRaisesRegex(support.Rejected,
                                "exact independently sealed complete parser authority"):
                            support.authority(api, seal, admitted)

    def test_real_capture_seal_rejects_equal_float_descriptor_bytes(self):
        p = self.current_adapter()
        support, admitted, api, _ = p._published_parser_context()
        changed = copy.deepcopy(admitted)
        descriptor = changed["published_source_seal"]["authority"]
        self.assertIs(type(descriptor["bytes"]), int)
        descriptor["bytes"] = float(descriptor["bytes"])
        self.assertEqual(changed, admitted)
        with self.assertRaisesRegex(support.Rejected, "wrong source admission seal"):
            support._source(api, p._PUBLISHED_PARSER_SEAL, changed)

    def test_fresh_module_leases_are_observed_to_be_independent(self):
        p = self.current_adapter()
        first, second, _ = self.instances(p)
        self.assertIsNot(first._VIEW_LEASE, second._VIEW_LEASE)
        self.assertTrue(first._VIEW_LEASE.acquire(blocking=False))
        try:
            self.assertTrue(second._VIEW_LEASE.acquire(blocking=False))
            second._VIEW_LEASE.release()
        finally:
            first._VIEW_LEASE.release()

    def test_cross_instance_overlap_is_rejected_before_second_directory_effect(self):
        p = self.current_adapter()
        audit = _FullViewEffectAudit()
        with patch.object(tempfile, "TemporaryDirectory", side_effect=audit.directory):
            first, second, admitted = self.instances(p)
            seal = admitted["published_source_seal"]["predecessor_view"]
            with first.predecessor_view(p.REPOSITORY, admitted=admitted, seal=seal):
                created = audit.created
                with self.assertRaisesRegex(_AuditOverlap, "overlapping full M view"):
                    with second.predecessor_view(p.REPOSITORY, admitted=admitted, seal=seal):
                        self.fail("second full M directory must never be yielded")
                self.assertEqual(audit.created, created)
                self.assertEqual(audit.active, 1)
            # Both local leases must be released, including the rejected instance.
            with second.predecessor_view(p.REPOSITORY, admitted=admitted, seal=seal):
                self.assertEqual(audit.active, 1)
        self.assertEqual((audit.active, audit.maximum, audit.rejected), (0, 1, 1))

    def test_actual_current_entrypoint_graph_has_only_sequential_full_views(self):
        p = self.current_adapter()
        audit = _FullViewEffectAudit()
        with patch.object(tempfile, "TemporaryDirectory", side_effect=audit.directory):
            outer = p.authority()
            previous = p.phase_previous(outer)
            p.current_candidate(outer)
            for name in ("src/frontend/ast.rs", "src/frontend/parser.rs"):
                for control in (False, True):
                    p.compose_array_instrumentation(previous, name,
                        (p.REPOSITORY / name).read_bytes(), control)
            for name, compose in (
                    ("src/frontend/lexer.rs", p.compose_current_lexer),
                    ("src/frontend/project/budget.rs", lambda a, raw:
                        p.compose_array_instrumentation(a, "src/frontend/project/budget.rs", raw)),
                    ("src/frontend/declaration_index/resource.rs", p.compose_namespace_resource),
                    ("src/frontend/source.rs", p.compose_source_read)):
                raw = compose(previous, (p.REPOSITORY / name).read_bytes())
                if name in ("src/frontend/lexer.rs", "src/frontend/project/budget.rs"):
                    p.phase_composed_body(outer, name, raw)
            for control in (False, True):
                p.phase_composed_body(outer, "src/frontend/parser/unit4_observer.rs",
                                      p.compose_observer_initializer(previous), control)
            p.phase_control_binding(outer)
        self.assertGreater(audit.created, 1)
        self.assertEqual((audit.active, audit.maximum, audit.rejected), (0, 1, 0))


if __name__ == "__main__":
    unittest.main()
