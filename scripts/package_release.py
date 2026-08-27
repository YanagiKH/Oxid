#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import shutil
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path


PACKAGE_FILES = (
    Path(".oxid/bootstrap/compiler.oxb"),
    Path(".oxid/bootstrap/manifest.json"),
    Path("compiler/main.ox"),
    Path("compiler/providers.toml"),
    Path("stdlib/frontend/bytecode.ox"),
)


def required_package_files(root: Path) -> list[tuple[Path, str]]:
    files: list[tuple[Path, str]] = []
    for relative in PACKAGE_FILES:
        source = root / relative
        if not source.is_file():
            raise SystemExit(f"required release file not found: {source}")
        files.append((source, relative.as_posix()))
    return files


def write_zip(asset: Path, files: list[tuple[Path, str]]) -> None:
    with zipfile.ZipFile(asset, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for source, archive_name in files:
            archive.write(source, archive_name)


def write_tar_gz(asset: Path, files: list[tuple[Path, str]]) -> None:
    with tarfile.open(asset, "w:gz") as archive:
        for source, archive_name in files:
            archive.add(source, arcname=archive_name)


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: package_release.py <binary> <asset-name>")

    binary = Path(sys.argv[1]).resolve()
    asset_name = sys.argv[2]
    if not binary.is_file():
        raise SystemExit(f"binary not found: {binary}")
    if Path(asset_name).name != asset_name:
        raise SystemExit("asset name must not contain a directory")

    output_dir = Path("dist")
    output_dir.mkdir(exist_ok=True)
    asset = output_dir / asset_name
    release_files = required_package_files(Path.cwd())

    with tempfile.TemporaryDirectory(prefix="oxid-package-") as temp_dir:
        staged_name = "oxid.exe" if binary.suffix == ".exe" else "oxid"
        staged = Path(temp_dir) / staged_name
        shutil.copy2(binary, staged)
        archive_files = [(staged, staged_name), *release_files]
        if asset_name.endswith(".zip"):
            write_zip(asset, archive_files)
        elif asset_name.endswith(".tar.gz"):
            write_tar_gz(asset, archive_files)
        else:
            raise SystemExit(f"unsupported archive type: {asset_name}")

    digest = hashlib.sha256(asset.read_bytes()).hexdigest()
    (output_dir / f"{asset_name}.sha256").write_text(f"{digest}  {asset_name}\n", encoding="utf-8")
    print(f"packaged {asset} ({digest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
