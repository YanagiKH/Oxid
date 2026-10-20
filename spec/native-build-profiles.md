# Explicit native build profiles

The ordinary typed native pipeline accepts `--profile dev|release` and
`--profile=dev|release`. Omitting it means `dev`. A profile is typed compiler
configuration and retained artifact identity, not an edition or entry policy.

Supported commands:

```
oxid compile main.ox --edition typed-preview --backend llvm --output main.elf --profile release
oxid build --manifest-path oxid.toml --profile release
oxid run --manifest-path oxid.toml --engine native --profile release
oxid test --manifest-path oxid.toml --engine native --profile release
oxid package --manifest-path oxid.toml --output app.tar --profile release
```

Manifest selection is already explicit typed-project opt-in; it does not require
an additional edition flag. Source compile still requires explicit typed-preview
and its existing backend/output/target validation. Profiles work with Result and
Process compilation/build/package/native run; native tests retain their existing
Result test-entry policy. Workspace `--package` selection, filters, and fail-fast
retain their existing semantics. All selected native tests are prepared before
execution, even with fail-fast.

An explicit profile (including `dev`) is rejected for interpreter run/test,
check, fmt, doctor, new, experimental import/producer/lexical-provider modes and
other unrelated commands. Profile options before the first command or after its
operands follow the existing global-option scanner. Nothing after `--`, or after
a script's name, is reinterpreted as a compiler profile. Unprofiled legacy argv
is unchanged. A recognized profile is not a legacy optimization option.

## Diagnostics and ordering

The scanner uses these exact messages:

- `--profile requires a value`
- `--profile may only be specified once`
- `unknown --profile \`VALUE\`; expected dev or release`
- `--profile is only supported by typed-preview source compile, manifest build, package, native run or native test`

Missing includes absent or empty values. A following dash-prefixed option is not
consumed as a separate profile value. Duplicate values are never last-wins.
Profile syntax errors participate in the existing first-global-error tier, ahead
of manifest-local errors. Existing earlier global errors remain first. Profile
applicability is checked after existing filter/fail-fast/engine applicability;
existing command, manifest-position, edition and operand validation still apply.
Error routing preserves the existing text/JSON and Process-intent channels.

## Native emission and publication

Dev passes `-O0`, release passes `-O2`, to Clang for both generated LLVM IR and the
bundled C runtime. The target and LLVM 19.1.7 toolchain contract stay unchanged.
There is no fast-math, LTO, native-CPU tuning, new platform support, debug-info
promise, or speedup guarantee. All language checking, checked arithmetic/bounds,
ownership validation, fuel and storage limits remain unchanged. Optimized native
behavior still needs real-toolchain acceptance, not merely an argv assertion.

Dev paths remain byte-for-byte unchanged beneath the selected project:

- `.oxid/typed-preview/result/NAME`
- `.oxid/typed-preview/process/NAME`
- `.oxid/typed-preview/test/case-ORDINAL`

Release uses separate corresponding directories `release-result`,
`release-process`, and `release-test`. The existing managed build lock, owner
receipt, staging, atomic replacement, and retained-inode execution mechanisms
apply independently in each namespace. A failed release build does not replace a
previous release executable or touch dev artifacts. These receipts are not a
compilation cache; every build still compiles. Plain source compile retains its
explicit output path and no-overwrite publication rule rather than assigning a
managed namespace.

Package serialization reads the exact retained artifact descriptor and its typed
profile. It never guesses a profile from the output filename. The unchanged
`oxid-native-bundle-v1` descriptive `profile` field is `native-preview-O0` for dev
and `native-preview-O2` for release. Existing dev metadata remains unchanged. The
archive digest still describes the included executable. Later packaging failure
can leave a successfully published managed executable, as documented in
[native-package.md](native-package.md).

## Verification boundary

Focused internal tests cover scanner selection/errors, profile-specific compiler
argv, metadata, and output namespaces, while retaining the Route layout and
manifest allocation assertions. Independent acceptance must build and execute
real dev/release programs and test suites, inspect actual compiler invocations,
exercise dependency rebuilds and failed-build preservation, and extract and run
packages without source/compiler dependencies. This feature alone is not R3
completion or a resolution of unrelated held qualification work.

## Recorded focused acceptance (2026-10-19 UTC)

The independently inspected acceptance archive binds the final formatted source
to commit `77be5f1093692ca31efeb022adb14fc372d91052`, tree
`c6faf0482146c92041d7e3e9f7806968577cf9fb`. Its clean-tree post-format
logs record 19 profile/configuration/publication unit tests, two existing carrier
regressions, 5 native-run core tests, 12 typed-run tests, one unsupported-CLI
regression, 11 ordinary companion tests and 7 real LLVM 19.1.7 native companion
tests passing. These are 57 focused test functions, not a whole-suite result.
Scoped compilation and Clippy also passed. Formatting for the seven selected
compiler files and the companion passed; main.rs was not whole-file format-clean.

The native companion exercises both O0/O2 profiles with Result and Process
entry routes, dependency rebuilds, filtering/fail-fast, source no-overwrite,
package metadata/digests and extracted executable deployment. The final-tree
real-toolchain argv/execution test passed; the separately preserved detailed
clang argv transcript belongs to the earlier pre-format binary. Executable
hashes are recorded in the supplied logs; compiler binaries were not included
for independent rehashing.

Three earlier ordinary failures are retained. Their planted fake artifacts were
rejected by ownership checks before toolchain selection. The corrected ordinary
case tests fresh-fixture nonpublication and owner-marker namespace isolation,
not preservation of existing valid artifacts with an absent toolchain. Separate
coverage establishes preservation after a real E0300 checking failure and after
an internally injected E0701 staging failure. These do not constitute a real
missing-toolchain-on-existing-artifact experiment.

`native_package_core`, `manifest_build_core` and `project_test_core` were
compile-checked only. Historical qualification, fuel/capacity probes and the
full suite were not run. No exact-public-head hosted CI, performance improvement
or completed R3 qualification is asserted. The publication successor changes
only this validation wording and the line wrapping of one new help statement
after the recorded tested tree; the pre-existing main formatting hunks remain.
