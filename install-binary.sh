#!/usr/bin/env bash
set -euo pipefail

REPO="thutch-conecrow/urd"
INSTALL_DIR="${URD_INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS
case "$(uname -s)" in
    Linux*)  OS="unknown-linux-gnu" ;;
    Darwin*) OS="apple-darwin" ;;
    *)       echo "Unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

# Detect architecture
case "$(uname -m)" in
    x86_64|amd64)  ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *)             echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"

# Determine version
if [ -n "${1:-}" ]; then
    VERSION="$1"
else
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | head -1 | cut -d'"' -f4)
    if [ -z "$VERSION" ]; then
        echo "Could not determine latest version" >&2
        exit 1
    fi
fi

ARCHIVE="urd-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"

echo "Installing urd ${VERSION} (${TARGET})..."

# Download and extract
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL" -o "${TMPDIR}/${ARCHIVE}"
tar xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"

# Install
mkdir -p "$INSTALL_DIR"
mv "${TMPDIR}/urd" "${INSTALL_DIR}/urd"
chmod +x "${INSTALL_DIR}/urd"

echo "Installed urd to ${INSTALL_DIR}/urd"

# Check if install dir is on PATH
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo ""
    echo "Note: ${INSTALL_DIR} is not on your PATH."
    echo "Add it with:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
