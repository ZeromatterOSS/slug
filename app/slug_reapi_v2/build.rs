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
    // Build scripts run before any async/DICE work. The vendored compiler keeps
    // this narrow REAPI surface hermetic across Cargo and Bazel environments.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    let manifest_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory is available"),
    );
    let proto_dir = manifest_dir.join("proto");
    let protos = [
        proto_dir.join("reapi_v2.proto"),
        proto_dir.join("google/protobuf/any.proto"),
        proto_dir.join("google/protobuf/duration.proto"),
        proto_dir.join("google/rpc/status.proto"),
        proto_dir.join("google/longrunning/operations.proto"),
    ];
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile_protos(&protos, &[proto_dir])
        .expect("REAPI protocol subset compiles");
}
