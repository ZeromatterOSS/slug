# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@fbcode_macros//build_defs:platform_utils.bzl", "platform_utils")
load("@fbcode_macros//build_defs/lib:oss.bzl", "translate_target")
load("@prelude//decls:common.bzl", "buck")
load("@prelude//os_lookup:defs.bzl", "Os", "OsLookup")

def _kuro_bundle_impl(ctx: AnalysisContext) -> list[Provider]:
    """
    Produce a directory layout that is similar to the one our release binary
    uses, this allows setting a path for Tpx relative to BUCK2_BINARY_DIR.
    """
    target_is_windows = ctx.attrs._target_os_type[OsLookup].os == Os("windows")

    binary_extension = ".exe" if target_is_windows else ""
    kuro_binary = "kuro" + binary_extension
    kuro_tpx_binary = "kuro-tpx" + binary_extension
    kuro_daemon_binary = "kuro-daemon" + binary_extension
    kuro_health_check_binary = "kuro-health-check" + binary_extension

    copied_dir = {}
    materialisations = []

    kuro = ctx.attrs.kuro[DefaultInfo].default_outputs[0]
    copied_dir[kuro_daemon_binary] = kuro
    materialisations.extend(ctx.attrs.kuro[DefaultInfo].other_outputs)

    kuro_client = ctx.attrs.kuro_client[DefaultInfo].default_outputs[0]
    copied_dir[kuro_binary] = kuro_client
    materialisations.extend(ctx.attrs.kuro_client[DefaultInfo].other_outputs)

    if ctx.attrs.kuro_health_check:
        kuro_health_check = ctx.attrs.kuro_health_check[DefaultInfo].default_outputs[0]
        copied_dir[kuro_health_check_binary] = kuro_health_check
        materialisations.extend(ctx.attrs.kuro_health_check[DefaultInfo].other_outputs)

    if ctx.attrs.tpx:
        tpx = ctx.attrs.tpx[DefaultInfo].default_outputs[0]
        copied_dir[kuro_tpx_binary] = ctx.actions.symlink_file(kuro_tpx_binary, tpx)
        materialisations.extend(ctx.attrs.tpx[DefaultInfo].other_outputs)

    out = ctx.actions.copied_dir("out", copied_dir)

    return [DefaultInfo(out, other_outputs = materialisations), RunInfo(cmd_args(out.project("kuro" + binary_extension), hidden = materialisations))]

_kuro_bundle = rule(
    impl = _kuro_bundle_impl,
    attrs = {
        "kuro": attrs.dep(),
        "kuro_client": attrs.dep(),
        "kuro_health_check": attrs.option(attrs.dep(), default = None),
        "labels": attrs.list(attrs.string(), default = []),
        "tpx": attrs.option(attrs.dep(), default = None),
        "_target_os_type": buck.target_os_type_arg(),
    },
)

def kuro_bundle(kuro, kuro_client, kuro_health_check, tpx, **kwargs):
    cxx_platform = platform_utils.get_cxx_platform_for_base_path(native.package_name())
    _kuro_bundle(
        kuro = translate_target(kuro),
        kuro_client = translate_target(kuro_client),
        # @oss-disable[end= ]: kuro_health_check = kuro_health_check,
        # @oss-disable[end= ]: tpx = tpx,
        default_target_platform = cxx_platform.target_platform,
        **kwargs
    )
