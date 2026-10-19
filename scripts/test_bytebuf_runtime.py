"""Dynamic runtime component checks; separate from native-source acceptance."""
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
                     "experimental byte runtime requires Linux x86_64")
class BytebufRuntime(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        compiler = shutil.which(os.environ.get("CC", "cc"))
        if compiler is None:
            raise unittest.SkipTest("C compiler unavailable")
        cls.temp = tempfile.TemporaryDirectory(prefix="oxid-bytebuf-")
        cls.addClassCleanup(cls.temp.cleanup)
        cls.binary = Path(cls.temp.name) / "bytebuf-runtime"
        command = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-O0", "-g",
                   "-DOXID_BYTEBUF_TESTING", str(ROOT / "tests/bytebuf_runtime.c"),
                   "-o", str(cls.binary)]
        if os.environ.get("OXID_BYTEBUF_SANITIZE") == "1":
            command[1:1] = ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
        result = subprocess.run(command, capture_output=True)
        if result.returncode:
            raise AssertionError((command, result.returncode, result.stdout, result.stderr))
        print("runtime command:", command, flush=True)
        print("runtime artifact sha256:", hashlib.sha256(cls.binary.read_bytes()).hexdigest(), flush=True)
        cls.adapter = Path(cls.temp.name) / "bytebuf-adapter"
        adapter_command = [str(ROOT / "tests/bytebuf_adapter.c") if part == str(ROOT / "tests/bytebuf_runtime.c")
                           else str(cls.adapter) if part == str(cls.binary) else part for part in command]
        result = subprocess.run(adapter_command, capture_output=True)
        if result.returncode:
            raise AssertionError((adapter_command, result.returncode, result.stdout, result.stderr))
        print("adapter command:", adapter_command, flush=True)
        print("adapter artifact sha256:", hashlib.sha256(cls.adapter.read_bytes()).hexdigest(), flush=True)

    def run_runtime(self, *args, input_bytes=b""):
        result = subprocess.run([str(self.binary), *args], cwd=self.temp.name,
                                input=input_bytes, capture_output=True)
        self.assertEqual(result.returncode, 0, (args, result.returncode, result.stderr))
        self.assertEqual(result.stderr, b"")
        return result.stdout

    def test_allocation_faults_growth_bounds_and_cleanup(self):
        self.assertEqual(self.run_runtime(), b"")

    def test_generated_runtime_adapter_move_cleanup_and_leak_detection(self):
        result = subprocess.run([str(self.adapter)], cwd=self.temp.name, capture_output=True)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, b"", b""))

    def test_injected_transport_partial_interrupt_error_eof_zero_progress(self):
        self.assertEqual(self.run_runtime("io"), b"")

    def test_actual_stdin_stdout_extent_in_source_free_working_directory(self):
        for length in [0, 1, 3, 1023, 1024, 1025, 8199]:
            with self.subTest(length=length):
                data = (bytes([0, 128, 255]) if length == 3 else
                        bytes((i * 73 + 19) & 255 for i in range(length)))
                self.assertEqual(self.run_runtime("echo", input_bytes=data), data)


if __name__ == "__main__":
    unittest.main(verbosity=2)
