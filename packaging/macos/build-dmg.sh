#!/bin/bash
#
# Build GBOS-Agent-macOS.dmg: a disk image containing a .pkg that installs the
# agent as a root LaunchDaemon.
#
# The package installs exactly one file — /usr/local/libexec/gbos-agent — and its
# postinstall runs that binary with --install-service, which is what writes the
# launchd plist and starts the daemon. The plist therefore lives in the binary
# rather than in a second copy here, so a packaged install and a hand-run
# `sudo gbos-agent --install-service` cannot drift apart.
#
# Usage:  ./build-dmg.sh
#
# Environment:
#   SIGN_IDENTITY   Optional. A "Developer ID Installer" identity. When set, the
#                   .pkg is signed, which is half of what Gatekeeper wants; the
#                   other half is notarization, which this does not do.
#   DEVELOPER_DIR   Optional. Only needed on a machine whose Xcode licence has
#                   not been accepted and whose Command Line Tools are
#                   installed separately.

set -euo pipefail

PRODUCT="GBOS Agent"
IDENTIFIER="com.gbaja.gbos.agent"
BINARY_NAME="gbos-agent"
INSTALL_DIR="usr/local/libexec"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGENT_DIR="$(cd "${HERE}/../.." && pwd)"
REPO_DIR="$(cd "${AGENT_DIR}/.." && pwd)"
BUILD_DIR="${AGENT_DIR}/target/packaging"

VERSION="$(sed -nE 's/^version *= *"([^"]+)".*/\1/p' "${AGENT_DIR}/Cargo.toml" | head -1)"

if [ -z "${VERSION}" ]; then
    echo "error: could not read the version from ${AGENT_DIR}/Cargo.toml" >&2
    exit 1
fi

echo "==> Building ${PRODUCT} ${VERSION}"

# Build every Apple target that is actually installed. A universal build needs
# both; with one, the image is single-architecture and says so at the end.
TARGETS=()

for candidate in aarch64-apple-darwin x86_64-apple-darwin; do
    if rustup target list --installed 2>/dev/null | grep -qx "${candidate}"; then
        TARGETS+=("${candidate}")
    fi
done

if [ ${#TARGETS[@]} -eq 0 ]; then
    echo "error: no apple-darwin target is installed." >&2
    echo "       rustup target add aarch64-apple-darwin" >&2
    exit 1
fi

BINARIES=()

for target in "${TARGETS[@]}"; do
    echo "==> cargo build --release --target ${target}"

    ( cd "${AGENT_DIR}" && cargo build --release --target "${target}" )

    BINARIES+=("${AGENT_DIR}/target/${target}/release/agent")
done

# --- Stage ------------------------------------------------------------------

STAGE="${BUILD_DIR}/stage"

rm -rf "${BUILD_DIR}"
mkdir -p "${STAGE}/${INSTALL_DIR}"

DESTINATION="${STAGE}/${INSTALL_DIR}/${BINARY_NAME}"

if [ ${#BINARIES[@]} -gt 1 ]; then
    echo "==> lipo: universal binary"
    lipo -create -output "${DESTINATION}" "${BINARIES[@]}"
else
    # COPYFILE_DISABLE keeps the copy from carrying extended attributes across as
    # AppleDouble sidecar files.
    COPYFILE_DISABLE=1 cp "${BINARIES[0]}" "${DESTINATION}"
fi

chmod 755 "${DESTINATION}"

# macOS 15 marks every file it creates with com.apple.provenance, and that
# attribute is protected: neither `xattr -c` nor `ditto --noextattr` removes it.
# It therefore travels into the payload as a `._gbos-agent` sidecar next to the
# binary, which is what any pkg built on a current macOS contains and is
# harmless. This is here so the attribute is cleared on systems where it is
# ordinary metadata.
xattr -c "${DESTINATION}" 2>/dev/null || true

# --- Component package ------------------------------------------------------

PKG="${BUILD_DIR}/${PRODUCT}.pkg"

echo "==> pkgbuild"

pkgbuild \
    --root "${STAGE}" \
    --identifier "${IDENTIFIER}" \
    --version "${VERSION}" \
    --install-location / \
    --ownership recommended \
    --scripts "${HERE}/scripts" \
    "${PKG}"

if [ -n "${SIGN_IDENTITY:-}" ]; then
    echo "==> productsign: ${SIGN_IDENTITY}"

    productsign --sign "${SIGN_IDENTITY}" "${PKG}" "${PKG}.signed"
    mv "${PKG}.signed" "${PKG}"
else
    echo "    (unsigned — Gatekeeper will ask the user to confirm on first open)"
fi

# --- Disk image -------------------------------------------------------------

DMGROOT="${BUILD_DIR}/dmg"
DMG="${BUILD_DIR}/GBOS-Agent-macOS.dmg"

mkdir -p "${DMGROOT}"
cp "${PKG}" "${DMGROOT}/Install ${PRODUCT}.pkg"

echo "==> hdiutil"

hdiutil create \
    -volname "${PRODUCT}" \
    -srcfolder "${DMGROOT}" \
    -ov \
    -format UDZO \
    "${DMG}" >/dev/null

# --- Hand it to the frontend ------------------------------------------------

DOWNLOADS="${REPO_DIR}/gbos/public/downloads"

if [ -d "${DOWNLOADS}" ]; then
    cp "${DMG}" "${DOWNLOADS}/GBOS-Agent-macOS.dmg"

    echo "==> Copied to ${DOWNLOADS}/GBOS-Agent-macOS.dmg"
else
    echo "    (no ${DOWNLOADS}; the dashboard's download link will 404 until this is copied there)"
fi

echo
echo "Built ${DMG} ($(du -h "${DMG}" | cut -f1))"

if [ ${#BINARIES[@]} -eq 1 ]; then
    echo "Note: ${TARGETS[0]} only. For a universal build:"
    echo "      rustup target add aarch64-apple-darwin x86_64-apple-darwin"
fi

echo
echo "The installer places the agent at /usr/local/libexec/${BINARY_NAME} and runs it"
echo "as a background service. It shows no window and no Dock icon; its log is"
echo "/var/log/gbos-agent.log and its status is in"
echo "  sudo /usr/local/libexec/${BINARY_NAME} --status"
