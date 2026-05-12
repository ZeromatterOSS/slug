# Plan 28.3 negative test: `rule_implementation_wrapper` is defined at
# the top of `@slug_builtins//:exports.bzl` but is NOT in the
# `exported_toplevels` dict. Referencing it here must fail at parse
# time — the export contract restricts visibility to listed names only.
_RUNTIME_REFERENCE = rule_implementation_wrapper  # noqa: F821 — must NOT exist
