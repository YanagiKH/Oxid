"""Inactive outer source runtime for the exact published formatter checkpoint.

A separately reviewed dispatcher must pin this module, every imported helper,
the unchanged byte-storage runner, and the fixed seal before calling install().
The dispatcher itself must be authenticated before import by the independent
final exact-head CI/adapter closure, not by its own package manifest. The source
seal must not pin that manifest: manifest -> dispatcher -> seal would be cyclic.
The seal is an explicit caller input, never a CLI or environment override. This
module generates no authority, changes no retained executable, and has no entry
point. Its historical preflight runs only inside the authenticated M view.
"""
import copy
import json
from pathlib import Path
import stat
import sys
import types

import adapters
import current as retained_current
import published_transition
from published_view import predecessor_view


PACKAGE_PATH = "tests/fixtures/typed_project_source_binding"
PACKAGE_MANIFEST = "package-manifest.json"
RETAINED_ACCOUNTING_KEY = retained_current.RETAINED_ACCOUNTING_KEY
LEXER_ACCOUNTING_KEY = retained_current.CURRENT_ACCOUNTING_KEY
CURRENT_ACCOUNTING_KEY = "published_source_unit2_accounting_source_binding"
HISTORICAL_PACKAGE_MANIFEST = {
    "path": PACKAGE_MANIFEST,
    "bytes": 15335,
    "sha256": "5fb5ee78a42609dab51f8e8bb23c325305954f49c52f0d18df6bbe47e57548bc",
}
HISTORICAL_PACKAGE_ALIAS = (
    "tests/qualification/published_formatter_current/predecessor-v1/"
    "tests/fixtures/typed_project_source_binding/package-manifest.json"
)
CURRENT_SOURCE_ARTIFACTS = (
    ("source_manifest", published_transition.CURRENT_NAME),
    ("authority", published_transition.AUTHORITY_NAME),
    ("patch", published_transition.PATCH_NAME),
    ("source_proof", published_transition.PROOF_NAME),
)
REPLACED_CAPTURE_FIELDS = frozenset((
    "current", "inputs", "package_bytes", "package_manifest",
))
PUBLISHED_CAPTURE_FIELDS = frozenset((
    "published_predecessor", "published_source_authority",
    "published_source_proof", "published_source_seal", "published_source_touched",
    "published_package_manifest",
    CURRENT_ACCOUNTING_KEY,
))
CURRENT_ACCOUNTING_FIELDS = retained_current.CURRENT_ACCOUNTING_FIELDS | {
    "published_main", "source_checkpoint", "retained_historical_source",
    "published_source_authority", "published_source_proof",
    "published_source_transition",
}


def same(left, right):
    """Exact recursive data equality; bool/int/float are distinct identities."""
    if type(left) is not type(right):
        return False
    if type(left) is dict:
        return left.keys() == right.keys() and all(same(left[key], right[key]) for key in left)
    if type(left) in (list, tuple):
        return len(left) == len(right) and all(same(a, b) for a, b in zip(left, right))
    return left == right


def _verify(body, expected, name):
    adapters.need(same(adapters.binding(name, body), expected), "unapproved complete body: " + name)


def _pure(value, forbidden_path=None):
    """Reject live helpers, filesystem objects, and escaped view path strings."""
    if value is None or type(value) in (bool, int, float, bytes):
        return
    if type(value) is str:
        adapters.need(forbidden_path is None or forbidden_path not in value,
                      "temporary predecessor path escaped admission")
        return
    if type(value) in (list, tuple):
        for item in value:
            _pure(item, forbidden_path)
        return
    adapters.need(type(value) is dict, "non-data object escaped source admission")
    for key, item in value.items():
        adapters.need(type(key) is str, "non-string captured data key")
        _pure(key, forbidden_path)
        _pure(item, forbidden_path)


def _fixed_seal(seal):
    adapters.need(type(seal) is dict
                  and set(seal) == published_transition.SEAL_FIELDS
                  and seal["schema"] == "oxid-published-main-source-seal-v1",
                  "missing fixed published-source seal")
    _pure(seal)
    for field in published_transition.SEAL_FIELDS - {"schema"}:
        descriptor = seal[field]
        adapters.need(type(descriptor) is dict
                      and set(descriptor) == {"path", "bytes", "sha256"}
                      and type(descriptor["bytes"]) is int
                      and descriptor["bytes"] > 0
                      and type(descriptor["sha256"]) is str
                      and len(descriptor["sha256"]) == 64
                      and all(c in "0123456789abcdef" for c in descriptor["sha256"]),
                      "invalid fixed source descriptor: " + field)
        retained_current.source_transition.relative(descriptor["path"])
    adapters.need("package_manifest" not in seal,
                  "cyclic current-package self-pin is not permitted")


def _regular(root, name):
    relative = retained_current.source_transition.relative(name)
    path = Path(root) / relative
    adapters.need(not path.is_symlink()
                  and all(not parent.is_symlink() for parent in path.parents),
                  "symlink source package path: " + relative)
    mode = path.stat().st_mode
    adapters.need(stat.S_ISREG(mode) and not mode & 0o111,
                  "nonregular or executable source package member: " + relative)
    return path


def _package_rows(raw, descriptor=None):
    if descriptor is not None:
        _verify(raw, descriptor, PACKAGE_MANIFEST)
    manifest = json.loads(raw)
    rows = retained_current.source_transition.rows_map(manifest["files"])
    adapters.need(PACKAGE_MANIFEST not in rows, "self-listed source package manifest")
    return rows


def _package_map(bodies, raw, descriptor):
    rows = _package_rows(raw, descriptor)
    adapters.need(set(bodies) == set(rows), "changed complete captured package map")
    for name, row in rows.items():
        _verify(bodies[name], row, name)
    return rows


def _current_names(old_rows):
    additions = {name for _, name in CURRENT_SOURCE_ARTIFACTS}
    adapters.need(len(old_rows) == 95 and len(additions) == 4
                  and not set(old_rows).intersection(additions),
                  "wrong historical package or named source-artifact roster")
    return set(old_rows) | additions


def _current_package_map(bodies, raw, historical_raw, seal):
    """Derive the exact current roster without a transitive dispatcher self-pin."""
    old_rows = _package_rows(historical_raw, HISTORICAL_PACKAGE_MANIFEST)
    rows = _package_rows(raw)
    expected = _current_names(old_rows)
    adapters.need(set(rows) == set(bodies) == expected and len(rows) == 99,
                  "missing or extra current source-package row")
    old_manifest, manifest = json.loads(historical_raw), json.loads(raw)
    adapters.need(same({k: v for k, v in manifest.items() if k != "files"},
                       {k: v for k, v in old_manifest.items() if k != "files"}),
                  "changed retained package-manifest metadata")
    for name, row in rows.items():
        _verify(bodies[name], row, name)
    for name, row in old_rows.items():
        if name not in ("run.py", "current-source.json"):
            _verify(bodies[name], row, name)
    for field, name in CURRENT_SOURCE_ARTIFACTS:
        _verify(bodies[name], seal[field], name)
    adapters.need(bodies["current-source.json"]
                  == bodies[published_transition.CURRENT_NAME],
                  "current alias differs from named published source")
    # run.py has exact row/body consistency above. Its authenticity comes only
    # from the independent final CI/adapter pre-import closure. No digest here
    # claims to authenticate the dispatcher that pins this helper and seal.
    return rows


def _current_package(repo, package, seal):
    """Reject unknown rows before reading bodies or materializing an M view."""
    _fixed_seal(seal)
    package = Path(package)
    historical_raw = _regular(repo, HISTORICAL_PACKAGE_ALIAS).read_bytes()
    old_rows = _package_rows(historical_raw, HISTORICAL_PACKAGE_MANIFEST)
    expected = _current_names(old_rows)
    raw = _regular(package, PACKAGE_MANIFEST).read_bytes()
    rows = _package_rows(raw)
    adapters.need(set(rows) == expected, "unknown current source-package member")
    retained_current.closed_package(package, expected | {PACKAGE_MANIFEST})
    bodies = {}
    for name, row in rows.items():
        body = _regular(package, name).read_bytes()
        _verify(body, row, name)
        bodies[name] = body
    _current_package_map(bodies, raw, historical_raw, seal)
    return raw, bodies


def _historical_package(historical, current_package_bytes):
    """Preserve every old member, with only the explicit current aliases replaced."""
    old = historical["package_bytes"]
    _package_map(old, historical["package_manifest"], HISTORICAL_PACKAGE_MANIFEST)
    for name, body in old.items():
        if name not in ("run.py", "current-source.json"):
            adapters.need(current_package_bytes.get(name) == body,
                          "changed immutable historical package member: " + name)
    adapters.need(old["current-source.json"]
                  == old[published_transition.PREDECESSOR_NAME]
                  == current_package_bytes[published_transition.PREDECESSOR_NAME],
                  "changed historical lexer manifest or its named alias")


def _source_capture(captured, seal):
    """Recheck pure captured identities without materializing a second M view."""
    _fixed_seal(seal)
    _pure(captured)
    adapters.need(same(captured["published_source_seal"], seal),
                  "captured source seal differs from pinned dispatcher seal")
    historical = captured["published_predecessor"]
    adapters.need(not set(historical).intersection(PUBLISHED_CAPTURE_FIELDS)
                  and set(captured) == set(historical) | PUBLISHED_CAPTURE_FIELDS,
                  "wrong-stage or incomplete historical capture transport")
    for field, value in historical.items():
        if field not in REPLACED_CAPTURE_FIELDS:
            adapters.need(same(captured[field], value),
                          "relabelled historical source capture: " + field)
    package_bytes = captured["package_bytes"]
    _current_package_map(package_bytes, captured["package_manifest"],
                         historical["package_manifest"], seal)
    adapters.need(same(captured["published_package_manifest"],
                       adapters.binding(PACKAGE_MANIFEST, captured["package_manifest"])),
                  "changed recorded current package-manifest identity")
    _historical_package(historical, package_bytes)
    old_package = historical["package_bytes"]
    adapters.need(same(historical["u8_index_resource_authority"],
                       json.loads(old_package["unit2-u8-resource-authority.json"])),
                  "changed retained accounting authority")
    adapters.need(same(historical["byte_storage_source"], json.loads(
        old_package[retained_current.source_transition.PREDECESSOR_NAME])),
        "wrong retained accounting source identity")
    old_authority = json.loads(old_package[retained_current.source_transition.AUTHORITY_NAME])
    adapters.need(same(historical["lexer_reservation_seal"], retained_current.SEAL)
                  and same(historical["lexer_reservation_authority"], old_authority)
                  and same(historical["lexer_reservation_touched"], old_authority["transition_paths"]),
                  "changed current or historical source association")
    adapters.need(type(historical["lexer_reservation_retained_package_manifest"]) is bytes
                  and adapters.sha(historical["lexer_reservation_retained_package_manifest"])
                  == retained_current.source_transition.RETAINED_PACKAGE_SHA,
                  "changed immutable predecessor package manifest")
    current = published_transition.pinned(
        package_bytes, seal["source_manifest"], published_transition.CURRENT_NAME)
    authority = published_transition.pinned(
        package_bytes, seal["authority"], published_transition.AUTHORITY_NAME)
    proof = published_transition.pinned(
        package_bytes, seal["source_proof"], published_transition.PROOF_NAME)
    _verify(package_bytes[published_transition.PATCH_NAME],
            seal["patch"], published_transition.PATCH_NAME)
    predecessor = json.loads(package_bytes[published_transition.PREDECESSOR_NAME])
    adapters.need(package_bytes["current-source.json"]
                  == package_bytes[published_transition.CURRENT_NAME]
                  and same(captured["current"], current)
                  and same(captured["published_source_authority"], authority)
                  and same(captured["published_source_proof"], proof)
                  and same(historical["current"], predecessor)
                  and same(historical["lexer_reservation_source"], predecessor),
                  "changed current or historical source association")
    adapters.need(current["reviewed_source_head"]
                  == published_transition.SOURCE_CHECKPOINT["head"]
                  and current["source_only_tree"]
                  == published_transition.SOURCE_CHECKPOINT["tree"]
                  and predecessor["reviewed_source_head"]
                  == published_transition.HISTORICAL_SOURCE["head"]
                  and predecessor["source_only_tree"]
                  == published_transition.HISTORICAL_SOURCE["tree"],
                  "current or retained historical checkpoint was relabelled")
    for inputs, manifest in ((captured["inputs"], current),
                             (historical["inputs"], predecessor)):
        rows = retained_current.source_transition.rows_map(manifest["files"])
        adapters.need(set(inputs) == set(rows), "changed complete captured source map")
        for name, row in rows.items():
            _verify(inputs[name], row, name)
    adapters.need(same(captured["published_source_touched"],
                       list(published_transition.TRANSITION_PATHS))
                  and set(captured["inputs"]) == set(historical["inputs"])
                  and [name for name in sorted(captured["inputs"])
                       if captured["inputs"][name] != historical["inputs"][name]]
                  == list(published_transition.TRANSITION_PATHS),
                  "wrong-stage or changed complete published source transition")
    return historical, current


def preflight(repo, package, seal):
    """Admit C, then run unchanged old preflight only against authenticated M."""
    adapters.need(sys.flags.optimize == 0 and __debug__,
                  "optimized Python is not supported")
    repo, package = Path(repo), Path(package)
    fixed_seal = copy.deepcopy(seal)
    raw, package_bytes = _current_package(repo, package, fixed_seal)
    # The unchanged public API is authenticated by capture_retained before load.
    # It supplies pure inverse/include operations; its old preflight is not used
    # against C. No live module is stored in the returned capture.
    _, _, api = retained_current.capture_retained(package)
    admitted = published_transition.admit(repo, package_bytes, fixed_seal, api)
    with predecessor_view(repo, admitted=admitted,
                          seal=fixed_seal["predecessor_view"]) as view:
        historical_package = view / PACKAGE_PATH
        _verify(_regular(historical_package, PACKAGE_MANIFEST).read_bytes(),
                HISTORICAL_PACKAGE_MANIFEST, PACKAGE_MANIFEST)
        historical = retained_current.preflight(view, historical_package)
        _pure(historical, str(view))
        adapters.need(same(historical["inputs"], admitted["predecessor_inputs"])
                      and same(historical["current"], admitted["predecessor"])
                      and len(historical["archived"]) == 117,
                      "historical admission changed the complete M source map")
        historical = copy.deepcopy(historical)
    # All temporary predecessor scopes have closed before returning or invoking
    # a current-source consumer. The original old capture remains intact.
    captured = {
        **copy.deepcopy(historical),
        "current": copy.deepcopy(admitted["current"]),
        "inputs": dict(admitted["inputs"]),
        "package_bytes": package_bytes,
        "package_manifest": raw,
        "published_predecessor": historical,
        "published_source_authority": copy.deepcopy(admitted["authority"]),
        "published_source_proof": copy.deepcopy(admitted["source_proof"]),
        "published_source_seal": fixed_seal,
        "published_source_touched": list(admitted["transition_touched"]),
        "published_package_manifest": adapters.binding(PACKAGE_MANIFEST, raw),
        CURRENT_ACCOUNTING_KEY: None,
    }
    captured[CURRENT_ACCOUNTING_KEY] = unit2_accounting_binding(captured, package, fixed_seal)
    _pure(captured)
    return captured


def unit2_accounting_binding(captured, package, seal):
    """Validate genuine old associations before binding identical accounting to C."""
    historical = captured["published_predecessor"]
    # This exact old function receives the complete OLD lexer capture and reads
    # only the unchanged byte-storage sibling. It is never handed C as old input.
    lexer_binding = retained_current.unit2_accounting_binding(historical, package)
    adapters.need(set(lexer_binding) == retained_current.CURRENT_ACCOUNTING_FIELDS
                  and same(historical[LEXER_ACCOUNTING_KEY], lexer_binding)
                  and same(captured[LEXER_ACCOUNTING_KEY], lexer_binding)
                  and same(captured[RETAINED_ACCOUNTING_KEY], historical[RETAINED_ACCOUNTING_KEY])
                  and same(historical[RETAINED_ACCOUNTING_KEY],
                           lexer_binding["predecessor_accounting_source_binding"]),
                  "changed genuine lexer or byte-storage accounting binding")
    historical, current = _source_capture(captured, seal)
    dependencies = lexer_binding["source_dependencies"]
    adapters.need(tuple(row["path"] for row in dependencies)
                  == retained_current.source_transition.ACCOUNTING_PATHS,
                  "wrong immutable accounting dependency roster")
    for row in dependencies:
        name = row["path"]
        adapters.need(any(same(row, entry) for entry in current["files"])
                      and captured["inputs"][name] == historical["inputs"][name]
                      == historical["byte_storage_inputs"][name],
                      "changed complete published accounting dependency: " + name)
        _verify(captured["inputs"][name], row, name)
    adapters.need(captured["u8_index_resource"] == historical["u8_index_resource"]
                  and same(captured["u8_index_resource_authority"],
                           historical["u8_index_resource_authority"]),
                  "changed retained derived accounting resource")
    resource = adapters.binding(lexer_binding["derived_resource"]["path"],
                                captured["u8_index_resource"])
    adapters.need(same(resource, lexer_binding["derived_resource"])
                  and same(resource, captured["u8_index_resource_authority"]["derived"]),
                  "wrong complete derived accounting resource identity")
    binding = {
        **copy.deepcopy(lexer_binding),
        "version": "unit2-published-main-identical-accounting-source-v1",
        "current_source": adapters.binding("current-source.json",
                                            captured["package_bytes"]["current-source.json"]),
        "reviewed_source_head": current["reviewed_source_head"],
        "source_only_tree": current["source_only_tree"],
        "predecessor_accounting_source_binding": copy.deepcopy(lexer_binding),
        "published_main": copy.deepcopy(published_transition.PUBLISHED_MAIN),
        "source_checkpoint": copy.deepcopy(published_transition.SOURCE_CHECKPOINT),
        "retained_historical_source": copy.deepcopy(published_transition.HISTORICAL_SOURCE),
        "published_source_authority": copy.deepcopy(seal["authority"]),
        "published_source_proof": copy.deepcopy(seal["source_proof"]),
        "published_source_transition": copy.deepcopy(seal["patch"]),
    }
    adapters.need(set(binding) == CURRENT_ACCOUNTING_FIELDS,
                  "wrong published accounting binding fields")
    return binding


def validate_unit2_accounting(captured, package, seal):
    expected = unit2_accounting_binding(captured, package, seal)
    actual = captured[CURRENT_ACCOUNTING_KEY]
    adapters.need(type(actual) is dict and set(actual) == CURRENT_ACCOUNTING_FIELDS
                  and same(actual, expected), "changed published accounting source binding")
    return expected


def outer_receipt(captured):
    """Keep old compiler identities nested under their genuine historical stage."""
    historical = captured["published_predecessor"]
    seal = captured["published_source_seal"]
    receipt = {
        "lexer_reservation_predecessor_receipt": retained_current.outer_receipt(historical),
        "published_source_provenance": {
            "schema": "oxid-published-main-source-receipt-v1",
            "published_main": copy.deepcopy(published_transition.PUBLISHED_MAIN),
            "source_checkpoint": copy.deepcopy(published_transition.SOURCE_CHECKPOINT),
            "retained_historical_source": copy.deepcopy(published_transition.HISTORICAL_SOURCE),
            "source_manifest": copy.deepcopy(seal["source_manifest"]),
            "package_manifest": copy.deepcopy(captured["published_package_manifest"]),
            "source_authority": copy.deepcopy(seal["authority"]),
            "source_proof": copy.deepcopy(seal["source_proof"]),
            "transition_patch": copy.deepcopy(seal["patch"]),
            "transition_touched": list(captured["published_source_touched"]),
            "predecessor_view": copy.deepcopy(seal["predecessor_view"]),
            "historical_source_manifest": adapters.binding(
                published_transition.PREDECESSOR_NAME,
                historical["package_bytes"]["current-source.json"]),
            "historical_package_manifest": copy.deepcopy(HISTORICAL_PACKAGE_MANIFEST),
            "source_only": True,
            "execution_qualified": False,
        },
        LEXER_ACCOUNTING_KEY: copy.deepcopy(historical[LEXER_ACCOUNTING_KEY]),
        CURRENT_ACCOUNTING_KEY: copy.deepcopy(captured[CURRENT_ACCOUNTING_KEY]),
    }
    _pure(receipt)
    return receipt


def _extend_receipt(value, extension):
    adapters.need(all(same(value[key], extension[key]) for key in set(value) & set(extension)),
                  "outer provenance would overwrite an existing receipt identity")
    return {**value, **copy.deepcopy(extension)}


def install(runtime, package, seal):
    """Wrap a fresh pinned byte-storage runner, never retained_current.install().

    Original preparation, verifier, main, observer controls, child commands,
    semantic/resource predicates, and result/failure accounting remain their
    unchanged functions. Only admission and explicit outer associations change.
    """
    _fixed_seal(seal)
    package = Path(package)
    saved = retained_current.retained_package(package)
    adapters.need(isinstance(runtime, types.ModuleType)
                  and runtime.PACKAGE == saved
                  and runtime.CURRENT_SOURCE_SHA
                  == retained_current.source_transition.PREDECESSOR_SHA
                  and same(runtime.CURRENT_SOURCE_BYTES,
                           retained_current.source_transition.PREDECESSOR_BYTES),
                  "published runtime requires the fresh byte-storage predecessor")
    adapters.need(adapters.sha(_regular(saved, "run.py").read_bytes())
                  == retained_current.source_transition.RETAINED_RUNNER_SHA,
                  "changed immutable byte-storage runtime body")
    for name in ("preflight", "prepare_archived", "prepare_unit2", "verify_unit2_result",
                 "assert_unchanged", "write_json", "main", "run_unit2_observer_controls"):
        function = getattr(runtime, name)
        adapters.need(isinstance(function, types.FunctionType)
                      and function.__globals__ is vars(runtime),
                      "wrapped or foreign byte-storage runtime function: " + name)
    fixed_seal = copy.deepcopy(seal)
    old_archived = runtime.prepare_archived
    old_prepare_unit2 = runtime.prepare_unit2
    old_verify_unit2_result = runtime.verify_unit2_result
    old_write_json = runtime.write_json
    runtime.PACKAGE = package
    runtime.__file__ = str(package / "run.py")
    runtime.CURRENT_SOURCE_SHA = fixed_seal["source_manifest"]["sha256"]
    runtime.CURRENT_SOURCE_BYTES = fixed_seal["source_manifest"]["bytes"]
    last = {}

    def admitted(repo, package=package):
        last.pop("captured", None)
        result = preflight(repo, package, fixed_seal)
        last["captured"] = result
        return result

    def assert_unchanged(repo, captured, package=package):
        fresh = admitted(repo, package)
        runtime.require(same(fresh, captured), "input identity changed during operation")

    def archived(output, captured):
        validate_unit2_accounting(captured, package, fixed_seal)
        return _extend_receipt(old_archived(output, captured), outer_receipt(captured))

    def prepare_unit2(output, captured):
        current_binding = validate_unit2_accounting(captured, package, fixed_seal)
        result = old_prepare_unit2(output, captured)
        adapters.need(same(result[RETAINED_ACCOUNTING_KEY], captured[RETAINED_ACCOUNTING_KEY]),
                      "changed prepared byte-storage accounting binding")
        return _extend_receipt(result, {
            LEXER_ACCOUNTING_KEY: captured[LEXER_ACCOUNTING_KEY],
            CURRENT_ACCOUNTING_KEY: current_binding,
        })

    def verify_unit2_result(output, captured, seam, prepare_only):
        current_binding = validate_unit2_accounting(captured, package, fixed_seal)
        adapters.need(same(seam[RETAINED_ACCOUNTING_KEY], captured[RETAINED_ACCOUNTING_KEY])
                      and same(seam[LEXER_ACCOUNTING_KEY], captured[LEXER_ACCOUNTING_KEY])
                      and same(seam[CURRENT_ACCOUNTING_KEY], current_binding),
                      "changed materialized accounting associations")
        # No retained verifier result or predicate is replaced, suppressed, or
        # interpreted as a fresh result without the original verifier succeeding.
        result = old_verify_unit2_result(output, captured, seam, prepare_only)
        selected = runtime.regular(output / "unit2/derived-package",
                                   "source-inputs.json").read_bytes()
        adapters.need(selected == captured["package_bytes"]["current-source.json"]
                      and same(adapters.binding("current-source.json", selected),
                               current_binding["current_source"]),
                      "wrong materialized published current manifest")
        source = output / "unit2/run/source"
        assembly = runtime.read_json(output / "unit2/run/assembly.json")
        adapters.need(assembly["source_inputs_sha256"]
                      == current_binding["current_source"]["sha256"],
                      "wrong current source assembly association")
        # Keep the actual admitted instrumentation map. Its five modified source
        # bodies are not incorrectly compared with uninstrumented C.
        runtime.check_entries(source, assembly["files"], exact=True)
        historical = captured["published_predecessor"]
        main = "src/main.rs"
        adapters.need(runtime.regular(source, main).read_bytes() == captured["inputs"][main]
                      and captured["inputs"][main] != historical["inputs"][main],
                      "wrong materialized published main source")
        lexer = "src/frontend/lexer.rs"
        adapters.need(runtime.regular(source, lexer).read_bytes()
                      == captured["inputs"][lexer] == historical["inputs"][lexer]
                      and captured["inputs"][lexer]
                      != historical["byte_storage_inputs"][lexer],
                      "wrong materialized current lexer")
        for row in current_binding["source_dependencies"]:
            _verify(runtime.regular(source, row["path"]).read_bytes(), row, row["path"])
        return _extend_receipt(result, {
            LEXER_ACCOUNTING_KEY: captured[LEXER_ACCOUNTING_KEY],
            CURRENT_ACCOUNTING_KEY: current_binding,
        })

    def failure_provenance(path, value, extension=None, error=None):
        """Optional metadata must never replace an already-written failure."""
        try:
            sidecar = Path(path).with_name(Path(path).name + ".published-provenance.json")
            record = {
                "schema": "oxid-published-source-failure-provenance-v1",
                "primary_receipt": Path(path).name,
                "primary_status": "failed",
                "primary_error": value.get("error"),
                "provenance_status": "rejected" if error is not None else "validated",
                "provenance_error": error,
                "provenance": extension,
            }
            old_write_json(sidecar, record)
        except (Exception, KeyboardInterrupt) as exc:
            # The primary failure is already preserved through the original
            # writer. Even reporting this secondary I/O failure cannot mask it.
            try:
                print("Could not record secondary published provenance: "
                      + type(exc).__name__ + ": " + str(exc), file=sys.stderr)
            except (Exception, KeyboardInterrupt):
                pass

    def write_json(path, value):
        if not (isinstance(value, dict)
                and value.get("schema") == "oxid-current-archive-binding-v1"
                and "captured" in last):
            old_write_json(path, value)
            return
        primary_failed = value.get("status") == "failed"
        if primary_failed:
            # Original main writes failure.json in finally, outside its try.
            # Preserve the original failure and its exact error BEFORE any
            # package reopen, capture validation, or optional provenance work.
            old_write_json(path, value)
        try:
            validate_unit2_accounting(last["captured"], package, fixed_seal)
            extension = outer_receipt(last["captured"])
            extended = _extend_receipt(value, extension)
        except (Exception, KeyboardInterrupt) as exc:
            error = type(exc).__name__ + ": " + str(exc)
            if primary_failed:
                failure_provenance(path, value, error=error)
                return
            if Path(path).name in ("result.json", "prepared.json"):
                # Terminal writes are also outside original main's try. Never
                # emit success/preparation without its required provenance, and
                # retain the attempted result separately from the failed verdict.
                failed = {
                    **value,
                    "status": "failed",
                    "semantic_pass": False,
                    "error": "published source provenance extension rejected: " + error,
                    "published_source_provenance_error": error,
                    "attempted_terminal_receipt_name": Path(path).name,
                    "attempted_terminal_receipt": copy.deepcopy(value),
                }
                old_write_json(Path(path).with_name("failure.json"), failed)
            # In-try plan writes keep original main's normal failure handling;
            # terminal rejection also exits unsuccessfully after its failure write.
            raise
        if primary_failed:
            failure_provenance(path, value, extension=extension)
            return
        old_write_json(path, extended)

    runtime.preflight = admitted
    runtime.prepare_archived = archived
    runtime.prepare_unit2 = prepare_unit2
    runtime.verify_unit2_result = verify_unit2_result
    runtime.assert_unchanged = assert_unchanged
    runtime.write_json = write_json
    return runtime
