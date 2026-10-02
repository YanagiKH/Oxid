# Independent mutation application validation

`application.validate(mutation, baseline_actual, mutant_actual, artifacts, applied)`
returns `applied`, `invalid`, or `evidence-missing`. It also accepts the complete
frozen request object. This is an application proof, not a verifier-outcome
classification. The comparison driver separately authenticates the frozen
request, controller, observer, overlay, binary/profile, and artifact bytes and
compares the expected outcome.

Artifacts are a mapping from filenames to parsed values; `{identity,value}`
wrappers are also accepted. Mutation before/after raw and both generic-probe raw
inputs must be parsed with the observer's `canonical(raw=True)`. Other artifacts
and the applied marker use ordinary canonical parsing.

For every raw mutation, the before tree must equal the unmutated baseline raw,
the after tree must equal the raw actually submitted to the association audit,
and any immutable checked raw must equal that after tree. Validation constructs
exactly the requested structural edit and compares the entire result. It rejects
unrelated extra changes even when every actual observation is edited consistently.
The applied marker is checked against the independently chosen edit; it is never
used to select a site. Limited omitted `.kind` marker labels are accepted without
changing the canonical raw path. Comparisons distinguish JSON booleans and ints.

Span categories and occurrence order come from the frozen independent
`raw_span_oracle.inventory`, including nested scalar fields in owned raw. Foreign
file replacement is verified against the observed source map, valid UTF-8 byte
ranges, and, when the old offsets do not fit, the observed checked first identifier
token. No source names are re-resolved and no compiler is invoked. Function names
used by frozen non-span selections are direct slices of existing declaration
spans. Other non-span rules check IDs, appended duplicates, call descriptor and
slot types, nominal field records, ownership roles, moved parameter returns,
nested call parent edges, retained loan descriptors, exact lexical cleanup
statement edits, and diagnostic origin fields. Return transfer requires a prior
parameter move on every reachable raw path to the selected return. Missing
cleanup additionally checks reachable raw paths for later access or StorageLive.

The entry seam joins the observed local/typed/frozen entry tuple to the unchanged
declaration index. The nonzero source-file seam joins the actual two-file map and
requires the exhaustive raw span inventory to change only file 0 to file 1.
Source-map substitutions require an actual changed allocation identity plus the
requested path/byte relationship. The stale-parser-generation request requires
`constructor-parser-identity.debug`: original source identity, distinct substitute
identity, and actual AST belongs_to predicates `[true,true,false]`. The source ID
must equal the unchanged active map and baseline source ID, and the actual early
E0500 bind rejection must precede raw construction/traversal/verification. Without
this separately authenticated supplementary capture it is `evidence-missing`;
ordinary Debug SourceProvenance omits generation identity.

Visitor seams require an exhaustive baseline count/validation journal, unchanged
count traversal, and exactly one omitted or repeated validation occurrence with
the requested structural category. Count seams use the reviewed overlay's actual
numeric counter reads, corroborated independently by the inventory and actual
journal: count-total requires both full traversals unchanged and exactly +1;
overflow requires precisely the count prefix ending at the first failed span,
zero preceding successful spans, and usize::MAX on the authenticated x86_64
build. Both require a real E0500 association denial, absent successful usage,
and no raw-verifier start or checked raw. Neither accepts a numeric marker alone.
Driver-boundary requests require their separately attributed driver operation and
restoration receipts and are never inferred from raw/application markers.

Run `python -B test_application.py`. Its JSON report labels actual immutable smoke
specimens separately from fabricated in-memory edits on real retained positive
raw. The latter test validator behavior, not compiler mutation coverage. Tests
exercise all 71 frozen span requests; explicit non-span sites, real count/visitor
journals, and source-map seams; changed-marker and extra-diff rejection; and the
remaining missing-evidence behavior. Original specimens and frozen requests are
never modified.

An applicability gap is recorded for the original `raw-wrong_scalar_result`
request: `control-span-variants-scalar` has only one call and its result local17
is Unit. There is no opposite I32/Bool result slot in that immutable baseline.
Changing Unit to Bool is rejected; a separately reviewed request supplement is
needed to choose a suitable existing source control. The test suite additionally
checks the independently approved `mutation-applicability-v1` request selecting
the already-qualified scalar `left::helper` I32-call control, while retaining the
original inapplicability result.
