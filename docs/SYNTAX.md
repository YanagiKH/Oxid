# Syntax direction

This file separates the current language surface from planned simplifications.

## Current syntax already in the runtime

- `fn`
- `let`
- `const`
- `if`
- `while`
- `return`
- `use`
- arrays
- indexing
- function calls
- `async fn`
- `await`

## Planned simplifications

These are targets for future language growth.

- shorter function declaration forms
- lighter module import forms
- pipeline-style chaining
- pattern matching
- typed record literals
- clearer error propagation syntax
- optional block-less one-line forms for tiny helpers

## Recommended compatibility rule

Add new syntax only when it is:

- shorter than the equivalent old form
- obvious to read at a glance
- easy to format consistently
- possible to explain in one sentence
- safe to parse without making the grammar brittle

## Proposal examples

- `let x = expr`
- `fn name(args) -> result`
- `match value { ... }`
- `try expr`
- `defer expr`
- `pipe value |> step`

These examples are proposals for the roadmap, not parser guarantees yet.
