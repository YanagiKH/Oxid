# Path dependency acceptance example (R1, experimental)

An independent three-package local path-dependency project:
`app -> library -> shared`. The library exposes scalar functions plus an
opaque record (`Packet`, private field) and enum (`Signal`) public API from
its `model` child module; the shared package is only a transitive dependency
of the application. Nothing is downloaded and no lockfile is written.

```text
path_dependency_acceptance/
  app/        name = "pathdep-accept-app", entry = "src/main.ox",
              [dependencies] lib = "../library"; root main + 3 test_ cases
  library/    entry = "src/lib.ox", [dependencies] shared = "../shared";
              pub mod model; with its own main/test_ that are never selected
  shared/     entry = "src/lib.ox"; base() = 20 via its own crate::seed()
```

## Usage

From `examples/projects/path_dependency_acceptance`:

```sh
oxid check --manifest-path app/oxid.toml
oxid run --manifest-path app/oxid.toml
# 46

oxid test --manifest-path app/oxid.toml
# PASS test_packet
# PASS test_signal
# PASS test_total
# test result: 3 passed; 0 failed; 3 total

oxid build --manifest-path app/oxid.toml
app/.oxid/typed-preview/result/pathdep-accept-app
# 46

oxid run --manifest-path app/oxid.toml --engine native
# 46

oxid test --manifest-path app/oxid.toml --engine native
# (same three PASS lines and total)
```

Native commands require Linux x86_64 with LLVM 19.1.7; point
`OXID_LLVM_BIN` at its bin directory (for example `/usr/lib/llvm-19/bin`).
Changing the shared `seed` from 20 to 21 makes the application return 48
and fails `test_packet`/`test_total` on the next invocation; each command
loads a fresh session. Dependency `main`/`test_` functions are ordinary
library functions: they never become the application entry or suite cases.
