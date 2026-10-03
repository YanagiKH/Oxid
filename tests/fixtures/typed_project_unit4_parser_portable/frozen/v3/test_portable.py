#!/usr/bin/env python3
"""Source/recipe and explicitly synthetic evidence controls; never builds Rust."""
import ast
import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

import portable as p


class PortableControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        cls.temporary = tempfile.TemporaryDirectory(prefix="oxid-portable-controls-")
        cls.root = Path(cls.temporary.name)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def test_exact_authority_and_unchanged_comparator_regions(self):
        c, proof = p.comparator()
        source = (p.HERE / "frozen/comparator/comparator.py").read_text()
        original = ast.parse(source)
        self.assertEqual(c.EXPECTED_FIELDS.__len__(), 22)
        self.assertEqual(proof["original_sha256"], p.COMPARATOR_SHA)
        function = next(x for x in original.body if isinstance(x, ast.FunctionDef) and x.name == "execution_manifest")
        self.assertGreater(function.end_lineno - function.lineno, 100)
        self.assertEqual(proof["unchanged_suffix_sha256"], "8dd8daea94ee2085aceeeb7a56a71242d285653685997995c5e047c2fe98c545")
        self.assertEqual(proof["unchanged_prefix_sha256"], "c018ad1927e861ba2abdd5609b99f8593367383bf7dfdfa3437c66bb669d979f")

    def test_complete_identity_inventory_controls(self):
        root = self.root / "small-map"; root.mkdir()
        (root / "file").write_bytes(b"reviewed")
        rec = p.identity(root / "file"); rec["path"] = "file"
        p.verify_map(root, [rec], exact=True)
        with self.assertRaises(p.Rejected): p.verify_map(root, [rec, rec])
        with self.assertRaises(p.Rejected): p.verify_map(root, [{**rec, "path": "../file"}])
        with self.assertRaises(p.Rejected): p.verify_map(root, [{**rec, "path": "/file"}])
        with self.assertRaises(p.Rejected): p.verify_map(root, [{**rec, "path": "missing"}])
        (root / "extra").write_bytes(b"")
        with self.assertRaises(p.Rejected): p.verify_map(root, [rec], exact=True)
        (root / "extra").unlink()
        (root / "file").write_bytes(b"changed!")
        with self.assertRaises(p.Rejected): p.verify_map(root, [rec])
        (root / "file").unlink(); (root / "file").symlink_to(p.HERE / "authority.json")
        with self.assertRaises(p.Rejected): p.verify_map(root, [rec])
        with self.assertRaises(p.Rejected): p.fresh(root)

    def test_helper_same_path_content_substitutions(self):
        root = self.root / "helpers"; shutil.copytree(p.HERE / "frozen/helpers", root)
        p.verify_map(root, self.a["helper_files"], exact=True)
        for row in self.a["helper_files"]:
            path = root / row["path"]; original = path.read_bytes()
            with self.subTest(helper=row["path"]):
                path.write_bytes(original + b"\n")
                with self.assertRaises(p.Rejected): p.verify_map(root, self.a["helper_files"], exact=True)
                path.write_bytes(original)

    def test_configs_and_inherited_environment(self):
        root = self.root / "config"; root.mkdir(); (root / "nested").mkdir(); (root / ".cargo").mkdir()
        for name in ("config", "config.toml"):
            path = root / ".cargo" / name; path.write_text("[build]\nrustflags=['--cfg=foreign']\n")
            with self.subTest(config=name), self.assertRaises(p.Rejected): p.no_cargo_configs(root / "nested")
            path.unlink()
        saved = {key: os.environ.get(key) for key in ("RUSTFLAGS", "RUSTC_WRAPPER", "CARGO_ENCODED_RUSTFLAGS", "PYTHONPATH", "CC", "LD_PRELOAD")}
        try:
            os.environ.update({key: "forbidden" for key in saved})
            clean = p.minimal_env(root)
            self.assertTrue(set(saved).isdisjoint(clean))
        finally:
            for key, value in saved.items():
                if value is None: os.environ.pop(key, None)
                else: os.environ[key] = value

    def test_measured_host_rejections(self):
        good = p.measured_host(); p.check_host(good, self.a)
        for key, value in (("os", "windows"), ("architecture", "aarch64"), ("python_pointer_width", 32), ("python_pointer_width", True)):
            changed = copy.deepcopy(good); changed[key] = value
            with self.subTest(key=key), self.assertRaises(p.Rejected): p.check_host(changed, self.a)
        for key, value in (("system", "Darwin"), ("machine", "arm64"), ("release", "")):
            changed = copy.deepcopy(good); changed["uname"][key] = value
            with self.subTest(key=key), self.assertRaises(p.Rejected): p.check_host(changed, self.a)

    def synthetic_cargo(self, directory, profile="debug"):
        root = directory; source = root / "source"; source.mkdir(parents=True)
        out = root / ("build-" + profile); result = out / "result"; result.mkdir(parents=True)
        toolchain = root / "toolchain"; (toolchain / "bin").mkdir(parents=True)
        (toolchain / "bin/rustc").write_bytes(b"SYNTHETIC Rust identity only")
        (source / "overlay-manifest.json").write_text("SYNTHETIC overlay bytes only\n")
        binary = result / "target" / self.a["recipe"]["target"] / profile / "deps/oxid-ab12"
        binary.parent.mkdir(parents=True)
        raw = bytearray(64); raw[:6] = b"\x7fELF\x02\x01"; raw[18:20] = (62).to_bytes(2, "little")
        binary.write_bytes(raw)
        cargo = {"reason": "compiler-artifact", "package_id": "path+" + source.as_uri() + "#oxid@0.9.0", "manifest_path": str(source / "Cargo.toml"),
                 "target": {"kind": ["bin"], "crate_types": ["bin"], "name": "oxid", "src_path": str(source / "src/cli.rs"), "edition": "2021", "doc": True, "doctest": False, "test": True},
                 "profile": {"opt_level": "0" if profile == "debug" else "3", "debuginfo": 0, "debug_assertions": profile == "debug", "overflow_checks": profile == "debug", "test": True},
                 "features": [], "filenames": [str(binary)], "executable": str(binary), "fresh": False}
        (result / "stdout.jsonl").write_text(json.dumps(cargo) + '\n{"reason":"build-finished","success":true}\n')
        (result / "stderr.txt").write_bytes(b"")
        build = {"schema": "oxid-unit4-parser-build-v1", "status": "built", "exit_code": 0, "control": False,
                 "profile": profile, "target": self.a["recipe"]["target"], "argv": [str(out / "rust-bin/cargo"), *self.a["recipe"]["cargo_tail"], *(["--release"] if profile == "release" else [])],
                 "environment": self.a["recipe"]["build_environment"], "cwd": str(source), "rustc_version": self.a["recipe"]["rustc_verbose"],
                 "rustc": p.identity(toolchain / "bin/rustc"), "overlay_manifest": p.identity(source / "overlay-manifest.json"),
                 "candidate_source_manifest_sha256": self.a["candidate_source_manifest_sha256"], "observer_source_sha256": self.a["helper_manifest_sha256"],
                 "stdout": p.identity(result / "stdout.jsonl"), "stderr": p.identity(result / "stderr.txt"), "binary": p.identity(binary)}
        invocation = {"started_ns": binary.stat().st_mtime_ns - 1000, "finished_ns": time.time_ns()}
        return root, out, toolchain, build, cargo, invocation

    def test_two_root_synthetic_cargo_positive_and_closed_mutations(self):
        for label, profile in (("alpha", "debug"), ("unrelated-omega", "release")):
            root, out, toolchain, build, cargo, invocation = self.synthetic_cargo(self.root / label, profile)
            def check(b): return p.verify_cargo(b, root, out, profile, toolchain, invocation, self.a)
            check(build)
            for key, value in (("cwd", str(root)), ("target", "aarch64-unknown-linux-gnu"), ("profile", "test"), ("control", True), ("exit_code", False),
                               ("observer_source_sha256", "0"*64), ("candidate_source_manifest_sha256", "0"*64), ("rustc_version", "rustc 1.99.0 fake"),
                               ("argv", build["argv"] + ["--features", "foreign"]), ("environment", {"RUSTFLAGS": "foreign"})):
                changed = copy.deepcopy(build); changed[key] = value
                with self.subTest(root=label, field=key), self.assertRaises(p.Rejected): check(changed)
            mutations = [("fresh", True), ("features", ["foreign"]), ("package_id", "foreign"), ("manifest_path", str(root / "foreign/Cargo.toml")),
                         ("target.kind", ["example"]), ("target.crate_types", ["lib"]), ("target.name", "foreign"), ("target.src_path", str(root / "foreign.rs")),
                         ("target.test", False), ("profile.opt_level", "s"), ("profile.test", False), ("profile.debug_assertions", profile != "debug"),
                         ("profile.overflow_checks", profile != "debug"), ("profile.debuginfo", 2)]
            stdout = out / "result/stdout.jsonl"; original_stdout = stdout.read_bytes()
            for key, value in mutations:
                changed = copy.deepcopy(cargo); target = changed; parts = key.split(".")
                for part in parts[:-1]: target = target[part]
                target[parts[-1]] = value
                stdout.write_text(json.dumps(changed) + '\n{"reason":"build-finished","success":true}\n')
                receipt = copy.deepcopy(build); receipt["stdout"] = p.identity(stdout)
                with self.subTest(root=label, cargo_field=key), self.assertRaises(p.Rejected): check(receipt)
            for rows in ([], [cargo, cargo, {"reason": "build-finished", "success": True}], [cargo, {"reason": "build-finished", "success": False}]):
                stdout.write_text("".join(json.dumps(x) + "\n" for x in rows)); receipt = copy.deepcopy(build); receipt["stdout"] = p.identity(stdout)
                with self.subTest(root=label, roster=len(rows)), self.assertRaises(p.Rejected): check(receipt)
            stdout.write_bytes(original_stdout)
            stale = copy.deepcopy(invocation); stale["started_ns"] = stale["finished_ns"] + 1
            with self.assertRaises(p.Rejected): p.verify_cargo(build, root, out, profile, toolchain, stale, self.a)
            binary = Path(build["binary"]["path"]); original = binary.read_bytes(); changed = bytearray(original); changed[18] = 183; binary.write_bytes(changed)
            receipt = copy.deepcopy(build); receipt["binary"] = p.identity(binary)
            with self.assertRaises(p.Rejected): check(receipt)
            binary.write_bytes(original)

    def test_two_root_real_source_materialization_and_coherent_substitutions(self):
        repo, checkout = os.environ.get("UNIT4_PORTABLE_HISTORICAL_REPO"), os.environ.get("UNIT4_PORTABLE_CHECKOUT")
        self.assertTrue(repo and checkout, "full source controls require both explicit repository inputs")
        roots = []
        for label in ("source-alpha", "unrelated/source-beta"):
            dest = self.root / label; dest.parent.mkdir(exist_ok=True)
            p.prepare(repo, checkout, dest); p.session_at(dest / "session.json", self.a); roots.append(dest)
        self.assertEqual(p.read(roots[0] / "source/candidate-source-manifest.json"), p.read(roots[1] / "source/candidate-source-manifest.json"))
        root = roots[0]; overlay_path = root / "source/overlay-manifest.json"; original_overlay = overlay_path.read_bytes()
        for name in ("src/frontend/parser.rs", "instrumentation.patch", "src/frontend/parser/unit4_observer.rs", "Cargo.lock", "Cargo.toml", "build.rs", "native/oxid_ffi.c"):
            path = root / "source" / name; raw = path.read_bytes(); path.write_bytes(raw + b"\n")
            altered = p.read(overlay_path)
            for row in altered["files"]:
                if row["path"] == name:
                    row.update({k: v for k, v in p.identity(path).items() if k != "path"})
            p.write(overlay_path, altered)
            with self.subTest(coherent_rehashed_source=name), self.assertRaises(p.Rejected): p.verify_overlay(root, self.a)
            path.write_bytes(raw); overlay_path.write_bytes(original_overlay)
        extra = root / "source/unreviewed.rs"; extra.write_bytes(b"")
        with self.assertRaises(p.Rejected): p.verify_overlay(root, self.a)
        extra.unlink()
        altered = p.read(overlay_path); altered["files"][0], altered["files"][1] = altered["files"][1], altered["files"][0]; p.write(overlay_path, altered)
        with self.assertRaises(p.Rejected): p.verify_overlay(root, self.a)
        overlay_path.write_bytes(original_overlay)
        with self.assertRaises(p.Rejected): p.prepare(repo, checkout, root)

    def test_missing_complete_evidence_is_failure(self):
        contract = os.environ.get("UNIT4_PORTABLE_CONTRACT")
        self.assertTrue(contract, "complete control suite requires exact materialized contract")
        c, _ = p.comparator(); frozen = c.load_contract(contract)
        result = c.compare_rows(frozen, [], {})
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["expected_observations"], 638)

    def test_optimized_cli_rejected(self):
        result = subprocess.run([sys.executable, "-O", "-B", str(p.HERE / "portable.py"), "inspect"], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("OPTIMIZED_PYTHON_UNSUPPORTED", result.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
