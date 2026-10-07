#!/usr/bin/env python3
"""Qualify the bounded scalar parser against unchanged canonical observers.

The fixed source matrix compares complete ASTs or the first diagnostic only.
Grammar-site refusals and two out-of-domain transport probes are separate;
this is syntax evidence, not a provider, self-hosting or filesystem sandbox claim.
"""
import argparse
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

import build_typed_lexer_observer as lexer_builder
import build_typed_parser_observer as parser_builder
import verify_repo
from verify_bounded_typed_lexer import digest, run


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/bounded_typed_parser"
ROSTER_SHA256 = "cdd0e7b91f289f3616c6483d3e49c0bd5c4d9ebede3aa12daa8e7cb92ff0476c"
CONTROLS_SHA256 = "e5a9d13de0ff71d7dd2a261ddbe1c5709bd8274b805273a89c71a62879c91979"
COUNTS = {"comparison": 118, "refusal": 21}
MEMBERS = tuple(name + ".ox" for name in (
    "admission buffers keywords lexer lexer_core main parser_admission parser_atom "
    "parser_banks parser_call parser_control parser_driver parser_expression parser_main "
    "parser_output parser_probe_output parser_signature parser_state parser_statement "
    "tape transcript").split())
HISTORICAL_ROOTS = ("main.ox", "admission.ox", "parser_admission.ox", "parser_main.ox")
PARSER_MEMBERS = tuple(name + ".ox" for name in (
    "parser_main buffers lexer_core keywords parser_state parser_signature parser_atom "
    "parser_call parser_expression parser_statement parser_control parser_driver parser_output").split())


def require(condition, message):
    if not condition:
        raise ValueError(message)


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_cases(path):
    manifest = json.loads(path.read_bytes())
    require(set(manifest) == {"schema_version", "scope", "cases", "transport"}
            and manifest["schema_version"] == 1, "invalid case manifest")
    cases, transport = manifest["cases"], manifest["transport"]
    require(Counter(case["role"] for case in cases) == COUNTS,
            "exact nonzero 118 comparison / 21 refusal selection required")
    require(len({case["name"] for case in cases}) == 139, "duplicate or missing case")
    for case in cases:
        data = bytes.fromhex(case["input_hex"])
        require(len(data) <= 128 and data.isascii(), "source outside bounded ASCII matrix")
        statuses = ("ok", "diagnostic", "lexical_diagnostic")
        if case["role"] == "refusal":
            statuses += ("outside_subset",)
        require(case["parser_status"] in statuses, "invalid canonical status")
        require(("refusal" in case) == (case["role"] == "refusal"), "invalid refusal expectation")
    require([(case["name"], bytes.fromhex(case["input_hex"])) for case in transport]
            == [("ascii129", b" " * 129), ("non_ascii_byte", b"\xff")],
            "exact two transport probes required")
    identity = hashlib.sha256(json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    require(identity == ROSTER_SHA256, "fixed independent case roster changed")
    return cases, transport


def observer_identity(binary, builder, kind):
    """Bind supplied executables to the existing builders' unchanged source copies."""
    directory = binary.parent
    manifest = json.loads((directory / "source-manifest.json").read_bytes())
    receipt = json.loads((directory / "build-evidence.json").read_bytes())
    require(receipt["exit_code"] == 0 and receipt["binary"] == binary.name
            and receipt["binary_sha256"] == digest(binary), "observer build/binary mismatch")
    require(manifest["source_patches"] == [], "canonical observer has source patches")
    sources = manifest["files"]
    wrappers = manifest["wrappers"]
    require([entry["original_path"] for entry in sources]
            == ["src/frontend/" + name for name in builder.SOURCE_FILES],
            "canonical source selection changed")
    require([entry["original_path"] for entry in wrappers]
            == ["tests/fixtures/bounded_typed_" + kind + "/observer/" + name
                for name, _ in builder.WRAPPER_FILES], "observer wrapper selection changed")
    for entry in sources + wrappers:
        require(digest(directory / entry["copied_path"]) == entry["sha256"]
                == digest(ROOT / entry["original_path"]), "observer source identity mismatch")
    return {"binary_sha256": digest(binary), "source_manifest_sha256": digest(directory / "source-manifest.json"),
            "build_evidence_sha256": digest(directory / "build-evidence.json"), "source_commit": manifest["commit"]}


def inventory(directory):
    return {path.relative_to(directory).as_posix(): digest(path)
            for path in sorted(directory.rglob("*")) if path.is_file()}


def fixture_modules(path):
    """Read only this fixture's flat declarations; never admit discovered modules."""
    text = path.read_text(encoding="utf-8")
    # Comments may separate declaration tokens. Ignore strings and comments so
    # their contents cannot manufacture or conceal a declaration in this check.
    text = re.sub(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"', " ", text, flags=re.DOTALL)
    tokens = re.findall(r"[A-Za-z_][A-Za-z_0-9]*|[^\s]", text)
    modules = []
    for index, token in enumerate(tokens):
        if token == "mod":
            require(index + 2 < len(tokens)
                    and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", tokens[index + 1])
                    and tokens[index + 2] == ";", "unexpected fixture module declaration")
            modules.append(tokens[index + 1] + ".ox")
    return modules


def copy_parser_sources(fixture_source, sources):
    """Preserve the historical snapshot while excluding unrelated static siblings."""
    prefix = "fixtures/typed-lexer-samples/"
    registered = set()
    for root in HISTORICAL_ROOTS:
        members = verify_repo.TYPED_PROJECTS.get(prefix + root, ())
        require(members and len(members) == len(set(members)), "historical fixture registration changed")
        registered.update(members)
    require(registered == {prefix + name for name in MEMBERS}, "historical fixture registration changed")
    require(verify_repo.TYPED_PROJECTS[prefix + "parser_main.ox"]
            == tuple(prefix + name for name in PARSER_MEMBERS), "parser root registration changed")
    for name in MEMBERS:
        path = fixture_source / name
        require(path.is_file() and not path.is_symlink(), "historical fixture source missing or not regular: " + name)
    modules = fixture_modules(fixture_source / "parser_main.ox")
    require(len(modules) == len(set(modules)) and set(modules) == set(PARSER_MEMBERS[1:]),
            "parser root module closure changed")
    for name in PARSER_MEMBERS[1:]:
        require(not fixture_modules(fixture_source / name), "unexpected parser child module declaration: " + name)
    for name in MEMBERS:
        shutil.copyfile(fixture_source / name, sources / name)


def check_execution(result, remaining, data, transport=False):
    require(result.returncode == (64 if transport else 0), "unexpected process exit status")
    require(not result.stderr, "unexpected process stderr")
    require(remaining == data[129:], "unexpected stdin consumption")
    if transport:
        require(not result.stdout, "transport refusal emitted stdout")


def completed(report, cases, modes):
    expected = {(case["name"], mode, case["role"]) for case in cases for mode in modes}
    checks = report["checks"]
    require(len(checks) == len(expected)
            and {(c["case"], c["mode"], c["role"]) for c in checks} == expected
            and all(c["passed"] for c in checks), "incomplete or duplicate bounded results")
    transport = report["transport"]
    require(len(transport) == 2 * len(modes)
            and {(c["case"], c["mode"]) for c in transport}
            == {(name, mode) for name in ("ascii129", "non_ascii_byte") for mode in modes}
            and all(c["passed"] for c in transport), "incomplete transport results")
    for mode in modes:
        controls = [c for c in report["controls"] if c["mode"] == mode]
        require(len(controls) == len({c["case"] for c in controls}) == 41
                and all(c["passed"] for c in controls), "incomplete 39+2 corruption controls")
    require(len(report["controls"]) == 41 * len(modes), "unexpected extra corruption controls")
    report["counts"] = {mode: {**dict(Counter(c["role"] for c in checks if c["mode"] == mode)),
                              "pending": 0, "transport": 2, "corruption_controls": 41} for mode in modes}


def qualify(compiler, parser_observer, lexer_observer, output, native=False):
    require(not sys.flags.optimize, "run without Python optimization: retained corruption assertions are required")
    cases, transport = load_cases(FIXTURES / "cases.json")
    require(digest(FIXTURES / "corruption_controls.py") == CONTROLS_SHA256,
            "39+2 corruption-control lineage changed")
    output.mkdir(parents=True, exist_ok=False)
    report = {"passed": False, "native": native, "checks": [], "controls": [], "transport": [],
              "scope": "128-byte ASCII scalar syntax; full AST / first diagnostic; no provider or self-hosting claim",
              "execution_scope": "One ELF, empty cwd, cleared environment, copied sources renamed before native runs; no filesystem sandbox claim",
              "compiler_sha256": digest(compiler), "controller_sha256": digest(Path(__file__)),
              "case_roster_sha256": ROSTER_SHA256, "corruption_controls_sha256": CONTROLS_SHA256}
    save(output / "report.json", report)
    try:
        report["parser_observer"] = observer_identity(parser_observer, parser_builder, "parser")
        report["lexer_observer"] = observer_identity(lexer_observer, lexer_builder, "lexer")
        empty, sources = output / "source-free", output / "source"
        empty.mkdir()
        sources.mkdir()
        fixture_source = ROOT / "fixtures/typed-lexer-samples"
        copy_parser_sources(fixture_source, sources)
        shutil.copyfile(ROOT / "scripts/parser_ast_observation.py", sources / "parser_ast_observation.py")
        shutil.copyfile(FIXTURES / "corruption_controls.py", sources / "corruption_controls.py")
        shutil.copyfile(FIXTURES / "cases.json", output / "cases.json")
        # Prevent imports from adding pyc files to the immutable source snapshot.
        sys.dont_write_bytecode = True
        projection = load_module("bounded_parser_projection", sources / "parser_ast_observation.py")
        controls = load_module("bounded_parser_corruptions", sources / "corruption_controls.py")
        report["source_sha256"] = inventory(sources)
        entry, binary = sources / "parser_main.ox", output / "parser"
        modes = ["reference"] + (["native"] if native else [])
        commands = {"reference": [compiler, "run", entry, "--edition", "typed-preview", "--entry-mode", "process"],
                    "native": [binary]}

        def execute(directory, command, data, mode=None):
            require(inventory(sources) == report["source_sha256"], "candidate snapshot changed")
            require(digest(compiler) == report["compiler_sha256"], "compiler changed")
            hidden = output / "retained-source"
            if mode == "native":
                require(digest(binary) == report["binary_sha256"], "native executable changed")
                require(not any(empty.iterdir()), "native cwd is not empty")
                sources.rename(hidden)
            try:
                try:
                    result, remaining = run(directory, command, data, empty, mode != "reference")
                except subprocess.TimeoutExpired as error:
                    (directory / "stdout").write_bytes(error.stdout or b"")
                    (directory / "stderr").write_bytes(error.stderr or b"")
                    save(directory / "receipt.json", {
                        "command": [str(part) for part in command], "status": "timeout",
                        "input_sha256": digest(directory / "input.bin"),
                        "stdout_sha256": digest(directory / "stdout"),
                        "stderr_sha256": digest(directory / "stderr"),
                        "cleared_environment": mode != "reference", "cwd": str(empty)})
                    raise
            finally:
                if mode == "native":
                    hidden.rename(sources)
            receipt_path = directory / "receipt.json"
            receipt = json.loads(receipt_path.read_bytes())
            receipt.update(stdout_sha256=digest(directory / "stdout"), stderr_sha256=digest(directory / "stderr"))
            save(receipt_path, receipt)
            return result, remaining

        setup = [("check", [compiler, "check", entry, "--edition", "typed-preview"])]
        if native:
            setup.append(("compile", [compiler, "compile", entry, "--edition", "typed-preview",
                                       "--backend", "llvm", "--entry-mode", "process", "--output", binary]))
        for name, command in setup:
            # Compiler setup keeps its toolchain environment and must not read stdin.
            result, remaining = execute(output / name, command, b"unread setup input", "reference")
            require(result.returncode == 0 and not result.stderr and remaining == b"unread setup input",
                    "parser setup failed: " + name)
        if native:
            require(binary.read_bytes()[:4] == b"\x7fELF", "expected ELF")
            report["binary_sha256"] = digest(binary)
        save(output / "report.json", report)

        # Collect all independent expectations before the first candidate run.
        expectations = {}
        canonical = output / "canonical"
        canonical.mkdir()
        for case in cases:
            data = bytes.fromhex(case["input_hex"])
            directory = canonical / case["name"]
            directory.mkdir()
            facts = {}
            for kind, observer in (("parser", parser_observer), ("lexer", lexer_observer)):
                result, remaining = execute(directory / kind, [observer], data)
                check_execution(result, remaining, data)
                facts[kind] = json.loads(result.stdout)
            require(facts["parser"]["status"] == case["parser_status"], "canonical status changed: " + case["name"])
            expected = facts["parser"]
            if case["role"] == "refusal":
                refusal = case["refusal"]
                expected = {"status": "domain_refusal", "family": refusal["family"],
                            "span": projection.span(refusal["start"], refusal["end"])}
            expectations[case["name"]] = (expected, facts["lexer"])
        for mode in modes:
            mode_root = output / mode
            (mode_root / "cases").mkdir(parents=True)
            for case in cases:
                data = bytes.fromhex(case["input_hex"])
                directory = mode_root / "cases" / case["name"]
                result, remaining = execute(directory, commands[mode], data, mode)
                check_execution(result, remaining, data)
                expected, lexical = expectations[case["name"]]
                got = projection.decode(result.stdout, data, lexical.get("tokens"))
                save(directory / "projected.json", got)
                # Exact retained filenames consumed by the unchanged 39+2 controls.
                (directory / "candidate.stdout").write_bytes(result.stdout)
                save(directory / "lexer.stdout", lexical)
                require(got == expected, "full canonical/refusal mismatch: " + mode + "/" + case["name"])
                report["checks"].append({"case": case["name"], "mode": mode, "role": case["role"],
                                         "status": got["status"], "passed": True})
                save(output / "report.json", report)
            report["controls"].extend({**control, "mode": mode} for control in controls.run(projection, mode_root))
            (mode_root / "transport").mkdir()
            for case in transport:
                data = bytes.fromhex(case["input_hex"])
                result, remaining = execute(mode_root / "transport" / case["name"], commands[mode], data, mode)
                check_execution(result, remaining, data, transport=True)
                report["transport"].append({"case": case["name"], "mode": mode, "passed": True})
                save(output / "report.json", report)
        require(inventory(sources) == report["source_sha256"], "final candidate snapshot changed")
        require(observer_identity(parser_observer, parser_builder, "parser") == report["parser_observer"]
                and observer_identity(lexer_observer, lexer_builder, "lexer") == report["lexer_observer"],
                "canonical observers changed")
        require(digest(compiler) == report["compiler_sha256"], "final compiler changed")
        if native:
            require(digest(binary) == report["binary_sha256"], "final native executable changed")
        completed(report, cases, modes)
        report["passed"] = True
    except Exception as error:
        report["error"] = type(error).__name__ + ": " + str(error)
        raise
    finally:
        save(output / "report.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oxid", type=Path, required=True)
    parser.add_argument("--parser-observer", type=Path, required=True)
    parser.add_argument("--lexer-observer", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    try:
        report = qualify(args.oxid.resolve(strict=True), args.parser_observer.resolve(strict=True),
                         args.lexer_observer.resolve(strict=True), args.output.resolve(), args.native)
    except (OSError, ValueError, AssertionError, subprocess.SubprocessError) as error:
        print("bounded typed parser qualification failed: " + str(error), file=sys.stderr)
        return 1
    print(json.dumps({"passed": report["passed"], "counts": report["counts"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
