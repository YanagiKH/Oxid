#!/usr/bin/env python3
"""Replay the frozen Unit2D independent gates in a new, bound source snapshot.

Python 3.10+ standard library only. Default scope is the eight ordinary tests
(including a unique marker), eight native families, and one physical family.
Neither the supplied Git checkout nor its existing Cargo target is modified.
"""
import argparse
import csv
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import time
import uuid

FIXTURE_REL = "tests/fixtures/fixed_array_unit2d_independent"
NATIVE_REL = "src/frontend/oir/owned/native_tests.rs"
MODULE_REL = "src/frontend/oir/owned/unit2d_independent.rs"
PREFIX = "frontend::oir::owned::native::tests::"
CHILD = PREFIX + "unit2d_independent::"
CONTROL_FILES = ("checkpoint1-heldout-v1.rs", "checkpoint1-partial-peak-v1.rs",
                 "checkpoint1-early-denial-v1.rs", "old-ir-export-v1.rs")
ORDINARY = [PREFIX + "independent_unit2d_" + name for name in (
    "checkpoint1_coordinate_oracle", "checkpoint1_partial_allocation_peaks",
    "checkpoint1_early_denial_peak", "export_old_ir")]
ORDINARY += [CHILD + "independent_unit2d_" + name for name in (
    "full_identity_and_closed_production_gate", "expansion_identity_and_bounded_prefix",
    "raw_authority_denials_precede_native_work")]
NATIVE = [CHILD + "independent_unit2d_" + name + "_llvm" for name in (
    "core_and_width", "phi_and_untaken", "transfer_chains", "core_fuel",
    "helper_effects_all_budgets", "cross_file_identity", "shared_aliases", "payload_extremes")]
PHYSICAL = CHILD + "independent_unit2d_external_storage_observer_llvm"
TOOLS = {"llvm-as": "LLVM version", "opt": "LLVM version",
         "clang": "clang version", "ld.lld": "LLD"}
PHASES = ("prepare", "build", "ordinary", "native", "physical", "verify")

# Provenance is deliberately split: historical inventories detect incomplete
# replay but are never used to decide correct language, fuel or storage behavior.
PROVENANCE = {
    "semantic_fuel_and_diagnostics": {
        "kind": "independent frozen source expectations",
        "sources": ["sources/reviewer-array-native-v3.rs", "sources/checkpoint1-heldout-v1.rs",
                    "sources/checkpoint1-partial-peak-v1.rs", "sources/checkpoint1-early-denial-v1.rs",
                    "expectations/expected-v1.json", "expectations/supplement-v1.json"],
        "role": "unchanged Rust assertions, scalar/coordinate oracles and hand-counted fuel schedules"},
    "physical_results": {
        "kind": "independent frozen expected-result manifest",
        "source": "expectations/physical-harness.tsv",
        "role": "exact case identities/status/stdout/stderr; 18 positives + 11 mutants = 29 rows",
        "counts": {"positive": 18, "mutant": 11, "total": 29}},
    "supplement_resource_assertion": {
        "kind": "post-original-observation resource/input-completeness assertion",
        "source": "expectations/supplement-v1.json:work_assertion",
        "role": "164 diagnostic bytes and progress beyond 164+128 expansions; explicitly not an independent pre-observation semantic/resource oracle"},
    "physical_observation_coverage": {
        "kind": "recorded text-preparation inventory",
        "source": "tools/storage/verify-checkpoint2-v2.json",
        "role": "coverage/reconstruction inventory; individual payload/guard checks come from the frozen observer"},
    "test_roster": {
        "kind": "frozen Rust test declarations plus replay marker",
        "counts": {"ordinary": 8, "ignored_native": 9, "total": 17},
        "role": "registration inventory, not a semantic oracle"},
    "old_ir": {
        "kind": "independently built historical PR28 baseline",
        "sources": ["expectations/old-ir-manifest.json", "expectations/old-ir-inventory.tsv"],
        "role": "exact no-regression byte comparison, not independently derived semantic expectations",
        "counts": {"modules": 90, "unique_modules": 59},
        "count_origin": "90 is also 9 source exporter builders x 2 guard choices x 5 source maps; 59 unique hashes is observed"},
    "native_execution_totals": {
        "kind": "historical observed qualification inventory",
        "source": "qualification-v2.json",
        "keys": ["elf_artifacts", "source_free_executions", "official_llvm_command_receipts"],
        "role": "336 ELF / 1210 execution / 3360 command coverage targets; sums of separate historical v1 and supplement runs, not semantic or fuel oracles or a prior combined-run claim"},
    "structural_totals": {
        "kind": "historical observed text-audit inventory",
        "source": "replay-inputs-v1.json:replay_inventories",
        "role": "275 exact sidecar names, 379 parsed bounds sites, 28 pointer phis; presence constraints only; individual properties are checked by llvm_structure.py"},
    "toolchain": {
        "kind": "pinned qualification environment contract",
        "values": {"rust": "1.99.0", "llvm": "19.1.7", "platform": "Linux x86_64",
                   "jobs": 2, "incremental": 0},
        "role": "replay configuration, not language expectations"},
    "formats": {
        "kind": "tool/file-format protocol invariants",
        "role": "40-character SHA-1 Git IDs, schema 1, ELF magic, command success 0 and exactly-one libtest pass; not semantic expectations"},
}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def load(path):
    return json.loads(Path(path).read_text())


def safe_relative(value):
    path = PurePosixPath(value)
    require(value and not path.is_absolute() and ".." not in path.parts
            and str(path) == value and "\\" not in value,
            "unsafe/noncanonical relative path: " + repr(value))
    return path


def inside(path, root):
    return Path(path).resolve().is_relative_to(Path(root).resolve())


def separate_paths(source, output):
    require(not inside(output, source) and not inside(source, output),
            "output must be disjoint from the supplied source checkout")


def tree_manifest(root):
    rows = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root).as_posix()
        require(not path.is_symlink(), "symlinks are unsupported in snapshot: " + relative)
        if path.is_file():
            rows.append(dict(path=relative, bytes=path.stat().st_size,
                             executable=bool(path.stat().st_mode & 0o111),
                             sha256=file_sha(path)))
    return rows


def assert_manifest(root, expected):
    actual = tree_manifest(root)
    require(actual == expected, "snapshot/input content or path inventory changed: " + str(root))


def parse_test_list(text):
    names = [line[:-6] for line in text.splitlines() if line.endswith(": test")]
    require(len(names) == len(set(names)), "duplicate libtest inventory entry")
    require(names, "empty libtest inventory")
    return set(names)


def assert_inventory(all_names, ignored_names, marker):
    expected_ignored = set(NATIVE + [PHYSICAL])
    expected = set(ORDINARY + [PREFIX + marker]) | expected_ignored
    observed = {name for name in all_names if "independent_unit2d" in name}
    observed_ignored = {name for name in ignored_names if "independent_unit2d" in name}
    require(observed == expected,
            "independent inventory mismatch: missing=" + repr(sorted(expected-observed))
            + " extra=" + repr(sorted(observed-expected)))
    require(observed_ignored == expected_ignored, "ignored/ordinary inventory mismatch")


def assert_one_pass(text, name):
    require(re.search(r"^test " + re.escape(name) + r" \.\.\. ok$", text, re.M),
            "test did not report its exact successful name: " + name)
    require(re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", text),
            "expected exactly one executed, unignored test: " + name)


def assert_execution_inventory(elf_count, execution_count, tool_count, qualification):
    # These expected numbers were observed in the qualified frozen-package run.
    # Passing counts alone can never establish semantic correctness.
    for label, actual, key in (("native ELF", elf_count, "elf_artifacts"),
                              ("source-free execution", execution_count, "source_free_executions"),
                              ("LLVM command receipt", tool_count, "official_llvm_command_receipts")):
        require(actual > 0, label + " inventory is empty")
        require(actual == qualification[key], label + " historical inventory count mismatch")


def assert_structure_inventory(sidecars, expected, bounds=None, phis=None):
    require(expected["provenance_kind"] == "historical-observed-inventory; not a semantic oracle",
            "structural inventory lacks explicit observed provenance")
    names = expected["structure_sidecars"]
    require(len(names) == len(set(names)) == expected["structure_sidecar_count"], "invalid structural sidecar inventory")
    require(set(sidecars) == set(names), "structural sidecar inventory mismatch: missing="
            + repr(sorted(set(names)-set(sidecars))) + " extra=" + repr(sorted(set(sidecars)-set(names))))
    if bounds is not None:
        require(bounds == expected["bounds_sites"], "historical bounds-site inventory mismatch")
    if phis is not None:
        require(phis == expected["pointer_phis"], "historical pointer-phi inventory mismatch")


def compare_result(actual, expected, context):
    for key in ("status", "stdout", "stderr"):
        require(actual[key] == expected[key], context + ": different " + key)


def compare_old_ir(root, manifest, inventory):
    expected_paths = set()
    for row in manifest:
        rel = safe_relative(row["path"])
        require(rel.parts[0] == "old-ir" and len(rel.parts) == 2, "bad old IR manifest path")
        filename = rel.parts[1]
        require(filename not in expected_paths, "duplicate old IR manifest path")
        expected_paths.add(filename)
        path = root / filename
        require(path.is_file() and path.stat().st_size == row["length"]
                and file_sha(path) == row["sha256"], "old IR hash/length mismatch: " + filename)
    require({p.name for p in root.iterdir()} == expected_paths | {"inventory.tsv"}, "old IR output inventory mismatch")
    require((root / "inventory.tsv").read_bytes() == inventory, "old IR TSV bytes differ")
    return dict(files=len(manifest), modules=sum(p.endswith(".ll") for p in expected_paths),
                unique_modules=len({row["sha256"] for row in manifest if row["path"].endswith(".ll")}),
                inventory_sha256=sha(inventory))


def assert_binary(binding, source_binding, root):
    require(binding["source_binding_sha256"] == source_binding,
            "binary belongs to a different prepared source")
    path = root / safe_relative(binding["relative_path"])
    require(inside(path, root / "evidence/bin") and not path.is_symlink(),
            "binary must be a retained regular evidence file")
    require(path.is_file() and file_sha(path) == binding["sha256"], "test binary hash changed")
    with path.open("rb") as stream:
        require(stream.read(4) == b"\x7fELF", "test binary is not ELF")
    return path


class Replay:
    def __init__(self, args):
        self.args = args
        self.root = args.output.resolve()
        require(not any(ord(c) < 32 for c in str(self.root)), "output path contains control characters")
        self.evidence = self.root / "evidence"
        self.source = self.root / "source"
        self.inputs = self.root / "inputs"
        self.state = {"schema": 1, "completed": [], "status": "started"}
        self.environment = os.environ.copy()
        self.environment.update(PYTHONDONTWRITEBYTECODE="1", LC_ALL="C", LANG="C")
        self.sequence = 0
        self.owned_output = False
        self.command_directory = None

    def command(self, label, argv, cwd=None, env=None):
        self.sequence += 1
        stem = f"{self.sequence:04d}-{label}"
        receipts = self.command_directory or self.evidence / "commands"
        receipts.mkdir(parents=True, exist_ok=True)
        require(not (receipts / (stem + ".json")).exists(), "receipt collision")
        argv = [str(value) for value in argv]
        record = dict(argv=argv, cwd=str(cwd or self.root), label=label,
                      start_ns=time.time_ns(), expected_exit=0,
                      stdout=stem + ".stdout", stderr=stem + ".stderr")
        save(receipts / (stem + ".json"), record)
        started = time.monotonic()
        try:
            with (receipts / record["stdout"]).open("wb") as stdout, \
                    (receipts / record["stderr"]).open("wb") as stderr:
                result = subprocess.run(argv, cwd=cwd or self.root,
                                        env=env or self.environment, stdout=stdout, stderr=stderr)
            record["exit"] = result.returncode
        except BaseException as exc:
            record["error"] = repr(exc)
            raise
        finally:
            record["seconds"] = time.monotonic() - started
            save(receipts / (stem + ".json"), record)
        require(result.returncode == 0, "command failed; retained receipt: " + str(receipts / (stem + ".json")))
        return (receipts / record["stdout"]).read_bytes(), (receipts / record["stderr"]).read_bytes()

    def git(self, *args):
        return self.command("git-" + args[0], ["git", "--no-optional-locks", "-C", self.args.repo, *args])[0]

    def prepare(self):
        require(self.args.repo is not None, "--repo is required to prepare")
        repo = self.args.repo.resolve()
        self.args.repo = repo
        separate_paths(repo, self.root)
        fixture = self.args.fixtures.resolve()
        separate_paths(fixture, self.root)
        require(not self.root.exists(), "output already exists; use --resume only for a verified completed phase")
        self.root.mkdir(parents=True)
        self.owned_output = True
        self.evidence.mkdir()
        actual_repo = Path(self.git("rev-parse", "--show-toplevel").decode().strip()).resolve()
        require(actual_repo == repo, "--repo must be the Git checkout root")
        dirty = self.git("status", "--porcelain=v1", "--untracked-files=all")
        require(not dirty, "dirty checkout rejected; commit/stash changes or use a clean checkout; no worktree bytes were copied")
        require(re.fullmatch(r"[0-9a-f]{40}", self.args.commit or ""), "--commit must be a full lowercase Git commit ID")
        head = self.git("rev-parse", "HEAD").decode().strip()
        commit = self.git("rev-parse", self.args.commit + "^{commit}").decode().strip()
        require(commit == self.args.commit, "source commit identity mismatch")
        tree = self.git("rev-parse", commit + "^{tree}").decode().strip()
        listing = self.git("ls-tree", "-r", "-z", commit)
        original_entries = []
        for entry in listing.split(b"\0"):
            if not entry:
                continue
            metadata, raw_path = entry.split(b"\t", 1)
            mode, kind, oid = metadata.decode().split()
            relative = raw_path.decode("utf-8")
            safe_relative(relative)
            require(kind == "blob" and mode in ("100644", "100755"),
                    "symlinks/submodules/nonregular Git entries unsupported: " + relative)
            original_entries.append(dict(path=relative, mode=mode, git_blob=oid))
        archive_path = self.evidence / "source.tar"
        self.git("archive", "--format=tar", "--output=" + str(archive_path), commit)
        self.source.mkdir()
        with tarfile.open(archive_path) as archive:
            members = archive.getmembers()
            seen = set()
            for member in members:
                value = member.name.rstrip("/")
                safe_relative(value)
                require(value not in seen, "duplicate archive member")
                seen.add(value)
                require(member.isfile() or member.isdir(), "archive links/special files are unsupported")
                path = self.source / value
                if member.isdir():
                    path.mkdir(parents=True, exist_ok=True)
                else:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with archive.extractfile(member) as stream:
                        path.write_bytes(stream.read())
                    path.chmod(0o755 if member.mode & 0o111 else 0o644)
        originals = tree_manifest(self.source)
        require({r["path"] for r in originals} == {r["path"] for r in original_entries},
                "git archive differs from full Git tree (export-ignore is unsupported)")
        original_map = {row["path"]: row for row in originals}
        for entry in original_entries:
            data = (self.source / entry["path"]).read_bytes()
            blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
            require(blob == entry["git_blob"], "archive bytes differ from Git blob (export-subst unsupported): " + entry["path"])
            require(original_map[entry["path"]]["executable"] == (entry["mode"] == "100755"), "archive mode mismatch")
            original_map[entry["path"]].update(entry)
        save(self.evidence / "original-source.json", originals)
        manifest = load(self.args.input_manifest)
        require(manifest["schema"] == 1, "unknown input manifest schema")
        self.inputs.mkdir()
        seen = set()
        for row in manifest["files"]:
            relative = str(safe_relative(row["path"]))
            require(relative not in seen, "duplicate fixture manifest path")
            seen.add(relative)
            source_path = fixture / relative
            require(inside(source_path, fixture) and not source_path.is_symlink(), "fixture escapes package")
            data = source_path.read_bytes()
            require(len(data) == row["bytes"] and sha(data) == row["sha256"], "frozen input changed: " + relative)
            destination = self.inputs / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        save(self.evidence / "input-manifest.json", manifest)
        save(self.evidence / "expectation-provenance.json", PROVENANCE)
        self.qualification = load(self.inputs / "qualification-v2.json")
        if commit in (self.qualification["source_head"], self.qualification.get("published_equal_tree_head")):
            require(tree == self.qualification["source_tree"], "historical source tree mismatch")
        frozen = (self.inputs / safe_relative(self.qualification["reviewer_module_path"])).read_bytes()
        require(sha(frozen) == self.qualification["reviewer_module_sha256"], "combined reviewer module mismatch")
        native = self.source / NATIVE_REL
        original_native = native.read_bytes()
        require(b"independent_unit2d" not in original_native, "source already contains ephemeral Unit2D registrations")
        require(not (self.source / MODULE_REL).exists(), "ephemeral module path already exists")
        (self.source / MODULE_REL).write_bytes(frozen)
        appendix = b"\n\n// Ephemeral independent Unit2D replay controls.\n"
        for name in CONTROL_FILES:
            appendix += (self.inputs / "sources" / name).read_bytes() + b"\n"
        appendix += b'\n#[cfg(test)]\n#[path = "unit2d_independent.rs"]\nmod unit2d_independent;\n'
        binding = dict(schema=1, commit=commit, tree=tree, checkout_head=head, run_root=str(self.root),
                       input_checkout=str(repo), profile=self.args.profile or "debug",
                       original_fixture_root=str(fixture), original_fixture_inventory=tree_manifest(fixture),
                       original_input_manifest_path=str(self.args.input_manifest.resolve()),
                       original_input_manifest_sha256=file_sha(self.args.input_manifest),
                       checkout_clean=True, archive_sha256=file_sha(archive_path),
                       original_manifest_sha256=file_sha(self.evidence / "original-source.json"),
                       input_manifest_sha256=file_sha(self.evidence / "input-manifest.json"),
                       inputs=tree_manifest(self.inputs), original_native_sha256=sha(original_native),
                       original_native_bytes=len(original_native), appendix_sha256=sha(appendix),
                       module_sha256=sha(frozen), nonce=uuid.uuid4().hex,
                       runner_sha256=file_sha(Path(__file__)),
                       capture_runner_sha256=file_sha(Path(__file__).with_name("replay_unit2d_tool_capture.py")))
        content_id = sha(json.dumps(binding, sort_keys=True, separators=(",", ":")).encode())
        marker = "independent_unit2d_replay_marker_" + content_id
        marker_source = ('\n#[test]\nfn ' + marker + '() {\n'
                         '    assert_eq!(std::env::var("OXID_UNIT2D_REPLAY_BINDING").unwrap(), "' + content_id + '");\n'
                         '    assert_eq!(env!("CARGO_MANIFEST_DIR"), ' + json.dumps(str(self.source), ensure_ascii=False) + ');\n'
                         '    eprintln!("OXID_UNIT2D_SOURCE_BINDING=' + content_id + '");\n}\n').encode()
        native.write_bytes(original_native + appendix + marker_source)
        binding.update(content_id=content_id, marker=marker, prepared=tree_manifest(self.source))
        save(self.evidence / "source-binding.json", binding)
        self.binding = binding
        self.state.update(source_binding_sha256=file_sha(self.evidence / "source-binding.json"),
                          tools=None, completed=["prepare"], status="prepared-only; no Rust/LLVM execution")
        save(self.root / "state.json", self.state)
        require(self.git("status", "--porcelain=v1", "--untracked-files=all") == dirty,
                "source checkout changed while preparing; archive itself remains bound")
        require(self.git("rev-parse", "HEAD").decode().strip() == head, "checkout HEAD moved while preparing")

    def check_source(self):
        require(file_sha(self.evidence / "source-binding.json") == self.state["source_binding_sha256"], "source binding changed")
        require(file_sha(self.evidence / "input-manifest.json") == self.binding["input_manifest_sha256"], "copied input manifest changed")
        require(file_sha(self.evidence / "original-source.json") == self.binding["original_manifest_sha256"], "original source manifest changed")
        require(file_sha(self.evidence / "source.tar") == self.binding["archive_sha256"], "source archive changed")
        require(load(self.evidence / "expectation-provenance.json") == PROVENANCE, "expectation provenance changed")
        require(self.binding["run_root"] == str(self.root), "prepared replay was relocated; prepare a fresh output")
        require(self.binding["runner_sha256"] == file_sha(Path(__file__)), "runner changed since preparation")
        require(self.binding["capture_runner_sha256"] == file_sha(Path(__file__).with_name("replay_unit2d_tool_capture.py")), "capture runner changed")
        assert_manifest(self.source, self.binding["prepared"])
        assert_manifest(self.inputs, self.binding["inputs"])

    def verify_original_inputs(self):
        require(file_sha(Path(self.binding["original_input_manifest_path"])) == self.binding["original_input_manifest_sha256"],
                "original input manifest changed since preparation")
        assert_manifest(Path(self.binding["original_fixture_root"]), self.binding["original_fixture_inventory"])
        # Resume audit commands get their own external receipts so that checking a
        # completed run never modifies its sealed evidence closure.
        directory = self.root / ("resume-checks-" + uuid.uuid4().hex)
        directory.mkdir(exist_ok=False)
        previous = self.command_directory
        previous_sequence = self.sequence
        self.command_directory = directory
        self.sequence = 0
        try:
            repo = Path(self.binding["input_checkout"])
            head, _ = self.command("original-head", ["git", "--no-optional-locks", "-C", repo, "rev-parse", "HEAD"])
            status, _ = self.command("original-status", ["git", "--no-optional-locks", "-C", repo,
                                   "status", "--porcelain=v1", "--untracked-files=all"])
            require(head.decode().strip() == self.binding["checkout_head"], "original checkout HEAD changed since preparation")
            require(not status, "original checkout is dirty since preparation")
        finally:
            self.command_directory = previous
            self.sequence = previous_sequence

    def configure(self):
        require(platform.system() == "Linux" and platform.machine() == "x86_64", "native qualification requires Linux x86_64")
        if self.state["tools"]:
            config = self.state["tools"]
            require(file_sha(Path(config["trusted_map_path"])) == config["trusted_map_sha256"], "original trusted-tool map changed")
            if self.args.trusted_tools:
                require(load(self.args.trusted_tools) == config["trusted"], "conflicting trusted-tool map on resume")
            if self.args.rust_bin:
                require(str(self.args.rust_bin.absolute()) == config["rust_bin"], "conflicting Rust directory on resume")
            if self.args.cargo_home:
                require(str(self.args.cargo_home.absolute()) == config["cargo_home"], "conflicting Cargo home on resume")
        else:
            require(self.args.rust_bin and self.args.trusted_tools, "--rust-bin and --trusted-tools are required for build/native phases")
            mapping = load(self.args.trusted_tools)
            require(set(mapping) == set(TOOLS), "trusted tool map must contain exactly llvm-as, opt, clang, ld.lld")
            for name, value in mapping.items():
                require(isinstance(value, str) and Path(value).is_absolute() and Path(value).is_file()
                        and os.access(value, os.X_OK), "invalid trusted tool path: " + name)
            rust_bin = self.args.rust_bin.absolute()
            config = dict(rust_bin=str(rust_bin), trusted=mapping,
                          trusted_map_path=str(self.args.trusted_tools.resolve()),
                          trusted_map_sha256=file_sha(self.args.trusted_tools),
                          hashes={name: file_sha(value) for name, value in mapping.items()},
                          rust_hashes={name: file_sha(rust_bin / name) for name in ("cargo", "rustc")},
                          cargo_home=str(self.args.cargo_home.absolute()) if self.args.cargo_home else os.environ.get("CARGO_HOME"),
                          rustup_home=os.environ.get("RUSTUP_HOME"))
            self.state["tools"] = config
            save(self.root / "state.json", self.state)
        for name, digest in config["hashes"].items():
            require(file_sha(config["trusted"][name]) == digest, "trusted LLVM executable changed: " + name)
        for name, digest in config["rust_hashes"].items():
            require(file_sha(Path(config["rust_bin"]) / name) == digest, "Rust executable changed: " + name)
        # Prevent ambient compiler wrappers/flags from changing the exact build.
        for key in list(self.environment):
            if key.startswith(("RUST", "CARGO_", "OXID_")) or key in ("CC", "CXX", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS", "LD", "AR", "CCC_OVERRIDE_OPTIONS", "PYTHONOPTIMIZE", "PYTHONPATH"):
                self.environment.pop(key)
        self.environment.update(CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", CARGO_NET_OFFLINE="true",
                                CARGO_TARGET_DIR=str(self.root / "target"),
                                RUSTC=str(Path(config["rust_bin"]) / "rustc"),
                                RUSTDOC=str(Path(config["rust_bin"]) / "rustdoc"),
                                RUST_BACKTRACE="1", OXID_UNIT2D_REPLAY_BINDING=self.binding["content_id"])
        if config["cargo_home"]:
            self.environment["CARGO_HOME"] = config["cargo_home"]
        if config["rustup_home"]:
            self.environment["RUSTUP_HOME"] = config["rustup_home"]
        self.environment["PATH"] = config["rust_bin"] + os.pathsep + str(Path(sys.executable).parent) + os.pathsep + os.environ.get("PATH", "")
        for key, folder in (("TMPDIR", "temporary"), ("OXID_OWNED_NATIVE_EVIDENCE", "native"),
                            ("OXID_UNIT2D_EXEC_EVIDENCE", "executions"), ("OXID_UNIT2D_OLD_IR_OUTPUT", "old-ir"),
                            ("OXID_UNIT2D_TOOL_RECEIPTS", "tool-receipts"), ("OXID_UNIT2D_CAPTURE_ROOT", "tool-captures")):
            path = self.evidence / folder
            path.mkdir(exist_ok=True)
            self.environment[key] = str(path)
        wrapper = self.evidence / "logged-tools"
        frozen = self.evidence / "frozen-loggers"
        if not wrapper.exists():
            wrapper.mkdir()
            frozen.mkdir()
            capture = self.evidence / "replay_unit2d_tool_capture.py"
            capture.write_bytes(Path(__file__).with_name(capture.name).read_bytes())
            capture.chmod(0o755)
            for name in TOOLS:
                (wrapper / name).symlink_to(capture)
                (frozen / name).symlink_to(self.inputs / "tools/native_tool_logger.py")
            save(self.evidence / "trusted-tools.json", config["trusted"])
            save(self.evidence / "tool-hashes.json", config["hashes"])
        require(file_sha(self.evidence / "replay_unit2d_tool_capture.py") == self.binding["capture_runner_sha256"], "installed capture runner changed")
        require(load(self.evidence / "trusted-tools.json") == config["trusted"], "installed trusted map changed")
        require(load(self.evidence / "tool-hashes.json") == config["hashes"], "installed trusted hashes changed")
        for name in TOOLS:
            require((wrapper / name).resolve() == self.evidence / "replay_unit2d_tool_capture.py", "tool wrapper link changed")
            require((frozen / name).resolve() == self.inputs / "tools/native_tool_logger.py", "frozen logger link changed")
        self.environment.update(OXID_LLVM_BIN=str(wrapper), OXID_UNIT2D_FROZEN_LOGGERS=str(frozen),
                                OXID_UNIT2D_TRUSTED_TOOLS=str(self.evidence / "trusted-tools.json"),
                                OXID_UNIT2D_TOOL_HASHES=str(self.evidence / "tool-hashes.json"),
                                OXID_UNIT2D_EXTERNAL_OBSERVERS=str(self.evidence / "physical"))
        # Only record task/toolchain environment, not unrelated user secrets.
        environment = {k: v for k, v in self.environment.items()
             if k.startswith(("CARGO_", "RUST", "OXID_")) or k in ("PATH", "TMPDIR", "LC_ALL", "LANG", "PYTHONDONTWRITEBYTECODE")}
        if (self.evidence / "environment.json").exists():
            require(load(self.evidence / "environment.json") == environment, "bound build/runtime environment changed on resume")
        else:
            save(self.evidence / "environment.json", environment)

    def build(self):
        require(not (self.root / "target").exists(), "fresh external Cargo target required; cannot retry a partial build")
        config = self.state["tools"]
        for name in ("rustc", "cargo"):
            output, _ = self.command(name + "-version", [Path(config["rust_bin"]) / name, "--version", "--verbose"])
            require(re.match(name + r" 1\.99\.0(?: |\n)", output.decode()), "requires pinned " + name + " 1.99.0")
            if name == "rustc":
                require(b"host: x86_64-unknown-linux-gnu" in output, "unexpected Rust host")
        for name, marker in TOOLS.items():
            output, _ = self.command(name + "-version", [config["trusted"][name], "--version"])
            require(re.search(re.escape(marker) + r" 19\.1\.7(?:$|[ (])", output.decode(), re.M), "requires pinned LLVM 19.1.7: " + name)
        self.check_source()
        argv = [Path(config["rust_bin"]) / "cargo", "test", "--bin", "oxid",
                "--locked", "--offline", "--no-run", "--jobs", "2", "--message-format=json"]
        if self.binding["profile"] == "release":
            argv.append("--release")
        output, _ = self.command("cargo-build", argv, cwd=self.source)
        artifacts = []
        for line in output.splitlines():
            try:
                item = json.loads(line)
            except (ValueError, UnicodeError):
                continue
            if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "oxid" \
                    and item.get("target", {}).get("kind") == ["bin"] and item.get("profile", {}).get("test") \
                    and item.get("executable"):
                artifacts.append(item)
        require(len(artifacts) == 1, "expected exactly one fresh Cargo oxid test artifact")
        artifact = artifacts[0]
        require(not artifact.get("fresh"), "Cargo reported a reused test binary")
        built = Path(artifact["executable"])
        require(inside(built, self.root / "target") and not built.is_symlink(), "Cargo executable is outside the fresh target")
        destination = self.evidence / "bin/oxid-unit2d-tests"
        destination.parent.mkdir()
        shutil.copy2(built, destination)
        binary = dict(relative_path=destination.relative_to(self.root).as_posix(), sha256=file_sha(destination),
                      bytes=destination.stat().st_size, cargo_artifact=artifact,
                      source_binding_sha256=self.state["source_binding_sha256"])
        require(binary["sha256"] == file_sha(built), "binary changed during evidence copy")
        save(self.evidence / "binary.json", binary)
        self.state["binary_binding_sha256"] = file_sha(self.evidence / "binary.json")
        self.check_source()
        self.inventory()
        self.run_test(PREFIX + self.binding["marker"])

    def binary(self):
        require(file_sha(self.evidence / "binary.json") == self.state["binary_binding_sha256"], "binary binding changed")
        return assert_binary(load(self.evidence / "binary.json"), self.state["source_binding_sha256"], self.root)

    def inventory(self):
        binary = self.binary()
        all_output, _ = self.command("test-inventory", [binary, "--list", "--format", "terse"], cwd=self.source)
        ignored_output, _ = self.command("ignored-inventory", [binary, "--ignored", "--list", "--format", "terse"], cwd=self.source)
        all_names, ignored = parse_test_list(all_output.decode()), parse_test_list(ignored_output.decode())
        assert_inventory(all_names, ignored, self.binding["marker"])
        save(self.evidence / "test-inventory.json", dict(all=sorted(all_names), ignored=sorted(ignored),
             scope=sorted(ORDINARY + [PREFIX + self.binding["marker"]] + NATIVE + [PHYSICAL])))

    def run_test(self, name, ignored=False):
        self.check_source()
        argv = [self.binary(), "--exact", name, "--nocapture", "--test-threads=1"]
        if ignored:
            argv.append("--ignored")
        output, errors = self.command(name.rsplit("::", 1)[-1], argv, cwd=self.source)
        assert_one_pass(output.decode(), name)
        if name == PREFIX + self.binding["marker"]:
            require(("OXID_UNIT2D_SOURCE_BINDING=" + self.binding["content_id"]).encode() in errors,
                    "marker output does not bind the source snapshot")
        self.binary()
        self.check_source()

    def ordinary(self):
        for name in ORDINARY:
            self.run_test(name)
        result = compare_old_ir(self.evidence / "old-ir", load(self.inputs / "expectations/old-ir-manifest.json"),
                                (self.inputs / "expectations/old-ir-inventory.tsv").read_bytes())
        require(result["modules"] == 90 and result["unique_modules"] == 59, "old IR baseline count mismatch")
        save(self.evidence / "old-ir-comparison.json", result)

    def native(self):
        for name in NATIVE:
            self.run_test(name, ignored=True)

    def physical(self):
        storage = self.inputs / "tools/storage"
        self.command("storage-text-tests", [sys.executable, "-B", "-m", "unittest", "-v", "test_observe_storage.py"], cwd=storage)
        physical = self.evidence / "physical"
        require(not physical.exists(), "physical output already exists; no evidence is overwritten")
        self.command("prepare-physical", [sys.executable, "-B", storage / "prepare_checkpoint2.py",
                     "--input", self.evidence / "native", "--output", physical,
                     "--source-commit", self.binding["commit"]], cwd=storage)
        verified, _ = self.command("verify-physical", [sys.executable, "-B", storage / "verify_checkpoint2.py", physical], cwd=storage)
        summary = json.loads(verified)
        require(summary == load(storage / "verify-checkpoint2-v2.json"), "physical preparation counts/claims changed")
        harness = (physical / "harness.tsv").read_bytes()
        require(harness == (self.inputs / "expectations/physical-harness.tsv").read_bytes(), "physical expected results changed")
        (physical / "manifest.tsv").write_bytes(harness)
        require((physical / "manifest.tsv").read_bytes() == harness, "manifest alias differs")
        save(self.evidence / "physical-alias.json", dict(source="physical/harness.tsv", alias="physical/manifest.tsv", sha256=sha(harness)))
        self.run_test(PHYSICAL, ignored=True)
        outcomes = []
        for row in csv.DictReader(io.StringIO(harness.decode()), delimiter="\t"):
            name = "physical-" + row["case"]
            actual = load(self.evidence / "executions" / (name + ".json"))
            actual.update(stdout=(self.evidence / "executions" / (name + ".stdout")).read_bytes(),
                          stderr=(self.evidence / "executions" / (name + ".stderr")).read_bytes())
            compare_result(actual, dict(status=int(row["status"]), stdout=bytes.fromhex(row["stdout_hex"]),
                                        stderr=bytes.fromhex(row["stderr_hex"])), name)
            outcomes.append(dict(case=row["case"], status=actual["status"]))
        require(len(outcomes) == 29 and sum(row["status"] == 1 for row in outcomes) == 11, "physical execution count mismatch")
        save(self.evidence / "physical-comparison.json", outcomes)

    def verify(self):
        self.verify_seals()
        self.check_source()
        self.binary()
        compare_old_ir(self.evidence / "old-ir", load(self.inputs / "expectations/old-ir-manifest.json"),
                       (self.inputs / "expectations/old-ir-inventory.tsv").read_bytes())
        native, executions = self.evidence / "native", self.evidence / "executions"
        elfs = sorted(native.glob("*.elf"))
        receipts = sorted(executions.glob("*.json"))
        tools = sorted((self.evidence / "tool-receipts").glob("*.json"))
        assert_execution_inventory(len(elfs), len(receipts), len(tools), self.qualification)
        for receipt in tools:
            row = load(receipt)
            require(row["tool"] in TOOLS and row["trusted_target"] == self.state["tools"]["trusted"][row["tool"]],
                    "LLVM receipt used an unbound tool")
            require(row["exit"] == 0, "LLVM command failed: " + receipt.name)
            require((receipt.parent / safe_relative(row["stdout"])).is_file()
                    and (receipt.parent / safe_relative(row["stderr"])).is_file(), "LLVM command lacks output receipts")
        elf_rows = {p.stem: dict(path=str(p.relative_to(self.evidence)), sha256=file_sha(p), bytes=p.stat().st_size) for p in elfs}
        for path in elfs:
            with path.open("rb") as stream:
                require(stream.read(4) == b"\x7fELF", "non-ELF native output")
        rows = []
        for path in receipts:
            row = load(path)
            require(row["env_clear"] is True and row["PATH"] == str(Path(row["cwd"]) / "no-tools"), "execution was not source-free")
            executable = Path(row["binary"]).name
            require(executable in elf_rows, "execution missing preserved ELF: " + path.name)
            require(isinstance(row["status"], int), "execution was signal-terminated: " + path.name)
            rows.append(dict(receipt=path.name, receipt_sha256=file_sha(path),
                             binary=elf_rows[executable], status=row["status"],
                             stdout_sha256=file_sha(path.with_suffix(".stdout")), stderr_sha256=file_sha(path.with_suffix(".stderr"))))
        save(self.evidence / "execution-binding.json", rows)
        self.verify_supplement()
        self.verify_structure()
        save(self.evidence / "artifact-manifest.json", dict(elf=list(elf_rows.values()),
             llvm=[dict(path=p.name, sha256=file_sha(p), bytes=p.stat().st_size) for p in sorted(native.glob("*.ll"))]))
        save(self.evidence / "verified.json", dict(status="passed", source_commit=self.binding["commit"],
             profile=self.binding["profile"],
             source_tree=self.binding["tree"], source_binding_sha256=self.state["source_binding_sha256"],
             test_binary_sha256=load(self.evidence / "binary.json")["sha256"],
             historical_binary_sha256=self.qualification["test_binary_sha256"],
             historical_binary_is_comparison_only=True, ordinary=8, native_families=9,
             expectation_provenance="expectation-provenance.json",
             fixed_inventory_totals_are_not_semantic_oracles=True,
             elf_artifacts=len(elfs), source_free_executions=len(receipts),
             scope="frozen independent Unit2D gates only; no production/release/whole-repository acceptance"))

    def verify_supplement(self):
        contract = load(self.inputs / "expectations/supplement-v1.json")
        cases = [dict(contract["positive_shared_alias"], name="shared-array-aliases"), *contract["extreme_cases"]]
        for expected in cases:
            stem = self.evidence / "executions" / expected["name"]
            actual = load(stem.with_suffix(".json"))
            actual.update(stdout=stem.with_suffix(".stdout").read_bytes(), stderr=stem.with_suffix(".stderr").read_bytes())
            compare_result(actual, dict(status=expected["status"], stdout=bytes.fromhex(expected["stdout_hex"]),
                                        stderr=bytes.fromhex(expected["stderr_hex"])), expected["name"])
        save(self.evidence / "supplement-comparison.json", dict(
            provenance="source-frozen semantic expected outputs from expectations/supplement-v1.json",
            cases=[item["name"] for item in cases], exact_status_stdout_stderr_match=True))

    def verify_structure(self):
        path = self.inputs / "tools/llvm_structure.py"
        spec = importlib.util.spec_from_file_location("unit2d_llvm_structure", path)
        module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = module
        spec.loader.exec_module(module)
        bounds, phis = [], []
        sidecars = sorted((self.evidence / "executions").glob("*.structure.tsv"))
        expected = load(self.evidence / "input-manifest.json")["replay_inventories"]
        assert_structure_inventory([path.name for path in sidecars], expected)
        for receipt in sidecars:
            name = receipt.name.removesuffix(".structure.tsv")
            llvm = self.evidence / "native" / (name + ".ll")
            if re.fullmatch(r"effects-[0-3]", name):
                llvm = self.evidence / "native" / (name + "-production.ll")
            text = llvm.read_text()
            for row in csv.DictReader(io.StringIO(receipt.read_text()), delimiter="\t"):
                observed = module.verify_bounds(text, row["function"], row["prefix"], int(row["length"]), row["guarded"] == "true")
                bounds.append(dict(module=llvm.name, sha256=file_sha(llvm), **observed))
        phi_modules = list((self.evidence / "native").glob("phi-*.ll")) + list((self.evidence / "native").glob("untaken-*.ll"))
        for llvm in sorted(phi_modules):
            text = llvm.read_text()
            for function in re.findall(r"^define .*?@([^ (]+)\(", text, re.M):
                for observed in module.CFG.parse(text, function).phi_predecessors():
                    phis.append(dict(module=llvm.name, function=function, **observed))
        assert_structure_inventory([path.name for path in sidecars], expected, len(bounds), len(phis))
        save(self.evidence / "structure.json", dict(bounds_sites=len(bounds), pointer_phis=len(phis), bounds=bounds, phis=phis,
             count_provenance="historical observed inventory, not a source-derived semantic oracle",
             property_check_source="frozen independent llvm_structure.py reader"))

    def evidence_inventory(self):
        rows = []
        for path in sorted(self.evidence.rglob("*")):
            relative = path.relative_to(self.evidence).as_posix()
            if relative == "temporary" or relative.startswith("temporary/"):
                continue
            if path.parent == self.evidence and path.name in self.state.get("seals", {}):
                continue
            if path.is_symlink():
                rows.append(dict(path=relative, kind="symlink", target=os.readlink(path)))
            elif path.is_dir():
                rows.append(dict(path=relative, kind="directory"))
            elif path.is_file():
                rows.append(dict(path=relative, kind="file", bytes=path.stat().st_size,
                                 executable=bool(path.stat().st_mode & 0o111), sha256=file_sha(path)))
            else:
                raise RuntimeError("unsupported evidence member: " + relative)
        return rows

    def seal(self, phase):
        rows = self.evidence_inventory()
        path = self.evidence / ("phase-" + phase + "-artifacts.json")
        save(path, rows)
        self.state.setdefault("seals", {})[path.name] = file_sha(path)

    def verify_seals(self, exact=True):
        # A resume may add evidence, but never replaces bytes from a completed
        # phase. The latest cumulative seal contains every previous receipt.
        seals = self.state.get("seals", {})
        for name, digest in seals.items():
            require(file_sha(self.evidence / safe_relative(name)) == digest, "phase seal changed")
        if not seals:
            return
        latest = next(name for phase in reversed(PHASES) for name in seals if name == "phase-" + phase + "-artifacts.json")
        expected = load(self.evidence / latest)
        actual = self.evidence_inventory()
        if exact:
            require(actual == expected, "completed evidence changed: exact member inventory/content mismatch")
        else:
            current = {row["path"]: row for row in actual}
            for row in expected:
                require(current.get(row["path"]) == row, "completed evidence changed: " + row["path"])

    def run(self):
        if self.args.resume:
            require(self.root.is_dir(), "resume output does not exist")
            self.state = load(self.root / "state.json")
            require(self.state["status"] != "failed" and not self.state.get("active"),
                    "failed/interrupted evidence cannot be reused; choose a new output")
            self.binding = load(self.evidence / "source-binding.json")
            if self.args.profile:
                require(self.args.profile == self.binding["profile"], "conflicting profile on resume; use a fresh output")
            if self.args.commit:
                require(self.args.commit == self.binding["commit"], "conflicting source commit on resume")
            if self.args.repo:
                require(str(self.args.repo.resolve()) == self.binding["input_checkout"], "conflicting source checkout on resume")
            self.qualification = load(self.inputs / "qualification-v2.json")
            self.sequence = len(list((self.evidence / "commands").glob("*.json")))
            self.check_source()
            self.verify_seals()
            self.verify_original_inputs()
            self.owned_output = True
        else:
            self.prepare()
        selected = "verify" if self.args.phase == "full" else self.args.phase
        if selected == "prepare":
            return
        self.configure()
        if self.args.resume:
            self.verify_seals()
        if "build" in self.state["completed"]:
            self.binary()
        for phase in PHASES[1:PHASES.index(selected)+1]:
            if phase in self.state["completed"]:
                continue
            self.state.update(active=phase, status="running")
            save(self.root / "state.json", self.state)
            print("Unit2D replay: " + phase, flush=True)
            getattr(self, phase)()
            self.seal(phase)
            self.state["completed"].append(phase)
            self.state.pop("active")
            self.state["status"] = "passed" if phase == "verify" else "partial: completed " + phase
            save(self.root / "state.json", self.state)


def main(argv=None):
    require(sys.flags.optimize == 0, "run without Python -O; frozen structural tools use assertions")
    parser = argparse.ArgumentParser(description=__doc__)
    default_fixture = Path(__file__).resolve().parent.parent / FIXTURE_REL
    parser.add_argument("--repo", type=Path, help="clean local Git checkout root; read-only input")
    parser.add_argument("--commit", help="exact 40-character commit to archive, required for preparation")
    parser.add_argument("--output", type=Path, help="new directory, disjoint from input checkout/package")
    parser.add_argument("--fixtures", type=Path, default=default_fixture)
    parser.add_argument("--input-manifest", type=Path, help="defaults to fixtures/replay-inputs-v1.json")
    parser.add_argument("--rust-bin", type=Path, help="installed pinned Rust 1.99.0 bin directory")
    parser.add_argument("--trusted-tools", type=Path, help="JSON map of exactly llvm-as/opt/clang/ld.lld to trusted absolute paths")
    parser.add_argument("--cargo-home", type=Path, help="existing dependency cache; build is always locked and offline")
    parser.add_argument("--phase", choices=("full", *PHASES), default="full", help="cumulative stopping phase; default full")
    parser.add_argument("--profile", choices=("debug", "release"), help="default debug for new runs; bound profile for resume")
    parser.add_argument("--resume", action="store_true", help="continue a successfully completed partial phase; never reuse failed evidence")
    parser.add_argument("--list", action="store_true", help="print scope/test names without preparing or executing anything")
    args = parser.parse_args(argv)
    if args.list:
        print(json.dumps(dict(scope="independent Unit2D only", ordinary=ORDINARY,
              marker="unique source-content marker (one ordinary test)", native=NATIVE,
              physical=PHYSICAL, phases=PHASES), indent=2))
        return 0
    require(args.output is not None, "--output is required")
    if args.input_manifest is None:
        args.input_manifest = args.fixtures / "replay-inputs-v1.json"
    replay = Replay(args)
    try:
        replay.run()
        print("Unit2D replay: " + replay.state["status"] + "; retained output: " + str(replay.root))
        return 0
    except BaseException as exc:
        if replay.owned_output and (replay.root / "state.json").is_file():
            replay.state.update(status="failed", error=repr(exc))
            save(replay.root / "state.json", replay.state)
        elif replay.owned_output and replay.evidence.is_dir():
            save(replay.evidence / "failure.json", dict(status="failed", error=repr(exc)))
        print("Unit2D replay failed: " + str(exc) + "; evidence was not cleaned", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
