# Oxid-first syntax direction

This file describes the planned simplifications for the authoring surface.

## Target forms

- `pub fn` for exported functions
- `mod name` for local module grouping
- `use path as alias` for concise imports
- `match value { ... }`
- `try expr`
- `defer expr`
- `pipe value |> step`
- short one-line helper forms for tiny functions

## Design rule

Every new form should be shorter than the equivalent verbose form, easy to read aloud, and easy to format consistently.

## Notes

The repository now includes preview files that organize these ideas into front-end modules, command tools, and documentation. Full parser-level support still belongs to the compiler migration path.
