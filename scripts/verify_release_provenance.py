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


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--source-ref", default=os.environ.get("GITHUB_REF"),
                        help="full tag/branch ref; defaults to GITHUB_REF in Actions")
    parser.add_argument("--commit", default=os.environ.get("GITHUB_SHA"),
                        help="complete source commit SHA; defaults to GITHUB_SHA in Actions")
    parser.add_argument("--bundle", type=Path,
                        help="signature bundle; defaults to ARCHIVE.sigstore.json")
    parser.add_argument("--trusted-root", type=Path,
                        help="independently obtained trusted root for offline verification")
    args = parser.parse_args(argv)
    try:
        command = verification_command(args.archive, args.source_ref, args.commit,
                                       args.bundle, args.trusted_root)
        with args.archive.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        result = subprocess.run(command, check=False, timeout=300, capture_output=True, text=True)
        if result.stderr:
            print(result.stderr, file=sys.stderr, end="")
        if result.returncode != 0:
            return 1
        # Inspect only statements returned by the successful cryptographic
        # verifier. With --bundle, gh checks the digest but permits renaming;
        # release delivery must also bind the signed platform archive name.
        records = json.loads(result.stdout)
        matching = False
        if isinstance(records, list):
            for record in records:
                if not isinstance(record, dict):
                    continue
                verified = record.get("verificationResult")
                statement = verified.get("statement") if isinstance(verified, dict) else None
                if not isinstance(statement, dict) or statement.get("_type") != "https://in-toto.io/Statement/v1" or statement.get("predicateType") != "https://slsa.dev/provenance/v1":
                    continue
                subjects = statement.get("subject")
                if isinstance(subjects, list):
                    matching |= any(isinstance(subject, dict) and subject.get("name") == args.archive.name
                                    and isinstance(subject.get("digest"), dict)
                                    and subject["digest"].get("sha256") == digest for subject in subjects)
        if not matching:
            raise ValueError("verified provenance does not identify this archive name and SHA-256")
        with args.archive.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != digest:
                raise ValueError("archive changed during provenance verification")
        print(result.stdout, end="")
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"release provenance verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
