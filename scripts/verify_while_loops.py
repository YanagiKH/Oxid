#!/usr/bin/env python3
"""Independent source-state/fuel oracle and mandatory standalone LLVM while gate.

Usage: verify_while_loops.py target/debug/oxid target/release/oxid
Reuses only the earlier independent scalar AST/rendering/scope primitives, not
compiler IR or compiler-derived expected values. Fuel is a source execution
schedule derived from the published abstract-operation rules. LLVM O0 is real;
missing tools fail. A subprocess timeout is merely a harness backstop.
"""
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile
import verify_mutable_locals as m

BUDGET = 1_000_000


class While(m.Stmt):
    def prefix(self):
        return "while "

    def text(self):
        return self.prefix() + self.expression.text() + " " + m.block_text(self.arms[0])


def loop(condition, body):
    return While("while", condition, arms=(tuple(body),))


def expression_slots(expr):
    # Parenthesized binary expressions introduce a separately charged group copy.
    return 1 + int(expr.kind == "binary") + sum(map(expression_slots, expr.children))


def body_slots(body):
    return sum(expression_slots(s.expression) + int(s.kind == "let")
               + sum(body_slots(arm) for arm in s.arms) for s in body)


def checked_block(body, parent, functions, result):
    scope, returned = m.Scope(functions, parent), False
    for statement in body:
        assert not returned
        tag = m.expression_type(statement.expression, scope, functions)
        if statement.kind == "let":
            assert not statement.annotation or statement.annotation == tag
            scope.declare(statement.name, m.Cell(tag, statement.mutable))
        elif statement.kind == "store":
            cell = scope.find(statement.name)
            assert cell.mutable and cell.tag == tag
        elif statement.kind == "return":
            assert tag == result
            returned = True
        elif statement.kind in ("if", "while"):
            assert tag == "bool"
            outcomes = [checked_block(arm, scope, functions, result) for arm in statement.arms]
            returned = statement.kind == "if" and len(outcomes) == 2 and all(outcomes)
        else:
            assert statement.kind == "discard"
    return returned


class Failure(Exception):
    def __init__(self, code, start, end):
        self.code, self.start, self.end = code, start, end


class Machine:
    def __init__(self, functions, origins, budget=BUDGET):
        self.functions, self.origins, self.remaining = functions, origins, budget
        self.slots = {name: len(fn.parameters) + body_slots(fn.body) for name, (fn, _) in functions.items()}
        self.operations = self.stores = self.calls = self.iterations = 0
        self.trace = hashlib.sha256()

    def charge(self, cost, start, end):
        if self.remaining < cost:
            raise Failure("E0601", start, end)
        self.remaining -= cost
        self.operations += cost

    def invoke(self, name, args, root=False):
        fn, offset = self.functions[name]
        if root:
            origin = self.origins[name]
            self.charge(1 + self.slots[name], origin, origin + len(name))
        scope = m.Scope(self.functions)
        for (parameter, tag), value in zip(fn.parameters, args):
            assert tag == value.tag
            scope.declare(parameter, m.Cell(tag, False, value))
        self.calls += 1
        try:
            self.block(fn.body, offset, scope)
        except m.Returned as result:
            assert result.value.tag == fn.result
            return result.value
        raise AssertionError("fell through")

    def expr(self, e, start, scope):
        end = start + len(e.text())
        if e.kind in ("constant", "name"):
            self.charge(1, start, end)
            return e.atom if e.kind == "constant" else scope.find(e.atom).value
        if e.kind == "call":
            pos, args = start + len(e.atom) + 1, []
            for child in e.children:
                args.append(self.expr(child, pos, scope))
                pos += len(child.text()) + 2
            self.charge(1 + len(args) + self.slots[e.atom], start, end)
            return self.invoke(e.atom, args)
        if e.kind == "not":
            value = self.expr(e.children[0], start + 1, scope)
            self.charge(1, start, end)
            assert value.tag == "bool"
            return m.Scalar("bool", not value.value)
        left, right = e.children
        lv = self.expr(left, start + 1, scope)
        operator = e.atom
        right_start = start + len(left.text()) + len(operator) + 3
        if operator in ("&&", "||"):
            self.charge(1, start + 1, end - 1)  # condition branch
            skip = (operator == "&&" and not lv.value) or (operator == "||" and lv.value)
            if skip:
                value = lv
            else:
                value = self.expr(right, right_start, scope)
                self.charge(1, start + 1, end - 1)  # RHS join edge
            self.charge(1, start + 1, end - 1)  # value selection
        else:
            rv = self.expr(right, right_start, scope)
            self.charge(1, start + 1, end - 1)
            if operator in ("+", "-", "*"):
                assert lv.tag == rv.tag == "i32"
                n = {"+": lambda: lv.value + rv.value, "-": lambda: lv.value - rv.value,
                     "*": lambda: lv.value * rv.value}[operator]()
                if not m.MIN <= n <= m.MAX:
                    op_start = start + len(left.text()) + 2
                    raise Failure("E0604", op_start, op_start + 1)
                value = m.Scalar("i32", n)
            else:
                assert lv.tag == rv.tag
                value = m.Scalar("bool", {"==": lambda: lv.value == rv.value, "!=": lambda: lv.value != rv.value,
                    "<": lambda: lv.value < rv.value, "<=": lambda: lv.value <= rv.value,
                    ">": lambda: lv.value > rv.value, ">=": lambda: lv.value >= rv.value}[operator]())
        self.charge(1, start, end)  # explicit source parentheses copy
        return value

    def block(self, body, offset, parent):
        scope, pos = m.Scope(self.functions, parent), offset + 2
        for statement in body:
            end = pos + len(statement.text())
            expression_start = pos + len(statement.prefix())
            if statement.kind == "while":
                self.charge(1, pos, end)  # preheader edge, once per encounter
                body_start = expression_start + len(statement.expression.text()) + 1
                body_end = body_start + len(m.block_text(statement.arms[0])) - 1
                while True:
                    condition = self.expr(statement.expression, expression_start, scope)
                    self.charge(1, pos, end)
                    if not condition.value:
                        break
                    self.iterations += 1
                    self.block(statement.arms[0], body_start, scope)
                    self.charge(1, body_end, body_end + 1)
            else:
                value = self.expr(statement.expression, expression_start, scope)
                if statement.kind == "let":
                    self.charge(1, pos, end)
                    scope.declare(statement.name, m.Cell(value.tag, statement.mutable, value))
                elif statement.kind == "store":
                    self.charge(1, pos, end)
                    cell = scope.find(statement.name)
                    assert cell.mutable and cell.tag == value.tag
                    cell.value = value
                    self.stores += 1
                    self.trace.update(repr((statement.name, value)).encode())
                elif statement.kind == "return":
                    self.charge(1, pos, end)
                    raise m.Returned(value)
                elif statement.kind == "if":
                    self.charge(1, pos, end)
                    selected = 0 if value.value else 1
                    if selected < len(statement.arms):
                        arm_start = expression_start + len(statement.expression.text()) + 1
                        if selected:
                            arm_start += len(m.block_text(statement.arms[0])) + 6
                        arm = statement.arms[selected]
                        self.block(arm, arm_start, scope)
                        close = arm_start + len(m.block_text(arm)) - 1
                        self.charge(1, close, close + 1)
            pos = end + 1


def case(category, body, result="i32", helpers=(), budget=BUDGET):
    source, functions, origins = "// 雪🦀\r\n", {}, {}
    for fn in (*helpers, m.Function("main", (), result, tuple(body))):
        origins[fn.name] = len(source) + 3
        functions[fn.name] = fn, len(source) + len(fn.prefix())
        source += fn.prefix() + m.block_text(fn.body) + "\r\n"
    for fn, _ in functions.values():
        scope = m.Scope(functions)
        for name, tag in fn.parameters:
            scope.declare(name, m.Cell(tag, False))
        assert checked_block(fn.body, scope, functions, fn.result)
    model = Machine(functions, origins, budget)
    try:
        value, failure = model.invoke("main", [], root=True), None
    except Failure as error:
        value, failure = None, error
    return category, source, value, failure, model


def cases():
    i, b, v, op = m.i, m.b, m.v, m.op
    for count in (0, 1, 2, 7, 31):
        yield case("counter", [m.let("n", i(0), True), loop(op(v("n"), "<", i(count)), [m.store("n", op(v("n"), "+", i(1)))]), m.ret(v("n"))])
    for tag, expr in (("bool", b(True)), ("i32", i(m.MIN)), ("unit", m.u())):
        yield case("typed_reset", [m.let("n", i(0), True), m.let("out", expr, True), loop(op(v("n"), "<", i(3)), [m.let("fresh", expr, True), m.store("fresh", expr), m.store("out", v("fresh")), m.store("n", op(v("n"), "+", i(1)))]), m.ret(v("out"))], tag)
    helper = m.Function("bump", (("p", "i32"),), "i32", (m.let("x", v("p"), True), m.store("x", op(v("x"), "+", i(1))), m.ret(v("x"))))
    check = m.Function("check", (("p", "i32"),), "bool", (m.ret(op(v("p"), "<", i(5))),))
    rng = random.Random(815763)
    for number in range(120):
        bound, initial, increment = rng.randrange(0, 9), rng.randrange(-20, 21), rng.randrange(-7, 8)
        if number % 6 == 0:
            initial = rng.choice((m.MIN, m.MIN+1, m.MAX-1, m.MAX))
        update = m.store("x", op(v("x"), rng.choice(("+", "-", "*")), i(increment)))
        arm = [m.let("old", v("x")), m.let("fresh", i(rng.randrange(-4, 5)), True), m.store("fresh", op(v("fresh"), "+", v("n"))), m.branch(op(v("n"), "<", i(rng.randrange(0, 5))), [update], [m.store("x", op(v("x"), "+", v("fresh")))]), m.store("n", m.call("bump", v("n")))]
        if number % 3 == 0:
            arm.insert(2, loop(op(v("fresh"), "<", i(2)), [m.store("fresh", op(v("fresh"), "+", i(1)))]))
        condition = op(v("n"), "<", i(bound))
        if number % 2:
            condition = op(condition, "&&", m.call("check", v("n")))
        yield case("seeded_nested_state", [m.let("n", i(0), True), m.let("x", i(initial), True), loop(condition, arm), m.ret(v("x"))], helpers=(helper, check))
    for flag in (False, True):
        bad = op(op(i(m.MAX), "+", i(1)), "==", i(0))
        for operator in ("&&", "||"):
            yield case("lazy_condition_error", [loop(op(b(flag), operator, bad), [m.ret(i(7))]), m.ret(i(2))])
    yield case("return_in_body", [loop(b(True), [m.ret(i(9))]), m.ret(i(2))])
    infinite = m.Function("never", (), "unit", (loop(b(True), []), m.ret(m.u())))
    yield case("unused_cycle", [m.ret(i(42))], helpers=(infinite,))
    yield case("empty_infinite", [loop(b(True), []), m.ret(m.u())], "unit")
    yield case("callee_exhaustion", [m.discard(m.call("never")), m.ret(m.u())], "unit", (infinite,))
    yield case("nested_exhaustion", [m.let("n", i(0), True), loop(b(True), [m.let("j", i(0), True), loop(op(v("j"), "<", i(3)), [m.store("j", op(v("j"), "+", i(1)))]), m.store("n", op(v("n"), "+", i(1)))]), m.ret(v("n"))])
    for count in (70_000, 90_907, 90_908, 110_000):
        yield case("default_budget_counter", [m.let("n", i(0), True), loop(op(v("n"), "<", i(count)), [m.store("n", op(v("n"), "+", i(1)))]), m.ret(v("n"))])
    for last in (False, True):
        # Base22 + 90906*11 = 999988. Four false if statements cost3
        # apiece including their slots; a taken empty arm adds one Goto.
        yield case("default_budget_exact" if not last else "default_budget_plus_one", [m.let("n", i(0), True), *[m.branch(b(False), []) for _ in range(3)], m.branch(b(last), []), loop(op(v("n"), "<", i(90_906)), [m.store("n", op(v("n"), "+", i(1)))]), m.ret(v("n"))])
    yield case("shared_calls_budget", [m.let("n", i(0), True), loop(op(v("n"), "<", i(100_000)), [m.store("n", m.call("bump", v("n")))]), m.ret(v("n"))], helpers=(helper,))


def native_resources():
    # Every function has a real cyclic CFG despite its false initial condition.
    for extra in (False, True):
        functions = []
        for index in range(256):
            name = "main" if index == 255 else f"f{index}"
            call = "f1(); " if extra and index == 0 else ""
            functions.append(f"fn {name}()->() {{ {call}" + "while false {} " * 5 + "return (); }")
        yield "blocks_4097" if extra else "blocks_4096", "\n".join(functions), not extra, b"()\n"
    for extra in (False, True):
        functions = [f"fn {'main' if index == 31 else f'f{index}'}()->() {{ " + "(); " * 254 + "while false {} return (); }" for index in range(32)]
        if extra:
            functions.append("fn extra()->() { return (); }")
        yield "slots_8193" if extra else "slots_8192", "\n".join(functions), not extra, b"()\n"
    for depth in (32, 33):
        functions = ["fn f0()->() { while false {} return (); }"]
        for index in range(1, depth):
            name = "main" if index == depth - 1 else f"f{index}"
            functions.append(f"fn {name}()->() {{ f{index-1}(); return (); }}")
        yield f"depth_{depth}", "\n".join(functions), depth == 32, b"()\n"
    params = ",".join(f"p{i}:bool" for i in range(64))
    args = ",".join(f"p{i}" for i in range(64))
    functions = ["fn unused()->() { while false {} return (); }", f"fn f0({params})->bool {{ return p0; }}"]
    functions.extend(f"fn f{i}({params})->bool {{ return f{i-1}({args}); }}" for i in range(1, 31))
    functions.append("fn main()->bool { return f30(" + ",".join(["true"]*64) + "); }")
    yield "guarded_32_frames_64_args", "\n".join(functions), True, b"true\n"


def self_check():
    # Grouped version of the hand-counted46-fuel source adds two frame slots,
    # four condition copies and three arithmetic copies:55 total.
    item = case("self", [m.let("n", m.i(0), True), loop(m.op(m.v("n"), "<", m.i(3)), [m.store("n", m.op(m.v("n"), "+", m.i(1)))]), m.ret(m.v("n"))])
    assert item[2] == m.Scalar("i32", 3) and item[4].operations == 55
    for budget in range(55):
        failed = case("self", [m.let("n", m.i(0), True), loop(m.op(m.v("n"), "<", m.i(3)), [m.store("n", m.op(m.v("n"), "+", m.i(1)))]), m.ret(m.v("n"))], budget=budget)
        assert failed[2] is None and failed[3].code == "E0601"
    assert expression_slots(m.op(m.b(False), "&&", m.b(True))) == 4


def location(source, offset):
    before = source[:offset]
    return before.count("\n") + 1, len(before.rsplit("\n", 1)[-1]) + 1


def verify(binaries):
    self_check()
    binaries = [str(Path(binary).resolve()) for binary in binaries]
    hashes = [hashlib.sha256(Path(binary).read_bytes()).hexdigest() for binary in binaries]
    counts = {key: 0 for key in ("source_cases", "success_cases", "overflow_cases", "fuel_cases", "compiled_artifacts", "negative_cases", "invocations", "modeled_operations", "modeled_stores", "modeled_iterations", "native_resource_cases", "compiled_resource_artifacts")}
    corpus, artifacts, traces = hashlib.sha256(), hashlib.sha256(), hashlib.sha256()
    categories, io = {}, set()
    with tempfile.TemporaryDirectory(prefix="oxid-while-oracle-") as directory:
        root = Path(directory)
        source_path = root / "while.ox"
        env = dict(os.environ, OXID_CACHE_DIR=str(root / "cache"))
        env.pop("OXID_PATH", None)
        clean = {"PATH": "/no/compiler-or-runtime", "LC_ALL": "C"}

        def command(args, **kwargs):
            return subprocess.run(args, cwd=root, env=env, capture_output=True, timeout=30, **kwargs)

        def invoke(binary, operation, json_output=True, output=None, missing=False):
            args = [binary, operation, source_path.name, "--edition=typed-preview"]
            if json_output:
                args.append("--message-format=json")
            if operation == "compile":
                args += ["--backend=llvm", "--output", str(output)]
            counts["invocations"] += 1
            if missing:
                return subprocess.run(args, cwd=root, env=dict(env, OXID_LLVM_BIN=str(root / "missing-tools")), capture_output=True, timeout=30)
            return command(args)

        for number, (category, source, value, failure, model) in enumerate(cases()):
            source_path.write_bytes(source.encode())
            corpus.update(source.encode() + b"\0")
            traces.update(model.trace.digest())
            counts["modeled_operations"] += model.operations
            counts["modeled_stores"] += model.stores
            counts["modeled_iterations"] += model.iterations
            previous = None
            if failure:
                line, column = location(source, failure.start)
                message = "execution fuel exhausted" if failure.code == "E0601" else "checked i32 arithmetic overflow"
                expected = (1, b"", f"error[{failure.code}] (oir-run): {message}\n  --> while.ox:{line}:{column}\n".encode())
            else:
                expected = (0, (value.text() + "\n").encode(), b"")
            for profile, binary in enumerate(binaries):
                checked = invoke(binary, "check")
                assert checked.returncode == 0 and not checked.stderr, (category, checked)
                observed = invoke(binary, "run")
                records = [json.loads(line) for line in observed.stdout.splitlines()]
                assert observed.returncode == expected[0] and not observed.stderr, (category, observed)
                if failure:
                    diagnostic = records[0]
                    assert diagnostic["code"] == failure.code and diagnostic["stage"] == "oir-run", (category, records)
                    primary = diagnostic["primary"]
                    assert (primary["start"], primary["end"]) == (len(source[:failure.start].encode()), len(source[:failure.end].encode())), (category, source, failure.__dict__, diagnostic)
                else:
                    assert records[-1]["result"] == value.record(), (category, source, records, value)
                human = invoke(binary, "run", False)
                assert (human.returncode, human.stdout, human.stderr) == expected, (category, source, human, expected)
                output = root / f"native-{number}-{profile}"
                compiled = invoke(binary, "compile", output=output)
                assert compiled.returncode == 0 and not compiled.stderr and output.is_file(), (category, source, compiled)
                hidden = source_path.with_suffix(".hidden")
                source_path.rename(hidden)
                try:
                    executed = subprocess.run([str(output)], cwd=root, env=clean, capture_output=True, timeout=10)
                finally:
                    hidden.rename(source_path)
                assert (executed.returncode, executed.stdout, executed.stderr) == expected, (category, source, executed, expected)
                artifact = output.read_bytes()
                assert artifact[:4] == b"\x7fELF"
                digest = hashlib.sha256(artifact).digest()
                if previous is not None:
                    assert previous == digest, (category, "profile artifact mismatch")
                previous = digest
                artifacts.update(digest)
                kind = failure.code if failure else value.tag
                if kind not in io:
                    dynamic = command(["readelf", "-d", str(output)])
                    needed = [line for line in dynamic.stdout.splitlines() if b"(NEEDED)" in line]
                    assert len(needed) == 1 and b"[libc.so.6]" in needed[0]
                    with open("/dev/full", "wb") as full:
                        result = subprocess.run([str(output)], stdout=subprocess.PIPE if failure else full, stderr=full if failure else subprocess.PIPE, env=clean, timeout=10)
                    assert result.returncode == 74
                    read_fd, write_fd = os.pipe()
                    os.close(read_fd)
                    try:
                        result = subprocess.run([str(output)], stdout=subprocess.PIPE if failure else write_fd, stderr=write_fd if failure else subprocess.PIPE, env=clean, timeout=10)
                    finally:
                        os.close(write_fd)
                    assert result.returncode == 74
                    io.add(kind)
                output.unlink()
                counts["compiled_artifacts"] += 1
            counts["source_cases"] += 1
            counts["fuel_cases" if failure and failure.code == "E0601" else "overflow_cases" if failure else "success_cases"] += 1
            categories[category] = categories.get(category, 0) + 1
        negatives = [
            ("fn main()->() { while 1 {} return; }", "E0300"),
            ("fn main()->() { while () {} return; }", "E0300"),
            ("fn main()->() { while false { absent; } return; }", "E0200"),
            ("fn main()->() { while false { let mut x=1; x=true; } return; }", "E0300"),
            ("fn main()->() { while true { return; } }", "E0302"),
            ("fn main()->() { while false { let x=1; } x; return; }", "E0200"),
            ("fn main()->() { while false { break; } return; }", "E0101"),
            ("fn main()->() { while false { continue; } return; }", "E0101"),
            ("fn main()->() { while false { return; 1; } return; }", "E0303"),
            ("fn main()->() { while false {} else {} return; }", "E0100"),
            ("fn main()->() { let x=while true {}; return; }", "E0100"),
            ("fn main()->() { let mut x=true; while x=false {} return; }", "E0100"),
        ]
        for source, code in negatives:
            source_path.write_text(source)
            corpus.update(source.encode() + b"\0")
            for binary in binaries:
                for operation in ("check", "run", "compile"):
                    output = root / "forbidden"
                    result = invoke(binary, operation, output=output, missing=True)
                    records = [json.loads(line) for line in result.stdout.splitlines()]
                    assert result.returncode == 1 and not result.stderr and records[0]["code"] == code, (source, code, result)
                    assert not output.exists() and not list(root.glob(".oxid-native-*"))
            counts["negative_cases"] += 1
        for category, source, admitted, stdout in native_resources():
            source_path.write_text(source)
            corpus.update(source.encode() + b"\0")
            previous = None
            for binary in binaries:
                checked = invoke(binary, "check")
                assert checked.returncode == 0 and not checked.stderr, (category, checked)
                output = root / "resource-native"
                rejected = invoke(binary, "compile", output=output, missing=True)
                records = [json.loads(line) for line in rejected.stdout.splitlines()]
                assert rejected.returncode == 1 and records[0]["code"] == ("E0701" if admitted else "E0700"), (category, records)
                assert not output.exists() and not list(root.glob(".oxid-native-*"))
                if admitted:
                    reference = invoke(binary, "run", False)
                    assert (reference.returncode, reference.stdout, reference.stderr) == (0, stdout, b""), (category, reference)
                    compiled = invoke(binary, "compile", output=output)
                    assert compiled.returncode == 0 and not compiled.stderr, (category, compiled)
                    hidden=source_path.with_suffix(".hidden")
                    source_path.rename(hidden)
                    try:
                        # Guard context adds one hidden argument; qualify the
                        # old64-user-argument/32-frame shape with guards too.
                        def stack_bound():
                            import resource
                            resource.setrlimit(resource.RLIMIT_STACK, (1024*1024, 1024*1024))
                        result = subprocess.run([str(output)], cwd=root, env=clean, capture_output=True, timeout=10, preexec_fn=stack_bound if category == "guarded_32_frames_64_args" else None)
                    finally:
                        hidden.rename(source_path)
                    assert (result.returncode, result.stdout, result.stderr) == (0, stdout, b""), (category, result)
                    digest=hashlib.sha256(output.read_bytes()).digest()
                    assert previous is None or previous == digest, (category,"resource profile mismatch")
                    previous=digest
                    artifacts.update(digest)
                    output.unlink()
                    counts["compiled_resource_artifacts"] += 1
            counts["native_resource_cases"] += 1
            categories[category] = categories.get(category, 0) + 1
        assert io == {"i32", "bool", "unit", "E0601", "E0604"}, io
        assert set(root.iterdir()) == {source_path}, list(root.iterdir())
    assert hashes == [hashlib.sha256(Path(binary).read_bytes()).hexdigest() for binary in binaries]
    print(json.dumps(dict(counts, categories=categories, compiler_sha256=hashes, corpus_sha256=corpus.hexdigest(), artifact_manifest_sha256=artifacts.hexdigest(), model_trace_sha256=traces.hexdigest()), sort_keys=True))
    print("while loops O0: independent state/fuel model, exact first-error origins, shared calls/loops budget, standalone profile-identical LLVM artifacts and I/O failures: PASS")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    verify(sys.argv[1:])
