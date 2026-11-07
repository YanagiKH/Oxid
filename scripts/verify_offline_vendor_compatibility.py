#!/usr/bin/env python3
"""Bind the offline-export source delta to the unchanged full retained replay.

The inherited controller still owns all execution, observer and resource
predicates. Provisional source pins permit preparation only, not qualification.
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
PREDECESSOR = "7279208226a3d4185c0b996491332519971e916e"
PREDECESSOR_POLICY = "scripts/verify_owned_text_compatibility.py"
PREDECESSOR_POLICY_PIN = (16603, "61bf3bf55b9c1f958fdf48fcc89e747cdf61700109ab979b098851249d9fa109")
REVIEWED_SOURCE_HEAD = "1434d068c675251ad32e72937c4b78d7df26f13c"
PINS_PROVISIONAL = False
CLI = "src/cli.rs"
MAIN = "src/main.rs"
PACKAGE = "src/project_package.rs"
COMMAND = "src/project_vendor.rs"
TEST = "src/project_package/vendor_tests.rs"
TEXT_RESOURCE_TEST = "src/frontend/oir/owned/source/owned_text_source_resource_tests.rs"
VECTOR_ROUTE_TEST = "src/frontend/oir/source/owned_vector_route_tests.rs"
VENDOR_CHANGED = frozenset((CLI, MAIN, PACKAGE, TEXT_RESOURCE_TEST, VECTOR_ROUTE_TEST))
VENDOR_ADDED = frozenset((COMMAND, TEST))
MODULE_BEFORE = b"mod project_registry;\n"
MODULE_AFTER = MODULE_BEFORE + b"mod project_vendor;\n"
DISPATCH_BEFORE = b"    if let Some(code) = project_registry::dispatch(&args) {\n"
DISPATCH_INSERT = (b"    if let Some(code) = project_vendor::dispatch(&args) {\n"
                   b"        std::process::exit(code);\n    }\n")
HELP_BEFORE = b'    println!("  oxid package --extract ARCHIVE --output NEW_DIRECTORY");\n'
HELP_INSERT = b'    println!("  oxid vendor --frozen --output NEW_DIRECTORY  Copy a project for offline use (Linux)");\n'
PACKAGE_INSERT_START = b"/// A detached local graph, not a replacement source for the original lock.\n"
PACKAGE_INSERT_END = (b'#[cfg(test)]\n#[path = "project_package/vendor_tests.rs"]\n'
                      b"mod vendor_tests;\n\n")
PACKAGE_FOLLOWING = b"#[derive(Clone, Copy, Debug, PartialEq, Eq)]\nenum Operation {\n"
REVIEWED_INPUTS = {
    CLI: (1696, "8bc616a1b25a70999312d4c9e7e5e7b9926bd242a8e8f6dd788157088428c484"),
    MAIN: (164426, "204fb0d840678b610a29e743c80aed11f3f3100056eff1b575c01ca6b683048a"),
    PACKAGE: (48290, "193754d0742ffb2fe62d5fdf4ae13bc5a63b37d303714b164989037545f3f601"),
    COMMAND: (14655, "5d2ca5dd17f0046f6c4caa693bfb86b9323ed8fcd2c6cc220f88180df9869401"),
    TEST: (15493, "11c9e415276863d59b5f9814a28cd4a86c7b62fc4a88ff073b41c8bc65c3762a"),
    TEXT_RESOURCE_TEST: (11224, "6c9e5f0f10a16493d67f338f1fe4f9cb89b9bd0b8816b766da906a0fbd578698"),
    VECTOR_ROUTE_TEST: (24514, "8f70ed5591221e313b95e94ad3209b1680bed60321cf8513d59296c0c9fd17bd"),
}

PACKAGE_HOSTS = b'#[cfg(any(target_os = "linux", target_os = "macos"))]'
OTHER_PACKAGE_HOSTS = b'#[cfg(not(any(target_os = "linux", target_os = "macos")))]'
TEXT_OLD_EXERCISE = b'''fn exercise(body: &str, skip: usize, allocations: u64, peak: u64) {
    let fixture = Fixture::new(&format!("{IMPORTS}{body}"));
    let specs = fixture.specs();
    let package = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap();
    let library = package
        .project
        .sources()
        .files()
        .iter()
        .find(|file| file.path() == specs[1].entry.to_str().unwrap())
        .unwrap();
    assert_eq!(library.text().as_bytes(), TEXT_SOURCE.as_bytes());
    let (program, entry) = check_enum_source(SourceOwner::packaged(&package)).unwrap();
'''
TEXT_NEW_EXERCISE = b'''fn exercise(body: &str, skip: usize, allocations: u64, peak: u64) {
    let fixture = Fixture::new(&format!("{IMPORTS}{body}"));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let specs = fixture.specs();
        let package = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap();
        let library = package
            .project
            .sources()
            .files()
            .iter()
            .find(|file| file.path() == specs[1].entry.to_str().unwrap())
            .unwrap();
        assert_eq!(library.text().as_bytes(), TEXT_SOURCE.as_bytes());
        exercise_owner(SourceOwner::packaged(&package), skip, allocations, peak);
    }
    // The same literal library is an ordinary declared module on every host,
    // including the Windows loader. Keep this path live on Linux/macOS too.
    let project = ProjectSources::load_typed(
        fixture.0.join("app/src/flat.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let text_path = fixture.0.join("app/src/text.ox").canonicalize().unwrap();
    let mut matching_modules = project
        .modules()
        .iter()
        .filter(|module| module.canonical_path.as_ref() == Some(&text_path));
    let module = matching_modules
        .next()
        .expect("loaded Text module must retain its canonical source identity");
    assert!(matching_modules.next().is_none());
    assert_eq!(module.relative_path, "text.ox");
    let library = project.sources().get(module.file);
    assert_eq!(library.text().as_bytes(), TEXT_SOURCE.as_bytes());
    exercise_owner(SourceOwner::project(&project), skip, allocations, peak);
}

fn exercise_owner(owner: SourceOwner<'_>, skip: usize, allocations: u64, peak: u64) {
    let (program, entry) = check_enum_source(owner).unwrap();
'''
TEXT_PACKAGE_REFUSAL = b'''
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[test]
fn owned_text_source_resource_package_policy_refuses_before_loading_or_allocating() {
    let fixture = Fixture::new(&format!("{IMPORTS}{FROM_UTF8}"));
    let specs = fixture.specs();
    let failure = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap_err();
    assert_eq!(failure.diagnostics.len(), 1);
    assert_eq!(
        (failure.diagnostics[0].code, failure.diagnostics[0].stage),
        ("E0006", "package"),
    );
    assert_eq!(
        failure.diagnostics[0].message,
        "package source policy is qualified only on Linux or macOS",
    );
    assert!(failure.sources.files().is_empty());
    assert_eq!(
        failure.usage,
        crate::frontend::project::SourceUsage::default()
    );
    assert_eq!(failure.allocator.attempts, 0);
    assert!(failure.allocator.trace.is_empty());
}
'''
VECTOR_PACKAGE_POSITIVE = (b"#[test]\n"
    b"fn owned_vector_public_route_package_owner_keeps_namespaces_and_unused_generic_facts() {\n")
VECTOR_PACKAGE_FOLLOWING = (b"#[test]\n"
    b"fn owned_vector_public_route_borrow_move_and_copy_misuse_cannot_reach_consumers() {\n")
VECTOR_PACKAGE_REFUSAL = b'''#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[test]
fn owned_vector_public_route_package_owner_refuses_unqualified_hosts_before_source_loading() {
    let fixture = Fixture::new(&workflow());
    let root = fs::canonicalize(&fixture.0).unwrap();
    let single = [PackageSpec {
        root: root.clone(),
        entry: root.join("main.ox"),
        dependencies: vec![],
    }];
    fs::create_dir(root.join("app")).unwrap();
    fs::create_dir(root.join("model")).unwrap();
    fs::write(root.join("app/main.ox"), "fn main()->i32{return 3;}").unwrap();
    fs::write(root.join("model/lib.ox"), "use alloc::bytes::ByteBuf;use alloc::vec::Vec;pub struct Entry{pub bytes:ByteBuf}pub fn inspect<Marker>(values:&Vec<Entry>)->(){return;}").unwrap();
    let multiple = [
        PackageSpec {
            root: root.join("app"),
            entry: root.join("app/main.ox"),
            dependencies: vec![PackageDependency {
                alias: "model".into(),
                package: 1,
            }],
        },
        PackageSpec {
            root: root.join("model"),
            entry: root.join("model/lib.ox"),
            dependencies: vec![],
        },
    ];
    for specs in [single.as_slice(), multiple.as_slice()] {
        let failure = PackageSources::load(specs, None, ProjectLimits::default()).unwrap_err();
        assert_eq!(failure.diagnostics.len(), 1);
        assert_eq!(
            (failure.diagnostics[0].code, failure.diagnostics[0].stage),
            ("E0006", "package"),
        );
        assert_eq!(
            failure.diagnostics[0].message,
            "package source policy is qualified only on Linux or macOS",
        );
        assert!(failure.sources.files().is_empty());
        assert_eq!(
            failure.usage,
            crate::frontend::project::SourceUsage::default()
        );
        assert_eq!(failure.allocator.attempts, 0);
        assert!(failure.allocator.trace.is_empty());
    }
}

'''


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and not any(parent.is_symlink() for parent in path.parents),
            "symlink pinned owned-text controller")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == PREDECESSOR_POLICY_PIN[0], "unsafe pinned owned-text controller")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, name) for name in fields) == tuple(getattr(path.stat(), name) for name in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned owned-text controller")
    module = types.ModuleType("_offline_vendor_retained_owned_text")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    return module


predecessor = load_predecessor(ROOT)
library = predecessor.library
RETAINED_RUN = predecessor.RETAINED_RUN
PREDECESSOR_ADDED = predecessor.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | VENDOR_ADDED
PREDECESSOR_POLICIES = {**predecessor.PREDECESSOR_POLICIES,
                        PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1]}
for _name in ("BASELINE", "BASELINE_SOURCE_SHA", "BINDING", "RETAINED", "RETAINED_RUNNER_SHA",
              "sha", "encoded", "identity", "regular", "git", "source_directives", "directive_manifest"):
    globals()[_name] = getattr(predecessor, _name)


def approved_cli(before):
    require(before.count(MODULE_BEFORE) == 1 and before.count(DISPATCH_BEFORE) == 1
            and b"mod project_vendor;" not in before and DISPATCH_INSERT not in before,
            "wrong predecessor CLI module/dispatch")
    return before.replace(MODULE_BEFORE, MODULE_AFTER, 1).replace(
        DISPATCH_BEFORE, DISPATCH_INSERT + DISPATCH_BEFORE, 1)


def approved_main(before):
    require(before.count(HELP_BEFORE) == 1 and HELP_INSERT not in before,
            "wrong predecessor package help")
    return before.replace(HELP_BEFORE, HELP_BEFORE + HELP_INSERT, 1)


def approved_text_resource_test(before):
    """Keep every existing Ledger assertion and all three fault entry bodies."""
    replacements = (
        (b"//! Real package source calls under the existing runtime allocation failpoint.\n",
         b"//! Real project and package source calls under the runtime allocation failpoint.\n"),
        (b"        ProjectLimits,\n", b"        ProjectLimits, ProjectSources,\n"),
        (b'        std::fs::write(root.join("app/src/main.ox"), source).unwrap();\n',
         b'        std::fs::write(root.join("app/src/main.ox"), source).unwrap();\n'
         b'        std::fs::write(root.join("app/src/flat.ox"), format!("mod text;\\n{source}")).unwrap();\n'
         b'        std::fs::write(root.join("app/src/text.ox"), TEXT_SOURCE).unwrap();\n'),
        (TEXT_OLD_EXERCISE, TEXT_NEW_EXERCISE),
    )
    require(TEXT_PACKAGE_REFUSAL not in before, "unexpected predecessor Text platform refusal")
    for old, new in replacements:
        require(before.count(old) == 1, "wrong predecessor Text platform replacement")
        before = before.replace(old, new, 1)
    return before + TEXT_PACKAGE_REFUSAL


def approved_vector_route_test(before):
    """Keep all old route bodies; gate its package case and add two-graph refusal."""
    require(before.count(VECTOR_PACKAGE_POSITIVE) == 1 and before.count(VECTOR_PACKAGE_FOLLOWING) == 1
            and VECTOR_PACKAGE_REFUSAL not in before, "wrong predecessor route platform replacement")
    return before.replace(VECTOR_PACKAGE_POSITIVE, PACKAGE_HOSTS + b"\n" + VECTOR_PACKAGE_POSITIVE, 1).replace(
        VECTOR_PACKAGE_FOLLOWING, VECTOR_PACKAGE_REFUSAL + VECTOR_PACKAGE_FOLLOWING, 1)


def unchanged_package(before, current):
    require(current.count(PACKAGE_INSERT_START) == 1 and current.count(PACKAGE_INSERT_END) == 1
            and before.count(PACKAGE_FOLLOWING) == 1 and PACKAGE_INSERT_START not in before,
            "wrong finite vendor package insertion")
    prefix, remainder = current.split(PACKAGE_INSERT_START)
    insertion, suffix = remainder.split(PACKAGE_INSERT_END)
    require(suffix.startswith(PACKAGE_FOLLOWING) and prefix + suffix == before,
            "existing package body changed outside vendor insertion")
    return PACKAGE_INSERT_START + insertion + PACKAGE_INSERT_END


# No new includes, configuration macros or build assets. Ordered attribute
# rows are literal, independent of REVIEWED_INPUTS, so repinning a source does
# not silently admit a different compile-time selection.
EMPTY_DIRECTIVES = {"legacy": ([], []), "attributes": (), "macros": (),
                    "c_preprocessor": (), "build_script": None}
COMMAND_DIRECTIVES = {**EMPTY_DIRECTIVES, "attributes": (
    (b"#[repr(C)]", 1), (b"#[cfg(test)]", 1), (b"#[test]", 1),
    (b'#[cfg(target_pointer_width = "64")]', 1), (b"#[test]", 3))}
TEST_DIRECTIVES = {**EMPTY_DIRECTIVES, "attributes": (
    (b"#[test]", 1), (b'#[cfg(all(target_pointer_width = "64", target_endian = "little"))]', 1),
    (b"#[test]", 5),
    (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width = "64",\n    target_endian = "little",\n    any(target_arch = "x86_64", target_arch = "aarch64")\n))]', 1),
    (b"#[test]", 3))}
PACKAGE_INSERT_DIRECTIVES = {"legacy": ([], [b'#[path = "project_package/vendor_tests.rs"]']),
    "attributes": ((b"#[repr(C)]", 2), (b"#[cfg(test)]", 2), (b"#[cfg(unix)]", 1),
                   (b"#[cfg(test)]", 3),
                   (b"#[derive(Clone, Copy, PartialEq, Eq)]", 1), (b"#[cfg(test)]", 4),
                   (b'#[path = "project_package/vendor_tests.rs"]', 1)),
    "macros": ((b'cfg!(all(\n        target_os = "linux",\n        target_pointer_width = "64",\n        target_endian = "little",\n        any(target_arch = "x86_64", target_arch = "aarch64")\n    ))', 1),),
    "c_preprocessor": (), "build_script": None}
PLATFORM_TEST_DIRECTIVES = {
    TEXT_RESOURCE_TEST: {
        **predecessor.REVIEWED_DIRECTIVES[TEXT_RESOURCE_TEST],
        "attributes": ((PACKAGE_HOSTS, 1), (b"#[test]", 3), (OTHER_PACKAGE_HOSTS, 1), (b"#[test]", 1)),
    },
    VECTOR_ROUTE_TEST: {
        **EMPTY_DIRECTIVES,
        "attributes": ((b"#[test]", 2), (PACKAGE_HOSTS, 1), (b"#[test]", 1),
                       (OTHER_PACKAGE_HOSTS, 1), (b"#[test]", 3),
                       (b'#[cfg(target_pointer_width = "64")]', 1), (b"#[test]", 7)),
    },
}


def capture_sources(repo, baseline, parent):
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | VENDOR_ADDED
    require(set(parent) == expected_parent and len(parent) == 574,
            "wrong pinned owned-text parent membership")
    require(VENDOR_CHANGED == {CLI, MAIN, PACKAGE, TEXT_RESOURCE_TEST, VECTOR_ROUTE_TEST}
            and VENDOR_ADDED == {COMMAND, TEST}
            and VENDOR_CHANGED <= expected_parent and not VENDOR_ADDED & expected_parent
            and len(expected) == 576, "wrong finite offline-vendor source membership")
    require(set(REVIEWED_INPUTS) == VENDOR_CHANGED | VENDOR_ADDED,
            "wrong offline-vendor pin membership")
    require(set(PLATFORM_TEST_DIRECTIVES) == {TEXT_RESOURCE_TEST, VECTOR_ROUTE_TEST},
            "wrong offline-vendor platform-test directive membership")
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing compiler source root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink compiler source member")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    require(actual == compiler, "unexpected compiler source membership")
    expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                     if str(path) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    try:
        inputs = {name: regular(repo, name) for name in sorted(expected)}
    except OSError as error:
        raise ValueError("unreadable offline-vendor compiler input: " + str(error)) from error
    for name, before in parent.items():
        if name not in VENDOR_CHANGED:
            require(inputs[name] == before, "unreviewed change outside offline-vendor boundary: " + name)
    require(inputs[CLI] == approved_cli(parent[CLI]), "CLI change exceeds exact vendor module/dispatch")
    require(inputs[MAIN] == approved_main(parent[MAIN]), "main change exceeds exact vendor help line")
    insertion = unchanged_package(parent[PACKAGE], inputs[PACKAGE])
    for name in (CLI, MAIN):
        require(source_directives(name, inputs[name]) == source_directives(name, parent[name]),
                "existing command compile-time directives changed: " + name)
    require(source_directives(PACKAGE, insertion) == PACKAGE_INSERT_DIRECTIVES,
            "vendor package insertion compile-time directive changed")
    require(source_directives(COMMAND, inputs[COMMAND]) == COMMAND_DIRECTIVES
            and source_directives(TEST, inputs[TEST]) == TEST_DIRECTIVES,
            "vendor new module compile-time directive changed")
    for name, expected_directives in PLATFORM_TEST_DIRECTIVES.items():
        require(source_directives(name, inputs[name]) == expected_directives,
                "platform regression compile-time directive changed: " + name)
    require(inputs[TEXT_RESOURCE_TEST] == approved_text_resource_test(parent[TEXT_RESOURCE_TEST]),
            "platform regression exceeds exact Text fixture/owner/refusal changes")
    require(inputs[VECTOR_ROUTE_TEST] == approved_vector_route_test(parent[VECTOR_ROUTE_TEST]),
            "platform regression exceeds exact package route guards/refusal changes")
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require((len(inputs[name]), sha(inputs[name])) == (length, digest),
                "offline-vendor reviewed input changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    previous_names = set(baseline) | predecessor.PREDECESSOR_ADDED
    inherited = predecessor.current_manifest(inputs=parent, baseline=baseline, checkout=checkout,
                                            parent={name: parent[name] for name in previous_names})
    result = {**inherited, "source_policy_parent": PREDECESSOR,
              "source_policy_inputs": PREDECESSOR_POLICIES,
              "source_policy_input_members": len(PREDECESSOR_POLICIES),
              "predecessor_compiler_input_members": len(parent),
              "offline_vendor_pins_provisional": PINS_PROVISIONAL,
              "offline_vendor_reviewed_source_head": REVIEWED_SOURCE_HEAD,
              "offline_vendor_controller": identity("scripts/verify_offline_vendor_compatibility.py",
                                                     regular(ROOT, "scripts/verify_offline_vendor_compatibility.py")),
              "offline_vendor_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                                  for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
              "offline_vendor_directive_inventory": [
                  {"path": name, "before": directive_manifest(source_directives(name, parent[name])) if name in parent else None,
                   "after": directive_manifest(source_directives(name, inputs[name]))}
                  for name in sorted(REVIEWED_INPUTS)],
              "changes": [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, value)}
                          for name, value in sorted(inputs.items()) if name in baseline and value != baseline[name]],
              "additions": sorted(PARENT_ADDED),
              "files": [identity(name, value) for name, value in sorted(inputs.items())],
              "unchanged_baseline_inputs": sum(inputs[name] == value for name, value in baseline.items()),
              "offline_vendor_semantics_qualified_by_this_gate": False,
              "offline_vendor_allocation_failures_qualified_by_this_gate": False,
              "offline_vendor_platform_qualified_by_this_gate": False}
    require(all(value is False for name, value in result.items() if name.endswith("qualified_by_this_gate")),
            "unexpected new qualification claim")
    return result


@contextmanager
def bound_predecessor():
    bindings = {"PREDECESSOR": PREDECESSOR, "PREDECESSOR_ADDED": PREDECESSOR_ADDED,
                "PREDECESSOR_POLICIES": PREDECESSOR_POLICIES, "PINS_PROVISIONAL": PINS_PROVISIONAL,
                "capture_sources": capture_sources, "current_manifest": current_manifest}
    require(library.run is RETAINED_RUN, "retained execution function was replaced")
    with predecessor.bound_predecessor() as context:
        before = {name: getattr(library, name) for name in bindings}
        try:
            for name, value in bindings.items():
                setattr(library, name, value)
            yield context
            require(library.run is RETAINED_RUN, "retained execution function changed")
        finally:
            for name, value in before.items():
                setattr(library, name, value)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(repo == ROOT and Path(__file__).resolve() == repo / "scripts/verify_offline_vendor_compatibility.py",
            "run the offline-vendor controller from the selected checkout")
    require(output != repo and repo not in output.parents and not output.exists(), "output must be new outside repository")
    output.mkdir(parents=True)
    controller = regular(repo, "scripts/verify_offline_vendor_compatibility.py")
    report = {"schema": "oxid-offline-vendor-retained-wrapper-v1", "status": "running", "semantic_pass": False,
              "compiler_executions": 0, "controller": identity("scripts/verify_offline_vendor_compatibility.py", controller),
              "source_policy_parent": PREDECESSOR, "inherited_input_members": 574, "expected_current_input_members": 576,
              "pins_provisional": PINS_PROVISIONAL, "reviewed_source_head": REVIEWED_SOURCE_HEAD,
              "new_feature_qualification": False, "platform_qualification": False,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional offline-vendor pins permit prepare-only admission, not qualification")
        require(args.prepare_only or not predecessor.PINS_PROVISIONAL, "provisional predecessor source pins")
        delegated = argparse.Namespace(**vars(args))
        delegated.output = output / "retained"
        with bound_predecessor() as context:
            status = RETAINED_RUN(delegated)
            require(status == 0, "unchanged retained replay refused; inspect retained raw evidence")
            artifact = "prepared.json" if args.prepare_only else "result.json"
            raw = regular(delegated.output, artifact)
            retained = json.loads(raw)
            require(retained.get("status") == ("prepared" if args.prepare_only else "passed")
                    and retained.get("semantic_pass") is (not args.prepare_only)
                    and retained.get("current_input_members") == 576
                    and retained.get("historical_results_apply_to_current") is False
                    and retained.get("controller") == identity(predecessor.predecessor.PREDECESSOR_POLICY,
                                                               regular(repo, predecessor.predecessor.PREDECESSOR_POLICY)),
                    "retained replay lacks the exact offline-vendor source/materialization binding")
            require(context and context["output"] == delegated.output, "missing unchanged observer preparation context")
            if not args.prepare_only:
                report.update(predecessor.predecessor.run_owned_vector_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], retained["test_function_executions"]))
        require(regular(repo, "scripts/verify_offline_vendor_compatibility.py") == controller,
                "offline-vendor controller changed")
        report.update(status="prepared" if args.prepare_only else "passed", semantic_pass=retained["semantic_pass"],
                      compiler_executions=retained["compiler_executions"], current_input_members=retained["current_input_members"],
                      retained_artifact=identity("retained/" + artifact, raw), retained_source=retained["current_source"])
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"], "output": str(output)}))
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
