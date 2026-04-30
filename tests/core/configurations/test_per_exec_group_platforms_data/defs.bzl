"""Plan 24 Phase 8: rule that fans actions across two exec groups with
disjoint `exec_compatible_with` constraints. Each group must resolve to
its matching registered platform — if the per-group resolver is broken
and every group falls back to the default candidate, one of the
constraints will mismatch and the build will fail with
`No compatible execution platform`.
"""

def _two_groups_impl(ctx):
    out_link = ctx.actions.declare_output("link.out")
    out_test = ctx.actions.declare_output("test.out")

    # The default group action — uses no exec_group, so it reuses the
    # rule's default execution platform.
    out_default = ctx.actions.declare_output("default.out")
    ctx.actions.write(out_default, "default")

    # `link` group constrained to linux. With both linux_platform and
    # darwin_platform registered, this must select linux_platform.
    ctx.actions.run(
        ["touch", out_link.as_output()],
        category = "link",
        exec_group = "link",
    )

    # `test` group constrained to darwin. Must select darwin_platform.
    ctx.actions.run(
        ["touch", out_test.as_output()],
        category = "test",
        exec_group = "test",
    )

    return [DefaultInfo(default_outputs = [out_default, out_link, out_test])]

two_groups = rule(
    impl = _two_groups_impl,
    attrs = {},
    exec_groups = {
        "link": exec_group(exec_compatible_with = ["//:linux"]),
        "test": exec_group(exec_compatible_with = ["//:darwin"]),
    },
)
