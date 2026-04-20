/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Generate source files containing bundled cell contents (prelude, bazel_tools).

use std::io;
use std::path::Path;

fn main() {
    imp().unwrap();
}

fn imp() -> io::Result<()> {
    let out_path = std::env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_path);
    let manifest_path = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let project_root = Path::new(&manifest_path)
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    // Generate prelude bundled files
    let prelude_path = project_root.join("prelude");
    assert!(
        prelude_path.join("prelude.bzl").exists(),
        "prelude/prelude.bzl not found"
    );
    println!("cargo:rerun-if-changed={}", prelude_path.display());
    write_include_file(
        &prelude_path,
        std::fs::File::create(out_dir.join("prelude_include.rs"))?,
    )?;

    // Generate bazel_tools bundled files
    let bazel_tools_path = project_root.join("bazel_tools");
    assert!(
        bazel_tools_path
            .join("tools/build_defs/repo/http.bzl")
            .exists(),
        "bazel_tools/tools/build_defs/repo/http.bzl not found"
    );
    println!("cargo:rerun-if-changed={}", bazel_tools_path.display());
    write_include_file(
        &bazel_tools_path,
        std::fs::File::create(out_dir.join("bazel_tools_include.rs"))?,
    )?;

    // Generate local_config_platform bundled files
    // This is a Bazel auto-generated repo providing HOST_CONSTRAINTS for the current platform.
    let local_config_platform_out = out_dir.join("local_config_platform_src");
    std::fs::create_dir_all(&local_config_platform_out)?;

    let os_constraint = match std::env::consts::OS {
        "linux" => "@platforms//os:linux",
        "macos" => "@platforms//os:osx",
        "windows" => "@platforms//os:windows",
        _ => "@platforms//os:linux",
    };
    let cpu_constraint = match std::env::consts::ARCH {
        "x86_64" => "@platforms//cpu:x86_64",
        "aarch64" => "@platforms//cpu:aarch64",
        _ => "@platforms//cpu:x86_64",
    };
    let constraints_content =
        format!("HOST_CONSTRAINTS = [\n    \"{cpu_constraint}\",\n    \"{os_constraint}\",\n]\n");
    std::fs::write(
        local_config_platform_out.join("constraints.bzl"),
        &constraints_content,
    )?;

    // Generate BUILD.bazel for local_config_platform with a platform() target
    // that uses the auto-detected host OS/CPU constraints.
    // This mirrors what Bazel's auto-generated @local_config_platform//:host provides.
    let build_content = "load(\":constraints.bzl\", \"HOST_CONSTRAINTS\")\n\nplatform(\n    name = \"host\",\n    constraint_values = HOST_CONSTRAINTS,\n)\n";
    std::fs::write(local_config_platform_out.join("BUILD.bazel"), build_content)?;

    write_include_file(
        &local_config_platform_out,
        std::fs::File::create(out_dir.join("local_config_platform_include.rs"))?,
    )?;

    // Generate local_config_python bundled cell.
    //
    // Mirrors Bazel's historical @local_config_python autoconfig: provides a host
    // py_runtime+py_runtime_pair wired into a toolchain() target. Kuro auto-
    // registers @local_config_python//:host_toolchain at cell-resolution time
    // when rules_python is in the module graph, so rules_python's py_library
    // and py_binary analysis (which looks up py3_runtime via
    // ctx.toolchains[@rules_python//python:toolchain_type]) finds a match.
    //
    // The interpreter_path is detected at kuro build time; if none is found,
    // "/usr/bin/python3" is used as the default (common on Linux). Users can
    // override at daemon startup by installing python3 at a standard path.
    let local_config_python_out = out_dir.join("local_config_python_src");
    std::fs::create_dir_all(&local_config_python_out)?;

    let interpreter_path = detect_host_python3();
    let python_build_content = format!(
        "load(\"@rules_python//python:py_runtime.bzl\", \"py_runtime\")\n\
         load(\"@rules_python//python:py_runtime_pair.bzl\", \"py_runtime_pair\")\n\
         load(\":stub_toolchain.bzl\", \"stub_toolchain_info\")\n\
         \n\
         py_runtime(\n\
         \x20   name = \"py3_runtime\",\n\
         \x20   interpreter_path = \"{interpreter_path}\",\n\
         \x20   python_version = \"PY3\",\n\
         \x20   visibility = [\"PUBLIC\"],\n\
         )\n\
         \n\
         py_runtime_pair(\n\
         \x20   name = \"py_runtime_pair\",\n\
         \x20   py3_runtime = \":py3_runtime\",\n\
         \x20   visibility = [\"PUBLIC\"],\n\
         )\n\
         \n\
         toolchain(\n\
         \x20   name = \"host_toolchain\",\n\
         \x20   toolchain = \":py_runtime_pair\",\n\
         \x20   toolchain_type = \"@rules_python//python:toolchain_type\",\n\
         \x20   visibility = [\"PUBLIC\"],\n\
         )\n\
         \n\
         # Stub impl for launcher_maker_toolchain_type: required to satisfy\n\
         # py_binary's rule-level toolchain list on bazel_9_or_later. Returns\n\
         # an empty ToolchainInfo. Never dereferenced on Linux/macOS because\n\
         # create_windows_exe_launcher is not called there.\n\
         stub_toolchain_info(name = \"launcher_maker_stub\", visibility = [\"PUBLIC\"])\n\
         \n\
         toolchain(\n\
         \x20   name = \"host_launcher_maker_toolchain\",\n\
         \x20   toolchain = \":launcher_maker_stub\",\n\
         \x20   toolchain_type = \"@bazel_tools//tools/launcher:launcher_maker_toolchain_type\",\n\
         \x20   visibility = [\"PUBLIC\"],\n\
         )\n"
    );
    let stub_toolchain_bzl = concat!(
        "# stub_toolchain_info: returns an empty ToolchainInfo. Used as the impl for\n",
        "# toolchain types that kuro must register to satisfy rule-level mandatory\n",
        "# declarations but that are never dereferenced on the host platform\n",
        "# (e.g., launcher_maker on Linux).\n",
        "def _stub_toolchain_info_impl(ctx):\n",
        "    return [platform_common.ToolchainInfo()]\n",
        "\n",
        "stub_toolchain_info = rule(\n",
        "    implementation = _stub_toolchain_info_impl,\n",
        "    attrs = {},\n",
        ")\n",
    );
    std::fs::write(
        local_config_python_out.join("stub_toolchain.bzl"),
        stub_toolchain_bzl,
    )?;
    std::fs::write(
        local_config_python_out.join("BUILD.bazel"),
        &python_build_content,
    )?;

    write_include_file(
        &local_config_python_out,
        std::fs::File::create(out_dir.join("local_config_python_include.rs"))?,
    )?;

    Ok(())
}

fn detect_host_python3() -> String {
    for candidate in [
        "/usr/bin/python3",
        "/usr/local/bin/python3",
        "/opt/homebrew/bin/python3",
    ] {
        if Path::new(candidate).exists() {
            return candidate.to_owned();
        }
    }
    "/usr/bin/python3".to_owned()
}

fn as_unix_like(path: &Path) -> String {
    path.to_str().unwrap().replace('\\', "/")
}

fn write_include_file(prelude: &Path, mut include_file: impl io::Write) -> io::Result<()> {
    #[allow(clippy::write_literal)]
    writeln!(include_file, "// {}generated by crate build.rs", "@")?;

    writeln!(
        include_file,
        "pub(crate) const DATA: &[crate::BundledFile] = &["
    )?;

    for res in walkdir::WalkDir::new(prelude) {
        let entry = res.map_err(|e| e.into_io_error().unwrap())?;
        if !entry.file_type().is_file() {
            continue;
        }

        writeln!(include_file, "crate::BundledFile {{")?;
        writeln!(
            include_file,
            "  path: r\"{}\",",
            as_unix_like(entry.path().strip_prefix(prelude).unwrap())
        )?;
        writeln!(
            include_file,
            "  contents: include_bytes!(r\"{}\"),",
            entry.path().display()
        )?;

        let exec_bit;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            exec_bit = entry.metadata()?.mode() & 0o111 != 0;
        }
        #[cfg(not(unix))]
        {
            exec_bit = false;
        }

        writeln!(include_file, "  is_executable: {exec_bit},")?;
        writeln!(include_file, "}},")?;
    }

    writeln!(include_file, "];")?;
    Ok(())
}
