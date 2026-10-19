# Experimental owned UTF-8 Text package

A pure ordinary-source `typed-preview` path dependency for the Oxid toolchain
with RFC 0032 ByteBuf and path-dependency sessions. The implementation is here;
`examples/projects/owned_utf8_text` is its partitioned regression consumer.
Consuming export uses the experimental `alloc::bytes::take` primitive. No I/O
imports, registry publication, stable stdlib status, or canonical `String` API
are implied.

## Import

From a repository project at `examples/projects/my_app/oxid.toml`:

```toml
[dependencies]
text = "../../../packages/utf8_text"
```

```oxid
use alloc::bytes::new;
use text::new_text;
use text::append_to_bytes;

fn main() -> i32 {
    let document = new_text();
    let mut bytes = new();
    let status = append_to_bytes(&document, &mut bytes);
    return 0;
}
```

Callers must match `GrowStatus` to handle a possibly refused nonempty export.

## API and ownership

- `Text` owns a private ByteBuf; mutable byte access never escapes.
- `new_text() -> Text`, `byte_len(&Text) -> i32`, `scalar_count(&Text) -> i32`.
- `validate(&[u8]) -> Validation`: `Valid` or `Invalid(byte_offset)`.
- `from_bytes(ByteBuf) -> FromBytes`: `Ok(Text)` or `Invalid(ByteBuf)`;
  invalid input returns the original unchanged owner. Validate first for offset.
- `append_utf8(&mut Text, &[u8]) -> AppendStatus`: `Ok`,
  `InvalidInput(byte_offset)`, `CapacityOverflow`, `AllocationFailed`, `MemoryLimit`.
- `append_scalar(&mut Text, i32) -> ScalarAppendStatus`: `Ok`,
  `InvalidScalar(original_value)`, or the same three recoverable growth failures.
- `equals_utf8(&Text, &[u8]) -> bool`: exact byte equality, without allocation.
- `truncate(&mut Text, i32) -> bool`: reject negative, past-end, or inside-scalar
  lengths unchanged; zero and current byte length are valid boundaries.
- `into_bytes(Text) -> ByteBuf`: consume Text and transfer its descriptor,
  preserving initialized bytes and capacity without payload copying or a new
  ByteBuf allocation. The consumed Text cannot be used afterward.
- `append_to_bytes(&Text, &mut ByteBuf) -> GrowStatus`: append exact initialized
  bytes through one checked `try_extend`; preserve any destination prefix.
  Source Text remains unchanged. Empty Text is a no-op. Recoverable refusal
  preserves destination bytes, length and capacity under the ByteBuf contract.
  Use an empty destination when a standalone serialized owner is wanted.

Validation is complete-input and strict. NUL, BOM, and Unicode noncharacters
are accepted. Surrogates, overlong encodings, stray continuations, values above
U+10FFFF, and incomplete tails are rejected. Invalid offsets identify the
start of the first ill-formed scalar sequence. Lengths/offsets are i32. Invalid
append input/scalars are refused before mutation; returned growth failures
preserve Text. Execution-budget failures remain terminal, not recoverable statuses.

`append_to_bytes` copies O(n) bytes and can allocate additional capacity. Peak live
memory includes Text storage, existing destination storage, and old-plus-new
destination allocations during growth, plus allocator-policy overhead. Export
can fail near the memory limit even if Text alone fits. Two retained owners do
not imply peak memory is exactly twice the text length. Ordinary whole-root
loan checks still apply. `into_bytes` avoids this payload copy and allocation;
ordinary call/control work and registry validation still apply, so the whole
function invocation is not promised to be allocation-free or constant-time.

Validation, equality and scalar counting are linear source loops; scalar count
is not grapheme count. There is no escaping byte borrow or general
field extraction, lossy conversion, normalization, streaming decoder, or text
indexing. Keep I/O in a separate consumer package: even unused loaded I/O
helpers affect Result/Test admission.

## Validation scope

On Linux x86_64 with LLVM 19.1.7, the `take`/`into_bytes` addition passed
three focused Take source/reference and native-IR regression tests and three
catalog/signature/identity tests. The Text regression target passed two
ordinary tests and its standalone-native test (three passed with ignored
native coverage enabled). These cover the updated consuming-export consumer,
including ownership rejection checks. Scoped Clippy with warnings denied and
the scoped rustfmt recheck passed; the initial rustfmt check required mechanical
formatting before these focused regression tests.

The consuming-export `oxutf8` app then passed CHECK (29 functions), and its
unchanged pure contracts passed CHECK (34 functions). The ordinary package
acceptance wrapper and the wrapper with ignored native coverage enabled both
passed. A separate direct native acceptance-script run also passed the fixed
fourteen-input corpus and retained its evidence; see the consumer README for
the scope of those runs.

These are focused results, not a full repository CI pass or completed R2.
Recoverable allocation-failure injection, stale-descriptor injection, and
failure-transport and other outstanding qualifications remain unverified.
