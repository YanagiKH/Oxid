# Unit3 compiler inputs through the retained Unit2 gate

This outer adapter runs the byte-identical historical Unit2 runner, protocol,
observer, normalizer, comparator and 21 resource tests with explicitly selected
current compiler inputs. It does not qualify Unit3 execution or activate public
project syntax.

```sh
python3 -B tests/fixtures/typed_project_unit3_compatibility/run.py \
  --repo . \
  --source-manifest tests/fixtures/typed_project_unit3_compatibility/source-inputs-unit3.json \
  --output /tmp/new-unit3-unit2-results
```

The output must be new and outside the checkout. Use the qualified official Rust
toolchain on PATH or select exact `--cargo` and `--rustc` executable paths. The
runner is offline/locked, with two Cargo jobs and incremental compilation off.
`--prepare-only` records zero compiler executions and creates no passing receipt.

The adapter verifies the historical Unit2 package and all selected current
compiler inputs, including exact membership under src/native. It creates a
private derived package in the output directory, changing only source-inputs.json
and that entry's identity in the containing package-inputs.json. All other
package files, including the runner, remain byte-identical. The historical
package in the checkout is never modified.

The unchanged historical runner checks the derived package, builds and runs both
profiles, and enforces its original 3,603 unique semantic cases and 21 named
resource tests per profile. Its fresh receipts bind the selected source and
derived package identities. The outer result binds the child result/invocation,
historical and derived identities, adapter hash, command status and output
hashes. Original/derived package bytes, selected input bytes and adapter bytes are
rechecked before emitting the outer result. Missing selection or mismatched
compiler inputs fails; there is no implicit historical fallback.

These are retained Unit2 gates only. Source-owner internal failures now suppress
untrusted origins; separate Unit3 constructor/privacy controls cover malformed
maps, stale parser identity and nonzero file IDs. The initial current source
manifest contains 117 core-v1 inputs. A later compiler change requires a reviewed
new current manifest and fresh receipts; historical identities stay unchanged.
