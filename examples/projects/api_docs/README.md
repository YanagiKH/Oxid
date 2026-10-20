# Typed API documentation example

An ordinary typed-preview workspace for `oxid doc`. The `api-docs-geometry`
member has no `main`; it declares public and private modules, a private module
with a public child, public and private structs, fields and enums, borrowed,
fixed-array and slice parameters, imported aliases and comment-attachment
cases. Its `name_rules` module pairs a local type with a same-spelled function
alias, and a type alias with a same-spelled value alias. It depends on the
unnamed local path package `geometry/vendor/units`, which deliberately repeats
the spellings `Point` and `area`. The `api-docs-app` member depends on
`geometry` and has a `main` that documentation never runs.

From this directory:

```sh
oxid doc --manifest-path oxid.toml --package api-docs-geometry
oxid doc --manifest-path oxid.toml --package api-docs-app
oxid doc --manifest-path geometry/vendor/units/oxid.toml
```

Each command checks the selected package and its dependencies, then prints
Markdown for the selected package only. Nothing is executed and no file is
written; redirect stdout to keep a copy. Comments in `geometry/src/notes.ox`
show which `///` lines attach. The exact expected documents are frozen in
`tests/typed_project_doc.rs`. Typed-preview is experimental.
