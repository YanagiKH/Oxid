"""Check independent UTF-8 oracle coverage and lock-refusal controls."""
import copy
from pathlib import Path
import shutil
import tempfile
import tomllib
import unittest

import verify_utf8_text_app as app


class Utf8Oracle(unittest.TestCase):
    def test_strict_decoding_precedes_all_output_and_only_changes_line_endings(self):
        self.assertEqual(app.expected(b"\xef\xbb\xbfA\r\nB\rC\n\x00"),
                         (0, b"\xef\xbb\xbfA\nB\nC\n\x00"))
        self.assertEqual(app.expected(b"valid\r\n" * 2000 + b"\xff"), (65, b""))
        for raw in (b"\xc0\x80", b"\xe0\x80\x80", b"\xed\xa0\x80", b"\xf0\x80\x80\x80",
                    b"\xf4\x90\x80\x80", b"\xf5\x80\x80\x80", b"\xe2\x82"):
            with self.subTest(raw=raw):
                self.assertEqual(app.expected(raw), (65, b""))
        for value in (0, 0xFEFF, 0xFDD0, 0xFFFF, 0x10FFFF):
            raw = chr(value).encode()
            self.assertEqual(app.expected(raw), (0, raw))

    def test_core_roster_has_every_octet_and_real_read_boundary_cases(self):
        by_name = dict(app.CASES)
        self.assertEqual(len(by_name), len(app.CASES))
        self.assertEqual(len({raw for _, raw in app.CASES}), len(app.CASES))
        self.assertTrue({bytes((value,)) for value in range(256)} <= set(by_name.values()))
        self.assertEqual(len(by_name["ascii-length-65536"]), 65536)
        self.assertEqual(app.expected(by_name["split-scalar-1023"])[0], 0)
        self.assertEqual(app.expected(by_name["late-invalid-65536"]), (65, b""))
        self.assertGreater(len(app.expected(app.LONG_INPUT)[1]), 4096)

    def test_complete_native_corpus_contains_every_unicode_scalar_once_in_order(self):
        count, previous = 0, -1
        for _, payload in app.unicode_chunks():
            self.assertLessEqual(len(payload), 65536)
            for scalar in payload.decode("utf-8", errors="strict"):
                value = ord(scalar)
                wanted = previous + 1
                if wanted == 0xD800:
                    wanted = 0xE000
                self.assertEqual(value, wanted)
                previous = value
                count += 1
        self.assertEqual((count, previous), (1112064, 0x10FFFF))

    def test_lock_oracle_preserves_both_members_shared_external_library_and_exact_sources(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for path in (app.APP, app.LIBRARY):
                shutil.copytree(app.ROOT / path, root / path, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
            project = root / app.APP
            ids = {location: app.identity(location) for location in app.LOCATIONS}
            packages = []
            for location in app.LOCATIONS:
                manifest = project / location / "oxid.toml"
                metadata = tomllib.loads(manifest.read_text())["project"]
                entry = "src/lib.ox" if location == app.LOCATIONS[2] else "src/main.ox"
                packages.append({"id": ids[location], "path": location, "name": metadata["name"],
                    "version": metadata["version"], "edition": "typed-preview", "entry": entry,
                    "manifest": app.digest(manifest), "sources": app.reference_source_checksum(project, location, entry)
                    if location == app.LOCATIONS[2] else "root"})
            valid = {"version": 3, "kind": "typed-workspace-v1", "manifest": app.digest(project / "oxid.toml"),
                     "member": [{"package": ids[p]} for p in sorted(app.LOCATIONS[:2], key=ids.__getitem__)],
                     "package": sorted(packages, key=lambda p: p["id"]),
                     "dependency": sorted([{"owner": ids[p], "alias": "text", "package": ids[app.LOCATIONS[2]]}
                                           for p in app.LOCATIONS[:2]], key=lambda p: p["owner"])}
            self.assertEqual(app.inspect_lock(project, valid), valid)
            self.assertEqual(len(app.lock_controls(project, valid)), 6)
            unknown = copy.deepcopy(valid)
            unknown["future"] = True
            with self.assertRaises(ValueError):
                app.inspect_lock(project, unknown)
            library = project / app.LOCATIONS[2] / "src/lib.ox"
            library.write_bytes(library.read_bytes() + b"\n// Changed bytes.\n")
            with self.assertRaisesRegex(ValueError, "source identity"):
                app.inspect_lock(project, valid)


if __name__ == "__main__":
    unittest.main()
