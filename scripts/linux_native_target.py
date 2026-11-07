"""Host ABI expectations for current Linux native application checks."""
from dataclasses import dataclass
import platform
import struct
import sys


@dataclass(frozen=True)
class LinuxTarget:
    triple: str
    architecture: str
    elf_machine: int
    readelf_machine: bytes


X86_64 = LinuxTarget("x86_64-unknown-linux-gnu", "x86_64", 62,
                     b"Advanced Micro Devices X86-64")
ARM64 = LinuxTarget("aarch64-unknown-linux-gnu", "aarch64", 183, b"AArch64")


def select(system, machine, byte_order, pointer_bytes):
    if system != "Linux" or byte_order != "little" or pointer_bytes != 8:
        return None
    if machine in ("x86_64", "AMD64"):
        return X86_64
    if machine in ("aarch64", "arm64"):
        return ARM64
    return None


def host():
    return select(platform.system(), platform.machine(), sys.byteorder,
                  struct.calcsize("P"))


def required_host():
    target = host()
    if target is None:
        raise ValueError("requires Linux x86_64 or ARM64 with 64-bit little-endian storage")
    return target


def elf_identity(kind=3):
    """The ELF type and machine bytes for this host, never a foreign target."""
    return kind.to_bytes(2, "little") + required_host().elf_machine.to_bytes(2, "little")
