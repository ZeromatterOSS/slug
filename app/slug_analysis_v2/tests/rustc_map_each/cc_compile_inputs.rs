//! Unchanged get_copts/dotd helpers consuming the complete published toolchain.
use slug_build_api_v2::AnalysisValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

fn workspace() -> PathBuf {
    let root = super::cc_toolchain::workspace();
    fs::write(
        root.join("rules_cc/cc/private/compile_inputs_proof.bzl"),
        include_str!("cc_compile_inputs.bzl"),
    )
    .unwrap();
    let build_path = root.join("rules_rust/BUILD.bazel");
    let existing = fs::read_to_string(&build_path).unwrap();
    fs::write(build_path, format!("load('@rules_cc//cc/private:compile_inputs_proof.bzl', compile_inputs='subject')\n{existing}\ncompile_inputs(name='inputs',toolchain=':producer',sources=['@@//:sample.c','@@//:sample.cc','@@//:sample.h','@@//:plain'])\n")).unwrap();
    for name in ["sample.c", "sample.cc", "sample.h", "plain"] {
        fs::write(root.join(name), "inert source; no compiler runs\n").unwrap();
    }
    let build_path = root.join("BUILD.bazel");
    let existing = fs::read_to_string(&build_path).unwrap();
    fs::write(
        build_path,
        format!("{existing}\nexports_files(['sample.c','sample.cc','sample.h','plain'])\n"),
    )
    .unwrap();
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    configuration: ConfigurationKey,
) -> Arc<ConfiguredNodeResult> {
    super::request_configured(dice, root, root_epoch(root), true, "inputs", configuration)
        .await
        .unwrap()
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
fn check(value: &ConfiguredNodeResult, changed: bool) {
    assert!(value.actions().is_empty());
    let snapshot = field(value, "snapshot");
    let common = if changed {
        vec![
            "-fomit-frame-pointer",
            "-fasynchronous-unwind-tables",
            "-DNO_FRAME_POINTER",
            "general,raw",
            "",
            "general,raw",
        ]
    } else {
        vec![]
    };
    let AnalysisValueKind::List(rows) = member(snapshot, "rows").kind() else {
        panic!("rows")
    };
    assert_eq!(rows.len(), 4);
    for (row, extension, language) in [
        (0, "c", "c"),
        (1, "cc", "cxx"),
        (2, "h", "cxx"),
        (3, "", ""),
    ] {
        let AnalysisValueKind::Tuple(values) = rows[row].kind() else {
            panic!("row")
        };
        assert_eq!(values[0].as_str(), Some(extension));
        let mut expected = common.clone();
        if changed && !language.is_empty() {
            expected.push(if language == "c" {
                "only-c"
            } else {
                "only-cxx"
            });
        }
        expected.push("rule-common");
        if !language.is_empty() {
            expected.push(if language == "c" {
                "rule-c"
            } else {
                "rule-cxx"
            });
        }
        assert_eq!(strings(&values[1]), expected);
    }
    let mut expected = common;
    if changed {
        expected.push("only-cxx");
    }
    expected.extend(["rule-common", "rule-cxx"]);
    assert_eq!(strings(member(snapshot, "override")), expected);
    for name in ["dotd", "cpp_dotd"] {
        assert!(
            matches!(member(snapshot, name).kind(), AnalysisValueKind::Boolean(value) if value == !changed)
        );
    }
    for name in ["showincludes_dotd", "disabled_dotd"] {
        assert!(matches!(
            member(snapshot, name).kind(),
            AnalysisValueKind::Boolean(false)
        ));
    }
    for name in ["save_temps", "process_headers"] {
        assert!(
            matches!(member(snapshot, name).kind(), AnalysisValueKind::Boolean(value) if value == changed)
        );
    }
}

fn changed_configuration() -> ConfigurationKey {
    config(&[
        (NativeCommandOption::Copt, "general,raw"),
        (NativeCommandOption::Copt, ""),
        (NativeCommandOption::Copt, "general,raw"),
        (NativeCommandOption::Conlyopt, "only-c"),
        (NativeCommandOption::Cxxopt, "only-cxx"),
        (NativeCommandOption::ExperimentalOmitfp, "true"),
        (NativeCommandOption::CcDotdFiles, "false"),
        (NativeCommandOption::SaveTemps, "true"),
        (NativeCommandOption::ProcessHeadersInDependencies, "true"),
    ])
}

#[tokio::test]
async fn unchanged_compile_helpers_consume_real_published_toolchain_options() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    check(request(&dice, &root, config(&[])).await.as_ref(), false);
    check(
        request(&dice, &root, changed_configuration())
            .await
            .as_ref(),
        true,
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn compile_inputs_and_unused_host_options_restore_in_same_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, config(&[])).await;
    let changed = request(&dice, &root, changed_configuration()).await;
    check(&changed, true);
    assert!(!field(&first, "snapshot").publication_eq(field(&changed, "snapshot")));
    let unused = request(
        &dice,
        &root,
        config(&[(NativeCommandOption::HostCopt, "unused-host-copt")]),
    )
    .await;
    assert!(field(&first, "snapshot").publication_eq(field(&unused, "snapshot")));
    assert!(!field(&first, "toolchain").publication_eq(field(&unused, "toolchain")));
    let restored = request(&dice, &root, config(&[])).await;
    assert_eq!(first, restored);
    fs::remove_dir_all(root).unwrap();
}
