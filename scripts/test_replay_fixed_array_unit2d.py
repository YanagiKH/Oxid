"""Source-only adversarial checks; no Cargo, LLVM, native binary, or network use."""
import argparse
import contextlib
import io
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import replay_fixed_array_unit2d as replay


# Literal compiler-error seams, independently specified from the six archived
# use sites. Line numbers refer to the unchanged public-array successor.
BORROWED_SLOT_LINE_CHANGES = {
    460: (b"            aggregate: slot,",
          b"            referent: BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap(),"),
    551: (b"            aggregate: slot,",
          b"            referent: BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap(),"),
    625: (b"    f.loans[0].aggregate = AggregateSlot::try_from_aggregate(a).unwrap();",
          b"    f.loans[0].referent = BorrowedSlot::check(BorrowedTy::Exact(a)).unwrap();"),
    676: (b"    g.references[0].aggregate = AggregateSlot::try_from_aggregate(a).unwrap();",
          b"    g.references[0].referent = BorrowedSlot::check(BorrowedTy::Exact(a)).unwrap();"),
    1879: (b"        aggregate: AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 1)).unwrap(),",
           b"        referent: BorrowedSlot::check(BorrowedTy::Exact(array(hir::Ty::I32, 1))).unwrap(),"),
    1893: (b"        aggregate: AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 1)).unwrap(),",
           b"        referent: BorrowedSlot::check(BorrowedTy::Exact(array(hir::Ty::I32, 1))).unwrap(),"),
}


class ReplayTests(unittest.TestCase):
    def setUp(self):
        # Keep this test's generated repositories/evidence in an explicitly chosen
        # disposable directory, never in the real source checkout.
        self.temp = tempfile.TemporaryDirectory(prefix="unit2d-python-", dir=os.environ.get("UNIT2D_TEST_TMPDIR"))
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def git(self, root, *args):
        return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.STDOUT).decode().strip()

    def fixture(self):
        fixture = self.root / "fixture inputs"
        (fixture / "sources").mkdir(parents=True)
        combined = (Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL
                    / replay.PUBLIC_ARRAY_ACTIVATION["frozen_module_path"]).read_bytes()
        (fixture / "sources/reviewer-array-native-v3.rs").write_bytes(combined)
        for name in replay.CONTROL_FILES:
            (fixture / "sources" / name).write_bytes(
                (Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL / "sources" / name).read_bytes()
                if name == "old-ir-export-v1.rs" else ("// exact frozen " + name + "\n").encode())
        replay.save(fixture / "qualification-v2.json", dict(source_head="f" * 40,
                    source_tree="e" * 40, reviewer_module_sha256=replay.sha(combined),
                    reviewer_module_path="sources/reviewer-array-native-v3.rs"))
        manifest = dict(schema=1, files=[dict(path=p.relative_to(fixture).as_posix(), bytes=p.stat().st_size,
                        sha256=replay.file_sha(p)) for p in sorted(fixture.rglob("*")) if p.is_file()])
        replay.save(fixture / "replay-inputs-v1.json", manifest)
        return fixture

    def repo(self):
        repo = self.root / "clean source with spaces"
        repo.mkdir()
        self.git(repo, "init", "-q")
        self.git(repo, "config", "user.email", "synthetic@example.invalid")
        self.git(repo, "config", "user.name", "Synthetic Replay Test")
        path = repo / replay.NATIVE_REL
        path.parent.mkdir(parents=True)
        path.write_bytes(b"// production native test source\n")
        (repo / "Cargo.toml").write_text('[package]\nname = "synthetic-never-built"\n')
        (repo / "kept.txt").write_bytes(b"original\n")
        self.git(repo, "add", ".")
        self.git(repo, "commit", "-qm", "synthetic")
        return repo

    def prepared(self, repo=None, profile=None):
        repo = repo or self.repo()
        fixture = self.fixture()
        args = argparse.Namespace(repo=repo, fixtures=fixture, input_manifest=fixture / "replay-inputs-v1.json",
                    commit=self.git(repo, "rev-parse", "HEAD"), output=self.root / "replay output", phase="prepare",
                    resume=False, rust_bin=None, trusted_tools=None, cargo_home=None, profile=profile)
        runner = replay.Replay(args)
        runner.run()
        return runner

    def test_public_successor_changes_only_obsolete_gate_expectations(self):
        frozen = (Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL
                  / replay.PUBLIC_ARRAY_ACTIVATION["frozen_module_path"]).read_bytes()
        current = replay.public_array_reviewer(frozen)
        old_name = replay.PUBLIC_ARRAY_ACTIVATION["historical_test"].encode()
        new_name = replay.PUBLIC_ARRAY_ACTIVATION["current_test"].encode()
        old_prefix, old_body = frozen.split(b"fn " + old_name, 1)
        new_prefix, new_body = current.split(b"fn " + new_name, 1)
        old_body, old_suffix = old_body.split(b"\n#[test]", 1)
        new_body, new_suffix = new_body.split(b"\n#[test]", 1)
        self.assertEqual(old_prefix, new_prefix)
        self.assertEqual(old_suffix, new_suffix)
        native_end = b"            let (sources, raw, _) = chain_fixture(ty, n);"
        self.assertEqual(old_body.split(native_end)[0].replace(b"production_denials", b"production_successes"),
                         new_body.split(native_end)[0])
        self.assertIn(b"(valid, denied, production_successes), (9, 81, 9)", new_body)
        self.assertIn(b"execute::run(&witness, Some(hir::DefId(0)))", new_body)
        self.assertIn(b"Ok(Scalar::I32(n as i32))", new_body)
        self.assertNotIn(b"UnsupportedArray", current)
        self.assertIn(replay.CHILD + new_name.decode(), replay.ORDINARY)
        self.assertNotIn(replay.CHILD + old_name.decode(), replay.ORDINARY)
        self.assertEqual(len(replay.ORDINARY), 7)
        self.assertEqual(len(replay.NATIVE), 8)

    def test_public_successor_rejects_drift_and_double_application(self):
        frozen = (Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL
                  / replay.PUBLIC_ARRAY_ACTIVATION["frozen_module_path"]).read_bytes()
        for changed in (frozen + b"\n", frozen.replace(b"denied += 1", b"denied += 0"),
                        replay.public_array_reviewer(frozen)):
            with self.subTest(digest=replay.sha(changed)):
                with self.assertRaisesRegex(RuntimeError, "exact frozen v3"):
                    replay.public_array_reviewer(changed)

    def test_public_successor_binding_rejects_historical_and_changed_identities(self):
        binding = {"public_array_activation": dict(replay.PUBLIC_ARRAY_ACTIVATION),
                   "borrowed_slot_compatibility": dict(replay.BORROWED_SLOT_COMPATIBILITY),
                   "projected_loan_compatibility": dict(replay.PROJECTED_LOAN_COMPATIBILITY),
                   "old_ir_resource_successor": dict(replay.OLD_IR_RESOURCE_SUCCESSOR),
                   "module_sha256": replay.PROJECTED_LOAN_COMPATIBILITY["current_module_sha256"]}
        replay.assert_current_module_binding(binding)
        for changed in ({}, {**binding, "module_sha256": replay.PUBLIC_ARRAY_ACTIVATION["frozen_module_sha256"]},
                        {**binding, "module_sha256": replay.PUBLIC_ARRAY_ACTIVATION["current_module_sha256"]},
                        {**binding, "public_array_activation": {**binding["public_array_activation"],
                          "native_wrong_identity_rejections": 80}}):
            with self.assertRaisesRegex(RuntimeError, "current public-array module binding"):
                replay.assert_current_module_binding(changed)

    def public_reviewer(self):
        frozen = (Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL
                  / replay.PUBLIC_ARRAY_ACTIVATION["frozen_module_path"]).read_bytes()
        return replay.public_array_reviewer(frozen)

    def test_borrowed_slot_successor_changes_only_six_exact_borrowed_sites(self):
        public = self.public_reviewer()
        current = replay.borrowed_slot_reviewer(public)
        old_lines, new_lines = public.splitlines(), current.splitlines()
        self.assertEqual(len(old_lines), len(new_lines))
        self.assertEqual({number: (old, new) for number, (old, new) in
                          enumerate(zip(old_lines, new_lines), 1) if old != new},
                         BORROWED_SLOT_LINE_CHANGES)
        self.assertEqual(replay.sha(current),
                         "234a2bf5cd68e01b0f4252ffe68a3700cdf9e9f8087f51c29e68daba6269adac")
        self.assertEqual(current.count(b"BorrowedSlot::check(BorrowedTy::Exact("), 6)
        self.assertNotIn(b"ScalarSlice", current)
        self.assertEqual(re.findall(rb"\bassert(?:_eq|_ne)?!\([\s\S]*?;\n", public),
                         re.findall(rb"\bassert(?:_eq|_ne)?!\([\s\S]*?;\n", current))
        owner = b"fn own(aggregate: AggregateTy, kind: OwnerKind, span: Span) -> OwnerDecl {"
        self.assertEqual(public.split(owner, 1)[1].split(b"\n}", 1)[0],
                         current.split(owner, 1)[1].split(b"\n}", 1)[0])
        self.assertIn(b"f.owners[0].aggregate = AggregateSlot::try_from_aggregate(a).unwrap();", current)

    def test_borrowed_slot_successor_is_exactly_reversible(self):
        public = self.public_reviewer()
        current = replay.borrowed_slot_reviewer(public)
        self.assertEqual(replay.borrowed_slot_reviewer(current, reverse=True), public)
        # Independently undo only the six literal line changes; this must recover
        # every assertion, ownership slot, and resource fact byte for byte.
        lines = current.splitlines(keepends=True)
        for number, (old, new) in BORROWED_SLOT_LINE_CHANGES.items():
            self.assertEqual(lines[number - 1], new + b"\n")
            lines[number - 1] = old + b"\n"
        self.assertEqual(b"".join(lines), public)
        self.assertEqual(replay.sha(public),
                         "611b66ce4628654d16c146462e147a08968a4d4b122ed07b2cb4bea908894f68")

    def test_borrowed_slot_successor_rejects_drift_and_double_application(self):
        public = self.public_reviewer()
        current = replay.borrowed_slot_reviewer(public)
        for data, reverse in ((public + b"\n", False),
                              (public.replace(b"denied += 1", b"denied += 0"), False),
                              (current, False), (current + b"\n", True), (public, True)):
            with self.subTest(reverse=reverse, digest=replay.sha(data)):
                with self.assertRaisesRegex(RuntimeError, "borrowed-slot adapter input identity differs"):
                    replay.borrowed_slot_reviewer(data, reverse=reverse)

    def test_borrowed_slot_successor_rejects_missing_and_duplicate_sites(self):
        public = self.public_reviewer()
        current = replay.borrowed_slot_reviewer(public)
        for data, reverse, hash_key, side in ((public, False, "input_module_sha256", 0),
                                             (current, True, "current_module_sha256", 1)):
            lines = data.splitlines(keepends=True)
            for number, pair in BORROWED_SLOT_LINE_CHANGES.items():
                for count in (0, 2):
                    changed_lines = lines[:]
                    self.assertEqual(lines[number - 1], pair[side] + b"\n")
                    changed_lines[number - 1] = (b"" if count == 0 else
                        lines[number - 1] + lines[number - 2] + lines[number - 1])
                    changed = b"".join(changed_lines)
                    with self.subTest(reverse=reverse, line=number, count=count):
                        # The real input hash rejects any such drift first. This
                        # override isolates the count-one guard behind that gate.
                        with self.assertRaisesRegex(RuntimeError, "input identity differs"):
                            replay.borrowed_slot_reviewer(changed, reverse=reverse)
                        with mock.patch.dict(replay.BORROWED_SLOT_COMPATIBILITY,
                                             {hash_key: replay.sha(changed)}):
                            with self.assertRaisesRegex(RuntimeError, "borrowed-slot adapter use site differs"):
                                replay.borrowed_slot_reviewer(changed, reverse=reverse)

    def test_borrowed_slot_successor_rejects_changed_output_identity(self):
        public = self.public_reviewer()
        current = replay.borrowed_slot_reviewer(public)
        for data, reverse, output_key in ((public, False, "current_module_sha256"),
                                          (current, True, "input_module_sha256")):
            with mock.patch.dict(replay.BORROWED_SLOT_COMPATIBILITY, {output_key: "0" * 64}):
                with self.assertRaisesRegex(RuntimeError, "borrowed-slot adapter output identity differs"):
                    replay.borrowed_slot_reviewer(data, reverse=reverse)

    def test_borrowed_slot_binding_requires_both_successor_metadata(self):
        binding = {"public_array_activation": dict(replay.PUBLIC_ARRAY_ACTIVATION),
                   "borrowed_slot_compatibility": dict(replay.BORROWED_SLOT_COMPATIBILITY),
                   "projected_loan_compatibility": dict(replay.PROJECTED_LOAN_COMPATIBILITY),
                   "old_ir_resource_successor": dict(replay.OLD_IR_RESOURCE_SUCCESSOR),
                   "module_sha256": replay.PROJECTED_LOAN_COMPATIBILITY["current_module_sha256"]}
        replay.assert_current_module_binding(binding)
        for field in ("public_array_activation", "borrowed_slot_compatibility", "projected_loan_compatibility", "old_ir_resource_successor"):
            missing = dict(binding)
            del missing[field]
            with self.subTest(field=field), self.assertRaisesRegex(RuntimeError, "module binding differs"):
                replay.assert_current_module_binding(missing)
        for field, value in (("input_module_sha256", "0" * 64),
                             ("current_module_sha256", "0" * 64),
                             ("replacement_count", 5), ("borrowed_identity", "BorrowedTy::ScalarSlice")):
            changed = {**binding, "borrowed_slot_compatibility": {
                **binding["borrowed_slot_compatibility"], field: value}}
            with self.subTest(field=field), self.assertRaisesRegex(RuntimeError, "module binding differs"):
                replay.assert_current_module_binding(changed)

    def test_projected_loan_successor_changes_only_two_root_projection_fields(self):
        previous = replay.borrowed_slot_reviewer(self.public_reviewer())
        current = replay.projected_loan_reviewer(previous)
        self.assertEqual(current.count(b"projection: Vec::new(),"), 2)
        self.assertEqual(current.replace(b"            projection: Vec::new(),\n", b"")
                         .replace(b"        projection: Vec::new(),\n", b""), previous)
        self.assertEqual(replay.projected_loan_reviewer(current, reverse=True), previous)
        self.assertEqual(replay.sha(previous), replay.BORROWED_SLOT_COMPATIBILITY["current_module_sha256"])
        self.assertEqual(replay.sha(current), replay.PROJECTED_LOAN_COMPATIBILITY["current_module_sha256"])

    def test_projected_loan_successor_rejects_drift_missing_duplicate_and_changed_output(self):
        previous = replay.borrowed_slot_reviewer(self.public_reviewer())
        current = replay.projected_loan_reviewer(previous)
        for data, reverse, input_key, output_key, side in (
                (previous, False, "input_module_sha256", "current_module_sha256", 0),
                (current, True, "current_module_sha256", "input_module_sha256", 1)):
            for changed in (data + b"\n", current if not reverse else previous):
                with self.assertRaisesRegex(RuntimeError, "input identity"):
                    replay.projected_loan_reviewer(changed, reverse=reverse)
            for pair in replay.PROJECTED_LOAN_REPLACEMENTS:
                fragment = pair[side].encode()
                for changed in (data.replace(fragment, b"", 1), data + fragment):
                    metadata = {**replay.PROJECTED_LOAN_COMPATIBILITY, input_key: replay.sha(changed)}
                    with mock.patch.dict(replay.PROJECTED_LOAN_COMPATIBILITY, metadata), self.assertRaisesRegex(RuntimeError, "use site"):
                        replay.projected_loan_reviewer(changed, reverse=reverse)
            with mock.patch.dict(replay.PROJECTED_LOAN_COMPATIBILITY, {output_key: "0" * 64}), self.assertRaisesRegex(RuntimeError, "output identity"):
                replay.projected_loan_reviewer(data, reverse=reverse)
        with mock.patch.dict(replay.PROJECTED_LOAN_COMPATIBILITY, {"replacement_count": 3}), self.assertRaisesRegex(RuntimeError, "inventory"):
            replay.projected_loan_reviewer(previous)

    def test_projected_loan_binding_rejects_predecessor_and_metadata_drift(self):
        binding = {"public_array_activation": dict(replay.PUBLIC_ARRAY_ACTIVATION),
                   "borrowed_slot_compatibility": dict(replay.BORROWED_SLOT_COMPATIBILITY),
                   "projected_loan_compatibility": dict(replay.PROJECTED_LOAN_COMPATIBILITY),
                   "old_ir_resource_successor": dict(replay.OLD_IR_RESOURCE_SUCCESSOR),
                   "module_sha256": replay.PROJECTED_LOAN_COMPATIBILITY["current_module_sha256"]}
        replay.assert_current_module_binding(binding)
        with self.assertRaises(RuntimeError):
            replay.assert_current_module_binding({**binding, "module_sha256": replay.BORROWED_SLOT_COMPATIBILITY["current_module_sha256"]})
        for key, value in (("input_module_sha256", "0" * 64), ("current_module_sha256", "0" * 64),
                           ("replacement_count", 1), ("projection_identity", "field projection")):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                replay.assert_current_module_binding({**binding, "projected_loan_compatibility": {
                    **binding["projected_loan_compatibility"], key: value}})

    def test_clean_archive_preserves_git_bytes_and_exact_append_prefix(self):
        runner = self.prepared()
        runner.check_source()
        original = (runner.args.repo / replay.NATIVE_REL).read_bytes()
        modified = (runner.source / replay.NATIVE_REL).read_bytes()
        self.assertTrue(modified.startswith(original))
        self.assertIn(runner.binding["marker"].encode(), modified)
        self.assertEqual(self.git(runner.args.repo, "status", "--porcelain"), "")
        self.assertFalse((runner.root / "target").exists())
        self.assertFalse((runner.evidence / "binary.json").exists())
        self.assertEqual(runner.state["completed"], ["prepare"])
        originals = replay.load(runner.evidence / "original-source.json")
        self.assertEqual(len(originals), 3)
        self.assertEqual({r["git_blob"] for r in originals}, {
            self.git(runner.args.repo, "rev-parse", "HEAD:" + r["path"]) for r in originals})

    def test_prepare_binds_separate_borrowed_successor_without_changing_frozen_input(self):
        runner = self.prepared()
        current = (runner.source / replay.MODULE_REL).read_bytes()
        frozen = (runner.inputs / replay.PUBLIC_ARRAY_ACTIVATION["frozen_module_path"]).read_bytes()
        self.assertEqual(replay.sha(frozen),
                         "b6b8f0a012c4d3b7dc1fa0140769174ee868af04db7e5ae3f34c8d1baa478c21")
        self.assertEqual(replay.borrowed_slot_reviewer(replay.projected_loan_reviewer(current, reverse=True), reverse=True),
                         replay.public_array_reviewer(frozen))
        self.assertEqual(runner.binding["projected_loan_compatibility"], replay.PROJECTED_LOAN_COMPATIBILITY)
        self.assertEqual(runner.binding["module_sha256"], replay.sha(current))
        self.assertEqual(runner.binding["public_array_activation"], replay.PUBLIC_ARRAY_ACTIVATION)
        self.assertEqual(runner.binding["borrowed_slot_compatibility"], replay.BORROWED_SLOT_COMPATIBILITY)
        provenance = replay.load(runner.evidence / "expectation-provenance.json")
        self.assertEqual(provenance["semantic_fuel_and_diagnostics"]["public_array_activation"],
                         replay.PUBLIC_ARRAY_ACTIVATION)
        self.assertEqual(provenance["borrowed_slot_compatibility"]["borrowed_slot_compatibility"],
                         replay.BORROWED_SLOT_COMPATIBILITY)
        runner.check_source()

    def test_real_git_owned_and_simulated_foreign_checkout_share_exact_scope(self):
        owned = self.prepared()
        args = argparse.Namespace(**vars(owned.args))
        args.output = self.root / "simulated foreign replay"
        runner = replay.Replay(args)
        original_run = subprocess.run
        environment = dict(os.environ, GIT_TEST_ASSUME_DIFFERENT_OWNER="1",
                           GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
        raw = original_run(["git", "--no-optional-locks", "-C", str(args.repo), "rev-parse", "--show-toplevel"],
                           env=environment, capture_output=True)
        self.assertEqual(raw.returncode, 128)
        self.assertIn(b"dubious ownership", raw.stderr)
        observations = []
        def foreign_child(argv, **kwargs):
            # Git's official child-only ownership test switch needs no chown,
            # privilege, permission or persistent Git configuration mutation.
            kwargs['env'] = dict(kwargs['env'], GIT_TEST_ASSUME_DIFFERENT_OWNER="1")
            observations.append((argv, dict(kwargs['env'])))
            return original_run(argv, **kwargs)
        runner.environment.update(GIT_DIR="/unwanted/repository", GIT_WORK_TREE="/unwanted/worktree",
            GIT_CONFIG_COUNT="1", GIT_CONFIG_KEY_0="safe.directory", GIT_CONFIG_VALUE_0="*")
        with mock.patch.object(replay.subprocess, "run", side_effect=foreign_child):
            runner.run()
            runner.args.resume = True
            runner.run()
        self.assertEqual(runner.state['completed'], ['prepare'])
        self.assertEqual(len(observations), 11)
        for argv, env in observations:
            self.assertEqual(argv[:8], replay.git_argv(args.repo))
            self.assertEqual({key for key in env if key.startswith('GIT_')},
                             {'GIT_CONFIG_NOSYSTEM', 'GIT_CONFIG_GLOBAL', 'GIT_TEST_ASSUME_DIFFERENT_OWNER'})
        labels = [replay.load(path)['label'] for path in next(runner.root.glob('resume-checks-*')).glob('*.json')]
        self.assertCountEqual(labels, ['original-head', 'original-status'])

    def test_real_git_exact_trust_rejects_other_repo_despite_inherited_wildcard(self):
        selected = self.repo()
        other = self.root / 'other checkout'
        other.mkdir()
        self.git(other, 'init', '-q')
        environment = dict(os.environ, GIT_TEST_ASSUME_DIFFERENT_OWNER='1', GIT_CONFIG_NOSYSTEM='1',
            GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_COUNT='1', GIT_CONFIG_KEY_0='safe.directory', GIT_CONFIG_VALUE_0='*')
        argv = replay.git_argv(selected, 'rev-parse', '--show-toplevel')
        admitted = subprocess.run(argv, env=environment, capture_output=True)
        self.assertEqual(admitted.returncode, 0)
        argv[argv.index('-C') + 1] = str(other)
        denied = subprocess.run(argv, env=environment, capture_output=True)
        self.assertEqual(denied.returncode, 128)
        self.assertIn(b'dubious ownership', denied.stderr)

    def test_checkout_scope_rejects_subdirectory_parent_discovery_and_rebinding(self):
        runner = self.prepared()
        nested = runner.args.repo / 'nested'
        nested.mkdir()
        with self.assertRaisesRegex(RuntimeError, 'checkout root'):
            runner.bind_git_checkout(nested)
        other = self.root / 'other checkout'
        other.mkdir()
        self.git(other, 'init', '-q')
        with self.assertRaisesRegex(RuntimeError, 'scope changed'):
            runner.bind_git_checkout(other)
        for bad in ('relative', '/safe/../wrong', '/safe/*', '/safe\nwrong'):
            with self.subTest(path=bad), self.assertRaisesRegex(RuntimeError, 'canonical absolute path'):
                replay.git_argv(bad, 'rev-parse', 'HEAD')

    def test_dirty_checkout_is_rejected_without_copying_uncommitted_bytes(self):
        repo = self.repo()
        (repo / "kept.txt").write_text("dirty edits\n")
        with self.assertRaisesRegex(RuntimeError, "dirty checkout rejected"):
            self.prepared(repo)
        self.assertEqual((repo / "kept.txt").read_text(), "dirty edits\n")
        self.assertFalse((self.root / "replay output/source").exists())

    def test_changed_prepared_source_rejected_before_any_build(self):
        runner = self.prepared()
        (runner.source / "kept.txt").write_text("substituted\n")
        with self.assertRaisesRegex(RuntimeError, "content or path inventory changed"):
            runner.check_source()
        self.assertFalse((runner.root / "target").exists())

    def test_archive_cannot_silently_omit_export_ignored_file(self):
        repo = self.repo()
        (repo / ".gitattributes").write_text("kept.txt export-ignore\n")
        self.git(repo, "add", ".gitattributes")
        self.git(repo, "commit", "-qm", "export-ignore")
        with self.assertRaisesRegex(RuntimeError, "export-ignore"):
            self.prepared(repo)

    def test_paths_reject_traversal_and_output_inside_symlinked_checkout(self):
        for path in ("../outside", "/absolute", "a/../outside", "a//b", "a\\b"):
            with self.subTest(path=path), self.assertRaises(RuntimeError):
                replay.safe_relative(path)
        repo = self.repo()
        alias = self.root / "alias"
        alias.symlink_to(repo, target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "disjoint"):
            replay.separate_paths(repo, alias / "output")

    def test_frozen_fixture_change_is_rejected(self):
        runner = self.prepared()
        (runner.inputs / "sources/old-ir-export-v1.rs").write_text("changed oracle")
        with self.assertRaisesRegex(RuntimeError, "content or path inventory changed"):
            runner.check_source()

    def test_existing_output_rejection_does_not_damage_previous_state(self):
        runner = self.prepared()
        state = (runner.root / "state.json").read_bytes()
        with contextlib.redirect_stderr(io.StringIO()):
            code = replay.main(["--repo", str(runner.args.repo), "--output", str(runner.root),
                                "--fixtures", str(runner.args.fixtures), "--phase", "prepare"])
        self.assertEqual(code, 1)
        self.assertEqual((runner.root / "state.json").read_bytes(), state)

    def test_binary_binding_rejects_replacement_wrong_source_and_path_escape(self):
        binary = self.root / "evidence/bin/fake-tests"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"\x7fELFsynthetic-not-executed")
        binding = dict(relative_path="evidence/bin/fake-tests", sha256=replay.file_sha(binary),
                       source_binding_sha256="bound-source")
        self.assertEqual(replay.assert_binary(binding, "bound-source", self.root), binary)
        with self.assertRaisesRegex(RuntimeError, "different prepared source"):
            replay.assert_binary(binding, "another-source", self.root)
        with self.assertRaisesRegex(RuntimeError, "unsafe"):
            replay.assert_binary(dict(binding, relative_path="../fake-tests"), "bound-source", self.root)
        binary.write_bytes(b"\x7fELFreplaced")
        with self.assertRaisesRegex(RuntimeError, "hash changed"):
            replay.assert_binary(binding, "bound-source", self.root)

    def test_inventory_rejects_missing_extra_and_unignored_native_family(self):
        marker = "independent_unit2d_replay_marker_synthetic"
        ignored = set(replay.NATIVE + [replay.PHYSICAL])
        expected = set(replay.ORDINARY + [replay.PREFIX + marker]) | ignored
        replay.assert_inventory(expected, ignored, marker)
        for all_names, ignored_names in ((expected - {replay.PHYSICAL}, ignored),
                (expected | {replay.PREFIX + "independent_unit2d_unexpected"}, ignored),
                (expected, ignored - {replay.NATIVE[0]})):
            with self.assertRaises(RuntimeError):
                replay.assert_inventory(all_names, ignored_names, marker)

    def test_zero_test_filter_and_ignored_only_success_are_not_passes(self):
        name = replay.ORDINARY[0]
        replay.assert_one_pass("test " + name + " ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;", name)
        for text in ("test result: ok. 0 passed; 0 failed; 0 ignored;", "test " + name + " ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored;"):
            with self.assertRaises(RuntimeError):
                replay.assert_one_pass(text, name)

    def test_old_ir_resource_successor_is_exact_reversible_and_physical(self):
        fixture = Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL
        frozen = (fixture / "expectations/old-ir-inventory.tsv").read_bytes()
        current = replay.old_ir_resource_inventory(frozen)
        self.assertEqual(replay.old_ir_resource_inventory(current, reverse=True), frozen)
        changes = []
        for old, new in zip(frozen.splitlines(), current.splitlines()):
            if old != new:
                a, b = old.split(b"\t"), new.split(b"\t")
                self.assertEqual(a[:5] + a[6:], b[:5] + b[6:])
                self.assertEqual((a[0], a[5], b[5]), (b"shared", b"22", b"24"))
                changes.append((a[1], a[2]))
        self.assertEqual(set(changes), {(g, str(m).encode()) for g in (b"false", b"true") for m in range(5)})
        self.assertEqual(len(changes), 10)
        exporter = (fixture / "sources/old-ir-export-v1.rs").read_bytes()
        adapted = replay.old_ir_resource_exporter(exporter)
        self.assertEqual(replay.old_ir_resource_exporter(adapted, reverse=True), exporter)
        self.assertIn(b"usage.expanded_cells,usage.native_bytes", adapted)
        self.assertIn(b"assert_eq!((usage.references, usage.loans), (0, expected_loans))", adapted)
        self.assertIn(b"historical_cells + 2 * expected_loans", adapted)
        self.assertIn(b"assert_eq!(usage.activation_fuel_cells(), historical_cells)", adapted)

    def test_old_ir_successors_reject_all_input_and_output_drift(self):
        fixture = Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL
        for name, function, before, after in (
            ("sources/old-ir-export-v1.rs", replay.old_ir_resource_exporter, "frozen_exporter_sha256", "current_exporter_sha256"),
            ("expectations/old-ir-inventory.tsv", replay.old_ir_resource_inventory, "frozen_inventory_sha256", "current_inventory_sha256")):
            frozen = (fixture / name).read_bytes()
            current = function(frozen)
            for value, reverse, output_key in ((frozen, False, after), (current, True, before)):
                for changed in (value + b"\n", value[:-1], value.replace(b"shared", b"renamed", 1)):
                    with self.subTest(name=name, reverse=reverse), self.assertRaisesRegex(RuntimeError, "input identity"):
                        function(changed, reverse=reverse)
                with mock.patch.dict(replay.OLD_IR_RESOURCE_SUCCESSOR, {output_key: "0" * 64}), self.assertRaisesRegex(RuntimeError, "output identity"):
                    function(value, reverse=reverse)
        with mock.patch.dict(replay.OLD_IR_RESOURCE_SUCCESSOR, {"changed_rows": 9}), self.assertRaisesRegex(RuntimeError, "row count"):
            replay.old_ir_resource_inventory(frozen)

    def test_old_ir_physical_inventory_rejects_each_changed_cell_and_extra_row(self):
        fixture = Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL
        frozen = (fixture / "expectations/old-ir-inventory.tsv").read_bytes()
        physical = replay.old_ir_resource_inventory(frozen)
        root = self.root / "old-ir"
        root.mkdir()
        (root / "a.ll").write_bytes(b"exact llvm\n")
        manifest = [dict(path="old-ir/a.ll", length=11, sha256=replay.file_sha(root / "a.ll"))]
        target = root / "inventory.tsv"
        target.write_bytes(physical)
        receipt = replay.compare_old_ir(root, manifest, frozen)
        self.assertEqual(target.read_bytes(), physical)
        self.assertEqual(receipt["current_inventory_sha256"], replay.sha(physical))
        # Every one of the 90 rows and seven fields remains exactly checked.
        lines = physical.splitlines(keepends=True)
        for row in range(1, len(lines)):
            for column in range(7):
                changed = lines[:]
                fields = changed[row].rstrip(b"\n").split(b"\t")
                fields[column] += b"x"
                changed[row] = b"\t".join(fields) + b"\n"
                target.write_bytes(b"".join(changed))
                with self.subTest(row=row, column=column), self.assertRaisesRegex(RuntimeError, "TSV"):
                    replay.compare_old_ir(root, manifest, frozen)
        for changed in (frozen, physical + lines[-1], b"".join(lines[:-1])):
            target.write_bytes(changed)
            with self.assertRaisesRegex(RuntimeError, "TSV"):
                replay.compare_old_ir(root, manifest, frozen)

    def test_old_ir_comparison_checks_exact_modules_tsv_and_extras(self):
        root = self.root / "old-ir"
        root.mkdir()
        (root / "a.ll").write_bytes(b"exact llvm\n")
        inventory = (Path(replay.__file__).resolve().parent.parent / replay.FIXTURE_REL / "expectations/old-ir-inventory.tsv").read_bytes()
        (root / "inventory.tsv").write_bytes(replay.old_ir_resource_inventory(inventory))
        manifest = [dict(path="old-ir/a.ll", length=11, sha256=replay.file_sha(root / "a.ll"))]
        result = replay.compare_old_ir(root, manifest, inventory)
        self.assertEqual(result["modules"], 1)
        with self.assertRaisesRegex(RuntimeError, "identity"):
            replay.compare_old_ir(root, manifest, b"header\nchanged\n")
        (root / "extra.ll").write_text("extra")
        with self.assertRaisesRegex(RuntimeError, "inventory"):
            replay.compare_old_ir(root, manifest, inventory)
        (root / "a.ll").write_bytes(b"wrong llvm\n")
        with self.assertRaisesRegex(RuntimeError, "hash/length"):
            replay.compare_old_ir(root, manifest, inventory)

    def test_expected_mutant_failure_requires_exact_status_and_output(self):
        expected = dict(status=1, stdout=b"", stderr=b"fatal: surviving poison at byte 1\n")
        replay.compare_result(expected.copy(), expected, "mutant")
        for key, value in (("status", 0), ("stderr", b"other error"), ("stdout", b"unexpected")):
            with self.subTest(key=key), self.assertRaisesRegex(RuntimeError, "different " + key):
                replay.compare_result(dict(expected, **{key: value}), expected, "mutant")

    def test_nonzero_python_command_preserves_stdout_stderr_status_and_argv(self):
        runner = self.prepared()
        with self.assertRaisesRegex(RuntimeError, "command failed"):
            runner.command("synthetic-failure", [sys.executable, "-c",
                'import sys; print("before failure"); print("diagnostic", file=sys.stderr); sys.exit(7)'])
        paths = list((runner.evidence / "commands").glob("*-synthetic-failure.json"))
        self.assertEqual(len(paths), 1)
        receipt = replay.load(paths[0])
        self.assertEqual(receipt["exit"], 7)
        self.assertEqual((paths[0].parent / receipt["stdout"]).read_bytes(), b"before failure\n")
        self.assertEqual((paths[0].parent / receipt["stderr"]).read_bytes(), b"diagnostic\n")
        self.assertEqual(receipt["argv"][0], sys.executable)

    def test_completed_evidence_replacement_blocks_resume(self):
        runner = self.prepared()
        output = runner.evidence / "executions"
        output.mkdir()
        (output / "synthetic.stdout").write_bytes(b"expected receipt")
        runner.seal("ordinary")
        runner.verify_seals()
        (output / "synthetic.stdout").write_bytes(b"replaced receipt")
        with self.assertRaisesRegex(RuntimeError, "completed evidence changed"):
            runner.verify_seals()

    def test_added_evidence_member_is_rejected_even_after_completed_phase(self):
        runner = self.prepared()
        native = runner.evidence / "native"
        native.mkdir()
        (native / "original.elf").write_bytes(b"synthetic existing artifact")
        runner.seal("verify")
        runner.verify_seals()
        (native / "unrecorded.elf").write_bytes(b"extra member")
        with self.assertRaisesRegex(RuntimeError, "exact member inventory"):
            runner.verify_seals()

    def test_final_summary_is_inside_the_sealed_output_closure(self):
        runner = self.prepared()
        replay.save(runner.evidence / "verified.json", dict(status="synthetic", executions=1))
        runner.seal("verify")
        replay.save(runner.evidence / "verified.json", dict(status="invented pass", executions=999))
        with self.assertRaisesRegex(RuntimeError, "exact member inventory"):
            runner.verify_seals()

    def test_original_checkout_must_remain_clean_on_resume(self):
        runner = self.prepared()
        (runner.args.repo / "kept.txt").write_text("new uncommitted work")
        runner.args.resume = True
        with self.assertRaisesRegex(RuntimeError, "original checkout is dirty"):
            runner.run()
        self.assertFalse((runner.root / "target").exists())

    def test_original_checkout_head_must_not_move_on_resume(self):
        runner = self.prepared()
        (runner.args.repo / "kept.txt").write_text("later source")
        self.git(runner.args.repo, "add", "kept.txt")
        self.git(runner.args.repo, "commit", "-qm", "later head")
        runner.args.resume = True
        with self.assertRaisesRegex(RuntimeError, "original checkout HEAD changed"):
            runner.run()

    def test_original_fixture_member_must_not_change_on_resume(self):
        runner = self.prepared()
        (runner.args.fixtures / "sources/old-ir-export-v1.rs").write_text("changed original input")
        runner.args.resume = True
        with self.assertRaisesRegex(RuntimeError, "content or path inventory changed"):
            runner.run()

    def test_original_input_manifest_must_not_change_on_resume(self):
        runner = self.prepared()
        runner.args.input_manifest.write_text('{"replaced":true}\n')
        runner.args.resume = True
        with self.assertRaisesRegex(RuntimeError, "original input manifest changed"):
            runner.run()

    def test_copied_input_manifest_cannot_change_after_prepare_before_first_seal(self):
        runner = self.prepared()
        replay.save(runner.evidence / "input-manifest.json", dict(substituted_inventory=True))
        with self.assertRaisesRegex(RuntimeError, "copied input manifest changed"):
            runner.check_source()

    def test_unregistered_extra_seal_filename_cannot_hide_added_evidence(self):
        runner = self.prepared()
        runner.seal("ordinary")
        replay.save(runner.evidence / "phase-forged-artifacts.json", [])
        with self.assertRaisesRegex(RuntimeError, "exact member inventory"):
            runner.verify_seals()

    def test_missing_or_substituted_structural_sidecar_is_rejected(self):
        expected = dict(provenance_kind="historical-observed-inventory; not a semantic oracle",
                        structure_sidecars=["a.structure.tsv", "empty-construction.structure.tsv"],
                        structure_sidecar_count=2, bounds_sites=3, pointer_phis=1)
        replay.assert_structure_inventory(expected["structure_sidecars"], expected, 3, 1)
        for names in (["a.structure.tsv"], ["a.structure.tsv", "substitute.structure.tsv"]):
            with self.assertRaisesRegex(RuntimeError, "sidecar inventory mismatch"):
                replay.assert_structure_inventory(names, expected, 3, 1)

    def test_missing_structural_site_is_rejected_without_changing_sidecar_names(self):
        expected = dict(provenance_kind="historical-observed-inventory; not a semantic oracle",
                        structure_sidecars=["a.structure.tsv"], structure_sidecar_count=1,
                        bounds_sites=3, pointer_phis=1)
        with self.assertRaisesRegex(RuntimeError, "bounds-site inventory mismatch"):
            replay.assert_structure_inventory(["a.structure.tsv"], expected, 2, 1)

    def test_zero_or_incomplete_execution_inventory_is_rejected(self):
        observed_targets = dict(elf_artifacts=336, source_free_executions=1210,
                                official_llvm_command_receipts=3360)
        replay.assert_execution_inventory(336, 1210, 3360, observed_targets)
        for counts in ((0, 0, 0), (336, 0, 3360), (336, 1209, 3360)):
            with self.subTest(counts=counts), self.assertRaises(RuntimeError):
                replay.assert_execution_inventory(*counts, observed_targets)

    def test_release_profile_is_source_bound_and_conflicting_resume_is_rejected(self):
        runner = self.prepared(profile="release")
        self.assertEqual(runner.binding["profile"], "release")
        runner.args.resume = True
        runner.args.profile = "debug"
        with self.assertRaisesRegex(RuntimeError, "conflicting profile"):
            runner.run()
        self.assertFalse((runner.root / "target").exists())

    def test_resume_rejects_changed_original_tool_map_before_tool_invocation(self):
        runner = self.prepared()
        mapping = self.root / "trusted-tools.json"
        replay.save(mapping, {"llvm-as": "/never/executed"})
        runner.state["tools"] = dict(trusted_map_path=str(mapping), trusted_map_sha256=replay.file_sha(mapping))
        replay.save(mapping, {"llvm-as": "/substituted/tool"})
        with self.assertRaisesRegex(RuntimeError, "original trusted-tool map changed"):
            runner.configure()
        self.assertFalse((runner.root / "target").exists())

    def test_completed_resume_rejects_environment_drift_without_rewriting_sealed_bytes(self):
        runner = self.prepared()
        rust = self.root / "synthetic-rust"
        rust.mkdir()
        for name in ("cargo", "rustc", "rustdoc"):
            (rust / name).write_bytes(b"synthetic tool bytes, never executed")
        mapping = self.root / "synthetic-tools.json"
        replay.save(mapping, {name: sys.executable for name in replay.TOOLS})
        # configure only binds paths/hashes/environment. It does not run a tool.
        runner.args.rust_bin = rust
        runner.args.trusted_tools = mapping
        runner.configure()
        binary = runner.evidence / "bin/oxid-unit2d-tests"
        binary.parent.mkdir()
        binary.write_bytes(b"\x7fELFsynthetic, never executed")
        replay.save(runner.evidence / "binary.json", dict(relative_path="evidence/bin/oxid-unit2d-tests",
                    sha256=replay.file_sha(binary), source_binding_sha256=runner.state["source_binding_sha256"]))
        runner.state.update(completed=list(replay.PHASES), status="passed",
                            binary_binding_sha256=replay.file_sha(runner.evidence / "binary.json"))
        runner.seal("verify")
        replay.save(runner.root / "state.json", runner.state)
        original = (runner.evidence / "environment.json").read_bytes()
        runner.args.resume, runner.args.phase = True, "full"
        with mock.patch.dict(os.environ, PATH=os.environ.get("PATH", "") + os.pathsep + "/synthetic-extra-directory"):
            with self.assertRaisesRegex(RuntimeError, "environment changed on resume"):
                runner.run()
        self.assertEqual((runner.evidence / "environment.json").read_bytes(), original)
        runner.verify_seals()

    def test_capture_preserves_failed_python_tool_outputs_without_copying_external_paths(self):
        temporary = self.root / "temporary"
        workspace = temporary / "synthetic-compiler-workspace"
        workspace.mkdir(parents=True)
        # A Python program with an LLVM-looking filename is never parsed by LLVM.
        source = workspace / "program.ll"
        source.write_text('from pathlib import Path\nimport sys\nPath("program.o").write_bytes(b"partial object")\nprint("mock stdout")\nprint("mock diagnostic",file=sys.stderr)\nsys.exit(7)\n')
        (workspace / "private.txt").write_text("must not be captured")
        external = self.root / "outside.ll"
        external.write_text("outside temporary root")
        tools = self.root / "wrapper"
        tools.mkdir()
        name = "synthetic-python-tool"
        (tools / name).symlink_to(Path(replay.__file__).with_name("replay_unit2d_tool_capture.py"))
        loggers = self.root / "logger"
        loggers.mkdir()
        (loggers / name).write_text('import subprocess,sys\nsys.exit(subprocess.call([sys.executable,*sys.argv[1:]]))\n')
        mapping, hashes = self.root / "map.json", self.root / "hashes.json"
        replay.save(mapping, {name: sys.executable})
        replay.save(hashes, {name: replay.file_sha(sys.executable)})
        env = dict(os.environ, TMPDIR=str(temporary), OXID_UNIT2D_CAPTURE_ROOT=str(self.root / "captures"),
                   OXID_UNIT2D_TRUSTED_TOOLS=str(mapping), OXID_UNIT2D_TOOL_HASHES=str(hashes),
                   OXID_UNIT2D_FROZEN_LOGGERS=str(loggers))
        result = subprocess.run([sys.executable, "-B", str(tools / name), "program.ll", "-o", "program.o",
                                 "private.txt", str(external)], cwd=workspace, env=env, capture_output=True)
        self.assertEqual(result.returncode, 7)
        self.assertEqual(result.stdout, b"mock stdout\n")
        self.assertEqual(result.stderr, b"mock diagnostic\n")
        receipt = replay.load(next((self.root / "captures").glob("*/receipt.json")))
        self.assertEqual(receipt["exit"], 7)
        self.assertEqual({Path(row["path"]).name for row in receipt["snapshots"]}, {"program.ll", "program.o"})
        self.assertEqual(len(list((self.root / "captures/blobs").iterdir())), 2)


if __name__ == "__main__":
    unittest.main()
