# Typed project API documentation

Status: experimental implementation.

`oxid doc` generates `.oxid/doc/index.html` from a `typed-preview` project.
The page contains package names and versions, public module paths, function
signatures, generic parameters and bounds, struct fields and enum variants.
Private declarations, private fields, function bodies and modules hidden by
private parents are excluded. A struct with private fields says so.

`///` comments immediately preceding public declarations, fields and variants
become descriptions. Leading `//!` comments describe a module. Descriptions
are plain text and HTML characters are escaped. The page uses local styles
without external resources.

## Project and workspace configuration

The command discovers `oxid.toml` in the same way as `check`, loads the ordinary
package graph and uses the existing typed lock protocol. `--locked`, `--offline`
and `--frozen` retain their normal meanings. No lockfile is created or changed.
Dependencies are checked in the retained source session but do not become API
sections unless independently selected as workspace members.

`doc --workspace`, `doc --package NAME` and the normal workspace defaults select
members. All selected packages must typecheck before a single combined page is
published at the workspace root. An existing workspace lock still covers the
complete member graph when documentation selects only one member.

Source operands, build-profile flags and `--entry-mode` are not documentation
options. `--message-format=json` produces one `doc-summary` with the output path
and selected package count. Legacy projects retain their existing `doc` route.

## Source ownership, limits and publication

Documentation reads the already loaded AST and source bytes only after the
ordinary checker succeeds. It does not execute a program, invoke LLVM or reload
source paths. Existing manifest, graph, source, syntax and workspace limits
remain in force. The complete HTML page is limited to 32 MiB; fragments are
counted before output allocation, including escaped UTF-8 descriptions.

The output contains no absolute source paths or timestamps. Identical selected
source/configuration produces identical HTML after relocation. Selection can
replace a previous selection's page; copy a generated page to retain it.

Publication stages one regular file in `.oxid/doc` and renames it after writing
all sections. Checking, rendering, staging or rename failures preserve the
previous index. Output directories and the final index reject symlinks and
nonregular objects. A killed process can leave a staging file; the ordinary
error path removes its own staging file. Files under `docs/` are not modified.

This API page does not provide Markdown rendering, documentation tests, search,
cross-package hyperlinks or hosted documentation versions. Those tools retain
their separate development and acceptance requirements.
