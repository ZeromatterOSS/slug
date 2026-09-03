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
        "tools/cpp/cc_configure.bzl",
        "f1264cd4a6552eba7368729212aba64031ecd4330923d2bef61a20791ee2b4c5",
        false,
    ),
    (
        "tools/cpp/lib_cc_configure.bzl",
        "da7e4ae162120582a7a703b5657286dffe61fdf37cc489a4fc7625608517370c",
        false,
    ),
    (
        "tools/cpp/windows_cc_configure.bzl",
        "7d1b13bdc2b1f5b8cbfded820664fa7265087ac58909a7df33dad6878ace0bf3",
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
}

#[tokio::test]
async fn immutable_snapshot_is_invariant_across_dice_instances_and_transactions() {
    let first_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let second_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let first = read(&first_dice, "tools/build_defs/repo/utils.bzl")
        .await
        .unwrap();
    let middle = read(&first_dice, "MODULE.bazel").await.unwrap();
    let sibling = read(&first_dice, "tools/build_defs/repo/http.bzl")
        .await
        .unwrap();
    let restored = read(&first_dice, "tools/build_defs/repo/utils.bzl")
        .await
        .unwrap();
    let independent_graph = read(&second_dice, "tools/build_defs/repo/utils.bzl")
        .await
        .unwrap();
    assert_eq!(first, restored);
    assert_eq!(first, independent_graph);
    assert_ne!(first.sha256(), middle.sha256());
    assert_ne!(first.sha256(), sibling.sha256());

    let identity = BuiltinBazelToolsSnapshot::CURRENT.route_identity();
    assert_eq!(identity.snapshot(), BuiltinBazelToolsSnapshot::Bazel9_2);
    assert_eq!(
        hex::encode(identity.manifest_sha256()),
        "c313fad68f4e475d744dc6de7b658515b33c634905222e934a9d09129371f56f"
    );
}

#[test]
fn checked_in_assets_are_exactly_the_reviewed_catalog() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("builtin/bazel_tools");
    let mut paths = files_below(&root, &root);
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
