# Packages

`oxid.toml` supports the following project-level fields:

- `project.name`
- `project.version`
- `project.entry`
- `[scripts]`
- `[dependencies]`
- `[features]`

## Minimal manifest

```toml
[project]
name = "demo"
version = "0.1.0"
entry = "src/main.ox"

[scripts]
run = "oxid run src/main.ox"
test = "oxid test"
fmt = "oxid fmt"
doctor = "oxid doctor"
doc = "oxid doc"

[dependencies]

[features]
async = true
macros = true
```

## Script entries

Scripts are plain command strings. They are meant for repeatable tasks such as running the entry file, formatting sources, or generating docs.

## Dependency entries

`oxid add <name> <target>` updates the dependency section automatically.

Typical targets are:

- local paths such as `./packages/demo`
- sibling repositories checked out locally
- future package registry identifiers

## Entry resolution

If `project.entry` is missing, the runtime falls back to:

1. `src/main.ox`
2. `main.ox`

That makes the package layout simple for new projects while still supporting custom entry points.

## Recommended package layout

```text
project/
├── oxid.toml
├── README.md
├── src/
│   ├── main.ox
│   └── lib.ox
├── stdlib/
├── examples/
└── tests/
```
