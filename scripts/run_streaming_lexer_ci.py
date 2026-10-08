#!/usr/bin/env python3
"""Build exact-head ordinary compilers and run the full lexical component-use gate."""
import argparse
import json
from pathlib import Path
import re
import shutil
import sys

from run_hir_producer_ci import Runner as BuildRunner, digest, require, select_executable
from qualify_streaming_lexer import validate_summary

INPUTS=("scripts/run_streaming_lexer_ci.py", "scripts/run_hir_producer_ci.py",
        "scripts/qualify_streaming_lexer.py", "scripts/qualify_streaming_lexer_controls.py",
        "scripts/streaming_lexer_protocol.py", "scripts/build_streaming_lexer.py",
        "scripts/build_streaming_lexer_observer.py", "scripts/build_hir_producers_v2.py",
        "fixtures/typed-streaming-lexer/sources.json", "fixtures/typed-frontend-v2/sources.json")


class Runner(BuildRunner):
    def run(self):
        require(not self.forbidden,"ordinary profiles forbid overrides: "+", ".join(self.forbidden))
        initial=self.identity("initial")
        self.receipt["source"]=initial
        self.receipt["inputs"]={name:digest(self.repo/name)for name in INPUTS}
        rust=self.call("rust-version",["rustc","-Vv"]).decode()
        require(re.search(r"^release: 1\.99\.0$",rust,re.MULTILINE),"requires qualified Rust1.99.0")
        self.call("cargo-version",[self.args.cargo,"-V"])
        for tool in ("clang","llc","ld.lld"):
            require("19.1.7"in self.call(tool+"-version",[self.args.llvm_bin/tool,"--version"]).decode(),"qualified LLVM19.1.7 required")
        self.call("cc-version",[self.args.cc,"--version"])
        binaries={}
        for profile in ("debug","release"):
            command=[self.args.cargo,"build","--locked","--offline","--bin","oxid",
                     "--message-format=json","--target-dir",self.target]
            if profile=="release":command.append("--release")
            data=self.call(profile+"-build",command)
            executable,artifact=select_executable(data,self.target,profile)
            retained=self.output/(profile+"-compiler")
            shutil.copy2(executable,retained)
            require(digest(retained)==digest(executable),"compiler snapshot identity")
            binaries[profile]=retained
            self.receipt["profiles"][profile]=dict(executable=str(executable),retained_executable=str(retained),
                executable_sha256=digest(retained),cargo_artifact=artifact,source=initial,complete=False)
            self.save()
        evidence=self.output/"component"
        self.call("component-use",[sys.executable,"-B",self.repo/"scripts/qualify_streaming_lexer.py",
            "--debug",binaries["debug"],"--release",binaries["release"],"--llvm-bin",self.args.llvm_bin,
            "--cc",self.args.cc,"--output",evidence],timeout=1200)
        expected=dict(source_head=initial["head"],compiler_sha256={p:digest(x)for p,x in binaries.items()},
            harness_sha256=self.receipt["inputs"]["scripts/qualify_streaming_lexer.py"],
            lexical_source_manifest_sha256=self.receipt["inputs"]["fixtures/typed-streaming-lexer/sources.json"],
            v2_source_manifest_sha256=self.receipt["inputs"]["fixtures/typed-frontend-v2/sources.json"])
        validate_summary(evidence,expected)
        for profile,path in binaries.items():
            require(digest(path)==self.receipt["profiles"][profile]["executable_sha256"],"compiler changed during qualification")
            self.receipt["profiles"][profile]["complete"]=True
        require(self.identity("final")==initial,"source changed during qualification")
        require({name:digest(self.repo/name)for name in INPUTS}==self.receipt["inputs"],"qualification input changed")
        self.receipt.update(complete=True,component_summary_sha256=digest(evidence/"summary.json"))
        self.save()


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--expected-head",required=True);p.add_argument("--event-sha",required=True)
    p.add_argument("--output",required=True,type=Path);p.add_argument("--llvm-bin",required=True,type=Path)
    p.add_argument("--cargo",default="cargo");p.add_argument("--cc",default="cc");a=p.parse_args()
    for name in ("expected_head","event_sha"):
        if not re.fullmatch(r"[0-9a-f]{40}",getattr(a,name)):p.error(name+" must be an exact lowercase commit SHA")
    a.llvm_bin=a.llvm_bin.resolve(strict=True);runner=None
    try:
        runner=Runner(a);runner.run()
    except Exception as error:
        if runner is not None:runner.receipt["error"]=str(error);runner.save()
        print(str(error),file=sys.stderr);return 1
    print(json.dumps({"complete":True,"receipt":str(runner.output/"receipt.json")}));return 0


if __name__=="__main__":sys.exit(main())
