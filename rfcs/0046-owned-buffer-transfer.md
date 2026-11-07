# Owned buffer transfer

Status: experimental in `typed-preview`.

`alloc::bytes::take(&mut ByteBuf) -> ByteBuf` transfers the existing allocation,
initialized length and capacity into a new owner. The source becomes an empty
buffer with zero capacity and can be used again. Empty sources return empty
buffers. No byte copying or allocation takes place, including at the current
live-memory limit. Dropping either value follows ordinary buffer cleanup.

```rust
use alloc::bytes::new;
use alloc::bytes::take;
use alloc::bytes::len;

fn main() -> i32 {
    let mut source = new();
    let output = take(&mut source);
    return len(&output);
}
```

The argument must be an exclusive reference to a mutable buffer. It can be a
record field or an explicitly forwarded exclusive parameter. Existing whole-root
borrows prohibit an overlapping staged view, a shared-to-exclusive upgrade or
use of a moved owner. The returned buffer can move through functions, record
fields and enum payloads. This operation does not add general partial field
moves or escaping references.

The interpreter validates source and result storage before clearing the source.
It transfers the existing allocation identity, without modifying the allocation
registry or live-memory ledger. The native helper checks the source descriptor
and rejects overlapping source/result storage before mutation. Existing catalog
identities and runtime opcodes retain their values; transfer appends a new one.
The operation adds no dynamic byte work. Ordinary call and primitive instruction
charges still apply under bounded execution.

The UTF-8 library exposes `into_bytes(Text) -> ByteBuf` using `take` on its
private buffer. This consumes the text while preserving its bytes and capacity.
The newline conversion application uses this buffer directly, avoiding a second
allocation proportional to the input.

Public checks cover populated and empty buffers, source reuse, nested owners,
exclusive forwarding, full-heap transfer, dropped results and borrow refusals.
Native checks execute both optimization profiles. Separate C component checks
exercise invalid descriptors, overlap rejection and allocation fault controls.
Retained compatibility assertions test prior bounded behavior; they do not
establish the new operation's semantics or complete the language's release gates.
