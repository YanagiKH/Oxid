# Self-hosting

Oxid 0.9 establishes a deterministic, incrementally replaceable bootstrap path. Release users install a standalone `oxid` binary and do not need Rust to write, interpret, compile, inspect, package, or run Oxid programs. Building the stage-0 implementation itself from source still requires Rust and a C/C++ compiler.

## Implemented path

1. The stage-0 frontend compiles `stdlib/frontend/bytecode.ox` and `compiler/main.ox` into OXBC 1.0.
2. The Oxid-authored emitter accepts normalized instruction records and emits a canonical bytecode stream.
3. Bootstrap decodes and re-encodes the compiler artifact for stage 1 and stage 2.
4. Stage 0, stage 1, and stage 2 must be byte-identical.
5. The decoded compiler surface is executed by the runtime.
6. `compiler/providers.toml` must declare the Oxid emitter, stage-0 frontend providers, and required parity.

Running without `--check` writes:

```text
.oxid/bootstrap/stage0.oxb
.oxid/bootstrap/stage1.oxb
.oxid/bootstrap/stage2.oxb
.oxid/bootstrap/compiler.oxb
.oxid/bootstrap/manifest.json
```

The manifest records artifact/AST versions, compiler version, module count, checksum, and parity results.

## Commands

```bash
oxid bootstrap
oxid bootstrap --check
oxid self-compile --check
oxid emit --check
oxid self-host --check
oxid frontend
```

## Current limitation

The equality check proves deterministic serialization and a stage artifact fixed point using the current bootstrap codec. CI additionally requires those artifacts to be byte-identical across Linux, Windows, macOS x86_64, and macOS arm64 before release packaging. This is not yet proof that an independently implemented Oxid lexer/parser produced the same compiler. The emitter is Oxid-authored; lexer, parser, diagnostics, and module providers remain stage-0. The self-hosted path must not become the default compiler until those providers are replaced and independent compiler parity is demonstrated across release platforms.
