# Plan 28.5 negative test: `_kuro_exported_native_probe` is defined
# in `@kuro_builtins//:exports.bzl::exported_native` but is NOT in
# `exported_toplevels`. Referencing it from an external `.bzl` must
# fail at parse time — the BUCK-only contract restricts visibility.
_RUNTIME_REFERENCE = _kuro_exported_native_probe  # noqa: F821 — must NOT exist
