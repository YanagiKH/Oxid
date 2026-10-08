#!/usr/bin/env python3
"""Build an independent full-source observer from unchanged canonical lexer files."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ("lexer.rs", "diagnostic.rs", "source.rs", "project/budget.rs")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def build(output):
    output = Path(output).resolve()
    if output.exists() or output.is_symlink():
        raise ValueError("observer output must be fresh")
    rustc = shutil.which("rustc")
    if rustc is None:
        raise ValueError("rustc is required to build the independent observer")
    bodies = {name: (ROOT / "src/frontend" / name).read_bytes() for name in SOURCES}
    wrapper = (ROOT / "tests/fixtures/bounded_typed_lexer/observer/frontend_mod.rs").read_text()
    wrapper = wrapper.replace("pub(crate) fn observe(text: String)",
                              "pub(crate) fn observe(text: String, limit: usize)")
    wrapper = wrapper.replace("lexer::lex_with_limit(sources.get(id), lexer::MAX_TOKENS)",
                              "lexer::lex_with_limit(sources.get(id), limit)")
    output.mkdir()
    for name, data in bodies.items():
        path = output / "frontend" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    (output / "frontend/mod.rs").write_text(wrapper)
    (output / "main.rs").write_text('''#![allow(dead_code)]
mod frontend;
fn main() {
    use std::io::Read;
    let limit = std::env::args().nth(1).map(|s| s.parse::<usize>().unwrap()).unwrap_or(100_000);
    let mut bytes = Vec::new();
    std::io::stdin().take(1_048_577).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= 1_048_576 && bytes.is_ascii() && limit <= 100_000);
    frontend::observe(String::from_utf8(bytes).unwrap(), limit);
}
''')
    binary = output / ("observer.exe" if os.name == "nt" else "observer")
    argv = [rustc, "--edition=2021", "-C", "opt-level=1", str(output / "main.rs"), "-o", str(binary)]
    result = subprocess.run(argv, capture_output=True, timeout=120)
    (output / "build.stdout").write_bytes(result.stdout)
    (output / "build.stderr").write_bytes(result.stderr)
    version = subprocess.run([rustc, "--version", "--verbose"], capture_output=True, check=True)
    receipt = dict(argv=argv, status=result.returncode, rustc=version.stdout.decode(),
                   canonical_sources=[dict(path="src/frontend/" + name, sha256=digest(data))
                                      for name, data in bodies.items()],
                   source_patches=[],
                   wrappers=[dict(path=name, sha256=digest((output / name).read_bytes()))
                             for name in ("main.rs", "frontend/mod.rs")],
                   binary_sha256=digest(binary.read_bytes()) if result.returncode == 0 else None)
    (output / "identity.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError("independent lexer observer build failed: " + str(output))
    return binary


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    print(build(parser.parse_args().output))
