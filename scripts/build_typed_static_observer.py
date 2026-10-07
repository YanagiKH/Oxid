#!/usr/bin/env python3
"""Build a narrow observer from the checkout's unchanged canonical scalar static frontend sources."""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time


SOURCE_FILES = (
    "lexer.rs", "diagnostic.rs", "source.rs", "project/budget.rs",
    "ast.rs", "parser.rs", "parser/arrays.rs", "parser/enums.rs",
    "owned_diagnostic.rs", "builtin_catalog.rs", "hir.rs", "typeck.rs",
    "declaration_index.rs", "declaration_index/enum_views.rs",
    "declaration_index/resource.rs", "declaration_index/sealed.rs",
    "declaration_index/source_owner.rs", "project.rs", "project/filesystem.rs",
    "oir/owned_types.rs", "oir/owned_types/enums.rs",
)
WRAPPER_FILES = (
    ("main.rs", "main.rs"),
    ("frontend_mod.rs", "frontend/mod.rs"),
    ("oir_mod.rs", "frontend/oir/mod.rs"),
    ("static_observer.rs", "frontend/static_observer.rs"),
)


def build(output):
    repository = Path(__file__).resolve().parents[1]
    wrapper_root = repository / "tests/fixtures/bounded_typed_static/observer"
    output = Path(os.path.abspath(output))
    if output.exists() or output.is_symlink():
        raise ValueError("--output must name a fresh, nonexistent directory")

    # Keep the rustc shim's name: resolving a rustup symlink changes dispatch.
    rustc = shutil.which("rustc")
    if rustc is None:
        raise ValueError("rustc must be available on PATH")
    rustc = os.path.abspath(rustc)
    version = subprocess.run(
        [rustc, "--version", "--verbose"], cwd=repository,
        capture_output=True, text=True, check=True,
    )
    # Trust only this resolved checkout in a read-only, isolated Git child.
    git_env = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"], "LC_ALL": "C",
               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
               "GIT_OPTIONAL_LOCKS": "0"}
    git_argv = ["/usr/bin/git", "-c", "safe.directory=" + str(repository),
                "-C", str(repository), "rev-parse", "HEAD"]
    commit = subprocess.run(
        git_argv, cwd=repository, env=git_env,
        capture_output=True, text=True, check=True,
    ).stdout.strip()

    # Snapshot the exact inputs once, so hashes and compiled copies agree.
    sources = {
        name: (repository / "src/frontend" / name).read_bytes()
        for name in SOURCE_FILES
    }
    wrappers = {
        name: (wrapper_root / name).read_bytes()
        for name, _ in WRAPPER_FILES
    }
    inherited = (repository / "tests/fixtures/bounded_typed_parser/observer/frontend_mod.rs").read_bytes()
    if not wrappers["frontend_mod.rs"].startswith(inherited):
        raise ValueError("static wrapper must preserve the complete parser adapter prefix")
    declaration = re.search(
        r"pub enum Kind \{([^}]+)\}", sources["lexer.rs"].decode(), re.S
    )
    if declaration is None:
        raise ValueError("canonical Kind declaration was not found")
    kinds = [name.strip() for name in declaration.group(1).split(",") if name.strip()]
    if len(kinds) != 47 or not all(re.fullmatch(r"[A-Za-z]+", name) for name in kinds):
        raise ValueError("observer requires the 47 implicit, unit Kind variants")

    output.mkdir(parents=True, exist_ok=False)
    copied = []
    for name, data in sources.items():
        target = output / "frontend" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        copied.append({
            "original_path": "src/frontend/" + name,
            "copied_path": str(target.relative_to(output)),
            "sha256": hashlib.sha256(data).hexdigest(),
            "byte_count": len(data),
            "unchanged": True,
        })
    wrapper_manifest = []
    for name, target_name in WRAPPER_FILES:
        data = wrappers[name]
        (output / target_name).parent.mkdir(parents=True, exist_ok=True)
        (output / target_name).write_bytes(data)
        wrapper_manifest.append({
            "original_path": str((wrapper_root / name).relative_to(repository)),
            "copied_path": target_name,
            "sha256": hashlib.sha256(data).hexdigest(),
            "byte_count": len(data),
        })
    # The complete observation/wiring addition is recorded separately from
    # unchanged canonical source files. No source substitution is hidden.
    wrapper_diff = "".join(
        "".join(difflib.unified_diff(
            [], wrappers[name].decode().splitlines(keepends=True),
            fromfile="/dev/null", tofile=target_name,
        ))
        for name, target_name in WRAPPER_FILES
    )
    (output / "wrapper-only.diff").write_text(wrapper_diff)
    manifest = {
        "repository": str(repository),
        "commit": commit,
        "git_read": {"argv": git_argv, "environment": git_env},
        "source_patches": [],
        "wrapper_diff": "wrapper-only.diff",
        "entrypoint": {
            "parser": "parser::parse_typed_counted",
            "source_mode": "ProjectCandidate",
            "array_syntax": "Enabled", "enum_syntax": "Enabled",
            "std_imports": "Enabled", "node_limit": "parser::MAX_NODES",
            "lexer": "lexer::lex_with_limit", "token_limit": "lexer::MAX_TOKENS",
            "input": "at most 128 ASCII bytes", "path": "stdin.ox",
            "source_map": "source::SourceMap::new",
            "allocator": "project::budget::Allocator::default",
            "diagnostic_projection": "first_diagnostic_of_first_failing_phase",
            "route_selector": "ast::Program::uses_owned_syntax",
            "scalar_resolver": "hir::resolve",
            "scalar_checker": "typeck::check",
            "project_scalar_selector": "ast::Program::uses_project_syntax",
            "project_scalar_loader": "ProjectSources::load_typed(stdin.ox, default limits)",
            "project_scalar_resolver": "hir::resolve_sources(SourceOwner::project)",
            "project_scalar_input": "controller --project-source; exact stdin bytes and retained stdin.ox checked",
            "typed_view": "TypedProgram::functions / TypedFunction existing views",
            "resolve_only_request": "--resolve-only",
            "resolved_view": "actual hir::Program after whole-program resolution, before typeck::check",
            "resolved_schema": "canonical-resolution-observation-1; status resolved; phase resolve; typing pending; resolved_hir; no checked types/flows",
            "flow_projection": "exact sixteen FlowSummary Debug representations",
            "owned_route": "public_route_required marker; external qualified CLI",
            "entry_admission": False,
            "schema": "canonical-static-observation-1",
        },
        "files": copied,
        "wrappers": wrapper_manifest,
        "builder": {
            "path": str(Path(__file__).resolve().relative_to(repository)),
            "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        },
        "kind_ids": [{"id": index + 1, "kind": name} for index, name in enumerate(kinds)],
    }
    (output / "source-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (output / "toolchain.txt").write_text(version.stdout)
    binary = output / ("canonical-static-observer.exe" if os.name == "nt" else "canonical-static-observer")
    command = [rustc, "--edition=2021", "-C", "opt-level=1", str(output / "main.rs"), "-o", str(binary)]
    started = time.monotonic()
    result = subprocess.run(command, cwd=repository, capture_output=True)
    (output / "build.stdout").write_bytes(result.stdout)
    (output / "build.stderr").write_bytes(result.stderr)
    receipt = {
        "argv": command,
        "cwd": str(repository),
        "rustc_version": version.stdout,
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "exit_code": result.returncode,
        "stdout": "build.stdout",
        "stderr": "build.stderr",
        "binary": binary.name,
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest() if binary.is_file() else None,
    }
    (output / "build-evidence.json").write_text(json.dumps(receipt, indent=2) + "\n")
    files = sorted(path for path in output.rglob("*") if path.is_file())
    (output / "artifact-hashes.sha256").write_text("".join(
        hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.relative_to(output).as_posix() + "\n"
        for path in files
    ))
    if result.returncode:
        raise ValueError("rustc failed; inspect " + str(output / "build.stderr"))
    return {"observer": str(binary), "source_manifest": str(output / "source-manifest.json"),
            "build_evidence": str(output / "build-evidence.json")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True,
                        help="fresh directory for copied sources, observer executable and receipts")
    args = parser.parse_args()
    try:
        result = build(args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print("canonical typed static observer build failed: " + str(error), file=sys.stderr)
        return 1
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
