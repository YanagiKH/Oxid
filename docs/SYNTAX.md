# Oxid-first syntax direction

Each new form should be shorter than the equivalent verbose form, easy to read aloud, and easy to format consistently.

## Target forms

- `pub fn` for exported declarations
- `mod name` for local grouping
- `use path as alias` for shorter imports
- `match value { ... }`
- `try expr`
- `defer expr`
- `pipe value |> step`
- compact one-line helpers
- typed record literals
- clearer error propagation forms

## Example direction

- `pub fn run()`
- `mod parser`
- `use stdlib/frontend/parser.ox as parse`
- `match token { ... }`
- `try load()`
- `defer close()`
- `value |> step()`

These are preview targets for the migration path.
