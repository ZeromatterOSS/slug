/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

fn main() {
    println!("cargo:rerun-if-env-changed=SLUG_BAZEL_PROTOC");
    let protoc = std::env::var_os("SLUG_BAZEL_PROTOC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            protoc_bin_vendored::protoc_bin_path().expect("vendored protoc is available")
        });
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    let proto_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory is available"),
    )
    .join("proto");
    let sources = [
        "build/bazel/remote/execution/v2/remote_execution.proto",
        "build/bazel/semver/semver.proto",
        "google/api/annotations.proto",
        "google/api/client.proto",
        "google/api/http.proto",
        "google/api/launch_stage.proto",
        "google/bytestream/bytestream.proto",
        "google/longrunning/operations.proto",
        "google/rpc/status.proto",
    ]
    .map(|path| proto_dir.join(path));
    for source in &sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile_protos(&sources, &[proto_dir])
        .expect("pinned REAPI and Google protocol definitions compile");
}
