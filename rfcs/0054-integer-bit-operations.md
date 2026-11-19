# Integer bit operations

Status: experimental, `typed-preview`.

Use `&`, `|`, `^` and `~` for integer masks and bit inversion. Use `<<` and
`>>` to shift a value:

```text
fn main() -> u8 {
    let flags = 240u8 | 15u8;
    return flags << 1;
}
```

This prints `254`. Left shift discards bits that leave the value's width.
Right shift copies the sign bit for signed integers and fills with zeroes for
unsigned integers. `~0u8` is `255`; `~0` is `-1`.

## Types and shift counts

All supported signed, unsigned and pointer-width integer types support these
operations. `&`, `|` and `^` require operands of the same type and evaluate
both operands from left to right. The result keeps that type.

A shift count may have any supported integer type. It must be nonnegative and
smaller than the left operand's bit width. Otherwise execution reports E0611
at the complete `<<` or `>>` operator and prints no result. Counts are checked
before shifting, including in optimized native builds.

For example, `1u128 << 127` is valid and `1u128 << 128` reports E0611.
Boolean values continue to use `!`, `&&` and `||`.

## Precedence and formatting

From strongest to weakest: unary operators; multiplication, division and
remainder; addition and subtraction; shifts; `&`; `^`; `|`; comparisons;
`&&`; `||`. Binary bit operations associate from left to right. Thus
`1 << 2 + 3` means `1 << (2 + 3)`, and `1 | 2 & 4` evaluates to `1`.

Each shift operator consists of two adjacent angle characters. Formatting
preserves those pairs and spaces binary masks while retaining borrow syntax
such as `&bytes`.

## Execution

Result text and JSON follow the declared integer type. Existing move, borrow,
bounds and integer-conversion rules still apply. Eight-, sixteen- and
thirty-two-byte reference cells retain their existing selection rules. Bit
operation scratch is admitted before source effects or argument delivery.

Reference execution supports both execution policies. Linux x86_64 native
builds require LLVM/Clang/LLD 19.1.7 and support both native build profiles.
Native programs run without project sources or the compiler.
