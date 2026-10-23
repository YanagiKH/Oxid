# Native build profiles

Status: experimental. Native project builds support Linux x86_64 with LLVM,
Clang and LLD 19.1.7.

## Choose a profile

```sh
oxid build --profile dev
oxid build --profile release --incremental --reproducible
```

| Profile | Program compilation | Runtime C compilation |
|---|---|---|
| `dev` | `-O0` | `-O0` |
| `release` | `-O2` | `-O2` |
| `bounded` | Alias for `dev` | Alias for `dev` |

The flag accepts a separate value or `--profile=VALUE` and may appear once.
Unknown, missing, empty or repeated values are errors. This selection applies
to a source-less native project `build`; explicit-file compilation keeps its
existing command contract.

Omission uses the selected package's `[build] profile`, then defaults to `dev`.
A command-line choice overrides the package setting. For a workspace, resolve
every selected member before native tools run. A dependency's profile does not
replace the importing binary's setting. Invalid manifest settings remain errors
even when overridden, and manifest edits remain lock-input changes.

## Execution and output

Optimization is independent of language edition, result/process entry,
reproducible diagnostic paths, object reuse and execution policy. Both profiles
retain checked overflow, guarded division, bounds checks, ownership behavior,
ordered effects, diagnostics and execution limits. Neither profile adds DWARF
information or changes interpreter execution.

LLVM verification, runtime compilation and linking run on every successful
build. Only a matching program object may be reused. The actual optimization
arguments participate in its cache key: `dev` and `bounded` share settings,
while `release` uses a different object. Reproducibility compares identical
settings and toolchains; it does not require O0 and O2 executables to match.

All profiles publish to `.oxid/bin/NAME`. A failed build preserves the previous
executable. Workspace builds keep their existing per-member publication behavior.

## Verification

`tests/native_profiles.rs` exercises public flag and manifest admission, member
precedence, tool arguments, cache selection and executable behavior. Real LLVM
cases cover both optimization levels, checked failures, ordered effects,
ownership, process output and resource exhaustion. Rust compiler debug/release
builds are separate from these Oxid native profiles.

`scripts/verify_native_profiles.py` uses the byte-count application's independent
byte/newline oracle across standalone and workspace layouts. It retains raw
executables and program objects from clean, cached and relocated builds, then
runs deployed executables after removing sources. Receipts identify commands,
inputs, compilers, tools, environment and artifacts. These checks establish no
performance ratio or reproducibility across different SDKs and toolchains.
