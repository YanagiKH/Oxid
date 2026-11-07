from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import sys
import tomllib

compiler = Path(sys.argv[1]).resolve(strict=True)
out = Path(sys.argv[2]).absolute()
out.mkdir()
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
report = {"schema": "oxid-registry-initial-public-trust-v1", "status": "running",
          "compiler": {"path": str(compiler), "sha256": sha(compiler)}, "commands": []}


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def run(name, args, cwd, *, expected=0, stdout=None):
    directory = out / ("command-" + name)
    directory.mkdir()
    argv = [str(compiler), *args]
    result = subprocess.run(argv, cwd=cwd, stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=60)
    (directory / "stdout").write_bytes(result.stdout)
    (directory / "stderr").write_bytes(result.stderr)
    row = {"name": name, "argv": argv, "cwd": str(cwd), "exit": result.returncode,
           "stdout_sha256": sha(directory / "stdout"), "stderr_sha256": sha(directory / "stderr")}
    report["commands"].append(row)
    (directory / "command.json").write_text(json.dumps(row, indent=2) + "\n")
    assert result.returncode == expected, (name, result.returncode, result.stdout, result.stderr)
    if stdout is not None:
        assert result.stdout == stdout and not result.stderr, (name, result.stdout, result.stderr)
    return result


try:
    registry = out / "registry"
    alternative = out / "different-registry"
    author = out / "author"
    app = out / "consumer"
    write(author / "oxid.toml", '[project]\nname="numbers"\nversion="1.2.0"\n'
          'edition="typed-preview"\nkind="lib"\nentry="src/lib.ox"\n')
    write(author / "src/lib.ox", 'pub fn answer() -> i32 { return 41; }\n')
    run("init", ["registry", "init", str(registry)], out)
    run("author-lock", ["lock"], author)
    run("publish", ["publish", "--registry", registry.as_uri()], author)
    released = registry / "packages/numbers/1.2.0"
    published = {p.name: sha(p) for p in released.iterdir() if p.is_file()}
    assert set(published) == {"release.toml", "artifact.tar"}
    run("republish-denied", ["publish", "--registry", registry.as_uri()], author, expected=1)
    assert published == {p.name: sha(p) for p in released.iterdir() if p.is_file()}
    write(app / "oxid.toml", '[project]\nname="registry-consumer"\n'
          'edition="typed-preview"\nentry="src/main.ox"\n[build]\nentry-mode="result"\n'
          '[dependencies]\nnumbers={registry="' + registry.as_uri()
          + '",package="numbers",version="1.2.0"}\n')
    write(app / "src/main.ox", 'fn main()->i32{return crate::numbers::answer();}\n')
    uncached = run("uncached-doc-denied", ["doc", "--message-format=json"], app, expected=1)
    diagnostics = [json.loads(line) for line in uncached.stdout.splitlines() if line.strip()]
    assert any(row.get("kind") == "diagnostic" and "not cached" in row.get("message", "") for row in diagnostics)
    assert not (app / ".oxid").exists() and not (app / "oxid.lock").exists()
    workspace = out / "doc-workspace"
    write(workspace / "oxid.toml", '[workspace]\nedition="typed-preview"\nmembers=["client"]\n')
    client = workspace / "client"
    write(client / "oxid.toml", (app / "oxid.toml").read_text())
    write(client / "src/main.ox", (app / "src/main.ox").read_text())
    uncached_workspace = run("uncached-workspace-doc-denied", ["doc", "--workspace", "--message-format=json"], workspace, expected=1)
    diagnostics = [json.loads(line) for line in uncached_workspace.stdout.splitlines() if line.strip()]
    assert any(row.get("kind") == "diagnostic" and "not cached" in row.get("message", "") for row in diagnostics)
    assert not (workspace / ".oxid").exists() and not (workspace / "oxid.lock").exists()
    assert not (client / ".oxid").exists() and not (client / "oxid.lock").exists()
    run("workspace-explicit-fetch", ["check", "--workspace"], workspace)
    run("first-use-run", ["run"], app, stdout=b"41\n")
    assert not (app / "oxid.lock").exists()
    run("consumer-lock", ["lock"], app)
    lock = (app / "oxid.lock").read_bytes()
    parsed = tomllib.loads(lock.decode())
    assert parsed["version"] == 4 and parsed["kind"] == "typed-registry-project-v1"
    packages = [p for p in parsed["package"] if p["registry"]]
    assert len(packages) == 1
    selected = packages[0]
    assert selected["name"] == "numbers" and selected["version"] == "1.2.0"
    assert selected["registry"] == registry.as_uri()
    assert selected["artifact"] == published["artifact.tar"]
    assert selected["artifact-bytes"] == (released / "artifact.tar").stat().st_size
    report["registry_package"] = selected
    roots = list((app / ".oxid/registry").iterdir())
    assert len(roots) == 1
    cache = roots[0]
    assert sha(cache / "artifact.tar") == published["artifact.tar"]
    registry_away = out / "registry-unavailable"
    registry.rename(registry_away)
    try:
        run("offline-run", ["run", "--frozen"], app, stdout=b"41\n")
        run("offline-doc", ["doc", "--frozen"], app)
        run("cached-plain-doc", ["doc"], app)
        run("cached-workspace-plain-doc", ["doc", "--workspace"], workspace)
        assert not (workspace / "oxid.lock").exists() and not (client / "oxid.lock").exists()
        assert (app / "oxid.lock").read_bytes() == lock
    finally:
        registry_away.rename(registry)
    cached_source = cache / "source/src/lib.ox"
    original = cached_source.read_bytes()
    cached_source.write_bytes(original.replace(b"41", b"99"))
    poisoned = cached_source.read_bytes()
    run("poisoned-cache-denied", ["check", "--offline"], app, expected=1)
    assert cached_source.read_bytes() == poisoned
    assert (app / "oxid.lock").read_bytes() == lock
    cached_source.write_bytes(original)
    run("restored-cache-run", ["run", "--frozen"], app, stdout=b"41\n")
    run("alternative-init", ["registry", "init", str(alternative)], out)
    write(author / "src/lib.ox", 'pub fn answer() -> i32 { return 42; }\n')
    run("author-update", ["lock", "--update"], author)
    run("alternative-publish", ["publish", "--registry", alternative.as_uri()], author)
    other = alternative / "packages/numbers/1.2.0"
    assert sha(other / "artifact.tar") != published["artifact.tar"]
    # Simulate an untrusted registry owner changing an immutable tuple behind
    # the supported publisher. Keep the old cache and lock as independent pins.
    for name in published:
        shutil.copyfile(other / name, released / name)
    saved_cache = out / "trusted-cache-withheld"
    cache.rename(saved_cache)
    try:
        run("changed-tuple-update-denied", ["lock", "--update"], app, expected=1)
        assert (app / "oxid.lock").read_bytes() == lock and not cache.exists()
        run("changed-tuple-check-denied", ["check"], app, expected=1)
        assert (app / "oxid.lock").read_bytes() == lock and not cache.exists()
    finally:
        saved_cache.rename(cache)
    run("trusted-cache-still-runs", ["run", "--frozen"], app, stdout=b"41\n")
    assert (app / "oxid.lock").read_bytes() == lock
    assert sha(compiler) == report["compiler"]["sha256"]
    report["status"] = "passed"
except Exception as error:
    report.update(status="failed", error=repr(error))
    raise
finally:
    (out / "result.json").write_text(json.dumps(report, indent=2) + "\n")
