#!/usr/bin/env python3
"""RFC0031 compile-time source authority probes, separate from frozen rosters.

Use the caller's CARGO_TARGET_DIR and a serial build lease. The temporary source
checkout is private; no compiler witness, raw execution API, or token is exported.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile

from verify_owned_witness_privacy import materialize_checkout

PROBES = (
    ("trusted-typed-lowering", True, None, """
fn check(typed: &typeck::TypedOwnedProgram<'_>) {
    let _ = association::lower_and_associate(typed);
}
"""),
    ("raw-cannot-replace-typed-owner", False, "E0308", """
fn check(raw: &super::super::RawOwnedProgram) {
    let _ = association::lower_and_associate(raw);
}
"""),
    ("raw-association-absent-in-production", False, "E0425", """
fn check(raw: super::super::RawOwnedProgram, typed: &typeck::TypedOwnedProgram<'_>) {
    let _ = association::associate(raw, typed);
}
"""),
    ("raw-helper-absent-in-production", False, "E0425", """
fn check(raw: super::super::RawOwnedProgram, typed: &typeck::TypedOwnedProgram<'_>) {
    let _ = association::associate_lowered(raw, typed);
}
"""),
    ("associated-owner-cannot-be-forged", False, "E0451", """
fn check(raw: super::super::RawOwnedProgram, sources: &super::super::SourceMap) {
    let _ = association::AssociatedOwned { raw, sources };
}
"""),
)


def main():
    root = Path(__file__).resolve().parent.parent
    if not os.environ.get("CARGO_TARGET_DIR"):
        raise SystemExit("Set coordinated CARGO_TARGET_DIR; no duplicate build target is created")
    reports = []
    with tempfile.TemporaryDirectory(prefix="oxid-byte-source-privacy-") as directory:
        checkout = Path(directory) / "checkout"
        materialize_checkout(root, checkout)
        module = checkout / "src/frontend/oir/owned/source/mod.rs"
        original = module.read_text()
        for name, succeeds, code, body in PROBES:
            module.write_text(original + "\nmod byte_storage_privacy_probe {\nuse super::*;\n" + body + "\n}\n")
            result = subprocess.run(
                ["cargo", "check", "--locked", "--bin", "oxid", "--message-format=json"],
                cwd=checkout, text=True, capture_output=True, timeout=180,
            )
            errors = []
            for line in result.stdout.splitlines():
                item = json.loads(line)
                if item.get("reason") == "compiler-message" and item["message"]["level"] == "error":
                    errors.append(item["message"])
            actual = [e["code"]["code"] for e in errors if e.get("code")]
            if succeeds:
                assert result.returncode == 0, (name, result.stdout, result.stderr)
            else:
                assert result.returncode != 0 and code in actual, (name, actual, result.stdout, result.stderr)
                assert not any(c in actual for c in ["E0412", "E0422", "E0432", "E0433"]), (name, actual)
            reports.append({"probe": name, "expected_success": succeeds, "exit_code": result.returncode, "error_codes": actual})
    print(json.dumps({"byte_storage_source_privacy": "passed", "probes": reports}, indent=2))


if __name__ == "__main__":
    main()
