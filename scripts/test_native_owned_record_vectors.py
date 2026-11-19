"""Run independent owned-record/vector C contracts on Linux and macOS64 hosts.

Set OXID_BYTEBUF_SANITIZE=1 to instrument every O0/O2 configuration with both
AddressSanitizer and UndefinedBehaviorSanitizer. Failures retain raw streams,
commands and binaries in the reported temporary directory.
"""
import json
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SUPPORTED_HOST = (platform.system() in ("Linux", "Darwin")
                  and platform.machine().lower() in ("x86_64", "aarch64", "arm64")
                  and struct.calcsize("P") == 8 and sys.byteorder == "little")
CONFIGURATIONS = (
    ("operations", "tests/owned_vectors_runtime.c", (),
     b"owned vector operations: 8 component groups passed\n"),
    ("registry", "tests/owned_vectors_registry_runtime.c", (),
     b"owned vector registry components: 5 passed\n"),
    ("codec", "tests/owned_record_codec_runtime.c", (),
     b"owned record codec components: 5 passed\n"),
    ("composed-codec", "tests/owned_record_codec_runtime.c",
     ("-DOXID_TEST_OWNED_VECTOR_COMPOSED_OPERATIONS",),
     b"owned record codec and vector components: 6 passed\n"),
)


def retained_command(directory, label, argv, environment, timeout):
    """Keep actual failures intact; no retry, output filtering or fallback mode."""
    row = {"argv": [str(value) for value in argv], "cwd": str(directory),
           "timeout_seconds": timeout,
           "sanitizer_options": {name: environment[name] for name in ("ASAN_OPTIONS", "UBSAN_OPTIONS")
                                 if name in environment}}
    try:
        result = subprocess.run(row["argv"], cwd=directory, env=environment,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout)
        (directory / (label + ".stdout")).write_bytes(result.stdout)
        (directory / (label + ".stderr")).write_bytes(result.stderr)
        row["exit_status"] = result.returncode
        return result
    except subprocess.TimeoutExpired as error:
        (directory / (label + ".stdout")).write_bytes(error.stdout or b"")
        (directory / (label + ".stderr")).write_bytes(error.stderr or b"")
        row["error"] = "command timed out"
        raise
    except OSError as error:
        row["error"] = str(error)
        raise
    finally:
        (directory / (label + ".json")).write_text(json.dumps(row, indent=2) + "\n")


@unittest.skipUnless(SUPPORTED_HOST,
                     "owned-record/vector C contracts require Linux or macOS on 64-bit little-endian x86_64/ARM64")
class NativeOwnedRecordVectors(unittest.TestCase):
    def test_operations_registry_codec_and_composed_resources(self):
        compiler_name = os.environ.get("CC", "cc")
        compiler = shutil.which(compiler_name)
        self.assertIsNotNone(compiler, "supported-host C contracts require an actual compiler: " + compiler_name)
        sanitizer = os.environ.get("OXID_BYTEBUF_SANITIZE") == "1"
        environment = dict(os.environ)
        if sanitizer:
            # An inherited recover/suppress/zero-exit setting cannot turn a
            # sanitizer finding into a successful test.
            environment["ASAN_OPTIONS"] = "halt_on_error=1:abort_on_error=1:exitcode=1"
            environment["UBSAN_OPTIONS"] = "halt_on_error=1:print_stacktrace=1"
            environment.pop("LSAN_OPTIONS", None)
        directory = Path(tempfile.mkdtemp(prefix="oxid-owned-record-vector-c-"))
        passed = 0
        try:
            for name, source, definitions, expected in CONFIGURATIONS:
                for optimization in ("O0", "O2"):
                    with self.subTest(configuration=name, optimization=optimization, sanitizer=sanitizer):
                        output = directory / (name + "-" + optimization)
                        output.mkdir()
                        binary = output / "contract"
                        command = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror",
                                   "-" + optimization, "-UNDEBUG", *definitions]
                        if sanitizer:
                            command += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer",
                                        "-fno-sanitize-recover=all"]
                        command += [str(ROOT / source), "-o", str(binary)]
                        evidence = "retained C contract evidence: " + str(output)
                        built = retained_command(output, "compile", command, environment, 120)
                        self.assertEqual(built.returncode, 0, (evidence, built.stdout, built.stderr))
                        result = retained_command(output, "run", [binary], environment, 60)
                        self.assertEqual((result.returncode, result.stdout, result.stderr),
                                         (0, expected, b""), (evidence, result.stdout, result.stderr))
                        passed += 1
        finally:
            if passed == 8 and os.environ.get("OXID_TEST_KEEP_ARTIFACTS") != "1":
                shutil.rmtree(directory)


if __name__ == "__main__":
    unittest.main(verbosity=2)
