# Self-compiling workflow

Oxid is designed to keep the Rust edge as small as possible while moving workflow logic into Oxid source files.

## Current workflow surface

- `tools/bootstrap.ox`
- `tools/compile.ox`
- `tools/self_compile.ox`
- `tools/self_host.ox`
- `stdlib/bootstrap.ox`
- `stdlib/compiler.ox`
- `stdlib/self_compile.ox`
- `stdlib/self_host.ox`

## Goal

The self-compiling path should make the tooling surface build, inspect, and describe itself through Oxid code first, with Rust acting only as the fallback boundary.

## Practical entry points

```bash
oxid bootstrap
oxid compile
oxid self-compile
oxid self-host
oxid script bootstrap
oxid script compile
oxid script self-compile
oxid script self-host
```

These commands and scripts run the Oxid tool previews directly from the repository and are the quickest path for users who clone the project and start writing Oxid code immediately.

## Success criteria

- bootstrap summaries stay readable
- compile snapshots explain the pipeline
- self-compile covers the tooling surface itself
- self-host remains a thin compatibility edge
