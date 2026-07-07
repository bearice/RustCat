#!/bin/bash

# Linux build script for RustCat
# This script handles all Linux-specific build tasks including:
# - Building the release binary
# - Bundling the binary with a .desktop entry and icon into a tarball
#
# The binary is dynamically linked against glibc. For a fully portable,
# reproducible build use the Nix flake (`nix build .#default`).

set -e

echo "🐧 Starting Linux build process..."

# Configuration
APP_NAME="RustCat"
BIN_NAME="rust_cat"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
ARCH=$(uname -m)
PKG_DIR="${APP_NAME}-${VERSION}-linux-${ARCH}"

echo "📦 Building release binary..."
cargo build --release

# Verify the binary
echo "✅ Verifying binary..."
file target/release/${BIN_NAME}

# Assemble a redistributable package directory
echo "🎨 Assembling package..."
rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}/bin" "${PKG_DIR}/share/applications" "${PKG_DIR}/share/icons"

cp target/release/${BIN_NAME} "${PKG_DIR}/bin/"
cp assets/rustcat.desktop "${PKG_DIR}/share/applications/"
cp assets/appIcon.ico "${PKG_DIR}/share/icons/rustcat.ico"

# A small install/uninstall helper for users not on Nix
cat > "${PKG_DIR}/install.sh" <<'INSTALL_EOF'
#!/bin/bash
# Simple installer: copies the bundled files into ~/.local
set -e
PREFIX="${PREFIX:-${HOME}/.local}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

install -Dm755 "${SCRIPT_DIR}/bin/rust_cat" "${PREFIX}/bin/rust_cat"
install -Dm644 "${SCRIPT_DIR}/share/applications/rustcat.desktop" "${PREFIX}/share/applications/rustcat.desktop"
install -Dm644 "${SCRIPT_DIR}/share/icons/rustcat.ico" "${PREFIX}/share/icons/rustcat.ico"

echo "Installed RustCat to ${PREFIX}. Run with: rust_cat"
INSTALL_EOF
chmod +x "${PKG_DIR}/install.sh"

# Archive it
echo "📦 Creating tarball..."
tar -czf "${PKG_DIR}.tar.gz" "${PKG_DIR}"

echo "✅ Done: ${PKG_DIR}.tar.gz"