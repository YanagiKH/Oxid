"""Check 64-bit-part runtime algorithms against Python arbitrary precision."""
import os
from pathlib import Path
import platform
import random
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
MASK = 2**128 - 1
SENTINEL = "123456789abcdef0 fedcba9876543210"


def parts(value):
    return f"{value & (2**64-1):016x} {value >> 64:016x}"


def signed(value):
    return value - 2**128 if value >= 2**127 else value


def corpus():
    values = {0, 1, 2, 3, 9, 10, 2**32 - 1, 2**32, 2**64 - 1, 2**64,
              2**64 + 1, 2**127 - 1, 2**127, 2**127 + 1, MASK - 1, MASK}
    pairs = {(a, b) for a in values for b in values}
    rng = random.Random(0x128C0DE)
    pairs |= {(rng.getrandbits(128), rng.getrandbits(128)) for _ in range(1000)}
    # Include products that fit, not only uniformly random overflowing products.
    pairs |= {(rng.getrandbits(64), rng.getrandbits(64)) for _ in range(500)}
    pairs |= {(rng.getrandbits(127), rng.randrange(4)) for _ in range(200)}
    rows, expected = [], []
    for a, b in sorted(pairs):
        for operation in ("m", "s", "d"):
            rows.append(f"{operation} {parts(a)} {parts(b)}\n")
            if operation == "d":
                expected.append("1 0000000000000001 0000000000000002 0000000000000003 0000000000000004" if b == 0
                                else f"0 {parts(a // b)} {parts(a % b)}")
            else:
                value = a*b if operation == "m" else signed(a)*signed(b)
                valid = 0 <= value <= MASK if operation == "m" else -(2**127) <= value < 2**127
                expected.append(f"0 {parts(value & MASK)}" if valid else "1 " + SENTINEL)
    values |= {rng.getrandbits(128) for _ in range(400)}
    for value in sorted(values):
        rows += [f"u {parts(value)}\n", f"i {parts(value)}\n"]
        expected += [str(value), str(signed(value))]
    return "".join(rows).encode(), "\n".join(expected).encode() + b"\n"


@unittest.skipUnless(platform.system() == "Linux", "native runtime currently requires Linux")
class Runtime128(unittest.TestCase):
    def test_bigint_multiplication_division_and_decimal_output_at_both_optimizations(self):
        compiler = shutil.which(os.environ.get("CC", "cc"))
        if compiler is None:
            self.skipTest("C compiler unavailable")
        inputs, expected = corpus()
        with tempfile.TemporaryDirectory(prefix="oxid-128-runtime-") as directory:
            for optimization in ("O0", "O2"):
                with self.subTest(optimization=optimization):
                    binary = Path(directory) / optimization
                    command = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-"+optimization,
                               str(ROOT / "tests/integer128_runtime.c"), "-o", str(binary)]
                    if os.environ.get("OXID_BYTEBUF_SANITIZE") == "1":
                        command[1:1] = ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
                    built = subprocess.run(command, capture_output=True, timeout=120)
                    self.assertEqual(built.returncode, 0, (built.stdout, built.stderr))
                    result = subprocess.run([str(binary)], cwd=directory, input=inputs, capture_output=True, timeout=60)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stderr, b"")
                    self.assertEqual(result.stdout, expected)
                    for operation in ("u", "i"):
                        read_fd, write_fd = os.pipe()
                        os.close(read_fd)
                        try:
                            failed = subprocess.run([str(binary)], input=f"{operation} {parts(MASK)}\n".encode(), stdout=write_fd, stderr=subprocess.PIPE, timeout=30)
                        finally:
                            os.close(write_fd)
                        self.assertEqual((failed.returncode, failed.stderr), (74, b""))



if __name__ == "__main__":
    unittest.main(verbosity=2)
