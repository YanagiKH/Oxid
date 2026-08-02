#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import shutil
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: package_release.py <binary> <asset-name>")

    binary = Path(sys.argv[1]).resolve()
    asset_name = sys.argv[2]
    if not binary.is_file():
        raise SystemExit(f"binary not found: {binary}")

    output_dir = Path("dist")
    output_dir.mkdir(exist_ok=True)
    asset = output_dir / asset_name

    with tempfile.TemporaryDirectory(prefix="oxid-package-") as temp_dir:
        staged_name = "oxid.exe" if binary.suffix == ".exe" else "oxid"
        staged = Path(temp_dir) / staged_name
        shutil.copy2(binary, staged)
        if asset_name.endswith(".zip"):
            with zipfile.ZipFile(asset, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
                archive.write(staged, staged_name)
        elif asset_name.endswith(".tar.gz"):
            with tarfile.open(asset, "w:gz") as archive:
                archive.add(staged, arcname=staged_name)
        else:
            raise SystemExit(f"unsupported archive type: {asset_name}")

    digest = hashlib.sha256(asset.read_bytes()).hexdigest()
    (output_dir / f"{asset_name}.sha256").write_text(f"{digest}  {asset_name}\n", encoding="utf-8")
    print(f"packaged {asset} ({digest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
