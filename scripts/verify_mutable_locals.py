#!/usr/bin/env python3
"""Independent mutable scalar model / reference / mandatory LLVM O0 parity.

Usage: python3 scripts/verify_mutable_locals.py target/debug/oxid target/release/oxid
Requires real pinned LLVM/Clang/LLD 19.1.7. Missing tools fail. The oracle owns
its AST, types, lexical frames, mutable cells, branch choices, call frames and
arbitrary-precision checked arithmetic; no compiler output supplies expectations.
Model traces establish independent witnesses, not observation of compiler stores.
Native programs run with source removed and no compiler/runtime tools in PATH.
"""
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile

MIN, MAX = -(2 ** 31), 2 ** 31 - 1


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


def type_text(tag):
    return "()" if tag == "unit" else tag


@dataclass(frozen=True)
class Expr:
    kind: str
    atom: object = None
    children: tuple = ()

    def text(self):
        if self.kind == "constant":
            return self.atom.text()
        if self.kind == "name":
            return self.atom
        if self.kind == "call":
            return self.atom + "(" + ", ".join(x.text() for x in self.children) + ")"
        if self.kind == "not":
            return "!" + self.children[0].text()
        assert self.kind == "binary"
        return "(" + self.children[0].text() + " " + self.atom + " " + self.children[1].text() + ")"


def i(value):
    return Expr("constant", Scalar("i32", value))


def b(value):
    return Expr("constant", Scalar("bool", value))


def u():
    return Expr("constant", Scalar("unit", None))


def v(name):
    return Expr("name", name)


def op(left, operator, right):
    return Expr("binary", operator, (left, right))


def call(name, *args):
    return Expr("call", name, args)


def inv(child):
    return Expr("not", children=(child,))


@dataclass(frozen=True)
class Stmt:
    kind: str
    expression: Expr
    name: str = ""
    mutable: bool = False
    annotation: str = ""
    arms: tuple = ()

    def prefix(self):
        if self.kind == "let":
            return "let " + ("mut " if self.mutable else "") + self.name + (": " + type_text(self.annotation) if self.annotation else "") + " = "
        return {"store": self.name + " = ", "return": "return ", "discard": "", "if": "if "}[self.kind]

    def text(self):
        text = self.prefix() + self.expression.text()
        if self.kind == "if":
            text += " " + block_text(self.arms[0])
            if len(self.arms) == 2:
                text += " else " + block_text(self.arms[1])
            return text
        return text + ";"


def block_text(body):
    return "{ " + " ".join(s.text() for s in body) + " }"


def let(name, expression, mutable=False, annotation=""):
    return Stmt("let", expression, name, mutable, annotation)


def store(name, expression):
    return Stmt("store", expression, name)


def ret(expression):
    return Stmt("return", expression)


def discard(expression):
    return Stmt("discard", expression)


def branch(condition, yes, no=None):
    return Stmt("if", condition, arms=(tuple(yes),) if no is None else (tuple(yes), tuple(no)))


@dataclass(frozen=True)
class Function:
    name: str
    parameters: tuple
    result: str
    body: tuple

    def prefix(self):
        parameters = ", ".join(name + ": " + type_text(tag) for name, tag in self.parameters)
        return "fn " + self.name + "(" + parameters + ") -> " + type_text(self.result) + " "


@dataclass
class Cell:
    tag: str
    mutable: bool
    value: Scalar | None = None


class Scope:
    def __init__(self, functions, parent=None):
        self.functions, self.parent, self.cells = functions, parent, {}

    def find(self, name):
        if name in self.cells:
            return self.cells[name]
        assert self.parent is not None, ("unknown local", name)
        return self.parent.find(name)

    def declare(self, name, cell):
        assert name not in self.functions
        ancestor = self
        while ancestor is not None:
            assert name not in ancestor.cells, ("shadowing", name)
            ancestor = ancestor.parent
        self.cells[name] = cell


def expression_type(expression, scope, functions):
    if expression.kind == "constant":
        return expression.atom.tag
    if expression.kind == "name":
        return scope.find(expression.atom).tag
    if expression.kind == "call":
        function, _ = functions[expression.atom]
        assert len(function.parameters) == len(expression.children)
        for child, (_, tag) in zip(expression.children, function.parameters):
            assert expression_type(child, scope, functions) == tag
        return function.result
    left = expression_type(expression.children[0], scope, functions)
    if expression.kind == "not":
        assert left == "bool"
        return "bool"
    right = expression_type(expression.children[1], scope, functions)
    if expression.atom in ("+", "-", "*"):
        assert left == right == "i32"
        return "i32"
    if expression.atom in ("&&", "||"):
        assert left == right == "bool"
    else:
        assert left == right and left in ("bool", "i32")
        assert expression.atom in ("==", "!=") or (left == "i32" and expression.atom in ("<", "<=", ">", ">="))
    return "bool"


def check_block(body, parent, functions, result):
    scope = Scope(functions, parent)
    returned = False
    for statement in body:
        assert not returned, "unreachable statement"
        tag = expression_type(statement.expression, scope, functions)
        if statement.kind == "let":
            assert not statement.annotation or statement.annotation == tag
            scope.declare(statement.name, Cell(tag, statement.mutable))
        elif statement.kind == "store":
            cell = scope.find(statement.name)
            assert cell.mutable and cell.tag == tag
        elif statement.kind == "return":
            assert tag == result
            returned = True
        elif statement.kind == "if":
            assert tag == "bool"
            outcomes = [check_block(arm, scope, functions, result) for arm in statement.arms]
            returned = len(outcomes) == 2 and all(outcomes)
    return returned


class Overflow(Exception):
    def __init__(self, offset):
        self.offset = offset


class Returned(Exception):
    def __init__(self, value):
        self.value = value


class Machine:
    def __init__(self, functions):
        self.functions, self.trace = functions, []

    def invoke(self, name, arguments):
        function, offset = self.functions[name]
        assert len(arguments) == len(function.parameters)
        frame = Scope(self.functions)
        for (parameter, tag), value in zip(function.parameters, arguments):
            assert tag == value.tag
            frame.declare(parameter, Cell(tag, False, value))
        self.trace.append(("enter", name))
        try:
            self.block(function.body, offset, frame)
        except Returned as returned:
            assert returned.value.tag == function.result
            self.trace.append(("return", name, returned.value))
            return returned.value
        raise AssertionError("function fell through")

    def expression(self, expression, offset, scope):
        if expression.kind == "constant":
            return expression.atom
        if expression.kind == "name":
            value = scope.find(expression.atom).value
            assert value is not None
            self.trace.append(("read", expression.atom, value))
            return value
        if expression.kind == "call":
            position, arguments = offset + len(expression.atom) + 1, []
            for child in expression.children:
                arguments.append(self.expression(child, position, scope))
                position += len(child.text()) + 2
            return self.invoke(expression.atom, arguments)
        if expression.kind == "not":
            value = self.expression(expression.children[0], offset + 1, scope)
            assert value.tag == "bool"
            return Scalar("bool", not value.value)
        left, right = expression.children
        lv = self.expression(left, offset + 1, scope)
        operator = expression.atom
        if operator in ("&&", "||"):
            assert lv.tag == "bool"
            if (operator == "&&" and not lv.value) or (operator == "||" and lv.value):
                self.trace.append(("skip", operator))
                return lv
        rv = self.expression(right, offset + len(left.text()) + len(operator) + 3, scope)
        if operator in ("&&", "||"):
            assert rv.tag == "bool"
            return rv
        if operator in ("+", "-", "*"):
            assert lv.tag == rv.tag == "i32"
            value = {"+": lambda: lv.value + rv.value, "-": lambda: lv.value - rv.value, "*": lambda: lv.value * rv.value}[operator]()
            if not MIN <= value <= MAX:
                raise Overflow(offset + len(left.text()) + 2)
            return Scalar("i32", value)
        assert lv.tag == rv.tag and lv.tag in ("i32", "bool")
        assert operator in ("==", "!=") or lv.tag == "i32"
        value = {"==": lambda: lv.value == rv.value, "!=": lambda: lv.value != rv.value,
                 "<": lambda: lv.value < rv.value, "<=": lambda: lv.value <= rv.value,
                 ">": lambda: lv.value > rv.value, ">=": lambda: lv.value >= rv.value}[operator]()
        return Scalar("bool", value)

    def block(self, body, offset, parent):
        scope, position = Scope(self.functions, parent), offset + 2
        for statement in body:
            value = self.expression(statement.expression, position + len(statement.prefix()), scope)
            if statement.kind == "let":
                assert not statement.annotation or statement.annotation == value.tag
                scope.declare(statement.name, Cell(value.tag, statement.mutable, value))
                self.trace.append(("initialize", statement.name, value, statement.mutable))
            elif statement.kind == "store":
                cell = scope.find(statement.name)
                assert cell.mutable and cell.tag == value.tag
                self.trace.append(("store", statement.name, cell.value, value))
                cell.value = value
            elif statement.kind == "return":
                raise Returned(value)
            elif statement.kind == "if":
                assert value.tag == "bool"
                self.trace.append(("branch", value.value))
                arm_offset = position + len(statement.prefix()) + len(statement.expression.text()) + 1
                if value.value:
                    self.block(statement.arms[0], arm_offset, scope)
                elif len(statement.arms) == 2:
                    self.block(statement.arms[1], arm_offset + len(block_text(statement.arms[0])) + 6, scope)
            position += len(statement.text()) + 1


HELPERS = (
    Function("bump", (("p", "i32"),), "i32", (let("x", v("p"), True), store("x", op(v("x"), "+", i(1))), ret(v("x")))),
    Function("flip", (("p", "bool"),), "bool", (let("x", v("p"), True), store("x", inv(v("x"))), ret(v("x")))),
    Function("unit", (("p", "unit"),), "unit", (let("x", v("p"), True), store("x", u()), ret(v("x")))),
    Function("first", (), "i32", (ret(op(i(MAX), "+", i(1))),)),
    Function("later", (), "i32", (ret(op(i(MIN), "-", i(1))),)),
    Function("pair", (("a", "i32"), ("b", "i32")), "i32", (ret(v("b")),)),
)


def modeled_case(category, body, result="i32", helpers=HELPERS):
    source, functions = "// 雪🦀\r\n", {}
    for function in (*helpers, Function("main", (), result, tuple(body))):
        functions[function.name] = function, len(source) + len(function.prefix())
        source += function.prefix() + block_text(function.body) + "\r\n"
    for function, _ in functions.values():
        parameters = Scope(functions)
        for name, tag in function.parameters:
            parameters.declare(name, Cell(tag, False))
        assert check_block(function.body, parameters, functions, function.result)
    machine = Machine(functions)
    try:
        value, error = machine.invoke("main", []), None
    except Overflow as failure:
        value, error = None, failure.offset
        assert source[error] in "+-*"
    return category, source, value, error, tuple(machine.trace)


def self_check_model():
    for invalid in [("i32", True), ("bool", 1), ("unit", 0), ("i32", MAX + 1)]:
        try:
            Scalar(*invalid)
        except AssertionError:
            pass
        else:
            raise AssertionError(("coerced scalar", invalid))
    case = modeled_case("self", [let("x", i(7), True), let("old", v("x")), store("x", op(v("x"), "+", i(5))), ret(op(op(v("old"), "*", i(100)), "+", v("x")))])
    assert case[2] == Scalar("i32", 712) and case[3] is None
    for bad in [store("x", b(True)), store("old", i(1))]:
        try:
            modeled_case("self", [let("x", i(1), True), let("old", i(1)), bad, ret(v("x"))])
        except AssertionError:
            pass
        else:
            raise AssertionError("bad assignment accepted")
    # RHS overflow cannot append a store, nor visit the later call or statement.
    case = modeled_case("self", [let("x", i(7), True), store("x", op(call("first"), "+", call("later"))), store("x", i(8)), ret(v("x"))])
    assert case[2] is None and case[3] is not None
    assert not any(event[0] == "store" for event in case[4])
    assert [event[1] for event in case[4] if event[0] == "enter"] == ["main", "first"]
    case = modeled_case("self", [let("x", i(10), True), store("x", call("bump", v("x"))), ret(v("x"))])
    assert case[2] == Scalar("i32", 11)
    assert [event[1] for event in case[4] if event[0] == "enter"] == ["main", "bump"]
    assert len([event for event in case[4] if event[0] == "store"]) == 2
    case = modeled_case("self", [let("x", b(False), True), store("x", op(b(True), "||", op(call("first"), "==", i(0)))), ret(v("x"))], "bool")
    assert case[2] == Scalar("bool", True)
    assert [event[1] for event in case[4] if event[0] == "enter"] == ["main"]
    # Both arms are checked even when the bad assignment is dormant.
    try:
        modeled_case("self", [let("x", i(1), True), branch(b(False), [store("x", b(True))]), ret(v("x"))])
    except AssertionError:
        pass
    else:
        raise AssertionError("unchosen assignment was not type checked")


def cases():
    yield modeled_case("snapshot", [let("x", i(7), True, "i32"), let("old", v("x")), store("x", op(v("x"), "+", i(5))), ret(op(op(v("old"), "*", i(100)), "+", v("x")))])
    for tag, initial, changed in [("bool", b(True), b(False)), ("i32", i(MIN), i(MAX)), ("unit", u(), u())]:
        for annotation in ("", tag):
            yield modeled_case("scalar_types", [let("x", initial, True, annotation), let("old", v("x")), store("x", changed), ret(v("x"))], tag)
            yield modeled_case("immutable_snapshot", [let("x", initial, True, annotation), let("old", v("x")), store("x", changed), ret(v("old"))], tag)
    for initial in (MIN, MIN + 1, -46341, -1, 0, 1, 46341, MAX - 1, MAX):
        for operator, rhs in (("+", i(1)), ("-", i(1)), ("*", i(-1)), ("+", v("x"))):
            yield modeled_case("checked_assignment", [let("x", i(initial), True), store("x", op(v("x"), operator, rhs)), ret(v("x"))])
    for left in (False, True):
        for right in (False, True):
            for operator in ("&&", "||"):
                yield modeled_case("logical_store", [let("x", b(left), True), store("x", op(v("x"), operator, call("flip", b(right)))), ret(v("x"))], "bool")
            yield modeled_case("branch_join", [let("x", i(1), True), branch(b(left), [store("x", i(10)), branch(b(right), [store("x", op(v("x"), "+", i(2)))], [store("x", op(v("x"), "+", i(3)))])], [store("x", i(20))]), store("x", op(v("x"), "*", i(2))), ret(v("x"))])
            yield modeled_case("sibling_scope", [let("x", i(1), True), branch(b(left), [let("y", i(10), True), store("y", i(12)), store("x", v("y"))], [let("y", i(20), True), store("y", i(23)), store("x", v("y"))]), let("y", i(5), True), store("y", op(v("y"), "+", v("x"))), ret(v("y"))])
    for condition in (False, True):
        yield modeled_case("missing_else", [let("x", i(7), True), branch(b(condition), [store("x", i(9))]), ret(v("x"))])
        yield modeled_case("returning_arm", [let("x", i(7), True), branch(b(condition), [store("x", i(9)), ret(v("x"))], [store("x", i(12))]), store("x", op(v("x"), "+", i(1))), ret(v("x"))])
        yield modeled_case("mutable_condition", [let("x", b(condition), True), let("y", i(1), True), branch(v("x"), [store("y", i(2))]), store("x", inv(v("x"))), branch(v("x"), [store("y", op(v("y"), "+", i(4)))]), ret(v("y"))])
        for operator in ("&&", "||"):
            yield modeled_case("skipped_or_executed_error", [let("x", b(condition), True), store("x", op(v("x"), operator, op(call("first"), "==", i(0)))), ret(v("x"))], "bool")
    for expression in [call("first"), call("later"), op(call("first"), "+", call("later")), op(call("later"), "+", call("first")), call("pair", call("first"), call("later")), call("pair", call("later"), call("first"))]:
        yield modeled_case("first_error", [let("x", i(8), True), store("x", expression), store("x", call("later")), ret(v("x"))])
    yield modeled_case("activation_isolation", [let("x", i(10), True), let("old", call("bump", v("x"))), store("x", call("bump", v("x"))), ret(op(op(v("old"), "*", i(100)), "+", call("bump", v("x"))))])
    yield modeled_case("unit_calls", [let("x", u(), True), store("x", call("unit", v("x"))), ret(v("x"))], "unit")
    rng = random.Random(0xA5516E)
    for _ in range(80):
        body = [let("x", i(rng.randrange(-1000, 1001)), True), let("y", i(rng.randrange(-1000, 1001)), True), let("old", v("x")), let("flag", b(bool(rng.randrange(2))), True)]
        for _ in range(6):
            target = rng.choice(("x", "y"))
            expression = op(v(target), rng.choice(("+", "-", "*")), i(rng.randrange(-20, 21)))
            if rng.randrange(2):
                body.append(branch(op(v("flag"), "||", op(v("x"), "<", v("y"))), [store(target, expression)], [store(target, call("bump", v(target)))]))
            else:
                body.append(store(target, expression))
            if rng.randrange(3) == 0:
                body.append(store("flag", call("flip", v("flag"))))
        body.append(ret(op(op(v("x"), "+", v("y")), "+", v("old"))))
        yield modeled_case("seeded_state_machine", body)


def negative_cases():
    prefix = "// 雪\r\nfn main() -> () { "
    for declaration in ("let x = 1;", "let x = true;", "let x = (); "):
        value = {"let x = 1;": "2", "let x = true;": "false", "let x = (); ": "()"}[declaration]
        source = prefix + declaration + " x = " + value + "; return; }"
        yield "immutable_target", source, "E0304", "type", source.rindex("x ="), 1
    source = "fn unused(x: i32) -> () { x = 2; return; } fn main() -> () { return; }"
    yield "immutable_parameter", source, "E0304", "type", source.index("x ="), 1
    for initial, wrong in (("1", "true"), ("1", "()"), ("true", "1"), ("true", "()"), ("()", "1"), ("()", "false")):
        for dormant in (False, True):
            source = prefix + "let mut x = " + initial + "; " + ("if false { " if dormant else "") + "x = " + wrong + "; " + ("} " if dormant else "") + "return; }"
            yield "fixed_types", source, "E0300", "type", source.rindex(wrong), len(wrong)
    for body, marker in [
        ("missing = 1;", "missing"), ("let mut x = x;", "x"),
        ("x = 1; let mut x = 2;", "x"),
        ("if true { let mut x = 1; } x = 2;", "x"),
        ("if true { let mut x = 1; } else { x = 2; }", "x"),
        ("let mut x = 1; if false { x = missing; }", "missing"),
    ]:
        source = prefix + body + " return; }"
        offset = source.index("x = 1") if body.startswith("x =") else source.rindex(marker)
        yield "scope", source, "E0200", "resolve", offset, len(marker)
    source = "fn f() -> () { return; } fn main() -> () { f = (); return; }"
    yield "function_target", source, "E0200", "resolve", source.index("f ="), 1
    for body in ["let mut x = 1; let x = 2;", "let x = 1; let mut x = 2;", "let mut x = 1; if true { let mut x = 2; }"]:
        source = prefix + body + " return; }"
        yield "shadowing", source, "E0201", "resolve", source.rindex("x ="), 1
    for body in ["let mut x;", "let mut x: i32;", "let mut x = ;", "let mut = 1;", "let mut mut x = 1;",
                 "let mut x = 1; x = ;", "let mut x = 1; let y = (x = 2);", "let mut x = 1; let mut y = 2; x = y = 3;",
                 "let mut x = true; if x = false { return; }", "let mut x = 1; (x) = 2;", "1 = 2;",
                 "let mut x = 1; x += 1;", "let mut x = 1; x -= 1;", "let mut x = 1; x *= 1;"]:
        yield "statement_only_syntax", prefix + body + " return; }", "E0100", "parse", None, None
    for source in ["fn f(mut x: i32) -> () { return; }", "fn main() -> i32 { let mut x = 1; return x = 2; }",
                   "fn f(x: i32) -> () { return; } fn main() -> () { let mut x = 1; f(x = 2); return; }",
                   "fn f() -> i32 { return 1; } fn main() -> () { f() = 2; return; }"]:
        yield "statement_only_syntax", source, "E0100", "parse", None, None
    for keyword in ("while", "for", "loop"):
        yield "unsupported_control", prefix + "let mut x = 1; " + keyword + " true { x = 2; } return; }", "E0101", "parse", None, None
    yield "rhs_height", "fn main() -> bool { let mut x = true; x = " + "!" * 64 + "x; return x; }", "E0400", "parse", None, None
    source = token_boundary_source()
    yield "token_limit", source + "/*x*/", "E0400", "lex", len(source), 5


def token_boundary_source():
    # Header 16 tokens, return suffix 5, each x=0; is 4. Three independent
    # trailing trivia tokens bring 16 + 4*24994 + 5 + 3 to exactly 100000.
    return "fn main()->i32{let mut x=0;" + "x=0;" * 24994 + "return x;} /*a*/ "


def native_resource_cases():
    for over in (False, True):
        # One place + initializer + 253 RHS constants + final read = 256 slots.
        body = "let mut x=0;" + "x=0;" * (254 if over else 253) + "return x;"
        yield "function_slots", "fn main()->i32{" + body + "}", not over
    for over in (False, True):
        body = "let mut x=0;" + "x=0;" * 253 + "return x;"
        source = "".join("fn " + ("main" if index == 0 else "f" + str(index)) + "()->i32{" + body + "}" for index in range(32))
        if over:
            source += "fn extra()->(){return;}"
        yield "aggregate_slots", source, not over
    for count in (256, 257):
        source = "".join("fn " + ("main" if index == 0 else "f" + str(index)) + "()->i32{let mut x=0;x=1;return x;}" for index in range(count))
        yield "function_count", source, count == 256
    for depth in (32, 33):
        source = ""
        for index in range(depth):
            name = "main" if index == 0 else "f" + str(index)
            value = "0" if index + 1 == depth else "f" + str(index + 1) + "()"
            source += "fn " + name + "()->i32{let mut x=1;x=" + value + ";return x;}"
        yield "call_depth", source, depth == 32
    # f0 has four slots (two constants, place, load), five instructions
    # (constants, init, store, load) and return: C0=10. Doubled callers each
    # add two value slots, two call edges and return: Cn=5+2*C(n-1).
    # Main calls f12/f11/f9 and returns f0. These four calls cost 99,835;
    # their result slots/edges, main return and root entry add 10. Fillers
    # add 5*17 + 6*11 + 2*2 =155, reaching exactly 100,000. Replacing the
    # two constants (4) by cheap() (5) gives +1. Init/store/load all count.
    source = "fn f0()->i32{let mut x=0;x=0;return x;}"
    costs = [10]
    for index in range(1, 13):
        source += f"fn f{index}()->i32{{f{index - 1}();return f{index - 1}();}}"
        costs.append(5 + 2 * costs[-1])
    source += "fn p17()->bool{0<1;0<1;return true;}fn p11()->bool{0<1;return true;}fn cheap()->bool{return true;}"
    assert sum(costs[index] for index in (12, 11, 9, 0)) + 10 + 5 * 17 + 6 * 11 + 2 * 2 == 100_000
    for over in (False, True):
        filler = "cheap();" if over else "true;true;"
        main = "fn main()->i32{f12();f11();f9();" + "p17();" * 5 + "p11();" * 6 + filler + "return f0();}"
        yield "native_fuel", source + main, not over
    for expression in ("false && recursive()", "true || recursive()"):
        source = "fn recursive()->bool{let mut x=true;x=recursive();return x;} fn main()->bool{let mut x=false;x=" + expression + ";return x;}"
        yield "skipped_recursion", source, False
    yield "unused_recursion", "fn recursive()->i32{let mut x=1;x=recursive();return x;}fn main()->i32{let mut x=0;return x;}", False


def reference_only_cases():
    yield "large_store_sequence", "fn main()->i32{let mut x=0;" + "x=x+1;" * 4000 + "return x;}", Scalar("i32", 4000), None
    yield "exact_token_boundary", token_boundary_source(), Scalar("i32", 0), None
    source = "fn sum(n:i32)->i32{let mut x=n;if n==0{return x;}x=x+sum(n-1);return x;}fn main()->i32{let mut x=100;x=x+sum(4);return x;}"
    yield "recursive_isolation", source, Scalar("i32", 110), None
    helper = "fn recursive()->bool{let mut x=true;x=recursive();return x;}"
    for expression, result, code in [("false && recursive()", False, None), ("true || recursive()", True, None), ("true && recursive()", None, "E0602"), ("false || recursive()", None, "E0602")]:
        yield "recursive_rhs", helper + "fn main()->bool{let mut x=false;x=" + expression + ";return x;}", None if result is None else Scalar("bool", result), code


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


def verify(binaries):
    self_check_model()
    binaries = [str(Path(binary).resolve()) for binary in binaries]
    counts = dict(binaries=len(binaries), source_cases=0, success_cases=0, overflow_cases=0,
                  negative_cases=0, native_resource_cases=0, reference_only_cases=0,
                  compiled_artifacts=0, compiled_resource_artifacts=0, invocations=0,
                  modeled_stores=0, modeled_calls=0, modeled_branches=0)
    categories = {}
    corpus_hash, artifact_hash, trace_hash = hashlib.sha256(), hashlib.sha256(), hashlib.sha256()
    compiler_hashes = [hashlib.sha256(Path(binary).read_bytes()).hexdigest() for binary in binaries]
    with tempfile.TemporaryDirectory(prefix="oxid-mutable-oracle-") as directory:
        root = Path(directory)
        name = 'mutable 雪;\'"\\\n\t\x1b.ox'
        source_path = root / name
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
            counts["invocations"] += 1
            return result

        def check(binary, source):
            result = invoke(binary, "check")
            records = records_of(result, source)
            assert result.returncode == 0 and len(records) == 1 and records[0]["kind"] == "check-summary" and records[0]["success"] and records[0]["errors"] == 0, (source[:1000], result)
            return records

        def runtime(binary, source, value, error):
            result = invoke(binary, "run")
            records = records_of(result, source)
            summary = records[-1]
            assert summary["kind"] == "run-summary"
            if error is None:
                assert value is not None
                expected = (0, (value.text() + "\n").encode(), b"")
                assert len(records) == 1 and summary["success"] and summary["errors"] == 0 and summary["result"] == value.record(), (source[:1000], records, value)
                if value.tag != "unit":
                    assert type(summary["result"]["value"]) is type(value.value)
            else:
                assert len(records) == 2 and not summary["success"] and summary["errors"] == 1 and summary["result"] is None, (source[:1000], records)
                assert records[0]["code"] == "E0604" and records[0]["stage"] == "oir-run"
                assert_origin(source, records[0]["primary"], error, 1)
                line, column = source[:error].count("\n") + 1, len(source[:error].rsplit("\n", 1)[-1]) + 1
                display_name = name.replace("\n", "\\n").replace("\t", "\\t").replace("\x1b", "\\u{1b}")
                expected = (1, b"", f"error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> {display_name}:{line}:{column}\n".encode())
            assert result.returncode == expected[0]
            text = invoke(binary, "run", json_mode=False)
            assert (text.returncode, text.stdout, text.stderr) == expected, (source[:1000], text, expected)
            return records, expected

        def native(binary, source, output, expected):
            result = invoke(binary, "compile", output=output)
            records = records_of(result, source)
            assert result.returncode == 0 and len(records) == 1 and records[0]["success"] and records[0]["kind"] == "compile-summary" and records[0]["output"] == str(output), (source[:1000], result)
            assert not list(root.glob(".oxid-native-*"))
            source_path.unlink()
            try:
                executed = command([str(output)], cwd=root, env=clean_env)
            finally:
                source_path.write_bytes(source.encode())
            assert (executed.returncode, executed.stdout, executed.stderr) == expected, (source[:1000], executed, expected)
            artifact = output.read_bytes()
            assert artifact[:4] == b"\x7fELF"
            digest = hashlib.sha256(artifact).digest()
            artifact_hash.update(digest)
            return digest

        for number, (category, source, value, error, trace) in enumerate(cases()):
            source_path.write_bytes(source.encode())
            corpus_hash.update(source.encode() + b"\0")
            trace_hash.update(repr(trace).encode() + b"\0")
            previous = previous_artifact = None
            for profile, binary in enumerate(binaries):
                checked = check(binary, source)
                observed, expected = runtime(binary, source, value, error)
                if previous is not None:
                    assert previous == (checked, observed, expected), "compiler profile mismatch"
                previous = checked, observed, expected
                output = root / f"native-{number}-{profile}"
                digest = native(binary, source, output, expected)
                if previous_artifact is not None:
                    assert previous_artifact == digest, "artifact compiler profile mismatch"
                previous_artifact = digest
                io_kind = "error" if error is not None else value.tag
                if io_kind not in tested_io:
                    dynamic = command(["readelf", "-d", str(output)])
                    needed = [line for line in dynamic.stdout.splitlines() if b"(NEEDED)" in line]
                    assert dynamic.returncode == 0 and len(needed) == 1 and b"[libc.so.6]" in needed[0]
                    with open("/dev/full", "wb") as full:
                        failure = subprocess.run([str(output)], stdout=subprocess.PIPE if error is not None else full, stderr=full if error is not None else subprocess.PIPE, env=clean_env, timeout=10)
                    assert failure.returncode == 74 and (failure.stdout if error is not None else failure.stderr) == b"", failure
                    read_fd, write_fd = os.pipe()
                    os.close(read_fd)
                    try:
                        failure = subprocess.run([str(output)], stdout=subprocess.PIPE if error is not None else write_fd, stderr=write_fd if error is not None else subprocess.PIPE, env=clean_env, timeout=10)
                    finally:
                        os.close(write_fd)
                    assert failure.returncode == 74 and (failure.stdout if error is not None else failure.stderr) == b"", failure
                    tested_io.add(io_kind)
                output.unlink()
                counts["compiled_artifacts"] += 1
            counts["source_cases"] += 1
            counts["success_cases" if error is None else "overflow_cases"] += 1
            for event in trace:
                key = {"store": "modeled_stores", "enter": "modeled_calls", "branch": "modeled_branches"}.get(event[0])
                if key:
                    counts[key] += 1
            categories[category] = categories.get(category, 0) + 1
        assert tested_io == {"i32", "bool", "unit", "error"}
        for category, source, code, stage, offset, width in negative_cases():
            source_path.write_bytes(source.encode())
            corpus_hash.update(source.encode() + b"\0")
            previous = {}
            for binary in binaries:
                for operation in ("check", "run", "compile"):
                    output = root / "invalid-output"
                    result = invoke(binary, operation, output=output, missing_tools=True)
                    records = records_of(result, source)
                    assert result.returncode == 1 and len(records) == 2 and records[0]["code"] == code and records[0]["stage"] == stage, (category, source[:1000], code, stage, records)
                    if offset is not None:
                        assert_origin(source, records[0]["primary"], offset, width)
                    if code == "E0304" or category == "fixed_types":
                        assert len(records[0]["secondary"]) == 1
                        declaration = source.index("x")
                        assert_origin(source, records[0]["secondary"][0]["span"], declaration, 1)
                    summary = records[-1]
                    assert summary["kind"] == f"{operation}-summary" and not summary["success"] and summary["errors"] == 1
                    if operation == "run":
                        assert summary["result"] is None
                    if operation in previous:
                        assert previous[operation] == records, (category, "negative profile mismatch")
                    previous[operation] = records
                    assert not output.exists() and not list(root.glob(".oxid-native-*"))
            counts["negative_cases"] += 1
            categories[category] = categories.get(category, 0) + 1
        for category, source, admitted in native_resource_cases():
            source_path.write_bytes(source.encode())
            corpus_hash.update(source.encode() + b"\0")
            previous_artifact = None
            for binary in binaries:
                check(binary, source)
                output = root / "resource-output"
                result = invoke(binary, "compile", output=output, missing_tools=True)
                records = records_of(result, source)
                code, stage = ("E0701", "native-toolchain") if admitted else ("E0700", "native-admission")
                assert result.returncode == 1 and len(records) == 2 and records[0]["code"] == code and records[0]["stage"] == stage, (category, admitted, records)
                assert not output.exists() and not list(root.glob(".oxid-native-*"))
                if admitted:
                    value = Scalar("i32", 1 if category == "function_count" else 0)
                    _, expected = runtime(binary, source, value, None)
                    digest = native(binary, source, output, expected)
                    if previous_artifact is not None:
                        assert previous_artifact == digest, (category, "resource artifact mismatch")
                    previous_artifact = digest
                    output.unlink()
                    counts["compiled_resource_artifacts"] += 1
            counts["native_resource_cases"] += 1
        for category, source, value, code in reference_only_cases():
            source_path.write_bytes(source.encode())
            corpus_hash.update(source.encode() + b"\0")
            for binary in binaries:
                check(binary, source)
                if code is None:
                    runtime(binary, source, value, None)
                else:
                    result = invoke(binary, "run")
                    records = records_of(result, source)
                    assert result.returncode == 1 and len(records) == 2 and records[0]["code"] == code and records[0]["stage"] == "oir-run" and records[-1]["result"] is None, (category, result)
            counts["reference_only_cases"] += 1
        assert set(root.iterdir()) == {source_path}, list(root.iterdir())
    assert counts["modeled_stores"] > 100 and counts["modeled_calls"] > 100 and counts["modeled_branches"] > 100
    assert compiler_hashes == [hashlib.sha256(Path(binary).read_bytes()).hexdigest() for binary in binaries], "compiler changed during oracle run"
    print(json.dumps(dict(counts, categories=categories, compiler_sha256=compiler_hashes, corpus_sha256=corpus_hash.hexdigest(), artifact_manifest_sha256=artifact_hash.hexdigest(), source_model_trace_sha256=trace_hash.hexdigest()), sort_keys=True))
    print("mutable locals O0: independent typed state/call/branch model, checked i32, reference/native parity, exact diagnostics, resource boundaries and standalone profile-identical ELF: PASS")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    verify(sys.argv[1:])
