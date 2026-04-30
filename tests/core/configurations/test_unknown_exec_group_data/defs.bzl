"""Plan 24 Phase 4: rule that requests an undeclared exec_group from
`actions.run`. The action layer must error with the valid (empty) list."""

def _bad_exec_group_impl(ctx):
    out = ctx.actions.declare_output("out.txt")

    # The rule below declares no `exec_groups`, so any non-empty
    # `exec_group` argument is a typo and must error.
    ctx.actions.run(
        ["touch", out.as_output()],
        category = "touch",
        exec_group = "nonexistent",
    )
    return [DefaultInfo(default_output = out)]

bad_exec_group = rule(
    impl = _bad_exec_group_impl,
    attrs = {},
)
