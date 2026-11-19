"""One reversible initializer for the retained parser overflow controls.

The fixture constructs an unparsed Parser. Its checked-Copy syntax marker is
false, exactly as in the production constructor. No source authority, resource
formula, predicate, test selector or supported observer domain changes here.
"""
import hashlib

VERSION = "unit2-unparsed-copy-marker-v1"
RESOURCE = "archive/resource/parser-resource-review-tests.rs"
PREDECESSOR_PIN = (2214, '67e229577ff316177af0849def214ed32f1e5495c3f4a515f90721bb1203a9e4')
DERIVED_PIN = (2244, 'eb976677ee480a67017b6eaff8bf8d71064744ec7a5f37f4f79b12808e7d5134')
BEFORE = b'parsed_has_named_slice:false,arrays:'
AFTER = b'parsed_has_named_slice:false,parsed_has_copy_records:false,arrays:'


def _pin(body):
    return len(body), hashlib.sha256(body).hexdigest()


def adapt(body):
    if type(body) is not bytes or _pin(body) != PREDECESSOR_PIN:
        raise ValueError("wrong checked-Copy parser resource predecessor")
    if body.count(BEFORE) != 1 or AFTER in body:
        raise ValueError("checked-Copy parser initializer seam drift")
    result = body.replace(BEFORE, AFTER, 1)
    if _pin(result) != DERIVED_PIN or reverse(result) != body:
        raise ValueError("wrong checked-Copy parser resource successor")
    return result


def reverse(body):
    if type(body) is not bytes or _pin(body) != DERIVED_PIN or body.count(AFTER) != 1:
        raise ValueError("wrong checked-Copy parser resource reverse input")
    result = body.replace(AFTER, BEFORE, 1)
    if _pin(result) != PREDECESSOR_PIN:
        raise ValueError("checked-Copy parser reverse identity differs")
    return result


def descriptor():
    return {"version": VERSION, "path": RESOURCE,
            "predecessor": {"bytes": PREDECESSOR_PIN[0], "sha256": PREDECESSOR_PIN[1]},
            "derived": {"bytes": DERIVED_PIN[0], "sha256": DERIVED_PIN[1]},
            "substitutions": [{"count": 1,
                               "before_sha256": hashlib.sha256(BEFORE).hexdigest(),
                               "after_sha256": hashlib.sha256(AFTER).hexdigest()}],
            "preserved": ["all assertions", "all resource formulas", "all test names"],
            "checked_copy_semantics_qualified": False}
