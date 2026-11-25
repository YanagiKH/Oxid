"""Regression tests for choosing an offline upgrade baseline."""
from pathlib import Path
import unittest
from unittest.mock import patch

import verify_release_install as installer


class PublishedUpgradeTests(unittest.TestCase):
    asset = "oxid-linux-x86_64.tar.gz"

    def test_same_version_and_newer_releases_are_never_baselines(self):
        for candidate, expected in (
            ("0.8.0", None),
            ("0.9.0-rc.1", "0.8.0"),
            ("0.9.0+local", "0.8.0"),
            ("0.10.0", "0.9.0"),
            ("1.0.0-rc.1", "0.9.0"),
        ):
            with self.subTest(candidate=candidate):
                self.assertEqual(installer.newest_published_previous(candidate, self.asset), expected)

    def test_archive_verification_receives_the_archive_and_published_baseline(self):
        archive = Path("dist") / self.asset
        previous = (Path("downloaded") / self.asset, "0.9.0")
        with (patch.object(installer, "host_asset", return_value=self.asset),
              patch.object(installer, "load_package_version", return_value="1.0.0-rc.1"),
              patch.object(installer, "fetch_previous", return_value=previous) as fetch,
              patch.object(installer, "verify") as verify):
            installer.verify_archive(archive, None, "auto")
        self.assertEqual(fetch.call_args.args[1:3], (self.asset, "0.9.0"))
        verify.assert_called_once_with(archive, previous)

    def test_inaugural_platform_checks_installation_without_fetching(self):
        asset = "oxid-linux-aarch64.tar.gz"
        archive = Path("dist") / asset
        with (patch.object(installer, "host_asset", return_value=asset),
              patch.object(installer, "load_package_version", return_value="1.0.0-rc.1"),
              patch.object(installer, "fetch_previous") as fetch,
              patch.object(installer, "verify") as verify,
              patch("sys.stdout")):
            installer.verify_archive(archive, None, "auto")
        fetch.assert_not_called()
        verify.assert_called_once_with(archive)


if __name__ == "__main__":
    unittest.main()
