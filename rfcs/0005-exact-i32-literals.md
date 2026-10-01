# RFC 0005: exact decimal i32 literals and scalar copies

Status: restricted experimental implementation for review. This extends
[RFC 0004](0004-bounded-reference-execution.md) with one exact integer type in
both check and explicit run. It is a provisional preview contract, not a stable
numeric type system, Rust compatibility promise, or completed M2/M3 milestone.

## Source and type contract

`i32` joins `bool` and unit. Parameters/results remain explicitly typed; let
annotations remain optional. Decimal literals have type i32 both in an explicit
i32 context and when unconstrained (`let x = 1`). Other contexts produce the
existing E0300 type mismatch. No truthiness, coercion, widening, multi-width
inference, promotion, or implicit conversion is introduced. Other widths such
as i64 and u32 remain E0202 unknown types.

A literal is one or more ASCII decimal digits, optionally preceded by one minus
token. Existing whitespace and comments may separate the minus and digits.
Leading zeroes are decimal. `0`, `0000`, `-0` and `-0000` are the same zero.
Range is always [-2147483648, 2147483647]; out-of-range literals produce E0203,
stage `resolve`, at the complete signed literal including intervening trivia.
`-2147483648` is valid without ever constructing positive 2147483648 as an i32.

The minus is a literal-only syntax, not a unary operator: grouping around a
complete signed literal is legal, but `-(1)`, `-x`, `-f()`, `--1` and `+1` are
unsupported. Numeric suffixes, separators, radices, decimal points, exponents and
non-ASCII digits are rejected as E0101/parse before magnitude conversion. The
lexer preserves mixed alphanumeric numeric candidates; the parser requires all
digit bytes to be ASCII. Unsupported spelling never silently accepts a prefix.
This does not add Unicode identifiers. Arithmetic, comparisons on integers and
casts remain unsupported, as do all other numeric types.

Literal representability is a compile-time validity condition, not an arithmetic
overflow policy. This increment defines no future checked/wrapping/saturating
arithmetic, shift, division, cast, NaN or floating-point optimization behavior.
No legacy f64 conversion or arithmetic participates in the new pipeline.

## Representation and validation

The lossless token tape retains original spelling. AST stores the digits' span
and a separate sign; the expression stores the complete literal span. Resolution
converts parser-validated ASCII digits with checked signed integer accumulation,
adding positive digits or subtracting negative digits. Leading zeroes may fill
the entire existing token budget. Numeric conversion allocates no bigint and
never rounds, truncates, wraps, or falls back to f64.

HIR stores a representable i32 constant; successful typed HIR has Ty::I32.
Lowering adds only the I32 constant Rvalue. The mandatory verifier accepts that
constant only in an i32 destination and enforces existing copy/call/return types,
origin validity, initialized definitions, dominance and resource rules.
No arithmetic/folding instruction or verifier bypass is added. All declarations
and unchosen branches must still compile before any execution.

## Execution, output and budgets

The compiler-carried zero-argument main may now return i32. Its text output is
canonical signed decimal followed by newline; success exits 0, including zero,
negative values and MIN. The result is never used as an OS process exit code.
Schema-1 run-summary adds `{ "type": "i32", "value": -2147483648 }` with integer
serialization. Existing bool/unit/check/diagnostic/failure-null records are
unchanged. All i32 values are exactly representable by binary64 JSON consumers,
but neither conversion nor serialization uses floating point.

The closed scalar enum gains I32(i32). Calls copy values into isolated frames;
repeated verified invocations remain independent. Fuel costs and failure order
are unchanged: a constant assignment costs one, and one i32 slot is one live
slot. Ceilings remain 1,000,000 fuel, 1,024 frames, 200,000 live slots and 256
argument-scratch scalars. Larger host representations increase actual byte
storage; the [validation report](../docs/architecture/i32-literal-validation.md)
records measurements. The old bool/unit-only byte estimate is historical.
These are count/work limits, not an allocator-success, fixed-process-memory,
termination, memory-safety, native-code or OS-sandbox guarantee.

## Acceptance and exclusions

Public source tests cover MIN/MAX and adjacent bounds, long zeroes, exact signed
origins, strict type errors, unsupported forms, complete-file checking, entry
order and canonical text/JSON. Raw-IR tests cover wrong constant/copy/call/return/
condition types, origins and local caps. Executor tests retain every predecessor
resource boundary and add i32 exact-cost, frame isolation, cap and 256-argument
cases. A Python bigint oracle independently checks conversion/range/output, and
a structured-source model independently checks values and call/branch/return
traces. All legacy and repository gates remain required.

No default-edition, legacy numeric/aliasing/OXBC, README layout, dependency,
artifact, ownership, native backend or host-I/O change is included.
