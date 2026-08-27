# Cache

Preprocessed source is cached under `.oxid/cache/preprocess/` using a deterministic content fingerprint. Set `OXID_CACHE_DIR` to place it elsewhere. If the cache location is read-only, Oxid continues with in-memory preprocessing instead of making source execution fail.

Remote Git dependencies are materialized separately under `.oxid/deps/<name>`. `--offline` and `--locked` never fetch an unavailable network commit; cached origins and commits are verified before use.

`oxid clean` removes the project `.oxid` directory, including build artifacts, bootstrap artifacts, caches, and dependency checkouts. Restore remote dependencies with `oxid fetch` or `oxid install` afterward.

