# Bounded owned record composition sample

This experimental typed-preview example declares a Batch containing a nominal
Meta record and a fixed i32 array across three modules. Model declarations use
an acyclic forward reference. Constructors deliberately initialize samples
before meta while physical fields remain in declaration order.

Whole values move through relay; forward explicitly reborrows the entire Batch.
The loop updates contained scalar leaves and inspect borrows the complete root
shared. Starting [1,2,3] and completed=0 gives [2,3,4] and completed=3, returning
**324** (`3 * 100 + 2 * 10 + 4`). No subobject becomes a separate owner or loan.

```sh
oxid check fixtures/typed-record-composition-samples/main.ox --edition typed-preview
oxid run fixtures/typed-record-composition-samples/main.ox --edition typed-preview
oxid compile fixtures/typed-record-composition-samples/main.ox --edition typed-preview \
  --backend llvm --output /tmp/oxid-record-composition
/tmp/oxid-record-composition
```

The reference runner and successfully compiled program print `324`. Use a fresh
output path. Linked loading requires Linux; native compilation requires Linux
x86_64 with LLVM/Clang/LLD 19.1.7, retaining the host ELF-loader/libc dependency.
This is not a stable ABI, completed milestone, soundness proof or v1.0 claim.
See [RFC0020](../../rfcs/0020-owned-record-composition.md), the
[specification](../../spec/typed-preview.md#bounded-owned-record-composition), and
[public acceptance controls](../../tests/typed_record_composition.rs).
