#!/usr/bin/env python3
"""Verify a compiler archive against its signed GitHub build provenance.

The GitHub CLI verifies the archive digest, Sigstore certificate, signature and
transparency evidence. Repository, producer workflow, source ref and commit are
required; checksum files alone cannot establish that identity. Supplying an
independently obtained trusted root permits verification without downloads.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


REPOSITORY = "YanagiKH/Oxid"
WORKFLOW = REPOSITORY + "/.github/workflows/release.yml"
ASSETS = frozenset({
    "oxid-linux-x86_64.tar.gz", "oxid-linux-aarch64.tar.gz",
    "oxid-macos-x86_64.tar.gz", "oxid-macos-aarch64.tar.gz",
    "oxid-windows-x86_64.zip",
})


def regular_file(path: Path) -> Path:
    if not path.is_file():
        raise ValueError(f"file not found: {path}")
    return path.resolve()


def verification_command(archive: Path, source_ref: str | None, commit: str | None,
                         bundle: Path | None, trusted_root: Path | None) -> list[str]:
    if archive.name not in ASSETS:
        raise ValueError("choose an Oxid compiler release archive")
    archive = regular_file(archive)
    if source_ref is None or re.fullmatch(r"refs/(?:heads|tags)/[^\x00-\x20\x7f]+", source_ref) is None:
        raise ValueError("supply --source-ref as a full tag or branch ref")
    if commit is None or re.fullmatch(r"[0-9a-fA-F]{40}", commit) is None:
        raise ValueError("supply --commit as the complete source commit SHA")
    bundle = regular_file(bundle if bundle is not None else archive.with_name(archive.name + ".sigstore.json"))
    command = [
        "gh", "attestation", "verify", str(archive),
        "--hostname", "github.com", "--repo", REPOSITORY,
        "--signer-workflow", WORKFLOW,
        "--source-ref", source_ref, "--source-digest", commit.lower(),
        "--cert-oidc-issuer", "https://token.actions.githubusercontent.com",
        "--predicate-type", "https://slsa.dev/provenance/v1",
        "--deny-self-hosted-runners", "--bundle", str(bundle), "--format", "json",
    ]
    if trusted_root is not None:
        command += ["--custom-trusted-root", str(regular_file(trusted_root))]
    return command


def digest_for(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_archive(archive: Path, source_ref: str | None, commit: str | None, bundle: Path | None,
                   trusted_root: Path | None, expected_digest: str | None = None) -> str:
    command = verification_command(archive, source_ref, commit, bundle, trusted_root)
    digest = digest_for(archive)
    if expected_digest is not None and digest != expected_digest:
        raise ValueError(f"archive changed after checking the checksum: {archive.name}")
    result = subprocess.run(command, check=False, timeout=300, capture_output=True, text=True)
    if result.stderr:
        print(result.stderr, file=sys.stderr, end="")
    if result.returncode != 0:
        raise ValueError(f"signature verification failed for {archive.name}")

    # gh verifies the digest. Also bind the approved platform archive name.
    records = json.loads(result.stdout)
    matching = False
    if isinstance(records, list):
        for record in records:
            if not isinstance(record, dict):
                continue
            verified = record.get("verificationResult")
            statement = verified.get("statement") if isinstance(verified, dict) else None
            if (not isinstance(statement, dict)
                    or statement.get("_type") != "https://in-toto.io/Statement/v1"
                    or statement.get("predicateType") != "https://slsa.dev/provenance/v1"):
                continue
            subjects = statement.get("subject")
            if isinstance(subjects, list):
                matching |= any(isinstance(subject, dict) and subject.get("name") == archive.name
                                and isinstance(subject.get("digest"), dict)
                                and subject["digest"].get("sha256") == digest for subject in subjects)
    if not matching:
        raise ValueError("verified provenance does not identify this archive name and SHA-256")
    if digest_for(archive) != digest:
        raise ValueError("archive changed during provenance verification")
    return result.stdout


def verify_distribution(directory: Path, source_ref: str | None, commit: str | None,
                        trusted_root: Path | None) -> None:
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError("release distribution must be an ordinary directory")
    expected = {name + suffix for name in ASSETS for suffix in ("", ".sha256", ".sigstore.json")}
    actual = {path.name for path in directory.iterdir()}
    if actual != expected:
        raise ValueError(f"invalid release distribution, missing {sorted(expected - actual)}, "
                         f"unexpected {sorted(actual - expected)}")
    for name in expected:
        path = directory / name
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"release file must be a regular file: {name}")
    inputs = {name: digest_for(directory / name) for name in expected}
    checksums = {}
    for name in sorted(ASSETS):
        archive = directory / name
        digest = inputs[name]
        checksum = archive.with_name(name + ".sha256")
        expected_line = f"{digest}  {name}\n".encode()
        with checksum.open("rb") as stream:
            original = stream.read(len(expected_line) + 2)
        if original not in (expected_line, expected_line.replace(b"\n", b"\r\n")):
            raise ValueError(f"invalid release checksum for {name}")
        checksums[name] = digest
    for name, digest in checksums.items():
        verify_archive(directory / name, source_ref, commit, None, trusted_root, digest)
    if {path.name for path in directory.iterdir()} != expected:
        raise ValueError("release distribution changed during verification")
    for name, digest in inputs.items():
        path = directory / name
        if path.is_symlink() or not path.is_file() or digest_for(path) != digest:
            raise ValueError(f"release file changed during verification: {name}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, nargs="?")
    parser.add_argument("--distribution", type=Path,
                        help="check the full set of downloaded release archives and sidecars")
    parser.add_argument("--source-ref", default=os.environ.get("GITHUB_REF"),
                        help="full tag/branch ref; defaults to GITHUB_REF in Actions")
    parser.add_argument("--commit", default=os.environ.get("GITHUB_SHA"),
                        help="complete source commit SHA; defaults to GITHUB_SHA in Actions")
    parser.add_argument("--bundle", type=Path,
                        help="signature bundle; defaults to ARCHIVE.sigstore.json")
    parser.add_argument("--trusted-root", type=Path,
                        help="independently obtained trusted root for offline verification")
    args = parser.parse_args(argv)
    if (args.archive is None) == (args.distribution is None):
        parser.error("choose one archive or --distribution DIRECTORY")
    if args.distribution is not None and args.bundle is not None:
        parser.error("--bundle applies to a single archive")
    try:
        if args.distribution is not None:
            verify_distribution(args.distribution, args.source_ref, args.commit, args.trusted_root)
            print(f"Verified {len(ASSETS)} release archives.")
        else:
            receipt = verify_archive(args.archive, args.source_ref, args.commit, args.bundle, args.trusted_root)
            print(receipt, end="")
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"release provenance verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
