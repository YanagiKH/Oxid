"""Authenticate owned-record vector acceptance DATA without running its verifier."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat


INPUTS = {
    "scripts/owned_record_vector_acceptance/aliases.ox": (
        458, "9748b20b8833fbe37467d9307d3aa40f6b30c050252d8c5b7380e375961d14b1"),
    "scripts/owned_record_vector_acceptance/bytebuf.ox": (
        1118, "2d78e1dfc18d8ffe0f9284fdc546780bba1546bc3e4a0d298534d038ba756ad2"),
    "scripts/owned_record_vector_acceptance/conflicting_borrows.ox": (
        268, "30f4fee28d72f1beeb6848d560c5180e3521192f4b87b31356d15d7e03fc4895"),
    "scripts/owned_record_vector_acceptance/copy_operation.ox": (
        157, "00ee679a01d735137006b3e555e8a779ffedcb337762fbe7382e62b456a2bffd"),
    "scripts/owned_record_vector_acceptance/double_move.ox": (
        201, "4cb6c23c13e2d1ccbac021ed61148b9f4be7c0ce30e2bf87a7cdfe124a65fa2f"),
    "scripts/owned_record_vector_acceptance/empty_new.ox": (
        131, "6e0da99b3872027dcc3fa018b384dea445eeb4e3276d36d4f7c0f964e4fefae0"),
    "scripts/owned_record_vector_acceptance/enum16.ox": (
        1746, "34a810415ec8697d9b74ff58f39e3c0787cabb33970bf3eed7e64a9e5472a715"),
    "scripts/owned_record_vector_acceptance/failed_reserve.ox": (
        2032, "e29e0632b6620960bf2dd456539962477058a24434ca8c873a965aeda2e14bb2"),
    "scripts/owned_record_vector_acceptance/fault_pop.ox": (
        557, "ee01e3fc85acaf131d8361ac6fdecf5d31a102aa7e7b3fdab304865d7c089dde"),
    "scripts/owned_record_vector_acceptance/fault_push_file.ox": (
        597, "02476b8f4b08eb3b8158eff2fa9f50d1066f3580f541e9b504cc47b9620df0f0"),
    "scripts/owned_record_vector_acceptance/files.ox": (
        1667, "d388a44d534ab7cc491660edffb52f076ac7d2123c15562c05d46d52cd068ed4"),
    "scripts/owned_record_vector_acceptance/generic_record.ox": (
        520, "aa36dcf91fbd8276c2deb80f111acca89936114edad68a8f806161ad25442179"),
    "scripts/owned_record_vector_acceptance/index_read.ox": (
        178, "d4588d3a2a57394ebc7cf1bbdbff3d1cf81a6b11e5c6e2b9522a99ccb22393f7"),
    "scripts/owned_record_vector_acceptance/nested_vector.ox": (
        152, "57d358f03987e8191c6f3d8f664c6367bdc88255107b4defcc45b8635e1132be"),
    "scripts/owned_record_vector_acceptance/package_main.ox": (
        651, "db347c29a81d6cfc1070d8b5098e8796d814f93ce21cdde676759e8cce6604c5"),
    "scripts/owned_record_vector_acceptance/package_model.ox": (
        181, "c6886446bbea07bb5c903d3edbabd06aab7458105da0a019033b1afa5a8e96b7"),
    "scripts/owned_record_vector_acceptance/unused_pop.ox": (
        57, "4cf6f51abcf1cccabc90cdb8f0d3051fa96a344ec0f42eef91f5d12226fe1d5a"),
    "scripts/owned_record_vector_acceptance/unused_push.ox": (
        62, "26f17eb517ae7f6cc746eccc625f37e3caf0cc5f3ae4fb92309960ed2ed0eb2c"),
    "scripts/owned_record_vector_acceptance/unused_template.ox": (
        173, "0a993078768ad4d20481c593de44478c39316092cb2c504f66868af4c485dddf"),
    "scripts/owned_record_vector_acceptance/zero_key.ox": (
        165, "a13ac531f3558bc1ec43489f3e95547210828351a55c7fdca06a752400fc4500"),
}


def _no_symlink(path: Path) -> None:
    if any(parent.is_symlink() for parent in (path, *path.parents)):
        raise RuntimeError("symlink in owned-record vector fixture-data path: " + str(path))


def _metadata(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_nlink,
            value.st_size, value.st_mtime_ns, value.st_ctime_ns)


def fixture_data_sources(root: Path) -> set[Path]:
    """Return only the twenty pinned files; neighbors remain language inputs."""
    sources = set()
    try:
        for relative, (length, digest) in INPUTS.items():
            path = root / relative
            _no_symlink(path)
            before = path.stat()
            if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
                    or before.st_mode & 0o111 or before.st_size != length):
                raise RuntimeError("nonregular or changed owned-record vector fixture-data input: " + relative)
            descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
            with os.fdopen(descriptor, "rb") as stream:
                if _metadata(os.fstat(stream.fileno())) != _metadata(before):
                    raise RuntimeError("owned-record vector fixture-data changed before read: " + relative)
                body = stream.read(length + 1)
                after = os.fstat(stream.fileno())
            _no_symlink(path)
            if (_metadata(before) != _metadata(after) or _metadata(path.stat()) != _metadata(before)
                    or len(body) != length or hashlib.sha256(body).hexdigest() != digest):
                raise RuntimeError("owned-record vector fixture-data body identity mismatch: " + relative)
            sources.add(path)
        return sources
    except OSError as error:
        raise RuntimeError("unreadable owned-record vector fixture-data input: " + str(error)) from error
