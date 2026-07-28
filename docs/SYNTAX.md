# Syntax direction

This file separates the current syntax from planned simplifications.

## Current syntax already supported by the runtime

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
- `print`

## Planned Oxid-first simplifications

These are proposals for future language growth.

- shorter function declarations
- lighter module import forms
- pipeline-style chaining
- pattern matching
- typed record literals
- clearer error propagation syntax
- optional one-line forms for tiny helpers

## Proposed direction

- `pub fn` for exported declarations
- `mod name` for local module grouping
- `use path as alias` for shorter imports
- `match value { ... }`
- `try expr`
- `defer expr`
- `pipe value |> step`

These examples are roadmap targets, not parser guarantees yet.

## Compatibility rule

Only add syntax when it is:

- shorter than the equivalent old form
- obvious to read at a glance
- easy to format consistently
- safe to parse without making the grammar brittle
