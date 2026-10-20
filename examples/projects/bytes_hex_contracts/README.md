# bytes_hex contracts

This ordinary-source typed-preview project depends on `packages/bytes_hex`
via a relative path (alias `hex`) and performs no I/O. It imports the public
`EncodeResult` and `DecodeResult` enums and the `encode_lower` and `decode`
functions, and matches every variant explicitly. Sixteen `test_` functions
return zero for success; the full contract is verified by running each of them
as a separate case under `oxid test`.

`main` is a Result-mode smoke with real codec behavior, not the full contract:
it calls only the existing `test_inputs_unchanged()` (successful and refused
`encode_lower` and `decode` calls, including `BufferTooSmall`, `InvalidDigit`
and `OddLength`, with destination and input checks), returns its nonzero code
unchanged on failure, and returns the fixed Result value 42 only on success.

Every expectation is fixed in this source and never produced by the package:

- A 512-character lowercase fixture for all 256 byte values, plus fixed
  uppercase and mixed-case forms of it. The 256-byte input is built by an
  index loop, and decoded output is compared with that index sequence.
- Small cases use `b"..."` literals. Two local 16-entry digit tables classify
  every byte value 0..255 independently of the package.
- Destinations are `ByteBuf` storage filled with nonuniform sentinels
  `(seed + 37 * offset) % 256`. Each error case checks every destination byte;
  each success checks the exact written prefix and every unwritten byte.

`encode_lower` coverage: all 256 bytes against the fixture, nibble order and
lowercase letters, empty input (`Written(0)` with an empty or sentinel
destination), zero-length, one-byte-short, much shorter, exact and oversized
destinations, and an unchanged input.

`decode` coverage:

- The 512-character fixture, its uppercase form and a mixed-case form all
  decode to the exact 256 bytes; `00Ff80` decodes to `00 ff 80`.
- Empty input, zero-length, short, exact and oversized destinations.
- `OddLength` first: odd inputs return it even when every byte is invalid
  (`g`, `0g0`, `zz0`, `\xff`) or valid (`abc`), and whatever the destination
  size.
- `InvalidDigit(offset)` with zero-based offsets in the high and low nibble of
  the first, second and third pairs, at the first, a middle and the last
  position, and at the first of several invalid bytes. Rejected classes
  include `g`/`G`, `x`, the neighbours `/`, `:`, `@` and backtick (0x60), NUL,
  tab, LF, CR, space,
  `+ , - . _`, and the non-ASCII bytes 0x80, 0xB0, 0xB9, 0xC1, 0xE6 and 0xFF,
  in both nibble positions. Every byte value is classified as accepted
  (exactly 22 values, decoded to the table nibble) or rejected at offset 1.
- Even invalid input with a short destination returns `InvalidDigit`, never
  `BufferTooSmall`. Late invalid bytes after many valid pairs leave exact and
  short sentinel destinations unchanged, so a write before full validation
  would be detected.
- Only fully valid even input returns `BufferTooSmall(required)`.
- Inputs are compared with independent copies after successful and refused
  calls. A short encode/decode round trip is a supplement only.

`LengthOverflow` is not executed: an input longer than 1,073,741,823 bytes
cannot be built under the current limits. Its guard (`input.len() > 1073741823`
before multiplication and before the capacity check) was reviewed in source.

Commands:

```sh
oxid check --manifest-path examples/projects/bytes_hex_contracts/oxid.toml
oxid test --manifest-path examples/projects/bytes_hex_contracts/oxid.toml --engine interpreter
oxid run --manifest-path examples/projects/bytes_hex_contracts/oxid.toml --engine interpreter --entry-mode result
```

Acceptance lives in `tests/typed_bytes_hex_package.rs`. No native, LLVM,
capacity, fuel or allocation-failure claim is made here.
