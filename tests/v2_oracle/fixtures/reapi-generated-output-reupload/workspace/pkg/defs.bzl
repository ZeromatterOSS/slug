def _producer_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(outputs = [out], command = "printf produced > $1", arguments = [out.path])
    return [DefaultInfo(files = depset([out]))]

producer_rule = rule(implementation = _producer_impl)

def _consumer_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [out],
        inputs = [ctx.file.src],
        command = "cat $1 > $2",
        arguments = [ctx.file.src.path, out.path],
    )
    return [DefaultInfo(files = depset([out]))]

consumer_rule = rule(
    implementation = _consumer_impl,
    attrs = {"src": attr.label(allow_single_file = True)},
)