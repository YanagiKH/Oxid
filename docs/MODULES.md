# Modules

Oxid module helpers should keep resolution simple and predictable.

## Goal

- normalize paths
- resolve relative imports
- keep a stable module key
- avoid scattering path logic across the repo

## Outcome

Module preview helpers should be reusable from command previews, self-host documentation, and package workflow scripts.
