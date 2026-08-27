# Self-compiling workflow

The repository contains an executable Oxid compiler surface in `compiler/main.ox` and `stdlib/frontend/bytecode.ox`, with provider selection in `compiler/providers.toml`.

Use `oxid bootstrap` to generate deterministic artifacts, or add `--check` in validation environments where no output should be written. `oxid self-compile`, `oxid emit`, and `oxid self-host` currently enter the same parity-gated bootstrap implementation.

The provider manifest makes replacement explicit:

- `emitter = "oxid"` is active;
- lexer, parser, diagnostics, and module providers remain `stage0`;
- `parity_required = true` prevents an unchecked provider transition.

Incremental frontend replacement should change one provider at a time, add behavior and artifact parity coverage, and keep the stage-0 recovery path until independent cross-platform builds agree.

