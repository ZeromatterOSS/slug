def _default_fields(available):
    default_key = "DefaultInfo"
    default_info = available[default_key]
    return "default_present=%s default_files=%d" % (
        "true" if default_key in available else "false",
        len(default_info.files.to_list()),
    )

def format(target):
    available = providers(target)
    custom_key = "//:defs.bzl%CustomInfo"
    summary_key = "//:defs.bzl%SummaryInfo"
    if summary_key in available:
        summary = available[summary_key]
        return "label=%s provider=SummaryInfo implicit=%s explicit=%s implicit_default_files=%d explicit_default_files=%d %s" % (
            target.label,
            summary.implicit_value,
            summary.explicit_value,
            summary.implicit_files,
            summary.explicit_files,
            _default_fields(available),
        )
    return "label=%s provider=CustomInfo value=%s %s" % (
        target.label,
        available[custom_key].value,
        _default_fields(available),
    )
