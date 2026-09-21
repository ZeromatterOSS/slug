//! Configured output roots through unchanged rules_cc and Rust path consumers.
use slug_build_api_v2::AnalysisValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

fn workspace() -> PathBuf {
    let root = super::workspace_with(include_str!("artifact_roots.bzl"), "");
    fs::write(root.join("rules_rust/BUILD.bazel"),
        "load(':proof.bzl','producer','consumer')\nproducer(name='producer')\nconsumer(name='consumer',target=':producer',execution=':producer',sources=['@@//:lib.rs',':external.h'])\nexports_files(['external.h'])\n").unwrap();
    fs::write(root.join("rules_rust/external.h"), "// source\n").unwrap();
    root
}
fn member<'a>(value: &'a AnalysisValue, name: &str) -> &'a AnalysisValue {
    let AnalysisValueKind::Struct(fields) = value.kind() else {
        panic!("struct")
    };
    fields.get(name).unwrap()
}
fn strings(value: &AnalysisValue) -> Vec<&str> {
    let AnalysisValueKind::List(values) = value.kind() else {
        panic!("list")
    };
    values.iter().map(|value| value.as_str().unwrap()).collect()
}
async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    configuration: ConfigurationKey,
) -> Arc<ConfiguredNodeResult> {
    super::request_configured(
        dice,
        root,
        root_epoch(root),
        true,
        "consumer",
        configuration,
    )
    .await
    .unwrap()
}

fn check(result: &ConfiguredNodeResult) {
    let root = field(result, "root");
    let AnalysisValueKind::ArtifactRoot(root_value) = root.kind() else {
        panic!("root survived publication")
    };
    let snapshot = field(result, "snapshot");
    let target = member(snapshot, "target");
    let exec = member(snapshot, "execution");
    let prefix = root_value.path();
    assert_eq!(prefix.split('/').count(), 3);
    assert!(prefix.starts_with("bazel-out/slugcfg-v2-"));
    assert!(!prefix.contains("-exec"));
    assert_eq!(
        member(target, "path").as_str(),
        Some(format!("{prefix}/external/rules_rust+/producer.out").as_str())
    );
    assert_eq!(
        member(target, "short").as_str(),
        Some("../rules_rust+/producer.out")
    );
    assert_eq!(
        member(target, "relative").as_str(),
        Some("external/rules_rust+/producer.out")
    );
    assert_eq!(
        member(target, "pre").as_str(),
        Some("external/rules_rust+/producer.pre")
    );
    assert!(matches!(
        member(target, "exec").kind(),
        AnalysisValueKind::Boolean(false)
    ));
    assert!(matches!(
        member(exec, "exec").kind(),
        AnalysisValueKind::Boolean(true)
    ));
    let AnalysisValueKind::ArtifactRoot(exec_root) = member(exec, "bin").kind() else {
        panic!("exec root")
    };
    assert!(exec_root.path().contains("-exec/bin"));
    assert_ne!(root_value, exec_root);
    assert_eq!(
        member(snapshot, "generated").as_str(),
        Some(format!("{prefix}/external/rules_rust+/producer.pre").as_str())
    );
    assert!(member(snapshot, "generated_root").publication_eq(root));
    assert_eq!(
        strings(member(snapshot, "source_paths")),
        ["lib.rs", "external/rules_rust+/external.h"]
    );
    assert_eq!(
        strings(member(snapshot, "source_relative")),
        ["lib.rs", "external.h"]
    );
    let AnalysisValueKind::List(sources) = member(snapshot, "source_roots").kind() else {
        panic!("roots")
    };
    for source in sources {
        let AnalysisValueKind::ArtifactRoot(root) = source.kind() else {
            panic!("source root")
        };
        assert_eq!(root.path(), "");
    }
    assert!(!sources[0].publication_eq(&sources[1]));
    assert_eq!(
        member(snapshot, "repeated_root_lookup").as_str(),
        Some("root")
    );
}

#[tokio::test]
async fn genuine_roots_publish_and_unchanged_path_helpers_restore_target_and_exec() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, config(&[])).await;
    check(&first);
    let changed = request(
        &dice,
        &root,
        config(&[(NativeCommandOption::HostCopt, "unused-root-payload")]),
    )
    .await;
    check(&changed);
    assert!(!field(&first, "root").publication_eq(field(&changed, "root")));
    assert_ne!(first, changed);
    assert_eq!(first, request(&dice, &root, config(&[])).await);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn direct_external_generated_label_remains_unsupported() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let error = super::request_configured(
        &dice,
        &root,
        root_epoch(&root),
        true,
        "producer.pre",
        config(&[]),
    )
    .await
    .unwrap_err();
    assert!(error.contains("external repository configured target shape is not supported: @@rules_rust+//:producer.pre"), "{error}");
    fs::remove_dir_all(root).unwrap();
}
