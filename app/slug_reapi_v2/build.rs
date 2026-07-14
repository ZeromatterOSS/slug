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
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc is available");
    // Build scripts run before any async/DICE work. The vendored compiler keeps
    // this narrow REAPI surface hermetic across developer and CI machines.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile_protos(
            &[
                "proto/reapi_v2.proto",
                "proto/google/protobuf/any.proto",
                "proto/google/protobuf/duration.proto",
                "proto/google/rpc/status.proto",
                "proto/google/longrunning/operations.proto",
            ],
            &["proto"],
        )
        .expect("REAPI protocol subset compiles");
}
