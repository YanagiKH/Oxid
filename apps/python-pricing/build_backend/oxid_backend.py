"""A dependency-free PEP 517 backend for one bounded Oxid scalar export.

Build hooks use the real `oxid library` command. Installed packages contain only
an ordinary CPython extension and its private, unchanged native library.
"""
from __future__ import annotations

import base64
import csv
import hashlib
import io
import json
import keyword
import os
from pathlib import Path, PurePosixPath
import platform
import re
import selectors
import shutil
import stat
import struct
import subprocess
import sys
import sysconfig
import tarfile
import tempfile
import time
import tomllib
import zipfile

SOURCE_FILES = 256
SOURCE_FILE_BYTES = 1024 * 1024
SOURCE_TOTAL_BYTES = 8 * SOURCE_FILE_BYTES
METADATA_BYTES = 1024 * 1024
NATIVE_BYTES = 32 * 1024 * 1024
ARTIFACT_BYTES = 80 * 1024 * 1024
PROCESS_OUTPUT_BYTES = 64 * 1024
BACKEND_FILES = ("pyproject.toml", "build_backend/oxid_backend.py", "build_backend/shim.c.in")
SCHEMA = "oxid-scalar-native-library-v1"
STATUSES = {"success": 0, "overflow": 1, "division_by_zero": 2, "fuel": 3, "invalid_output": 4}
METADATA_KEYS = {"schema", "abi_version", "export", "symbol", "parameters", "result", "statuses", "target", "profile", "execution_policy", "fuel", "library_sha256", "header_sha256", "compiler_package_version", "compiler_sha256", "ir_sha256", "sources"}
SHA = re.compile(r"[0-9a-f]{64}\Z")
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z_0-9]{0,63}\Z")
NAME = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
VERSION = re.compile(r"(?:0|[1-9][0-9]*)(?:\.(?:0|[1-9][0-9]*)){1,3}\Z")


class BuildError(RuntimeError):
    """An input or host is outside this backend's explicit contract."""


def fail(message):
    raise BuildError(message)


def relative(value):
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > 1024:
        fail("expected a bounded relative POSIX path")
    parts = value.split("/")
    if any(p in ("", ".", "..") for p in parts) or "\\" in value or "\0" in value or ":" in value:
        fail("unsafe relative path")
    if PurePosixPath(value).is_absolute():
        fail("absolute paths are not source identities")
    return value


def checked_path(root, name):
    path = Path(root)
    for part in relative(name).split("/"):
        path = path / part
        try:
            info = path.lstat()
        except OSError as error:
            fail(f"missing input {name}: {error}")
        if stat.S_ISLNK(info.st_mode):
            fail(f"links are not accepted: {name}")
    return path


def read_regular(path, maximum):
    """Bound allocation and reject links, hardlinks, and racing replacements."""
    try:
        before = Path(path).lstat()
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_size > maximum:
            fail(f"not a bounded, singly owned regular file: {path}")
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
        with os.fdopen(descriptor, "rb") as stream:
            opened = os.fstat(stream.fileno())
            if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_nlink) != (before.st_dev, before.st_ino, before.st_size, 1):
                fail("input changed before reading")
            payload = stream.read(maximum + 1)
            after = os.fstat(stream.fileno())
        current = Path(path).lstat()
        identity = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns, s.st_nlink)
        if len(payload) > maximum or identity(opened) != identity(after) or identity(after) != identity(current):
            fail("input changed while reading")
        return payload
    except OSError as error:
        fail(f"cannot read input {path}: {error}")


def digest(payload):
    return hashlib.sha256(payload).hexdigest()


def no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            fail(f"duplicate metadata key: {key}")
        result[key] = value
    return result


def load_project(root=None):
    root = Path.cwd() if root is None else Path(root)
    root = root.absolute()
    if any(path.is_symlink() for path in (root, *root.parents)) or not root.is_dir():
        fail("project root must be a real directory")
    raw = read_regular(checked_path(root, "pyproject.toml"), METADATA_BYTES)
    try:
        document = tomllib.loads(raw.decode("utf-8"))
    except (ValueError, UnicodeError) as error:
        fail(f"invalid pyproject.toml: {error}")
    if set(document) != {"build-system", "project", "tool"}:
        fail("only static build-system, project, and tool tables are supported")
    if document["build-system"] != {"requires": [], "build-backend": "oxid_backend", "backend-path": ["build_backend"]}:
        fail("use the shipped in-tree backend with no dynamic build dependencies")
    project = document["project"]
    if not isinstance(project, dict) or set(project) != {"name", "version", "description", "requires-python"}:
        fail("project metadata must contain exactly name, version, description, requires-python")
    if not isinstance(project["name"], str) or not NAME.fullmatch(project["name"]) or len(project["name"]) > 64:
        fail("name must be a canonical lowercase distribution name, at most 64 characters")
    if not isinstance(project["version"], str) or not VERSION.fullmatch(project["version"]) or len(project["version"]) > 64:
        fail("version must have two to four canonical decimal release components")
    if project["requires-python"] != ">=3.11":
        fail("this backend requires the explicit Python >=3.11 contract")
    description = project["description"]
    if not isinstance(description, str) or len(description.encode("utf-8")) > 512 or any(ord(c) < 32 or ord(c) == 127 for c in description):
        fail("description must be a single bounded metadata line")
    tool = document["tool"]
    if not isinstance(tool, dict) or set(tool) != {"oxid"} or not isinstance(tool["oxid"], dict):
        fail("only the closed tool.oxid configuration is supported")
    config = tool["oxid"]
    if set(config) != {"module", "source", "sources", "export", "profile", "fuel"}:
        fail("unknown or missing tool.oxid setting")
    if not isinstance(config["module"], str) or not IDENTIFIER.fullmatch(config["module"]) or config["module"].startswith("_") or keyword.iskeyword(config["module"]):
        fail("module must be a public ASCII Python identifier")
    if not isinstance(config["export"], str) or not config["export"].startswith("crate::") or not IDENTIFIER.fullmatch(config["export"][7:]):
        fail("export must be exactly crate::<ASCII identifier>")
    if keyword.iskeyword(config["export"][7:]):
        fail("export must also be a Python identifier, excluding keywords")
    if config["profile"] not in ("dev", "release"):
        fail("profile must be dev or release")
    if type(config["fuel"]) is not int or not 0 <= config["fuel"] <= 1000000:
        fail("fuel must be an integer from 0 through 1000000")
    entry = relative(config["source"])
    sources = config["sources"]
    if not isinstance(sources, list) or not 1 <= len(sources) <= SOURCE_FILES - len(BACKEND_FILES):
        fail("sources must be a bounded explicit manifest")
    folded = set()
    parent = PurePosixPath(entry).parent
    for value in sources:
        relative(value)
        if not value.endswith(".ox") or value.casefold() in folded:
            fail("source paths must be unique .ox identities, without case aliases")
        folded.add(value.casefold())
        if not PurePosixPath(value).is_relative_to(parent):
            fail("all declared sources must be under the entry's parent directory")
    if entry not in sources:
        fail("entry source is missing from the manifest")
    # Reject undeclared source neighbors and links before launching any tool.
    source_root = root if str(parent) == "." else checked_path(root, str(parent))
    found = set()
    pending = [source_root]
    visited = 0
    while pending:
        directory = pending.pop()
        with os.scandir(directory) as rows:
            for row in rows:
                visited += 1
                if visited > 2 * SOURCE_FILES:
                    fail("source directory traversal exceeds its independent bound")
                if row.is_symlink():
                    fail("source tree contains a link")
                if row.is_dir(follow_symlinks=False):
                    pending.append(Path(row.path))
                elif row.is_file(follow_symlinks=False):
                    if row.name.endswith(".ox"):
                        found.add(Path(row.path).relative_to(root).as_posix())
                else:
                    fail("source tree contains a special file")
    if found != set(sources):
        fail("declared source manifest differs from the source tree")
    payloads = {}
    total = 0
    for name in (*BACKEND_FILES, *sources):
        data = read_regular(checked_path(root, name), min(SOURCE_FILE_BYTES, SOURCE_TOTAL_BYTES - total))
        total += len(data)
        if total > SOURCE_TOTAL_BYTES:
            fail("source manifest exceeds the independent 8 MiB aggregate bound")
        payloads[name] = data
    return {**config, **project, "root": root, "payloads": payloads, "source_parent": parent, "call_name": config["export"][7:]}


def check_settings(config_settings):
    if config_settings not in (None, {}):
        fail("backend config_settings are closed; configure pyproject.toml instead")


def wheel_tag():
    if sys.implementation.name != "cpython" or sys.version_info < (3, 11) or sys.platform != "linux" or struct.calcsize("P") != 8 or sys.byteorder != "little" or sysconfig.get_config_var("Py_GIL_DISABLED"):
        fail("requires a normal-GIL CPython >=3.11 on little-endian Linux64")
    machine = platform.machine()
    if machine not in ("x86_64", "aarch64") or sysconfig.get_platform() != f"linux-{machine}":
        fail("unsupported or cross-configured Linux architecture")
    try:
        libc = os.confstr("CS_GNU_LIBC_VERSION")
    except (OSError, ValueError):
        libc = None
    if not isinstance(libc, str) or not libc.startswith("glibc "):
        fail("the initial GNU target requires the host's glibc runtime and SDK")
    suffix = sysconfig.get_config_var("EXT_SUFFIX")
    if not isinstance(suffix, str) or not re.fullmatch(r"\.cpython-[0-9]+-[A-Za-z0-9_-]+\.so", suffix):
        fail("unsupported actual CPython extension suffix")
    version = f"cp{sys.version_info.major}{sys.version_info.minor}"
    return f"{version}-{version}-linux_{machine}"


def metadata_files(config):
    # Metadata preparation is static: no subprocess, tool discovery or imports.
    metadata = ("Metadata-Version: 2.1\n" + f"Name: {config['name']}\nVersion: {config['version']}\nSummary: {config['description']}\nRequires-Python: >=3.11\n\n").encode()
    wheel = f"Wheel-Version: 1.0\nGenerator: oxid-pep517-v1\nRoot-Is-Purelib: false\nTag: {wheel_tag()}\n".encode()
    return {"METADATA": metadata, "WHEEL": wheel}


def distribution(config):
    return f"{config['name'].replace('-', '_')}-{config['version']}"


def output_directory(value):
    path = Path(value).absolute()
    for ancestor in (path, *path.parents):
        if ancestor.is_symlink():
            fail("output path may not traverse links")
    if not path.is_dir():
        fail("the frontend must create the output directory")
    return path


def publish_file(staged, destination):
    # link() is atomic and fails if any previous output already exists.
    os.link(staged, destination, follow_symlinks=False)


def exact_members(directory, expected):
    """Do not allocate an unbounded list from a producer-controlled directory."""
    observed = set()
    with os.scandir(directory) as entries:
        for entry in entries:
            if len(observed) >= len(expected) or entry.name not in expected:
                fail("unexpected artifact member")
            observed.add(entry.name)
    if observed != set(expected):
        fail("missing artifact member")


def get_requires_for_build_wheel(config_settings=None):
    check_settings(config_settings)
    load_project()
    return []


def get_requires_for_build_sdist(config_settings=None):
    return get_requires_for_build_wheel(config_settings)


def prepare_metadata_for_build_wheel(metadata_directory, config_settings=None):
    check_settings(config_settings)
    config = load_project()
    files = metadata_files(config)
    output = output_directory(metadata_directory)
    name = distribution(config) + ".dist-info"
    with tempfile.TemporaryDirectory(prefix=".oxid-metadata-", dir=output) as temporary:
        staged = Path(temporary) / name
        staged.mkdir()
        for member, payload in files.items():
            (staged / member).write_bytes(payload)
        destination = output / name
        destination.mkdir()  # Never replace a previous prepared directory.
        try:
            for member in files:
                publish_file(staged / member, destination / member)
        except BaseException:
            shutil.rmtree(destination)
            raise
    return name


def tool_identity(requested, label):
    if not isinstance(requested, str) or not requested or "\0" in requested:
        fail(f"{label} requires one executable, without shell arguments")
    invocation = shutil.which(requested)
    if invocation is None:
        fail(f"cannot find {label} executable: {requested}")
    invocation = str(Path(invocation).absolute())
    resolved = Path(invocation).resolve(strict=True)
    info = resolved.stat()
    if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= 256 * 1024 * 1024:
        fail(f"{label} executable exceeds its independent file bound")
    hasher = hashlib.sha256()
    remaining = 256 * 1024 * 1024
    with resolved.open("rb") as stream:
        while block := stream.read(min(1024 * 1024, remaining + 1)):
            remaining -= len(block)
            if remaining < 0:
                fail(f"{label} executable grew beyond its independent bound")
            hasher.update(block)
    after = resolved.stat()
    if (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        fail(f"{label} executable changed while hashing")
    return {"path": invocation, "resolved": str(resolved), "sha256": hasher.hexdigest()}


def run_tool(arguments, cwd, seconds, commands):
    environment = os.environ.copy()
    for key in ("CC", "CFLAGS", "CPPFLAGS", "LDFLAGS", "CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "LIBRARY_PATH", "LD_PRELOAD", "LD_LIBRARY_PATH"):
        environment.pop(key, None)
    started = time.monotonic()
    process = subprocess.Popen(arguments, cwd=cwd, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    streams = {"stdout": bytearray(), "stderr": bytearray()}
    selector = selectors.DefaultSelector()
    try:
        for name, stream in (("stdout", process.stdout), ("stderr", process.stderr)):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        while selector.get_map():
            remaining = seconds - (time.monotonic() - started)
            if remaining <= 0:
                fail("build tool deadline exceeded")
            for key, _ in selector.select(min(remaining, 0.1)):
                data = os.read(key.fileobj.fileno(), 8192)
                if not data:
                    selector.unregister(key.fileobj)
                else:
                    payload = streams[key.data]
                    if len(payload) + len(data) > PROCESS_OUTPUT_BYTES:
                        fail("build tool output exceeds its independent bound")
                    payload.extend(data)
        remaining = seconds - (time.monotonic() - started)
        if remaining <= 0:
            fail("build tool deadline exceeded")
        code = process.wait(timeout=remaining)
    except BaseException:
        try:
            os.killpg(process.pid, 9)
        except ProcessLookupError:
            pass
        process.wait()
        raise
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()
    receipt = {"argv": list(arguments), "exit_code": code, **{key: bytes(value).decode("utf-8", "replace") for key, value in streams.items()}}
    commands.append(receipt)
    if code:
        fail(f"build tool failed ({code}): {receipt['stderr']}")
    return receipt


def native_header(arity):
    if type(arity) is not int or not 0 <= arity <= 64:
        fail("native arity is outside 0..64")
    prefix = '#ifndef OXID_SCALAR_LIBRARY_V1_H\n#define OXID_SCALAR_LIBRARY_V1_H\n#include <stdint.h>\n#ifdef __cplusplus\nextern "C" {\n#endif\nenum oxid_library_status { OXID_LIBRARY_OK=0, OXID_LIBRARY_OVERFLOW=1, OXID_LIBRARY_DIVISION_BY_ZERO=2, OXID_LIBRARY_FUEL=3, OXID_LIBRARY_INVALID_OUTPUT=4 };\nuint32_t oxid_library_abi_version(void);\n/* out must point to caller-owned, writable, naturally aligned int32_t storage.\n * Every nonzero status preserves *out. No source pointers or resources escape. */\nint32_t oxid_library_call_v1('
    return (prefix + ''.join(f'int32_t arg{i}, ' for i in range(arity)) + 'int32_t *out);\n#ifdef __cplusplus\n}\n#endif\n#endif\n').encode()


def validate_bundle(bundle, config, compiler_info, source_records):
    bundle = Path(bundle)
    if bundle.is_symlink() or not bundle.is_dir():
        fail("native output must be exactly the closed three-file bundle")
    exact_members(bundle, {"liboxid_library.so", "oxid_library.h", "metadata.json"})
    library = read_regular(bundle / "liboxid_library.so", NATIVE_BYTES)
    header = read_regular(bundle / "oxid_library.h", 64 * 1024)
    metadata = read_regular(bundle / "metadata.json", METADATA_BYTES)
    try:
        data = json.loads(metadata.decode("utf-8"), object_pairs_hook=no_duplicates)
    except (ValueError, UnicodeError) as error:
        fail(f"invalid native metadata: {error}")
    if not isinstance(data, dict) or set(data) != METADATA_KEYS:
        fail("native metadata has an unknown or missing field")
    arity = data["parameters"]
    if not isinstance(arity, list) or len(arity) > 64 or any(item != "i32" for item in arity):
        fail("native parameters must be zero through 64 i32 values")
    expected = {"schema": SCHEMA, "abi_version": 1, "export": config["export"], "symbol": "oxid_library_call_v1", "result": "i32", "statuses": STATUSES, "target": host_target(), "profile": config["profile"], "execution_policy": "bounded", "fuel": config["fuel"], "library_sha256": digest(library), "header_sha256": digest(header), "compiler_sha256": compiler_info["sha256"]}
    if any(data.get(key) != value or (type(value) is int and type(data.get(key)) is not int) for key, value in expected.items()):
        fail("native compiler, ABI, target, policy, options, or payload tuple mismatch")
    if not isinstance(data["statuses"], dict) or any(type(value) is not int for value in data["statuses"].values()):
        fail("native status values must be integer ABI codes, excluding bool")
    if not isinstance(data["compiler_package_version"], str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?", data["compiler_package_version"]):
        fail("invalid compiler package version")
    if not isinstance(data["ir_sha256"], str) or not SHA.fullmatch(data["ir_sha256"]):
        fail("invalid IR digest")
    if header != native_header(len(arity)):
        fail("header does not match the exact closed scalar ABI")
    records = data["sources"]
    if not isinstance(records, list) or not 1 <= len(records) <= SOURCE_FILES:
        fail("native source inventory is outside the declared bound")
    observed = {}
    for record in records:
        if not isinstance(record, dict) or set(record) != {"path", "sha256"}:
            fail("invalid native source record")
        name = relative(record["path"])
        if name.casefold() in {value.casefold() for value in observed}:
            fail("duplicate or aliased native source identity")
        observed[name] = record["sha256"]
    if observed != source_records:
        fail("compiler source closure differs from the retained source manifest")
    machine = 62 if platform.machine() == "x86_64" else 183
    if len(library) < 64 or library[:7] != b"\x7fELF\x02\x01\x01" or struct.unpack_from("<HHI", library, 16) != (3, machine, 1):
        fail("native library is not the expected host ELF64 shared object")
    return {"library": library, "header": header, "metadata": metadata, "data": data, "arity": len(arity)}


def host_target():
    wheel_tag()
    return platform.machine() + "-unknown-linux-gnu"


def render_shim(template, config, arity, library_name):
    values = {
        "@MODULE@": config["module"], "@EXPORT@": config["call_name"],
        "@ARITY@": str(arity), "@LIBRARY@": library_name,
        "@PARAMETERS@": ", ".join(["int32_t"] * arity + ["int32_t *"]),
        "@ARGUMENTS@": ", ".join([f"values[{i}]" for i in range(arity)] + ["&output"]),
    }
    text = template.decode("utf-8")
    for key, value in values.items():
        if key not in text:
            fail("shim template is missing an expected marker")
        text = text.replace(key, value)
    if re.search(r"@[A-Z_]+@", text):
        fail("shim template contains an unbound marker")
    return text.encode()


def record_bytes(files, record_name):
    output = io.StringIO(newline="")
    writer = csv.writer(output, lineterminator="\n")
    for name in sorted(files):
        payload = files[name]
        encoded = base64.urlsafe_b64encode(hashlib.sha256(payload).digest()).rstrip(b"=").decode()
        writer.writerow((name, "sha256=" + encoded, str(len(payload))))
    writer.writerow((record_name, "", ""))
    return output.getvalue().encode()


def build_wheel(wheel_directory, config_settings=None, metadata_directory=None):
    check_settings(config_settings)
    config = load_project()
    tag = wheel_tag()
    metadata = metadata_files(config)
    if metadata_directory is not None:
        prepared = Path(metadata_directory)
        if prepared.is_symlink() or not prepared.is_dir() or prepared.name != distribution(config) + ".dist-info":
            fail("prepared metadata has an unexpected identity or member")
        exact_members(prepared, metadata)
        for name, expected in metadata.items():
            if read_regular(prepared / name, METADATA_BYTES) != expected:
                fail("prepared metadata differs from the static project metadata")
    output = output_directory(wheel_directory)
    filename = distribution(config) + "-" + tag + ".whl"
    if (output / filename).exists() or (output / filename).is_symlink():
        fail("wheel output already exists")
    compiler = tool_identity(os.environ.get("OXID", "oxid"), "Oxid")
    cc_request = os.environ.get("OXID_PYTHON_CC")
    if cc_request is None:
        llvm_bin = os.environ.get("OXID_LLVM_BIN")
        cc_request = str(Path(llvm_bin) / "clang") if llvm_bin else "clang-19"
    cc = tool_identity(cc_request, "C compiler")
    commands = []
    with tempfile.TemporaryDirectory(prefix=".oxid-wheel-", dir=output) as temporary:
        stage = Path(temporary)
        source = stage / "source"
        source.mkdir()
        for name, payload in config["payloads"].items():
            destination = source / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(payload)
        entry = PurePosixPath(config["source"])
        cwd = source / entry.parent
        bundle = stage / "bundle"
        args = [compiler["path"], "--edition=typed-preview", "library", "--export=" + config["export"], "--profile=" + config["profile"], "--fuel=" + str(config["fuel"]), "--output=" + str(bundle), "--", entry.name]
        run_tool(args, cwd, 180, commands)
        sources = {str(PurePosixPath(name).relative_to(entry.parent)): digest(config["payloads"][name]) for name in config["sources"]}
        admitted = validate_bundle(bundle, config, compiler, sources)
        if tool_identity(compiler["path"], "Oxid") != compiler:
            fail("Oxid executable changed during compilation")
        version = run_tool([cc["path"], "--no-default-config", "--version"], stage, 10, commands)
        if not re.search(r"\bclang version 19\.1\.7\b", version["stdout"]):
            fail("the initial backend requires LLVM clang 19.1.7")
        library_name = "liboxid_" + digest(admitted["library"]) + ".so"
        shim = render_shim(config["payloads"]["build_backend/shim.c.in"], config, admitted["arity"], library_name)
        (stage / "shim.c").write_bytes(shim)
        extension = stage / ("_native" + sysconfig.get_config_var("EXT_SUFFIX"))
        includes = []
        for key in ("include", "platinclude"):
            include = sysconfig.get_path(key)
            if not isinstance(include, str) or not Path(include).is_absolute() or not Path(include).is_dir():
                fail("actual interpreter development headers are required")
            if include not in includes:
                includes.append(include)
        c_args = [cc["path"], "--no-default-config", "--target=" + host_target(), "-std=c11", "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror", "-fvisibility=hidden", "-Wl,-z,noexecstack", "-Wl,-z,relro", "-Wl,-z,now", *("-I" + value for value in includes), str(stage / "shim.c"), "-ldl", "-o", str(extension)]
        run_tool(c_args, stage, 60, commands)
        if tool_identity(cc["path"], "C compiler") != cc:
            fail("C compiler executable changed during compilation")
        extension_bytes = read_regular(extension, NATIVE_BYTES)
        package = config["module"]
        private = package + "/_oxid_native/"
        files = {
            package + "/__init__.py": f'"""An Oxid scalar function compiled for this Python host."""\nfrom ._native import {config["call_name"]}\n\n__all__ = ["{config["call_name"]}"]\n'.encode(),
            package + "/" + extension.name: extension_bytes,
            private + library_name: admitted["library"],
            private + "oxid_library.h": admitted["header"],
            private + "metadata.json": admitted["metadata"],
        }
        provenance = {"schema": "oxid-python-native-build-v1", "wheel_tag": tag, "extension_sha256": digest(extension_bytes), "library_filename": library_name, "library_sha256": digest(admitted["library"]), "compiler": compiler, "c_compiler": cc, "python": {"implementation": sys.implementation.name, "version": platform.python_version(), "soabi": sysconfig.get_config_var("SOABI")}, "source_manifest": sources, "commands": commands, "abi3_qualified": False, "manylinux_qualified": False}
        provenance_bytes = (json.dumps(provenance, sort_keys=True, indent=2) + "\n").encode()
        if len(provenance_bytes) > METADATA_BYTES:
            fail("build provenance exceeds the metadata bound")
        files[private + "build.json"] = provenance_bytes
        prefix = distribution(config) + ".dist-info/"
        files.update({prefix + name: payload for name, payload in metadata.items()})
        files[prefix + "RECORD"] = record_bytes(files, prefix + "RECORD")
        if sum(map(len, files.values())) > ARTIFACT_BYTES:
            fail("wheel payload exceeds its independent artifact bound")
        staged = stage / filename
        with zipfile.ZipFile(staged, "x", compression=zipfile.ZIP_STORED, allowZip64=False) as archive:
            for name in sorted(files):
                relative(name)
                info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
                info.external_attr = (stat.S_IFREG | 0o644) << 16
                archive.writestr(info, files[name])
        if staged.stat().st_size > ARTIFACT_BYTES:
            fail("wheel archive exceeds its independent artifact bound")
        publish_file(staged, output / filename)
    return filename


def build_sdist(sdist_directory, config_settings=None):
    check_settings(config_settings)
    config = load_project()
    output = output_directory(sdist_directory)
    name = distribution(config)
    filename = name + ".tar.gz"
    if (output / filename).exists() or (output / filename).is_symlink():
        fail("sdist output already exists")
    with tempfile.TemporaryDirectory(prefix=".oxid-sdist-", dir=output) as temporary:
        staged = Path(temporary) / filename
        with tarfile.open(staged, "w:gz", format=tarfile.USTAR_FORMAT) as archive:
            for member, payload in sorted(config["payloads"].items()):
                info = tarfile.TarInfo(name + "/" + relative(member))
                info.size = len(payload)
                info.mode = 0o644
                info.mtime = 0
                archive.addfile(info, io.BytesIO(payload))
            metadata = metadata_files(config)["METADATA"]
            info = tarfile.TarInfo(name + "/PKG-INFO")
            info.size = len(metadata)
            info.mode = 0o644
            info.mtime = 0
            archive.addfile(info, io.BytesIO(metadata))
        if staged.stat().st_size > SOURCE_TOTAL_BYTES + METADATA_BYTES:
            fail("sdist archive exceeds its independent bound")
        publish_file(staged, output / filename)
    return filename
