"""Exercise the Unix installer with local downloads and isolated destinations."""
import hashlib
import io
import json
import os
from pathlib import Path
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
            "OXID_VERSION": "v0.9.0", "TEST_MACHINE": "aarch64",
            "TEST_DOWNLOADS": str(self.mirror), "TEST_REQUESTS": str(self.root / "requests.jsonl"),
        }

    def tool(self, name, code):
        path = self.tools / name
        path.write_text("#!" + sys.executable + "\n" + code)
        path.chmod(0o755)

    def run_installer(self):
        return subprocess.run(["sh", SCRIPT], cwd=self.root, env=self.environment,
                              capture_output=True, timeout=15)

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


if __name__ == "__main__":
    unittest.main()
