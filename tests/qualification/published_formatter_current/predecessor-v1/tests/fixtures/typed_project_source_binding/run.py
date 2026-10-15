#!/usr/bin/env python3
"""Explicit outer lexer-source dispatcher; immutable historical chain retained."""
from pathlib import Path as _Path
import hashlib as _hashlib
import sys as _sys
import types as _types

_sys.dont_write_bytecode = True
_PACKAGE = _Path(__file__).resolve().parent
_HELPERS = _PACKAGE.parents[1] / 'qualification' / 'lexer_reservation_current'
_HELPER_PINS = {'adapters.py': '0520fbf2c651f9b22897a19937c54749b0153ee234bae424dc4132d48e198e2a', 'source_transition.py': 'e642061c863ac1b836eeaeea72c8fdc1152097be4dc285ee2d071ea3372da9a0', 'seal.py': 'fa8c298cc5cc65dee984034190ab2e1fe945812e7a873004e14b3d3e242cf878', 'current.py': 'f15abe15b48cf6a5b7a196ec77c45d8c34c8e0fca6d98dfbf2bb35fcabbcf850'}
_RETAINED_RUNNER_SHA = '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76'

def _read_regular(path):
    if not path.is_file() or path.is_symlink() or any(p.is_symlink() for p in path.parents):
        raise ValueError('nonregular current dispatcher dependency: ' + str(path))
    return path.read_bytes()

_modules = {}
# Authenticate every executable dependency before importing any helper.
_bodies = {name: _read_regular(_HELPERS / name) for name in _HELPER_PINS}
for _name, _raw in _bodies.items():
    if _hashlib.sha256(_raw).hexdigest() != _HELPER_PINS[_name]:
        raise ValueError('changed pinned lexer dispatcher helper: ' + _name)
for _name, _raw in _bodies.items():
    _module = _types.ModuleType(_name[:-3])
    _module.__file__ = str(_HELPERS / _name)
    _sys.modules[_name[:-3]] = _module
    exec(compile(_raw, _module.__file__, 'exec'), _module.__dict__)
    _modules[_name] = _module
_saved = _PACKAGE.parent / 'typed_project_source_binding_byte_storage_v1' / 'run.py'
_raw = _read_regular(_saved)
if _hashlib.sha256(_raw).hexdigest() != _RETAINED_RUNNER_SHA:
    raise ValueError('changed immutable source-binding predecessor runner')
_runtime = _types.ModuleType('lexer_reservation_retained_execution')
_runtime.__file__ = str(_saved)
exec(compile(_raw, str(_saved), 'exec'), _runtime.__dict__)
_modules['current.py'].install(_runtime, _PACKAGE)
# The public exact inverse and historical adapter interfaces remain available.
globals().update({name: value for name, value in vars(_runtime).items()
                  if not name.startswith('__')})

if __name__ == '__main__':
    raise SystemExit(main())
