# Plan 28.4 Stage 8 acceptance fixture for the
# `ctx.coverage_instrumented` migration. The Rust impl was deleted;
# the bundled `_kuro_coverage_instrumented` in
# `@kuro_builtins//:exports.bzl` now serves the call by reading the
# kuro-internal `kuro_collect_code_coverage()` global registered in
# `app/kuro_interpreter_for_build/src/interpreter/functions/kuro_runtime.rs`.
#
# When the test runs without `--collect_code_coverage`, the flag is
# its default (`false`) and both the no-arg and with-dep call shapes
# must return `False`. The Rust impl ignored `dep`, so the migrated
# version must too.
#
# We don't exercise the `--collect_code_coverage=true` branch here
# because there's no clean way to invoke `kuro build` from a fixture
# rule with custom flags. The pre-existing `test_collect_code_coverage`
# tests in `test_native_rules.py` cover the toggle path; this fixture
# pins behavioural parity for the default-off path through the
# Starlark facade.

def _coverage_instrumented_proof_impl(ctx):
    no_arg = ctx.coverage_instrumented()
    if no_arg != False:
        fail("Plan 28.4 Stage 8: ctx.coverage_instrumented() default = %r, want False" % no_arg)

    # `dep` is accepted but ignored. Pass `None` (default) explicitly
    # plus a non-None placeholder; both must return the same result.
    dep_none = ctx.coverage_instrumented(None)
    if dep_none != False:
        fail("Plan 28.4 Stage 8: ctx.coverage_instrumented(None) = %r, want False" % dep_none)

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "coverage-instrumented-proof-ok\n")
    return [DefaultInfo(default_output = out)]

coverage_instrumented_proof = rule(
    implementation = _coverage_instrumented_proof_impl,
    attrs = {},
)
