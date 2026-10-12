/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathDirectoryName;

use crate::EvaluatedNonrootModule;
use crate::LogicalModuleFileId;
use crate::NonrootModuleKey;
use crate::module_eval::DirectNonregistryEvaluationError;
use crate::module_eval::evaluate_direct_nonregistry_module_closure_with_events;

const MANIFEST_DOMAIN: &[u8] = b"slug-v2:builtin-bazel-tools-manifest:v1\0";

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash, Allocative)]
pub enum BuiltinBazelToolsSnapshot {
    Bazel9_2,
}

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash, Allocative)]
pub struct BuiltinBazelToolsRouteIdentity {
    snapshot: BuiltinBazelToolsSnapshot,
    manifest_sha256: [u8; 32],
}

impl BuiltinBazelToolsRouteIdentity {
    pub fn snapshot(self) -> BuiltinBazelToolsSnapshot {
        self.snapshot
    }

    pub fn manifest_sha256(self) -> [u8; 32] {
        self.manifest_sha256
    }
}

impl BuiltinBazelToolsSnapshot {
    pub const CURRENT: Self = Self::Bazel9_2;

    pub fn bazel_version(self) -> &'static str {
        match self {
            Self::Bazel9_2 => "9.2.0",
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::Bazel9_2 => "bazel-9.2.0-8220c6198837d5c13d53fea211cf3282aa12408a",
        }
    }

    pub fn route_identity(self) -> BuiltinBazelToolsRouteIdentity {
        BuiltinBazelToolsRouteIdentity {
            snapshot: self,
            manifest_sha256: manifest_sha256(self),
        }
    }
}

struct CatalogEntry {
    path: &'static str,
    bytes: &'static [u8],
    expected_sha256: &'static str,
    executable: bool,
}

const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        path: "MODULE.bazel",
        bytes: include_bytes!("../builtin/bazel_tools/MODULE.bazel"),
        expected_sha256: "a51e647c77be3c7dcb861131e339f2b65301bb572d2a9ac3d7eef30ca5b8a523",
        executable: false,
    },
    CatalogEntry {
        path: "src/conditions/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/src/conditions/BUILD"),
        expected_sha256: "7a2d956c2c38092b93276b6cb11492f0ef7ce401c879d68a57e40b45f9163f16",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/BUILD"),
        expected_sha256: "e1818f24f7603cf65cb8a85f7e41a80c82e5bdd805fe652f71d435c447af0e36",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/bash_launcher.cc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/bash_launcher.cc"),
        expected_sha256: "fdbd84b0563defe83f73ebf0eeda648cca47b560c6cb7149f681a01030242bf5",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/bash_launcher.h",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/bash_launcher.h"),
        expected_sha256: "124b479382848c8d3ba41e986420e2487cd2d16e4c77c4256e133ba1e5d640f8",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/dummy.cc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/dummy.cc"),
        expected_sha256: "bd0b0d9441b8f60d1cd52a6f96db34da57210014491d7adc15af788e823c0567",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/java_launcher.cc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/java_launcher.cc"),
        expected_sha256: "23a8caa29f750241e239f34273a2673b5a1176587f696b71a53f7fdd780ae07e",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/java_launcher.h",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/java_launcher.h"),
        expected_sha256: "9cac494d70d5c320305c1f20b8de8144101d1f2bf72b6cf3c751219e69a7e3dd",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/launcher.cc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/launcher.cc"),
        expected_sha256: "2643cc9044ef1cf2458127033b8283e6534ff09f8f97a869d82e1fb5613f7c7b",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/launcher.h",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/launcher.h"),
        expected_sha256: "e052389698c0862fee610769945f749a70a2c6da08cbfe027383c25d8fd8acc8",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/launcher_main.cc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/launcher_main.cc"),
        expected_sha256: "09c7e588471adc7bf6047fcc339c175a538e2c267ebaadb053152978aa733d98",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/launcher_maker.cc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/launcher_maker.cc"),
        expected_sha256: "622320eddc3029ad7efc379edb8e4642a4a9539c73c7818dcbccd3f171b44f0f",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/launcher_maker_test.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/launcher_maker_test.bzl"),
        expected_sha256: "95a2448e9b703697d8dbbd4e22bb6f10961d58def53ff4992f6766f39ef54de2",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/launcher_maker_test.cc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/launcher_maker_test.cc"),
        expected_sha256: "f462c72a1a1afcaa1cf1ecfcc96d3280bd31638bcbb94bef7f74eda88ada0d1b",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/python_launcher.cc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/python_launcher.cc"),
        expected_sha256: "1f2695479a7051c89df2539893b25e5092682f62d76b5d7864a4c7b93251d3c3",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/python_launcher.h",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/python_launcher.h"),
        expected_sha256: "961a88392eff53fe40336a41bd2025b20b62e33c2f3f57b005febb8d5750f0d7",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/util/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/util/BUILD"),
        expected_sha256: "d63b7a3415138b146544bd8668c85167f4c0fca07189fce6473cf9a9f0f80655",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/win_manifest.xml",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/win_manifest.xml"),
        expected_sha256: "cc2f6dfeaac5395643f8056c098d2b4fd82c1352d35fcf77c0229d5d3aee7cd9",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/win_resources.rc",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/win_resources.rc"),
        expected_sha256: "063baa5b722fde9a7ac1d086a02994286950c490acfd5119bc2eb78f56c5acc2",
        executable: false,
    },
    CatalogEntry {
        path: "src/tools/launcher/win_rules.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/src/tools/launcher/win_rules.bzl"),
        expected_sha256: "04e42889b0b7a9f12685def9e12bfa182aca513ffcc5707d1da14dd507a9e186",
        executable: false,
    },
    CatalogEntry {
        path: "tools/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/BUILD"),
        expected_sha256: "b0fbb2f8eb70acce9a307cca3d487a360f32a89d412e22a39c38346b979fc1a6",
        executable: false,
    },
    CatalogEntry {
        path: "tools/allowlists/function_transition_allowlist/BUILD",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/allowlists/function_transition_allowlist/BUILD"
        ),
        expected_sha256: "28ba77597c529ea11281c8247a3bcfb55d440de6299cc517831573efdac538eb",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs.bzl"),
        expected_sha256: "d5f935c4e72a365438711f08a2640094cbf0a03392eebb06d8cecdc58b8ab19c",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/build_info/BUILD"),
        expected_sha256: "91db40a0a2ecc4eb8aaed489dd4ce117af5a0e5d64ed7d9d6f21417e4873bb1a",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/bazel_cc_build_info.bzl",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/build_info/bazel_cc_build_info.bzl"
        ),
        expected_sha256: "72cc98ddc39e4c7c66d23541c7b18e10b70fde6008e2883144554b7487e97514",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/bazel_java_build_info.bzl",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/build_info/bazel_java_build_info.bzl"
        ),
        expected_sha256: "2ca911d9ab6b426cd120294a09148de8b641d14ebf6d9b3faaeaa86b3f4fc6bb",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/templates/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/build_info/templates/BUILD"),
        expected_sha256: "569d06599fc494e6c7385a110f9f2c1c7806f471f66dda60c66969b95456f470",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/templates/non_volatile_file.h.template",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/build_info/templates/non_volatile_file.h.template"
        ),
        expected_sha256: "4d014ba8408a9a1e5cc9aca57183dee8bda0561ee92d4732bf174d0392218269",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/templates/non_volatile_file.properties.template",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/build_info/templates/non_volatile_file.properties.template"
        ),
        expected_sha256: "c0ca85d3db416d3be5c68662b893e720bd817fb60c2b8405b41ecd51909a8f53",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/templates/redacted_file.h.template",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/build_info/templates/redacted_file.h.template"
        ),
        expected_sha256: "2e00a18d90e804f52af2079fdcdad7d78ec7e04790f9173c20dc2cb7a4092db0",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/templates/redacted_file.properties.template",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/build_info/templates/redacted_file.properties.template"
        ),
        expected_sha256: "6ae3ee07bd3c4686ec4086678ce999cbc1720929ed1fa0ddeff605e96d01160a",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/templates/volatile_file.h.template",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/build_info/templates/volatile_file.h.template"
        ),
        expected_sha256: "83f47681c317f7b1249b6dcc9fa23a089233cf2f3a3801fedaca82869def8807",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/build_info/templates/volatile_file.properties.template",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/build_info/templates/volatile_file.properties.template"
        ),
        expected_sha256: "0dd90374e26f9c19b9ade0de574a64733dac9c9371bb9feda2ebcab66948b483",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/cc/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/cc/BUILD"),
        expected_sha256: "a24f1afcd5bfaaf9fc88ae3455213c83d61988bac5a80e58dd9f954281f6009d",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/cc/action_names.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/cc/action_names.bzl"),
        expected_sha256: "ede4d3bd51a2a772180a0f3a47cf083e898d4104ec8de27f30ca36a5b8c13951",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/cc/cc_import.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/cc/cc_import.bzl"),
        expected_sha256: "a11736b1cf82a1216b62b6c8af280d739721c6dde470ff83cd939112a0a84093",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/cc/whitelists/parse_headers_and_layering_check/BUILD",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/cc/whitelists/parse_headers_and_layering_check/BUILD"
        ),
        expected_sha256: "cce7a8a7321bdcd332d89d01581a3023bc7d73765e8e303419b91f890ebda583",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/BUILD"),
        expected_sha256: "58fc51781cf26bfbcbd2c615f4cd0bd64892c3f7332e403eb1a885fea27ff3ca",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/cache.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/cache.bzl"),
        expected_sha256: "119c3fb281fcb02ce8aa0cd2f4fa315830ab160b483e4e041986422d2294d15b",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/git.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/git.bzl"),
        expected_sha256: "c4f89658b4465dc4e42f87312b74d549fb434197bf0ade88fc4276550f68811b",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/git_worker.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/git_worker.bzl"),
        expected_sha256: "0bf607d50370d151bba1b541e8023ff040527f50f8fa8884157002ed9c63c339",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/http.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/http.bzl"),
        expected_sha256: "9e908b9d6491cb950a9713d8b758b7b6f83871adbc768eb4997ca12e06ac240a",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/java.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/java.bzl"),
        expected_sha256: "94fa09f776bb93a5ed3de1fccdb3a8f22c8792d01e5d7df6d588817b2cf02d7d",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/jvm.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/jvm.bzl"),
        expected_sha256: "b3e2ff70d3706171123636248d7175dcb0046bbedea776016d49befc7a810309",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/local.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/local.bzl"),
        expected_sha256: "f41d310ee3fcef8a637ddff5b21eb05724ad377bbb1b679d146327478613e4db",
        executable: false,
    },
    CatalogEntry {
        path: "tools/build_defs/repo/utils.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/build_defs/repo/utils.bzl"),
        expected_sha256: "902f228e729bb7ee86f86a3d434ccbddd9350bb5c7c869fa2f5fda90361605db",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/BUILD"),
        expected_sha256: "0e3fdd3293d39a64f3fbda755d8a3c0bbd3b6ffd5fb0717ab594bc1f1e29a535",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/cc_configure.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/cc_configure.bzl"),
        expected_sha256: "f1264cd4a6552eba7368729212aba64031ecd4330923d2bef61a20791ee2b4c5",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/cc_flags_supplier.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/cc_flags_supplier.bzl"),
        expected_sha256: "e0150e4e012bbc2a6464d04ca7893c5da22fbf628c3e043e5d0bfb57a0baa1c9",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/cc_toolchain_config_lib.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/cc_toolchain_config_lib.bzl"),
        expected_sha256: "8df69d2032049acaafedd3feab3c5dbd36335de2ff76013e99c0815611a0c5d5",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/compiler_flag.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/compiler_flag.bzl"),
        expected_sha256: "23c0c595ff86daf7e4b7a9ffaff88ec4b79f060ee6db8d1121112e60c4efee75",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/lib_cc_configure.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/lib_cc_configure.bzl"),
        expected_sha256: "da7e4ae162120582a7a703b5657286dffe61fdf37cc489a4fc7625608517370c",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/modules_tools/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/modules_tools/BUILD"),
        expected_sha256: "6f0727f09faaee782e8cf2219586243f767c7c4cb6dc3b9024f8c4c3aba9dad3",
        executable: false,
    },
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    CatalogEntry {
        path: "tools/cpp/modules_tools/aggregate-ddi",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/modules_tools/aggregate-ddi"),
        expected_sha256: "f12eb5bb2b0d8634745ff88deeed1c038f903e4d6058caf8c60db1de126c1f28",
        executable: true,
    },
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    CatalogEntry {
        path: "tools/cpp/modules_tools/generate-modmap",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/modules_tools/generate-modmap"),
        expected_sha256: "222b381da1faf69abf68f5763ad0531aa7aef110d3b16917ddb27e7f57165f70",
        executable: true,
    },
    CatalogEntry {
        path: "tools/cpp/toolchain_utils.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/toolchain_utils.bzl"),
        expected_sha256: "feedbafb1be793c3e7710ece67e1ffb349d55ed726a358ee8d1dd8a68118cb7a",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/unix_cc_toolchain_config.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/unix_cc_toolchain_config.bzl"),
        expected_sha256: "1226e7f37ff91b55a3cf8eaddb60343cef7c71fc9bdd04ef6d372b27314bce37",
        executable: false,
    },
    CatalogEntry {
        path: "tools/cpp/windows_cc_configure.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/cpp/windows_cc_configure.bzl"),
        expected_sha256: "7d1b13bdc2b1f5b8cbfded820664fa7265087ac58909a7df33dad6878ace0bf3",
        executable: false,
    },
    CatalogEntry {
        path: "tools/def_parser/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/def_parser/BUILD"),
        expected_sha256: "891d56515113353a675db4f62e51380f3816b703b18585361e02de84e16e95ca",
        executable: false,
    },
    CatalogEntry {
        path: "tools/def_parser/no_op.bat",
        bytes: include_bytes!("../builtin/bazel_tools/tools/def_parser/no_op.bat"),
        expected_sha256: "4ebf44828201a3228a0521723c48d26db87565e44ef862fb65d5ead92840f64d",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/BUILD"),
        expected_sha256: "0ea0dae7e42fdfb3f2fbe945fae3e84e57e263ac6affd4dc6a74b351868832a6",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/BUILD.java_tools",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/BUILD.java_tools"),
        expected_sha256: "dffe53e6c4c3445fdcf14965ffdd104f439727185c34cfe7c3f9c6ea9b4f116a",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/BUILD.java_tools_prebuilt",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/BUILD.java_tools_prebuilt"),
        expected_sha256: "aebb48b76036e56140be4559504d5b3f8ca054ab673eff08ba1ba4d73f38c42e",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/default_java_toolchain.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/default_java_toolchain.bzl"),
        expected_sha256: "94baf15d9c6998411a211630cc56a421b6d4b99415850b422be0371e431443fb",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/java_toolchain_alias.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/java_toolchain_alias.bzl"),
        expected_sha256: "96f7baca9fc31f01ebe780d3e94169b57542424f3a851215348565935e6cfaee",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/launcher_flag_alias.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/launcher_flag_alias.bzl"),
        expected_sha256: "9c4f1a4656abb69476f0b1f3f39c06a28d7115342ee65598285e208d67a8c545",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/local_java_repository.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/local_java_repository.bzl"),
        expected_sha256: "0fb433f59ccc627f52313c12e8ea0607bc5539ddee98f310decaf787b1024c76",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/nosystemjdk/README",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/nosystemjdk/README"),
        expected_sha256: "1df3fa40d06cd46928f7b7046d47eca8a278b38034e87946685d4492a1cf4ba8",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/proguard_allowlister.py",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/proguard_allowlister.py"),
        expected_sha256: "665481fd85cf41360680b9b7b1c136eb9eee56132f936f0e983bb51f54560bcb",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/proguard_allowlister_test.py",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/proguard_allowlister_test.py"),
        expected_sha256: "433ad2fb428f361523915a8c94c97ea6963e906a9b9ee708356249fdc9adb1d6",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/proguard_allowlister_test_input.pgcfg",
        bytes: include_bytes!(
            "../builtin/bazel_tools/tools/jdk/proguard_allowlister_test_input.pgcfg"
        ),
        expected_sha256: "cdcee843bedc114f941a3eed56938e4c8c1ec82a576ef857a188da90d3e021b3",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/remote_java_repository.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/remote_java_repository.bzl"),
        expected_sha256: "ae5ca42c5d33cfb988221e578273f53f83846786296a488604c6877cb4217f88",
        executable: false,
    },
    CatalogEntry {
        path: "tools/jdk/toolchain_utils.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/jdk/toolchain_utils.bzl"),
        expected_sha256: "f8979c720fcdc1cb62fbf184aa6e667abc1343fdc22302c7d37c329f6d0b7b09",
        executable: false,
    },
    CatalogEntry {
        path: "tools/launcher/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/launcher/BUILD"),
        expected_sha256: "aa1b943956b6a7c3044f73583f5bc972bfc658607f7a3b745d51c7e7d016aab7",
        executable: false,
    },
    CatalogEntry {
        path: "tools/launcher/empty.sh",
        bytes: include_bytes!("../builtin/bazel_tools/tools/launcher/empty.sh"),
        expected_sha256: "f3840c1e7a239cca9e5b2967c5e4a32e1c34c51a6f23f3cbafae08313e6ff55c",
        executable: true,
    },
    CatalogEntry {
        path: "tools/osx/xcode_configure.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/osx/xcode_configure.bzl"),
        expected_sha256: "26d758318e481f8971dabd43e24d0b4e85c30eb074da39d3b63c778f39ebd942",
        executable: false,
    },
    CatalogEntry {
        path: "tools/python/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/python/BUILD"),
        expected_sha256: "a9989e4f44dec38e493508e4db75419471f1f6fd87c532454d25921ba40ca79f",
        executable: false,
    },
    CatalogEntry {
        path: "tools/python/private/py_test_alias.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/python/private/py_test_alias.bzl"),
        expected_sha256: "e16604aa92e5d4d82745f6353e41d1bab11992926f8d3a48bb15db25a1719b9b",
        executable: false,
    },
    CatalogEntry {
        path: "tools/python/python_bootstrap_template.txt",
        bytes: include_bytes!("../builtin/bazel_tools/tools/python/python_bootstrap_template.txt"),
        expected_sha256: "b5146d4eb484e8e30905d9a51adae93a21c6f951f549ae9e043e266b0fb90c3b",
        executable: false,
    },
    CatalogEntry {
        path: "tools/python/python_version.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/python/python_version.bzl"),
        expected_sha256: "8568edfc1b408d4640be1f1b4f3f52136b1ac17d17f3f05cbfe218e3e9fe5e38",
        executable: false,
    },
    CatalogEntry {
        path: "tools/python/srcs_version.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/python/srcs_version.bzl"),
        expected_sha256: "95498780651a61714a1e8a08fef234d8d3860f5fac714f74ed02b40e1eae5b79",
        executable: false,
    },
    CatalogEntry {
        path: "tools/python/toolchain.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/python/toolchain.bzl"),
        expected_sha256: "d9b9101682acde3cff1437ca31ab45631c4b94eb360b0ecfa3a2d72307d69ee4",
        executable: false,
    },
    CatalogEntry {
        path: "tools/python/utils.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/python/utils.bzl"),
        expected_sha256: "e28cdc0fbf6e6a2f862c5afdf0561d5ba1d118b4bf56327c92d32bd681d92761",
        executable: false,
    },
    CatalogEntry {
        path: "tools/res/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/res/BUILD"),
        expected_sha256: "bef477365d864eab46fcfe73c635bafd11a7300e4e47c158abe20d269e07e8ac",
        executable: false,
    },
    CatalogEntry {
        path: "tools/res/win_res.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/res/win_res.bzl"),
        expected_sha256: "d78b202e5609bc322f99990897a8e5e01a44e645b0f4e1c19b4677a3ea1bc275",
        executable: false,
    },
    CatalogEntry {
        path: "tools/res/winsdk_configure.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/res/winsdk_configure.bzl"),
        expected_sha256: "f6463d7e0a136ffff7e9099532f11f9fe7db91bd93e423b5e7101b104d035375",
        executable: false,
    },
    CatalogEntry {
        path: "tools/res/winsdk_toolchain.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/res/winsdk_toolchain.bzl"),
        expected_sha256: "a19f04238ee0b76dcbaa7aed4d4356fa03db805b6cf7ace179bc358a4cd63938",
        executable: false,
    },
    CatalogEntry {
        path: "tools/test/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/BUILD"),
        expected_sha256: "81db88f41f7a9a07af246a42cfa7a8b6e118012b4f41830aaee9ffe4a4a9ee17",
        executable: false,
    },
    CatalogEntry {
        path: "tools/test/default_test_toolchain.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/default_test_toolchain.bzl"),
        expected_sha256: "c013158dde96f9b4699af24806fab64e4574e398fe94f612e25a16b1fa4f16f6",
        executable: false,
    },
    CatalogEntry {
        path: "tools/test/dummy.sh",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/dummy.sh"),
        expected_sha256: "14a80dd0456a276c4707b36d8fb39cd180bb436c965fe13c79541fc8613d397c",
        executable: true,
    },
    CatalogEntry {
        path: "tools/test/generate-xml.sh",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/generate-xml.sh"),
        expected_sha256: "368e50ceca617b237c60adf70105cf6e1d33427f232c78239a3e7c10a4d93ebf",
        executable: true,
    },
    CatalogEntry {
        path: "tools/test/test-setup.sh",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/test-setup.sh"),
        expected_sha256: "49ba08927c3c556c52c6f771eaca362a0dbd1b6e19fd2667c61d92c33a32278a",
        executable: true,
    },
];

fn file_sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn manifest_sha256(snapshot: BuiltinBazelToolsSnapshot) -> [u8; 32] {
    manifest_sha256_for(
        snapshot,
        CATALOG
            .iter()
            .map(|entry| (entry.path, entry.bytes, entry.executable)),
    )
}

fn manifest_sha256_for<'a>(
    snapshot: BuiltinBazelToolsSnapshot,
    entries: impl ExactSizeIterator<Item = (&'a str, &'a [u8], bool)>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(MANIFEST_DOMAIN);
    frame(&mut hasher, snapshot.tag().as_bytes());
    hasher.update((entries.len() as u64).to_be_bytes());
    for (path, bytes, executable) in entries {
        frame(&mut hasher, path.as_bytes());
        hasher.update(file_sha256(bytes));
        hasher.update([u8::from(executable)]);
        hasher.update((bytes.len() as u64).to_be_bytes());
    }
    hasher.finalize().into()
}

fn valid_relative_path(path: &str) -> bool {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return false;
    }
    let first = path.split('/').next().unwrap_or_default().as_bytes();
    !(first.len() >= 2 && first[0].is_ascii_alphabetic() && first[1] == b':')
}

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Allocative)]
pub enum BuiltinBazelToolsSourceKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BuiltinBazelToolsSourceFileValue {
    path: CompactString,
    bytes: Arc<[u8]>,
    sha256: [u8; 32],
    executable: bool,
}

impl BuiltinBazelToolsSourceFileValue {
    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[doc(hidden)]
    pub fn bytes_arc(&self) -> &Arc<[u8]> {
        &self.bytes
    }

    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    pub fn executable(&self) -> bool {
        self.executable
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum BuiltinBazelToolsSourceFileError {
    InvalidPath {
        path: CompactString,
    },
    WrongKind {
        path: CompactString,
        actual: BuiltinBazelToolsSourceKind,
    },
    UnsupportedCatalog {
        path: CompactString,
    },
    Integrity {
        path: CompactString,
        expected_sha256: CompactString,
        actual_sha256: CompactString,
    },
}

impl fmt::Display for BuiltinBazelToolsSourceFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { path } => {
                write!(f, "invalid built-in bazel_tools source-file path {path:?}")
            }
            Self::WrongKind { path, actual } => write!(
                f,
                "built-in bazel_tools source-file path {path:?} has wrong kind {actual:?}"
            ),
            Self::UnsupportedCatalog { path } => write!(
                f,
                "built-in bazel_tools source-file path {path:?} is outside the Bazel 9.2 catalog"
            ),
            Self::Integrity {
                path,
                expected_sha256,
                actual_sha256,
            } => write!(
                f,
                "built-in bazel_tools file {path:?} has SHA-256 {actual_sha256}, expected {expected_sha256}"
            ),
        }
    }
}

impl std::error::Error for BuiltinBazelToolsSourceFileError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BuiltinBazelToolsSourceFileKey {
    snapshot: BuiltinBazelToolsSnapshot,
    path: CompactString,
}

impl BuiltinBazelToolsSourceFileKey {
    pub fn new(snapshot: BuiltinBazelToolsSnapshot, path: impl Into<CompactString>) -> Self {
        Self {
            snapshot,
            path: path.into(),
        }
    }
}

impl fmt::Display for BuiltinBazelToolsSourceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "builtin-bazel-tools-source-file:{}", self.path)
    }
}

fn validated_file(
    path: CompactString,
    bytes: &'static [u8],
    expected_sha256: &'static str,
    executable: bool,
) -> Result<BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError> {
    let sha256 = file_sha256(bytes);
    let actual_sha256 = hex::encode(sha256);
    if actual_sha256 != expected_sha256 {
        return Err(BuiltinBazelToolsSourceFileError::Integrity {
            path,
            expected_sha256: CompactString::new(expected_sha256),
            actual_sha256: CompactString::from(actual_sha256),
        });
    }
    Ok(BuiltinBazelToolsSourceFileValue {
        path,
        bytes: Arc::from(bytes),
        sha256,
        executable,
    })
}

fn lookup(
    path: CompactString,
) -> Result<BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError> {
    if !valid_relative_path(path.as_str()) {
        return Err(BuiltinBazelToolsSourceFileError::InvalidPath { path });
    }
    if let Some(entry) = CATALOG.iter().find(|entry| entry.path == path.as_str()) {
        return validated_file(path, entry.bytes, entry.expected_sha256, entry.executable);
    }
    let prefix = format!("{path}/");
    if CATALOG.iter().any(|entry| entry.path.starts_with(&prefix)) {
        return Err(BuiltinBazelToolsSourceFileError::WrongKind {
            path,
            actual: BuiltinBazelToolsSourceKind::Directory,
        });
    }
    Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { path })
}

#[async_trait]
impl Key for BuiltinBazelToolsSourceFileKey {
    type Value = Arc<Result<BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match self.snapshot {
            BuiltinBazelToolsSnapshot::Bazel9_2 => Arc::new(lookup(self.path.clone())),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_value: &Self::Value) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum BuiltinBazelToolsDirectoryListingError {
    Source(BuiltinBazelToolsSourceFileError),
    ConflictingEntryKinds { path: CompactString },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct BuiltinBazelToolsDirectoryListingKey {
    snapshot: BuiltinBazelToolsSnapshot,
    directory: PackagePath,
}

impl BuiltinBazelToolsDirectoryListingKey {
    pub(crate) fn new(snapshot: BuiltinBazelToolsSnapshot, directory: PackagePath) -> Self {
        Self {
            snapshot,
            directory,
        }
    }
}

impl fmt::Display for BuiltinBazelToolsDirectoryListingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "builtin-bazel-tools-directory-listing:{}",
            self.directory
        )
    }
}

fn builtin_directory_listing(
    directory: &PackagePath,
) -> Result<PathDirectoryListing, BuiltinBazelToolsDirectoryListingError> {
    for entry in CATALOG {
        validated_file(
            CompactString::new(entry.path),
            entry.bytes,
            entry.expected_sha256,
            entry.executable,
        )
        .map_err(BuiltinBazelToolsDirectoryListingError::Source)?;
    }

    let directory = directory.as_str();
    if CATALOG.iter().any(|entry| entry.path == directory) {
        return Err(BuiltinBazelToolsDirectoryListingError::Source(
            BuiltinBazelToolsSourceFileError::WrongKind {
                path: CompactString::new(directory),
                actual: BuiltinBazelToolsSourceKind::File,
            },
        ));
    }
    let prefix = if directory.is_empty() {
        String::new()
    } else {
        format!("{directory}/")
    };
    let mut children = BTreeMap::<&str, PathDirectoryEntryKind>::new();
    for entry in CATALOG {
        let Some(remainder) = entry.path.strip_prefix(&prefix) else {
            continue;
        };
        let (name, kind) = match remainder.split_once('/') {
            Some((name, _)) => (name, PathDirectoryEntryKind::Directory),
            None => (remainder, PathDirectoryEntryKind::File),
        };
        match children.get(name) {
            Some(existing) if *existing != kind => {
                return Err(
                    BuiltinBazelToolsDirectoryListingError::ConflictingEntryKinds {
                        path: CompactString::new(if directory.is_empty() {
                            name.to_owned()
                        } else {
                            format!("{directory}/{name}")
                        }),
                    },
                );
            }
            Some(_) => {}
            None => {
                children.insert(name, kind);
            }
        }
    }
    if children.is_empty() {
        return Ok(PathDirectoryListing::Missing);
    }
    let entries = children
        .into_iter()
        .map(|(name, kind)| {
            PathDirectoryEntry::new(
                PathDirectoryName::new(name)
                    .expect("pinned built-in catalog names are single path components"),
                kind,
            )
        })
        .collect::<Vec<_>>();
    Ok(PathDirectoryListing::Present(PathDirectoryEntries::new(
        entries,
    )))
}

#[async_trait]
impl Key for BuiltinBazelToolsDirectoryListingKey {
    type Value = Arc<Result<PathDirectoryListing, BuiltinBazelToolsDirectoryListingError>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match self.snapshot {
            BuiltinBazelToolsSnapshot::Bazel9_2 => {
                Arc::new(builtin_directory_listing(&self.directory))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_value: &Self::Value) -> bool {
        true
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct BuiltinBazelToolsModuleValue {
    pub(crate) route_identity: BuiltinBazelToolsRouteIdentity,
    pub(crate) module_sha256: [u8; 32],
    pub(crate) module: EvaluatedNonrootModule,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum BuiltinBazelToolsModuleError {
    Source(BuiltinBazelToolsSourceFileError),
    Evaluation(DirectNonregistryEvaluationError),
}

impl fmt::Display for BuiltinBazelToolsModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Evaluation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for BuiltinBazelToolsModuleError {}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct BuiltinBazelToolsModuleKey(BuiltinBazelToolsSnapshot);

#[allow(dead_code)]
impl BuiltinBazelToolsModuleKey {
    pub(crate) fn new(snapshot: BuiltinBazelToolsSnapshot) -> Self {
        Self(snapshot)
    }
}

impl fmt::Display for BuiltinBazelToolsModuleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "builtin-bazel-tools-module:{:?}", self.0)
    }
}

fn evaluate_builtin_module_source(
    snapshot: BuiltinBazelToolsSnapshot,
    source: &BuiltinBazelToolsSourceFileValue,
) -> Result<BuiltinBazelToolsModuleValue, BuiltinBazelToolsModuleError> {
    let (module, events) = evaluate_direct_nonregistry_module_closure_with_events(
        NonrootModuleKey::new("bazel_tools", ""),
        LogicalModuleFileId::new("@@bazel_tools//:MODULE.bazel"),
        source.bytes(),
        &[],
        true,
    );
    assert!(
        events
            .expect("captured built-in MODULE evaluation returns an event batch")
            .events()
            .is_empty(),
        "the pinned built-in MODULE must remain print-free"
    );
    Ok(BuiltinBazelToolsModuleValue {
        route_identity: snapshot.route_identity(),
        module_sha256: source.sha256(),
        module: module.map_err(BuiltinBazelToolsModuleError::Evaluation)?,
    })
}

#[async_trait]
impl Key for BuiltinBazelToolsModuleKey {
    type Value = Arc<Result<BuiltinBazelToolsModuleValue, BuiltinBazelToolsModuleError>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let source = ctx
            .compute(&BuiltinBazelToolsSourceFileKey::new(self.0, "MODULE.bazel"))
            .await
            .expect("built-in source-file DICE computation cannot fail");
        let source = match source.as_ref() {
            Ok(source) => source,
            Err(error) => {
                return Arc::new(Err(BuiltinBazelToolsModuleError::Source(error.clone())));
            }
        };
        Arc::new(evaluate_builtin_module_source(self.0, source))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;

    use super::*;

    #[derive(Default)]
    struct ModuleTracker(Mutex<Vec<(ActivationKind, bool)>>);

    impl ModuleTracker {
        fn take(&self) -> Vec<(ActivationKind, bool)> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    impl ActivationTracker for ModuleTracker {
        fn key_activated(
            &self,
            _key: &DynKey,
            _deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key.downcast_ref::<BuiltinBazelToolsModuleKey>().is_some() {
                self.0
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_none()));
            }
        }
    }

    fn module_source() -> BuiltinBazelToolsSourceFileValue {
        lookup(CompactString::new("MODULE.bazel")).unwrap()
    }

    fn listing_rows(directory: &str) -> Vec<(String, PathDirectoryEntryKind)> {
        let directory = PackagePath::parse(directory).unwrap();
        let PathDirectoryListing::Present(entries) = builtin_directory_listing(&directory).unwrap()
        else {
            panic!("expected present built-in directory {directory}")
        };
        entries
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.name().as_os_str().to_str().unwrap().to_owned(),
                    entry.kind(),
                )
            })
            .collect()
    }

    #[test]
    fn builtin_directory_listing_is_sorted_unique_and_direct() {
        assert!(
            CATALOG
                .windows(2)
                .all(|entries| entries[0].path < entries[1].path),
            "built-in catalog paths must be sorted and unique"
        );
        assert_eq!(
            listing_rows(""),
            [
                ("MODULE.bazel".to_owned(), PathDirectoryEntryKind::File),
                ("src".to_owned(), PathDirectoryEntryKind::Directory),
                ("tools".to_owned(), PathDirectoryEntryKind::Directory),
            ]
        );
        assert_eq!(
            listing_rows("tools"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("allowlists".to_owned(), PathDirectoryEntryKind::Directory),
                ("build_defs".to_owned(), PathDirectoryEntryKind::Directory),
                ("build_defs.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("cpp".to_owned(), PathDirectoryEntryKind::Directory),
                ("def_parser".to_owned(), PathDirectoryEntryKind::Directory),
                ("jdk".to_owned(), PathDirectoryEntryKind::Directory),
                ("launcher".to_owned(), PathDirectoryEntryKind::Directory),
                ("osx".to_owned(), PathDirectoryEntryKind::Directory),
                ("python".to_owned(), PathDirectoryEntryKind::Directory),
                ("res".to_owned(), PathDirectoryEntryKind::Directory),
                ("test".to_owned(), PathDirectoryEntryKind::Directory),
            ]
        );
        assert_eq!(
            listing_rows("tools/def_parser"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("no_op.bat".to_owned(), PathDirectoryEntryKind::File),
            ]
        );
        assert_eq!(
            listing_rows("tools/build_defs/cc"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("action_names.bzl".to_owned(), PathDirectoryEntryKind::File,),
                ("cc_import.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("whitelists".to_owned(), PathDirectoryEntryKind::Directory),
            ]
        );
        assert_eq!(
            listing_rows("tools/build_defs/cc/whitelists"),
            [(
                "parse_headers_and_layering_check".to_owned(),
                PathDirectoryEntryKind::Directory
            )]
        );
        assert_eq!(
            listing_rows("tools/build_defs/cc/whitelists/parse_headers_and_layering_check"),
            [("BUILD".to_owned(), PathDirectoryEntryKind::File)]
        );
        assert_eq!(
            listing_rows("tools/build_defs/repo"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("cache.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("git.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("git_worker.bzl".to_owned(), PathDirectoryEntryKind::File,),
                ("http.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("java.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("jvm.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("local.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("utils.bzl".to_owned(), PathDirectoryEntryKind::File),
            ]
        );
        assert_eq!(
            listing_rows("tools/cpp"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("cc_configure.bzl".to_owned(), PathDirectoryEntryKind::File,),
                (
                    "cc_flags_supplier.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                (
                    "cc_toolchain_config_lib.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                ("compiler_flag.bzl".to_owned(), PathDirectoryEntryKind::File),
                (
                    "lib_cc_configure.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                (
                    "modules_tools".to_owned(),
                    PathDirectoryEntryKind::Directory
                ),
                (
                    "toolchain_utils.bzl".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "unix_cc_toolchain_config.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                (
                    "windows_cc_configure.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
            ]
        );
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert_eq!(
            listing_rows("tools/cpp/modules_tools"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("aggregate-ddi".to_owned(), PathDirectoryEntryKind::File),
                ("generate-modmap".to_owned(), PathDirectoryEntryKind::File),
            ]
        );
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        assert_eq!(
            listing_rows("tools/cpp/modules_tools"),
            [("BUILD".to_owned(), PathDirectoryEntryKind::File)]
        );
        assert_eq!(
            listing_rows("tools/jdk"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("BUILD.java_tools".to_owned(), PathDirectoryEntryKind::File),
                (
                    "BUILD.java_tools_prebuilt".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "default_java_toolchain.bzl".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "java_toolchain_alias.bzl".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "launcher_flag_alias.bzl".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "local_java_repository.bzl".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                ("nosystemjdk".to_owned(), PathDirectoryEntryKind::Directory),
                (
                    "proguard_allowlister.py".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "proguard_allowlister_test.py".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "proguard_allowlister_test_input.pgcfg".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "remote_java_repository.bzl".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                (
                    "toolchain_utils.bzl".to_owned(),
                    PathDirectoryEntryKind::File
                ),
            ]
        );
        assert_eq!(
            listing_rows("tools/jdk/nosystemjdk"),
            [("README".to_owned(), PathDirectoryEntryKind::File)]
        );
        assert_eq!(
            listing_rows("tools/launcher"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("empty.sh".to_owned(), PathDirectoryEntryKind::File),
            ]
        );
        assert_eq!(
            listing_rows("tools/osx"),
            [(
                "xcode_configure.bzl".to_owned(),
                PathDirectoryEntryKind::File
            )]
        );
        assert_eq!(
            listing_rows("tools/python"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("private".to_owned(), PathDirectoryEntryKind::Directory),
                (
                    "python_bootstrap_template.txt".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                (
                    "python_version.bzl".to_owned(),
                    PathDirectoryEntryKind::File
                ),
                ("srcs_version.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("toolchain.bzl".to_owned(), PathDirectoryEntryKind::File),
                ("utils.bzl".to_owned(), PathDirectoryEntryKind::File),
            ]
        );
        assert_eq!(
            listing_rows("tools/python/private"),
            [("py_test_alias.bzl".to_owned(), PathDirectoryEntryKind::File)]
        );
        assert_eq!(
            builtin_directory_listing(&PackagePath::parse("tools/python/runfiles").unwrap())
                .unwrap(),
            PathDirectoryListing::Missing,
        );
        assert_eq!(
            listing_rows("tools/res"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("win_res.bzl".to_owned(), PathDirectoryEntryKind::File),
                (
                    "winsdk_configure.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                (
                    "winsdk_toolchain.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
            ]
        );
        assert_eq!(
            listing_rows("src/tools/launcher"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                ("bash_launcher.cc".to_owned(), PathDirectoryEntryKind::File,),
                ("bash_launcher.h".to_owned(), PathDirectoryEntryKind::File,),
                ("dummy.cc".to_owned(), PathDirectoryEntryKind::File),
                ("java_launcher.cc".to_owned(), PathDirectoryEntryKind::File,),
                ("java_launcher.h".to_owned(), PathDirectoryEntryKind::File,),
                ("launcher.cc".to_owned(), PathDirectoryEntryKind::File),
                ("launcher.h".to_owned(), PathDirectoryEntryKind::File),
                ("launcher_main.cc".to_owned(), PathDirectoryEntryKind::File,),
                ("launcher_maker.cc".to_owned(), PathDirectoryEntryKind::File,),
                (
                    "launcher_maker_test.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                (
                    "launcher_maker_test.cc".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                (
                    "python_launcher.cc".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                ("python_launcher.h".to_owned(), PathDirectoryEntryKind::File,),
                ("util".to_owned(), PathDirectoryEntryKind::Directory),
                ("win_manifest.xml".to_owned(), PathDirectoryEntryKind::File,),
                ("win_resources.rc".to_owned(), PathDirectoryEntryKind::File,),
                ("win_rules.bzl".to_owned(), PathDirectoryEntryKind::File),
            ]
        );
        assert_eq!(
            listing_rows("src/tools/launcher/util"),
            [("BUILD".to_owned(), PathDirectoryEntryKind::File)]
        );
        assert_eq!(
            listing_rows("tools/test"),
            [
                ("BUILD".to_owned(), PathDirectoryEntryKind::File),
                (
                    "default_test_toolchain.bzl".to_owned(),
                    PathDirectoryEntryKind::File,
                ),
                ("dummy.sh".to_owned(), PathDirectoryEntryKind::File),
                ("generate-xml.sh".to_owned(), PathDirectoryEntryKind::File),
                ("test-setup.sh".to_owned(), PathDirectoryEntryKind::File),
            ]
        );
    }

    #[test]
    fn function_transition_allowlist_catalog_bytes_and_direct_listing_are_exact() {
        let path = "tools/allowlists/function_transition_allowlist/BUILD";
        let file = lookup(CompactString::new(path)).unwrap();
        assert_eq!(file.path(), path);
        assert_eq!(
            hex::encode(file.sha256()),
            "28ba77597c529ea11281c8247a3bcfb55d440de6299cc517831573efdac538eb"
        );
        assert_eq!(file.bytes().len(), 281);
        assert!(!file.executable());
        assert_eq!(
            listing_rows("tools/allowlists"),
            [(
                "function_transition_allowlist".to_owned(),
                PathDirectoryEntryKind::Directory
            )]
        );
        assert_eq!(
            listing_rows("tools/allowlists/function_transition_allowlist"),
            [("BUILD".to_owned(), PathDirectoryEntryKind::File)]
        );
        for path in [
            "tools/allowlists/absent/BUILD",
            "tools/allowlists/BUILD",
            "tools/allowlists/function_transition_allowlist/BUILD.bazel",
        ] {
            assert!(matches!(
                lookup(CompactString::new(path)),
                Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { .. })
            ));
        }
        assert_eq!(
            builtin_directory_listing(&PackagePath::parse("tools/allowlists/absent").unwrap())
                .unwrap(),
            PathDirectoryListing::Missing
        );
    }

    #[test]
    fn builtin_directory_listing_distinguishes_missing_and_file() {
        assert_eq!(
            builtin_directory_listing(&PackagePath::parse("absent").unwrap()).unwrap(),
            PathDirectoryListing::Missing
        );
        assert!(matches!(
            builtin_directory_listing(&PackagePath::parse("MODULE.bazel").unwrap()),
            Err(BuiltinBazelToolsDirectoryListingError::Source(
                BuiltinBazelToolsSourceFileError::WrongKind {
                    actual: BuiltinBazelToolsSourceKind::File,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn integrity_failure_is_exercised_through_validation() {
        let entry = &CATALOG[0];
        let error = validated_file(
            CompactString::new(entry.path),
            b"changed",
            entry.expected_sha256,
            entry.executable,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            BuiltinBazelToolsSourceFileError::Integrity { .. }
        ));
    }

    #[test]
    fn current_snapshot_owns_the_exact_bazel_version() {
        assert_eq!(BuiltinBazelToolsSnapshot::CURRENT.bazel_version(), "9.2.0");
        assert_eq!(
            BuiltinBazelToolsSnapshot::CURRENT
                .route_identity()
                .snapshot(),
            BuiltinBazelToolsSnapshot::CURRENT
        );
    }

    #[test]
    fn manifest_identity_discriminates_bytes_and_executable_mode() {
        let snapshot = BuiltinBazelToolsSnapshot::CURRENT;
        let original = manifest_sha256(snapshot);
        let changed_bytes = manifest_sha256_for(
            snapshot,
            CATALOG.iter().enumerate().map(|(index, entry)| {
                (
                    entry.path,
                    if index == 0 {
                        b"changed".as_slice()
                    } else {
                        entry.bytes
                    },
                    entry.executable,
                )
            }),
        );
        let changed_mode = manifest_sha256_for(
            snapshot,
            CATALOG.iter().enumerate().map(|(index, entry)| {
                (
                    entry.path,
                    entry.bytes,
                    if index == 0 {
                        !entry.executable
                    } else {
                        entry.executable
                    },
                )
            }),
        );

        assert_ne!(changed_bytes, original);
        assert_ne!(changed_mode, original);
        assert_ne!(changed_bytes, changed_mode);
    }

    #[test]
    fn builtin_bazel_tools_module_retains_the_complete_pinned_value() {
        let source = module_source();
        assert_eq!(
            hex::encode(source.sha256()),
            "a51e647c77be3c7dcb861131e339f2b65301bb572d2a9ac3d7eef30ca5b8a523"
        );
        assert!(!source.executable());

        let value =
            evaluate_builtin_module_source(BuiltinBazelToolsSnapshot::CURRENT, &source).unwrap();
        assert_eq!(
            hex::encode(value.route_identity.manifest_sha256()),
            if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
                "0cc12333267512ebe791d8f845735c5d9bd5f78b3d6a211f08fdc92ef62bacf7"
            } else {
                "ef108fa963117d4af9776ccbc4e380faa71f5eabda04725d846372459709b172"
            }
        );
        assert_eq!(value.module_sha256, source.sha256());
        assert_eq!(
            value.module.base.expected_key,
            NonrootModuleKey::new("bazel_tools", "")
        );
        assert_eq!(value.module.base.declared_name, "bazel_tools");
        assert_eq!(value.module.base.declared_version, "");
        assert_eq!(value.module.base.repo_name, "bazel_tools");

        let dependencies = value
            .module
            .base
            .dependencies
            .iter()
            .map(|(apparent, dep)| (apparent.as_str(), dep.name.as_str(), dep.version.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            dependencies,
            [
                ("rules_license", "rules_license", "1.0.0"),
                ("buildozer", "buildozer", "8.5.1"),
                ("platforms", "platforms", "1.0.0"),
                ("zlib", "zlib", "1.3.1.bcr.5"),
                ("com_google_protobuf", "protobuf", "33.4"),
                ("rules_java", "rules_java", "9.1.0"),
                ("rules_cc", "rules_cc", "0.2.17"),
                ("rules_python", "rules_python", "1.7.0"),
                ("rules_shell", "rules_shell", "0.6.1"),
                ("apple_support", "apple_support", "1.24.2"),
            ]
        );
        assert!(!value.module.base.dependencies.contains_key("bazel_tools"));
        assert_eq!(
            value.module.base.dependencies,
            value.module.base.original_dependencies
        );
        assert_eq!(
            value
                .module
                .base
                .nodep_dependencies
                .iter()
                .map(|dep| (dep.name.as_str(), dep.version.as_str()))
                .collect::<Vec<_>>(),
            [
                ("bazel_features", "1.42.1"),
                ("rules_apple", "4.1.0"),
                ("rules_swift", "3.1.2"),
                ("abseil-cpp", "20250814.1"),
            ]
        );

        assert_eq!(
            value
                .module
                .extension_usages
                .iter()
                .map(|usage| (usage.bzl_label.as_str(), usage.extension_name.as_str()))
                .collect::<Vec<_>>(),
            [
                (
                    "@bazel_tools//tools/osx:xcode_configure.bzl",
                    "xcode_configure_extension"
                ),
                ("@rules_java//java:extensions.bzl", "toolchains"),
                (
                    "@bazel_tools//tools/test:extensions.bzl",
                    "remote_coverage_tools_extension"
                ),
                ("@buildozer//:buildozer_binary.bzl", "buildozer_binary"),
                (
                    "//:MODULE.bazel",
                    "//tools/res:winsdk_configure.bzl winsdk_configure"
                ),
            ]
        );
        assert_eq!(
            value
                .module
                .extension_usages
                .iter()
                .flat_map(|usage| usage.proxies.iter())
                .flat_map(|proxy| proxy.imports.local_to_exported.keys())
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            [
                "local_config_xcode",
                "local_jdk",
                "remote_java_tools",
                "remote_coverage_tools",
                "buildozer_binary",
                "local_config_winsdk",
            ]
        );
        let innate = &value.module.extension_usages[4];
        assert_eq!(innate.tags.len(), 1);
        assert_eq!(innate.tags[0].tag_class, "repo");
        assert_eq!(
            value
                .module
                .base
                .toolchains
                .iter()
                .map(|v| v.as_str())
                .collect::<Vec<_>>(),
            [
                "//tools/launcher:all",
                "//tools/test:all",
                "@local_config_winsdk//:all",
                "//tools/res:empty_rc_toolchain",
            ]
        );
        assert!(value.module.base.execution_platforms.is_empty());
        assert!(value.module.base.flag_aliases.is_empty());

        assert_eq!(
            value,
            evaluate_builtin_module_source(BuiltinBazelToolsSnapshot::CURRENT, &source).unwrap()
        );
    }

    #[test]
    fn builtin_bazel_tools_module_preserves_typed_errors() {
        let invalid = BuiltinBazelToolsSourceFileValue {
            path: "MODULE.bazel".into(),
            bytes: Arc::from(b"module(".as_slice()),
            sha256: file_sha256(b"module("),
            executable: true,
        };
        assert!(matches!(
            evaluate_builtin_module_source(BuiltinBazelToolsSnapshot::CURRENT, &invalid),
            Err(BuiltinBazelToolsModuleError::Evaluation(
                DirectNonregistryEvaluationError::Preparation(_)
            ))
        ));
        assert!(matches!(
            BuiltinBazelToolsModuleError::Source(
                BuiltinBazelToolsSourceFileError::UnsupportedCatalog {
                    path: "outside".into()
                }
            ),
            BuiltinBazelToolsModuleError::Source(_)
        ));
    }

    #[tokio::test]
    async fn builtin_bazel_tools_module_is_a_callerless_reused_leaf() {
        let tracker = Arc::new(ModuleTracker::default());
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let key = BuiltinBazelToolsModuleKey::new(BuiltinBazelToolsSnapshot::CURRENT);
        let data = || UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };

        let mut transaction = dice.updater_with_data(data()).commit().await;
        let first = transaction.compute(&key).await.unwrap();
        assert!(first.as_ref().is_ok());
        assert_eq!(tracker.take(), [(ActivationKind::Evaluated, true)]);
        drop(transaction);

        let mut transaction = dice.updater_with_data(data()).commit().await;
        let second = transaction.compute(&key).await.unwrap();
        assert_eq!(first, second);
        assert_eq!(tracker.take(), [(ActivationKind::Reused, true)]);

        let implementation = include_str!("builtin_repository.rs")
            .split("impl Key for BuiltinBazelToolsModuleKey")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for forbidden in [
            "RootModuleGraphKey",
            "RootRepositoryRouteKey",
            "Host",
            "Registry",
            "Lockfile",
            "RepositoryMapping",
        ] {
            assert!(!implementation.contains(forbidden), "{forbidden}");
        }
    }
}
