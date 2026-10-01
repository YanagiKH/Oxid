# RFC 0008: native checked i32 arithmetic

Status: accepted bounded direction and checked-overflow policy, 2026-10-01;
experimental implementation for independent review. Extends
[RFC 0006](0006-checked-i32-arithmetic.md) and
[RFC 0007](0007-llvm-scalar-native.md). Owner/reviewer: unassigned.

## Scope and observable contract

Admit verified `CheckedI32` assignments for ordinary `+`, `-` and `*` in the
existing nonrecursive Linux x86_64 LLVM preview. The parser, typed HIR, verifier,
reference semantics, source grammar, private scalar calling convention, resource
ceilings, tool trust model and no-clobber publication remain unchanged. This adds
no operators, general unary negation, casts, wider integers, recursion, profile
switch, native JSON mode, ownership claim or completed roadmap milestone.

The first executed overflowing operator terminates the executable with status 1,
empty stdout, and the same E0604 human diagnostic on stderr as reference `run`
for the compile-time source argument. The location is the operator's source line
and Unicode-scalar column. The error still uses stage `oir-run`: it denotes the
shared verified-OIR execution semantics, not a claim that an interpreter ran.
Successful bool/unit/i32 results and their status 0 are unchanged. Diagnostic
write failures, including broken pipes and signal-setup failures, terminate with
74, just like successful-result output failures. Partial stderr is possible when
a write fails. Host termination, blocked output and resource exhaustion are not
language overflow behavior.

Diagnostics embed the source path as supplied to `compile`, with the existing
human renderer's control-character escaping. This path is fixed at compile time;
renaming/removing source files or running elsewhere does not change it. It can be
visible in the executable's constant data. The executable never reads source at
runtime. No full source text or runtime JSON envelope is embedded.

## Lowering and failure adapter

Each checked assignment lowers to the corresponding LLVM
`llvm.sadd.with.overflow.i32`, `llvm.ssub.with.overflow.i32`, or
`llvm.smul.with.overflow.i32` intrinsic. The two-element result has a modulo-i32
component and a signed-overflow bit. These operations are defined for every pair
of input values, including overflowing pairs, as specified by the
[LLVM 19 language reference](https://releases.llvm.org/19.1.0/docs/LangRef.html#arithmetic-with-overflow-intrinsics).

Lowering branches on the overflow bit before exposing the i32 result. The success
block extracts the value and resumes the original OIR statement sequence. The
failure block calls a private noreturn adapter and is terminated by `unreachable`.
The adapter writes only the pre-rendered diagnostic to stderr, then calls POSIX
`_exit(1)` or `_exit(74)`. The `unreachable` follows an actually nonreturning call;
it is never a replacement for checking overflow. No plain wrapping arithmetic,
`nsw`, `nuw`, poison-based overflow assumption, hardware trap or platform-dependent
signal exit implements the language error.

Existing OIR order and call continuations are retained. Calls and each operand
finish exactly once, left before right, and the operator runs only after both.
An earlier failure prevents later operands, calls and statements from executing.
Discarded values still execute; unchosen branches and unused functions do not.
The checked result is not folded or reassociated by the Oxid frontend. Native
compilation still specifies LLVM/Clang `-O0`, without LTO or fast-math. Compiler
Cargo debug/release profiles do not select different language arithmetic.

The failure adapter extends private native ABI 1 without changing scalar
representations or user function signatures. Its parameters are a pointer to
constant diagnostic bytes and a 64-bit unsigned length (Linux x86_64 size_t).
Every diagnostic byte is emitted as an LLVM hexadecimal string escape. Paths
cannot supply IR syntax or symbols; numeric DefId/local identities name globals
and blocks. Strings are length-delimited and need no trailing NUL.

## Resource and compatibility boundaries

One arithmetic OIR assignment still costs one reference fuel in addition to its
operand assignments, calls and local allocation. The existing conservative
native cost recurrence already counts all assignments, including arithmetic,
both branch arms and repeated callee costs. `return 1 + 2;` therefore has an
inclusive bound of eight fuel. Native overflow occurs before the successful
return/output path; no new dynamic fuel machinery is needed under existing
whole-file admission ceilings.

Lowering adds two basic blocks per arithmetic assignment and constant diagnostic
data proportional to each rendered path/location. The original OIR block ceiling
applies before these backend blocks; aggregate locals bound arithmetic assignments
to at most 8,192. Compiler storage/time, allocator failure and host output blocking
remain outside the reference-fuel model. No ABI stability or portable ELF promise
is introduced. Default/explicit legacy routing, legacy f64 arithmetic and OXBC
bytes remain unchanged.

Alternatives rejected are wrapping/saturating results, profile-dependent overflow,
LLVM poison/unchecked flags, hardware traps, embedding an interpreter, or silently
falling back to the reference runner. Returning a tagged error through every
private scalar function would widen the initial ABI unnecessarily; this subset
has no recoverable exceptions, destructors or user I/O to unwind.

## Acceptance evidence

The [validation report](../docs/architecture/native-arithmetic-validation.md)
records generated Python-bigint/reference/native comparison, exact first-error
origins, MIN/MAX boundaries, operand/argument/call/discard ordering, chosen
branches, standalone artifact execution, error-output fault injection, compiler
profile parity and unchanged legacy gates. Local evidence is separate from
independent review and future CI results. Optimization beyond O0, native recursion,
cross compilation, production safety and complete numeric semantics remain open.
