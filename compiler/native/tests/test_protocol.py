#!/usr/bin/env python3
"""Host-driver protocol controls. No fake performs Oxid semantics."""
from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("scalar_compiler_build", Path(__file__).resolve().parents[1] / "build.py")
build_driver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(build_driver)
MODULE = build_driver.PREFIX + b"define i32 @main() { ret i32 0 }\n" + build_driver.SUFFIX


class Protocol(unittest.TestCase):
    def result(self, stdout=MODULE, stderr=b"", status=0):
        return subprocess.CompletedProcess(["compiler", "input.ox"], status, stdout, stderr)

    def test_exact_complete_success(self):
        self.assertEqual(build_driver.module_from_result(self.result()), MODULE)

    def test_no_return_value_banner_or_diagnostic_is_stripped(self):
        for data in (MODULE + b"0\n", b"0\n" + MODULE, b"diagnostic\n" + MODULE,
                     MODULE + b"diagnostic\n", MODULE[:-1], build_driver.PREFIX,
                     MODULE + MODULE, b"", MODULE.replace(b"ret i32", b"\x00ret i32")):
            with self.subTest(data=data):
                with self.assertRaises(build_driver.BuildError):
                    build_driver.module_from_result(self.result(stdout=data))

    def test_partial_output_failed_status_or_diagnostics_never_succeeds(self):
        for result in (self.result(status=65), self.result(status=74),
                       self.result(stderr=b"error\n"), self.result(stdout=build_driver.PREFIX, status=74)):
            with self.assertRaises(build_driver.BuildError):
                build_driver.module_from_result(result)

    def script(self, root, name, body):
        path = root / name
        path.write_text("#!/usr/bin/env python3\n" + body)
        path.chmod(0o700)
        return path

    def inputs(self, root):
        source = root / "input.ox"
        source.write_text("fn main()->i32{return 42;}")
        runtime = root / "runtime.c"
        runtime.write_text("/* protocol-only fixture; never used for semantic acceptance */\n")
        compiler = self.script(root, "compiler", "import sys\nsys.stdout.buffer.write(" + repr(MODULE) + ")\n")
        clang = self.script(root, "clang", "raise SystemExit(19)\n")
        return compiler, source, clang, runtime

    def test_existing_file_and_symlink_are_preserved_before_compiler_invocation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            compiler, source, clang, runtime = self.inputs(root)
            destination = root / "output"
            destination.write_bytes(b"keep\x00bytes")
            for _ in range(2):
                before = destination.lstat()
                with self.assertRaisesRegex(build_driver.BuildError, "already exists"):
                    build_driver.build(compiler, source, clang, runtime, destination, "dev")
                self.assertEqual(destination.lstat().st_ino, before.st_ino)
                self.assertEqual(destination.read_bytes(), b"keep\x00bytes")
            destination.unlink()
            destination.symlink_to(root / "absent")
            with self.assertRaisesRegex(build_driver.BuildError, "already exists"):
                build_driver.build(compiler, source, clang, runtime, destination, "dev")
            self.assertTrue(destination.is_symlink())
            self.assertFalse((root / "absent").exists())

    def test_clang_failure_creates_no_published_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            compiler, source, clang, runtime = self.inputs(root)
            destination = root / "output"
            with self.assertRaisesRegex(build_driver.BuildError, "Clang rejected"):
                build_driver.build(compiler, source, clang, runtime, destination, "release")
            self.assertFalse(os.path.lexists(destination))
            self.assertFalse(list(root.glob(".oxid-scalar-build-*")))

    def test_rejected_stream_never_reaches_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, source, clang, runtime = self.inputs(root)
            streams = ((MODULE + b"0\n", b"", 0),
                       (MODULE, b"diagnostic\n", 0),
                       (build_driver.PREFIX, b"", 74),
                       (b"", b"", 65))
            for index, (stdout, stderr, status) in enumerate(streams):
                compiler = self.script(root, "compiler-" + str(index),
                    "import sys\nsys.stdout.buffer.write(" + repr(stdout) + ")\n"
                    "sys.stderr.buffer.write(" + repr(stderr) + ")\n"
                    "raise SystemExit(" + str(status) + ")\n")
                destination = root / ("output-" + str(index))
                with self.assertRaises(build_driver.BuildError):
                    build_driver.build(compiler, source, clang, runtime, destination, "dev")
                self.assertFalse(os.path.lexists(destination))
                self.assertFalse(list(root.glob(".oxid-scalar-build-*")))

    def test_output_created_during_link_is_not_replaced(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            compiler, source, _, runtime = self.inputs(root)
            destination = root / "output"
            clang = self.script(root, "race-clang", "from pathlib import Path\nimport sys\n"
                "p=Path(sys.argv[sys.argv.index('-o')+1]);p.write_bytes(b'executable');p.chmod(0o700)\n"
                f"Path({str(destination)!r}).write_bytes(b'raced')\n")
            with self.assertRaisesRegex(build_driver.BuildError, "appeared"):
                build_driver.build(compiler, source, clang, runtime, destination, "dev")
            self.assertEqual(destination.read_bytes(), b"raced")


if __name__ == "__main__":
    unittest.main()
