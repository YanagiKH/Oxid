# Local reference bindings

Status: implemented for the experimental typed edition.

An aggregate can be borrowed across several statements by naming its reference:

```oxid
struct Counter { value: i32 }

fn main() -> i32 {
    let mut counter = Counter { value: 40 };
    if true {
        let current = &mut counter;
        current.value = current.value + 2;
    }
    return counter.value;
}
```

The program returns 42. `let current = &counter;` grants shared access;
`let current = &mut counter;` grants exclusive access. The reference binding
itself is immutable. An optional annotation must match exactly, for example
`let current: &mut Counter = &mut counter;`.

The initializer borrows a complete named aggregate. Records, fixed arrays and
existing owned resource types keep their ordinary field, indexing and helper
operations. Borrow an existing reference explicitly with `&*current` or
`&mut *current`, including when passing it to a function. A shared reference
cannot grant exclusive access.

A local borrow lasts until the end of its declaring block. Normal completion,
`return`, `break` and `continue` release references before ending the storage
they protect. Inner reborrows end before their parents. Shared borrows may
coexist; an exclusive borrow prevents overlapping access, including access
through its parent. An owner cannot move, be replaced or be destroyed while
borrowed. A reference cannot be used outside its scope.

The last use of a reference does not shorten its lifetime. Use an inner block
of an `if`, loop or match arm when the owner must become available again.

Reference values cannot yet be copied into another binding, stored in a record,
returned from a function or reassigned. Local field, element, scalar, slice and
temporary borrows remain unavailable. Enum values cannot be borrowed.
Existing call-only field and slice
borrows retain their own rules. General dereference expressions are unchanged.

The raw verifier checks acquisition, release, parent authority and control-flow
joins independently of source lowering. Consumers retain the same loan identity
and generation checks used for calls. Each local reference occupies one loan
slot and one cleanup entry; parent chains allow at most 64 links. Existing
source, storage and work limits apply, with resource exhaustion reported as
`E0400`. Local acquisition and release consume the same execution work in
reference and native programs.

Existing programs keep their previous syntax and behavior. This extension does
not change the edition selector, artifact version or native ABI. The public
contract is exercised through checking, reference execution, formatting and
source-free Linux native programs, including loops, branches, generic bodies,
early exits, shared siblings and invalid ownership or permission combinations.
