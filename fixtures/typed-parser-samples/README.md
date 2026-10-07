# Bounded scalar typed-preview parser

This Oxid-written frontend component consumes up to 128 ASCII source bytes through
the existing token interface. It parses scalar function syntax and emits a
bounded AST transcript with exact byte spans, or the first syntax diagnostic.
An independent decoder compares complete syntax facts with the unchanged Rust
parser. It does not resolve names/types or execute the parsed input.

The grammar includes public/private functions, typed parameters and results,
let/let mut, bare-name assignment, return, if/else blocks, while, break/continue,
booleans, raw decimal spelling, unit, names/calls, grouping, unary operators,
arithmetic, comparisons and logical operators. Unknown names/types and duplicate
bindings remain syntax. Else-if shorthand and tail expressions are excluded.
[CONTRACT.md](CONTRACT.md) specifies exact grammar-site refusals for modules,
imports, records, enums/match, qualified paths, references and arrays.

The implementation is currently under the shared
[typed-lexer sample directory](../typed-lexer-samples/parser_main.ox). Its explicit
loop and charged-row continuations use 128 AST rows and 129 frame slots; the
existing packed carrier is unchanged. OPA1 success is exactly 1,559 bytes and
failure is an 11-byte header, with no partial AST. The independent
[decoder](../../scripts/parser_ast_observation.py) validates ownership, links,
fields, heights, token references and spans before projection. It reconstructs
global expression postorder IDs and per-function block preorder IDs.

## Reproduce the comparison

On the qualified Linux x86_64 host, build the compiler and make LLVM/Clang/LLD
19.1.7 available through `OXID_LLVM_BIN`, then use fresh output directories:

```sh
python3 scripts/build_typed_parser_observer.py --output /tmp/parser-observer
python3 scripts/build_typed_lexer_observer.py --output /tmp/lexer-observer
python3 scripts/verify_bounded_typed_parser.py \
  --oxid target/debug/oxid \
  --parser-observer /tmp/parser-observer/canonical-parser-observer \
  --lexer-observer /tmp/lexer-observer/canonical-lexer-observer \
  --output /tmp/scalar-parser-check --native
```

The fixed roster contains 118 complete AST/first-diagnostic comparisons and
21 site refusals. Two separate transport probes reject 129 ASCII bytes and a
non-ASCII byte with status 64 and empty streams. The controller retains all
inputs, outputs, commands and identities. Forty-one malformed-observation
controls retain their historical lineage, including two exact successors when
Call and Let rows became implemented. Source-free execution means a cleared
environment and an empty working directory with copied source hidden; it is
not a filesystem sandbox or tracing claim.

## Limits and status

The completed implementation at `0bb3bbb` passes ordinary native admission with
I = 6,460, W = 2,571, 87 functions, 1,485 blocks and 63,384 explicit bytes including
the Process wrapper. The I/W headroom is 1,732/5,621; all other compiler gates
remain active. [ADMISSION.md](ADMISSION.md) preserves earlier failed X-limit
experiments and distinguishes the native inventory successor from allocation
optimization. Reference storage and language fuel remain unchanged.

The 128-byte input bound makes 64 nested statement blocks unreachable; the real
source matrix tests 19 nested If blocks, while decoder controls exercise the
separate transport contract. Prefix chains test expression depth/height limits.
[STAGE.md](STAGE.md) records implementation and review corrections. Exact-head
hosted debug/release qualification is a separate gate. Production compiler
providers, Unicode, recovery diagnostic lists and compiler self-rebuilds are
outside this component.
