//! Genuine ctx recovery and unchanged artifact naming wrapper over a published toolchain.
use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

const PROOF: &str = "rules_cc/cc/private/context_naming_proof.bzl";

fn workspace() -> PathBuf {
    let root = super::cc_toolchain::workspace();
    fs::write(root.join(PROOF), include_str!("cc_context_naming.bzl")).unwrap();
    let build = root.join("rules_rust/BUILD.bazel");
    let existing = fs::read_to_string(&build).unwrap();
    fs::write(build, format!("load('@rules_cc//cc/private:context_naming_proof.bzl', naming='subject')\n{existing}\nnaming(name='naming',toolchain=':producer')\n")).unwrap();
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    configuration: ConfigurationKey,
) -> Arc<ConfiguredNodeResult> {
    super::request_configured(dice, root, root_epoch(root), true, "naming", configuration)
        .await
        .unwrap()
}

fn names(result: &ConfiguredNodeResult) -> Vec<&str> {
    assert!(result.actions().is_empty());
    let AnalysisValueKind::Struct(snapshot) = field(result, "snapshot").kind() else {
        panic!("snapshot")
    };
    let AnalysisValueKind::List(names) = snapshot.get("names").unwrap().kind() else {
        panic!("names")
    };
    names.iter().map(|name| name.as_str().unwrap()).collect()
}

#[tokio::test]
async fn original_context_and_unchanged_naming_query_use_published_toolchain() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, config(&[])).await;
    assert_eq!(
        names(&first),
        [
            "dir/file.o",
            "dir/file.pic.o",
            "dir/file.d",
            "dir/libfile.a"
        ]
    );
    let unused = request(
        &dice,
        &root,
        config(&[(NativeCommandOption::HostCopt, "unused")]),
    )
    .await;
    assert!(field(&first, "snapshot").publication_eq(field(&unused, "snapshot")));
    assert!(!field(&first, "toolchain").publication_eq(field(&unused, "toolchain")));
    assert_eq!(first, request(&dice, &root, config(&[])).await);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn naming_override_and_schema_classification_restore_in_same_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, config(&[])).await;
    let build = root.join("rules_rust/BUILD.bazel");
    let baseline = fs::read_to_string(&build).unwrap();
    fs::write(
        &build,
        baseline.replace(
            "subject(name='producer',",
            "subject(name='producer',object_prefix='custom-',object_extension='.obj',",
        ),
    )
    .unwrap();
    let changed = request(&dice, &root, config(&[])).await;
    assert_eq!(
        names(&changed),
        [
            "dir/custom-file.obj",
            "dir/file.pic.o",
            "dir/file.d",
            "dir/libfile.a"
        ]
    );
    assert!(!field(&first, "toolchain").publication_eq(field(&changed, "toolchain")));
    fs::write(&build, baseline).unwrap();
    assert_eq!(first, request(&dice, &root, config(&[])).await);

    let source = include_str!("cc_context_naming.bzl");
    fs::write(
        root.join(PROOF),
        source.replace(
            "Shape = provider(fields = [\"value\"])",
            "Shape = provider()",
        ),
    )
    .unwrap();
    let schema_changed = request(&dice, &root, config(&[])).await;
    let AnalysisValueKind::Provider(before) = field(&first, "shape").kind() else {
        panic!("shape")
    };
    let AnalysisValueKind::Provider(after) = field(&schema_changed, "shape").kind() else {
        panic!("shape")
    };
    assert_eq!(before.identity(), after.identity());
    assert_eq!(before.fields(), after.fields());
    assert!(before.has_schema());
    assert!(!after.has_schema());
    assert!(!field(&first, "shape").publication_eq(field(&schema_changed, "shape")));
    fs::write(root.join(PROOF), source).unwrap();
    let restored = request(&dice, &root, config(&[])).await;
    assert!(field(&first, "shape").publication_eq(field(&restored, "shape")));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn recovered_context_cannot_escape_into_published_providers() {
    let root = workspace();
    fs::write(
        root.join(PROOF),
        include_str!("cc_context_naming.bzl").replace(
            "snapshot = struct(names = names, label = str(recovered.label))",
            "snapshot = recovered",
        ),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let error =
        super::request_configured(&dice, &root, root_epoch(&root), true, "naming", config(&[]))
            .await
            .unwrap_err();
    assert!(
        error.contains("unsupported value/capability of type `analysis_ctx`"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}
