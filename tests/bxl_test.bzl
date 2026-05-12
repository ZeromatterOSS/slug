# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@fbcode//slug/tests:buck_e2e.bzl", "slug_e2e_test")
load("@fbcode_macros//build_defs:export_files.bzl", "export_file")

def bxl_test(src, name = None, labels = None, buck_args: list[str] | None = None, bxl_args: list[str] | None = None, **kwargs):
    """
    Creates a test target from a slug bxl script. BXL script must use "test" as entry
    point.

    Parameters:
        src: source path of BXL script. This cannot be a target since bxl
            can only be invoked from the repo and not from buck-out.
        name: Name of the test target. If unspecified, use src as the name.
        buck_args: Arguments to `slug bxl` invocation for buck specifically.
            Common examples are `--config` flags and `---modifier` flags.
            Ex. buck_args = ["--config", "build.use_limited_hybrid=false"]
        bxl_args: Arguments to `slug bxl` invocation after `--`. These are
            arguments to bxl script specifically.
    """

    if ":" in src:
        fail("`src` cannot be a target. Found `{}` for `src`".format(src))
    if not src.endswith(".bxl"):
        fail("`src` must end in '.bxl'. Found `{}` for `src`".format(src))

    # Need to include `name` to keep this target unique, in case there are multiple bxl_tests defined for same bxl file
    export_file_name = "{}.{}.export_file".format(src, name)
    export_file(name = export_file_name, src = src, mode = "reference")

    # This is ugly but needed for buck1 compatibility
    cell = native.repository_name()[1:]
    base_path = native.package_name()
    bxl_main = "{}//{}/{}:test".format(cell, base_path, src)

    if not name:
        name = src

    env = {
        "BXL_MAIN": bxl_main,
        # This env var is used to properly declare a dep on the src file.
        # I didn't use `resources` or `deps` because attaching to an env var makes debugging easier if needed.
        "_BXL_SRC": "$(location :{})".format(export_file_name),
    }
    if bxl_args:
        env["BXL_ARGS"] = " ".join(bxl_args)
    if buck_args:
        env["BUCK_ARGS"] = " ".join(buck_args)

    slug_e2e_test(
        name = name,
        env = env,
        srcs = {"fbcode//slug/tests/e2e_util:test_bxl_template.py": "test_bxl_template.py"},
        labels = ["bxl_test"] + (labels if labels else []),
        test_with_compiled_slug = False,
        test_with_deployed_slug = True,
        skip_deployed_slug_version_dep = True,
        **kwargs
    )
