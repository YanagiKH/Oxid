from pathlib import Path
import hashlib
import json
import os
import stat
import subprocess
import sys

compiler = Path(sys.argv[1]).resolve(strict=True)
fixture = Path(sys.argv[2]).resolve(strict=True)
out = Path(sys.argv[3]).absolute()
out.mkdir()
app = fixture / "consumer"
roots = list((app / ".oxid/registry").iterdir())
assert len(roots) == 1
cache = roots[0]
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
lock = (app / "oxid.lock").read_bytes()
report = {"schema": "oxid-registry-cache-filesystem-boundaries-v1", "status": "running",
          "compiler_sha256": sha(compiler), "fixture": str(fixture), "commands": []}


def denied(name):
    d = out / name
    d.mkdir()
    argv = [str(compiler), "check", "--offline", "--message-format=json"]
    result = subprocess.run(argv, cwd=app, stdin=subprocess.DEVNULL, capture_output=True, timeout=15)
    (d / "stdout").write_bytes(result.stdout)
    (d / "stderr").write_bytes(result.stderr)
    row = {"name": name, "argv": argv, "cwd": str(app), "exit": result.returncode,
           "stdout_sha256": sha(d / "stdout"), "stderr_sha256": sha(d / "stderr")}
    report["commands"].append(row)
    (d / "command.json").write_text(json.dumps(row, indent=2) + "\n")
    assert result.returncode == 1 and not result.stderr, (name, result.returncode, result.stdout, result.stderr)
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
    assert any(x.get("kind") == "diagnostic" and x.get("code") == "E0001" and x.get("stage") == "project" for x in rows)
    assert (app / "oxid.lock").read_bytes() == lock


try:
    for label, relative in [("source", "source/src/lib.ox"), ("record", "release.toml"), ("artifact", "artifact.tar")]:
        target = cache / relative
        original = target.read_bytes()
        alias = out / (label + "-hardlink")
        os.link(target, alias)
        try:
            denied(label + "-hardlink-denied")
            assert target.stat().st_ino == alias.stat().st_ino and target.stat().st_nlink == 2
            assert target.read_bytes() == original and alias.read_bytes() == original
        finally:
            alias.unlink()
        held = out / (label + "-held")
        target.rename(held)
        target.symlink_to(held)
        try:
            denied(label + "-symlink-denied")
            assert target.is_symlink() and target.readlink() == held and held.read_bytes() == original
        finally:
            target.unlink()
            held.rename(target)
    target = cache / "source/src/lib.ox"
    held = out / "source-held-fifo"
    target.rename(held)
    os.mkfifo(target, 0o600)
    try:
        denied("source-fifo-denied")
        assert stat.S_ISFIFO(target.lstat().st_mode)
    finally:
        target.unlink()
        held.rename(target)
    held = out / "source-held-missing"
    target.rename(held)
    try:
        denied("source-missing-denied")
        assert not target.exists()
    finally:
        held.rename(target)
    extra = cache / ".user-extra"
    extra.write_bytes(b"preserve local changes\n")
    try:
        denied("extra-file-denied")
        assert extra.read_bytes() == b"preserve local changes\n"
    finally:
        extra.unlink()
    extra.mkdir()
    try:
        denied("extra-directory-denied")
        assert extra.is_dir() and not list(extra.iterdir())
    finally:
        extra.rmdir()
    directory = cache / "source/src"
    held = out / "source-directory-held"
    directory.rename(held)
    directory.symlink_to(held, target_is_directory=True)
    try:
        denied("source-directory-link-denied")
        assert directory.is_symlink() and directory.readlink() == held
    finally:
        directory.unlink()
        held.rename(directory)
    result = subprocess.run([str(compiler), "run", "--frozen"], cwd=app,
                            stdin=subprocess.DEVNULL, capture_output=True, timeout=15)
    (out / "restored.stdout").write_bytes(result.stdout)
    (out / "restored.stderr").write_bytes(result.stderr)
    assert result.returncode == 0 and result.stdout == b"41\n" and not result.stderr
    assert (app / "oxid.lock").read_bytes() == lock
    assert sha(compiler) == report["compiler_sha256"]
    report.update(status="passed", restored_result=41, original_lock_preserved=True)
except Exception as error:
    report.update(status="failed", error=repr(error))
    raise
finally:
    (out / "result.json").write_text(json.dumps(report, indent=2) + "\n")
