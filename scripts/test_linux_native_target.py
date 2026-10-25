import unittest
from unittest.mock import patch

import linux_native_target as target


class LinuxNativeTargetTests(unittest.TestCase):
    def test_supported_hosts_have_their_distinct_standard_elf_identities(self):
        for machine, expected, identity in [
                ("x86_64", target.X86_64, b"\x03\x00\x3e\x00"),
                ("aarch64", target.ARM64, b"\x03\x00\xb7\x00")]:
            self.assertEqual(target.select("Linux", machine, "little", 8), expected)
            with patch.object(target, "host", return_value=expected):
                self.assertEqual(target.elf_identity(), identity)
                self.assertEqual(target.elf_identity(1), b"\x01\x00" + identity[2:])
        self.assertNotEqual(target.X86_64.elf_machine, target.ARM64.elf_machine)

    def test_unknown_oses_architectures_byte_orders_and_widths_are_rejected(self):
        for values in [("Darwin", "aarch64", "little", 8),
                       ("Windows", "AMD64", "little", 8),
                       ("Linux", "riscv64", "little", 8),
                       ("Linux", "aarch64", "big", 8),
                       ("Linux", "x86_64", "little", 4)]:
            self.assertIsNone(target.select(*values))
        with patch.object(target, "host", return_value=None):
            with self.assertRaisesRegex(ValueError, "requires Linux"):
                target.elf_identity()


if __name__ == "__main__":
    unittest.main()
