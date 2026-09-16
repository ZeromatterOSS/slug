# Protocol source provenance

These `.proto` files are copied verbatim. They are inputs to the checked-in
crate's Cargo and Bazel build scripts; generated Rust is not checked in.

| Source group | Revision | License |
| --- | --- | --- |
| `build/bazel/**` | Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, embedded `third_party/remoteapis` tree `72e3f5671ce4f611759905b548d34fca26591543` | Apache-2.0, notice in each file and root `LICENSE-APACHE` |
| `google/api/**`, `google/bytestream/**`, `google/longrunning/**`, `google/rpc/**` | googleapis commit `de157ca34fa487ce248eb9130293d630b501e4ad`, selected by Bazel 9.2.0 as `0.0.0-20250604-de157ca3` | Apache-2.0, notice in each file and root `LICENSE-APACHE` |
| `google/protobuf/**` | `protoc-bin-vendored-linux-x86_64` 3.2.0 include tree, the locked protoc distribution also used by Bazel's selected generator | BSD-3-Clause terms in each file header |

Bazel 9.2.0 applies `third_party/googleapis.patch` (SHA-256
`751c57e02dc8d8c1cd0375c971720d5184de2c05d1f627e7177dc946e19dafd5`)
to the Google APIs module. That patch changes BUILD/MODULE files only; it does
not alter this proto closure. The pinned `remote_execution.proto` has Git blob
`e9cab05dd2e500bb7d754ee33e004fc612adf5f6` in the Bazel tree.

## Copied file SHA-256

Paths are relative to `proto/`.

| Path | SHA-256 |
| --- | --- |
| `build/bazel/remote/execution/v2/remote_execution.proto` | `9d2723bc91bd7fe154b52118cdc1818daa547c303d01cc346d7bf8fb6caa9cbc` |
| `build/bazel/semver/semver.proto` | `22b2af125690142af1c8152ba3a4ca15ffaa1265111dedc6e39b5412989b5be1` |
| `google/api/annotations.proto` | `e79ea741cb605a65e78ca322174764a4af9fde1962c1631e12b84c4934ba9a6c` |
| `google/api/client.proto` | `a5a13ea853fcb58095c645506dadc174fc200185973b2804af679c30ecf7399d` |
| `google/api/http.proto` | `4a4d9be6a5c7f1989c93c25c71b48ff1b401645790b8b978ad34d579e29c4a2a` |
| `google/api/launch_stage.proto` | `6ffd80d69f94430b4704b40fca9a339e10887f315efa66ac9c3c7d5587d6aba5` |
| `google/bytestream/bytestream.proto` | `961b833f35f4bdc51df4bca017cffdba299893e89762bf8041465560106dd3d6` |
| `google/longrunning/operations.proto` | `fcf560f3b8b9d1bdda82803026ac8caa616b0ee19d0f554e8eb9ef7b2f201fc2` |
| `google/rpc/status.proto` | `3b5c712455570ac4342dd3c521c4c11011652ae9a0fbca75ba22fcc45c6e1991` |
| `google/protobuf/any.proto` | `bcf5de6ce463b1a38ff76b77955aa7e580b4ec12af34a927b1d45c9efb0faf66` |
| `google/protobuf/descriptor.proto` | `428bd7cfc74c4e53dcab06d37080aa589452c2f93360bcbaccd5d1d78a50e8b1` |
| `google/protobuf/duration.proto` | `a3f7301ff2956ec2e30c2241ece07197e4a86c752348d5607224819d4921c9fe` |
| `google/protobuf/empty.proto` | `ecef3d54cc9e079673b9816c67bac770f7f3bf6dada2d4596ba69d71daa971e6` |
| `google/protobuf/timestamp.proto` | `14052c6042c1dd2d0b50245f2812eaab6eaf82db0b6e8ce483eae527f73b6ee8` |
| `google/protobuf/wrappers.proto` | `6878dc7534cf9805eee56345cdc38b5e9ca39dece246710c663c7007a67fef49` |

`remote_execution.proto` imports semver, Google API annotations, longrunning
operations, four well-known protobuf definitions, and RPC status.
`operations.proto` imports API annotations/client, more well-known definitions,
and RPC status; API annotations/client recursively import HTTP/launch-stage.
`bytestream.proto` has no imports. This table contains the complete recursive
source closure. Cargo and Bazel use the locked `protoc-bin-vendored` 3.2.0
compiler and `tonic-build` from `Cargo.lock`/`Cargo.Bazel.lock`; the Bazel
build script selects `//:vendored_protoc` and includes these exact source files
as action inputs.
