"""Exercise enum-vector C ownership with adversarial and compiler-emitted schemas.

Every O0/O2 command and artifact is retained outside the checkout. Generated
schemas require the real Rust producer receipt from verify_native_owned_enum_vectors.py.
Set OXID_BYTEBUF_SANITIZE=1 to instrument both configurations with ASan and UBSan.
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
HARNESS = "tests/owned_enum_vectors_runtime.c"
PRODUCER = "src/frontend/oir/owned/native_owned_enum_vector_schema_tests.rs"
EMITTER = "src/frontend/oir/owned/native_owned_vector_schemas.rs"
PRODUCER_TEST = ("frontend::oir::owned::native::owned_enum_vector_schema_tests::"
                 "native_owned_enum_vector_schemas_authenticate_distinct_roots_offsets_and_ineligible_sentinels")
SUPPORTED_HOST = (platform.system() in ("Linux", "Darwin")
                  and platform.machine().lower() in ("x86_64", "aarch64", "arm64")
                  and struct.calcsize("P") == 8 and sys.byteorder == "little")
GENERATED_MANIFEST = "OXID_TEST_OWNED_ENUM_SCHEMA_MANIFEST"
REQUIRE_GENERATED = "OXID_TEST_REQUIRE_GENERATED_OWNED_ENUM_SCHEMAS"
SANITIZER_FLAGS = ("-fsanitize=address,undefined", "-fno-omit-frame-pointer", "-fno-sanitize-recover=all")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def regular(path):
    path = Path(path)
    require(not path.is_symlink() and stat.S_ISREG(path.stat().st_mode), "unsafe evidence/source file: " + str(path))
    return path.read_bytes()


def identity(path):
    body = regular(path)
    return {"path": str(path), "bytes": len(body), "sha256": sha(body)}


def retained_command(directory, label, argv, environment, timeout, cwd=None):
    directory, cwd = Path(directory), Path(cwd or directory)
    row = {"argv": [str(value) for value in argv], "cwd": str(cwd),
           "timeout_seconds": timeout,
           "selected_environment": {name: environment[name] for name in
                ("ASAN_OPTIONS", "UBSAN_OPTIONS", "OXID_TEST_OWNED_ENUM_SCHEMA_OUTPUT",
                 GENERATED_MANIFEST, REQUIRE_GENERATED, "OXID_BYTEBUF_SANITIZE") if name in environment}}
    stdout, stderr = b"", b""
    try:
        result = subprocess.run(row["argv"], cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                capture_output=True, timeout=timeout)
        stdout, stderr = result.stdout, result.stderr
        row["exit_status"] = result.returncode
        return result
    except subprocess.TimeoutExpired as error:
        stdout, stderr = error.stdout or b"", error.stderr or b""
        row["error"] = "command timed out"
        raise
    except OSError as error:
        row["error"] = str(error)
        raise
    finally:
        for stream, body in (("stdout", stdout), ("stderr", stderr)):
            path = directory / (label + "." + stream)
            path.write_bytes(body)
            row[stream] = identity(path)
        (directory / (label + ".json")).write_text(json.dumps(row, indent=2) + "\n")


def strict_flags(optimization, sanitize):
    require(optimization in ("O0", "O2"), "unknown optimization")
    flags = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-" + optimization, "-UNDEBUG"]
    if sanitize:
        flags += SANITIZER_FLAGS
    return flags


def strict_environment(sanitize):
    environment = dict(os.environ)
    if sanitize:
        environment["ASAN_OPTIONS"] = "halt_on_error=1:abort_on_error=1:exitcode=1"
        environment["UBSAN_OPTIONS"] = "halt_on_error=1:print_stacktrace=1"
        environment.pop("LSAN_OPTIONS", None)
    return environment


def capture_sources(repo):
    """Copy the exact recursively included C source closure, not rewritten stubs."""
    repo = Path(repo).resolve(strict=True)
    pending, captured = [HARNESS], {}
    while pending:
        name = pending.pop()
        path = repo / name
        require(not path.is_symlink() and path.resolve().is_relative_to(repo), "source include escapes checkout")
        body = regular(path)
        if name in captured:
            require(captured[name] == body, "source changed while collecting includes")
            continue
        captured[name] = body
        for relative in re.findall(rb'(?m)^\s*#\s*include\s*"([^"\n]+)"', body):
            selected = (path.parent / relative.decode()).resolve(strict=True)
            require(selected.is_relative_to(repo), "source include escapes checkout")
            pending.append(selected.relative_to(repo).as_posix())
    return captured


def repository_snapshot(repo, directory, label, environment):
    result = retained_command(directory, label, ["git", "rev-parse", "HEAD", "HEAD^{tree}"],
                              environment, 30, repo)
    require(result.returncode == 0 and not result.stderr, "cannot identify current source commit/tree")
    values = result.stdout.decode().splitlines()
    require(len(values) == 2 and all(re.fullmatch("[a-f0-9]{40}", value) for value in values),
            "invalid current source commit/tree")
    return {"head": values[0], "tree": values[1]}


def rust_test_catalog(listing):
    """Authenticate the complete libtest listing and its own reported count."""
    require(type(listing) is bytes, "Rust catalog must be raw bytes")
    match = re.fullmatch(rb"((?:[A-Za-z_][A-Za-z_0-9:]*: test\n)+)\n([0-9]+) (test|tests), 0 benchmarks\n", listing)
    require(match is not None, "invalid complete Rust test catalog")
    names = tuple(line[:-6].decode() for line in match[1].splitlines())
    count = int(match[2])
    require(count == len(names) and len(set(names)) == count
            and match[3] == (b"test" if count == 1 else b"tests"),
            "Rust catalog summary/count/uniqueness changed")
    return names


def validate_selected_rust_tests(catalog, listing, execution, selected):
    """Derive exact filter accounting from a real complete binary catalog."""
    names = rust_test_catalog(catalog)
    selected = tuple(selected)
    require(selected and len(set(selected)) == len(selected)
            and all(names.count(name) == 1 for name in selected),
            "selected Rust tests must occur exactly once in the complete catalog")
    require(rust_test_catalog(listing) == selected, "selected Rust listing differs from its exact required tests")
    count, filtered = len(selected), len(names) - len(selected)
    word = "test" if count == 1 else "tests"
    expected = (rf"\n?running {count} {word}\n"
                + "".join("test " + re.escape(name) + r" \.\.\. ok\n" for name in selected)
                + rf"\ntest result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; "
                + rf"{filtered} filtered out; finished in [0-9.]+s\n\n")
    require(re.fullmatch(expected.encode(), execution) is not None,
            "selected Rust execution/count/filtered summary differs from its complete catalog")
    return {"catalog_test_count": len(names), "selected_tests": list(selected),
            "expected_filtered_out": filtered, "execution_scope": "selected tests; complete binary catalog listed only"}


def validate_producer_streams(catalog, listing, execution):
    return validate_selected_rust_tests(catalog, listing, execution, (PRODUCER_TEST,))


def catalog_receipt(directory, label, selection):
    return {**selection, "command": identity(directory / (label + ".json")),
            "stdout": identity(directory / (label + ".stdout")),
            "stderr": identity(directory / (label + ".stderr"))}


def load_generated_manifest(path, repo, snapshot):
    path = Path(path).resolve(strict=True)
    report = json.loads(regular(path))
    require(report.get("schema") == "oxid-owned-enum-vector-schema-producer-v1"
            and report.get("status") == "passed" and report.get("profile") in ("debug", "release")
            and report.get("producer_test") == PRODUCER_TEST and report.get("repository") == snapshot,
            "generated schema requires a successful current-source Rust producer receipt")
    for name in (PRODUCER, EMITTER):
        require(report["source_files"][name] == identity(repo / name), "schema producer source changed: " + name)
    streams = {}
    for label in ("catalog", "list", "run"):
        command = json.loads(regular(path.parent / ("producer-" + label + ".json")))
        argv = command.get("argv", [])
        expected = ["test", "--locked"]
        if report["profile"] == "release":
            expected.append("--release")
        expected += ["--bin", "oxid"]
        if label == "catalog":
            expected += ["--", "--list"]
        else:
            expected += [PRODUCER_TEST, "--", "--exact"]
            expected += ["--list"] if label == "list" else ["--nocapture", "--test-threads=1"]
        expected += ["--color", "never"]
        require(argv[1:] == expected and command.get("cwd") == str(repo)
                and command.get("exit_status") == 0, "wrong Rust schema producer command/outcome")
        for stream in ("stdout", "stderr"):
            selected = path.parent / ("producer-" + label + "." + stream)
            require(command[stream] == identity(selected), "Rust schema producer stream changed")
        if label == "run":
            require(command["selected_environment"].get("OXID_TEST_OWNED_ENUM_SCHEMA_OUTPUT") == report["output"]["path"],
                    "Rust producer did not target the retained schema file")
        streams[label] = regular(path.parent / ("producer-" + label + ".stdout"))
    selection = validate_producer_streams(streams["catalog"], streams["list"], streams["run"])
    require(report.get("catalog") == catalog_receipt(path.parent, "producer-catalog", selection),
            "Rust producer full catalog/hash/filter accounting changed")
    source = Path(report["output"]["path"])
    require(source.parent == path.parent and source.name == "compiler-schemas.ll"
            and report["output"] == identity(source) and 0 < source.stat().st_size <= 1024 * 1024,
            "compiler-emitted schema bytes changed or escaped producer output")
    return report, source


def evidence_directory(label):
    selected = os.environ.get("OXID_TEST_OWNED_ENUM_NATIVE_EVIDENCE")
    if selected:
        directory = Path(selected).resolve() / label
        require(not directory.exists(), "native evidence output must be new")
        directory.mkdir(parents=True)
    else:
        directory = Path(tempfile.mkdtemp(prefix="oxid-owned-enum-vector-" + label + "-"))
    require(directory != ROOT and ROOT not in directory.parents, "native evidence must be outside checkout")
    return directory


@unittest.skipUnless(SUPPORTED_HOST,
                     "enum-vector C contracts require Linux or macOS on64-bit little-endian x86_64/ARM64")
class NativeOwnedEnumVectors(unittest.TestCase):
    def run_contract(self, generated_manifest=None):
        mode = "generated" if generated_manifest else "adversarial"
        sanitize = os.environ.get("OXID_BYTEBUF_SANITIZE") == "1"
        directory = evidence_directory(mode)
        environment = strict_environment(sanitize)
        report = {"schema": "oxid-owned-enum-vector-c-contract-v1", "status": "running",
                  "mode": mode, "sanitizer": sanitize, "configurations": [], "output": str(directory)}
        try:
            snapshot = repository_snapshot(ROOT, directory, "repository-before", environment)
            report["repository"] = snapshot
            captured = capture_sources(ROOT)
            for name, body in captured.items():
                target = directory / "source" / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(body)
            report["source_files"] = [{"path": name, "bytes": len(body), "sha256": sha(body)}
                                      for name, body in sorted(captured.items())]
            compiler = shutil.which(os.environ.get("CC", "cc"))
            require(compiler is not None, "supported-host C contracts require an actual compiler")
            compiler_pin = identity(Path(compiler).resolve(strict=True))
            report["c_compiler"] = {"invocation": compiler, "executable": compiler_pin}
            version = retained_command(directory, "c-compiler-version", [compiler, "--version"], environment, 30)
            require(version.returncode == 0 and version.stdout, "C compiler version command failed")
            schema = None
            if generated_manifest:
                producer, schema = load_generated_manifest(generated_manifest, ROOT, snapshot)
                report["producer"] = producer
                report["producer_manifest"] = identity(Path(generated_manifest).resolve(strict=True))
                llvm_root = os.environ.get("OXID_TEST_OWNED_ENUM_LLVM_BIN") or os.environ.get("OXID_LLVM_BIN")
                require(llvm_root, "generated schemas require an explicit LLVM19.1.7 bin directory")
                clang = Path(llvm_root).absolute() / "clang"
                clang_pin = identity(clang.resolve(strict=True))
                report["llvm_clang"] = {"invocation": str(clang), "executable": clang_pin}
                version = retained_command(directory, "llvm-clang-version", [clang, "--version"], environment, 30)
                require(version.returncode == 0 and b"19.1.7" in version.stdout and not version.stderr,
                        "generated schemas require actual LLVM Clang19.1.7")
            for optimization in ("O0", "O2"):
                output = directory / optimization
                output.mkdir()
                binary = output / "contract"
                command = [compiler, *strict_flags(optimization, sanitize)]
                if schema:
                    schema_object = output / "compiler-schemas.o"
                    schema_command = [clang, "-x", "ir", "-c", schema, "-" + optimization,
                                      "-Wno-override-module"]
                    if sanitize:
                        schema_command += SANITIZER_FLAGS
                    schema_command += ["-o", schema_object]
                    compiled = retained_command(output, "schema-compile", schema_command, environment, 120)
                    require(compiled.returncode == 0 and not compiled.stderr, "real LLVM schema compilation failed")
                    require(identity(schema_object)["bytes"] > 0, "empty compiled schema object")
                    command += ["-DOXID_TEST_GENERATED_OWNED_ENUM_SCHEMAS"]
                command += [directory / "source" / HARNESS]
                if schema:
                    command.append(schema_object)
                command += ["-o", binary]
                built = retained_command(output, "compile", command, environment, 120)
                require(built.returncode == 0, "C enum-vector contract build failed; inspect " + str(output))
                result = retained_command(output, "run", [binary], environment, 60)
                require((result.returncode, result.stdout, result.stderr) == (0, b"", b""),
                        "C enum-vector ownership contract failed; inspect " + str(output))
                configuration = {"optimization": optimization, "binary": identity(binary), "status": "passed"}
                if schema:
                    configuration["schema_object"] = identity(schema_object)
                report["configurations"].append(configuration)
            require(capture_sources(ROOT) == captured, "enum-vector source closure changed during C contracts")
            require(repository_snapshot(ROOT, directory, "repository-after", environment) == snapshot,
                    "current source commit/tree changed during C contracts")
            require(identity(Path(compiler).resolve(strict=True)) == compiler_pin, "C compiler changed")
            if schema:
                load_generated_manifest(generated_manifest, ROOT, snapshot)
                require(identity(clang.resolve(strict=True)) == clang_pin, "LLVM schema compiler changed")
            report["status"] = "passed"
        except BaseException as error:
            report.update(status="failed", error=f"{type(error).__name__}: {error}")
            raise
        finally:
            (directory / "result.json").write_text(json.dumps(report, indent=2) + "\n")
            print("retained enum-vector C evidence: " + str(directory), flush=True)

    def test_adversarial_schemas_and_exact_ownership_boundaries(self):
        self.run_contract()

    def test_generated_schemas_from_the_real_rust_compiler(self):
        manifest = os.environ.get(GENERATED_MANIFEST)
        if not manifest:
            if os.environ.get(REQUIRE_GENERATED) == "1":
                self.fail("mandatory generated enum schemas require the real Rust producer manifest")
            self.skipTest("real Rust schema producer manifest absent; mandatory Linux CI runs verify_native_owned_enum_vectors.py")
        self.run_contract(manifest)


if __name__ == "__main__":
    unittest.main(verbosity=2)
