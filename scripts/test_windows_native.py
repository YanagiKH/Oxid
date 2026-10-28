"""Portable corruption controls for the independent Windows PE verifier."""
import struct
import unittest

from verify_windows_native import pe


def image(library=b"KERNEL32.dll"):
    data = bytearray(1024)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 60, 128)
    data[128:132] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", data, 132, 0x8664, 1, 123, 0, 0, 240, 2)
    optional = 152
    struct.pack_into("<H", data, optional, 0x20B)
    struct.pack_into("<HH", data, optional + 68, 3, 0x160)
    struct.pack_into("<QQ", data, optional + 72, 8388608, 4096)
    struct.pack_into("<I", data, optional + 108, 16)
    struct.pack_into("<II", data, optional + 120, 0x1000, 40)
    struct.pack_into("<II", data, optional + 152, 0x1100, 12)
    struct.pack_into("<4I", data, 392 + 8, 512, 0x1000, 512, 512)
    struct.pack_into("<5I", data, 512, 0x1060, 0, 0, 0x1040, 0x1080)
    data[576:576 + len(library) + 1] = library + b"\0"
    return data


class WindowsNativePeTests(unittest.TestCase):
    def test_reads_real_field_offsets_and_import_rvas(self):
        details = pe(image())
        self.assertEqual(details["machine"], 0x8664)
        self.assertEqual(details["format"], "PE32+")
        self.assertEqual(details["dll_flags"], 0x160)
        self.assertEqual(details["stack_reserve"], 8388608)
        self.assertEqual(details["stack_commit"], 4096)
        self.assertEqual(details["imports"], ["kernel32.dll"])

    def test_each_loader_or_hardening_boundary_is_required(self):
        # Exact independent PE field offsets, not values exported by the compiler.
        for name, offset, encoding, value in [
            ("machine", 132, "H", 0xAA64),
            ("optional-magic", 152, "H", 0x10B),
            ("not-executable", 150, "H", 0),
            ("dll-image", 150, "H", 0x2002),
            ("stripped-relocations", 150, "H", 3),
            ("no-base-relocations", 304, "I", 0),
            ("truncated-base-relocations", 308, "I", 7),
            ("non-console", 220, "H", 2),
            ("no-aslr", 222, "H", 0x120),
            ("no-high-entropy", 222, "H", 0x140),
            ("no-nx", 222, "H", 0x60),
            ("small-stack", 224, "Q", 1048576),
            ("excess-stack-commit", 232, "Q", 8388609),
            ("unmapped-imports", 272, "I", 0x2000),
            ("truncated-imports", 276, "I", 19),
            ("oversized-sections", 408, "I", 513),
        ]:
            with self.subTest(name=name):
                data = image()
                struct.pack_into("<" + encoding, data, offset, value)
                with self.assertRaises(ValueError):
                    pe(data)

    def test_dynamic_crt_and_external_imports_are_refused(self):
        for name in (b"VCRUNTIME140.dll", b"ucrtbase.dll", b"MSVCRT.dll", b"LLVM.dll",
                     b"C:\\tools\\runtime.dll", b"..\\evil.dll", b"other.dll"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                pe(image(name))
        self.assertEqual(pe(image(b"api-ms-win-core-file-l1-1-0.dll"))["imports"],
                         ["api-ms-win-core-file-l1-1-0.dll"])

    def test_truncated_or_unterminated_metadata_is_not_an_artifact(self):
        for length in (0, 63, 128, 151, 391, 511, 1023):
            with self.subTest(length=length), self.assertRaises(ValueError):
                pe(image()[:length])
        data = image()
        struct.pack_into("<5I", data, 532, 0x1060, 0, 0, 0x1040, 0x1080)
        with self.assertRaisesRegex(ValueError, "unterminated"):
            pe(data)
        data = image()
        data[576:] = b"A" * (len(data) - 576)
        with self.assertRaisesRegex(ValueError, "DLL import name"):
            pe(data)


if __name__ == "__main__":
    unittest.main()
