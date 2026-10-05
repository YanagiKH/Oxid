#!/usr/bin/env python3
"""Independent lazy tagged-tree / reference / mandatory real LLVM O0 gate.

Usage: python3 scripts/verify_boolean_logic.py target/debug/oxid target/release/oxid
Current unary source: add --expectation-amendment checked-unary-negation-v1.
Without the explicit selection, historical negative expectations stay unchanged.
Requires Linux x86_64 and pinned LLVM/Clang/LLD 19.1.7. Missing tools fail.
The source model traverses only chosen logical operands; Python bool/int
coercion, compiler parsing/evaluation, and captured compiler errors never supply
expected answers. Every native corpus case executes with source removed and
Oxid/LLVM/Python absent from PATH; profile artifact hashes must match.
"""
import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile

from verify_native_arithmetic import verify_adapter

UNARY_EXPECTATION_AMENDMENT_ID = "checked-unary-negation-v1"
UNARY_NEGATIVE_SOURCE_SHA256 = "99a4ea084344bf53c945e6d754676c0cb5dbac1d34d08b7852b786c11688f8fd"
UNARY_NEGATIVE_OLD_EXPECTATION_SHA256 = "eaf1a799ff638414999b3a45e66dc13340b940196fbbf538635e89e8eb9d363e"

MIN, MAX = -(2 ** 31), 2 ** 31 - 1
COMPARATORS = ("==", "!=", "<", "<=", ">", ">=")
BOUNDARIES = (MIN, MIN + 1, -46341, -46340, -2, -1, 0, 1, 2, 46340, 46341, MAX - 1, MAX)


@dataclass(frozen=True)
class Scalar:
    tag: str
    value: object

    def __post_init__(self):
        assert ((self.tag == "i32" and type(self.value) is int and MIN <= self.value <= MAX)
                or (self.tag == "bool" and type(self.value) is bool)
                or (self.tag == "unit" and self.value is None)), self

    def text(self):
        return "()" if self.tag == "unit" else str(self.value).lower()

    def record(self):
        return {"type": "unit"} if self.tag == "unit" else {"type": self.tag, "value": self.value}


@dataclass(frozen=True)
class Node:
    kind: str
    children: tuple = ()
    atom: object = None

    def text(self):
        if self.kind == "leaf":
            return self.atom.text()
        if self.kind == "variable":
            return self.atom
        if self.kind == "not":
            return "!" + self.children[0].text()
        if self.kind == "group":
            return "(" + self.children[0].text() + ")"
        if self.kind == "call":
            return self.atom + "(" + ", ".join(child.text() for child in self.children) + ")"
        assert self.kind == "binary"
        return f"({self.children[0].text()} {self.atom} {self.children[1].text()})"


def leaf(tag, value):
    return Node("leaf", atom=Scalar(tag, value))


def boolean(value):
    return leaf("bool", value)


def integer(value):
    return leaf("i32", value)


def binary(left, op, right):
    return Node("binary", (left, right), op)


def invert(child):
    return Node("not", (child,))


def call(name, *args):
    return Node("call", tuple(args), name)


class Overflow(Exception):
    def __init__(self, offset):
        self.offset = offset


@dataclass(frozen=True)
class Function:
    name: str
    parameters: tuple
    result: str
    body: Node

    def prefix(self):
        parameters = ", ".join(name + ": " + tag for name, tag in self.parameters)
        return f"fn {self.name}({parameters}) -> {self.result} {{ return "


def evaluate(node, offset=0, functions=None, variables=None, trace=None):
    """A lazy recursive tree walk; constructing a tree never evaluates children."""
    functions = {} if functions is None else functions
    variables = {} if variables is None else variables
    trace = [] if trace is None else trace
    if node.kind == "leaf":
        return node.atom
    if node.kind == "variable":
        return variables[node.atom]
    if node.kind == "group":
        return evaluate(node.children[0], offset + 1, functions, variables, trace)
    if node.kind == "not":
        result = evaluate(node.children[0], offset + 1, functions, variables, trace)
        assert result.tag == "bool", result
        return Scalar("bool", not result.value)
    if node.kind == "call":
        function, body_offset = functions[node.atom]
        argument_offset = offset + len(node.atom) + 1
        arguments = []
        for child in node.children:
            arguments.append(evaluate(child, argument_offset, functions, variables, trace))
            argument_offset += len(child.text()) + 2
        assert len(arguments) == len(function.parameters)
        for (_, tag), argument in zip(function.parameters, arguments):
            assert argument.tag == tag
        trace.append("enter " + node.atom)
        result = evaluate(function.body, body_offset, functions,
                          dict(zip((p[0] for p in function.parameters), arguments)), trace)
        assert result.tag == function.result
        trace.append("return " + node.atom)
        return result
    assert node.kind == "binary"
    left, right = node.children
    op = node.atom
    lv = evaluate(left, offset + 1, functions, variables, trace)
    if op in ("&&", "||"):
        assert lv.tag == "bool", lv
        # Branch BEFORE visiting the right tree, including its call arguments.
        if (op == "&&" and lv.value is False) or (op == "||" and lv.value is True):
            return lv
        rv = evaluate(right, offset + len(left.text()) + len(op) + 3, functions, variables, trace)
        assert rv.tag == "bool", rv
        return rv
    rv = evaluate(right, offset + len(left.text()) + len(op) + 3, functions, variables, trace)
    if op in ("+", "-", "*"):
        assert lv.tag == rv.tag == "i32", (lv, op, rv)
        value = {"+": lambda: lv.value + rv.value,
                 "-": lambda: lv.value - rv.value,
                 "*": lambda: lv.value * rv.value}[op]()
        if not MIN <= value <= MAX:
            raise Overflow(offset + len(left.text()) + 2)
        return Scalar("i32", value)
    assert op in COMPARATORS and lv.tag == rv.tag and lv.tag in ("i32", "bool")
    assert op in ("==", "!=") or lv.tag == "i32"
    value = {"==": lambda: lv.value == rv.value, "!=": lambda: lv.value != rv.value,
             "<": lambda: lv.value < rv.value, "<=": lambda: lv.value <= rv.value,
             ">": lambda: lv.value > rv.value, ">=": lambda: lv.value >= rv.value}[op]()
    return Scalar("bool", value)


HELPERS = (
    Function("id_i32", (("x", "i32"),), "i32", Node("variable", atom="x")),
    Function("id_bool", (("x", "bool"),), "bool", Node("variable", atom="x")),
    Function("take", (("a", "bool"), ("b", "bool")), "bool", Node("variable", atom="b")),
)


def modeled_case(category, expression, helpers=HELPERS):
    source = "// 雪\r\n"
    functions = {}
    for function in helpers:
        prefix = function.prefix()
        functions[function.name] = function, len(source) + len(prefix)
        source += prefix + function.body.text() + "; }\r\n"
    prefix = "fn main() -> bool { return "
    offset = len(source) + len(prefix)
    source += prefix + expression.text() + "; }"
    trace = []
    try:
        result, error = evaluate(expression, offset, functions, trace=trace), None
    except Overflow as failure:
        result, error = None, failure.offset
    return category, source, result, error, tuple(trace)


def self_check_model():
    for invalid in [("i32", True), ("bool", 1), ("unit", 0), ("i32", MAX + 1)]:
        try:
            Scalar(*invalid)
        except AssertionError:
            pass
        else:
            raise AssertionError(("oracle coerced a scalar", invalid))
    for expression in [invert(integer(1)), binary(integer(1), "&&", boolean(True)),
                       binary(boolean(False), "||", integer(0)),
                       binary(boolean(True), "==", integer(1))]:
        try:
            evaluate(expression)
        except AssertionError:
            pass
        else:
            raise AssertionError(("oracle accepted bad executed types", expression))
    plus = binary(integer(MAX), "+", integer(1))
    minus = binary(integer(MIN), "-", integer(1))
    bad = binary(plus, "==", minus)
    # Evaluate first errors independently of outer results and RHS failures.
    for expression, marker in [(bad, "+"), (binary(minus, "!=", plus), "-")]:
        try:
            evaluate(expression)
        except Overflow as failure:
            assert expression.text()[failure.offset] == marker
        else:
            raise AssertionError("oracle missed overflow")
    assert evaluate(binary(boolean(False), "&&", bad)) == Scalar("bool", False)
    assert evaluate(binary(boolean(True), "||", bad)) == Scalar("bool", True)
    functions = (Function("first", (), "bool", boolean(False)),
                 Function("second", (), "bool", boolean(True)))
    case = modeled_case("self", binary(call("first"), "&&", call("second")), functions)
    assert case[2:] == (Scalar("bool", False), None, ("enter first", "return first"))
    case = modeled_case("self", binary(call("first"), "||", call("second")), functions)
    assert case[2:] == (Scalar("bool", True), None, ("enter first", "return first", "enter second", "return second"))
    for left in (False, True):
        for right in (False, True):
            functions = (Function("left", (), "bool", boolean(left)),
                         Function("right", (), "bool", boolean(right)))
            for op in ("&&", "||"):
                case = modeled_case("self", binary(call("left"), op, call("right")), functions)
                selected = left if (op == "&&" and left is False) or (op == "||" and left is True) else right
                expected_trace = ("enter left", "return left")
                if (op == "&&" and left is True) or (op == "||" and left is False):
                    expected_trace += ("enter right", "return right")
                assert case[2:] == (Scalar("bool", selected), None, expected_trace)
    # A skipped call must not even evaluate a poisoned argument.
    case = modeled_case("self", binary(boolean(True), "||", call("id_bool", bad)))
    assert case[2:] == (Scalar("bool", True), None, ())
    bad_first = Function("bad_first", (), "bool", bad)
    good_later = Function("good_later", (), "bool", boolean(True))
    for expression in [binary(call("bad_first"), "||", call("good_later")),
                       call("take", call("bad_first"), call("good_later"))]:
        case = modeled_case("self", expression, (good_later, bad_first) + HELPERS)
        assert case[2] is None and case[1][case[3]] == "+"
        assert case[4] == ("enter bad_first",), case


def expressions():
    yield "not", invert(boolean(True))
    for left in (False, True):
        for right in (False, True):
            for op in ("&&", "||"):
                yield "truth_table", binary(boolean(left), op, boolean(right))
                yield "call_truth_table", binary(call("id_bool", boolean(left)), op, call("id_bool", boolean(right)))
    for count in range(1, 12):
        for value in (False, True):
            node = boolean(value)
            for _ in range(count):
                node = invert(node)
            yield "not", node
    failure = binary(binary(integer(MAX), "+", integer(1)), "==", integer(0))
    other_failure = binary(binary(integer(MIN), "*", integer(-1)), "!=", integer(0))
    for op in ("&&", "||"):
        for left in (False, True):
            for right in [failure, invert(failure), call("id_bool", failure), other_failure]:
                yield "lazy_error", binary(boolean(left), op, right)
                yield "lazy_call_argument", binary(call("id_bool", boolean(left)), op, call("take", right, other_failure))
        yield "left_first_error", binary(failure, op, other_failure)
        yield "left_first_error", binary(other_failure, op, failure)
    # Actual outgoing predecessors are checkedN_ok after checked arithmetic.
    for left in [binary(integer(1), "+", integer(2)),
                 binary(binary(integer(3), "*", integer(4)), "-", integer(2))]:
        for comparator in COMPARATORS:
            l = binary(left, comparator, integer(3))
            r = binary(binary(integer(5), "*", integer(6)), "==", integer(30))
            for op in ("&&", "||"):
                yield "llvm_checked_predecessors", binary(l, op, r)
                yield "llvm_nested_predecessors", binary(binary(l, op, call("id_bool", boolean(False))), op,
                                                        binary(r, "||", call("id_bool", boolean(True))))
                yield "llvm_call_continuations", binary(call("id_bool", l), op, call("id_bool", r))
    rng = random.Random(0xB001_10C1)

    def arithmetic(depth):
        if depth == 0 or rng.randrange(3) == 0:
            node = integer(rng.choice(BOUNDARIES + (rng.randint(-100, 100),)))
            return call("id_i32", node) if rng.randrange(3) == 0 else node
        return binary(arithmetic(depth - 1), rng.choice(("+", "-", "*")), arithmetic(depth - 1))

    def logical(depth):
        if depth == 0 or rng.randrange(5) == 0:
            if rng.randrange(2) == 0:
                node = boolean(bool(rng.randrange(2)))
                return call("id_bool", node) if rng.randrange(2) else node
            return binary(arithmetic(1), rng.choice(COMPARATORS), arithmetic(1))
        if rng.randrange(5) == 0:
            return invert(logical(depth - 1))
        return binary(logical(depth - 1), rng.choice(("&&", "||", "==", "!=")), logical(depth - 1))

    for _ in range(320):
        # Full trees remain below native's 256-local cap, including calls/groups.
        yield "seeded_lazy_tree", logical(3)


def cases():
    for category, expression in expressions():
        yield modeled_case(category, expression)
    for expression, value in [
        ("true || false && false", True), ("false && true || true", True),
        ("!false == true", True), ("1 < 2 && 3 >= 3", True),
        ("(1 < 2) && !(3 == 4)", True), ("1+2*3==7 && 20-4-3==13 || false", True),
        ("true && true && false", False), ("false || false || true", True),
        ("!true!=!false", True), ("! /* 雪 */ false && // 🦀\r\n true", True),
        ("false/*é*/||/*雪*/!true", False), ("(false || true) == (true && !false)", True),
    ]:
        yield "grammar", "// 🦀\r\nfn main() -> bool { return " + expression + "; }", Scalar("bool", value), None, ()
    yield "mixed_chain", "fn main() -> bool {return false" + "||true&&false" * 30 + ";}", Scalar("bool", False), None, ()
    for chosen in (False, True):
        for op in ("&&", "||"):
            for right in (False, True):
                expression = binary(call("id_bool", boolean(chosen)), op, call("id_bool", boolean(right)))
                case = modeled_case("branch", expression)
                selected = case[2].value
                for number in (MIN, MAX):
                    arms = [f"return {number};", "return 2147483647 + 1;"]
                    if selected is False:
                        arms.reverse()
                    source = case[1][:case[1].index("fn main")]
                    source += f"fn main() -> i32 {{ if {expression.text()} {{ {arms[0]} }} else {{ {arms[1]} }} }}"
                    yield "branch", source, Scalar("i32", number), None, case[4]
    for source, result in [
        ("fn id(x: bool) -> bool { return x; } fn main() -> bool { let x = false || true; let y: bool = !x; return id(x && !y); }", Scalar("bool", True)),
        ("fn main() -> () { false && 2147483647 + 1 == 0; true || -2147483648 - 1 == 0; let ignored = !false; return; }", Scalar("unit", None)),
        ("fn unused() -> bool { return true && 2147483647 + 1 == 0; } fn main() -> bool { return !false; }", Scalar("bool", True)),
        ("fn main() -> bool { if false && 2147483647 + 1 == 0 { return false; } return true || -2147483648 - 1 == 0; }", Scalar("bool", True)),
    ]:
        yield "context", source, result, None, ()
    # Independent function-body source mapping distinguishes declaration order
    # from runtime call/argument order. Traces are model witnesses; the public
    # CLI observes those decisions through distinct first-overflow operators.
    first = Function("first", (), "bool", binary(binary(integer(MAX), "+", integer(1)), "==", integer(0)))
    later = Function("later", (), "bool", binary(binary(integer(MIN), "-", integer(1)), "==", integer(0)))
    helpers = (later, first) + HELPERS
    for expr in [
        binary(call("first"), "&&", call("later")),
        binary(call("later"), "||", call("first")),
        binary(binary(boolean(False), "&&", call("first")), "||", call("later")),
        binary(binary(boolean(True), "||", call("later")), "&&", call("first")),
        call("take", call("first"), binary(boolean(False), "&&", call("later"))),
        call("take", binary(boolean(False), "&&", call("first")), call("later")),
        call("take", binary(boolean(True), "||", call("later")), call("first")),
        call("take", call("later"), binary(boolean(True), "||", call("first"))),
        binary(binary(boolean(False), "||", call("first")), "==", call("later")),
        invert(binary(boolean(False), "||", call("first"))),
    ]:
        yield modeled_case("source_model_call_order", expr, helpers)
    for result in [Scalar("bool", False), Scalar("unit", None), Scalar("i32", 42)]:
        for statement, marker in [("true && 2147483647 + 1 == 0;", "+ 1"),
                                  ("let ignored = false || -2147483648 * -1 == 0;", "* -1")]:
            ty = "()" if result.tag == "unit" else result.tag
            source = f"fn main() -> {ty} {{ {statement} return {result.text()}; }}"
            yield "discard_error", source, None, source.index(marker), ()
    for count, template in [(63, "{}true"), (62, "({}true)"), (62, "id_bool({}true)"),
                            (62, "{}true && false"), (62, "{}true == false")]:
        expression = template.format("!" * count)
        value = False if "&&" in template else count % 2 == 0
        if "==" in template:
            value = not value
        source = "fn id_bool(x: bool) -> bool { return x; } fn main() -> bool { return " + expression + "; }"
        yield "height_boundary", source, Scalar("bool", value), None, ()
    for op, value in [("&&", True), ("||", False)]:
        yield "height_boundary", "fn main() -> bool { return " + op.join([str(value).lower()] * 64) + "; }", Scalar("bool", value), None, ()
    yield "token_width_boundary", "fn main() -> bool { return !false && " + "0" * 65536 + "==0; }", Scalar("bool", True), None, ()
    source = "fn main() -> bool { return !false || false; }"
    gap = 1048576 - len(source)
    source += ("/*" + " " * 59996 + "*/") * (gap // 60000)
    source += " " * (gap % 60000)
    assert len(source.encode()) == 1048576
    yield "source_boundary", source, Scalar("bool", True), None, ()


def negative_cases():
    prefix = "// 🦀\r\nfn main() -> bool { return "
    for operand in ("1", "()"):
        for begin in [prefix, "fn main() -> bool { return true; } fn unused() -> bool { return "]:
            yield "invalid_not", begin + "!" + operand + "; }", "E0300", "type", len(begin) + 1, len(operand)
    for op in ("&&", "||"):
        for left in ("1", "()", "true", "false"):
            for right in ("2", "()", "true", "false"):
                if left in ("true", "false") and right in ("true", "false"):
                    continue
                offset, width = (len(left) + len(op) + 2, len(right)) if left in ("true", "false") else (0, len(left))
                for begin, end in [(prefix, "; }"),
                                   ("fn main() -> bool { return true; } fn unused() -> bool { return ", "; }"),
                                   ("fn main() -> bool { if true { return true; } else { return ", "; } }")]:
                    yield "invalid_logical", begin + f"{left} {op} {right}" + end, "E0300", "type", len(begin) + offset, width
    for expression, offset, width in [("!1 < 2", 1, 1), ("!true + 1", 0, 5), ("!true * 2", 0, 5)]:
        yield "invalid_precedence", prefix + expression + "; }", "E0300", "type", len(prefix) + offset, width
    for expression, missing in [("false && missing()", "missing"), ("true || missing()", "missing"),
                                ("left() && right()", "left"), ("left() || right()", "left"),
                                ("1 && missing()", "missing")]:
        yield "eager_resolution", prefix + expression + "; }", "E0200", "resolve", len(prefix) + expression.index(missing), len(missing)
    for expression, marker, code in [
        ("true & false", "&", "E0101"), ("true | false", "|", "E0101"),
        ("true & & false", "&", "E0101"), ("true | /* 雪 */ | false", "|", "E0101"),
        ("true ^ false", "^", "E0100"), ("~true", "~", "E0100"),
        ("true ! false", "!", "E0100"), ("true && || false", "||", "E0100"),
        ("true || && false", "&&", "E0100"), ("-(1)", "-", "E0101"),
        ("true ! = false", "!", "E0100"),
    ]:
        yield "invalid_token", prefix + expression + "; }", code, "parse", len(prefix) + expression.index(marker), len(marker)
    for expression in ("true &&", "false ||", "!", "true && !"):
        yield "incomplete", prefix + expression + "; }", "E0100", "parse", len(prefix) + len(expression), 1
    for first in COMPARATORS:
        for second in COMPARATORS:
            begin = prefix + f"true && 1 {first} 2 /* 雪 */ "
            yield "comparison_chain", begin + second + " 3 || false; }", "E0100", "parse", len(begin), len(second)
    for count, template in [(64, "{}true"), (63, "({}true)"), (63, "id_bool({}true)"),
                            (63, "{}true && false"), (63, "{}true == false")]:
        expression = template.format("!" * count)
        source = "fn id_bool(x: bool) -> bool { return x; } fn main() -> bool { return " + expression + "; }"
        yield "height_limit", source, "E0400", "parse", None, None
    for expression in ["&&".join(["true"] * 65), "||".join(["false"] * 65), "!" * 5000 + "true",
                       "!" * 62 + "true && false && true", "!" * 62 + "true || false || true"]:
        yield "height_limit", prefix + expression + "; }", "E0400", "parse", None, None
    yield "token_width_limit", prefix + "!false && " + "0" * 65537 + "==0; }", "E0400", "lex", len(prefix) + len("!false && "), 65537
    yield "source_limit", "//" + " " * 1048575, "E0400", "source", None, None
    source = "fn main()->(){" + "!true;" * 33329 + "return;} "
    yield "token_count_limit", source + "/*x*/", "E0400", "lex", len(source), 5


def negative_expectation_amendment(selection):
    """Explicit current-source view; retain the historical tuple and its seal."""
    if selection is None:
        return None
    if selection != UNARY_EXPECTATION_AMENDMENT_ID:
        raise ValueError("unknown boolean negative expectation amendment")
    return {
        "amendment_id": selection,
        "scope": "current-production-cli-only",
        "source_sha256": UNARY_NEGATIVE_SOURCE_SHA256,
        "old_expectation_sha256": UNARY_NEGATIVE_OLD_EXPECTATION_SHA256,
        "old_expected": {"category": "invalid_token", "code": "E0101", "stage": "parse", "offset": 33, "width": 1},
        "effective_expected": {"category": "invalid_return_type", "code": "E0300", "stage": "type", "offset": 33, "width": 4},
        "derivation": "Checked i32 unary negation admits -(1), but its i32 result mismatches the declared bool return; the complete expression is the primary span",
    }


def amended_negative_cases(selection=None):
    """Amend exactly one pinned source/expectation; fail closed on drift."""
    amendment = negative_expectation_amendment(selection)
    cases = list(negative_cases())
    if amendment is None:
        return cases
    matches = 0
    for index, case in enumerate(cases):
        if hashlib.sha256(case[1].encode()).hexdigest() != amendment["source_sha256"]:
            continue
        encoded = json.dumps(case, ensure_ascii=True, separators=(",", ":")).encode()
        if hashlib.sha256(encoded).hexdigest() != amendment["old_expectation_sha256"]:
            raise ValueError("boolean amendment historical expectation identity mismatch")
        expected = amendment["effective_expected"]
        cases[index] = (expected["category"], case[1], expected["code"], expected["stage"], expected["offset"], expected["width"])
        matches += 1
    if matches != 1:
        raise ValueError("boolean amendment requires exactly one pinned historical case")
    return cases


def native_resource_cases():
    """Unchanged inclusive/+1 ceilings, with bool logic in every fixture."""
    for count in (256, 257):
        source = "fn main() -> bool { return !false; }" + "".join(f"fn f{i}() -> bool {{ return !false; }}" for i in range(1, count))
        yield "functions", source, count == 256
    for count in (64, 65):
        parameters = ",".join(f"p{i}: bool" for i in range(count))
        source = f"fn unused({parameters}) -> bool {{ return !p0; }} fn main() -> bool {{ return !false; }}"
        yield "parameters", source, count == 64
    for over in (False, True):
        # 85*(two literals + one merge) + bare-return slot = 256.
        source = "fn main() -> () {" + "false&&true;" * 85 + ("true;" if over else "") + "return;}"
        yield "function_slots", source, not over
    for over in (False, True):
        # 32*256 = 8192; one additional bare-return function adds one slot.
        source = "".join(f"fn {'main' if i == 0 else 'f' + str(i)}() -> () {{" + "false&&true;" * 85 + "return;}" for i in range(32))
        # Blocks must stay below 4096 to isolate locals, so use unary logic.
        source = source.replace("false&&true;" * 85, "!true;" * 127 + "true;")
        if over:
            source += "fn extra() -> () {return;}"
        yield "aggregate_slots", source, not over
    for over in (False, True):
        # 23*(1+2*85) + (1+2*81) = exactly 4096 merge-heavy blocks.
        source = "".join(f"fn {'main' if i == 0 else 'f' + str(i)}() -> () {{" + "false&&true;" * (85 if i < 23 else 81) + "return;}" for i in range(24))
        if over:
            source += "fn extra() -> bool {return !false;}"
        yield "aggregate_blocks", source, not over
    for depth in (32, 33):
        source = "fn main() -> bool { return f0(); }"
        for index in range(depth - 1):
            result = "!false" if index == depth - 2 else f"f{index + 1}()"
            source += f"fn f{index}() -> bool {{ return {result}; }}"
        yield "call_depth", source, depth == 32
    # Comparison base cost 7; doubling recurrence 5+2*C gives f13=98299.
    # Main cost: 3 + 95*17 + 6*11 + 8 (skipped logical) + 8 (literals),
    # plus root allocation 1, is 100000. A cheap call replaces two literals
    # (5 versus 4) for +1. Even skipped merges must contribute one assignment.
    source = "fn f0() -> bool { return 0<1; }"
    for index in range(1, 14):
        source += f"fn f{index}() -> bool {{ f{index - 1}(); return f{index - 1}(); }}"
    source += "fn p17() -> bool {0<1;0<1;return true;} fn p11() -> bool {0<1;return true;} fn cheap() -> bool {return true;}"
    for over in (False, True):
        filler = "false&&true;" + "true;" * (2 if over else 4) + ("cheap();" if over else "")
        main = "fn main() -> bool {" + "p17();" * 95 + "p11();" * 6 + filler + "return f13();}"
        yield "fuel", source + main, not over
    for expression in ("false && recursive()", "true || recursive()"):
        yield "skipped_recursion", "fn recursive() -> bool {return recursive();} fn main() -> bool {return " + expression + ";}", False
    yield "unused_recursion", "fn recursive() -> bool {return recursive();} fn main() -> bool {return !false;}", False
    # Acyclic but exponentially expensive RHS remains rejected when skipped.
    expensive = "fn f0() -> bool {return !false;}"
    for index in range(1, 16):
        expensive += f"fn f{index}() -> bool {{ f{index - 1}(); return f{index - 1}(); }}"
    yield "skipped_expensive", expensive + "fn main() -> bool {return false&&f15();}", False


def reference_only_cases():
    helper = "fn recursive() -> bool {return recursive();} "
    for expression, value, code in [
        ("false && recursive()", Scalar("bool", False), None),
        ("true || recursive()", Scalar("bool", True), None),
        ("true && recursive()", None, "E0602"),
        ("false || recursive()", None, "E0602"),
        ("recursive() && false", None, "E0602"),
        ("recursive() || true", None, "E0602"),
    ]:
        yield helper + "fn main() -> bool {return " + expression + ";}", value, code
    # Public sources approaching 100k syntax nodes are constrained first by
    # the independent 100k lexical-token cap; a fabricated parse-node failure
    # would be misleading. These reach the exact token cap with logical syntax.
    # Header has 9 tokens (including its one space), suffix return;} has 3;
    # 33329 repetitions of !true; have 99987 tokens and syntax nodes.
    source = "fn main()->(){" + "!true;" * 33329 + "return;}"
    # 9 + 99987 + 3 = 99999; one trailing trivia token makes exactly 100000.
    yield source + " ", Scalar("unit", None), None


def command(args, **kwargs):
    return subprocess.run(args, capture_output=True, input=b"", timeout=30, **kwargs)


def records_of(result, source):
    assert result.stderr == b"", (source[:1000], result)
    records = [json.loads(line) for line in result.stdout.splitlines()]
    assert records and all(record["schema_version"] == 1 and record["edition"] == "typed-preview" for record in records), records
    return records


def assert_origin(source, primary, offset, width):
    assert (primary["start"], primary["end"]) == (len(source[:offset].encode()), len(source[:offset + width].encode())), (source[:1000], offset, primary)
    assert primary["line"] == source[:offset].count("\n") + 1
    assert primary["column"] == len(source[:offset].rsplit("\n", 1)[-1]) + 1


def verify(binaries, *, expectation_amendment=None):
    amendment = negative_expectation_amendment(expectation_amendment)
    negatives = amended_negative_cases(expectation_amendment)
    self_check_model()
    binaries = [str(Path(binary).resolve()) for binary in binaries]
    count = {"binaries": len(binaries), "source_cases": 0, "success_cases": 0, "overflow_cases": 0,
             "compiled_artifacts": 0, "negative_cases": 0, "native_resource_cases": 0, "compiled_resource_artifacts": 0, "reference_only_cases": 0, "source_model_traces": 0, "invocations": 0}
    categories = {}
    artifact_hashes = hashlib.sha256()
    source_hashes = hashlib.sha256()
    compiler_hashes = [hashlib.sha256(Path(binary).read_bytes()).hexdigest() for binary in binaries]
    with tempfile.TemporaryDirectory(prefix="oxid-boolean-logic-") as directory:
        root = Path(directory)
        name = 'source 雪;\'"\\\n\t\x1b.ox'
        path = root / name
        env = dict(os.environ, OXID_CACHE_DIR=str(root / "cache"))
        env.pop("OXID_PATH", None)
        clean_env = {"PATH": "/no-tools", "LC_ALL": "C"}
        tested_io = set()

        def invoke(binary, operation, *, json_mode=True, output=None, missing_tools=False):
            args = [binary, operation, name, "--edition=typed-preview"]
            if json_mode:
                args.append("--message-format=json")
            if operation == "compile":
                args.extend(["--backend=llvm", "--output", str(output)])
            run_env = dict(env, OXID_LLVM_BIN=str(root / "missing-tools")) if missing_tools else env
            result = command(args, cwd=root, env=run_env)
            count["invocations"] += 1
            return result

        for number, (category, source, value, error, trace) in enumerate(cases()):
            path.write_bytes(source.encode())
            source_hashes.update(source.encode() + b"\0")
            previous = previous_artifact = None
            for profile, binary in enumerate(binaries):
                checked = invoke(binary, "check")
                check_records = records_of(checked, source)
                assert checked.returncode == 0 and len(check_records) == 1 and check_records[0]["kind"] == "check-summary" and check_records[0]["success"] and check_records[0]["errors"] == 0, (category, source[:1000], check_records)
                reference = invoke(binary, "run", json_mode=False)
                json_run = invoke(binary, "run")
                records = records_of(json_run, source)
                summary = records[-1]
                assert summary["kind"] == "run-summary"
                if error is None:
                    assert value is not None
                    expected = (0, (value.text() + "\n").encode(), b"")
                    assert len(records) == 1 and summary["success"] and summary["errors"] == 0 and summary["result"] == value.record(), (source[:1000], value, records)
                    if value.tag != "unit":
                        assert type(summary["result"]["value"]) is type(value.value), (value, records)
                else:
                    assert len(records) == 2 and not summary["success"] and summary["errors"] == 1 and summary["result"] is None, (source[:1000], records)
                    assert records[0]["code"] == "E0604" and records[0]["stage"] == "oir-run", (source[:1000], records)
                    assert_origin(source, records[0]["primary"], error, 1)
                    # Derive the complete existing human error independently,
                    # including scalar columns and escaped path controls.
                    line = source[:error].count("\n") + 1
                    column = len(source[:error].rsplit("\n", 1)[-1]) + 1
                    display_name = name.replace("\n", "\\n").replace("\t", "\\t").replace("\x1b", "\\u{1b}")
                    message = f"error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> {display_name}:{line}:{column}\n"
                    expected = (1, b"", message.encode())
                    assert records[0]["message"] == "checked i32 arithmetic overflow"
                assert (reference.returncode, reference.stdout, reference.stderr) == expected, (source[:1000], reference, expected)
                assert json_run.returncode == expected[0]
                observed = check_records, expected, records
                if previous is not None:
                    assert observed == previous, (source[:1000], "compiler-profile result mismatch")
                previous = observed
                output = root / f"native-{number}-{profile}"
                compiled = invoke(binary, "compile", output=output)
                compile_records = records_of(compiled, source)
                assert compiled.returncode == 0 and len(compile_records) == 1, (category, source[:1000], compiled)
                compile_summary = compile_records[0]
                assert compile_summary["kind"] == "compile-summary" and compile_summary["success"] and compile_summary["errors"] == 0 and compile_summary["output"] == str(output), compile_summary
                assert not list(root.glob(".oxid-native-*"))
                # No source, Oxid, LLVM, LD_LIBRARY_PATH or Python is available.
                path.unlink()
                try:
                    native = command([str(output)], cwd=root, env=clean_env)
                finally:
                    path.write_bytes(source.encode())
                assert (native.returncode, native.stdout, native.stderr) == expected, (category, source[:1000], native, expected)
                artifact = output.read_bytes()
                assert artifact[:4] == b"\x7fELF"
                digest = hashlib.sha256(artifact).digest()
                if previous_artifact is not None:
                    assert digest == previous_artifact, (source[:1000], "artifact compiler-profile mismatch")
                previous_artifact = digest
                artifact_hashes.update(digest)
                io_kind = "error" if error is not None else value.text()
                if io_kind in ("true", "false", "error") and io_kind not in tested_io:
                    dynamic = command(["readelf", "-d", str(output)])
                    needed = [line for line in dynamic.stdout.splitlines() if b"(NEEDED)" in line]
                    assert dynamic.returncode == 0 and len(needed) == 1 and b"[libc.so.6]" in needed[0], dynamic
                    header = command(["readelf", "-h", str(output)])
                    assert header.returncode == 0 and b"DYN (Position-Independent Executable file)" in header.stdout and b"Advanced Micro Devices X86-64" in header.stdout
                    with open("/dev/full", "wb") as full:
                        failed = subprocess.run([str(output)], stdout=subprocess.PIPE if error is not None else full, stderr=full if error is not None else subprocess.PIPE, env=clean_env, timeout=10)
                    assert failed.returncode == 74 and (failed.stdout if error is not None else failed.stderr) == b"", failed
                    read_fd, write_fd = os.pipe()
                    os.close(read_fd)
                    try:
                        failed = subprocess.run([str(output)], stdout=subprocess.PIPE if error is not None else write_fd, stderr=write_fd if error is not None else subprocess.PIPE, env=clean_env, timeout=10)
                    finally:
                        os.close(write_fd)
                    assert failed.returncode == 74 and (failed.stdout if error is not None else failed.stderr) == b"", failed
                    tested_io.add(io_kind)
                output.unlink()
                count["compiled_artifacts"] += 1
            count["source_cases"] += 1
            count["source_model_traces"] += bool(trace)
            count["success_cases" if error is None else "overflow_cases"] += 1
            categories[category] = categories.get(category, 0) + 1
        assert tested_io == {"true", "false", "error"}
        for category, source, code, stage, offset, width in negatives:
            path.write_bytes(source.encode())
            previous = {}
            for binary in binaries:
                for operation in ("check", "run", "compile"):
                    output = root / "invalid-output"
                    result = invoke(binary, operation, output=output, missing_tools=True)
                    records = records_of(result, source)
                    assert result.returncode == 1 and len(records) == 2, (category, source[:1000], records)
                    assert records[0]["code"] == code and records[0]["stage"] == stage, (category, source[:1000], code, stage, records)
                    if offset is not None:
                        assert_origin(source, records[0]["primary"], offset, width)
                    summary = records[-1]
                    assert summary["kind"] == f"{operation}-summary" and not summary["success"] and summary["errors"] == 1
                    if operation == "run":
                        assert summary["result"] is None
                    observed = result.returncode, records
                    if operation in previous:
                        assert observed == previous[operation], (category, "negative profile mismatch")
                    previous[operation] = observed
                    assert not output.exists() and not list(root.glob(".oxid-native-*"))
            count["negative_cases"] += 1
            categories[category] = categories.get(category, 0) + 1
        for category, source, admitted in native_resource_cases():
            path.write_bytes(source.encode())
            previous = None
            previous_resource_digest = None
            for binary in binaries:
                checked = invoke(binary, "check")
                check_records = records_of(checked, source)
                assert checked.returncode == 0 and len(check_records) == 1 and check_records[0]["success"], (category, check_records)
                output = root / "resource-output"
                compiled = invoke(binary, "compile", output=output, missing_tools=True)
                records = records_of(compiled, source)
                code = "E0701" if admitted else "E0700"
                stage = "native-toolchain" if admitted else "native-admission"
                assert compiled.returncode == 1 and len(records) == 2 and records[0]["code"] == code and records[0]["stage"] == stage, (category, admitted, records)
                if previous is not None:
                    assert records == previous, (category, "resource profile mismatch")
                previous = records
                assert not output.exists() and not list(root.glob(".oxid-native-*"))
                if admitted:
                    # Admission boundaries also pass the real toolchain and
                    # standalone execution; missing-tool classification alone
                    # cannot establish that a large admitted CFG really works.
                    value = Scalar("unit", None) if category in ("function_slots", "aggregate_slots", "aggregate_blocks") else Scalar("bool", True)
                    expected = (0, (value.text() + "\n").encode(), b"")
                    reference = invoke(binary, "run", json_mode=False)
                    assert (reference.returncode, reference.stdout, reference.stderr) == expected, (category, reference)
                    result = invoke(binary, "run")
                    result_records = records_of(result, source)
                    assert result.returncode == 0 and len(result_records) == 1 and result_records[0]["result"] == value.record(), (category, result_records)
                    compiled = invoke(binary, "compile", output=output)
                    compile_records = records_of(compiled, source)
                    assert compiled.returncode == 0 and len(compile_records) == 1 and compile_records[0]["success"] and compile_records[0]["output"] == str(output), (category, compiled)
                    path.unlink()
                    try:
                        native = command([str(output)], cwd=root, env=clean_env)
                    finally:
                        path.write_bytes(source.encode())
                    assert (native.returncode, native.stdout, native.stderr) == expected, (category, native, expected)
                    artifact = output.read_bytes()
                    assert artifact[:4] == b"\x7fELF"
                    digest = hashlib.sha256(artifact).digest()
                    if previous_resource_digest is not None:
                        assert digest == previous_resource_digest, (category, "resource artifact profile mismatch")
                    previous_resource_digest = digest
                    artifact_hashes.update(digest)
                    output.unlink()
                    assert not list(root.glob(".oxid-native-*"))
                    count["compiled_resource_artifacts"] += 1
            count["native_resource_cases"] += 1
        for source, value, code in reference_only_cases():
            path.write_bytes(source.encode())
            for binary in binaries:
                checked = invoke(binary, "check")
                checked_records = records_of(checked, source)
                assert checked.returncode == 0 and len(checked_records) == 1 and checked_records[0]["success"], (source[:1000], checked_records)
                result = invoke(binary, "run")
                records = records_of(result, source)
                if code is None:
                    assert result.returncode == 0 and len(records) == 1 and records[0]["result"] == value.record(), (source[:1000], records)
                else:
                    assert result.returncode == 1 and len(records) == 2 and records[0]["code"] == code and records[0]["stage"] == "oir-run", (source[:1000], records)
                    assert records[-1]["result"] is None and records[-1]["errors"] == 1
            count["reference_only_cases"] += 1
        verify_adapter(root)
    assert categories["truth_table"] == 8 and categories["call_truth_table"] == 8
    assert compiler_hashes == [hashlib.sha256(Path(binary).read_bytes()).hexdigest() for binary in binaries], "compiler binaries changed while oracle was running"
    print(json.dumps(dict(count, categories=categories, compiler_sha256=compiler_hashes,
                          negative_expectation_amendment=amendment,
                          corpus_sha256=source_hashes.hexdigest(), artifact_manifest_sha256=artifact_hashes.hexdigest()), sort_keys=True))
    print("boolean logic O0: lazy tagged Python/reference/native parity, source-model call-order witnesses, full static RHS checks, exact resource boundaries, first-error origins, profile-identical standalone ELF and output failures: PASS")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binaries", nargs="+")
    parser.add_argument("--expectation-amendment", choices=(UNARY_EXPECTATION_AMENDMENT_ID,),
                        help="explicit current-source expectation; preserves the historical negative corpus")
    args = parser.parse_args(argv)
    if sys.flags.optimize:
        parser.error("Python assertions must remain enabled")
    verify(args.binaries, expectation_amendment=args.expectation_amendment)


if __name__ == "__main__":
    main()
