# RFC 0001: an opt-in bool/unit checker

Status: experimental implementation proposal, reviewed with its pull request.
This is a provisional check-only contract, not acceptance of a stable edition,
complete language grammar, ownership model or ABI. Owner/reviewer assignment
remains unassigned in the feature inventory until recorded by maintainers.

## Motivation and bounded decision

The legacy interpreter uses dynamic values, f64 literals, and an AST without
function type annotations or general expression spans. Reinterpreting that AST
as a typed core would lose information and risk changing existing programs.

Introduce `typed-preview` as an explicit, additive selector for `oxid check`.
Keep all default behavior on `legacy-0.9`. Use an independent source/token/AST →
resolved HIR → typed HIR path for bool/unit functions, locals, direct calls and
terminal returns. The exact grammar, diagnostics, option handling and resource
bounds are in [the preview specification](../spec/typed-preview.md).

The review decision is limited to shipping that experimental behavior. Final
edition naming, numeric rules, ownership, modules, control flow, ABI, execution,
artifacts and LLVM remain separate decisions. Nothing in this proposal changes
the full project roadmap or declares a milestone complete.

## Compatibility and alternatives

First extract the legacy syntax mechanically, retaining runtime/artifact imports
and proving representative artifact-byte parity. Then add the new frontend and
an early CLI gate. Unsupported typed operations cannot reach any legacy effects.
The old Cargo package, toolchain version and OXBC 1.0 payload remain unchanged.

Retrofitting the legacy AST was rejected because it cannot retain exact numeric
spelling or typed syntax. A full runtime/CLI rewrite was rejected as unnecessary
regression risk. Direct AST-to-LLVM emission would bypass the required analysis
boundary and is outside this proposal. Temporary duplicated lexer/parser code
is a deliberate compatibility cost.

## Trust, performance and evidence

The checker never loads modules, expands macros, executes user code, resolves
packages, calls host builtins or emits artifacts. It reads one bounded source
and uses bounded tokens/nodes/nesting/diagnostics. This is not an OS sandbox.
It makes no performance, memory-safety or production-readiness claim.

The public CLI tests cover accepted programs, compile-fail reasons and ranges,
legacy parity, unsupported commands before side effects, malformed flags,
encoding/artifact failures and resource limits. Unit tests verify lossless token
ranges, source coordinates, diagnostic escaping, resolved IDs and complete
expression/local type tables. [The validation report](../docs/architecture/typed-preview-validation.md)
records actual commands and host coverage; missing targets remain unverified.

## Follow-on boundary

The separately bounded [RFC 0002](0002-verified-straight-line-oir.md) adds
verified straight-line OIR without new source syntax. Boolean branches, scopes
and joins remain a later increment before ownership or native code. Exact integer literals and reference execution need a separate numerical
contract; LLVM needs a specified target/toolchain/ABI. None may silently adopt
legacy semantics when unsupported.
