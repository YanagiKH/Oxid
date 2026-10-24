# Floating-point numbers

Status: experimental, `typed-preview`.

Use `f64` for general decimal calculations and `f32` when a smaller value is
useful. A decimal point or exponent selects `f64`; suffix a literal with `f32`
to select single precision:

```text
fn main() -> f64 {
    let distance = 1.5;
    let seconds = 2.0;
    return distance / seconds;
}
```

This prints `0.75`. `0.1f32`, `1f32`, `1.0f64` and `1e-3` are also valid.
Plain integer literals keep their existing integer type. Operands must have
the same type; an annotation does not implicitly convert a literal.

## Arithmetic and comparisons

Both types use IEEE 754 binary arithmetic with rounding to nearest, ties to
even. `f32` has 24 significant binary bits; `f64` has 53. Each operation rounds
to its declared width. Native optimization does not enable fast math or
combine separate operations into a fused multiply-add.

`+`, `-`, `*`, `/`, `%` and unary `-` operate on floats. Remainder uses a
quotient truncated toward zero, so `-7f64 % 2f64` is `-1`. Overflow produces
infinity; division by zero can produce infinity or NaN. These operations do
not report integer overflow or integer division errors.

NaN compares unequal to every value, including itself. Its ordering comparisons
are false. Positive and negative zero compare equal; decimal output preserves
negative zero as `-0`. Arithmetic NaNs use the canonical positive quiet NaN bit
pattern: `0x7fc00000` for `f32`, `0x7ff8000000000000` for `f64`.

A finite decimal literal outside its type's finite range is a checking error.
Tiny literals round to a subnormal value or signed zero. Underscores and
hexadecimal float literals are not supported.

## Explicit conversions and bit patterns

Named numeric values support `.to_f32()` and `.to_f64()`. These conversions
may round; conversion to a smaller range can produce infinity. Float-to-integer
conversions are not part of these methods. Changing float width canonicalizes
NaN; conversion to the same float type preserves its bits.

Use `.to_bits()` to obtain a float's exact `u32` or `u64` representation.
Use `f32::from_bits(bits)` or `f64::from_bits(bits)` to reconstruct a value
from a named `u32` or `u64` value:

```text
fn main() -> f32 {
    let bits = 1065353216u32;
    return f32::from_bits(bits);
}
```

This prints `1`. Bit construction and extraction preserve all bits, including
NaN payloads. Unary negation flips the sign bit. These bit operations do not
perform a numeric conversion. Constructors currently take a named value;
store a computed bit pattern in a variable first.

## Storage and execution

Floats can be copied, passed to functions and stored in records, fixed arrays,
scalar slices and enum payloads. `f32` occupies four bytes and `f64` eight,
with matching alignment. Existing aggregate move and borrow rules apply.

Result-mode execution prints the shortest decimal text that round-trips to
the selected float type, using fixed decimal notation. Infinity is `inf` or
`-inf`; NaN is `NaN`. JSON results store that text as a string, preserving
special values and negative zero. Process entries still return `i32`.

Reference execution supports both execution policies. Native builds support
Linux x86_64 with LLVM/Clang/LLD 19.1.7, in both native build profiles. The
compiler embeds its small decimal-formatting runtime; building or running a
native program does not require Rust or an Oxid installation on the deployment
host. Existing libc requirements still apply.
