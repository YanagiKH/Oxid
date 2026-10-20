# Experimental byte/hex codec

A pure ordinary-source `typed-preview` path dependency for converting arbitrary
bytes to lowercase ASCII hexadecimal and decoding strict ASCII hexadecimal back
to bytes. The caller supplies the destination storage. This package imports no
I/O, allocator, or compiler builtin and adds no language feature or execution
policy.

It is an experimental binary/text conversion package, not a stable standard
library interface or a completed R4/v1.0 milestone.

## Import

From a repository project at `examples/projects/my_app/oxid.toml`:

```toml
[dependencies]
hex = "../../../packages/bytes_hex"
```

```oxid
use hex::EncodeResult;
use hex::DecodeResult;
use hex::encode_lower;
use hex::decode;
```

The dependency alias is chosen by the caller. Both result enums are ordinary
owned values, with either no payload or one `i32` payload.

## Encoding

`encode_lower(input: &[u8], output: &mut [u8]) -> EncodeResult`

- `Written(byte_count)`: exactly the prefix `output[0..byte_count]` contains
  lowercase ASCII hexadecimal; the remaining suffix is unchanged.
- `BufferTooSmall(required_byte_count)`: the entire destination is unchanged.
  The payload is the minimum required output length.
- `LengthOverflow`: twice the input length cannot be represented by `i32`.
  The entire destination is unchanged.

Every input byte produces exactly two characters, using `0` through `9` and `a`
through `f`, with the high nibble first. No `0x` prefix, separator, whitespace,
newline, NUL terminator, or other suffix is written. For example, the three
bytes `00 80 ff` produce exactly the six ASCII bytes `0080ff`.

Length overflow is checked before multiplication and before output capacity.
An input length greater than `1073741823` returns `LengthOverflow`; otherwise
the required output length is exactly twice the input length. Insufficient
capacity returns `BufferTooSmall` before the first write. Empty input returns
`Written(0)` and leaves every destination byte unchanged, including when the
destination is also empty.

This arithmetic guard does not promise that an input near that size is
constructible or executable under the current runtime limits.

## Decoding

`decode(input: &[u8], output: &mut [u8]) -> DecodeResult`

- `Written(byte_count)`: exactly the prefix `output[0..byte_count]` contains
  decoded bytes; the remaining suffix is unchanged.
- `OddLength`: the input has an odd number of bytes.
- `InvalidDigit(byte_offset)`: the indicated byte is not an accepted ASCII
  hexadecimal character.
- `BufferTooSmall(required_byte_count)`: the valid input needs a destination
  of at least that length.

All three returned error variants leave the entire destination unchanged.
Decoding accepts only ASCII `0` through `9`, `a` through `f`, and `A` through
`F`. Letter case can be mixed. Whitespace is not trimmed; separators, `0x`
prefixes, NUL, and non-ASCII bytes are rejected. No UTF-8 decoding or locale
interpretation is performed.

The order is part of the contract:

1. An odd input length returns `OddLength`, without scanning digits or checking
   destination capacity.
2. For an even input length, scan every byte from left to right. Return
   `InvalidDigit` at the first invalid byte, before checking destination
   capacity.
3. For valid input, the required destination length is half the input length.
   Return `BufferTooSmall` if necessary.
4. Only after all these checks, decode and write the complete output prefix.

Offsets are zero-based byte positions relative to the supplied input view, not
character positions or offsets in any larger owner.

| ASCII input | Destination length | Result |
| --- | --- | --- |
| empty | 0 | `Written(0)` |
| `00Ff80` | 3 | `Written(3)`, with bytes `00 ff 80` |
| `00Ff80` | 2 | `BufferTooSmall(3)`, with no mutation |
| `g` | 0 | `OddLength` |
| `0g0` | 0 | `OddLength` |
| `g0` | 0 | `InvalidDigit(0)` |
| `0g` | 0 | `InvalidDigit(1)` |
| `00ff 0` | 0 | `InvalidDigit(4)` |
| `0x` | 1 | `InvalidDigit(1)` |

Empty input is valid and returns `Written(0)` without changing the destination.
An oversized destination is permitted; its unwritten suffix is preserved.

## Ownership, work, and failure boundaries

The input is shared and remains unchanged. The output is exclusively borrowed
for the call. Existing whole-root loan rules apply: the package does not add
overlapping or in-place input/output support, stored references, or escaping
views. Callers can pass an already supported call-only byte subrange; all
reported lengths and offsets then refer to that supplied view.

Encoding takes linear time. Successful decoding makes one full validation pass
and one decoding pass, also linear time. Both functions use constant local
working storage and do not allocate or grow a payload buffer. Ordinary runtime
bookkeeping still applies, so the complete invocation is not promised to be
allocation-free.

Bytes are widened to `i32` before arithmetic. Encoding divides by 16 and takes
the remainder, producing nibbles in `0..15`; their ASCII values fit `u8`.
The overflow guard bounds the encoding length and cursor by `2147483646`.
Decoding validates both nibbles before computing `high * 16 + low`, which is
always in `0..255`. An even input length is at most `2147483646`, so pair
indexing, loop increments, and division by two stay within `i32`.

Preservation guarantees describe returned codec errors. They do not replace
the runtime's terminal failures. Execution-budget exhaustion can interrupt a
successful-path write after a prefix has changed. No fuel, memory, admission,
or other limit is raised, bypassed, or caught. There is no package-specific
input-length cap beyond the representability required by encoding, and no
promise that arbitrarily long input can finish within the execution budget.

## Streaming integration

The package operates on complete views and performs no I/O. A Process-mode
caller can use the existing `read_some` and `write_some` APIs without changing
the codec:

- An encoder can initialize a 1,024-byte input buffer and a 2,048-byte output
  buffer once, encode each reported read prefix, and write exactly the
  `Written` prefix.
- A decoder can initialize a 1,025-byte staging buffer and a 512-byte output
  buffer once. Retain at most one trailing ASCII character between reads;
  read at most 1,024 new bytes, decode the largest even prefix, and preserve
  an optional trailing character for the next read. A remaining character
  at EOF is malformed input.

Those chunk sizes are caller choices, not new runtime capacity limits.
Neither pattern needs a growing whole-input buffer or a total-byte counter.
Existing execution limits still apply; bounded payload storage is not a
promise of unlimited stream processing.

The decoder validates an entire supplied chunk before changing its output
buffer. A malformed chunk produces no decoded output from that call, but a
streaming caller may already have written earlier valid chunks. Errors cannot
undo that earlier output. Chunk processing does not promise the same
error-selection order as validating one complete stream view. A write error
can likewise leave an externally written prefix. Callers must handle partial
writes, zero write progress, and the existing interrupted-I/O statuses.

## Validation scope

This package was authored and reviewed from source. No compiler check,
formatter run, interpreter execution, native execution, or CI result is
claimed here.

Independent consumers should use the ordinary path dependency and public enum
matching. Keep test helpers outside the package so production users do not
load them. Acceptance should cover:

- Exact lowercase output and mixed-case decoding against fixed byte oracles.
- All 256 byte values against an independent oracle, not only round trips.
- Empty, undersized, exact-sized, and oversized destinations.
- Every destination byte after a returned error, and every unwritten suffix
  after success.
- Odd-length precedence, first-invalid-byte offsets, and invalid-input
  precedence over insufficient destination capacity.
- NUL, non-ASCII bytes, prefixes, separators, and whitespace rejection.
- Chunked Process conversion across read boundaries, including a hex pair
  split between reads and an odd trailing character at EOF.
- Late malformed input with explicit expectations for already written output.

The large-length encoding guard requires source-level arithmetic review;
current resource limits do not provide an ordinary runtime test input for
that boundary. Focused ordinary interpreter cases, native qualification, and
full repository/release qualification are separate evidence.
