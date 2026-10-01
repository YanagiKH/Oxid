#!/usr/bin/env python3
"""Independent tagged scalar-comparison / checked-bigint / real LLVM O0 gate.

Usage: python3 scripts/verify_scalar_comparisons.py target/debug/oxid target/release/oxid
Requires Linux x86_64 and mandatory LLVM/Clang/LLD 19.1.7, as native-preview.
Expected answers come only from this source-tree model, never compiler internals.
Every valid case is checked, run in human/JSON modes, compiled by every profile,
and executed without its source, Oxid, LLVM or any tools on PATH.
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

from verify_i32_arithmetic import MIN, MAX
from verify_native_arithmetic import verify_adapter

COMPARATORS = ("==", "!=", "<", "<=", ">", ">=")
BOUNDARIES = (MIN, MIN + 1, -46341, -46340, -2, -1, 0, 1, 2, 46340, 46341, MAX - 1, MAX)


@dataclass(frozen=True)
class Scalar:
    tag: str
    value: object

    def __post_init__(self):
        # isinstance(True, int) is true in Python: exact host types are vital.
        assert self.tag in ("i32", "bool", "unit")
        assert ((self.tag == "i32" and type(self.value) is int and MIN <= self.value <= MAX)
                or (self.tag == "bool" and type(self.value) is bool)
                or (self.tag == "unit" and self.value is None)), self

    def text(self):
        return "()" if self.tag == "unit" else str(self.value).lower()

    def record(self):
        return {"type": "unit"} if self.tag == "unit" else {"type": self.tag, "value": self.value}


@dataclass(frozen=True)
class Expr:
    text: str
    result: Scalar | None
    error: int | None = None  # Unicode-scalar index of the first overflowing operator.


def leaf(tag, value, call=False):
    result = Scalar(tag, value)
    text = result.text()
    return Expr(f"id_{tag}({text})" if call else text, result)


def binary(left, op, right):
    """Evaluate children left-first, range-check arithmetic, preserve tagged bools."""
    text = f"({left.text} {op} {right.text})"
    if left.error is not None:
        return Expr(text, None, 1 + left.error)
    if right.error is not None:
        return Expr(text, None, len(left.text) + len(op) + 3 + right.error)
    lv, rv = left.result, right.result
    assert lv is not None and rv is not None
    if op in ("+", "-", "*"):
        assert lv.tag == rv.tag == "i32", (lv, op, rv)
        value = {"+": lambda: lv.value + rv.value,
                 "-": lambda: lv.value - rv.value,
                 "*": lambda: lv.value * rv.value}[op]()
        if not MIN <= value <= MAX:
            return Expr(text, None, len(left.text) + 2)
        return Expr(text, Scalar("i32", value))
    assert op in COMPARATORS
    assert lv.tag == rv.tag and lv.tag in ("i32", "bool"), (lv, op, rv)
    assert op in ("==", "!=") or lv.tag == "i32", (lv, op, rv)
    # These are direct mathematical comparisons, never subtract-to-compare,
    # unsigned comparisons, float conversion or host bool/int coercion.
    value = {"==": lambda: lv.value == rv.value, "!=": lambda: lv.value != rv.value,
             "<": lambda: lv.value < rv.value, "<=": lambda: lv.value <= rv.value,
             ">": lambda: lv.value > rv.value, ">=": lambda: lv.value >= rv.value}[op]()
    return Expr(text, Scalar("bool", value))


def self_check_model():
    for invalid in [("i32", True), ("bool", 1), ("unit", 0)]:
        try:
            Scalar(*invalid)
        except AssertionError:
            pass
        else:
            raise AssertionError(("oracle silently coerced a scalar", invalid))
    for left, op, right in [
        (leaf("i32", 1), "==", leaf("bool", True)),
        (leaf("bool", False), "!=", leaf("i32", 0)),
        (leaf("bool", False), "<", leaf("bool", True)),
        (leaf("unit", None), "==", leaf("unit", None)),
    ]:
        try:
            binary(left, op, right)
        except AssertionError:
            pass
        else:
            raise AssertionError(("oracle accepted an invalid operand table", left, op, right))
    assert binary(leaf("i32", MIN), "<", leaf("i32", MAX)).result == Scalar("bool", True)
    overflow = binary(leaf("i32", MAX), "+", leaf("i32", 1))
    first = binary(overflow, "!=", binary(leaf("i32", MIN), "-", leaf("i32", 1)))
    assert first.text[first.error] == "+"
    second = binary(leaf("i32", 0), "<=", overflow)
    assert second.text[second.error] == "+"


def expressions():
    for left in BOUNDARIES:
        for right in BOUNDARIES:
            for op in COMPARATORS:
                yield "boundary", binary(leaf("i32", left), op, leaf("i32", right))
    for left in (False, True):
        for right in (False, True):
            for op in ("==", "!="):
                yield "bool_table", binary(leaf("bool", left), op, leaf("bool", right))
    rng = random.Random(0xC04A132)

    def arithmetic(depth):
        if depth == 0 or rng.randrange(4) == 0:
            value = rng.choice(BOUNDARIES + (rng.randint(-1000, 1000),))
            return leaf("i32", value, bool(rng.randrange(2)))
        return binary(arithmetic(depth - 1), rng.choice(("+", "-", "*")), arithmetic(depth - 1))

    def boolean(depth):
        # Even a full depth-three bool tree stays within native's 256-slot cap.
        if depth == 0 or rng.randrange(3) == 0:
            if rng.randrange(4) == 0:
                return leaf("bool", bool(rng.randrange(2)), True)
            return binary(arithmetic(2), rng.choice(COMPARATORS), arithmetic(2))
        return binary(boolean(depth - 1), rng.choice(("==", "!=")), boolean(depth - 1))

    for _ in range(200):
        yield "generated_arithmetic", binary(arithmetic(4), rng.choice(COMPARATORS), arithmetic(4))
    for _ in range(300):
        yield "generated_nested", boolean(3)
    for text, value in [
        ("1+2*3==7", True), ("1+2<4*2", True), ("20-4-3>=13", True),
        ("1--2<=3", True), ("-2147483648<-1+0", True),
        ("(1<2)==true", True), ("false==(2<1)", True),
        ("(1==1)!=(2>=2)", False), ("((1<2)==true)!=false", True),
        ("1/* 雪 */<=/* 🦀 */2", True), ("1!=// é\r\n2", True),
    ]:
        yield "grammar", Expr(text, Scalar("bool", value))


HELPERS = "fn id_i32(x: i32) -> i32 { return x; } fn id_bool(x: bool) -> bool { return x; } "


def cases():
    prefix = "// 雪\r\n" + HELPERS + "fn main() -> bool { return "
    for category, expr in expressions():
        yield category, prefix + expr.text + "; }", expr.result, None if expr.error is None else len(prefix) + expr.error
    # Branch outcomes are independently selected from comparison truth values.
    for left, right in [(MIN, MAX), (-1, 0), (0, 0), (MAX, MIN)]:
        for op in COMPARATORS:
            expr = binary(leaf("i32", left, True), op, leaf("i32", right, True))
            chosen = expr.result.value
            for value in (MIN, MAX):
                arms = [f"return id_i32({value});", "return 2147483647 + 1;"]
                if not chosen:
                    arms.reverse()
                source = HELPERS + f"fn main() -> i32 {{ if {expr.text} {{ {arms[0]} }} else {{ {arms[1]} }} }}"
                yield "branch", source, Scalar("i32", value), None
    for source, value in [
        ("fn main() -> bool { return later(1 < 2); } fn later(x: bool) -> bool { return x != false; }", Scalar("bool", True)),
        ("fn main() -> bool { let inferred = -1 < 0; let explicit: bool = inferred; return explicit == false; }", Scalar("bool", False)),
        ("fn main() -> () { 1 < 2; false != true; let ignored = 2 <= 1; return; }", Scalar("unit", None)),
        ("fn unused() -> bool { return 2147483647 + 1 == 0; } fn main() -> bool { return 0 != 0; }", Scalar("bool", False)),
        ("fn main() -> bool { if 2 < 1 { return 2147483647 + 1 == 0; } return 1 == 1; }", Scalar("bool", True)),
    ]:
        yield "context", source, value, None
    helpers = "// 🦀\r\nfn later() -> i32 { return -2147483648 - 1; }\r\nfn first() -> i32 { return 2147483647 + 1; }\r\nfn take(a: bool, b: bool) -> bool { return b; }\r\n"
    for body, marker in [
        ("return first() < later();", "+ 1"),
        ("return later() >= first();", "- 1"),
        ("return 0 == first();", "+ 1"),
        ("return first() != 0;", "+ 1"),
        ("return (first() < 1) == (later() > 2);", "+ 1"),
        ("return take(first() < 0, later() > 0);", "+ 1"),
        ("return take(later() < 0, first() > 0);", "- 1"),
        ("first() < 1; return later() == 0;", "+ 1"),
        ("let ignored = later() != 0; return first() < 1;", "- 1"),
        ("if first() < 0 { return true; } return false;", "+ 1"),
        ("return 0 == (0 * first());", "+ 1"),
        ("return (first() * 0) == 0;", "+ 1"),
    ]:
        source = helpers + f"fn main() -> bool {{ {body} }}"
        yield "first_error", source, None, source.index(marker)
    for result in [Scalar("bool", False), Scalar("unit", None), Scalar("i32", 42)]:
        for statement in ["2147483647 + 1 < 0;", "let discarded = -2147483648 * -1 != 0;"]:
            ty = "()" if result.tag == "unit" else result.tag
            source = f"fn main() -> {ty} {{ {statement} return {result.text()}; }}"
            error = source.index("+ 1") if "+" in statement else source.index("* -1")
            yield "discard_error", source, None, error
    # Height 64 is inclusive, and comparisons/groups/calls all contribute.
    for terms, template in [(63, "{} == 0"), (62, "({} == 0)"), (62, "id_bool({} == 0)"), (61, "({} == 0) == true")]:
        expr = template.format("+".join(["0"] * terms))
        yield "height_boundary", HELPERS + f"fn main() -> bool {{ return {expr}; }}", Scalar("bool", True), None
    # At the numeric token limit, comparison recognition must remain adjacent.
    yield "token_boundary", "fn main() -> bool { return " + "0" * 65536 + "==0; }", Scalar("bool", True), None
    # Exactly MAX_SOURCE_BYTES, with legal short comments (no giant token).
    source = "fn main() -> bool { return 0==0; }"
    gap = 1048576 - len(source)
    source += "/*" + (" " * 59996) + "*/"
    gap -= 60000
    source += ("/*" + " " * 59996 + "*/") * (gap // 60000)
    remainder = gap % 60000
    source += " " * remainder
    assert len(source.encode()) == 1048576
    yield "source_boundary", source, Scalar("bool", True), None


def negative_cases():
    prefix = "// 🦀\r\nfn main() -> bool { return "
    scalar_types = (("i32", "1"), ("bool", "true"), ("unit", "()"))
    for op in COMPARATORS:
        for left_ty, left in scalar_types:
            for right_ty, right in scalar_types:
                equality = op in ("==", "!=")
                if left_ty == right_ty and (left_ty == "i32" or (equality and left_ty == "bool")):
                    continue
                expr = f"{left} {op} {right}"
                invalid_left = left_ty == "unit" or (not equality and left_ty != "i32")
                offset, width = (0, len(left)) if invalid_left else (len(left) + len(op) + 2, len(right))
                for begin, end in [
                    (prefix, "; }"),
                    ("fn main() -> bool { return true; } fn unused() -> bool { return ", "; }"),
                    ("fn main() -> bool { if true { return true; } else { return ", "; } }"),
                ]:
                    yield "invalid_operand", begin + expr + end, "E0300", "type", len(begin) + offset, width
    for first in COMPARATORS:
        for second in COMPARATORS:
            begin = prefix + f"1 {first} 2 /* 雪 */ "
            yield "chain", begin + second + " 3; }", "E0100", "parse", len(begin), len(second)
    for expr, marker in [
        ("1 = = 1", "="), ("1 ! = 2", "!"), ("1 < = 2", "="),
        ("1 > /* 雪 */ = 2", "="), ("1 =/* 雪 */=2", "="),
        ("1 !/* 雪 */=2", "!"), ("!true", "!"),
        ("true && false", "&"), ("true || false", "|"),
        ("1 / 2 == 0", "/"), ("1 as bool", "as"),
    ]:
        yield "token", prefix + expr + "; }", "E0101", "parse", len(prefix) + expr.index(marker), len(marker)
    for expr, marker in [("(1 < 2) + 3", "(1 < 2)"), ("(1 < 2) < 3", "(1 < 2)"), ("1 < (2 < 3)", "(2 < 3)")]:
        yield "invalid_context", prefix + expr + "; }", "E0300", "type", len(prefix) + expr.index(marker), len(marker)
    for expr in ["1 <", "true ==", "1 <=", "1 < < 2"]:
        marker = expr.index("<", expr.index("<") + 1) if expr == "1 < < 2" else len(expr)
        # The absent right operand is reported on the following semicolon.
        suffix = "; }"
        yield "incomplete", prefix + expr + suffix, "E0100", "parse", len(prefix) + marker, 1
    for terms, template in [(64, "{} == 0"), (63, "({} == 0)"), (63, "id_bool({} == 0)"), (62, "({} == 0) == true")]:
        expr = template.format("+".join(["0"] * terms))
        source = HELPERS + f"fn main() -> bool {{ return {expr}; }}"
        yield "height_limit", source, "E0400", "parse", None, None
    yield "token_limit", prefix + "0" * 65537 + "==0; }", "E0400", "lex", len(prefix), 65537
    source = "//" + " " * 1048575
    yield "source_limit", source, "E0400", "source", None, None


def native_resource_cases():
    """Exact existing admission caps, with comparisons present in each source."""
    for count in (256, 257):
        source = "fn main() -> bool { return 0<1; }" + "".join(f"fn f{i}() -> bool {{ return 0<1; }}" for i in range(1, count))
        yield "functions", source, count == 256
    for count in (64, 65):
        params = ",".join(f"p{i}: bool" for i in range(count))
        source = f"fn unused({params}) -> bool {{ return p0==true; }} fn main() -> bool {{ return 0<1; }}"
        yield "parameters", source, count == 64
    for count in (85, 86):
        # Three slots per comparison, plus one unit slot for bare return.
        source = "fn main() -> () {" + "0<1;" * count + "return;}"
        yield "function_slots", source, count == 85
    for count in (32, 33):
        source = "".join(f"fn {'main' if i == 0 else 'f' + str(i)}() -> () {{" + "0<1;" * 85 + "return;}" for i in range(count))
        yield "aggregate_slots", source, count == 32
    for extra in (False, True):
        source = "".join(f"fn {'main' if i == 0 else 'f' + str(i)}() -> () {{" + "if 0<1 {} else {}" * 85 + "return;}" for i in range(16))
        if extra:
            source += "fn extra() -> bool { return 0<1; }"
        yield "aggregate_blocks", source, not extra
    for depth in (32, 33):
        source = "fn main() -> bool { return f0(); }"
        for i in range(depth - 1):
            result = "0<1" if i == depth - 2 else f"f{i + 1}()"
            source += f"fn f{i}() -> bool {{ return {result}; }}"
        yield "call_depth", source, depth == 32
    # Base comparison cost=7, doubler cost=5+2*child. f13 costs 98,299.
    # Main adds 3, 95*17, 6*11 and 8*2; root allocation adds 1 = 100,000.
    source = "fn f0() -> bool { return 0<1; }"
    for i in range(1, 14):
        source += f"fn f{i}() -> bool {{ f{i - 1}(); return f{i - 1}(); }}"
    source += "fn p17() -> bool { 0<1; 0<1; return true; } fn p11() -> bool { 0<1; return true; } fn cheap() -> bool { return true; }"
    for over in (False, True):
        filler = "true;" * (6 if over else 8) + ("cheap();" if over else "")
        main = "fn main() -> bool {" + "p17();" * 95 + "p11();" * 6 + filler + "return f13();}"
        yield "fuel", source + main, not over


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
    count = {"binaries": len(binaries), "source_cases": 0, "success_cases": 0, "overflow_cases": 0,
             "compiled_artifacts": 0, "negative_cases": 0, "native_resource_cases": 0, "invocations": 0}
    categories = {}
    with tempfile.TemporaryDirectory(prefix="oxid-scalar-comparison-") as directory:
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

        for number, (category, source, value, error) in enumerate(cases()):
            path.write_bytes(source.encode())
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
            count["success_cases" if error is None else "overflow_cases"] += 1
            categories[category] = categories.get(category, 0) + 1
        assert tested_io == {"true", "false", "error"}
        for category, source, code, stage, offset, width in negative_cases():
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
            count["native_resource_cases"] += 1
        verify_adapter(root)
    assert categories["boundary"] == 13 * 13 * 6 and categories["bool_table"] == 8
    print(json.dumps(dict(count, categories=categories), sort_keys=True))
    print("scalar comparisons O0: tagged Python/reference/native parity, complete scalar tables, strict grammar/types, exact resource boundaries, first-error origins, profile-identical standalone ELF and output failures: PASS")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    verify(sys.argv[1:])
