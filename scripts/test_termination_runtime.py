"""Real process-signal contracts for the opt-in native runtime helper."""
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
SUPPORTED = (platform.system() in ("Linux", "Darwin")
             and platform.machine().lower() in ("x86_64", "amd64", "aarch64", "arm64")
             and struct.calcsize("P") == 8 and sys.byteorder == "little")


@unittest.skipUnless(SUPPORTED, "termination runtime requires a supported POSIX ABI")
class TerminationRuntime(unittest.TestCase):
    def test_opt_in_sticky_notifications_rollback_and_scope_restoration(self):
        selected = os.environ.get("OXID_TEST_CC") or shutil.which("clang") or shutil.which("cc")
        self.assertIsNotNone(selected, "a real C compiler is required")
        evidence = os.environ.get("OXID_TEST_TERMINATION_NATIVE_EVIDENCE")
        with tempfile.TemporaryDirectory(prefix="oxid-termination-runtime-") as temporary:
            output = Path(evidence) if evidence else Path(temporary)
            output.mkdir(parents=True, exist_ok=True)
            commands = []
            sanitizer = os.environ.get("OXID_TERMINATION_SANITIZE") == "1"
            for optimization in ("O0", "O2"):
                for unsupported in (False, True):
                    label = optimization + ("-unsupported" if unsupported else "-supported")
                    executable = output / label
                    command = [selected, "-std=c11", "-Wall", "-Wextra", "-Werror",
                               "-" + optimization, "-g", str(ROOT / "tests/termination_runtime.c"),
                               "-o", str(executable)]
                    if unsupported:
                        command += ["-DOXID_TERMINATION_FORCE_UNSUPPORTED=1"]
                    if sanitizer:
                        command += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
                    built = subprocess.run(command, capture_output=True, timeout=120, check=False)
                    self.assertEqual(built.returncode, 0, built.stderr.decode(errors="replace"))
                    run = subprocess.run([str(executable)], capture_output=True, timeout=20, check=False)
                    self.assertEqual(run.returncode, 0, run.stderr.decode(errors="replace"))
                    self.assertEqual((run.stdout, run.stderr), (b"", b""))
                    if evidence:
                        for name, content in (("build.stdout", built.stdout), ("build.stderr", built.stderr),
                                              ("run.stdout", run.stdout), ("run.stderr", run.stderr)):
                            (output / (label + "." + name)).write_bytes(content)
                    commands.append({"optimization": optimization, "unsupported": unsupported,
                                     "build": command, "build_status": built.returncode,
                                     "run": [str(executable)], "run_status": run.returncode})
            if evidence:
                (output / "result.json").write_text(json.dumps({"status": "passed", "sanitizer": sanitizer,
                                                                "commands": commands}, indent=2) + "\n")


if __name__ == "__main__":
    unittest.main()
