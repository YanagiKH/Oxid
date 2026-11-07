"""Require real host executable formats in typed-vector native evidence."""
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

import verify_typed_vectors as gate
from verify_float_vector import verify_policy_variation


class NativeVectorOutput(unittest.TestCase):
    def inspect(self, body, system="linux", machine="x86_64"):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "program"
            path.write_bytes(body)
            with patch.object(gate.sys, "platform", system), \
                    patch.object(gate.platform, "machine", return_value=machine):
                return gate.native_identity(path)

    def elf(self, machine=62, kind=2):
        # System V ELF64: EI_CLASS=2, EI_DATA=1, e_type/e_machine at16/18.
        header = bytearray(64)
        header[:7] = b"\x7fELF\x02\x01\x01"
        struct.pack_into("<HH", header, 16, kind, machine)
        return header

    def macho(self, machine=0x01000007, kind=2):
        # Darwin mach_header_64: CPU_TYPE_*|ABI64 and MH_EXECUTE=2.
        return struct.pack("<8I", 0xfeedfacf, machine, 3, kind, 0, 0, 0, 0)

    def test_linux_accepts_native_x86_64_and_arm64_headers(self):
        self.assertEqual(self.inspect(self.elf()), {"format": "ELF64", "machine": "x86_64"})
        self.assertEqual(self.inspect(self.elf(183), machine="aarch64"),
                         {"format": "ELF64", "machine": "aarch64"})
        self.assertEqual(self.inspect(self.elf(kind=3))["format"], "ELF64")

    def test_linux_rejects_scripts_foreign_cpu_and_nonexecutables(self):
        for body in (b"#!/bin/sh\nprintf '42\\n'\n", self.elf(183), self.elf(kind=1),
                     self.elf(kind=4), self.elf()[:19], self.macho()):
            with self.subTest(body=bytes(body[:20])), self.assertRaises(ValueError):
                self.inspect(body)

    def test_macos_accepts_thin_x86_64_and_arm64_executable_headers(self):
        self.assertEqual(self.inspect(self.macho(), "darwin"),
                         {"format": "Mach-O64", "machine": "x86_64"})
        self.assertEqual(self.inspect(self.macho(0x0100000c), "darwin", "arm64"),
                         {"format": "Mach-O64", "machine": "arm64"})

    def test_macos_rejects_wrong_cpu_dylib_object_elf_and_truncated_headers(self):
        for body in (self.macho(0x0100000c), self.macho(kind=6), self.macho(kind=1),
                     self.elf(), self.macho()[:31], b"\xca\xfe\xba\xbe" + b"\0" * 28):
            with self.subTest(body=bytes(body[:20])), self.assertRaises(ValueError):
                self.inspect(body, "darwin")

    def test_unknown_native_platform_and_machine_are_refused(self):
        for system, machine in (("win32", "AMD64"), ("linux", "riscv64"), ("darwin", "ppc")):
            with self.subTest(system=system, machine=machine), self.assertRaises(ValueError):
                self.inspect(self.elf(), system, machine)


class ExecutionPolicyCode(unittest.TestCase):
    def artifacts(self):
        return {(policy, name, profile): ("0" if policy == "bounded" else "1") * 64
                for policy in ("bounded", "hosted")
                for name in ("finite-array", "heap-vector")
                for profile in ("dev", "release")}

    def test_default_requires_distinct_bytes_for_every_case_and_profile(self):
        artifacts = self.artifacts()
        self.assertEqual(verify_policy_variation(artifacts, ("finite-array", "heap-vector")), [])
        for name in ("finite-array", "heap-vector"):
            for profile in ("dev", "release"):
                changed = dict(artifacts)
                changed[("hosted", name, profile)] = changed[("bounded", name, profile)]
                with self.subTest(name=name, profile=profile), self.assertRaises(ValueError):
                    verify_policy_variation(changed, ("finite-array", "heap-vector"))

    def test_declared_finite_case_still_leaves_heap_cases_strict(self):
        artifacts = self.artifacts()
        for profile in ("dev", "release"):
            artifacts[("hosted", "finite-array", profile)] = artifacts[("bounded", "finite-array", profile)]
        rows = verify_policy_variation(artifacts, ("finite-array", "heap-vector"), ("finite-array",))
        self.assertEqual(rows, [{"case": "finite-array", "profile": profile, "sha256": "0" * 64}
                               for profile in ("dev", "release")])
        for profile in ("dev", "release"):
            changed = dict(artifacts)
            changed[("hosted", "heap-vector", profile)] = changed[("bounded", "heap-vector", profile)]
            with self.subTest(profile=profile), self.assertRaises(ValueError):
                verify_policy_variation(changed, ("finite-array", "heap-vector"), ("finite-array",))

    def test_misspelled_exception_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unknown policy-equivalent"):
            verify_policy_variation(self.artifacts(), ("finite-array", "heap-vector"), ("typo",))


if __name__ == "__main__":
    unittest.main()
