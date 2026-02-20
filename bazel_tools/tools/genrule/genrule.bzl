# Copyright 2024 The Bazel Authors. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Starlark implementation of Bazel's genrule() rule.

genrule generates one or more files using a user-defined bash command.
It is the most general rule, allowing arbitrary command execution.

See: https://bazel.build/reference/be/general#genrule
"""

def _genrule_impl(ctx):
    # Collect input files from srcs
    inputs = list(ctx.files.srcs)

    # Collect tool files
    tool_inputs = list(ctx.files.tools)

    # Declare output files from outs string list
    outs = [ctx.actions.declare_file(out) for out in ctx.attr.outs]

    if not outs:
        fail("genrule '{}' must have at least one output in 'outs'".format(ctx.label.name))

    # Build make-variable substitution strings
    srcs_paths = " ".join([f.path for f in inputs])
    outs_paths = " ".join([o.path for o in outs])
    first_out = outs[0].path
    first_src = inputs[0].path if inputs else ""

    # $(@D): output directory
    # If there's one output: dirname of that output
    # If there are multiple outputs: the package output directory
    out_dir = outs[0].dirname

    # Pick the right command:
    # cmd_bash overrides cmd on Linux/macOS
    cmd = ctx.attr.cmd_bash if ctx.attr.cmd_bash else ctx.attr.cmd

    if not cmd:
        fail("genrule '{}' must have a non-empty 'cmd' or 'cmd_bash'".format(ctx.label.name))

    # Expand $(location ...) and $(locations ...) patterns
    all_srcs_tools = list(ctx.attr.srcs) + list(ctx.attr.tools)
    if all_srcs_tools and ("$(location " in cmd or "$(locations " in cmd):
        cmd = ctx.expand_location(cmd, targets = all_srcs_tools)

    # Expand $(VARNAME) make-variable patterns
    cmd = ctx.expand_make_variables("cmd", cmd, {
        "SRCS": srcs_paths,
        "OUTS": outs_paths,
        # $(@D): output directory (or first output's dir)
        "@D": out_dir,
        # $(RULEDIR): the package output directory
        "RULEDIR": out_dir,
        # $(GENDIR) / $(BINDIR): approximate with out_dir root
        "GENDIR": out_dir,
        "BINDIR": out_dir,
        # $(TARGET): the target label
        "TARGET": str(ctx.label),
    })

    # Handle single-character $ substitutions (deprecated but still used):
    # $@ = output file (one output) or output dir (multiple outputs)
    # $< = first source file
    # $^ = all source files (space-separated)
    if len(outs) == 1:
        cmd = cmd.replace("$@", first_out)
    else:
        cmd = cmd.replace("$@", out_dir)
    cmd = cmd.replace("$<", first_src)
    cmd = cmd.replace("$^", srcs_paths)

    ctx.actions.run_shell(
        outputs = outs,
        inputs = inputs + tool_inputs,
        command = cmd,
        mnemonic = "Genrule",
    )

    return DefaultInfo(files = depset(outs))

genrule = rule(
    implementation = _genrule_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "outs": attr.string_list(),
        "cmd": attr.string(default = ""),
        "cmd_bash": attr.string(default = ""),
        "cmd_bat": attr.string(default = ""),
        "cmd_ps": attr.string(default = ""),
        "tools": attr.label_list(
            allow_files = True,
            cfg = "exec",
        ),
        "toolchains": attr.label_list(),
        "executable": attr.bool(default = False),
        "local": attr.bool(default = False),
        "message": attr.string(default = ""),
        "output_to_bindir": attr.bool(default = False),
    },
)
