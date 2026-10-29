# Use Oxid functions as a Python package

An Oxid function can be installed as a normal Python wheel. The package imports
a CPython extension, which calls the compiled Oxid function directly.

From the repository root:

```sh
python3 -m pip wheel --no-deps --no-index ./apps/python-pricing -w /tmp/oxid-wheels
python3 -m pip install --no-deps --no-index /tmp/oxid-wheels/oxid_pricing-*.whl
python3 -c 'import oxid_pricing; print(oxid_pricing.line_total(1250, 3, 2))'
```

This prints `1875`. Build with a normal GIL-enabled CPython 3.11 or newer on
little-endian glibc Linux x86_64 or ARM64, its C headers, Oxid and LLVM 19.1.7.
`OXID` selects the Oxid executable; `OXID_LLVM_BIN` selects the LLVM directory.
`OXID_PYTHON_CC` selects a single Clang 19.1.7 executable. Shell command strings
and environment compiler flags are not accepted as build options.

The resulting wheel is specific to the actual Python version and architecture,
such as `cp312-cp312-linux_x86_64`. It depends on that build host's Linux C
library and SDK. It makes no `abi3`, manylinux, musllinux, Windows, macOS,
free-threaded or cross-distribution compatibility promise. Runtime installation
needs no Oxid compiler, LLVM, source files or build cache.

## Configure a package

Copy the example's `build_backend` directory and `pyproject.toml` into a new
Python project. The backend stays inside that project so ordinary isolated
PEP 517 frontends can load it without downloading build dependencies.

```toml
[build-system]
requires = []
build-backend = "oxid_backend"
backend-path = ["build_backend"]

[project]
name = "oxid-pricing"
version = "0.1.0"
description = "Call an Oxid pricing function from Python"
requires-python = ">=3.11"

[tool.oxid]
module = "oxid_pricing"
source = "src/pricing.ox"
sources = ["src/pricing.ox"]
export = "crate::line_total"
profile = "release"
fuel = 1000000
```

The distribution name uses lowercase letters, decimal digits and single
hyphens. Versions have two through four decimal release components, without
leading zeroes except `0`. Dynamic metadata, dependencies and unknown settings
are rejected. The module and exported function must be ASCII Python identifiers;
the module cannot start with `_`. `export` selects one root `pub` function.

List every Oxid source in `sources`, including the entry file and its local
modules. Paths use `/`, remain inside the entry's parent directory and cannot
contain `.` or `..` components. Links, duplicate names, case aliases and
undeclared neighboring `.ox` files are rejected. No source is guessed or read
from outside that manifest. The compiler must report exactly the same source
identities and hashes.

The function takes zero through 64 `i32` arguments and returns `i32`. Helpers
may use the scalar operations supported by
[native libraries](NATIVE_LIBRARIES.md). Python objects, references, callbacks,
owned values, arrays and I/O do not cross this interface. `profile` is `dev`
(O0) or `release` (O2); `fuel` is an integer from `0` through `1000000`.

## Python calls and errors

```python
from oxid_pricing import line_total

assert line_total(1250, 3, 2) == 1875
```

Each call requires the exact positional argument count. Arguments must be
Python integers in the `i32` range; booleans are rejected. Keyword arguments
are not supported. Results are ordinary Python integers.

| Condition | Python exception |
|---|---|
| Wrong argument count, keyword arguments, non-integers or booleans | `TypeError` |
| Input outside `-2147483648..2147483647` or native integer overflow | `OverflowError` |
| Division or remainder by zero | `ZeroDivisionError` |
| Execution allowance exhausted | `RuntimeError` |
| Invalid internal output storage or unknown native status | `SystemError` |
| Missing private library, missing symbols or unsupported ABI | `ImportError` |

The extension releases the GIL during native computation. Each invocation has
its own execution allowance. The module keeps its native library alive until
every active call has returned; separately installed packages have separate
library handles.

## Build artifacts

A wheel includes the extension, a private digest-named copy of the original
Oxid shared library, its matching C header and metadata, and build provenance.
The extension opens that adjacent library by its absolute installed path and
looks up symbols through its own handle. It does not search for an Oxid library
on `PATH` or `LD_LIBRARY_PATH`.

The backend checks the compiler identity, source closure, exact scalar header,
ABI metadata, target and output hashes before publishing the wheel. Wheel
`RECORD` entries describe its installed bytes. These checks do not authenticate
an untrusted compiler or prevent later modification of an installed wheel;
use a trusted compiler, Python, C toolchain and package source.

Standard PEP 517 metadata, wheel and sdist hooks are provided. Preparing metadata
runs no compiler or native code. An sdist contains the backend, template,
configuration and declared sources; building its wheel still needs the build
tools. Existing wheel or sdist outputs are never overwritten.

Build host limits are independent of the language's resource limits: at most
256 input files, 1 MiB per input and 8 MiB in total; native metadata is limited
to 1 MiB, each shared object to 32 MiB, and a wheel to 80 MiB. Tool output is
limited to 64 KiB per stream, with 180 seconds for Oxid and 60 seconds for the
extension compiler. These bounds can reject a project without publishing an
artifact.
