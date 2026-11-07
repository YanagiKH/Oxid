# Byte arithmetic and results

Status: experimental, `typed-preview`.

`u8` is an unsigned integer from 0 to 255. Use it directly for byte-sized
calculations:

```text
fn main() -> u8 {
    let code = 250u8;
    return code + 5u8;
}
```

This prints `255` and exits successfully. The same arithmetic and result rules
apply in individual files, linked modules, and programs using records, arrays
or enums.

## Operations

Bytes support `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>` and `>=`.
Arithmetic requires two `u8` operands. Overflow and subtraction below zero
report E0604; division or remainder by zero report E0607. Errors identify the
executed operator and produce no partial result. Unsigned unary negation is
unavailable.

Unsuffixed decimal literals remain `i32`. Use `1u8` in byte arithmetic or
convert a named value with `.to_u8_checked()`. That conversion accepts an
existing byte as well as the other integer types, and reports E0610 for a value
outside 0..255. `.to_i32()` widens a byte when a calculation needs more range.
There is no implicit conversion or wrapping arithmetic.

## Results and storage

Result-mode `main() -> u8` prints a decimal value followed by a newline. JSON
uses an integer value, for example `"result":{"type":"u8","value":255}`.
Process-mode entries still require `fn main() -> i32` and interpret its result
as an exit status.

Byte fields and array elements continue to occupy one byte. Programs with only
`bool`, `i32`, `u8` and `()` keep eight-byte tagged scalar cells. The executor
admits arithmetic and conversion helper storage before source effects; a
memory refusal cannot partially deliver program arguments.

Reference execution and Linux x86_64 LLVM native builds implement these rules
under bounded and hosted execution. Both `dev` and `release` native programs
run without project sources or the compiler. Existing move, borrow, bounds,
array-length and native storage limits remain in effect.
