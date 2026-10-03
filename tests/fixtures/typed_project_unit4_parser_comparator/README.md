# Unit4 parser comparator checkpoint

This package preserves the exact independently approved v8b comparator, its
independent Rust Debug decoder, synthetic controls and protocol evidence. It is
an additive source checkpoint. It changes no compiler, contract, observer,
production workflow or CI wiring.

The authorized local comparison completed successfully: 638 of 638 observations
across debug and release, with no issues. The result is
`evidence/local-amended-comparison.json`, SHA256
`174583a5c32fbe194969a1455ab917992d49696f8b300235ca9fcd64d9f8ba5a`.
The command receipt records 78.91 seconds and maximum RSS of 2,591,984 KiB
(approximately 2.47 GiB). This was a new comparison of retained observations;
it did not rerun the compiler or collect new candidate observations. Fresh
portable and hosted qualification remain separate pending gates.

## Frozen source and current evidence

`frozen/v8b/` contains every checkpoint-bound member and the exact checkpoint:

- Checkpoint SHA256: `78b152c74d9623ae2a66bc12c169bb6c9ed4077657d9bcde0e60978331a5afdc`
- Comparator SHA256: `7c40e4782bee8082dc41534227348c26f952f3b870904cda9e71862b0be42a6b`
- Decoder SHA256: `8fb1708d7824eb453db5c036ac2e7d49ca76e69ff222f1c3cf1e16f41272aa41`

The frozen README and source-only receipts retain historical status statements
from their creation. They are preserved byte for byte. This README, the separate
review artifacts and the local execution evidence describe the later current
status; historical records are not rewritten to claim later success.

The semantic review repeated 46 normal and 46 optimized test methods and 30
independent boundary controls. The final loader review repeated 52 normal and
52 optimized methods plus 16 held-out loader controls. The reviewed fixes bind
required ordering to the first source-relevant admission, enforce the prior
rejection barrier before recording allocator failure, and preserve the exhausted
node budget after a targeted admission seam. All other expected predicates,
source origins, raw/AST normalization and execution identity checks remain.

## Original execution and effective comparison authority

The retained raw observations still name decoded execution contract
`b19819e2e4af627dfe3877ef7753fe237aa7830b16d2a83197fae0ed02010cbc`
and original freeze
`7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027`.
The effective comparison identity is
`oxid-unit4-parser-v1-location-amendment-v1`, with canonical document SHA256
`c2d4f8db28f7815b3e17ca13fec9a7da70f0923dd052656edb339bc0cd7740cd`.
The independently source-authored amendment changes only four integer
coordinates for one diagnostic projection. The loader proves that exact
whole-document delta and independently checks the complete source token.

`evidence/original-failed-comparison.json` preserves the original six mismatches
under its original authority. Four were caused by whole-trace interpretation of
first-admission ordering, and two by the source-proven diagnostic-coordinate
defect. The revised pass does not turn that original result into a pass. Earlier
false-accept controls and their closures are identified in the frozen replay
receipts and independent reviews. No earlier source package is duplicated here.

## Portability boundary

The local comparator CLI requires schema-v2 authorization, the exact checkpoint
and four reviewed amendment artifacts. The retained local authorization and
command use historical absolute paths and build/helper receipts. They are
evidence of that completed run, not reusable authorization for a new checkout.
Raw observations and binaries are not included in this source package.

A portable wrapper may consume the exact comparator, decoder and test bytes
under `frozen/v8b/`. Any change from local receipt admission to fresh source/recipe
admission needs a separately pinned implementation and review. It must preserve
the reviewed predicates and global event checks, exact source/seam/profile/host/
observer/binary identities, the complete 638-row roster, and the separate
execution/effective identity domains. See `REPRODUCE.md` for source-only checks
and the retained local comparison protocol.
