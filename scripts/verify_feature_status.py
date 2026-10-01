#!/usr/bin/env python3
"""Validate the evidence inventory without interpreting it as certification."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STATUSES = {"proposed", "experimental", "implemented", "validated", "stable"}
CLASSIFICATIONS = {"production-path", "demo", "synthetic", "planned"}


def verify_feature_status(root: Path = ROOT) -> None:
    document = json.loads((root / "docs/feature-status.json").read_text(encoding="utf-8"))
    if document.get("schema_version") != 1:
        raise ValueError("feature status schema_version must be 1")
    if not re.fullmatch(r"[0-9a-f]{40}", document.get("baseline_commit", "")):
        raise ValueError("feature status needs a full baseline commit")
    if not isinstance(document.get("scope"), str) or not document["scope"].strip():
        raise ValueError("feature status must describe its inventory scope")
    features = document.get("features")
    if not isinstance(features, list) or not features:
        raise ValueError("feature status must contain a nonempty features list")
    seen = set()
    for feature in features:
        identifier = feature.get("id", "")
        if not re.fullmatch(r"[a-z][a-z0-9-]*", identifier) or identifier in seen:
            raise ValueError(f"invalid or duplicate feature id: {identifier}")
        seen.add(identifier)
        status = feature.get("status")
        if status not in STATUSES:
            raise ValueError(f"{identifier}: unknown status {status}")
        if feature.get("classification") not in CLASSIFICATIONS:
            raise ValueError(f"{identifier}: unknown classification")
        for field in ("summary", "owner", "reviewer", "edition", "backend", "target_scope", "limitations"):
            if not isinstance(feature.get(field), str) or not feature[field].strip():
                raise ValueError(f"{identifier}: {field} must be nonempty text")
        for field in ("spec", "implementation", "tests", "evidence"):
            paths = feature.get(field)
            if not isinstance(paths, list):
                raise ValueError(f"{identifier}: {field} must be a list")
            for relative in paths:
                if not isinstance(relative, str) or Path(relative).is_absolute():
                    raise ValueError(f"{identifier}: invalid repository path in {field}")
                path = (root / relative).resolve()
                if not path.is_relative_to(root.resolve()) or not path.is_file():
                    raise ValueError(f"{identifier}: missing or escaping {field} path: {relative}")
        if not feature["spec"]:
            raise ValueError(f"{identifier}: a scope/specification reference is required")
        if status in {"implemented", "validated", "stable"}:
            if feature["classification"] != "production-path":
                raise ValueError(f"{identifier}: implemented claims require a production path")
            if not feature["implementation"] or not feature["tests"]:
                raise ValueError(f"{identifier}: implemented claims need code and tests")
        if status in {"validated", "stable"}:
            if feature["owner"] == "unassigned" or feature["reviewer"] == "unassigned" or not feature["evidence"]:
                raise ValueError(f"{identifier}: validated claims need an owner, reviewer, and evidence")
        if status == "stable":
            for field in ("compatibility_policy", "review"):
                if not isinstance(feature.get(field), str) or not feature[field].strip():
                    raise ValueError(f"{identifier}: stable claims need {field}")
    print(f"feature status inventory valid: {len(features)} entries (not a certification)")


if __name__ == "__main__":
    try:
        verify_feature_status()
    except (OSError, ValueError, TypeError, AttributeError) as error:
        print(f"feature status error: {error}", file=sys.stderr)
        raise SystemExit(1)
