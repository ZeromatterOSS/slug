# Simplified BUILD for Kuro - scripts available as source targets
# In Buck2/Kuro, source files in a package are automatically targets
# The target //pkg:file.sh resolves to the source file file.sh

# NOTE: This is an empty BUILD file. Source files like collect_cc_coverage.sh
# should be accessible as @bazel_tools//tools/test:collect_cc_coverage.sh
# rules_cc references :collect_cc_coverage (without .sh extension) so we need an alias
# But since alias() is a stub, this doesn't work yet.

# Export these files to make them visible (stub - doesn't actually do anything yet)
exports_files([
    "test-setup.sh",
    "generate-xml.sh",
    "collect_coverage.sh",
    "collect_cc_coverage.sh",
    "dummy.sh",
])
