# Signed and unsigned integer widths

Status: experimental, `typed-preview`.

Choose the integer type that fits your data. These types store exact values;
arithmetic reports an error instead of wrapping around.

| Type | Range | Example literal |
| --- | --- | --- |
| `i8` | −128 to 127 | `-128i8` |
| `i16` | −32768 to 32767 | `32767i16` |
| `u16` | 0 to 65535 | `65535u16` |
| `u32` | 0 to 4294967295 | `4294967295u32` |
| `u64` | 0 to 18446744073709551615 | `18446744073709551615u64` |
| `isize` | −9223372036854775808 to 9223372036854775807 | `-1isize` |
| `usize` | 0 to 18446744073709551615 | `1usize` |

`isize` and `usize` are 64 bits on the currently supported execution targets.
They do not replace the `i32` indexes, lengths or arguments used by existing
array, byte, file and process APIs.

```text
fn main() -> u64 {
    let packets = 18446744073709551614u64;
    return packets + 1u64;
}
```

This prints `18446744073709551615` and exits successfully. Unsuffixed literals
remain `i32`; `let count: u64 = 1;` is a type error. Use `1u64` or explicitly
convert a named value. The existing `i64` suffix and unsuffixed `i32` grammar
are unchanged; an `i32` suffix is unavailable. Out-of-range decimal literals
report E0203 before execution. Negative unsigned literals are rejected, including
`-0u64`. Type names cannot be replaced by a declaration or import alias.

## Arithmetic and conversion

The new types support `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>` and
`>=`. Both operands must have the same type. Signed types also support unary
`-`. Overflow and unsigned subtraction below zero report E0604. Division or
remainder by zero report E0607. Signed division truncates toward zero; the
remainder has the dividend's sign. Negating the signed minimum, or dividing
or taking its remainder by −1, reports overflow.

Conversions use a named local or parameter and report E0610 if its value does
not fit the target:

```text
fn main() -> u64 {
    let port = 65535u16;
    return port.to_u64_checked();
}
```

Checked conversions are `.to_i8_checked()`, `.to_i16_checked()`,
`.to_i32_checked()`, `.to_i64_checked()`, `.to_u8_checked()`,
`.to_u16_checked()`, `.to_u32_checked()`, `.to_u64_checked()`,
`.to_isize_checked()` and `.to_usize_checked()`. Their receivers may be any
integer type. There is no implicit conversion. The existing infallible `u8.to_i32()` and
`i32.to_i64()` / `u8.to_i64()` methods keep their existing inputs.

`255u8` is a byte literal. Bytes can also be enum payloads and result values,
with the checked arithmetic described in [byte arithmetic](0052-byte-integer-arithmetic.md).
Use `.to_i32()` when a calculation needs more than 0..255.

## Collections and results

All these integers can be copied through parameters, local bindings, returns,
record fields, enum payloads, fixed arrays and borrowed slices. Existing move,
borrow, bounds and evaluation-order rules apply. Storage uses the integer's
actual width and alignment: 1, 2, 4 or 8 bytes. An enum containing a 64-bit
scalar payload uses 16 bytes aligned to 8.

Result-mode programs print exact decimal text. JSON encodes `i64`, `u64`,
`isize` and `usize` as decimal strings so values beyond 2⁵³ remain exact.
Smaller integer results use JSON numbers. Process-mode `main` still returns
an `i32` status.

Reference execution and Linux x86_64 LLVM native builds implement the same
checks under bounded and hosted execution, with `dev` and `release` builds.
Admission includes the selected tagged value storage, argument snapshots and
integer helper temporaries before source effects run. Existing narrow and
`i64` programs retain their earlier storage prices. Native executables run
without project sources or the compiler.

128-bit integers, floating-point types, numeric coercion and wrapping
arithmetic are not provided by this extension.
