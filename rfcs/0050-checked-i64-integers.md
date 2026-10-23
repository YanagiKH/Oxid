# Checked 64-bit signed integers

Status: experimental, `typed-preview`.

`i64` represents every integer from −9223372036854775808 through
9223372036854775807 exactly. Decimal literals use an explicit `i64` suffix:

```text
fn main() -> i64 {
    let count = 9007199254740993i64;
    return count + 1i64;
}
```

Unsuffixed literals remain `i32`. An annotation does not convert a literal;
`let count: i64 = 1;` is a type error. Leading zeroes and whitespace or comments
between a unary minus and its literal are accepted. Separators, exponent,
floating-point and nondecimal spellings are unavailable. An out-of-range
literal fails with E0203 before execution, including in unused declarations
and skipped branches. A type or import alias cannot redefine `i64`.

## Operations and conversions

Both operands of `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>` and `>=`
must have the same type. Unary `-` accepts `i64`. Arithmetic never wraps:
overflow fails with E0604, including negating the minimum value and dividing
or taking its remainder by −1. Division by zero and remainder by zero fail
with E0607. Division truncates toward zero; a nonzero remainder has the sign
of the dividend. Diagnostics identify the executed operator.

Conversions use a named local or parameter:

| Conversion | Accepted input | Result |
| --- | --- | --- |
| `value.to_i64()` | `i32` or `u8` | Exact `i64` |
| `value.to_i32_checked()` | `i64` | `i32`, or E0610 outside its range |
| `value.to_u8_checked()` | `i32` or `i64` | `u8`, or E0610 outside 0..255 |
| `value.to_i32()` | `u8` | Exact `i32` |

No implicit numeric conversions, wrapping operators, generic numeric inference
or additional integer types are introduced by this contract.

## Values and entry points

`i64` is copied by value in bindings, calls, returns and scalar reads. It can
appear in record fields, `[i64; N]` fixed arrays, borrowed fixed arrays,
`&[i64]` / `&mut [i64]` call-only slices, and enum payloads. Existing moves,
whole-owner borrowing, call evaluation order, bounds and array length limits
apply. Indexes, lengths, process exit statuses and existing byte/file APIs
remain `i32`.

Result-mode `main() -> i64` prints the exact signed decimal value and a newline.
Its successful process exit status is 0. Process-mode `main` still returns
`i32`. JSON run summaries encode `i64` values as decimal **strings**, preserving
values beyond JavaScript's exact integer range:
`"result":{"type":"i64","value":"9007199254740993"}`.
Other scalar JSON encodings retain their existing representations.

## Execution and storage

Reference and Linux x86_64 LLVM native execution use the same checked arithmetic
under bounded and hosted policies, at both `dev` and `release` optimization.
Native programs run independently of source files and the compiler. Native
division guards zero and the minimum/−1 pair before LLVM division or remainder.
Output converts the minimum value through unsigned arithmetic, without signed
overflow or floating-point formatting.

An `i64` scalar occupies eight bytes aligned to eight in owned storage; an enum
with an `i64` payload has a sixteen-byte layout aligned to eight. Reference
programs using `i64` select sixteen-byte tagged scalar cells and snapshots.
Programs without it retain the existing eight-byte cells and layouts. Admission
charges the selected cells, complete control carriers and native output storage
before execution; widening does not enlarge an existing bounded memory limit.
Hosted execution retains storage admission without cumulative work fuel.

The retained narrow OIR import and observer formats do not admit `i64`.
New integer behavior is checked separately through public CLI tests, a Python
arbitrary-precision oracle with operands supplied after compilation, source-free
native execution, and C output tests with address/undefined-behavior sanitizers.
