# Unit4 portable parser admission

This additive package transports reviewed portable adapter v3, its independent admission review and the independently audited fresh local result. The adapter is under `frozen/v3`; invoke its unchanged `portable.py` there. Relative helper, comparator and amendment paths are preserved. The source admission review is under `reviews/v3`.

Fresh local qualification passed on source head `03a3a93dd90074484cfc2d0aa976b1327d757c79`: four new debug/release instrumented/control builds, twelve ordinary passivity pairs and 638 effective-contract observations with zero issues. An independent read-only audit reproduced the exact comparison bytes. See [the current evidence summary](CURRENT-EVIDENCE.md). This local result does not establish the pending hosted combined qualification on the exact intended PR head.

The [publication projection ledger](publication-projection.json) records a single wording change in the nonexecuted `frozen/v3/DESIGN.md`. Its original checkpoint remains unchanged. Every other checkpoint-bound member is byte-exact, including the runtime, authority, helpers and semantic comparator. Verify the published package with `python -B verify_publication.py --package .`; verify the unchanged execution authority with `python -B frozen/v3/portable.py inspect`. Historical source documents retain their original candidate-stage status; this README and the current evidence summary state the later result.

The adapter's README and DESIGN describe commands, source/toolchain authority, original versus effective contract identity, operational bounds and required gates. Maintainers manage CI activation and publication.
