# Modules

Oxid module handling should stay simple enough to predict.

## Current helpers

- `normalize_module_path`
- `module_group`
- `import_alias`
- `module_key`
- `module_resolve`
- `module_preview`
- `resolve_relative`

## Goals

- support short local imports
- support explicit aliases
- support predictable relative resolution
- keep module summaries readable
- keep the frontend, interop, and package layout aligned

## Recommended form

```text
use "core.ox";
use "frontend/parser.ox" as parse;
mod frontend;
```
