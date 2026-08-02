# Compiler and bundles

`oxid compile input.ox -o output.oxb` performs a deterministic source-bundle compilation:

1. resolve imports relative to each module;
2. canonicalize and deduplicate modules;
3. inline the module graph;
4. expand one-line macros;
5. lex and parse the complete program;
6. write a single `.oxb` artifact.

`oxid build` performs the same compilation for the manifest entry and writes `.oxid/bin/<project>.oxb` plus a build report.

The current `.oxb` representation is validated bundled source, not machine code. This gives fast, transparent, portable artifacts while the roadmap develops serialized AST and bytecode stages.
