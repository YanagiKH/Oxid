"""File resource lifetime and OS-attempt contracts in the native C adapter."""
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(platform.system() in ("Linux", "Darwin")
                         and platform.machine() in ("x86_64", "aarch64", "arm64"),
                     "native files require Linux or macOS on x86_64 or ARM64")
class NativeFiles(unittest.TestCase):
    def test_file_resources_faults_attempts_and_descriptor_classes(self):
        compiler = shutil.which(os.environ.get("CC", "cc"))
        if compiler is None:
            self.skipTest("C compiler unavailable")
        with tempfile.TemporaryDirectory(prefix="oxid-native-files-") as directory:
            for optimization in ("O0", "O2"):
                with self.subTest(optimization=optimization):
                    binary = Path(directory) / optimization
                    command = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror",
                               "-" + optimization, str(ROOT / "tests/native_files_runtime.c"),
                               "-o", str(binary)]
                    if os.environ.get("OXID_BYTEBUF_SANITIZE") == "1":
                        command[1:1] = ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
                    built = subprocess.run(command, capture_output=True, timeout=120)
                    self.assertEqual(built.returncode, 0, (built.stdout, built.stderr))
                    result = subprocess.run([str(binary)], cwd=directory,
                                            capture_output=True, timeout=60)
                    self.assertEqual((result.returncode, result.stdout, result.stderr),
                                     (0, b"", b""))


if __name__ == "__main__":
    unittest.main(verbosity=2)
