"""Run the Windows installer in real PowerShell with isolated local archives."""
import hashlib
import os
from pathlib import Path
import platform
import shutil
import stat
import struct
import subprocess
import tempfile
import unittest
import warnings
import zipfile


SCRIPT = Path(__file__).resolve().parents[1] / "install.ps1"
POWERSHELL = os.environ.get("OXID_TEST_POWERSHELL") or shutil.which("pwsh") or shutil.which("powershell")


@unittest.skipUnless(POWERSHELL, "PowerShell is not installed")
@unittest.skipUnless(platform.machine().lower() in {"amd64", "x86_64"}, "Windows release requires x86_64")
class WindowsInstallerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.mirror = self.root / "downloads"
        self.destination = self.root / "installed"
        self.temporary = self.root / "temporary"
        self.requests = self.root / "requests.txt"
        self.mirror.mkdir()
        self.temporary.mkdir()
        self.asset = "oxid-windows-x86_64.zip"
        self.archive = self.mirror / self.asset
        self.payload = b"local compiler image\x00\xff\n"
        self.write_archive(self.payload)
        self.environment = {
            **os.environ, "OXID_INSTALL_DIR": str(self.destination), "OXID_VERSION": "v0.9.0",
            "OXID_ARCHIVE": "", "LOCALAPPDATA": str(self.root / "local-app-data"),
            "TMP": str(self.temporary), "TEMP": str(self.temporary), "TMPDIR": str(self.temporary),
            "TEST_DOWNLOADS": str(self.mirror), "TEST_REQUESTS": str(self.requests),
            "TEST_INSTALLER_SCRIPT": str(SCRIPT), "TEST_DOWNLOAD_FAILURE": "",
            "DOTNET_EnableDiagnostics": "0",
        }

    def write_archive(self, payload, entries=None):
        if entries is None:
            entry = zipfile.ZipInfo("oxid.exe")
            entry.external_attr = (stat.S_IFREG | 0o755) << 16
            entry.compress_type = zipfile.ZIP_DEFLATED
            entries = [(entry, payload)]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            with zipfile.ZipFile(self.archive, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                for entry, body in entries:
                    archive.writestr(entry, body)
        self.checksum()

    def checksum(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.archive.with_suffix(self.archive.suffix + ".sha256").write_text(f"{digest}  {self.asset}\n")

    def previous_executable(self):
        self.destination.mkdir(exist_ok=True)
        binary = self.destination / "oxid.exe"
        binary.write_bytes(b"previous executable")
        return binary

    def run_installer(self, parameters=None, prelude="", definitions_only=False):
        if definitions_only:
            parameters = {"Version": self.environment["OXID_VERSION"],
                          "InstallDir": self.environment["OXID_INSTALL_DIR"],
                          "Archive": self.environment["OXID_ARCHIVE"], **(parameters or {})}
        arguments = "@{" + "; ".join(
            key + "='" + str(value).replace("'", "''") + "'"
            for key, value in (parameters or {}).items()) + "}"
        load = "& $env:TEST_INSTALLER_SCRIPT @arguments"
        if definitions_only:
            load = """$tokens = $null; $parseErrors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile(
    $env:TEST_INSTALLER_SCRIPT, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count) { throw 'Invalid installer syntax.' }
$definitions = @($ast.EndBlock.Statements | Where-Object {
    $_ -is [System.Management.Automation.Language.FunctionDefinitionAst] })
if (-not $definitions.Count) { throw 'Missing installer functions.' }
foreach ($definition in $definitions) { . ([scriptblock]::Create($definition.Extent.Text)) }
""" + prelude + "\nInstall-OxidRelease @arguments"
            prelude = ""
        wrapper = self.root / "run-installer.ps1"
        wrapper.write_text("""$ErrorActionPreference = 'Stop'
function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile)
    [System.IO.File]::AppendAllText($env:TEST_REQUESTS, $Uri + [Environment]::NewLine)
    if ($env:TEST_DOWNLOAD_FAILURE -eq '1' -or
        ($env:TEST_DOWNLOAD_FAILURE -eq 'checksum' -and $Uri.EndsWith('.sha256'))) {
        throw 'Unexpected download request.'
    }
    $source = Join-Path $env:TEST_DOWNLOADS ($Uri.Split('/')[-1])
    [System.IO.File]::Copy($source, $OutFile, $false)
}
""" + prelude + "\n$arguments = " + arguments + "\n" + load + "\n", encoding="utf-8")
        return subprocess.run([POWERSHELL, "-NoLogo", "-NoProfile", "-NonInteractive", "-File", str(wrapper)],
                              cwd=self.root, env=self.environment, capture_output=True, timeout=30)

    def assert_clean(self):
        self.assertEqual(list(self.temporary.iterdir()), [])
        if self.destination.exists():
            self.assertEqual(list(self.destination.glob(".oxid-install-*")), [])

    def test_offline_archive_never_downloads(self):
        self.environment.update(OXID_ARCHIVE=str(self.archive), TEST_DOWNLOAD_FAILURE="1")
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual((self.destination / "oxid.exe").read_bytes(), self.payload)
        self.assertFalse(self.requests.exists())
        self.assert_clean()

    def test_explicit_parameters_override_environment_and_accept_literal_paths(self):
        destination = self.root / "installed [reviewed] with spaces"
        self.environment.update(OXID_ARCHIVE="missing.zip", TEST_DOWNLOAD_FAILURE="1")
        result = self.run_installer({"Archive": self.archive, "InstallDir": destination, "Version": "v0.8.0"})
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual((destination / "oxid.exe").read_bytes(), self.payload)
        self.assertFalse(self.destination.exists())
        self.assertFalse(self.requests.exists())
        self.assertEqual(list(destination.iterdir()), [destination / "oxid.exe"])
        self.assert_clean()

    def test_latest_and_default_install_directory_remain_available(self):
        self.environment.update(OXID_VERSION="", OXID_INSTALL_DIR="")
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        destination = Path(self.environment["LOCALAPPDATA"]) / "Oxid" / "bin" / "oxid.exe"
        self.assertEqual(destination.read_bytes(), self.payload)
        self.assertEqual(self.requests.read_text().splitlines(), [
            "https://github.com/YanagiKH/Oxid/releases/latest/download/" + self.asset,
            "https://github.com/YanagiKH/Oxid/releases/latest/download/" + self.asset + ".sha256"])
        self.assert_clean()

    def test_online_pinned_upgrade_and_rollback(self):
        for version, payload in [("v0.8.0", b"older compiler"), ("v0.9.0", self.payload),
                                 ("v0.8.0", b"older compiler")]:
            with self.subTest(version=version):
                self.write_archive(payload)
                result = self.run_installer({"Version": version})
                self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
                self.assertEqual((self.destination / "oxid.exe").read_bytes(), payload)
                base = "https://github.com/YanagiKH/Oxid/releases/download/" + version + "/"
                self.assertEqual(self.requests.read_text().splitlines()[-2:],
                                 [base + self.asset, base + self.asset + ".sha256"])
                self.assert_clean()

    def test_offline_upgrade_and_rollback(self):
        self.environment.update(OXID_ARCHIVE=str(self.archive), TEST_DOWNLOAD_FAILURE="1")
        for payload in [b"older compiler", self.payload, b"older compiler"]:
            self.write_archive(payload)
            result = self.run_installer()
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
            self.assertEqual((self.destination / "oxid.exe").read_bytes(), payload)
            self.assertFalse(self.requests.exists())
            self.assert_clean()

    def test_partial_copy_failure_preserves_previous_executable(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        self.environment["TEST_STAGE_LOG"] = str(self.root / "stage.txt")
        result = self.run_installer(definitions_only=True, prelude="""function Copy-OxidStream {
    param([System.IO.Stream]$Source, [System.IO.Stream]$Destination)
    [System.IO.File]::WriteAllText($env:TEST_STAGE_LOG, $Destination.Name)
    $Destination.Write([byte[]](112,97,114,116,105,97,108), 0, 7)
    throw 'Controlled partial installation copy failure.'
}""")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"Controlled partial installation copy failure.", result.stderr)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assertEqual(Path(Path(self.environment["TEST_STAGE_LOG"]).read_text()).parent.parent, self.destination)
        self.assert_clean()

    def test_failed_publication_preserves_previous_executable(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer(definitions_only=True, prelude="""function Move-OxidExecutable {
    param([string]$Source, [string]$Destination, [string]$Backup, [bool]$Replace)
    if (-not $Replace) { throw 'Expected an existing destination.' }
    if ([System.IO.File]::ReadAllBytes($Source).Length -eq 0) { throw 'Expected staged executable.' }
    throw 'Controlled publication failure.'
}""")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"Controlled publication failure.", result.stderr)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assert_clean()

    def test_first_install_does_not_overwrite_a_destination_that_appears_before_move(self):
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer(definitions_only=True, prelude="""$script:realMove = (Get-Command Move-OxidExecutable).ScriptBlock
function Move-OxidExecutable {
    param([string]$Source, [string]$Destination, [string]$Backup, [bool]$Replace)
    if ($Replace) { throw 'Expected a first installation.' }
    [System.IO.File]::WriteAllText($Destination, 'concurrent executable')
    & $script:realMove $Source $Destination $Backup $Replace
}""")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.destination / "oxid.exe").read_bytes(), b"concurrent executable")
        self.assert_clean()

    def test_partial_publication_restores_backup_when_destination_is_missing(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer(definitions_only=True, prelude="""$script:realMove = (Get-Command Move-OxidExecutable).ScriptBlock
$script:moveCalls = 0
function Move-OxidExecutable {
    param([string]$Source, [string]$Destination, [string]$Backup, [bool]$Replace)
    $script:moveCalls++
    if ($script:moveCalls -eq 1) {
        [System.IO.File]::Move($Destination, $Backup)
        throw 'Controlled partial publication failure.'
    }
    & $script:realMove $Source $Destination $Backup $Replace
}""")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"Controlled partial publication failure.", result.stderr)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assert_clean()

    def test_post_replace_error_restores_backup_over_new_executable(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer(definitions_only=True, prelude="""$script:realMove = (Get-Command Move-OxidExecutable).ScriptBlock
$script:moveCalls = 0
function Move-OxidExecutable {
    param([string]$Source, [string]$Destination, [string]$Backup, [bool]$Replace)
    $script:moveCalls++
    & $script:realMove $Source $Destination $Backup $Replace
    if ($script:moveCalls -eq 1) { throw 'Controlled post-replacement failure.' }
}""")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"Controlled post-replacement failure.", result.stderr)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assert_clean()

    def test_failed_restoration_retains_the_only_previous_copy(self):
        self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer(definitions_only=True, prelude="""function Move-OxidExecutable {
    param([string]$Source, [string]$Destination, [string]$Backup, [bool]$Replace)
    if ($Backup) {
        [System.IO.File]::Move($Destination, $Backup)
        throw 'Controlled partial publication failure.'
    }
    throw 'Controlled restoration failure.'
}""")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"automatic restoration failed", result.stderr)
        saved = list(self.destination.glob(".oxid-install-*/previous.exe"))
        self.assertEqual(len(saved), 1)
        self.assertEqual(saved[0].read_bytes(), b"previous executable")
        self.assertFalse((self.destination / "oxid.exe").exists())
        self.assertEqual(list(self.temporary.iterdir()), [])

    def test_missing_malformed_and_incorrect_checksums_preserve_previous_executable(self):
        binary = self.previous_executable()
        self.environment.update(OXID_ARCHIVE=str(self.archive), TEST_DOWNLOAD_FAILURE="1")
        checksum = self.archive.with_suffix(self.archive.suffix + ".sha256")
        for contents in [None, "", "invalid checksum\n", "a" * 63, "a" * 65, "g" * 64,
                         "0" * 64 + "  " + self.asset + "\n",
                         hashlib.sha256(b"").hexdigest() + "  other-file\n"]:
            with self.subTest(checksum=contents):
                if contents is None:
                    checksum.unlink()
                else:
                    checksum.write_text(contents)
                result = self.run_installer()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(binary.read_bytes(), b"previous executable")
                self.assertFalse(self.requests.exists())
                self.assert_clean()

    def test_uppercase_sha256_is_accepted_and_archive_bytes_are_verified(self):
        checksum = self.archive.with_suffix(self.archive.suffix + ".sha256")
        checksum.write_text(hashlib.sha256(self.archive.read_bytes()).hexdigest().upper() + "\n")
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual((self.destination / "oxid.exe").read_bytes(), self.payload)
        self.archive.write_bytes(self.archive.read_bytes() + b"corrupted after checksumming")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"Checksum verification failed.", result.stderr)
        self.assertEqual((self.destination / "oxid.exe").read_bytes(), self.payload)
        self.assert_clean()

    def test_missing_offline_archive_and_failed_downloads_preserve_previous_executable(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.root / "missing.zip")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.requests.exists())
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assert_clean()
        self.environment["OXID_ARCHIVE"] = ""
        for failure in ["1", "checksum"]:
            self.environment["TEST_DOWNLOAD_FAILURE"] = failure
            result = self.run_installer()
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(binary.read_bytes(), b"previous executable")
            self.assert_clean()

    def test_missing_nested_case_alias_and_duplicate_root_binaries_are_rejected(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        for entries in [[("README.txt", b"no executable")], [("bin/oxid.exe", self.payload)],
                        [("Oxid.exe", self.payload)], [("ox\u00adid.exe", self.payload)],
                        [("oxid.exe", self.payload), ("oxid.exe", b"duplicate")]]:
            with self.subTest(members=[entry[0] for entry in entries]):
                self.write_archive(self.payload, entries)
                result = self.run_installer()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b"exactly one root oxid.exe", result.stderr)
                self.assertEqual(binary.read_bytes(), b"previous executable")
                self.assert_clean()

    def test_zip_binary_must_be_regular_not_link_directory_or_device(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        for attributes in [(stat.S_IFLNK | 0o777) << 16, (stat.S_IFDIR | 0o755) << 16,
                           (stat.S_IFIFO | 0o600) << 16, 0x10, 0x400, 0x40]:
            with self.subTest(attributes=attributes):
                entry = zipfile.ZipInfo("oxid.exe")
                entry.external_attr = attributes
                self.write_archive(self.payload, [(entry, self.payload)])
                result = self.run_installer()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b"regular oxid.exe executable", result.stderr)
                self.assertEqual(binary.read_bytes(), b"previous executable")
                self.assert_clean()

    def test_windows_archive_attributes_are_accepted(self):
        entry = zipfile.ZipInfo("oxid.exe")
        entry.create_system = 0
        entry.external_attr = 0x20
        self.write_archive(self.payload, [(entry, self.payload)])
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual((self.destination / "oxid.exe").read_bytes(), self.payload)
        self.assert_clean()

    def test_only_the_exact_binary_is_written_even_with_traversal_members(self):
        outside = self.root / "outside.exe"
        outside.write_bytes(b"untouched")
        self.write_archive(self.payload, [("oxid.exe", self.payload), ("../outside.exe", b"changed"),
                                         ("../../escaped.exe", b"changed"), (str(outside), b"changed"),
                                         ("..\\outside.exe", b"changed"),
                                         ("ox\u00adid.exe", b"wrong compiler"),
                                         ("docs/readme.txt", b"ignored")])
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual(outside.read_bytes(), b"untouched")
        self.assertEqual(list(self.destination.iterdir()), [self.destination / "oxid.exe"])
        self.assertEqual((self.destination / "oxid.exe").read_bytes(), self.payload)
        self.assertFalse((self.root / "escaped.exe").exists())
        self.assert_clean()

    def test_truncated_zip_and_incorrect_uncompressed_length_preserve_previous_executable(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        original = self.archive.read_bytes()
        wrong_length = bytearray(original)
        central = wrong_length.index(b"PK\x01\x02")
        local = wrong_length.index(b"PK\x03\x04")
        struct.pack_into("<I", wrong_length, central + 24, len(self.payload) + 100)
        struct.pack_into("<I", wrong_length, local + 22, len(self.payload) + 100)
        for contents in [original[:-16], bytes(wrong_length)]:
            with self.subTest(truncated=contents == original[:-16]):
                self.archive.write_bytes(contents)
                self.checksum()
                result = self.run_installer()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(binary.read_bytes(), b"previous executable")
                self.assert_clean()

    def test_nonregular_destination_directory_is_untouched_before_download(self):
        binary = self.destination / "oxid.exe"
        binary.mkdir(parents=True)
        keep = binary / "keep.txt"
        keep.write_bytes(b"untouched")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"destination must be a regular file", result.stderr)
        self.assertEqual(keep.read_bytes(), b"untouched")
        self.assertEqual(list(binary.iterdir()), [keep])
        self.assertFalse(self.requests.exists())
        self.assert_clean()

    def test_destination_links_including_dangling_links_are_untouched(self):
        self.destination.mkdir()
        binary = self.destination / "oxid.exe"
        outside = self.root / "outside.exe"
        outside.write_bytes(b"untouched")
        for target in [outside, self.root / "absent.exe"]:
            with self.subTest(target=target.name):
                try:
                    binary.symlink_to(target)
                except (OSError, NotImplementedError) as error:
                    self.skipTest(f"Symbolic links are not available: {error}")
                result = self.run_installer()
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(binary.is_symlink())
                self.assertEqual(os.readlink(binary), str(target))
                self.assertEqual(outside.read_bytes(), b"untouched")
                self.assertFalse(self.requests.exists())
                self.assert_clean()
                binary.unlink()

    def test_cleanup_preserves_preexisting_files_with_installer_like_names(self):
        existing = self.destination / ".oxid-install-keep"
        existing.mkdir(parents=True)
        saved = existing / "previous.exe"
        saved.write_bytes(b"user saved compiler")
        old_temp = self.temporary / "oxid-install-keep"
        old_temp.mkdir()
        (old_temp / "keep.txt").write_bytes(b"user file")
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual(saved.read_bytes(), b"user saved compiler")
        self.assertCountEqual(list(self.destination.iterdir()), [existing, self.destination / "oxid.exe"])
        self.assertEqual(list(self.temporary.iterdir()), [old_temp])
        self.assertEqual((old_temp / "keep.txt").read_bytes(), b"user file")

    def test_backup_cleanup_failure_does_not_fail_a_successful_update(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        result = self.run_installer(prelude="""function Remove-Item {
    [CmdletBinding()]
    param([string]$LiteralPath, [switch]$Recurse, [switch]$Force)
    if ([System.IO.Path]::GetFileName($LiteralPath).StartsWith('.oxid-install-')) {
        throw 'Controlled backup cleanup failure.'
    }
    Microsoft.PowerShell.Management\\Remove-Item @PSBoundParameters
}""")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual(binary.read_bytes(), self.payload)
        saved = list(self.destination.glob(".oxid-install-*/previous.exe"))
        self.assertEqual(len(saved), 1)
        self.assertEqual(saved[0].read_bytes(), b"previous executable")
        self.assertEqual(list(self.temporary.iterdir()), [])

    @unittest.skipUnless(os.name == "nt", "Windows sharing violations require a Windows filesystem")
    def test_windows_locked_executable_preserves_previous_version(self):
        binary = self.previous_executable()
        self.environment["OXID_ARCHIVE"] = str(self.archive)
        self.environment["TEST_LOCKED_BINARY"] = str(binary)
        result = self.run_installer(prelude="""$locked = [System.IO.File]::Open(
    $env:TEST_LOCKED_BINARY, [System.IO.FileMode]::Open,
    [System.IO.FileAccess]::Read, [System.IO.FileShare]::None)
""")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(binary.read_bytes(), b"previous executable")
        self.assert_clean()

    @unittest.skipUnless(os.name == "nt", "Junctions require Windows")
    def test_windows_destination_junction_is_preserved(self):
        self.destination.mkdir()
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "keep.txt").write_bytes(b"untouched")
        binary = self.destination / "oxid.exe"
        self.environment.update(TEST_JUNCTION=str(binary), TEST_JUNCTION_TARGET=str(outside))
        result = self.run_installer(prelude="""New-Item -ItemType Junction -Path $env:TEST_JUNCTION -Target $env:TEST_JUNCTION_TARGET | Out-Null""")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(binary.is_dir())
        self.assertEqual((outside / "keep.txt").read_bytes(), b"untouched")
        self.assertFalse(self.requests.exists())
        self.assert_clean()


if __name__ == "__main__":
    unittest.main()
