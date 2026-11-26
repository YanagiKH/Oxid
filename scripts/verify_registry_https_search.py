#!/usr/bin/env python3
"""Exercise catalog export and authenticated HTTPS search with a local TLS server."""
import argparse
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading


TOKEN = "test-token+only/for.the_fixture="
HEADER = b"Oxid registry catalog 1\n"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def invoke(compiler, cwd, environment, args, status=0):
    result = subprocess.run([str(compiler), *args], cwd=cwd, env=environment,
                            capture_output=True, timeout=45)
    require(result.returncode == status,
            f"{args[:2]} exited {result.returncode}: {result.stderr[:256]!r}")
    require(TOKEN.encode() not in result.stdout + result.stderr, "registry disclosed its access token")
    return result


def export(compiler, root, environment):
    registry = root / "index"
    invoke(compiler, root, environment, ["registry", "init", str(registry)])
    source = registry.as_uri()
    require(invoke(compiler, root, environment, ["registry", "catalog", source]).stdout == HEADER,
            "empty catalog should contain only a header")
    for ordinal, (name, version) in enumerate((("math", "1.2.0"), ("bytes", "1.0.0"), ("math", "1.10.0"))):
        project = root / f"package-{ordinal}"
        (project / "src").mkdir(parents=True)
        (project / "oxid.toml").write_text(
            f'[project]\nname = "{name}"\nversion = "{version}"\n'
            'edition = "typed-preview"\nkind = "lib"\nentry = "src/lib.ox"\n'
        )
        (project / "src/lib.ox").write_text("pub fn value() -> i32 { return 42; }\n")
        invoke(compiler, project, environment, ["lock", "--offline"])
        invoke(compiler, project, environment, ["publish", "--registry", source])
    invoke(compiler, root, environment, ["registry", "yank", source, "math", "1.10.0"])
    body = invoke(compiler, root, environment, ["registry", "catalog", source]).stdout
    require(body == HEADER + b"bytes 1.0.0\nmath 1.2.0\n", "catalog exported a stale or yanked version")
    return body


@contextmanager
def serve(root, catalog):
    private_key = root / "key.pem"
    certificate = root / "cert.pem"
    subprocess.run([
        "openssl", "req", "-new", "-newkey", "rsa:2048", "-days", "1", "-x509", "-nodes",
        "-subj", "/CN=127.0.0.1", "-addext", "subjectAltName=IP:127.0.0.1",
        "-keyout", str(private_key), "-out", str(certificate),
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30, check=True)
    requests = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            authorization = self.headers.get("Authorization")
            requests.append((self.path, authorization))
            if self.path == "/public":
                self.respond(200, catalog)
            elif not authorization:
                self.respond(401)
            elif authorization != "Bearer " + TOKEN:
                self.respond(403)
            elif self.path == "/catalog":
                self.respond(200, catalog)
            elif self.path == "/redirect":
                self.respond(302, extra=("Location", "/secret-target"))
            elif self.path == "/large":
                self.respond(200, b"x" * (1024 * 1024 + 1))
            elif self.path == "/many":
                packages = "".join(f"package-{number:04} 1.0.0\n" for number in range(4097))
                self.respond(200, HEADER + packages.encode())
            elif self.path == "/malformed":
                self.respond(200, HEADER + b"bytes 1.0.0\nbytes 2.0.0\n")
            else:
                self.respond(404)

        def respond(self, status, body=b"", extra=None):
            self.send_response(status)
            self.send_header("Content-Length", str(len(body)))
            if extra:
                self.send_header(*extra)
            self.end_headers()
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                pass

    with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(certificate, private_key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        server_thread = threading.Thread(target=server.serve_forever)
        server_thread.start()
        try:
            yield f"https://127.0.0.1:{server.server_port}", certificate, requests
        finally:
            server.shutdown()
            server_thread.join()


def verify(compiler, root):
    environment = dict(os.environ, NO_PROXY="127.0.0.1", no_proxy="127.0.0.1")
    for key in ("CURL_CA_BUNDLE", "SSL_CERT_FILE", "SSL_CERT_DIR", "OXID_REGISTRY_TOKEN"):
        environment.pop(key, None)
    settings = root / "curl-config"
    settings.mkdir()
    (settings / ".curlrc").write_text("location\n")
    environment["CURL_HOME"] = str(settings)
    catalog = export(compiler, root, environment)
    with serve(root, catalog) as (base, certificate, requests):
        trusted = dict(environment, CURL_CA_BUNDLE=str(certificate))
        authorized = dict(trusted, OXID_REGISTRY_TOKEN=TOKEN)

        def search(path, env=authorized, prefix=None, status=0, token_env="OXID_REGISTRY_TOKEN"):
            args = ["registry", "search", base + path]
            if prefix:
                args.append(prefix)
            if token_env:
                args.extend(["--token-env", token_env])
            return invoke(compiler, root, env, args, status=status)

        require(search("/catalog").stdout == b"bytes 1.0.0\nmath 1.2.0\n", "complete search failed")
        require(search("/catalog", prefix="ma").stdout == b"math 1.2.0\n", "prefix search failed")
        require(search("/catalog", prefix="nomatch").stdout == b"No packages match.\n", "empty search failed")
        public = search("/public", authorized, token_env=None)
        require(public.stdout == b"bytes 1.0.0\nmath 1.2.0\n" and requests[-1] == ("/public", None),
                "public search sent an inherited token")
        result = search("/catalog", authorized, status=1, token_env=None)
        require(b"authorization failed" in result.stderr and requests[-1] == ("/catalog", None),
                "curl received a token without an explicit option")
        result = search("/catalog", dict(trusted, OXID_REGISTRY_TOKEN="wrong-token"), status=1)
        require(b"authorization failed" in result.stderr and not result.stdout, "missing authentication error")
        before = len(requests)
        result = search("/catalog", trusted, status=1)
        require(b"variable is unset" in result.stderr and len(requests) == before, "unset token reached the registry")
        result = search("/catalog", authorized, status=1, token_env="invalid=variable")
        require(b"variable name" in result.stderr and len(requests) == before, "invalid variable reached the registry")
        result = search("/catalog", dict(environment, OXID_REGISTRY_TOKEN=TOKEN), status=1)
        require(b"certificate" in result.stderr and len(requests) == before, "untrusted TLS reached the registry")
        result = search("/catalog", dict(trusted, OXID_REGISTRY_TOKEN="bad\nheader"), status=1)
        require(b"valid bearer token" in result.stderr and len(requests) == before, "invalid token reached the registry")
        result = search("/redirect", status=1)
        require(b"HTTP 302" in result.stderr and len(requests) == before + 1, "redirect was followed")
        require(b"exceeds 1 MiB" in search("/large", status=1).stderr, "oversized catalog was accepted")
        require(b"4096-package limit" in search("/many", prefix="math", status=1).stderr,
                "search skipped validation of nonmatching packages")
        require(b"unique and sorted" in search("/malformed", status=1).stderr, "duplicate names were accepted")
        require(requests[0] == ("/catalog", "Bearer " + TOKEN), "request did not authenticate")
    print("HTTPS catalog export, authentication, TLS, filtering, redirect and response limits passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="oxid-registry-https-") as directory:
        verify(args.compiler.resolve(strict=True), Path(directory))


if __name__ == "__main__":
    main()
