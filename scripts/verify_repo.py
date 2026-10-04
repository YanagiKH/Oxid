#!/usr/bin/env python3
from __future__ import annotations

import re
import subprocess
import sys
import tempfile
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path

from verify_feature_status import verify_feature_status
from verify_fixture_data import fixture_data_sources
from verify_typed_formatter import verify as verify_typed_formatter


ROOT = Path(__file__).resolve().parents[1]
RUNNABLE_GROUPS = ("tests", "examples", "tools", "apps")
RUNNABLE_PACKAGE_FILES = (
    "packages/demo/package.ox",
    "packages/demo/src/main.ox",
    "packages/demo/src/interop.ox",
    "packages/demo/tests/smoke.ox",
    "packages/workflow_preview.ox",
)
# Exact typed fixture inventory; all other checked-in .ox files retain their
# existing legacy checks. Never exclude an entire fixture directory.
TYPED_SOURCE_FILES = ("fixtures/owned_source/batch.ox",)
# Every member is explicitly named. Child files are checked through their root
# so their crate-relative imports preserve the real project context.
TYPED_PROJECTS = {
    "fixtures/typed-record-composition-samples/main.ox": (
        "fixtures/typed-record-composition-samples/main.ox",
        "fixtures/typed-record-composition-samples/model.ox",
        "fixtures/typed-record-composition-samples/ops.ox",
    ),
    "fixtures/typed-array-samples/main.ox": (
        "fixtures/typed-array-samples/main.ox",
        "fixtures/typed-array-samples/stats.ox",
        "fixtures/typed-array-samples/samples.ox",
    ),
    "fixtures/typed-project-batch/main.ox": (
        "fixtures/typed-project-batch/main.ox",
        "fixtures/typed-project-batch/jobs.ox",
        "fixtures/typed-project-batch/state.ox",
    ),
    "fixtures/typed-slice-samples/main.ox": (
        "fixtures/typed-slice-samples/main.ox",
        "fixtures/typed-slice-samples/buffers.ox",
        "fixtures/typed-slice-samples/stats.ox",
    ),
}
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


def runnable_sources(root: Path = ROOT) -> list[Path]:
    runnable = []
    for group in RUNNABLE_GROUPS:
        runnable.extend(sorted((root / group).glob("*.ox")))
    runnable.extend(root / relative for relative in RUNNABLE_PACKAGE_FILES)
    return runnable


def source_plan(sources: list[Path], root: Path = ROOT) -> tuple[list[tuple[Path, bool]], list[Path], int]:
    # Admission is mandatory even for direct callers. Only exact frozen paths
    # are data; an unlisted .ox in the same directories remains a legacy check.
    fixture_data = fixture_data_sources(root)
    available = set(sources)
    typed_entries = [root / relative for relative in TYPED_SOURCE_FILES]
    typed_members = set(typed_entries)
    for entry, members in TYPED_PROJECTS.items():
        if entry not in members or len(members) != len(set(members)):
            raise RuntimeError("invalid explicit typed project inventory")
        paths = {root / relative for relative in members}
        if paths & typed_members:
            raise RuntimeError("overlapping typed source inventories")
        typed_entries.append(root / entry)
        typed_members.update(paths)
    if not typed_members <= available:
        raise RuntimeError("typed source fixture missing from discovery")
    if not fixture_data <= available:
        raise RuntimeError("source-only fixture data missing from discovery")
    if fixture_data & (typed_members | set(runnable_sources(root))):
        raise RuntimeError("source-only fixture data overlaps a typed or runnable inventory")
    legacy = [(source, False) for source in sources if source not in typed_members | fixture_data]
    return legacy + [(source, True) for source in typed_entries], typed_entries, len(typed_members)


def verify_test_fixture_registration(root: Path = ROOT) -> None:
    """The CLI's opt-in exclusions must equal the already frozen data inventory."""
    fixture_data = fixture_data_sources(root)
    try:
        manifest = tomllib.loads((root / "oxid.toml").read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise RuntimeError(f"unreadable test fixture registration: {error}") from error
    configured = manifest.get("test-fixtures", {})
    expected = {path.relative_to(root).as_posix() for path in fixture_data}
    if (not isinstance(configured, dict) or set(configured) != expected
            or any(value is not True for value in configured.values())):
        raise RuntimeError("test fixture registration must exactly match frozen source-only data")


def main() -> int:
    executable = Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "target/release/oxid").resolve()
    if not executable.is_file():
        raise RuntimeError(f"Oxid executable not found: {executable}")

    verify_feature_status(ROOT)
    verify_versions()
    verify_readmes()
    verify_assets()
    verify_local_markdown_links()

    sources = sorted(path for path in ROOT.rglob("*.ox") if ".oxid" not in path.parts and "target" not in path.parts)
    checks, typed_entries, typed_member_count = source_plan(sources, ROOT)
    verify_test_fixture_registration(ROOT)
    language_source_count = len(checks) + typed_member_count - len(typed_entries)
    print(
        f"fixture-data validation passed: {len(sources) - language_source_count} source-only files "
        "(frozen manifest/body identities only; no compiler checks, executions or feature claim)"
    )
    # Admit all source-only fixture identities before invoking the executable,
    # including the independently registered formatter golden check.
    verify_typed_formatter(executable)
    for source, typed in checks:
        command = [str(executable), "check", str(source)]
        if typed:
            command.append("--edition=typed-preview")
        run(command)

    runnable = runnable_sources(ROOT)

    with tempfile.TemporaryDirectory(prefix="oxid-verify-") as temp_dir:
        temp = Path(temp_dir)
        for source in runnable:
            run([str(executable), "run", str(source)], cwd=temp)
        for source in typed_entries:
            run([str(executable), "run", str(source), "--edition=typed-preview"], cwd=temp)

    run([str(executable), "test"])
    run([str(executable), "build"])
    run([str(executable), "doctor"])
    print(
        f"repository verification passed: {language_source_count} language sources, {len(checks)} checks, "
        f"{len(runnable) + len(typed_entries)} runnable programs "
        f"({language_source_count - typed_member_count} legacy sources, "
        f"{len(runnable)} legacy runnable programs, {typed_member_count} typed source members / "
        f"{len(typed_entries)} typed entry runs)"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, ValueError) as error:
        print(f"verification error: {error}", file=sys.stderr)
        raise SystemExit(1)
