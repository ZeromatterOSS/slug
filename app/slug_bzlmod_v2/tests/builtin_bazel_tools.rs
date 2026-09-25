use std::sync::Arc;

use dice::DetectCycles;
use dice::Dice;
use slug_bzlmod_v2::BuiltinBazelToolsSnapshot;
use slug_bzlmod_v2::BuiltinBazelToolsSourceFileError;
use slug_bzlmod_v2::BuiltinBazelToolsSourceFileKey;
use slug_bzlmod_v2::BuiltinBazelToolsSourceKind;

const FILES: &[(&str, &str, bool)] = &[
    (
        "MODULE.bazel",
        "a51e647c77be3c7dcb861131e339f2b65301bb572d2a9ac3d7eef30ca5b8a523",
        false,
    ),
    (
        "src/conditions/BUILD",
        "7a2d956c2c38092b93276b6cb11492f0ef7ce401c879d68a57e40b45f9163f16",
        false,
    ),
    (
        "src/tools/launcher/BUILD",
        "e1818f24f7603cf65cb8a85f7e41a80c82e5bdd805fe652f71d435c447af0e36",
        false,
    ),
    (
        "src/tools/launcher/bash_launcher.cc",
        "fdbd84b0563defe83f73ebf0eeda648cca47b560c6cb7149f681a01030242bf5",
        false,
    ),
    (
        "src/tools/launcher/bash_launcher.h",
        "124b479382848c8d3ba41e986420e2487cd2d16e4c77c4256e133ba1e5d640f8",
        false,
    ),
    (
        "src/tools/launcher/dummy.cc",
        "bd0b0d9441b8f60d1cd52a6f96db34da57210014491d7adc15af788e823c0567",
        false,
    ),
    (
        "src/tools/launcher/java_launcher.cc",
        "23a8caa29f750241e239f34273a2673b5a1176587f696b71a53f7fdd780ae07e",
        false,
    ),
    (
        "src/tools/launcher/java_launcher.h",
        "9cac494d70d5c320305c1f20b8de8144101d1f2bf72b6cf3c751219e69a7e3dd",
        false,
    ),
    (
        "src/tools/launcher/launcher.cc",
        "2643cc9044ef1cf2458127033b8283e6534ff09f8f97a869d82e1fb5613f7c7b",
        false,
    ),
    (
        "src/tools/launcher/launcher.h",
        "e052389698c0862fee610769945f749a70a2c6da08cbfe027383c25d8fd8acc8",
        false,
    ),
    (
        "src/tools/launcher/launcher_main.cc",
        "09c7e588471adc7bf6047fcc339c175a538e2c267ebaadb053152978aa733d98",
        false,
    ),
    (
        "src/tools/launcher/launcher_maker.cc",
        "622320eddc3029ad7efc379edb8e4642a4a9539c73c7818dcbccd3f171b44f0f",
        false,
    ),
    (
        "src/tools/launcher/launcher_maker_test.bzl",
        "95a2448e9b703697d8dbbd4e22bb6f10961d58def53ff4992f6766f39ef54de2",
        false,
    ),
    (
        "src/tools/launcher/launcher_maker_test.cc",
        "f462c72a1a1afcaa1cf1ecfcc96d3280bd31638bcbb94bef7f74eda88ada0d1b",
        false,
    ),
    (
        "src/tools/launcher/python_launcher.cc",
        "1f2695479a7051c89df2539893b25e5092682f62d76b5d7864a4c7b93251d3c3",
        false,
    ),
    (
        "src/tools/launcher/python_launcher.h",
        "961a88392eff53fe40336a41bd2025b20b62e33c2f3f57b005febb8d5750f0d7",
        false,
    ),
    (
        "src/tools/launcher/util/BUILD",
        "d63b7a3415138b146544bd8668c85167f4c0fca07189fce6473cf9a9f0f80655",
        false,
    ),
    (
        "src/tools/launcher/win_manifest.xml",
        "cc2f6dfeaac5395643f8056c098d2b4fd82c1352d35fcf77c0229d5d3aee7cd9",
        false,
    ),
    (
        "src/tools/launcher/win_resources.rc",
        "063baa5b722fde9a7ac1d086a02994286950c490acfd5119bc2eb78f56c5acc2",
        false,
    ),
    (
        "src/tools/launcher/win_rules.bzl",
        "04e42889b0b7a9f12685def9e12bfa182aca513ffcc5707d1da14dd507a9e186",
        false,
    ),
    (
        "tools/BUILD",
        "b0fbb2f8eb70acce9a307cca3d487a360f32a89d412e22a39c38346b979fc1a6",
        false,
    ),
    (
        "tools/build_defs.bzl",
        "d5f935c4e72a365438711f08a2640094cbf0a03392eebb06d8cecdc58b8ab19c",
        false,
    ),
    (
        "tools/build_defs/build_info/BUILD",
        "91db40a0a2ecc4eb8aaed489dd4ce117af5a0e5d64ed7d9d6f21417e4873bb1a",
        false,
    ),
    (
        "tools/build_defs/build_info/bazel_cc_build_info.bzl",
        "72cc98ddc39e4c7c66d23541c7b18e10b70fde6008e2883144554b7487e97514",
        false,
    ),
    (
        "tools/build_defs/build_info/bazel_java_build_info.bzl",
        "2ca911d9ab6b426cd120294a09148de8b641d14ebf6d9b3faaeaa86b3f4fc6bb",
        false,
    ),
    (
        "tools/build_defs/build_info/templates/BUILD",
        "569d06599fc494e6c7385a110f9f2c1c7806f471f66dda60c66969b95456f470",
        false,
    ),
    (
        "tools/build_defs/build_info/templates/non_volatile_file.h.template",
        "4d014ba8408a9a1e5cc9aca57183dee8bda0561ee92d4732bf174d0392218269",
        false,
    ),
    (
        "tools/build_defs/build_info/templates/non_volatile_file.properties.template",
        "c0ca85d3db416d3be5c68662b893e720bd817fb60c2b8405b41ecd51909a8f53",
        false,
    ),
    (
        "tools/build_defs/build_info/templates/redacted_file.h.template",
        "2e00a18d90e804f52af2079fdcdad7d78ec7e04790f9173c20dc2cb7a4092db0",
        false,
    ),
    (
        "tools/build_defs/build_info/templates/redacted_file.properties.template",
        "6ae3ee07bd3c4686ec4086678ce999cbc1720929ed1fa0ddeff605e96d01160a",
        false,
    ),
    (
        "tools/build_defs/build_info/templates/volatile_file.h.template",
        "83f47681c317f7b1249b6dcc9fa23a089233cf2f3a3801fedaca82869def8807",
        false,
    ),
    (
        "tools/build_defs/build_info/templates/volatile_file.properties.template",
        "0dd90374e26f9c19b9ade0de574a64733dac9c9371bb9feda2ebcab66948b483",
        false,
    ),
    (
        "tools/build_defs/cc/BUILD",
        "a24f1afcd5bfaaf9fc88ae3455213c83d61988bac5a80e58dd9f954281f6009d",
        false,
    ),
    (
        "tools/build_defs/cc/action_names.bzl",
        "ede4d3bd51a2a772180a0f3a47cf083e898d4104ec8de27f30ca36a5b8c13951",
        false,
    ),
    (
        "tools/build_defs/cc/cc_import.bzl",
        "a11736b1cf82a1216b62b6c8af280d739721c6dde470ff83cd939112a0a84093",
        false,
    ),
    (
        "tools/build_defs/cc/whitelists/parse_headers_and_layering_check/BUILD",
        "cce7a8a7321bdcd332d89d01581a3023bc7d73765e8e303419b91f890ebda583",
        false,
    ),
    (
        "tools/build_defs/repo/BUILD",
        "58fc51781cf26bfbcbd2c615f4cd0bd64892c3f7332e403eb1a885fea27ff3ca",
        false,
    ),
    (
        "tools/build_defs/repo/cache.bzl",
        "119c3fb281fcb02ce8aa0cd2f4fa315830ab160b483e4e041986422d2294d15b",
        false,
    ),
    (
        "tools/build_defs/repo/git.bzl",
        "c4f89658b4465dc4e42f87312b74d549fb434197bf0ade88fc4276550f68811b",
        false,
    ),
    (
        "tools/build_defs/repo/git_worker.bzl",
        "0bf607d50370d151bba1b541e8023ff040527f50f8fa8884157002ed9c63c339",
        false,
    ),
    (
        "tools/build_defs/repo/http.bzl",
        "9e908b9d6491cb950a9713d8b758b7b6f83871adbc768eb4997ca12e06ac240a",
        false,
    ),
    (
        "tools/build_defs/repo/java.bzl",
        "94fa09f776bb93a5ed3de1fccdb3a8f22c8792d01e5d7df6d588817b2cf02d7d",
        false,
    ),
    (
        "tools/build_defs/repo/jvm.bzl",
        "b3e2ff70d3706171123636248d7175dcb0046bbedea776016d49befc7a810309",
        false,
    ),
    (
        "tools/build_defs/repo/local.bzl",
        "f41d310ee3fcef8a637ddff5b21eb05724ad377bbb1b679d146327478613e4db",
        false,
    ),
    (
        "tools/build_defs/repo/utils.bzl",
        "902f228e729bb7ee86f86a3d434ccbddd9350bb5c7c869fa2f5fda90361605db",
        false,
    ),
    (
        "tools/cpp/BUILD",
        "0e3fdd3293d39a64f3fbda755d8a3c0bbd3b6ffd5fb0717ab594bc1f1e29a535",
        false,
    ),
    (
        "tools/cpp/cc_configure.bzl",
        "f1264cd4a6552eba7368729212aba64031ecd4330923d2bef61a20791ee2b4c5",
        false,
    ),
    (
        "tools/cpp/cc_flags_supplier.bzl",
        "e0150e4e012bbc2a6464d04ca7893c5da22fbf628c3e043e5d0bfb57a0baa1c9",
        false,
    ),
    (
        "tools/cpp/cc_toolchain_config_lib.bzl",
        "8df69d2032049acaafedd3feab3c5dbd36335de2ff76013e99c0815611a0c5d5",
        false,
    ),
    (
        "tools/cpp/compiler_flag.bzl",
        "23c0c595ff86daf7e4b7a9ffaff88ec4b79f060ee6db8d1121112e60c4efee75",
        false,
    ),
    (
        "tools/cpp/lib_cc_configure.bzl",
        "da7e4ae162120582a7a703b5657286dffe61fdf37cc489a4fc7625608517370c",
        false,
    ),
    (
        "tools/cpp/modules_tools/BUILD",
        "6f0727f09faaee782e8cf2219586243f767c7c4cb6dc3b9024f8c4c3aba9dad3",
        false,
    ),
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    (
        "tools/cpp/modules_tools/aggregate-ddi",
        "f12eb5bb2b0d8634745ff88deeed1c038f903e4d6058caf8c60db1de126c1f28",
        true,
    ),
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    (
        "tools/cpp/modules_tools/generate-modmap",
        "222b381da1faf69abf68f5763ad0531aa7aef110d3b16917ddb27e7f57165f70",
        true,
    ),
    (
        "tools/cpp/toolchain_utils.bzl",
        "feedbafb1be793c3e7710ece67e1ffb349d55ed726a358ee8d1dd8a68118cb7a",
        false,
    ),
    (
        "tools/cpp/unix_cc_toolchain_config.bzl",
        "1226e7f37ff91b55a3cf8eaddb60343cef7c71fc9bdd04ef6d372b27314bce37",
        false,
    ),
    (
        "tools/cpp/windows_cc_configure.bzl",
        "7d1b13bdc2b1f5b8cbfded820664fa7265087ac58909a7df33dad6878ace0bf3",
        false,
    ),
    (
        "tools/jdk/BUILD",
        "0ea0dae7e42fdfb3f2fbe945fae3e84e57e263ac6affd4dc6a74b351868832a6",
        false,
    ),
    (
        "tools/jdk/BUILD.java_tools",
        "dffe53e6c4c3445fdcf14965ffdd104f439727185c34cfe7c3f9c6ea9b4f116a",
        false,
    ),
    (
        "tools/jdk/BUILD.java_tools_prebuilt",
        "aebb48b76036e56140be4559504d5b3f8ca054ab673eff08ba1ba4d73f38c42e",
        false,
    ),
    (
        "tools/jdk/default_java_toolchain.bzl",
        "94baf15d9c6998411a211630cc56a421b6d4b99415850b422be0371e431443fb",
        false,
    ),
    (
        "tools/jdk/java_toolchain_alias.bzl",
        "96f7baca9fc31f01ebe780d3e94169b57542424f3a851215348565935e6cfaee",
        false,
    ),
    (
        "tools/jdk/launcher_flag_alias.bzl",
        "9c4f1a4656abb69476f0b1f3f39c06a28d7115342ee65598285e208d67a8c545",
        false,
    ),
    (
        "tools/jdk/local_java_repository.bzl",
        "0fb433f59ccc627f52313c12e8ea0607bc5539ddee98f310decaf787b1024c76",
        false,
    ),
    (
        "tools/jdk/nosystemjdk/README",
        "1df3fa40d06cd46928f7b7046d47eca8a278b38034e87946685d4492a1cf4ba8",
        false,
    ),
    (
        "tools/jdk/proguard_allowlister.py",
        "665481fd85cf41360680b9b7b1c136eb9eee56132f936f0e983bb51f54560bcb",
        false,
    ),
    (
        "tools/jdk/proguard_allowlister_test.py",
        "433ad2fb428f361523915a8c94c97ea6963e906a9b9ee708356249fdc9adb1d6",
        false,
    ),
    (
        "tools/jdk/proguard_allowlister_test_input.pgcfg",
        "cdcee843bedc114f941a3eed56938e4c8c1ec82a576ef857a188da90d3e021b3",
        false,
    ),
    (
        "tools/jdk/remote_java_repository.bzl",
        "ae5ca42c5d33cfb988221e578273f53f83846786296a488604c6877cb4217f88",
        false,
    ),
    (
        "tools/jdk/toolchain_utils.bzl",
        "f8979c720fcdc1cb62fbf184aa6e667abc1343fdc22302c7d37c329f6d0b7b09",
        false,
    ),
    (
        "tools/launcher/BUILD",
        "aa1b943956b6a7c3044f73583f5bc972bfc658607f7a3b745d51c7e7d016aab7",
        false,
    ),
    (
        "tools/launcher/empty.sh",
        "f3840c1e7a239cca9e5b2967c5e4a32e1c34c51a6f23f3cbafae08313e6ff55c",
        true,
    ),
    (
        "tools/osx/xcode_configure.bzl",
        "26d758318e481f8971dabd43e24d0b4e85c30eb074da39d3b63c778f39ebd942",
        false,
    ),
    (
        "tools/python/BUILD",
        "a9989e4f44dec38e493508e4db75419471f1f6fd87c532454d25921ba40ca79f",
        false,
    ),
    (
        "tools/python/private/py_test_alias.bzl",
        "e16604aa92e5d4d82745f6353e41d1bab11992926f8d3a48bb15db25a1719b9b",
        false,
    ),
    (
        "tools/python/python_bootstrap_template.txt",
        "b5146d4eb484e8e30905d9a51adae93a21c6f951f549ae9e043e266b0fb90c3b",
        false,
    ),
    (
        "tools/python/python_version.bzl",
        "8568edfc1b408d4640be1f1b4f3f52136b1ac17d17f3f05cbfe218e3e9fe5e38",
        false,
    ),
    (
        "tools/python/srcs_version.bzl",
        "95498780651a61714a1e8a08fef234d8d3860f5fac714f74ed02b40e1eae5b79",
        false,
    ),
    (
        "tools/python/toolchain.bzl",
        "d9b9101682acde3cff1437ca31ab45631c4b94eb360b0ecfa3a2d72307d69ee4",
        false,
    ),
    (
        "tools/python/utils.bzl",
        "e28cdc0fbf6e6a2f862c5afdf0561d5ba1d118b4bf56327c92d32bd681d92761",
        false,
    ),
    (
        "tools/res/BUILD",
        "bef477365d864eab46fcfe73c635bafd11a7300e4e47c158abe20d269e07e8ac",
        false,
    ),
    (
        "tools/res/win_res.bzl",
        "d78b202e5609bc322f99990897a8e5e01a44e645b0f4e1c19b4677a3ea1bc275",
        false,
    ),
    (
        "tools/res/winsdk_configure.bzl",
        "f6463d7e0a136ffff7e9099532f11f9fe7db91bd93e423b5e7101b104d035375",
        false,
    ),
    (
        "tools/res/winsdk_toolchain.bzl",
        "a19f04238ee0b76dcbaa7aed4d4356fa03db805b6cf7ace179bc358a4cd63938",
        false,
    ),
    (
        "tools/test/BUILD",
        "81db88f41f7a9a07af246a42cfa7a8b6e118012b4f41830aaee9ffe4a4a9ee17",
        false,
    ),
    (
        "tools/test/default_test_toolchain.bzl",
        "c013158dde96f9b4699af24806fab64e4574e398fe94f612e25a16b1fa4f16f6",
        false,
    ),
    (
        "tools/test/dummy.sh",
        "14a80dd0456a276c4707b36d8fb39cd180bb436c965fe13c79541fc8613d397c",
        true,
    ),
    (
        "tools/test/generate-xml.sh",
        "368e50ceca617b237c60adf70105cf6e1d33427f232c78239a3e7c10a4d93ebf",
        true,
    ),
    (
        "tools/test/test-setup.sh",
        "49ba08927c3c556c52c6f771eaca362a0dbd1b6e19fd2667c61d92c33a32278a",
        true,
    ),
];

async fn read(
    dice: &Arc<Dice>,
    path: &str,
) -> Result<slug_bzlmod_v2::BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError> {
    let mut transaction = dice.updater().commit().await;
    let value = transaction
        .compute(&BuiltinBazelToolsSourceFileKey::new(
            BuiltinBazelToolsSnapshot::Bazel9_2,
            path,
        ))
        .await
        .unwrap();
    value.as_ref().clone()
}

#[tokio::test]
async fn exact_catalog_bytes_digests_and_archive_modes_are_owned() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for (path, expected_sha256, executable) in FILES {
        let value = read(&dice, path).await.unwrap();
        assert_eq!(value.path(), *path);
        assert_eq!(hex::encode(value.sha256()), *expected_sha256);
        assert_eq!(value.executable(), *executable, "pinned mode for {path}");
        assert!(!value.bytes().is_empty());
    }
}

#[tokio::test]
async fn builtin_cc_whitelist_source_file_is_exact_and_has_directory_ancestors() {
    const PATH: &str = "tools/build_defs/cc/whitelists/parse_headers_and_layering_check/BUILD";
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let file = read(&dice, PATH).await.unwrap();
    assert_eq!(
        file.bytes(),
        &include_bytes!(
            "../builtin/bazel_tools/tools/build_defs/cc/whitelists/parse_headers_and_layering_check/BUILD"
        )[..]
    );
    assert_eq!(file.bytes().len(), 241);
    assert_eq!(
        hex::encode(file.sha256()),
        "cce7a8a7321bdcd332d89d01581a3023bc7d73765e8e303419b91f890ebda583"
    );
    assert!(!file.executable());
    for directory in [
        "tools/build_defs/cc/whitelists",
        "tools/build_defs/cc/whitelists/parse_headers_and_layering_check",
    ] {
        assert!(matches!(
            read(&dice, directory).await,
            Err(BuiltinBazelToolsSourceFileError::WrongKind {
                actual: BuiltinBazelToolsSourceKind::Directory,
                ..
            })
        ));
    }
}

#[tokio::test]
async fn runtime_jdk_build_uses_build_tools_bytes_not_source_tree_wrapper() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let build = read(&dice, "tools/jdk/BUILD").await.unwrap();
    assert_eq!(build.bytes().len(), 5_438);
    // Bazel 9.2.0 source-tree tools/jdk/BUILD is an 864-byte packaging manifest;
    // embedded_tools instead renames tools/jdk/BUILD.tools to this runtime path.
    assert_ne!(
        hex::encode(build.sha256()),
        "718a647c5763ccb0f370aa1b2526c6e387168ba0fba91d19e8ba163732dbe373"
    );
}

#[tokio::test]
async fn runtime_python_build_uses_build_tools_bytes_not_source_tree_wrapper() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let build = read(&dice, "tools/python/BUILD").await.unwrap();
    assert_eq!(
        hex::encode(build.sha256()),
        "a9989e4f44dec38e493508e4db75419471f1f6fd87c532454d25921ba40ca79f"
    );
    // Bazel 9.2 source-tree tools/python/BUILD packages embedded_tools;
    // the runtime archive renames BUILD.tools to tools/python/BUILD.
    assert_ne!(
        hex::encode(build.sha256()),
        "30336493cac4de1e474ce11547afc0789fcdf084e13ac43340a6cf6e40f3be24"
    );
    assert!(!build.executable());
}

#[tokio::test]
async fn invalid_wrong_kind_and_unsupported_catalog_are_distinct() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for path in [
        "",
        "/MODULE.bazel",
        "tools//test/BUILD",
        "tools/./test/BUILD",
        "tools/../MODULE.bazel",
        "tools\\test\\BUILD",
        "C:/MODULE.bazel",
        "bad\0path",
    ] {
        assert!(matches!(
            read(&dice, path).await,
            Err(BuiltinBazelToolsSourceFileError::InvalidPath { .. })
        ));
    }
    assert!(matches!(
        read(&dice, "tools/test").await,
        Err(BuiltinBazelToolsSourceFileError::WrongKind {
            actual: BuiltinBazelToolsSourceKind::Directory,
            ..
        })
    ));
    assert!(matches!(
        read(&dice, "tools/test/extensions.bzl").await,
        Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { .. })
    ));
    assert!(matches!(
        read(&dice, "tools/cpp/modules_tools").await,
        Err(BuiltinBazelToolsSourceFileError::WrongKind {
            actual: BuiltinBazelToolsSourceKind::Directory,
            ..
        })
    ));
    assert!(matches!(
        read(&dice, "tools/jdk").await,
        Err(BuiltinBazelToolsSourceFileError::WrongKind {
            actual: BuiltinBazelToolsSourceKind::Directory,
            ..
        })
    ));
    for path in ["tools/jdk/BUILD.tools", "tools/jdk/not-in-archive.bzl"] {
        assert!(matches!(
            read(&dice, path).await,
            Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { .. })
        ));
    }
    for path in [
        "tools/python/BUILD.tools",
        "tools/python/runfiles/BUILD",
        "tools/python/pywrapper_template.txt",
    ] {
        assert!(matches!(
            read(&dice, path).await,
            Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { .. })
        ));
    }
    assert!(matches!(
        read(&dice, "tools/python/private").await,
        Err(BuiltinBazelToolsSourceFileError::WrongKind {
            actual: BuiltinBazelToolsSourceKind::Directory,
            ..
        })
    ));
    for path in [
        "tools/cpp/modules_tools/aggregate-ddi.exe",
        "tools/cpp/modules_tools/generate-modmap.exe",
    ] {
        assert!(
            matches!(
                read(&dice, path).await,
                Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { .. })
            ),
            "Windows binary must remain outside the source catalog: {path}"
        );
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    for path in [
        "tools/cpp/modules_tools/aggregate-ddi",
        "tools/cpp/modules_tools/generate-modmap",
    ] {
        assert!(
            matches!(
                read(&dice, path).await,
                Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { .. })
            ),
            "Linux release binary must remain absent on this host: {path}"
        );
    }
}

#[tokio::test]
async fn immutable_snapshot_is_invariant_across_dice_instances_and_transactions() {
    let first_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let second_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let first = read(&first_dice, "tools/cpp/BUILD").await.unwrap();
    let middle = read(&first_dice, "MODULE.bazel").await.unwrap();
    let sibling = read(&first_dice, "tools/cpp/modules_tools/BUILD")
        .await
        .unwrap();
    let python = read(&first_dice, "tools/python/BUILD").await.unwrap();
    let restored = read(&first_dice, "tools/cpp/BUILD").await.unwrap();
    let independent_graph = read(&second_dice, "tools/cpp/BUILD").await.unwrap();
    let independent_python = read(&second_dice, "tools/python/BUILD").await.unwrap();
    assert_eq!(first, restored);
    assert_eq!(first, independent_graph);
    assert_eq!(python, independent_python);
    assert_ne!(first.sha256(), middle.sha256());
    assert_ne!(first.sha256(), sibling.sha256());
    assert_ne!(python.sha256(), first.sha256());

    let identity = BuiltinBazelToolsSnapshot::CURRENT.route_identity();
    assert_eq!(identity.snapshot(), BuiltinBazelToolsSnapshot::Bazel9_2);
    assert_eq!(
        hex::encode(identity.manifest_sha256()),
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            "331063679b7adc7084be104a455c13df476cdda2aa13acb07edf2dc7e12d9e7c"
        } else {
            "f4e5f390e55c75e9a91fb3dadf950927c8b0b56ab7ea7da99d4c5eed6d9d5151"
        }
    );
}

#[test]
fn embedded_catalog_paths_remain_exact_with_bazel_input_metadata() {
    let root = if let Some(runfiles) = std::env::var_os("TEST_SRCDIR") {
        std::path::PathBuf::from(runfiles).join("_main/app/slug_bzlmod_v2/builtin/bazel_tools")
    } else {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("builtin/bazel_tools")
    };
    let mut paths = files_below(&root, &root);
    let bazel_input_sidecars = [
        "src/conditions/BUILD.bazel",
        "src/tools/launcher/BUILD.bazel",
        "src/tools/launcher/util/BUILD.bazel",
        "tools/BUILD.bazel",
        "tools/build_defs/build_info/BUILD.bazel",
        "tools/build_defs/build_info/templates/BUILD.bazel",
        "tools/build_defs/cc/BUILD.bazel",
        "tools/build_defs/cc/whitelists/parse_headers_and_layering_check/BUILD.bazel",
        "tools/build_defs/repo/BUILD.bazel",
        "tools/cpp/BUILD.bazel",
        "tools/cpp/modules_tools/BUILD.bazel",
        "tools/jdk/BUILD.bazel",
        "tools/launcher/BUILD.bazel",
        "tools/python/BUILD.bazel",
        "tools/res/BUILD.bazel",
        "tools/test/BUILD.bazel",
    ];
    paths.retain(|path| !bazel_input_sidecars.contains(&path.as_str()));
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let linux_release_assets = [
            "tools/cpp/modules_tools/aggregate-ddi",
            "tools/cpp/modules_tools/generate-modmap",
        ];
        for asset in linux_release_assets {
            assert!(
                paths.iter().any(|path| path == asset),
                "checked-in release asset missing from Bazel inputs: {asset}"
            );
        }
        paths.retain(|path| !linux_release_assets.contains(&path.as_str()));
    }
    paths.sort();
    assert_eq!(
        paths,
        FILES.iter().map(|(path, _, _)| *path).collect::<Vec<_>>()
    );
}

fn files_below(root: &std::path::Path, at: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(at).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files_below(root, &path));
        } else {
            out.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    out
}
