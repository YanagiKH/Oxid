# Owned enum payloads

Status: experimental implementation in `typed-preview`. Checking, bounded
reference execution and Linux x86_64 native builds support this contract.
Native builds require LLVM, Clang and LLD 19.1.7.

## Use values in alternatives

An enum variant can contain one record, fixed scalar array or other finite
enum, in addition to the existing `bool`, `i32` and `()` payloads:

```text
struct Pair { left: i32, right: i32 }
enum Content { Pair(Pair), Bytes([i32; 2]), Empty }
enum Result { Ready(Content), Failed(i32) }

fn total(result: Result) -> i32 {
    match result {
        Result::Ready(content) => {
            match content {
                Content::Pair(pair) => { return pair.left + pair.right; },
                Content::Bytes(bytes) => { return bytes[0] + bytes[1]; },
                Content::Empty => { return 0; },
            }
        },
        Result::Failed(code) => { return code; },
    }
}
fn main() -> i32 {
    return total(Result::Ready(Content::Pair(Pair { left: 19, right: 23 })));
}
```

Payload type syntax accepts a visible record or enum name, its imported alias,
a `crate::` path, or a fixed scalar array type. Array lengths remain 0..1024;
array elements are `bool`, `i32`, `u8` or `()`. A direct scalar `u8` payload
remains unsupported. Empty records and zero-length arrays are valid owned
payloads. A nullary variant has no argument; `Unit(())` still requires one.

Types match exactly. Names in separate packages retain distinct nominal
identities even when declarations have identical spelling and fields. A public
enum's payload type must be visible through that public interface. Public
record types can keep their fields private; extracting a payload grants no
additional field visibility.

The payload containment graph must be acyclic. Direct and indirect recursion
fail during resolution, including in unused declarations. Combined record and
enum containment depth is at most 64, regardless of declaration order or shared
subgraphs. Records retain their existing field rules: this change does not add
enum fields to records, arrays of enums, references in payloads or heap storage.

## Move and consume

Constructing an owned alternative consumes its complete payload. Passing,
returning, assigning and discarding enum values use whole-value ownership.
Using a moved value reports an ownership error. Different alternatives of an
enum have the same enum type; assignment still requires a mutable destination.

`match` consumes an available named local or parameter. Every variant must
occur once, with one immutable binding for a payload. An owned binding is
initialized only in the selected arm and can be moved, passed to an ordinary
call-borrow helper, or consumed by another match. Binding scope and field
privacy follow the existing language rules. Only continuing arms participate
in the ownership join.

Match remains a statement. Nested matches are available; nested patterns,
guards, wildcards, tuple payloads and arbitrary scrutinee expressions are not.
Enum borrowing, equality, partial moves and source-visible tags are unchanged.

## Representation and execution

The private layout reserves four bytes for the variant tag followed by enough
space for the largest alternative. Its size is
`align4(4 + max(4, payload sizes))`, with alignment four. Its logical width is
`1 + max(1, payload widths)`. Record and fixed-array widths retain their
existing definitions. Scalar-only and nullary enums therefore retain size
eight and width two. No stable layout, serialization or FFI ABI is introduced.

Transfers first validate the tag and every leaf of the active alternative.
Inactive alternatives and padding are never interpreted as typed values. A
validation failure leaves destination bytes, ownership and generations intact;
an invalid later leaf cannot commit an earlier field. Reference execution and
LLVM use the same checked declarations and sealed ownership witness.

Construction costs `1 + width(destination enum)` after evaluating its operand.
Selected-arm consumption costs `1 + width(scrutinee enum)`; written dispatch
order retains its existing cost. Whole moves, replacement, calls and returns
use the existing width-based ledger. All charges precede the corresponding
effects. In particular, source expressions, temporaries and cleanup add their
own costs beyond these individual operations.

Existing declaration, layout, ownership, native inventory, execution and
allocation limits remain in force. Layout analysis uses bounded, fallible
scratch reservations without expanding shared containment graphs. Scalar-only
declaration checks retain their previous allocation path. Optimization at O2
preserves the same validation, effect ordering and execution limits as O0.

## Compatibility and verification

This extends the typed source contract from
[bounded enum matches](0023-bounded-enum-match.md). Legacy scripts and `.oxb`
packages retain their separate runtime contract. No edition becomes stable
through this addition.

The public tests in `tests/owned_enum_payloads.rs` and
`tests/owned_enum_payload_acceptance.rs` cover construction, nested consumption,
empty payloads, cross-package visibility and identity, cycles, ownership errors,
native profiles and execution after source removal. Internal verifier tests
cover forged types, incomplete initialization, bounded depth and shared graphs,
reservation failures, exact fuel and late invalid leaves. A native corruption
harness checks destination preservation at both optimization levels and
includes a deliberately incorrect partial-copy control.

The [frame filter](../apps/frame-filter/README.md) uses owned payloads across
two local libraries. `scripts/verify_owned_enum_app.py` compares it with an
independent byte protocol oracle, including I/O failures, earlier output,
runtime errors, relocation and source-free deployment.

`scripts/verify_enum_payload_compatibility.py` runs the retained scalar/record
assertions against the current compiler using a separately pinned source
boundary. Those historical assertion bodies and observers remain unchanged;
their compatibility result does not qualify the new payload semantics.
