# Transient hosted permission-capability controller (review candidate)

This component is separate from the reviewed14-file public adapter. It is not
ready for a qualification claim until independently reviewed and exercised in
the root CI container. The local uid1000 host needs no privilege change.

The proposed join first requires a complete main collection with only the twelve
frozen permission-capability gaps. It uses an existing non-root account through
installed `setpriv`, clears supplementary groups and applies `--no-new-privs`.
Only a newly created child-owned `/tmp` directory changes ownership. Existing
paths are never chmod'ed, accounts/settings are never created or modified, and
unreadable inputs fail closed. The child re-verifies the exact same original
compiler/observer/config/build-stream paths and therefore preserves their byte
identities and inode/stat context. It materializes the frozen contract transport
into its own0700 root and uses a source-bound prebuilt trap, avoiding root's Rust
cache. Every candidate operation is a frozen source-denial operation, so LLVM is
not required in the child.

The shard is always `PARTIAL_CAPABILITY_SHARD`, with six public plus six lifecycle
tuples and raw ordinary/observer/passivity evidence. It cannot stand alone as a
full pass. The final controller re-runs every raw comparator for the complete
main roster, then replaces only unsupported-capability keys exactly once using
raw shard comparisons with the same head/source/binary/adapter/effective-contract
bindings. Missing, duplicate, extra, wrongly scoped, failed or unexecuted
replacements are rejected. It preserves the original raw receipts and records
explicit hashes for each replacement; a summary cannot substitute for raw data.

`selftest.py` covers the source-only replacement-domain controls. No real
non-root launch, ownership change or final hosted join has been exercised locally.

The read-access probe runs as the exact proposed UID/GID with no supplementary
groups before a candidate invocation. An opt-in bounded readonly-copy mode may
copy at most400 declared input files /512MiB into the fresh child-owned root.
The original configs, manifests, streams and build receipts remain byte-identical
in that transport. New, explicit path-view documents replace only declared
artifact paths and bind their immutable original recipes; the same reviewed
candidate/source/build predicate verifies the views. No build is reexecuted or
misrepresented. The join deterministically reconstructs these projections and
checks both original and physical identities. Existing paths are never chmod'ed.

The worker must be the leader of the outer controller-owned session. Its nested
controlled processes share that group, and an inner failure kills only that
owned group. Normal nested completion does not kill the worker. The controller
cleans the complete group when the worker exits or its timeout/stream bound is
reached. The policy and actual worker/session/group IDs are bound in the shard
and checked against the controller's terminal subprocess receipt.

The successor admission additionally requires each replacement's exact effective
UID/GID, empty supplementary groups, and EACCES/EPERM read-denial errno, including
the ordinary lifecycle counterpart. Executed ordinary counterparts must explicitly
have passed; nonexecuted counterparts must preserve the paired capability gap.
The readonly ledger is reconstructed from both original source manifests, every
original build dependency, and the fixed helper-package inputs. Every logical
entry, role, and physical file must match that closure exactly, before generating
or verifying a path view. An existing unlisted copied file cannot fill a missing
ledger member.

`integration_controls.py` incorporates the independent review's nine unchanged
join regression mutations. Its optional `--copy-contract-package` exercises the
same real raw-join path with explicitly synthetic receipt path projections and
byte-verified copied inputs; it never invokes a compiler or claims that synthetic
receipts are qualification evidence.
