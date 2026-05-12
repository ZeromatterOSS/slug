#!/usr/bin/env bash
# Plan 16.8: canary target inventory.
#
# Source this to get BENCH_TARGETS_{SMALL,MEDIUM,LARGE,XL} arrays.
# Consumers pick a size based on available time / CI budget.
#
# The targets are Bazel-shaped labels; they assume
# `/var/mnt/dev/llvm-project/utils/bazel` is checked out and registered
# as a Slug workspace.

# One small cc_library inside the repo — single-digit seconds.
BENCH_TARGETS_SMALL=(
  "@llvm-project//llvm:config"
)

# A medium target — ~100 actions, tens of seconds.
BENCH_TARGETS_MEDIUM=(
  "@llvm-project//clang:analysis_htmllogger_gen"
)

# Large target — ~4k actions, minutes.
BENCH_TARGETS_LARGE=(
  "@llvm-project//clang:clang"
)

# Extra-large target — ~20k actions, tens of minutes. CI gate only.
BENCH_TARGETS_XL=(
  "@llvm-project//llvm:llvm"
)
