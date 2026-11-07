"""Owned buffer transfer C component checks, separate from complete native source execution."""
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(platform.system() == "Linux" and platform.machine() == "x86_64",
                     "binary delivery requires Linux x86_64")
class BytebufTransfer(unittest.TestCase):
    def test_ownership_transfer_refusals_and_full_heap(self):
        compiler = shutil.which(os.environ.get("CC", "cc"))
        if compiler is None:
            self.skipTest("C compiler unavailable")
        with tempfile.TemporaryDirectory(prefix="oxid-bytebuf-transfer-") as directory:
            for optimization in ("O0", "O2"):
                with self.subTest(optimization=optimization):
                    binary = Path(directory) / ("bytebuf-transfer-" + optimization)
                    command = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-" + optimization,
                               "-g", "-DOXID_BYTEBUF_TESTING", str(ROOT / "tests/bytebuf_transfer.c"),
                               "-o", str(binary)]
                    if os.environ.get("OXID_BYTEBUF_SANITIZE") == "1":
                        command[1:1] = ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
                    built = subprocess.run(command, capture_output=True, timeout=120)
                    print("compile command:", command, "exit:", built.returncode, flush=True)
                    self.assertEqual(built.returncode, 0, (built.stdout, built.stderr))
                    print("component sha256:", hashlib.sha256(binary.read_bytes()).hexdigest(), flush=True)
                    result = subprocess.run([str(binary)], cwd=directory, capture_output=True, timeout=60)
                    print("run command:", [str(binary)], "exit:", result.returncode, flush=True)
                    self.assertEqual((result.returncode, result.stdout, result.stderr), (0, b"", b""))


if __name__ == "__main__":
    unittest.main(verbosity=2)
