# Compiler and OXBC artifacts

Oxid supports source interpretation and ahead-of-time artifact creation from the same frontend.

```bash
oxid run app.ox
oxid compile app.ox -o app.oxb
oxid ast app.ox -o app.oxa
oxid inspect app.oxb
oxid run app.oxb
```

`oxid compile` resolves the import graph relative to each module, preprocesses source, parses every module with its own source name, orders dependencies deterministically, and serializes the combined program. `oxid ast` writes the same versioned representation with an `.oxa` extension for workflows that want to identify serialized AST explicitly.

## OXBC 1.0

An artifact starts with the `OXBC` magic followed by:

- artifact format major and minor versions (`1.0`);
- serialized AST version (`1`);
- module count;
- payload length;
- a deterministic FNV-1a payload checksum;
- the encoded program, including source ranges.

The reader rejects bad magic, unsupported versions, truncated or trailing data, checksum mismatches, excessive payloads, excessive AST depth, and excessive collection sizes. Major, minor, and AST versions must match the runtime currently; compatibility is explicit rather than silently guessed.

OXBC is not native machine code. Oxid 0.9 decodes the serialized AST and executes it in the runtime. The stable, bounded container makes artifacts deterministic and gives later bytecode/runtime stages a versioned migration point.

`oxid build` resolves dependencies, compiles the manifest entry to `.oxid/bin/<project>.oxb`, and records a build report under `.oxid/`.

