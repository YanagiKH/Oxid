"""Bounded pure byte derivation for exact current-source observer overlays."""
import hashlib
import importlib.util
import json
from pathlib import Path

PACKAGE = Path(__file__).resolve().parent
ROOT = PACKAGE.parents[2]
IDENTITY_SHA = '6c8bf1dd92e9975c18f77fe492ff1b98d8ca09752f9bbcd6f284f9366d1c7709'
raw = (PACKAGE / 'identity.json').read_bytes()
if hashlib.sha256(raw).hexdigest() != IDENTITY_SHA:
    raise ValueError('changed projected observer identity')
IDENTITY = json.loads(raw)
spec = importlib.util.spec_from_file_location('_projected_observer_binding', ROOT / 'tests/fixtures/typed_project_source_binding/run.py')
binding = importlib.util.module_from_spec(spec)
spec.loader.exec_module(binding)


def check(values, expected):
    binding.require(set(values) == set(expected), 'observer input membership differs')
    for path, identity in expected.items():
        data = values[path]
        binding.require(type(data) is bytes and len(data) == identity['bytes']
                        and binding.digest(data) == identity['sha256'], 'observer input identity differs: '+path)


def transform(values, reverse=False):
    before, after = ('derived', 'originals') if reverse else ('originals', 'derived')
    check(values, IDENTITY[before])
    name = 'reverse.patch' if reverse else 'derive.patch'
    patch = (PACKAGE / name).read_bytes()
    identity = IDENTITY['patches'][name]
    result, touched = binding.apply_inverse_patch(values, patch, identity['sha256'],
                                                  identity['bytes'], tuple(IDENTITY['paths']))
    check(result, IDENTITY[after])
    binding.require(touched == IDENTITY['paths'], 'observer derivation scope differs')
    return result


def derive(values):
    result = transform(values)
    binding.require(transform(result, reverse=True) == values, 'observer reverse differs')
    return result


def reverse(values):
    return transform(values, reverse=True)
