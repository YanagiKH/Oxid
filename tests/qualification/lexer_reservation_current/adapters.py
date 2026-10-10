"""Prepared, inactive, current-only fallible-lexer observer transformations.

No fixture, source manifest, or authority is written here. Activation requires
independently reviewed final source/derived identities in an outer sealed caller.
The historical Unit1 and Unit4 adapters remain immutable and independently usable.
"""
import hashlib
import json
import re

UNIT1_ORIGINAL_SHA256 = "db72d3efc3ea17abfc5a59f674a0a71b77ee784d2cfb2330e7af103910ecef61"
LEXER_PATH = "src/frontend/lexer.rs"
TOKEN_OBSERVER = b"crate::frontend::parser::unit4_observer"
LIFECYCLE_OBSERVER = b"crate::frontend::lifecycle_observer"
UNIT1_CASE = "heldout_checked_counter_overflow_stays_distinct_from_allocation_failure"


class Reject(ValueError):
    pass


def need(condition, reason):
    if not condition:
        raise Reject(reason)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def binding(name, body):
    return {"path": name, "bytes": len(body), "sha256": sha(body)}


def verify(body, expected, name):
    need(binding(name, body) == expected, "unapproved complete body: " + name)


def replace_exact(raw, seams):
    """One occurrence per seam; exact reverse, no fuzzy patch or normalization."""
    result = raw
    for before, after in seams:
        need(before != after and result.count(before) == 1 and after not in result,
             "missing, duplicate, or already applied adapter seam")
        result = result.replace(before, after, 1)
    restored = inverse_exact(result, seams)
    need(restored == raw, "adapter did not recover every original byte")
    return result


def inverse_exact(derived, seams):
    restored = derived
    for before, after in reversed(seams):
        need(restored.count(after) == 1, "missing or duplicate inverse adapter seam")
        restored = restored.replace(after, before, 1)
    return restored


# Exactly the original stage/text assertions are retained in the else branch.
# The five lexer request origins follow the frozen minimal fixture spellings:
# root "mod a;\\n", child "fn f()->(){return;}\\n". No expected span is learned
# from the candidate's diagnostic, token stream, or allocator failure output.
UNIT1_BEFORE = b'''        let parser_reserve=matches!(a.trace[ordinal-1].kind,"module declarations"|"module items");
        assert_eq!(d.stage,if parser_reserve{"parse"}else{"source-project"});
        assert!(d.message.contains("overflow"),"ordinal={ordinal}: {d:?}");
'''
UNIT1_AFTER = b'''        if a.trace[ordinal-1].kind=="lexer token tape" {
            let expected=[(4,(0,0,3)),(8,(0,6,7)),(4,(1,0,2)),(8,(1,5,6)),(16,(1,10,11))];
            let sites=a.trace.iter().enumerate().filter(|(_,e)|e.kind=="lexer token tape").collect::<Vec<_>>();
            assert_eq!(sites.len(),expected.len(),"exact current lexer request roster");
            assert_eq!(sites.iter().map(|(i,_)|i+1).collect::<Vec<_>>(),vec![5,6,19,20,21]);
            for ((_,site),(target,_)) in sites.iter().zip(expected) {
                assert_eq!((site.length,site.element_bytes,site.success),(target,std::mem::size_of::<lexer::Token>(),true));
            }
            let index=sites.iter().position(|(i,_)|*i==ordinal-1).unwrap();
            assert_eq!(f.diagnostics.len(),1);
            assert_eq!(f.allocator.attempts,usize::MAX);
            for (actual,baseline) in f.allocator.trace.iter().zip(&a.trace) {
                assert_eq!((actual.kind,actual.length,actual.element_bytes,actual.success),
                    (baseline.kind,baseline.length,baseline.element_bytes,true));
            }
            assert_eq!((d.code,d.stage,d.message.as_str()),("E0400","lex","token storage resource limit exceeded"));
            assert_eq!(d.primary.map(tuple),Some(expected[index].1));
            assert!(d.secondary.is_empty() && d.notes.is_empty());
        } else {
            let parser_reserve=matches!(a.trace[ordinal-1].kind,"module declarations"|"module items");
            assert_eq!(d.stage,if parser_reserve{"parse"}else{"source-project"});
            assert!(d.message.contains("overflow"),"ordinal={ordinal}: {d:?}");
        }
'''
UNIT1_SEAMS = ((UNIT1_BEFORE, UNIT1_AFTER),)


def unit1_counter_domain(raw):
    """Only the new exact label gets the lex diagnostic; old controls survive."""
    need(sha(raw) == UNIT1_ORIGINAL_SHA256, "changed immutable Unit1 original")
    result = replace_exact(raw, UNIT1_SEAMS)
    names = re.findall(rb"#\[test\]\s*fn ([A-Za-z0-9_]+)\(", raw)
    need(names == re.findall(rb"#\[test\]\s*fn ([A-Za-z0-9_]+)\(", result),
         "Unit1 original case identities changed")
    prefix, body = raw.split(("fn " + UNIT1_CASE + "()").encode(), 1)
    need(result.startswith(prefix) and body.count(UNIT1_BEFORE) == 1,
         "Unit1 adaptation escaped the approved counter case")
    receipt = {
        "schema": "oxid-unit1-lexer-reservation-correspondence-v1",
        "original": binding("reviewer_additional.rs", raw),
        "derived": binding("reviewer_additional.lexer-reservation-v1.rs", result),
        "inverse_sha256": sha(inverse_exact(result, UNIT1_SEAMS)),
        "cases": [{"name": name.decode(), "adaptation": "lexer-counter-overflow-only"
                   if name.decode() == UNIT1_CASE else "unchanged"} for name in names],
        "label": "lexer token tape",
        "new_diagnostic": ["E0400", "lex", "token storage resource limit exceeded"],
        "old_nonlexer_stage_text_primary_prefix_rules": "retained",
    }
    return result, receipt


CORE_START = b'''fn lex_core(
    source: &SourceFile,
    limit: usize,
    allocator: &mut Allocator,
    observer: &mut ReservationObserver,
) -> Result<Vec<Token>, Failure> {
'''
CORE_END = b'''        allocator,
        observer,
    )?;
    Ok(tokens)
}
'''
LIFECYCLE_SEAMS = (
    (CORE_START, CORE_START + b'    ' + LIFECYCLE_OBSERVER + b'::event("lex_attempt", source.path());\n'),
    (CORE_END, CORE_END.replace(b'    Ok(tokens)\n', b'    ' + LIFECYCLE_OBSERVER + b'::event("lex_complete", source.path());\n    Ok(tokens)\n')),
)
APPEND_END = b'''    tokens.push(token);
    Ok(())
}
'''
TOKEN_SEAMS = ((APPEND_END, APPEND_END.replace(b'    Ok(())\n',
    b'    ' + TOKEN_OBSERVER + b'::lex_token(*tokens.last().unwrap());\n    Ok(())\n')),)


def lexer_schedule(raw):
    """Static seam/cardinality fence, not a replacement for executed hook tests."""
    need(raw.count(CORE_START) == raw.count(CORE_END) == raw.count(APPEND_END) == 1,
         "current lexer helper/core seam drift")
    core = raw.split(CORE_START, 1)[1].split(CORE_END, 1)[0] + CORE_END
    need(core.count(b"append(\n") == 2, "ordinary and EOF append call roster changed")
    need(core.count(b"Token { kind, span }") == core.count(b"kind: Kind::Eof") == 1,
         "ordinary/EOF pending token construction changed")
    need(raw.count(b"tokens.push(") == 1, "append bypass or duplicate push")
    need(LIFECYCLE_OBSERVER not in raw and b"::lex_token(" not in raw,
         "source is already instrumented")
    return core


def instrument_lexer(raw, expected_source, expected_derived, kind):
    """Admit complete bytes before using reviewed hooks; caller supplies pins.

    Lifecycle hooks live in the actual core, including test-only direct callers.
    Token events follow the only successful push; rejected appends have no event.
    No delegating wrapper is instrumented. The two observer builds are separate.
    """
    verify(raw, expected_source, LEXER_PATH)
    lexer_schedule(raw)
    need(kind in ("lifecycle", "token"), "unknown lexer observation kind")
    seams = LIFECYCLE_SEAMS if kind == "lifecycle" else TOKEN_SEAMS
    result = replace_exact(raw, seams)
    verify(result, expected_derived, LEXER_PATH)
    need(result.count(b'::event("lex_attempt"') == (kind == "lifecycle")
         and result.count(b'::event("lex_complete"') == (kind == "lifecycle")
         and result.count(b"::lex_token(") == (kind == "token"),
         "missing or duplicate lexer callback")
    return result, {"schema": "oxid-fallible-lexer-hook-correspondence-v1",
                    "kind": kind, "source": expected_source, "derived": expected_derived,
                    "inverse": binding(LEXER_PATH, inverse_exact(result, seams)),
                    "ordinary_append_sites": 1, "eof_append_sites": 1,
                    "successful_append_hook_sites": int(kind == "token"),
                    "actual_core_attempt_sites": int(kind == "lifecycle"),
                    "successful_eof_completion_sites": int(kind == "lifecycle"),
                    "wrapper_hook_sites": 0}


UNIT1_ROSTER_SHA = "331bf92273fb3f9904aeff24e279ff546387a211239e0ae7ce1ac20c04468f79"

def unit1_correspondence(raw_roster, adapter_receipt):
    """Preserve every one of the 69 frozen names; extra controls are separate."""
    need(sha(raw_roster) == UNIT1_ROSTER_SHA, "changed original Unit1 case roster")
    names = json.loads(raw_roster)["tests"]
    need(len(names) == len(set(names)) == 69, "wrong original Unit1 case cardinality")
    adapted = [name for name in names if name.endswith("::" + UNIT1_CASE)]
    need(len(adapted) == 1, "ambiguous adapted Unit1 case")
    return {**adapter_receipt, "cases": [
        {"name": name, "adaptation": "lexer-counter-overflow-only"
         if name in adapted else "unchanged"} for name in names],
        "original_roster_sha256": sha(raw_roster), "original_cases": 69,
        "execution_qualified": False}


BUDGET_PATH = "src/frontend/project/budget.rs"
BUDGET_BEFORE = b'        #[cfg(test)]\n        self.trace.push(ReserveEvent {'
BUDGET_AFTER = (b'        #[cfg(test)]\n        ' + TOKEN_OBSERVER
    + b'::reserve(kind, length, element_bytes, success);\n' + BUDGET_BEFORE)
BUDGET_SEAMS = ((BUDGET_BEFORE, BUDGET_AFTER),)


def instrument_budget(raw, expected_source, expected_derived):
    """Transport the retained public reserve hook; control builds stay plain.

    This is the exact insertion exposed by compose_array_instrumentation. It
    preserves that location relative to successful trace recording; it does not
    convert a refused/missing bounded trace into a qualified reservation event.
    """
    verify(raw, expected_source, BUDGET_PATH)
    need(b"::reserve(kind, length, element_bytes, success)" not in raw,
         "budget source already instrumented")
    result = replace_exact(raw, BUDGET_SEAMS)
    verify(result, expected_derived, BUDGET_PATH)
    return result, {"schema": "oxid-fallible-lexer-reserve-hook-correspondence-v1",
                    "source": expected_source, "derived": expected_derived,
                    "inverse": binding(BUDGET_PATH, inverse_exact(result, BUDGET_SEAMS)),
                    "reserve_hook_sites": 1, "control_build_hook_sites": 0}
