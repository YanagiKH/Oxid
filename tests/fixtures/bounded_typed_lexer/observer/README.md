# Canonical bounded lexer observer

Build from the repository root with a Rust compiler available on `PATH`:

```sh
python3 scripts/build_typed_lexer_observer.py --output /tmp/oxid-lexer-observer
printf 'fn main() {}' | /tmp/oxid-lexer-observer/canonical-lexer-observer
```

The output directory must not already exist. The builder copies the checkout's
`lexer.rs`, `diagnostic.rs`, `source.rs`, and `project/budget.rs` unchanged into
that directory and compiles them directly with `rustc`. It does not run Cargo or
change compiler sources. Source hashes, the source commit, wrapper hashes,
toolchain version, exact compiler command, build logs and artifact hashes are
retained beside the executable. A failed compile retains its evidence there.
The source commit identifies the checkout; source hashes identify the exact
copied working-tree bytes, including any local edits.

The wrapper reads at most 129 bytes and admits ASCII input of at most 128 bytes.
Out-of-domain input exits 2, emits a wrapper error on stderr and emits no token
result. Admitted input produces one JSON record and exits 0:

- `status: ok` includes every token, including trivia and EOF, with fields
  `id`, `kind`, `file_id`, `start` and `end`
- `status: diagnostic` includes the real canonical diagnostic's JSON record
  under `diagnostic`; the lexer API does not return partial tokens on failure

Token spans use half-open byte offsets. Token IDs are canonical enum
discriminants plus one, fixed by the 47 implicit unit variants in the source
manifest. They describe this observation protocol, not stable IDs promised by
future compiler versions.
