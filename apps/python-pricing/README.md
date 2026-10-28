# Oxid functions in Python

Build a Python wheel containing an Oxid function and a CPython extension:

```sh
python3 -m pip wheel --no-deps --no-index . -w /tmp/oxid-wheels
python3 -m pip install --no-deps --no-index /tmp/oxid-wheels/oxid_pricing-*.whl
python3 -c 'import oxid_pricing; print(oxid_pricing.line_total(1250, 3, 2))'
```

The result is `1875`. Build with CPython 3.11 or newer on Linux x86_64 or ARM64,
Oxid on `PATH` (or `OXID=/path/to/oxid`), LLVM 19.1.7, and the current Python's
C headers. `OXID_LLVM_BIN` selects the LLVM tools directory;
`OXID_PYTHON_CC` can select its Clang executable directly.

The installed wheel needs neither Oxid, LLVM nor the source files. It works
with the Python version, architecture and Linux system libraries for which
it was built. See [Python packages](../../docs/PYTHON_PACKAGES.md) for the
configuration and error contract.
