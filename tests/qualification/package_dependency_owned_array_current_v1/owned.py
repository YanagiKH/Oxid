"""Fixed, inert owned-array current-source derivation. This module issues no authority.

The returned controller is source text, never an import or execution target. Its
first executable statement rejects. The original owners have not admitted these
recipes, prepared images, test artifacts, processes, or tar transport.
"""
from __future__ import annotations

import ast
import hashlib
import json
import os
from pathlib import Path
import stat
from types import MappingProxyType

_RECIPE_SHA256 = "97a94fcb1e5fe32b1b51732bbc21478d04d78b010ed1ac0c35ea70e6f906772a"
_SOURCE = "scripts/verify_owned_array_native.py"
_MEMBERS = ("producer-both", "independent-profile", "package-both", "compact-audit")
_HELD = "owned-array is unregistered; original A/PreparedSource/artifact/process/tar-owner review is required"
_PACKAGE_FILES = ("owned.py", "recipes.json", "test_owned.py", "package-manifest.json")
_PACKAGE_LIMITS = {"owned.py": 65536, "recipes.json": 524288,
                   "test_owned.py": 65536, "package-manifest.json": 8192}


class DerivationError(ValueError):
    """A fixed byte or preservation constraint did not match."""


class AuthorityHeld(RuntimeError):
    """No live owned-array policy or execution authority exists in this package."""


def _require(condition, message):
    if not condition:
        raise DerivationError(message)


def _sha(body):
    return hashlib.sha256(body).hexdigest()


def _record(body):
    return {"bytes": len(body), "sha256": _sha(body)}


def _ast_identity(node):
    """owned-ast-v1: all semantic fields; no locations; empty type_params omitted.

    Python 3.12 added empty type_params to ordinary functions/classes. Omitting
    only that empty field keeps the fixed 3.11 grammar inventory reproducible;
    nonempty type parameters and every other field remain represented.
    """
    def normalize(value):
        if isinstance(value, ast.AST):
            return {"node": type(value).__name__, "fields": {
                key: normalize(item) for key, item in ast.iter_fields(value)
                if not (key == "type_params" and item == [])}}
        if type(value) is list:
            return [normalize(item) for item in value]
        if type(value) is bytes:
            return {"bytes_hex": value.hex()}
        if value is Ellipsis:
            return {"constant": "Ellipsis"}
        if type(value) is complex:
            return {"complex_repr": repr(value)}
        return value
    return _sha(json.dumps(normalize(node), sort_keys=True, separators=(",", ":")).encode())


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        _require(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def _json(body):
    return json.loads(body, object_pairs_hook=_unique_object)


def _stat_key(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def _read_regular(path, limit):
    """Bounded read through one nonsymlink descriptor with stable ancestry."""
    path = path.absolute()
    ancestors = [(parent, parent.lstat()) for parent in path.parents]
    _require(all(stat.S_ISDIR(row.st_mode) and not stat.S_ISLNK(row.st_mode)
                 for _, row in ancestors), "package ancestor is not a nonsymlink directory")
    before = path.lstat()
    _require(stat.S_ISREG(before.st_mode) and before.st_size <= limit,
             "package member is nonregular or oversized")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(descriptor, "rb") as stream:
        _require(_stat_key(os.fstat(stream.fileno())) == _stat_key(before), "package member changed before read")
        body = stream.read(limit + 1)
        _require(len(body) == before.st_size and len(body) <= limit, "package member changed or exceeded bound")
        _require(_stat_key(os.fstat(stream.fileno())) == _stat_key(before), "package descriptor changed during read")
    _require(_stat_key(path.lstat()) == _stat_key(before), "package member was replaced")
    _require(all(_stat_key(parent.lstat()) == _stat_key(row) for parent, row in ancestors),
             "package ancestor changed during read")
    return body


def _recipes():
    directory = Path(__file__).absolute().parent
    directory_before = directory.lstat()
    _require(sorted(path.name for path in directory.iterdir()) == sorted(_PACKAGE_FILES), "closed package membership differs")
    manifest_body = _read_regular(directory / "package-manifest.json", _PACKAGE_LIMITS["package-manifest.json"])
    manifest = _json(manifest_body)
    _require(manifest.get("schema") == "oxid-package-dependency-owned-array-package-v1"
             and manifest.get("status") == "inert-unregistered" and manifest.get("execution_authorized") is False,
             "package manifest identity differs")
    _require(set(manifest["files"]) == set(_PACKAGE_FILES) - {"package-manifest.json"}, "package pin membership differs")
    captured = {name: _read_regular(directory / name, _PACKAGE_LIMITS[name]) for name in manifest["files"]}
    for name, body in captured.items():
        _require(_record(body) == manifest["files"][name], "package member authentication failed: " + name)
    body = captured["recipes.json"]
    _require(_sha(body) == _RECIPE_SHA256, "fixed recipe body changed")
    _require(_read_regular(directory / "package-manifest.json", _PACKAGE_LIMITS["package-manifest.json"]) == manifest_body,
             "package manifest changed during authentication")
    _require(_stat_key(directory.lstat()) == _stat_key(directory_before)
             and sorted(path.name for path in directory.iterdir()) == sorted(_PACKAGE_FILES),
             "closed package membership changed during authentication")
    return _json(body)


def _authenticate(retained_bodies, recipe):
    # Exact builtin types prevent caller getters, iterators, conversions, or
    # bytes subclasses from participating in the input boundary.
    _require(type(retained_bodies) is dict, "retained input must be an exact dict")
    snapshot = retained_bodies.copy()
    _require(all(type(key) is str for key in snapshot), "input names must be exact strings")
    _require(all(type(value) is bytes for value in snapshot.values()), "input bodies must be exact bytes")
    pins = recipe["retained_inputs"]
    _require(set(snapshot) == set(pins), "retained input closure differs")
    for name, pin in pins.items():
        _require(_record(snapshot[name]) == pin, "retained input authentication failed: " + name)
    # Nothing parses any retained JSON, Python or Rust before every body passes.
    return MappingProxyType(snapshot)


def _functions(body):
    lines = body.splitlines(keepends=True)
    result = []
    for node in ast.parse(body).body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            first = min([node.lineno] + [item.lineno for item in getattr(node, "decorator_list", ())])
            fragment = b"".join(lines[first - 1:node.end_lineno])
            result.append({"name": node.name, "first_line": first, "last_line": node.end_lineno,
                           **_record(fragment),
                           "ast_sha256": _ast_identity(node)})
    return result


def _uppercase(body):
    result = []
    for node in ast.parse(body).body:
        targets = node.targets if isinstance(node, ast.Assign) else ([node.target] if isinstance(node, ast.AnnAssign) else [])
        names = [target.id for target in targets if isinstance(target, ast.Name) and target.id.isupper()]
        if names:
            result.append({"names": names, "ast_sha256": _ast_identity(node)})
    return result


def _commands(body):
    """Complete command-bearing AST inventory, including original argv nodes."""
    result = []
    call_names = {"run_child", "command_admission", "independent_command", "git_bytes", "git_argv"}
    for node in ast.walk(ast.parse(body)):
        if isinstance(node, ast.Call):
            name = node.func.id if isinstance(node.func, ast.Name) else (node.func.attr if isinstance(node.func, ast.Attribute) else "")
            if name in call_names or name in {"command", "git"}:
                result.append({"line": node.lineno, "kind": "call", "name": name,
                               "ast_sha256": _ast_identity(node)})
        elif isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            names = [target.id for target in targets if isinstance(target, ast.Name)]
            if any(name in {"argv", "expected_argv", "expected_build", "expected_labels", "arguments"} for name in names):
                result.append({"line": node.lineno, "kind": "argv-assignment", "name": ",".join(names),
                               "ast_sha256": _ast_identity(node)})
    return sorted(result, key=lambda item: (item["line"], item["kind"], item["name"]))


def _apply(original, edits):
    pieces, cursor = [], 0
    for edit in edits:
        start, end = edit["start"], edit["end"]
        before, after = edit["before"].encode(), edit["after"].encode()
        _require(cursor <= start <= end <= len(original), "overlapping or invalid fixed islands")
        _require(original[start:end] == before, "fixed island preimage differs: " + edit["id"])
        _require(original.count(before) == edit["global_occurrences"], "fixed island occurrence count differs")
        pieces.extend((original[cursor:start], after))
        cursor = end
    pieces.append(original[cursor:])
    return b"".join(pieces)


def _recover(derived, edits):
    pieces, cursor = [], 0
    for edit in edits:
        start, end = edit["derived_start"], edit["derived_end"]
        after = edit["after"].encode()
        _require(cursor <= start <= end <= len(derived) and derived[start:end] == after,
                 "fixed inverse island differs: " + edit["id"])
        pieces.extend((derived[cursor:start], edit["before"].encode()))
        cursor = end
    pieces.append(derived[cursor:])
    return b"".join(pieces)


def _relationship(bodies, recipe):
    expected = recipe["source_relationship"]
    current = _json(bodies[expected["current_manifest"]["path"]])
    historical = _json(bodies[expected["historical_manifest"]["path"]])
    now = {row["path"]: row for row in current["files"]}
    old = {row["path"]: row for row in historical["files"]}
    _require(len(now) == len(current["files"]) == 376 and set(now) == set(old), "current source closure changed")
    _require([name for name in sorted(now) if now[name] != old[name]] == ["src/main.rs"], "source delta differs")
    _require(now["src/main.rs"] == expected["current_main"] and old["src/main.rs"] == expected["historical_main"], "main identity differs")
    _require(current["reviewed_source_head"] == expected["current_checkpoint"]["head"]
             and current["source_only_tree"] == expected["current_checkpoint"]["tree"], "current checkpoint differs")
    _require(historical["reviewed_source_head"] == expected["historical_checkpoint"]["head"]
             and historical["source_only_tree"] == expected["historical_checkpoint"]["tree"], "historical checkpoint differs")
    _require(current["files"] == recipe["current_all376_members"], "all376 commitment map differs")
    return expected


def _proofs(original, derived, bodies, recipe):
    proof = recipe["preservation"]
    _require(_recover(derived, recipe["edits"]) == original, "inverse is not exact")
    _require(_functions(original) == proof["original_controller_functions"], "original function inventory differs")
    _require(_functions(derived) == proof["derived_controller_functions"], "derived function inventory differs")
    _require(_uppercase(original) == _uppercase(derived) == proof["historical_uppercase"], "historical constants changed")
    _require(_commands(original) == proof["original_commands"], "original commands changed")
    _require(_commands(derived) == proof["derived_commands"], "derived commands changed")
    original_functions = {row["name"]: row for row in _functions(original)}
    derived_functions = {row["name"]: row for row in _functions(derived)}
    changed = {edit["function"] for edit in recipe["edits"]} - {"<module>"}
    _require(sorted(changed) == proof["changed_functions"], "source-sensitive function roster differs")
    _require(sorted(set(original_functions) - changed) == proof["unchanged_functions"], "unchanged function roster differs")
    for name in proof["unchanged_functions"]:
        _require(all(original_functions[name][key] == derived_functions[name][key]
                     for key in ("bytes", "sha256", "ast_sha256")), "non-island function changed: " + name)
    commands_before = [row["ast_sha256"] for row in proof["original_commands"]]
    for exception in proof["command_exception_nodes"]:
        _require(commands_before.count(exception["ast_sha256"]) == 1, "preflight exception is not unique")
        commands_before.remove(exception["ast_sha256"])
    _require(sorted(commands_before) == sorted(row["ast_sha256"] for row in proof["derived_commands"]),
             "non-preflight command/argv AST changed")
    for name, expected in proof["retained_python_functions"].items():
        _require(_functions(bodies[name]) == expected, "retained function inventory differs: " + name)
    for row in proof["unchanged_source_spans"]:
        before = original[row["start"]:row["end"]]
        after = derived[row["derived_start"]:row["derived_end"]]
        _require(before == after and _record(before) == row["identity"], "non-island source changed")
    tree = ast.parse(derived)
    executable = [node for node in tree.body if not (isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant) and type(node.value.value) is str)
                  and not (isinstance(node, ast.ImportFrom) and node.module == "__future__")]
    _require(isinstance(executable[0], ast.Raise) and executable[0].cause is None, "missing first-executable barrier")
    return proof


def fixed_derivation(retained_bodies):
    """Authenticate the complete fixed closure, then return only inert bytes/data."""
    recipe = _recipes()
    authenticated = _authenticate(retained_bodies, recipe)
    relationship = _relationship(authenticated, recipe)
    original = authenticated[_SOURCE]
    derived = _apply(original, recipe["edits"])
    _require(_record(derived) == recipe["derived_identity"], "derived identity differs")
    proofs = _proofs(original, derived, authenticated, recipe)
    return {"schema": recipe["schema"], "status": "inert-unregistered", "semantic_pass": False,
            "members": {name: {"body": derived, **recipe["roles"][name]} for name in _MEMBERS},
            "source_relationship": relationship, "preservation": proofs,
            "held_authorities": recipe["held_authorities"], "execution_performed": False}


def inverse(member, derived_body):
    """Recover the exact pinned controller, accepting no derivative variation."""
    _require(type(member) is str and member in _MEMBERS, "unknown fixed member")
    _require(type(derived_body) is bytes, "derived body must be exact bytes")
    recipe = _recipes()
    _require(_record(derived_body) == recipe["derived_identity"], "inverse requires exact derived bytes")
    original = _recover(derived_body, recipe["edits"])
    _require(_record(original) == recipe["retained_inputs"][_SOURCE], "inverse source identity differs")
    return original


def current_policy(original_job_context):
    raise AuthorityHeld(_HELD)


def plan(original_job_context):
    raise AuthorityHeld(_HELD)


def run(*args, **kwargs):
    raise AuthorityHeld(_HELD)
