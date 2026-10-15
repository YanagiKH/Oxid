#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import os
import shutil
import stat
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


def ensure_replaceable_output(path: Path) -> None:
    """Reject directories and any symlink without following links.

    Publish only replaces absent paths or non-symlink regular files. Existing
    directories or symlinks (including dangling ones) must stay untouched.
    """
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    if not stat.S_ISREG(mode):
        raise SystemExit(
            f"refusing to overwrite non-regular-file output target: {path}"
        )


class PublishError(Exception):
    def __init__(self, message: str, keep: Path | None = None) -> None:
        super().__init__(message)
        self.keep = keep


def publish(prepared: list[tuple[Path, Path]], work_dir: Path) -> None:
    """Move prepared files to their final paths, attempting rollback on failure.

    Existing final files are first renamed into work_dir, then the new files are
    renamed into place, all within one directory tree on the same filesystem.
    This is a sequence of renames, not one atomic step: for a short time the
    final files may be missing, and a crash part way leaves the old files in
    work_dir.
    """
    undo: list[tuple[str, Path, Path]] = []
    try:
        for index, (_, final) in enumerate(prepared):
            if final.exists():
                backup = work_dir / f"previous-{index}-{final.name}"
                os.replace(final, backup)
                undo.append(("restore", backup, final))
        for new, final in prepared:
            os.replace(new, final)
            undo.append(("remove", new, final))
    except BaseException as error:
        # Attempt rollback after a publication exception. OSError failures
        # during rollback retain work_dir for recovery; repeated interruptions
        # and process crashes are outside this guarantee.
        problems = []
        for action, saved, final in reversed(undo):
            try:
                if action == "remove":
                    final.unlink()
                else:
                    os.replace(saved, final)
            except OSError as undo_error:
                problems.append(f"{final}: {undo_error}")
        if problems:
            raise PublishError(
                f"failed to publish release files: {error}; rollback failed for "
                + "; ".join(problems)
                + f"; previous files are kept in {work_dir}",
                keep=work_dir,
            ) from error
        if not isinstance(error, OSError):
            raise
        raise PublishError(
            f"failed to publish release files: {error}; "
            "final files were put back as they were before this run"
        ) from error


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
    checksum = output_dir / f"{asset_name}.sha256"
    # Link-non-following: refuse directories and any symlink before mutating.
    ensure_replaceable_output(asset)
    ensure_replaceable_output(checksum)
    release_files = required_package_files(Path.cwd())
    if asset_name.endswith(".zip"):
        write_archive = write_zip
    elif asset_name.endswith(".tar.gz"):
        write_archive = write_tar_gz
    else:
        raise SystemExit(f"unsupported archive type: {asset_name}")

    # Build both files in a private directory inside dist so that publishing is
    # a same-filesystem rename and a failure never touches the final names.
    work_dir = Path(tempfile.mkdtemp(prefix=f".{asset_name}.", suffix=".tmp", dir=output_dir))
    keep_work_dir = False
    try:
        new_asset = work_dir / asset_name
        new_checksum = work_dir / checksum.name
        try:
            with tempfile.TemporaryDirectory(prefix="oxid-package-") as temp_dir:
                staged_name = "oxid.exe" if binary.suffix == ".exe" else "oxid"
                staged = Path(temp_dir) / staged_name
                shutil.copy2(binary, staged)
                write_archive(new_asset, [(staged, staged_name), *release_files])
            digest = hashlib.sha256(new_asset.read_bytes()).hexdigest()
            new_checksum.write_text(f"{digest}  {asset_name}\n", encoding="utf-8")
        except OSError as error:
            raise SystemExit(f"failed to package {asset_name}: {error}") from error
        try:
            publish([(new_asset, asset), (new_checksum, checksum)], work_dir)
        except PublishError as error:
            keep_work_dir = error.keep is not None
            raise SystemExit(str(error)) from error
    finally:
        if not keep_work_dir:
            shutil.rmtree(work_dir, ignore_errors=True)
    print(f"packaged {asset} ({digest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
