# Borrowing different record fields

Status: experimental

Typed programs may borrow different fields of one record in the same call.
Each exclusive reference can update its own field:

```rust
struct Value { amount: i32 }
struct Pair { left: Value, right: Value }

fn change(left: &mut Value, right: &mut Value) -> i32 {
    left.amount = left.amount + 10;
    right.amount = right.amount + 20;
    return left.amount * 100 + right.amount;
}

fn main() -> i32 {
    let mut pair = Pair {
        left: Value { amount: 1 }, right: Value { amount: 2 }
    };
    return change(&mut pair.left, &mut pair.right);
}
```

This returns `1122`. Shared and exclusive references may also coexist when
their field paths differ. A later argument may read a different field while an
earlier exclusive borrow is active. Arguments still evaluate left to right.

Nested paths are compared from their common outer record. Paths such as
`value.branch.left` and `value.branch.right` differ at `left` and `right`, so
they may be borrowed together. A field and its own descendant overlap; so do
two references to the same field. Borrowing the whole record overlaps every
field. These conflicts are rejected before execution.

The same rules apply when forwarding an exclusive reference parameter, using
`&mut *parameter.field`. Fields may contain records, fixed arrays, buffers or
files. Separate array fields may supply separate slices; two ranges of the
same array field retain the existing conservative overlap rule.

The outer owner remains alive and cannot be moved, replaced or destroyed while
any field borrow is active. An exclusive borrow still requires a mutable owner
or an exclusive reference. Nominal types, field visibility, reference identity
and generation checks remain enforced.

References remain call-only. This feature does not add stored or returned
references, partial moves, or separate borrowing of scalar fields and array
indices. Existing source, nesting and work limits continue to apply.

Reference execution and supported Linux native builds use the same checked
field paths. Public tests cover nested forwarding, left-to-right evaluation,
resource-owning sibling fields, array slices, empty records and overlapping
paths. Native tests run both execution policies and build profiles, then run
the copied programs after removing their sources and build directories.
