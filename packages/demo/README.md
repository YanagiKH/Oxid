# Demo Oxid Package

This folder shows a user-facing Oxid package layout.

## Layout

- `src/main.ox` contains the application entry.
- `src/lib.ox` contains reusable package helpers.
- `src/interop.ox` contains a bridge preview entry.
- `tests/smoke.ox` contains a smoke test.
- `oxid.toml` contains package metadata and scripts.

## Useful commands

```bash
oxid script run
oxid script test
oxid script fmt
oxid script doctor
oxid script doc
oxid script clean
```

## Goal

This package is designed to be copied, edited, and used as the starting point for Oxid-based development.
