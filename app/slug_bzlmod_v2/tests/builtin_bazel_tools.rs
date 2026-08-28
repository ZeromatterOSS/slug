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
        true,
    ),
    (
        "src/conditions/BUILD",
        "7a2d956c2c38092b93276b6cb11492f0ef7ce401c879d68a57e40b45f9163f16",
        true,
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
        "tools/test/BUILD",
        "81db88f41f7a9a07af246a42cfa7a8b6e118012b4f41830aaee9ffe4a4a9ee17",
        true,
    ),
    (
        "tools/test/default_test_toolchain.bzl",
        "c013158dde96f9b4699af24806fab64e4574e398fe94f612e25a16b1fa4f16f6",
        true,
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
    let first = read(&first_dice, "tools/test/BUILD").await.unwrap();
    let middle = read(&first_dice, "MODULE.bazel").await.unwrap();
    let restored = read(&first_dice, "tools/test/BUILD").await.unwrap();
    let independent_graph = read(&second_dice, "tools/test/BUILD").await.unwrap();
    assert_eq!(first, restored);
    assert_eq!(first, independent_graph);
    assert_ne!(first.sha256(), middle.sha256());

    let identity = BuiltinBazelToolsSnapshot::CURRENT.route_identity();
    assert_eq!(identity.snapshot(), BuiltinBazelToolsSnapshot::Bazel9_2);
    assert_eq!(
        hex::encode(identity.manifest_sha256()),
        "0b7a4da7823e336384fc633e3e2964f01d5711c0a2b1a919a124bf629f9c599d"
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
