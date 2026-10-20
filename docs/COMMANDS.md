# Oxid commands

Run `oxid --version` to check the installed version, or `oxid help` for built-in help. Commands below describe the 0.9 toolchain.

## Run and package

| Command | What it does |
|---|---|
| `oxid run <file.ox\|file.oxb\|file.oxa>` | Execute source or load and interpret an artifact |
| `oxid <existing-file>` | Shorthand for `run` |
| `oxid check <file>` | Parse source or validate an artifact without executing it |
| `oxid compile <file.ox> [-o output.oxb]` | Package the module graph as OXBC 1.0 serialized AST |
| `oxid ast <file.ox> [-o output.oxa]` | Write the same artifact representation with an `.oxa` extension |
| `oxid inspect <artifact>` | Print versions, counts, and checksum |
| `oxid repl` | Start the interactive interpreter |
| `oxid watch <file.ox>` | Rerun after project files change |

The default legacy `check` does not type-check or enforce ownership. Legacy `compile` does not emit native machine code. See [artifact format](COMPILER.md) and [module ordering](MODULES.md).

## Experimental typed commands

`oxid check entry.ox --edition typed-preview` checks the complete explicitly
declared project without execution. `run` selects bounded reference execution;
`compile --backend llvm --output ./new-program` selects the Linux x86_64 native
preview. Each requires one entry source; a manifest build is not a typed build.
Run/compile require an original zero-argument scalar main in the root file.
Declared modules use `mod name;`, direct imports use `use crate::m::item;`, and
`pub` controls module/item/field visibility. See [typed preview](../spec/typed-preview.md),
[native preview](../spec/native-preview.md) and the
[three-file example](../fixtures/typed-project-batch/README.md) for limits and
reproduction. Module-free syntax has no discovery host gate; child loading
currently admits Linux only.

### Explicit manifest project formatting

`oxid fmt --manifest-path oxid.toml --check` checks formatting for the selected
typed-preview package. Add `--package <name>` when selecting from a workspace.
Exactly one mode is required: `--check` writes no files and exits 1 for formatting
drift; `--write` explicitly replaces changed source files and exits 0 on success.
Both modes exit 2 on errors. No mode is implied, and source operands are rejected.

Only the selected package's declared source files are formatted; dependency
sources are loaded through the existing project session but are not rewritten.
Formatting is syntax-only and does not run programs or native tools. Output is
text-only; `--profile`, `--backend`, `--engine`, `--target`, output paths and
experimental compiler selectors are unsupported. `--manifest-path` must follow
`fmt`, and `--` ends option recognition. Single-file typed formatting and default
legacy formatting keep their existing behavior.

Write mode validates and stages all candidates before replacing originals,
checks for changed input bytes, and replaces each file atomically. A later
replacement failure can leave earlier files formatted; the operation is not a
whole-project transaction. See [project formatting](../spec/typed-project-format.md)
for the write boundary, source-path restrictions and limitations.

### Explicit manifest project doctor

`oxid doctor --manifest-path oxid.toml` validates manifest configuration and
resolves the selected typed-preview package's dependency graph. It does not open
or validate source files, check program health, run the application or its tests,
or write project build artifacts. Missing or invalid source files therefore do
not make this configuration-only check fail.

Add `--package <name>` to select a workspace package. The default check does not
require LLVM; add the valueless, non-repeatable `--native` flag to also check the
native toolchain through the existing standalone doctor. That opt-in probe may
create temporary probe files; it does not build or run the selected application.

`--manifest-path` must follow `doctor`. Manifest and package selectors accept
split or `=` values, and `--` ends option recognition. Output is text-only.
Explicit `--edition typed-preview` and `--message-format text` are accepted;
legacy edition, JSON output, source operands, profile, engine, entry mode,
backend, target, output and experimental compiler selectors are rejected before
manifest I/O. Exit status is 0 for success and 1 for a failed check or invalid
request.

Bare `oxid doctor` keeps its legacy project checks. The standalone
`oxid doctor --edition typed-preview` remains the existing native toolchain
doctor and does not select a manifest project.

## Create and maintain a project

`oxid new <workspace-path> --edition typed-preview --workspace` creates a virtual
workspace with two ordinary typed-preview projects, `workspace-app` and
`workspace-library`, connected by an explicit local path dependency. The
destination must not exist and its parent must already exist. The valueless
`--workspace` flag is local to `new`, occurs at most once before `--`, and requires
explicit typed-preview edition selection. Select either project with
`--package <name>` when checking, running, testing or building the workspace.
Creation is text-only and creates no lockfile, cache or downloaded package.
Failures after creation starts retain partial files for inspection; this is not
an atomic whole-workspace transaction. Without `--workspace`, typed `new` keeps
its existing single-project template. See
[typed workspace scaffolding](../spec/typed-workspace-new.md) for the generated
layout and commands.

| Command | What it does |
|---|---|
| `oxid new <name>` / `oxid init <name>` | Create a new directory and project scaffold; the directory must not exist |
| `oxid web new <name>` / `oxid discord new <name>` | Create an HTTP or Discord starter project |
| `oxid build [--offline\|--locked\|--frozen]` | Resolve dependencies and write `.oxid/bin/<project>.oxb` |
| `oxid test` | Recursively execute `.ox` files under `tests/` and `examples/`, except explicitly registered source-data fixtures |
| `oxid lint` | Run source checks across the project |
| `oxid fmt [path]` | Rewrite source formatting in a file or project |
| `oxid doctor` / `oxid diagnose` | Check the manifest, entry point, and project structure |
| `oxid doc` | Write a built-in reference to `docs/API.md`, replacing an existing file |
| `oxid script <name> [args...]` | Run a manifest script with appended arguments |
| `oxid clean` | Delete the project's `.oxid` artifacts, caches, and dependency checkouts |

`lint` currently runs syntax checks, not a separate static-analysis engine. Manifest scripts launch an executable and argument vector without a shell; see [Scripts](SCRIPTS.md).

### Source-data fixtures

Compiler inputs that are data rather than executable language tests can be
registered by exact path in the current project's `oxid.toml`:

```toml
[test-fixtures]
"tests/fixtures/parser/invalid-input.ox" = true
```

Paths are relative to that manifest's directory and must name existing `.ox`
files beneath `tests/` or `examples/`. Directories, globs, parent traversal,
absolute paths, symlinks, duplicate entries and values other than `true` are
rejected before any tests execute. Unlisted files, including neighboring files
in the same fixture directory, remain runnable. Without this section, recursive
discovery is unchanged. Explicit `run` and `check` commands are unaffected, and
a fixture-only project still reports that no runnable tests were found.

In this repository, `scripts/verify_repo.py` requires these entries to match the
validated frozen source-data inventory exactly. Registration does not establish
compiler correctness or replace the dedicated tests for those inputs.

## Manage dependencies

- `oxid add <name> <path-or-pinned-git-source>`
- `oxid remove <name>` / `oxid list`
- `oxid lock [--offline|--locked]`
- `oxid fetch [--offline|--locked]`
- `oxid update`
- `oxid install [--offline|--locked|--frozen]`

See [Packages](PACKAGES.md) for source rules, lockfile changes, and offline behavior. `install` resolves dependencies and builds the project; it does not install a global executable.

## Measure and inspect

- `oxid bench [--iterations N] [--json report.json]` measures Oxid's internal workloads.
- `oxid frontend` prints provider declarations; it does not select compiler providers.
- `oxid module`, `oxid syntax`, and `oxid interop` print reference information.
- `oxid bridge <python|java|go|c|cpp|all> [output]` generates host process adapters.

## Legacy bootstrap aliases

`oxid bootstrap`, `oxid self-compile`, `oxid emit`, and `oxid self-host` all run the same [artifact round-trip check](BOOTSTRAP.md). They do not rebuild the compiler.

Each accepts `--check`. Without that flag, the command writes artifacts and a manifest under `.oxid/bootstrap/`. With it, the command skips those output writes; source preprocessing may still use its cache. See [self-hosting status](SELF_HOSTING.md) for the exact checks.
