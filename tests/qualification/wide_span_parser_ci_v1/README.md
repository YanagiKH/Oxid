# Wide-span parser CI-only successor

The accepted workflow is retained exactly in `predecessor/`. Its SHA-256 is
9c6e7b902f18e24d4b4f81af77e915463171bdddb2d7cd8fa504c0be9135ec51.
The only workflow change is one step after successful ordinary debug/release
HIR-producer qualification. It consumes that job's complete receipt and retains
its results under the existing always-uploaded artifact directory. No old step,
job identity, dependency, disposition, source binding, compiler checkpoint or
40-minute job budget changes. Runtime execution is not claimed by these proofs.

`activate.py --check` replays the existing PR64/PR65/text mechanical generators
and then derives the successor. `successor.py` provides a private historical
reader mapping for exactly `.github/workflows/ci.yml`; every other historical
read uses the original strict reader. The original namespaces and generators
remain byte-identical. Their old output assertions compare the explicit fixed
predecessor view; the new complete inventory separately admits current bytes.
The fixed seven-file archive contains only replaced CI/test/inventory seams.

`workflow-proof.json` records the complete old/new workflow identities and the
literal insertion. The named input inventory preserves every predecessor row
except the three explicitly derived active inputs, adds this closed namespace
and the closed wide-span candidate fixture, and includes all four runtime tools.
Its check rejects unapproved predecessor input changes rather than refreshing
hashes. `test_successor.py` exercises exact insertion, immutable historical
producer reproduction, input closure and mutation rejection without compiling.

The fixture-data count adjustment has its own named storage successor under
`wide_span_fixture_storage_v1`. This generator composes its exact output map;
its explicit original-body reader is separate from the one-path CI workflow
mapping. Dispatcher/parser/lifecycle/hosted identity changes are storage-only.
Current native snapshots close both new helper namespaces. Existing text/trim
namespaces, all compiler/source manifests and all 173 dispositions stay intact.
The existing current-CI test command discovers the five CI successor controls
and four fixture-storage controls exactly once through an explicit load hook.
