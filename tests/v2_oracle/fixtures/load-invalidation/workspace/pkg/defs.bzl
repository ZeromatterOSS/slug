load(":message.bzl", "MESSAGE")

print("SLUG_TERMINAL_EVENT_PARENT_BZL")

def _message_impl(ctx):
    if MESSAGE == "one":
        print("SLUG_TERMINAL_EVENT_RULE_MESSAGE_ONE")
    else:
        print("SLUG_TERMINAL_EVENT_RULE_MESSAGE_TWO")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, MESSAGE + "\n")
    return [DefaultInfo(files = depset([out]))]

def _analysis_failure_impl(_ctx):
    print("SLUG_TERMINAL_EVENT_RULE_ANALYSIS_FAILURE_PREFIX")
    fail("SLUG_TERMINAL_" + "EVENT_TERMINAL_ANALYSIS_FAILURE")

def _execution_failure_impl(ctx):
    print("SLUG_TERMINAL_EVENT_RULE_EXECUTION_FAILURE_PREFIX")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.run_shell(
        outputs = [out],
        command = "printf 'SLUG_%s_%s\\n' TERMINAL EVENT_TERMINAL_ACTION_FAILURE_DIAGNOSTIC >&2; exit 23",
    )
    return [DefaultInfo(files = depset([out]))]

message_rule = rule(implementation = _message_impl)
analysis_failure_rule = rule(implementation = _analysis_failure_impl)
execution_failure_rule = rule(implementation = _execution_failure_impl)
