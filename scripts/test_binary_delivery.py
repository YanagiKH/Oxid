"""Focused C adapter checks; do not claim native Oxid execution."""
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
class BinaryDelivery(unittest.TestCase):
    def test_preflight_refusal_and_direct_status_commit(self):
        compiler = shutil.which(os.environ.get("CC", "cc"))
        if compiler is None:
            self.skipTest("C compiler unavailable")
        with tempfile.TemporaryDirectory(prefix="oxid-binary-delivery-") as directory:
            for optimization in ("O0", "O2"):
                with self.subTest(optimization=optimization):
                    binary = Path(directory) / f"binary-delivery-{optimization}"
                    command = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-" + optimization, "-g",
                               "-DOXID_BYTEBUF_TESTING", str(ROOT / "tests/binary_delivery.c"),
                               "-o", str(binary)]
                    if os.environ.get("OXID_BYTEBUF_SANITIZE") == "1":
                        command[1:1] = ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
                    compiled = subprocess.run(command, capture_output=True, timeout=120)
                    print("compile command:", command, "exit:", compiled.returncode, flush=True)
                    self.assertEqual(compiled.returncode, 0, (compiled.stdout, compiled.stderr))
                    print("component sha256:", hashlib.sha256(binary.read_bytes()).hexdigest(), flush=True)
                    result = subprocess.run([str(binary)], cwd=directory, capture_output=True, timeout=30)
                    print("run command:", [str(binary)], "cwd:", directory, "exit:", result.returncode, flush=True)
                    self.assertEqual((result.returncode, result.stdout, result.stderr), (0, b"", b""))


if __name__ == "__main__":
    unittest.main(verbosity=2)
