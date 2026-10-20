# Typed project doctor

This describes an experimental developer-tool extension, not a v1.0 release
qualification. Bare `oxid doctor` keeps its legacy project health check, and
explicit `oxid doctor --edition typed-preview` keeps its toolchain-only probe.

## Explicit project selection

```
oxid doctor --manifest-path app/oxid.toml
oxid doctor --manifest-path workspace/oxid.toml --package app
oxid doctor --manifest-path workspace/oxid.toml --package=app --native
```

`--manifest-path` and `--package` accept both the `=` and the separated form,
once each. `--manifest-path` must follow the `doctor` command word. `--native`
is a flag. No project name is required of a direct manifest; the existing
typed manifest and workspace selection rules apply unchanged, using the same
resolution as manifest `check`. This adds no new loader, graph, source or
native behavior, and no implicit manifest discovery: without
`--manifest-path`, both existing doctor routes are unchanged.

Only typed-preview manifests and text output are supported. `--edition
typed-preview` and `--message-format text` are accepted; JSON output, profiles,
engines, entry modes, backends, targets, output paths, experimental options,
a `legacy-0.9` edition and any extra operand are rejected. `--native` takes no
value and may be given once: `--native=VALUE` fails with "does not accept a
value" and a repeated `--native` with "may only be specified once". Every
rejection, including a JSON request, is a text `E0001` diagnostic on stderr
with an empty stdout; doctor does not use sticky JSON.

## Default report

By default the command resolves the existing manifest graph and reports the
selected package and its dependency closure. It does not open, parse or check
source files, does not compile or execute the project, does not look up the
native toolchain, and creates no artifacts, lockfiles or caches. The report must
state `sources: not checked`.

The report is text with fixed structure lines, packages in the existing graph
order (selected package first, then manifest declaration DFS order) and a
shared dependency listed once:

```
typed project configuration resolved
edition: typed-preview
selected manifest: "PATH"
selected package: "NAME"
entry: "PATH"
packages: N
package INDEX: manifest="PATH" name="NAME" entry="PATH"
dependency SOURCE: alias="ALIAS" target=TARGET
sources: not checked
native probe: not requested (use --native)
```

An unnamed package is reported as `<unnamed>` instead of a quoted name. Each
package line is followed by its own dependency lines. The index after
`dependency` is the owning (source) package's index, not a per-edge sequence,
so a package with two dependencies prints the same index twice; `TARGET`
identifies the target package's index in the same report. Every external
string (path, name, alias) is double-quoted with control characters, quotes and
backslashes escaped as Rust `char::escape_default` escapes; this is not JSON
string output.

Consequently a missing, unreadable, syntactically invalid, semantically invalid
or non-UTF-8 source file does not prevent a successful report. Manifest-level
facts are still validated: a missing or invalid `entry` declaration, a
non-typed-preview edition, an unknown or missing workspace package selector,
an invalid workspace member manifest (selected or not), a dependency cycle or
a dependency directory without a manifest all fail. Unselected workspace
members are validated as selection requires but are not reported.

## Native probe

`--native` additionally runs the existing typed toolchain doctor probe: it
compiles and links the bundled runtime, verifies the produced header and never
executes the product. It still does not check project sources. The probe's
status is returned as-is; a missing or failing toolchain reports `E0701`.
The complete configuration report, including `sources: not checked`, is
written and flushed to stdout before the probe starts, so a failed probe still
leaves the report on stdout; a probe is not source validation. The package,
dependency and sources lines are identical to the default report; only the
final line differs and reads `native probe: requested`. Native doctor output
follows the report, so `--native` stdout is never empty and never equals the
default report. If writing or
flushing the report fails, no probe runs. Command-line, manifest and selection
failures are reported before, and instead of, the probe.

## Results

- Exit 0: the configuration was reported (and, with `--native`, the probe
  succeeded).
- Exit 1: a command-line, manifest or selection error, a failure writing the
  report (no probe runs), or a failed `--native` probe (after the report).

## Evidence boundary

Focused CLI controls are in `tests/manifest_project_doctor.rs`. They use only a
missing toolchain directory or failing recording stubs; they never run real
LLVM or native products and do not touch held native boundaries. A successful
`--native` report requires the separately authorized LLVM 19.1.7/Linux x86_64
toolchain and is outside these controls. Results must identify the exact source
revision. This command alone does not satisfy the complete G6 tooling gate,
certify all platforms, or establish v1.0 release readiness.
