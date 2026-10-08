# Self-hosting status and artifact round-trip

Oxid 0.9 does not independently compile its compiler. Release users can run the
standalone Rust-built `oxid` executable without installing Rust; rebuilding that
host implementation still requires Rust and a C/C++ compiler. These are separate
capabilities.

## What the existing command verifies

1. The Rust frontend parses `stdlib/frontend/bytecode.ox` and `compiler/main.ox`.
2. Rust's OXBC writer serializes that AST to the file historically called stage 0.
3. The same Rust codec decodes and re-encodes it twice, producing stages 1 and 2.
4. All three serialized artifacts must be byte-identical.
5. The runtime interprets the decoded compiler demonstration, which checks a
   fixed representative textual instruction sequence.
6. Required text is checked in `compiler/providers.toml`; this does not dispatch
   compiler providers or prove that a declared provider was used.

The Oxid-authored textual emitter is not connected to `compile_file`'s binary
OXBC writer. Neither the demonstration nor serialization equality proves that
an Oxid compiler can compile arbitrary supported source or rebuild itself.

## Compatible commands and files

Existing command names and outputs are retained:

```bash
oxid bootstrap
oxid bootstrap --check
oxid self-compile --check
oxid emit --check
oxid self-host --check
oxid frontend
```

`bootstrap`, `self-compile`, `emit`, and `self-host` all enter the same artifact
round-trip path. Without `--check`, that path writes:

```text
.oxid/bootstrap/stage0.oxb
.oxid/bootstrap/stage1.oxb
.oxid/bootstrap/stage2.oxb
.oxid/bootstrap/compiler.oxb
.oxid/bootstrap/manifest.json
```

The manifest's version/count/checksum and equality fields describe serialized
artifacts. They are not compiler provenance or independent self-hosting evidence.
`--check` skips writing those output artifacts; repository source preprocessing
may still use its ordinary cache.

CI compares the serialized compiler demonstration across its configured host
matrix. This is useful portable-format regression coverage. It is distinct from
comparing native binaries built for the same target.

## Required future self-hosting evidence

Real self-hosting requires production provider dispatch with execution tracing
and no hidden stage-0 fallback; a compiler capable of arbitrary supported inputs;
a seed producing C1; C1 rebuilding the same compiler source as C2; and C2
producing C3. C2/C3 comparison must hold with the same target, dependencies, and
options. A clean environment without Rust/Cargo must rebuild the designated
compiler and standard library from a published Oxid seed. None of those gates
is satisfied by the current serialization round-trip.

## Explicit lexical component use

The experimental [source-scale lexical route](architecture/streaming-lexical-provider.md)
now provides a separate actual provider seam. An Oxid streaming lexer supplies
source-bound tokens to the real typed-project parser, with per-module execution
receipts, explicit canonical Rust lexical comparison and no fallback. The target
is the complete existing v2 producer source closures and the new lexer itself.
Parsing, semantic analysis, ownership, OIR and backend authority remain Rust-owned.
This component-use gate does not satisfy the C1/C2/C3 or Rust-free rebuild gates
above; the legacy providers.toml substring check is unchanged.
