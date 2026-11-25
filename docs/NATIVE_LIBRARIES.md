# Call Oxid from C, Rust and Python

Oxid can build a shared library that runs inside another program. This
command supports x86_64 and ARM64 on Linux and macOS 11 or newer, and Windows
MSVC x64, with LLVM 19.1.7. Exported functions take `i32` arguments and return
`i32`. They report errors without terminating the caller.

Save this as `pricing.ox`:

```rust
pub fn line_total(price: i32, quantity: i32, divisor: i32) -> i32 {
    return price * quantity / divisor;
}
```

Build a new version directory:

```sh
mkdir -p dist
oxid --edition typed-preview library --export crate::line_total \
  --profile release --output dist/pricing-v1 pricing.ox
```

The directory contains `liboxid_library.so` on Linux, `liboxid_library.dylib`
on macOS or `oxid_library.dll` with `oxid_library.lib` on Windows. Every build
includes `oxid_library.h` and `metadata.json`. Keep the matching files together.
Use a fresh directory for each version. The compiled library runs independently
of Oxid and its source files.

## C

Save as `client.c`:

```c
#include "oxid_library.h"
#include <inttypes.h>
#include <stdio.h>

int main(void) {
    int32_t total = 0;
    int32_t status = oxid_library_call_v1(1250, 3, 2, &total);
    if (status != OXID_LIBRARY_OK) return 1;
    printf("%" PRId32 "\n", total);
    return 0;
}
```

```sh
case "$(uname -s)" in
  Darwin) library_origin='@loader_path' ;;
  Linux) library_origin='$ORIGIN' ;;
esac
cc -I dist/pricing-v1 client.c -L dist/pricing-v1 -loxid_library \
  "-Wl,-rpath,$library_origin/dist/pricing-v1" -o c-client
./c-client
```

On Windows, run the build and client commands in the Visual Studio x64 Native
Tools Command Prompt. Link the `.lib` file and place the client beside the DLL.

```bat
clang -I dist\pricing-v1 client.c dist\pricing-v1\oxid_library.lib -o dist\pricing-v1\c-client.exe
dist\pricing-v1\c-client.exe
```

## Rust

Save as `client.rs`:

```rust
#[link(name = "oxid_library")]
unsafe extern "C" {
    fn oxid_library_call_v1(price: i32, quantity: i32, divisor: i32, out: *mut i32) -> i32;
}

fn main() {
    let mut total = 0;
    let status = unsafe { oxid_library_call_v1(1250, 3, 2, &mut total) };
    assert_eq!(status, 0);
    println!("{total}");
}
```

```sh
rustc --edition=2021 client.rs -L native=dist/pricing-v1 \
  -C "link-arg=-Wl,-rpath,$library_origin/dist/pricing-v1" -o rust-client
./rust-client
```

On Windows, put the Rust executable beside the DLL as well.

```bat
rustc --edition=2021 client.rs -L native=dist\pricing-v1 -o dist\pricing-v1\rust-client.exe
dist\pricing-v1\rust-client.exe
```

## Python

Save as `client.py` and run `python3 client.py`:

```python
import ctypes
import sys
from pathlib import Path

name = {"win32": "oxid_library.dll", "darwin": "liboxid_library.dylib"}.get(
    sys.platform, "liboxid_library.so"
)
library = ctypes.CDLL(str((Path("dist/pricing-v1") / name).resolve()))
call = library.oxid_library_call_v1
call.argtypes = [ctypes.c_int32, ctypes.c_int32, ctypes.c_int32,
                 ctypes.POINTER(ctypes.c_int32)]
call.restype = ctypes.c_int32
total = ctypes.c_int32()
status = call(1250, 3, 2, ctypes.byref(total))
if status != 0:
    raise RuntimeError(f"Oxid call failed with status {status}")
print(total.value)
```

All three clients print `1875` and call the same shared-library bytes.

## Errors and limits

The generated header matches the selected function's exact argument list.
`oxid_library_abi_version()` returns `1`; keep the matching header and metadata
when loading a library with a different source signature.

| Status | Meaning |
|---|---|
| `0` | Success; the result was written. |
| `1` | Integer overflow. |
| `2` | Division or remainder by zero. |
| `3` | Execution allowance exhausted. |
| `4` | Null or misaligned output pointer. |

Errors leave the output unchanged. The caller supplies live, writable, aligned
`int32_t` storage. Each invocation has its own allowance, shared with its helper
calls; `--fuel` selects `0` through `1000000`, with `1000000` as the default.
`dev` uses O0 and `release` uses O2; both retain these checks.

Helpers can use `bool`, `i32`, `()`, conditions, loops and direct calls, including
declared source modules. References, owned values, I/O, recursion and callbacks
are not supported by this interface. For an installable CPython extension using
this bounded scalar ABI, see [Python packages](PYTHON_PACKAGES.md). See the
[ABI contract](../rfcs/0074-scalar-native-libraries.md) for publication and limits.
