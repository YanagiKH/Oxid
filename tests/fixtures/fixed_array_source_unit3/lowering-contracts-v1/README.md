# Fixed-array source lowering: independent five-model checkpoint

This package preserves five independently reviewed source-derived expectations for the private array lowering slice. It contains no compiler result, candidate observation or native execution evidence. Public array syntax and production raw-array gates remain closed.

The exact source baseline is commit bb2b5d7c39d5940cbbf959444646c2dacb8edcb4, tree a5398061eecc0e35726b648bf7edcd56434e1337. The grouped-access source and both snapshot sources retain their published bytes. RHS-bounds and index-read-bounds variants are two new source-data fixtures. All five .ox paths are explicitly listed in lineage.json for the repository's source-data registration; they are not newly activated language tests.

Use a local Git object store containing that commit:

    python replay.py --repo /path/to/Oxid
    python replay.py --repo /path/to/Oxid --out /existing/parent/new-models
    python -O replay.py --repo /path/to/Oxid --out /existing/parent/new-models-optimized

The verify-only form checks exact current package membership, canonical regular-file paths, all hashes, original authority identities and named source Git objects. An externally pinned root digest can be supplied with --expected-manifest-sha256. The repository source-data registry pins that digest separately. The reconstruction form requires a new output directory outside the frozen package, input repository and its Git storage, and never overwrites or merges with an existing directory. Resolved symlink parents do not bypass containment. It reads named Git objects without switching a checkout, fetching, building or running Oxid.

Current membership is defined solely by the root freeze-manifest.json dictionary. The nested original checkpoint manifest is historical authority identity, not a claim that its full former directory is installed here. Historical README.md and the original machine-bound generator are intentionally absent. Their original identities remain in that immutable manifest. model_generator.py is a packaging adapter: only repository/output binding, explicit UTF-8 byte writes, explicit baseline rejection and direct-entry handling differ. It reproduces all eleven source/model/summary bodies byte-for-byte. No expected source, model or correction bytes were retargeted for portability.

The original five-model authority alone is not admitted. Apply correction1 and correction2, and read the storage identity statement. The corrections change only exact effect wording inside already-paid operations: successful return closes and branches the caller before observing/popping the callee; transfer snapshots use pre-write destination generation1 while the Transfer event identifies installed generation2. The replay applies explicit hash-bound JSON-pointer replacements with original-value checks, leaving historical model bytes untouched. Its effective-models output is the combined authority for comparison.

Expected reference fuel is grouped51, snapshot success74, final-bounds65, RHS-bounds43 and index-after-effect-bounds64. Every budget through each terminal boundary is enumerated. Snapshot helper normal return/release completes64; paying final access at65 either stores saved5 or fails bounds while preserving99 and omitting later reads. Source-derived operation identities, origins, Counts and effects must be compared to actual rows. These arithmetic expectations are not evidence that the compiler or either consumer ran.

replay.py remains fail-closed under Python -O: package containment, membership, identity, correction and zero-overwrite checks use explicit conditions, not Python assertions. A successful replay proves transport and reconstruction of these expectations only. It does not qualify lowering, ownership verification, native admission/emission, machine execution or public activation.

Packaging revision2 preserves revision1 and changes only output containment and byte-portable materialization. Model/source/summary reconstruction and all correction payloads remain identical.
