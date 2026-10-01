# Legacy 0.9 characterization

Status: experimental, partial behavioral contract. Baseline source revision:
`7e681d25fa58828b817d66a8701a6913a7158932`.

The executable checks are in [tests/legacy_semantics.rs](../tests/legacy_semantics.rs).
Run `cargo test --test legacy_semantics --locked`. Each positive and runtime-error
case runs through both `oxid run main.ox` and the production `oxid compile` /
`oxid run main.oxb` path. Expected results are fixed examples, not values computed
by the implementation under test. Seven tests expect matching results on both
paths; one explicitly records their different module initialization order. Passing them preserves these selected legacy
behaviors; it does not establish all source/artifact semantics are equivalent.

## Numbers and equality

- Numeric literals and values use IEEE-754 binary64 (`f64`), with no distinct
  integer type. `5 / 2` yields `2.5`. Integers beyond binary64's exact range may
  round; `9007199254740992 + 1` compares equal to `9007199254740992`.
- Numeric equality uses absolute difference strictly less than binary64 epsilon
  (`2^-52`). This is existing behavior, not the proposed static core's rule.
  Thus `0 == 0.0000000000000001` is true while the corresponding `<` is also
  true; a difference exactly equal to epsilon does not compare equal.
- Booleans and numbers are distinct: `true == 1` is false.
- Remainder follows the dividend sign (`-5 % 2` gives `-1`). Division and
  remainder by zero produce runtime errors. This does not define every
  overflow, NaN, infinity, or signed-zero edge case.

## Containers

Array and record assignment and parameter passing share mutable storage.
Changing an element or nested record through an alias changes the original.
`const` prevents binding reassignment, but does not freeze container contents.
Equality on acyclic arrays/records is structural and recursively uses legacy
value equality. Record JSON keys are emitted in sorted order.

Cycles, lifetime/resource behavior, and all scope interactions of `const` are
outside this characterization. Reference counting is an implementation detail,
not proof of static ownership, leak freedom, or memory safety.

## Evaluation

Callee expressions, call arguments, binary operands, array elements, and record
field values are evaluated left-to-right. Record field evaluation follows source
order even though record serialization sorts keys. The tests cover these
operations; assignment-target order and every control-flow form remain outside
this partial specification.

`and` / `or` short-circuit and return a selected operand, rather than converting
it to a boolean. Null, false, numeric zero, empty strings, empty arrays, and empty
records are falsey. Nonempty containers are truthy even if their contents are
falsey.

## Tasks

An async function call or `spawn` creates a pending task without immediately
running its body. `join_all` executes the supplied tasks sequentially in input
order. A completed task's result is reused on subsequent `join` / `await` calls.
These tests establish lazy evaluation, ordering, and memoization; they do not
establish concurrency, a scheduler, an I/O reactor, or cancellation guarantees.

## Modules and errors

The import-first example initializes its module once despite repeated imports,
before entry `main` executes. A separate test records an existing difference:
source execution runs an import at its top-level position, while artifact
compilation hoists imported module statements before the importer's local
statements. An entry that prints `before`, imports a module printing `module`,
then prints `after` produces `before/module/after` as source and
`module/before/after` as an artifact. The test uses a distinct expected result
for each path.

These cases are not a complete module resolution, cyclic import, or
initialization-order contract.

The covered runtime failures exit with status 1, no standard output, and an
`error:` diagnostic containing the responsible `main.ox` line/range and reason.
They include division/remainder by zero, invalid and out-of-bounds array indexes,
and constant binding reassignment. This is not a stable diagnostic schema or
compile-time rejection guarantee: the legacy artifact compiler accepts these
syntactically valid programs and errors occur when they are run.
