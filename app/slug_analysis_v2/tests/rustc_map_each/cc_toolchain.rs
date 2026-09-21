//! Full unchanged CcToolchainInfo constructor and dependency publication. The
//! caller supplies inert tool/FDO inputs, not a production cc_toolchain rule.
use slug_build_api_v2::AnalysisValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

const SOURCE: &str = "rules_cc/cc/private/rules_impl/cc_toolchain_info.bzl";

fn build(root: &std::path::Path, static_file: &str, unused: &str) {
    fs::write(
        root.join("rules_rust/BUILD.bazel"),
        format!(
            "load(':proof.bzl','subject')\n\
         subject(name='producer',static_file='@@//:{static_file}',unused='{unused}')\n\
         subject(name='consumer',dep=':producer')\n\
         subject(name='empty',runtime='empty')\nsubject(name='empty_consumer',dep=':empty')\n\
         subject(name='none',runtime='none')\nsubject(name='none_consumer',dep=':none')\n\
         subject(name='missing_static',dep=':none',failure='static')\n\
         subject(name='missing_dynamic',dep=':none',failure='dynamic')\n"
        ),
    )
    .unwrap();
}

pub(super) fn workspace() -> PathBuf {
    let root = super::workspace_with(
        "load('@rules_cc//cc/private:toolchain_proof.bzl','make_subject')\n\
         load('//rust/private:rustc.bzl','get_linker_and_args')\n\
         subject = make_subject(get_linker_and_args)\n",
        "",
    );
    fs::write(
        root.join("rules_cc/cc/private/toolchain_proof.bzl"),
        include_str!("cc_toolchain.bzl"),
    )
    .unwrap();
    let files = ["header.h", "static.a", "dynamic.so", "other.a"];
    for name in files {
        fs::write(root.join(name), "authored inert input\n").unwrap();
    }
    fs::write(
        root.join("BUILD.bazel"),
        format!("exports_files({files:?})\n"),
    )
    .unwrap();
    build(&root, "static.a", "A");
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    super::request_configured(dice, root, root_epoch(root), true, target, configuration).await
}

fn member<'a>(value: &'a AnalysisValue, name: &str) -> &'a AnalysisValue {
    match value.kind() {
        AnalysisValueKind::Provider(provider) => provider.field(name).unwrap(),
        AnalysisValueKind::Struct(values) => values.get(name).unwrap(),
        _ => panic!("expected provider or struct for {name}"),
    }
}

fn list(value: &AnalysisValue) -> Vec<&str> {
    let AnalysisValueKind::List(values) = value.kind() else {
        panic!("string list")
    };
    values.iter().map(|v| v.as_str().unwrap()).collect()
}

fn file_paths(value: &AnalysisValue) -> Vec<String> {
    let AnalysisValueKind::Depset(values) = value.kind() else {
        panic!("File depset")
    };
    values
        .to_list()
        .iter()
        .map(|value| {
            let AnalysisValueKind::Artifact(file) = value.kind() else {
                panic!("File member")
            };
            file.path().into_owned()
        })
        .collect()
}

fn check_query(result: &ConfiguredNodeResult, force_pic: bool, user: &str) {
    let snapshot = field(result, "snapshot");
    assert_eq!(member(snapshot, "tool").as_str(), Some("toolchain/bin/gcc"));
    assert!(matches!(
        member(snapshot, "direct").kind(),
        AnalysisValueKind::Boolean(false)
    ));
    let mut expected = Vec::new();
    if force_pic {
        expected.push("-pie");
    }
    expected.extend([user, "--legacy", "--sysroot=/sysroot"]);
    assert_eq!(list(member(snapshot, "argv")), expected);
    assert!(
        matches!(member(snapshot, "pic_without_feature").kind(), AnalysisValueKind::Boolean(value) if value == force_pic)
    );
    assert!(matches!(
        member(snapshot, "pic_with_feature").kind(),
        AnalysisValueKind::Boolean(true)
    ));
    let AnalysisValueKind::Dictionary(environment) = member(snapshot, "env").kind() else {
        panic!("environment")
    };
    assert_eq!(environment.len(), 1);
    assert_eq!(environment[0].0.as_str(), Some("SYSROOT"));
    assert_eq!(environment[0].1.as_str(), Some("/sysroot"));
    assert!(file_paths(member(snapshot, "disabled_static")).is_empty());
    assert!(file_paths(member(snapshot, "disabled_dynamic")).is_empty());
    assert!(result.actions().is_empty());
}

fn options(force_pic: bool, user: &str) -> ConfigurationKey {
    config(&[
        (
            NativeCommandOption::ForcePic,
            if force_pic { "true" } else { "false" },
        ),
        (NativeCommandOption::Linkopt, user),
    ])
}

#[tokio::test]
async fn complete_cc_toolchain_provider_and_original_functions_survive_publication() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for force_pic in [false, true] {
        let producer = request(&dice, &root, "producer", options(force_pic, "--user"))
            .await
            .unwrap();
        let consumer = request(&dice, &root, "consumer", options(force_pic, "--user"))
            .await
            .unwrap();
        check_query(&producer, force_pic, "--user");
        check_query(&consumer, force_pic, "--user");
        let tc = field(&consumer, "toolchain");
        assert!(field(&producer, "toolchain").publication_eq(tc));
        let AnalysisValueKind::Provider(provider) = tc.kind() else {
            panic!("complete provider")
        };
        // All upstream fields, including unused metadata, survive; this is not
        // a hand-built projection of the handful the linker happens to read.
        let source = include_str!(
            "../fixtures/rustc-map-each/rules_cc/cc/private/rules_impl/cc_toolchain_info.bzl"
        );
        let schema = source
            .split("fields = {")
            .nth(1)
            .unwrap()
            .split("},  # buildifier")
            .next()
            .unwrap();
        for line in schema.lines() {
            if let Some(rest) = line.trim().strip_prefix('"') {
                if let Some((name, _)) = rest.split_once("\":") {
                    assert!(
                        provider.field(name).is_some(),
                        "missing complete provider field {name}"
                    );
                }
            }
        }
        assert_eq!(member(tc, "compiler").as_str(), Some("gcc"));
        assert_eq!(member(tc, "_abi").as_str(), Some("proof-abi"));
        let context = member(member(tc, "_cc_info"), "compilation_context");
        let AnalysisValueKind::List(headers) = member(context, "direct_public_headers").kind()
        else {
            panic!("real context headers")
        };
        assert_eq!(headers.len(), 1);
        assert!(matches!(headers[0].kind(), AnalysisValueKind::Artifact(_)));
        assert!(matches!(
            member(tc, "_toolchain_features").kind(),
            AnalysisValueKind::CcToolchainFeatures(_)
        ));
        assert!(matches!(
            member(tc, "_build_variables").kind(),
            AnalysisValueKind::CcToolchainVariables(_)
        ));
        assert_eq!(
            file_paths(member(field(&consumer, "snapshot"), "static")),
            ["static.a"]
        );
        assert_eq!(
            file_paths(member(field(&consumer, "snapshot"), "dynamic")),
            ["dynamic.so"]
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn complete_cc_toolchain_runtime_capture_none_empty_and_enabled_errors() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let empty = request(&dice, &root, "empty_consumer", options(false, "--user"))
        .await
        .unwrap();
    check_query(&empty, false, "--user");
    for name in ["static", "dynamic"] {
        assert!(file_paths(member(field(&empty, "snapshot"), name)).is_empty());
    }
    let none = request(&dice, &root, "none_consumer", options(false, "--user"))
        .await
        .unwrap();
    check_query(&none, false, "--user");
    for name in ["static", "dynamic"] {
        assert!(matches!(
            member(field(&none, "snapshot"), name).kind(),
            AnalysisValueKind::None
        ));
        let error = request(
            &dice,
            &root,
            &format!("missing_{name}"),
            options(false, "--user"),
        )
        .await
        .err()
        .expect("enabled None capture must fail");
        assert!(error.contains(&format!("Toolchain supports embedded runtimes, but didn't provide {name}_runtime_lib attribute.")), "{error}");
    }
    assert!(!field(&none, "toolchain").publication_eq(field(&empty, "toolchain")));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn complete_cc_toolchain_capture_payload_and_configuration_restore_in_same_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let a = options(false, "--A");
    let first = request(&dice, &root, "consumer", a.clone()).await.unwrap();
    build(&root, "other.a", "A");
    let capture = request(&dice, &root, "consumer", a.clone()).await.unwrap();
    assert_eq!(
        file_paths(member(field(&capture, "snapshot"), "static")),
        ["other.a"]
    );
    assert!(!field(&first, "static_alias").publication_eq(field(&capture, "static_alias")));
    build(&root, "static.a", "B");
    let unused = request(&dice, &root, "consumer", a.clone()).await.unwrap();
    assert!(field(&first, "snapshot").publication_eq(field(&unused, "snapshot")));
    assert!(!field(&first, "toolchain").publication_eq(field(&unused, "toolchain")));
    build(&root, "static.a", "A");
    let configured = request(&dice, &root, "consumer", options(true, "--B"))
        .await
        .unwrap();
    check_query(&configured, true, "--B");
    assert!(!field(&first, "cpp_alias").publication_eq(field(&configured, "cpp_alias")));
    let restored = request(&dice, &root, "consumer", a).await.unwrap();
    assert_eq!(first, restored);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn complete_cc_toolchain_changed_source_fails_closed_then_reauthenticates() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let original = request(&dice, &root, "consumer", options(false, "--user"))
        .await
        .unwrap();
    let path = root.join(SOURCE);
    let source = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("{source}\n")).unwrap();
    let error = request(&dice, &root, "consumer", options(false, "--user"))
        .await
        .err()
        .expect("unpinned constructor must not serialize functions");
    assert!(
        error.contains("function") || error.contains("callable") || error.contains("authenticated"),
        "{error}"
    );
    fs::write(&path, source).unwrap();
    let restored = request(&dice, &root, "consumer", options(false, "--user"))
        .await
        .unwrap();
    check_query(&restored, false, "--user");
    assert!(field(&original, "snapshot").publication_eq(field(&restored, "snapshot")));
    // Reloaded module globals may carry a new identity token: successful
    // reauthentication is required, not alpha-equality with the old function.
    fs::remove_dir_all(root).unwrap();
}
