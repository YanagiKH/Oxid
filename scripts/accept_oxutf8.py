"""Focused ordinary-source oxutf8 acceptance; artifacts belong outside checkout.

Example: python3 scripts/accept_oxutf8.py target/debug/oxid --engine interpreter
         --evidence /tmp/oxutf8-reference
Native mode builds and packages, then runs only the deployed binary through the
segmented-stdin cli_driver (cwd=/, empty env). LLVM must already be installed;
this script never downloads or installs tools. Does not expand the driver.
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DRIVER = Path(__file__).resolve().parent / "cli_driver.py"

CORPUS = [
    ("valid-empty", b"", 0),
    ("valid-ascii", b"Oxid oxutf8: plain ASCII copy.\n", 0),
    ("valid-nul", b"\x00A\x00\x00B\n\x00", 0),
    (
        "valid-multibyte",
        b"A\xc2\x80\xdf\xbf\xe0\xa0\x80\xed\x9f\xbf\xee\x80\x80\xef\xbf\xbf"
        b"\xf0\x90\x80\x80\xf4\x8f\xbf\xbf\n",
        0,
    ),
    ("valid-combining", b"e\xcc\x81 a\xcc\x88\xcc\xa3\n", 0),
    ("valid-bom", b"\xef\xbb\xbfBOM first\n", 0),
    ("valid-cross-1024", b"a" * 1022 + b"\xf0\x9f\x98\x80\n", 0),
    ("valid-8199", ("A\u00a2\u20ac\U0001f600".encode() * 819) + "\u00a2\u20ac\U0001f600".encode(), 0),
    ("invalid-overlong", b"A\xc0\xaf\n", 65),
    ("invalid-surrogate", b"A\xed\xa0\x80\n", 65),
    ("invalid-above-10ffff", b"A\xf4\x90\x80\x80\n", 65),
    ("invalid-lone-continuation", b"A\x80\n", 65),
    ("invalid-truncated", b"A\xe2\x82", 65),
    ("invalid-late-truncated-after-1024", b"a" * 1500 + b"\xf0\x9f\x98", 65),
]

CONTRACT_TESTS = (
    b"PASS test_append_keeps_text_and_destination_prefix\n"
    b"PASS test_append_twice_is_exact\n"
    b"PASS test_empty_text_appends_nothing\n"
    b"PASS test_valid_classes_round_trip\n"
    b"PASS test_invalid_classes_return_original_owner\n"
    b"test result: 5 passed; 0 failed; 5 total\n"
)
CONTRACT_RESULT = b"42\n"
SOURCE_PATHS = [
    "examples/projects/oxutf8/oxid.toml",
    "examples/projects/oxutf8/app/oxid.toml",
    "examples/projects/oxutf8/app/src/main.ox",
    "examples/projects/oxutf8/contracts/oxid.toml",
    "examples/projects/oxutf8/contracts/src/main.ox",
    "packages/utf8_text/oxid.toml",
    "packages/utf8_text/src/lib.ox",
]


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def materialize_fixture(destination: Path) -> Path:
    for path in SOURCE_PATHS:
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / path, target)
    return destination / "examples/projects/oxutf8"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("compiler", type=Path)
    parser.add_argument("--engine", choices=["interpreter", "native"], required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    assert DRIVER.is_file(), DRIVER

    fixture_root = Path(tempfile.mkdtemp(prefix="oxid-oxutf8-fixture-"))
    try:
        project = materialize_fixture(fixture_root)
        cache = evidence / "cache"
        cache.mkdir()
        llvm = Path(os.environ.get("OXID_LLVM_BIN", str(fixture_root / "absent-llvm")))
        env = os.environ.copy()
        env["OXID_CACHE_DIR"] = str(cache)
        env["OXID_LLVM_BIN"] = str(llvm)
        env["PATH"] = str(fixture_root / "absent-path")
        env.pop("OXID_PATH", None)

        receipt = {
            "engine": args.engine,
            "compiler_sha256": sha(compiler.read_bytes()),
            "commands": [],
            "corpus": [],
        }

        def invoke(label, command, cwd, data=b"", extra_env=None):
            run_env = env if extra_env is None else extra_env
            result = subprocess.run(
                [str(part) for part in command],
                cwd=cwd,
                input=data,
                capture_output=True,
                env=run_env,
            )
            (evidence / (label + ".stdout")).write_bytes(result.stdout)
            (evidence / (label + ".stderr")).write_bytes(result.stderr)
            row = {
                "label": label,
                "command": [str(part) for part in command],
                "cwd": str(cwd),
                "exit": result.returncode,
                "input_bytes": len(data),
                "input_sha256": sha(data),
                "stdout_bytes": len(result.stdout),
                "stdout_sha256": sha(result.stdout),
                "stderr_sha256": sha(result.stderr),
            }
            receipt["commands"].append(row)
            (evidence / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
            assert result.returncode == 0, row
            assert result.stderr == b"", (row, result.stderr)
            return result.stdout

        manifest = project / "oxid.toml"
        base = [compiler, "--manifest-path", manifest]

        for package in ["oxutf8", "oxutf8-contracts"]:
            out = invoke(
                f"check-{package}",
                [base[0], "check", *base[1:], f"--package={package}"],
                fixture_root,
            )
            text = out.decode()
            assert text.startswith("typed-preview check ok (")
            assert text.endswith(" functions; check only)\n")

        tests = invoke(
            "contracts-test-interpreter",
            [base[0], "test", *base[1:], "--package=oxutf8-contracts"],
            fixture_root,
        )
        assert tests == CONTRACT_TESTS, tests
        result = invoke(
            "contracts-result-interpreter",
            [
                base[0],
                "run",
                *base[1:],
                "--package=oxutf8-contracts",
                "--engine=interpreter",
                "--entry-mode=result",
            ],
            fixture_root,
        )
        assert result == CONTRACT_RESULT, result

        deploy = evidence / "deploy"
        deploy.mkdir()
        if args.engine == "native":
            llvm = Path(os.environ["OXID_LLVM_BIN"]).resolve(strict=True)
            for tool in ["clang", "opt", "llvm-as", "ld.lld"]:
                version = invoke("tool-" + tool, [llvm / tool, "--version"], ROOT)
                assert b"19.1.7" in version, (tool, version)
            invoke(
                "contracts-test-native",
                [base[0], "test", *base[1:], "--package=oxutf8-contracts", "--engine=native"],
                fixture_root,
            )
            invoke(
                "build-oxutf8",
                [base[0], "build", *base[1:], "--package=oxutf8", "--entry-mode=process"],
                fixture_root,
            )
            archive = evidence / "oxutf8.tar"
            invoke(
                "package-oxutf8",
                [
                    base[0],
                    "package",
                    *base[1:],
                    "--package=oxutf8",
                    "--entry-mode=process",
                    "--output",
                    archive,
                ],
                fixture_root,
            )
            receipt["archive_sha256"] = sha(archive.read_bytes())
            with tarfile.open(archive) as package:
                package.extractall(deploy, filter="data")
            executable = deploy / "bin/oxutf8"
            receipt["native_sha256"] = sha(executable.read_bytes())
            run_command = [executable]
            use_driver = True
        else:
            run_command = [
                compiler,
                "run",
                "--manifest-path",
                manifest,
                "--package=oxutf8",
                "--engine=interpreter",
                "--entry-mode=process",
            ]
            use_driver = False

        empty_stderr = evidence / "empty.stderr"
        empty_stderr.write_bytes(b"")
        for name, data, exit_code in CORPUS:
            expected = data if exit_code == 0 else b""
            (evidence / f"input-{name}.bin").write_bytes(data)
            (evidence / f"expected-{name}.bin").write_bytes(expected)
            pin = {
                "name": name,
                "bytes": len(data),
                "sha256": sha(data),
                "exit": exit_code,
                "stdout_sha256": sha(expected),
            }
            if use_driver:
                out_dir = evidence / f"driver-{name}"
                cmd = [
                    "python3",
                    str(DRIVER),
                    "--stdin-file",
                    str(evidence / f"input-{name}.bin"),
                    "--segments",
                    "1,7,4096",
                    "--timeout",
                    "60",
                    "--cwd",
                    "/",
                    "--env-clear",
                    "--out",
                    str(out_dir),
                    "--expect-exit",
                    str(exit_code),
                    "--expect-stdout-file",
                    str(evidence / f"expected-{name}.bin"),
                    "--expect-stderr-file",
                    str(empty_stderr),
                    "--",
                    *map(str, run_command),
                ]
                result = subprocess.run(cmd, cwd="/", capture_output=True)
                (evidence / f"driver-{name}.stdout").write_bytes(result.stdout)
                (evidence / f"driver-{name}.stderr").write_bytes(result.stderr)
                assert result.returncode == 0, (name, result)
                report = json.loads((out_dir / "result.json").read_text())
                assert report.get("verdict", {}).get("reject_reasons") == [] and report.get("verdict", {}).get("record_ok") is True, report
                pin["driver_exit"] = report.get("exit_code", report.get("exit"))
                receipt["commands"].append(
                    {
                        "label": f"driver-{name}",
                        "command": cmd,
                        "cwd": "/",
                        "exit": result.returncode,
                        "driver_result": {
                            "exit_code": report.get("exit_code"),
                            "stdout_bytes": report.get("stdout_bytes"),
                            "stderr_bytes": report.get("stderr_bytes"),
                        },
                    }
                )
            else:
                result = subprocess.run(
                    [str(part) for part in run_command],
                    cwd=fixture_root,
                    input=data,
                    capture_output=True,
                    env=env,
                )
                (evidence / f"run-{name}.stdout").write_bytes(result.stdout)
                (evidence / f"run-{name}.stderr").write_bytes(result.stderr)
                assert result.returncode == exit_code, (name, result)
                assert result.stdout == expected, (name, sha(result.stdout), sha(expected))
                assert result.stderr == b"", (name, result.stderr)
                receipt["commands"].append(
                    {
                        "label": f"run-{name}",
                        "command": [str(part) for part in run_command],
                        "cwd": str(fixture_root),
                        "exit": result.returncode,
                        "stdout_sha256": sha(result.stdout),
                    }
                )
            receipt["corpus"].append(pin)
            print(f"{args.engine} {name} bytes={len(data)} exit={exit_code} sha256={sha(data)}")
            (evidence / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        (evidence / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    finally:
        shutil.rmtree(fixture_root, ignore_errors=True)


if __name__ == "__main__":
    main()
