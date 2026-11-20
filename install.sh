#!/usr/bin/env sh
set -eu

REPOSITORY="YanagiKH/Oxid"
INSTALL_DIR="${OXID_INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${OXID_VERSION:-latest}"
VERIFY_SIGNATURE="${OXID_VERIFY_SIGNATURE:-0}"

case "$VERIFY_SIGNATURE" in
  0) ;;
  1)
    case "$VERSION" in
      *[!0-9A-Za-z.+-]*) echo "Signature verification requires a specific release tag in OXID_VERSION." >&2; exit 1 ;;
    esac
    if ! printf '%s\n' "$VERSION" | grep -Eq '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'; then
      echo "Signature verification requires a specific release tag in OXID_VERSION." >&2
      exit 1
    fi
    command -v gh >/dev/null 2>&1 || { echo "Install a recent GitHub CLI to verify the signature." >&2; exit 1; }
    ;;
  *) echo "OXID_VERIFY_SIGNATURE must be 0 or 1." >&2; exit 1 ;;
esac

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

ASSET="oxid-${PLATFORM}-${ARCH}.tar.gz"
if [ "$VERSION" = "latest" ]; then
  BASE_URL="https://github.com/${REPOSITORY}/releases/latest/download"
else
  BASE_URL="https://github.com/${REPOSITORY}/releases/download/${VERSION}"
fi

TEMP_DIR="$(mktemp -d)"
STAGED=""
cleanup() {
  if [ -n "$STAGED" ]; then rm -f "$STAGED"; fi
  rm -rf "$TEMP_DIR"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

if [ -n "${OXID_ARCHIVE:-}" ]; then
  case "$OXID_ARCHIVE" in
    /*) LOCAL_ARCHIVE="$OXID_ARCHIVE" ;;
    *) LOCAL_ARCHIVE="$(pwd)/$OXID_ARCHIVE" ;;
  esac
  cp "$LOCAL_ARCHIVE" "${TEMP_DIR}/${ASSET}"
  cp "${LOCAL_ARCHIVE}.sha256" "${TEMP_DIR}/${ASSET}.sha256"
else
  curl --fail --location --silent --show-error "${BASE_URL}/${ASSET}" --output "${TEMP_DIR}/${ASSET}"
  curl --fail --location --silent --show-error "${BASE_URL}/${ASSET}.sha256" --output "${TEMP_DIR}/${ASSET}.sha256"
fi

EXPECTED="$(awk 'NR == 1 { print $1; exit }' "${TEMP_DIR}/${ASSET}.sha256" | tr 'A-F' 'a-f')"
case "$EXPECTED" in
  ''|*[!0-9a-f]*) echo "Invalid SHA-256 checksum." >&2; exit 1 ;;
esac
[ "${#EXPECTED}" -eq 64 ] || { echo "Invalid SHA-256 checksum." >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL="$(sha256sum "${TEMP_DIR}/${ASSET}" | awk '{ print $1 }')"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL="$(shasum -a 256 "${TEMP_DIR}/${ASSET}" | awk '{ print $1 }')"
else
  echo "A SHA-256 utility is required." >&2
  exit 1
fi
[ "$EXPECTED" = "$ACTUAL" ] || { echo "Checksum verification failed." >&2; exit 1; }

if [ "$VERIFY_SIGNATURE" = 1 ]; then
  BUNDLE="${TEMP_DIR}/${ASSET}.sigstore.json"
  if [ -n "${OXID_ARCHIVE:-}" ]; then
    cp "${LOCAL_ARCHIVE}.sigstore.json" "$BUNDLE"
  else
    curl --fail --location --silent --show-error "${BASE_URL}/${ASSET}.sigstore.json" --output "$BUNDLE"
  fi
  SUBJECT_CHECK="any(.[] | .verificationResult.statement.subject[]?; .name == \"${ASSET}\" and .digest.sha256 == \"${ACTUAL}\")"
  set -- gh attestation verify "${TEMP_DIR}/${ASSET}" --hostname github.com --repo "$REPOSITORY" \
    --signer-workflow "${REPOSITORY}/.github/workflows/release.yml" --source-ref "refs/tags/${VERSION}" \
    --cert-oidc-issuer https://token.actions.githubusercontent.com --predicate-type https://slsa.dev/provenance/v1 \
    --deny-self-hosted-runners --bundle "$BUNDLE" --format json --jq "$SUBJECT_CHECK"
  if [ -n "${OXID_TRUSTED_ROOT:-}" ]; then
    set -- "$@" --custom-trusted-root "$OXID_TRUSTED_ROOT"
  fi
  if ! MATCHED="$("$@")" || [ "$MATCHED" != true ]; then
    echo "Signature verification failed." >&2
    exit 1
  fi
fi

tar -xzf "${TEMP_DIR}/${ASSET}" -C "$TEMP_DIR" oxid
if [ ! -f "${TEMP_DIR}/oxid" ] || [ -L "${TEMP_DIR}/oxid" ]; then
  echo "The archive must contain a regular oxid executable." >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
if [ -L "${INSTALL_DIR}/oxid" ] || { [ -e "${INSTALL_DIR}/oxid" ] && [ ! -f "${INSTALL_DIR}/oxid" ]; }; then
  echo "The install destination must be a regular file." >&2
  exit 1
fi
STAGED="$(mktemp "${INSTALL_DIR}/.oxid-install.XXXXXX")"
install -m 0755 "${TEMP_DIR}/oxid" "$STAGED"
mv -f "$STAGED" "${INSTALL_DIR}/oxid"
STAGED=""

echo "Installed Oxid to ${INSTALL_DIR}/oxid"
echo "Add ${INSTALL_DIR} to PATH if it is not already available."
