"""Exact stdin initializer successor controls; no current source qualification."""
import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import portable as p


ENUM_ADAPTER = {
    "version": "unit4-direct-parser-closed-array-enum-v1",
    "original": {"path": "observer.rs", "bytes": 19856,
                 "sha256": "6c48934b5f690ad65671c62bd3968a933ae79c5a576d641133de2eacf0e18d9a"},
    "derived": {"path": "src/frontend/parser/unit4_observer.rs", "bytes": 20002,
                "sha256": "b3bc4912fe5ab95b0ef03a196093c8aa71c45cf1a88a60d154d8a5281852a150"},
    "old_seam_sha256": "86139c32f7f37beca1912bc07ae58ad44767f56c2101137f49dbceb18019c24d",
    "new_seam_sha256": "33e5bc0b1e5bf7e758194f501508e24f41016c67438dc2f7c1a11568d869ff61",
    "substitutions": 1,
}
STDIN_ADAPTER = {
    "version": "unit4-direct-parser-closed-stdin-v1", "predecessor": ENUM_ADAPTER,
    "derived": {"path": "src/frontend/parser/unit4_observer.rs", "bytes": 20052,
                "sha256": "8b7efa8484d9636dda5329a4708fa762d09c53bd76b42c485b871973410c7928"},
    "old_seam_sha256": "2378d0fc75ce8239ced45e5a4391de6fd6fffd3f13cd12c500b5c620f84b4f0a",
    "new_seam_sha256": "97168b6287145dda2fffb8c2f77cdb71c47e6b46ca74dc900d950fc0fc47b701",
    "substitutions": 1,
}


class ClosedStdinInitializerTests(unittest.TestCase):
    def setUp(self):
        historical = p.read(p.FROZEN / "authority.json")
        self.a = {"helper_files": historical["helper_files"],
                  "current": {"observer_initializer_adapter": copy.deepcopy(STDIN_ADAPTER)}}
        self.original = (p.FROZEN / "frozen/helpers/observer.rs").read_bytes()

    def test_stdin_successor_recovers_exact_enum_predecessor_and_frozen_helper(self):
        current = p.compose_observer_initializer(self.a)
        insertion = b"            std_imports: StdImportPolicy::Closed,\n"
        self.assertEqual(current.count(insertion), 1)
        predecessor = current.replace(insertion, b"", 1)
        self.assertEqual((len(predecessor), p.sha(predecessor)),
                         (ENUM_ADAPTER["derived"]["bytes"], ENUM_ADAPTER["derived"]["sha256"]))
        enum_insertion = (b"            arrays: ArraySyntaxPolicy::Closed,\n"
                          b"            enums: EnumSyntaxPolicy::Closed,\n"
                          b"            storage: enums::SyntaxStorage::default(),\n")
        self.assertEqual(predecessor.count(enum_insertion), 1)
        self.assertEqual(predecessor.replace(enum_insertion, b"", 1), self.original)
        self.assertEqual((len(current), p.sha(current)),
                         (STDIN_ADAPTER["derived"]["bytes"], STDIN_ADAPTER["derived"]["sha256"]))
        self.assertNotIn(b"StdImportPolicy::BuiltinCandidate", current)

    def test_changed_current_or_predecessor_authority_rejects(self):
        for path, value in ((("derived", "sha256"), "0" * 64),
                            (("predecessor", "derived", "sha256"), "0" * 64),
                            (("predecessor", "new_seam_sha256"), "0" * 64),
                            (("substitutions",), 2),
                            (("old_seam_sha256",), "0" * 64)):
            altered = copy.deepcopy(self.a)
            node = altered["current"]["observer_initializer_adapter"]
            for name in path[:-1]:
                node = node[name]
            node[path[-1]] = value
            with self.subTest(path=path), self.assertRaisesRegex(p.Rejected, "unapproved observer initializer successor"):
                p.compose_observer_initializer(altered)
        self.a["current"]["observer_initializer_adapter"] = copy.deepcopy(ENUM_ADAPTER)
        with self.assertRaisesRegex(p.Rejected, "unapproved observer initializer successor"):
            p.compose_observer_initializer(self.a)

    def test_coherent_rehash_cannot_open_historical_stdin_policy(self):
        current = p.compose_observer_initializer(self.a)
        changed = current.replace(b"StdImportPolicy::Closed", b"StdImportPolicy::BuiltinCandidate", 1)
        row = self.a["current"]["observer_initializer_adapter"]["derived"]
        row.update(bytes=len(changed), sha256=p.sha(changed))
        with self.assertRaisesRegex(p.Rejected, "unapproved observer initializer successor"):
            p.compose_observer_initializer(self.a)

    def test_frozen_helper_drift_rejects_before_derivation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            helper = root / "frozen/helpers/observer.rs"
            helper.parent.mkdir(parents=True)
            helper.write_bytes(self.original + b"\n")
            with patch.object(p, "FROZEN", root):
                with self.assertRaisesRegex(p.Rejected, "historical observer identity"):
                    p.compose_observer_initializer(self.a)


if __name__ == "__main__":
    unittest.main()
