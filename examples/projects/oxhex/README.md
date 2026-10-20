# oxhex: bounded hex Process clients

Experimental ordinary-source workspace. Both members take a direct path
dependency on the pure package `packages/bytes_hex` (alias `hex`) and use only
the existing `ByteBuf`, `read_some` and `write_some` support. Storage is
initialized once and never grows with the total input size.

```text
oxhex/
  oxid.toml            [workspace] members = ["encode", "decode"]
  encode/oxid.toml     project "oxhex_encode", hex = "../../../../packages/bytes_hex"
  encode/src/main.ox   stdin bytes -> lowercase ASCII hex on stdout
  decode/oxid.toml     project "oxhex_decode", hex = "../../../../packages/bytes_hex"
  decode/src/main.ox   stdin ASCII hex -> bytes on stdout
```

```sh
oxid run --manifest-path examples/projects/oxhex/oxid.toml --package oxhex_encode --engine interpreter --entry-mode process < input.bin > output.hex
oxid run --manifest-path examples/projects/oxhex/oxid.toml --package oxhex_decode --engine interpreter --entry-mode process < input.hex > output.bin
```

## oxhex_encode

A 1,024-byte input buffer and a 2,048-byte output buffer. Each `read_some`
result `Data(n)` is encoded with `encode_lower(&input[0..n], &mut output)`, and
exactly the `Written(2n)` prefix is drained before the next read. `Eof` ends
with status 0. Output is raw lowercase hex: no prefix, separator, newline or
terminator. Empty input gives empty output.

## oxhex_decode

A 1,025-byte staging buffer and a 512-byte output buffer. Each read fills at
most 1,024 new bytes after an optional carried byte at `staging[0]`. The client
decodes only the largest complete even prefix with `decode`, writes exactly its
`Written` prefix, and carries at most one trailing byte into the next read. A
hex pair may therefore be split across reads. Uppercase, lowercase and mixed
case are accepted; whitespace and newlines are not trimmed.

- A chunk is decoded as one view. If it contains an invalid byte, none of that
  chunk's bytes are written and the status is 65.
- Earlier chunks have already been written and cannot be retracted. Package
  error preservation is per call, not whole-stream atomicity.
- At EOF with a carried byte the status is 65, even if that byte is not a hex
  digit; complete pairs read before it were already written.
- Because pairs are decoded as they arrive, a stream such as `abc` writes
  `ab` (one byte 0xab) and then returns 65 at EOF. The package's whole-view
  precedence (odd length before invalid digit) applies to each decoded view,
  not to the whole stream.

## Statuses and streams (both clients)

| Outcome | Status | stdout | stderr |
| --- | --- | --- | --- |
| Complete success (including empty input) | 0 | exact transformed bytes | empty |
| Malformed hex or odd EOF (decoder only) | 65 | bytes of earlier complete chunks | empty |
| Impossible package/runtime count or result | 70 | possibly a prefix | empty |
| Buffer initialization allocation failure or memory-limit refusal | 71 | empty | empty |
| Read error, write error or zero-progress write | 74 | possibly a prefix | empty |

stderr is exactly zero bytes for every outcome above: the clients write only
to stdout and define no diagnostic format. No package-local or stream-global
invalid offset is reported; the status code is the only error signal. Runtime
terminal failures (for example execution-budget exhaustion) keep the
interpreter's own diagnostics and are not part of this contract.

Interrupted reads and writes are retried only for EINTR (4). Partial writes are
drained until the intended prefix is complete; a write reporting zero bytes
returns 74 instead of looping. An impossible count is checked before slicing
and returns 70. Existing execution limits still apply: bounded storage does not
promise that an arbitrarily long stream fits the execution budget.

## Acceptance

`tests/typed_bytes_hex_package.rs` copies the sources into a fresh repo-shaped
tree and runs CHECK for both members, then the interpreter Process route with
exact status, stdout and stderr. Regular-file stdin makes the 1,024-byte read
boundaries deterministic: empty input, all 256 bytes, mixed case, lengths around
1,024 and multiple chunks, even and odd EOF, invalid bytes in the first chunk,
and late invalid data after an emitted chunk. A pipe with controlled chunk
injection waits for each step's expected output before writing the next step.
Only a step with non-empty expected output is a synchronization point, so this
proves a hex pair split across reads (`41a` then `b42`) and late invalid data
after emitted output. A step whose expected output is empty does not wait: the
`c` then `D` steps may be merged into one read, so a one-byte read carrying a
pending character is not guaranteed and is not claimed. Closing stdout before the
first write proves status 74. Forced EINTR and short-write coverage, allocation failure, status 70
and native execution are not exercised: the public Process route has no
controllable seam for them.
