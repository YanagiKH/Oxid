# Typed compiler-source export relocation example

An ordinary typed-preview workspace for `oxid export`. The selected member
`export-app` has nested modules (`report` -> `frame`) and depends on the
workspace member `export-geo` and the non-member `export-stats`. Both depend on
the shared kernel in `core` (one package, exported once), and each has its own
local package named `units` (`geo/vendor/units` and `stats/vendor/units`):
same name and same function spelling, distinct packages. The kernel's name is
written with manifest escapes for a quote, backslash, Unicode letter, BEL and
tab. The unrelated member `export-tools` is not in the app's closure.

From this directory:

```sh
oxid export --manifest-path oxid.toml --package export-app --output ../export-app-src
mv ../export-app-src /somewhere/else/export-app-src
cd /somewhere/else
oxid check --manifest-path export-app-src/oxid.toml
oxid run --manifest-path export-app-src/oxid.toml
# 1134
oxid test --manifest-path export-app-src/oxid.toml
# PASS test_area
# PASS test_distinct_units
# PASS test_report
# test result: 3 passed; 0 failed; 3 total
```

The export is a normalized compiler-source tree, not a backup:

```text
export-app-src/
  oxid.toml                 export-app; geo = "packages/p1", stats = "packages/p4"
  src/.root.ox              app/src/main.ox
  src/report.ox, src/report/frame.ox
  packages/p1/              export-geo; kernel = "../p2", units = "../p3"
  packages/p2/              the shared kernel (escaped name), used by p1 and p4
  packages/p3/              geo's units
  packages/p4/              export-stats; measure = "../p5", kernel = "../p2"
  packages/p5/              stats' units
```

Generated manifests keep only edition, entry `src/.root.ox`, the retained name
and dependency aliases in their original order. The manifest comment and
`version` values, `app/assets`, `app/scripts`, the undeclared
`app/src/unused.ox`, this README, the workspace manifest and the `export-tools`
member, lockfiles and build artifacts are deliberately not exported. No host
path appears in the output. Export checks every package in the closure first and
never runs source or native tools; the output directory must not exist. The
exact expected tree is frozen in `tests/typed_project_export.rs`.
Typed-preview is experimental.
