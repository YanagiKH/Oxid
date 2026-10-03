# Unit3 passive observation schema, review draft v1

The observer accepts only a source request: `id`, `entry`, `source_root`,
`source_files` (relative path, byte count, SHA256), `mode`, `operations`,
`profiles`, `fuel_budgets`, and optional native emission controls. Unknown
request keys fail closed. No expected file, expected identity, result, route,
count, trace, annotation, or oracle outcome is an input. The Python controller
verifies source transport hashes and invokes one exact ignored test in one
test executable per profile. It does not resolve or rewrite source.

Artifacts for each request:

* `loaded.debug`: actual loaded ProjectSources (immutable map, ASTs, module
  headers, parser identities, load usage)
* `index.debug`: the actual frozen index from the one prescribed construction
  pipeline, captured immediately after successful finish
* `checked.debug`: the actual immutable CheckedSourceProgram after complete
  verification, before both consumers. Existing derived Debug already exposes
  the complete nested raw program; no witness getter or reconstruction exists
* `raw-before-audit.debug`: actual raw representation borrowed at registry entry,
  before association/raw verification; retained even when later checking denies
* `audit.debug`: original BindUsage plus two separate ordered visit lists.
  Each visit includes kind, exact raw structural path, full observed span when
  applicable, pass (`count` or `validate`), and actual call success. This is an
  actual visitor-call log, never the independent expected inventory
* `reference-default.*` and `reference-fuel-N.*`: actual checked.run() result
  or diagnostic and chronological trace; all runs use the same wrapper and its
  bound map/entry/body. Fuel controls only reduce the existing resource limit
* `native-default.ll` / diagnostics: actual checked.native_module() output on
  the same wrapper; emitted bytes and SHA256 are recorded by the controller.
  This is LLVM generation only. No ELF/native execution claim is made

The mechanical Rust Debug parser preserves all fields. A separate mechanical
normalizer converts IDs to integers, spans to [file,start,end], Option to null
or value, tuple enum payloads to named fields and records full tags. It rejects
unknown raw enum variants. It does not infer source permissions or targets.
The independently authored raw_span_oracle.py remains the expected inventory.

Audit instrumentation registers addresses of actual raw stored fields using a
separate exhaustive Rust raw walker. It never calls the production audit.
Actual audit span/declaration calls look up the referenced stored field address
and append that path. Missing registration produces an unresolved path; a
missing or repeated audit branch produces a missing or repeated log entry.
The independent Python raw inventory checks both passes by multiset. In
particular equal spans stored in different fields cannot conceal duplication.
The original copied DiagnosticOrigins Option input remains unchanged; a
cfg(test)-only additional reference points to its actual stored Option. Both
field identities are logged using this reference while the original value
copies still supply the actual checks. Values/order/accounting remain unchanged.

Runtime trace rows record actual operation-start context (route, function,
activation where present, block, statement/terminator/merge, full raw payload),
charge attempts (cost, available fuel, full origin, allowed), actual existing
consumer events unchanged, committed operation boundaries and actual owner
state/generation transitions. A denied charge is retained; no commit or unwind
is synthesized. Existing Event variants and their definitions remain intact.
The test-only journal has an enabled flag; all hooks are cfg(test), and the
ordinary public parser/driver/source-selection path is untouched.

Test journal ceilings are 250,000 rows, 64MiB charged logical journal bytes, 64MiB per artifact,
and 256MiB aggregate uncompressed output per request, using checked arithmetic.
An exceeded ceiling panics explicitly as OBSERVATION_LIMIT, which the controller
classifies as observer failure, never semantic rejection. No truncated evidence
is accepted. Complete text artifacts may be losslessly gzip-compressed with
both uncompressed length/hash and compressed hash bound in the receipt.
Debug serialization uses a bounded formatter; container spare capacity and
allocator overhead are not claimed to fit the logical-byte ledger. This
instrumentation's allocations are separate from the production audit's
allocation-free behavior. No observer memory figure qualifies core resources.

Exact patch is generated as `overlay.patch`; it applies only to a hash-verified
minimal source copy. Original core-v1 and all frozen artifacts remain untouched.
No full source corpus run is authorized until schema/overlay and independent
expectations have been reviewed. An approved small hand-prescribed smoke is
separate from qualification.
