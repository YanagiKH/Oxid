#!/usr/bin/env python3
"""Build the exact modular streaming lexical component through ordinary native compilation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "fixtures/typed-streaming-lexer/sources.json"
BUNDLE_MAGIC = "oxid-lexical-provider-v1\nlexer_sha256="


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked_sources():
    manifest_bytes = MANIFEST.read_bytes()
    manifest = json.loads(manifest_bytes)
    if manifest.get("schema_version") != 1 or manifest.get("entry") != "main":
        raise ValueError("unsupported streaming lexer source manifest")
    rows = manifest["sources"]
    pending, selected = ["main"], {}
    while pending:
        name = pending.pop()
        if name in selected:
            continue
        if re.fullmatch(r"[a-z_]+", name) is None:
            raise ValueError("invalid streaming lexer module name")
        row = rows[name]
        relative = Path(row["path"])
        if relative != Path("fixtures/typed-streaming-lexer") / (name + ".ox"):
            raise ValueError("source outside exact streaming lexer closure")
        data = (ROOT / relative).read_bytes()
        if len(data) != row["bytes"] or digest(data) != row["sha256"]:
            raise ValueError("streaming lexer source identity changed: " + name)
        if not data.isascii():
            raise ValueError("streaming lexer source is not ASCII")
        selected[name] = data
        pending.extend(re.findall(r"\bmod\s+(\w+)\s*;", data.decode("ascii")))
    if set(selected) != set(rows):
        raise ValueError("streaming lexer manifest has unused members")
    return manifest_bytes, selected


def build(compiler, llvm_bin, output, provider=None):
    compiler = Path(compiler).resolve(strict=True)
    llvm_bin = Path(llvm_bin).resolve(strict=True)
    output = Path(output).resolve()
    if output.exists() or output.is_symlink():
        raise ValueError("output must be a fresh directory")
    provider = Path(provider).resolve(strict=True) if provider is not None else None
    manifest_bytes, sources = checked_sources()
    output.mkdir()
    source_dir = output / "sources"
    source_dir.mkdir()
    for name, data in sources.items():
        (source_dir / (name + ".ox")).write_bytes(data)
    (output / "source-manifest.json").write_bytes(manifest_bytes)
    bundle = output / "bundle"
    bundle.mkdir()
    argv = [str(compiler), "compile", str(source_dir / "main.ox"),
            "--edition=typed-preview", "--backend=llvm", "--entry-mode=process",
            "--message-format=json", "--output", str(bundle / "lexer")]
    if provider is not None:
        argv.extend(["--experimental-lexical-provider", str(provider)])
    result = subprocess.run(argv, env=dict(os.environ, OXID_LLVM_BIN=str(llvm_bin)),
                            capture_output=True, timeout=120)
    (output / "compile.stdout").write_bytes(result.stdout)
    (output / "compile.stderr").write_bytes(result.stderr)
    receipt = dict(argv=argv, exit_code=result.returncode,
                   compiler_sha256=digest(compiler.read_bytes()),
                   source_manifest_sha256=digest(manifest_bytes),
                   source_count=len(sources), source_bytes=sum(map(len, sources.values())),
                   stdout_sha256=digest(result.stdout), stderr_sha256=digest(result.stderr),
                   lexical_provider=str(provider) if provider is not None else None)
    if result.returncode == 0:
        receipt["lexer_sha256"] = digest((bundle / "lexer").read_bytes())
        (bundle / "manifest.txt").write_text(BUNDLE_MAGIC + receipt["lexer_sha256"] + "\n")
    (output / "build-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError("streaming lexer native build failed; see " + str(output))
    return bundle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", required=True, type=Path)
    parser.add_argument("--llvm-bin", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--provider", type=Path,
                        help="explicit existing lexical bundle for rebuilding this real closure")
    args = parser.parse_args()
    print(build(args.compiler, args.llvm_bin, args.output, args.provider))


if __name__ == "__main__":
    main()
