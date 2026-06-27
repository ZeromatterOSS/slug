load(":message.bzl", "MESSAGE")

def _message_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, MESSAGE + "\n")
    return [DefaultInfo(files = depset([out]))]

message_rule = rule(implementation = _message_impl)