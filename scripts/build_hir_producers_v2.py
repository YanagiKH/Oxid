#!/usr/bin/env python3
"""Build the opt-in v2 source overlays without changing historical v1 inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "fixtures/typed-frontend-v2/sources.json"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def materialize(output):
    manifest = json.loads(MANIFEST.read_bytes())
    if manifest.get("schema_version") != 2:
        raise ValueError("unsupported v2 source manifest")
    sources = manifest["sources"]
    pending, selected = ["parser_main", "ast_static_main"], {}
    while pending:
        name = pending.pop()
        if name in selected:
            continue
        if re.fullmatch(r"[a-z_]+", name) is None:
            raise ValueError("invalid source module")
        record = sources[name]
        relative = Path(record["path"])
        if relative.parent not in (Path("fixtures/typed-lexer-samples"), Path("fixtures/typed-frontend-v2")) or relative.name != name + ".ox":
            raise ValueError("source outside explicit v1/v2 closure")
        data = (ROOT / relative).read_bytes()
        if digest(data) != record["sha256"]:
            raise ValueError("source identity changed: " + name)
        selected[name] = data
        pending.extend(re.findall(r"\bmod\s+(\w+)\s*;", data.decode()))
    if set(selected) != set(sources):
        raise ValueError("source manifest has unused entries")
    # Check the whole closure before creating any output. Never overwrite runs.
    output.mkdir()
    for name, data in selected.items():
        (output / (name + ".ox")).write_bytes(data)
    (output / "source-manifest.json").write_bytes(MANIFEST.read_bytes())
    return manifest


def build(compiler, llvm_bin, output):
    compiler = compiler.resolve(strict=True)
    llvm_bin = llvm_bin.resolve(strict=True)
    output = output.resolve()
    output.mkdir()
    manifest = materialize(output / "sources")
    bundle = output / "bundle"
    bundle.mkdir()
    env = dict(os.environ, OXID_LLVM_BIN=str(llvm_bin))
    receipts = []
    for role, root in (("parser", "parser_main"), ("static", "ast_static_main")):
        argv = [str(compiler), "compile", str(output / "sources" / (root + ".ox")),
                "--edition=typed-preview", "--backend=llvm", "--entry-mode=process",
                "--output", str(bundle / role)]
        result = subprocess.run(argv, env=env, capture_output=True, timeout=120)
        (output / (role + ".stdout")).write_bytes(result.stdout)
        (output / (role + ".stderr")).write_bytes(result.stderr)
        receipts.append(dict(role=role, argv=argv, exit_code=result.returncode,
                             stdout_sha256=digest(result.stdout), stderr_sha256=digest(result.stderr)))
        (output / "build-receipts.json").write_text(json.dumps(receipts, indent=2) + "\n")
        if result.returncode:
            raise RuntimeError(role + " native build failed; retained output: " + str(output))
    hashes = {role: digest((bundle / role).read_bytes()) for role in ("parser", "static")}
    (bundle / "manifest.txt").write_text("OXID-HIR-PRODUCERS-2\n" + "".join(
        role + " " + value + "\n" for role, value in hashes.items()))
    (output / "build-evidence.json").write_text(json.dumps(dict(
        schema_version=2, compiler_sha256=digest(compiler.read_bytes()),
        source_manifest_sha256=digest(MANIFEST.read_bytes()), executable_sha256=hashes,
        source_count=len(manifest["sources"])), indent=2) + "\n")
    return bundle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", required=True, type=Path)
    parser.add_argument("--llvm-bin", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    print(build(args.compiler, args.llvm_bin, args.output))


if __name__ == "__main__":
    main()
