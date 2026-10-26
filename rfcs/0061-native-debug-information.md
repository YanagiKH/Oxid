# Native debug information

`oxid build --debug-info` includes DWARF 5 function names and source line tables
in native executables on Linux x86_64 and ARM64. It uses the same checked project
and native code as an ordinary build. It does not change execution policy,
arithmetic, ownership, resource cleanup or optimization.

## Configuration

```toml
[build]
profile = "dev"
debug-info = true
reproducible = true
```

The default is `false`. The setting accepts one unquoted boolean; duplicate or
invalid values fail before building. `--debug-info` and `--no-debug-info` override
each selected workspace member's setting and may be specified only once. These
are project build options. Dependencies do not override application settings.

## Debugger behavior

With source files available, GDB can stop by function name or file and line,
step through code and show source positions in the call stack. Runtime builtins
and C helpers do not receive invented Oxid source declarations. Source line
positions refer to the files retained by the checked compiler session.

Line information is independent of `dev` and `release` optimization. Optimizers
can combine or remove source lines and functions; breakpoints in removed code
may have no executable location. [Scalar variable information](0064-native-debug-variables.md)
adds parameter types and local variables. Reference and aggregate layout
descriptions remain unavailable.

Original-path builds retain source paths. Reproducible builds use the existing
project-relative source layout and `.` as the debug compilation directory.
Moving an unchanged relative layout preserves executable bytes with the same
compiler, LLVM/SDK, configuration and environment. The executable runs without
sources or a debugger; source text display requires the corresponding files.

## Build and failure behavior

Debug metadata is derived only after source checking and verified native lowering.
The metadata pass preserves LLVM instructions and uses escaped strings for source
names and paths. It counts output before allocating, limits the marked input to
the existing native IR budget and the final debug module to 128 MiB, and rejects
overflow or allocation failure with `E0700`. Line lookup uses the retained line
index without repeatedly scanning source prefixes for columns.

LLVM verifies the resulting module before object compilation. Managed builds
publish only after successful linking and preserve the previous executable on
failure. Object-cache identity includes the complete LLVM module, so changing
debug information cannot reuse a nonmatching program object. Clean, cached and
relocated builds keep the same output within the reproducibility contract.

## Verification

`tests/native_debug_info.rs` uses real LLVM tools and GDB to check source
breakpoints, call stacks, scalar and owned code, generic enum switches,
development/release optimization, bounded/hosted execution, cache reuse,
relocation, special path characters and source-free execution. Ordinary tests
also check invalid settings and preservation of previous outputs. The native
CI jobs include GDB and enable `SYS_PTRACE` in their isolated job containers.

The retained compatibility gate keeps prior compiler inputs and conformance
assertions authenticated. Those checks cover default code generation; the
debugger tests separately exercise enabled debug information. Platform results
apply only where actually executed.
