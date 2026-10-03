# Typed-project Unit4 source contracts

This package preserves the independently reviewed parser and public-CLI expectation contracts and their exact source authorities. It is a source-only checkpoint; it contains no new compiler observations, native receipts or activation result.

The accepted identities are:

- Public v3 freeze: `12b40d321014719805f4864f297ba3598a3c4c5ec6f416de99a0218de68f2a85`
- Parser v1 package freeze: `7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027`
- Parser decoded contract: `b19819e2e4af627dfe3877ef7753fe237aa7830b16d2a83197fae0ed02010cbc`

The replacement contracts have new identities. They do not recover the lost historical Unit4 files. Published Unit1/2/3 expectations remain unchanged. The transport records the superseded v1/v2 identities and corrected findings without duplicating their rejected corpora. `publication-summary.json` describes the technical scope and remaining qualification boundaries.

## Transport and path resolution

`authorities.tar.gz` contains 158 regular files. There are no loose Oxid sources in this repository package. Each archive name is the original absolute logical path with its first slash removed. The transport manifest binds every original path, member size/hash, the compressed archive, the decoded tar, and each nested gzip's decoded bytes. All frozen authority files are retained byte for byte, including their historical provenance and limited admitted scopes.

Historical absolute paths must be resolved through the exact member table, never by falling back to files on the current machine. After verified extraction, the mapping is `destination / archive_path`. Relative paths inside each frozen package retain their original local relationships. The transported authoring scripts retain their original path constants; they are provenance assets, not automatically portable execution entrypoints. A later adapter must use the supplied resolver and separately bind its own implementation.

```sh
python3 -B tests/fixtures/typed_project_unit4_contracts/transport.py verify
python3 -B tests/fixtures/typed_project_unit4_contracts/test_transport.py
python3 -B tests/fixtures/typed_project_unit4_contracts/transport.py extract \
  --destination /tmp/oxid-unit4-contracts-fresh
python3 -B tests/fixtures/typed_project_unit4_contracts/transport.py resolve \
  --destination /tmp/oxid-unit4-contracts-fresh \
  --logical-path /workspace/shared/oxid-reset-recovery-20261003/new-contracts/public-v3/PUBLIC-FREEZE-v3.json
```

Choose a fresh destination outside the repository, with an existing real-directory parent. Extraction rejects an existing destination, including a symlink. All package bytes, member headers, names, sizes, hashes, nested decoded identities and accepted contract identities are checked before the destination is created. The helper never calls tar extraction routines. It writes only validated regular files using exclusive creation and then verifies the complete materialized file/directory inventory. A filesystem write failure may leave an incomplete fresh directory; it is not a usable result and is never silently reused.

Production admission pins the exact transport-manifest SHA256 and both accepted freeze identities. Synthetic tests replace trust roots only within their own process to exercise the generic format boundaries. There is no CLI option that silently trusts a replacement manifest. Updating the package requires a new reviewed identity and an explicit transport-pin change.

The helper assumes a stable filesystem during verification/materialization. It rejects pre-existing symlink/reparse ancestry and does not claim resistance to an adversary concurrently mutating the destination using the same filesystem privileges.

## Resource bounds and controls

Admission caps manifest bytes at 1 MiB, compressed archive bytes at 8 MiB, decoded tar at 32 MiB, individual members at 8 MiB, the member count at 512, and one nested decoded payload at 128 MiB. The decoded tar and up to 32 MiB of retained member slices coexist; one nested decode is released after its identity check. Compressed input, temporary read/join/decompression copies and Python object/allocator overhead also consume memory. These payload limits are not an exact peak-RSS claim. This package's actual sizes and largest nested decoded identity are in the manifest.

The synthetic controls cover stale authority pairs, missing/extra/duplicate inventories, unsafe/portable-colliding paths, file/directory collisions, symlink/hardlink/directory/PAX headers, noncanonical metadata and padding, compressed/member/decoded corruption, truncated/concatenated/trailing gzip streams, decompression bounds, destination preservation, symlink ancestry and altered materialized trees. They do not execute the compiler or replace source-contract qualification.
