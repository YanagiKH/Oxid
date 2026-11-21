"""Admission and failure handling at the external provenance verifier boundary."""
from contextlib import redirect_stderr
import io
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_release_provenance as verifier


class ReleaseProvenanceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.archive = self.root / "oxid-linux-aarch64.tar.gz"
        self.archive.write_bytes(b"synthetic archive")
        self.bundle = self.archive.with_name(self.archive.name + ".sigstore.json")
        self.bundle.write_text("{}\n")
        self.args = [str(self.archive), "--source-ref", "refs/tags/v0.9.0", "--commit", "a" * 40]

    def invoke(self, args):
        with redirect_stderr(io.StringIO()):
            return verifier.main(args)

    def test_incomplete_source_identity_stops_before_verifier(self):
        with patch.object(verifier.subprocess, "run") as run:
            for ref, commit in [("v0.9.0", "a" * 40), ("refs/tags/", "a" * 40),
                                ("refs/tags/v0.9.0\n", "a" * 40), ("refs/heads/main", "abc"),
                                ("refs/tags/v0.9.0", "z" * 40)]:
                with self.subTest(ref=ref, commit=commit):
                    self.assertEqual(self.invoke([str(self.archive), "--source-ref", ref, "--commit", commit]), 1)
                    run.assert_not_called()

    def test_missing_archive_or_bundle_is_an_error(self):
        with patch.object(verifier.subprocess, "run") as run:
            self.archive.unlink()
            self.assertEqual(self.invoke(self.args), 1)
            self.archive.write_bytes(b"synthetic archive")
            self.bundle.unlink()
            self.assertEqual(self.invoke(self.args), 1)
            self.bundle.mkdir()
            self.assertEqual(self.invoke(self.args), 1)
            run.assert_not_called()

    def test_missing_offline_trust_root_is_an_error(self):
        with patch.object(verifier.subprocess, "run") as run:
            self.assertEqual(self.invoke(self.args + ["--trusted-root", str(self.root / "missing")]), 1)
            run.assert_not_called()

    def test_verifier_failure_timeout_or_missing_executable_cannot_pass(self):
        with patch.object(verifier.subprocess, "run", return_value=subprocess.CompletedProcess([], 7)):
            self.assertEqual(self.invoke(self.args), 1)
        for error in [FileNotFoundError("gh not installed"), subprocess.TimeoutExpired("gh", 300)]:
            with self.subTest(error=type(error).__name__):
                with patch.object(verifier.subprocess, "run", side_effect=error):
                    self.assertEqual(self.invoke(self.args), 1)

    def receipt(self, name=None, digest=None):
        return json.dumps([{"verificationResult": {"statement": {
            "_type": "https://in-toto.io/Statement/v1",
            "predicateType": "https://slsa.dev/provenance/v1",
            "subject": [{"name": name or self.archive.name,
                         "digest": {"sha256": digest or hashlib.sha256(self.archive.read_bytes()).hexdigest()}}],
        }}}])

    def test_verified_statement_must_identify_archive_name_and_bytes(self):
        with patch.object(verifier.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, self.receipt(), "")):
            with patch("sys.stdout", new_callable=io.StringIO):
                self.assertEqual(self.invoke(self.args), 0)
        for raw in [self.receipt(name="oxid-linux-x86_64.tar.gz"), self.receipt(digest="0" * 64),
                    "[]", "{}", "not json", json.dumps([{"attestation": {"statement": {}}}])]:
            with self.subTest(receipt=raw):
                with patch.object(verifier.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, raw, "")):
                    self.assertEqual(self.invoke(self.args), 1)

    def test_archive_mutation_during_verification_is_an_error(self):
        signed = self.receipt()

        def verify(*args, **kwargs):
            self.archive.write_bytes(b"replaced archive")
            return subprocess.CompletedProcess([], 0, signed, "")

        with patch.object(verifier.subprocess, "run", side_effect=verify):
            self.assertEqual(self.invoke(self.args), 1)

    def distribution(self):
        for name in verifier.ASSETS:
            path = self.root / name
            path.write_bytes(name.encode())
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            newline = "\r\n" if name.endswith(".zip") else "\n"
            path.with_name(name + ".sha256").write_bytes(f"{digest}  {name}{newline}".encode())
            path.with_name(name + ".sigstore.json").write_text("{}\n")
        return ["--distribution", str(self.root), *self.args[1:]]

    def verified_download(self, command, **kwargs):
        archive = Path(command[3])
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        data = self.receipt(name=archive.name, digest=digest)
        return subprocess.CompletedProcess([], 0, data, "")

    def test_every_downloaded_platform_is_checked_against_the_tag_and_commit(self):
        args = self.distribution()
        with patch.object(verifier.subprocess, "run", side_effect=self.verified_download) as run:
            with patch("sys.stdout", new_callable=io.StringIO):
                self.assertEqual(self.invoke(args), 0)
        seen = set()
        for call in run.call_args_list:
            command = call.args[0]
            archive = Path(command[3])
            seen.add(archive.name)
            self.assertEqual(command[command.index("--source-ref") + 1], "refs/tags/v0.9.0")
            self.assertEqual(command[command.index("--source-digest") + 1], "a" * 40)
            self.assertEqual(Path(command[command.index("--bundle") + 1]),
                             archive.with_name(archive.name + ".sigstore.json"))
        self.assertEqual(run.call_count, len(verifier.ASSETS))
        self.assertEqual(seen, verifier.ASSETS)

    def test_incomplete_unexpected_or_mismatched_downloads_are_rejected_before_verification(self):
        args = self.distribution()
        original = self.bundle.read_bytes()
        with patch.object(verifier.subprocess, "run") as run:
            self.bundle.unlink()
            self.assertEqual(self.invoke(args), 1)
            self.bundle.write_bytes(original)
            extra = self.root / "unrecognized.zip"
            extra.touch()
            self.assertEqual(self.invoke(args), 1)
            extra.unlink()
            self.archive.with_name(self.archive.name + ".sha256").write_text("0" * 64 + "\n")
            self.assertEqual(self.invoke(args), 1)
            run.assert_not_called()

    def test_downloads_cannot_change_during_provenance_checks(self):
        args = self.distribution()

        def verify(command, **kwargs):
            self.bundle.write_text("{\"changed\":true}\n")
            return self.verified_download(command, **kwargs)

        with patch.object(verifier.subprocess, "run", side_effect=verify):
            self.assertEqual(self.invoke(args), 1)

    def test_publication_requires_verification_after_download_and_before_upload(self):
        workflow = Path(verifier.__file__).resolve().parents[1] / ".github/workflows/release.yml"
        publish = workflow.read_text().split("\n  publish:\n", 1)[1]
        check = (
            "      - name: Verify downloaded release archives before publishing\n"
            "        env:\n"
            "          GH_TOKEN: ${{ github.token }}\n"
            "        run: python -B scripts/verify_release_provenance.py --distribution dist\n"
        )
        self.assertEqual(publish.count(check), 1)
        self.assertLess(publish.index("uses: actions/download-artifact@"), publish.index(check))
        self.assertLess(publish.index(check), publish.index("uses: softprops/action-gh-release@"))


if __name__ == "__main__":
    unittest.main()
