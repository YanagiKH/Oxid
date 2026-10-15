"""Inactive genuine predecessor-view controls; no implicit fixture discovery.

The bounded caller authenticates these bytes and passes the complete admitted
fixture plus an effects monitor. The monitor must reject cross-instance overlap
and require cleanup reconciliation before a later view. Tests alone do not
establish a process-wide lifetime bound.
"""
import copy
import hashlib
import os
from pathlib import PurePosixPath
import stat
import unittest
from unittest import mock


def build_suite(view, reject, *, repo, admitted, seal, expected_bodies, effects):
    if (not isinstance(admitted, dict) or not isinstance(seal, dict)
            or not isinstance(expected_bodies, dict) or len(expected_bodies) != 681):
        raise ValueError("complete genuine predecessor fixture is required")
    if effects is None or not all(callable(getattr(effects, n, None))
                                  for n in ("assert_no_residue", "assert_view_entered",
                                            "assert_single_view_lifetime")):
        raise ValueError("bounded effects monitor is required")

    def reachable(error, wanted):
        seen, pending = set(), [error]
        while pending:
            item = pending.pop()
            if item is wanted:
                return True
            if id(item) in seen:
                continue
            seen.add(id(item))
            pending.extend(x for x in (item.__cause__, item.__context__) if x is not None)
            pending.extend(getattr(item, "exceptions", ()))
        return False

    class GenuinePredecessorView(unittest.TestCase):
        def tearDown(self):
            effects.assert_no_residue()
            effects.assert_single_view_lifetime()

        def context(self, **changes):
            args = dict(repo=repo, admitted=admitted, seal=seal)
            args.update(changes)
            return view.predecessor_view(**args)

        def test_complete_view_body_and_roster_then_cleanup(self):
            with self.context() as root:
                effects.assert_view_entered(root)
                expected_dirs = {str(p) for name in expected_bodies
                                 for p in PurePosixPath(name).parents if str(p) != "."}
                files, directories, pending = set(), set(), [""]
                while pending:
                    parent = pending.pop()
                    with os.scandir(root / parent) as entries:
                        for entry in entries:
                            name = parent + "/" + entry.name if parent else entry.name
                            self.assertIn(name, set(expected_bodies) | expected_dirs)
                            mode = entry.stat(follow_symlinks=False).st_mode
                            if name in expected_dirs:
                                self.assertTrue(stat.S_ISDIR(mode))
                                self.assertNotIn(name, directories)
                                directories.add(name)
                                pending.append(name)
                            else:
                                self.assertTrue(stat.S_ISREG(mode))
                                self.assertNotIn(name, files)
                                files.add(name)
                    self.assertLessEqual(len(files), 681)
                    self.assertLessEqual(len(directories), 111)
                self.assertEqual(files, set(expected_bodies))
                self.assertEqual(directories, expected_dirs)
                for name, expected in expected_bodies.items():
                    self.assertEqual(view._read_regular(root, name), expected, name)
                self.assertEqual(sum(map(len, expected_bodies.values())), 38003640)
            self.assertFalse(root.exists())

        def test_actual_inverse_is_preferred_over_older_predecessor_map(self):
            value = copy.deepcopy(admitted)
            historical = value["published_predecessor"]
            value["predecessor_inputs"] = {"wrong-older-stage": b"not M"}
            self.assertEqual(view._old_inputs(value), historical["inputs"])

        def test_missing_fixed_seal_fails_before_capture(self):
            with mock.patch.object(view, "_capture", side_effect=AssertionError("capture called")):
                with self.assertRaises(reject), self.context(seal=None):
                    self.fail("unsealed view entered")

        def test_bad_descriptor_fails_without_materialization(self):
            for key, value in (("path", "other.json"), ("bytes", 524289),
                               ("bytes", 0), ("bytes", True),
                               ("bytes", float(seal["bytes"])),
                               ("sha256", "A" * 64),
                               ("sha256", "0" * 64), ("extra", True)):
                changed = copy.deepcopy(seal)
                changed[key] = value
                with self.subTest(field=key):
                    with mock.patch.object(view.tempfile, "TemporaryDirectory",
                                           side_effect=AssertionError("materialization called")):
                        with self.assertRaises(reject), self.context(seal=changed):
                            self.fail("changed descriptor entered")

        def test_changed_current_capture_source_fails_before_materialization(self):
            changed = copy.deepcopy(admitted)
            changed["published_predecessor"]["inputs"]["src/main.rs"] += b"\n"
            with mock.patch.object(view.tempfile, "TemporaryDirectory",
                                   side_effect=AssertionError("materialization called")):
                with self.assertRaises(reject), self.context(admitted=changed):
                    self.fail("changed source entered")

        def test_same_module_overlap_is_rejected_before_second_capture(self):
            with self.context() as root:
                effects.assert_view_entered(root)
                with mock.patch.object(view, "_capture",
                                       side_effect=AssertionError("second capture called")):
                    with self.assertRaises(reject), self.context():
                        self.fail("overlapping same-module view entered")

        def test_operation_failure_retains_exact_object_and_cleans_view(self):
            failure = RuntimeError("injected operation failure")
            with self.assertRaises(RuntimeError) as got:
                with self.context() as root:
                    raise failure
            self.assertIs(got.exception, failure)
            self.assertFalse(root.exists())

        def test_postclosure_body_drift_is_rejected_and_cleaned(self):
            with self.assertRaises(reject):
                with self.context() as root:
                    target = root / "src/main.rs"
                    with target.open("ab") as stream:
                        stream.write(b"\n")
            self.assertFalse(root.exists())

        def test_operation_and_preservation_failure_both_retained(self):
            operation = RuntimeError("injected operation failure")
            preservation = ValueError("injected preservation failure")
            original_verify = view._verify_view
            count = 0

            def verify(*args):
                nonlocal count
                count += 1
                if count == 2:
                    raise preservation
                return original_verify(*args)

            with mock.patch.object(view, "_verify_view", side_effect=verify):
                with self.assertRaises(BaseExceptionGroup) as got:
                    with self.context() as root:
                        raise operation
            self.assertTrue(reachable(got.exception, operation))
            self.assertTrue(reachable(got.exception, preservation))
            self.assertFalse(root.exists())

        def test_extra_file_is_rejected_before_close(self):
            with self.assertRaises(reject):
                with self.context() as root:
                    with (root / "unexpected-member").open("xb") as stream:
                        stream.write(b"extra")
            self.assertFalse(root.exists())

        def test_extra_empty_directory_is_rejected_before_close(self):
            with self.assertRaises(reject):
                with self.context() as root:
                    (root / "unexpected-directory").mkdir()
            self.assertFalse(root.exists())

        def test_cleanup_failure_alone_remains_failure(self):
            cleanup = OSError("injected late cleanup failure")
            original_temporary = view.tempfile.TemporaryDirectory

            class LateCleanupFailure:
                def __init__(self, **kwargs):
                    self.inner = original_temporary(**kwargs)

                def __enter__(self):
                    return self.inner.__enter__()

                def __exit__(self, *args):
                    self.inner.__exit__(*args)
                    raise cleanup

            with mock.patch.object(view.tempfile, "TemporaryDirectory", LateCleanupFailure):
                with self.assertRaises(OSError) as got:
                    with self.context() as root:
                        pass
            self.assertIs(got.exception, cleanup)
            self.assertFalse(root.exists())

        def test_cleanup_failure_retains_operation_and_preservation_objects(self):
            cleanup = OSError("injected late cleanup failure")
            operation = RuntimeError("injected operation failure")
            preservation = ValueError("injected preservation failure")
            original_verify = view._verify_view
            original_temporary = view.tempfile.TemporaryDirectory
            count = 0

            def verify(*args):
                nonlocal count
                count += 1
                if count == 2:
                    raise preservation
                return original_verify(*args)

            class LateCleanupFailure:
                def __init__(self, **kwargs):
                    self.inner = original_temporary(**kwargs)

                def __enter__(self):
                    return self.inner.__enter__()

                def __exit__(self, *args):
                    self.inner.__exit__(*args)
                    raise cleanup

            with mock.patch.object(view, "_verify_view", side_effect=verify), \
                 mock.patch.object(view.tempfile, "TemporaryDirectory", LateCleanupFailure):
                with self.assertRaises(OSError) as got:
                    with self.context() as root:
                        raise operation
            self.assertIs(got.exception, cleanup)
            self.assertTrue(reachable(got.exception, operation))
            self.assertTrue(reachable(got.exception, preservation))
            self.assertFalse(root.exists())

        def test_next_sequential_view_runs_only_after_clean_close(self):
            with self.context() as first:
                effects.assert_view_entered(first)
            effects.assert_no_residue()
            with self.context() as second:
                effects.assert_view_entered(second)
                self.assertNotEqual(first, second)
                self.assertFalse(first.exists())
            self.assertFalse(second.exists())

    return unittest.defaultTestLoader.loadTestsFromTestCase(GenuinePredecessorView)

