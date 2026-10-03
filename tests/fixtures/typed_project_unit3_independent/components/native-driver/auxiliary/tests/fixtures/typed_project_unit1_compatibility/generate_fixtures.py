"""Author-controlled intent and fixture creation; never reads compiler output."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CASES = []


def ok(functions, value=None, ty="i32"):
    return {"check": {"exit": 0, "functions": functions},
            "run": {"exit": 0, "result": {"type": ty, **({"value": value} if ty != "unit" else {})}}}


def fail(code, stage, count=1):
    return {operation: {"exit": 1, "diagnostics": [[code, stage]] * count} for operation in ("check", "run")}


def add(name, source, route, intent, expected, *, group="OriginalSingleFile", future=None):
    data = source.encode("utf-8") if isinstance(source, str) else source
    rel = f"fixtures/{name}.ox"
    (ROOT / rel).write_bytes(data)
    CASES.append(dict(name=name, path=rel, sha256=hashlib.sha256(data).hexdigest(), bytes=len(data),
                      route=route, group=group, intent=intent, expected=expected,
                      **({"future_intent": future} if future else {})))


add("contextual_functions", """fn as() -> i32 { return 2; }
fn crate() -> i32 { return 3; }
fn self() -> i32 { return 5; }
fn super() -> i32 { return 7; }
fn main() -> i32 { return as() + crate() + self() + super(); }
""", "scalar", "All four contextual words remain ordinary function identifiers; independent arithmetic is 2+3+5+7=17", ok(5, 17))
add("contextual_locals", "fn main() -> i32 { let as = 2; let crate = 3; let self = 5; let super = 7; return as + crate + self + super; }\n",
    "scalar", "All four contextual words remain ordinary local identifiers", ok(1, 17))
add("contextual_type_value_pairs", """struct as { n: i32 }
struct crate { n: i32 }
struct self { n: i32 }
struct super { n: i32 }
fn as() -> i32 { return 2; }
fn crate() -> i32 { return 3; }
fn self() -> i32 { return 5; }
fn super() -> i32 { return 7; }
fn main() -> i32 {
    let a: as = as { n: as() };
    let b: crate = crate { n: crate() };
    let c: self = self { n: self() };
    let d: super = super { n: super() };
    return a.n + b.n + c.n + d.n;
}
""", "owned", "Type annotations, constructors and callees select separate namespaces for the same four spellings", ok(5, 17))
add("builtin_value_names", "fn bool() -> i32 { return 4; } fn i32() -> i32 { return 9; } fn main() -> i32 { return bool() + i32(); }\n",
    "scalar", "Builtin type spellings remain legal value declarations", ok(3, 13))
add("double_colon_return", "fn main()::i32 {}\n", "parse", "Unsupported function punctuation diagnoses the first one-byte colon, never a globally combined double-colon token", fail("E0100", "parse"))
add("spaced_colons", "fn main(): :i32 {}\n", "parse", "Trivia-separated colons remain ordinary invalid punctuation", fail("E0100", "parse"))
add("cast_scalar", "fn main() -> i32 { return (6 + 3) as i32; }\n", "parse", "as is contextual only for future import aliasing; scalar casts remain unavailable", fail("E0101", "parse"))
add("cast_owned", "struct S { x: i32 } fn main() -> i32 { let s = S { x: 9 }; return s.x as i32; }\n", "parse", "The owned surface does not enable casts", fail("E0101", "parse"))
add("colon_token_cap", ":" * 100000, "parse", "Exactly 100000 one-byte colon tokens pass lex admission then fail ordinary parsing", fail("E0100", "parse"))
add("colon_token_one_over", ":" * 100001, "lex", "Token 100001 fails lex admission at byte [100000,100001); :: cannot reduce the existing token count", fail("E0400", "lex"))
add("number_token_cap", "fn main() -> i32 { return " + "0" * 65536 + "; }\n", "scalar", "65536 ASCII zero digits are one admitted token and exactly represent zero", ok(1, 0))
add("number_token_one_over", "fn main() -> i32 { return " + "0" * 65537 + "; }\n", "lex", "65537-byte number token fails before integer conversion or parser admission", fail("E0400", "lex"))
long = "prefix_" + "x" * 8185
add("long_common_prefix", f"fn {long}a() -> i32 {{ return 19; }}\nfn {long}b() -> i32 {{ return 23; }}\nfn main() -> i32 {{ return {long}a() + {long}b(); }}\n",
    "scalar", "8193-byte names sharing 8192 bytes remain distinct; shared indexing must preserve original identity", ok(3, 42))
add("scalar_duplicate_before_body", "fn repeat() -> bool { return 1; }\nfn repeat() -> i32 { return 2; }\nfn main() -> i32 { return missing; }\n",
    "scalar", "Declaration duplicate is reported before invalid return typing and unknown body names; scalar signatures themselves are valid", fail("E0201", "resolve"))
add("unknown_signature_plus_duplicate", "fn same(x: Missing) -> i32 { return 1; }\nfn same() -> i32 { return 2; }\nfn main() -> i32 { return 3; }\n",
    "owned", "Unknown nominal-looking signature forces owned selection; later duplicate wins before the earlier unknown signature", fail("E0201", "resolve"))
add("owned_declaration_order", "fn same(x: Missing) -> i32 { return 1; }\nstruct A { value: OtherMissing }\nfn same() -> i32 { return 2; }\nstruct A {}\nfn same() -> i32 { return 3; }\n",
    "owned", "Three declaration conflicts are emitted in lexical item order; no field/signature error leaks through", fail("E0201", "resolve", 3))
add("owned_fields_before_signatures", "fn first(x: Missing) -> i32 { return 1; }\nstruct Record { a: i32, a: bool }\nfn next(x: AlsoMissing) -> i32 { return 2; }\n",
    "owned", "Absent declaration collisions, record-field diagnostic precedes first and next function signatures even though function text comes first",
    {op: {"exit": 1, "diagnostics": [["E0201", "resolve"], ["E0202", "resolve"], ["E0202", "resolve"]]} for op in ("check", "run")})
for name, comment1, comment2 in (("origins_unicode_crlf", "é", "雪"), ("origins_ascii_crlf", "xx", "xxx")):
    add(name, f"/*{comment1}*/fn same() -> i32 {{ return 1; }}\r\n/*{comment2}*/fn same() -> i32 {{ return 2; }}\r\n",
        "scalar", "Equal byte offsets across independently invoked files, different Unicode scalar columns; primary and earlier-declaration secondary must use this file's bytes", fail("E0201", "resolve"))
add("unicode_runtime_overflow", "// é🦀\r\nfn main() -> i32 { /* 雪 */ return 2147483647 + 1; }\r\n", "scalar",
    "Checked overflow points to the one-byte plus after a multibyte comment; CRLF counts one newline",
    {"check": {"exit": 0, "functions": 1}, "run": {"exit": 1, "diagnostics": [["E0604", "oir-run"]]}})
add("control_text_origin", "fn main() -> i32 { /*\x1b[31m*/ return absent; }\n", "scalar", "Human source rendering must escape ESC rather than emit a terminal control sequence", fail("E0200", "resolve"))
add("scalar_helpers_loop", """fn step(x: i32) -> i32 { return x * 2; }
fn main() -> i32 { let mut total = 1; let mut index = 0; while index < 4 { total = step(total) + index; index = index + 1; } return total; }
""", "scalar", "Four loop/helper applications yield total 2,5,12,27 with isolated call parameters and mutable local storage", ok(2, 27))
add("scalar_short_circuit", "fn danger() -> bool { return 2147483647 + 1 == 0; } fn main() -> bool { return false && danger(); }\n", "scalar",
    "Unchosen direct helper is checked but never executed, so overflow cannot fire", ok(2, False, "bool"))
add("owned_helpers_moves_reborrows", """struct Counter { n: i32 }
fn make(n: i32) -> Counter { return Counter { n: n }; }
fn bump(p: &mut Counter) -> () { p.n = p.n + 2; return; }
fn relay_borrow(p: &mut Counter) -> () { bump(&mut *p); return; }
fn read(p: &Counter) -> i32 { return p.n; }
fn shared_reborrow(p: &mut Counter) -> i32 { return read(&*p); }
fn relay(p: Counter) -> Counter { return p; }
fn finish(p: Counter) -> i32 { return p.n; }
fn main() -> i32 { let mut value = make(3); relay_borrow(&mut value); bump(&mut value); let seen = shared_reborrow(&mut value); let moved = relay(value); return seen + finish(moved); }
""", "owned", "Constructor returns, exclusive and shared reborrows, normal loan release and whole moves preserve value 3+2+2=7; observed plus consumed value is 14", ok(8, 14))
add("owned_use_after_move", "struct S { n: i32 } fn take(p: S) -> i32 { return p.n; } fn main() -> i32 { let s = S { n: 3 }; let n = take(s); return s.n + n; }\n",
    "owned", "Whole-value argument consumes the owner; subsequent field projection reports use after move", fail("E0310", "ownership"))
add("owned_conflicting_borrows", "struct S { n: i32 } fn both(a: &mut S, b: &S) -> i32 { return b.n; } fn main() -> i32 { let mut s = S { n: 3 }; return both(&mut s, &s); }\n",
    "owned", "A call cannot overlap exclusive and shared loans of one owner", fail("E0311", "ownership"))
add("owned_rhs_before_mutability", "struct S { n: i32 } fn main() -> i32 { let s = S { n: 3 }; s.n = true + 1; return s.n; }\n",
    "owned", "Invalid arithmetic RHS wins before immutable field target validation", fail("E0300", "type"))
add("owned_mutability_before_assignment_type", "struct S { n: i32 } fn main() -> i32 { let s = S { n: 3 }; s.n = true; return s.n; }\n",
    "owned", "Well-typed bool RHS reaches immutable target check before assignment type compatibility", fail("E0304", "type"))
add("scalar_fuel_loop", "fn main() -> i32 { while true {} return 0; }\n", "scalar", "Accepted empty infinite loop is stopped by the fixed one-million reference fuel budget",
    {"check": {"exit": 0, "functions": 1}, "run": {"exit": 1, "diagnostics": [["E0601", "oir-run"]]}})
add("owned_fuel_reborrow", "struct S { n: i32 } fn idle(p: &mut S) -> () { return; } fn pass(p: &mut S) -> () { idle(&mut *p); return; } fn main() -> i32 { let mut s = S { n: 1 }; while true { pass(&mut s); } return s.n; }\n",
    "owned", "Repeated call-only reborrows release normally until fixed fuel denies the next operation; storage stays bounded",
    {"check": {"exit": 0, "functions": 3}, "run": {"exit": 1, "diagnostics": [["E0601", "oir-run"]]}})
add("check_without_entry", "fn helper() -> i32 { return 8; }\n", "scalar", "Check is entry-optional; run requires original main",
    {"check": {"exit": 0, "functions": 1}, "run": {"exit": 1, "diagnostics": [["E0600", "oir-run"]]}})

# Explicit migration cases are independently specified but excluded from the
# OriginalSingleFile preservation gate once public project syntax is activated.
add("migration_module", "mod absent; fn main() -> i32 { return 7; }\n", "parse", "Historical mod token is unsupported", fail("E0101", "parse"), group="DeliberateFutureMigration",
    future="After activation: parse mod, attempt absent.ox, report E0002/source at absent. Unit1 public syntax remains closed.")
add("migration_import", "use crate::answer as choose; fn answer() -> i32 { return 7; } fn main() -> i32 { return choose(); }\n", "parse", "Historical use token is unsupported", fail("E0101", "parse"), group="DeliberateFutureMigration",
    future="After activation: direct original import alias selects answer; check has two original functions and run returns i32 7.")
add("migration_visibility", "pub fn main() -> i32 { return 11; }\n", "parse", "Historical pub token is unsupported", fail("E0101", "parse"), group="DeliberateFutureMigration",
    future="After activation: scalar public main is valid, check one function and run returns i32 11.")
add("migration_qualified_call", "fn answer() -> i32 { return 13; } fn main() -> i32 { return crate::answer(); }\n", "parse", "Historical absolute call path fails at first colon", fail("E0100", "parse"), group="DeliberateFutureMigration",
    future="After activation: absolute original callee resolves answer; check two functions and run returns i32 13.")

(ROOT / "suite-intent.json").write_text(json.dumps({
    "version": 1, "authority": "Authored before any execution of these fixtures, from contract revision3 and pinned predecessor source/spec",
    "contract_sha256": "25faad72c1c05370e80c43c8ef890b99ebd608baa39f1431521387d5ca44e0aa",
    "base_commit": "fbcfeb2a2de8fe9d335d6c8051d254cccdb663dc", "base_tree": "658c83465aed3a1423483dcdcc23a3d10037131f",
    "cases": CASES}, indent=2) + "\n")
print(f"Wrote {len(CASES)} independently specified sources")
