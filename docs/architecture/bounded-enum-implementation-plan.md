# Bounded enum implementation plan

Design gate: RFC 0023 accepted by the coordinating project owner on 2026-10-05.
Save this accepted contract before production semantic edits.

1. Inspect current carrier layouts, ledgers, CFG/scalar definition rules and
   whole-owner transfers. Measure compact candidate representations with a tiny
   standalone probe; preserve existing actual-carrier size regression tests.
2. Freeze exact sum layout, match raw proof, fuel and resource contract. Obtain
   explicit coordinator acceptance before production edits.
3. Add checked nominal enum inventory/layout and independently rejected malformed
   declarations. Preserve old limits and existing record identities. Add raw
   construction/match/transfer vertical slice in existing owned OIR, paired
   reference and native consumers, exact fuel and malformed-IR controls.
4. Add source grammar/formatting, shared project declaration/type resolution,
   owned HIR/typeck and lowering. Gate unsupported contexts explicitly. Test
   source ownership joins and loop edges, bool/i32/unit/nullary payload cases,
   resource denials and scanner pilot. Activate public typed routes together.
5. Integrate ordinary debug/release tests, formatting/lint/Python/repository
   checks and source-free native execution. Independent review, fixes and fresh
   integrated reruns. Record feature evidence without modifying frozen oracles.
6. Separate current qualification successor bound to the actual source identity;
   root handles remote publication, hosted exact-head CI and merge decision.

Disk: 1.9 GiB free observed at setup. No heavy build started. Use one bounded
nonincremental debug-info-free target; keep evidence and required executables.
Coordinate target allocation with root before builds. Save design and thin
checkpoints early. Stop rather than bypass a qualification failure or alter
historical evidence. Never perform unrelated unpublished remaining40 work.

Likely write surfaces: frontend lexer/parser/AST/formatter/project/declaration
index; owned source HIR/typeck/inventory/lowering; checked owned types; raw shape,
CFG and flow; sealed plans; reference/native consumers; focused fixtures/specs.

Highest risks: canonical binary dispatch and variant-safe scalar consumption; accidental
static-leaf reads of inactive payloads in call/return/move paths; growth of
ubiquitous enum carriers and uncharged nested vectors; legacy fuel drift;
continued-arm joins and loop restoration; path splitting without a second symbol
resolver. Each needs independent negative controls, not producer-only checks.


## Thin implementation checkpoints

A. Checked enums: IDs, declarations, scalar-only payload validation, fixed layout,
   combined caps and size/ledger tests. No public activation or semantic claim.
B. Raw pilot: ConstructEnum, canonical MatchDecl/binary dispatch/ConsumeVariant,
   source-independent malformed raw tests and whole-value reference transfers.
   Add native tag-safe transfers/dispatch and exact operation-fuel parity.
C. Source pilot: parser/formatter and shared declaration resolution, owned source
   typing/lowering. Add public check/run/native activation only when raw consumers
   reject malformed variants independently. Add multi-module scanner example.
D. Feature acceptance: compile-fail matrix, loop/join/all-path-return, inactive
   storage mutations, every-fuel cases, actual enclosing sizes, independently
   recomputed resource inventory and allocation failures; ordinary regressions.
E. Separate qualification: source identity binding, independent review and native
   source-free execution evidence; root owns publication/exact-head CI/merge.

The tiny candidate-layout probe is saved at /tmp/oxid-enum-design with source,
compiler version and stdout. Measurements: aggregate carrier 16 -> 16 bytes,
compact owner slot 8 -> 8 bytes, u32-tag/four-byte-payload 8 bytes, alignment 4.
No Cargo build has run. Actual source-carrier measurements remain checkpoint A/B
work, not evidence already established by the candidate probe.
