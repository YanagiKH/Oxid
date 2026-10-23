#!/usr/bin/env python3
"""Independently validate typed source bundles and offline byte-count rebuilds.

Python tarfile/tomllib/hashlib check the artifact, not compiler-reported hashes.
Original projects are deleted before extraction/check/test/run. --native also
rebuilds with the real SDK and runs deployed ELF files with no source tree/PATH.
This is R1 local source-package evidence, not a v1 or publisher-authenticity claim.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tomllib

from verify_typed_package_app import APPLICATION, CASES, ROOT, Runner, digest, require, save
from verify_typed_package_lock import framed

LIBRARIES = {
    "packages/counts": ("target/model/lib.ox", "target/model/package_probe.ox", "target/tests/smoke.ox", "target/tests/support.ox"),
    "packages/report": (".oxid/source/lib.ox", ".oxid/source/package_probe.ox", ".oxid/tests/smoke.ox", ".oxid/tests/support.ox"),
}
WORKSPACE = ('\n[workspace]\nedition = "typed-preview"\n'
             'members = ["packages/counts", "packages/report"]\ndefault-members = ["."]\n')


def package_id(location):
    return hashlib.sha256(framed(b"oxid-typed-local-package-v1") + framed(location.encode())).hexdigest()


def payloads(project):
    paths = ["oxid.toml", "oxid.lock", "src/main.ox", "tests/counts.ox"]
    for location, names in LIBRARIES.items():
        paths.extend([location + "/oxid.toml", *(location + "/" + name for name in names)])
    return {"source/" + path: (project / path).read_bytes() for path in paths}


def check_lock(payload, workspace):
    lock = tomllib.loads(payload["source/oxid.lock"].decode())
    ids = {path: package_id(path) for path in (".", *LIBRARIES)}
    require(lock["version"] == (3 if workspace else 2)
            and lock["kind"] == ("typed-workspace-v1" if workspace else "typed-local-v1"),
            "bundle changed the authoritative lock format")
    if workspace:
        require(lock["manifest"] == hashlib.sha256(payload["source/oxid.toml"]).hexdigest()
                and lock["member"] == [{"package": value} for value in sorted(ids.values())],
                "shared lock does not bind every member and the raw workspace manifest")
    else:
        require(lock["root"] == ids["."], "standalone root identity changed")
    expected = []
    for location, identity in ids.items():
        prefix = "source/" if location == "." else "source/" + location + "/"
        raw = payload[prefix + "oxid.toml"]
        metadata = tomllib.loads(raw.decode())["project"]
        entry = "src/main.ox" if location == "." else LIBRARIES[location][0]
        checksum = "root"
        if location != ".":
            names = sorted(LIBRARIES[location][:2], key=lambda name: name.encode())
            data = framed(b"oxid-typed-local-source-v1") + framed(entry.encode()) + len(names).to_bytes(8, "little")
            for name in names:
                data += b"F" + framed(name.encode()) + framed(payload[prefix + name])
            checksum = "sha256:" + hashlib.sha256(data).hexdigest()
        expected.append({"id": identity, "path": location, "name": metadata["name"],
                         "version": metadata["version"], "edition": "typed-preview", "entry": entry,
                         "manifest": hashlib.sha256(raw).hexdigest(), "sources": checksum})
    require(lock["package"] == sorted(expected, key=lambda row: row["id"]),
            "bundled lock identities/manifests/loaded source closures differ")
    edges = sorted(((ids["."], "counts", ids["packages/counts"]),
                    (ids["."], "report", ids["packages/report"]),
                    (ids["packages/report"], "counts", ids["packages/counts"])))
    require(lock["dependency"] == [{"owner": owner, "alias": alias, "package": package}
                                   for owner, alias, package in edges], "shared nominal package diamond changed")
    return lock


def inspect_archive(path, expected, workspace):
    require(path.stat().st_size <= 64 * 1024 * 1024, "archive exceeds format cap")
    raw = path.read_bytes()
    found = {}
    offset = 0
    with tarfile.open(path, mode="r:") as archive:
        members = archive.getmembers()
        require(len(members) == len(expected) + 1 and len(members) <= 2048,
                "archive payload roster contains omissions or incidental files")
        require([member.name for member in members] == ["OXID-PACKAGE.toml", *sorted(expected, key=lambda value: value.encode())],
                "inventory must precede exactly the path-byte-sorted payload")
        for member in members:
            require(member.offset == offset and member.offset_data == offset + 512 and member.isfile()
                    and member.type == tarfile.REGTYPE and not member.pax_headers,
                    "archive uses an extension, link, directory or unexpected header")
            require(member.mode == 0o644 and member.uid == member.gid == member.mtime == 0
                    and member.uname == member.gname == member.linkname == "", "noncanonical ustar metadata")
            header = raw[offset:offset + 512]
            require(header[100:124] == b"0000644\0" + b"0000000\0" * 2
                    and header[124:136] == f"{member.size:011o}\0".encode()
                    and header[136:148] == b"00000000000\0"
                    and header[156:265] == b"0" + bytes(100) + b"ustar\0" + b"00"
                    and header[265:329] == bytes(64)
                    and header[329:345] == b"0000000\0" * 2 and header[500:] == bytes(12),
                    "ustar numeric/string fields are not canonical")
            checksum = sum(header[:148]) + 8 * 32 + sum(header[156:])
            require(header[148:156] == f"{checksum:06o}\0 ".encode(), "invalid ustar checksum")
            body = archive.extractfile(member).read()
            require(len(body) == member.size, "tarfile returned truncated payload")
            found[member.name] = body
            end = offset + 512 + member.size
            offset = offset + 512 + ((member.size + 511) // 512) * 512
            require(raw[end:offset] == bytes(offset - end), "nonzero payload padding")
    require(raw[offset:] == bytes(1024), "archive must end with exactly two zero blocks")
    inventory = tomllib.loads(found.pop("OXID-PACKAGE.toml").decode())
    require(set(inventory) == {"version", "kind", "root", "files"}
            and inventory["version"] == 1 and inventory["kind"] == "typed-source-package-v1"
            and inventory["root"] == "source", "wrong typed package inventory")
    expected_rows = [{"path": name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}
                     for name, data in sorted(expected.items(), key=lambda item: item[0].encode())]
    require(inventory["files"] == expected_rows and found == expected,
            "inventory/body hashes or exact retained source bytes differ")
    lock = check_lock(found, workspace)
    return {"sha256": digest(path), "bytes": len(raw), "files": len(found), "root": "source",
            "inventory": inventory, "lock": lock}


def summary(result, operation, archive=None, output=None, metadata=None):
    require(not result.stderr, "package JSON wrote stderr")
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
    require(len(rows) == 1, "package must emit one summary record")
    base = {"kind": "package-summary", "schema": "oxid-typed-package-summary-v1", "schema_version": 1,
            "edition": "typed-preview", "operation": operation, "success": result.returncode == 0}
    row = rows[0]
    require(all(row.get(key) == value for key, value in base.items()), "wrong package summary envelope")
    if result.returncode == 0:
        require(row == {**base, "archive": str(archive.resolve()), "output": str(output.resolve()) if output else None,
                        **{key: metadata[key] for key in ("sha256", "bytes", "files", "root")}},
                "package summary disagrees with independent artifact inspection")
    else:
        require(result.returncode == 1 and isinstance(row.get("error"), str) and row["error"],
                "package rejection must produce an ordinary bounded error")
    return row


class BundleRunner(Runner):
    def package(self, name, project, archive, expected, workspace):
        result = self.command(name, [self.compiler, "package", "--frozen", "--output", archive,
                                     "--message-format=json"], project)
        metadata = inspect_archive(archive, expected, workspace)
        summary(result, "create", archive, archive, metadata)
        self.report.setdefault("archives", {})[name] = metadata
        return metadata

    def rejected(self, name, project, archive):
        result = self.command(name, [self.compiler, "package", "--frozen", "--output", archive,
                                     "--message-format=json"], project, failure=True)
        summary(result, "create")

    def check(self, workspace, native):
        project = self.output / "original"
        shutil.copytree(APPLICATION, project, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
        for location, names in LIBRARIES.items():
            directory = project / location
            original = (directory / "src/lib.ox").read_text()
            manifest = directory / "oxid.toml"
            text = manifest.read_text().replace("[project]", '[project]\nkind = "lib"\nentry = "' + names[0] + '"', 1)
            if workspace:
                old_name = tomllib.loads(text)["project"]["name"]
                text = text.replace('name = "' + old_name + '"', 'name = "shared"', 1)
            manifest.write_text(text + '\n[tests]\npackage = "' + names[2] + '"\n')
            for name, data in ((names[0], original + "\nmod package_probe;\n"),
                               (names[1], "fn unused_but_checked() -> i32 { return 0; }\n"),
                               (names[2], "mod support;\nfn main() -> i32 { return crate::support::status(); }\n"),
                               (names[3], "pub fn status() -> i32 { return 0; }\n")):
                path = directory / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(data)
        if workspace:
            with (project / "oxid.toml").open("a") as stream:
                stream.write(WORKSPACE)
        for name in (".env", ".git/config", ".oxid/bin/incidental", "packages/counts/target/cache"):
            path = project / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("incidental fixture bytes must not enter the bundle\n")
        self.command("lock", [self.compiler, "lock", "--offline"], project)
        initial_inputs = payloads(project)
        initial_archive = self.output / "initial.oxpkg.tar"
        initial = self.package("package-initial", project, initial_archive, initial_inputs, workspace)
        for name in initial_inputs:
            path = project / name.removeprefix("source/")
            os.utime(path, (946684800, 946684800))
            os.chmod(path, 0o600)
        previous = os.umask(0o077)
        try:
            repeated = self.package("package-metadata-independent", project, self.output / "repeated.oxpkg.tar", initial_inputs, workspace)
        finally:
            os.umask(previous)
        require(repeated["sha256"] == initial["sha256"], "mtime/mode/umask changed source bundle bytes")
        relocated = self.output / "relocated project"
        project.rename(relocated)
        project = relocated
        moved = self.package("package-relocated", project, self.output / "relocated.oxpkg.tar", initial_inputs, workspace)
        require(moved["sha256"] == initial["sha256"], "complete relative-layout relocation changed artifact bytes")

        test = project / "packages/report" / LIBRARIES["packages/report"][2]
        valid_test = test.read_bytes()
        test.write_text("fn main() -> i32 { return false; }\n")
        self.rejected("invalid-dependency-test", project, initial_archive)
        require(digest(initial_archive) == initial["sha256"], "test refusal replaced existing archive")
        test.write_bytes(valid_test)
        dependency = project / "packages/counts" / LIBRARIES["packages/counts"][0]
        text = dependency.read_text()
        marker = "totals.lines = totals.lines + 1;"
        require(text.count(marker) == 1, "application edit marker changed")
        dependency.write_text(text.replace(marker, "totals.lines = totals.lines + 2;"))
        self.rejected("stale-dependency-lock", project, initial_archive)
        require(digest(initial_archive) == initial["sha256"], "lock refusal replaced existing archive")
        self.command("accept-dependency-update", [self.compiler, "lock", "--update"], project)
        test = project / "tests/counts.ox"
        text = test.read_text()
        require(text.count("line_count(&totals) != 2") == 3, "application test edit marker changed")
        test.write_text(text.replace("line_count(&totals) != 2", "line_count(&totals) != 4"))
        updated_inputs = payloads(project)
        updated_archive = self.output / "updated.oxpkg.tar"
        updated = self.package("package-updated", project, updated_archive, updated_inputs, workspace)
        require(updated["sha256"] != initial["sha256"], "accepted input changes did not change package bytes")
        shutil.rmtree(project)
        require(not project.exists() and not (self.output / "original").exists(), "original package tree remains")
        self.report["original_source_removed"] = True

        for name, archive, metadata, expected, multiplier in (("initial", initial_archive, initial, initial_inputs, 1),
                                                             ("updated", updated_archive, updated, updated_inputs, 2)):
            verified = self.command(name + "-verify", [self.compiler, "package", "--verify", archive,
                                                        "--sha256", metadata["sha256"].upper(), "--message-format=json"], self.output)
            summary(verified, "verify", archive, None, metadata)
            destination = self.output / (name + "-recipient")
            wrong = self.command(name + "-wrong-sha", [self.compiler, "package", "--extract", archive,
                                                        "--sha256", "0" * 64, "--output", destination,
                                                        "--message-format=json"], self.output, failure=True)
            summary(wrong, "extract")
            require(not destination.exists(), "digest refusal created extraction destination")
            previous = os.umask(0o077)
            try:
                extracted = self.command(name + "-extract", [self.compiler, "package", "--extract", archive,
                                                               "--sha256", metadata["sha256"], "--output", destination,
                                                               "--message-format=json"], self.output)
            finally:
                os.umask(previous)
            summary(extracted, "extract", archive, destination, metadata)
            require(destination.stat().st_mode & 0o777 == 0o700, "extraction root must remain private")
            for path in destination.rglob("*"):
                require(not path.is_symlink() and path.stat().st_mode & 0o777 == (0o644 if path.is_file() else 0o755),
                        "extraction inherited umask or published noncanonical file/directory modes")
            actual = {path.relative_to(destination).as_posix(): path.read_bytes()
                      for path in destination.rglob("*") if path.is_file()}
            require(actual == expected, "extraction altered or added payload bytes")
            recipient = destination / "source"
            flags = ["--workspace"] if workspace else []
            self.command(name + "-offline-check", [self.compiler, "check", *flags, "--frozen", "--offline"], recipient)
            tested = self.command(name + "-offline-test", [self.compiler, "test", *flags, "--frozen", "--offline",
                                                            "--message-format=json"], recipient)
            tests = [json.loads(line) for line in tested.stdout.splitlines()]
            require(not tested.stderr and len([row for row in tests if row.get("kind") == "test-result"
                                               and row.get("success") is True and row.get("exit_status") == 0]) == (3 if workspace else 1)
                    and tests[-1].get("success") is True, "extracted declared tests did not pass")
            self.cases(name + "-interpreted", [self.compiler, "run", "--frozen", "--offline"], recipient, multiplier=multiplier)
            if native:
                built = self.command(name + "-offline-build", [self.compiler, "build", *flags, "--frozen", "--offline",
                                                                "--message-format=json"], recipient)
                rows = [json.loads(line) for line in built.stdout.splitlines()]
                require(not built.stderr and len([row for row in rows if row.get("kind") == "compile-summary"
                                                  and row.get("success") is True]) == 1, "extracted binary did not build")
                if workspace:
                    members = {row["member"]: row["status"] for row in rows if row.get("kind") == "workspace-member"}
                    require(members == {".": "ok", "packages/counts": "checked-library", "packages/report": "checked-library"},
                            "workspace library/binary build behavior changed")
                executable = recipient / ".oxid/bin/byte-count"
                require(executable.read_bytes()[:7] == b"\x7fELF\x02\x01\x01", "offline build did not emit native ELF")
                self.cases(name + "-native", [executable], recipient, multiplier=multiplier)
                self.report.setdefault("native_sha256", {})[name] = digest(executable)
                if name == "updated":
                    deployment = self.output / "deployment"
                    deployment.mkdir()
                    deployed = deployment / "byte-count"
                    shutil.copy2(executable, deployed)
            shutil.rmtree(destination)
        if native:
            self.cases("deployed", [deployed], deployment, multiplier=2, env={"PATH": ""})
            require(digest(deployed) == self.report["native_sha256"]["updated"], "deployed ELF changed")
            self.report["source_free_deployment"] = {"sha256": digest(deployed), "cases": len(CASES), "empty_path": True}


def verify(args):
    output = args.output.resolve()
    require(not output.exists() and not args.output.is_symlink(), "output directory must be new")
    require(output != ROOT and ROOT not in output.parents, "output must be outside the repository")
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(len(compilers) == len(set(compilers)) and all(path.is_file() and os.access(path, os.X_OK) for path in compilers),
            "compiler inputs must be distinct executable files")
    output.mkdir(parents=True)
    source = {path.relative_to(APPLICATION).as_posix(): digest(path) for path in sorted(APPLICATION.rglob("*"))
              if path.is_file() and ".oxid" not in path.parts and path.name != "oxid.lock"}
    controllers = {name: digest(ROOT / "scripts" / name) for name in
                   (Path(__file__).name, "verify_typed_package_app.py", "verify_typed_package_lock.py")}
    report = {"schema": "oxid-typed-source-bundle-application-v1", "status": "running", "native": args.native,
              "scope": "R1 local source bundles; not v1, publisher authentication or native reproducibility",
              "source_sha256": source, "controllers_sha256": controllers, "compilers": []}
    try:
        for index, compiler in enumerate(compilers):
            directory = output / f"compiler-{index}"
            directory.mkdir()
            retained = directory / "oxid"
            shutil.copy2(compiler, retained)
            row = {"path": str(compiler), "retained": str(retained), "sha256": digest(retained),
                   "profile": compiler.parent.name, "status": "running", "variants": {}}
            require(digest(compiler) == row["sha256"], "compiler changed while copying")
            report["compilers"].append(row)
            for workspace in (False, True):
                name = "workspace" if workspace else "standalone"
                destination = directory / name
                destination.mkdir()
                variant = row["variants"][name] = {"status": "running"}
                BundleRunner(destination, retained, variant).check(workspace, args.native)
                variant["status"] = "passed"
            require(digest(retained) == row["sha256"], "retained compiler changed")
            row["status"] = "passed"
        for variant in ("standalone", "workspace"):
            for name in ("package-initial", "package-updated"):
                hashes = {row["variants"][variant]["archives"][name]["sha256"] for row in report["compilers"]}
                require(len(hashes) == 1, "compiler profile changed deterministic source bundle bytes")
        require(all(digest(APPLICATION / name) == value for name, value in source.items()), "repository application changed")
        require(all(digest(ROOT / "scripts" / name) == value for name, value in controllers.items()), "oracle code changed")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "compilers": len(compilers), "native": args.native,
                          "variants_per_compiler": 2, "output": str(output)}))
        return 0
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError, subprocess.TimeoutExpired) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true")
    try:
        return verify(parser.parse_args())
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
