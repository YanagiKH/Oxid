#!/usr/bin/env python3
"""No-build, no-generation controls for the inactive preparation helpers."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import adapters as a
import ci_inventory as ci
import source_transition as s

REPO = Path(__file__).resolve().parents[3]
UNIT1 = REPO / "tests/fixtures/typed_project_unit1_independent"


class AdapterControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.lexer = (REPO / a.LEXER_PATH).read_bytes()
        cls.original = (UNIT1 / "reviewer_additional.rs").read_bytes()

    def test_unit1_exact_inverse_and_all_case_names(self):
        derived, receipt = a.unit1_counter_domain(self.original)
        self.assertEqual(a.inverse_exact(derived, a.UNIT1_SEAMS), self.original)
        receipt = a.unit1_correspondence((UNIT1 / "expected-test-roster.json").read_bytes(), receipt)
        self.assertEqual(len(receipt["cases"]), 69)
        self.assertEqual(sum(row["adaptation"] != "unchanged" for row in receipt["cases"]), 1)
        self.assertFalse(receipt["execution_qualified"])

    def test_unit1_original_tampering_refused(self):
        for raw in (self.original + b"\n", self.original.replace(b'source-project', b'lex'), b""):
            with self.subTest(raw=raw[-20:]), self.assertRaises(a.Reject):
                a.unit1_counter_domain(raw)

    def test_unit1_double_adaptation_refused(self):
        derived, _ = a.unit1_counter_domain(self.original)
        with self.assertRaises(a.Reject):
            a.unit1_counter_domain(derived)

    def test_unit1_nonlexer_assertions_exactly_retained(self):
        derived, _ = a.unit1_counter_domain(self.original)
        self.assertIn(a.UNIT1_BEFORE.replace(b"        ", b"            "), derived)
        for original_line in (
            b'        assert!(!d.message.contains("allocation failed"));',
            b'        assert_eq!(f.allocator.trace.len(),ordinal-1,"overflow must reject before a reserve attempt");',
            b'        if f.sources.files().is_empty(){assert_eq!(d.primary,None);}else{assert!(d.primary.is_some());}',
        ):
            self.assertEqual(derived.count(original_line), self.original.count(original_line))
        self.assertIn(b'vec![5,6,19,20,21]', derived)
        self.assertIn(b'"lexer token tape"', derived)
        self.assertIn(b'd.secondary.is_empty() && d.notes.is_empty()', derived)

    def test_roster_mutations_refused(self):
        raw = (UNIT1 / "expected-test-roster.json").read_bytes()
        _, receipt = a.unit1_counter_domain(self.original)
        for changed in (raw + b" ", raw.replace(a.UNIT1_CASE.encode(), b"replacement")):
            with self.assertRaises(a.Reject):
                a.unit1_correspondence(changed, receipt)

    def _instrument(self, kind, raw=None):
        raw = self.lexer if raw is None else raw
        seams = a.LIFECYCLE_SEAMS if kind == "lifecycle" else a.TOKEN_SEAMS
        # This computed value is a unit-test fixture, not a frozen derived-map
        # identity or independent approval. Final pins must be reviewed separately.
        expected = a.replace_exact(raw, seams)
        return a.instrument_lexer(raw, a.binding(a.LEXER_PATH, raw),
                                  a.binding(a.LEXER_PATH, expected), kind)

    def test_lifecycle_core_only_exact_inverse(self):
        derived, receipt = self._instrument("lifecycle")
        self.assertEqual(a.inverse_exact(derived, a.LIFECYCLE_SEAMS), self.lexer)
        prefix, core = derived.split(a.CORE_START, 1)
        self.assertNotIn(a.LIFECYCLE_OBSERVER, prefix)
        self.assertEqual(core.count(b'::event("lex_attempt"'), 1)
        self.assertEqual(core.count(b'::event("lex_complete"'), 1)
        self.assertLess(core.index(b'::event("lex_attempt"'), core.index(b'let limit ='))
        self.assertGreater(core.index(b'::event("lex_complete"'), core.index(b'kind: Kind::Eof'))
        self.assertEqual(receipt["wrapper_hook_sites"], 0)

    def test_token_hook_only_after_successful_push(self):
        derived, receipt = self._instrument("token")
        self.assertEqual(a.inverse_exact(derived, a.TOKEN_SEAMS), self.lexer)
        self.assertEqual(derived.count(b"::lex_token("), 1)
        self.assertIn(b'tokens.push(token);\n    ' + a.TOKEN_OBSERVER + b'::lex_token(*tokens.last().unwrap());\n    Ok(())', derived)
        self.assertEqual(receipt["successful_append_hook_sites"], 1)

    def test_complete_source_and_derived_pins_required(self):
        derived = a.replace_exact(self.lexer, a.TOKEN_SEAMS)
        for source, expected in ((self.lexer + b"\n", derived), (self.lexer, derived + b"\n")):
            with self.assertRaises(a.Reject):
                a.instrument_lexer(source, a.binding(a.LEXER_PATH, self.lexer),
                    a.binding(a.LEXER_PATH, expected), "token")

    def test_missing_and_duplicate_hook_seams_refused(self):
        for changed in (self.lexer.replace(a.APPEND_END, b""),
                        self.lexer + a.APPEND_END,
                        self.lexer.replace(b"tokens.push(token);", b"tokens.extend([token]);"),
                        self.lexer.replace(b"Token { kind, span }", b"token")):
            with self.assertRaises(a.Reject):
                a.lexer_schedule(changed)

    def test_wrong_kind_and_already_instrumented_refused(self):
        with self.assertRaises(a.Reject):
            a.instrument_lexer(self.lexer, a.binding(a.LEXER_PATH, self.lexer), {}, "unknown")
        for kind in ("lifecycle", "token"):
            derived, _ = self._instrument(kind)
            with self.assertRaises(a.Reject):
                a.lexer_schedule(derived)

    def test_inverse_missing_duplicate_and_wrong_stage_refused(self):
        derived, _ = self._instrument("token")
        for changed in (self.lexer, derived.replace(a.TOKEN_SEAMS[0][1], b""),
                        derived + a.TOKEN_SEAMS[0][1]):
            with self.assertRaises(a.Reject):
                a.inverse_exact(changed, a.TOKEN_SEAMS)

    def test_budget_preserves_exact_public_reserve_hook(self):
        raw = (REPO / a.BUDGET_PATH).read_bytes()
        expected = a.replace_exact(raw, a.BUDGET_SEAMS)
        derived, receipt = a.instrument_budget(raw, a.binding(a.BUDGET_PATH, raw),
                                               a.binding(a.BUDGET_PATH, expected))
        self.assertEqual(a.inverse_exact(derived, a.BUDGET_SEAMS), raw)
        self.assertEqual(derived.count(b"::reserve(kind, length, element_bytes, success)"), 1)
        self.assertEqual(receipt["control_build_hook_sites"], 0)
        for changed in (derived, raw + a.BUDGET_BEFORE, raw.replace(a.BUDGET_BEFORE, b"")):
            with self.assertRaises(a.Reject):
                a.instrument_budget(changed, a.binding(a.BUDGET_PATH, changed),
                                    a.binding(a.BUDGET_PATH, expected))


class SourceControls(unittest.TestCase):
    def test_generation_absent_and_admission_fails_closed(self):
        root = Path(__file__).resolve().parent
        for name in (s.CURRENT_NAME, s.AUTHORITY_NAME, s.PATCH_NAME, "seal.py"):
            self.assertFalse((root / name).exists())
        with self.assertRaises(a.Reject):
            s.admit(root, {}, None, None)

    def test_source_path_validation(self):
        for bad in ("", "/tmp/a", "../a", "src/../a", "src//a", "src/./a", "src\\a"):
            with self.subTest(path=bad), self.assertRaises(a.Reject):
                s.relative(bad)
        self.assertEqual(s.relative("src/a.rs"), "src/a.rs")

    def test_map_order_duplicates_and_fields(self):
        row = a.binding("src/a.rs", b"a")
        other = a.binding("src/z.rs", b"z")
        self.assertEqual(s.rows_map([row, other]), {row["path"]: row, other["path"]: other})
        for rows in ([row, row], [other, row], [{**row, "mode": "100644"}], []):
            with self.assertRaises(a.Reject):
                s.rows_map(rows)

    def test_complete_files_modes_dirs_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, body in (("src/a.rs", b"a"), ("native/proof.txt", b"proof")):
                path = root / name
                path.parent.mkdir(exist_ok=True)
                path.write_bytes(body)
            selected = ["native/proof.txt", "src/a.rs"]
            s.complete_compiler_members(root, selected)
            rows = [a.binding(name, (root / name).read_bytes()) for name in selected]
            s.read_inputs(root, rows)
            empty = root / "src/__pycache__"
            empty.mkdir()
            with self.assertRaises(a.Reject):
                s.complete_compiler_members(root, selected)
            empty.rmdir()
            extra = root / "src/extra.txt"
            extra.write_bytes(b"extra")
            with self.assertRaises(a.Reject):
                s.complete_compiler_members(root, selected)
            extra.unlink()
            target = root / "src/a.rs"
            target.chmod(0o755)
            with self.assertRaises(a.Reject):
                s.read_inputs(root, rows)
            target.chmod(0o644)
            target.unlink()
            target.symlink_to(root / "native/proof.txt")
            with self.assertRaises(a.Reject):
                s.read_inputs(root, rows)

    def test_complete_content_and_git_blob(self):
        row = s.identity("src/a", b"hello\n")
        self.assertEqual(row["git_blob"], "ce013625030ba8dba906f756967f9e9ca394464a")
        self.assertEqual(row["mode"], "100644")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "x").write_bytes(b"wrong")
            with self.assertRaises(a.Reject):
                s.read_inputs(root, [a.binding("x", b"right")])

    def test_predecessor_remains_exact(self):
        raw = (REPO / "tests/fixtures/typed_project_source_binding/current-source.json").read_bytes()
        self.assertEqual((len(raw), a.sha(raw)), (s.PREDECESSOR_BYTES, s.PREDECESSOR_SHA))
        data = json.loads(raw)
        self.assertEqual(len(s.rows_map(data["files"])), 376)
        self.assertEqual(data["reviewed_source_head"], s.PREDECESSOR_HEAD)

    def test_retained_public_api_pins_before_loading(self):
        root = REPO / "tests/fixtures/typed_project_source_binding"
        package = {name: (root / name).read_bytes() for name in
                   ("run.py", "u8_cross_host.py", "byte_storage.py", "package-manifest.json")}
        api, byte = s.retained_public_api(root, package)
        self.assertEqual(byte.SOURCE_SHA, s.PREDECESSOR_SHA)
        self.assertEqual(api.include_directives({"src/a.rs": b'include_str!("a");'}, api),
                         [{"source": "src/a.rs", "ordinal": 0, "expression": 'include_str!("a")'}])
        for name in package:
            changed = dict(package)
            changed[name] += b"\n"
            with self.assertRaises(a.Reject):
                s.retained_public_api(root, changed)


class InventoryControls(unittest.TestCase):
    def test_every_173_direct_entries_required(self):
        audit = (REPO / "docs/architecture/fallible-lexer-source-consumer-audit.md").read_bytes()
        workflow = (REPO / ".github/workflows/ci.yml").read_bytes()
        rows = ci.inventory(audit, workflow)
        self.assertEqual(len(rows), 173)
        plan = [{"id": row["id"], "command_sha256": row["command_sha256"],
                 "disposition": "current-execution", "rationale": "test only"} for row in rows]
        self.assertTrue(ci.validate_plan(rows, plan))
        for changed in (plan[:-1], plan + [plan[-1]], list(reversed(plan))):
            with self.assertRaises(a.Reject):
                ci.validate_plan(rows, changed)
        changed = copy.deepcopy(plan)
        changed[0]["disposition"] = "waived"
        with self.assertRaises(a.Reject):
            ci.validate_plan(rows, changed)
        with self.assertRaises(a.Reject):
            ci.inventory(audit, workflow + b"\n")


if __name__ == "__main__":
    unittest.main()
