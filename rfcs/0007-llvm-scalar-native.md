# RFC 0007: bounded LLVM scalar native preview

Status: experimental implementation for review. The bounded first-backend
direction is approved; this document does not claim a stable ABI, independent
review sign-off, or completion of a roadmap milestone. Owner/reviewer: unassigned.

## Decision and motivation

Add explicit `compile --edition typed-preview --backend llvm --output <new-file>`
for a conservatively bounded, nonrecursive bool/unit/i32 subset. Keep checking
and the broader bounded reference interpreter independent. Unsupported programs
fail explicitly; no fallback to reference or legacy execution is permitted.
LLVM is an optional compile-time dependency, not a generated-program dependency.

The complete command, ABI, admission cost recurrence, caps, diagnostics,
output behavior and trust model are specified in
[the native preview contract](../spec/native-preview.md).

## Alternatives

- Full recursive native execution would need equivalent fuel/frame/slot accounting
  and independent stack management. It is deferred rather than silently weakening
  the reference contract.
- Evaluating source at compile time and emitting its result would skip executable
  function/branch lowering. This backend instead lowers verified OIR to real LLVM
  functions, basic blocks, calls and returns.
- In-process LLVM bindings introduce build/link/version coupling into the Oxid
  host binary. This first version uses pinned external tools with explicit argv.
- C transpilation would delegate too much scalar lowering to a different language.
  Only the small stdout adapter is C; Oxid code lowers directly to LLVM IR.
- Native arithmetic is deferred until checked-overflow lowering has its own
  differential evidence. Plain wrapping add or poison-producing flags are not
  substitutes for the reference interpreter's arithmetic contract.

## Scope, costs and compatibility

Only Linux x86_64 host/target is supported. ELF PIE depends on libc and its loader.
Rust still builds the compiler; users compiling this subset need the disclosed
LLVM/C development tools. No runtime, legacy CLI default, OXBC version, stable
language edition or manifest field changes. Existing compile-error JSON gains a
compile-summary after valid global typed compile selection.

Whole-file call-graph admission is O(functions + call sites); cost/depth/live-slot
bounds are computed once per function in reverse topological order. Repeated
runtime calls are counted at every call site. Generated code contains each
function once; no recursive source expansion or inlining is performed. Native
ceilings deliberately reject some legal reference programs. Only a small scalar
source subset is executable; this does not establish ownership, memory-model,
FFI, production safety, general termination, performance superiority or self-hosting.

## Acceptance and open boundaries

Tests must cover real LLVM verification/object generation/linking, exact MIN/MAX,
bool/unit, forward calls, mixed argument/result types, branches/joins, dead
recursion, resource rejections before tools, shell/IR identifier isolation,
no-clobber publication (including races/symlinks), failure cleanup, wrong/missing
tools, source errors with no effects, and no automatic artifact execution.

Native stdout is compared with both the reference runner and independent Python
expectations. ELF headers/dependencies, output-failure code 74, repeat builds, and
32-frame/64-argument stack stress are checked. Local and future CI evidence are
separate. Tool authentication, cross compilation, portability baseline, stable
ABI/debug info, optimization, checked arithmetic, recursive native execution,
OS isolation and self-hosting remain future work.
