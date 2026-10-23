# Owned UTF-8 text

Status: experimental, typed-preview local library.

`packages/utf8_text` provides move-only `Text` through ordinary Oxid source and
local dependencies. It uses `alloc::bytes::ByteBuf`; it adds no compiler type,
intrinsic, reserved name, or implicit conversion. Application dependency aliases
use the existing package identity and privacy rules.

## Invariant and conversion

`Text` has one private `ByteBuf` field containing complete UTF-8 sequences.
`new_text()` constructs an empty value. `from_bytes(ByteBuf)` validates all
initialized bytes before moving the owner into `FromBytes::Ok(Text)`. On invalid
input, `FromBytes::Invalid(ByteBuf)` returns the original owner and bytes.

`validate(&[u8])` returns `Validation::Valid` or
`Validation::Invalid(byte_offset)`. The offset identifies the start of the first
ill-formed sequence, including an incomplete sequence at the end. Validation
rejects isolated continuation bytes, C0/C1 leads, overlong sequences, surrogate
code points, F5..FF leads and values above U+10FFFF. NUL, BOM and Unicode
noncharacters are valid. No locale or replacement decoding applies.

## Observation and mutation

`byte_len(&Text)` counts UTF-8 bytes. `scalar_count(&Text)` counts Unicode scalar
values; it does not count grapheme clusters. `equals_utf8(&Text, &[u8])` compares
the complete initialized byte sequences exactly.

`append_utf8(&mut Text, &[u8])` validates the complete borrowed input before
requesting growth. `AppendStatus` is `Ok`, `InvalidInput(offset)`,
`CapacityOverflow`, `AllocationFailed`, or `MemoryLimit`. Any refusal preserves
the previous text. `append_scalar(&mut Text, i32)` encodes one scalar into a
local four-byte array and appends the initialized prefix. `ScalarAppendStatus`
uses the same growth outcomes and `InvalidScalar(value)` for a negative value,
a surrogate, or a value above U+10FFFF. Invalid scalars do not allocate or mutate
the text. Byte-buffer allocation and live-memory accounting remain unchanged.

`truncate(&mut Text, i32)` accepts a length from zero through the current byte
length only at a scalar boundary. It returns `true` on success, including an
unchanged length, and `false` with no mutation otherwise.
`append_to_bytes(&Text, &mut ByteBuf)` appends a copy of the initialized bytes;
it preserves the text owner and the existing destination prefix. Its return is
the underlying `GrowStatus`.

The API exposes no reference to the private buffer and no returned or stored
borrow. Text can move, occur in existing admitted records/enums, or be borrowed
using the existing call-only ownership rules. Cleanup uses existing recursive
resource handling. This does not introduce general strings, string literals,
Unicode normalization, grapheme segmentation, generic collections or a stable
ABI.

## Application and checks

`apps/utf8-normalize` is a two-binary workspace with a shared external local
dependency on the text library. The application reads bytes to EOF, validates
them, changes CRLF and CR to LF in a copied working buffer, and writes all output
with partial-write and interrupted-call handling. Invalid UTF-8 yields process
status 65 and no output. Capacity failure is 70, allocation/live-memory refusal
is 71, and I/O or no-progress writes are 74. Empty input succeeds with no output.
An I/O failure after a successful write may leave a prefix in the output.

The application explicitly selects `execution.policy = "hosted"` because input
length determines its work. Compiler and memory limits still apply. `contracts`
is a separate result entry point and declared `oxid test` target for API checks.
Native compilation remains Linux x86_64 with LLVM 19.1.7, in dev/O0 or release/O2.

The application verifier compares complete output bytes and statuses to
Python's strict UTF-8 decoder followed by CRLF/CR replacement. It retains raw
commands and executable identities, checks cache and relocation behavior, and
replays copied executables after removing their sources. These scoped checks
do not establish the remaining language-wide v1 release conditions.
