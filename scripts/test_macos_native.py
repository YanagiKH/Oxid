"""Prevent malformed or foreign native outputs from qualifying as Mach-O."""
import struct
import unittest

from verify_macos_native import macho


def image(cpu=0x1000007, library=b"/usr/lib/libSystem.B.dylib", signature=False):
    length = (24 + len(library) + 1 + 7) & ~7
    command = struct.pack("<6I", 0xC, length, 24, 0, 0, 0)
    command += (library + b"\0").ljust(length - 24, b"\0")
    if signature:
        command += struct.pack("<4I", 0x1D, 16, 32 + length + 16, 8)
    header = struct.pack("<8I", 0xFEEDFACF, cpu, 0, 2, 1 + signature,
                         len(command), 0x200000, 0)
    return header + command + (b"signature"[:8] if signature else b"")


class MachoQualification(unittest.TestCase):
    def test_both_native_cpu_types_require_system_libraries_and_arm64_signature(self):
        self.assertEqual(macho(image(), 0x1000007)["libraries"], ["/usr/lib/libSystem.B.dylib"])
        self.assertTrue(macho(image(0x100000C, signature=True), 0x100000C)["code_signature"])
        with self.assertRaisesRegex(ValueError, "no code signature"):
            macho(image(0x100000C), 0x100000C)

    def test_foreign_architecture_non_executable_and_non_pie_outputs_are_rejected(self):
        for offset, value in [(0, 0), (4, 0x100000C), (12, 1), (24, 0)]:
            changed = bytearray(image())
            struct.pack_into("<I", changed, offset, value)
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                macho(changed, 0x1000007)

    def test_non_system_dependencies_cannot_qualify_as_standalone_programs(self):
        for library in (b"@rpath/libLLVM.dylib", b"/tmp/project/liboxid.dylib", b"libc.dylib"):
            with self.subTest(library=library), self.assertRaisesRegex(ValueError, "non-system"):
                macho(image(library=library), 0x1000007)

    def test_truncation_and_invalid_load_command_extents_are_rejected(self):
        for data in (image()[:31], image()[:-1]):
            with self.assertRaises(ValueError):
                macho(data, 0x1000007)
        for offset, value in [(16, 100), (20, 0xFFFFFFFF), (36, 4), (36, 0xFFFFFFF8), (40, 4)]:
            changed = bytearray(image())
            struct.pack_into("<I", changed, offset, value)
            with self.subTest(offset=offset, value=value), self.assertRaises(ValueError):
                macho(changed, 0x1000007)

    def test_arm64_signature_must_fit_inside_the_actual_artifact(self):
        data = bytearray(image(0x100000C, signature=True))
        position = 32 + struct.unpack_from("<I", data, 36)[0]
        struct.pack_into("<I", data, position + 8, len(data))
        with self.assertRaisesRegex(ValueError, "signature extent"):
            macho(data, 0x100000C)


if __name__ == "__main__":
    unittest.main()
