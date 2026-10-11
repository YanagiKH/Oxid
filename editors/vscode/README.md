# Optional VS Code task template

This is an opt-in template for checking saved typed-preview source and checking
formatting without rewriting it. Merely having this directory in a checkout
does not install, activate, or run anything. This is not an extension, language
server, debugger, formatter-on-save, or v1.0 compatibility claim.

## Manual import

1. Use an already installed Oxid executable that supports the typed-preview
   commands. The template resolves `oxid` from the task environment's PATH.
   Alternatively, edit each `command` to the absolute executable path, without
   extra shell quotes, options, or a shell wrapper. No installation is provided.
2. Open a trusted local workspace folder in desktop VS Code. Tasks require a
   folder; loose-file and browser-only use are outside this template's scope.
3. Review `tasks.json`. Manually copy it to your workspace's `.vscode/tasks.json`
   only if that file does not exist. Otherwise merge its two task objects into
   your existing `tasks` array; do not overwrite existing configuration.
4. Save and activate the intended `.ox` entry file. Use **Terminal: Run Task**
   and select one of the two explicitly named Oxid tasks.

Both tasks run only when invoked. There is no folder-open trigger, background
watcher, default build task, dependency task, source execution, or compilation.
The template does not change workspace trust, editor settings, or permissions.

## Current-file restrictions

`${file}` selects the active saved file, not an automatically discovered project
entry. Save edits first; the compiler reads disk, not an unsaved editor buffer.
Choose the actual root entry when checking a declared multi-file project.
Checking a child module alone can produce different results. Formatting checks
only the active file, not its imports. Untitled buffers, non-Oxid documents,
remote URI documents, and automatic manifest entry discovery are unsupported.
Rerunning a task reevaluates the active file, so check which tab is selected.

The process tasks pass separate arguments with `--` before the file. Spaces,
shell metacharacters, and dash-prefixed names are not shell commands or options.
This does not restrict which saved file you choose. Multi-root workspace setup
and platform-specific executable resolution need manual verification.

## Results and limitations

The check task selects typed-preview explicitly and never falls back to legacy
checking. It may read explicitly declared modules but does not execute source or
create compiler output/cache files. Current declared-child loading is Linux-only;
matching Windows path strings does not establish Windows multi-file support.

Located text diagnostics are matched from a header and its immediately following
primary location. Navigation uses the line only: compiler columns count Unicode
scalars rather than editor UTF-16 units. Secondary labels, notes, and unlocated
errors remain in the terminal. Escaped control characters in unusual filenames
cannot be round-tripped by this text matcher. Multiple errors on the same line
may be collapsed by the editor; the terminal remains the complete report.
Do not interpret an empty Problems view as success; inspect the task exit status.

The formatting task is read-only `fmt --check`, not legacy `fmt`. Exit 0 means
already formatted; exit 1 means formatting drift; exit 2 means a CLI, read,
encoding, syntax, admission, invariant, or output error. All output stays in the
terminal, and errors are not reclassified as formatting drift.

Oxid already has provisional schema-v1 JSON diagnostics, including byte ranges
and a terminal check summary. This small template uses the existing text
renderer for VS Code's line-based matcher; it does not add a JSON protocol or
promise stable editor APIs. See [typed-preview source/diagnostic contract](../../spec/typed-preview.md#source-and-diagnostic-contract)
and [formatter contract](../../rfcs/0017-bounded-typed-formatter.md).

## Verification

From the repository root:

```sh
node editors/vscode/test_tasks.cjs
python3 -B -m unittest discover -s scripts -p test_vscode_tasks.py -v
```

These tests read the actual template and use Node's ECMAScript RegExp engine.
They do not run Oxid, install anything, change editor settings, or emulate the
VS Code task engine. The Python test skips with an explicit reason if Node is
absent; such a skip does not establish matcher correctness. No npm packages are
required. Real editor integration and public CLI smoke tests remain separate.

Before claiming editor validation, manually verify saved valid/invalid files,
two errors with intervening labels/notes, missing/unlocated errors, paths with
spaces and drive colons, and an emoji preceding an error. Confirm line navigation,
task exit status, no auto-run on workspace open, and unchanged source bytes after
clean/drift/error formatting checks. This proposal has not run that editor smoke.

The task shape follows the official [VS Code task guide](https://code.visualstudio.com/docs/debugtest/tasks)
and [task schema appendix](https://code.visualstudio.com/docs/reference/tasks-appendix).
