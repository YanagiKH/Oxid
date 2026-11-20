# Integer maps

Use `IntMap<T>` to store values under `i32` keys. Values can be scalars or
records that implement `Copy`. Lookups use binary search. Insertions and
removals shift entries in key order.

Create a map with `new::<T>()`. Call `try_insert(&mut map, key, value)` to add
or replace a value. An allocation failure leaves all entries intact. Use
`get(&map, key)` to copy a value out, `remove(&mut map, key)` to remove it,
and `len(&map)` to count entries. Both `get` and `remove` return `Option<T>`.

Run the [example](../../examples/typed-int-map) from the repository root.

```sh
cd examples/typed-int-map
oxid run
oxid test
```

The example prints `42`.
