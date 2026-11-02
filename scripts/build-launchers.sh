#!/usr/bin/env bash
# Build launchers for all supported targets.
#
# This script is run in the release workflow to produce prebuilt launchers.
# It uses `cross` for cross-compilation from Linux, and native builds for
# the host platform.
#
# Usage: ./scripts/build-launchers.sh [version]
#
# Outputs: ./dist/wf-launcher-<target>

set -euo pipefail

VERSION="${1:-$(cargo metadata --format-version=1 | jq -r '.packages[] | select(.name=="wf-cli") | .version')}"
WORKSPACE_ROOT="$(git rev-parse --show-toplevel)"
DIST_DIR="${WORKSPACE_ROOT}/dist"

echo "Building launchers for WasmFoundry ${VERSION}"
echo "Workspace: ${WORKSPACE_ROOT}"
echo "Output: ${DIST_DIR}"

mkdir -p "${DIST_DIR}"

# Targets to build (from plan.md §125)
TARGETS=(
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-pc-windows-msvc"
)

# Check if cross is installed
if ! command -v cross &> /dev/null; then
    echo "Installing cross..."
    cargo install cross --locked
fi

# Build for each target
for TARGET in "${TARGETS[@]}"; do
    echo "=== Building for ${TARGET} ==="
    
    OUTPUT_NAME="wf-launcher"
    if [[ "${TARGET}" == *"windows"* ]]; then
        OUTPUT_NAME="wf-launcher.exe"
    fi
    
    OUTPUT_PATH="${DIST_DIR}/${OUTPUT_NAME}-${TARGET}"
    
    if [[ "${TARGET}" == "${HOST_TARGET:-$(rustc -vV | grep host | cut -d' ' -f2)}" ]]; then
        # Native build
        echo "Native build for ${TARGET}"
        cargo build --release --target "${TARGET}" -p wf-launcher 2>&1 | tail -20
        cp "${WORKSPACE_ROOT}/target/${TARGET}/release/${OUTPUT_NAME}" "${OUTPUT_PATH}"
    else
        # Cross-compile
        echo "Cross-compiling for ${TARGET}"
        cross build --release --target "${TARGET}" -p wf-launcher 2>&1 | tail -20
        cp "${WORKSPACE_ROOT}/target/${TARGET}/release/${OUTPUT_NAME}" "${OUTPUT_PATH}"
    fi
    
    # Verify binary runs (basic sanity check)
    if [[ "${TARGET}" == *"linux"* ]] && [[ "$(uname -s)" == "Linux" ]]; then
        if "${OUTPUT_PATH}" --help 2>&1 | grep -q "wf-launcher"; then
            echo "✓ Binary verified"
        else
            echo "⚠ Binary verification failed (may be expected for cross-compiled)"
        fi
    fi
    
    # Generate checksum
    sha256sum "${OUTPUT_PATH}" > "${OUTPUT_PATH}.sha256"
    echo "  Checksum: $(cat "${OUTPUT_PATH}.sha256")"
done

echo ""
echo "=== Build complete ==="
echo "Launchers in ${DIST_DIR}:"
ls -la "${DIST_DIR}"/wf-launcher*

# Generate combined checksums file
echo "=== Generating combined checksums ==="
cd "${DIST_DIR}"
sha256sum wf-launcher-* > checksums.sha256 2>/dev/null || true
cat checksums.sha256

echo ""
echo "Done. Version: ${VERSION}"