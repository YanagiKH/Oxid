# Experimental streaming lexical component

These five genuine Oxid modules implement ASCII tokenization with a rolling
128-cell input buffer, exact keyword state, bounded output buffer and full-width
contextual byte spans. They use only existing i32, fixed-array/record, borrow and
Process I/O semantics. There is no whole-source/token array or heap collection.

Build a closed local bundle:

```sh
python3 scripts/build_streaming_lexer.py --compiler /path/to/oxid \
  --llvm-bin /path/to/llvm-19/bin --output /new/lexer-build
oxid check project/main.ox --edition=typed-preview \
  --experimental-lexical-provider /new/lexer-build/bundle
```

The builder checks every digest in sources.json before copying the exact module
closure. `--provider /existing/lexical/bundle` rebuilds this same source through
the real lexical-provider loader route. Every output directory must be fresh.

The [LXI1/LXS1 contract](../../spec/streaming-lexical-provider.md) preserves tokens,
trivia, EOF and lexical diagnostic spans. Deterministic carry padding is checked
exactly. Token offsets combine explicit u32 chunk bases with bounded relative
endpoints; total source positions are not byte-wide.

The Rust compiler retains the source, checks the returned stream and compares
against its canonical lexer, then moves the producer's token allocation into its
real parser. Parsing, resolution, ownership, OIR and backend authority remain
Rust-controlled. Provider failure has no fallback. Default selection and v1/v2
HIR protocols remain unchanged.

The [qualification ledger](../../docs/architecture/streaming-lexical-provider.md)
distinguishes component use from self-hosting. The full 31-module v2 parser/static
source union and this five-module lexer are the actual qualification inputs.
Admission and fuel ceilings remain unchanged; especially long tokens can exhaust
fuel before a lexical terminal. Unicode and a Rust-free compiler rebuild are
outside this increment.
