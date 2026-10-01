# Install Oxid

Choose a release binary to run Oxid without installing Rust, or build the current checkout from source. Source builds need stable Rust and a C/C++ compiler. Python 3.11 or later is also needed for repository verification.

## Release archives

1. Open [GitHub Releases](https://github.com/YanagiKH/Oxid/releases) and choose a release.
2. Download the archive for your platform and its adjacent `.sha256` file.
3. Verify the archive checksum, extract `oxid` or `oxid.exe`, and put it on `PATH`.
4. Run `oxid --version`.

The release workflow targets Linux x86_64, Windows x86_64, macOS x86_64, and macOS arm64. Check the selected release for available assets; Linux arm64 is not covered by that workflow.

A release binary includes the interpreter and project tools. Python, Java, Go, and other external process adapters still need their respective runtimes or toolchains.

## Linux and macOS installer

The repository installer downloads a release, checks its SHA-256 checksum, and installs it in `$HOME/.local/bin` by default. Review [install.sh](../install.sh) before running it:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/YanagiKH/Oxid/main/install.sh -o install-oxid.sh
sh install-oxid.sh
export PATH="$HOME/.local/bin:$PATH"
oxid --version
```

Set `OXID_INSTALL_DIR` to choose another directory or `OXID_VERSION` to pin a release tag. Without `OXID_VERSION`, the installer uses the latest release. The Unix installer supports Linux x86_64 and macOS x86_64/arm64.

## Windows installer

Review [install.ps1](../install.ps1), then run it in PowerShell:

```powershell
Invoke-WebRequest https://raw.githubusercontent.com/YanagiKH/Oxid/main/install.ps1 -OutFile install-oxid.ps1
.\install-oxid.ps1
& "$env:LOCALAPPDATA\Oxid\bin\oxid.exe" --version
```

The installer checks the archive checksum and supports Windows x86_64. `OXID_INSTALL_DIR` and `OXID_VERSION` override the installation directory and release tag. If your machine's script policy prevents execution, use a release archive or follow your organization's policy.

## Cargo installation

With stable Rust and a C/C++ compiler installed:

```bash
cargo install --git https://github.com/YanagiKH/Oxid --locked
oxid --version
```

This builds the repository's default branch. For reproducible builds, use a reviewed checkout at a specific commit and run `cargo install --path . --locked`.

## Build a checkout

```bash
git clone https://github.com/YanagiKH/Oxid.git
cd Oxid
cargo build --release --locked
./target/release/oxid run examples/hello.ox
```

On Windows, use `.\target\release\oxid.exe` for the executable. To check a source contribution, follow [Contributing](../CONTRIBUTING.md).

## Docker

From a repository checkout with Docker installed:

```bash
docker build -t oxid .
docker run --rm oxid --version
docker run --rm -v "$PWD:/workspace" oxid run /workspace/examples/hello.ox
```

The image builds the Rust host implementation and runs as a non-root user. The bind-mount example uses a Unix-like shell.

Next: [create and run a project](QUICKSTART.md).
