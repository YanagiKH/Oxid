#!/usr/bin/env python3
"""Compile actual sibling consumers against the private owned witness boundary.

The positive sibling can inspect immutable accessors. Negative siblings must fail
for private fields or mutability, not because a marker/type/import is missing.
A temporary source checkout and target directory leave the candidate untouched.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


PROBES = [
    ("immutable-access", True, (), """
        fn inspect(w: &VerifiedOwnedProgram) {
            let _ = (w.functions(), w.declarations(), w.usage());
        }
    """),
    ("replace-program-using-struct-update", False, ("E0451",), """
        fn forge(base: VerifiedOwnedProgram, replacement: RawOwnedProgram) -> VerifiedOwnedProgram {
            VerifiedOwnedProgram { program: replacement, ..base }
        }
    """),
    ("mutate-private-program-field", False, ("E0616",), """
        fn mutate(w: &mut VerifiedOwnedProgram, replacement: RawOwnedProgram) {
            w.program = replacement;
        }
    """),
    ("mutate-through-immutable-accessor", False, ("E0596", "E0594"), """
        fn mutate(w: &mut VerifiedOwnedProgram) {
            w.functions()[0].blocks.clear();
        }
    """),
]


def main():
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix="oxid-owned-privacy-") as directory:
        checkout = Path(directory) / "checkout"
        checkout.mkdir()
        for name in ["Cargo.toml", "Cargo.lock", "build.rs"]:
            shutil.copy2(root / name, checkout / name)
        for name in ["src", "native", "compiler", "stdlib"]:
            shutil.copytree(root / name, checkout / name)
        module = checkout / "src/frontend/oir/owned/mod.rs"
        original = module.read_text()
        env = dict(os.environ, CARGO_TARGET_DIR=str(Path(directory) / "target"))
        reports = []
        for name, succeeds, codes, body in PROBES:
            module.write_text(original + "\nmod consumer_privacy_probe {\n"
                              "use super::*;\n"
                              "use super::verified::VerifiedOwnedProgram;\n"
                              + body + "\n}\n")
            command = ["cargo", "check", "--locked", "--bin", "oxid", "--message-format=json"]
            result = subprocess.run(command, cwd=checkout, env=env,
                                    text=True, capture_output=True, timeout=180)
            errors = []
            for line in result.stdout.splitlines():
                item = json.loads(line)
                if item.get("reason") == "compiler-message":
                    message = item["message"]
                    if message["level"] == "error":
                        errors.append(message)
            actual_codes = [e["code"]["code"] for e in errors if e.get("code")]
            if succeeds:
                assert result.returncode == 0, (name, result.stdout, result.stderr)
            else:
                assert result.returncode != 0 and any(code in actual_codes for code in codes), (
                    name, result.returncode, actual_codes, result.stdout, result.stderr)
                assert not any(code in actual_codes for code in ["E0412", "E0422", "E0432", "E0433"]), (
                    "probe failed on a missing import/type instead of witness privacy", name, actual_codes)
            reports.append({"probe": name, "expected_success": succeeds,
                            "exit_code": result.returncode, "error_codes": actual_codes})
        print(json.dumps({"owned_witness_privacy": "passed", "probes": reports}, indent=2))


if __name__ == "__main__":
    main()
