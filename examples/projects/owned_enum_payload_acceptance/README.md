# Owned enum payload acceptance (R2 companion)

Independent end-to-end fixtures for owned enum payloads, driven only through the
public `oxid` CLI by `tests/owned_enum_payload_acceptance.rs`. The test copies
this directory to a fresh temporary directory; nothing is built in-tree.

Fixed expectations (written before execution):

| Project | Shape | `check` | stdout | exit | stderr |
| --- | --- | --- | --- | --- | --- |
| `ready` | path dependency `lib = "../library"`; public `Outcome::Ready(Reading)` with private-field `Reading`; moved through `forward`/`hold`, returned, consumed by `settle` | 0 | `42\n` | 0 | empty |
| `failed` | same flow with `Outcome::Failed(-7)` | 0 | `-7\n` | 0 | empty |
| `values` | direct and grouped function-result payloads: array, nested enum, zero-size record/array, unit; replacement and discard | 0 | `42\n` | 0 | empty |

Each value holds for `run --engine interpreter`, `run --engine native`, and the
standalone executable from `build` (result mode,
`.oxid/typed-preview/result/<name>`) run with an empty environment from `/`.

`values` total: pair 14 + grouped pair 14 + nested 6 + grouped nested 6 +
empty record 1 + grouped empty array 2 + unit 3 + replaced `Failed(-4)` = 42.

Negative sources live in the test; each `check --message-format=json` exits 1
with exactly one diagnostic of the fixed code/stage/line and no `E0500`:

| Case | Code/stage |
| --- | --- |
| moved-use of a path-dependency enum | E0310/ownership |
| nominal type mismatch (same-shape record from a call) | E0300/type |
| private field read from an extracted payload | E0206/type |
| unused direct by-value cycle | E0202/resolve |
| unused mutual by-value cycle | E0202/resolve |
