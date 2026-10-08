#!/usr/bin/env python3
"""Independent source-scale lexical component-use qualification, Linux x86_64 only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

import build_hir_producers_v2 as hir_builder
import build_streaming_lexer as lexer_builder
import build_streaming_lexer_observer as observer_builder
from streaming_lexer_protocol import decode, request
from qualify_streaming_lexer_controls import Controls, expected_receipts as control_receipts

ROOT = Path(__file__).resolve().parents[1]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def closure(directory, entry):
    pending, selected = [entry], {}
    while pending:
        name = pending.pop()
        if name in selected:
            continue
        require(re.fullmatch(r"[a-z_]+", name) is not None, "invalid module name")
        path = directory / (name + ".ox")
        body = path.read_bytes()
        selected[path.resolve()] = body
        # Track names independently of Path keys, with no guessed file selection.
        pending.extend(re.findall(r"\bmod\s+(\w+)\s*;", body.decode("ascii")))
        selected[name] = None
    return {path: body for path, body in selected.items() if isinstance(path, Path)}


def canonical(observer, source, limit=100000):
    result = subprocess.run([str(observer), str(limit)], input=source, capture_output=True,
                            timeout=30, check=True)
    facts = json.loads(result.stdout)
    if facts["status"] == "ok":
        return {"tokens": [[t["id"], t["start"], t["end"]] for t in facts["tokens"]]}
    d = facts["diagnostic"]
    tags = {"unterminated string literal": 1, "unterminated block comment": 2,
            "token resource limit exceeded": 3}
    require(d["stage"] == "lex" and not d["secondary"] and not d["notes"], "canonical diagnostic scope")
    return {"diagnostic": dict(tag=tags[d["message"]], code=d["code"], stage=d["stage"],
                               message=d["message"], start=d["primary"]["start"], end=d["primary"]["end"])}


def split_cases():
    cases = {
        "identifiers": b"_ A a0 __under123 z9z",
        "short-keywords": b"fn if or mod use pub let mut for ref and fnx",
        "four-keywords": b"else true loop enum move impl type null enumx",
        "five-keywords": b"break while false macro const match async await trait whiles",
        "long-keywords": b"struct return import unsafe extern continue macro_rules macro_rulesx",
        "numbers": b"0 123_abc 12.3 4..a 8xFF 9_ 0123 foo0",
        "strings": b'"" "a\\\"b" "x\\\\y" "a\\\nq" "/*hi*/"',
        "comments": b"// words\r\n /*a**/ /**/ /*/*/ fn //tail",
        "operators": b"(){}:,; = == ! != & && . | || < <= > >= - -> + * / % [] # ' @",
        "adjacency": b"a/*c*/b//x\r\nreturn!=fn_0->0.1;&&||/ =/ !/ -/",
        "ascii-invalid": bytes([0, 1, 2, 3, 4, 5, 6, 7, 8, 14, 15, 16, 27, 31, 36, 63, 64, 92, 94, 96, 126, 127]),
        "whitespace": b" \t\n\v\f\r\t fn\r\n\n",
        "open-string": b'fn f() { "open',
        "open-escape": b'a "unfinished\\',
        "open-comment": b"let x=0; /* a**",
        "slash-star": b"/ /**/ //\n/***/ /* x */ **/ /",
    }
    seen = set()
    for name, source in cases.items():
        for suffix, data in [("plain", source)] + [(str(i), b" " * (128-i) + source) for i in range(len(source)+1)]:
            if data not in seen:
                seen.add(data)
                yield name + "-" + suffix, data, 100000
    for name, data in (("long-identifier", b"a"*400+b" fn"),
                       ("long-comment", b"/*"+b"x"*500+b"*/return"),
                       ("long-string", b'"'+b'\\"'*220+b'" 123'),
                       ("long-whitespace", b" "*300+b"fn")):
        yield name, data, 100000
    for name, data in (("empty", b""), ("one", b"x"), ("trivia", b" x y"),
                       ("drain", b"x y "+b"a"*400), ("string-precedence", b' "open'),
                       ("comment-precedence", b" /*open")):
        for limit in range(5):
            yield name + "-limit-" + str(limit), data, limit


def expected_receipts():
    names=[]
    for profile in ("debug","release"):
        names += [profile+"-build-"+role for role in ("parser","static","lexer")]
        names += [profile+"-rebuilt-lex-"+str(i) for i in range(36)]
        for case in ("scalar","leading255","loop255"):
            names += [profile+"-replay-"+case+"-"+stage for stage in ("parser","static","check","run","compile","native")]
    names += ["corpus-"+str(i) for i in range(623)]
    return names


def validate_summary(root,expected):
    """Re-admit exact recipe, identities and retained command streams."""
    summary=json.loads((root/"summary.json").read_bytes())
    require(summary.get("schema_version")==1 and summary.get("status")=="passed"
            and summary.get("component_use_only") is True,"incomplete lexical component summary")
    for key,value in expected.items():require(summary.get(key)==value,"lexical identity mismatch: "+key)
    require(type(summary.get("module_invocations"))is int and summary["module_invocations"]==82,"module invocation recipe")
    require(type(summary.get("corpus_cases"))is int and summary["corpus_cases"]==623,"split corpus recipe")
    require(type(summary.get("exact_diagnostics"))is int and summary["exact_diagnostics"]==69,"diagnostic corpus recipe")
    receipts=json.loads((root/"receipts.json").read_bytes())
    require([r["name"]for r in receipts]==expected_receipts(),"exact component command recipe")
    for row in receipts:
        require(type(row.get("status"))is int and row["status"]==0,"component command failed")
        require(isinstance(row.get("argv"),list) and row["argv"],"missing component command")
        for stream in ("stdout","stderr"):
            require(row[stream+"_sha256"]==digest((root/(row["name"]+"."+stream)).read_bytes()),"component stream identity")
    require(summary["receipts_sha256"]==digest((root/"receipts.json").read_bytes()),"component receipt identity")
    modules=json.loads((root/"module-receipts.json").read_bytes())
    require(len(modules)==82 and summary["modules_sha256"]==digest((root/"module-receipts.json").read_bytes()),"module receipt identity")
    for profile in ("debug","release"):
        control=root/(profile+"-controls")
        original=json.loads((control/"summary.json").read_bytes())
        require(summary["controls"][profile]==original and original["status"]=="passed"
                and type(original["receipts"])is int and original["receipts"]==111
                and original["compiler_sha256"]==expected["compiler_sha256"][profile],"failure-control recipe")
        rows=json.loads((control/"receipts.json").read_bytes())
        require(len(rows)==111 and original["receipts_sha256"]==digest((control/"receipts.json").read_bytes()),"control receipt identity")
        require([(r["name"],r["status"])for r in rows]==control_receipts(),"exact failure-control recipe")
        for row in rows:
            require(type(row.get("status"))is int and row["status"]in(0,1,7),"invalid control status")
            for stream in ("stdout","stderr"):
                require(row[stream+"_sha256"]==digest((control/(row["name"]+"."+stream)).read_bytes()),"control stream identity")
    return summary


class Run:
    def __init__(self, debug, release, llvm, output, cc):
        self.compilers = {"debug": debug.resolve(strict=True), "release": release.resolve(strict=True)}
        self.llvm = llvm.resolve(strict=True)
        self.root = output.resolve()
        self.root.mkdir()
        self.env = dict(os.environ, OXID_LLVM_BIN=str(self.llvm))
        self.cc = cc
        self.receipts = []
        self.modules = []
        self.observer = observer_builder.build(self.root / "observer")
        self.seed = lexer_builder.build(self.compilers["release"], self.llvm, self.root / "seed")

    def call(self, name, argv, data=None, status=0, timeout=120):
        argv = [str(x) for x in argv]
        start = time.monotonic()
        result = subprocess.run(argv, input=data, env=self.env, cwd=ROOT,
                                capture_output=True, timeout=timeout)
        for stream in ("stdout", "stderr"):
            (self.root / (name + "." + stream)).write_bytes(getattr(result, stream))
        self.receipts.append(dict(name=name, argv=argv, status=result.returncode,
                                 input_sha256=digest(data) if data is not None else None,
                                 stdout_sha256=digest(result.stdout), stderr_sha256=digest(result.stderr),
                                 elapsed_seconds=time.monotonic()-start))
        (self.root / "receipts.json").write_text(json.dumps(self.receipts, indent=2)+"\n")
        require(result.returncode == status, f"{name}: expected {status}, got {result.returncode}: {result.stdout!r} {result.stderr!r}")
        return result

    def observations(self, result, expected, producer, label):
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        records = [row for row in rows if row.get("kind") == "lexical-provider"]
        require(len(records) == len(expected), label + ": module invocation count")
        require(rows[-1].get("success") is True and not result.stderr, label + ": successful summary")
        require({Path(row["path"]).resolve() for row in records} == set(expected), label + ": exact loaded module set")
        require([row["file_id"] for row in records] == list(range(len(records))), label + ": source file IDs")
        require(len({row["source_identity"] for row in records}) == len(records), label + ": unique retained source identities")
        remaining = 100000
        for record in records:
            path = Path(record["path"]).resolve()
            source = expected[path]
            framed = request(source, remaining)
            replay = subprocess.run([str(producer)], input=framed, capture_output=True, timeout=10)
            require(replay.returncode == 0 and not replay.stderr, label + ": independent producer replay")
            facts = decode(replay.stdout, source, remaining)
            require(facts == canonical(self.observer, source, remaining), label + ": canonical token parity")
            require(record["source_bytes"] == len(source) and record["source_sha256"] == digest(source), label + ": source binding")
            require(record["executable_sha256"] == digest(producer.read_bytes()) and record["input_sha256"] == digest(framed)
                    and record["input_written"] == len(framed), label + ": executable/input identity")
            require(record["captured_stdout_sha256"] == digest(replay.stdout)
                    and record["captured_stdout_bytes"] == len(replay.stdout)
                    and record["captured_stderr_bytes"] == 0
                    and record["captured_stderr_sha256"] == digest(b""), label + ": stream identity")
            for key in ("spawned", "leader_reaped", "stdin_closed", "stdout_eof", "stderr_eof",
                        "comparison_attempted", "comparison_matched", "producer_tokens_reached_parser"):
                require(record.get(key) is True, label + ": missing " + key)
            require(record["stop"] == "exited" and record["exit_status"] == 0
                    and record["signal"] is None and record["fallback"] is False, label + ": no-fallback completion")
            remaining -= len(facts["tokens"])-1
            self.modules.append(dict(label=label, path=str(path), source_sha256=digest(source),
                                     source_bytes=len(source), tokens=len(facts["tokens"]),
                                     retained_source_identity=record["source_identity"],
                                     producer_sha256=record["executable_sha256"]))

    def compile(self, profile, label, source, expected, provider, target):
        result = self.call(label, [self.compilers[profile], "compile", source, "--edition=typed-preview",
                           "--backend=llvm", "--entry-mode=process", "--experimental-lexical-provider", provider,
                           "--message-format=json", "--output", target])
        self.observations(result, expected, provider / "lexer", label)

    def rebuild(self, profile):
        directory = self.root / profile
        directory.mkdir()
        source_dir = directory / "v2-sources"
        hir_builder.materialize(source_dir)
        bundle = directory / "hir-bundle"
        bundle.mkdir()
        for role, entry in (("parser", "parser_main"), ("static", "ast_static_main")):
            self.compile(profile, profile+"-build-"+role, source_dir/(entry+".ox"),
                         closure(source_dir, entry), self.seed, bundle/role)
        hashes = {role:digest((bundle/role).read_bytes()) for role in ("parser", "static")}
        (bundle/"manifest.txt").write_text("OXID-HIR-PRODUCERS-2\n"+"".join(role+" "+value+"\n" for role,value in hashes.items()))
        lexer_dir = directory / "lexer-sources"
        lexer_dir.mkdir()
        _, bodies = lexer_builder.checked_sources()
        for name, body in bodies.items():
            (lexer_dir/(name+".ox")).write_bytes(body)
        rebuilt = directory / "rebuilt-lexer"
        rebuilt.mkdir()
        self.compile(profile, profile+"-build-lexer", lexer_dir/"main.ox",
                     closure(lexer_dir, "main"), self.seed, rebuilt/"lexer")
        (rebuilt/"manifest.txt").write_text(lexer_builder.BUNDLE_MAGIC+digest((rebuilt/"lexer").read_bytes())+"\n")
        # The rebuilt component itself must process the entire genuine union.
        for index, (path, data) in enumerate({**closure(source_dir, "parser_main"),
                                            **closure(source_dir, "ast_static_main"),
                                            **closure(lexer_dir, "main")}.items()):
            result = self.call(profile+"-rebuilt-lex-"+str(index), [rebuilt/"lexer"], request(data))
            require(not result.stderr and decode(result.stdout,data)==canonical(self.observer,data), "rebuilt lexer complete-source replay")
        self.replay(profile, bundle)
        return rebuilt

    def replay(self, profile, bundle):
        simple=b"fn main()->i32{return 7;}"
        loop=b"fn main()->i32{let mut n=0;while n<3{n=n+1;}return n;}"
        cases=(("scalar",simple,7),("leading255",b" "*(255-len(simple))+simple,7),
               ("loop255",b" "*(255-len(loop))+loop,3))
        for name,source,value in cases:
            label=profile+"-replay-"+name
            opa=self.call(label+"-parser",[bundle/"parser"],source).stdout
            require(len(opa)==1559 and opa[:4]==b"OPA2", "genuine parser frame")
            ast=b"AST2"+bytes([len(source)])+source+opa
            wire=self.call(label+"-static",[bundle/"static"],ast).stdout
            require(len(wire)==2607 and wire[:1559]==opa and wire[1559:1564]==b"STF2\0", "genuine static frame")
            path=self.root/(label+".ox");path.write_bytes(source)
            for operation in ("check","run","compile"):
                argv=[self.compilers[profile],operation,path,"--edition=typed-preview",
                      "--experimental-hir-producers",bundle,"--message-format=json"]
                target=self.root/(label+".elf")
                if operation=="compile":argv += ["--backend=llvm","--output",target]
                result=self.call(label+"-"+operation,argv)
                rows=[json.loads(line) for line in result.stdout.splitlines()]
                require(rows[-1]["success"] is True and not result.stderr, "producer replay summary")
                if operation=="run":require(rows[-1]["result"]=={"type":"i32","value":value},"producer replay result")
            require(self.call(label+"-native",[target]).stdout==str(value).encode()+b"\n", "native replay result")

    def corpus(self, producer):
        rows=[]
        for index,(name,source,limit) in enumerate(split_cases()):
            result=self.call("corpus-"+str(index),[producer],request(source,limit))
            actual=decode(result.stdout,source,limit)
            require(not result.stderr and actual==canonical(self.observer,source,limit), "split/token/diagnostic parity: "+name)
            rows.append(dict(name=name,bytes=len(source),limit=limit,diagnostic="diagnostic" in actual))
        (self.root/"corpus.json").write_text(json.dumps(rows,indent=2)+"\n")
        return rows

    def qualify(self):
        rebuilt={profile:self.rebuild(profile) for profile in ("debug","release")}
        corpus=self.corpus(rebuilt["release"]/"lexer")
        controls={profile:Controls(self.compilers[profile],self.llvm,rebuilt[profile],
                                  self.root/(profile+"-controls"),self.cc).run()
                  for profile in ("debug","release")}
        (self.root/"module-receipts.json").write_text(json.dumps(self.modules,indent=2)+"\n")
        summary=dict(schema_version=1,status="passed",component_use_only=True,
                     source_head=subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT).decode().strip(),
                     compiler_sha256={p:digest(x.read_bytes()) for p,x in self.compilers.items()},
                     harness_sha256=digest(Path(__file__).read_bytes()),
                     lexical_source_manifest_sha256=digest(lexer_builder.MANIFEST.read_bytes()),
                     v2_source_manifest_sha256=digest(hir_builder.MANIFEST.read_bytes()),
                     module_invocations=len(self.modules),corpus_cases=len(corpus),controls=controls,
                     exact_diagnostics=sum(r["diagnostic"] for r in corpus),
                     receipts_sha256=digest((self.root/"receipts.json").read_bytes()),
                     modules_sha256=digest((self.root/"module-receipts.json").read_bytes()))
        (self.root/"summary.json").write_text(json.dumps(summary,indent=2)+"\n")
        print(json.dumps(summary,indent=2))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--debug",required=True,type=Path)
    parser.add_argument("--release",required=True,type=Path)
    parser.add_argument("--llvm-bin",required=True,type=Path)
    parser.add_argument("--output",required=True,type=Path)
    parser.add_argument("--cc",default="cc")
    a=parser.parse_args()
    Run(a.debug,a.release,a.llvm_bin,a.output,a.cc).qualify()


if __name__=="__main__":
    main()
