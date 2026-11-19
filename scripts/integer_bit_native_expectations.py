"""Exact bit-operator diagnostic changes for unchanged native oracle bodies."""
import hashlib
import json
from pathlib import Path
import sys
import types

PARENTS = {'boolean_logic': ('verify_boolean_logic.py',
                   45343,
                   '5b816ebddddfa368903309571bd5a71d4c9f714c327f3f93c285531b5c5aeac5'),
 'scalar_comparisons': ('verify_scalar_comparisons.py',
                        26825,
                        'd29f0ef712eba242a42ae93ccc17c5c0ee6bd8980e136d5161c850fb1d0ba350')}
CHANGES = {'boolean_logic': (('7e05a873491dd937aa11ebd7f5a6d8a79fcc8d53a83edf3fc6df889db787fb20',
                    '1f141b5ebbf57cab3ee43fd38c5469002b276fcb2597f68cb9225a6db1b5c91f',
                    ('invalid_token',
                     '// 🦀\r\nfn main() -> bool { return true & false; }',
                     'E0300',
                     'type',
                     33,
                     4)),
                   ('480802c27a00eaea2bc71d54156551277172ad9a0a5b9c848adf228bbd0f91d6',
                    'ef2a1f636ca6af58f2eaf0a2b59126995f44e076f7c0ee0ca51fa1b2c676cf28',
                    ('invalid_token',
                     '// 🦀\r\nfn main() -> bool { return true | false; }',
                     'E0300',
                     'type',
                     33,
                     4)),
                   ('613d5072d494798db79943c70a910a7184c9bdedff52ba1bfa20a5be917f2529',
                    'b39d28c9b5aad96799ca926b9f24e60b1549bd5c08d649f0ec952e5b6a01959e',
                    ('invalid_token',
                     '// 🦀\r\nfn main() -> bool { return true & & false; }',
                     'E0101',
                     'parse',
                     40,
                     1)),
                   ('e214872acb41dd65f0aa3c6b5bfd09e4621a2109c02ee74c3905b6d4ba631aba',
                    '1ed674e5207db9ebc1c022c6a22d2af9d72da9e9a7a09ca0bc0ec8471282ff8f',
                    ('invalid_token',
                     '// 🦀\r\nfn main() -> bool { return true | /* 雪 */ | false; }',
                     'E0101',
                     'parse',
                     48,
                     1)),
                   ('9e0de3a683b554751c27ad635fe15fa5cb6271c3bb1bb3b517b9d815d68b4b1c',
                    '9b3112003308edadad7af7f8fab13a46477bcaa3ae1b9753f98c4123a9037c80',
                    ('invalid_token',
                     '// 🦀\r\nfn main() -> bool { return true ^ false; }',
                     'E0300',
                     'type',
                     33,
                     4)),
                   ('ee0f083481fb168684be1e99538370a8bf4c820c144989503330aa6f02914e38',
                    '8124edc053148e761d33181ad727d2a9ebbbb4d832648310f7b4eac0e024e23d',
                    ('invalid_token',
                     '// 🦀\r\nfn main() -> bool { return ~true; }',
                     'E0300',
                     'type',
                     34,
                     4))),
 'scalar_comparisons': (('7e05a873491dd937aa11ebd7f5a6d8a79fcc8d53a83edf3fc6df889db787fb20',
                         '7466c343c230bd9d179cd6a81bd472a1dcd1ca3e9d68d9552cea25b194683d96',
                         ('token',
                          '// 🦀\r\nfn main() -> bool { return true & false; }',
                          'E0300',
                          'type',
                          33,
                          4)),
                        ('480802c27a00eaea2bc71d54156551277172ad9a0a5b9c848adf228bbd0f91d6',
                         '8414850fe00f64925a98effc2841ae674b398393835820ceafe614dc83e5c324',
                         ('token',
                          '// 🦀\r\nfn main() -> bool { return true | false; }',
                          'E0300',
                          'type',
                          33,
                          4)))}
ROSTERS = {'boolean_logic': {'82d35e005675742dc87ad6477b61c75e76c54145cc797dc8c93303c7ab0adecc': {'case_count': 148,
                                                                                        'derived_sha256': '352fa323efb33626d504ce15dde8cc15a85625aa5a45a14c9caa210848a4ae73',
                                                                                        'selection': 'checked-unary-negation-v1'},
                   'b5ea2f3ccdfd931a01e4e47713d919a75e5885bc3db76930ae8e7777e0c70a15': {'case_count': 148,
                                                                                        'derived_sha256': '812dccf8902954b19a3263177b86da42de353b37afc5322f4f9893d4bed79af9',
                                                                                        'selection': 'original'}},
 'scalar_comparisons': {'8b0ad0e81f19d3b7618e52b0f10e8632f64298d4d9dd0e5e9a13fbd25e2482cc': {'case_count': 197,
                                                                                             'derived_sha256': '9199d380931b0ad1120d1b3165e5ac93b682e7d41b695e817a57b8ea2f8ec294',
                                                                                             'selection': 'original'}}}

def encoded(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()


def amended_cases(suite, original):
    original = list(original)
    digest = hashlib.sha256(encoded(original)).hexdigest()
    roster = ROSTERS[suite].get(digest)
    if roster is None or len(original) != roster["case_count"]:
        raise ValueError("changed pinned native negative case roster")
    result = list(original)
    for source_sha, old_sha, after in CHANGES[suite]:
        matches = [i for i, case in enumerate(result)
                   if hashlib.sha256(case[1].encode()).hexdigest() == source_sha]
        if len(matches) != 1:
            raise ValueError("bit diagnostic amendment requires one exact source")
        index = matches[0]
        if hashlib.sha256(encoded(result[index])).hexdigest() != old_sha:
            raise ValueError("changed pinned native negative expectation")
        result[index] = after
    if hashlib.sha256(encoded(result)).hexdigest() != roster["derived_sha256"]:
        raise ValueError("wrong derived native negative expectation roster")
    restored = list(result)
    for source_sha, _, _ in CHANGES[suite]:
        index = next(i for i, case in enumerate(result)
                     if hashlib.sha256(case[1].encode()).hexdigest() == source_sha)
        restored[index] = original[index]
    if restored != original:
        raise ValueError("bit diagnostic amendment changed another case")
    return result


def load(suite):
    name, length, digest = PARENTS[suite]
    path = Path(__file__).with_name(name)
    body = path.read_bytes()
    if len(body) != length or hashlib.sha256(body).hexdigest() != digest:
        raise ValueError("changed pinned native oracle before Python execution")
    module = types.ModuleType("_oxid_bit_native_" + suite)
    module.__file__ = str(path)
    sys.modules[module.__name__] = module
    exec(compile(body, str(path), "exec"), module.__dict__)
    return module


def prepare(suite):
    module = load(suite)
    if suite == "boolean_logic":
        preceding = module.amended_negative_cases
        module.amended_negative_cases = lambda selection=None: amended_cases(suite, preceding(selection))
    else:
        preceding = module.negative_cases
        module.negative_cases = lambda: amended_cases(suite, preceding())
    return module


def descriptor(suite):
    return {"scope": "current integer bit operations; original native oracle body unchanged",
            "parent": PARENTS[suite], "source_expectation_changes": CHANGES[suite],
            "negative_case_rosters": ROSTERS[suite]}
