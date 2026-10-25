# Native scalar variable information

Status: experimental

`oxid build --debug-info` describes scalar function parameters and local
variables in DWARF 5 on Linux x86_64 and ARM64. In GDB, use `info args`,
`info locals`, `print name` and `ptype name` after initialization. The same
manifest setting, profiles, execution policies and workspace selection described
in [native debug information](0061-native-debug-information.md) apply.

## Values and scopes

The supported types are `bool`, `char`, `i8`, `u8`, `i16`, `u16`, `i32`, `u32`,
`i64`, `u64`, `isize`, `usize`, `i128`, `u128`, `f32` and `f64`. Signedness and
width describe the source value, including when its existing storage slot is
wider. Native targets currently use 64-bit pointer-sized integers.

Metadata uses the checked session's retained source declarations and parsed
lexical blocks. Parameters retain their original argument positions. Local
bindings include mutable variables, range indices, scalar record patterns and
scalar enum payload patterns. Leaving a block hides its variables. Compiler
temporaries and loop cursors are omitted; only exact source declaration spans
can supply a displayed name.

Unit values, references and owned aggregates have no variable descriptions in
this increment. Inspect variables after their initialization. A debugger may
show a value as `<optimized out>` when no valid machine location remains,
including SSA values in `dev`. Release optimization can also fold or remove
source code. This metadata does not promise that a removed value can be read.

## Emission and failure behavior

Both verified LLVM emitters add private variable markers only when debug
information is enabled. SSA values use `llvm.dbg.value`; existing stack storage
uses `llvm.dbg.declare`. The ownership emitter describes its existing scalar
allocation plus a byte offset, so a temporary pointer register's lifetime cannot
invalidate the variable's storage location. No executable allocation, load,
store, control-flow instruction, ownership operation or ABI is added.

Lexical scope and binding inventories use fallible allocations with a checked
128 MiB aggregate scratch limit per function. Source position lookup uses sorted
indices and ancestor links rather than scanning earlier siblings. The inherited
two-pass renderer counts the complete module before allocation and retains its
128 MiB output limit. Invalid markers, overflow or allocation failures report
`E0700`; LLVM verifies the module before compilation. Failure preserves the
previous managed executable.

Non-debug emission remains unchanged. Debug metadata participates in object
cache identity and the existing fixed-toolchain reproducibility contract.

## Verification

`tests/native_debug_info.rs` invokes LLVM, DWARF verification and GDB on actual
executables. It checks parameters, assignment, nested scopes, range indices,
hidden cursors, all sixteen scalar types, scalar destructuring and enum payloads.
Both bounded and hosted execution policies are covered. Native output and DWARF
verification also run with release optimization. Existing tests cover source
breakpoints, call stacks, cache reuse, relocation, workspace overrides and
source-free execution.

The separate retained compatibility gate authenticates all compiler inputs and
prior source policies and runs the unchanged scalar/record conformance and
resource assertions. Those results cover their original default-emission scope;
the real debugger checks qualify the newly enabled metadata.
