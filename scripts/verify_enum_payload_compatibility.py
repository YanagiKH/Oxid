#!/usr/bin/env python3
"""Check scalar/record compatibility after adding finite owned enum payloads.

This successor admits only the listed enum implementation changes relative to
its pinned project-workflow parent. Historical assertion bodies, observers and
comparators remain exact. New enum semantics have separate public/native and
application checks; this compatibility gate does not qualify those semantics.
"""
import argparse
import copy
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tarfile

BASELINE = "9ff8ef90a26f003f4616838dfc186fa3bfb6845c"
BASELINE_SOURCE_SHA = "d33ecc4e41975a062790540ba9e9a270d7c6043a31f8f4b9a8d7a044c79530b0"
RETAINED_RUNNER_SHA = "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76"
BINDING = "tests/fixtures/typed_project_source_binding"
RETAINED = "tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py"
ORCHESTRATION_CHANGED = frozenset(("src/cli.rs", "src/main.rs", "src/frontend/mod.rs",
                                  "src/frontend/driver.rs", "src/frontend/native.rs"))
PACKAGE_CHANGED = frozenset(("src/frontend/project.rs", "src/frontend/declaration_index.rs",
                            "src/frontend/declaration_index/sealed.rs",
                            "src/frontend/declaration_index/source_owner.rs",
                            "src/frontend/oir/project.rs", "src/frontend/oir/source.rs",
                            "src/frontend/oir/source/sealed.rs"))
NATIVE_PRESENTATION_CHANGED = frozenset(("src/frontend/oir/native.rs",
                                        "src/frontend/oir/owned/native.rs",
                                        "src/frontend/oir/source/sealed.rs",
                                        "src/frontend/oir/owned/source/program.rs"))
PROJECT_CHANGED = ORCHESTRATION_CHANGED | PACKAGE_CHANGED | NATIVE_PRESENTATION_CHANGED
PROJECT_ADDED = frozenset(("src/project_cli.rs", "src/project_cli/workspace.rs", "src/project_cli/package.rs",
                   "src/project_package.rs", "src/project_package/archive.rs", "src/project_lock.rs", "src/doctor_cli.rs",
                   "src/frontend/native_cache.rs", "src/frontend/native/doctor.rs", "src/frontend/project/packages.rs",
                   "src/frontend/project/packages/tests.rs", "src/frontend/build_config.rs"))
PREDECESSOR = "407d33695379b4e82fc3f2074a2dc897fc57943e"
PREDECESSOR_POLICY_SHA = "76d4df296015df6ca7b5b46509b778e17d23c922fd80df4b4ed3d182d81356c6"
OWNED_ENUM_CHANGED = frozenset((
    'src/frontend/ast.rs',
    'src/frontend/declaration_index/enum_views.rs',
    'src/frontend/format/ast_tests.rs',
    'src/frontend/oir/owned/enum_native_tests.rs',
    'src/frontend/oir/owned/execute.rs',
    'src/frontend/oir/owned/flow.rs',
    'src/frontend/oir/owned/mod.rs',
    'src/frontend/oir/owned/native.rs',
    'src/frontend/oir/owned/plan.rs',
    'src/frontend/oir/owned/shape.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/association.rs',
    'src/frontend/oir/owned/source/byte_storage_tests.rs',
    'src/frontend/oir/owned/source/candidate_adapter.rs',
    'src/frontend/oir/owned/source/candidate_native.rs',
    'src/frontend/oir/owned/source/hir_budget.rs',
    'src/frontend/oir/owned/source/lower.rs',
    'src/frontend/oir/owned/source/mod.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/typeck.rs',
    'src/frontend/oir/owned_types.rs',
    'src/frontend/oir/owned_types/byte_storage_tests.rs',
    'src/frontend/oir/owned_types/enums.rs',
    'src/frontend/parser/enum_syntax_tests.rs',
    'src/frontend/parser/enums.rs',
))
OWNED_ENUM_ADDED = frozenset((
    'src/frontend/oir/owned/owned_enum_tests.rs',
    'src/frontend/oir/owned/source/owned_enum_resources.rs',
    'src/frontend/oir/owned_types/owned_enum_tests.rs',
))
# Exact reviewed semantic delta; extra code changes require an explicit new pin.
OWNED_ENUM_INPUTS = {
    'src/frontend/ast.rs': (38841, '2efb61d4b372f7ec141805213198cc1bbaefa547125fc2b016e4a9c2d4908061'),
    'src/frontend/declaration_index/enum_views.rs': (10367, '7ee8a8ad4f2262bcf9ccc9b6eb07814cca8a5dc8378ab778fb136a7890cc43e6'),
    'src/frontend/format/ast_tests.rs': (63302, 'a90a42efd663fc49acb1a1da2fcec78460315ff17ca4ea3e74c741a37d42c0cf'),
    'src/frontend/oir/owned/enum_native_tests.rs': (61990, '3595c5193c8d1cb9648d191dd3d5b64d5063df5acdd57cc43ef2200225e37801'),
    'src/frontend/oir/owned/execute.rs': (156274, 'd447224e299516c11774b5b7e326d04feef68a80ac4742b9f4000cfa5e39a120'),
    'src/frontend/oir/owned/flow.rs': (44634, 'f7c2eba453e5e3b43c0f43673182e17417478e4457a6ce78eb435f14c3eb3154'),
    'src/frontend/oir/owned/mod.rs': (14211, '77e2cff5a81bbd5f51857ff11e0747f9912ebf998eb1554729a03acab4212590'),
    'src/frontend/oir/owned/native.rs': (193802, 'f1352e5c99375c48cbca288b17f7d549172020f5c51a82ff947fc51751259082'),
    'src/frontend/oir/owned/owned_enum_tests.rs': (19180, '21b8d486f4c338eb07913c2673237791e6b4adadb1055fb59c0e0050b699f493'),
    'src/frontend/oir/owned/plan.rs': (21998, '446dc9b289b0b8316dd616bd6c87ca889b00b9a2b85f107bd81d306b9999f8d5'),
    'src/frontend/oir/owned/shape.rs': (51039, '5a0cd1d2f79e606e78e9cdf7cf2a97284faf314f637ec3b8651d874036c25a3e'),
    'src/frontend/oir/owned/source/array_pipeline_rows.rs': (53299, '1028caf02aa421a4c911db55d69dd399f0fe90324f6044deee00a7605b8c467f'),
    'src/frontend/oir/owned/source/association.rs': (58241, 'f32ee0a5ca71212be7baa4cd6cf9f4b30431e451718dae234c9f27024744091c'),
    'src/frontend/oir/owned/source/byte_storage_tests.rs': (25478, '01e09ddd829e31e10e17fe938cbdc6f4297ddcbb8c60450484512c430af9b51e'),
    'src/frontend/oir/owned/source/candidate_adapter.rs': (41879, 'c8667b2040e9c7c885b1b4333c9729cc3c7d0c22aece337c9f85b50a9cdc4946'),
    'src/frontend/oir/owned/source/candidate_native.rs': (32232, '4840e9822fe3cd259ba27b7e0ab0ae16f146f2edb4d2ae130fd9f5d2056c0de2'),
    'src/frontend/oir/owned/source/hir_budget.rs': (48147, 'd66324cee1554bb79f43e8db4b7a4d41bae32938155f6fe7d008213d03752c32'),
    'src/frontend/oir/owned/source/lower.rs': (105579, '7fca02996aacd155851449f82699385493f7af29268de2ca98a9cccc11979128'),
    'src/frontend/oir/owned/source/mod.rs': (1793, '2a0318b4031ac4e29f8d94646b46e9a17b7f36725b26066eb1c5c8f6efce2e33'),
    'src/frontend/oir/owned/source/owned_enum_resources.rs': (4468, '2e26934a58b544a119a77c408c06e4670acd15b1facd3ad55fb28c8c6cf30049'),
    'src/frontend/oir/owned/source/resolve.rs': (134485, 'e49f1d9a8f8570e868c9f814fdb1822118d133b4c0f1e91ae6e2294a17e28c7f'),
    'src/frontend/oir/owned/source/typeck.rs': (204009, '9aed451c96056d6481c196a051f2e59ee56b38bd1b0053423eafa27b37f53c09'),
    'src/frontend/oir/owned_types.rs': (73840, 'cdb97ba912f0e8350aea18fcf3504026fc52b9aa95242168791a07f065f6b63a'),
    'src/frontend/oir/owned_types/byte_storage_tests.rs': (9219, 'ff429dbff65f808031cc2a3ddc87c3ca6ab09ef76ae3452e16f8206c0a44024c'),
    'src/frontend/oir/owned_types/enums.rs': (52246, '77dee0c99e60b6d71bea15764f952ee818ee098cfb107ec7bb144453e8aca02b'),
    'src/frontend/oir/owned_types/owned_enum_tests.rs': (12226, '7a837618631d67ac6800b3bb48d72a2021359437e5d4c751e472a99f38cd2581'),
    'src/frontend/parser/enum_syntax_tests.rs': (29694, '96073e5bb56fb260897963fd819eb7a2fc6d59c9a5bc011fd3f95c2b95ab63b5'),
    'src/frontend/parser/enums.rs': (15417, 'ede22ba10cda27532d84535b88b4e2b3c8a82682840ebdaf0ff4c0e377c142fc'),
}

CHANGED = PROJECT_CHANGED | OWNED_ENUM_CHANGED
ADDED = PROJECT_ADDED | OWNED_ENUM_ADDED
INSTRUMENTED = frozenset(("src/frontend/declaration_index.rs", "src/frontend/parser.rs",
                          "src/frontend/source.rs", "src/frontend/oir/mod.rs",
                          "src/frontend/project.rs"))
# This admission scanner is deliberately conservative, not a Rust evaluator.
# All existing expressions stay byte-identical, and no new include/path input is
# allowed, including multiline macros, alternate delimiters and cfg_attr paths.
GAP = rb'(?:\s|/\*.*?\*/|//[^\n]*(?:\n|$))*'
INCLUDE_START = re.compile(rb'\binclude(?:_str|_bytes)?' + GAP + rb'!', re.S)
INCLUDE = re.compile(INCLUDE_START.pattern + GAP
                     + rb'(?:\([^;]*?\)|\{[^;]*?\}|\[[^;]*?\])', re.S)
PATH_ATTRIBUTE = re.compile(rb'#' + GAP + rb'!?' + GAP + rb'\[[^\]]*\bpath\b[^\]]*\]', re.S)



def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def identity(name, body):
    return {"path": name, "bytes": len(body), "sha256": sha(body)}


def directives(body):
    includes = INCLUDE.findall(body)
    require(len(includes) == len(INCLUDE_START.findall(body)), "incomplete compile-time include")
    return includes, PATH_ATTRIBUTE.findall(body)



def regular(root, name):
    path = root / name
    require(not path.is_symlink() and all(not p.is_symlink() for p in path.parents),
            "symlink source: " + name)
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
            and not before.st_mode & 0o111 and before.st_size <= 2 * 1024 * 1024,
            "nonregular, executable, linked or oversized source: " + name)
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, f) for f in fields)
            == tuple(getattr(path.stat(), f) for f in fields) and len(body) == before.st_size,
            "source changed during read: " + name)
    return body


def capture_sources(repo, baseline, predecessor):
    """Admit the explicit finite-enum delta and retain predecessor byte authority.

    Earlier project/presentation changes remain pinned to PREDECESSOR. The old
    scalar/record predicates exercise compatibility, not the new payload kinds.
    """
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing source root")
        for parent, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(parent) / name
                require(not path.is_symlink(), "symlink source member")
                relative = path.relative_to(repo).as_posix()
                (directories if name in dirs else actual).add(relative)
    expected = {n for n in baseline if n.startswith(("src/", "native/"))} | ADDED
    require(actual == expected, "unexpected compiler source membership")
    expected_dirs = {str(p) for n in expected for p in PurePosixPath(n).parents
                     if str(p) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    inputs = {name: regular(repo, name) for name in sorted(set(baseline) | ADDED)}
    for name, before in baseline.items():
        if name not in CHANGED:
            require(inputs[name] == before, "unchanged semantic/resource input changed: " + name)
    require(set(predecessor) == set(baseline) | PROJECT_ADDED,
            "wrong predecessor compiler input membership")
    for name, body in predecessor.items():
        if name not in OWNED_ENUM_CHANGED:
            require(inputs[name] == body,
                    "unreviewed change outside owned enum boundary: " + name)
    require(set(OWNED_ENUM_INPUTS) == OWNED_ENUM_CHANGED | OWNED_ENUM_ADDED,
            "wrong reviewed owned enum input membership")
    for name, (length, digest) in OWNED_ENUM_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "owned enum source does not match reviewed input: " + name)
    # These files need no new compile-time inputs. Retain their exact include
    # and #[path] expressions; existing macro/fixture closure remains unchanged.
    for name in CHANGED | ADDED:
        require(directives(inputs[name]) == directives(baseline.get(name, b"")),
                "owned enum compile-time input directive changed: " + name)
    return inputs


def git(repo, *args):
    result = subprocess.run(["git", "--no-replace-objects", "-C", str(repo), *args],
                            capture_output=True, timeout=120)
    require(result.returncode == 0, "git command failed: " + " ".join(args)
            + ": " + result.stderr.decode(errors="replace")[-2000:])
    return result.stdout


def checkout_identity(repo, expected_head, allow_dirty):
    require(re.fullmatch("[0-9a-f]{40}", expected_head) is not None, "invalid expected head")
    head = git(repo, "rev-parse", "HEAD").decode().strip()
    require(head == expected_head, "checkout does not match expected head")
    status = git(repo, "status", "--porcelain=v1", "--untracked-files=all")
    require(allow_dirty or not status, "committed qualification requires a clean checkout")
    return {"head": head, "tree": git(repo, "rev-parse", "HEAD^{tree}").decode().strip(),
            "source_kind": "worktree" if status else "committed",
            "worktree_status_sha256": sha(status)}


def extract_baseline(repo, destination, fetch=False, revision=BASELINE):
    exists = subprocess.run(["git", "--no-replace-objects", "-C", str(repo),
                             "cat-file", "-e", revision + "^{commit}"],
                            capture_output=True).returncode == 0
    if not exists and fetch:
        git(repo, "fetch", "--no-tags", "--depth=1", "https://github.com/YanagiKH/Oxid.git", revision)
    require(git(repo, "rev-parse", revision + "^{commit}").decode().strip() == revision,
            "missing pinned baseline; use --fetch-baseline to fetch its exact public commit")
    require(git(repo, "rev-parse", "--show-object-format").strip() == b"sha1",
            "pinned repository requires SHA-1 Git object identities")
    expected = {}
    for row in git(repo, "ls-tree", "-rzl", "--full-tree", revision).split(b"\0"):
        if not row:
            continue
        metadata, name = row.split(b"\t", 1)
        mode, kind, object_id, length = metadata.split()
        require(mode in (b"100644", b"100755") and kind == b"blob"
                and re.fullmatch(rb"[0-9a-f]{40}", object_id) is not None and length.isdigit(),
                "nonregular pinned tree member")
        name = name.decode("utf-8")
        require(name not in expected, "duplicate pinned tree member")
        expected[name] = (int(mode[-3:], 8), object_id.decode("ascii"), int(length))
    expected_dirs = {str(p) for name in expected for p in PurePosixPath(name).parents
                     if str(p) != "."}
    destination.mkdir()
    archive = destination.with_suffix(".tar")
    with archive.open("xb") as stream:
        result = subprocess.run(["git", "--no-replace-objects", "-c", "tar.umask=0022", "-C", str(repo),
                                 "archive", "--format=tar", revision],
                                stdout=stream, stderr=subprocess.PIPE, timeout=120)
    require(result.returncode == 0, "cannot archive pinned baseline")
    seen, directories = set(), set()
    with tarfile.open(archive) as source:
        for member in source:
            name = PurePosixPath(member.name)
            require(not name.is_absolute() and ".." not in name.parts, "unsafe baseline archive path")
            path = destination.joinpath(*name.parts)
            if member.isdir():
                require(member.name in expected_dirs and member.name not in directories,
                        "unexpected pinned archive directory")
                directories.add(member.name)
                path.mkdir(parents=True, exist_ok=True)
            else:
                require(member.isfile(), "nonregular baseline archive member")
                require(member.name in expected and member.name not in seen,
                        "unexpected pinned archive member")
                mode, object_id, length = expected[member.name]
                require(member.mode & 0o7777 == mode, "pinned archive mode changed")
                require(member.size == length, "pinned archive blob length changed")
                with source.extractfile(member) as incoming:
                    body = incoming.read(length + 1)
                require(len(body) == length, "pinned archive blob read length changed")
                blob = b"blob " + str(len(body)).encode("ascii") + b"\0" + body
                require(hashlib.sha1(blob, usedforsecurity=False).hexdigest() == object_id,
                        "pinned archive blob changed: " + member.name)
                seen.add(member.name)
                path.parent.mkdir(parents=True, exist_ok=True)
                with path.open("xb") as target:
                    target.write(body)
                path.chmod(mode)
    require(seen == set(expected) and directories == expected_dirs,
            "pinned archive tree membership changed")
    archive.unlink()


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def current_manifest(inputs, baseline, checkout, predecessor):
    changes = [{"path": n, "before": identity(n, baseline[n]), "after": identity(n, b)}
               for n, b in sorted(inputs.items()) if n in baseline and b != baseline[n]]
    return {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
            "checkout": checkout, "changes": changes, "additions": sorted(ADDED),
            "assertion_scope": "retained single-package Unit2 semantics and resources",
            "package_boundary_inputs": sorted(PACKAGE_CHANGED),
            "native_presentation_inputs": sorted(NATIVE_PRESENTATION_CHANGED),
            "package_semantics_qualified_by_this_gate": False,
            "workspace_semantics_qualified_by_this_gate": False,
            "native_reproducibility_qualified_by_this_gate": False,
            "native_optimization_qualified_by_this_gate": False,
            "owned_enum_semantics_qualified_by_this_gate": False,
            "source_policy_parent": PREDECESSOR,
            "owned_enum_boundary_inputs": sorted(OWNED_ENUM_CHANGED),
            "owned_enum_additions": sorted(OWNED_ENUM_ADDED),
            "owned_enum_reviewed_inputs": [{"path": n, "bytes": size, "sha256": digest}
                                            for n, (size, digest) in sorted(OWNED_ENUM_INPUTS.items())],
            "owned_enum_changes": [{"path": n, "before": identity(n, predecessor[n]),
                                    "after": identity(n, inputs[n])}
                                   for n in sorted(OWNED_ENUM_CHANGED)
                                   if inputs[n] != predecessor[n]],
            "unchanged_baseline_inputs": len(baseline) - len(changes),
            "files": [identity(n, b) for n, b in sorted(inputs.items())]}


def verify_materialized(output, inputs):
    source = output / "unit2/run/source"
    assembly = json.loads((output / "unit2/run/assembly.json").read_bytes())
    changes = assembly["instrumentation"]
    require(len(changes) == len(INSTRUMENTED) and {r["path"] for r in changes} == INSTRUMENTED,
            "unexpected Unit2 instrumentation scope")
    for row in changes:
        require(row["before_sha256"] == sha(inputs[row["path"]]), "wrong instrumentation input")
        require(row["after_sha256"] == sha(regular(source, row["path"])), "changed instrumentation output")
    for name, body in inputs.items():
        if name not in INSTRUMENTED:
            require(regular(source, name) == body, "materialized current input changed: " + name)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained scalar/record semantics and resources on the owned-enum successor; new payload semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_enum_payload_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        checkout = checkout_identity(repo, args.expected_head, args.allow_dirty)
        baseline_root = output / "baseline"
        extract_baseline(repo, baseline_root, args.fetch_baseline)
        old_source = regular(baseline_root, BINDING + "/current-source.json")
        require(sha(old_source) == BASELINE_SOURCE_SHA, "wrong historical source manifest")
        original_body = regular(baseline_root, RETAINED)
        require(sha(original_body) == RETAINED_RUNNER_SHA, "wrong retained Unit2 predicates")
        historical = load("r1_historical_source_binding", baseline_root / BINDING / "run.py")
        captured = historical.preflight(baseline_root)
        original = load("r1_retained_unit2_assertions", baseline_root / RETAINED)
        predecessor_root = output / "predecessor"
        extract_baseline(repo, predecessor_root, args.fetch_baseline, PREDECESSOR)
        policy = "scripts/verify_project_workflow_current.py"
        require(sha(regular(predecessor_root, policy)) == PREDECESSOR_POLICY_SHA,
                "wrong pinned predecessor source policy")
        require(regular(repo, policy) == regular(predecessor_root, policy),
                "predecessor source policy changed")
        predecessor_inputs = {name: regular(predecessor_root, name)
                              for name in sorted(set(captured["inputs"]) | PROJECT_ADDED)}
        inputs = capture_sources(repo, captured["inputs"], predecessor_inputs)
        manifest = current_manifest(inputs, captured["inputs"], checkout, predecessor_inputs)
        manifest_raw = encoded(manifest)
        manifest_path = output / "current-source.json"
        manifest_path.write_bytes(manifest_raw)
        report.update(checkout=checkout, current_source=identity("current-source.json", manifest_raw),
                      historical_source=identity(BINDING + "/current-source.json", old_source),
                      retained_assertions=identity(RETAINED, original_body),
                      current_input_members=len(inputs), historical_results_apply_to_current=False)
        seam = original.prepare_unit2(output, captured)
        (output / "resource-seam.json").write_bytes(encoded(seam))
        selected = copy.deepcopy(captured)
        # The separate source manifest is the existing compatibility seam. Other
        # historical captures, identities, comparators and expectations stay exact.
        selected["package_bytes"]["current-source.json"] = manifest_raw

        def unchanged(actual_repo, _capture):
            require(checkout_identity(actual_repo, args.expected_head, args.allow_dirty) == checkout,
                    "checkout changed during qualification")
            require(regular(actual_repo, policy) == regular(predecessor_root, policy),
                    "predecessor source policy changed during qualification")
            require(capture_sources(actual_repo, captured["inputs"], predecessor_inputs) == inputs,
                    "current source changed during qualification")
            require(manifest_path.read_bytes() == manifest_raw, "current manifest changed")

        original.assert_unchanged = unchanged
        original.CURRENT_SOURCE_SHA = sha(manifest_raw)
        unchanged(repo, selected)
        command = [sys.executable, "-B", seam["compatibility_runner"], "--repo", str(repo),
                   "--source-manifest", str(manifest_path), "--output", str(output / "unit2"),
                   "--cargo", args.cargo, "--rustc", args.rustc]
        if args.prepare_only:
            command.append("--prepare-only")
        env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", PYTHONDONTWRITEBYTECODE="1")
        env.pop("PYTHONOPTIMIZE", None)
        report["compiler_executions"] = 0 if args.prepare_only else "see retained Cargo command evidence"
        with (output / "runner.stdout").open("xb") as stdout, (output / "runner.stderr").open("xb") as stderr:
            completed = subprocess.run(command, cwd=repo, env=env, stdout=stdout, stderr=stderr, timeout=5400)
        (output / "command.json").write_bytes(encoded({"argv": command, "exit_status": completed.returncode,
            "stdout": identity("runner.stdout", (output / "runner.stdout").read_bytes()),
            "stderr": identity("runner.stderr", (output / "runner.stderr").read_bytes())}))
        require(completed.returncode == 0, "retained Unit2 gate failed; see runner.stderr and unit2 evidence")
        verified = original.verify_unit2_result(output, selected, seam, args.prepare_only)
        verify_materialized(output, inputs)
        if not args.prepare_only:
            verified.update(original.run_unit2_observer_controls(repo, output, selected, seam))
            original.verify_unit2_result(output, selected, seam, False)
            verify_materialized(output, inputs)
        unchanged(repo, selected)
        report.update(verified)
        report["status"] = "prepared" if args.prepare_only else "passed"
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        artifact = "prepared.json" if args.prepare_only else "result.json"
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"],
                          "source_kind": checkout["source_kind"], "output": str(output)}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", semantic_pass=False, error=f"{type(error).__name__}: {error}")
        (output / "failure.json").write_bytes(encoded(report))
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        for name in set(sys.modules) - set(aliases):
            sys.modules.pop(name, None)
        sys.modules.update(aliases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true", help="fetch only the fixed public baseline commit if missing")
    parser.add_argument("--allow-dirty", action="store_true", help="record local worktree evidence; never label it committed")
    parser.add_argument("--prepare-only", action="store_true", help="exercise admission/assembly with zero compiler executions")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
