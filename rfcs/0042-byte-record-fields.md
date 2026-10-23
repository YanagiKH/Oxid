# Byte record fields

Status: experimental in `typed-preview`. Record fields can contain `u8` or
`[u8; N]` for N=0..1024, including nested records and zero-length arrays. Native execution
supports Linux x86_64 with LLVM, Clang and LLD 19.1.7 at O0 and O2.

## Construct, access and borrow

```text
struct Packet { tag: u8, data: [u8; 2] }
fn byte(value: i32) -> u8 { return value.to_u8_checked(); }
fn main() -> i32 {
    let mut packet = Packet { tag: byte(7), data: [byte(19), byte(23)] };
    packet.tag = byte(9);
    let first = packet.data[0];
    let second = packet.data[1];
    return first.to_i32() + second.to_i32();
}
```

Field names, visibility, construction and exact types follow the existing
[record composition](0020-owned-record-composition.md) rules. A byte is exactly
0..255. Named i32 values narrow through `to_u8_checked()` and named byte values
widen through `to_i32()`; literals remain i32 and no implicit conversion occurs.
An out-of-range narrowing reports E0610 before the destination is written.

Named-root paths can read and assign byte leaves, index byte-array fields and
read their length. Byte-array fields can supply the existing call-only fixed
array or slice borrows, including exclusive access and explicit reborrowing.
Loans retain whole-root authority: borrowing one field does not permit an
overlapping borrow of its containing owner. Array elements cannot be borrowed
individually, and whole array fields cannot be partially moved out of a record.

Complete records remain nominal, move-only values. They can be passed,
returned, replaced, nested in other records, and moved into or out of finite
enum alternatives. Field visibility is preserved after enum extraction.
References and enums remain excluded as record fields. No heap storage, byte
arithmetic, coercions or I/O signature changes are introduced.

## Layout and execution

Byte fields use one byte with alignment one. A byte-array field uses stride one
and `max(1, N)` bytes, including the existing initialized empty-array sentinel.
Records apply their ordinary alignment and padding rules to all fields. Each
scalar leaf has logical width one and a fixed array has width `max(1, N)`;
smaller physical storage does not reduce the corresponding fuel charges.

Source checking and the independent raw declaration verifier both admit these
types. Existing bounds, exact layout, containment depth and allocation limits
remain in force, including unused declarations. No new consumer operation or
runtime is needed: the checked interpreter and LLVM use the existing byte
loads, stores, transfers and projected loans. This private layout is not a
stable serialization or FFI ABI.

Indexed writes retain RHS-before-index evaluation. Complete transfers validate
all active leaves before committing destination bytes or ownership. A later
invalid bool must not commit an earlier valid byte field. Failed narrowing must
preserve an existing destination and its owner generation.

## Verification

Public CLI tests cover scalar, nested and array fields, empty/maximal arrays,
borrows, moves, bounds, conversions, package privacy and native execution after
source removal. Internal tests cover independently checked layout, allocation
failure and destination preservation. A real LLVM corruption harness runs at
O0 and O2 with a valid case, late invalid bool and a deliberately incorrect
partial-copy control.

The [frame filter](../apps/frame-filter/README.md) stores its private numeric
pair fields as bytes while preserving its public API and binary protocol.
Its independent oracle remains unchanged. Historical byte-storage fixtures,
registry and assertion bodies retain their original source scope; the current
source census records the explicitly renamed field tests separately.
