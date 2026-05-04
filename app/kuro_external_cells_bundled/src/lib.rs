/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#[derive(Copy, Clone)]
pub struct BundledFile {
    pub path: &'static str,
    /// FIXME(JakobDegen): Consider compressing the data
    pub contents: &'static [u8],
    pub is_executable: bool,
}

#[derive(Copy, Clone)]
pub struct BundledCell {
    pub name: &'static str,
    pub files: &'static [BundledFile],
    pub is_testing: bool,
}

#[cfg(buck_build)]
mod prelude {
    include!("prelude/contents.rs");
}

#[cfg(not(buck_build))]
mod prelude {
    include!(concat!(env!("OUT_DIR"), "/prelude_include.rs"));
}

const PRELUDE: BundledCell = BundledCell {
    name: "prelude",
    files: prelude::DATA,
    is_testing: false,
};

#[cfg(buck_build)]
mod bazel_tools {
    include!("bazel_tools/contents.rs");
}

#[cfg(not(buck_build))]
mod bazel_tools {
    include!(concat!(env!("OUT_DIR"), "/bazel_tools_include.rs"));
}

const BAZEL_TOOLS: BundledCell = BundledCell {
    name: "bazel_tools",
    files: bazel_tools::DATA,
    is_testing: false,
};

#[cfg(buck_build)]
mod kuro_builtins {
    include!("kuro_builtins/contents.rs");
}

#[cfg(not(buck_build))]
mod kuro_builtins {
    include!(concat!(env!("OUT_DIR"), "/kuro_builtins_include.rs"));
}

/// Bundled-builtins cell. Ships `exports.bzl` whose public symbols
/// are injected into every BUILD/`.bzl` file via the interpreter's
/// `bazel_builtins_autoload`.
const KURO_BUILTINS: BundledCell = BundledCell {
    name: "kuro_builtins",
    files: kuro_builtins::DATA,
    is_testing: false,
};

#[cfg(not(buck_build))]
mod local_config_platform {
    include!(concat!(
        env!("OUT_DIR"),
        "/local_config_platform_include.rs"
    ));
}

const LOCAL_CONFIG_PLATFORM: BundledCell = BundledCell {
    name: "local_config_platform",
    files: local_config_platform::DATA,
    is_testing: false,
};

#[cfg(not(buck_build))]
mod local_config_python {
    include!(concat!(env!("OUT_DIR"), "/local_config_python_include.rs"));
}

const LOCAL_CONFIG_PYTHON: BundledCell = BundledCell {
    name: "local_config_python",
    files: local_config_python::DATA,
    is_testing: false,
};

const TEST_CELL: BundledCell = BundledCell {
    name: "test_bundled_cell",
    files: &[
        BundledFile {
            path: ".buckconfig",
            contents: include_bytes!("../test_data/.buckconfig"),
            is_executable: false,
        },
        BundledFile {
            path: "BUCK_TREE",
            contents: include_bytes!("../test_data/BUCK_TREE"),
            is_executable: false,
        },
        BundledFile {
            path: "dir/src.txt",
            contents: include_bytes!("../test_data/dir/src.txt"),
            is_executable: false,
        },
        BundledFile {
            path: "dir/src2.txt",
            contents: include_bytes!("../test_data/dir/src2.txt"),
            is_executable: true,
        },
        BundledFile {
            path: "dir/src3.txt",
            contents: include_bytes!("../test_data/dir/src3.txt"),
            is_executable: true,
        },
        BundledFile {
            path: "dir/BUCK.fixture",
            contents: include_bytes!("../test_data/dir/BUCK.fixture"),
            is_executable: false,
        },
        BundledFile {
            path: "dir/defs.bzl",
            contents: include_bytes!("../test_data/dir/defs.bzl"),
            is_executable: false,
        },
    ],
    is_testing: true,
};

pub const fn get_bundled_data() -> &'static [BundledCell] {
    // bazel_tools is required for rules_cc (references @bazel_tools//tools/cpp:...)
    // local_config_platform provides HOST_CONSTRAINTS for the current platform
    // prelude is included for legacy (non-bzlmod) projects that reference it via
    // [external_cells] prelude = bundled in .buckconfig
    &[
        PRELUDE,
        BAZEL_TOOLS,
        KURO_BUILTINS,
        LOCAL_CONFIG_PLATFORM,
        LOCAL_CONFIG_PYTHON,
        TEST_CELL,
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sanity_check() {
        let c = super::TEST_CELL;
        assert!(c.files.iter().any(|file| {
            file.path == "dir/src.txt"
            // Git may check out files on Windows with \r\n as line separator.
            && std::str::from_utf8(file.contents).unwrap().replace("\r\n", "\n") == "foobar\n"
        }))
    }

    #[test]
    fn test_bundled_prelude_data() {
        let c = super::PRELUDE;
        // Check that there's at least 50 files with a reasonable amount of data
        assert!(
            c.files
                .iter()
                .filter(|file| file.contents.len() > 100)
                .count()
                > 50
        );
    }

    #[test]
    fn test_bundled_bazel_tools_data() {
        let c = super::BAZEL_TOOLS;

        // Make sure http.bzl exists (critical for module extensions)
        // Path is tools/build_defs/repo/http.bzl (matching @bazel_tools//tools/build_defs/repo:http.bzl)
        assert!(c.files.iter().any(|file| {
            file.path == "tools/build_defs/repo/http.bzl"
                && std::str::from_utf8(file.contents)
                    .unwrap()
                    .contains("http_archive")
        }));

        // Make sure toolchain_utils.bzl exists (critical for rules_cc)
        // Path is tools/cpp/toolchain_utils.bzl (matching @bazel_tools//tools/cpp:toolchain_utils.bzl)
        assert!(c.files.iter().any(|file| {
            file.path == "tools/cpp/toolchain_utils.bzl"
                && std::str::from_utf8(file.contents)
                    .unwrap()
                    .contains("find_cpp_toolchain")
        }));

        // Should have at least 50 .bzl files
        assert!(
            c.files
                .iter()
                .filter(|file| file.path.ends_with(".bzl"))
                .count()
                > 50,
            "Expected at least 50 .bzl files in bazel_tools"
        );
    }
}
