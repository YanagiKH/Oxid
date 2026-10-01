# Artifact round-trip (legacy bootstrap commands)

`oxid bootstrap` currently validates serialized compiler-demonstration artifacts.
It does not run an Oxid compiler rebuilding itself.

```bash
oxid bootstrap          # round-trip check and write .oxid/bootstrap artifacts
oxid bootstrap --check  # skip writing the output artifacts
```

The command uses repository compiler-demonstration sources when available and
otherwise uses sources embedded in the Rust-built release binary. It checks
required provider-manifest text, serializes the parsed AST, decodes/re-encodes it
twice with the same codec, checks byte equality, and interprets the demonstration.

Deterministic output depends on normalized module ordering, stable record
ordering, versioned AST serialization, and platform-independent encoding.
Cross-platform artifact comparison is a format regression gate; it is not
sufficient to retire stage-0 providers. See [self-hosting requirements](SELF_HOSTING.md).
