# Typed project formatting

This describes an experimental developer-tool extension, not a v1.0 release
qualification. Single-file `fmt` retains its existing stdout/check behavior.

## Explicit project selection

```
oxid fmt --manifest-path app/oxid.toml --check
oxid fmt --manifest-path workspace/oxid.toml --package app --write
```

Project formatting requires exactly one of `--check` and `--write`. It never
implicitly overwrites files. The existing typed manifest/workspace selection
rules apply; this does not infer an edition from source syntax or change legacy
formatting. Backend, engine, target, execution policy, optimization profile,
JSON output and explicit source operands are not project-formatter options.

The formatter uses the existing immutable compiler session and its source
loading rules, but does not type-check, lower, build or execute the program.
Consequently source/manifest loading and syntax errors can prevent formatting;
semantic compilation errors do not by themselves prevent formatting.
Dependencies can be loaded by the existing session, but only modules belonging
to the selected package are formatting targets. Only selected-package source pathnames are targeted; dependency path overlaps
loaded in the session are refused in write mode. Unselected workspace packages
are not enumerated or formatted, but an unloaded member can declare the same
pathname as a selected file. Such aliases are not independently detected or
isolated. This is not recursive discovery of every `.ox` file under a directory.

## Results

- Exit 0: the selected files are already formatted (`--check`), or the requested
  writes completed (`--write`).
- Exit 1: `--check` found at least one file requiring formatting.
- Exit 2: command selection, source loading, formatting or filesystem I/O failed.

Project output is a report, not concatenated source code. Diagnostic paths must
be escaped for terminal display. There is no native-toolchain requirement.

## Explicit writes

All selected source files are formatted before any replacement is attempted.
Changed contents are staged into newly created temporary files in the original
files' directories. Staging preserves original permissions and synchronizes
file contents before replacement. Write mode refuses symbolic links in the selected source path spelling,
read-only files, and source paths also owned by a loaded dependency; check mode
retains the existing loader's policy. These are pathname checks, not a global
filesystem alias inventory.

Before replacement, the original bytes must still match the loaded snapshot.
They are checked collectively before the first replacement and again before
replacing each file. A mismatch is an error, not permission to overwrite newer
edits. Each replacement uses the platform's rename operation, without a
remove-original fallback.

This is **not a multi-file transaction** and **not an adversarial-filesystem
race defense**. Another process can still change a file after the final check.
If a later replacement fails, earlier replacements may already have succeeded;
the error reports partial progress. It does not restore stale originals over
concurrent edits. Handled failures clean up remaining staging files where
possible. Process termination and storage failures can leave staging files;
this feature makes no whole-project durability or rollback guarantee.

## Evidence boundary

Focused implementation tests and CLI integration tests are required for this
extension. Their results must identify the exact source revision. Adding this
command alone does not satisfy the complete G6 tooling gate, certify all
platforms, or establish v1.0 release readiness.
