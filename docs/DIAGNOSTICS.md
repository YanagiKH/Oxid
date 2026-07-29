# Diagnostics

Oxid diagnostics are intended to be readable and source-linked.

## Desired shape

- error or warning kind
- code
- file path
- line and column
- human-readable message
- short hint
- recovery suggestion when possible

## Intent

The diagnostics layer should be usable from Oxid-owned preview modules before the Rust bootstrap layer is reduced further.
