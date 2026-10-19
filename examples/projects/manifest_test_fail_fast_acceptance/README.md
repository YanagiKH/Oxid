# manifest_test_fail_fast_acceptance

Fixed client for `tests/manifest_test_fail_fast_acceptance.rs` (explicit
manifest `oxid test --fail-fast`). The test copies this directory to a
temporary root and never runs it in-tree.

- `app/` (`failfast-app`, path dependency on `helper/`): the fixed suite in
  source order: pass -> false -> pass -> divide-by-zero -> unit
  (`test_early_pass`, `test_early_false`, `test_mid_pass`,
  `test_late_divide`, `test_late_unit`). `main` and `src/child.ox`'s
  `test_child_false` are not cases.
- `helper/` (`failfast-helper`): `seven()` / `nothing()`; its own one-case
  suite fails on its last (only) case.
- `green/` (`failfast-green`): every case passes.
- `tail/` (`failfast-tail`): only the last case fails (non-zero i32).
- `oxid.toml`: virtual workspace over the four members.
- `legacy/`: a tiny side-effect-free legacy probe for the no-manifest forms.

Frozen expectations (hand-written from the contract):

```text
$ oxid test --manifest-path app/oxid.toml
PASS test_early_pass
FAIL test_early_false
PASS test_mid_pass
FAIL test_late_divide
PASS test_late_unit
test result: 3 passed; 2 failed; 5 total
$ oxid test --manifest-path app/oxid.toml --fail-fast
PASS test_early_pass
FAIL test_early_false
test result: 1 passed; 1 failed; 2 total
test stopped: 3 selected tests not run (--fail-fast)
$ oxid test --manifest-path app/oxid.toml --filter late --fail-fast
FAIL test_late_divide
test result: 0 passed; 1 failed; 1 total
test stopped: 1 selected tests not run (--fail-fast)
```

Both runs with `--fail-fast` exit 1. `--package failfast-app` on the
workspace behaves the same. All-pass (`green`) and last-case failure (`tail`)
print no stopped line. Refusals: `--fail-fast does not accept a value`,
`--fail-fast may only be specified once`, and on check/run/build/package
`--fail-fast is only supported by explicit manifest test` (a valid `--filter`
reports its existing applicability error first).

Legacy, script and `--` forms are pinned to the base 2dcbc8a1 binary
(sha256 4c4e8da1321d9742704d1bcbc63044cfd72968f3e78ca669fdbd0d972e7a256e).

Native cases are `#[ignore]`d and need LLVM 19.1.7:
`OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
manifest_test_fail_fast_acceptance -- --include-ignored`. They also check
that every selected `case-<ordinal>` artifact is prepared before any case
runs and keeps its original suite ordinal.

## Validation

The reviewed source passed fourteen ordinary and four LLVM 19.1.7 native
companion tests. Existing filter controls also passed twelve ordinary and five
native tests. See the focused fail-fast validation status in
`../../../spec/typed-project-test.md` for source/binary binding and scope limits.
