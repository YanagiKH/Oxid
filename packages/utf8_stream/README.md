# Experimental incremental UTF-8 decoder

An ordinary-source `typed-preview` path dependency for strict UTF-8 decoding
across arbitrary read boundaries. It imports no I/O, allocator, or compiler
builtin. It keeps scalar state, not the input payload. This is a new package;
the existing `utf8_text` package and `oxutf8` complete-buffer contracts are
unchanged. This is not a stable standard library API or completed R4 milestone.

## Frozen public API

- `Decoder`: owned record whose fields are private.
- `new_decoder() -> Decoder`: create an active, empty stream.
- `push_byte(&mut Decoder, u8) -> Step`:
  - `Pending`: accepted a byte, but its scalar needs more continuation bytes.
  - `Scalar(i32)`: accepted a byte completing exactly one Unicode scalar.
  - `Invalid(i32)`: malformed UTF-8; payload is the zero-based stream byte
    offset of the ill-formed sequence's leading byte.
  - `PositionOverflow`: accepting another byte would make the total number
    of submitted bytes exceed `2147483647`.
  - `Finished`: input was submitted after successful EOF finalization;
    the byte is not consumed and the decoder is unchanged.
- `finish(&mut Decoder) -> Finish`:
  - `Complete`: empty input or a stream ending at a scalar boundary.
  - `Invalid(i32)`: an earlier malformed sequence or incomplete final scalar.
  - `PositionOverflow`: an earlier position overflow.

From a repository project at `examples/projects/my_app/oxid.toml`:

```toml
[dependencies]
stream = "../../../packages/utf8_stream"
```

```oxid
use stream::Decoder;
use stream::Step;
use stream::Finish;
use stream::new_decoder;
use stream::push_byte;
use stream::finish;
```

No reset operation is provided. Construct a new decoder for a new stream.

## UTF-8 and error positions

ASCII, NUL, BOM, combining characters, and Unicode noncharacters are accepted.
Scalars range from U+0000 through U+10FFFF, excluding U+D800 through U+DFFF.
BOM is emitted as an ordinary scalar; it is not stripped. Normalization,
grapheme segmentation, lossy replacement and Unicode word classification are
outside this interface.

Accepted leading bytes are ASCII, C2..DF, E0..EF and F0..F4. Continuations
are 80..BF, except the first continuation after E0 must be A0..BF, after ED
80..9F, after F0 90..BF, and after F4 80..8F. These restrictions reject
overlong forms, surrogates and values above U+10FFFF.

An invalid leading byte or stray continuation reports its own offset.
A rejected continuation reports the pending sequence's lead, not the
offending continuation. EOF during a sequence also reports that lead:

| Bytes submitted | Result |
| --- | --- |
| `80` | `Invalid(0)` |
| `41 C0` | `Scalar(65)`, then `Invalid(1)` |
| `41 E2 28` | `Scalar(65)`, `Pending`, then `Invalid(1)` |
| `ED A0` | `Pending`, then `Invalid(0)` |
| `F4 90` | `Pending`, then `Invalid(0)` |
| `41 F0 90`, then EOF | `finish` returns `Invalid(1)` |
| `F4 8F BF BF`, then EOF | `Scalar(1114111)`, then `Complete` |

The error-triggering byte is not reinterpreted as a new scalar. Earlier scalar
events cannot be retracted. A caller requiring all-or-nothing external output
must defer that output until `finish` returns `Complete`.

## Lifecycle and precedence

Malformed input and position overflow are sticky failures. After `Invalid(n)`,
every `push_byte` returns `Invalid(n)` and every `finish` returns `Invalid(n)`.
After `PositionOverflow`, both operations keep returning that status.
These repeated calls do not mutate the decoder.

Successful `finish` closes the stream. Repeating `finish` returns `Complete`;
subsequent `push_byte` calls return `Finished` without reopening it. Finishing
an incomplete scalar permanently fails the stream, so later continuation
bytes cannot repair a prematurely finalized input.

`push_byte` checks existing terminal state first, then position overflow,
then UTF-8 validity. At most 2147483647 submitted bytes are representable.
The next active-stream call returns `PositionOverflow` before inspecting its
byte, including when a scalar was pending. EOF itself adds no byte. The
package does not expose or permit callers to forge its internal offset.
This representability bound is not a promise that current execution limits
permit a stream of that size.

## Costs and failure boundary

The package retains eight i32 fields and performs constant-size scalar work
per byte. It neither allocates a payload buffer nor retains input references.
Existing runtime ownership checks, bookkeeping, fuel and memory policies still
apply; this is not a whole-invocation allocation-free or unlimited-stream
execution guarantee. No compiler or runtime limit is raised or bypassed.

Recoverable enums describe completed calls only. Runtime terminal failures
are not caught, converted to decoder statuses, or promised to roll back
partially executed state updates.

## Independent consumer contract: oxtextstat

The planned Process consumer should use a fixed initialized read buffer and
submit each reported byte exactly once in order. Read chunk boundaries have
no meaning to this decoder. Call `finish` only on actual EOF, not at the end
of each read. Keep I/O imports in the consumer, outside this pure package.

Consumer output on valid EOF is exactly three ASCII decimal fields separated
by single spaces and followed by LF:

```text
bytes scalars lines
```

`bytes` counts all input bytes; `scalars` counts `Step::Scalar` events.
`lines` is the number of U+000A events plus one if the input is nonempty and
its final scalar is not U+000A. CR has no special meaning; CRLF contributes
one line terminator. BOM and NUL count as ordinary scalars. Empty input
produces exactly `0 0 0\n`.

All counters must check i32 overflow before incrementing, including the final
unterminated-line increment. Use `i32_decimal::format_i32`, not an independent
decimal implementation. No stdout is produced before valid EOF and successful
counter finalization. A subsequent write failure may leave an output prefix.

Suggested fixed application outcomes:

- Exit 0: exact successful report and empty stderr.
- Exit 65: invalid/incomplete UTF-8 and empty stdout/stderr.
- Exit 70: counter/position overflow or unexpected decoder lifecycle status,
  with no stdout/stderr before report writing.
- Exit 71: existing read-buffer initialization allocation/memory refusal.
- Exit 74: read/write failure or zero-progress write; stdout may contain a
  report prefix only if failure happens during final report writing.

Follow the existing Process I/O route for initialization, partial reads and
writes, Linux EINTR retry and zero-progress write rejection. These codes are
proposed application policy, not decoder results or newly qualified failures.

## Acceptance specification and current status

Source was authored for review only. No format check, compiler, interpreter,
Process or native execution, CI pass, or qualification is claimed.

Initial mandatory acceptance is ordinary CHECK/interpreter TEST plus the
existing interpreter Process route. Native execution or deployment
qualification requires a separately approved later step.

Independent pure cases should cover:

- Exact scalar events for ASCII/NUL, all 2/3/4-byte boundary values, BOM,
  noncharacters and combining sequences, against fixed expected scalars.
- Every partition of representative multibyte sequences into chunks,
  including one-byte chunks and empty chunk boundaries. The public API is
  byte-at-a-time; a chunk wrapper must not finalize at chunk boundaries.
- Invalid leads, stray continuations, overlong forms, surrogates and values
  beyond U+10FFFF; invalid second, third and fourth bytes where applicable.
- Incomplete EOF after each possible partial sequence, with exact lead
  offsets both at zero and after a valid prefix.
- Sticky malformed-input errors, EOF idempotency on success and failure,
  rejection of further input after successful finish, and inability to repair
  an incomplete stream after EOF.
- Position overflow and arithmetic reviewed from source: do not add a public
  state-forging hook or claim a runtime boundary test without a feasible
  supported input.

Independent Process fixtures should include exact reports for empty input,
LF, CR, CRLF, repeated/ending LF, unterminated ASCII, embedded NUL, BOM,
combining text and mixed multibyte input crossing the read-buffer boundary.
Late invalid and truncated inputs must produce no report. Validate exact
status, stdout and stderr. Do not derive expected scalar or line counts from
this decoder or accept round trips as the sole oracle.

Do not modify unrelated Cargo/test-harness work or existing Text semantics.
This package and its consumer do not claim to complete general streaming I/O,
forced I/O-failure testing, resource qualification, R2, R4 or v1.0.
