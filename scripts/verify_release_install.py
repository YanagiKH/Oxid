#!/usr/bin/env python3
"""Install a release archive locally and run a new Oxid project."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]


def run(args: list[str], cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[bytes]:
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, timeout=90)
    if result.returncode != 0:
        output = (result.stdout + result.stderr).decode("utf-8", errors="replace")
        raise ValueError(f"{Path(args[0]).name} {args[1]} exited with status {result.returncode}\n{output}")
    return result


def verify(archive: Path) -> None:
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    env = {name: value for name, value in os.environ.items() if not name.upper().startswith("OXID_")}
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="local release archive with its .sha256 file")
    args = parser.parse_args()
    try:
        verify(args.archive.resolve(strict=True))
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"Release installation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
