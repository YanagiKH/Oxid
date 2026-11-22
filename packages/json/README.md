# Read and write JSON

Call `validate(&bytes)` to check one UTF-8 JSON value. It accepts objects,
arrays, strings, numbers, booleans and null. A `Syntax` or `InvalidUtf8`
result reports the zero-based byte offset where validation failed.

The validator checks escaped Unicode pairs and leaves numbers in their
original format. Nested containers use a growable stack. If the configured
heap limit is reached, validation returns `MemoryLimit`.

Use `quote(&bytes)` to turn UTF-8 text into a JSON string and
`decode_string(&bytes)` to read one JSON string. Both return owned `Text` on
success. `quote` escapes quotes, backslashes and control characters.

Pass an owned `ByteBuf` to `parse` for a document you can navigate. Start at
node `0`, then use `first_child` and `next_sibling` to walk its objects and
arrays. Nodes give the byte range and kind of each value. `append_key` and
`append_value` copy those bytes into your buffer. On error, `parse` returns the
input buffer with the diagnostic.

Try the [example](../../examples/typed-json) from the repository root.

```sh
cd examples/typed-json
oxid run
oxid test
```

The example prints `1` for a valid document.
