# Standalone bounded byte storage pilot

RFC0031 acceptance fixture, expected result **638** from bytes
`[0, 255, 128, 255]`. `byte` is an ordinary declared function using the explicit
checked conversion, not a builtin. Indexed reads bind a named byte before
widening. The owner moves through `relay`, and slice calls borrow its whole root.

```sh
oxid check fixtures/typed-byte-storage/main.ox --edition typed-preview
oxid run fixtures/typed-byte-storage/main.ox --edition typed-preview
oxid compile fixtures/typed-byte-storage/main.ox --edition typed-preview --backend llvm --output byte-storage
```

The final command requires the existing Linux x86_64 LLVM/Clang/LLD19.1.7 O0
native environment. Native admission is narrower than source/reference admission.
This fixture is not an I/O migration: existing I/O buffers remain i32-backed.
Record-contained byte arrays, u8 arithmetic, implicit integer coercions, heap
buffers and subslices remain excluded. Qualification is pending the current
implementation, independent resource review and exact-head hosted CI; the
fixture itself is not evidence of completion.
