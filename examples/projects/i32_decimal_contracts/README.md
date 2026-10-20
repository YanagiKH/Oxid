# I32 decimal contracts

This ordinary-source typed-preview project depends on `packages/i32_decimal`
via a relative path (alias `decimal`) and performs no I/O. It imports the
public `ParseResult` and `FormatResult` enums and the `parse_i32` and
`format_i32` functions. Twenty-two `test_` functions return zero for success;
`main` runs all of them and returns the fixed Result value 42.

Every expectation is written by hand in the source. No value, index, length or
byte is produced by the package under test and then reused as an oracle. Bytes
are built either with `b"..."` literals or with the ordinary `byte` helper
(`value.to_u8_checked()` on a named i32), and every buffer reaches the package
as a whole fixed-array slice (`&input` or `&mut buffer`).

`parse_i32` coverage:

- `0`, `-0`, `+0`, `±1`, `9`/`10`, `99`/`100` and their negatives, nine and ten
  digit values, `2147483647`, `+2147483647`, `-2147483648` and neighbours.
- Leading zeros, including zero-padded `MAX` (20 bytes) and `MIN` (21 bytes).
- `Empty` for zero bytes; `SignOnly` for exactly `-` or `+`.
- `InvalidDigit(index)` with the zero-based byte index for doubled or trailing
  signs, space/tab/CR/LF, underscore, point, comma, NUL, 0xFF, high-bit digits
  (0xB0, 0xB9), `/` and `:`, letters, `0x` and exponent forms.
- `Overflow(index)` at the first digit that leaves the i32 range, for positive,
  `+`-signed, negative and zero-padded inputs and long strings. A later invalid
  byte never replaces an earlier overflow (`2147483648x` is `Overflow(9)`), and
  an earlier invalid byte wins over a later overflow.
- Inputs compared with independent byte arrays after parsing.

`format_i32` coverage:

- Exact bytes and `Written` length for small values, `MAX` (10 bytes), `MIN`
  (11 bytes), `-2147483647` and `±1000000000`; only negatives carry `-`.
- `BufferTooSmall(required)` for a zero-length buffer, a buffer one byte short,
  and a much shorter buffer, with every byte of the buffer left unchanged.
- Exact-length buffers, and oversized buffers whose suffix keeps its original
  distinct sentinel bytes.
- A small format-then-parse round trip, as a supplement only.

`MIN` is written `-2147483647 - 1` in source. The only commands this case
needs are ordinary check, interpreter test and interpreter Result run:

```sh
oxid check --manifest-path examples/projects/i32_decimal_contracts/oxid.toml
oxid test --manifest-path examples/projects/i32_decimal_contracts/oxid.toml --engine interpreter
oxid run --manifest-path examples/projects/i32_decimal_contracts/oxid.toml --engine interpreter --entry-mode result
```

Acceptance lives in `tests/typed_i32_decimal_package.rs`. No native, LLVM,
capacity, fuel or allocation-failure claim is made here.
