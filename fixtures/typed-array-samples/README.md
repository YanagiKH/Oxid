# Fixed scalar array samples

This experimental typed-preview example combines a structural `[i32; 8]` array,
whole moves, shared/exclusive call borrows, reborrows, checked indexing, length,
loops and an opaque scalar-field `Stats` record across three declared modules.

Filtering `[5, -2, 7, 0, -1, 9, 4, -3]` keeps `[5, 7, 0, 9, 4, 0, 0, 0]`.
The count is 5, sum is 25 and weighted checksum is 75, giving **5325**.
The public CLI integration test checks every final element as well as count,
sum and the final result; the checksum alone is not the sequence oracle.

```sh
oxid check fixtures/typed-array-samples/main.ox --edition typed-preview
oxid run fixtures/typed-array-samples/main.ox --edition typed-preview
oxid compile fixtures/typed-array-samples/main.ox --edition typed-preview \
  --backend llvm --output /tmp/oxid-array-samples
/tmp/oxid-array-samples
```

Use a fresh output path. Declared-child loading requires Linux. Native compilation
requires Linux x86_64 and LLVM/Clang/LLD 19.1.7, uses O0 and retains the host ELF
loader/libc dependency. The executable does not need source files or Oxid.

See [the array contract](../../rfcs/0016-fixed-scalar-arrays.md) and
[public-route validation](../../docs/architecture/fixed-array-public-validation.md).
No nested arrays, slices, heap collection, element references or v1.0 stability
is implied.
