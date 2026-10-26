# Borrowing record fields

Typed programs can pass a record field to a function that accepts the same
record type by reference. Shared borrows read the field; exclusive borrows can
update it. The field may contain other records, arrays, buffers or files.

```rust
struct Position { x: i32, y: i32 }
struct Entity { position: Position, tag: i32 }

fn advance(position: &mut Position) -> () {
    position.x = position.x + 1;
    return;
}

fn step(entity: &mut Entity) -> () {
    advance(&mut *entity.position);
    return;
}

fn main() -> i32 {
    let mut entity = Entity {
        position: Position { x: 1, y: 2 }, tag: 7
    };
    step(&mut entity);
    return entity.position.x;
}
```

This returns `2`. Use `&value.field` or `&mut value.field` for an owned local.
When forwarding a reference parameter, use `&*parameter.field` or
`&mut *parameter.field`. A path may contain several record fields.

The declared record type must match exactly. Two record types with identical
fields remain different types. Every field in the path must be visible to the
calling module. An exclusive borrow requires a mutable owner or an exclusive
reference parameter.

Different fields may be borrowed together under the
[field borrowing rules](0067-disjoint-field-borrows.md). A later argument may
also read a different field while an earlier exclusive borrow is active:

```rust
update(&mut wrapper.inner, wrapper.amount);
```

Arguments evaluate left to right, and each borrow starts before evaluating the
next argument. The whole owner cannot move or be destroyed until all its field
borrows end. References remain call-only: they cannot be stored in locals or
records, returned, or used to move a field out of its owner. Existing scalar
slice and resource borrowing rules apply inside a borrowed record.

Reference execution and Linux x86_64 native builds support these operations in
both execution policies and build profiles. The checked nominal field path
determines each view's type and offset; the original outer owner retains loan,
generation and cleanup authority. Nested reborrows compose those paths without
creating an independent owner or exposing a pointer.
