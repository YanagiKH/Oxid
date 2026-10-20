# manifest_native_profile_acceptance

Fixed client for `tests/manifest_native_profile_acceptance.rs` (native
`--profile dev|release`, task r1-native-profile-21fa59b0). The test copies this
directory (and `packages/utf8_text`, keeping the relative path) to a temporary
root and never builds it in-tree. Every expectation is hand-written from the
frozen contract and the arithmetic below; none is generated from candidate
output. dev and release must give identical results.

- `app/` (`npp-app`; path dependencies `model` -> `../model`, `text` ->
  `../../../../packages/utf8_text`): `main` returns
  scalar 21 + record 9 + enum 2 + owned buffer 6 + Text 4 = 42.
  - scalar: `scale(base())` = 7 * 3 = 21
  - record: `pair_sum(&make_pair(base(), 2))` = 7 + 2 = 9
  - enum: `score(measure(base(), 1)) + score(measure(-6, 9))` = 8 + (-6) = 2
  - owned buffer: `byte_total(&filled())` = 1 + 2 + 3 = 6
  - Text: "Ox" + U+00A2 = 4 bytes, 3 scalars; `(4 * 10 + 3) / 10` = 4
  - suite (all pass, source order): `test_scalar_scale`, `test_record_pair`,
    `test_enum_outcomes`, `test_buffer_bytes`, `test_text_scalars`,
    `test_unit_tail`.
- `model/` (`npp-model`): `base()` = 7. The rebuild case changes it to 8:
  main becomes 24 + 10 + 3 + 6 + 4 = 47 and the first three app cases fail.
- `gate/` (`npp-gate`, depends on `model`): pass -> false -> divide-by-zero
  (E0607) -> unit, for native `--filter` + `--fail-fast` under both profiles.
- `standalone/main.ox`: single-file typed source compile client, also 42.
- `oxid.toml`: virtual workspace over `app`, `model`, `gate`.

Frozen expectations:

```text
$ oxid build --manifest-path app/oxid.toml [--profile dev | --profile=dev]
typed-preview native build ok: "app/.oxid/typed-preview/result/npp-app"
$ oxid build --manifest-path app/oxid.toml --profile release
typed-preview native build ok: "app/.oxid/typed-preview/release-result/npp-app"
$ oxid run --manifest-path app/oxid.toml --engine native --profile=release
42
$ oxid test --manifest-path app/oxid.toml --engine native --profile release
PASS test_scalar_scale
PASS test_record_pair
PASS test_enum_outcomes
PASS test_buffer_bytes
PASS test_text_scalars
PASS test_unit_tail
test result: 6 passed; 0 failed; 6 total
$ oxid test --manifest-path gate/oxid.toml --engine native --profile release --fail-fast
PASS test_gate_first
FAIL test_gate_false
test result: 1 passed; 1 failed; 2 total
test stopped: 2 selected tests not run (--fail-fast)
$ oxid package --manifest-path app/oxid.toml --output ../../../dist/release.tar --profile release
typed-preview native package ok: managed executable "app/.oxid/typed-preview/release-result/npp-app"; archive "../../../dist/release.tar"
$ oxid compile standalone/main.ox --edition typed-preview --backend llvm --output out/release.elf --profile release
typed-preview native compile ok: "out/release.elf"
```

Namespaces: dev keeps `result/NAME`, `process/NAME`, `test/case-ORDINAL`;
release uses `release-result/NAME`, `release-process/NAME`,
`release-test/case-ORDINAL`. Package metadata `profile` is
`native-preview-O0` (dev, default) or `native-preview-O2` (release), and its
`sha256` is the digest of `bin/NAME`, which equals the retained managed
executable. dev passes `-O0` and release `-O2` to clang for both the LLVM IR
and the C runtime; the link step carries no optimization flag.

CLI diagnostics (`error[E0001] (cli): ...`; PACKAGE keeps its failure summary):

- `--profile requires a value`
- `--profile may only be specified once`
- ``unknown --profile `VALUE`; expected dev or release``
- `--profile is only supported by typed-preview source compile, manifest build, package, native run or native test`

Profile syntax errors are in the global first-error tier (ahead of
manifest-local errors such as `--fail-fast=1`); applicability is checked after
the existing filter / fail-fast / engine applicability. Explicit `dev` is
refused by interpreter run/test, check, fmt, doctor, new and the
experimental import mode. Words after `--` or after a script name are unchanged.

Native cases need LLVM 19.1.7 on Linux x86_64:
`OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
manifest_native_profile_acceptance -- --include-ignored`. No speed claim, no
fuel / capacity / qualification probes.

## Validation status

The final formatted tree `c6faf0482146c92041d7e3e9f7806968577cf9fb` has
recorded 11 ordinary and 7 actual LLVM 19.1.7 native companion passes. The three
earlier fixture failures remain part of the acceptance history. See
[the profile specification](../../../spec/native-build-profiles.md#recorded-focused-acceptance-2026-10-19-utc)
for exact source identity, the separate internal/CLI coverage, the distinction
between fresh no-toolchain checks and preservation coverage, and unexecuted
qualification/full-suite scopes. These results are not hosted-CI certification.
