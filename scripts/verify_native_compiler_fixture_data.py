"""Authenticate the standalone scalar compiler's subject inputs as data.

These inputs include deliberately rejected syntax. Authentication checks exact
bytes and the complete acceptance-case inventory; it does not compile or run
an input or qualify the compiler's behavior.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat


PROJECT = "compiler/native"
FIXTURE_ROOT = PROJECT + "/tests/fixtures"
FIXTURE_FILES = ('comments.ox',
 'control.ox',
 'forward.ox',
 'late_syntax_error.ox',
 'late_type_error.ox',
 'minimum.ox',
 'negative_division.ox',
 'scopes.ox',
 'short_circuit.ox',
 'unit.ox',
 'unreachable_error.ox')
FIXTURE_INPUTS = (('compiler/native/tests/fixtures/comments.ox',
  150,
  'e4515fdbeaf3ba9030443e527d7e2bf4ad7820a90b028854606589788eb2ffb6'),
 ('compiler/native/tests/fixtures/control.ox',
  328,
  '6ae5455626507a603ca8d61a56e612cbcd03e7cefedb26f0377976bc811f0935'),
 ('compiler/native/tests/fixtures/forward.ox',
  91,
  '61ce60ef27d417c0a7226558613a8bcfacdc382be7a0acc73699adaf502fa033'),
 ('compiler/native/tests/fixtures/late_syntax_error.ox',
  85,
  '68fdac24bebd494bd8e62a61499c57253770719065afae7d57d3f3ea4f634bad'),
 ('compiler/native/tests/fixtures/late_type_error.ox',
  93,
  'c7db1dd256de2c496e3432cfb500808d0dd52600c94ab28566619cba48dcdb99'),
 ('compiler/native/tests/fixtures/minimum.ox',
  41,
  '5715cd371a6bde7e26435728f45456980214b81bc1ce5f13f5dc2ed002d4fbc3'),
 ('compiler/native/tests/fixtures/negative_division.ox',
  54,
  '7ccacd24af5692e45f227b0ec6d5d0075db1604b510e17fff1cfb10417ad3afd'),
 ('compiler/native/tests/fixtures/scopes.ox',
  166,
  'e91db91f5f252ddd7bd9a08310c04cabc207024954e92416f00c0183cf6e2a4d'),
 ('compiler/native/tests/fixtures/short_circuit.ox',
  131,
  'a789a93f5f10dac69c67389e38ff0bd40f5acf95cb309cea361e9fec6134c990'),
 ('compiler/native/tests/fixtures/unit.ox',
  96,
  '3e651c48ad9db12bf7d44167e12158cce1e0adf98800a6c3ac2db8d88bd58880'),
 ('compiler/native/tests/fixtures/unreachable_error.ox',
  51,
  '1ec8b81b351357a5b5597aad6cfa9fd31e56d949d97bb3bf7d3548313107af11'))
CASES_JSON = ('compiler/native/tests/cases.json',
 5105,
 'eb5ecb4d3e85449779278787db3dc50971eada3fe64bbf650f50a75d88cc2cf8')
CASE_NAMES = ('forward',
 'control',
 'scopes',
 'short-circuit',
 'negative-division',
 'minimum',
 'unit',
 'comments',
 'boolean-false',
 'else-if',
 'late-type',
 'late-syntax',
 'unreachable-error',
 'immutable',
 'active-shadow',
 'escaped-scope',
 'function-shadow',
 'call-type',
 'call-arity',
 'condition-type',
 'break-outside',
 'missing-return',
 'positive-minimum-magnitude',
 'missing-main',
 'main-argument',
 'recursive',
 'indirect-recursive',
 'unsupported-array',
 'unsupported-import',
 'unterminated-comment',
 'overflow-add',
 'overflow-subtract',
 'overflow-multiply',
 'overflow-negate',
 'division-zero',
 'remainder-zero',
 'division-minimum',
 'remainder-minimum',
 'argument-order',
 'conditional-scopes',
 'basic-comments',
 'nested-if')


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def metadata(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_nlink,
            value.st_size, value.st_mtime_ns, value.st_ctime_ns)


def read_exact(root: Path, row) -> bytes:
    name, length, digest = row
    path = root / name

    def no_symlink():
        require(not any(parent.is_symlink() for parent in (path, *path.parents)),
                "symlink in scalar compiler fixture input: " + name)

    try:
        no_symlink()
        before = path.stat()
        require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                and not before.st_mode & 0o111 and before.st_size == length,
                "nonregular or changed scalar compiler fixture input: " + name)
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        with os.fdopen(descriptor, "rb") as stream:
            require(metadata(os.fstat(stream.fileno())) == metadata(before),
                    "scalar compiler fixture changed before read: " + name)
            body = stream.read(length + 1)
            after = os.fstat(stream.fileno())
        no_symlink()
        require(metadata(before) == metadata(after) == metadata(path.stat())
                and len(body) == length and hashlib.sha256(body).hexdigest() == digest,
                "scalar compiler fixture body identity mismatch: " + name)
        return body
    except OSError as error:
        raise RuntimeError("unreadable scalar compiler fixture input: " + str(error)) from error


def unique_object(pairs):
    require(len({key for key, _ in pairs}) == len(pairs), "duplicate scalar compiler case field")
    return dict(pairs)


def authenticated_inputs(root: Path, *, required: bool = False):
    require(isinstance(FIXTURE_FILES, tuple) and len(FIXTURE_FILES) == 11
            and all(type(name) is str and re.fullmatch(r"[a-z_]+\.ox", name) for name in FIXTURE_FILES),
            "invalid scalar compiler fixture filename inventory")
    expected = {FIXTURE_ROOT + "/" + name for name in FIXTURE_FILES}
    require(len(expected) == 11, "duplicate scalar compiler fixture filename")
    require(isinstance(FIXTURE_INPUTS, tuple) and len(FIXTURE_INPUTS) == 11
            and all(isinstance(row, tuple) and len(row) == 3 and type(row[0]) is str
                    and type(row[1]) is int and 0 < row[1] <= 65536 and type(row[2]) is str
                    and re.fullmatch(r"[0-9a-f]{64}", row[2]) for row in FIXTURE_INPUTS)
            and {row[0] for row in FIXTURE_INPUTS} == expected,
            "invalid exact scalar compiler fixture-data inventory")
    require(isinstance(CASES_JSON, tuple) and len(CASES_JSON) == 3
            and CASES_JSON[0] == PROJECT + "/tests/cases.json"
            and type(CASES_JSON[1]) is int and 0 < CASES_JSON[1] <= 65536
            and type(CASES_JSON[2]) is str and re.fullmatch(r"[0-9a-f]{64}", CASES_JSON[2]),
            "invalid scalar compiler case manifest identity")
    selected = (CASES_JSON, *FIXTURE_INPUTS)
    paths = [root / row[0] for row in selected]
    if (not required and not (root / PROJECT / "oxid.toml").exists()
            and not any(path.exists() or path.is_symlink() for path in paths)):
        return None
    return {row[0]: read_exact(root, row) for row in selected}


def parse_cases(inputs):
    try:
        cases = json.loads(inputs[CASES_JSON[0]].decode("utf-8"), object_pairs_hook=unique_object)
    except (ValueError, UnicodeError) as error:
        raise RuntimeError("invalid scalar compiler case manifest: " + str(error)) from error
    require(isinstance(cases, list) and len(cases) == 42
            and all(isinstance(case, dict) for case in cases),
            "scalar compiler acceptance requires all 42 cases")
    require(tuple(case.get("name") for case in cases) == CASE_NAMES
            and len(set(CASE_NAMES)) == 42,
            "scalar compiler acceptance case names or order changed")
    sources, referenced = {}, []
    for case in cases:
        name, kind = case["name"], case.get("kind")
        require(type(name) is str and re.fullmatch(r"[a-z][a-z0-9-]*", name)
                and type(case.get("reference")) is bool and kind in ("value", "reject", "runtime"),
                "invalid scalar compiler case role")
        fields = {"name", "kind", "reference"}
        fields.add("file" if "file" in case else "source")
        if kind == "value":
            fields.add("stdout")
            require(type(case.get("stdout")) is str, "missing scalar compiler value expectation")
        elif kind == "runtime":
            fields.add("error")
            require(type(case.get("error")) is str and bool(case["error"]),
                    "missing scalar compiler runtime failure expectation")
        require(set(case) == fields, "unexpected scalar compiler case fields")
        if "file" in case:
            require(type(case["file"]) is str and case["file"] in FIXTURE_FILES,
                    "unregistered scalar compiler subject input")
            referenced.append(case["file"])
            sources[name] = inputs[FIXTURE_ROOT + "/" + case["file"]]
        else:
            require(type(case["source"]) is str and bool(case["source"]),
                    "missing scalar compiler inline input")
            sources[name] = case["source"].encode("utf-8")
    require(len(referenced) == 11 and set(referenced) == set(FIXTURE_FILES),
            "scalar compiler file fixtures must each be exercised once")
    require({kind: sum(case["kind"] == kind for case in cases) for kind in ("value", "reject", "runtime")}
            == {"value": 13, "reject": 20, "runtime": 9},
            "scalar compiler positive, negative or runtime case census changed")
    return cases, sources


def load_cases(root: Path):
    """Capture every exact case and its source bytes before native acceptance."""
    return parse_cases(authenticated_inputs(root, required=True))


def fixture_data_sources(root: Path) -> set[Path]:
    inputs = authenticated_inputs(root)
    if inputs is None:
        return set()
    parse_cases(inputs)
    return {root / row[0] for row in FIXTURE_INPUTS}
