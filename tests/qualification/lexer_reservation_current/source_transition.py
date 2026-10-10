"""Prepared exact outer source admission, inactive until a separately pinned seal.

This module never creates a source manifest/map/authority, invokes a compiler, or
opens the private parser package. The supplied seal must come from the reviewed
immutable current dispatcher, never from an untrusted command-line override.
"""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat

from adapters import Reject, binding, need, sha, verify

PREDECESSOR_SHA = "402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa"
PREDECESSOR_BYTES = 74192
PREDECESSOR_HEAD = "e3c1b4a1a3ef457326f11a802896c125202fe797"
PREDECESSOR_TREE = "5fdb4f06a8fcbc61724676df55a4c3bee130eaf7"
PREDECESSOR_NAME = "byte-storage-source-v1.json"
CURRENT_NAME = "lexer-reservation-source-v2.json"
AUTHORITY_NAME = "lexer-reservation-authority-v2.json"
PATCH_NAME = "lexer-reservation-transition-v2.patch"
RECIPE = "git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS"
ACCOUNTING_PATHS = (
    "src/frontend/declaration_index/resource.rs",
    "src/frontend/declaration_index/sealed.rs",
    "src/frontend/declaration_index/u8_reservation.rs",
)


def relative(name):
    need(isinstance(name, str) and name and not PurePosixPath(name).is_absolute()
         and "\\" not in name and str(PurePosixPath(name)) == name
         and all(p not in (".", "..") for p in PurePosixPath(name).parts),
         "invalid selected input path")
    return name


def rows_map(rows):
    need(isinstance(rows, list) and rows, "empty selected source map")
    names = [relative(row["path"]) for row in rows]
    need(names == sorted(set(names)), "unordered or duplicate selected source map")
    need(all(set(row) == {"path", "bytes", "sha256"} for row in rows),
         "unexpected selected source identity fields")
    return dict(zip(names, rows))


def identity(name, body):
    return {**binding(name, body), "mode": "100644", "git_blob": hashlib.sha1(
        b"blob " + str(len(body)).encode() + b"\0" + body).hexdigest()}


def read_inputs(repo, rows):
    inputs = {}
    for name, row in rows_map(rows).items():
        path = repo
        for part in PurePosixPath(name).parts:
            path = path / part
            need(not path.is_symlink(), "symlink in selected input: " + name)
        mode = path.stat().st_mode
        need(stat.S_ISREG(mode) and not mode & 0o111, "nonregular/executable source: " + name)
        body = path.read_bytes()
        verify(body, row, name)
        inputs[name] = body
    return inputs


def complete_compiler_members(repo, selected):
    """No cache-name exclusions, including empty unexpected source directories."""
    actual_files, actual_dirs = set(), set()
    for root in ("src", "native"):
        base = repo / root
        need(base.is_dir() and not base.is_symlink(), "missing compiler root")
        for parent, directories, files in os.walk(base, followlinks=False):
            for name in directories + files:
                path = Path(parent) / name
                need(not path.is_symlink(), "symlink compiler member")
                relative_name = path.relative_to(repo).as_posix()
                if name in directories:
                    actual_dirs.add(relative_name)
                else:
                    need(path.is_file() and not path.stat().st_mode & 0o111,
                         "nonregular/executable compiler member")
                    actual_files.add(relative_name)
    expected = {name for name in selected if name.startswith(("src/", "native/"))}
    dirs = {str(parent) for name in expected for parent in PurePosixPath(name).parents
            if str(parent) not in (".", "src", "native")}
    need(actual_files == expected and actual_dirs == dirs,
         "missing or extra complete compiler member/directory")


def pinned_json(package_bytes, expected, name):
    raw = package_bytes[name]
    verify(raw, expected, name)
    return json.loads(raw)


def admit(repo, package_bytes, seal, api):
    """Validate both complete closures before predecessor materialization.

    api is the pinned retained public source-binding API, specifically its exact
    inverse-patch implementation and lexical include-expression scanner. An
    immutable caller must authenticate api and seal before calling this function.
    """
    need(isinstance(seal, dict) and seal.get("schema") == "oxid-lexer-reservation-seal-v1",
         "final independently reviewed source seal is absent")
    current = pinned_json(package_bytes, seal["source_manifest"], CURRENT_NAME)
    authority = pinned_json(package_bytes, seal["authority"], AUTHORITY_NAME)
    patch = package_bytes[PATCH_NAME]
    verify(patch, seal["patch"], PATCH_NAME)
    raw = package_bytes[PREDECESSOR_NAME]
    need(len(raw) == PREDECESSOR_BYTES and sha(raw) == PREDECESSOR_SHA,
         "changed complete byte-storage predecessor")
    predecessor = json.loads(raw)
    need(predecessor["reviewed_source_head"] == PREDECESSOR_HEAD
         and predecessor["source_only_tree"] == PREDECESSOR_TREE,
         "stale byte-storage predecessor checkpoint")
    before = rows_map(predecessor["files"])
    after = rows_map(current["files"])
    need(len(before) == 376, "changed original predecessor member count")
    compiler_additions = authority["compiler_additions"]
    fixture_additions = authority["fixture_additions"]
    additions = compiler_additions + fixture_additions
    need(compiler_additions == sorted(set(compiler_additions))
         and fixture_additions == sorted(set(fixture_additions))
         and all(name.startswith(("src/", "native/")) for name in compiler_additions)
         and all(name.startswith(("tests/fixtures/", "fixtures/")) for name in fixture_additions)
         and len(additions) == len(set(additions))
         and not set(before).intersection(additions)
         and set(after) == set(before) | set(additions)
         and authority["removed_paths"] == [], "unapproved source membership transition")
    paths = [name for name in after if after[name] != before.get(name)]
    need(paths and authority["transition_paths"] == paths, "wrong ordered outer transition scope")
    need(authority["schema"] == "oxid-fallible-lexer-source-transition-v1"
         and authority["recipe"] == RECIPE
         and authority["base_head"] == PREDECESSOR_HEAD
         and authority["base_tree"] == PREDECESSOR_TREE
         and authority["reviewed_source_head"] == current["reviewed_source_head"] == seal["compiler_head"]
         and authority["source_only_tree"] == current["source_only_tree"] == seal["compiler_tree"]
         and authority["adapter_head"] == seal["adapter_head"]
         and authority["predecessor_source_sha256"] == current["lexer_reservation_predecessor_sha256"] == PREDECESSOR_SHA
         and authority["predecessor_source_bytes"] == PREDECESSOR_BYTES
         and authority["current_source_sha256"] == seal["source_manifest"]["sha256"]
         and authority["current_source_bytes"] == seal["source_manifest"]["bytes"]
         and authority["transition_patch_sha256"] == sha(patch)
         and authority["transition_patch_bytes"] == len(patch), "stale outer source authority")
    omit = {"files", "purpose", "reviewed_source_head", "source_only_tree",
            "lexer_reservation_predecessor_sha256"}
    need({k: v for k, v in current.items() if k not in omit}
         == {k: v for k, v in predecessor.items() if k not in omit},
         "changed retained source provenance")
    inputs = read_inputs(repo, current["files"])
    complete_compiler_members(repo, inputs)
    identities = [identity(name, body) for name, body in inputs.items()]
    need(authority["current_input_identities"] == identities, "stale complete source identities")
    fixtures = [row for row in identities if row["path"].startswith(("tests/fixtures/", "fixtures/"))]
    includes = api.include_directives(inputs, api)
    need(authority["compile_time_fixture_inputs"] == fixtures
         and authority["compile_time_include_directives"] == includes,
         "stale complete fixture/include closure")
    restored, touched = api.apply_inverse_patch(inputs, patch, sha(patch), len(patch), tuple(paths))
    need(list(touched) == paths and sorted(restored) == list(before), "wrong-stage inverse or member loss")
    for name, body in restored.items():
        verify(body, before[name], name)
    restored = dict(sorted(restored.items()))
    need(authority["predecessor_input_identities"] == [identity(n, b) for n, b in restored.items()],
         "stale complete predecessor identities")
    old_includes = api.include_directives(restored, api)
    added_includes = authority["compile_time_include_additions"]
    need(len(old_includes) == 136 and len(fixtures) == 78 + len(fixture_additions)
         and [row for row in includes if row not in old_includes] == added_includes
         and [row for row in includes if row not in added_includes] == old_includes,
         "changed/missing/reordered/duplicated retained include expression")
    need([identity(n, b) for n, b in restored.items() if n.startswith(("tests/fixtures/", "fixtures/"))]
         == [row for row in fixtures if row["path"] not in fixture_additions],
         "changed retained compile-time fixture")
    need(authority["transition_inputs"] == [
        {"path": name, "before": identity(name, restored[name]) if name in restored else None,
         "after": identity(name, inputs[name])} for name in paths], "stale outer delta identities")
    compiler_count = sum(name.startswith(("src/", "native/")) for name in inputs)
    need(authority["current_source_members"] == len(inputs)
         and authority["predecessor_source_members"] == len(restored)
         and authority["compiler_source_members"] == compiler_count
         and authority["compiler_bodies"] == compiler_count + 3,
         "stale complete source cardinalities")
    need(all(inputs[name] == restored[name] for name in ACCOUNTING_PATHS),
         "Unit2 immutable accounting dependency changed; separate review required")
    need(authority["unit2_accounting_dependencies"] == [identity(n, inputs[n]) for n in ACCOUNTING_PATHS],
         "stale Unit2 accounting transport identities")
    return {"current": current, "inputs": inputs, "authority": authority,
            "predecessor_inputs": restored, "transition_touched": touched,
            "source_only": True, "execution_qualified": False}


RETAINED_RUNNER_SHA = "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76"
RETAINED_SCANNER_SHA = "7664dc9ff6597c3c586154af3b3a38f095147d025b8fd6e544f6c7a93f250ff8"
RETAINED_BYTE_HELPER_SHA = "e9eef8a6475d6c79c5e93a127af8ed99e46eb029de4da7909371b44dcd6f6084"
RETAINED_PACKAGE_SHA = "c68a80705e43c2711d492d68fdbf63213e7d7b7f02dd1288c12403382ad3208c"


def retained_public_api(package, package_bytes):
    """Authenticate the unchanged public interface before Python module loading.

    Merely loading these declarations does not call preflight, materialize,
    compile, generate authorities, or inspect any private-parser package.
    """
    import types
    pinned = (("run.py", RETAINED_RUNNER_SHA),
              ("u8_cross_host.py", RETAINED_SCANNER_SHA),
              ("byte_storage.py", RETAINED_BYTE_HELPER_SHA),
              ("package-manifest.json", RETAINED_PACKAGE_SHA))
    for name, digest in pinned:
        need(sha(package_bytes[name]) == digest, "changed retained public API: " + name)
    modules = {}
    for name in ("run.py", "u8_cross_host.py", "byte_storage.py"):
        module = types.ModuleType("retained_lexer_reservation_" + name[:-3])
        module.__file__ = str(package / name)
        exec(compile(package_bytes[name], module.__file__, "exec"), module.__dict__)
        modules[name] = module
    public = dict(vars(modules["run.py"]))
    public["include_directives"] = modules["u8_cross_host.py"].include_directives
    return types.SimpleNamespace(**public), modules["byte_storage.py"]
