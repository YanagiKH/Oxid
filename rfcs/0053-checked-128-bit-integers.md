# 128-bit integers

Status: experimental, `typed-preview`.

Use `i128` or `u128` for exact whole numbers beyond the 64-bit range:

```text
fn main() -> u128 {
    let amount = 18446744073709551616u128;
    return amount * 10u128;
}
```

This prints `184467440737095516160`. `i128` ranges from
−170141183460469231731687303715884105728 to
170141183460469231731687303715884105727. `u128` ranges from zero to
340282366920938463463374607431768211455.

## Operations and conversions

Both types support `+`, `-`, `*`, `/`, `%` and comparisons. Arithmetic requires
operands of the same type. Signed division truncates toward zero; the remainder
has the dividend's sign. Overflow, including signed minimum divided by −1 or
its remainder, reports E0604. A zero divisor reports E0607. Unary `-` accepts
`i128` and reports overflow for its minimum value.

Use `.to_i128_checked()` or `.to_u128_checked()` to convert a named integer.
Conversions from these types to every other supported integer use the same
checked method names. An out-of-range conversion reports E0610. There are no
implicit conversions or wrapping arithmetic. Unsuffixed literals remain `i32`.

## Results and storage

Result-mode entries print exact decimal values followed by a newline. JSON
represents both types as decimal strings, for example
`"result":{"type":"u128","value":"18446744073709551616"}`.
Process-mode entries still return an `i32` exit status.

128-bit values work in locals, calls, returns, fixed arrays, records and enum
payloads. Fields and array elements occupy 16 bytes with 16-byte alignment.
A scalar enum payload follows its tag at offset 16; such an enum occupies
32 bytes. Borrow and move rules also apply to these values.

Programs that use a 128-bit type select 32-byte tagged reference cells and
16-byte native scalar slots. Programs using earlier integer types retain
their existing storage costs. Memory admission includes operation helpers and
occurs before argument delivery or source effects.

Checking and reference execution support both execution policies. Linux
x86_64 native builds require LLVM/Clang/LLD 19.1.7 and support `dev` and
`release` profiles. Native executables run without sources or the compiler.
Array lengths and indexes remain `i32`; other compiler and storage limits
continue to apply.
