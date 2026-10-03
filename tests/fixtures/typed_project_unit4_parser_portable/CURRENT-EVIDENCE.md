# Current parser qualification evidence

The independently audited fresh local qualification passed on source head `03a3a93dd90074484cfc2d0aa976b1327d757c79`, tree `06caab7624ddc17dd3d41f85871b534b7cf1dce4`. It used the separately verified historical compiler source `d9e6b9bf172abd5e15da7212c9e6224e29ccc768` and the exact reviewed portable v3 package.

All thirteen stages succeeded in about 297.4 seconds: four fresh instrumented/control debug/release builds, twelve ordinary passivity pairs and 248 cases yielding 319 observations in each profile. The strict effective comparison passed all 638 observations with zero issues. Original execution contract IDs remain distinct from the effective source-coordinate amendment identity.

- Original source admission review checkpoint: `3cfd3aed59a11b02dab37156fbbf5fdd4799c874ee6540b48a376e27394dc412`
- Original frozen package checkpoint: `8a71652fa28e4c7580112f5e447b8eed7dce56497d1ece1e1b49a2e93dbb2af3`
- [Fresh result checkpoint](evidence/local-03a3a93d/result-checkpoint.json): `ee28e435a690bb77729c4a176e3da8bc902f600c2e709ce9365d0379ebf4a788`
- [Independent actual-result review checkpoint](reviews/fresh-local-v3/review-checkpoint.json): `cbfa31161f69f596e660fad6834e6f342cf373baac0327923b9b7876083e539c`
- Original and independently replayed comparison: `b3fde4b8ce24ae22dd2070f602ae8daeecb7d05af55fc5638fab20976e9e7305`

Independent read-only replay verified the source/build/host/passivity/collection receipts and produced exactly the original comparison bytes. All 4,371 original regular files and eight symlink targets were unchanged. The reviewer performed no compiler builds or candidate recollection. See the [receipt audit](reviews/fresh-local-v3/receipt-audit.json) and [replay report](reviews/fresh-local-v3/replay-report.json).

The original result checkpoint predates the independent audit and preserves that audit as pending in its historical `remaining` field. The later review checkpoint records its completion. Neither record is rewritten by this publication.

This repository carries selected unchanged receipts listed in [the transport ledger](evidence/transported-receipts.json), not the complete raw evidence or executable binaries. Their absolute paths record the original execution locations and are not claims of executable receipts at this repository location. The separately retained compact evidence archive is 33,733,523 bytes, SHA256 `adb70750ddf2b15d07a927176b58b127699f60d10a23885d5683f898f5b66aa6`; the four-binary archive is 14,285,670 bytes, SHA256 `833e42ec5495358e0bb7f0b48bfda86db0db613d590f7d2f7ae3b8dc330314d2`. Archive metadata is included under `evidence/local-03a3a93d`.

The remaining gate is hosted combined qualification for the exact intended PR head. This progress publication changes neither production compiler inputs nor CI activation. The recorded ordinary passivity coverage is the prescribed six pairs per profile; broader unchanged instrumentation properties come from the prior static/source review.
