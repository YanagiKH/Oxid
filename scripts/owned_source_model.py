#!/usr/bin/env python3
"""Independent RFC0014 source oracle, deliberately separate from compiler IR.

The tagged source language, names/types, finite-state availability/capabilities,
dynamic scalar semantics, and template scheduler are independent implementations.
No compiler module, generated raw trace, or acceptance result defines an expected
answer. Fixture JSON is frozen before a candidate adapter is allowed to compare it.
"""
from __future__ import annotations

from collections import Counter, deque
from dataclasses import dataclass, field, replace
import hashlib
import itertools
import json
import re
from pathlib import Path
from typing import Any

SCALARS = frozenset(("i32", "bool", "()"))
MIN_I32, MAX_I32 = -(2 ** 31), 2 ** 31 - 1
SCHEMA = "oxid-owned-source-v1"
SEED = 1400403


@dataclass(frozen=True)
class Node:
    key: str
    tag: str
    data: dict[str, Any]


@dataclass(frozen=True)
class Record:
    name: str
    fields: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class Function:
    name: str
    parameters: tuple[tuple[str, str], ...]
    result: str
    body: Node
    inline: bool = False


@dataclass(frozen=True)
class Program:
    records: tuple[Record, ...]
    functions: tuple[Function, ...]
    preamble: str = ""
    newline: str = "\n"
    # Item order is optional; declaration collection is intentionally two-pass.
    order: tuple[tuple[str, int], ...] = ()


class Builder:
    """Case-local deterministic labels; argument evaluation is explicit in the AST."""
    def __init__(self):
        self.serial = 0

    def node(self, tag, **data):
        key = f"n{self.serial:04d}"
        self.serial += 1
        return Node(key, tag, data)

    def i(self, value): return self.node("int", value=value)
    def b(self, value): return self.node("bool", value=value)
    def u(self): return self.node("unit")
    def v(self, name): return self.node("name", name=name)
    def f(self, base, name): return self.node("field", base=base, name=name)
    def op(self, lhs, op, rhs): return self.node("binary", lhs=lhs, op=op, rhs=rhs)
    def unary(self, op, value): return self.node("unary", op=op, value=value)
    def group(self, value): return self.node("group", value=value)
    def lit(self, record, *fields): return self.node("literal", record=record, fields=tuple(fields))
    def call(self, function, *args): return self.node("call", function=function, args=tuple(args))
    def borrow(self, name, mutable=False, forwarded=False, trivia=" "):
        return self.node("borrow", name=name, mutable=mutable, forwarded=forwarded, trivia=trivia)
    def let(self, name, value, mutable=False, annotation=None):
        return self.node("let", name=name, value=value, mutable=mutable, annotation=annotation)
    def store(self, name, value): return self.node("store", name=name, value=value)
    def write(self, base, name, value): return self.node("write", base=base, name=name, value=value)
    def ret(self, value=None): return self.node("return", value=value)
    def discard(self, value): return self.node("discard", value=value)
    def block(self, *statements): return self.node("block", statements=tuple(statements))
    def iff(self, condition, yes, no=None): return self.node("if", condition=condition, yes=yes, no=no)
    def loop(self, condition, body): return self.node("while", condition=condition, body=body)
    def brk(self): return self.node("break")
    def cont(self): return self.node("continue")


@dataclass(frozen=True)
class Rendered:
    source: str
    origins: dict[str, tuple[int, int]]

    @property
    def sha256(self): return hashlib.sha256(self.source.encode()).hexdigest()

    def span(self, key): return list(self.origins[key])


_RESERVED_IDENTIFIERS = frozenset((
    "fn", "let", "mut", "return", "if", "else", "while", "break", "continue", "struct", "true", "false",
    "for", "loop", "match", "enum", "impl", "trait", "type", "const", "static", "pub", "use", "mod", "extern",
    "async", "await", "unsafe", "move", "ref", "in", "where", "self", "Self", "super", "crate", "as", "dyn", "null",
))


def _source_identifier(name):
    if not isinstance(name, str) or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name) is None or name in _RESERVED_IDENTIFIERS:
        raise ValueError("MODEL_DOMAIN: invalid or reserved source identifier")


def _source_type(ty, references_allowed=False):
    if ty == "()": return
    if not isinstance(ty, str): raise ValueError("MODEL_DOMAIN: type syntax must be a string")
    if ty.startswith("&") and not references_allowed:
        raise ValueError("MODEL_DOMAIN: reference type is only a parameter form; use authored source for syntax negatives")
    if ty.startswith("&mut "): ty = ty[5:]
    elif ty.startswith("&"): ty = ty[1:]
    _source_identifier(ty)


def _source_trivia(trivia):
    if not isinstance(trivia, str): raise ValueError("MODEL_DOMAIN: trivia must be a string")
    offset = 0
    while offset < len(trivia):
        if trivia[offset] in " \t\r\n\v\f": offset += 1
        elif trivia.startswith("//", offset):
            end = trivia.find("\n", offset + 2)
            if end < 0: raise ValueError("MODEL_DOMAIN: line-comment trivia must terminate before following syntax")
            offset = end + 1
        elif trivia.startswith("/*", offset):
            end = trivia.find("*/", offset + 2)
            if end < 0: raise ValueError("MODEL_DOMAIN: unterminated block-comment trivia")
            offset = end + 2
        else: raise ValueError("MODEL_DOMAIN: only whitespace/comments may precede source syntax")


def validate_render_domain(program):
    """Fail closed on tagged input that could render different source semantics.

    This bounded tagged domain is intentionally not an arbitrary-source parser.
    Syntactic exclusions outside it use independently authored exact-token cases.
    """
    if not isinstance(program, Program): raise ValueError("MODEL_DOMAIN: expected a Program")
    _source_trivia(program.preamble)
    if program.newline not in ("\n", "\r\n"): raise ValueError("MODEL_DOMAIN: newline must be LF or CRLF")
    wanted = [("record", i) for i in range(len(program.records))] + [("function", i) for i in range(len(program.functions))]
    if program.order:
        if (any(not isinstance(item, tuple) or len(item) != 2 for item in program.order)
                or len(program.order) != len(wanted)
                or any(kind not in ("record", "function") or type(index) is not int for kind, index in program.order)
                or sorted(program.order) != sorted(wanted)):
            raise ValueError("MODEL_DOMAIN: item order must be a complete declaration permutation")
    for record in program.records:
        if not isinstance(record, Record): raise ValueError("MODEL_DOMAIN: expected a Record")
        _source_identifier(record.name)
        for name, ty in record.fields: _source_identifier(name); _source_type(ty)
    expression_tags = {"int", "bool", "unit", "name", "field", "borrow", "literal", "call", "group", "unary", "binary"}
    statement_tags = {"let", "store", "write", "discard", "return", "if", "while", "break", "continue"}
    binary_ops = {"+", "-", "*", "/", "%", "==", "!=", "<", "<=", ">", ">=", "&&", "||"}
    def expr(n, condition_root=False, argument=False):
        if not isinstance(n, Node) or n.tag not in expression_tags: raise ValueError("MODEL_DOMAIN: unsupported expression form")
        d, tag = n.data, n.tag
        if tag == "unary" and d["op"] == "!" and d["value"].tag == "binary":
            raise ValueError("MODEL_DOMAIN: unary binary operand needs an explicit Group")
        if tag == "binary" and d["op"] in binary_ops:
            comparisons = {"==", "!=", "<", "<=", ">", ">="}
            precedence = {"||": 1, "&&": 2, "==": 3, "!=": 3, "<": 3, "<=": 3, ">": 3, ">=": 3, "+": 5, "-": 5, "*": 6, "/": 6, "%": 6}
            for side in ("lhs", "rhs"):
                child = d[side]
                if isinstance(child, Node) and child.tag == "binary" and child.data["op"] in binary_ops:
                    child_level, own_level = precedence[child.data["op"]], precedence[d["op"]]
                    if ((d["op"] in comparisons and child.data["op"] in comparisons)
                            or child_level < own_level or (side == "rhs" and child_level == own_level)):
                        raise ValueError("MODEL_DOMAIN: noncanonical expression needs an explicit Group")
        if tag == "int" and type(d["value"]) is not int: raise ValueError("MODEL_DOMAIN: integer payload must be exact int")
        if tag == "bool" and type(d["value"]) is not bool: raise ValueError("MODEL_DOMAIN: bool payload must be exact bool")
        if tag in ("name", "borrow"): _source_identifier(d["name"])
        if tag == "field": _source_identifier(d["base"]); _source_identifier(d["name"])
        if tag == "borrow":
            if not argument: raise ValueError("MODEL_DOMAIN: borrow node is only a complete direct-call argument")
            _source_trivia(d["trivia"])
            if d["mutable"] and not d["forwarded"] and not d["trivia"]:
                raise ValueError("MODEL_DOMAIN: mutable owner borrow needs trivia separating mut from its name")
            if type(d["mutable"]) is not bool or type(d["forwarded"]) is not bool: raise ValueError("MODEL_DOMAIN: borrow flags must be bool")
        if tag == "group": expr(d["value"])
        if tag == "unary":
            if d["op"] not in ("!", "-", "+"): raise ValueError("MODEL_DOMAIN: unsupported unary form")
            expr(d["value"], condition_root)
        if tag == "binary":
            if d["op"] not in binary_ops: raise ValueError("MODEL_DOMAIN: unsupported binary form")
            expr(d["lhs"], condition_root); expr(d["rhs"], condition_root)
        if tag == "literal":
            if condition_root: raise ValueError("MODEL_DOMAIN: condition-root literal needs an explicit Group")
            _source_identifier(d["record"])
            for name, value in d["fields"]: _source_identifier(name); expr(value)
        if tag == "call":
            _source_identifier(d["function"])
            for arg in d["args"]: expr(arg, argument=True)
    def block(n):
        if not isinstance(n, Node) or n.tag != "block": raise ValueError("MODEL_DOMAIN: expected a statement block")
        for statement in n.data["statements"]:
            if not isinstance(statement, Node) or statement.tag not in statement_tags: raise ValueError("MODEL_DOMAIN: unsupported statement form")
            d, tag = statement.data, statement.tag
            if tag in ("let", "store"): _source_identifier(d["name"])
            if tag == "write": _source_identifier(d["base"]); _source_identifier(d["name"])
            if tag == "let":
                if type(d["mutable"]) is not bool: raise ValueError("MODEL_DOMAIN: binding mutability must be bool")
                if d["annotation"] is not None: _source_type(d["annotation"])
            if d.get("value") is not None: expr(d["value"])
            if tag in ("if", "while"):
                expr(d["condition"], condition_root=True)
                block(d["yes"] if tag == "if" else d["body"])
                if tag == "if" and d["no"]: block(d["no"])
            if tag == "block": block(statement)
    for function in program.functions:
        if not isinstance(function, Function): raise ValueError("MODEL_DOMAIN: expected a Function")
        _source_identifier(function.name); _source_type(function.result)
        if type(function.inline) is not bool: raise ValueError("MODEL_DOMAIN: inline rendering flag must be bool")
        for name, ty in function.parameters: _source_identifier(name); _source_type(ty, references_allowed=True)
        block(function.body)


class Renderer:
    """Token origins are measured while writing, in UTF-8 bytes, never characters."""
    def __init__(self, program):
        validate_render_domain(program)
        self.program, self.parts, self.size, self.origins = program, [], 0, {}

    def put(self, text):
        self.parts.append(text)
        self.size += len(text.encode())

    def mark(self, key, callback):
        if key in self.origins:
            raise ValueError(f"AST node is shared or label duplicated: {key}")
        start = self.size
        callback()
        self.origins[key] = start, self.size

    def token(self, key, text): self.mark(key, lambda: self.put(text))

    def type_token(self, key, ty):
        if ty.startswith("&"):
            prefix = "&mut " if ty.startswith("&mut ") else "&"
            def body():
                self.put(prefix); self.token(key + ".referent", ty[len(prefix):])
            self.mark(key, body)
        else: self.token(key, ty)

    def expr(self, n):
        d, tag = n.data, n.tag
        if tag == "int" and type(d["value"]) is not int:
            raise ValueError("tagged integer literal requires an exact Python int")
        if tag == "bool" and type(d["value"]) is not bool:
            raise ValueError("tagged bool literal requires an exact Python bool")
        if tag == "unary" and d["op"] == "!" and d["value"].tag == "binary":
            raise ValueError("unary binary operand needs an explicit Group; implicit grouping changes source costs")
        if tag == "binary":
            comparisons = {"==", "!=", "<", "<=", ">", ">="}
            precedence = {"||": 1, "&&": 2, "==": 3, "!=": 3, "<": 3, "<=": 3, ">": 3, ">=": 3, "+": 5, "-": 5, "*": 6, "/": 6, "%": 6}
            if d["op"] not in precedence: raise ValueError("binary operator outside the tagged source domain")
            own = precedence[d["op"]]
            for side in ("lhs", "rhs"):
                child = d[side]
                if child.tag == "binary":
                    child_level = precedence[child.data["op"]]
                    if (d["op"] in comparisons and child.data["op"] in comparisons) or child_level < own or (side == "rhs" and child_level == own):
                        raise ValueError("noncanonical source AST needs an explicit Group; implicit grouping changes source costs")
        def body():
            if tag == "int": self.put(str(d["value"]))
            elif tag == "bool": self.put("true" if d["value"] else "false")
            elif tag == "unit": self.put("()")
            elif tag == "name": self.token(n.key + ".name", d["name"])
            elif tag == "field":
                self.token(n.key + ".base", d["base"]); self.put(".")
                self.token(n.key + ".field", d["name"])
            elif tag == "group":
                self.put("("); self.expr(d["value"]); self.put(")")
            elif tag == "unary":
                self.token(n.key + ".operator", d["op"]); self.expr(d["value"])
            elif tag == "binary":
                self.expr(d["lhs"]); self.put(" ")
                self.token(n.key + ".operator", d["op"]); self.put(" "); self.expr(d["rhs"])
            elif tag == "literal":
                self.token(n.key + ".record", d["record"]); self.put(" {")
                if d["fields"]: self.put(" ")
                for index, (name, value) in enumerate(d["fields"]):
                    if index: self.put(", ")
                    self.token(f"{n.key}.field.{index}", name); self.put(": "); self.expr(value)
                if d["fields"]: self.put(" ")
                self.put("}")
            elif tag == "borrow":
                self.put("&")
                if d["mutable"]: self.put("mut" + d["trivia"])
                elif d["trivia"] != " ": self.put(d["trivia"])
                if d["forwarded"]: self.put("*")
                self.token(n.key + ".name", d["name"])
            elif tag == "call":
                self.token(n.key + ".callee", d["function"]); self.put("(")
                for index, value in enumerate(d["args"]):
                    if index: self.put(", ")
                    self.expr(value)
                self.put(")")
            else: raise ValueError(f"not an expression: {tag}")
        self.mark(n.key, body)

    def stmt(self, n, depth, inline=False):
        d, tag = n.data, n.tag
        if not inline: self.put("    " * depth)
        def body():
            if tag == "let":
                self.put("let " + ("mut " if d["mutable"] else ""))
                self.token(n.key + ".name", d["name"])
                if d["annotation"]:
                    self.put(": "); self.token(n.key + ".annotation", d["annotation"])
                self.put(" = "); self.expr(d["value"]); self.put(";")
            elif tag in ("store", "write"):
                start = self.size
                self.token(n.key + ".base", d.get("base", d.get("name")))
                if tag == "write":
                    self.put("."); self.token(n.key + ".field", d["name"])
                self.origins[n.key + ".target"] = start, self.size
                self.put(" "); self.token(n.key + ".operator", "="); self.put(" ")
                self.expr(d["value"]); self.put(";")
            elif tag == "return":
                self.put("return")
                if d["value"] is not None:
                    self.put(" "); self.expr(d["value"])
                self.put(";")
            elif tag == "discard": self.expr(d["value"]); self.put(";")
            elif tag in ("break", "continue"): self.put(tag + ";")
            elif tag in ("if", "while"):
                self.put(tag + " "); self.expr(d["condition"]); self.put(" ")
                self.block(d["yes"] if tag == "if" else d["body"], depth)
                if tag == "if" and d["no"] is not None:
                    self.put(" else "); self.block(d["no"], depth)
            elif tag == "block": self.block(n, depth, mark=False)
            else: raise ValueError(tag)
        self.mark(n.key, body)
        self.put(" " if inline else self.program.newline)

    def block(self, n, depth, inline=False, mark=True):
        def body():
            self.put("{" + (" " if inline else self.program.newline))
            for statement in n.data["statements"]: self.stmt(statement, depth + 1, inline)
            if not inline: self.put("    " * depth)
            self.token(n.key + ".close", "}")
        self.mark(n.key, body) if mark else body()

    def render(self):
        self.put(self.program.preamble)
        items = self.program.order or tuple(("record", i) for i in range(len(self.program.records))) + tuple(("function", i) for i in range(len(self.program.functions)))
        for kind, index in items:
            if kind == "record":
                record = self.program.records[index]
                key = f"record.{index}"
                def body():
                    self.put("struct "); self.token(key + ".name", record.name); self.put(" {")
                    if record.fields: self.put(" ")
                    for position, (name, ty) in enumerate(record.fields):
                        if position: self.put(", ")
                        self.token(f"{key}.field.{position}.name", name); self.put(": ")
                        self.token(f"{key}.field.{position}.type", ty)
                    if record.fields: self.put(" ")
                    self.put("}")
                self.mark(key, body)
            else:
                fn = self.program.functions[index]
                key = f"function.{index}"
                def body():
                    self.put("fn "); self.token(key + ".name", fn.name); self.put("(")
                    for position, (name, ty) in enumerate(fn.parameters):
                        if position: self.put(", ")
                        self.token(f"{key}.parameter.{position}.name", name); self.put(": ")
                        self.type_token(f"{key}.parameter.{position}.type", ty)
                    self.put(") -> "); self.token(key + ".result", fn.result); self.put(" ")
                    self.block(fn.body, 0, fn.inline)
                self.mark(key, body)
            self.put(self.program.newline)
        return Rendered("".join(self.parts), self.origins)


def render(program): return Renderer(program).render()


def batch_program():
    b = Builder()
    retry = Function("retry", (("state", "&mut Batch"),), "()", b.block(
        b.write("state", "retries", b.op(b.f("state", "retries"), "+", b.i(1))), b.ret()))
    commit = Function("commit", (("state", "&mut Batch"), ("job", "i32")), "()", b.block(
        b.write("state", "completed", b.op(b.f("state", "completed"), "+", b.i(1))),
        b.write("state", "checksum", b.op(b.f("state", "checksum"), "+", b.op(b.v("job"), "*", b.i(10)))), b.ret()))
    dispatch = Function("dispatch", (("state", "&mut Batch"), ("job", "i32")), "()", b.block(
        b.discard(b.call("commit", b.borrow("state", True, True), b.v("job"))), b.ret()))
    done = Function("done", (("state", "&Batch"),), "bool", b.block(b.ret(b.op(b.f("state", "completed"), ">=", b.i(6)))), True)
    relay = Function("relay", (("state", "Batch"),), "Batch", b.block(b.ret(b.v("state"))), True)
    finish = Function("finish", (("state", "Batch"),), "i32", b.block(b.ret(
        b.op(b.op(b.f("state", "checksum"), "+", b.op(b.f("state", "completed"), "*", b.i(100))), "+", b.f("state", "retries")))))
    main = Function("main", (), "i32", b.block(
        b.let("state", b.lit("Batch", ("completed", b.i(0)), ("retries", b.i(0)), ("checksum", b.i(0)), ("active", b.b(True))), True),
        b.loop(b.f("state", "active"), b.block(
            b.let("attempt", b.i(0), True),
            b.loop(b.op(b.v("attempt"), "<", b.i(3)), b.block(
                b.store("attempt", b.op(b.v("attempt"), "+", b.i(1))),
                b.iff(b.op(b.v("attempt"), "<", b.i(2)), b.block(b.discard(b.call("retry", b.borrow("state", True))), b.cont())),
                b.let("job", b.op(b.f("state", "completed"), "+", b.i(1))),
                b.discard(b.call("dispatch", b.borrow("state", True), b.v("job"))), b.brk())),
            b.iff(b.call("done", b.borrow("state")), b.block(b.write("state", "active", b.b(False)), b.brk())), b.cont())),
        b.let("completed", b.call("relay", b.v("state"))), b.ret(b.call("finish", b.v("completed")))))
    return Program((Record("Batch", (("completed", "i32"), ("retries", "i32"), ("checksum", "i32"), ("active", "bool"))),), (retry, commit, dispatch, done, relay, finish, main))


def batch_ledger():
    """Arithmetic ledger derived from RFC §5, independent of any evaluator."""
    completed, retries, checksum = 0, 0, 0
    rows = []
    for job in range(1, 7):
        retries += 1
        rows.append({"job": job, "event": "retry", "completed": completed, "retries": retries, "checksum": checksum})
        completed += 1
        checksum += job * 10
        rows.append({"job": job, "event": "commit", "completed": completed, "retries": retries, "checksum": checksum})
    return {"retries": retries, "commits": completed, "checksum": checksum,
            "result": checksum + completed * 100 + retries, "state_transitions": rows,
            "source_fuel": None, "source_fuel_status": "pending independent source template schedule; raw 1086 is not source evidence"}


@dataclass(frozen=True)
class Diagnostic:
    stage: str
    code: str
    origin: str
    message: str
    related: tuple[str, ...] = ()

    def json(self, rendered):
        return {"stage": self.stage, "code": self.code, "span": rendered.span(self.origin),
                "origin": self.origin, "message": bounded_text(self.message, 1024),
                "related": [rendered.span(key) for key in self.related[:2]]}


class Rejected(Exception):
    def __init__(self, stage, code, origin, message, related=()):
        self.diagnostic = Diagnostic(stage, code, origin, message, tuple(related))
        super().__init__(message)


def bounded_text(value, limit=64):
    if limit < 3: raise ValueError("bound must leave room for suffix")
    raw = value.encode()
    if len(raw) <= limit: return value
    return raw[:limit - 3].decode("utf-8", errors="ignore") + "..."


def ref_type(ty):
    if ty.startswith("&mut "): return "exclusive", ty[5:]
    if ty.startswith("&"): return "shared", ty[1:]
    return None


@dataclass
class Binding:
    key: str
    name: str
    ty: str | None
    mutable: bool
    origin: str
    kind: str
    position: int | None = None


class NamesTypes:
    """Names are completely resolved before any body types or ownership checks."""
    def __init__(self, program):
        self.program = program
        self.records, self.functions, self.bindings = {}, {}, {}
        self.references, self.types, self.projections = {}, {}, {}
        self.function_bindings, self.function_index, self.flows = {}, {}, {}

    def reject(self, stage, code, origin, message, related=()):
        raise Rejected(stage, code, origin, message, related)

    def value_type(self, ty, origin, parameter=False, field_type=False):
        reference = ref_type(ty)
        if reference:
            if not parameter: self.reject("parse", "E0101", origin, "reference type is only allowed in a parameter")
            if reference[1] not in self.records:
                self.reject("resolve", "E0202", origin + ".referent", "unknown record type")
        elif ty not in SCALARS and ty not in self.records:
            self.reject("resolve", "E0202", origin, "unknown type")
        if field_type and ty not in SCALARS:
            self.reject("resolve", "E0202", origin, "record fields must be scalar")

    def check(self):
        validate_render_domain(self.program)
        self.check_expression_grammar()
        for index, record in enumerate(self.program.records):
            key = f"record.{index}"
            if record.name in self.records:
                self.reject("resolve", "E0201", key + ".name", "duplicate record")
            if record.name in ("i32", "bool"):
                self.reject("resolve", "E0202", key + ".name", "reserved scalar type")
            self.records[record.name] = (index, record)
            seen = set()
            for position, (name, _) in enumerate(record.fields):
                if name in seen: self.reject("resolve", "E0201", f"{key}.field.{position}.name", "duplicate field")
                seen.add(name)
        for index, fn in enumerate(self.program.functions):
            if fn.name in self.functions:
                self.reject("resolve", "E0201", f"function.{index}.name", "duplicate function")
            self.functions[fn.name], self.function_index[fn.name] = fn, index
        for index, record in enumerate(self.program.records):
            for position, (_, ty) in enumerate(record.fields):
                self.value_type(ty, f"record.{index}.field.{position}.type", field_type=True)
        # Annotation/type references in every function are resolved before bodies.
        for index, fn in enumerate(self.program.functions):
            self.value_type(fn.result, f"function.{index}.result")
            for position, (_, ty) in enumerate(fn.parameters):
                self.value_type(ty, f"function.{index}.parameter.{position}.type", parameter=True)
            self.resolve_annotations(fn.body)
        for index, fn in enumerate(self.program.functions):
            env = {}
            self.function_bindings[fn.name] = []
            for position, (name, ty) in enumerate(fn.parameters):
                key = f"function.{index}.parameter.{position}.name"
                self.declare(fn.name, env, Binding(key, name, ty, False, key, "parameter", position))
            self.resolve_block(fn.name, fn.body, env, 0)
        for fn in self.program.functions:
            exits = self.type_block(fn, fn.body)
            if "next" in exits:
                self.reject("type", "E0302", fn.body.key + ".close", "function must explicitly return on every path")
        return self

    def check_expression_grammar(self):
        # Parsing precedes every name/type failure. This is a deliberately small
        # tagged grammar check, not a parser or a producer acceptance oracle.
        def expr(n):
            d, tag = n.data, n.tag
            if tag == "unary":
                if d["op"] != "!":
                    if d["op"] == "-" and d["value"].tag == "int":
                        raise ValueError("signed literals must be represented by Int, not a general unary AST")
                    self.reject("parse", "E0101", n.key + ".operator", "general unary arithmetic is not supported")
                expr(d["value"])
            elif tag == "binary":
                expr(d["lhs"])
                if d["op"] not in ("+", "-", "*", "==", "!=", "<", "<=", ">", ">=", "&&", "||"):
                    self.reject("parse", "E0101", n.key + ".operator", "unsupported scalar operator")
                expr(d["rhs"])
            elif tag == "group": expr(d["value"])
            elif tag == "literal":
                for _, value in d["fields"]: expr(value)
            elif tag == "call":
                for argument in d["args"]: expr(argument)
        def block(n):
            for statement in n.data["statements"]:
                d, tag = statement.data, statement.tag
                if isinstance(d.get("value"), Node): expr(d["value"])
                if tag in ("if", "while"):
                    expr(d["condition"])
                    block(d["yes"] if tag == "if" else d["body"])
                    if tag == "if" and d["no"]: block(d["no"])
                if tag == "block": block(statement)
        for function in self.program.functions: block(function.body)

    def resolve_annotations(self, block):
        for n in block.data["statements"]:
            d = n.data
            if n.tag == "let" and d["annotation"]:
                self.value_type(d["annotation"], n.key + ".annotation")
            for key in ("yes", "no", "body"):
                if isinstance(d.get(key), Node): self.resolve_annotations(d[key])
            if n.tag == "block": self.resolve_annotations(n)

    def declare(self, fn, env, binding):
        if binding.name in env or binding.name in self.functions:
            self.reject("resolve", "E0201", binding.origin, "active value name already declared")
        env[binding.name] = binding.key
        self.bindings[binding.key] = binding
        self.function_bindings[fn].append(binding.key)

    def lookup(self, env, name, origin, node):
        if name not in env: self.reject("resolve", "E0200", origin, "unknown value")
        self.references[node] = env[name]

    def resolve_expr(self, n, env):
        d, tag = n.data, n.tag
        if tag == "int":
            if not MIN_I32 <= d["value"] <= MAX_I32:
                self.reject("resolve", "E0203", n.key, "i32 literal is out of range")
        elif tag == "name": self.lookup(env, d["name"], n.key + ".name", n.key)
        elif tag == "field": self.lookup(env, d["base"], n.key + ".base", n.key)
        elif tag == "borrow": self.lookup(env, d["name"], n.key + ".name", n.key)
        elif tag in ("group", "unary"): self.resolve_expr(d["value"], env)
        elif tag == "binary":
            self.resolve_expr(d["lhs"], env); self.resolve_expr(d["rhs"], env)
        elif tag == "call":
            if d["function"] not in self.functions:
                self.reject("resolve", "E0200", n.key + ".callee", "unknown function")
            for arg in d["args"]: self.resolve_expr(arg, env)
        elif tag == "literal":
            if d["record"] not in self.records:
                self.reject("resolve", "E0202", n.key + ".record", "unknown record")
            fields = dict(self.records[d["record"]][1].fields)
            seen = set()
            for index, (name, value) in enumerate(d["fields"]):
                if name in seen: self.reject("resolve", "E0201", f"{n.key}.field.{index}", "duplicate literal field")
                if name not in fields: self.reject("resolve", "E0200", f"{n.key}.field.{index}", "unknown literal field")
                seen.add(name); self.resolve_expr(value, env)

    def resolve_block(self, fn, block, parent, loop_depth):
        env = dict(parent)
        for n in block.data["statements"]:
            d, tag = n.data, n.tag
            if tag in ("store", "write"):
                self.lookup(env, d.get("base", d.get("name")), n.key + ".base", n.key)
            if isinstance(d.get("value"), Node): self.resolve_expr(d["value"], env)
            if tag == "let":
                self.declare(fn, env, Binding(n.key, d["name"], d["annotation"], d["mutable"], n.key + ".name", "local"))
            if tag in ("break", "continue") and not loop_depth:
                self.reject("resolve", "E0204", n.key, "loop transfer outside a loop")
            if tag in ("if", "while"):
                self.resolve_expr(d["condition"], env)
                self.resolve_block(fn, d["yes"] if tag == "if" else d["body"], env, loop_depth + (tag == "while"))
                if tag == "if" and d["no"] is not None:
                    self.resolve_block(fn, d["no"], env, loop_depth)
            if tag == "block": self.resolve_block(fn, n, env, loop_depth)

    def same(self, expected, actual, origin):
        if expected != actual: self.reject("type", "E0300", origin, f"expected {bounded_text(expected)}, found {bounded_text(actual)}")

    def projection(self, n):
        binding = self.bindings[self.references[n.key]]
        record_name = ref_type(binding.ty)[1] if ref_type(binding.ty) else binding.ty
        if record_name not in self.records:
            self.reject("type", "E0300", n.key + ".base", "field base must be a record or reference")
        index, record = self.records[record_name]
        for position, (name, ty) in enumerate(record.fields):
            if name == n.data["name"]:
                self.projections[n.key] = (index, position)
                return ty
        self.reject("type", "E0305", n.key + ".field", "unknown projected field")

    def expr(self, n):
        d, tag = n.data, n.tag
        if tag in ("int", "bool", "unit"):
            ty = {"int": "i32", "bool": "bool", "unit": "()"}[tag]
        elif tag == "name":
            ty = self.bindings[self.references[n.key]].ty
            if ref_type(ty): self.reject("type", "E0312", n.key, "reference parameter requires a field or explicit reborrow")
        elif tag == "field": ty = self.projection(n)
        elif tag == "borrow":
            binding = self.bindings[self.references[n.key]]
            ref = ref_type(binding.ty)
            if d["forwarded"] != bool(ref):
                self.reject("type", "E0312", n.key, "use explicit forwarded reference syntax for reference parameters only")
            record = ref[1] if ref else binding.ty
            if record in SCALARS: self.reject("type", "E0300", n.key, "scalar borrowing is not supported")
            if d["mutable"] and not ref and not binding.mutable:
                self.reject("type", "E0304", n.key, "exclusive borrowing requires a mutable owner", (binding.origin,))
            ty = ("&mut " if d["mutable"] else "&") + record
        elif tag == "group": ty = self.expr(d["value"])
        elif tag == "unary":
            if d["op"] != "!": raise AssertionError("unsupported unary operator survived source grammar")
            ty = "bool"
            self.same(ty, self.expr(d["value"]), d["value"].key)
        elif tag == "binary":
            lhs, rhs, op = self.expr(d["lhs"]), self.expr(d["rhs"]), d["op"]
            if op in ("&&", "||"): self.same("bool", lhs, d["lhs"].key); self.same("bool", rhs, d["rhs"].key); ty = "bool"
            elif op in ("==", "!="):
                if lhs not in ("bool", "i32"): self.reject("type", "E0300", d["lhs"].key, "aggregate/unit equality is not supported")
                self.same(lhs, rhs, d["rhs"].key); ty = "bool"
            else:
                self.same("i32", lhs, d["lhs"].key); self.same("i32", rhs, d["rhs"].key)
                ty = "bool" if op in ("<", "<=", ">", ">=") else "i32"
        elif tag == "literal":
            ty = d["record"]
            fields = dict(self.records[ty][1].fields)
            for name, value in d["fields"]: self.same(fields[name], self.expr(value), value.key)
            missing = [name for name in fields if name not in dict(d["fields"])]
            if missing:
                names = ", ".join(bounded_text(name) for name in missing[:8])
                tail = f"; {len(missing) - 8} more omitted" if len(missing) > 8 else ""
                self.reject("type", "E0300", n.key, "missing fields: " + names + tail)
        elif tag == "call":
            fn = self.functions[d["function"]]
            if len(d["args"]) != len(fn.parameters): self.reject("type", "E0300", n.key, "wrong argument count")
            for arg, (_, expected) in zip(d["args"], fn.parameters): self.same(expected, self.expr(arg), arg.key)
            ty = fn.result
        else: raise ValueError(tag)
        self.types[n.key] = ty
        return ty

    def type_block(self, fn, block):
        exits = {"next"}
        for n in block.data["statements"]:
            if "next" not in exits: self.reject("type", "E0303", n.key, "unreachable statement")
            d, tag = n.data, n.tag
            out = {"next"}
            if tag == "let":
                actual = self.expr(d["value"])
                if ref_type(actual): self.reject("parse", "E0101", d["value"].key, "borrow is only a call argument")
                binding = self.bindings[n.key]
                if binding.ty: self.same(binding.ty, actual, d["value"].key)
                binding.ty = actual
            elif tag in ("store", "write"):
                actual = self.expr(d["value"])
                binding = self.bindings[self.references[n.key]]
                if tag == "store" or not ref_type(binding.ty):
                    if not binding.mutable: self.reject("type", "E0304", n.key + ".target", "immutable binding", (binding.origin,))
                expected = self.projection(n) if tag == "write" else binding.ty
                self.same(expected, actual, d["value"].key)
            elif tag == "return":
                self.same(fn.result, self.expr(d["value"]) if d["value"] else "()", d["value"].key if d["value"] else n.key)
                out = {"return"}
            elif tag == "discard": self.expr(d["value"])
            elif tag in ("break", "continue"): out = {tag}
            elif tag in ("if", "while"):
                self.same("bool", self.expr(d["condition"]), d["condition"].key)
                if tag == "if":
                    out = self.type_block(fn, d["yes"]) | (self.type_block(fn, d["no"]) if d["no"] else {"next"})
                else: out = {"next"} | (self.type_block(fn, d["body"]) & {"return"})
            elif tag == "block": out = self.type_block(fn, n)
            exits = (exits - {"next"}) | out
            self.flows[n.key] = frozenset(out)
        self.flows[block.key] = frozenset(exits)
        return exits


def alias_partitions(count):
    """Restricted-growth strings enumerate each set partition exactly once."""
    if count == 0:
        yield ()
        return
    def extend(prefix):
        if len(prefix) == count:
            yield prefix
        else:
            for value in range(max(prefix) + 2): yield from extend(prefix + (value,))
    yield from extend((0,))


def alias_contract(partition, modes):
    return all(partition[i] != partition[j] or (modes[i] == modes[j] == "shared")
               for i in range(len(modes)) for j in range(i))


@dataclass(frozen=True)
class Capability:
    key: str
    root: str
    mode: str
    parent: str | None
    call: str | None
    origin: str


@dataclass(frozen=True)
class Availability:
    owners: tuple[tuple[str, str, str | None], ...] = ()
    capabilities: tuple[Capability, ...] = ()
    calls: tuple[str, ...] = ()


@dataclass
class Point:
    action: str
    origin: str
    data: tuple = ()
    edges: list[int] = field(default_factory=list)


class SourceGraph:
    """Independent source evaluation points, including each lazy/argument edge.

    This is not raw OIR: it has no producer IDs, layout or SSA, and cannot enter
    any compiler consumer. States enumerate availability and source permissions.
    """
    def __init__(self, checked, function):
        self.checked, self.function, self.points = checked, function, []
        self.start = self.add([], "entry", function.body.key)
        parameters = [key for key in checked.function_bindings[function.name]
                      if checked.bindings[key].kind == "parameter" and checked.bindings[key].ty not in SCALARS
                      and not ref_type(checked.bindings[key].ty)]
        self.block(function.body, [self.start], parameters, None)

    def add(self, incoming, action, origin, *data):
        index = len(self.points)
        self.points.append(Point(action, origin, tuple(data)))
        for source in incoming: self.points[source].edges.append(index)
        return index

    def emit(self, incoming, action, origin, *data): return [self.add(incoming, action, origin, *data)]

    def expr(self, n, incoming, cause=None):
        d, tag, ty = n.data, n.tag, self.checked.types[n.key]
        owner = "temporary:" + n.key
        if tag == "name" and ty not in SCALARS:
            return self.emit(incoming, "move", n.key, self.checked.references[n.key], owner, cause or n.key), owner
        if tag == "field":
            return self.emit(incoming, "read", n.key, self.checked.references[n.key]), None
        if tag in ("int", "bool", "unit", "name"): return incoming, None
        if tag in ("group", "unary"):
            return self.expr(d["value"], incoming, cause)
        if tag == "binary":
            incoming, _ = self.expr(d["lhs"], incoming, cause)
            if d["op"] in ("&&", "||"):
                branch = self.add(incoming, "choice", n.key)
                rhs, _ = self.expr(d["rhs"], [branch], cause)
                return self.emit([branch] + rhs, "merge", n.key), None
            return self.expr(d["rhs"], incoming, cause)
        if tag == "literal":
            for _, value in d["fields"]: incoming, _ = self.expr(value, incoming, cause)
            return self.emit(incoming, "initialize", n.key, owner), owner
        if tag == "call":
            incoming = self.emit(incoming, "open", n.key, n.key)
            for index, arg in enumerate(d["args"]):
                if arg.tag == "borrow":
                    incoming = self.emit(incoming, "borrow", arg.key, self.checked.references[arg.key],
                                         "exclusive" if arg.data["mutable"] else "shared", n.key, f"{n.key}.argument.{index}")
                else:
                    incoming, result = self.expr(arg, incoming, cause)
                    if result:
                        incoming = self.emit(incoming, "consume", arg.key, result, cause or arg.key)
                        incoming = self.emit(incoming, "end", arg.key, result)
            result = owner if ty not in SCALARS else None
            return self.emit(incoming, "invoke", n.key, n.key, result), result
        raise ValueError(tag)

    def cleanup(self, incoming, owners, origin):
        for owner in reversed(owners): incoming = self.emit(incoming, "end", origin, owner)
        return incoming

    def block(self, block, incoming, prefix, loop):
        active = list(prefix)
        for n in block.data["statements"]:
            d, tag = n.data, n.tag
            if tag == "let":
                incoming, result = self.expr(d["value"], incoming, n.key)
                if result:
                    incoming = self.emit(incoming, "move", n.key, result, n.key, n.key)
                    incoming = self.emit(incoming, "end", n.key, result)
                    active.append(n.key)
            elif tag == "store":
                incoming, result = self.expr(d["value"], incoming, n.key)
                if result:
                    incoming = self.emit(incoming, "replace", n.key + ".target", self.checked.references[n.key], result, n.key)
                    incoming = self.emit(incoming, "end", n.key, result)
            elif tag == "write":
                incoming, _ = self.expr(d["value"], incoming, n.key)
                incoming = self.emit(incoming, "write", n.key + ".target", self.checked.references[n.key])
            elif tag == "discard":
                incoming, result = self.expr(d["value"], incoming, n.key)
                if result:
                    incoming = self.emit(incoming, "consume", n.key, result, n.key)
                    incoming = self.emit(incoming, "end", n.key, result)
            elif tag == "return":
                if d["value"] is not None: incoming, result = self.expr(d["value"], incoming, n.key)
                else: result = None
                incoming = self.cleanup(incoming, active, n.key)
                if result: incoming = self.emit(incoming, "consume", n.key, result, n.key)
                self.emit(incoming, "return", n.key)
                incoming = []
            elif tag in ("break", "continue"):
                header, exit_, depth = loop
                incoming = self.cleanup(incoming, active[depth:], n.key)
                transfer = self.add(incoming, tag, n.key)
                self.points[transfer].edges.append(header if tag == "continue" else exit_)
                incoming = []
            elif tag == "block": incoming = self.block(n, incoming, active, loop)
            elif tag == "if":
                incoming, _ = self.expr(d["condition"], incoming, n.key)
                branch = self.add(incoming, "choice", n.key)
                yes = self.block(d["yes"], [branch], active, loop)
                no = self.block(d["no"], [branch], active, loop) if d["no"] else [branch]
                incoming = self.emit(yes + no, "merge", n.key) if yes or no else []
            elif tag == "while":
                header = self.add(incoming, "loop", n.key)
                incoming, _ = self.expr(d["condition"], [header], n.key)
                branch = self.add(incoming, "choice", n.key)
                exit_ = self.add([branch], "merge", n.key)
                back = self.block(d["body"], [branch], active, (header, exit_, len(active)))
                for point in back: self.points[point].edges.append(header)
                incoming = [exit_]
            else: raise ValueError(tag)
        return self.cleanup(incoming, active[len(prefix):], block.key + ".close") if incoming else []

    def initial_states(self):
        bindings = [self.checked.bindings[key] for key in self.checked.function_bindings[self.function.name]
                    if self.checked.bindings[key].kind == "parameter"]
        refs = [binding for binding in bindings if ref_type(binding.ty)]
        owners = tuple(sorted((binding.key, "available", None) for binding in bindings
                              if binding.ty not in SCALARS and not ref_type(binding.ty)))
        # Shared entry aliases are all permitted; exclusive parameters are disjoint.
        # Enumerate at most four reference positions, the declared corpus bound.
        if len(refs) > 4: raise ValueError("model corpus supports at most four reference parameters")
        for partition in alias_partitions(len(refs)):
            modes = tuple(ref_type(binding.ty)[0] for binding in refs)
            if not alias_contract(partition, modes): continue
            if any(partition[i] == partition[j] and ref_type(refs[i].ty)[1] != ref_type(refs[j].ty)[1]
                   for i in range(len(refs)) for j in range(i)): continue
            caps = tuple(Capability(binding.key, f"entry-root:{partition[index]}", modes[index], None, None, binding.origin)
                         for index, binding in enumerate(refs))
            yield Availability(owners, caps)

    def step(self, point, state):
        owners = {key: (status, cause) for key, status, cause in state.owners}
        caps, calls = list(state.capabilities), list(state.calls)
        action, data = point.action, point.data
        by_key = {cap.key: cap for cap in caps}

        def available(key):
            status, cause = owners.get(key, ("dead", None))
            if status != "available":
                if key.startswith("temporary:"): raise AssertionError("invalid independent temporary lifecycle")
                declaration = self.checked.bindings[key].origin
                raise Rejected("ownership", "E0310", point.origin, "owner is not available on every path", tuple(x for x in (cause, declaration) if x))

        def access(key, mode):
            if key in by_key:
                parent = by_key[key]
                if mode != "shared" and parent.mode == "shared":
                    raise Rejected("ownership", "E0313", point.origin, "shared permission cannot write or reborrow exclusively", (parent.origin,))
                root, authority = parent.root, key
                ancestors = {key}
                cursor = parent.parent
                while cursor:
                    ancestors.add(cursor); cursor = by_key[cursor].parent
            else:
                available(key)
                root, authority, ancestors = key, None, set()
            for other in caps:
                if other.root == root and other.key not in ancestors and (mode != "shared" or other.mode == "exclusive"):
                    raise Rejected("ownership", "E0311", point.origin, "conflicting live loan", (other.origin,))
            return root, authority

        if action == "initialize": owners[data[0]] = ("available", None)
        elif action == "move":
            source, destination, cause = data
            access(source, "exclusive")
            owners[source] = ("moved", cause)
            owners[destination] = ("available", None)
        elif action == "consume":
            source, cause = data
            access(source, "exclusive"); owners[source] = ("moved", cause)
        elif action == "replace":
            target, source, cause = data
            access(source, "exclusive")
            # Replacement restores a moved live slot; it must still be unloaned.
            if owners.get(target, ("dead",))[0] == "dead": raise AssertionError("replacement outside source lifetime")
            if any(cap.root == target for cap in caps):
                conflict = next(cap for cap in caps if cap.root == target)
                raise Rejected("ownership", "E0311", point.origin, "replacement conflicts with loan", (conflict.origin,))
            owners[source], owners[target] = ("moved", cause), ("available", None)
        elif action == "end":
            if any(cap.root == data[0] for cap in caps): raise AssertionError("source cleanup crossed live loan")
            owners.pop(data[0], None)
        elif action in ("read", "write"): access(data[0], "shared" if action == "read" else "exclusive")
        elif action == "open": calls.append(data[0])
        elif action == "borrow":
            key, mode, call, handle = data
            if not calls or calls[-1] != call: raise AssertionError("source call region mismatch")
            root, parent = access(key, mode)
            caps.append(Capability(handle, root, mode, parent, call, point.origin))
        elif action == "invoke":
            call, result = data
            if not calls or calls.pop() != call: raise AssertionError("source invocation region mismatch")
            removed = {cap.key for cap in caps if cap.call == call}
            if any(cap.parent in removed and cap.key not in removed for cap in caps): raise AssertionError("child capability escaped call")
            caps = [cap for cap in caps if cap.call != call]
            if result: owners[result] = ("available", None)
        elif action == "return":
            if calls or any(cap.call for cap in caps): raise AssertionError("call capability survives source return")
        return Availability(tuple(sorted((key, status, cause) for key, (status, cause) in owners.items())), tuple(caps), tuple(calls))

    def saturate(self, state_ceiling=100000):
        queue = deque((self.start, state) for state in self.initial_states())
        seen, failures = set(), []
        while queue:
            point_id, state = queue.popleft()
            if (point_id, state) in seen: continue
            if len(seen) >= state_ceiling:
                raise ValueError("MODEL_INCOMPLETE: finite-state oracle state ceiling exceeded")
            seen.add((point_id, state))
            point = self.points[point_id]
            try: after = self.step(point, state)
            except Rejected as error:
                failures.append(error.diagnostic)
                continue
            for edge in point.edges: queue.append((edge, after))
        # Finite product fixed point; no path/execution limit and no constant folding.
        return {"points": len(self.points), "states": len(seen), "failures": failures,
                "entry_alias_partitions": sum(1 for _ in self.initial_states()), "algorithm": "finite-state-saturation"}


def static_check(checked, rendered=None):
    functions, errors = {}, []
    for fn in checked.program.functions:
        result = SourceGraph(checked, fn).saturate()
        functions[fn.name] = {key: value for key, value in result.items() if key != "failures"}
        errors.extend(result["failures"])
    if rendered:
        errors.sort(key=lambda error: (*rendered.origins[error.origin], error.code))
    return {"accepted": not errors, "functions": functions, "diagnostics": errors}


@dataclass
class OwnedValue:
    identity: tuple[int, str, int]
    record: str
    fields: dict[str, Any]
    available: bool = True


@dataclass(frozen=True)
class ReferenceValue:
    handle: int


@dataclass
class RuntimeCapability:
    handle: int
    owner: tuple[int, str, int]
    mode: str
    parent: int | None
    call: tuple[int, str, int]
    origin: str


@dataclass
class Activation:
    number: int
    function: Function
    environment: dict[str, Any]
    scopes: list[list[str]] = field(default_factory=list)


class Transfer(Exception):
    def __init__(self, kind, value=None): self.kind, self.value = kind, value


class DynamicFailure(Exception):
    def __init__(self, code, origin, message):
        self.code, self.origin, self.message = code, origin, message


class Machine:
    """Concrete source execution; a step bound terminates models, never proves safety.

    Owner instances include activation and generation. Numeric state does not
    depend on the separate cost planner or compiler output. Events retain scalar
    argument snapshots and every committed write, even after a later failure.
    """
    def __init__(self, checked, step_bound=100000):
        self.checked, self.step_bound = checked, step_bound
        self.frames, self.events, self.owners, self.capabilities = [], [], {}, {}
        self.next_frame, self.next_handle, self.next_call = 0, 0, 0
        self.generations, self.steps = Counter(), 0

    @property
    def frame(self): return self.frames[-1]

    def event(self, kind, origin, **data):
        self.steps += 1
        if self.steps > self.step_bound:
            raise DynamicFailure("MODEL_BOUND", origin, "model execution bound reached; compiler admission is unverified")
        event = {"index": len(self.events), "kind": kind, "origin": origin,
                 "function": self.frame.function.name if self.frames else None,
                 "activation": self.frame.number if self.frames else None, **data}
        self.events.append(event)

    def owner(self, site, record, fields):
        generation_key = self.frame.number, site
        self.generations[generation_key] += 1
        identity = (*generation_key, self.generations[generation_key])
        value = OwnedValue(identity, record, dict(fields))
        self.owners[identity] = value
        return value

    def value(self, n): return self.frame.environment[self.checked.references[n.key]]

    def authority(self, value, mode, origin):
        if isinstance(value, ReferenceValue):
            capability = self.capabilities[value.handle]
            if mode == "exclusive" and capability.mode == "shared": raise AssertionError("dynamic illegal permission")
            ancestor, excluded = capability, set()
            while ancestor:
                excluded.add(ancestor.handle)
                ancestor = self.capabilities.get(ancestor.parent)
            identity, parent = capability.owner, capability.handle
        else:
            if not value.available: raise AssertionError("dynamic unavailable owner")
            identity, parent, excluded = value.identity, None, set()
        for cap in self.capabilities.values():
            if cap.owner == identity and cap.handle not in excluded and (cap.mode == "exclusive" or mode == "exclusive"):
                raise AssertionError("dynamic conflicting permission")
        return self.owners[identity], parent

    def moved(self, value, site):
        self.authority(value, "exclusive", site)
        value.available = False
        return self.owner(site, value.record, value.fields)

    def expr(self, n):
        d, tag, ty = n.data, n.tag, self.checked.types[n.key]
        if tag in ("int", "bool", "unit"):
            value = d.get("value")
            self.event("scalar-expression", n.key, expression=tag, value=value)
            return value
        if tag == "name":
            value = self.value(n)
            if isinstance(value, OwnedValue):
                moved = self.moved(value, "temporary:" + n.key)
                self.event("owned-name", n.key, source=list(value.identity), destination=list(moved.identity), record=ty)
                return moved
            self.event("scalar-expression", n.key, expression=tag, binding=self.checked.references[n.key], value=value)
            return value
        if tag == "field":
            owner, _ = self.authority(self.value(n), "shared", n.key)
            value = owner.fields[d["name"]]
            self.event("field-read", n.key, owner=list(owner.identity), field=d["name"], value=value)
            return value
        if tag == "group":
            value = self.expr(d["value"])
            if ty in SCALARS: self.event("scalar-expression", n.key, expression=tag, value=value)
            return value
        if tag == "unary":
            value = self.expr(d["value"])
            self.event("scalar-expression", n.key, expression=tag)
            if d["op"] != "!": raise AssertionError("unsupported unary operator survived source grammar")
            return not value
        if tag == "binary":
            lhs = self.expr(d["lhs"])
            op = d["op"]
            if op in ("&&", "||"):
                evaluate = lhs if op == "&&" else not lhs
                self.event("lazy-branch", n.key, rhs_taken=evaluate)
                rhs = self.expr(d["rhs"]) if evaluate else lhs
                if evaluate: self.event("lazy-goto", n.key)
                self.event("lazy-merge", n.key, value=rhs)
                return rhs
            rhs = self.expr(d["rhs"])
            self.event("scalar-expression", n.key, expression=tag, lhs=lhs, rhs=rhs, operator=op)
            if op == "+": return self.checked_i32(lhs + rhs, n.key + ".operator")
            if op == "-": return self.checked_i32(lhs - rhs, n.key + ".operator")
            if op == "*": return self.checked_i32(lhs * rhs, n.key + ".operator")
            return {"<": lambda: lhs < rhs, "<=": lambda: lhs <= rhs, ">": lambda: lhs > rhs,
                    ">=": lambda: lhs >= rhs, "==": lambda: lhs == rhs, "!=": lambda: lhs != rhs}[op]()
        if tag == "literal":
            fields = {}
            for name, value in d["fields"]: fields[name] = self.expr(value)
            result = self.owner("temporary:" + n.key, d["record"], fields)
            self.event("construct", n.key, record=ty, owner=list(result.identity), fields=list(fields.items()))
            return result
        if tag == "call":
            call = self.frame.number, n.key, self.next_call
            self.next_call += 1
            fn = self.checked.functions[d["function"]]
            self.event("call-open", n.key, call=list(call), callee=fn.name)
            values, loans = [], []
            for position, arg in enumerate(d["args"]):
                if arg.tag == "borrow":
                    value = self.value(arg)
                    mode = "exclusive" if arg.data["mutable"] else "shared"
                    owner, parent = self.authority(value, mode, arg.key)
                    handle = self.next_handle
                    self.next_handle += 1
                    self.capabilities[handle] = RuntimeCapability(handle, owner.identity, mode, parent, call, arg.key)
                    loans.append(handle)
                    values.append(ReferenceValue(handle))
                    self.event("borrow-acquire", arg.key, call=list(call), position=position, handle=handle,
                               parent=parent, owner=list(owner.identity), mode=mode)
                else:
                    value = self.expr(arg)
                    if isinstance(value, OwnedValue):
                        staged = self.moved(value, f"staged:{n.key}:{position}")
                        self.event("prepare-owned", arg.key, call=list(call), position=position, record=value.record,
                                   source=list(value.identity), staged=list(staged.identity))
                        self.event("temporary-end", arg.key, owner=list(value.identity), record=value.record)
                        values.append(staged)
                    else:
                        self.event("prepare-scalar", arg.key, call=list(call), position=position, value=value)
                        values.append(value)
            self.event("call-invoke", n.key, call=list(call), callee=fn.name)
            result = self.invoke(fn, values)
            for handle in reversed(loans):
                if any(cap.parent == handle for cap in self.capabilities.values()): raise AssertionError("child loan survives return")
                cap = self.capabilities.pop(handle)
                self.event("borrow-release", cap.origin, call=list(call), handle=handle, owner=list(cap.owner))
            if isinstance(result, OwnedValue):
                returned = self.moved(result, "result:" + n.key)
                self.event("call-result", n.key, source=list(result.identity), destination=list(returned.identity), record=result.record)
                return returned
            return result
        raise ValueError(tag)

    @staticmethod
    def checked_i32(value, origin):
        if not MIN_I32 <= value <= MAX_I32: raise DynamicFailure("E0604", origin, "integer overflow")
        return value

    def block(self, block):
        self.frame.scopes.append([])
        try:
            for n in block.data["statements"]: self.stmt(n)
        except Transfer as transfer:
            self.end_scope(transfer.origin)
            raise
        else:
            self.end_scope(block.key + ".close")

    def end_scope(self, origin):
        for key in reversed(self.frame.scopes.pop()):
            value = self.frame.environment.pop(key)
            if isinstance(value, OwnedValue):
                if any(cap.owner == value.identity for cap in self.capabilities.values()): raise AssertionError("cleanup with live loan")
                self.event("lexical-end", origin, owner=list(value.identity), record=value.record, available=value.available)

    def stmt(self, n):
        d, tag = n.data, n.tag
        if tag == "let":
            value = self.expr(d["value"])
            if isinstance(value, OwnedValue):
                installed = self.moved(value, n.key)
                self.event("let-owned", n.key, source=list(value.identity), owner=list(installed.identity), record=value.record)
                value = installed
            else: self.event("let-scalar", n.key, binding=n.key, value=value, mutable=d["mutable"])
            self.frame.environment[n.key] = value
            self.frame.scopes[-1].append(n.key)
        elif tag == "store":
            value, key = self.expr(d["value"]), self.checked.references[n.key]
            old = self.frame.environment[key]
            if isinstance(value, OwnedValue):
                if any(cap.owner == old.identity for cap in self.capabilities.values()): raise AssertionError("replacement with live loan")
                self.authority(value, "exclusive", n.key)
                value.available = False
                old.fields, old.available = dict(value.fields), True
                self.event("replace-owned", n.key, source=list(value.identity), owner=list(old.identity), record=value.record,
                           fields=list(old.fields.items()))
            else:
                self.event("store-scalar", n.key, binding=key, before=old, value=value)
                self.frame.environment[key] = value
        elif tag == "write":
            value = self.expr(d["value"])
            owner, _ = self.authority(self.value(n), "exclusive", n.key)
            self.event("field-write", n.key, target=n.key + ".target", owner=list(owner.identity), field=d["name"], before=owner.fields[d["name"]], value=value)
            owner.fields[d["name"]] = value
        elif tag == "discard":
            value = self.expr(d["value"])
            if isinstance(value, OwnedValue):
                self.authority(value, "exclusive", n.key); value.available = False
                self.event("discard-owned", n.key, owner=list(value.identity), record=value.record)
        elif tag == "return":
            value = self.expr(d["value"]) if d["value"] is not None else None
            if d["value"] is None: self.event("return-unit", n.key)
            transfer = Transfer("return", value)
            transfer.origin = n.key
            raise transfer
        elif tag in ("break", "continue"):
            transfer = Transfer(tag)
            transfer.origin = n.key
            raise transfer
        elif tag == "block": self.block(n)
        elif tag == "if":
            condition = self.expr(d["condition"])
            self.event("branch", n.key, taken=condition)
            chosen = d["yes"] if condition else d["no"]
            if chosen is not None:
                self.block(chosen)
                self.event("goto", chosen.key + ".close")
        elif tag == "while":
            self.event("goto", n.key)
            while True:
                condition = self.expr(d["condition"])
                self.event("branch", n.key, taken=condition)
                if not condition: break
                try: self.block(d["body"])
                except Transfer as transfer:
                    if transfer.kind == "return": raise
                    self.event("goto", transfer.origin)
                    if transfer.kind == "break": break
                    continue
                self.event("goto", d["body"].key + ".close")
        else: raise ValueError(tag)

    def invoke(self, fn, arguments):
        number = self.next_frame
        self.next_frame += 1
        frame = Activation(number, fn, {})
        self.frames.append(frame)
        parameter_owners = []
        for position, value in enumerate(arguments):
            key = f"function.{self.checked.function_index[fn.name]}.parameter.{position}.name"
            if isinstance(value, OwnedValue):
                value.available = False
                value = self.owner(key, value.record, value.fields)
                parameter_owners.append(key)
                self.event("owned-parameter", key, owner=list(value.identity), position=position, record=value.record)
            frame.environment[key] = value
        try:
            self.block(fn.body)
        except Transfer as transfer:
            if transfer.kind != "return": raise AssertionError("transfer escaped function")
            for key in reversed(parameter_owners):
                value = frame.environment[key]
                self.event("lexical-end", transfer.origin, owner=list(value.identity), record=value.record, available=value.available)
            self.event("return-owned" if isinstance(transfer.value, OwnedValue) else "return-scalar", transfer.origin,
                       record=transfer.value.record if isinstance(transfer.value, OwnedValue) else None,
                       value=None if isinstance(transfer.value, OwnedValue) else transfer.value)
            self.frames.pop()
            return transfer.value
        raise AssertionError("typed function reached implicit return")

    def run(self, function="main"):
        if function not in self.checked.functions:
            return {"result": None, "result_type": None, "failure": {"code": "E0600", "origin": None}, "events": []}
        fn = self.checked.functions[function]
        if fn.parameters or fn.result not in SCALARS:
            return {"result": None, "result_type": fn.result, "failure": {"code": "E0600", "origin": f"function.{self.checked.function_index[function]}.name"}, "events": []}
        try:
            result = self.invoke(fn, [])
            failure = None
        except DynamicFailure as error:
            result, failure = None, {"code": error.code, "origin": error.origin, "message": error.message}
        except RecursionError as error:
            raise ValueError("MODEL_INCOMPLETE: Python interpreter depth exhausted before source execution completed") from error
        return {"result": result, "result_type": fn.result, "failure": failure, "events": self.events,
                "activations": self.next_frame, "generations": sum(self.generations.values()),
                "writes": [event for event in self.events if event["kind"] == "field-write"]}


class TemplateScheduler:
    """Published template accounting, separate from scalar execution/acceptance.

    Descriptor counts come from the tagged source and frozen scalar/ownership
    templates. Concrete semantic events select executed templates; no raw trace
    or compiler accounting is read. A schedule item is charged before its effect.
    """
    def __init__(self, checked):
        self.checked = checked
        self.frames = {fn.name: self.frame_counts(fn) for fn in checked.program.functions}

    def width(self, record): return max(1, len(self.checked.records[record][1].fields))

    def frame_counts(self, fn):
        counts = Counter(S=0, A=0, P=0, O=0, R=0, L=0, C=0)
        owners, expressions, calls = [], [], []
        def owner(key, record, kind):
            counts["O"] += 1; counts["P"] += self.width(record)
            owners.append({"site": key, "record": record, "kind": kind})
        for position, (_, ty) in enumerate(fn.parameters):
            key = f"function.{self.checked.function_index[fn.name]}.parameter.{position}.name"
            if ty in SCALARS: counts["S"] += 1
            elif ref_type(ty): counts["R"] += 1
            else: owner(key, ty, "parameter")
        def expr(n):
            d, tag, ty = n.data, n.tag, self.checked.types[n.key]
            if tag == "borrow": return
            if ty in SCALARS:
                counts["S"] += 1
                expressions.append(n.key)
            if tag == "name" and ty not in SCALARS: owner("temporary:" + n.key, ty, "temporary")
            if tag == "literal":
                for _, value in d["fields"]: expr(value)
                owner("temporary:" + n.key, ty, "temporary")
            if tag in ("group", "unary"): expr(d["value"])
            if tag == "binary": expr(d["lhs"]); expr(d["rhs"])
            if tag == "call":
                counts["C"] += 1; counts["A"] += len(d["args"])
                calls.append(n.key)
                callee = self.checked.functions[d["function"]]
                for position, (arg, (_, parameter_ty)) in enumerate(zip(d["args"], callee.parameters)):
                    if arg.tag == "borrow": counts["L"] += 1
                    else:
                        expr(arg)
                        if parameter_ty not in SCALARS: owner(f"staged:{n.key}:{position}", parameter_ty, "staged-argument")
                if ty not in SCALARS: owner("result:" + n.key, ty, "call-result")
        def block(n):
            for statement in n.data["statements"]:
                d, tag = statement.data, statement.tag
                if isinstance(d.get("value"), Node): expr(d["value"])
                if tag == "return" and d["value"] is None: counts["S"] += 1
                if tag == "let":
                    ty = self.checked.bindings[statement.key].ty
                    if ty in SCALARS: counts["S"] += 1
                    else: owner(statement.key, ty, "local")
                if tag in ("if", "while"):
                    expr(d["condition"])
                    block(d["yes"] if tag == "if" else d["body"])
                    if tag == "if" and d["no"]: block(d["no"])
                if tag == "block": block(statement)
        block(fn.body)
        counts["X"] = counts["S"] + counts["A"] + counts["P"] + 4 * counts["O"] + 8 * counts["R"] + 12 * counts["L"] + 2 * counts["C"]
        return {**dict(counts), "owners": owners, "scalar_expressions": expressions, "calls": calls}

    def schedule(self, dynamic, function="main"):
        items, spent = [], 0
        def add(operation, amount, origin, event_index):
            nonlocal spent
            items.append({"index": len(items), "operation": operation, "cost": amount, "start_fuel": spent,
                          "end_fuel": spent + amount, "origin": origin, "semantic_event": event_index})
            spent += amount
        if not dynamic["events"]: return {"total_fuel": None, "items": [], "frames": self.frames}
        add("Root", 1 + self.frames[function]["X"], f"function.{self.checked.function_index[function]}.name", -1)
        for event in dynamic["events"]:
            kind, origin, index = event["kind"], event["origin"], event["index"]
            record = event.get("record")
            width = self.width(record) if record else None
            if kind in ("scalar-expression", "let-scalar", "store-scalar", "return-unit"):
                add("Scalar", 1, origin, index)
            elif kind in ("field-read", "field-write", "branch", "goto", "lazy-branch", "lazy-goto", "lazy-merge", "prepare-scalar", "borrow-acquire"):
                operation = {"field-read": "ReadField", "field-write": "WriteField", "branch": "Branch", "goto": "Goto",
                             "lazy-branch": "Branch", "lazy-goto": "Goto", "lazy-merge": "BoolMerge",
                             "prepare-scalar": "PrepareScalar", "borrow-acquire": "PrepareBorrow"}[kind]
                add(operation, 1, origin, index)
            elif kind in ("owned-name", "construct"):
                add("StorageLive", 1, origin, index)
                add("MoveInitialize" if kind == "owned-name" else "Construct", 1 + width, origin, index)
            elif kind == "let-owned":
                add("StorageLive", 1, origin, index); add("MoveInitialize", 1 + width, origin, index); add("StorageEnd", 1 + width, origin, index)
            elif kind == "replace-owned":
                add("Replace", 1 + 2 * width, origin, index); add("StorageEnd", 1 + width, origin, index)
            elif kind == "discard-owned":
                add("Discard", 1 + width, origin, index); add("StorageEnd", 1 + width, origin, index)
            elif kind in ("lexical-end", "temporary-end", "prepare-owned"):
                add("PrepareOwned" if kind == "prepare-owned" else "StorageEnd", 1 + width, origin, index)
            elif kind in ("call-open", "call-invoke"):
                callee = self.checked.functions[event["callee"]]
                owned = [ty for _, ty in callee.parameters if ty not in SCALARS and not ref_type(ty)]
                borrowed = sum(bool(ref_type(ty)) for _, ty in callee.parameters)
                if kind == "call-open": add("OpenCall", 1 + len(owned), origin, index)
                else:
                    charge = 1 + len(callee.parameters) + self.frames[callee.name]["X"] + sum(self.width(ty) for ty in owned) + borrowed * (borrowed - 1) // 2
                    add("Invoke", charge, origin, index)
            elif kind in ("return-owned", "return-scalar"):
                frame = self.frames[event["function"]]
                charge = 1 + frame["P"] + frame["L"] + frame["C"] + frame["R"] + (width or 0)
                add("ReturnOwned" if kind == "return-owned" else "ReturnScalar", charge, origin, index)
            elif kind not in ("borrow-release", "call-result", "owned-parameter"):
                raise ValueError(f"unknown semantic event: {kind}")
        return {"total_fuel": spent, "items": items, "frames": self.frames,
                "complete_execution": dynamic["failure"] is None}

    @staticmethod
    def at_budget(schedule, dynamic, budget):
        if budget < 0: raise ValueError("negative fuel")
        for item in schedule["items"]:
            if item["end_fuel"] > budget:
                writes = [event for event in dynamic["writes"] if event["index"] < item["semantic_event"]]
                paid_items = schedule["items"][:item["index"]]
                committed = TemplateScheduler.committed_writes(paid_items, dynamic)
                return {"result": None, "failure": {"code": "E0601", "origin": item["origin"], "operation": item["operation"]},
                        "paid": item["start_fuel"], "remaining": budget - item["start_fuel"], "writes": writes,
                        "committed_writes": committed}
        return {"result": dynamic["result"], "failure": dynamic["failure"], "paid": schedule["total_fuel"],
                "remaining": budget - schedule["total_fuel"], "writes": dynamic["writes"],
                "committed_writes": TemplateScheduler.committed_writes(schedule["items"], dynamic)}

    @staticmethod
    def committed_writes(paid_items, dynamic):
        operations = {"field-write": "WriteField", "replace-owned": "Replace", "store-scalar": "Scalar"}
        committed = []
        for item in paid_items:
            if item["semantic_event"] < 0: continue
            event = dynamic["events"][item["semantic_event"]]
            if operations.get(event["kind"]) == item["operation"]: committed.append(event)
        return committed


@dataclass(frozen=True)
class Case:
    identifier: str
    family: str
    program: Program | None = None
    source: str | None = None
    excluded: tuple[str, str, str] | None = None  # stage, code, unique marked fragment
    note: str = ""
    exact_span: tuple[int, int] | None = None


def standard_helpers(b):
    return (
        Function("read", (("p", "&S"),), "i32", b.block(b.ret(b.f("p", "value")))),
        Function("mutate", (("p", "&mut S"),), "i32", b.block(
            b.write("p", "value", b.op(b.f("p", "value"), "+", b.i(1))), b.ret(b.f("p", "value")))),
        Function("take", (("p", "S"),), "i32", b.block(b.ret(b.f("p", "value")))),
        Function("relay", (("p", "S"),), "S", b.block(b.ret(b.v("p")))),
    )


def simple_case(identifier, family, build, *, empty=False, result="i32", extra=None, preamble=""):
    b = Builder()
    helpers = standard_helpers(b) if not empty else ()
    if extra: helpers += tuple(extra(b))
    body = build(b)
    records = (Record("Empty"),) if empty else (Record("S", (("value", "i32"),)),)
    return Case(identifier, family, Program(records, helpers + (Function("main", (), result, b.block(*body)),), preamble))


def alias_cases():
    for arity in range(1, 5):
        for partition in alias_partitions(arity):
            for bits in itertools.product((False, True), repeat=arity):
                b = Builder()
                fn = Function("inspect", tuple((f"p{i}", "&mut S" if bit else "&S") for i, bit in enumerate(bits)),
                              "i32", b.block(b.ret(b.i(0))))
                declarations = [b.let(f"x{i}", b.lit("S", ("value", b.i(i))), True) for i in range(max(partition) + 1)]
                call = b.call("inspect", *(b.borrow(f"x{root}", bit) for root, bit in zip(partition, bits)))
                identifier = "alias-" + str(arity) + "-" + "".join(map(str, partition)) + "-" + "".join("x" if bit else "s" for bit in bits)
                yield Case(identifier, "07-alias-partitions", Program((Record("S", (("value", "i32"),)),),
                           (fn, Function("main", (), "i32", b.block(*declarations, b.ret(call))))))


def semantic_cases():
    yield Case("batch", "08-integrated-pilot", batch_program(), note="Exact RFC §5 source; separate arithmetic ledger")
    yield simple_case("empty-owned-move", "01-records", lambda b: [b.let("a", b.lit("Empty")), b.let("z", b.v("a")), b.ret(b.i(7))], empty=True)
    yield simple_case("empty-moved-use", "04-moves", lambda b: [b.let("a", b.lit("Empty")), b.discard(b.v("a")), b.discard(b.v("a")), b.ret(b.i(0))], empty=True)
    for permutation in itertools.permutations(("number", "flag", "unit")):
        b = Builder()
        fields = {"number": b.i(7), "flag": b.b(True), "unit": b.u()}
        main = Function("main", (), "i32", b.block(b.let("m", b.lit("Mixed", *((name, fields[name]) for name in permutation))), b.ret(b.f("m", "number"))))
        yield Case("mixed-fields-" + "-".join(permutation), "01-records", Program((Record("Mixed", (("number", "i32"), ("flag", "bool"), ("unit", "()"))),), (main,)))
    for identifier, fields in (("missing-field", ()), ("unknown-field", (("other", 1),)), ("duplicate-field", (("value", 1), ("value", 2)))):
        yield simple_case(identifier, "01-records", lambda b, fields=fields: [b.let("x", b.lit("S", *((name, b.i(v)) for name, v in fields))), b.ret(b.i(0))])
    b = Builder()
    main = Function("main", (), "i32", b.block(b.let("x", b.lit("A", ("v", b.i(1)))), b.let("y", b.v("x"), annotation="B"), b.ret(b.i(0))))
    yield Case("nominal-same-layout", "01-records", Program((Record("A", (("v", "i32"),)), Record("B", (("v", "i32"),))), (main,)))
    yield simple_case("unknown-projection", "02-names", lambda b: [b.let("x", b.lit("S", ("value", b.i(1)))), b.ret(b.f("x", "missing"))])
    yield simple_case("unknown-base", "02-names", lambda b: [b.ret(b.f("absent", "value"))])
    yield simple_case("active-shadow", "02-names", lambda b: [b.let("x", b.i(1)), b.iff(b.b(True), b.block(b.let("x", b.i(2)))), b.ret(b.i(0))])
    yield simple_case("lexical-reuse", "02-names", lambda b: [b.iff(b.b(True), b.block(b.let("x", b.lit("S", ("value", b.i(1)))))), b.let("x", b.lit("S", ("value", b.i(2)))), b.ret(b.f("x", "value"))])
    yield simple_case("function-collision", "02-names", lambda b: [b.let("read", b.i(1)), b.ret(b.i(0))])
    b = Builder()
    p = Program((Record("shared", (("value", "i32"),)),), (
        Function("main", (), "i32", b.block(b.let("shared", b.lit("shared", ("value", b.i(8)))), b.ret(b.f("shared", "value")))),))
    yield Case("type-local-homonym-forward", "02-names", replace(p, order=(("function", 0), ("record", 0))))
    b = Builder()
    yield Case("type-function-homonym", "02-names", Program((Record("same"),), (
        Function("same", (), "same", b.block(b.ret(b.lit("same")))),
        Function("main", (), "i32", b.block(b.discard(b.call("same")), b.ret(b.i(3)))))))
    for tag in ("let", "call", "discard"):
        def moved(b, tag=tag):
            use = {"let": lambda: b.let("y", b.v("x")), "call": lambda: b.discard(b.call("take", b.v("x"))), "discard": lambda: b.discard(b.v("x"))}[tag]()
            return [b.let("x", b.lit("S", ("value", b.i(4)))), use, b.ret(b.f("x", "value"))]
        yield simple_case("use-after-" + tag, "04-moves", moved)
    for mode in ("self", "relay", "moved-self"):
        def replacement(b, mode=mode):
            body = [b.let("x", b.lit("S", ("value", b.i(9))), True)]
            if mode == "moved-self": body.append(b.discard(b.v("x")))
            body += [b.store("x", b.call("relay", b.v("x")) if mode == "relay" else b.v("x")), b.ret(b.f("x", "value"))]
            return body
        yield simple_case("replace-" + mode, "04-moves", replacement)
    for both, restore in itertools.product((False, True), repeat=2):
        def joins(b, both=both, restore=restore):
            body = [b.let("x", b.lit("S", ("value", b.i(1))), True),
                    b.iff(b.b(True), b.block(b.discard(b.v("x"))), b.block(b.discard(b.v("x"))) if both else None)]
            if restore: body.append(b.store("x", b.lit("S", ("value", b.i(5)))))
            body.append(b.ret(b.f("x", "value")))
            return body
        yield simple_case(f"join-move-{int(both)}-restore-{int(restore)}", "04-moves", joins)
    yield simple_case("partial-recovery", "04-moves", lambda b: [b.let("x", b.lit("S", ("value", b.i(1))), True), b.discard(b.v("x")), b.write("x", "value", b.i(2)), b.ret(b.i(0))])
    yield simple_case("moved-terminating-arm", "06-flow", lambda b: [b.let("x", b.lit("S", ("value", b.i(3)))), b.iff(b.b(False), b.block(b.discard(b.v("x")), b.ret(b.i(0)))), b.ret(b.f("x", "value"))])
    for restore in (False, True):
        def cycle(b, restore=restore):
            body = [b.discard(b.v("x"))]
            if restore: body.append(b.store("x", b.lit("S", ("value", b.i(4)))))
            body += [b.store("n", b.op(b.v("n"), "+", b.i(1))), b.cont()]
            return [b.let("x", b.lit("S", ("value", b.i(1))), True), b.let("n", b.i(0), True), b.loop(b.op(b.v("n"), "<", b.i(2)), b.block(*body)), b.ret(b.i(0))]
        yield simple_case("continue-move-restore-" + str(int(restore)), "06-flow", cycle)
    for transfer in ("break", "continue", "return"):
        def cleanup(b, transfer=transfer):
            jump = b.brk() if transfer == "break" else b.cont() if transfer == "continue" else b.ret(b.i(17))
            return [b.let("n", b.i(0), True), b.loop(b.op(b.v("n"), "<", b.i(2)), b.block(
                b.let("first", b.call("relay", b.lit("S", ("value", b.i(1))))),
                b.store("n", b.op(b.v("n"), "+", b.i(1))), b.iff(b.b(True), b.block(jump)),
                b.let("later", b.lit("S", ("value", b.i(2)))))), b.ret(b.i(3))]
        yield simple_case("cleanup-prefix-" + transfer, "06-flow", cleanup)
    yield simple_case("false-loop-initial", "06-flow", lambda b: [b.loop(b.b(False), b.block(b.let("x", b.lit("S", ("value", b.i(4)))))), b.ret(b.i(1))])
    yield simple_case("unreachable-after-return", "06-flow", lambda b: [b.ret(b.i(1)), b.let("never", b.lit("S", ("value", b.i(2))))])
    yield simple_case("unreachable-after-break", "06-flow", lambda b: [b.loop(b.b(True), b.block(b.brk(), b.let("never", b.lit("S", ("value", b.i(2)))))), b.ret(b.i(0))])
    # Immediate scalar snapshots are distinct from later owner reads.
    for reverse in (False, True):
        def extra(b, reverse=reverse):
            parameters = (("p", "&mut S"), ("v", "i32")) if reverse else (("v", "i32"), ("p", "&mut S"))
            return [Function("snapshot", parameters, "i32", b.block(b.ret(b.v("v"))))]
        yield simple_case("snapshot-order-" + str(int(reverse)), "07-immediate-arguments", lambda b, reverse=reverse: [
            b.let("x", b.lit("S", ("value", b.i(3))), True), b.ret(b.call("snapshot", *( (b.borrow("x", True), b.f("x", "value")) if reverse else (b.f("x", "value"), b.borrow("x", True)))))], extra=extra)
    for reverse in (False, True):
        def extra(b, reverse=reverse):
            return [Function("mix", (("p", "&S"), ("v", "S")) if reverse else (("v", "S"), ("p", "&S")), "i32", b.block(b.ret(b.i(0))))]
        yield simple_case("move-loan-order-" + str(int(reverse)), "07-immediate-arguments", lambda b, reverse=reverse: [
            b.let("x", b.lit("S", ("value", b.i(1)))), b.ret(b.call("mix", *((b.borrow("x"), b.v("x")) if reverse else (b.v("x"), b.borrow("x")))))], extra=extra)
    for parent in ("shared", "exclusive"):
        for child in ("shared", "exclusive"):
            def extra(b, parent=parent, child=child):
                return [Function("forward", (("p", "&S" if parent == "shared" else "&mut S"),), "i32", b.block(
                    b.ret(b.call("read" if child == "shared" else "mutate", b.borrow("p", child == "exclusive", True)))))]
            yield simple_case(f"reborrow-{parent}-{child}", "08-capabilities", lambda b, parent=parent: [
                b.let("x", b.lit("S", ("value", b.i(1))), True), b.ret(b.call("forward", b.borrow("x", parent == "exclusive")))], extra=extra)
    def shared_children(b):
        return [Function("two", (("a", "&S"), ("z", "&S")), "i32", b.block(b.ret(b.op(b.f("a", "value"), "+", b.f("z", "value"))))),
                Function("forward", (("p", "&mut S"),), "i32", b.block(b.ret(b.call("two", b.borrow("p", forwarded=True), b.borrow("p", forwarded=True)))))]
    yield simple_case("shared-children-exclusive-parent", "08-capabilities", lambda b: [b.let("x", b.lit("S", ("value", b.i(4))), True), b.ret(b.call("forward", b.borrow("x", True)))], extra=shared_children)
    for inner in ("read", "mutate"):
        def extra(b): return [Function("hold", (("p", "&S"), ("v", "i32")), "i32", b.block(b.ret(b.v("v"))))]
        yield simple_case("nested-argument-" + inner, "09-nested-lazy", lambda b, inner=inner: [b.let("x", b.lit("S", ("value", b.i(6))), True), b.ret(b.call("hold", b.borrow("x"), b.call(inner, b.borrow("x", inner == "mutate"))))], extra=extra)
    for flag in (False, True):
        def extra(b): return [Function("hold", (("p", "&S"), ("v", "bool")), "bool", b.block(b.ret(b.v("v"))))]
        yield simple_case("lazy-nested-conflict-" + str(int(flag)), "09-nested-lazy", lambda b, flag=flag: [b.let("x", b.lit("S", ("value", b.i(0))), True), b.ret(b.call("hold", b.borrow("x"), b.op(b.b(flag), "&&", b.group(b.op(b.call("mutate", b.borrow("x", True)), ">", b.i(0))))))], result="bool", extra=extra)
    for flag in (False, True):
        yield simple_case("lazy-dynamic-effects-" + str(int(flag)), "09-nested-lazy", lambda b, flag=flag: [b.let("x", b.lit("S", ("value", b.i(0))), True), b.discard(b.op(b.b(flag), "&&", b.group(b.op(b.call("mutate", b.borrow("x", True)), ">", b.i(0))))), b.ret(b.f("x", "value"))])
    yield simple_case("field-target-after-rhs-move", "10-stores", lambda b: [b.let("x", b.lit("S", ("value", b.i(1))), True), b.write("x", "value", b.call("take", b.v("x"))), b.ret(b.i(0))])
    yield simple_case("field-target-after-inner-mutation", "10-stores", lambda b: [b.let("x", b.lit("S", ("value", b.i(1))), True), b.write("x", "value", b.call("mutate", b.borrow("x", True))), b.ret(b.f("x", "value"))])
    yield simple_case("whole-replace-nested-constructor", "10-stores", lambda b: [b.let("x", b.lit("S", ("value", b.i(1))), True), b.store("x", b.lit("S", ("value", b.call("take", b.v("x"))))), b.ret(b.call("read", b.borrow("x")))])
    for context in ("consume", "assign", "discard", "return"):
        def body(b, context=context):
            value = b.call("relay", b.lit("S", ("value", b.i(9))))
            if context == "consume": return [b.ret(b.call("take", value))]
            if context == "assign": return [b.let("x", value), b.ret(b.f("x", "value"))]
            if context == "discard": return [b.discard(value), b.ret(b.i(9))]
            return [b.ret(value)]
        yield simple_case("owned-result-" + context, "11-results", body, result="S" if context == "return" else "i32")
    def pair_owned(b): return [Function("pair", (("a", "S"), ("z", "S")), "i32", b.block(b.ret(b.op(b.f("a", "value"), "+", b.f("z", "value")))))]
    yield simple_case("independent-owned-results", "11-results", lambda b: [b.ret(b.call("pair", b.call("relay", b.lit("S", ("value", b.i(2)))), b.call("relay", b.lit("S", ("value", b.i(5))))))], extra=pair_owned)
    yield simple_case("immutable-exclusive-borrow", "14-diagnostics", lambda b: [b.let("x", b.lit("S", ("value", b.i(1)))), b.ret(b.call("mutate", b.borrow("x", True)))])
    yield simple_case("immutable-field-write", "14-diagnostics", lambda b: [b.let("x", b.lit("S", ("value", b.i(1)))), b.write("x", "value", b.i(2)), b.ret(b.i(0))])
    for mode in ("shared", "exclusive"):
        def extra(b, mode=mode): return [Function("write", (("p", "&S" if mode == "shared" else "&mut S"),), "i32", b.block(b.write("p", "value", b.i(2)), b.ret(b.f("p", "value"))))]
        yield simple_case("reference-write-" + mode, "14-diagnostics", lambda b, mode=mode: [b.let("x", b.lit("S", ("value", b.i(1))), True), b.ret(b.call("write", b.borrow("x", mode == "exclusive")))], extra=extra)
    yield simple_case("overflow-after-write", "09-nested-lazy", lambda b: [b.let("x", b.lit("S", ("value", b.i(0))), True), b.discard(b.call("mutate", b.borrow("x", True))), b.write("x", "value", b.op(b.i(MAX_I32), "+", b.i(1))), b.ret(b.i(0))])


def excluded_cases():
    # Exactly authored syntax negatives. No parser is used to invent expectations.
    prefix = "struct S { value: i32 }\n"
    fixtures = (
        ("reference-local", "let r = &x;", "&x", "parse", "E0101"),
        ("temporary-literal-borrow", "read(&S { value: 1 });", "S", "parse", "E0101"),
        ("temporary-call-borrow", "read(&make());", "(", "parse", "E0101"),
        ("field-borrow", "read(&x.value);", ".", "parse", "E0101"),
        ("parenthesized-borrow", "read((&x));", "&x", "parse", "E0101"),
        ("parenthesized-place", "read(&(x));", "(", "parse", "E0101"),
        ("arbitrary-deref", "*x;", "*", "parse", "E0101"),
        ("group-projection", "(x).value;", ".", "parse", "E0101"),
        ("chained-projection", "x.value.other;", ".other", "parse", "E0101"),
        ("shorthand-literal", "let y = S { value };", "value", "parse", "E0101"),
        ("update-literal", "let y = S { ..x };", "..", "parse", "E0101"),
        ("trailing-call-comma", "read(&x,);", ")", "parse", "E0101"),
        ("condition-literal", "if S { value: 1 } { return 0; }", ":", "parse", "E0101"),
    )
    for identifier, statement, fragment, stage, code in fixtures:
        source = prefix + "fn main() -> i32 { let x = S { value: 1 }; " + statement + " return 0; }\n"
        token = {"reference-local": "&", "temporary-literal-borrow": "{", "temporary-call-borrow": "(",
                 "field-borrow": ".", "parenthesized-borrow": "&", "parenthesized-place": "(",
                 "arbitrary-deref": "*", "group-projection": ".", "chained-projection": ".",
                 "shorthand-literal": "}", "update-literal": ".", "trailing-call-comma": ")", "condition-literal": ":"}[identifier]
        if identifier == "temporary-call-borrow": offset = statement.index("(", statement.index("make"))
        elif identifier == "parenthesized-place": offset = statement.index("(", statement.index("&"))
        elif identifier == "chained-projection": offset = statement.index(".other")
        elif identifier == "trailing-call-comma": offset = statement.rindex(")")
        else: offset = statement.index(token)
        start = len(source[:source.index(statement) + offset].encode())
        code = "E0101" if token in ("&", ".") else "E0100"
        yield Case("excluded-" + identifier, "03-excluded-syntax", source=source,
                   excluded=(stage, code, token), exact_span=(start, start + len(token.encode())),
                   note="Exact first unexpected grammar token, frozen before compiler comparison")
    for identifier, source, fragment in (
        ("reference-field", "struct S { value: &S }\nfn main() -> i32 { return 0; }\n", "&S"),
        ("reference-result", prefix + "fn bad() -> &S { return; }\n", "&S"),
        ("mutable-parameter", prefix + "fn bad(mut x: S) -> () { return; }\n", "mut x"),
        ("trailing-parameter-comma", prefix + "fn bad(x: S,) -> () { return; }\n", ",)"),
    ):
        token = "mut" if identifier == "mutable-parameter" else ")" if identifier == "trailing-parameter-comma" else "&"
        offset = source.index(",)") + 1 if identifier == "trailing-parameter-comma" else source.index(token)
        start = len(source[:offset].encode())
        yield Case("excluded-" + identifier, "03-excluded-syntax", source=source, excluded=("parse", "E0101" if token == "&" else "E0100", token),
                   exact_span=(start, start + len(token.encode())), note="Exact first unexpected grammar token")




def correspondence_facts(checked, rendered):
    """Normalize facts by independently supplied origins, never producer ID spelling.

    The future inspector must resolve raw identity equality back to these origins.
    It must not copy this document into its observation or ask raw verification to
    decide whether a mutable/nominal/mode/snapshot source fact is true.
    """
    records = []
    for index, record in enumerate(checked.program.records):
        records.append({"identity": f"record.{index}", "name": record.name, "span": rendered.span(f"record.{index}.name"),
                        "fields": [{"identity": f"record.{index}.field.{position}", "name": name, "type": ty,
                                    "span": rendered.span(f"record.{index}.field.{position}.name")}
                                   for position, (name, ty) in enumerate(record.fields)]})
    bindings = []
    for fn in checked.program.functions:
        for key in checked.function_bindings[fn.name]:
            binding = checked.bindings[key]
            bindings.append({"identity": key, "function": fn.name, "name": binding.name, "type": binding.ty,
                             "mutable": binding.mutable, "position": binding.position, "class": binding.kind,
                             "span": rendered.span(binding.origin)})
    calls, transfers, projections, literals, stores = [], [], [], [], []
    def owned_result(n):
        if checked.types[n.key] in SCALARS or ref_type(checked.types[n.key]): return None
        if n.tag == "group": return owned_result(n.data["value"])
        return ("result:" if n.tag == "call" else "temporary:") + n.key
    def field_fact(n, fn, kind):
        record, field = checked.projections[n.key]
        binding = checked.bindings[checked.references[n.key]]
        return {"kind": kind, "function": fn.name, "expression": n.key,
                "base_binding": binding.key, "base_kind": "reference-parameter" if ref_type(binding.ty) else "owner",
                "record_identity": f"record.{record}", "field_identity": f"record.{record}.field.{field}",
                "span": rendered.span(n.key), "base_span": rendered.span(n.key + ".base"),
                "field_span": rendered.span(n.key + ".field")}
    def expr(n, fn, parent, statement):
        d, tag = n.data, n.tag
        if tag == "call":
            args = []
            callee = checked.functions[d["function"]]
            for position, (arg, (_, ty)) in enumerate(zip(d["args"], callee.parameters)):
                mode = ref_type(ty)[0] if ref_type(ty) else "scalar" if ty in SCALARS else "owned"
                args.append({"position": position, "mode": mode, "type": ty, "span": rendered.span(arg.key),
                             "snapshot_expression": arg.key if mode == "scalar" else None,
                             "immediate_prepare": True})
            calls.append({"identity": n.key, "function": fn.name, "callee": callee.name,
                          "span": rendered.span(n.key), "parent": parent, "arguments": args,
                          "result_type": callee.result, "result_owner": owned_result(n)})
            for position, arg in enumerate(d["args"]): expr(arg, fn, {"call": n.key, "argument": position}, statement)
        elif tag == "name" and checked.types[n.key] not in SCALARS:
            transfers.append({"source_binding": checked.references[n.key], "destination": "temporary:" + n.key,
                              "destination_class": "temporary", "primary": rendered.span(n.key),
                              "charge": rendered.span(n.key), "cause": rendered.span(statement)})
        elif tag == "field":
            projections.append({**field_fact(n, fn, "read"), "result_type": checked.types[n.key]})
        elif tag == "literal":
            record_index, record = checked.records[d["record"]]
            field_positions = {name: position for position, (name, _) in enumerate(record.fields)}
            literals.append({"function": fn.name, "expression": n.key, "record_identity": f"record.{record_index}",
                             "span": rendered.span(n.key), "record_span": rendered.span(n.key + ".record"),
                             "result_owner": owned_result(n), "written_fields": [
                                 {"position": position, "field_identity": f"record.{record_index}.field.{field_positions[name]}",
                                  "field_span": rendered.span(f"{n.key}.field.{position}"), "value_expression": value.key,
                                  "value_type": checked.types[value.key], "value_span": rendered.span(value.key)}
                                 for position, (name, value) in enumerate(d["fields"])]})
            for _, value in d["fields"]: expr(value, fn, parent, statement)
        elif tag in ("group", "unary"): expr(d["value"], fn, parent, statement)
        elif tag == "binary": expr(d["lhs"], fn, parent, statement); expr(d["rhs"], fn, parent, statement)
    def block(n, fn):
        for statement in n.data["statements"]:
            d, tag = statement.data, statement.tag
            if isinstance(d.get("value"), Node): expr(d["value"], fn, None, statement.key)
            if tag in ("let", "store", "write"):
                binding = checked.bindings[statement.key if tag == "let" else checked.references[statement.key]]
                store = {"kind": tag, "function": fn.name, "statement": statement.key, "target_binding": binding.key,
                         "target_type": binding.ty, "target_span": rendered.span(statement.key + (".name" if tag == "let" else ".target")),
                         "value_expression": d["value"].key, "value_type": checked.types[d["value"].key],
                         "value_span": rendered.span(d["value"].key), "source_owner": owned_result(d["value"]),
                         "charge_span": rendered.span(statement.key), "rhs_before_store": True}
                if tag == "write": store["projection"] = field_fact(statement, fn, "write")
                stores.append(store)
            if tag in ("if", "while"):
                expr(d["condition"], fn, None, statement.key)
                block(d["yes"] if tag == "if" else d["body"], fn)
                if tag == "if" and d["no"]: block(d["no"], fn)
            elif tag == "block": block(statement, fn)
    for fn in checked.program.functions: block(fn.body, fn)
    return {"schema": SCHEMA, "source_sha256": rendered.sha256, "records": records,
            "bindings": bindings, "calls": calls, "transfers": transfers,
            "projections": projections, "literals": literals, "stores": stores}


def correspondence_differences(expected, observed, path="facts"):
    """A structural comparison only. It creates no witness or acceptance result."""
    failures = []
    if type(expected) is not type(observed): return [f"{path}: type mismatch"]
    if isinstance(expected, dict):
        for key in sorted(expected.keys() | observed.keys()):
            if key not in expected or key not in observed: failures.append(f"{path}.{key}: missing/unexpected field")
            else: failures.extend(correspondence_differences(expected[key], observed[key], f"{path}.{key}"))
    elif isinstance(expected, list):
        if len(expected) != len(observed): failures.append(f"{path}: length mismatch")
        for index, (left, right) in enumerate(zip(expected, observed)):
            failures.extend(correspondence_differences(left, right, f"{path}[{index}]"))
    elif expected != observed: failures.append(f"{path}: value mismatch")
    return failures


def alias_diagnostic_alternatives(program, checked, rendered):
    """First conflict per root, with its earliest still-live source cause.

    This narrowly applies to the direct-owner alias partition corpus. It never
    reads an observed verifier ordering, and stops after each root's first error.
    """
    main = next(function for function in program.functions if function.name == "main")
    call = main.body.data["statements"][-1].data["value"]
    if call.tag != "call" or any(arg.tag != "borrow" or arg.data["forwarded"] for arg in call.data["args"]):
        raise ValueError("MODEL_DOMAIN: alias alternatives require direct-owner borrow arguments")
    prior, failed, alternatives = {}, set(), []
    for argument in call.data["args"]:
        root = checked.references[argument.key]
        if root in failed: continue
        earlier = prior.setdefault(root, [])
        conflict = next((previous for previous in earlier if previous.data["mutable"] or argument.data["mutable"]), None)
        if conflict is not None:
            alternatives.append({"stage": "ownership", "code": "E0311", "span": rendered.span(argument.key),
                                 "cause_span": rendered.span(conflict.key),
                                 "declaration_span": rendered.span(checked.bindings[root].origin), "root_binding": root})
            failed.add(root)
        else: earlier.append(argument)
    return sorted(alternatives, key=lambda item: (item["declaration_span"], item["span"]))


def case_expectation(case):
    if case.program is None:
        source = case.source
        stage, code, fragment = case.excluded
        if case.exact_span is None: raise ValueError("authored syntax negative requires an exact frozen token span")
        origin = list(case.exact_span)
        return {"schema": SCHEMA, "id": case.identifier, "family": case.family, "seed": SEED,
                "source_sha256": hashlib.sha256(source.encode()).hexdigest(), "source": source,
                "origins": {"excluded-region": origin}, "expected": {"status": "reject", "stage": stage, "code": code,
                "span": origin, "span_match": "exact"}, "note": case.note,
                "evidence_kind": "independently-authored-syntax-negative"}
    rendered = render(case.program)
    result = {"schema": SCHEMA, "id": case.identifier, "family": case.family, "seed": SEED,
              "source_sha256": rendered.sha256, "source": rendered.source,
              "origins": {key: list(value) for key, value in sorted(rendered.origins.items())},
              "note": case.note, "function_count": len(case.program.functions), "evidence_kind": "independent-tagged-source-model"}
    try:
        checked = NamesTypes(case.program).check()
    except Rejected as error:
        result["expected"] = {"status": "reject", **error.diagnostic.json(rendered), "span_match": "exact"}
        return result
    result["facts"] = correspondence_facts(checked, rendered)
    result["template_frames"] = TemplateScheduler(checked).frames
    static = static_check(checked, rendered)
    result["static"] = {"accepted": static["accepted"], "functions": static["functions"]}
    if not static["accepted"]:
        diagnostics = static["diagnostics"]
        # Alternative reachable failures are retained, never tuned to verifier traversal.
        unique = {json.dumps(error.json(rendered), sort_keys=True): error.json(rendered) for error in diagnostics}
        result["expected"] = {"status": "reject", **diagnostics[0].json(rendered), "span_match": "exact",
                              "reachable_diagnostics": list(unique.values()),
                              "selection": "single reachable error" if len(unique) == 1 else "independently reachable alternatives; selection not qualified"}
        if case.family == "07-alias-partitions":
            alternatives = alias_diagnostic_alternatives(case.program, checked, rendered)
            if not alternatives: raise AssertionError("rejected alias program has no independently justified conflict")
            chosen = next(item for item in alternatives if item["span"] == result["expected"]["span"])
            result["expected"]["diagnostic_alternatives"] = alternatives
            result["expected"]["related"] = [chosen["cause_span"], chosen["declaration_span"]]
            result["expected"]["span_match"] = "exact" if len(alternatives) == 1 else "exact-alternative"
            result["expected"]["selection"] = "first conflicting acquisition per root, earliest live cause; actual selection must be deterministic"
        return result
    dynamic = Machine(checked).run()
    if dynamic["failure"] and dynamic["failure"]["code"].startswith("MODEL"):
        raise ValueError("MODEL_INCOMPLETE: dynamic event bound cannot become a compiler runtime expectation")
    schedule = TemplateScheduler(checked).schedule(dynamic)
    result["dynamic"], result["schedule"] = dynamic, schedule
    result["expected"] = {"status": "accept", "result": dynamic["result"], "result_type": dynamic["result_type"],
                          "runtime_failure": dynamic["failure"]}
    return result


def coverage_manifest(expectations):
    families = {}
    for item in expectations:
        family = families.setdefault(item["family"], {"cases": 0, "accepted": 0, "rejected": 0,
                   "maximum_static_points": 0, "maximum_reached_states": 0, "maximum_owners": 0, "maximum_calls": 0})
        family["cases"] += 1
        family["accepted" if item["expected"]["status"] == "accept" else "rejected"] += 1
        for function in item.get("static", {}).get("functions", {}).values():
            family["maximum_static_points"] = max(family["maximum_static_points"], function["points"])
            family["maximum_reached_states"] = max(family["maximum_reached_states"], function["states"])
        for counts in item.get("template_frames", {}).values():
            family["maximum_owners"] = max(family["maximum_owners"], counts["O"])
            family["maximum_calls"] = max(family["maximum_calls"], counts["C"])
    return {"schema": SCHEMA, "model_seed": SEED, "qualification": "MODEL_ONLY; production source is not admitted by this manifest",
            "unique_source_programs": len(expectations), "families": families,
            "oracle_limits": {"reference_parameters_per_function": 4, "static_states_per_function": 100000,
                              "dynamic_events": 100000, "limit_outcome": "MODEL_INCOMPLETE/error, never acceptance"},
            "actual_cli_invocations": 0, "actual_llvm_modules": 0, "actual_native_executions": 0,
            "pending_gates": ["candidate-source adapter and correspondence observations",
                              "raw-invalid operation/role classification mutants", "all resource boundary/native/failure-store gates",
                              "exact debug/release production CLI qualification", "independent held-out reviewer corpus"]}


def extended_cases():
    """Focused cross-feature pairs absent from the compact alias enumeration."""
    # Type-only exclusions are grammatical source and go through independent types.
    b = Builder()
    helpers = standard_helpers(b)
    main = Function("main", (), "i32", b.block(b.let("x", b.lit("S", ("value", b.i(1)))),
        b.discard(b.call("take", b.v("x"))), b.ret(b.call("read", b.borrow("x", trivia=" /*雪🦀*/ ")))))
    yield Case("utf8-crlf-borrow-origin", "14-diagnostics", Program((Record("S", (("value", "i32"),)),), helpers + (main,), "// 雪🦀\r\n", "\r\n"))
    for number in (MIN_I32 - 1, MIN_I32, MAX_I32, MAX_I32 + 1):
        yield simple_case("scalar-literal-" + str(number), "00-scalar-contract", lambda b, number=number: [b.ret(b.i(number))])
    yield simple_case("scalar-general-minus", "00-scalar-contract", lambda b: [b.let("x", b.i(3)), b.ret(b.unary("-", b.v("x")))])
    yield simple_case("scalar-minus-group", "00-scalar-contract", lambda b: [b.ret(b.unary("-", b.group(b.i(1))))])
    yield simple_case("scalar-unary-plus", "00-scalar-contract", lambda b: [b.ret(b.unary("+", b.i(1)))])
    for operator, identifier in (("/", "division"), ("%", "remainder")):
        yield simple_case("scalar-unsupported-" + identifier, "00-scalar-contract", lambda b, operator=operator: [b.ret(b.op(b.i(7), operator, b.i(3)))])
    yield simple_case("scalar-first-invalid-arithmetic-operand", "00-scalar-contract", lambda b: [b.ret(b.op(b.b(True), "+", b.i(1)))])
    yield simple_case("scalar-second-invalid-comparison-operand", "00-scalar-contract", lambda b: [b.ret(b.op(b.i(1), "<", b.b(True)))], result="bool")
    yield simple_case("scalar-not-grouped-logical", "00-scalar-contract", lambda b: [b.ret(b.unary("!", b.group(b.op(b.b(True), "&&", b.b(False)))))], result="bool")
    yield simple_case("scalar-grouped-comparison", "00-scalar-contract", lambda b: [b.ret(b.op(b.b(True), "!=", b.group(b.op(b.i(1), ">=", b.i(2)))))], result="bool")
    for index, expression in enumerate(("1 < 2 < 3", "1 == 2 < 3", "1 < 2 == true")):
        source = "struct Empty {}\nfn main() -> bool { return " + expression + "; }\n"
        token = "==" if index == 2 else "<"
        offset = source.rindex(token)
        yield Case("scalar-comparison-chain-" + str(index), "00-scalar-contract", source=source,
                   excluded=("parse", "E0100", token), exact_span=(offset, offset + len(token)),
                   note="RFC0009 non-associative comparison tier; exact second comparator")
    # Exact reference parameter modes: no implicit exclusive-to-shared coercion.
    yield simple_case("borrow-mode-exclusive-to-shared", "03-excluded-types", lambda b: [b.let("x", b.lit("S", ("value", b.i(1))), True), b.ret(b.call("read", b.borrow("x", True)))])
    yield simple_case("borrow-mode-shared-to-exclusive", "03-excluded-types", lambda b: [b.let("x", b.lit("S", ("value", b.i(1))), True), b.ret(b.call("mutate", b.borrow("x")))])
    yield simple_case("scalar-borrow", "03-excluded-types", lambda b: [b.let("n", b.i(1), True), b.ret(b.call("read", b.borrow("n")))])
    yield simple_case("aggregate-equality", "03-excluded-types", lambda b: [b.let("a", b.lit("S", ("value", b.i(1)))), b.let("z", b.lit("S", ("value", b.i(1)))), b.ret(b.op(b.v("a"), "==", b.v("z")))], result="bool")
    for form in ("bare", "direct-borrow", "owner-forward"):
        def helper(b, form=form):
            if form == "bare": value = b.v("p")
            elif form == "direct-borrow": value = b.call("read", b.borrow("p"))
            else: value = b.call("read", b.borrow("p", forwarded=True))
            return [Function("bad", (("p", "S" if form == "owner-forward" else "&S"),), "i32", b.block(b.discard(value), b.ret(b.i(0))))]
        yield simple_case("excluded-reference-" + form, "03-excluded-types", lambda b: [b.ret(b.i(0))], extra=helper)
    b = Builder()
    yield Case("nested-record-field", "03-excluded-types", Program((Record("S"), Record("T", (("nested", "S"),))), (Function("main", (), "i32", b.block(b.ret(b.i(0)))),)))
    for space in ("record", "function", "field"):
        b = Builder()
        records = (Record("S"), Record("S")) if space == "record" else (Record("S", (("v", "i32"), ("v", "bool"))),) if space == "field" else (Record("S"),)
        functions = (Function("main", (), "i32", b.block(b.ret(b.i(0)))),)
        if space == "function": functions += (Function("main", (), "i32", b.block(b.ret(b.i(1)))),)
        yield Case("duplicate-namespace-" + space, "02-names", Program(records, functions))
    for inner_mode, later in (("shared", "read"), ("exclusive", "read"), ("shared", "exclusive-child")):
        def extra(b, inner_mode=inner_mode, later=later):
            parameter = "&S" if inner_mode == "shared" else "&mut S"
            hold = Function("hold", (("q", parameter), ("v", "i32")), "i32", b.block(b.ret(b.v("v"))))
            if later == "read": rhs = b.f("p", "value")
            else: rhs = b.call("mutate", b.borrow("p", True, True))
            forward = Function("forward", (("p", "&mut S"),), "i32", b.block(b.ret(b.call("hold", b.borrow("p", inner_mode == "exclusive", True), rhs))))
            return [hold, forward]
        yield simple_case(f"parent-access-{inner_mode}-{later}", "08-capabilities", lambda b: [b.let("x", b.lit("S", ("value", b.i(5))), True), b.ret(b.call("forward", b.borrow("x", True)))], extra=extra)
    # Declaration order differs from written evaluation order; helper writes expose it.
    for reverse in (False, True):
        b = Builder()
        tick = Function("tick", (("p", "&mut S"),), "i32", b.block(b.write("p", "value", b.op(b.f("p", "value"), "+", b.i(1))), b.ret(b.f("p", "value"))))
        order = ("b", "a") if reverse else ("a", "b")
        value = b.lit("Pair", *((field_name, b.call("tick", b.borrow("counter", True))) for field_name in order))
        main = Function("main", (), "i32", b.block(b.let("counter", b.lit("S", ("value", b.i(0))), True), b.let("pair", value), b.ret(b.op(b.op(b.f("pair", "a"), "*", b.i(10)), "+", b.f("pair", "b")))))
        yield Case("written-field-effects-" + str(int(reverse)), "01-records", Program((Record("S", (("value", "i32"),)), Record("Pair", (("a", "i32"), ("b", "i32")))), (tick, main)))
    b = Builder()
    helpers = standard_helpers(b)
    fail_literal = b.lit("Pair", ("a", b.call("mutate", b.borrow("counter", True))), ("b", b.op(b.i(MAX_I32), "+", b.i(1))))
    main = Function("main", (), "i32", b.block(b.let("counter", b.lit("S", ("value", b.i(0))), True), b.let("pair", fail_literal), b.ret(b.i(0))))
    yield Case("literal-overflow-before-construction", "01-records", Program((Record("S", (("value", "i32"),)), Record("Pair", (("a", "i32"), ("b", "i32")))), helpers + (main,)))
    for grouped in (False, True):
        yield simple_case("condition-old-flag-" + str(int(grouped)), "05-condition-grammar", lambda b, grouped=grouped: [b.let("flag", b.b(True)), b.iff(b.group(b.v("flag")) if grouped else b.v("flag"), b.block(b.ret(b.i(1))), b.block(b.ret(b.i(0))))])
    def predicate(b): return [Function("pred", (("p", "S"),), "bool", b.block(b.ret(b.op(b.f("p", "value"), ">", b.i(0)))))]
    yield simple_case("condition-nested-literal-call", "05-condition-grammar", lambda b: [b.iff(b.call("pred", b.lit("S", ("value", b.i(1)))), b.block(b.ret(b.i(1))), b.block(b.ret(b.i(0))))], extra=predicate)
    yield simple_case("condition-grouped-aggregate-comparison", "05-condition-grammar", lambda b: [b.iff(b.group(b.op(b.lit("S", ("value", b.i(1))), "==", b.lit("S", ("value", b.i(1))))), b.block(b.ret(b.i(1))), b.block(b.ret(b.i(0))))])
    for outcome in ("break", "continue", "return"):
        def nested(b, outcome=outcome):
            transfer = b.brk() if outcome == "break" else b.cont() if outcome == "continue" else b.ret(b.i(23))
            return [b.let("i", b.i(0), True), b.loop(b.op(b.v("i"), "<", b.i(2)), b.block(
                b.let("outer", b.lit("S", ("value", b.i(1)))), b.store("i", b.op(b.v("i"), "+", b.i(1))), b.let("j", b.i(0), True),
                b.loop(b.op(b.v("j"), "<", b.i(2)), b.block(b.let("inner", b.call("relay", b.lit("S", ("value", b.i(2))))), b.store("j", b.op(b.v("j"), "+", b.i(1))), transfer)))), b.ret(b.i(11))]
        yield simple_case("nested-loop-cleanup-" + outcome, "06-flow", nested)
    def unreachable_cycle(b): return [Function("unused", (), "()", b.block(b.loop(b.b(False), b.block(b.cont())), b.ret()))]
    yield simple_case("unused-real-cycle", "06-flow", lambda b: [b.ret(b.i(1))], extra=unreachable_cycle)
    yield simple_case("syntactic-terminating-loop", "06-flow", lambda b: [b.loop(b.b(True), b.block(b.brk())), b.ret(b.i(1))])
    def interleaved(b):
        return [Function("interleave", (("v", "i32"), ("owned", "S"), ("p", "&S"), ("flag", "bool")), "i32", b.block(b.ret(b.op(b.op(b.v("v"), "+", b.f("owned", "value")), "+", b.f("p", "value")))))]
    yield simple_case("interleaved-parameter-positions", "11-results", lambda b: [b.let("x", b.lit("S", ("value", b.i(3)))), b.ret(b.call("interleave", b.i(1), b.lit("S", ("value", b.i(2))), b.borrow("x"), b.b(True)))], extra=interleaved)
    b = Builder()
    fn = Function("empty_between", (("a", "i32"), ("e", "Empty"), ("z", "i32")), "i32", b.block(b.ret(b.op(b.v("a"), "+", b.v("z")))))
    main = Function("main", (), "i32", b.block(b.ret(b.call("empty_between", b.i(3), b.lit("Empty"), b.i(5)))))
    yield Case("empty-between-scalars", "11-results", Program((Record("Empty"),), (fn, main)))
    b = Builder()
    pair = Function("pair", (("first", "i32"), ("second", "i32")), "i32", b.block(b.ret(b.v("first"))))
    helpers = standard_helpers(b)
    main = Function("main", (), "i32", b.block(b.let("x", b.lit("S", ("value", b.i(0))), True), b.ret(b.call("pair", b.call("mutate", b.borrow("x", True)), b.op(b.i(MAX_I32), "+", b.i(1))))))
    yield Case("argument-overflow-keeps-earlier-write", "09-nested-lazy", Program((Record("S", (("value", "i32"),)),), helpers + (pair, main)))
    # Valid parse but unavailable use in an unused helper is still rejected.
    def unused_bad(b): return [Function("unused", (("p", "S"),), "i32", b.block(b.discard(b.v("p")), b.ret(b.f("p", "value"))))]
    yield simple_case("unused-helper-owned-use", "04-moves", lambda b: [b.ret(b.i(0))], extra=unused_bad)




def batch_variants():
    for variant in ("conditional-move", "continue-carried-move", "alias-conflict", "nested-reborrow-conflict",
                    "use-after-relay", "shared-after-reinitialization", "overflow-after-earlier-write"):
        program = batch_program()
        b = Builder(); b.serial = 10000
        functions = list(program.functions)
        main = functions[-1]
        statements = list(main.body.data['statements'])
        if variant == "conditional-move":
            statements.insert(1, b.iff(b.b(True), b.block(b.discard(b.v('state')))))
        elif variant == "continue-carried-move":
            outer = statements[1]
            body = outer.data['body']
            updated = list(body.data['statements'])
            updated.insert(-1, b.discard(b.v('state')))
            statements[1] = replace(outer, data={**outer.data, 'body': replace(body, data={'statements': tuple(updated)})})
        elif variant == "alias-conflict":
            functions.insert(-1, Function('conflict', (('p', '&mut Batch'), ('q', '&Batch')), '()', b.block(b.ret())))
            statements.insert(1, b.discard(b.call('conflict', b.borrow('state', True), b.borrow('state'))))
        elif variant == "nested-reborrow-conflict":
            functions.insert(-1, Function('hold', (('p', '&Batch'), ('v', '()')), '()', b.block(b.ret())))
            functions.insert(-1, Function('bad_forward', (('p', '&mut Batch'),), '()', b.block(
                b.discard(b.call('hold', b.borrow('p', forwarded=True), b.call('retry', b.borrow('p', True, True)))), b.ret())))
            statements.insert(1, b.discard(b.call('bad_forward', b.borrow('state', True))))
        elif variant == "use-after-relay": statements.insert(-1, b.discard(b.f('state', 'checksum')))
        elif variant == "shared-after-reinitialization":
            statements.insert(-1, b.store('state', b.lit('Batch', ('completed', b.i(0)), ('retries', b.i(0)), ('checksum', b.i(0)), ('active', b.b(False)))))
            statements.insert(-1, b.discard(b.call('done', b.borrow('state'))))
        elif variant == "overflow-after-earlier-write":
            commit = functions[1]
            body = list(commit.body.data['statements'])
            assignment = body[1]
            body[1] = replace(assignment, data={**assignment.data, 'value': b.op(b.i(MAX_I32), '+', b.i(1))})
            functions[1] = replace(commit, body=replace(commit.body, data={'statements': tuple(body)}))
        functions[-1] = replace(main, body=replace(main.body, data={'statements': tuple(statements)}))
        yield Case('batch-' + variant, '08-integrated-pilot-variants', replace(program, functions=tuple(functions)))


def historical_standalone_cases():
    path = Path(__file__).resolve().parents[1] / "tests/fixtures/owned_source/historical-standalone-blocks.json"
    for row in json.loads(path.read_text()):
        if hashlib.sha256(row["source"].encode()).hexdigest() != row["source_sha256"]:
            raise ValueError("historical source original changed")
        yield Case("excluded-standalone-" + row["original_id"], "03-excluded-syntax", source=row["source"],
                   excluded=("parse", "E0100", "{"), exact_span=tuple(row["unexpected_brace"]),
                   note="Original unsupported standalone block preserved; semantic scope coverage uses if true")


def corpus():
    yield from historical_standalone_cases()
    yield from semantic_cases()
    yield from alias_cases()
    yield from excluded_cases()
    yield from extended_cases()
    yield from batch_variants()


@dataclass(frozen=True)
class Denial:
    failure: str
    operation: str
    role: str
    subject: str
    state: str | None = None
    counterpart: str | None = None
    granted: str | None = None
    requested: str | None = None
    mutable: bool | None = None
    permission_validated: bool = False


def classify_denial(denial):
    """Independent reviewed §6.2 matrix; cannot construct a verifier witness."""
    owner = denial.subject in ("local", "parameter")
    reference = denial.subject == "reference-parameter" and denial.granted in ("shared", "exclusive")
    normalized_move = denial.operation == "MoveInitialize" and denial.role == "source" and owner and denial.counterpart == "temporary"
    if denial.failure == "Resource": return "E0400"
    if denial.failure == "Unavailable" and owner and denial.state == "moved":
        direct = (denial.operation in ("ReadField", "WriteField") and denial.role == "base") or (denial.operation == "PrepareBorrow" and denial.role == "authority")
        if normalized_move or direct: return "E0310"
    if denial.failure == "LoanConflict":
        if normalized_move: return "E0311"
        if denial.permission_validated:
            if denial.operation in ("ReadField", "WriteField") and denial.role == "base" and (owner or reference): return "E0311"
            if denial.operation == "Replace" and denial.role == "destination" and denial.subject == "local" and denial.mutable: return "E0311"
            if denial.operation == "PrepareBorrow" and denial.role == "authority" and (owner or reference) and denial.requested in ("shared", "exclusive"): return "E0311"
    if denial.failure == "Permission" and reference and denial.granted == "shared":
        if denial.operation == "WriteField" and denial.role == "base": return "E0313"
        if denial.operation == "PrepareBorrow" and denial.role == "authority" and denial.requested == "exclusive": return "E0313"
    return "E0500"


def diagnostic_payload(message, labels=(), notes=()):
    """Bounded retained diagnostic text only; origins/path/rendering are separate."""
    retained = {"message": bounded_text(message, 1024), "labels": [bounded_text(label, 256) for label in labels[:2]],
                "notes": [bounded_text(note, 256) for note in notes[:2]]}
    retained["bytes"] = len(retained["message"].encode()) + sum(len(value.encode()) for values in (retained["labels"], retained["notes"]) for value in values)
    if retained["bytes"] > 2048: raise AssertionError("diagnostic retained-text bound violated")
    return retained


def missing_field_message(names):
    count, displayed = 0, []
    for name in names:
        if count < 8: displayed.append(bounded_text(name))
        count += 1
    result = "missing fields: " + ", ".join(displayed)
    if count > 8: result += f"; {count - 8} more omitted"
    return diagnostic_payload(result)


def amplification_source(invalid_literals=100):
    if not 0 <= invalid_literals <= 101: raise ValueError("bounded diagnostic accumulation fixture")
    fields = [f"f{index:04d}" + "x" * 895 for index in range(1024)]
    source = "struct Wide { " + ", ".join(name + ": i32" for name in fields) + " }\n"
    source += "fn main() -> i32 {\n"
    source += "".join(f"    let v{index} = Wide {{}};\n" for index in range(invalid_literals))
    source += "    return 0;\n}\n"
    return source, fields
