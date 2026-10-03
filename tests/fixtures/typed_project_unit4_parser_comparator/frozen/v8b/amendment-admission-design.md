# Disabled source-only amendment admission

This is a design for a later checkpoint. v6 has no amendment CLI option and no
enabled route to adopt changed expectations. The original frozen package and
every raw execution identity remain untouched. No new coordinate values are
selected here; the separately authored amendment supplies them.

## Exact authorities required

The next implementation will require one new coordinator authorization, pinned
by its external SHA256, naming the executing comparator checkpoint and exact
artifact triples for effective-contract.json, amendment.json and the independent
source review. The comparator will pin the approved descriptor, amendment and
review content hashes in source after the independent review is available.
Partial, stale, duplicate, additional or unreviewed amendment authorities fail.
The original durable authority, observer/build/helper and host gates remain.

The proposed source-authored schema is at
new-contracts/parser-v1-location-amendment-v1/artifacts. It declares an ordered
single amendment, its base freeze/gzip/decoded identities, case index and ID,
exact source record, old/new whole-case and diagnostic canonical hashes, complete
old/new diagnostic subtrees, one primary-object replacement and exactly four
changed integer leaves. The descriptor binds the entire effective canonical
document. These proposed bytes alone are not adoption authority.

## Mechanical admission

1. Load and verify the original package through the unchanged load_contract gate
2. Verify descriptor, amendment and review identities and their mutual bindings
3. Require the exact base freeze, gzip bytes/hash and decoded bytes/hash
4. Bind case index/ID/source, complete old case/diagnostic/primary and old hashes
5. Deep-copy the verified base document and apply the single replacement once
6. Require all changed leaves to be exactly the four reviewed integer pointers
7. Require the full new diagnostic and whole-case hashes, then the descriptor's
   full effective canonical-document bytes and hash
8. Independently derive source positions for the source-designated pub token and
   require the amended primary to describe that entire token
9. Return the effective comparison document and its distinct identity receipt

No input files or original in-memory base document are modified. Reapplication
fails the old-document/case/diagnostic preconditions. Canonicalization follows
the descriptor: UTF-8 json.dumps with ensure_ascii=False, sort_keys=True,
separators=(',', ':'), allow_nan=False and no final newline.

## Execution versus comparison identities

execution_manifest and normalize_raw continue to validate original collection
contract/freeze hashes. The actual source/limits/seam/mode roster is unchanged
by the proven four-leaf delta. compare_rows receives the admitted effective
document only after all transport checks pass. Its result retains truthful
execution identities and adds the descriptor's mandatory effective identity,
descriptor hash, base freeze/decoded hashes, ordered amendment hashes and full
effective canonical hash. Historical receipts are never relabeled. A fresh
comparison under new approved authority is required to qualify the amendment.

## Required controls

Keep all existing tests. Add unchanged-base/one-application positive evidence;
missing review/authorization; stale/wrong descriptor, amendment or review;
changed source/roster/message/code/recognition; additional pointer; wrong old
case/diagnostic; repeated application; changed coordinate type (bool/float);
incorrect new whole-document identity; false effective result labeling; and
proof that raw normalization continues to require the original collection IDs.
The next checkpoint requires independent review before any full comparison.

## Implementation status in v8

The reviewed route is now implemented as described above, with exact final
artifact pins and mandatory new coordinator authorization. This design remains
a historical record of the disabled v6/v7 route; README.md describes current
v8 behavior. No full comparison is authorized by this checkpoint alone.
