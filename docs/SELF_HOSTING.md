# Self-hosting plan

Oxid 0.8 separates user-facing language use from the stage-0 implementation. Release users do not install Rust; they install a standalone Oxid binary and work with `.ox` and `.oxb` files.

## Completed boundary work

- standalone release binaries and checksums;
- Oxid-authored standard library, examples, tools, package workflows, Web, Discord, and interop modules;
- real parser support for concise Oxid syntax;
- deterministic import bundling and syntax validation;
- unit and repository-wide bootstrap verification;
- isolated native C/C++ ABI code.

## Next bootstrap stages

1. Define a versioned serialized AST and bytecode format.
2. Implement the emitter and deterministic serializer in Oxid.
3. Compile the Oxid frontend with stage-0 and execute it on the runtime VM.
4. Compare stage-0 and stage-1 artifacts byte-for-byte where deterministic.
5. Replace lexer, parser, diagnostics, and module resolution one verified component at a time.
6. Keep a small recovery bootstrap and native platform boundary.

## Success criteria

- a clean checkout reproduces the same stage-1 compiler artifact;
- stage-1 compiles every repository source and its own source;
- cross-platform CI compares bootstrap outputs;
- normal development and release builds no longer compile the stage-0 frontend.
