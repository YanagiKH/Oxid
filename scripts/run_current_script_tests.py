#!/usr/bin/env python3
"""Run current script tests, with exact source-pinned suites replayed separately.

The delegated modules assert exact historical compiler/workflow identities. They
remain mandatory, unchanged, in historical-qualification.yml at the revision
below. Their historical results are not evidence for the current compiler.
Every other top-level scripts/test_*.py module, including future additions, runs
here. Import failures and test failures are ordinary failing results.
"""
import hashlib
from pathlib import Path
import sys
import unittest


HISTORICAL_REVISION = "9ff8ef90a26f003f4616838dfc186fa3bfb6845c"
# SHA-256 of each complete file from git show HISTORICAL_REVISION:scripts/NAME.py.
# A changed module cannot be delegated to a checkout that only tests old bytes.
HISTORICAL_ONLY = {
    "test_bounded_byte_storage": (
        "459c88202ada5bbd4bcb7a54ca13a54092d8f9ab91e9e2addae0d70eeb1ec71d",
        "binds the frozen byte-storage source registry and pre-extension enum exclusions"),
    "test_lexer_current_ci": (
        "ed412ef926631af68d17b6a339598bb34df13ac5b48275b4a264c58f6922931c",
        "asserts the frozen compiler and CI source descriptors"),
    "test_lexer_reservation_adapters_current": (
        "aa06030158d030e1c996f677363f9e29a95d6cfd048ae9967d6248366fc49a98",
        "adapts observers for the frozen source roster"),
    "test_lexer_reservation_source_current": (
        "2a2cfe16518034fd4576aea4c6867311f573befe8f0000a46b15f49bba4500ce",
        "admits the exact frozen source before its retained lexer controls"),
    "test_owned_array_native": (
        "d2a43331c08bf40bbdc55e14d3da6d3a9dcb07179b9704a7a873274b45ab43e8",
        "authenticates the frozen native suite and scheduling bindings"),
    "test_package_json_source": (
        "0add901c9c1542ba81d6579958eb595faf4506013598fbdf01a3524a3190e822",
        "checks the frozen package-JSON source and accounting bindings"),
    "test_projected_observer_adapters": (
        "436407ccb95150a444459b2818e2f908c13f5a34414c36258ced66053c2c669f",
        "adapts projected-slice observers for the frozen source roster"),
    "test_published_formatter_source": (
        "81c46045c946e6a6c9ef1e5defc5072c13bf7d3b3da5f853408ae0f55171a6ba",
        "admits the exact frozen source before its retained hosted formatter controls"),
    "test_record_composition_current_contracts": (
        "60268c1115b22a8f1715acc362c6cf41383563361a3111aebb05e2d4ea5028ac",
        "reconstructs historical projected inputs after exact frozen-source admission"),
    "test_record_composition_observer_adapters": (
        "de16f5a5c3a5436122edae93387eceef4d1848dc40e83b4155f78ae29a094c52",
        "admits the frozen source before reconstructing composition observer inputs"),
    "test_record_composition_runtime_view": (
        "4462a91036c8bb080be53d31a5111d6fde4025da93c1da39f8060af1838a6f2e",
        "admits the frozen source before retained runtime and parser amendment controls"),
    "test_typed_project_source_binding": (
        "52eb836b04aa3b7cf737f77cf0544c52d8f4db967ed9b4d2e2756e7efb707be0",
        "requires the frozen source identity before its predecessor binding mutation suite"),
}

# These complete source-pinned modules test policies on their exact parent
# compilers. ci.yml replays them unchanged in separate required jobs.
PREDECESSOR_ONLY = {
    "test_windows_native_compatibility": (
        "aacdb546d5e7fb7a69551316fd5135eeb08b04944a27e526a660d49217c158f4",
        "8180b1bbcbcf9ccd4d648ff5ba0ec9ea3d5b971f",
        "requires the pre-record-field editor source admission"),
    "test_copy_record_debug_compatibility": (
        "8fb0a586a790371179f98ffdba89090d5e5d092d0a14dd5a91aa951d0130b549",
        "9a5ecaaeb5fa50aff2296bf990d63167143cf312",
        "requires the pre-Windows-native source admission and default-emission scope"),
    "test_qualified_completion_compatibility": (
        "b46c1953e8f66816a405d612f36fdedd44737831a5d9b5ef080f4851ab111d3a",
        "20ea8ba102975d2e8ac0c667b96ce835a1bc6ef9",
        "requires the pre-debug ordinary type facade identity"),
    "test_checked_copy_compatibility": (
        "e16894e15eed4bbe27e32370c9e29bdffb309680b4eb5b94f42aafea8ecb2ae2",
        "6c3161640dbcc36a484ebdd23eb2594ccde4e16a",
        "requires the pre-Windows protected dispatch activation"),
    "test_typed_vector_compatibility": (
        "87b834d9c64d369716843cd1428aea54d940c94f50f5c4c00033c031a1f0ce1b",
        "6c3161640dbcc36a484ebdd23eb2594ccde4e16a",
        "requires the pre-Windows protected dispatch activation"),
    "test_arm64_host_compatibility": (
        "91b9cb1ae16da868ee873a36f6efef8ef93745bfed3b8684340c828cf77421a1",
        "18314e95d6afa3d8a4da7970dbddc828f4f6cbac",
        "binds the pre-documentation module owners and ARM64 test-only correction scope"),
    "test_byte_string_compatibility": (
        'd60fd64d3a8116e191727e84194729063d1068b4da17380a3e3b6ee4952c31d8',
        "22d04a015a444edcb600a6011cdb0e2c8529cb34",
        "asserts the complete compiler source identity before native files"),
    "test_buffer_transfer_compatibility": (
        "483fa4180a3b409eab07818800f378a96a6eddecaf13b94c0ad5cef214252512",
        "c45d519127d314f522762c35879c0c485994ea42",
        "asserts the complete compiler source identity before byte string literals"),
    "test_byte_buffer_compatibility": (
        "6b0213e3f6ce75d3ea4fed2566d1b4eae0ece002db2c95cdff7dea4c41da669c",
        "2e3fe21f76d0027d2b24cb5c7f02ba38ad354809",
        "asserts the complete pre-hosted compiler source identity"),
    "test_hosted_execution_compatibility": (
        "67a0aff2a4a77e0b8a202871492841f09f0c164bf27944bd012b35b1f44adaaf",
        "559a6e6aa0b2ab2022d8fa37546ac62aeac9954e",
        "asserts the complete compiler source identity before buffer transfer"),
}


def selected_modules(directory):
    """Select by exact module name; no prefix, failure, or exception filtering."""
    discovered = sorted(Path(directory).glob("test_*.py"))
    pinned = HISTORICAL_ONLY.keys() | PREDECESSOR_ONLY.keys()
    selected = [path for path in discovered if path.stem not in pinned]
    delegated = [path for path in discovered if path.stem in pinned]
    return selected, delegated


class CurrentTestLoader(unittest.TestLoader):
    """Filter exact historical files before importing, preserving discovery hooks."""

    def __init__(self, directory):
        super().__init__()
        self.directory = Path(directory).resolve()

    def _match_path(self, path, full_path, pattern):
        candidate = Path(full_path)
        if (candidate.parent.resolve() == self.directory
                and candidate.stem in HISTORICAL_ONLY.keys() | PREDECESSOR_ONLY.keys()):
            return False
        return super()._match_path(path, full_path, pattern)


def main():
    if len(sys.argv) != 1:
        print("usage: python3 -B scripts/run_current_script_tests.py", file=sys.stderr)
        return 2
    directory = Path(__file__).resolve().parent
    selected, delegated = selected_modules(directory)
    print(f"Current script modules: {len(selected)}", file=sys.stderr)
    pinned = {**HISTORICAL_ONLY, **{name: (row[0], row[2]) for name, row in PREDECESSOR_ONLY.items()}}
    for name, (expected, _) in pinned.items():
        path = directory / (name + ".py")
        try:
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError:
            actual = None
        if actual != expected:
            print(
                f"error: cannot delegate changed or missing historical module {path.name}; "
                "retain the frozen file and migrate new tests to a current test module",
                file=sys.stderr,
            )
            return 1
    for path in delegated:
        if path.stem in PREDECESSOR_ONLY:
            _, revision, reason = PREDECESSOR_ONLY[path.stem]
            print(f"Mandatory predecessor replay: {path.name} @ {revision}: {reason}", file=sys.stderr)
            continue
        print(
            f"Mandatory historical replay: {path.name} @ {HISTORICAL_REVISION}: "
            f"{HISTORICAL_ONLY[path.stem][1]}",
            file=sys.stderr,
        )
    if not selected:
        print("error: no current script test modules discovered", file=sys.stderr)
        return 1

    loader = CurrentTestLoader(directory)
    # One standard discovery pass preserves the broad pattern passed into
    # load_tests hooks, including hooks that discover additional nested suites.
    suite = loader.discover(
        str(directory), pattern="test_*.py", top_level_dir=str(directory)
    )
    if suite.countTestCases() == 0:
        print("error: current script modules contain no tests", file=sys.stderr)
        return 1
    return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
