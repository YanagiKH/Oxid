# Roadmap

## Oxid 0.8

- standalone checksummed releases without a user-side Rust dependency;
- concise syntax, for-in loops, loop control, pipelines, and short functions;
- single-file `.oxb` bundles and project compilation;
- generated Python, Java, Go, C, and C++ bridges;
- Web routing/HTTP response serving and Discord interaction dispatch;
- exhaustive source/example/tool/package CI.

## Next

- versioned bytecode and serialized AST;
- long-running async network adapter API;
- structured maps/records and JSON parsing;
- source spans across imported modules;
- package lockfile and remote dependency resolver;
- benchmark harness covering cold start, parser throughput, bundle time, and runtime operations.

## Self-hosting

- Oxid-authored bytecode emitter;
- deterministic stage-0/stage-1 artifact comparison;
- incremental replacement of frontend components;
- self-hosted compiler as the default release path after cross-platform equivalence is proven.
