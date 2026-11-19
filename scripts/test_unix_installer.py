"""Exercise the Unix installer with local downloads and isolated destinations."""
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "install.sh"


@unittest.skipUnless(os.name == "posix", "Unix installer requires a POSIX shell")
class UnixInstallerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.mirror = self.root / "downloads"
        self.tools = self.root / "tools"
        self.destination = self.root / "installed"
        for directory in [self.mirror, self.tools, self.root / "temporary"]:
            directory.mkdir()
        self.payload = b"local ARM64 compiler image\x00\xff\n"
        self.asset = "oxid-linux-aarch64.tar.gz"
        with tarfile.open(self.mirror / self.asset, "w:gz") as archive:
            member = tarfile.TarInfo("oxid")
            member.size = len(self.payload)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(self.payload))
        digest = hashlib.sha256((self.mirror / self.asset).read_bytes()).hexdigest()
        (self.mirror / (self.asset + ".sha256")).write_text(f"{digest}  {self.asset}\n")
        self.tool("uname", "import os,sys\nprint('Linux' if sys.argv[1]=='-s' else os.environ['TEST_MACHINE'])\n")
        self.tool("curl", """import json,os,pathlib,shutil,sys
args=sys.argv[1:]
url=next(value for value in args if value.startswith('https://'))
with open(os.environ['TEST_REQUESTS'],'a') as stream:stream.write(json.dumps(url)+'\\n')
if os.environ.get('TEST_DOWNLOAD_FAILURE')=='1':raise SystemExit(22)
destination=pathlib.Path(args[args.index('--output')+1])
shutil.copyfile(pathlib.Path(os.environ['TEST_DOWNLOADS'])/url.rsplit('/',1)[1],destination)
""")
        self.environment = {
            **os.environ, "PATH": str(self.tools) + os.pathsep + os.environ["PATH"],
            "TMPDIR": str(self.root / "temporary"), "OXID_INSTALL_DIR": str(self.destination),
            "OXID_VERSION": "v0.9.0", "OXID_ARCHIVE": "", "TEST_MACHINE": "aarch64",
            "TEST_DOWNLOADS": str(self.mirror), "TEST_REQUESTS": str(self.root / "requests.jsonl"),
        }

    def tool(self, name, code):
        path = self.tools / name
        path.write_text("#!" + sys.executable + "\n" + code)
        path.chmod(0o755)

    def run_installer(self):
        return subprocess.run(["sh", SCRIPT], cwd=self.root, env=self.environment,
                              capture_output=True, timeout=15)

    def archive(self, payload):
        with tarfile.open(self.mirror / self.asset, "w:gz") as archive:
            member = tarfile.TarInfo("oxid")
            member.size = len(payload)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(payload))
        self.checksum()

    def checksum(self):
        digest = hashlib.sha256((self.mirror / self.asset).read_bytes()).hexdigest()
        (self.mirror / (self.asset + ".sha256")).write_text(f"{digest}  {self.asset}\n")

    def previous_executable(self):
        self.destination.mkdir()
        binary = self.destination / "oxid"
        binary.write_bytes(b"previous executable")
        binary.chmod(0o755)
        return binary

    def assert_clean(self):
        self.assertEqual(list((self.root / "temporary").iterdir()), [])
        if self.destination.exists():
            self.assertEqual(list(self.destination.glob(".oxid-install.*")), [])

    def test_linux_arm_aliases_install_the_checked_asset_for_pinned_and_latest_versions(self):
        for machine, version, url in [
            ("aarch64", "v0.9.0", "https://github.com/YanagiKH/Oxid/releases/download/v0.9.0/"),
            ("arm64", "latest", "https://github.com/YanagiKH/Oxid/releases/latest/download/"),
        ]:
            self.environment.update(TEST_MACHINE=machine, OXID_VERSION=version)
            result = self.run_installer()
            self.assertEqual(result.returncode, 0, result.stderr)
            binary = self.destination / "oxid"
            self.assertEqual(binary.read_bytes(), self.payload)
            self.assertTrue(os.access(binary, os.X_OK))
            requests = [json.loads(line) for line in (self.root / "requests.jsonl").read_text().splitlines()]
            self.assertEqual(requests[-2:], [url + self.asset, url + self.asset + ".sha256"])
            self.assertEqual(list((self.root / "temporary").iterdir()), [])

    def test_bad_checksum_preserves_the_previous_executable(self):
        self.destination.mkdir()
        binary = self.destination / "oxid"
        binary.write_bytes(b"previous executable")
        (self.mirror / (self.asset + ".sha256")).write_text("0" * 64 + f"  {self.asset}\n")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assertEqual(list((self.root / "temporary").iterdir()), [])

    def test_failed_download_preserves_the_previous_executable(self):
        self.destination.mkdir()
        binary = self.destination / "oxid"
        binary.write_bytes(b"previous executable")
        self.environment["TEST_DOWNLOAD_FAILURE"] = "1"
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assertEqual(list((self.root / "temporary").iterdir()), [])

    def test_offline_install_uses_local_archive_and_never_downloads(self):
        self.environment.update(OXID_ARCHIVE=str(self.mirror / self.asset), TEST_DOWNLOAD_FAILURE="1")
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.destination / "oxid").read_bytes(), self.payload)
        self.assertFalse((self.root / "requests.jsonl").exists())
        self.assert_clean()

    def test_offline_missing_or_corrupt_checksum_preserves_the_previous_executable(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.mirror / self.asset)
        checksum = self.mirror / (self.asset + ".sha256")
        for contents in [None, "invalid checksum\n", "0" * 64 + f"  {self.asset}\n"]:
            if contents is None:
                checksum.unlink()
            else:
                checksum.write_text(contents)
            result = self.run_installer()
            self.assertNotEqual(result.returncode, 0, contents)
            self.assertEqual(binary.read_bytes(), b"previous executable")
            self.assertFalse((self.root / "requests.jsonl").exists())
            self.assert_clean()

    def test_partial_install_failure_preserves_the_previous_executable(self):
        binary = self.previous_executable()
        self.tool("install", "import pathlib,sys\npathlib.Path(sys.argv[-1]).write_bytes(b'partial copy')\nraise SystemExit(17)\n")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assertTrue(os.access(binary, os.X_OK))
        self.assert_clean()

    def test_publish_failure_preserves_the_previous_executable(self):
        binary = self.previous_executable()
        self.tool("mv", "raise SystemExit(19)\n")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assert_clean()

    def test_checksum_for_a_different_file_cannot_authorize_the_archive(self):
        binary = self.previous_executable()
        (self.mirror / (self.asset + ".sha256")).write_text(hashlib.sha256(b"").hexdigest() + "  /dev/null\n")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assert_clean()

    def test_nonregular_destinations_are_preserved(self):
        self.destination.mkdir()
        binary = self.destination / "oxid"
        binary.mkdir()
        (binary / "keep").write_bytes(b"untouched")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(binary.iterdir()), [binary / "keep"])
        (binary / "keep").unlink()
        binary.rmdir()
        outside = self.root / "outside"
        outside.write_bytes(b"untouched")
        binary.symlink_to(outside)
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(binary.is_symlink())
        self.assertEqual(outside.read_bytes(), b"untouched")
        self.assert_clean()

    def test_archive_executable_must_be_a_regular_file(self):
        binary = self.previous_executable()
        outside = self.root / "outside"
        outside.write_bytes(b"not an archive executable")
        with tarfile.open(self.mirror / self.asset, "w:gz") as archive:
            member = tarfile.TarInfo("oxid")
            member.type = tarfile.SYMTYPE
            member.linkname = str(outside)
            archive.addfile(member)
        self.checksum()
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assert_clean()

    def test_pinned_installation_can_upgrade_and_restore_an_older_version(self):
        for version, payload in [("v0.8.0", b"older executable"), ("v0.9.0", self.payload), ("v0.8.0", b"older executable")]:
            self.archive(payload)
            self.environment["OXID_VERSION"] = version
            result = self.run_installer()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((self.destination / "oxid").read_bytes(), payload)
            requests = [json.loads(line) for line in (self.root / "requests.jsonl").read_text().splitlines()]
            self.assertTrue(all(f"/download/{version}/" in url for url in requests[-2:]))
            self.assert_clean()

    @unittest.skipUnless(shutil.which("shasum"), "shasum is not installed")
    def test_shasum_fallback_checks_the_archive_bytes(self):
        for name in ["sh", "awk", "tr", "cut", "cp", "mktemp", "rm", "tar", "gzip", "install", "mv", "mkdir", "shasum", "perl"]:
            executable = shutil.which(name)
            self.assertIsNotNone(executable, name)
            (self.tools / name).symlink_to(executable)
        self.environment["PATH"] = str(self.tools)
        self.environment["OXID_ARCHIVE"] = str(self.mirror / self.asset)
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        (self.mirror / self.asset).write_bytes(b"changed archive")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.destination / "oxid").read_bytes(), self.payload)
        self.assert_clean()


if __name__ == "__main__":
    unittest.main()
