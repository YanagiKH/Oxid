#!/usr/bin/env python3
"""Install a release archive locally and run a new Oxid project."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib

from package_release import PACKAGE_FILES


ROOT = Path(__file__).resolve().parents[1]


def run(args: list[str], cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[bytes]:
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, timeout=90)
    if result.returncode != 0:
        output = (result.stdout + result.stderr).decode("utf-8", errors="replace")
        raise ValueError(f"{Path(args[0]).name} {args[1]} exited with status {result.returncode}\n{output}")
    return result


def clean_environment() -> dict[str, str]:
    return {name: value for name, value in os.environ.items() if not name.upper().startswith("OXID_")}


def verify(archive: Path) -> None:
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    env = clean_environment()
    windows = sys.platform == "win32"
    if windows:
        installer = ["pwsh", "-NoLogo", "-NoProfile", "-NonInteractive", "-File", str(ROOT / "install.ps1")]
    else:
        installer = ["sh", str(ROOT / "install.sh")]
    with tempfile.TemporaryDirectory(prefix="oxid-release-install-") as directory:
        work = Path(directory)
        install_dir = work / "installed"
        run(installer, work, dict(env, OXID_ARCHIVE=str(archive), OXID_INSTALL_DIR=str(install_dir)))
        oxid = str(install_dir / ("oxid.exe" if windows else "oxid"))
        reported = run([oxid, "--version"], work, env)
        if reported.stdout.splitlines() != [f"Oxid {version}".encode()] or reported.stderr:
            raise ValueError(f"expected Oxid {version}, found {reported.stdout!r} {reported.stderr!r}")

        run([oxid, "new", "hello", "--edition=typed-preview"], work, env)
        project = work / "hello"
        (project / "src/main.ox").write_text("fn main() -> i32 { return 6 * 7; }\n", encoding="utf-8")
        run([oxid, "check"], project, env)
        run([oxid, "test"], project, env)
        executed = run([oxid, "run", "--entry-mode=result"], project, env)
        if executed.stdout != b"42\n" or executed.stderr:
            raise ValueError(f"unexpected program output {executed.stdout!r} {executed.stderr!r}")
    print(f"Installed and ran Oxid {version} from {archive.name}")


def verify_binary(binary: Path) -> None:
    system = {"linux": "linux", "darwin": "macos", "win32": "windows"}.get(sys.platform)
    machine = platform.machine().lower()
    arch = {"amd64": "x86_64", "arm64": "aarch64"}.get(machine, machine)
    supported = {("linux", "x86_64"), ("linux", "aarch64"), ("macos", "x86_64"),
                 ("macos", "aarch64"), ("windows", "x86_64")}
    if (system, arch) not in supported:
        raise ValueError(f"no release archive for {sys.platform} {machine}")
    suffix = "zip" if system == "windows" else "tar.gz"
    asset = f"oxid-{system}-{arch}.{suffix}"
    env = clean_environment()
    with tempfile.TemporaryDirectory(prefix="oxid-candidate-archive-") as directory:
        work = Path(directory)
        for relative in PACKAGE_FILES:
            if relative.parts[0] != ".oxid":
                destination = work / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(ROOT / relative, destination)
        run([str(binary), "bootstrap"], work, env)
        run([sys.executable, "-B", str(ROOT / "scripts/package_release.py"), str(binary), asset], work, env)
        verify(work / "dist" / asset)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, nargs="?", help="local release archive with its .sha256 file")
    parser.add_argument("--binary", type=Path, help="build and install an archive from this compiler")
    args = parser.parse_args()
    if (args.archive is None) == (args.binary is None):
        parser.error("provide either an archive or --binary")
    try:
        if args.binary is None:
            verify(args.archive.resolve(strict=True))
        else:
            verify_binary(args.binary.resolve(strict=True))
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"Release installation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
