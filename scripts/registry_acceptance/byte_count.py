from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import sys
import tomllib

compiler = Path(sys.argv[1]).resolve(strict=True)
repo = Path(sys.argv[2]).resolve(strict=True)
out = Path(sys.argv[3]).absolute()
out.mkdir()
native = sys.argv[4:] == ["--native"]
work = out / "working"
work.mkdir()
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
report = {"schema": "oxid-registered-byte-count-initial-v1", "status": "running",
          "compiler_sha256": sha(compiler), "native": native, "commands": [], "deployments": []}
cases = [b"", b"hello\nworld\n", bytes(range(256)), "台灣🦀\n".encode()]
cases += [b"A" * n + b"\n" for n in [0, 1, 31, 32, 33, 63, 64, 65]]


def command(name, argv, cwd, *, data=b"", expected=None, env=None):
    d = out / ("command-" + name)
    d.mkdir()
    result = subprocess.run([str(x) for x in argv], cwd=cwd, input=data,
                            capture_output=True, timeout=120, env=env)
    (d / "stdin").write_bytes(data)
    (d / "stdout").write_bytes(result.stdout)
    (d / "stderr").write_bytes(result.stderr)
    row = {"name": name, "argv": [str(x) for x in argv], "cwd": str(cwd), "exit": result.returncode,
           "stdin_sha256": sha(d / "stdin"), "stdout_sha256": sha(d / "stdout"),
           "stderr_sha256": sha(d / "stderr"), "environment": "inherited" if env is None else env}
    report["commands"].append(row)
    (d / "command.json").write_text(json.dumps(row, indent=2) + "\n")
    assert result.returncode == 0 and not result.stderr, (name, result.returncode, result.stdout, result.stderr)
    if expected is not None:
        assert result.stdout == expected, (name, result.stdout, expected)
    return result


def data_cases(prefix, argv, cwd, env=None):
    for i, data in enumerate(cases):
        expected = f"{data.count(bytes([10]))} {len(data)}\n".encode()
        command(prefix + f"-{i:02}", argv, cwd, data=data, expected=expected, env=env)


def deploy(name, binary):
    d = out / ("deployment-" + name)
    d.mkdir()
    target = d / "byte-count"
    shutil.copy2(binary, target)
    assert target.read_bytes().startswith(b"\x7fELF")
    digest = sha(target)
    hidden = out / "working-withheld"
    work.rename(hidden)
    try:
        assert not work.exists()
        data_cases(name, [target], d, env={})
    finally:
        hidden.rename(work)
    assert sha(target) == digest
    report["deployments"].append({"name": name, "sha256": digest,
                                   "source_registry_cache_paths_absent": True, "environment": {}})


try:
    original = repo / "apps/byte-count"
    report["application_source_sha256"] = {str(p.relative_to(original)): sha(p)
                                            for p in original.rglob("*") if p.is_file()}
    authors = work / "authors"
    shutil.copytree(original / "packages", authors)
    registry = work / "registry"
    uri = registry.as_uri()
    command("registry-init", [compiler, "registry", "init", registry], work)
    model, formatter = authors / "counts", authors / "report"
    for p in [model, formatter]:
        m = p / "oxid.toml"
        m.write_text(m.read_text().replace('[project]\n', '[project]\nkind="lib"\nentry="src/lib.ox"\n', 1))
    manifest = formatter / "oxid.toml"
    manifest.write_text(manifest.read_text().replace('counts = "../counts"',
        'counts={registry="' + uri + '",package="byte-count-model",version="0.1.0"}'))
    for label, p in [("model", model), ("report", formatter)]:
        command(label + "-lock", [compiler, "lock"], p)
        command(label + "-publish", [compiler, "publish", "--registry", uri], p)
    app = work / "consumer"
    app.mkdir()
    shutil.copytree(original / "src", app / "src")
    shutil.copytree(original / "tests", app / "tests")
    text = (original / "oxid.toml").read_text()
    text = text.replace('counts = "packages/counts"',
        'counts={registry="' + uri + '",package="byte-count-model",version="0.1.0"}')
    text = text.replace('report = "packages/report"',
        'report={registry="' + uri + '",package="byte-count-report",version="0.1.0"}')
    (app / "oxid.toml").write_text(text)
    command("consumer-check", [compiler, "check"], app)
    tested = command("consumer-test", [compiler, "test", "--message-format=json"], app)
    summaries = [json.loads(line) for line in tested.stdout.splitlines() if b'"kind":"test-summary"' in line]
    assert len(summaries) == 1 and summaries[0]["passed"] == 1 and summaries[0]["failed"] == 0
    command("consumer-lock", [compiler, "lock"], app)
    lock = (app / "oxid.lock").read_bytes()
    parsed = tomllib.loads(lock.decode())
    selected = [p for p in parsed["package"] if p["registry"]]
    assert len(selected) == 2 and {p["name"] for p in selected} == {"byte-count-model", "byte-count-report"}
    data_cases("reference", [compiler, "run", "--frozen"], app)
    absent = work / "registry-unavailable"
    registry.rename(absent)
    try:
        if native:
            for profile in ["dev", "release"]:
                command("consumer-build-" + profile, [compiler, "build", "--frozen", "--profile", profile], app)
                deploy("native-" + profile, app / ".oxid/bin/byte-count")
        archive = work / "byte-count.oxpkg.tar"
        command("package", [compiler, "package", "--frozen", "--output", archive], app)
        command("verify-package", [compiler, "package", "--verify", archive], app)
        restored = work / "restored"
        extracted = command("extract-package", [compiler, "package", "--extract", archive,
            "--output", restored, "--message-format=json"], app)
        summary = json.loads(extracted.stdout)
        assert summary["success"] is True and summary["operation"] == "extract"
        exported = restored / summary["root"]
        for p in restored.rglob("oxid.toml"):
            assert all(isinstance(value, str) for value in tomllib.loads(p.read_text()).get("dependencies", {}).values())
        command("export-frozen-check", [compiler, "check", "--frozen"], exported)
        if native:
            command("export-frozen-build", [compiler, "build", "--frozen", "--profile", "release"], exported)
            deploy("portable-rebuilt", exported / ".oxid/bin/byte-count")
    finally:
        absent.rename(registry)
    assert (app / "oxid.lock").read_bytes() == lock
    assert sha(compiler) == report["compiler_sha256"]
    report.update(status="passed", cases_per_execution=len(cases), nominal_diamond_shared=True,
                  registry_packages=selected, original_registry_lock_preserved=True)
except Exception as error:
    report.update(status="failed", error=repr(error))
    raise
finally:
    (out / "result.json").write_text(json.dumps(report, indent=2) + "\n")
