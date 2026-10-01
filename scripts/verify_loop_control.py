#!/usr/bin/env python3
"""Independent source-state/transfer/fuel model and real LLVM loop-control gate.

Usage: verify_loop_control.py target/debug/oxid target/release/oxid
Scalar expression scheduling is reused from the earlier independent while model.
This model independently implements source transfer outcomes, target scoping,
static flow, and dynamic loop/arm edges. It never consumes compiler IR or uses
compiler outputs as expected values. Native O0 tools are mandatory.
"""
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile
import verify_while_loops as w

m, BUDGET, Failure, location = w.m, w.BUDGET, w.Failure, w.location
loop, expression_slots = w.loop, w.expression_slots


class Transfer(m.Stmt):
    def prefix(self):
        return self.kind

    def text(self):
        return self.kind + ";"


def transfer(kind):
    assert kind in ("break", "continue")
    return Transfer(kind, None)


def body_slots(body):
    return sum((expression_slots(s.expression) if s.expression is not None else 0)
               + int(s.kind == "let") + sum(body_slots(arm) for arm in s.arms) for s in body)


def checked_block(body, parent, functions, result, depth=0):
    scope, exits = m.Scope(functions, parent), {"next"}
    for s in body:
        assert "next" in exits, ("unreachable", s)
        current = {"next"}
        if s.kind in ("break", "continue"):
            assert depth > 0, ("outside loop", s)
            current = {s.kind}
        else:
            tag = m.expression_type(s.expression, scope, functions)
            if s.kind == "let":
                assert not s.annotation or s.annotation == tag
                scope.declare(s.name, m.Cell(tag, s.mutable))
            elif s.kind == "store":
                cell = scope.find(s.name)
                assert cell.mutable and cell.tag == tag
            elif s.kind == "return":
                assert tag == result
                current = {"return"}
            elif s.kind == "while":
                assert tag == "bool"
                child = checked_block(s.arms[0], scope, functions, result, depth + 1)
                current = {"next"} | (child & {"return"})
            elif s.kind == "if":
                assert tag == "bool"
                current = set().union(*(checked_block(a, scope, functions, result, depth) for a in s.arms))
                if len(s.arms) == 1:
                    current.add("next")
            else:
                assert s.kind == "discard"
        exits = (exits - {"next"}) | current
    return exits


class Jump(Exception):
    def __init__(self, kind):
        self.kind = kind


class Machine(w.Machine):
    def __init__(self, functions, origins, budget=BUDGET):
        self.functions, self.origins, self.remaining = functions, origins, budget
        self.slots = {name: len(fn.parameters) + body_slots(fn.body) for name, (fn, _) in functions.items()}
        self.operations = self.stores = self.calls = self.iterations = 0
        self.breaks = self.continues = 0
        self.trace = hashlib.sha256()

    def block(self, body, offset, parent):
        scope, pos = m.Scope(self.functions, parent), offset + 2
        for s in body:
            end = pos + len(s.text())
            expression_start = pos + len(s.prefix())
            if s.kind in ("break", "continue"):
                self.charge(1, pos, end)
                self.breaks += int(s.kind == "break")
                self.continues += int(s.kind == "continue")
                self.trace.update(repr((s.kind, pos, end)).encode())
                raise Jump(s.kind)
            if s.kind == "while":
                self.charge(1, pos, end)
                body_start = expression_start + len(s.expression.text()) + 1
                body_end = body_start + len(m.block_text(s.arms[0])) - 1
                while True:
                    condition = self.expr(s.expression, expression_start, scope)
                    self.charge(1, pos, end)
                    if not condition.value:
                        break
                    self.iterations += 1
                    try:
                        self.block(s.arms[0], body_start, scope)
                    except Jump as jump:
                        if jump.kind == "break":
                            break
                        assert jump.kind == "continue"
                        continue
                    self.charge(1, body_end, body_end + 1)
            else:
                value = self.expr(s.expression, expression_start, scope)
                if s.kind == "let":
                    self.charge(1, pos, end)
                    scope.declare(s.name, m.Cell(value.tag, s.mutable, value))
                elif s.kind == "store":
                    self.charge(1, pos, end)
                    cell = scope.find(s.name)
                    assert cell.mutable and cell.tag == value.tag
                    cell.value = value
                    self.stores += 1
                    self.trace.update(repr((s.name, value)).encode())
                elif s.kind == "return":
                    self.charge(1, pos, end)
                    raise m.Returned(value)
                elif s.kind == "if":
                    self.charge(1, pos, end)
                    selected = 0 if value.value else 1
                    if selected < len(s.arms):
                        arm_start = expression_start + len(s.expression.text()) + 1
                        if selected:
                            arm_start += len(m.block_text(s.arms[0])) + 6
                        arm = s.arms[selected]
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
        assert checked_block(fn.body, scope, functions, fn.result) == {"return"}
    model = Machine(functions, origins, budget)
    try:
        value, failure = model.invoke("main", [], root=True), None
    except Failure as error:
        value, failure = None, error
    return category, source, value, failure, model


def cases():
    i, b, v, op, br, co = m.i, m.b, m.v, m.op, transfer("break"), transfer("continue")
    for tag, value in (("i32", i(7)), ("bool", b(True)), ("unit", m.u())):
        yield case("break_scalar", [loop(b(True), [br]), m.ret(value)], tag)
        yield case("unused_cycle", [loop(b(True), [br]), m.ret(value)], tag,
                   (m.Function("unused", (), "unit", (loop(b(False), [co]), m.ret(m.u()))),))
    for count in (0, 1, 2, 7, 31):
        yield case("continue_counter", [m.let("n", i(0), True), loop(op(v("n"), "<", i(count)), [m.store("n", op(v("n"), "+", i(1))), co]), m.ret(v("n"))])
    helper = m.Function("bump", (("p", "i32"),), "i32", (m.ret(op(v("p"), "+", i(1))),))
    check = m.Function("check", (("p", "i32"),), "bool", (m.ret(op(v("p"), "<", i(8))),))
    rng = random.Random(611827)
    for number in range(144):
        bound, threshold, skip = rng.randrange(1, 10), rng.randrange(1, 6), rng.randrange(0, 7)
        initial = rng.choice((m.MIN, m.MAX, -5, 0, 7)) if number % 5 == 0 else rng.randrange(-20, 21)
        update = m.store("x", op(v("x"), rng.choice(("+", "-", "*")), i(rng.randrange(-3, 4))))
        inner = [m.let("j", i(0), True), loop(b(True), [m.store("j", op(v("j"), "+", i(1))),
                 m.branch(op(v("j"), ">=", i(threshold)), [br]),
                 m.branch(op(v("j"), "<", i(2)), [co]),
                 m.store("sum", op(v("sum"), "+", v("old")))])]
        end = m.branch(op(v("n"), ">", i(threshold + 1)), [br], [co])
        body = [m.let("old", v("n")), m.let("fresh", i(2), True), m.store("n", m.call("bump", v("n"))),
                m.branch(op(v("n"), "<", i(skip)), [co]), *inner,
                m.store("fresh", op(v("fresh"), "+", v("old"))), update,
                m.store("sum", op(v("sum"), "+", v("fresh"))), end]
        condition = op(op(v("n"), "<", i(bound)), "&&", m.call("check", v("n")))
        yield case("seeded_nested_transfers", [m.let("n", i(0), True), m.let("x", i(initial), True), m.let("sum", i(0), True),
                   loop(condition, body), m.ret(op(v("x"), "+", v("sum")))], helpers=(helper, check))
    for flag in (False, True):
        for first in ("return", "break", "continue"):
            for second in ("return", "break", "continue"):
                a = [m.ret(i(11))] if first == "return" else [transfer(first)]
                z = [m.ret(i(17))] if second == "return" else [transfer(second)]
                choose = m.Function("choose", (("flag", "bool"),), "i32", (m.let("n", i(0), True),
                         loop(op(v("n"), "<", i(2)), [m.store("n", op(v("n"), "+", i(1))), m.branch(v("flag"), a, z)]), m.ret(v("n"))))
                yield case("mixed_terminal_arms", [m.ret(m.call("choose", b(flag)))], helpers=(choose,))
    bad = m.Function("bad", (), "bool", (m.discard(op(i(m.MAX), "+", i(1))), m.ret(b(True))))
    for flag in (False, True):
        for operator in ("&&", "||"):
            yield case("lazy_condition_error", [loop(op(b(flag), operator, m.call("bad")), [br]), m.ret(b(True))], "bool", (bad,))
    for kind in ("break", "continue"):
        yield case("transfer_before_error", [m.let("n", i(0), True), loop(op(v("n"), "<", i(2)), [m.store("n", op(v("n"), "+", i(1))),
                   m.branch(b(True), [transfer(kind)], [m.discard(op(i(m.MAX), "+", i(1)))])]), m.ret(v("n"))])
        yield case("error_before_transfer", [loop(b(True), [m.discard(op(i(m.MAX), "+", i(1))), transfer(kind)]), m.ret(i(0))])
    yield case("infinite_continue", [loop(b(True), [co]), m.ret(m.u())], "unit")
    never = m.Function("never", (), "unit", (loop(b(True), [co]), m.ret(m.u())))
    yield case("callee_exhaustion", [m.discard(m.call("never")), m.ret(m.u())], "unit", (never,))
    for last in (False, True):
        # Continue replaces the implicit backedge with the same cost. The
        # independently hand-counted grouped counter base22 + 90906*11 plus
        # four false-if costs3 is exactly1m; selecting the final arm adds1.
        yield case("default_budget_exact" if not last else "default_budget_plus_one", [m.let("n", i(0), True),
                   *[m.branch(b(False), []) for _ in range(3)], m.branch(b(last), []),
                   loop(op(v("n"), "<", i(90_906)), [m.store("n", op(v("n"), "+", i(1))), co]), m.ret(v("n"))])
    yield case("shared_calls_budget", [m.let("n", i(0), True), loop(op(v("n"), "<", i(100_000)), [m.store("n", m.call("bump", v("n"))), co]), m.ret(v("n"))], helpers=(helper,))


def native_resources():
    for category, source, accepted, stdout in w.native_resources():
        yield category, source.replace("while false {}", "while false { continue; }"), accepted, stdout


def negatives():
    yield "fn main()->() { break; return; }", "E0204"
    yield "fn main()->() { continue; return; }", "E0204"
    yield "fn f()->() { break; return; } fn main()->() { while true { f(); break; } return; }", "E0204"
    yield "fn main()->() { while false {} continue; return; }", "E0204"
    for word in ("break", "continue"):
        for suffix in (" 1;", " true;", " x;", " 'label;", "()", ""):
            yield f"fn main()->() {{ while false {{ {word}{suffix} }} return; }}", "E0100"
        yield f"fn main()->() {{ while false {{ let x={word}; }} return; }}", "E0100"
        yield f"fn main()->() {{ while false {{ {word}; (); }} return; }}", "E0303"
        yield f"fn main()->() {{ while true {{ {word}; }} }}", "E0302"
        yield f"fn main()->() {{ while false {{ if true {{ {word}; }} else {{ absent; }} }} return; }}", "E0200"
        yield f"fn main()->() {{ while false {{ if true {{ {word}; }} else {{ let x:bool=1; }} }} return; }}", "E0300"
    for a in ("break;", "continue;", "return;"):
        for z in ("break;", "continue;", "return;"):
            yield f"fn main()->() {{ while false {{ if true {{ {a} }} else {{ {z} }} (); }} return; }}", "E0303"


def self_check():
    sample = [loop(m.b(True), [transfer("break")]), m.ret(m.i(7))]
    item = case("self", sample)
    assert item[4].operations == 9 and item[4].breaks == 1 and item[2] == m.Scalar("i32", 7)
    for budget in range(9):
        failed = case("self", sample, budget=budget)
        assert failed[3].code == "E0601"
    sample = [m.let("n", m.i(0), True), loop(m.op(m.v("n"), "<", m.i(2)), [m.store("n", m.op(m.v("n"), "+", m.i(1))), transfer("continue")]), m.ret(m.v("n"))]
    item = case("self", sample)
    assert item[4].operations == 44 and item[4].continues == 2 and item[2] == m.Scalar("i32", 2)
    for budget in range(44):
        assert case("self", sample, budget=budget)[3].code == "E0601"


def verify(binaries):
    self_check()
    binaries = [str(Path(binary).resolve()) for binary in binaries]
    hashes = [hashlib.sha256(Path(binary).read_bytes()).hexdigest() for binary in binaries]
    counts = {key: 0 for key in ("source_cases", "success_cases", "overflow_cases", "fuel_cases", "compiled_artifacts", "negative_cases", "invocations", "modeled_operations", "modeled_stores", "modeled_iterations", "modeled_breaks", "modeled_continues", "native_resource_cases", "compiled_resource_artifacts")}
    corpus, artifacts, traces = hashlib.sha256(), hashlib.sha256(), hashlib.sha256()
    categories, io = {}, set()
    with tempfile.TemporaryDirectory(prefix="oxid-loop-control-oracle-") as directory:
        root = Path(directory)
        source_path = root / "loop-control.ox"
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
            counts["modeled_breaks"] += model.breaks
            counts["modeled_continues"] += model.continues
            previous = None
            if failure:
                line, column = location(source, failure.start)
                message = "execution fuel exhausted" if failure.code == "E0601" else "checked i32 arithmetic overflow"
                expected = (1, b"", f"error[{failure.code}] (oir-run): {message}\n  --> loop-control.ox:{line}:{column}\n".encode())
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
        for source, code in negatives():
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
    print("loop control O0: independent state/fuel model, exact first-error origins, shared calls/loops budget, standalone profile-identical LLVM artifacts and I/O failures: PASS")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    verify(sys.argv[1:])
