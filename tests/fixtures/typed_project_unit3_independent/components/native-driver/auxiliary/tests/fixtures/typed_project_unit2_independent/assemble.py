"""Assemble reviewed instrumentation in a new copy of exact compiler inputs."""
import json
from pathlib import Path
import shutil

import protocol as p


RESOURCE_MODULES = (
    ("src/frontend/declaration_index.rs", "src/frontend/declaration_index/reviewer_resource.rs",
     ("resource-review-tests.rs", "supplemental-resource-review-tests.rs",
      "old-admission-review-test.rs", "exposure-origin-review-test.rs")),
    ("src/frontend/parser.rs", "src/frontend/parser/reviewer_resource.rs", ("parser-resource-review-tests.rs",)),
    ("src/frontend/source.rs", "src/frontend/source/reviewer_resource.rs", ("source-identity-review-tests.rs",)),
)


def inventory(root):
    return [{"path": str(file.relative_to(root)), "bytes": file.stat().st_size,
             "sha256": p.digest(file.read_bytes())}
            for file in sorted(root.rglob("*")) if file.is_file()]


def assemble(repo, output, package, manifest):
    p.check_files(repo, manifest["files"])
    actual = [str(file.relative_to(repo)) for part in ("src", "native")
              for file in (repo / part).rglob("*") if file.is_file()]
    expected = [entry["path"] for entry in manifest["files"]
                if entry["path"].startswith(("src/", "native/"))]
    p.exact_ids(actual, expected)
    p.require(not output.exists(), "candidate output already exists")
    output.mkdir(parents=True)
    for entry in manifest["files"]:
        target = output / entry["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p.relative_file(repo, entry["path"]), target)
    changes = []

    def append(module, addition):
        file = output / module
        before = file.read_bytes()
        file.write_bytes(before + addition)
        changes.append({"path": module, "before_sha256": p.digest(before),
                        "after_sha256": p.digest(file.read_bytes())})

    for module, destination, payloads in RESOURCE_MODULES:
        target = output / destination
        p.require(not target.exists(), "resource instrumentation already exists")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"".join((package / "archive/resource" / name).read_bytes() for name in payloads))
        relative = str(Path(destination).relative_to(Path(module).parent))
        append(module, ('\n#[cfg(test)]\n#[path = "' + relative + '"]\n'
                        'mod independent_unit2_resource_review;\n').encode())

    observer = output / "src/frontend/oir/unit2_observer.rs"
    p.require(not observer.exists(), "observer instrumentation already exists")
    shutil.copy2(package / "semantic/observer.rs", observer)
    append("src/frontend/oir/mod.rs", b"\n#[cfg(test)]\npub(in crate::frontend) mod unit2_observer;\n")

    file = output / "src/frontend/project.rs"
    before = file.read_bytes()
    old = b"    ) -> Result<String, Box<Diagnostic>> {\n        let mut file = File::open(path)"
    new = (b"    ) -> Result<String, Box<Diagnostic>> {\n"
           b"        #[cfg(test)]\n"
           b"        crate::frontend::oir::unit2_observer::record_source_read(path, display, origin);\n"
           b"        let mut file = File::open(path)")
    p.require(before.count(old) == 1, "reviewed read-entry patch context changed")
    file.write_bytes(before.replace(old, new))
    changes.append({"path": str(file.relative_to(output)), "before_sha256": p.digest(before),
                    "after_sha256": p.digest(file.read_bytes())})
    return {"source_inputs_sha256": p.digest((package / "source-inputs.json").read_bytes()),
            "instrumentation": changes, "files": inventory(output)}
