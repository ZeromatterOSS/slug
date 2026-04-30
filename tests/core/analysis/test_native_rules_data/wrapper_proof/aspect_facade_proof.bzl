# Plan 28.4 Stage 4 acceptance fixture. Mirrors `facade_proof.bzl`
# (Stage 3) but for the aspect dispatch path. Two checks fold into one
# build:
#
#   1. Inside the aspect impl, `ctx.kuro_facade_active == True` and
#      `ctx.kuro_facade_kind == "aspect"`. The kind disambiguator
#      proves the aspect-side wrapper (`_invoke_aspect`) ran rather
#      than the rule-side wrapper.
#
#   2. `ctx.target_platform_has_constraint(...)` answers correctly
#      from inside an aspect. Stage 3 deleted the Rust impls on both
#      `AnalysisContext` and `AspectContext`; the aspect facade
#      reuses the same Starlark shim. The previous Rust stub returned
#      `False` unconditionally — anything truthful here is proof the
#      shim is wired in.

_HOST_OS_LABEL = "@platforms//os:linux"
_NON_HOST_OS_LABEL = "@platforms//os:windows"

FacadeAspectInfo = provider(fields = [
    "facade_active",
    "facade_kind",
    "matched_host",
    "matched_non_host",
])

def _facade_aspect_impl(target, ctx):
    matching = platform_common.ConstraintValueInfo(
        label = _HOST_OS_LABEL,
        constraint_setting = "@platforms//os:os",
    )
    non_matching = platform_common.ConstraintValueInfo(
        label = _NON_HOST_OS_LABEL,
        constraint_setting = "@platforms//os:os",
    )
    return [FacadeAspectInfo(
        facade_active = getattr(ctx, "kuro_facade_active", False),
        facade_kind = getattr(ctx, "kuro_facade_kind", ""),
        matched_host = ctx.target_platform_has_constraint(matching),
        matched_non_host = ctx.target_platform_has_constraint(non_matching),
    )]

facade_aspect = aspect(implementation = _facade_aspect_impl)

def _aspect_facade_leaf_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "aspect-facade-leaf\n")
    return [DefaultInfo(default_output = out)]

aspect_facade_leaf = rule(
    implementation = _aspect_facade_leaf_impl,
    attrs = {},
)

def _aspect_facade_collector_impl(ctx):
    for d in ctx.attrs.deps:
        if FacadeAspectInfo not in d:
            fail("Plan 28.4 Stage 4: aspect did not run on dep %s" % d.label)
        info = d[FacadeAspectInfo]
        if not info.facade_active:
            fail("Plan 28.4 Stage 4: ctx.kuro_facade_active missing inside aspect impl on %s" % d.label)
        if info.facade_kind != "aspect":
            fail("Plan 28.4 Stage 4: ctx.kuro_facade_kind = %r (want \"aspect\") on %s" % (info.facade_kind, d.label))
        if not info.matched_host:
            fail("Plan 28.4 Stage 4: aspect target_platform_has_constraint returned False for host OS %s" % _HOST_OS_LABEL)
        if info.matched_non_host:
            fail("Plan 28.4 Stage 4: aspect target_platform_has_constraint returned True for non-host OS %s" % _NON_HOST_OS_LABEL)

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "aspect-facade-proof-ok\n")
    return [DefaultInfo(default_output = out)]

aspect_facade_collector = rule(
    implementation = _aspect_facade_collector_impl,
    attrs = {
        "deps": attr.label_list(aspects = [facade_aspect]),
    },
)
