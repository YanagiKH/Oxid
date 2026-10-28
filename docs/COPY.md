# Copying records

Use `#[derive(Copy)]` when assigning a record or passing it to a function should
leave the original value available.

```oxid
#[derive(Copy)]
struct Point {
    x: i32,
    y: i32,
}

fn total(point: Point) -> i32 {
    return point.x + point.y;
}

fn main() -> i32 {
    let first = Point { x: 20, y: 1 };
    let second = first;
    return total(first) + total(second);
}
```

The program prints `42`. Each copy has its own fields: changing one copy does
not change another. Records without the attribute keep their move semantics.

Run the [complete example](../examples/copy-records) with a compiler built from
this source tree:

```sh
cd examples/copy-records
oxid run
oxid test
oxid build --profile release
```

A Copy record can contain scalar values and other explicitly declared Copy
records. Generic fields require a `T: Copy` bound. Arrays, enums, references,
byte buffers, files and vectors cannot be fields of a Copy record.

Shared borrows permit copying. An exclusive borrow prevents copying or other
access until the borrow ends. Use record patterns to extract a nested record;
returning an aggregate field directly is not yet supported.

`Vec<T>` and scalar slices still require scalar elements. Declaring a record
Copy does not make it a supported vector element.

Use a compiler built from this source tree. Published 0.9 binaries do not
support this attribute.
