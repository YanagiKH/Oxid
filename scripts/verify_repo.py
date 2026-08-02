#!/usr/bin/env python3
from __future__ import annotations

import re
import subprocess
import sys
import tempfile
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNNABLE_GROUPS = ("tests", "examples", "tools", "apps")
RUNNABLE_PACKAGE_FILES = (
    "packages/demo/package.ox",
    "packages/demo/src/main.ox",
    "packages/demo/src/interop.ox",
    "packages/demo/tests/smoke.ox",
    "packages/workflow_preview.ox",
)
READMES = ("README.md", "README_ZH.md", "README_JP.md")
IMAGES = (
    "docs/assets/quickstart.svg",
    "docs/assets/architecture.svg",
    "docs/assets/interop.svg",
    "docs/assets/web-discord.svg",
)


def run(command: list[str], *, cwd: Path = ROOT) -> None:
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True)
    if result.returncode:
        detail = (result.stdout + result.stderr).strip()
        raise RuntimeError(f"command failed ({' '.join(command)}):\n{detail}")


def readme_shape(text: str) -> tuple[list[int], int, int]:
    heading_levels = [len(match.group(1)) for match in re.finditer(r"^(#{1,6})\s+", text, re.MULTILINE)]
    return heading_levels, text.count("```") // 2, text.count("<img ")


def verify_readmes() -> None:
    documents = [(ROOT / name).read_text(encoding="utf-8") for name in READMES]
    expected_shape = readme_shape(documents[0])
    for name, document in zip(READMES, documents, strict=True):
        if readme_shape(document) != expected_shape:
            raise RuntimeError(f"{name} does not have the same section/code/image structure as README.md")
        for image in IMAGES:
            if image not in document:
                raise RuntimeError(f"{name} does not reference {image}")
        for sibling in READMES:
            if sibling not in document:
                raise RuntimeError(f"{name} does not link to {sibling}")


def verify_versions() -> None:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    manifest = tomllib.loads((ROOT / "oxid.toml").read_text(encoding="utf-8"))
    if cargo["package"]["version"] != manifest["project"]["version"]:
        raise RuntimeError("Cargo.toml and oxid.toml versions differ")


def verify_assets() -> None:
    for relative in IMAGES:
        ET.parse(ROOT / relative)


def verify_local_markdown_links() -> None:
    pattern = re.compile(r"!?(?:\[[^\]]*\])\(([^)]+)\)")
    for document in ROOT.rglob("*.md"):
        if ".git" in document.parts or "target" in document.parts:
            continue
        text = document.read_text(encoding="utf-8")
        for raw_target in pattern.findall(text):
            target = raw_target.strip().split(maxsplit=1)[0].strip("<>")
            if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                continue
            path_text = target.split("#", 1)[0]
            if path_text and not (document.parent / path_text).resolve().exists():
                relative_document = document.relative_to(ROOT)
                raise RuntimeError(f"broken local Markdown link in {relative_document}: {target}")


def main() -> int:
    executable = Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "target/release/oxid").resolve()
    if not executable.is_file():
        raise RuntimeError(f"Oxid executable not found: {executable}")

    verify_versions()
    verify_readmes()
    verify_assets()
    verify_local_markdown_links()

    sources = sorted(path for path in ROOT.rglob("*.ox") if ".oxid" not in path.parts and "target" not in path.parts)
    for source in sources:
        run([str(executable), "check", str(source)])

    runnable = []
    for group in RUNNABLE_GROUPS:
        runnable.extend(sorted((ROOT / group).glob("*.ox")))
    runnable.extend(ROOT / relative for relative in RUNNABLE_PACKAGE_FILES)

    with tempfile.TemporaryDirectory(prefix="oxid-verify-") as temp_dir:
        temp = Path(temp_dir)
        for source in runnable:
            run([str(executable), "run", str(source)], cwd=temp)

    run([str(executable), "test"])
    run([str(executable), "build"])
    run([str(executable), "doctor"])
    print(f"repository verification passed: {len(sources)} sources, {len(runnable)} runnable programs")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as error:
        print(f"verification error: {error}", file=sys.stderr)
        raise SystemExit(1)
