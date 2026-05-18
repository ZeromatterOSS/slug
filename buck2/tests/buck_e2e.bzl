# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file. You may select, at
# your option, one of the above-listed licenses.

def _python_pytest_impl(ctx):
    if not ctx.files.srcs:
        return [
            DefaultInfo(),
            ExternalRunnerTestInfo(type = "python", command = []),
        ]

    test_file = ctx.files.srcs[0]
    return [
        DefaultInfo(default_output = test_file),
        ExternalRunnerTestInfo(
            type = "python",
            command = [
                "bash",
                "-c",
                "root=\"$PWD\"; while [ ! -x \"$root/target/debug/slug\" ] && [ \"$root\" != / ]; do root=\"${root%/*}\"; done; TEST_EXECUTABLE=\"$root/target/debug/slug\" exec python -m pytest \"$1\"",
                "--",
                test_file,
            ],
            env = ctx.attrs.env,
            labels = ctx.attrs.labels,
            run_from_project_root = True,
            use_project_relative_paths = True,
        ),
    ]

_python_pytest = rule(
    implementation = _python_pytest_impl,
    test = True,
    attrs = {
        "srcs": attr.label_list(allow_files = True, default = []),
        "env": attr.string_dict(default = {}),
        "labels": attr.string_list(default = []),
    },
)

def buck2_e2e_test(
        name,
        srcs = None,
        data_dir = None,
        labels = None,
        env = None,
        **_kwargs):
    _unused = data_dir
    env = dict(env or {})
    env.setdefault("RUST_BACKTRACE", "1")
    env.setdefault("RUST_LIB_BACKTRACE", "0")
    env.setdefault("BUCK2_E2E_TEST_FLAVOR", "isolated")
    env.setdefault("BUCK2_RUNTIME_THREADS", "8")
    env.setdefault("BUCK2_MAX_BLOCKING_THREADS", "8")

    _python_pytest(
        name = name,
        srcs = srcs or [],
        env = env,
        labels = labels or [],
    )

def buck2_core_tests(extra_attrs = {}, target_extra_attrs = {}):
    _unused = (extra_attrs, target_extra_attrs)
