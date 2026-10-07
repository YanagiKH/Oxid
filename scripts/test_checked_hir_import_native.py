"""Driver controls only; fake commands do not qualify LLVM or native execution."""
from pathlib import Path
import sys
import tempfile
import unittest

import verify_checked_hir_import_native as gate


@unittest.skipUnless(sys.platform == "linux", "Linux-only process-group controls")
class DriverTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def run_python(self, text, name="probe", seconds=5):
        return gate.run(self.root, name, [sys.executable, "-c", text], cwd=self.root,
                        env={"PATH": "/no-tools"}, seconds=seconds)

    def test_exact_streams_and_nonzero_exit_are_preserved(self):
        result = self.run_python("import sys; print('out'); print('err', file=sys.stderr); sys.exit(7)")
        self.assertEqual(result, (7, b"out\n", b"err\n"))
        with self.assertRaisesRegex(RuntimeError, "status 7"):
            gate.success(result, "probe")
        self.assertTrue((self.root / "probe.json").is_file())

    def test_timeout_kills_process_group_and_retains_receipt(self):
        with self.assertRaisesRegex(RuntimeError, "exceeded"):
            self.run_python("import time; time.sleep(20)", seconds=0.05)
        import json
        receipt = json.loads((self.root / "probe.json").read_text())
        self.assertTrue(receipt["timed_out"])
        self.assertEqual(receipt["status"], -9)

    def test_timeout_kills_spawned_descendant(self):
        import time
        child = "import os, pathlib, time; pathlib.Path('descendant.pid').write_text(str(os.getpid())); time.sleep(30)"
        leader = f"import subprocess, sys, time; subprocess.Popen([sys.executable, '-c', {child!r}]); time.sleep(30)"
        with self.assertRaisesRegex(RuntimeError, "exceeded"):
            self.run_python(leader, seconds=0.5)
        pid = int((self.root / "descendant.pid").read_text())
        state = Path(f"/proc/{pid}/stat")
        for _ in range(100):
            try:
                # A killed orphan can briefly remain a zombie for host init.
                if state.read_text().split(") ", 1)[1].split()[0] == "Z":
                    break
            except FileNotFoundError:
                break
            time.sleep(0.01)
        else:
            self.fail("timed-out descendant is still running")

    def test_evidence_rejects_even_ignored_in_tree_destinations(self):
        with self.assertRaisesRegex(RuntimeError, "outside the checkout"):
            gate.evidence_destination(gate.ROOT / "target" / "native-gate-evidence")
        self.assertEqual(gate.evidence_destination(self.root), self.root.resolve())

    def test_no_stdin_or_ambient_environment(self):
        result = self.run_python("import os, sys; assert sys.stdin.read() == ''; assert 'HOME' not in os.environ")
        self.assertEqual(result, (0, b"", b""))

    def test_evidence_cannot_be_replaced(self):
        self.run_python("print('original')")
        with self.assertRaises(FileExistsError):
            self.run_python("print('replacement')")
        self.assertEqual((self.root / "probe.stdout").read_bytes(), b"original\n")

    def test_output_read_is_bounded(self):
        with self.assertRaisesRegex(RuntimeError, "1 MiB"):
            self.run_python("import sys; sys.stdout.write('x' * (2 << 20))")
        self.assertEqual((self.root / "probe.stdout").stat().st_size, 2 << 20)

    def test_elf_requires_pie_architecture(self):
        path = self.root / "program.elf"
        header = bytearray(20)
        header[:6] = b"\x7fELF\x02\x01"
        header[16:20] = b"\x03\x00\x3e\x00"
        path.write_bytes(header)
        gate.verify_elf(path)
        header[16] = 2
        path.write_bytes(header)
        with self.assertRaisesRegex(RuntimeError, "ELF64 PIE"):
            gate.verify_elf(path)


if __name__ == "__main__":
    unittest.main()
