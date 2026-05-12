#!/bin/bash
# Sync bazel_tools from upstream Bazel repository
#
# This script downloads the tools/ directory from Bazel's repository
# and places it in the bazel_tools/ directory at the project root.
# These files are bundled into the slug binary as @bazel_tools.
#
# Usage: ./scripts/sync_bazel_tools.sh [VERSION]
#   VERSION defaults to 9.0.0

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BAZEL_TOOLS_DIR="${PROJECT_ROOT}/bazel_tools"

# Use specified version or default to 9.0.0
BAZEL_VERSION="${1:-9.0.0}"

echo "==> Syncing bazel_tools from Bazel ${BAZEL_VERSION}"

# Clean up any existing directory
if [ -d "${BAZEL_TOOLS_DIR}" ]; then
    echo "==> Removing existing bazel_tools directory"
    rm -rf "${BAZEL_TOOLS_DIR}"
fi

# Create temp directory for clone
TEMP_DIR="$(mktemp -d)"
trap "rm -rf ${TEMP_DIR}" EXIT

echo "==> Cloning Bazel repository (sparse checkout)..."
cd "${TEMP_DIR}"

# Use sparse checkout to get only the tools directory
git clone --depth 1 --filter=blob:none --sparse \
    --branch "${BAZEL_VERSION}" \
    https://github.com/bazelbuild/bazel.git bazel-src 2>&1 || {
    echo "Failed to clone with tag ${BAZEL_VERSION}, trying as branch..."
    git clone --depth 1 --filter=blob:none --sparse \
        https://github.com/bazelbuild/bazel.git bazel-src 2>&1
}

cd bazel-src

# Configure sparse checkout for tools directory only
git sparse-checkout set tools

echo "==> Copying tools/ to bazel_tools/tools/"
mkdir -p "${BAZEL_TOOLS_DIR}/tools"
cp -r tools/* "${BAZEL_TOOLS_DIR}/tools/"

# Create a MODULE.bazel for the bundled cell
cat > "${BAZEL_TOOLS_DIR}/MODULE.bazel" << 'EOF'
# Bundled @bazel_tools repository
# Synced from https://github.com/bazelbuild/bazel
module(name = "bazel_tools")
EOF

# Create an empty BUILD.bazel at the root
cat > "${BAZEL_TOOLS_DIR}/BUILD.bazel" << 'EOF'
# Root BUILD file for @bazel_tools
EOF

# Create a .buckconfig for cell recognition
cat > "${BAZEL_TOOLS_DIR}/.buckconfig" << 'EOF'
[cells]
bazel_tools = .
EOF

echo "==> Counting files..."
FILE_COUNT=$(find "${BAZEL_TOOLS_DIR}" -type f | wc -l)
echo "==> Synced ${FILE_COUNT} files to bazel_tools/"

# Verify critical files exist
CRITICAL_FILES=(
    "tools/build_defs/repo/http.bzl"
    "tools/cpp/toolchain_utils.bzl"
)

echo "==> Verifying critical files..."
for file in "${CRITICAL_FILES[@]}"; do
    if [ -f "${BAZEL_TOOLS_DIR}/${file}" ]; then
        echo "    OK: ${file}"
    else
        echo "    MISSING: ${file}"
        exit 1
    fi
done

echo "==> Done! bazel_tools synced successfully."
