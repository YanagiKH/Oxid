# Portable Unit4 parser admission candidate, v3

This is an adapter candidate for independent final review. It is not a passing
parser qualification or a hosted CI receipt. V3 binds the exact approved v8b
comparator and strict source-coordinate amendment. Real four-build,
ordinary-passivity, full-corpus and hosted qualification remain pending.

Two small V3 recipe fixes additionally support relative session arguments while
rejecting symlink ancestry and bound the initial Rust version probe to 15 seconds.

The inherited V2 recipe repairs the independent review's Git provenance, executable PATH and
same-session host consistency findings. The predecessor checkpoint and review
remain byte-for-byte under `reviews/provisional-v1/`.

`portable.py` adds portable source/build admission around exact frozen helper
v7 and comparator v8b sources. It does not edit compiler files, base expectations or
the older local observations. The original 283-file and derived 286-file maps,
Rust/LLVM/target selection, locked dependency sources and exact helper maps are
in the pinned `authority.json`. `DESIGN.md` describes trust boundaries, command
interfaces, known gaps and required negative controls. `checkpoint.json` binds
all included files except itself.

## Repeat the source controls

Use Python 3.11 or newer with assertions enabled. The historical checkout must
have HEAD exactly `d9e6b9bf172abd5e15da7212c9e6224e29ccc768`. The current checkout
must have the same complete compiler input bytes; its own HEAD/tree are recorded
separately. `UNIT4_PORTABLE_CONTRACT` is the already verified original parser
package from the durable contract transport. No candidate build is performed.

```sh
export UNIT4_PORTABLE_HISTORICAL_REPO=/absolute/historical-worktree
export UNIT4_PORTABLE_CHECKOUT=/absolute/current-checkout
export UNIT4_PORTABLE_CONTRACT=/absolute/verified-contract/parser
python3 -B portable.py inspect
python3 -B test_portable.py
```

For the reviewer regressions and integrated synthetic receipt/passivity controls,
also provide the official Rust root and a populated locked-crate cache:

```sh
export UNIT4_PORTABLE_TOOLCHAIN=/absolute/rust-1.99.0-x86_64-unknown-linux-gnu
export UNIT4_PORTABLE_CARGO_CACHE=/absolute/cargo-cache
python3 -B -m unittest test_portable test_review_regressions test_receipts test_passivity test_semantic_rebind test_recipe_fixes -v
```

These tests materialize sources and run native/Python/Rust identity probes; they
do not compile Rust/C or execute a candidate observer. Synthetic receipts remain
explicitly labeled and cannot satisfy the complete 638-row contract.

The frozen comparator suite is retained without path edits. The runner resolves
its base input and the exact packaged amendment artifacts before loading tests:

```sh
python3 -B run_frozen_tests.py --contract-dir "$UNIT4_PORTABLE_CONTRACT"
python3 -O -B run_frozen_tests.py --contract-dir "$UNIT4_PORTABLE_CONTRACT"
```

The second command covers the independent comparator's optimized-mode
regression suite. The portable execution CLI and frozen
collector helpers deliberately reject optimized Python.

## Historical Git authority in a shallow CI checkout

The eventual combined job must explicitly make the exact historical object
available. Never replace the helper's source pin with the current checkout.

```sh
git fetch --no-tags origin d9e6b9bf172abd5e15da7212c9e6224e29ccc768
git cat-file -e d9e6b9bf172abd5e15da7212c9e6224e29ccc768^{commit}
git worktree add --detach "$RUNNER_TEMP/parser-authority" \
  d9e6b9bf172abd5e15da7212c9e6224e29ccc768
python3 -B portable.py prepare --repo "$RUNNER_TEMP/parser-authority" \
  --checkout "$GITHUB_WORKSPACE" --output "$RUNNER_TEMP/parser-fresh"
```

Rust 1.99.0 for x86_64-unknown-linux-gnu is required by this draft. Installation
and dependency fetching belong to the caller's existing authorized CI setup.
The adapter's build command uses a fresh private cache, two jobs and incremental
compilation disabled. The intended final fresh recipe, after adapter review,
builds four binaries and requires both passivity and
complete contract collection:

```sh
for profile in debug release; do
  python3 -B portable.py build --session "$SESSION" --profile "$profile" \
    --toolchain "$RUST_ROOT" --cargo-cache "$CARGO_CACHE"
  python3 -B portable.py build --session "$SESSION" --profile "$profile" \
    --toolchain "$RUST_ROOT" --cargo-cache "$CARGO_CACHE" --control
  python3 -B portable.py passivity --session "$SESSION" --profile "$profile"
  python3 -B portable.py collect --session "$SESSION" --profile "$profile" \
    --contract-dir "$FROZEN_CONTRACT"
done
python3 -B portable.py compare --session "$SESSION" --contract-dir "$FROZEN_CONTRACT"
```

The passivity helper checks three prescribed ordinary sources in two parser
modes, six pairs per profile. Its new receipts do not replace the full contract
gate or imply fresh execution from historical local passivity. Execution receipts
retain the original base contract/freeze identities. Only the new independent
comparison applies the strictly admitted four-coordinate amendment and reports
both identity domains; no historical receipt or raw observation is relabeled.
Real runs and final adapter admission remain pending review. The hosting job must enforce
an explicit deadline and practical disk/evidence limits.
