"""Regression tests for scripts/package_release.py.

Each case runs the real packaging script with sys.executable inside a fresh
temporary directory that holds a fake executable and synthetic release files.
Nothing here builds, bootstraps, touches the network, or publishes.
"""
import hashlib
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path, PurePosixPath


SCRIPT = Path(__file__).resolve().with_name("package_release.py")

RELEASE_FILES = {
    ".oxid/bootstrap/compiler.oxb": b"OXB\x00\x01synthetic-compiler\xff\xfe\n",
    ".oxid/bootstrap/manifest.json": b'{"synthetic": "manifest", "version": 1}\n',
    "compiler/main.ox": b"// synthetic compiler entry\nfn main() {}\n",
    "compiler/providers.toml": b'[providers]\nsynthetic = "yes"\n',
    "stdlib/frontend/bytecode.ox": b"// synthetic bytecode frontend\r\n\x00tail",
}
BINARY_BYTES = b"\x7fELF-fake-oxid-binary\x00\x01\x02\xff"

ARCHIVE_SUFFIXES = (".zip", ".tar.gz", ".tgz", ".tar", ".7z")

# package_release.py writes the checksum with Path.write_text, whose default
# newline handling turns "\n" into os.linesep ("\r\n" on Windows).
PLATFORM_NEWLINE = os.linesep

# Runs package_release.main() in a child interpreter after installing one
# injected fault. Faults raise OSError from inside the standard library calls
# the script makes; nothing fills a disk or changes permissions.
FAULT_HARNESS = r"""
import errno, importlib.util, os, pathlib, sys, tarfile, zipfile

script, fault, *argv = sys.argv[1:]
dist = pathlib.Path.cwd().resolve() / "dist"
asset_name = argv[1]


def in_dist(path):
    try:
        pathlib.Path(path).resolve().relative_to(dist)
    except ValueError:
        return False
    return True


def fail_on_call(owner, attribute, call_number):
    original = getattr(owner, attribute)
    calls = [0]

    def wrapper(*args, **kwargs):
        calls[0] += 1
        if calls[0] == call_number:
            raise OSError(errno.ENOSPC, "injected: no space left on device")
        return original(*args, **kwargs)

    setattr(owner, attribute, wrapper)


if fault in ("compress", "compress-first"):
    # Fail while adding a member: the third one, after the archive already
    # holds data, or the first one.
    nth = 3 if fault == "compress" else 1
    fail_on_call(zipfile.ZipFile, "write", nth)
    fail_on_call(tarfile.TarFile, "add", nth)
elif fault == "digest-read":
    original_read_bytes = pathlib.Path.read_bytes

    def read_bytes(self):
        if in_dist(self):
            raise OSError(errno.EIO, "injected: archive read failed")
        return original_read_bytes(self)

    pathlib.Path.read_bytes = read_bytes
elif fault == "checksum-write":
    original_write_text = pathlib.Path.write_text

    def write_text(self, data, *args, **kwargs):
        if self.name.endswith(".sha256"):
            with open(self, "w", encoding="utf-8") as handle:
                handle.write(data[: len(data) // 2])
            raise OSError(errno.ENOSPC, "injected: checksum write failed")
        return original_write_text(self, data, *args, **kwargs)

    pathlib.Path.write_text = write_text
elif fault.startswith("publish-"):
    # publish-<archive|checksum>[-always]: fail os.replace onto that final
    # path, either once or on every attempt (which also breaks rollback).
    kind = fault.split("-")[1]
    always = fault.endswith("-always")
    target = dist / (asset_name if kind == "archive" else asset_name + ".sha256")
    original_replace = os.replace
    failed = [False]

    def replace(src, dst, *args, **kwargs):
        if pathlib.Path(dst).resolve() == target and (always or not failed[0]):
            failed[0] = True
            raise OSError(errno.EIO, "injected: publish failed")
        return original_replace(src, dst, *args, **kwargs)

    os.replace = replace
else:
    raise SystemExit(f"unknown fault: {fault}")

spec = importlib.util.spec_from_file_location("package_release_under_test", script)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
sys.argv = [script, *argv]
raise SystemExit(module.main())
"""

FORMATS = (
    ("oxid-build", "oxid-linux-x86_64.tar.gz"),
    ("oxid-build.exe", "oxid-windows-x86_64.zip"),
)
PREPARE_FAULTS = ("compress", "compress-first", "digest-read", "checksum-write")


def expected_checksum_bytes(digest, asset_name, newline=PLATFORM_NEWLINE):
    return f"{digest}  {asset_name}{newline}".encode("utf-8")


class PackageReleaseTests(unittest.TestCase):
    def setUp(self):
        self.make_workspace()

    def make_workspace(self):
        temp = tempfile.TemporaryDirectory(prefix="oxid-package-test-")
        self.addCleanup(temp.cleanup)
        self.base = Path(temp.name)
        self.root = self.base / "repo"
        self.root.mkdir()
        for relative, data in RELEASE_FILES.items():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        self.bin_dir = self.base / "bin"
        self.bin_dir.mkdir()

    def fake_binary(self, name):
        binary = self.bin_dir / name
        binary.write_bytes(BINARY_BYTES)
        binary.chmod(0o755)
        return binary

    def run_script(self, *args):
        return subprocess.run(
            [sys.executable, "-B", str(SCRIPT), *map(str, args)],
            cwd=self.root,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )

    def expected_members(self, staged_name):
        return {staged_name: BINARY_BYTES, **RELEASE_FILES}

    def assert_member_name_safe(self, name):
        self.assertNotIn("\\", name)
        path = PurePosixPath(name)
        self.assertFalse(path.is_absolute(), name)
        self.assertNotIn("..", path.parts, name)
        self.assertFalse(name.endswith("/"), name)

    def read_zip(self, asset):
        with zipfile.ZipFile(asset) as archive:
            self.assertIsNone(archive.testzip())
            infos = archive.infolist()
            names = [info.filename for info in infos]
            self.assertEqual(len(names), len(set(names)), names)
            members = {}
            for info in infos:
                self.assertFalse(info.is_dir(), info.filename)
                self.assert_member_name_safe(info.filename)
                members[info.filename] = archive.read(info)
        return members

    def read_tar_gz(self, asset):
        with asset.open("rb") as handle:
            self.assertEqual(handle.read(2), b"\x1f\x8b", "asset is not gzip data")
        with tarfile.open(asset, "r:gz") as archive:
            infos = archive.getmembers()
            names = [info.name for info in infos]
            self.assertEqual(len(names), len(set(names)), names)
            members = {}
            for info in infos:
                self.assertTrue(info.isreg(), f"{info.name} is not a regular file")
                self.assert_member_name_safe(info.name)
                members[info.name] = archive.extractfile(info).read()
        return members

    def assert_no_release_pair(self):
        leftovers = [
            path.relative_to(self.base).as_posix()
            for path in self.base.rglob("*")
            if path.is_file()
            and (path.name.endswith(".sha256") or path.name.endswith(ARCHIVE_SUFFIXES))
        ]
        self.assertEqual(leftovers, [], "failed run left package or checksum files")
        # Suffix matching alone misses outputs such as dist/oxid, so a dist
        # directory left by a failed run must be empty (an empty dist is fine).
        dist = self.root / "dist"
        if dist.exists() or dist.is_symlink():
            self.assertTrue(dist.is_dir() and not dist.is_symlink(), "dist is not a directory")
            dist_entries = sorted(
                path.relative_to(self.base).as_posix() for path in dist.rglob("*")
            )
            self.assertEqual(dist_entries, [], "failed run left files in dist")

    def assert_failure(self, result, *diagnostic_parts):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("packaged", result.stdout)
        for part in diagnostic_parts:
            self.assertIn(part, result.stderr)
        self.assert_no_release_pair()

    def check_success(self, binary_name, asset_name, staged_name, reader):
        binary = self.fake_binary(binary_name)
        result = self.run_script(binary, asset_name)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

        dist = self.root / "dist"
        asset = dist / asset_name
        checksum = dist / f"{asset_name}.sha256"
        self.assertEqual(
            sorted(path.name for path in dist.iterdir()),
            sorted([asset_name, f"{asset_name}.sha256"]),
        )

        members = reader(asset)
        self.assertEqual(len(members), 6, sorted(members))
        self.assertEqual(members, self.expected_members(staged_name))

        digest = hashlib.sha256(asset.read_bytes()).hexdigest()
        checksum_bytes = checksum.read_bytes()
        self.assertEqual(checksum_bytes, expected_checksum_bytes(digest, asset_name))
        newline = PLATFORM_NEWLINE.encode("ascii")
        self.assertTrue(checksum_bytes.endswith(newline), checksum_bytes)
        body = checksum_bytes[: -len(newline)]
        self.assertNotIn(b"\n", body, checksum_bytes)
        self.assertNotIn(b"\r", body, checksum_bytes)
        self.assertIn(digest, result.stdout)

    def test_tar_gz_package(self):
        self.check_success("oxid-build", "oxid-linux-x86_64.tar.gz", "oxid", self.read_tar_gz)

    def test_zip_package(self):
        self.check_success("oxid-build.exe", "oxid-windows-x86_64.zip", "oxid.exe", self.read_zip)

    def test_staged_binary_name_follows_suffix_not_format(self):
        cases = (
            ("oxid-build.exe", "cross.tar.gz", "oxid.exe", self.read_tar_gz),
            ("oxid-build", "cross.zip", "oxid", self.read_zip),
        )
        for binary_name, asset_name, staged_name, reader in cases:
            with self.subTest(asset=asset_name):
                self.make_workspace()
                self.check_success(binary_name, asset_name, staged_name, reader)

    def test_missing_binary(self):
        missing = self.bin_dir / "does-not-exist"
        result = self.run_script(missing, "oxid.tar.gz")
        self.assert_failure(result, "binary not found", "does-not-exist")

    def test_each_required_file_missing(self):
        for relative in RELEASE_FILES:
            with self.subTest(missing=relative):
                self.make_workspace()
                binary = self.fake_binary("oxid-build")
                (self.root / relative).unlink()
                for asset_name in ("oxid.tar.gz", "oxid.zip"):
                    result = self.run_script(binary, asset_name)
                    self.assert_failure(
                        result, "required release file not found", str(Path(relative))
                    )

    def test_asset_name_with_directory(self):
        binary = self.fake_binary("oxid-build")
        for asset_name in ("nested/oxid.tar.gz", "../oxid.zip", str(self.base / "oxid.zip")):
            with self.subTest(asset=asset_name):
                result = self.run_script(binary, asset_name)
                self.assert_failure(result, "asset name must not contain a directory")
                self.assertFalse((self.root / "nested").exists())

    def test_unsupported_archive_type(self):
        binary = self.fake_binary("oxid-build")
        for asset_name in ("oxid.tar", "oxid.tgz", "oxid.7z", "oxid"):
            with self.subTest(asset=asset_name):
                result = self.run_script(binary, asset_name)
                self.assert_failure(result, "unsupported archive type", asset_name)

    def test_leftover_check_rejects_extensionless_dist_file(self):
        dist = self.root / "dist"
        dist.mkdir()
        self.assert_no_release_pair()  # an empty dist may remain after a failure
        (dist / "oxid").write_bytes(BINARY_BYTES)
        with self.assertRaises(AssertionError):
            self.assert_no_release_pair()

    def test_wrong_argument_count(self):
        binary = self.fake_binary("oxid-build")
        for args in ((), (binary,), (binary, "oxid.zip", "extra")):
            with self.subTest(argc=len(args)):
                result = self.run_script(*args)
                self.assert_failure(result, "usage: package_release.py <binary> <asset-name>")

    def run_with_fault(self, fault, *args):
        temp_root = self.base / "tmp"
        temp_root.mkdir(exist_ok=True)
        env = dict(os.environ, TMPDIR=str(temp_root), TEMP=str(temp_root), TMP=str(temp_root))
        result = subprocess.run(
            [sys.executable, "-B", "-c", FAULT_HARNESS, str(SCRIPT), fault, *map(str, args)],
            cwd=self.root,
            env=env,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(sorted(temp_root.iterdir()), [], "failed run left temp files")
        return result

    def dist_snapshot(self):
        dist = self.root / "dist"
        return {
            path.relative_to(dist).as_posix(): path.read_bytes() if path.is_file() else None
            for path in sorted(dist.rglob("*"))
        }

    def publish_previous_release(self, binary_name, asset_name):
        binary = self.fake_binary(binary_name)
        result = self.run_script(binary, asset_name)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        snapshot = self.dist_snapshot()
        self.assertEqual(sorted(snapshot), sorted([asset_name, f"{asset_name}.sha256"]))
        # The next build produces different bytes, so an overwrite would show.
        binary.write_bytes(BINARY_BYTES + b"-next-build")
        return binary, snapshot

    def assert_fault_failure(self, result, *diagnostic_parts):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("packaged", result.stdout)
        self.assertNotIn("packaged", result.stderr)
        for part in diagnostic_parts:
            self.assertIn(part, result.stderr)

    def test_prepare_failure_leaves_no_new_release_files(self):
        for fault in PREPARE_FAULTS:
            for binary_name, asset_name in FORMATS:
                with self.subTest(fault=fault, asset=asset_name):
                    self.make_workspace()
                    binary = self.fake_binary(binary_name)
                    result = self.run_with_fault(fault, binary, asset_name)
                    self.assert_fault_failure(result, "injected")
                    self.assert_no_release_pair()

    def test_prepare_failure_keeps_existing_release_bytes(self):
        for fault in PREPARE_FAULTS:
            for binary_name, asset_name in FORMATS:
                with self.subTest(fault=fault, asset=asset_name):
                    self.make_workspace()
                    binary, before = self.publish_previous_release(binary_name, asset_name)
                    result = self.run_with_fault(fault, binary, asset_name)
                    self.assert_fault_failure(result, "injected")
                    self.assertEqual(self.dist_snapshot(), before)

    def test_publish_failure_is_reported_and_rolled_back(self):
        for kind in ("archive", "checksum"):
            for binary_name, asset_name in FORMATS:
                with self.subTest(fail=kind, asset=asset_name, previous=False):
                    self.make_workspace()
                    binary = self.fake_binary(binary_name)
                    result = self.run_with_fault(f"publish-{kind}", binary, asset_name)
                    self.assert_fault_failure(result, "failed to publish", "injected")
                    self.assert_no_release_pair()
                with self.subTest(fail=kind, asset=asset_name, previous=True):
                    self.make_workspace()
                    binary, before = self.publish_previous_release(binary_name, asset_name)
                    result = self.run_with_fault(f"publish-{kind}", binary, asset_name)
                    self.assert_fault_failure(result, "failed to publish", "injected")
                    self.assertEqual(self.dist_snapshot(), before)

    def test_publish_rollback_failure_keeps_backup_and_says_where(self):
        binary_name, asset_name = FORMATS[0]
        binary, before = self.publish_previous_release(binary_name, asset_name)
        result = self.run_with_fault("publish-checksum-always", binary, asset_name)
        self.assert_fault_failure(result, "failed to publish", "rollback failed", "injected")
        dist = self.root / "dist"
        # The archive was restored; the old checksum could not be put back and
        # must survive in the named work directory instead of being deleted.
        self.assertEqual((dist / asset_name).read_bytes(), before[asset_name])
        self.assertFalse((dist / f"{asset_name}.sha256").exists())
        kept = [
            path for path in dist.rglob("*")
            if path.is_file() and path.read_bytes() == before[f"{asset_name}.sha256"]
        ]
        self.assertEqual(len(kept), 1, sorted(self.dist_snapshot()))
        self.assertIn(str(kept[0].parent.name), result.stderr)

    def test_success_leaves_no_work_directory(self):
        for binary_name, asset_name in FORMATS:
            with self.subTest(asset=asset_name):
                self.make_workspace()
                self.publish_previous_release(binary_name, asset_name)
                binary = self.bin_dir / binary_name
                result = self.run_script(binary, asset_name)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                after = self.dist_snapshot()
                self.assertEqual(sorted(after), sorted([asset_name, f"{asset_name}.sha256"]))
                digest = hashlib.sha256(after[asset_name]).hexdigest()
                self.assertEqual(
                    after[f"{asset_name}.sha256"], expected_checksum_bytes(digest, asset_name)
                )

    def test_rejects_existing_directory_output_target(self):
        # Non-empty directory at archive or checksum must be refused, not moved
        # into the temp work dir (where success would rmtree its contents).
        for which in ("archive", "checksum"):
            with self.subTest(target=which):
                self.make_workspace()
                binary = self.fake_binary("oxid-build")
                asset_name = "oxid-linux-x86_64.tar.gz"
                dist = self.root / "dist"
                dist.mkdir()
                target = dist / (asset_name if which == "archive" else f"{asset_name}.sha256")
                target.mkdir()
                marker = target / "keep-me.txt"
                marker.write_bytes(b"preserve-directory-contents")
                result = self.run_script(binary, asset_name)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn("packaged", result.stdout)
                self.assertNotIn("packaged", result.stderr)
                self.assertIn("non-regular-file output target", result.stderr)
                self.assertIn(target.relative_to(self.root).as_posix(), result.stderr)
                self.assertTrue(target.is_dir() and not target.is_symlink())
                self.assertEqual(marker.read_bytes(), b"preserve-directory-contents")
                # No publish work directory should remain after the early reject.
                work_dirs = [
                    path for path in dist.iterdir()
                    if path.is_dir() and path.name != target.name
                ]
                self.assertEqual(work_dirs, [])

    def test_rejects_dangling_symlink_output_target(self):
        # Dangling symlink is invisible to Path.exists(); publish must not skip
        # backup then claim restore after a later failure. Reject up front.
        for which in ("archive", "checksum"):
            with self.subTest(target=which):
                self.make_workspace()
                binary = self.fake_binary("oxid-build")
                asset_name = "oxid-linux-x86_64.tar.gz"
                dist = self.root / "dist"
                dist.mkdir()
                target = dist / (asset_name if which == "archive" else f"{asset_name}.sha256")
                target.symlink_to("does-not-exist-anywhere")
                result = self.run_script(binary, asset_name)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn("packaged", result.stdout)
                self.assertNotIn("packaged", result.stderr)
                self.assertIn("non-regular-file output target", result.stderr)
                self.assertIn(target.relative_to(self.root).as_posix(), result.stderr)
                # Must not claim the previous object was restored after a publish fault.
                self.assertNotIn("put back", result.stderr)
                self.assertNotIn("rollback", result.stderr)
                self.assertTrue(target.is_symlink())
                self.assertEqual(os.readlink(target), "does-not-exist-anywhere")
                work_dirs = [path for path in dist.iterdir() if path.is_dir()]
                self.assertEqual(work_dirs, [])



if __name__ == "__main__":
    unittest.main()
