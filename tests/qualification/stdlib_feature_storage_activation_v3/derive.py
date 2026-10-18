"""Exact direct-adapter derivation; no canonical output writes."""
from pathlib import Path
import hashlib
BEFORE = (2006, '19b822f32b2edc0a103a583f568b74409e13809237ef12722d7053ac3ec673da')
def derive(raw):
    if type(raw) is not bytes or (len(raw),hashlib.sha256(raw).hexdigest()) != BEFORE:
        raise ValueError('fixed direct adapter predecessor')
    text=raw.decode()
    changes=[("p=HERE/'successor.py'", "p=REPO/'tests/qualification/stdlib_feature_storage_activation_v3/successor.py'"),
      ('wide_span_fixture_storage_v1/predecessor.py','stdlib_feature_storage_v1/candidate.py'),
      ('wide_span_fixture_storage_v1/lifecycle.py','stdlib_feature_storage_activation_v3/lifecycle.py'),
      ('wide_span_fixture_storage_v1/hosted_controls.py','stdlib_feature_storage_activation_v3/hosted_controls.py')]
    for before,after in changes:
        if text.count(before)!=1: raise ValueError('exact adapter seam')
        text=text.replace(before,after)
    return text.encode()
