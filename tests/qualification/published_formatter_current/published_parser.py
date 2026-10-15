"""Inactive, explicitly sealed parser transport; no activation data is bundled.

The derived portable module authenticates this support file and obtains admitted
from the pinned published_current.preflight entry point. Every retained module
is loaded at its genuine path in a fresh authenticated M view and dies before
that view closes. Returned values contain JSON data or bytes, never callables.
"""
import copy
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path, PurePosixPath
import types
import sys

DIRECTORY = "tests/qualification/unit4_parser_current/"
SUPPORT_DIRECTORY = "tests/qualification/published_formatter_current/"
PACKAGE = "tests/fixtures/typed_project_source_binding/"
OLD_PORTABLE_SHA = "3366c49f267ab98bd792a1cd1b01ebbaaca548bc19685d841eb688cf39f99162"
OLD_AUTHORITY_SHA = "d8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c"
OLD_PREPHASE_SHA = "917431fe269d807c76b2f16ae7fef7f4e6c70aa301f9823f65de4fe039b94baf"
OLD_SOURCE_SHA = "9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261"
CURRENT_NAME = "published-formatter-source-v1.json"
SOURCE_AUTHORITY_NAME = "published-formatter-authority-v1.json"
SOURCE_PROOF_NAME = "published-main-source-proof-v1.json"
PUBLISHED_MAIN = {"head": "15eac633e167c261ba0246ced49cfdc1c6cef434",
                  "tree": "f66d5ae31272268c29f520ec523ee1fa31c15ee9"}
SOURCE_CHECKPOINT = {"head": "fbff527c895e6cd4ec9350bd8b5d460bc20bb80d",
                     "tree": "89e90c3ec83d0712d0037a0407fc6571883c5e9c",
                     "parent": PUBLISHED_MAIN["head"]}
AUTHORITY_SCHEMA = "oxid-unit4-published-main-parser-authority-v1"
CANDIDATE_SCHEMA = "oxid-unit4-published-main-candidate-source-manifest-v1"
BINDING_KEY = "published_main_parser"
SEAL_FIELDS = frozenset(("schema", "authority", "support", "view",
                         "source_runtime", "source_runtime_dependencies",
                         "source_dispatcher", "source_manifest", "source_seal"))
REPLACED_FIELDS = frozenset((
    "schema", "reviewed_source_head", "source_only_tree",
    "current_source_manifest", "source_binding_runner", "source_delta",
    "current_base_files", "current_derived_files", "current_control_derived_files",
    "current_candidate_source_manifest_sha256"))
RETAINED_IMPORT_ALIASES = ("adapters", "source_transition", "seal", "current")


class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def same(actual, expected, message):
    """The retained portable.same semantics, with the support rejection type."""
    need(type(actual) is type(expected), message)
    if isinstance(expected, dict):
        need(actual.keys() == expected.keys(), message)
        for key in expected:
            same(actual[key], expected[key], message)
    elif isinstance(expected, list):
        need(len(actual) == len(expected), message)
        for left, right in zip(actual, expected):
            same(left, right, message)
    else:
        need(actual == expected, message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()


def identity(name, raw):
    return {"path": name, "bytes": len(raw), "sha256": sha(raw)}


def _pure(value, view):
    if value is None or type(value) in (bool, int, float, bytes):
        return
    if type(value) is str:
        need(str(view) not in value, "temporary predecessor path escaped parser transport")
        return
    if type(value) in (list, tuple):
        for item in value:
            _pure(item, view)
        return
    need(type(value) is dict, "live object escaped parser transport")
    for key, item in value.items():
        need(type(key) is str, "non-string parser data key")
        _pure(key, view)
        _pure(item, view)


@contextmanager
def _import_scope(view):
    """Restore known aliases and reject/clean up exact unexpected M aliases."""
    missing = object()
    # Snapshot identities for unexpected aliases too. Only the four named aliases
    # and names actually found to point into this view may be restored below.
    previous = dict(sys.modules)

    def restore():
        unexpected = []
        for name, module in list(sys.modules.items()):
            filename = vars(module).get("__file__") if isinstance(module, types.ModuleType) else None
            if isinstance(filename, str) and Path(filename).absolute().is_relative_to(view):
                if name not in RETAINED_IMPORT_ALIASES:
                    unexpected.append(name)
        unexpected.sort()
        # Restore before rejecting and before the enclosing M view closes. Other
        # newly imported modules or overwritten non-M aliases remain untouched.
        for name in (*RETAINED_IMPORT_ALIASES, *unexpected):
            module = previous.get(name, missing)
            if module is missing:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = module
        if unexpected:
            violation = Rejected("unexpected retained import aliases: " + ",".join(unexpected))
            violation.aliases = tuple(unexpected)
            raise violation

    try:
        yield
    except BaseException as operation_error:
        try:
            restore()
        except BaseException as cleanup_error:
            raise BaseExceptionGroup(
                "retained operation and import cleanup both failed",
                [operation_error, cleanup_error]) from None
        raise
    else:
        restore()


def rows_map(rows, count):
    need(type(rows) is list and len(rows) == count, "complete map cardinality")
    result = {}
    for row in rows:
        need(type(row) is dict and set(row) == {"path", "bytes", "sha256"},
             "closed map identity")
        name = row["path"]
        need(type(name) is str and name and "\\" not in name
             and not PurePosixPath(name).is_absolute()
             and str(PurePosixPath(name)) == name
             and all(x not in ("", ".", "..") for x in PurePosixPath(name).parts),
             "safe map path")
        need(name not in result and type(row["bytes"]) is int and row["bytes"] >= 0,
             "duplicate map member or invalid byte count")
        digest = row["sha256"]
        need(type(digest) is str and len(digest) == 64
             and all(c in "0123456789abcdef" for c in digest), "map digest")
        result[name] = row
    return result


def _module(p, row, name):
    p.verify_map(p.REPOSITORY, [row])
    path = p.REPOSITORY / row["path"]
    raw = path.read_bytes()
    same(identity(row["path"], raw), row, "exact second-read publication module body")
    module = types.ModuleType(name)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def _seal(p, seal):
    need(type(seal) is dict and set(seal) == SEAL_FIELDS
         and seal["schema"] == "oxid-unit4-published-main-parser-seal-v1",
         "missing fixed parser seal")
    need(seal["authority"]["path"] == DIRECTORY + "authority.json"
         and seal["authority"]["sha256"] == p.AUTHORITY_SHA,
         "current outer authority pin")
    need(seal["source_manifest"]["path"] == PACKAGE + "current-source.json"
         and seal["source_dispatcher"]["path"] == PACKAGE + "run.py",
         "fixed current source facade paths")
    need(all(seal[key]["path"] == SUPPORT_DIRECTORY + filename for key, filename in
             (("support", "published_parser.py"), ("view", "published_view.py"),
              ("source_runtime", "published_current.py"))), "fixed publication helper paths")
    p.verify_map(p.REPOSITORY, [seal[k] for k in
                 ("authority", "support", "view", "source_runtime",
                  "source_dispatcher", "source_manifest")])
    need(Path(__file__).absolute() == p.REPOSITORY / seal["support"]["path"],
         "actual pinned parser support path")
    same(identity(seal["support"]["path"], Path(__file__).read_bytes()),
         seal["support"], "actual pinned parser support")
    row = seal["authority"]
    raw = (p.REPOSITORY / row["path"]).read_bytes()
    same(identity(row["path"], raw), row, "exact second-read current authority body")
    return p.load(raw)


def _source(p, seal, admitted):
    """Authenticate the supplied preflight data again; never trust a label alone."""
    need(type(admitted) is dict and "published_predecessor" in admitted,
         "complete published current preflight capture required")
    same(admitted["published_source_seal"], seal["source_seal"],
         "wrong source admission seal")
    current = admitted["current"]
    predecessor = admitted["published_predecessor"]
    before = rows_map(predecessor["current"]["files"], 376)
    after = rows_map(current["files"], 376)
    need(list(before) == list(after)
         and [name for name in after if before[name] != after[name]] == ["src/main.rs"],
         "exact published-main source membership and one-file delta")
    for capture, expected in ((predecessor, before), (admitted, after)):
        need(set(capture["inputs"]) == set(expected), "complete captured source bodies")
        for name, row in expected.items():
            need(type(capture["inputs"][name]) is bytes, "captured source body type")
            same(identity(name, capture["inputs"][name]), row,
                 "captured source body identity")
    need(all(admitted["inputs"][n] == predecessor["inputs"][n]
             for n in after if n != "src/main.rs"), "unexplained source body change")
    p.verify_map(p.REPOSITORY, current["files"])
    source_raw = (p.REPOSITORY / seal["source_manifest"]["path"]).read_bytes()
    same(p.load(source_raw), current, "exact current manifest capture")
    same(source_raw, admitted["package_bytes"]["current-source.json"],
         "exact current manifest bytes and aliases")
    same(source_raw, admitted["package_bytes"][CURRENT_NAME],
         "exact current manifest bytes and aliases")
    source_seal = seal["source_seal"]
    for key, name, captured in (
            ("authority", SOURCE_AUTHORITY_NAME, "published_source_authority"),
            ("source_proof", SOURCE_PROOF_NAME, "published_source_proof")):
        row = source_seal[key]
        need(row["path"] == name, "named published source evidence")
        raw = admitted["package_bytes"][name]
        same(identity(name, raw), row, "published source evidence identity")
        same(p.load(raw), admitted[captured], "published source evidence capture")
        p.verify_map(p.REPOSITORY, [{**row, "path": PACKAGE + name}])
    need(source_seal["source_manifest"]["path"] == CURRENT_NAME,
         "source seal current manifest path")
    same(identity(CURRENT_NAME, source_raw), source_seal["source_manifest"],
         "source seal current manifest")
    transition = admitted["published_source_authority"]
    proof = admitted["published_source_proof"]
    for evidence in (transition, proof):
        same(evidence["published_main"], PUBLISHED_MAIN, "published main provenance")
        same(evidence["source_checkpoint"], SOURCE_CHECKPOINT, "published source checkpoint")
    same(transition["transition_paths"], ["src/main.rs"], "published source transition scope")
    need(current["reviewed_source_head"] == SOURCE_CHECKPOINT["head"]
         and current["source_only_tree"] == SOURCE_CHECKPOINT["tree"]
         and current["published_main_head"] == PUBLISHED_MAIN["head"]
         and current["published_main_tree"] == PUBLISHED_MAIN["tree"]
         and current["published_source_predecessor_sha256"] == OLD_SOURCE_SHA,
         "published source provenance")
    need(transition["source_only"] is True and transition["execution_qualified"] is False,
         "source transport cannot claim execution qualification")
    return current, predecessor, before, after


def current_candidate(a):
    active = a["current"]
    binding = active[BINDING_KEY]
    need(active["schema"] == AUTHORITY_SCHEMA
         and binding["stage"] in ("phase", "prephase"), "current candidate stage")
    rows_map(active["current_base_files"], 539)
    return {"schema": CANDIDATE_SCHEMA, "historical_commit": a["base_commit"],
            "reviewed_source_head": active["reviewed_source_head"],
            "source_only_tree": active["source_only_tree"],
            "current_source_manifest_sha256": active["current_source_manifest"]["sha256"],
            "source_provenance": copy.deepcopy(binding["source_provenance"]),
            "files": copy.deepcopy(active["current_base_files"])}


def _transport(old, seal, admitted, stage):
    need(stage in ("phase", "prephase") and BINDING_KEY not in old["current"],
         "wrong transport stage or double transport")
    need(("lexer_phase" in old["current"]) == (stage == "phase"),
         "exact historical phase stage")
    same(old["current_source"], admitted["published_predecessor"]["current"],
         "complete admitted M manifest matches historical parser association")
    current = admitted["current"]
    before = rows_map(old["current_source"]["files"], 376)
    after = rows_map(current["files"], 376)
    need(list(before) == list(after)
         and [n for n in before if before[n] != after[n]] == ["src/main.rs"],
         "exact parser source transport")
    original = rows_map(old["original_files"], 283)
    for field in ("instrumentation", "control_instrumentation"):
        need("src/main.rs" not in {r["path"] for r in old[field]},
             "main must remain uninstrumented")
    historical_delta = [{"path": n, "before": original.get(n), "after": r}
                        for n, r in after.items() if original.get(n) != r]
    need(len(historical_delta) == len(old["current"]["source_delta"]) == 341
         and [r["path"] for r in historical_delta]
         == [r["path"] for r in old["current"]["source_delta"]],
         "unchanged historical-to-current341 delta roster")
    expected_old_delta = copy.deepcopy(historical_delta)
    next(r for r in expected_old_delta if r["path"] == "src/main.rs")["after"] = before["src/main.rs"]
    same(expected_old_delta, old["current"]["source_delta"],
         "source delta changed outside complete main after identity")
    merged = original | after
    base = [merged[n] for n in sorted(merged)]
    old_base = rows_map(old["current"]["current_base_files"], 539)
    need(len(base) == 539 and set(merged) == set(old_base), "exact base539 membership")
    restored_base = [before["src/main.rs"] if r["path"] == "src/main.rs" else r for r in base]
    same(restored_base, old["current"]["current_base_files"], "complete base inverse")
    active = copy.deepcopy(old["current"])
    active.update(schema=AUTHORITY_SCHEMA, reviewed_source_head=SOURCE_CHECKPOINT["head"],
                  source_only_tree=SOURCE_CHECKPOINT["tree"],
                  current_source_manifest=copy.deepcopy(seal["source_manifest"]),
                  source_binding_runner=copy.deepcopy(seal["source_dispatcher"]),
                  source_delta=historical_delta, current_base_files=base)
    provenance = {
        "published_main": PUBLISHED_MAIN, "source_checkpoint": SOURCE_CHECKPOINT,
        "retained_historical_source": admitted["published_source_authority"]["retained_historical_source"],
        "predecessor_source_manifest": copy.deepcopy(old["current"]["current_source_manifest"]),
        "current_source_manifest": copy.deepcopy(seal["source_manifest"]),
        "source_transition_authority": {**seal["source_seal"]["authority"],
                                        "path": PACKAGE + SOURCE_AUTHORITY_NAME},
        "source_proof": {**seal["source_seal"]["source_proof"],
                         "path": PACKAGE + SOURCE_PROOF_NAME},
        "main_delta": [{"path": "src/main.rs", "before": before["src/main.rs"],
                        "after": after["src/main.rs"]}],
        "source_only": True, "execution_qualified": False,
    }
    active[BINDING_KEY] = {
        "version": "oxid-unit4-published-main-parser-layer-v1", "stage": stage,
        "predecessor_authority_sha256": OLD_AUTHORITY_SHA if stage == "phase" else OLD_PREPHASE_SHA,
        "source_provenance": provenance,
    }
    result = dict(copy.deepcopy(old), current=active, current_source=copy.deepcopy(current))
    candidate = encoded(current_candidate(result))
    active["current_candidate_source_manifest_sha256"] = sha(candidate)
    for field in ("current_derived_files", "current_control_derived_files"):
        old_map = rows_map(old["current"][field], 542)
        same(old_map["src/main.rs"], before["src/main.rs"], "uninstrumented predecessor main")
        derived = dict(old_map)
        derived["src/main.rs"] = after["src/main.rs"]
        derived["candidate-source-manifest.json"] = identity("candidate-source-manifest.json", candidate)
        ordered = [derived[n] for n in sorted(derived, key=lambda n: PurePosixPath(n).parts)]
        need(len(ordered) == 542, "complete current role542")
        active[field] = ordered
        inverse = [old_map[r["path"]] if r["path"] in
                   ("src/main.rs", "candidate-source-manifest.json") else r for r in ordered]
        same(inverse, old["current"][field], "complete role-map inverse")
    recovered = copy.deepcopy(active)
    del recovered[BINDING_KEY]
    for key in REPLACED_FIELDS:
        recovered[key] = copy.deepcopy(old["current"][key])
    same(recovered, old["current"], "transport changed unrelated complete authority fields")
    return result


def _retained(p, view):
    path = view / DIRECTORY / "portable.py"
    p.regular(path)
    raw = path.read_bytes()
    need(sha(raw) == OLD_PORTABLE_SHA, "exact retained complete parser body")
    module = types.ModuleType("oxid_published_parser_retained")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    need(module.REPOSITORY == view and module.AUTHORITY_SHA == OLD_AUTHORITY_SHA
         and module.PRE_PHASE_AUTHORITY_SHA == OLD_PREPHASE_SHA,
         "retained module genuine view and immutable pins")
    return module


def _view(p, seal, admitted):
    # This is deliberately a fresh module. Its local lease cannot prevent a
    # second module instance from opening another view. The reviewed caller graph
    # is synchronous/sequential; the effects audit must observe that whole graph.
    module = _module(p, seal["view"], "oxid_published_parser_view")
    return module.predecessor_view(
        p.REPOSITORY, admitted=admitted,
        seal=admitted["published_source_seal"]["predecessor_view"])


@contextmanager
def _retained_scope(p, seal, admitted):
    old = None
    try:
        with _view(p, seal, admitted) as view:
            with _import_scope(view):
                old = _retained(p, view)
                yield old, view
    except BaseException as error:
        # Translate only this exact retained rejection family, after cleanup.
        # Exception groups keep original operation/cleanup errors unchanged.
        if old is not None and isinstance(error, old.Rejected):
            raise Rejected(str(error)) from error
        raise


def authority(p, seal, admitted):
    active = _seal(p, seal)
    _source(p, seal, admitted)
    with _retained_scope(p, seal, admitted) as (old, view):
        previous = old.authority()
        result = _transport(previous, seal, admitted, "phase")
        same(result["current"], active, "exact independently sealed complete parser authority")
        _pure(result, view)
    return result


def provenance_previous(p, a, seal, admitted):
    active = _seal(p, seal)
    _source(p, seal, admitted)
    with _retained_scope(p, seal, admitted) as (old, view):
        previous = old.authority()
        expected = _transport(previous, seal, admitted, "phase")
        same(expected["current"], active, "exact full outer inverse input")
        same(a, expected, "exact full outer inverse input")
        result = copy.deepcopy(previous)
        _pure(result, view)
    return result


def phase_previous(p, a, seal, admitted):
    active = _seal(p, seal)
    _source(p, seal, admitted)
    with _retained_scope(p, seal, admitted) as (old, view):
        previous = old.authority()
        expected = _transport(previous, seal, admitted, "phase")
        same(expected["current"], active, "exact phase input; no double inversion")
        same(a, expected, "exact phase input; no double inversion")
        prephase = old.phase_previous(previous)
        result = _transport(prephase, seal, admitted, "prephase")
        same(current_candidate(result), current_candidate(expected),
             "phase does not change current candidate provenance")
        _pure(result, view)
    return result


def compose(p, a, name, raw, control, operation, seal, admitted):
    """Four finite composer bridges; all old predicates run unchanged in M."""
    scopes = {
        "lexer": {("src/frontend/lexer.rs", False)},
        "array": {("src/frontend/ast.rs", False), ("src/frontend/ast.rs", True),
                  ("src/frontend/parser.rs", False), ("src/frontend/parser.rs", True),
                  ("src/frontend/project/budget.rs", False)},
        "namespace": {("src/frontend/declaration_index/resource.rs", False)},
        "source": {("src/frontend/source.rs", False)},
    }
    need(type(control) is bool and operation in scopes and (name, control) in scopes[operation],
         "closed published composer target/role")
    active = _seal(p, seal)
    _source(p, seal, admitted)
    with _retained_scope(p, seal, admitted) as (old, view):
        previous = old.authority()
        full = _transport(previous, seal, admitted, "phase")
        same(full["current"], active, "current sealed phase authority")
        old_prephase = old.phase_previous(previous)
        expected = _transport(old_prephase, seal, admitted, "prephase")
        same(a, expected, "composer requires exact current prephase stage")
        same(raw, admitted["inputs"][name], "unchanged complete M/current composer input")
        same(raw, admitted["published_predecessor"]["inputs"][name],
             "unchanged complete M/current composer input")
        if operation == "lexer":
            result = old.compose_current_lexer(old_prephase, raw)
        elif operation == "array":
            result = old.compose_array_instrumentation(old_prephase, name, raw, control)
        elif operation == "namespace":
            result = old.compose_namespace_resource(old_prephase, raw)
        else:
            result = old.compose_source_read(old_prephase, raw)
        field = "current_control_derived_files" if control else "current_derived_files"
        wanted = rows_map(expected["current"][field], 542)[name]
        same(identity(name, result), wanted, "complete current prephase composer output")
    return result
