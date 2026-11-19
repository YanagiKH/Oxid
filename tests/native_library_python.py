"""Direct native calls only; no compiler or Oxid subprocess fallback exists."""
import concurrent.futures
import ctypes
import hashlib
import pathlib
import sys
import threading

if sys.flags.optimize:
    raise RuntimeError("native ABI assertions require Python without -O/-OO")

library_path = pathlib.Path(sys.argv[1]).resolve(strict=True)
mode = sys.argv[2]
if mode not in ("pricing", "fuel"):
    raise ValueError("expected pricing|fuel mode")
fuel_only = mode == "fuel"
before = hashlib.sha256(library_path.read_bytes()).hexdigest()
library = ctypes.CDLL(str(library_path))
library.oxid_library_abi_version.argtypes = []
library.oxid_library_abi_version.restype = ctypes.c_uint32
call = library.oxid_library_call_v1
call.argtypes = [ctypes.c_int32, ctypes.c_int32, ctypes.c_int32, ctypes.POINTER(ctypes.c_int32)]
call.restype = ctypes.c_int32
assert library.oxid_library_abi_version() == 1


def examples():
    for unit, quantity, divisor, status, result in [
        (1250, 3, 2, 0, 1875),
        (-7, 1, 2, 0, -3),
        (0, 1, 1, 0, 0),
        (-2147483648, 1, 1, 0, -2147483648),
        (2147483647, 1, 1, 0, 2147483647),
        (2147483647, 2, 1, 1, 0),
        (-2147483648, 1, -1, 1, 0),
        (1, 1, 0, 2, 0),
        (42, 1, 1, 0, 42),
    ]:
        expected_status = 3 if fuel_only else status
        output = ctypes.c_int32(123456789)
        assert call(unit, quantity, divisor, ctypes.byref(output)) == expected_status
        assert output.value == (123456789 if expected_status else result)
    assert call(1, 1, 1, None) == 4
    storage = (ctypes.c_ubyte * 16)(*[0xA5] * 16)
    address = ctypes.addressof(storage)
    offset = next(i for i in range(4) if (address + i) % ctypes.alignment(ctypes.c_int32))
    pointer = ctypes.cast(address + offset, ctypes.POINTER(ctypes.c_int32))
    assert call(1, 1, 1, pointer) == 4
    assert bytes(storage) == bytes([0xA5] * 16)


examples()
barrier = threading.Barrier(4)


def worker(_):
    barrier.wait()
    for _ in range(128):
        examples()


with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
    list(executor.map(worker, range(4)))
examples()
assert hashlib.sha256(library_path.read_bytes()).hexdigest() == before
print("native Python calls passed")
