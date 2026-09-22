# Authored native-caller inputs. All toolchain/provider constructors stay unchanged.
load("//cc/common:cc_common.bzl", "cc_common")
load("//cc/private:cc_internal.bzl", "cc_internal")
load("//cc/private:cc_info.bzl", "CcCompilationContextInfo")
load("//cc/private:toolchain_proof.bzl", "make_compile_toolchain")
load("//cc:cc_toolchain_config_lib.bzl", "feature", "flag_set", "flag_group", "env_set", "env_entry")

RegistrationProof = provider(fields = ["variables", "context", "builtin_after", "dirs_after", "seeded", "original_argv"])
ForeignOutput = provider(fields = ["file"])
# Deliberately authored forgeries, not alternate native provider definitions.
CcToolchainInfo = provider(fields = ["unused"])
SchemalessToolchain = provider()

def _foreign(ctx):
    output = ctx.actions.declare_file("foreign.o")
    ctx.actions.write(output, "inert foreign output")
    return [ForeignOutput(file = output)]

foreign_output = rule(implementation = _foreign)

def _suspended(subctx, arguments):
    cc_internal.create_cc_compile_action(**arguments)
    fail("suspended constructor unexpectedly succeeded")

suspended = subrule(implementation = _suspended)

def _file(target):
    return target[DefaultInfo].files.to_list()[0]

def _impl(ctx):
    mode = ctx.attr.failure
    compile_inputs = {}
    if mode == "malformed_without":
        compile_inputs["without"] = 17
    elif mode == "malformed_fallback":
        compile_inputs = {"without": depset(), "compiler": 17}
    compile_features = []
    if mode == "command_cache":
        compile_features = [feature(name = "cache_probe", enabled = True,
            flag_sets = [flag_set(actions = ["c++-compile"], flag_groups = [
                flag_group(iterate_over = "obj.items", flags = ["--cached=%{obj.items}"]),
                flag_group(iterate_over = "live", flags = ["--direct=%{live}"]),
            ])],
            env_sets = [env_set(actions = ["c++-compile"], env_entries = [env_entry(key = "SOURCE", value = "%{env_file}")])])]
    elif mode == "command_error":
        compile_features = [feature(name = "lazy_error", enabled = True,
            flag_sets = [flag_set(actions = ["c++-compile"], flag_groups = [flag_group(flags = ["%{malformed_unused}"])])],
            env_sets = [env_set(actions = ["c++-compile"], env_entries = [env_entry(key = "BAD", value = "%{malformed_unused}")])])]
    tc = make_compile_toolchain(ctx, compile_requirements = ["custom-compile-tool"], compile_inputs = compile_inputs, compile_features = compile_features)
    features = cc_common.configure_features(ctx = ctx, cc_toolchain = tc,
        requested_features = ["cc_include_scanning"] if mode == "scan_requested_default" else [], unsupported_features = [])
    source = _file(ctx.attr.source)
    extra = _file(ctx.attr.extra)
    cache = _file(ctx.attr.cache)
    header = _file(ctx.attr.header)
    base = cc_common.create_compilation_context(headers = depset([header]))
    context_fields = {name: getattr(base, name) for name in dir(base)}
    context_headers = [header]
    context_fields["direct_headers"] = context_headers
    context = CcCompilationContextInfo(**context_fields)
    live = ["before"]
    bindings = {
        "live": live,
        # Lazy native conversion would reject this if an expansion queried it.
        "malformed_unused": 17,
    }
    if mode in ["command_cache", "command_error"]:
        bindings.update({"obj": struct(items = live), "env_file": source,
            "preprocessor_defines": [], "quote_include_paths": [], "include_paths": [],
            "system_include_paths": [], "framework_include_paths": []})
    variables = cc_internal.cc_toolchain_variables(vars = bindings)
    seeded = None
    if mode == "command_cache":
        # Do not add source_file: legacy compiler_input_flags would add -c. The
        # env query's File variable lives in a separate unused-by-flags binding.
        seeded = cc_common.get_memory_inefficient_command_line(feature_configuration = features,
            action_name = "c++-compile", variables = variables)
    before = ctx.actions.declare_file(ctx.label.name + ".before")
    middle = ctx.actions.declare_file(ctx.label.name + ".middle")
    after = ctx.actions.declare_file(ctx.label.name + ".after")
    ctx.actions.write(before, "before")
    first = None
    dotd = None
    for index in range(3):
        output = ctx.actions.declare_file(ctx.label.name + "." + str(index) + ".o")
        dependency = ctx.actions.declare_file(ctx.label.name + "." + str(index) + ".d")
        if index == 0:
            first = output
            dotd = dependency
            if ctx.attr.failure == "conflict_before":
                ctx.actions.write(output, "conflicting ordinary action before pending")
        if index == 2 and ctx.attr.failure == "conflict_pending":
            dependency = dotd
        arguments = dict(
            action_construction_context = ctx, configuration = ctx.configuration,
            cc_compilation_context = context, cc_toolchain = tc,
            feature_configuration = features, source = source,
            compile_build_variables = variables,
            toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
            output_file = output, dotd_file = dependency,
            additional_compilation_inputs = [extra],
            additional_compilation_inputs_set = depset([extra]),
            cache_key_inputs = depset([cache]), needs_include_validation = True,
        )
        if mode == "fake_ctx":
            arguments["action_construction_context"] = struct()
        elif mode == "fake_features":
            arguments["feature_configuration"] = struct()
        elif mode == "fake_toolchain":
            arguments["cc_toolchain"] = struct()
        elif mode == "foreign_toolchain_key":
            arguments["cc_toolchain"] = CcToolchainInfo(unused = None)
        elif mode == "schemaless_toolchain":
            arguments["cc_toolchain"] = SchemalessToolchain()
        elif mode == "fake_context":
            arguments["cc_compilation_context"] = struct()
        elif mode == "fake_header":
            context_fields["_header_info"] = struct()
            arguments["cc_compilation_context"] = CcCompilationContextInfo(**context_fields)
        elif mode == "malformed_headers":
            context_fields["headers"] = depset(["not a File"])
            arguments["cc_compilation_context"] = CcCompilationContextInfo(**context_fields)
        elif mode == "source_output":
            arguments["output_file"] = source
        elif mode == "foreign_output":
            arguments["output_file"] = ctx.attr.foreign[ForeignOutput].file
        elif mode == "tree_output":
            arguments["output_file"] = ctx.actions.declare_directory("tree")
        elif mode == "tree_source":
            arguments["source"] = ctx.actions.declare_directory("tree-source")
        elif mode == "bad_source":
            arguments["source"] = header
        elif mode == "malformed_builtin":
            tc._builtin_include_files.append("not a File")
        elif mode == "scan_true":
            arguments["should_scan_includes"] = True
        elif mode == "scan_false":
            arguments["should_scan_includes"] = False
        elif mode == "suspended":
            suspended(arguments)
            fail("suspended constructor unexpectedly succeeded")
        cc_internal.create_cc_compile_action(**arguments)
        if index == 0:
            ctx.actions.write(middle, "middle")
            live.append("after")
            context_headers.append(extra)
            # Native construction snapshots these toolchain lists eagerly.
            tc._builtin_include_files.append(extra)
            tc.built_in_include_directories.append("late/./include/../headers")
    if ctx.attr.failure == "conflict_after":
        ctx.actions.write(first, "conflicting ordinary action after pending")
    if ctx.attr.failure == "conflict_dotd":
        ctx.actions.write(dotd, "conflicting dotd ordinary action")
    ctx.actions.write(after, "after")
    original_argv = None
    if mode == "command_cache":
        original_argv = cc_common.get_memory_inefficient_command_line(feature_configuration = features,
            action_name = "c++-compile", variables = variables)
    return [RegistrationProof(variables = variables, context = context,
        builtin_after = tc._builtin_include_files, dirs_after = tc.built_in_include_directories,
        seeded = seeded, original_argv = original_argv)]

registration_subject = rule(implementation = _impl, fragments = ["cpp"], subrules = [suspended], attrs = {
    "failure": attr.string(), "runtime": attr.string(default = "files"),
    "unused": attr.string(default = "A"), "object_prefix": attr.string(),
    "object_extension": attr.string(default = ".o"),
    "header": attr.label(default = "@@//:header.h", allow_single_file = True),
    "static_file": attr.label(default = "@@//:static.a", allow_single_file = True),
    "dynamic_file": attr.label(default = "@@//:dynamic.so", allow_single_file = True),
    "source": attr.label(default = "@@//:allocator_library.cc", allow_single_file = True),
    "extra": attr.label(default = "@@//:private.h", allow_single_file = True),
    "cache": attr.label(default = "@@//:changed.h", allow_single_file = True),
    "foreign": attr.label(default = "@@//:registration_foreign"),
})
