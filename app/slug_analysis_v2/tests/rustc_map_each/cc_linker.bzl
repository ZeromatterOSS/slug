# Authored boundary input, not production toolchain construction. All called
# creator/legacy/configure/link-variable/Rust linker functions remain unchanged.
load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "env_entry", "env_set", "feature", "flag_group", "flag_set", "tool_path")
load("@rules_cc//cc/private:cc_internal.bzl", "cc_internal")
load("@rules_cc//cc/private/toolchain_config:cc_toolchain_config_info.bzl", "create_cc_toolchain_config_info")
load("@rules_cc//cc/private/toolchain_config:configure_features.bzl", "configure_features")
load("//rust/private:rustc.bzl", "get_linker_and_args")

LinkerProof = provider(fields = ["rows", "linker", "configuration"])
ACTIONS = [
    "c++-link-executable",
    "c++-link-nodeps-dynamic-library",
    "c++-link-static-library",
    "c++-link-dynamic-library",
]

def _impl(ctx):
    # Mark each action distinctly without replacing its authentic legacy config.
    # Archive user flags expose the Rust helper's is_using_linker=True default.
    flags = [flag_set(actions = [a], flag_groups = [flag_group(flags = ["--action=" + a])]) for a in ACTIONS]
    flags.append(flag_set(actions = ["c++-link-static-library"], flag_groups = [
        flag_group(iterate_over = "user_link_flags", flags = ["--static-user=%{user_link_flags}"]),
    ]))
    probe = feature(name = "link_probe", enabled = True, flag_sets = flags,
        env_sets = [env_set(actions = [a], env_entries = [
            env_entry(key = "ACTION", value = a), env_entry(key = "LIB", value = "one;two"),
        ]) for a in ACTIONS])
    info = create_cc_toolchain_config_info(ctx = ctx, toolchain_identifier = "rust-link-proof",
        compiler = "gcc", target_libc = "local", features = [probe],
        tool_paths = [tool_path(name = n, path = "bin/" + n) for n in ["gcc", "ar", "strip"]])
    model = cc_internal.cc_toolchain_features(toolchain_config_info = info, tools_directory = "toolchain")
    envelope = struct(
        _supports_header_parsing = False,
        _cc_info = struct(compilation_context = struct(_module_map = None)),
        _toolchain_features = model, _is_tool_configuration = False,
        _toolchain_label = ctx.label,
        _fdo_context = struct(branch_fdo_profile = None, propeller_optimize_info = None),
        _cpp_configuration = ctx.fragments.cpp,
        _build_variables_dict = {"legacy_link_flags": ["--legacy"], "sysroot": "/sysroot"},
    )
    configuration = configure_features(ctx = ctx, cc_toolchain = envelope,
        requested_features = ctx.features, unsupported_features = ctx.disabled_features)
    linker = ctx.attr.src[DefaultInfo].files.to_list()[0]
    rust = struct(linker_preference = ctx.attr.preference, linker = linker, linker_type = ctx.attr.driver)
    # Upstream uses substring membership in ("dylib"), so "lib" takes NODEPS.
    cases = [(c, False) for c in ["bin", "dylib", "lib", "staticlib", "cdylib", "proc-macro"]] + [("rlib", True)]
    if ctx.attr.invalid:
        cases = [(ctx.attr.invalid, False)]
    rows = {}
    for crate_type, binary_override in cases:
        tool, direct, args, env = get_linker_and_args(ctx, crate_type, rust, envelope,
            configuration, depset(["runtime", "other", "runtime"]), add_flags_for_binary = binary_override)
        rows[crate_type] = struct(tool = tool, direct = direct, args = args, env = env)
    return [LinkerProof(rows = rows, linker = linker, configuration = configuration)]

subject = rule(implementation = _impl, fragments = ["cpp"], attrs = {
    "compiler": attr.string(default = "gcc"),
    "src": attr.label(default = "@@//:lib.rs", allow_single_file = True),
    "preference": attr.string(default = "cc"), "driver": attr.string(default = "direct"),
    "invalid": attr.string(),
})
