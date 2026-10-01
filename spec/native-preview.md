# Experimental LLVM scalar native preview

This is a narrow `typed-preview` backend, not completion of M2/M3, a stable ABI,
Rust compatibility, ownership checking, a sandbox, or self-hosting. Default and
explicit `legacy-0.9` compilation remain OXBC serialized-AST generation.

## Command

```sh
oxid compile input.ox --edition typed-preview --backend llvm --output ./program
oxid compile input.ox --edition=typed-preview --backend=llvm \
  --target=x86_64-unknown-linux-gnu --output=./program --message-format=json
./program
```

The compile command never runs its output. It requires one source, an explicit
`--backend llvm`, and `--output` naming a new file in an existing directory.
Directory syntax (a trailing `/`, `/.`, or `/..`, including `.` and `..`) is
rejected before tools run; it is never normalized into a different output file.
Backend, target and output options follow the command, may surround the source,
and accept `--name=value` or `--name value`. Each occurs at most once. A value
starting with `-` must be written with a directory prefix, e.g. `./-program`.
Global edition/format options retain their existing placement and separator
rules. `--` ends option recognition; later words are source operands. No backend,
profile, optimization, CPU feature, linker-argument, library, or cross-compilation
fallback exists. Unknown options/values fail with E0001 before source reading.

The only build host is Linux x86_64 and the only target is
`x86_64-unknown-linux-gnu` (also the default). Generated files are ELF PIE
executables, not OXBC. They depend on the host-compatible libc and ELF loader;
they do not depend on Oxid, Rust, LLVM shared libraries, Python, or source files.
This is not a portable-binary or minimum-glibc-version promise.

## Admission before tools or output

Compilation first runs the complete source → typed HIR → verified OIR path,
including unused functions and unchosen branches. Only the immutable verified
witness can enter native admission. A declared zero-argument scalar `main` is
required, carried by its resolved DefId rather than reconstructed from spans.

Supported operations are exact bool/unit/i32 constants, immutable copies, checked
i32 addition/subtraction/multiplication, same-type i32/bool equality and i32-only
signed ordering comparisons, bool negation and explicit short-circuit bool merges,
initialized mutable scalar places with explicit load/store,
explicit direct nonrecursive calls,
Branch, Goto and Return. Native compilation rejects all other OIR operations.
Arithmetic follows the ordered, checked-overflow semantics in
[RFC 0006](../rfcs/0006-checked-i32-arithmetic.md) and
[RFC 0008](../rfcs/0008-native-checked-i32.md). Comparisons follow
[RFC 0009](../rfcs/0009-scalar-comparisons.md). Boolean logic follows
[RFC 0010](../rfcs/0010-boolean-logical-operators.md), and mutable scalar storage
follows [RFC 0011](../rfcs/0011-mutable-scalar-locals.md). Ordinary bool-condition while and shared runtime fuel follow [RFC 0012](../rfcs/0012-while-runtime-fuel.md). There are no source I/O operations,
pointers, containers, break/continue, indirect calls, modules or implicit legacy adapters
in this subset.

The entire call graph must be acyclic, including dead declarations and calls in
constant-false branches and skipped logical RHSs. Iterative leaf-first traversal rejects recursive graphs.
The following inclusive bounds are additional native restrictions:

| Resource | Maximum |
| --- | ---: |
| Functions in the file | 256 |
| Parameters per function | 64 |
| Combined slots per function, including parameters, values and places | 256 |
| Aggregate locals / maximum live slots | 8,192 |
| Aggregate basic blocks | 4,096 |
| Call depth, including main | 32 |
| Conservative reference fuel upper bound for transitively acyclic functions | 100,000 |
| Shared runtime fuel in guarded modules | 1,000,000 |
| Guarded-module diagnostic data | 16 MiB |
| Guarded-module emitted LLVM text | 64 MiB |

All declarations must meet structural/depth/live-slot bounds. For a function F
whose CFG and transitive callees are acyclic, compute
`C(F) = slots(F) + sum(statements(block) + merge_count(block) + 1 for all blocks) +
sum(argument_count(call) + C(callee) for all calls)` in callee-first order.
Require `1 + C(F) <= 100,000`. Calls at distinct sites are counted separately;
there is no memoization discount. For those acyclic functions, summing both arms overestimates every executed path. It also counts
callee allocation and every reference execution operation. An arithmetic
or comparison assignment counts once; operand evaluation has its own
assignments/calls. Thus `return 1 + 2;`, `return 1 < 2;` and
`return true == false;` each have an inclusive bound of eight fuel. This conservative
admission can reject programs that run successfully in the reference interpreter.
A bool negation or join merge costs one. `return !true;` has bound 6; ungrouped
`return a && b;` / `return a || b;` with literal operands have bound 10, counting
both paths even if the reference runtime short-circuits in 8. Whole-function
slot allocation still includes skipped RHS temporaries and unchosen mutable
places. `slots(F)` is the sum of the value and place declaration tables; each
Initialize/Store/Load costs one statement. `let mut x = 1; x = 2; return x;` has
four slots and an inclusive bound of eleven fuel.

Depth is `1 + max(callee_depth)` and live slots are
`slots(F) + max(callee_live_slots)`, with zero for an empty maximum. These bounds
are below the runner's 1,024-frame/200,000-slot limits. Transitively acyclic
functions retain their static fuel restriction; cyclic work instead uses the
shared dynamic budget described below. They do
not equate OS stack bytes to slots or bound compiler time, tool execution,
allocation failure, output blocking, or host resource limits. A 32-frame,
64-parameter representative stress case is tested with a 1 MiB process stack;
this is evidence, not an all-environments stack-safety theorem.

## Cyclic modules and shared runtime fuel

A cyclic intraprocedural CFG or cyclic transitive callee has unknown static cost.
It never uses a one-pass sum as an execution bound. If any function is cyclic,
including an unused function, all emitted functions share one private i64 fuel
counter through a hidden pointer. Root allocation costs 1+slots; statements,
merges and noncall terminators cost 1; calls cost 1+arguments+callee slots, before
callee execution. Each executed iteration consumes fuel. Failure before the next
operation is E0601 at that exact reference origin. Empty infinite loops therefore
fail deterministically. Source checking still requires an explicit return after
while; native recursion is still rejected. An always-returning loop body may
produce an acyclic CFG and retain the unguarded emitter.

Guarded diagnostic data is deduplicated by failure kind/origin and streamed into
a checked byte counter before allocation; total data must be <=16 MiB. Exact LLVM
text is counted before allocating its buffer and must be <=64 MiB. E0700 rejects
excess before tools/output effects. These are guarded-only preview representation
bounds; old acyclic admission is unchanged. They bound neither machine-code size
nor tool runtime. Diagnostic text has escaped paths and line/column, no excerpts.

## Lowering and private ABI

Native ABI version 1 is private and provisional. LLVM values are `i1` for bool,
`i8` containing zero for unit, and exact `i32`. Definitions use the verifier's
single-assignment local IDs as LLVM SSA names. Each mutable place uses a private
typed alloca in the LLVM entry prologue, before branching to the possibly nonzero
OIR entry. Initialize/Store emit typed stores; Load produces a fresh SSA snapshot.
The verifier establishes initialization dominance before emission. No source
pointers, aliases or mutation of earlier copied values are exposed.
Copies/constants use bitwise OR
with zero, with no numeric conversion, `undef`, `poison`, `nsw`, or `nuw`.
Checked arithmetic uses LLVM `sadd`, `ssub` and `smul` signed-overflow intrinsics.
Each operation branches on the overflow bit, extracts its i32 result only on the
success path, and calls a noreturn diagnostic adapter on failure. Operand, call,
statement and first-error order remain unchanged. Discarded arithmetic executes;
unchosen branches do not. No hardware trap or unchecked/wrapping operation is a
substitute for the overflow branch. Backend lowering adds two blocks per checked
assignment; the 4,096-block ceiling measures original OIR blocks. At most 8,192
arithmetic assignments can fit the aggregate local ceiling.
Comparisons lower to i1 results with `icmp eq`/`ne` over i32 or i1, or signed
`icmp slt`/`sle`/`sgt`/`sge` over i32 only. The verifier rejects unit, mixed types
and bool ordering. These instructions add no error path or runtime adapter;
operand arithmetic can still fail before comparison.
Bool negation uses `xor i1`; short-circuit values use branches and `phi i1` at
block entry. Phi input labels identify the actual predecessor LLVM exit block,
including checked-arithmetic success blocks and, in guarded modules, the final
terminator-guard success block. Phi is first; its pure selection precedes the
merge fuel check, but no later operation or effect may precede that check.
No eager and/or/select replaces the source's conditional RHS execution.
Branches, direct calls and returns retain their OIR structure. Functions are
`internal` and `noinline`; this preview uses `-O0` and no LTO or fast-math.

Symbols are deterministic numeric `__oxid_fn_<DefId>` names. Source identifiers
and comments never enter LLVM text. Overflow diagnostics contain the compile-time
source path, pre-rendered with the reference human renderer and encoded entirely
as hexadecimal LLVM constant bytes. Paths cannot supply IR syntax or symbols.
Names such as `write` or `printf` cannot collide with runtime/library symbols.
The C-compatible entry shim returns an output status, never the scalar value.
It widens bool to i32 when calling the C output adapter. Unit has a no-argument
printer. There is no general C/FFI ABI promise for user functions.

## Toolchain and artifact boundary

The trusted tools are Clang, opt and LLD, each exactly version 19.1.7. Set
`OXID_LLVM_BIN` to their directory containing `clang`, `opt`, and `ld.lld`, or
provide `clang-19`, `opt-19`, and `ld.lld-19` on PATH. The compiler checks all
three version reports on each invocation. The qualified Debian package build is
`1:19.1.7-3+b1`; see the validation report for provenance. A working host C
header/startup/libc development environment is also required. Building the Oxid
compiler itself still uses Rust/Cargo and C/C++, as before.

Version checks are compatibility gates, not authentication of arbitrary tools.
The user-selected tools, installation, PATH and C development environment are
trusted. Oxid does not download packages, run installation commands, accept raw
compiler flags, or use a shell. Clang default configuration is disabled and
`CCC_OVERRIDE_OPTIONS` is removed. Tools are invoked with separate argv items.

After source/admission/output preflight and version checks, a new owner-only
scratch directory is atomically reserved beside the destination. opt explicitly
verifies the emitted LLVM module; Clang generates its object and separately
compiles the embedded small C printer; Clang invokes the pinned LLD to link PIE
with non-executable stack, RELRO and immediate relocation. Generated executable
publication uses a same-filesystem hard link: it fails if any destination exists,
including a symlink or a file created after preflight. No existing file is ever
replaced. Source/output equality therefore fails safely. Normal success and
failure remove scratch files. Abrupt process death, untrusted concurrent changes
to the output directory, or cleanup I/O errors are outside the cleanup guarantee.
Filesystems without hard-link support fail cleanly; there is no unsafe fallback.

## Output and diagnostics

Successful executables write exactly `true\n`, `false\n`, `()\n`, or signed
decimal i32 plus newline, and exit 0. The printer handles i32 MIN by widening
before negation. It handles partial writes and retries EINTR. A failed write,
zero-progress write, or signal-setup failure returns 74 (EX_IOERR); SIGPIPE is
ignored so a closed pipe also returns 74. An output failure can leave a partial
line.

Fuel exhaustion writes exactly the reference human E0601/oir-run diagnostic at
the next operation, with empty stdout and exit 1. Fuel is checked before arithmetic
or storage, preserving first-error order. An executed checked-i32 overflow writes exactly the reference human E0604/oir-run
diagnostic to stderr, including the operator's source line and Unicode-scalar
column, leaves stdout empty and exits 1. It stops at the first error, including in
discarded expressions or arguments. The embedded source path is the argument
supplied to `compile`, with control characters escaped by the reference renderer;
it remains fixed if source files are moved/deleted or the executable runs elsewhere.
Paths may be visible in executable constant data. There is no runtime source read.
The private adapter receives length-delimited bytes, writes them using the same
partial-write/EINTR/SIGPIPE handling, and calls `_exit`. Diagnostic output failure
exits 74 and can leave partial stderr. There is no native JSON output mode or
program input/arguments contract.

Compiler success is exit 0. Text mode prints a compile summary only, never the
program result. JSON mode emits one `compile-summary` with schema_version 1,
edition `typed-preview`, success, errors and output (path on success, null on
failure). Ordinary errors are exit 1. E0700/native-admission describes entry,
unsupported-operation, call-graph and bound failures; E0701/native-toolchain
covers host, tools and artifact I/O. Existing frontend/OIR diagnostic codes and
exit 2 internal-invariant behavior are unchanged. Valid global options selecting
compile use compile-summary for later errors; invalid global options retain the
pre-existing check-summary. Diagnostics escape source/tool-controlled text.

See [RFC 0007](../rfcs/0007-llvm-scalar-native.md),
[RFC 0008](../rfcs/0008-native-checked-i32.md),
[RFC 0009](../rfcs/0009-scalar-comparisons.md), the
[scalar predecessor evidence](../docs/architecture/native-preview-validation.md),
[checked-arithmetic evidence](../docs/architecture/native-arithmetic-validation.md),
[comparison evidence](../docs/architecture/scalar-comparison-validation.md),
[boolean logic evidence](../docs/architecture/boolean-logic-validation.md),
and [mutable-local evidence](../docs/architecture/mutable-locals-validation.md).

[While validation](../docs/architecture/while-validation.md) covers the cyclic/guarded extension and its qualification limits.
