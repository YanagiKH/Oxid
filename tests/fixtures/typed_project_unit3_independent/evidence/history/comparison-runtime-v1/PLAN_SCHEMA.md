# Portable independent comparison plan, version 1

The collector and comparator share a source-only plan. The plan contains no
expected results, diagnostics, raw IDs, spans, event facts or comparison values.
Freeze its SHA-256 before collection; pass that SHA explicitly to comparison.
Paths below are absolute, normalized physical paths. Each file identity is
exactly `{ "path": string, "bytes": integer, "sha256": string }`.

Top-level fields (no others):

```
{
  "schema": "unit3-independent-portable-plan-v1",
  "kind": "source" | "native" | "mutation",
  "scope": "full" | "bounded",
  "materialization": FILE_IDENTITY,
  "builds": { BUILD_KEY: BUILD_BINDING, ... },
  "rows": [ROW, ...]
}
```

BUILD_KEY is an arbitrary unique simple name referenced by rows. Every build
must be used. Each BUILD_BINDING has `family`, `profile`, `build`, `prepared`,
and `wrapper`. The last three are FILE_IDENTITY values. `family` is `source`,
`native`, `mutation-v2` or `mutation-v3`; `profile` is `debug` or `release`.
Native bindings additionally have `build_wrapper` (FILE_IDENTITY of the actual
portable native build wrapper receipt). Mutation bindings additionally have
`portable_build` (FILE_IDENTITY of the unchanged portable build receipt whose
overlay-only view is the `build` identity). No other binding fields are allowed.

Each source or mutation ROW has exactly these fields:

```
{
  "id": "frozen source case ID or effective mutation ID",
  "profile": "debug" | "release",
  "build": "BUILD_KEY",
  "receipt": "/absolute/future/inner/receipt.json",
  "wrapper_receipt": "/absolute/future/wrapper-receipt.json"
}
```

Each native ROW has those fields plus exactly:

```
{
  "group": "native-default" | "native-fuel" | "reference-fuel" | "driver" | "no-clobber",
  "operation": "check" | "run" | "compile",
  "format": "text" | "json",
  "fuel": null | INTEGER,
  "noclobber": null | "file" | "symlink",
  "source_root": "/materialized/root/sources/ID",
  "entry": "/materialized/root/sources/ID/main.ox",
  "output": null | "/absolute/inner/receipt-directory/program-or-occupied-output",
  "input_request_sha256": "frozen selected source request digest"
}
```

The comparator derives each allowed source-root and entry from the verified
shared materialization and original source request. Compile output is the
receipt directory's `program`, or `occupied-output` for no-clobber; check/run
output is null. Input-request hashes use the existing native controller's
`json.dumps(request, sort_keys=True)` convention. Native row selection follows
the frozen request groups; profile remains part of identity.

Full source scope is exactly 152 cases × two profiles. Full native scope is
exactly 300 primary invocations: 64 default, 36 native fuel, 36 reference fuel,
144 driver and 20 no-clobber. Full mutation scope is exactly 110 internal
mutations × two profiles, with four driver boundary IDs attributed separately
to eight native profile results. Bounded scope must be a nonempty subset of
the same finite roster and is never reported as full qualification.

Receipt paths and row keys must be unique. Missing rows, unknown IDs/profiles,
unexpected operations, unused builds, mismatched wrapper/build/source hashes,
and changed shared source membership reject. Actual streams/journals remain
unchanged. A source-only plan cannot itself certify an observation; comparison
reads and hashes the actual artifacts after verifying these associations.

The staged comparator exposes `prepare` and `compare`; it does not invoke a
candidate or LLVM. Full mutation comparison additionally requires
`--native-evidence` and `--native-evidence-sha256` for a successful full native
comparison with the same entrance and view. Its ten relevant actual receipts
are rechecked to establish the four driver requests and eight profile results.
Run with `PYTHONOPTIMIZE=0 python3 -B`; README.md gives the complete CLI.
