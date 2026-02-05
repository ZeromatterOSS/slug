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

"""Starlark implementation of filegroup rule for Kuro.

This provides a filegroup rule that groups files together and makes them
available to other rules as dependencies.
"""

def _filegroup_impl(ctx):
    """Implementation of the filegroup rule.

    Returns DefaultInfo with all the source files as outputs.
    """
    # Collect all files from srcs
    files = []
    for src in ctx.attrs.srcs:
        # src could be a source file or a target producing files
        if hasattr(src, "default_outputs"):
            # It's a target with DefaultInfo
            files.extend(src.default_outputs)
        elif hasattr(src, "short_path"):
            # It's a source artifact
            files.append(src)
        else:
            # Try to get it as an artifact
            files.append(src)

    # Also include any data files
    for data in ctx.attrs.data:
        if hasattr(data, "default_outputs"):
            files.extend(data.default_outputs)
        elif hasattr(data, "short_path"):
            files.append(data)
        else:
            files.append(data)

    return [
        DefaultInfo(default_outputs = files),
    ]

filegroup = rule(
    implementation = _filegroup_impl,
    attrs = {
        "srcs": attrs.list(attrs.source(), default = []),
        "data": attrs.list(attrs.source(), default = []),
        # visibility is an internal attribute - don't define it explicitly
    },
)
