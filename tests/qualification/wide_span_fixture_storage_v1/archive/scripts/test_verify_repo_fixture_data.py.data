"""Frozen source data admission is separate from compiler checks and runs."""
import contextlib
import hashlib
import io
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import build_hir_producers_v2
import verify_fixture_data
import verify_repo

STREAMING_ROOT = "fixtures/typed-streaming-lexer/main.ox"
STREAMING_FILES = tuple("fixtures/typed-streaming-lexer/" + name + ".ox"
                        for name in ("main", "data", "frame", "keywords", "scanner"))


DATA = Path("tests/fixtures/fixed_array_source_unit3")
U8_DATA = Path("tests/qualification/bounded_u8_current")
BYTE_DATA = Path("tests/qualification/byte_storage_current")
BYTE_PILOT = "fixtures/typed-byte-storage/main.ox"
U8_MANIFEST = U8_DATA / "source-data-manifest.json"
U8_SOURCES = tuple((U8_DATA / "oracle" / name).as_posix() for name in (
    *(f"pairs-{route}-{batch:02}.ox" for route in ("owned", "scalar") for batch in range(32)),
    "roundtrip-owned.ox", "roundtrip-scalar.ox"))


def copy_u8_data(root):
    destination = root / U8_DATA
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(verify_repo.ROOT / U8_MANIFEST, root / U8_MANIFEST)
    shutil.copytree(verify_repo.ROOT / U8_DATA / "oracle", destination / "oracle")

SAMPLE_MEMBERS = (
    "fixtures/typed-array-samples/main.ox",
    "fixtures/typed-array-samples/stats.ox",
    "fixtures/typed-array-samples/samples.ox",
)
SLICE_SAMPLE_MEMBERS = (
    "fixtures/typed-slice-samples/main.ox",
    "fixtures/typed-slice-samples/buffers.ox",
    "fixtures/typed-slice-samples/stats.ox",
)
EXPRESSION_SAMPLE_MEMBERS = (
    "fixtures/typed-expression-samples/main.ox",
    "fixtures/typed-expression-samples/arena.ox",
    "fixtures/typed-expression-samples/scanner.ox",
    "fixtures/typed-expression-samples/parser.ox",
    "fixtures/typed-expression-samples/evaluator.ox",
)
STDIN_ENTRY = "fixtures/typed-expression-samples/stdin.ox"
STACK_MAIN_ENTRY = "fixtures/typed-expression-samples/stack_main.ox"
STACK_STDIN_ENTRY = "fixtures/typed-expression-samples/stack_stdin.ox"
ARTIFACT_MAIN_ENTRY = "fixtures/typed-expression-samples/artifact_main.ox"
ARTIFACT_LOAD_ENTRY = "fixtures/typed-expression-samples/artifact_load.ox"
ARTIFACT_ADDED_FILES = (
    ARTIFACT_MAIN_ENTRY,
    "fixtures/typed-expression-samples/artifact_writer.ox",
    ARTIFACT_LOAD_ENTRY,
    "fixtures/typed-expression-samples/artifact_reader.ox",
)
STACK_ADDED_FILES = (
    STACK_MAIN_ENTRY,
    "fixtures/typed-expression-samples/stack_code.ox",
    "fixtures/typed-expression-samples/lowering.ox",
    STACK_STDIN_ENTRY,
)
STACK_SAMPLE_MEMBERS = STACK_ADDED_FILES[:-1] + EXPRESSION_SAMPLE_MEMBERS[1:]
LEXER_MAIN_ENTRY = "fixtures/typed-lexer-samples/main.ox"
LEXER_ADMISSION_ENTRY = "fixtures/typed-lexer-samples/admission.ox"
LEXER_ADDED_FILES = (
    LEXER_MAIN_ENTRY,
    "fixtures/typed-lexer-samples/tape.ox",
    "fixtures/typed-lexer-samples/transcript.ox",
    "fixtures/typed-lexer-samples/lexer.ox",
    "fixtures/typed-lexer-samples/keywords.ox",
    LEXER_ADMISSION_ENTRY,
)
LEXER_CORE_ADDED_FILES = (
    "fixtures/typed-lexer-samples/lexer_core.ox",
    "fixtures/typed-lexer-samples/buffers.ox",
)
PARSER_ADMISSION_ENTRY = "fixtures/typed-lexer-samples/parser_admission.ox"
PARSER_ADMISSION_ADDED_FILES = (
    PARSER_ADMISSION_ENTRY,
    "fixtures/typed-lexer-samples/parser_banks.ox",
    "fixtures/typed-lexer-samples/parser_probe_output.ox",
)
PARSER_MAIN_ENTRY = "fixtures/typed-lexer-samples/parser_main.ox"
PARSER_ADDED_FILES = (
    PARSER_MAIN_ENTRY,
    "fixtures/typed-lexer-samples/parser_state.ox",
    "fixtures/typed-lexer-samples/parser_signature.ox",
    "fixtures/typed-lexer-samples/parser_atom.ox",
    "fixtures/typed-lexer-samples/parser_call.ox",
    "fixtures/typed-lexer-samples/parser_expression.ox",
    "fixtures/typed-lexer-samples/parser_statement.ox",
    "fixtures/typed-lexer-samples/parser_control.ox",
    "fixtures/typed-lexer-samples/parser_driver.ox",
    "fixtures/typed-lexer-samples/parser_output.ox",
)
STATIC_ROOTS = tuple("fixtures/typed-lexer-samples/" + name + ".ox" for name in (
    "static_main resolver_main typed_main ast_consumer_probe ast_static_main").split())
STATIC_ADDED_FILES = tuple("fixtures/typed-lexer-samples/" + name + ".ox" for name in (
    "ast_block ast_consumer_probe ast_expression ast_input ast_output ast_statement "
    "ast_static_main ast_validate resolver_driver resolver_main resolver_names resolver_output "
    "static_column static_common static_main static_output static_probe static_state "
    "typed_diagnostic typed_driver typed_expression typed_main typed_output typed_statement").split())
V2_OVERLAY_FILES = tuple("fixtures/typed-frontend-v2/" + name + ".ox" for name in (
    "ast_input ast_output ast_static_main lexer_core parser_main parser_output "
    "source_buffer typed_diagnostic typed_output").split())
V2_ROOTS = ("parser_main", "ast_static_main")
SAMPLE_PROJECTS = (
    ("tests/fixtures/bounded_enum_scanner/main.ox",
     "tests/fixtures/bounded_enum_scanner/scanner.ox"),
    ("fixtures/typed-record-composition-samples/main.ox",
     "fixtures/typed-record-composition-samples/model.ox",
     "fixtures/typed-record-composition-samples/ops.ox"),SAMPLE_MEMBERS, SLICE_SAMPLE_MEMBERS,
    EXPRESSION_SAMPLE_MEMBERS)
ALL_SAMPLE_MEMBERS = tuple(name for members in SAMPLE_PROJECTS for name in members)


def discover(root):
    return sorted(p for p in root.rglob("*.ox") if ".oxid" not in p.parts and "target" not in p.parts)


def fingerprint(values):
    return hashlib.sha256(json.dumps(values, separators=(",", ":")).encode()).hexdigest()


class PublishedRegistrationTests(unittest.TestCase):
    def test_full_published_inputs_and_exact_predecessor_inventory(self):
        root = verify_repo.ROOT
        data_sources = verify_fixture_data.fixture_data_sources(root)
        u8_sources = {root / name for name in U8_SOURCES}
        self.assertEqual(len(u8_sources), 66)
        self.assertTrue(u8_sources <= data_sources)
        self.assertEqual(len(data_sources - u8_sources), 122)
        self.assertEqual(len(data_sources), 188)
        self.assertEqual(verify_fixture_data.SOURCE_DATA_MANIFESTS[-1][0], U8_MANIFEST.as_posix())
        registered = set()
        for (relative, _), body_count, source_count in zip(
                verify_fixture_data.SOURCE_DATA_MANIFESTS[:-1], (120, 10, 23, 20), (101, 6, 10, 5), strict=True):
            manifest = json.loads((root / relative).read_bytes())
            self.assertEqual(len(manifest["files"]), body_count)
            self.assertEqual(sum(name.endswith(".ox") for name in manifest["files"]), source_count)
            registered.add(root / relative)
            registered.update((root / relative).parent / name for name in manifest["files"])
        # Replay documentation shares this directory but is not source data.
        # Pin the registered closure; unlisted .ox files still reach source_plan.
        self.assertEqual(len(registered), 177)
        u8_manifest = json.loads((root / U8_MANIFEST).read_bytes())
        self.assertEqual(len(u8_manifest["files"]), 67)
        self.assertEqual({root / U8_DATA / name for name in u8_manifest["files"] if name.endswith(".ox")}, u8_sources)
        u8_registered = {root / U8_MANIFEST} | {root / U8_DATA / name for name in u8_manifest["files"]}
        self.assertFalse(registered & u8_registered)
        self.assertEqual(len(registered | u8_registered), 245)
        self.assertTrue(all(path.is_file() for path in registered | u8_registered))
        sources = discover(root)
        checks, typed_entries, typed_members = verify_repo.source_plan(sources, root)
        byte_data = verify_repo.byte_storage_fixture_data_sources(root)
        self.assertEqual(len(byte_data), 325)
        self.assertFalse(data_sources & byte_data)
        self.assertEqual(verify_repo.fixture_data_sources(root), data_sources | byte_data)
        self.assertEqual(len(data_sources | byte_data), 188 + 325)
        language = [p.relative_to(root).as_posix() for p in sources if p not in data_sources | byte_data]
        check_inventory = [(p.relative_to(root).as_posix(), typed) for p, typed in checks]
        run_inventory = [(p.relative_to(root).as_posix(), False) for p in verify_repo.runnable_sources(root)]
        run_inventory += [(p.relative_to(root).as_posix(), True) for p in typed_entries]
        # Explicit RFC0031 runnable successor; preserve every predecessor equation.
        self.assertEqual([name for name in language if name == BYTE_PILOT], [BYTE_PILOT])
        self.assertEqual([row for row in check_inventory if row[0] == BYTE_PILOT], [(BYTE_PILOT, True)])
        self.assertEqual([row for row in run_inventory if row[0] == BYTE_PILOT], [(BYTE_PILOT, True)])
        self.assertFalse(any(path.relative_to(root).as_posix() in language for path in byte_data))
        language = [name for name in language if name != BYTE_PILOT]
        check_inventory = [row for row in check_inventory if row[0] != BYTE_PILOT]
        run_inventory = [row for row in run_inventory if row[0] != BYTE_PILOT]
        typed_members -= 1
        typed_entries = [path for path in typed_entries if path != root / BYTE_PILOT]
        # The genuine streaming lexer is five new language files and one
        # check-only root. Subtract only this exact successor before replaying
        # every historical inventory and fingerprint below.
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_PROJECTS[0], STREAMING_ROOT)
        self.assertEqual(verify_repo.TYPED_PROJECTS[STREAMING_ROOT], STREAMING_FILES)
        self.assertEqual([name for name in language if name in STREAMING_FILES], sorted(STREAMING_FILES))
        self.assertEqual([row for row in check_inventory if row[0] in STREAMING_FILES], [(STREAMING_ROOT, True)])
        self.assertFalse(any(row[0] in STREAMING_FILES for row in run_inventory))
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (209, 143, 75, 79, 8))
        language = [name for name in language if name not in STREAMING_FILES]
        check_inventory = [row for row in check_inventory if row[0] not in STREAMING_FILES]
        typed_members -= len(STREAMING_FILES)
        # Exactly nine new language members belong to the materialized v2
        # category. Every old language/check/run identity remains unchanged.
        self.assertEqual(verify_repo.TYPED_OVERLAY_FILES, V2_OVERLAY_FILES)
        self.assertEqual(verify_repo.TYPED_OVERLAY_ROOTS, V2_ROOTS)
        self.assertEqual([name for name in language if name in V2_OVERLAY_FILES], list(V2_OVERLAY_FILES))
        self.assertFalse(any(name in V2_OVERLAY_FILES for name, _ in check_inventory + run_inventory))
        self.assertTrue(all(root / name not in data_sources for name in V2_OVERLAY_FILES))
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (204, 142, 75, 74, 8))
        self.assertEqual(len(check_inventory) + len(V2_ROOTS), 144)
        language = [name for name in language if name not in V2_OVERLAY_FILES]
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_PROJECTS[1:-5],
                         (ARTIFACT_MAIN_ENTRY, ARTIFACT_LOAD_ENTRY, LEXER_MAIN_ENTRY,
                          LEXER_ADMISSION_ENTRY, PARSER_ADMISSION_ENTRY, PARSER_MAIN_ENTRY))
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_PROJECTS[-5:], STATIC_ROOTS)
        self.assertEqual([name for name in language if name in STATIC_ADDED_FILES], sorted(STATIC_ADDED_FILES))
        self.assertEqual([row for row in check_inventory if row[0] in STATIC_ADDED_FILES],
                         [(root, True) for root in STATIC_ROOTS])
        self.assertFalse(any(row[0] in STATIC_ADDED_FILES for row in run_inventory))
        self.assertTrue(all(root / name not in data_sources for name in STATIC_ADDED_FILES))
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (195, 142, 75, 74, 8))
        # Remove only the exact 24 static additions and five check-only roots.
        # The complete parser predecessor and every older fingerprint survive.
        language = [name for name in language if name not in STATIC_ADDED_FILES]
        check_inventory = [row for row in check_inventory if row[0] not in STATIC_ADDED_FILES]
        typed_members -= len(STATIC_ADDED_FILES)
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (171, 137, 75, 50, 8))
        self.assertEqual(fingerprint(language), "46eaccd1162463c996d4dbe2bc1ccff2fdbdb4d143980eb22de7d1222f34e8c9")
        self.assertEqual(fingerprint(check_inventory), "3e09782ec358fcf00b56b31676adc6c8ec63c2a22842649610f2001418df3a36")
        self.assertEqual(fingerprint(run_inventory), "4d05786c555a004ab645edb9daf8c8f811201cfdb60c9ed3e516d11aaec181f4")
        self.assertEqual([name for name in language if name in PARSER_ADDED_FILES], sorted(PARSER_ADDED_FILES))
        self.assertEqual([row for row in check_inventory if row[0] in PARSER_ADDED_FILES],
                         [(PARSER_MAIN_ENTRY, True)])
        self.assertFalse(any(row[0] in PARSER_ADDED_FILES for row in run_inventory))
        self.assertTrue(all(root / name not in data_sources for name in PARSER_ADDED_FILES))
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (161 + len(PARSER_ADDED_FILES), 136 + 1, 75, 40 + len(PARSER_ADDED_FILES), 8))
        # The real parser adds ten files and one check-only root. Capture its
        # exact predecessor independently, preserving the synthetic carrier,
        # every old language/check/run identity, and all frozen fixture data.
        language = [name for name in language if name not in PARSER_ADDED_FILES]
        check_inventory = [row for row in check_inventory if row[0] not in PARSER_ADDED_FILES]
        typed_members -= len(PARSER_ADDED_FILES)
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (161, 136, 75, 40, 8))
        self.assertEqual(fingerprint(language), "cc2c9cf7e672dab9f5adb7f936b9a77c494af7e6e526848fd0a2af4d7c58f4c7")
        self.assertEqual(fingerprint(check_inventory), "36348af0b0d0985d7fc9a706b2dd278c362147b882ff4c87d59544cb213d0f57")
        self.assertEqual(fingerprint(run_inventory), "4d05786c555a004ab645edb9daf8c8f811201cfdb60c9ed3e516d11aaec181f4")
        added_names = LEXER_CORE_ADDED_FILES + PARSER_ADMISSION_ADDED_FILES
        self.assertEqual([name for name in language if name in added_names], sorted(added_names))
        self.assertEqual([row for row in check_inventory if row[0] in added_names],
                         [(PARSER_ADMISSION_ENTRY, True)])
        self.assertFalse(any(row[0] in added_names for row in run_inventory))
        self.assertTrue(all(root / name not in data_sources for name in added_names))
        # Preserve the original lexer roster by subtracting only the two
        # extracted modules and three synthetic carrier files. No parser claim.
        language = [name for name in language if name not in added_names]
        check_inventory = [row for row in check_inventory if row[0] not in added_names]
        typed_members -= len(added_names)
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (156, 135, 75, 35, 8))
        self.assertEqual([name for name in language if name in LEXER_ADDED_FILES], sorted(LEXER_ADDED_FILES))
        self.assertEqual([row for row in check_inventory if row[0] in LEXER_ADDED_FILES],
                         [(LEXER_MAIN_ENTRY, True), (LEXER_ADMISSION_ENTRY, True)])
        self.assertFalse(any(row[0] in LEXER_ADDED_FILES for row in run_inventory))
        self.assertTrue(all(root / name not in data_sources for name in LEXER_ADDED_FILES))
        # Both input-requiring lexer roots use process mode only in their
        # dedicated verifier. Subtract the six exact members and two checks.
        language = [name for name in language if name not in LEXER_ADDED_FILES]
        check_inventory = [row for row in check_inventory if row[0] not in LEXER_ADDED_FILES]
        typed_members -= len(LEXER_ADDED_FILES)
        self.assertEqual([name for name in language if name in ARTIFACT_ADDED_FILES], sorted(ARTIFACT_ADDED_FILES))
        self.assertEqual([row for row in check_inventory if row[0] in ARTIFACT_ADDED_FILES],
                         [(ARTIFACT_MAIN_ENTRY, True), (ARTIFACT_LOAD_ENTRY, True)])
        self.assertFalse(any(row[0] in ARTIFACT_ADDED_FILES for row in run_inventory))
        self.assertTrue(all(root / name not in data_sources for name in ARTIFACT_ADDED_FILES))
        # Subtract exactly the four artifact files and their two input-requiring
        # root checks. No runnable entry or historical source identity changes.
        language = [name for name in language if name not in ARTIFACT_ADDED_FILES]
        check_inventory = [row for row in check_inventory if row[0] not in ARTIFACT_ADDED_FILES]
        typed_members -= len(ARTIFACT_ADDED_FILES)
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_FILES, (STDIN_ENTRY, STACK_STDIN_ENTRY))
        self.assertEqual(verify_repo.TYPED_PROJECTS[STACK_MAIN_ENTRY], STACK_SAMPLE_MEMBERS)
        self.assertEqual([name for name in language if name in STACK_ADDED_FILES], sorted(STACK_ADDED_FILES))
        self.assertEqual([row for row in check_inventory if row[0] in STACK_ADDED_FILES],
                         [(STACK_MAIN_ENTRY, True), (STACK_STDIN_ENTRY, True)])
        self.assertEqual([row for row in run_inventory if row[0] in STACK_ADDED_FILES], [(STACK_MAIN_ENTRY, True)])
        self.assertTrue(all(root / name not in data_sources for name in STACK_ADDED_FILES))
        # Only four new files, two root checks, and one fixed entry run are
        # added. The shared expression modules still count as single members.
        language = [name for name in language if name not in STACK_ADDED_FILES]
        check_inventory = [row for row in check_inventory if row[0] not in STACK_ADDED_FILES]
        run_inventory = [row for row in run_inventory if row[0] not in STACK_ADDED_FILES]
        typed_entries = [entry for entry in typed_entries if entry.relative_to(root).as_posix() not in STACK_ADDED_FILES]
        typed_members -= len(STACK_ADDED_FILES)
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (142, 129, 74, 21, 7))
        self.assertEqual([name for name in language if name == STDIN_ENTRY], [STDIN_ENTRY])
        self.assertEqual([row for row in check_inventory if row[0] == STDIN_ENTRY], [(STDIN_ENTRY, True)])
        self.assertFalse(any(row[0] == STDIN_ENTRY for row in run_inventory))
        self.assertNotIn(root / STDIN_ENTRY, data_sources)
        # Subtract only the effectful root's check and member. It adds no run,
        # and all earlier source, check and run inventories remain frozen.
        language = [name for name in language if name != STDIN_ENTRY]
        check_inventory = [row for row in check_inventory if row[0] != STDIN_ENTRY]
        typed_members -= 1
        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),
                         (141, 128, 74, 20, 7))
        # Preserve the exact pre-expression totals using only its five named
        # members and single typed root check/run as the subtraction.
        self.assertEqual((len([name for name in language if name not in EXPRESSION_SAMPLE_MEMBERS]),
                          len([row for row in check_inventory if row[0] not in EXPRESSION_SAMPLE_MEMBERS]),
                          len([row for row in run_inventory if row[0] not in EXPRESSION_SAMPLE_MEMBERS]),
                          typed_members - len(EXPRESSION_SAMPLE_MEMBERS),
                          len([entry for entry in typed_entries
                               if entry.relative_to(root).as_posix() not in EXPRESSION_SAMPLE_MEMBERS])),
                         (136, 127, 73, 15, 6))
        # Each explicitly named public project adds one typed root check/run.
        # Children remain language sources, never frozen data or standalone runs.
        for members in SAMPLE_PROJECTS:
            with self.subTest(sample=members[0]):
                self.assertEqual([name for name in language if name in members], sorted(members))
                self.assertTrue(all(root / name not in data_sources for name in members))
                self.assertEqual([row for row in check_inventory if row[0] in members], [(members[0], True)])
                self.assertEqual([row for row in run_inventory if row[0] in members], [(members[0], True)])
        # Preserve fingerprints captured at the exact predecessor of the
        # published 7b362af fixture-only commit. Subtract only the explicit
        # array, slice, composition, enum and expression sample deltas, never a directory or path class.
        predecessor_language = [name for name in language if name not in ALL_SAMPLE_MEMBERS]
        predecessor_checks = [row for row in check_inventory if row[0] not in ALL_SAMPLE_MEMBERS]
        predecessor_runs = [row for row in run_inventory if row[0] not in ALL_SAMPLE_MEMBERS]
        self.assertEqual(fingerprint(predecessor_language), "7bba06dbfb7fa7cbe33ed94eb0201224cb3633f0048d621de30a7630adcce1b4")
        self.assertEqual(fingerprint(predecessor_checks), "38a388157d6bef824ab927a6f06021349e4a6e4a87ada8edd6ffea61873493ed")
        self.assertEqual(fingerprint(predecessor_runs), "ff2cfd1a71b69c54062c03ea03928945ef3a00eb83c513fd96b05360897afe9e")

    def test_main_reports_data_separately_and_never_sends_it_to_compiler(self):
        output = io.StringIO()
        with patch.object(sys, "argv", ["verify_repo.py", sys.executable]), \
                patch.object(verify_repo, "run") as run, \
                patch.object(verify_repo, "verify_typed_formatter") as formatter, \
                contextlib.redirect_stdout(output):
            self.assertEqual(verify_repo.main(), 0)
        formatter.assert_called_once_with(Path(sys.executable).resolve())
        commands = [call.args[0] for call in run.call_args_list]
        byte_pilot = str(verify_repo.ROOT / BYTE_PILOT)
        self.assertEqual([command[1] for command in commands if byte_pilot in command], ["check", "run"])
        self.assertFalse(any(str(path) in command for path in verify_repo.byte_storage_fixture_data_sources(verify_repo.ROOT)
                             for command in commands))
        commands = [command for command in commands if byte_pilot not in command]
        overlay_commands = [command for command in commands if len(command) > 2
                            and Path(command[2]).parent.name == "typed-frontend-v2"]
        self.assertEqual(len(overlay_commands), 2)
        self.assertEqual([Path(command[2]).stem for command in overlay_commands], list(V2_ROOTS))
        for command in overlay_commands:
            self.assertEqual(command[:2], [str(Path(sys.executable).resolve()), "check"])
            self.assertEqual(command[3:], ["--edition=typed-preview"])
            self.assertFalse(Path(command[2]).is_relative_to(verify_repo.ROOT))
        self.assertFalse(any(str(verify_repo.ROOT / name) in command
                             for name in V2_OVERLAY_FILES for command in commands))
        for relative in (STREAMING_ROOT, STDIN_ENTRY, STACK_STDIN_ENTRY, ARTIFACT_MAIN_ENTRY, ARTIFACT_LOAD_ENTRY,
                         LEXER_MAIN_ENTRY, LEXER_ADMISSION_ENTRY, PARSER_ADMISSION_ENTRY, PARSER_MAIN_ENTRY) + STATIC_ROOTS:
            stdin_root = str(verify_repo.ROOT / relative)
            self.assertEqual([command for command in commands if stdin_root in command],
                             [[str(Path(sys.executable).resolve()), "check", stdin_root, "--edition=typed-preview"]])
        added_roots = {str(verify_repo.ROOT / name) for name in
                       (STREAMING_ROOT, STDIN_ENTRY, STACK_MAIN_ENTRY, STACK_STDIN_ENTRY, ARTIFACT_MAIN_ENTRY,
                        ARTIFACT_LOAD_ENTRY, LEXER_MAIN_ENTRY, LEXER_ADMISSION_ENTRY,
                        PARSER_ADMISSION_ENTRY, PARSER_MAIN_ENTRY) + STATIC_ROOTS}
        predecessor_commands = [command for command in commands
                                if command not in overlay_commands and not added_roots.intersection(command)]
        self.assertEqual(len(predecessor_commands), 205)  # 128 checks, 74 runs, test/build/doctor.
        self.assertEqual(sum("--edition=typed-preview" in command for command in predecessor_commands), 14)
        for members in SAMPLE_PROJECTS + (STACK_SAMPLE_MEMBERS,):
            with self.subTest(sample=members[0]):
                sample_root = str(verify_repo.ROOT / members[0])
                sample_commands = [command for command in commands if sample_root in command]
                self.assertEqual(len(sample_commands), 2)
                self.assertEqual({command[1] for command in sample_commands}, {"check", "run"})
                self.assertTrue(all("--edition=typed-preview" in command for command in sample_commands))
                for child in members[1:]:
                    self.assertFalse(any(str(verify_repo.ROOT / child) in command for command in commands))
        self.assertFalse(any("fixed_array_source_unit3" in arg
                             for call in run.call_args_list for arg in call.args[0]))
        self.assertFalse(any(str(verify_repo.ROOT / name) in command
                             for name in U8_SOURCES for command in commands))
        for relative in PARSER_ADDED_FILES[1:] + tuple(name for name in STATIC_ADDED_FILES if name not in STATIC_ROOTS):
            self.assertFalse(any(str(verify_repo.ROOT / relative) in command for command in commands))
        self.assertIn("fixture-data validation passed: 513 source-only files", output.getvalue())
        self.assertIn("no compiler checks, executions or feature claim", output.getvalue())
        additional_members = (ARTIFACT_ADDED_FILES + LEXER_ADDED_FILES + LEXER_CORE_ADDED_FILES
                              + PARSER_ADMISSION_ADDED_FILES + PARSER_ADDED_FILES + STATIC_ADDED_FILES + STREAMING_FILES)
        self.assertIn(f"{146 + 1 + len(additional_members) + len(V2_OVERLAY_FILES)} language sources, "
                      f"{131 + 1 + len(verify_repo.TYPED_CHECK_ONLY_PROJECTS) + len(V2_ROOTS)} checks, "
                      "76 runnable programs", output.getvalue())
        self.assertIn(f"121 legacy sources, 67 legacy runnable programs, "
                      f"{25 + 1 + len(additional_members)} typed source members / "
                      "9 typed entry runs", output.getvalue())
        self.assertIn("9 v2 overlay source members / 2 materialized typed checks in a 31-module closure",
                      output.getvalue())

    def test_v2_closure_matches_builder_and_checks_materialized_byte_identities(self):
        root = verify_repo.ROOT
        sources = verify_repo.typed_overlay_sources(root)
        self.assertEqual(len(sources), 31)
        overlay_paths = {root / name for name in V2_OVERLAY_FILES}
        closure_paths = {path for path, _ in sources.values()}
        self.assertEqual(len(closure_paths & overlay_paths), 9)
        self.assertEqual(len(closure_paths - overlay_paths), 22)
        self.assertTrue(closure_paths - overlay_paths <= {
            root / name for members in verify_repo.TYPED_PROJECTS.values() for name in members})
        # Independent declaration walk and the real v2 builder both identify
        # the same closed source set. Neither compiler mock creates this proof.
        pending, reached = list(V2_ROOTS), set()
        while pending:
            name = pending.pop()
            if name in reached:
                continue
            reached.add(name)
            pending.extend(re.findall(r"\bmod\s+([A-Za-z_]\w*)\s*;", sources[name][1].decode()))
        self.assertEqual(reached, set(sources))
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "checked"
            built = Path(temporary) / "built"
            build_hir_producers_v2.materialize(built)
            called = []

            def inspect_check(command):
                called.append(command)
                self.assertEqual(command[1], "check")
                self.assertEqual(command[3:], ["--edition=typed-preview"])
                self.assertEqual({path.name for path in destination.iterdir()},
                                 {name + ".ox" for name in sources})
                for name, (original, body) in sources.items():
                    self.assertEqual((destination / (name + ".ox")).read_bytes(), body)
                    self.assertEqual(body, original.read_bytes())
                    self.assertEqual(body, (built / (name + ".ox")).read_bytes())

            with patch.object(verify_repo, "run", side_effect=inspect_check):
                verify_repo.check_typed_overlays(Path(sys.executable), sources, destination)
            self.assertEqual([Path(command[2]).stem for command in called], list(V2_ROOTS))

    @unittest.skipUnless(shutil.which("git"), "Git is required for checkout conversion control")
    def test_git_autocrlf_preserves_frozen_bytes_and_converts_other_text(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "index"
            checkout = Path(directory) / "checkout"
            root.mkdir()
            shutil.copy2(verify_repo.ROOT / ".gitattributes", root / ".gitattributes")
            shutil.copytree(verify_repo.ROOT / DATA, root / DATA)
            copy_u8_data(root)
            # Published bodies happen to be LF-only. This temporary, unregistered
            # source proves that the attribute also preserves intentional CRLF.
            (root / DATA / "contracts-v2/checkout-crlf-control.ox").write_bytes(b"// control\r\n")
            (root / "ordinary.txt").write_bytes(b"ordinary\ntext\n")

            def git(*args):
                return subprocess.run(["git", "-c", "core.autocrlf=true", "-c", "core.eol=crlf",
                                       "-c", "core.safecrlf=false", *args], cwd=root,
                                      check=True, text=True, capture_output=True)

            git("init", "--quiet")
            git("add", "--", ".gitattributes", DATA.as_posix(), U8_DATA.as_posix(), "ordinary.txt")
            git("checkout-index", "--all", "--force", "--prefix", checkout.as_posix() + "/")
            self.assertEqual((checkout / "ordinary.txt").read_bytes(), b"ordinary\r\ntext\r\n")
            self.assertEqual(len(verify_fixture_data.fixture_data_sources(checkout)), 188)
            scope_files = [p for folder in (DATA, U8_DATA) for p in (root / folder).rglob("*") if p.is_file()]
            for source in scope_files:
                self.assertEqual((checkout / source.relative_to(root)).read_bytes(), source.read_bytes())


class FixtureAdmissionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        shutil.copytree(verify_repo.ROOT / DATA, self.root / DATA)
        copy_u8_data(self.root)
        shutil.copytree(verify_repo.ROOT / BYTE_DATA, self.root / BYTE_DATA)
        self.manifest = self.root / verify_fixture_data.SOURCE_DATA_MANIFESTS[0][0]
        self.document = json.loads(self.manifest.read_bytes())
        self.source = next(self.manifest.parent / name for name in self.document["files"] if name.endswith(".ox"))
        for relative in verify_repo.TYPED_SOURCE_FILES + verify_repo.TYPED_CHECK_ONLY_FILES:
            self.write(relative, b"// typed inventory member\n")
        for members in verify_repo.TYPED_PROJECTS.values():
            for relative in members:
                self.write(relative, b"// typed inventory member\n")

    def write(self, relative, body):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)
        return path

    def assert_samples_use_root(self, checks, entries):
        checked = [path for path, _ in checks]
        for members in SAMPLE_PROJECTS + (STACK_SAMPLE_MEMBERS,):
            sample_root = self.root / members[0]
            self.assertIn((sample_root, True), checks)
            self.assertIn(sample_root, entries)
            for child in members[1:]:
                self.assertNotIn(self.root / child, checked)
                self.assertNotIn(self.root / child, entries)

    def assert_stdin_addition(self, checks, entries, count, predecessor_check_count):
        byte_pilot = self.root / BYTE_PILOT
        self.assertEqual([row for row in checks if row[0] == byte_pilot], [(byte_pilot, True)])
        self.assertEqual([path for path in entries if path == byte_pilot], [byte_pilot])
        checks = [row for row in checks if row[0] != byte_pilot]
        entries = [path for path in entries if path != byte_pilot]
        count -= 1
        streaming = {self.root / name for name in STREAMING_FILES}
        self.assertEqual([row for row in checks if row[0] in streaming], [(self.root / STREAMING_ROOT, True)])
        self.assertFalse(any(entry in streaming for entry in entries))
        checks = [row for row in checks if row[0] not in streaming]
        count -= len(STREAMING_FILES)
        static_files = {self.root / name for name in STATIC_ADDED_FILES}
        self.assertEqual([row for row in checks if row[0] in static_files],
                         [(self.root / root, True) for root in STATIC_ROOTS])
        self.assertFalse(any(entry in static_files for entry in entries))
        checks = [row for row in checks if row[0] not in static_files]
        count -= len(STATIC_ADDED_FILES)
        parser_files = {self.root / name for name in PARSER_ADDED_FILES}
        self.assertEqual([row for row in checks if row[0] in parser_files],
                         [(self.root / PARSER_MAIN_ENTRY, True)])
        self.assertFalse(any(entry in parser_files for entry in entries))
        checks = [row for row in checks if row[0] not in parser_files]
        count -= len(PARSER_ADDED_FILES)
        added_names = LEXER_CORE_ADDED_FILES + PARSER_ADMISSION_ADDED_FILES
        added = {self.root / name for name in added_names}
        self.assertEqual([row for row in checks if row[0] in added],
                         [(self.root / PARSER_ADMISSION_ENTRY, True)])
        self.assertFalse(any(entry in added for entry in entries))
        checks = [row for row in checks if row[0] not in added]
        count -= len(added_names)
        lexer_files = {self.root / name for name in LEXER_ADDED_FILES}
        self.assertEqual([row for row in checks if row[0] in lexer_files],
                         [(self.root / LEXER_MAIN_ENTRY, True), (self.root / LEXER_ADMISSION_ENTRY, True)])
        self.assertFalse(any(entry in lexer_files for entry in entries))
        checks = [row for row in checks if row[0] not in lexer_files]
        count -= len(LEXER_ADDED_FILES)
        artifact_files = {self.root / name for name in ARTIFACT_ADDED_FILES}
        self.assertEqual([row for row in checks if row[0] in artifact_files],
                         [(self.root / ARTIFACT_MAIN_ENTRY, True), (self.root / ARTIFACT_LOAD_ENTRY, True)])
        self.assertFalse(any(entry in artifact_files for entry in entries))
        checks = [row for row in checks if row[0] not in artifact_files]
        count -= len(ARTIFACT_ADDED_FILES)
        added = {self.root / name for name in STACK_ADDED_FILES}
        stack_entry = self.root / STACK_MAIN_ENTRY
        stack_stdin = self.root / STACK_STDIN_ENTRY
        self.assertEqual([row for row in checks if row[0] in added], [(stack_entry, True), (stack_stdin, True)])
        self.assertEqual([entry for entry in entries if entry in added], [stack_entry])
        checks = [row for row in checks if row[0] not in added]
        entries = [entry for entry in entries if entry not in added]
        count -= len(STACK_ADDED_FILES)
        stdin_entry = self.root / STDIN_ENTRY
        self.assertEqual([row for row in checks if row[0] == stdin_entry], [(stdin_entry, True)])
        self.assertNotIn(stdin_entry, entries)
        self.assertEqual((len([row for row in checks if row[0] != stdin_entry]), len(entries), count - 1),
                         (predecessor_check_count, 7, 20))

    def repinned(self, raw):
        # Exercise malformed registration handling beyond the production digest
        # barrier. Production pins and all public originals remain unchanged.
        self.manifest.write_bytes(raw)
        registrations = list(verify_fixture_data.SOURCE_DATA_MANIFESTS)
        registrations[0] = (registrations[0][0], hashlib.sha256(raw).hexdigest())
        return patch.object(verify_fixture_data, "SOURCE_DATA_MANIFESTS", tuple(registrations))

    def assert_no_compiler(self, pattern):
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(verify_repo, "ROOT", self.root))
            stack.enter_context(patch.object(sys, "argv", ["verify_repo.py", sys.executable]))
            for name in ("verify_feature_status", "verify_versions", "verify_readmes", "verify_assets",
                         "verify_local_markdown_links"):
                stack.enter_context(patch.object(verify_repo, name))
            compiler = stack.enter_context(patch.object(verify_repo.subprocess, "run"))
            with self.assertRaisesRegex(RuntimeError, pattern):
                verify_repo.main()
            compiler.assert_not_called()

    def test_u8_oracle_missing_source_expectations_and_manifest_fail_before_compiler(self):
        for relative in (Path(U8_SOURCES[0]), U8_DATA / "oracle/coverage.json", U8_MANIFEST):
            path = self.root / relative
            original = path.read_bytes()
            with self.subTest(path=relative):
                path.unlink()
                self.assert_no_compiler("missing or non-file")
                path.write_bytes(original)

    def test_u8_oracle_body_mutation_fails_before_compiler(self):
        for relative in (Path(U8_SOURCES[0]), Path(U8_SOURCES[-1]), U8_DATA / "oracle/coverage.json"):
            path = self.root / relative
            original = path.read_bytes()
            with self.subTest(path=relative):
                path.write_bytes(bytes([original[0] ^ 1]) + original[1:])
                self.assert_no_compiler("body identity mismatch")
                path.write_bytes(original)

    def test_u8_rehashed_missing_or_extra_registration_is_not_authority(self):
        path = self.root / U8_MANIFEST
        original = path.read_bytes()
        for extra in (False, True):
            document = json.loads(original)
            if extra:
                document["files"]["oracle/unreviewed.ox"] = {"bytes": 0, "sha256": hashlib.sha256(b"").hexdigest()}
            else:
                document["files"].pop("oracle/pairs-owned-00.ox")
            with self.subTest(extra=extra):
                path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")
                self.assert_no_compiler("manifest digest mismatch")
        path.write_bytes(original)

    def test_u8_missing_discovery_is_rejected_and_unlisted_neighbor_stays_legacy(self):
        source = self.root / U8_SOURCES[0]
        with self.assertRaisesRegex(RuntimeError, "data missing from discovery"):
            verify_repo.source_plan([path for path in discover(self.root) if path != source], self.root)
        extra = self.write(U8_DATA / "oracle/unlisted.ox", b"invalid candidate\n")
        checks, entries, count = verify_repo.source_plan(discover(self.root), self.root)
        self.assertIn((extra, False), checks)
        self.assertNotIn(extra, entries)
        self.assertFalse(any(path == self.root / name for path, _ in checks for name in U8_SOURCES))

    def test_changed_source_even_same_size_fails_before_compiler(self):
        body = self.source.read_bytes()
        self.source.write_bytes(bytes([body[0] ^ 1]) + body[1:])
        self.assert_no_compiler("body identity mismatch")

    def test_changed_expectations_fail_before_compiler(self):
        (self.manifest.parent / "cases.json").write_bytes(b"{}\n")
        self.assert_no_compiler("body identity mismatch")

    def test_missing_source_fails_before_compiler(self):
        self.source.unlink()
        self.assert_no_compiler("missing or non-file")

    def test_missing_non_source_body_fails_before_compiler(self):
        (self.manifest.parent / "README.md").unlink()
        self.assert_no_compiler("missing or non-file")

    def test_missing_required_manifest_fails_before_compiler(self):
        self.manifest.unlink()
        self.assert_no_compiler("missing or non-file")

    def test_stale_manifest_fails_before_compiler(self):
        self.manifest.write_bytes(self.manifest.read_bytes() + b"\n")
        self.assert_no_compiler("manifest digest mismatch")

    def test_changed_supplement_fails_before_compiler(self):
        supplement = self.root / verify_fixture_data.SOURCE_DATA_MANIFESTS[1][0]
        (supplement.parent / "cases.json").write_bytes(b"{}\n")
        self.assert_no_compiler("body identity mismatch")

    def test_changed_typing_source_fails_before_compiler(self):
        typing = self.root / verify_fixture_data.SOURCE_DATA_MANIFESTS[2][0]
        source = typing.parent / "fixtures/guard-array-free/main.ox"
        body = source.read_bytes()
        source.write_bytes(bytes([body[0] ^ 1]) + body[1:])
        self.assert_no_compiler("body identity mismatch")

    def test_changed_typing_expectations_fail_before_compiler(self):
        typing = self.root / verify_fixture_data.SOURCE_DATA_MANIFESTS[2][0]
        (typing.parent / "positive-facts.json").write_bytes(b"{}\n")
        self.assert_no_compiler("body identity mismatch")

    def test_missing_typing_manifest_fails_before_compiler(self):
        (self.root / verify_fixture_data.SOURCE_DATA_MANIFESTS[2][0]).unlink()
        self.assert_no_compiler("missing or non-file")

    def test_unlisted_typing_source_remains_a_language_check(self):
        extra = self.write(DATA / "typing-contracts-v1/fixtures/unlisted.ox", b"invalid candidate\n")
        checks, entries, count = verify_repo.source_plan(discover(self.root), self.root)
        self.assert_stdin_addition(checks, entries, count, 8)
        self.assert_samples_use_root(checks, entries)
        self.assertIn((extra, False), checks)

    def test_changed_lowering_source_fails_before_compiler(self):
        lowering = self.root / verify_fixture_data.SOURCE_DATA_MANIFESTS[3][0]
        source = lowering.parent / "authority/checkpoint-01/fixtures/rhs-snapshot-success/main.ox"
        body = source.read_bytes()
        source.write_bytes(bytes([body[0] ^ 1]) + body[1:])
        self.assert_no_compiler("body identity mismatch")

    def test_changed_lowering_correction_fails_before_compiler(self):
        lowering = self.root / verify_fixture_data.SOURCE_DATA_MANIFESTS[3][0]
        (lowering.parent / "authority/checkpoint-01-correction-2/correction.json").write_bytes(b"{}\n")
        self.assert_no_compiler("body identity mismatch")

    def test_missing_lowering_manifest_fails_before_compiler(self):
        (self.root / verify_fixture_data.SOURCE_DATA_MANIFESTS[3][0]).unlink()
        self.assert_no_compiler("missing or non-file")

    def test_unlisted_lowering_source_remains_a_language_check(self):
        extra = self.write(DATA / "lowering-contracts-v1/unlisted.ox", b"invalid candidate\n")
        checks, entries, count = verify_repo.source_plan(discover(self.root), self.root)
        self.assert_stdin_addition(checks, entries, count, 8)
        self.assert_samples_use_root(checks, entries)
        self.assertIn((extra, False), checks)

    def test_missing_discovered_source_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "data missing from discovery"):
            verify_repo.source_plan([p for p in discover(self.root) if p != self.source], self.root)

    def test_unlisted_sources_inside_and_outside_package_remain_legacy(self):
        extras = [self.write(DATA / "contracts-v2/fixtures/unlisted.ox", b"invalid candidate\n"),
                  self.write("fixtures/unrelated.ox", b"unrelated\n")]
        checks, entries, count = verify_repo.source_plan(discover(self.root), self.root)
        self.assert_stdin_addition(checks, entries, count, 9)
        self.assert_samples_use_root(checks, entries)
        for source in extras:
            self.assertIn((source, False), checks)

    def test_typed_inventory_overlap_fails_before_compiler(self):
        relative = self.source.relative_to(self.root).as_posix()
        with patch.object(verify_repo, "TYPED_SOURCE_FILES", verify_repo.TYPED_SOURCE_FILES + (relative,)):
            self.assert_no_compiler("overlaps")

    def test_check_only_inventory_overlap_fails_before_compiler(self):
        relative = self.source.relative_to(self.root).as_posix()
        with patch.object(verify_repo, "TYPED_CHECK_ONLY_FILES", verify_repo.TYPED_CHECK_ONLY_FILES + (relative,)):
            self.assert_no_compiler("overlaps")

    def test_missing_check_only_root_fails_before_compiler(self):
        (self.root / STDIN_ENTRY).unlink()
        self.assert_no_compiler("typed source fixture missing from discovery")

    def test_missing_stack_check_only_root_fails_before_compiler(self):
        (self.root / STACK_STDIN_ENTRY).unlink()
        self.assert_no_compiler("typed source fixture missing from discovery")

    def test_explicit_legacy_runnable_overlap_fails_before_compiler(self):
        relative = self.source.relative_to(self.root).as_posix()
        with patch.object(verify_repo, "RUNNABLE_PACKAGE_FILES", (relative,)):
            self.assert_no_compiler("overlaps")

    def test_globbed_legacy_runnable_overlap_fails_before_compiler(self):
        group = self.source.parent.relative_to(self.root).as_posix()
        with patch.object(verify_repo, "RUNNABLE_GROUPS", (group,)):
            self.assert_no_compiler("overlaps")

    def test_duplicate_manifest_registration_is_rejected(self):
        registrations = verify_fixture_data.SOURCE_DATA_MANIFESTS
        with patch.object(verify_fixture_data, "SOURCE_DATA_MANIFESTS", registrations + (registrations[0],)):
            self.assert_no_compiler("duplicate")

    def test_duplicate_json_keys_are_rejected(self):
        for raw in (b'{"files":{},"files":{}}', b'{"files":{"a.ox":{},"a.ox":{}}}'):
            with self.subTest(raw=raw), self.repinned(raw):
                self.assert_no_compiler("duplicate")

    def test_malformed_manifest_and_records_are_rejected(self):
        name = self.source.relative_to(self.manifest.parent).as_posix()
        record = self.document["files"][name]
        documents = [[], {}, {"files": []}, {"files": {}}, {"files": {name: []}},
                     {"files": {name: {"bytes": True, "sha256": record["sha256"]}}},
                     {"files": {name: {"bytes": -1, "sha256": record["sha256"]}}},
                     {"files": {name: {"bytes": record["bytes"], "sha256": "bad"}}},
                     {"files": {name: {**record, "unexpected": 1}}}]
        for document in documents:
            with self.subTest(document=document), self.repinned(json.dumps(document).encode()):
                self.assert_no_compiler("malformed")
        with self.repinned(b"{broken"):
            self.assert_no_compiler("unreadable")

    def test_path_escapes_and_noncanonical_paths_are_rejected(self):
        record = self.document["files"][self.source.relative_to(self.manifest.parent).as_posix()]
        paths = ("../escape.ox", "/absolute.ox", "a/../escape.ox", "./alias.ox", "a//alias.ox",
                 "C:/escape.ox", "a\\escape.ox", "")
        for path in paths:
            with self.subTest(path=path), self.repinned(json.dumps({"files": {path: record}}).encode()):
                self.assert_no_compiler("invalid fixture-data path")
        with patch.object(verify_fixture_data, "SOURCE_DATA_MANIFESTS", (("../outside.json", "0" * 64),)):
            self.assert_no_compiler("invalid fixture-data path")

    def test_manifest_cannot_register_itself(self):
        raw = json.dumps({"files": {"freeze-manifest.json": {"bytes": 0, "sha256": "0" * 64}}}).encode()
        with self.repinned(raw):
            self.assert_no_compiler("duplicate")

    def test_symlinked_body_is_rejected_even_with_matching_bytes(self):
        target = self.write("elsewhere.ox", self.source.read_bytes())
        self.source.unlink()
        try:
            self.source.symlink_to(target)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        self.assert_no_compiler("symlink")


class TypedOverlayAdmissionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        shutil.copytree(verify_repo.ROOT / DATA, self.root / DATA)
        copy_u8_data(self.root)
        shutil.copytree(verify_repo.ROOT / BYTE_DATA, self.root / BYTE_DATA)
        members = set(verify_repo.TYPED_SOURCE_FILES + verify_repo.TYPED_CHECK_ONLY_FILES)
        members.update(name for project in verify_repo.TYPED_PROJECTS.values() for name in project)
        members.update(V2_OVERLAY_FILES)
        for name in members | {verify_repo.TYPED_OVERLAY_MANIFEST, "oxid.toml"}:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(verify_repo.ROOT / name, path)
        self.manifest = self.root / verify_repo.TYPED_OVERLAY_MANIFEST
        self.document = json.loads(self.manifest.read_bytes())

    def assert_no_compiler(self, pattern):
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(verify_repo, "ROOT", self.root))
            stack.enter_context(patch.object(sys, "argv", ["verify_repo.py", sys.executable]))
            for name in ("verify_feature_status", "verify_versions", "verify_readmes", "verify_assets",
                         "verify_local_markdown_links"):
                stack.enter_context(patch.object(verify_repo, name))
            compiler = stack.enter_context(patch.object(verify_repo.subprocess, "run"))
            formatter = stack.enter_context(patch.object(verify_repo, "verify_typed_formatter"))
            with self.assertRaisesRegex(RuntimeError, pattern):
                verify_repo.main()
            compiler.assert_not_called()
            formatter.assert_not_called()

    def repinned(self, document):
        # Reach structural/closure validation behind the immutable production
        # identity barrier without changing any checked-in manifest or source.
        raw = json.dumps(document).encode()
        self.manifest.write_bytes(raw)
        return patch.object(verify_repo, "TYPED_OVERLAY_MANIFEST_SHA256", hashlib.sha256(raw).hexdigest())

    def test_manifest_is_required_even_if_entire_overlay_directory_is_absent(self):
        shutil.rmtree(self.manifest.parent)
        self.assert_no_compiler("missing or non-file typed overlay input")

    def test_missing_manifest_with_present_overlays_fails_before_compiler(self):
        self.manifest.unlink()
        self.assert_no_compiler("missing or non-file typed overlay input")

    def test_stale_manifest_fails_before_compiler(self):
        self.manifest.write_bytes(self.manifest.read_bytes() + b"\n")
        self.assert_no_compiler("typed overlay manifest digest mismatch")

    def test_changed_overlay_and_shared_module_fail_before_compiler(self):
        for name in ("source_buffer", "parser_atom"):
            path = self.root / self.document["sources"][name]["path"]
            original = path.read_bytes()
            with self.subTest(name=name):
                path.write_bytes(bytes([original[0] ^ 1]) + original[1:])
                self.assert_no_compiler("typed overlay source identity mismatch")
            path.write_bytes(original)

    def test_missing_overlay_and_shared_module_fail_before_compiler(self):
        for name in ("source_buffer", "parser_atom"):
            path = self.root / self.document["sources"][name]["path"]
            original = path.read_bytes()
            with self.subTest(name=name):
                path.unlink()
                self.assert_no_compiler("missing")
            path.write_bytes(original)

    def test_every_registered_source_must_be_discovered(self):
        sources = discover(self.root)
        for name in self.document["sources"]:
            excluded = self.root / self.document["sources"][name]["path"]
            with self.subTest(name=name), self.assertRaisesRegex(RuntimeError, "missing from discovery"):
                verify_repo.source_plan([source for source in sources if source != excluded], self.root)

    def test_unlisted_overlay_neighbor_remains_a_legacy_check(self):
        extra = self.manifest.parent / "unregistered.ox"
        extra.write_bytes(b"invalid candidate\n")
        checks, entries, count = verify_repo.source_plan(discover(self.root), self.root)
        self.assertIn((extra, False), checks)
        self.assertNotIn(extra, entries)
        self.assertIn((self.root / BYTE_PILOT, True), checks)
        self.assertIn(self.root / BYTE_PILOT, entries)
        self.assertEqual(count - 1, 79)  # Explicit RFC0031 pilot subtraction.
        self.assertFalse(any(path == self.root / name for path, _ in checks for name in V2_OVERLAY_FILES))

    def test_overlay_cannot_also_be_a_standalone_typed_source_or_legacy_run(self):
        for attribute in ("TYPED_SOURCE_FILES", "TYPED_CHECK_ONLY_FILES", "RUNNABLE_PACKAGE_FILES"):
            with self.subTest(attribute=attribute), patch.object(
                    verify_repo, attribute, getattr(verify_repo, attribute) + (V2_OVERLAY_FILES[0],)):
                self.assert_no_compiler("typed overlay inventory overlaps")

    def test_missing_import_and_unused_registration_are_rejected(self):
        del self.document["sources"]["parser_atom"]
        with self.repinned(self.document):
            self.assert_no_compiler("typed overlay closure missing module: parser_atom")
        self.document = json.loads((verify_repo.ROOT / verify_repo.TYPED_OVERLAY_MANIFEST).read_bytes())
        original = verify_repo.ROOT / "fixtures/typed-lexer-samples/tape.ox"
        relative = original.relative_to(verify_repo.ROOT).as_posix()
        (self.root / relative).write_bytes(original.read_bytes())
        self.document["sources"]["tape"] = {"path": relative,
                                             "sha256": hashlib.sha256(original.read_bytes()).hexdigest()}
        with self.repinned(self.document):
            self.assert_no_compiler("typed overlay manifest has unused sources")

    def test_paths_cannot_escape_the_exact_module_source_directories(self):
        for relative in ("../source_buffer.ox", "/tmp/source_buffer.ox",
                         "fixtures/typed-frontend-v2/../typed-frontend-v2/source_buffer.ox",
                         "fixtures/typed-frontend-v2/source_buffer2.ox"):
            self.document["sources"]["source_buffer"]["path"] = relative
            with self.subTest(relative=relative), self.repinned(self.document):
                self.assert_no_compiler("invalid typed overlay source registration")

    def test_overlay_inventory_cannot_be_incomplete(self):
        del self.document["sources"]["source_buffer"]
        with self.repinned(self.document):
            self.assert_no_compiler("typed overlay inventory must match")

    def test_symlinked_source_is_rejected_with_matching_bytes(self):
        path = self.root / V2_OVERLAY_FILES[0]
        target = self.root / "elsewhere.ox"
        target.write_bytes(path.read_bytes())
        path.unlink()
        try:
            path.symlink_to(target)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        self.assert_no_compiler("symlink in typed overlay input")


if __name__ == "__main__":
    unittest.main()
