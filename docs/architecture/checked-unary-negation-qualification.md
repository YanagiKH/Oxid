# Checked unary negation qualification successor

This qualification-only successor binds source checkpoint
`bf48512acf86e2d23c28b6b9b16de3be3d127051`, source tree
`715d047f37db8b7658bda688ff4e5961609193f8`. It changes no production code or frozen
historical oracle. Language semantics and exact source/native validation are
specified in RFC 0021 and `checked-unary-negation-validation.md`.

## Bounded compatibility changes

- Current input admission has 199 members, including 143 `src`/`native` files
  (146 compiler bodies including Cargo manifests and build.rs). A pinned 25-path
  unary inverse restores all 196 record-composition inputs before the historical
  inverse chain. The three added source files are unary regression test modules
- The owned-source CLI selector `checked-unary-negation-v1` retains the four
  division/record amendments and adds exactly `scalar-general-minus` → i32 -3
  and `scalar-minus-group` → i32 -1. Each row binds its original source and
  predecessor expectation. The original model, corpus and historical model tests
  remain unchanged
- The boolean verifier has an explicit
  `--expectation-amendment checked-unary-negation-v1` selector: `-(1)` in a
  boolean return context reaches type checking and emits E0300. The default
  historical expectation remains intact. The current native-suite dispatcher
  supplies the selector
- Current parser instrumentation restores only its exact AST/parser overlap
  before composing historical instrumentation. All 248 frozen parser sources
  were inspected: their only minus bytes are return arrows, so none needs a
  unary semantic expectation amendment. The complete parser comparator remains
  unchanged
- Record-composition observer controls use an explicitly admitted, reconstructed
  composition predecessor view. Their frozen modules, assertions and sealed
  packages are unchanged. These checks continue to test the archived adapters;
  they do not claim current unary runtime observation
- Current declaration extraction still reads the actual committed source and
  checks current source hashes. The derived declaration probe remains byte-for-
  byte identical to its independently derived predecessor probe. The explicit
  provenance amendment accounts for four changed source files and the four-line
  shift of two declarations

## CI and verification boundaries

The existing source-admission, Unit2, ordinary Python, current public/lifecycle,
parser, hosted-root and native-suite CI entry points remain active. The owned
CLI gate selects the cumulative unary amendment. The public lifecycle patch and
frozen historical comparators are unchanged. No historical failure is erased or
reclassified as a current pass.

Use `tests/fixtures/typed_project_source_binding/run.py preflight` for admission,
`run-unit2` for actual current semantic/resource replay, and the unchanged current
Unit4 gate CLI for public/parser execution. Source admission and adapter controls
alone do not establish semantic qualification. Local Linux results cannot prove
other-host or hosted exact-head CI success; merge eligibility requires all
applicable CI checks on the actual published head.
