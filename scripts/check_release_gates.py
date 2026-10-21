#!/usr/bin/env python3
"""Report technical v1 readiness and prevent an unqualified stable release.

Without --release-tag, validate/report the inventory for ordinary development CI.
With --release-tag, allow 0.x releases and explicit SemVer prereleases, but reject
stable v1 while complete technical acceptance collectors are unimplemented.
Every release tag must match Cargo.toml package.version, or --package-version
when the workflow supplies the version explicitly.
The inventory is deliberately not an executable certification format: changing
status flags or adding evidence filenames cannot make a stable release pass.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INVENTORY = ROOT / "docs/release-gates-v1.json"
MANIFEST = ROOT / "Cargo.toml"
GATE_IDS = tuple(f"G{number}" for number in range(1, 11))
UNMET_STATUSES = {"unmet", "in-progress"}
TAG_PATTERN = re.compile(
    r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)


def nonempty_text(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def load_inventory(path: Path = INVENTORY) -> list[dict]:
    document = json.loads(path.read_text(encoding="utf-8"))
    fields = {"schema_version", "specification", "scope", "gates"}
    if not isinstance(document, dict) or set(document) != fields:
        raise ValueError("release inventory has missing or unknown fields")
    if type(document["schema_version"]) is not int or document["schema_version"] != 1:
        raise ValueError("release inventory schema_version must be 1")
    if document["specification"] != "docs/ROADMAP_V2.md":
        raise ValueError("release inventory must reference docs/ROADMAP_V2.md")
    if not nonempty_text(document["scope"]):
        raise ValueError("release inventory must describe its scope")
    gates = document["gates"]
    if not isinstance(gates, list) or len(gates) != len(GATE_IDS):
        raise ValueError("release inventory must contain exactly G1 through G10")
    identifiers = []
    for gate in gates:
        if not isinstance(gate, dict) or set(gate) != {
            "id", "title", "status", "required", "remaining"
        }:
            raise ValueError("release gate has missing or unknown fields")
        if not isinstance(gate["id"], str) or gate["id"] not in GATE_IDS:
            raise ValueError("release inventory contains an unknown gate")
        identifiers.append(gate["id"])
        if not nonempty_text(gate["title"]):
            raise ValueError(f"{gate['id']}: title must be nonempty text")
        if not isinstance(gate["status"], str) or gate["status"] not in UNMET_STATUSES:
            raise ValueError(
                f"{gate['id']}: only unmet/in-progress can be recorded; "
                "complete technical acceptance collectors are not implemented"
            )
        for field in ("required", "remaining"):
            values = gate[field]
            if not isinstance(values, list) or not values or not all(
                nonempty_text(value) for value in values
            ):
                raise ValueError(f"{gate['id']}: {field} must be a nonempty text list")
    if set(identifiers) != set(GATE_IDS):
        raise ValueError("release inventory must contain each of G1 through G10 once")
    return sorted(gates, key=lambda gate: GATE_IDS.index(gate["id"]))


def load_package_version(path: Path = MANIFEST) -> str:
    document = tomllib.loads(path.read_text(encoding="utf-8"))
    package = document.get("package")
    if not isinstance(package, dict) or not nonempty_text(package.get("version")):
        raise ValueError("Cargo.toml must contain a package.version string")
    return package["version"]


def release_decision(tag: str, package_version: str) -> tuple[bool, str]:
    match = TAG_PATTERN.fullmatch(tag)
    if not match:
        raise ValueError("release tag must be a full v-prefixed SemVer, such as v1.0.0-rc.1")
    if tag[1:] != package_version:
        raise ValueError(
            f"release tag {tag} does not match package.version {package_version!r}"
        )
    major, _, _, prerelease, _ = match.groups()
    if prerelease:
        if any(part.isdigit() and len(part) > 1 and part.startswith("0")
               for part in prerelease.split(".")):
            raise ValueError("numeric prerelease identifiers must not have leading zeroes")
        return True, (
            f"{tag} is a prerelease for qualification; publish it as a prerelease. "
            "This does not establish stable v1 readiness."
        )
    if major == "0":
        return True, f"{tag} is a pre-v1 release; v1 technical gates do not block it."
    if major != "1":
        return False, f"{tag} is outside this checker's v1 scope; no stable release qualification exists."
    # No complete G1-G10 acceptance collectors exist yet. This explicit refusal
    # must be replaced by real technical acceptance, not inventory status flags.
    return False, (
        f"{tag} is blocked: complete G1-G10 technical acceptance collectors "
        "and passing results are not implemented."
    )


def main(
    argv: list[str] | None = None, *, inventory: Path = INVENTORY,
    manifest: Path = MANIFEST,
) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-tag", help="enforce readiness for this v-prefixed SemVer tag")
    parser.add_argument("--package-version", help="package version supplied by the release workflow")
    args = parser.parse_args(argv)
    if args.package_version is not None and args.release_tag is None:
        parser.error("--package-version requires --release-tag")
    try:
        gates = load_inventory(inventory)
        decision = None
        if args.release_tag is not None:
            version = args.package_version
            if version is None:
                version = load_package_version(manifest)
            decision = release_decision(args.release_tag, version)
    except (OSError, ValueError) as error:
        print(f"release gate error: {error}", file=sys.stderr)
        return 1
    print("v1 technical readiness: NOT READY (inventory is not acceptance evidence)")
    for gate in gates:
        print(f"{gate['id']} {gate['status']}: {gate['title']}")
        for gap in gate["remaining"]:
            print(f"  {gap}")
    if decision is not None:
        allowed, message = decision
        print(message, file=sys.stdout if allowed else sys.stderr)
        return 0 if allowed else 1
    print("Inventory valid. Stable v1 publication remains blocked.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
