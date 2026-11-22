# Streaming UTF-8

Decode UTF-8 from a file or network stream a piece at a time. The decoder keeps
track of an unfinished character even when its bytes arrive in different reads.

Add the package to your `oxid.toml`.

```toml
[dependencies]
stream = "../../packages/utf8_stream"
```

Create a decoder with `new_decoder()`. For each chunk, call
`next(&mut decoder, &chunk, 0)` and use its result.

- `Read::Character(character)` yields a Unicode character and its byte offset
  from the start of the stream. To keep reading the same chunk, pass
  `character.next_in_chunk` as the next cursor.
- `Read::NeedInput` means the entire chunk has been read. Pass the next chunk
  with a cursor of zero.
- `Read::Invalid(offset)` identifies the start of an invalid character.
  `Read::TooLong` reports a stream longer than the supported byte offset.
  Both results leave the decoder stopped.
- `Read::InvalidCursor` means the cursor falls outside the chunk. Retry with a
  valid cursor.

At the end of the stream, call `finish(&decoder)`. It reports a complete
stream, a truncated character with its byte offset, or the previous error.

[Example](../../examples/typed-utf8-stream/src/main.ox)
