# Experimental i32 decimal codec

A pure ordinary-source `typed-preview` path dependency for converting complete
ASCII decimal fields to and from `i32`. It uses existing byte slices, enums and
checked integer arithmetic. It imports no I/O, allocator or compiler builtin,
and adds no compiler feature or execution-policy limit.

This package is experimental. It is not a stable `std::num` interface, general
numeric support, or a v1.0 completion claim.

## Import

From a repository project at `examples/projects/my_app/oxid.toml`:

```toml
[dependencies]
decimal = "../../../packages/i32_decimal"
```

```oxid
use decimal::ParseResult;
use decimal::FormatResult;
use decimal::parse_i32;
use decimal::format_i32;
```

The dependency's alias is chosen by the caller. All result enums are ordinary
owned values, with either no payload or one `i32` payload.

## Parsing contract

`parse_i32(input: &[u8]) -> ParseResult`

- `Value(value)`: the complete input is a decimal representation of that `i32`.
- `Empty`: the input has zero bytes.
- `SignOnly`: the complete input is exactly `+` or `-`.
- `InvalidDigit(byte_offset)`: a byte where a digit is required is not ASCII
  `0` through `9`.
- `Overflow(byte_offset)`: incorporating the digit at that byte offset would
  make the signed value fall outside `-2147483648..2147483647`.

The grammar is one optional initial ASCII `+` or `-`, followed by at least one
ASCII digit. Leading zeroes are accepted, including in signed forms. `0`, `-0`,
`+0`, and `000` all produce `Value(0)`.

Whitespace is never trimmed. Prefixes, separators, decimal points, embedded
signs, NUL and non-ASCII bytes are invalid. No UTF-8 decoding or locale-specific
interpretation is performed.

Offsets are zero-based byte positions in the original input, including its
optional sign and leading zeroes. Parsing proceeds from left to right. Each
byte is checked for digit validity before its numeric incorporation; the first
invalid byte or overflowing digit terminates parsing. A later invalid byte
does not replace an earlier overflow:

| Input | Result |
| --- | --- |
| `+008080` | `Value(8080)` |
| `-2147483648` | `Value(-2147483648)` |
| `2147483647` | `Value(2147483647)` |
| `2147483648` | `Overflow(9)` |
| `-2147483649` | `Overflow(10)` |
| `+2147483648` | `Overflow(10)` |
| `2147483648x` | `Overflow(9)` |
| `214748364x8` | `InvalidDigit(9)` |
| `--1` | `InvalidDigit(1)` |
| `12 ` | `InvalidDigit(2)` |

The shared input is unchanged on every result. There is no package-specific
input-length limit; existing source, storage and execution limits still apply.
In particular, accepting leading zeroes is not a promise that an arbitrarily
long input can finish within the current execution budget.

## Formatting contract

`format_i32(value: i32, output: &mut [u8]) -> FormatResult`

- `Written(byte_count)`: exactly the prefix `output[0..byte_count]` now contains
  canonical ASCII decimal for `value`; the remaining suffix is unchanged.
- `BufferTooSmall(required_byte_count)`: the entire output is unchanged, and the
  payload is the minimum number of bytes needed for this value.

Canonical output uses `-` only for a negative value, never `+`, and has no
leading zeroes. Zero is `0`. No NUL terminator, newline, whitespace or other
suffix is written. The function computes the complete width before its first
write. Width is from 1 through 11 bytes; an 11-byte destination can represent
every `i32`.

For example, parsing `+008080` and formatting its value produces exactly four
bytes, `8080`. Formatting `-2147483648` requires 11 bytes and produces those
exact bytes. A ten-byte destination for that value returns
`BufferTooSmall(11)` without changing even its first byte.

The caller owns the destination and controls any subsequent I/O. This function
does not allocate or grow a payload buffer. Existing runtime bookkeeping still
applies; the entire invocation is not promised to be allocation-free.

## Arithmetic and failure boundaries

Parsing accumulates a nonpositive value. Its lower bound is `-2147483648` for
negative input and `-2147483647` otherwise. Before each digit, it checks that
multiplication by ten and subtraction of that digit stay within this bound.
It never constructs positive `2147483648` or negates `-2147483648`.

Formatting also retains a nonpositive working value. Division by ten truncates
toward zero. Each remainder is in `-9..0`, so negating just that remainder and
adding ASCII `0` is safe. The formatter never takes the absolute value of MIN.

The recoverable status contracts do not replace the runtime's existing
terminal failures. In particular, output preservation is guaranteed for a
returned `BufferTooSmall`, not for an execution-budget failure interrupting a
successful-path write. No limit is raised, bypassed, or caught by this package.

## Validation scope

This package was authored and reviewed from source. No compiler, interpreter,
native executable, or CI result is claimed here.

The independent consumer should exercise ordinary path-dependency imports and
public enum matching. Its fixed expected values and output bytes must not be
derived from this implementation. Focused acceptance should cover:

- Signed and unsigned zero, leading zeroes, both bounds, and digit-width changes.
- Empty/sign-only input, invalid bytes at exact offsets, and earliest-error
  precedence between invalid bytes and overflow.
- Just-outside positive and negative bounds, including a leading plus sign.
- Exact canonical formatting bytes, reported widths, and unchanged suffixes.
- Zero-length, one-byte-short, exact-size and oversized destinations, checking
  every original byte after a `BufferTooSmall` result.
- A small round-trip set as supplemental evidence, not as the sole oracle.

Keep acceptance functions in the separate consumer rather than loading test
helpers into every package user. Ordinary focused checking and interpreter
tests establish only their actual tested scope; native qualification and full
release certification remain separate.
