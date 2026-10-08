#!/usr/bin/env python3
"""Compile genuine sibling consumers against the actual frontend definitions.

Only the generated root frontend module receives an additional sibling probe;
production child modules are included unchanged by absolute source path.
"""
import argparse
import hashlib
import json
from pathlib import Path
import os
import re
import shutil
import subprocess


PROBES = {
    "immutable-access": (True, """
fn inspect(index: &super::declaration_index::DeclarationIndex<'_>) {
    let _ = index.sources();
    let _ = index.root_original_main();
    let _ = index.function(super::hir::DefId(0));
}
"""),
    "construct-source-owner": (False, """
fn forge<'s>() -> super::declaration_index::SourceOwner<'s> {
    super::declaration_index::SourceOwner { kind: panic!() }
}
"""),
    "replace-source-view": (False, """
fn substitute(source: &mut super::declaration_index::SourceOwner<'_>) {
    source.kind = panic!();
}
"""),
    "construct-index": (False, """
fn forge<'s>() -> super::declaration_index::DeclarationIndex<'s> {
    super::declaration_index::DeclarationIndex { tables: panic!() }
}
"""),
    "construct-facts": (False, """
fn forge<'s>() -> super::declaration_index::DeclarationFacts<'s> {
    super::declaration_index::DeclarationFacts { tables: panic!(), scratch: panic!(), plan: panic!() }
}
"""),
    "construct-clean-witness": (False, """
fn forge<'s>() {
    let _ = super::declaration_index::sealed::CleanOriginals { facts: panic!() };
}
"""),
    "replace-source-association": (False, """
fn substitute(index: &mut super::declaration_index::DeclarationIndex<'_>) {
    index.tables.sources = panic!();
}
"""),
    "mutable-table-access": (False, """
fn mutate(index: &mut super::declaration_index::DeclarationIndex<'_>) {
    index.tables.originals.clear();
}
"""),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--cargo", default="cargo")
    args = parser.parse_args()
    repo, output = args.repo.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    (output / "native").symlink_to(repo / "native", target_is_directory=True)
    original = (repo / "src/frontend/mod.rs").read_text()
    source_hashes = {str(p.relative_to(repo)): hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in sorted((repo / "src/frontend").rglob("*.rs"))}
    # Keep the real package/dependency graph and exact lockfile. Only redirect
    # its binary entrypoint and disable the unrelated native linker build script.
    manifest = (repo / "Cargo.toml").read_text()
    assert manifest.count('build = "build.rs"') == 1
    assert manifest.count('path = "src/cli.rs"') == 1
    manifest = manifest.replace('build = "build.rs"', 'build = false').replace(
        'path = "src/cli.rs"', 'path = "main.rs"')
    lock = (repo / "Cargo.lock").read_bytes()
    inputs = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
              for name in ("Cargo.toml", "Cargo.lock")}
    env = dict(os.environ, RUSTC=str(Path(shutil.which(args.rustc) or args.rustc).absolute()),
               CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2")
    rows = []
    for name, (expected, probe) in PROBES.items():
        directory = output / name
        frontend = directory / "frontend"
        frontend.mkdir(parents=True)
        for child in (repo / "src/frontend").iterdir():
            if child.name != "mod.rs":
                (frontend / child.name).symlink_to(child, target_is_directory=child.is_dir())
        source = directory / "main.rs"
        module = frontend / "mod.rs"
        module.write_text(original + "\nmod sibling_probe {\n" + probe + "\n}\n")
        source.write_text('#![allow(dead_code, unused_imports, unreachable_code)]\nmod frontend;\nfn main() {}\n')
        (directory / "Cargo.toml").write_text(manifest)
        (directory / "Cargo.lock").write_bytes(lock)
        argv = [args.cargo, "check", "--offline", "--locked", "--bin", "oxid",
                "--manifest-path", str(directory / "Cargo.toml"),
                "--target-dir", str(output / "target"), "--message-format=json"]
        result = subprocess.run(argv, text=True, capture_output=True, env=env,
                                cwd=directory, timeout=180)
        messages = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
        errors = [m["message"] for m in messages if m.get("reason") == "compiler-message"
                  and m["message"]["level"] == "error"]
        codes = [m["code"]["code"] if m.get("code") else None for m in errors]
        artifacts = [{"package_id": m["package_id"], "files": [
            {"path": f, "sha256": hashlib.sha256(Path(f).read_bytes()).hexdigest()}
            for f in m["filenames"]]} for m in messages
            if m.get("reason") == "compiler-artifact"]
        assert (directory / "Cargo.lock").read_bytes() == lock
        (output / (name + ".stdout")).write_text(result.stdout)
        (output / (name + ".stderr")).write_text(result.stderr)
        success = result.returncode == 0
        if not expected:
            # A random compile failure is not evidence of the intended seal.
            allowed = {"E0451", "E0616"}
            if name == "construct-clean-witness":
                allowed.add("E0603")
            matched = any(code in allowed for code in codes) and all(
                code in allowed or (code is None and re.fullmatch(r"type `[^`]+` is private", error["message"]))
                for code, error in zip(codes, errors))
        else:
            matched = True
        rows.append({"name": name, "expected_compile_success": expected,
                     "actual_compile_success": success, "exit": result.returncode,
                     "intended_privacy_error": matched, "argv": argv,
                     "error_codes": codes, "dependency_artifacts": artifacts,
                     "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest()})
    for path, expected in source_hashes.items():
        assert hashlib.sha256((repo / path).read_bytes()).hexdigest() == expected
    for path, expected in inputs.items():
        assert hashlib.sha256((repo / path).read_bytes()).hexdigest() == expected
    passed = all(r["actual_compile_success"] == r["expected_compile_success"] and r["intended_privacy_error"] for r in rows)
    (output / "result.json").write_text(json.dumps({"passed": passed, "source_files": source_hashes, "cargo_inputs": inputs, "probes": rows}, indent=2) + "\n")
    print(f"{sum(r['actual_compile_success'] == r['expected_compile_success'] and r['intended_privacy_error'] for r in rows)}/{len(rows)} seal probes passed")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
