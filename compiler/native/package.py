#!/usr/bin/env python3
"""Package the native compiler and the inputs needed to rebuild it."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile


ROOT = Path(__file__).resolve().parents[2]
PROJECT = Path("compiler/native")
BINARY = PROJECT / "bin/oxid-scalar-compiler"
MANIFEST = PROJECT / "manifest.json"
RUNTIME_FILES = (
    "native/typed_preview.c", "native/typed_character.c", "native/typed_bytebuf.c", "native/typed_termination.c",
    "native/typed_vector.c", "native/experimental_bytebuf.c", "native/experimental_bytebuf.h",
    "native/experimental_vector.c", "native/experimental_vector.h", "native/experimental_files.c",
)
ELF_TARGETS = {62: "x86_64-unknown-linux-gnu", 183: "aarch64-unknown-linux-gnu"}
MACHO_TARGETS = {0x1000007: "x86_64-apple-macosx11.0.0", 0x100000C: "arm64-apple-macosx11.0.0"}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_file(path: Path) -> bytes:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"missing regular compiler input: {path}")
    return path.read_bytes()


def seed_target(seed: Path, binary: bytes) -> str:
    if not os.access(seed, os.X_OK):
        raise ValueError("the seed file is not executable")
    target = None
    if len(binary) >= 64 and binary[:6] == b"\x7fELF\x02\x01":
        if int.from_bytes(binary[16:18], "little") in (2, 3):
            target = ELF_TARGETS.get(int.from_bytes(binary[18:20], "little"))
    elif len(binary) >= 32 and binary[:4] == b"\xcf\xfa\xed\xfe":
        if int.from_bytes(binary[12:16], "little") == 2:
            target = MACHO_TARGETS.get(int.from_bytes(binary[4:8], "little"))
    if target is None:
        raise ValueError("the seed must be a Linux or macOS x86-64 or ARM64 executable")
    return target


def package(seed: Path, destination: Path, source: Path = ROOT / PROJECT / "src") -> dict:
    seed = seed.absolute()
    destination = destination.absolute()
    binary = read_file(seed)
    contents = {BINARY.as_posix(): binary}
    for path in ("LICENSE", *(str(PROJECT / name) for name in ("SCOPE.md", "build.py", "runtime.c")),
                 *RUNTIME_FILES):
        contents[path] = read_file(ROOT / path)
    for path in sorted(source.rglob("*.ox")):
        identity = (PROJECT / "src" / path.relative_to(source)).as_posix()
        contents[identity] = read_file(path)
    if (PROJECT / "src/main.ox").as_posix() not in contents:
        raise ValueError("missing native compiler entry source")

    manifest = {"target": seed_target(seed, binary), "compiler": BINARY.as_posix(),
                "files": {path: digest(data) for path, data in sorted(contents.items())}}
    contents[MANIFEST.as_posix()] = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    with tempfile.TemporaryDirectory(prefix=".oxid-native-kit-", dir=destination.parent) as directory:
        staged = Path(directory) / "kit.tar.gz"
        with staged.open("wb") as stream:
            with gzip.GzipFile(filename="", mode="wb", fileobj=stream, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                    for path, data in sorted(contents.items()):
                        entry = tarfile.TarInfo(path)
                        entry.mode = 0o755 if path == BINARY.as_posix() else 0o644
                        entry.size = len(data)
                        archive.addfile(entry, io.BytesIO(data))
            stream.flush()
            os.fsync(stream.fileno())
        archive_sha256 = digest(staged.read_bytes())
        os.link(staged, destination)
    return {"archive": str(destination), "archive_sha256": archive_sha256, **manifest}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = package(args.compiler, args.output)
    except (OSError, ValueError) as error:
        parser.exit(1, f"could not package native compiler: {error}\n")
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
