# Frontend

The frontend preview is a compact summary of the language pipeline.

## Preview stages

- lex
- parse
- AST
- recovery
- diagnostics
- modules
- syntax
- emission
- linting

## Why it matters

Keeping the frontend surface visible in Oxid source makes the future self-host migration easier. The preview files are intentionally lightweight and should stay readable.
