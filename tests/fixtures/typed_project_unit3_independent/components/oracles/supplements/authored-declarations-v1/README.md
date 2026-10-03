# Source-only declaration supplement, awaiting independent review

Parent freeze is preserved unchanged:
`b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba`.
No original package file is edited. The original manifest covers its original
463 files; this directory is an explicitly separate addition with its own
manifest. The original freeze.py checks an exact directory inventory and will
correctly notice these additional files; use this supplement's per-entry parent
verification to prove all original frozen bytes remain unchanged.

The reviewer identified that authored source cases used a simpler declarations
object without field rows, whereas raw_span_oracle.check requires normalized
declarations including every field. Full association checking remains required
for those accepted cases, including all four owned overflow layout variants.

build_supplement.py reads only the original frozen sources, source hashes and
expected declaration metadata. Its independent bounded lexical scanner extracts
top-level functions/records/fields, follows explicit modules in DFS order, and
assigns dense IDs by declaration kind. It skips balanced function bodies and
never invokes the production parser/resolver or reads a candidate snapshot.
Unknown declaration/field syntax fails closed. It verifies all existing
declaration/name/field origins before emitting complete normalized rows for all
152 fixtures, so the addition cannot silently replace a previously frozen ID.

declarations.json is the complete source-derived declaration input to the
independent association comparator. Join it by case ID and source manifest,
and supply its declarations list to raw_span_oracle.check. Raw shape, execution,
diagnostics, call targets, fuel, ownership outcomes and allowed alternatives
remain those in the original freeze. This supplement does not weaken or limit
the original association obligations.

Reproduce with `python3 -B build_supplement.py`. The source-only self-check
records exactly how many missing field origins were added and how many existing
ones were independently rederived. All original file hashes are checked both
before and after generation.

## Entry mutation clarification

The existing `bind-entry_id` request means a test-only fault injection into the
constructor-local selected entry value before the owned typed-entry versus
index-root-entry agreement check. The control's real index and typed entry stay
unchanged. Change only the candidate selected local entry to DefId0 and require
E0500/oir-project-bind before a CheckedSourceProgram is constructed.

It does not request mutation of an already sealed wrapper, a production setter,
an alternate-entry constructor parameter, a raw-parts factory or corruption
after the checked-construction boundary. If the adapter cannot place this exact
test-only injection, the case is blocked pending the proper seam; it is not
replaced by post-construction corruption or counted as passing from privacy.

