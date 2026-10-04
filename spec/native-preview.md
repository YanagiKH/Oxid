# Experimental LLVM native preview

Source-integration status: experimental. The ownership source subset uses the
production `typed-preview` route. See the
[source qualification ledger](../docs/architecture/owned-source-validation.md)
for the exact source, compiler, target and validation scope. This describes the
current repository, not support in an older released binary.

This is a narrow `typed-preview` backend, not completion of M2/M3, a stable ABI,
Rust compatibility, a general memory-safety guarantee, a sandbox, or self-hosting. Default and
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
including all declared files, unused functions and unchosen branches. Only the immutable verified
witness for the selected module-wide scalar or owned route can enter native
admission. The owned source contract is specified in
[typed preview](typed-preview.md#nominal-owned-structs-and-call-only-borrowing).
Any owned syntax, even an unused declaration, selects owned lowering for the
whole linked project; scalar-only projects retain their existing emitter/admission.
No source or admission failure falls back to scalar, legacy or reference execution. A declared zero-argument scalar `main` is
required, carried by its resolved DefId rather than reconstructed from spans.

The scalar-only route supports exact bool/unit/i32 constants, immutable copies, checked
i32 addition/subtraction/multiplication/division/remainder, same-type i32/bool equality and i32-only
signed ordering comparisons, bool negation and explicit short-circuit bool merges,
initialized mutable scalar places with explicit load/store,
explicit direct nonrecursive calls,
Branch, Goto and Return. Native compilation rejects all other OIR operations.
Arithmetic follows the ordered, checked-overflow semantics in
[RFC 0006](../rfcs/0006-checked-i32-arithmetic.md) and
[RFC 0008](../rfcs/0008-native-checked-i32.md), extended by
[RFC 0018](../rfcs/0018-checked-i32-division.md). Comparisons follow
[RFC 0009](../rfcs/0009-scalar-comparisons.md). Boolean logic follows
[RFC 0010](../rfcs/0010-boolean-logical-operators.md), and mutable scalar storage
follows [RFC 0011](../rfcs/0011-mutable-scalar-locals.md). Ordinary bool-condition while and shared runtime fuel follow [RFC 0012](../rfcs/0012-while-runtime-fuel.md). Unlabeled break/continue follow [RFC 0013](../rfcs/0013-loop-control.md), using existing charged Goto edges. Native guarding follows actual CFG cycles: a break-only while can be acyclic, whereas continue targets its original condition header. The owned route additionally supports nominal structs containing scalars,
records and fixed scalar arrays under [RFC 0020](../rfcs/0020-owned-record-composition.md),
whole-value transfers/replacement, bounded scalar leaf access, owned helper returns and
explicit call-only shared/exclusive loans and reborrows through its sealed
witness. Fixed scalar arrays also support complete construction/transfers,
checked signed indexing and `len()` under [RFC 0016](../rfcs/0016-fixed-scalar-arrays.md).
Their private positive storage includes initialized zero-length sentinels; no
array ABI, element references or heap allocation is exposed. Indexed writes keep
RHS-before-index snapshots, and guarded access charges fuel before bounds.
Invalid executed indexes emit the exact reference E0606/oir-owned-run diagnostic,
including source origin, with empty stdout and exit 1. No element pointer is
formed before the signed bounds check succeeds. There are no source I/O operations, address values, heap containers,
indirect calls, module initialization or implicit legacy adapters in this subset.
Bounded declaration-only modules, direct imports and visibility are resolved before
emission, as specified by [RFC 0015](../rfcs/0015-bounded-typed-projects.md).
Their metadata adds no runtime import, entry call or fuel charge. The required
main is the original root declaration; an imported/child main does not qualify.
The [project validation ledger](../docs/architecture/typed-project-unit4-validation.md)
separates public source qualification from historical private-consumer evidence.

The entire call graph must be acyclic, including dead declarations and calls in
constant-false branches and skipped logical RHSs. Iterative leaf-first traversal rejects recursive graphs.
The following inclusive bounds are the scalar-only native restrictions. Owned
modules retain them and add the expanded-storage restrictions below; the scalar
slot/cost formulas and examples in this section apply to scalar-only modules.

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

## Owned-module storage, costs and private representation

Owned modules retain 256 functions, 64 user parameters, 4,096 OIR blocks, call
depth 32, whole-call-graph recursion rejection, at most 256 scalar slots per
function and 8,192 aggregate/maximum-call-path scalar slots. Every declaration
is included, even if unused or statically skipped. A scalar `main` is required;
owned return values are permitted only in helpers. Native admission additionally
requires:

| Owned resource | Inclusive maximum |
| --- | ---: |
| Scalar slots plus all owner slots per function, `S + O` | 256 |
| Sum of expanded cells X over all functions | 8,192 |
| Maximum call-path sum of X | 8,192 |
| Sum of explicit native arena bytes, including any wrapper fuel cell | 1 MiB |
| Maximum call-path explicit arena bytes, including any wrapper fuel cell | 1 MiB |
| Owned diagnostic data, including acyclic overflow diagnostics | 16 MiB |
| Owned emitted LLVM text, acyclic or guarded | 64 MiB |

For each function let S be scalar locals plus mutable places, A all call argument
descriptors, O all owners, R incoming references, L loans, C calls, and
P the sum of owner widths: `max(1, record_field_count)` for each record and
`max(1, N)` for each fixed array, including empty and unit arrays. B is the aligned
owner arena including parameter, local, temporary, staged-argument and result
storage, with inter-owner padding. On the qualified x86_64 representation:

```text
X = S + A + P + 4O + 8R + 12L + 2C
Dnative = 8(S+A) + 8(R+L) + align4(B)
```

Byte sums/path sums use Dnative and add one 8-byte wrapper fuel cell when any
function has cyclic cost. The cell is added once to each whole-module/path
bound, not once per function. Scalar field layout is declaration order, bool
and unit 1-byte size/alignment, i32 4-byte size/alignment, with checked natural
padding. An empty struct has one private identity byte. Arrays use element
stride 1 for bool/unit and 4 for i32, with positive size
`align_up(max(1, N * stride), alignment)`. Zero-array sentinel bytes and unit
storage are initialized and never exposed as invalid elements. Since B ≤ 4P, Dnative
≤ 8X on this representation; the existing cell ceiling bounds total explicit
native storage by 65,544 bytes including the guarded fuel cell. The separate
1 MiB byte checks remain as defenses against representation changes. These
figures exclude LLVM spills, ABI stack use, machine code, tool memory and RSS.

For a transitively acyclic function, start its conservative cost at X, then
sum every merge, statement and terminator using the owned ledger in
[typed preview](typed-preview.md#owned-verification-execution-and-accounting).
At each Invoke substitute the callee's total cost for the callee-X term already
included in the Invoke charge. Require `1 + cost(F) <= 100,000`. Both conditional
arms and every call site are included. A cyclic CFG or transitive cyclic callee
has unknown static cost; it does not gain a guessed finite bound. If any function
is cyclic, all functions share the 1,000,000-operation guarded budget. Guarding
uses these owned costs, including expanded transfers and normal-edge release,
rather than the scalar-only call/root/return formulas below.

The ownership emitter uses entry-prologue owner byte arenas, i64 scalar/snapshot
cells and pointer cells for incoming references and active loans. Incoming owned
arguments are transferred field-by-field for records and element-by-element for
arrays into independent callee storage; owned results use caller-owned output storage and are transferred before callee
return. Staging backing storage remains allocated until return even after its
logical ownership is consumed. Shared reference pointers may alias. The emitter
adds no `noalias`, `inbounds`, `nonnull`, `sret`, `byval` or lifetime assumptions.
This is a private convention, not a stable source layout or C/FFI ABI.

Bool merges select the predecessor's slot pointer before loading its value, so
an untaken uninitialized slot is not read. Fuel/overflow success paths must
dominate affected stores; an exhausted charge performs no store. Whole source
argument/literal evaluation order and first-error behavior are preserved.
Counted LLVM expansion stops when its byte ceiling is exceeded, before reserving
the final text buffer, and checked count/render parity is required.

Reference/native comparisons, held-out source cases, native artifacts and store
control-flow checks belong to the [source qualification ledger](../docs/architecture/owned-source-validation.md).
The [raw-consumer report](../docs/architecture/owned-consumers-validation.md)
records predecessor fixtures only; its Batch sizes and 1,086-fuel schedule are
not source measurements.

## Cyclic modules and shared runtime fuel

A cyclic intraprocedural CFG or cyclic transitive callee has unknown static cost.
It never uses a one-pass sum as an execution bound. If any function is cyclic,
including an unused function, all emitted functions share one private i64 fuel
counter through a hidden pointer. For scalar-only modules, root allocation costs 1+slots; statements,
merges and noncall terminators cost 1; calls cost 1+arguments+callee slots, before
callee execution. Each executed iteration consumes fuel. Failure before the next
operation is E0601 at that exact reference origin. Empty infinite loops therefore
fail deterministically. Source checking still requires an explicit return after
while; native recursion is still rejected. An always-returning loop body may
produce an acyclic CFG and retain the unguarded emitter.

Guarded diagnostic data is deduplicated by failure kind/origin and streamed into
a checked byte counter before allocation; total data must be <=16 MiB. Exact LLVM
text is counted before allocating its buffer and must be <=64 MiB. E0700 rejects
excess before tools/output effects. For scalar-only modules these are guarded-only preview representation
bounds; old acyclic scalar admission is unchanged. Owned modules apply their
16 MiB/64 MiB representation bounds even when acyclic. They bound neither machine-code size
nor tool runtime. Diagnostic text has escaped paths and line/column, no excerpts.

## Lowering and private ABI

The scalar-only native ABI version 1 is private and provisional. LLVM values are `i1` for bool,
`i8` containing zero for unit, and exact `i32`. Definitions use the verifier's
single-assignment local IDs as LLVM SSA names. Each mutable place uses a private
typed alloca in the LLVM entry prologue, before branching to the possibly nonzero
OIR entry. Initialize/Store emit typed stores; Load produces a fresh SSA snapshot.
The verifier establishes initialization dominance before emission. No source
pointers, aliases or mutation of earlier copied values are exposed.
Copies/constants use bitwise OR
with zero, with no numeric conversion, `undef`, `poison`, `nsw`, or `nuw`.
Checked addition/subtraction/multiplication use LLVM `sadd`, `ssub` and `smul`
signed-overflow intrinsics. Division/remainder use guarded `sdiv`/`srem` only
after excluding zero divisors and the MIN/-1 pair.
Each operation branches on the overflow predicate, obtains its i32 result only on the
success path, and calls a noreturn diagnostic adapter on failure. Operand, call,
statement and first-error order remain unchanged. Discarded arithmetic executes;
unchosen branches do not. No hardware trap or unchecked/wrapping operation is a
substitute for the overflow branch. Backend lowering adds two blocks per checked addition/subtraction/multiplication
assignment and four per division/remainder assignment; the 4,096-block ceiling measures original OIR blocks. At most 8,192
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

Scalar symbols are deterministic numeric `__oxid_fn_<DefId>` names; owned
functions use private numeric `__oxid_owned_fn_<DefId>` symbols. Source identifiers
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
discarded expressions or arguments. Division and remainder first guard zero
(E0607/oir-run) and the MIN/-1 pair (E0604/oir-run); LLVM `sdiv`/`srem`
execute only on the successful path. Both errors use the reference diagnostic
and preserve the same fuel and evaluation order. The embedded source path is the argument
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

[Loop-control validation](../docs/architecture/loop-control-validation.md) covers exact transfer fuel and real native execution.


[RFC 0014](../rfcs/0014-owned-structs-call-borrows.md) specifies the owned-source
contract. [Owned source qualification](../docs/architecture/owned-source-validation.md)
records actual production CLI and reference/native evidence separately from
preactivation candidate results.
Linux x86_64, pinned LLVM 19.1.7 and O0 remain the only native qualification
target; O2, LTO, other targets, heap/drop safety and a completed roadmap milestone
are outside this increment.

The [fixed-array public-route evidence](../docs/architecture/fixed-array-public-validation.md)
records real source-file CLI compilation and source-free native execution.
Array source length 0..1024 does not override stricter native frame, local, IR,
call-graph or fuel admission. The three-module samples pilot is also covered;
its complete sequence is checked separately from its result checksum.
