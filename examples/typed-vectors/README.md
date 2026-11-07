# Growable vectors

Append numbers to a `Vec<i32>`, extend it from an array, and read the result.
The program prints `42`.

```sh
oxid run
oxid test
oxid build --profile release
```

Use a compiler built from this repository; the published 0.9 binaries do not
include `alloc::vec`. See the [vector guide](../../docs/VECTORS.md) for the API,
element types and borrowing rules.
