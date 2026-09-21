//! Genuine Files and configured dependency providers through unchanged public
//! create_compilation_context. Only the caller and its inputs are authored.
use slug_build_api_v2::AnalysisValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

const FILES: &[&str] = &[
    "public.h",
    "shared.h",
    "public.inc",
    "private.inc",
    "textual.h",
    "separate.h",
    "ordinary.h",
    "exported.h",
    "changed.h",
];

fn build(root: &std::path::Path, ordinary: &str, exported: &str) {
    fs::write(root.join("rules_rust/BUILD.bazel"), format!(
        "load(':proof.bzl','subject')\n\
         subject(name='ordinary',public=['@@//:{ordinary}'])\n\
         subject(name='exported',public=['@@//:{exported}','@@//:public.inc'],private=['@@//:shared.h','@@//:private.inc'],textual=['@@//:textual.h'])\n\
         subject(name='producer',public=['@@//:public.h','@@//:shared.h','@@//:public.inc'],private=['@@//:shared.h','@@//:private.inc'],textual=['@@//:textual.h'],separate=['@@//:separate.h'],ordinary=[':ordinary'],exported=[':exported'])\n\
         subject(name='consumer',forwarded=':producer')\n"
    )).unwrap();
}

fn workspace() -> PathBuf {
    // Private context kwargs require a rules_cc caller. Do not move or replace
    // any imported upstream function to gain access.
    let root = super::workspace_with(
        "load('@rules_cc//cc/private:header_proof.bzl','subject')\n",
        "",
    );
    fs::write(
        root.join("rules_cc/cc/private/header_proof.bzl"),
        include_str!("cc_headers.bzl"),
    )
    .unwrap();
    for name in FILES {
        fs::write(root.join(name), "// authored header input\n").unwrap();
    }
    fs::write(
        root.join("BUILD.bazel"),
        format!("exports_files({FILES:?})\n"),
    )
    .unwrap();
    build(&root, "ordinary.h", "exported.h");
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    target: &str,
) -> Arc<ConfiguredNodeResult> {
    super::request_configured(dice, root, root_epoch(root), true, target, config(&[]))
        .await
        .unwrap()
}

fn member<'a>(context: &'a AnalysisValue, name: &str) -> &'a AnalysisValue {
    let AnalysisValueKind::Provider(provider) = context.kind() else {
        panic!("unchanged builder must publish its real compilation-context provider")
    };
    provider.field(name).unwrap()
}

fn files(context: &AnalysisValue, name: &str) -> Vec<String> {
    let AnalysisValueKind::List(values) = member(context, name).kind() else {
        panic!("{name} must be an immutable list projection")
    };
    values
        .iter()
        .map(|value| {
            let AnalysisValueKind::Artifact(file) = value.kind() else {
                panic!("{name} must retain real File inputs")
            };
            file.path().into_owned()
        })
        .collect()
}

fn check_direct(context: &AnalysisValue, exported: &str, separate: bool) {
    assert_eq!(
        files(context, "direct_public_headers"),
        ["public.h", "shared.h", exported]
    );
    assert_eq!(files(context, "direct_private_headers"), ["shared.h"]);
    // Explicit textual first; public .inc before private .inc. Exported duplicates
    // are deduplicated independently, and ordinary headers are not merged here.
    assert_eq!(
        files(context, "direct_textual_headers"),
        ["textual.h", "public.inc", "private.inc"]
    );
    let mut direct = vec!["public.h", "shared.h", exported, "shared.h"];
    if separate {
        direct.extend(["separate.h", "separate.h"]);
    }
    assert_eq!(files(context, "direct_headers"), direct);
}

#[tokio::test]
async fn authentic_compilation_context_headers_merge_and_publish_to_consumer() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let producer = request(&dice, &root, "producer").await;
    let consumer = request(&dice, &root, "consumer").await;
    let produced = field(&producer, "context");
    let consumed = field(&consumer, "context");
    check_direct(produced, "exported.h", true);
    // Exported merging copies normalized public/private/textual lists, but not
    // the separate_module_headers of its dependency (HeaderInfo.create:877ff).
    check_direct(consumed, "exported.h", false);
    assert!(produced.publication_eq(field(&consumer, "received")));
    assert_eq!(member(produced, "_header_info"), field(&producer, "alias"));
    assert_eq!(
        member(field(&consumer, "received"), "_header_info"),
        field(&consumer, "received_alias")
    );
    assert_ne!(
        member(consumed, "_header_info"),
        field(&consumer, "received_alias"),
        "authentic merge constructs a fresh HeaderInfo occurrence"
    );
    assert!(producer.actions().is_empty());
    assert!(consumer.actions().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn authentic_header_dependency_changes_and_source_restoration_share_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "consumer").await;
    build(&root, "changed.h", "exported.h");
    let hidden = request(&dice, &root, "consumer").await;
    check_direct(field(&hidden, "context"), "exported.h", false);
    assert!(
        !field(&first, "context").publication_eq(field(&hidden, "context")),
        "ordinary dependency's hidden HeaderInfo edge must affect publication"
    );
    assert!(
        !member(field(&first, "context"), "_header_info")
            .publication_eq(member(field(&hidden, "context"), "_header_info")),
        "hidden dependency change must survive through HeaderInfo, not only generic provider headers"
    );
    build(&root, "ordinary.h", "changed.h");
    let exported = request(&dice, &root, "consumer").await;
    check_direct(field(&exported, "context"), "changed.h", false);
    assert!(!field(&first, "context").publication_eq(field(&exported, "context")));
    build(&root, "ordinary.h", "exported.h");
    let restored = request(&dice, &root, "consumer").await;
    assert_eq!(first, restored);
    fs::remove_dir_all(root).unwrap();
}
