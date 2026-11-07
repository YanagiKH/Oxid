#!/usr/bin/env python3
"""Exercise real O0/O2 native builds, cache reuse and reproducible deployment.

Requires Linux x86_64 or ARM64, LLVM 19.1.7 and its SDK. Each optimization profile must
produce identical untouched ELF bytes across clean/cache/relocated builds and
compiler profiles in the same recorded environment. O0 and O2 must change real
object/ELF code, while both pass the independent byte-count oracle. No timing
or cross-toolchain reproducibility claim is made.
"""
import linux_native_target as native_target
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

from verify_reproducible_native import APP, LABELS, Oracle, clean, prepare_layout, tree_hashes
from verify_typed_package_app import APPLICATION, ROOT, digest, require, save

PROFILES = {"dev": "-O0", "release": "-O2"}


def machine_text(body):
    """Read the actual .text section, independently of compiler/cache metadata."""
    require(len(body) >= 64 and body[:7] == b"\x7fELF\x02\x01\x01", "expected little-endian ELF64")
    offset = int.from_bytes(body[40:48], "little")
    size = int.from_bytes(body[58:60], "little")
    count = int.from_bytes(body[60:62], "little")
    names_index = int.from_bytes(body[62:64], "little")
    require(size == 64 and 0 < count < 4096 and names_index < count
            and offset + size * count <= len(body), "invalid bounded ELF section table")

    def section(index):
        row = body[offset + index * size:offset + (index + 1) * size]
        start = int.from_bytes(row[24:32], "little")
        length = int.from_bytes(row[32:40], "little")
        require(start + length <= len(body), "ELF section exceeds retained file")
        return row, body[start:start + length]

    _, names = section(names_index)
    for index in range(count):
        row = body[offset + index * size:offset + (index + 1) * size]
        name = int.from_bytes(row[:4], "little")
        require(name < len(names), "invalid ELF section name index")
        if names[name:].split(b"\0", 1)[0] == b".text":
            require(int.from_bytes(row[4:8], "little") == 1 and int.from_bytes(row[8:16], "little") & 4,
                    ".text is not executable program data")
            _, text = section(index)
            require(text, "empty machine-code section")
            return text
    raise ValueError("ELF has no machine-code .text section")


class ProfileOracle(Oracle):
    native_profile = None

    def command(self, label, argv, cwd, payload=b"", failure=False, overrides=None):
        # Add the actual public CLI flag, leaving the shared byte/layout/cache
        # oracle independent of the compiler's internal settings representation.
        argv = list(argv)
        if len(argv) > 1 and argv[1] == "build":
            require(self.native_profile in PROFILES, "build has no selected native profile")
            argv.insert(2, "--profile=" + self.native_profile)
        return super().command(label, argv, cwd, payload, failure, overrides)

    def build(self, *args, **kwargs):
        artifact = super().build(*args, **kwargs)
        self.report["builds"][-1].update(native_profile=self.native_profile,
                                         optimization=PROFILES[self.native_profile])
        return artifact

    def retain_object(self, layout, variant, compiler_profile):
        entries = list((layout / APP / ".oxid/cache/native-object-v1").glob("*.oxobj"))
        require(len(entries) == 1, "cold profile build did not publish exactly one object")
        container = entries[0].read_bytes()
        require(len(container) >= 152 and container[:16] == b"OXID-OBJECT-v1\0\0",
                "cache object has an invalid framing header")
        require(int.from_bytes(container[48:56], "little") == len(container) - 88
                and hashlib.sha256(container[88:]).digest() == container[56:88],
                "cache object has an invalid retained size or checksum")
        body = container[88:]
        require(body[:7] == b"\x7fELF\x02\x01\x01" and body[16:20] == native_target.elf_identity(1),
                "cached program is not an actual host relocatable object")
        retained = self.output / "artifacts" / f"{compiler_profile}-{self.native_profile}-{variant}.o"
        retained.write_bytes(body)
        retained.chmod(0o444)
        self.report["objects"].append({"compiler_profile": compiler_profile, "native_profile": self.native_profile,
                                       "variant": variant, "artifact": str(retained), "bytes": len(body),
                                       "sha256": digest(retained), "container_sha256": digest(entries[0]),
                                       "cache_key": container[16:48].hex()})


def verify(args):
    require(native_target.host() is not None, "requires Linux x86_64 or ARM64")
    output = args.output.resolve()
    require(not output.exists() and not args.output.is_symlink() and ROOT not in output.parents and output != ROOT,
            "output must be a new directory outside the repository")
    llvm = args.llvm_bin.resolve(strict=True)
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(len(compilers) == len(set(compilers)) and all(path.is_file() and os.access(path, os.X_OK) for path in compilers),
            "compiler inputs must be distinct executable files")
    environment = {"PATH": str(llvm) + ":/usr/bin:/bin", "OXID_LLVM_BIN": str(llvm), "LANG": "C.UTF-8",
                   "LC_ALL": "C.UTF-8", "TZ": "UTC", "SOURCE_DATE_EPOCH": "0", "TMPDIR": str(output / "scratch")}
    if "LD_LIBRARY_PATH" in os.environ:
        environment["LD_LIBRARY_PATH"] = os.environ["LD_LIBRARY_PATH"]
    sources = tree_hashes(APPLICATION, exclude_outputs=True)
    controllers = {name: digest(ROOT / "scripts" / name) for name in
                   (Path(__file__).name, "verify_reproducible_native.py", "verify_typed_package_app.py")}
    output.mkdir(parents=True)
    for name in ("scratch", "artifacts", "compilers", "tools-spy"):
        (output / name).mkdir()
    for tool in ("clang", "opt", "ld.lld"):
        path = output / "tools-spy" / tool
        marker = str(output / "tools-spy-invoked").replace("'", "'\\''")
        path.write_text("#!/bin/sh\nprintf '%s\\n' invoked >> '" + marker + "'\nexit 91\n")
        path.chmod(0o700)
    report = {"schema": "oxid-native-profiles-application-v1", "status": "running",
              "scope": "Linux x86_64 or ARM64 bounded execution, same recorded SDK/environment and preserved relative layout; no speed or cross-SDK claim",
              "environment": environment, "host": platform.uname()._asdict(), "source_inputs": sources,
              "controllers_sha256": controllers, "compilers": [], "tools": [], "layouts": {}, "builds": [],
              "objects": [], "controls": [], "native_profiles": PROFILES}
    report["environment_sha256"] = hashlib.sha256(json.dumps(environment, sort_keys=True).encode()).hexdigest()
    oracle = ProfileOracle(output, environment, report)
    try:
        for index, compiler in enumerate(compilers):
            retained = output / "compilers" / f"oxid-{index}"
            before = digest(compiler)
            shutil.copyfile(compiler, retained)
            retained.chmod(0o555)
            require(digest(compiler) == digest(retained) == before, "compiler changed during retention")
            report["compilers"].append({"profile": f"compiler-{index}", "original_profile": compiler.parent.name,
                                        "source": str(compiler), "retained": str(retained), "sha256": before})
        for tool in ("clang", "opt", "ld.lld"):
            path = llvm / tool
            result, command = oracle.command("version-" + tool, [path, "--version"], output)
            require(b"19.1.7" in result.stdout, "expected actual LLVM 19.1.7 tools")
            report["tools"].append({"name": tool, "invocation": str(path), "physical": str(path.resolve(strict=True)),
                                    "sha256": digest(path.resolve()), "command": command})
        layouts = []
        for workspace in (False, True):
            variant = "workspace" if workspace else "standalone"
            first = output / (variant + "-first")
            relocated = output / (variant + "-longer-relocated-root-遷移")
            prepare_layout(first, workspace)
            owner = first if workspace else first / APP
            oracle.command(variant + "-lock", [report["compilers"][0]["retained"], "lock", "--offline"], owner)
            shutil.copytree(first, relocated)
            inputs = tree_hashes(first, exclude_outputs=True)
            require(tree_hashes(relocated, exclude_outputs=True) == inputs, "relocation changed source/manifest/lock bytes")
            report["layouts"][variant] = {"first": str(first), "relocated": str(relocated), "files": inputs}
            layouts.append((variant, workspace, first, relocated))
        for compiler in report["compilers"]:
            compiler_profile, binary = compiler["profile"], Path(compiler["retained"])
            for native_profile in PROFILES:
                oracle.native_profile = native_profile
                profile = compiler_profile + "-" + native_profile
                for variant, workspace, first, relocated in layouts:
                    clean(first)
                    oracle.build(binary, first, workspace, variant, profile, "clean-1")
                    clean(first)
                    for path in first.rglob("*.ox"):
                        os.utime(path, (946684800, 946684800))
                    oracle.build(binary, first, workspace, variant, profile, "clean-2", nested=True)
                    clean(first)
                    oracle.build(binary, first, workspace, variant, profile, "cache-cold", True, "miss")
                    oracle.retain_object(first, variant, compiler_profile)
                    (first / APP / ".oxid/bin/byte-count").unlink()
                    oracle.build(binary, first, workspace, variant, profile, "cache-warm", True, "hit", nested=True)
                    clean(relocated)
                    oracle.build(binary, relocated, workspace, variant, profile, "relocated-clean")
                    shutil.copytree(first / APP / ".oxid/cache", relocated / APP / ".oxid/cache")
                    # Cross-root reuse may miss conservatively; raw linked bytes
                    # must agree independently of whether the object was reused.
                    oracle.build(binary, relocated, workspace, variant, profile, "relocated-transferred-cache", True)
                    (relocated / APP / ".oxid/bin/byte-count").unlink()
                    oracle.build(binary, relocated, workspace, variant, profile, "relocated-cache-warm", True, "hit")
                    rows = [row for row in report["builds"] if row["profile"] == profile and row["variant"] == variant]
                    require(tuple(row["label"] for row in rows) == LABELS, "seven-build profile sequence is incomplete")
                    reference = Path(rows[0]["artifact"]).read_bytes()
                    require(all(Path(row["artifact"]).read_bytes() == reference for row in rows),
                            "raw linked ELF changed across clean/cache/relocated builds within a native profile")
                    oracle.preflight_controls(binary, relocated, workspace, profile, variant)
            require(digest(binary) == compiler["sha256"], "retained compiler changed")
        report["optimization_effects"] = []
        for variant, _, first, relocated in layouts:
            profile_bytes = {}
            object_bytes = {}
            for native_profile in PROFILES:
                rows = [row for row in report["builds"] if row["native_profile"] == native_profile and row["variant"] == variant]
                profile_bytes[native_profile] = Path(rows[0]["artifact"]).read_bytes()
                require(all(Path(row["artifact"]).read_bytes() == profile_bytes[native_profile] for row in rows),
                        "Rust compiler profile changed generated ELF bytes within a native profile")
                objects = [row for row in report["objects"] if row["native_profile"] == native_profile and row["variant"] == variant]
                object_bytes[native_profile] = Path(objects[0]["artifact"]).read_bytes()
                require(all(Path(row["artifact"]).read_bytes() == object_bytes[native_profile] for row in objects),
                        "Rust compiler profile changed generated object bytes within a native profile")
            require(profile_bytes["dev"] != profile_bytes["release"] and object_bytes["dev"] != object_bytes["release"],
                    "O2 did not change actual program object and linked ELF code for the application")
            text_bytes = {profile: machine_text(body) for profile, body in profile_bytes.items()}
            object_text_bytes = {profile: machine_text(body) for profile, body in object_bytes.items()}
            require(text_bytes["dev"] != text_bytes["release"]
                    and object_text_bytes["dev"] != object_text_bytes["release"],
                    "profile differences are only metadata: actual object/ELF machine code is unchanged")
            report["optimization_effects"].append({"variant": variant,
                "elf_sha256": {profile: hashlib.sha256(body).hexdigest() for profile, body in profile_bytes.items()},
                "object_sha256": {profile: hashlib.sha256(body).hexdigest() for profile, body in object_bytes.items()},
                "elf_bytes": {profile: len(body) for profile, body in profile_bytes.items()},
                "object_bytes": {profile: len(body) for profile, body in object_bytes.items()},
                "elf_text_sha256": {profile: hashlib.sha256(body).hexdigest() for profile, body in text_bytes.items()},
                "object_text_sha256": {profile: hashlib.sha256(body).hexdigest() for profile, body in object_text_bytes.items()},
                "elf_text_bytes": {profile: len(body) for profile, body in text_bytes.items()},
                "object_text_bytes": {profile: len(body) for profile, body in object_text_bytes.items()}})
            require(tree_hashes(first, exclude_outputs=True) == report["layouts"][variant]["files"]
                    and tree_hashes(relocated, exclude_outputs=True) == report["layouts"][variant]["files"],
                    "build changed retained source/lock bytes")
            shutil.rmtree(first)
            shutil.rmtree(relocated)
        report["source_free_deployments"] = []
        for row in report["builds"]:
            if row["label"] == "relocated-cache-warm":
                commands = oracle.cases(row["profile"] + "-" + row["variant"] + "-deployed", Path(row["artifact"]), {"PATH": ""})
                report["source_free_deployments"].append({"artifact": row["artifact"], "native_profile": row["native_profile"],
                                                         "sha256": row["sha256"], "commands": commands})
        require(all(digest(Path(row["artifact"])) == row["sha256"] for row in report["builds"] + report["objects"]),
                "retained ELF/object changed during oracle execution or source-free deployment")
        require(tree_hashes(APPLICATION, exclude_outputs=True) == sources, "repository application changed")
        require(all(digest(ROOT / "scripts" / name) == value for name, value in controllers.items()), "oracle controller changed")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "compilers": len(compilers), "builds": len(report["builds"]),
                          "commands": len(report["commands"]), "output": str(output)}))
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    try:
        return verify(parser.parse_args())
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
