//! Complete unchanged initializer and public compile composition, not execution.
use slug_build_api_v2::AnalysisValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

mod command_expansion;
mod registration;
mod registry;

const PROOF: &str = "rules_cc/cc/private/compile_context_proof.bzl";
const FILES: &[&str] = &[
    "public.h",
    "private.h",
    "textual.h",
    "ordinary.h",
    "implementation.h",
    "changed.h",
    "allocator_library.cc",
    "allocator_library.c",
];

fn declarations(main: bool) -> String {
    let toolchain = if main {
        "@@rules_rust+//:producer"
    } else {
        ":producer"
    };
    let common = format!(
        "toolchain='{toolchain}',public=['@@//:public.h'],private=['@@//:private.h'],textual=['@@//:textual.h'],deps=['@@//:ordinary'],implementation_deps=['@@//:implementation'],visibility=['//visibility:public']"
    );
    format!(
        "load('@rules_cc//cc/private:compile_context_proof.bzl','exec_subject',composition='subject')\n\
         composition(name='ordinary',leaf='dep',public=['@@//:ordinary.h'],visibility=['//visibility:public'])\n\
         composition(name='implementation',leaf='impl',public=['@@//:implementation.h'],visibility=['//visibility:public'])\n\
         composition(name='init',{common})\n\
         composition(name='system',{common},requested=['system_include_paths'])\n\
         composition(name='external',{common},requested=['external_include_paths'])\n\
         exec_subject(name='exec_init',dep=':init',visibility=['//visibility:public'])\n\
         composition(name='matrix',rows=[':init',':system',':external',':exec_init'],visibility=['//visibility:public'])\n\
         composition(name='header_compile',{common},public_compile=True)\n\
         composition(name='source_compile',{common},public_compile=True,sources=['@@//:allocator_library.cc'])\n\
         composition(name='typed_compile',{common},public_compile=True,configured_compile=True,sources=['@@//:allocator_library.cc'],compile_flags=['-DRECIPE_A'])\n\
         composition(name='typed_c',{common},public_compile=True,configured_compile=True,sources=['@@//:allocator_library.c'])\n\
         composition(name='typed_pic',{common},public_compile=True,configured_compile=True,requested=['supports_pic'],sources=['@@//:allocator_library.cc'])\n"
    )
}

fn workspace() -> PathBuf {
    let root = super::cc_toolchain::workspace();
    fs::write(root.join(PROOF), include_str!("compile_context.bzl")).unwrap();
    fs::write(
        root.join("rules_cc/cc/private/compile_registration_proof.bzl"),
        include_str!("compile_context/lifecycle.bzl"),
    )
    .unwrap();
    for name in FILES {
        fs::write(
            root.join(name),
            "// authored regular source input; no compiler runs\n",
        )
        .unwrap();
    }
    let build = root.join("rules_rust/BUILD.bazel");
    let existing = fs::read_to_string(&build).unwrap().replace(
        "name='producer',",
        "name='producer',visibility=['//visibility:public'],",
    );
    fs::write(build, format!("{existing}\n{}\ncomposition(name='main_matrix',rows=['@@//:init','@@//:system','@@//:external','@@//:exec_init'])\n", declarations(false))).unwrap();
    let build = root.join("BUILD.bazel");
    let existing = fs::read_to_string(&build).unwrap();
    fs::write(
        build,
        format!(
            "{existing}\nexports_files({FILES:?})\n{}\n\
             load('@rules_cc//cc/private:compile_registration_proof.bzl','registration_subject','foreign_output')\n\
             foreign_output(name='registration_foreign')\n\
             registration_subject(name='registration_lifecycle',tags=['no-sandbox','ordinary-tag'])\n\
             registration_subject(name='registration_conflict_before',failure='conflict_before')\n\
             registration_subject(name='registration_conflict_after',failure='conflict_after')\n\
             registration_subject(name='registration_conflict_dotd',failure='conflict_dotd')\n\
             registration_subject(name='registration_conflict_pending',failure='conflict_pending')\n",
            declarations(true)
        ),
    )
    .unwrap();
    let build = root.join("BUILD.bazel");
    let mut contents = fs::read_to_string(&build).unwrap();
    for mode in registration::NATIVE_MODES {
        contents.push_str(&format!(
            "registration_subject(name='native_{mode}',failure='{mode}')\n"
        ));
    }
    fs::write(build, contents).unwrap();
    registry::prepare_workspace(&root);
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    registry::request(dice, root, target, configuration).await
}

fn member<'a>(value: &'a AnalysisValue, name: &str) -> &'a AnalysisValue {
    match value.kind() {
        AnalysisValueKind::Provider(value) => value.field(name).unwrap(),
        AnalysisValueKind::Struct(value) => value.get(name).unwrap(),
        _ => panic!("provider/struct field {name}"),
    }
}
fn values(value: &AnalysisValue) -> Vec<AnalysisValue> {
    match value.kind() {
        AnalysisValueKind::Depset(value) => value.to_list(),
        AnalysisValueKind::List(value) => value.to_vec(),
        _ => panic!("depset or immutable list"),
    }
}
fn strings(value: &AnalysisValue) -> Vec<String> {
    values(value)
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}
fn paths(value: &AnalysisValue) -> Vec<String> {
    values(value)
        .iter()
        .map(|v| {
            let AnalysisValueKind::Artifact(file) = v.kind() else {
                panic!("File")
            };
            file.path().into_owned()
        })
        .collect()
}

fn check_context(
    context: &AnalysisValue,
    mode: usize,
    main: bool,
    bin: &str,
    implementation: bool,
    changed: bool,
) {
    // Includes are flattened own-before-dependency; defines use dep-before-own.
    let external = !main && mode == 2;
    let system = mode == 1;
    let deps = if implementation {
        vec!["dep", "impl"]
    } else {
        vec!["dep"]
    };
    let mut includes = if external || system {
        vec![]
    } else {
        vec!["own-include".to_owned()]
    };
    includes.extend(deps.iter().map(|d| format!("{d}-include")));
    assert_eq!(strings(member(context, "includes")), includes);
    let mut systems = if external {
        vec![]
    } else {
        vec!["own-system".to_owned()]
    };
    if system {
        systems.push("own-include".to_owned());
    }
    systems.extend(deps.iter().map(|d| format!("{d}-system")));
    assert_eq!(strings(member(context, "system_includes")), systems);
    let repo = if main { "" } else { "external/rules_rust+" };
    let combined = if main {
        bin.to_owned()
    } else {
        format!("{bin}/{repo}")
    };
    let mut quotes = vec![
        if main {
            ".".to_owned()
        } else {
            repo.to_owned()
        },
        combined.clone(),
        "own-quote".to_owned(),
    ];
    quotes.extend(deps.iter().map(|d| format!("{d}-quote")));
    assert_eq!(strings(member(context, "quote_includes")), quotes);
    assert_eq!(
        strings(member(context, "framework_includes")),
        ["own-framework"]
    );
    assert_eq!(
        strings(member(context, "external_includes")),
        if external {
            vec![
                repo.to_owned(),
                combined,
                "own-quote".to_owned(),
                "own-system".to_owned(),
                "own-include".to_owned(),
            ]
        } else {
            vec![]
        }
    );
    let mut defines = deps
        .iter()
        .map(|d| format!("{d}-define"))
        .collect::<Vec<_>>();
    defines.push("own-define".to_owned());
    assert_eq!(strings(member(context, "defines")), defines);
    assert_eq!(strings(member(context, "local_defines")), ["own-local"]);
    assert_eq!(
        paths(member(context, "direct_public_headers")),
        ["public.h"]
    );
    assert_eq!(
        paths(member(context, "direct_private_headers")),
        ["private.h"]
    );
    assert_eq!(
        paths(member(context, "direct_textual_headers")),
        ["textual.h"]
    );
    let mut headers = paths(member(context, "headers"));
    headers.sort();
    let mut expected = vec![
        "header.h",
        if changed { "changed.h" } else { "ordinary.h" },
        "public.h",
        "private.h",
        "textual.h",
    ];
    if implementation {
        expected.push("implementation.h");
    }
    expected.sort();
    assert_eq!(headers, expected);
    assert!(matches!(
        member(context, "_header_info").kind(),
        AnalysisValueKind::CcHeaderInfo(_)
    ));
}

fn check_matrix(result: &ConfiguredNodeResult, main: bool, changed: bool) {
    assert!(result.actions().is_empty());
    let AnalysisValueKind::List(rows) = field(result, "rows").kind() else {
        panic!("rows")
    };
    assert_eq!(rows.len(), 4);
    for (index, row) in rows.iter().enumerate() {
        let AnalysisValueKind::ArtifactRoot(slug_build_api_v2::AnalysisArtifactRoot::Derived(root)) =
            member(member(row, "snapshot"), "root").kind()
        else {
            panic!("derived root")
        };
        assert_eq!(
            root.configuration().kind(),
            if index == 3 {
                slug_configuration_v2::SlugConfigurationKind::Exec
            } else {
                slug_configuration_v2::SlugConfigurationKind::Target
            }
        );
        check_context(
            member(row, "context"),
            index,
            main,
            root.path(),
            false,
            changed,
        );
        check_context(
            member(row, "implementation"),
            index,
            main,
            root.path(),
            true,
            changed,
        );
        assert_eq!(
            member(member(row, "context"), "_header_info"),
            member(row, "alias")
        );
        assert!(
            member(member(row, "info"), "compilation_context")
                .publication_eq(member(row, "context"))
        );
    }
}

#[tokio::test]
async fn unchanged_initializer_publishes_external_target_exec_dependency_contexts() {
    let root = workspace();
    let dice = registry::dice(&root);
    let result = request(&dice, &root, "matrix", config(&[])).await.unwrap();
    check_matrix(&result, false, false);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn unchanged_initializer_publishes_main_target_exec_dependency_contexts() {
    let root = workspace();
    let dice = registry::dice(&root);
    let result = request(&dice, &root, "main_matrix", config(&[]))
        .await
        .unwrap();
    check_matrix(&result, true, false);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn initializer_context_dependency_and_configuration_restore_in_same_dice() {
    let root = workspace();
    let dice = registry::dice(&root);
    let first = request(&dice, &root, "matrix", config(&[])).await.unwrap();
    // Mutate only workspace-owned source metadata, never selected immutable
    // repository generations. Their four source corpora remain byte-identical.
    let build = root.join("BUILD.bazel");
    let original = fs::read_to_string(&build).unwrap();
    fs::write(
        &build,
        original.replace("public=['@@//:ordinary.h']", "public=['@@//:changed.h']"),
    )
    .unwrap();
    let changed = request(&dice, &root, "matrix", config(&[])).await.unwrap();
    check_matrix(&changed, false, true);
    assert!(!field(&first, "rows").publication_eq(field(&changed, "rows")));
    let first_rows = values(field(&first, "rows"));
    let changed_rows = values(field(&changed, "rows"));
    for (first_row, changed_row) in first_rows.iter().zip(&changed_rows) {
        let before = member(first_row, "context");
        let after = member(changed_row, "context");
        assert_eq!(
            paths(member(before, "direct_headers")),
            paths(member(after, "direct_headers"))
        );
        assert!(
            !member(before, "_header_info").publication_eq(member(after, "_header_info")),
            "ordinary dependency must remain in the published hidden HeaderInfo graph"
        );
    }
    fs::write(&build, original).unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "matrix", config(&[])).await.unwrap()
    );
    let configured = request(
        &dice,
        &root,
        "matrix",
        config(&[(NativeCommandOption::HostCopt, "unused-context-config")]),
    )
    .await
    .unwrap();
    check_matrix(&configured, false, false);
    assert!(!field(&first, "rows").publication_eq(field(&configured, "rows")));
    assert_eq!(
        first,
        request(&dice, &root, "matrix", config(&[])).await.unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn unchanged_public_compile_header_only_succeeds_and_unconfigured_source_is_rejected() {
    let root = workspace();
    let dice = registry::dice(&root);
    let header = request(&dice, &root, "header_compile", config(&[]))
        .await
        .unwrap();
    assert!(header.actions().is_empty());
    let snapshot = field(&header, "snapshot");
    let AnalysisValueKind::ArtifactRoot(root_value) = member(snapshot, "root").kind() else {
        panic!("root")
    };
    check_context(
        field(&header, "context"),
        0,
        false,
        root_value.path(),
        false,
        false,
    );
    let AnalysisValueKind::Tuple(outputs) = member(snapshot, "outputs").kind() else {
        panic!("locally observed outputs")
    };
    assert_eq!(outputs.len(), 3);
    for output in outputs {
        assert!(values(output).is_empty());
    }
    let error = request(&dice, &root, "source_compile", config(&[]))
        .await
        .err()
        .expect("empty feature selection has no configured compiler action");
    assert!(
        error.contains("Expected action_config for 'c++-compile' to be configured"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}
