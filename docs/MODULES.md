# Module system

## Goals

- keep imports short
- keep resolution predictable
- keep cache keys cheap
- keep module loading readable
- make module boundaries obvious for users

## Proposed resolution rules

- resolve relative imports from the current file directory
- normalize repeated separators
- canonicalize file paths before caching
- skip duplicate loads by canonical path
- use a stable cache key per resolved module

## Suggested module layers

- `stdlib/frontend/modules.ox` for resolution helpers
- `stdlib/frontend/pipeline.ox` for module load flow
- `stdlib/cache.ox` for cache metadata
- `tools/modules_preview.ox` for user-facing previews

## Example workflow

1. resolve the module path
2. build or reuse a cache key
3. load the source once
4. preprocess or expand macros
5. parse the module
6. register exported names

## Design target

The user should be able to understand module loading without reading the Rust bootstrap first.
