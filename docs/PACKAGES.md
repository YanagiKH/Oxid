# Packages

`oxid.toml` is meant to describe the project, scripts, dependencies, and feature flags.

## Minimal manifest

```toml
[project]
name = "demo"
version = "0.1.0"
entry = "src/main.ox"

[scripts]
run = "oxid run src/main.ox"
test = "oxid test"
doctor = "oxid doctor"
doc = "oxid doc"
bootstrap = "oxid run tools/bootstrap.ox"
frontend = "oxid run tools/frontend_preview.ox"

[dependencies]

[features]
async = true
macros = true
```

## Notes

- keep script names short
- keep entry points stable
- use local paths for development dependencies
- keep package examples close to the template
