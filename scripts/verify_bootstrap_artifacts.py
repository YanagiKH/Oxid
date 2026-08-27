#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit(
            "usage: verify_bootstrap_artifacts.py <artifact-directory> <expected-count>"
        )

    root = Path(sys.argv[1])
    expected_count = int(sys.argv[2])
    if expected_count < 2:
        raise SystemExit("expected artifact count must be at least 2")
    artifacts = sorted(root.rglob("compiler.oxb"))
    if len(artifacts) != expected_count:
        raise SystemExit(
            f"found {len(artifacts)} compiler artifacts under {root}, expected {expected_count}"
        )

    reference = artifacts[0].read_bytes()
    if not reference.startswith(b"OXBC"):
        raise SystemExit(f"compiler artifact has an invalid header: {artifacts[0]}")
    digest = hashlib.sha256(reference).hexdigest()
    mismatches: list[str] = []
    for artifact in artifacts[1:]:
        contents = artifact.read_bytes()
        if contents != reference:
            mismatches.append(f"{artifact} ({hashlib.sha256(contents).hexdigest()})")
    if mismatches:
        details = "\n  ".join(mismatches)
        raise SystemExit(
            f"compiler artifacts differ from {artifacts[0]} ({digest}):\n  {details}"
        )

    print(
        f"verified {len(artifacts)} byte-identical compiler artifacts "
        f"({len(reference)} bytes, sha256 {digest})"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as error:
        raise SystemExit(f"bootstrap artifact validation failed: {error}") from error
