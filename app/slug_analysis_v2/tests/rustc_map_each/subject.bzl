load("@rules_rust//rust/private:rustc.bzl", "construct_arguments")

_NESTED_ROOT_LABEL = Label("@@//nested:unused")
_NESTED_EXTERNAL_LABEL = Label("//nested:unused")

def _no_coverage():
    return False

def _impl(ctx):
    output = ctx.actions.declare_file("probe.rlib")
    if not ctx.attr.src.is_source or output.is_source:
        fail("File.is_source must distinguish source and generated artifacts")
    if "is_source" not in dir(ctx.attr.src) or "is_source" not in dir(output):
        fail("File.is_source must be discoverable")
    if ctx.attr.src.label.workspace_root != "" or ctx.label.workspace_root != "external/rules_rust+":
        fail("Label.workspace_root must use the canonical repository, without the package")
    if "workspace_root" not in dir(ctx.label):
        fail("Label.workspace_root must be discoverable")
    if _NESTED_ROOT_LABEL.workspace_root != "" or _NESTED_EXTERNAL_LABEL.workspace_root != "external/rules_rust+":
        fail("Label.workspace_root must ignore package and target components")
    # Exercise the argument-builder API with a real source File and native
    # actions. These explicit inputs select only the admitted callback slice.
    argument_ctx = struct(
        actions = ctx.actions,
        attr = struct(),
        label = ctx.label,
        executable = struct(_process_wrapper = ctx.attr.src),
        var = {"COMPILATION_MODE": "dbg"},
        genfiles_dir = struct(path = "genfiles"),
        configuration = struct(coverage_enabled = False),
        coverage_instrumented = _no_coverage,
    )
    toolchain = struct(
        target_arch = "x86_64",
        target_os = "linux",
        target_flag_value = "x86_64-unknown-linux-gnu",
        compilation_mode_opts = {"dbg": struct(opt_level = "0", debug_info = "2", strip_level = "none")},
        rust_std = depset(),
        lto = struct(mode = "manual"),
        _codegen_units = 0,
        coverage_supported = False,
        _experimental_link_std_dylib = False,
        _toolchain_generated_sysroot = False,
        _rename_first_party_crates = False,
        extra_rustc_flags_for_crate_types = {},
        extra_exec_rustc_flags = [],
        extra_rustc_flags = [],
        _no_std = "off",
        env = {},
    )
    crate = struct(
        root = ctx.attr.src,
        root_path = "unused-for-regular-file.rs",
        name = "probe",
        type = "rlib",
        output = output,
        rustc_output = None,
        rustc_env = {},
        compile_data_targets = depset(),
        edition = "2021",
        wrapped_crate_type = None,
        is_test = False,
    )
    args, env = construct_arguments(
        ctx = argument_ctx,
        attr = struct(name = "probe"),
        file = struct(),
        toolchain = toolchain,
        cc_toolchain = None,
        feature_configuration = None,
        crate_info = crate,
        dep_info = struct(transitive_build_infos = depset(), direct_crates = depset(), transitive_crates = depset()),
        linkstamp_outs = [],
        ambiguous_libs = {},
        output_hash = None,
        rust_flags = [],
        out_dir = None,
        build_env_files = [],
        build_flags_files = depset(),
        tool_path = "rustc",
        emit = [],
        remap_path_prefix = None,
        skip_expanding_rustc_env = True,
    )
    ctx.actions.run(outputs = [output], executable = "process_wrapper", arguments = args.all, inputs = [ctx.attr.src], env = env)
    return [DefaultInfo(files = depset([output]))]

subject = rule(implementation = _impl, attrs = {"src": attr.label(allow_single_file = True)})
