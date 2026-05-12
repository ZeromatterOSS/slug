# Plan 28.5 negative test: `_slug_exported_native_probe` is defined
# in `@slug_builtins//:exports.bzl::exported_native` but is NOT in
# `exported_toplevels`. Referencing it from an external `.bzl` must
# fail at parse time — the BUCK-only contract restricts visibility.
_RUNTIME_REFERENCE = _slug_exported_native_probe  # noqa: F821 — must NOT exist
