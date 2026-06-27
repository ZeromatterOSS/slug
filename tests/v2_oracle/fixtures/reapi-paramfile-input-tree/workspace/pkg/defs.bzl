def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    args = ctx.actions.args()
    args.add_all(["alpha", "beta gamma"])
    args.use_param_file("@%s", use_always = True)
    ctx.actions.run_shell(
        outputs = [out],
        command = "cat $1 > $2",
        arguments = [args, out.path],
        mnemonic = "ParamfileProbe",
    )
    return [DefaultInfo(files = depset([out]))]

probe_rule = rule(implementation = _impl)