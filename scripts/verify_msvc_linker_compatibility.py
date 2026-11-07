#!/usr/bin/env python3
"""Bind one reviewed MSVC linker source change to an unchanged retained policy.

Scalar-library and registry policies keep their original predicates, observer
assembly and debug/release builds. This wrapper grants no Windows, platform or
new-feature qualification. Missing after-pins refuse even preparation.
"""
import argparse
from contextlib import contextmanager
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import types


ROOT = Path(__file__).resolve().parents[1]
NATIVE_INPUT = "src/frontend/native.rs"
BEFORE_NATIVE_PIN = (26278, "a2b3dec1185c5be9d9a935ab13bb90375f63b7a4f998558829440827d578baf9")
SOURCE_POLICIES = {
    "scalar-library": {
        "parent": "260c816fb984391493df84e498511dd9fc013f42",
        "controller": "scripts/verify_scalar_library_compatibility.py",
        "controller_pin": (63079, "29c1d76604adfeddf2006213922ceb59947f7a193454ffb542df2ba0f7597119"),
        "input_members": 522,
    },
    "registry": {
        "parent": "af0fe5e1fbaf4fb49b62e894a53ad4b5ed307ee7",
        "controller": "scripts/verify_registry_compatibility.py",
        "controller_pin": (35050, "d6bd23496f38bde3d89e7e97a2d724b73a0f233cfef99a6dc9fad752bdec9766"),
        "input_members": 527,
    },
}

# Exact one-file patch from the immutable reviewed source. A caller cannot
# derive an after-pin from its checkout or supply one through CLI flags.
REVIEWED_NATIVE_PIN = (29659, "84611a7cbfdf154f47f3dba3b39625fc225b78debd38673f11cded54eefde741")
REVIEWED_NATIVE_SOURCE_COMMIT = "563860bfef73254d460faebc3c88d8b34ed77ab2"
BEFORE_NATIVE_DIRECTIVES = {
    "attributes": (
        (b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1),
        (b'#[cfg(not(all(\n    any(target_os = "linux", target_os = "macos"),\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n)))]', 1),
        (b'#[cfg(unix)]', 1), (b'#[cfg(test)]', 1), (b'#[test]', 2),
    ),
    "build_script": None,
    "c_preprocessor": (),
    "legacy": ([
        b'include_str!("../../native/typed_preview.c")',
        b'include_str!("../../native/typed_character.c")',
        b'include_str!("../../native/typed_float.c")',
        b'include_bytes!(concat!(env!("OUT_DIR")',
        b'include_str!("../../native/typed_integer128.c")',
        b'include_str!("../../native/typed_bytebuf.c")',
        b'include_str!("../../native/experimental_bytebuf.c")',
        b'include_str!("../../native/experimental_files.c")',
        b'include_str!("../../native/experimental_arguments.c")',
        b'include_str!("../../native/experimental_bytebuf.h")',
        b'include_str!("../../native/experimental_vector.c")',
        b'include_str!("../../native/experimental_vector.h")',
        b'include_str!("../../native/typed_vector.c")',
    ], []),
    "macros": (
        (b'include_str!("../../native/typed_preview.c")', 1),
        (b'include_str!("../../native/typed_character.c")', 1),
        (b'include_str!("../../native/typed_float.c")', 1),
        (b'include_bytes!(concat!(env!("OUT_DIR"), "/float_format.a"))', 1),
        (b'env!("OUT_DIR")', 1),
        (b'include_str!("../../native/typed_integer128.c")', 1),
        (b'include_str!("../../native/typed_bytebuf.c")', 1),
        (b'include_str!("../../native/experimental_bytebuf.c")', 1),
        (b'include_str!("../../native/experimental_files.c")', 1),
        (b'include_str!("../../native/experimental_arguments.c")', 1),
        (b'include_str!("../../native/experimental_bytebuf.h")', 1),
        (b'include_str!("../../native/experimental_vector.c")', 1),
        (b'include_str!("../../native/experimental_vector.h")', 1),
        (b'include_str!("../../native/typed_vector.c")', 1),
        (b'cfg!(windows)', 1), (b'cfg!(target_os = "macos")', 1),
        (b'cfg!(windows)', 1), (b'cfg!(target_os = "macos")', 4),
    ),
}
# The only new selector directives are a cfg(test) module and its two tests.
# Includes, asset paths, platform cfg macros and build input bytes stay exact.
REVIEWED_NATIVE_DIRECTIVES = {
    **BEFORE_NATIVE_DIRECTIVES,
    "attributes": BEFORE_NATIVE_DIRECTIVES["attributes"] + ((b'#[cfg(test)]', 1), (b'#[test]', 2)),
}
PINS_PROVISIONAL = False


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def identity(name, body):
    return {"path": name, "bytes": len(body), "sha256": sha(body)}


def read_policy(repo, spec):
    path = repo / spec["controller"]
    require(not path.is_symlink() and all(not parent.is_symlink() for parent in path.parents),
            "symlink pinned retained controller")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == spec["controller_pin"][0], "unsafe pinned retained controller")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), sha(body)) == spec["controller_pin"], "changed pinned retained controller")
    return body


def reviewed_pins():
    require(REVIEWED_NATIVE_PIN is not None and REVIEWED_NATIVE_DIRECTIVES is not None
            and REVIEWED_NATIVE_SOURCE_COMMIT is not None,
            "missing independently reviewed native after-pin/directive inventory; preparation is unavailable")
    pin = REVIEWED_NATIVE_PIN
    require(type(pin) is tuple and len(pin) == 2 and type(pin[0]) is int and pin[0] > 0
            and type(pin[1]) is str and len(pin[1]) == 64
            and all(value in "0123456789abcdef" for value in pin[1]), "invalid reviewed native after-pin")
    require(type(REVIEWED_NATIVE_DIRECTIVES) is dict
            and set(REVIEWED_NATIVE_DIRECTIVES) == {"legacy", "attributes", "macros", "c_preprocessor", "build_script"},
            "invalid reviewed native directive inventory")
    require(type(REVIEWED_NATIVE_SOURCE_COMMIT) is str and len(REVIEWED_NATIVE_SOURCE_COMMIT) == 40
            and all(value in "0123456789abcdef" for value in REVIEWED_NATIVE_SOURCE_COMMIT),
            "invalid reviewed native source commit")
    return pin, REVIEWED_NATIVE_DIRECTIVES


def directive_manifest(value):
    if isinstance(value, bytes):
        return {"bytes_hex": value.hex()}
    if isinstance(value, dict):
        return {name: directive_manifest(item) for name, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [directive_manifest(item) for item in value]
    return value


class RetainedPolicy:
    def __init__(self, name, library):
        require(name in SOURCE_POLICIES, "unknown retained source policy")
        self.name, self.spec, self.library = name, SOURCE_POLICIES[name], library
        self.original_run, self.original_manifest = library.run, library.current_manifest
        self.parent_added = library.PARENT_ADDED
        self.policies = {**library.PREDECESSOR_POLICIES,
                         self.spec["controller"]: self.spec["controller_pin"][1]}

    def capture_sources(self, repo, baseline, parent):
        pin, directives = reviewed_pins()
        expected = set(baseline) | self.parent_added
        require(set(parent) == expected and len(parent) == self.spec["input_members"]
                and NATIVE_INPUT in parent, "wrong retained parent compiler input membership")
        require((len(parent[NATIVE_INPUT]), sha(parent[NATIVE_INPUT])) == BEFORE_NATIVE_PIN,
                "wrong frozen native before-input")
        require(NATIVE_INPUT not in self.library.UNCHANGED_INPUTS
                and set(self.library.UNCHANGED_INPUTS) <= expected, "protected source cannot enter linker delta")
        actual, directories = set(), set()
        for part in ("src", "native"):
            require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing source root")
            for directory, dirs, files in os.walk(repo / part, followlinks=False):
                for name in dirs + files:
                    path = Path(directory) / name
                    require(not path.is_symlink(), "symlink compiler source")
                    (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
        compiler = {name for name in expected if name.startswith(("src/", "native/"))}
        require(actual == compiler, "unexpected compiler source membership")
        expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                         if str(path) not in (".", "src", "native")}
        require(directories == expected_dirs, "unexpected compiler source directory")
        inputs = {name: self.library.regular(repo, name) for name in sorted(expected)}
        for name, before in parent.items():
            if name != NATIVE_INPUT:
                require(inputs[name] == before, "unreviewed source outside the one-file linker delta: " + name)
        require((len(inputs[NATIVE_INPUT]), sha(inputs[NATIVE_INPUT])) == pin,
                "native source does not match the reviewed linker after-pin")
        require(self.library.source_directives(NATIVE_INPUT, inputs[NATIVE_INPUT]) == directives,
                "reviewed native linker compile-time directive changed")
        before_directives = self.library.source_directives(NATIVE_INPUT, parent[NATIVE_INPUT])
        require(before_directives == BEFORE_NATIVE_DIRECTIVES, "frozen native before-directive inventory changed")
        require(directives["legacy"] == before_directives["legacy"],
                "one-file linker delta cannot add a compile-time source asset")
        return inputs

    def current_manifest(self, inputs, baseline, checkout, parent):
        inherited = self.original_manifest(parent, baseline, checkout, parent)
        result = {name: value for name, value in inherited.items() if name.endswith("qualified_by_this_gate")}
        result.update({
            "schema": "oxid-project-workflow-current-source-v1", "baseline_commit": self.library.BASELINE,
            "checkout": checkout, "assertion_scope": "retained bounded scalar/record semantics and resources",
            "debug_information_enabled_for_retained_claim": False,
            "source_policy_parent": self.spec["parent"], "source_policy_inputs": self.policies,
            "source_policy_input_members": len(self.policies),
            "predecessor_compiler_input_members": len(parent), "msvc_linker_source_policy": self.name,
            "msvc_linker_pins_provisional": PINS_PROVISIONAL,
            "msvc_linker_reviewed_source_commit": REVIEWED_NATIVE_SOURCE_COMMIT,
            "msvc_linker_controller": identity("scripts/verify_msvc_linker_compatibility.py",
                                               self.library.regular(ROOT, "scripts/verify_msvc_linker_compatibility.py")),
            "msvc_linker_reviewed_change": {"path": NATIVE_INPUT,
                                           "before": identity(NATIVE_INPUT, parent[NATIVE_INPUT]),
                                           "after": identity(NATIVE_INPUT, inputs[NATIVE_INPUT]),
                                           "before_directives": directive_manifest(self.library.source_directives(NATIVE_INPUT, parent[NATIVE_INPUT])),
                                           "after_directives": directive_manifest(REVIEWED_NATIVE_DIRECTIVES)},
            "retained_protected_inputs": {name: identity(name, parent[name]) for name in sorted(self.library.UNCHANGED_INPUTS)},
            "changes": [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
                        for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]],
            "additions": sorted(self.parent_added),
            "unchanged_baseline_inputs": sum(inputs[name] == body for name, body in baseline.items()),
            "files": [identity(name, body) for name, body in sorted(inputs.items())],
            "msvc_linker_semantics_qualified_by_this_gate": False,
            "msvc_linker_windows_platform_qualified_by_this_gate": False,
            "msvc_linker_native_execution_qualified_by_this_gate": False,
        })
        require(all(value is False for name, value in result.items() if name.endswith("qualified_by_this_gate")),
                "unexpected feature/platform qualification claim")
        return result

    @contextmanager
    def bound(self):
        bindings = {"PREDECESSOR": self.spec["parent"], "PREDECESSOR_ADDED": self.parent_added,
                    "PREDECESSOR_POLICIES": self.policies, "PINS_PROVISIONAL": PINS_PROVISIONAL,
                    "capture_sources": self.capture_sources, "current_manifest": self.current_manifest}
        before = {name: getattr(self.library, name) for name in bindings}
        require(self.library.run is self.original_run, "retained runner was replaced")
        try:
            for name, value in bindings.items():
                setattr(self.library, name, value)
            yield
            require(self.library.run is self.original_run, "retained runner changed")
        finally:
            for name, value in before.items():
                setattr(self.library, name, value)


def load_policy(repo, name):
    require(name in SOURCE_POLICIES, "unknown retained source policy")
    spec = SOURCE_POLICIES[name]
    body = read_policy(repo, spec)
    module = types.ModuleType("_msvc_linker_retained_" + name.replace("-", "_"))
    module.__file__ = str(repo / spec["controller"])
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return RetainedPolicy(name, module)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(repo == ROOT and Path(__file__).resolve() == repo / "scripts/verify_msvc_linker_compatibility.py",
            "run the wrapper from the selected checkout")
    require(output != repo and repo not in output.parents and not output.exists(), "output must be new outside repository")
    require(args.source_policy in SOURCE_POLICIES, "unknown retained source policy")
    output.mkdir(parents=True)
    controller = Path(__file__).read_bytes()
    spec = SOURCE_POLICIES[args.source_policy]
    report = {"schema": "oxid-msvc-linker-retained-wrapper-v1", "status": "running",
              "scope": "delegated unchanged retained semantic/resource replay; no Windows/platform qualification",
              "source_policy": args.source_policy, "source_policy_parent": spec["parent"],
              "inherited_input_members": spec["input_members"], "semantic_pass": False, "compiler_executions": 0,
              "pins_provisional": PINS_PROVISIONAL,
              "reviewed_native_pin_missing": REVIEWED_NATIVE_PIN is None or REVIEWED_NATIVE_DIRECTIVES is None
                  or REVIEWED_NATIVE_SOURCE_COMMIT is None,
              "reviewed_native_source_commit": REVIEWED_NATIVE_SOURCE_COMMIT,
              "controller": identity("scripts/verify_msvc_linker_compatibility.py", controller),
              "windows_qualification": False, "platform_qualification": False, "new_feature_qualification": False,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        reviewed_pins()
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional MSVC linker pins permit prepare-only admission, not qualification")
        selected = load_policy(repo, args.source_policy)
        delegated = argparse.Namespace(**vars(args))
        delegated.output = output / "retained"
        with selected.bound():
            status = selected.original_run(delegated)
        require(status == 0, "retained controller refused; inspect retained/failure.json and raw evidence")
        artifact = "prepared.json" if args.prepare_only else "result.json"
        raw = selected.library.regular(output / "retained", artifact)
        retained = json.loads(raw)
        require(retained.get("status") == ("prepared" if args.prepare_only else "passed")
                and retained.get("semantic_pass") is (not args.prepare_only), "retained replay did not establish the requested result")
        require(retained.get("current_input_members") == spec["input_members"]
                and retained.get("controller") == identity(spec["controller"], read_policy(repo, spec))
                and retained.get("historical_results_apply_to_current") is False,
                "retained result lacks its exact source/materialization binding")
        require(selected.library.regular(repo, "scripts/verify_msvc_linker_compatibility.py") == controller,
                "wrapper controller changed during qualification")
        report.update(status="prepared" if args.prepare_only else "passed", semantic_pass=retained["semantic_pass"],
                      compiler_executions=retained["compiler_executions"],
                      retained_artifact=identity("retained/" + artifact, raw), retained_source=retained["current_source"],
                      finished_at_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "source_policy": args.source_policy,
                          "semantic_pass": report["semantic_pass"], "output": str(output)}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", semantic_pass=False, error=f"{type(error).__name__}: {error}")
        (output / "failure.json").write_bytes(encoded(report))
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        report.setdefault("finished_at_utc", datetime.datetime.now(datetime.timezone.utc).isoformat())
        (output / "wrapper.json").write_bytes(encoded(report))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-policy", choices=tuple(SOURCE_POLICIES), required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--prepare-only", action="store_true")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
