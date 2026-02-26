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
    # Use ctx.files.srcs to get flat list of File objects from both source files and targets
    files = ctx.files.srcs + ctx.files.data

    return [
        DefaultInfo(files = depset(files)),
    ]

filegroup = rule(
    implementation = _filegroup_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True, default = []),
        "data": attr.label_list(allow_files = True, default = []),
        # visibility is an internal attribute - don't define it explicitly
    },
)
