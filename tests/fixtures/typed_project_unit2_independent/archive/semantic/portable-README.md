# Frozen Unit2 private qualification packet

The 1.05 MB compressed byte corpus preserves all 3,603 final cases and their
independent expectations without checking in 12,867 generated source files.
It is a static export, not a replacement semantic model. Sources total 366,806
bytes; the bounded uncompressed corpus is 13,747,865 bytes.

Distinct cases: 93 original oracle examples, 2,884 exhaustive cases within the
declared small-tree visibility family, 512 initial import cases plus 64 reviewed
module-B replacements, 34 parser/loading cases, seven direct legacy scheduling
cases, and nine grammar controls. Replacements do not increase the distinct count.
The original invalid module-a/A inputs and initial v3 failures remain in the
external review archive; they are not silently treated as passing index tests.

Run materialize.py into a fresh temporary directory. It verifies compressed,
uncompressed, case-ID, request and source identities, then writes only source
files and source-only requests into the observer input tree. Expected structures
stay in the corpus, used exclusively by compare.py. Source creation order is
retained, including the opposite orders of the original A pair.

observer.rs is the actual reviewed cfg(test) adapter. Install it as
src/frontend/oir/unit2_observer.rs in an isolated compiler copy, declare its
cfg(test), frontend-visible module in oir/mod.rs, and apply only the reviewed
cfg(test) read-source-callback.patch. The helper records actual program-level
Loader::read_source entries, preserving native encoded path bytes as hex,
display spelling and origin. This is neither OS-open tracing nor a new native
path-policy proof. Non-test compiler behavior is unchanged by the adapter.

Run exactly frontend::oir::unit2_observer::observe_source_queue with
OXID_OBSERVER_QUEUE pointing at queue.tsv and OXID_OBSERVER_OUTPUT at a fresh
raw JSONL file. Build/run debug and release, with separate receipts binding
compiler source, adapter, corpus/request identities, toolchain, binary, argv,
exit status, output hashes, unique expected IDs and exact terminal case count.
normalize.py reads only raw observations and source-only requests. compare.py
uses the unchanged structural comparator plus explicitly reviewed prose and
metadata amendments. Pass it profile, normalized JSONL and a fresh report path.

The original three scalar cap cases deliberately call the direct legacy scalar
API although ordinary syntax routing would select owned. They are not public
or actual-route evidence. The seven predecessor byte comparisons and resource
review's nine original controls are separately recorded historical/resource
evidence, not part of this portable corpus's 3,603-case semantic comparison.

Use an outer runner to require successful process completion and exact IDs,
profiles and candidate identities; a comparator match on a truncated or stale
file is insufficient. The producer owns the zero/missing/duplicate/stale receipt
negative controls and repository integration. No public project syntax, linked
ownership execution or native target is activated by this package.
