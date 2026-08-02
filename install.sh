#!/usr/bin/env sh
set -eu

REPOSITORY="YanagiKH/Oxid"
INSTALL_DIR="${OXID_INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${OXID_VERSION:-latest}"

case "$(uname -s)" in
  Linux) PLATFORM="linux" ;;
  Darwin) PLATFORM="macos" ;;
  *) echo "Unsupported operating system. Use the Windows installer on Windows." >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

if [ "$PLATFORM" = "linux" ] && [ "$ARCH" != "x86_64" ]; then
  echo "Linux release binaries currently support x86_64. Build from source for ${ARCH}." >&2
  exit 1
fi

ASSET="oxid-${PLATFORM}-${ARCH}.tar.gz"
if [ "$VERSION" = "latest" ]; then
  BASE_URL="https://github.com/${REPOSITORY}/releases/latest/download"
else
  BASE_URL="https://github.com/${REPOSITORY}/releases/download/${VERSION}"
fi

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT HUP INT TERM

curl --fail --location --silent --show-error "${BASE_URL}/${ASSET}" --output "${TEMP_DIR}/${ASSET}"
curl --fail --location --silent --show-error "${BASE_URL}/${ASSET}.sha256" --output "${TEMP_DIR}/${ASSET}.sha256"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$TEMP_DIR" && sha256sum -c "${ASSET}.sha256")
elif command -v shasum >/dev/null 2>&1; then
  EXPECTED="$(cut -d ' ' -f 1 "${TEMP_DIR}/${ASSET}.sha256")"
  ACTUAL="$(shasum -a 256 "${TEMP_DIR}/${ASSET}" | cut -d ' ' -f 1)"
  [ "$EXPECTED" = "$ACTUAL" ] || { echo "Checksum verification failed." >&2; exit 1; }
else
  echo "A SHA-256 utility is required." >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
tar -xzf "${TEMP_DIR}/${ASSET}" -C "$TEMP_DIR"
install -m 0755 "${TEMP_DIR}/oxid" "${INSTALL_DIR}/oxid"

echo "Installed Oxid to ${INSTALL_DIR}/oxid"
echo "Add ${INSTALL_DIR} to PATH if it is not already available."
