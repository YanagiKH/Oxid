# Typed formatter acceptance data

Proposed RFC: [0017](../../../rfcs/0017-bounded-typed-formatter.md).

These are design fixtures, not passing test evidence. No formatter or test
runner is included. All inputs and exact expected outputs were written by hand
against the published scalar/struct/module grammar and RFC policy.

- `cases.json`: 28 exact UTF-8 positive input/output pairs and 13 malformed
  inputs with source-inspected error classes. JSON escapes make CRLF, tabs,
  Unicode whitespace and preserved trailing comment spaces unambiguous.
- `obligations.json`: public CLI, resource-boundary and invariant checks to
  implement with the formatter. Recipes are declarative; no large generated
  corpora or qualification framework is needed.
- `sources.json`: exact published-main file identities inspected for design.

For positive cases, encode `input` and `expected` as UTF-8 without newline
conversion. Run source-to-stdout formatting, both check outcomes, exact source
immutability, protected-atom/comment-anchor equivalence, normalized AST
equivalence and a second-format fixed point. The `check_input_exit` value is
0 only when input already equals expected; checking every expected output is 0.
Do not regenerate goldens from actual formatter results.

Negative cases reject the whole source before output in either mode. Existing
parser recovery can add diagnostics; require the specified relevant code/stage,
not a new fixed diagnostic count. The public exit distinction is 1 for drift
and 2 for errors.

The first implementation increment should use the real bounded lexer/parser
seams to produce one formatted string, then wire the explicit CLI route.
A few ordinary unit/public-CLI tests can consume this contract; it does not
require a new validation system or any unrelated language work.

Baseline: `17ed3de243785006ab888d87af4428d06117b21d`. No production source,
grammar or existing test was changed by this design.

