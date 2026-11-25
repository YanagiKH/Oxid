#!/usr/bin/env python3
"""Verify an offline installation and optionally upgrade and roll back."""
from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

from check_release_gates import TAG_PATTERN, load_package_version
from package_release import PACKAGE_FILES


ROOT = Path(__file__).resolve().parents[1]
PUBLISHED_ARCHIVES = {
    "0.8.0": {
        "oxid-linux-x86_64.tar.gz": "ed7b85d9a924e59cc20eddd6e6f1bd3021eeae8bd2a41db2042ede60da906773",
        "oxid-macos-aarch64.tar.gz": "0072aa3c941da266371d51fd633165166378bb6ec50b34b8b0842ffa91a1c881",
        "oxid-macos-x86_64.tar.gz": "9da4e0c07bedc485149ab9a5643562ce57ebe9760e14f40e51ee39ef5d5bfdb3",
        "oxid-windows-x86_64.zip": "4772e3524d10ffeec62df841074149b3b844ae60c410204e5cfabf0ce88e2cf9",
    },
    "0.9.0": {
        "oxid-linux-x86_64.tar.gz": "73359497b72e367353ad80a913880c582701c88a18e4a52fd188427f7c940aed",
        "oxid-macos-aarch64.tar.gz": "3cdc70ee46d458e8a4691277e4edfdd2bec34c3b0e56a4b87bf93173b69b3097",
        "oxid-macos-x86_64.tar.gz": "3708d3931903aadc97590cde4fa2ceaf0323440bffaacef957a7e16d35c1839a",
        "oxid-windows-x86_64.zip": "86d1354c02e79f56dcf8eb03ad2123789472098483fb6da46cbf96fd35bbb1ed",
    },
}


def run(args: list[str], cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[bytes]:
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, timeout=90)
    if result.returncode != 0:
        output = (result.stdout + result.stderr).decode("utf-8", errors="replace")
        raise ValueError(f"{Path(args[0]).name} {args[1]} exited with status {result.returncode}\n{output}")
    return result


def clean_environment() -> dict[str, str]:
    return {name: value for name, value in os.environ.items() if not name.upper().startswith("OXID_")}


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def check_version(oxid: str, version: str, work: Path, env: dict[str, str]) -> None:
    reported = run([oxid, "--version"], work, env)
    if reported.stdout.splitlines() != [f"Oxid {version}".encode()] or reported.stderr:
        raise ValueError(f"expected Oxid {version}, found {reported.stdout!r} {reported.stderr!r}")


def check_typed_project(oxid: str, work: Path, env: dict[str, str]) -> None:
    run([oxid, "new", "hello", "--edition=typed-preview"], work, env)
    project = work / "hello"
    (project / "src/main.ox").write_text("fn main() -> i32 { return 6 * 7; }\n", encoding="utf-8")
    run([oxid, "check"], project, env)
    run([oxid, "test"], project, env)
    executed = run([oxid, "run", "--entry-mode=result"], project, env)
    if executed.stdout != b"42\n" or executed.stderr:
        raise ValueError(f"unexpected program output {executed.stdout!r} {executed.stderr!r}")


def check_legacy_project(oxid: str, project: Path, expected: dict[Path, str],
                         env: dict[str, str]) -> None:
    run([oxid, "check", "src/main.ox"], project, env)
    execution = run([oxid, "run", "src/main.ox"], project, env)
    if execution.stdout != b"42\n" or execution.stderr:
        raise ValueError(f"existing project output changed: {execution.stdout!r} {execution.stderr!r}")
    for path, value in expected.items():
        if digest(path) != value:
            raise ValueError(f"existing file changed: {path.name}")


def verify(archive: Path, previous: tuple[Path, str] | None = None) -> None:
    version = load_package_version()
    if previous and previous[1] == version:
        raise ValueError("upgrade verification requires a previous release with a different version")
    env = clean_environment()
    windows = sys.platform == "win32"
    if windows:
        installer = ["pwsh", "-NoLogo", "-NoProfile", "-NonInteractive", "-File", str(ROOT / "install.ps1")]
    else:
        installer = ["sh", str(ROOT / "install.sh")]
    with tempfile.TemporaryDirectory(prefix="oxid-release-install-") as directory:
        work = Path(directory)
        install_dir = work / "installed"
        executable = install_dir / ("oxid.exe" if windows else "oxid")
        oxid = str(executable)

        def install_environment(source: Path) -> dict[str, str]:
            return dict(env, OXID_ARCHIVE=str(source), OXID_INSTALL_DIR=str(install_dir))

        if previous:
            previous_archive, previous_version = previous
            run(installer, work, install_environment(previous_archive))
            check_version(oxid, previous_version, work, env)
            old_binary = digest(executable)

            run([oxid, "new", "existing"], work, env)
            project = work / "existing"
            entry = project / "src/main.ox"
            entry.write_text("fn main() { print(6 * 7); }\n", encoding="utf-8")
            data = project / "local-data.bin"
            data.write_bytes(b"saved data\x00\xff")
            user_file = install_dir / "local-config.toml"
            user_file.write_text('profile = "personal"\n', encoding="utf-8")
            expected = {path: digest(path) for path in (entry, project / "oxid.toml", data, user_file)}
            check_legacy_project(oxid, project, expected, env)

            corrupt = work / "corrupt" / archive.name
            corrupt.parent.mkdir()
            shutil.copyfile(archive, corrupt)
            with corrupt.open("r+b") as stream:
                first = stream.read(1)
                if not first:
                    raise ValueError("cannot verify failure recovery with an empty archive")
                stream.seek(0)
                stream.write(bytes([first[0] ^ 1]))
            shutil.copyfile(Path(str(archive) + ".sha256"), Path(str(corrupt) + ".sha256"))
            refused = subprocess.run(installer, cwd=work, env=install_environment(corrupt),
                                     capture_output=True, timeout=90)
            if refused.returncode == 0 or b"Checksum verification failed." not in (
                refused.stdout + refused.stderr
            ):
                raise ValueError("installer did not reject the corrupted upgrade checksum")
            if digest(executable) != old_binary:
                raise ValueError("failed upgrade changed the installed compiler")
            check_legacy_project(oxid, project, expected, env)

        run(installer, work, install_environment(archive))
        check_version(oxid, version, work, env)
        check_typed_project(oxid, work, env)
        if previous:
            check_legacy_project(oxid, project, expected, env)
            run(installer, work, install_environment(previous_archive))
            check_version(oxid, previous_version, work, env)
            if digest(executable) != old_binary:
                raise ValueError("rollback did not restore the previous compiler")
            check_legacy_project(oxid, project, expected, env)

    print(f"Installed and ran Oxid {version} from {archive.name}")
    if previous:
        print(f"Verified {previous[1]} to {version}, rejected a corrupted upgrade, and rolled back "
              "without changes to the existing project or its saved data.")


def host_asset() -> str:
    system = {"linux": "linux", "darwin": "macos", "win32": "windows"}.get(sys.platform)
    machine = platform.machine().lower()
    arch = {"amd64": "x86_64", "arm64": "aarch64"}.get(machine, machine)
    supported = {("linux", "x86_64"), ("linux", "aarch64"), ("macos", "x86_64"),
                 ("macos", "aarch64"), ("windows", "x86_64")}
    if (system, arch) not in supported:
        raise ValueError(f"no release archive for {sys.platform} {machine}")
    suffix = "zip" if system == "windows" else "tar.gz"
    return f"oxid-{system}-{arch}.{suffix}"


def fetch_previous(work: Path, asset: str, version: str, env: dict[str, str]) -> tuple[Path, str]:
    expected = PUBLISHED_ARCHIVES[version].get(asset)
    if expected is None:
        raise ValueError(f"Oxid {version} has no published archive for {asset}")
    folder = work / "previous"
    folder.mkdir()
    archive = folder / asset
    curl = "curl.exe" if sys.platform == "win32" else "curl"
    url = f"https://github.com/YanagiKH/Oxid/releases/download/v{version}/{asset}"
    run([curl, "--proto", "=https", "--proto-redir", "=https", "--tlsv1.2",
         "--fail", "--location", "--silent", "--show-error", "--retry", "2",
         "--retry-max-time", "50", "--connect-timeout", "15", "--max-time", "25",
         "--max-filesize", "1048576", "--output", str(archive), url], work, env)
    if archive.stat().st_size > 1048576 or digest(archive) != expected:
        raise ValueError(f"published archive checksum mismatch for {asset}")
    archive.with_name(asset + ".sha256").write_bytes(f"{expected}  {asset}\n".encode("ascii"))
    return archive, version


def version_core(version: str) -> tuple[int, int, int]:
    match = TAG_PATTERN.fullmatch(f"v{version}")
    if match is None:
        raise ValueError(f"invalid package version {version!r}")
    major, minor, patch = match.group(1, 2, 3)
    return int(major), int(minor), int(patch)


def newest_published_previous(candidate: str, asset: str) -> str | None:
    candidate_core = version_core(candidate)
    return max((version for version, archives in PUBLISHED_ARCHIVES.items()
                if asset in archives and version_core(version) < candidate_core),
               key=version_core, default=None)


def verify_archive(archive: Path, previous: tuple[Path, str] | None,
                   published_previous: str | None) -> None:
    if published_previous is None:
        verify(archive, previous)
        return
    asset = host_asset()
    if archive.name != asset:
        raise ValueError(f"expected the release archive for this host, {asset}")
    candidate = load_package_version()
    if published_previous == "auto":
        published_previous = newest_published_previous(candidate, asset)
        if published_previous is None:
            print(f"No earlier published archive for {asset}. Checking a fresh installation.")
            verify(archive)
            return
    if version_core(published_previous) >= version_core(candidate):
        raise ValueError("published upgrade baseline must be older than the candidate")
    with tempfile.TemporaryDirectory(prefix="oxid-published-archive-") as directory:
        baseline = fetch_previous(Path(directory), asset, published_previous, clean_environment())
        verify(archive, baseline)


def verify_binary(binary: Path, previous: tuple[Path, str] | None = None,
                  published_previous: str | None = None) -> None:
    asset = host_asset()
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
        verify_archive(work / "dist" / asset, previous, published_previous)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, nargs="?", help="local release archive with its .sha256 file")
    parser.add_argument("--binary", type=Path, help="build and install an archive from this compiler")
    parser.add_argument("--previous-archive", type=Path, help="older archive with its .sha256 file")
    parser.add_argument("--previous-version", help="version reported by the older compiler")
    parser.add_argument("--published-previous", choices=["auto", *sorted(PUBLISHED_ARCHIVES)],
                        help="use a pinned published baseline, or auto-select the newest older release for this host")
    args = parser.parse_args()
    if (args.archive is None) == (args.binary is None):
        parser.error("provide either an archive or --binary")
    if (args.previous_archive is None) != (args.previous_version is None):
        parser.error("provide both --previous-archive and --previous-version")
    if args.published_previous and args.previous_archive is not None:
        parser.error("choose either a local or a published previous archive")
    try:
        previous = ((args.previous_archive.resolve(strict=True), args.previous_version)
                    if args.previous_archive is not None else None)
        if args.binary is None:
            verify_archive(args.archive.resolve(strict=True), previous, args.published_previous)
        else:
            verify_binary(args.binary.resolve(strict=True), previous, args.published_previous)
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"Release installation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
