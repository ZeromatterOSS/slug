# BUILD file for Kuro's bazel_tools test directory
# Provides file targets needed by rules_cc for testing and coverage

load("@bazel_tools//tools/build_rules:filegroup.bzl", "filegroup")

# Test infrastructure files
filegroup(
    name = "test-setup",
    srcs = ["test-setup.sh"],
    visibility = ["PUBLIC"],
)

filegroup(
    name = "generate-xml",
    srcs = ["generate-xml.sh"],
    visibility = ["PUBLIC"],
)

# Coverage collection scripts needed by cc_test
filegroup(
    name = "collect_coverage",
    srcs = ["collect_coverage.sh"],
    visibility = ["PUBLIC"],
)

filegroup(
    name = "collect_cc_coverage",
    srcs = ["collect_cc_coverage.sh"],
    visibility = ["PUBLIC"],
)

filegroup(
    name = "dummy",
    srcs = ["dummy.sh"],
    visibility = ["PUBLIC"],
)
