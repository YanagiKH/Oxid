# Owned text

Build `Hello, Oxid 🦀` from two UTF-8 text values, reserve space before appending,
and move the greeting through a vector. Running the example prints `16`, its
UTF-8 byte length. The greeting contains 13 Unicode scalar values.

From this directory, with `oxid` on `PATH`:

```sh
oxid lock --offline
oxid check --frozen
oxid run --frozen
oxid test --frozen
oxid build --profile=release --frozen
.oxid/bin/owned-text
```

Development builds use `oxid build --profile=dev --frozen`. Native builds need
Linux x86_64 and LLVM 19.1.7. The built executable can be copied elsewhere and
run without Oxid or the source files.

[`src/main.ox`](src/main.ox) constructs text from borrowed bytes with
`from_utf8`, then joins text with `append_text`. Both report allocation errors
explicitly. `Text` keeps its valid UTF-8 bytes private. It can move into and out
of a vector; popping a text gives its ownership back to the caller.

[`tests/text.ox`](tests/text.ox) checks text ownership through vector moves and
enum alternatives. The library's broader UTF-8 checks run in
[`utf8-normalize/contracts`](../../apps/utf8-normalize/contracts).
