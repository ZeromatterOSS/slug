//! Complete unchanged Rust helper and public cc_common load closure. The .bzl
//! subject supplies explicit toolchain/FDO envelopes, not full production tools.
use slug_build_api_v2::AnalysisValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

const EXE: &str = "c++-link-executable";
const NODEPS: &str = "c++-link-nodeps-dynamic-library";
const ARCHIVE: &str = "c++-link-static-library";
const SHARED: &str = "c++-link-dynamic-library";
const CASES: &[(&str, &str)] = &[
    ("bin", EXE),
    ("dylib", NODEPS),
    ("lib", NODEPS), // rustc.bzl:403 tests substring membership in ("dylib").
    ("staticlib", ARCHIVE),
    ("cdylib", SHARED),
    ("proc-macro", SHARED),
    ("rlib", EXE), // add_flags_for_binary takes precedence over invalid crate type.
];

fn workspace() -> PathBuf {
    super::workspace_with(
        include_str!("cc_linker.bzl"),
        "subject(name='subject')\nsubject(name='direct',preference='rust')\nsubject(name='indirect',preference='rust',driver='indirect')\nsubject(name='bad_rlib',invalid='rlib')\nsubject(name='unknown',invalid='unknown')\n",
    )
}

async fn request(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    super::request_configured(
        dice,
        workspace,
        root_epoch(workspace),
        true,
        target,
        configuration,
    )
    .await
}

fn row<'a>(result: &'a ConfiguredNodeResult, name: &str, member: &str) -> &'a AnalysisValue {
    let AnalysisValueKind::Dictionary(rows) = field(result, "rows").kind() else {
        panic!("rows must retain a dict")
    };
    let value = &rows
        .iter()
        .find(|(key, _)| key.as_str() == Some(name))
        .unwrap()
        .1;
    let AnalysisValueKind::Struct(fields) = value.kind() else {
        panic!("row must retain a struct")
    };
    fields.get(member).unwrap()
}

fn argv<'a>(result: &'a ConfiguredNodeResult, name: &str) -> Vec<&'a str> {
    let AnalysisValueKind::List(values) = row(result, name, "args").kind() else {
        panic!("argv must retain a list")
    };
    values.iter().map(|v| v.as_str().unwrap()).collect()
}

fn environment(result: &ConfiguredNodeResult, name: &str, action: &str) {
    let AnalysisValueKind::Dictionary(values) = row(result, name, "env").kind() else {
        panic!("environment must retain a dictionary")
    };
    let values: Vec<_> = values
        .iter()
        .map(|(k, v)| (k.as_str().unwrap(), v.as_str().unwrap()))
        .collect();
    assert_eq!(values, [("ACTION", action), ("LIB", "one;two")]);
}

fn expected(action: &str, user: &[&str]) -> Vec<String> {
    // cc_toolchain_config_info.bzl:111–112 places authored features before
    // get_features_to_appear_last (legacy_features.bzl:1048), including sysroot.
    // Rust appends LIB paths after that entire expansion.
    let mut args: Vec<String> = if action == ARCHIVE {
        vec!["rcsD".into()]
    } else {
        let mut values = Vec::new();
        if matches!(action, NODEPS | SHARED) {
            values.push("-shared".into());
        }
        for path in ["runtime", "other"] {
            values.extend([
                "-Xlinker".into(),
                "-rpath".into(),
                "-Xlinker".into(),
                format!("$ORIGIN/{path}"),
            ]);
        }
        values.extend(
            user.iter()
                .filter(|flag| action == EXE || !matches!(**flag, "-pie" | "-Wl,-pie"))
                .map(|flag| (*flag).to_owned()),
        );
        values.push("--legacy".into());
        values
    };
    args.push(format!("--action={action}"));
    if action == ARCHIVE {
        args.extend(user.iter().map(|flag| format!("--static-user={flag}")));
    } else {
        args.push("--sysroot=/sysroot".into());
    }
    args.extend(["-LIBPATH:one".into(), "-LIBPATH:two".into()]);
    args
}

#[tokio::test]
async fn authentic_rust_linker_action_matrix_and_invalid_crates() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let flags = [
        "-pie",
        "-Wl,-pie",
        "-pie=keep",
        "-Wl,-pie,keep",
        "--user",
        "--user",
        "",
    ];
    let options: Vec<_> = flags
        .iter()
        .map(|value| (NativeCommandOption::Linkopt, *value))
        .collect();
    let result = request(&dice, &root, "subject", config(&options))
        .await
        .unwrap();
    for &(name, action) in CASES {
        assert_eq!(argv(&result, name), expected(action, &flags), "{name}");
        assert_eq!(
            row(&result, name, "tool").as_str(),
            Some(if action == ARCHIVE {
                "toolchain/bin/ar"
            } else {
                "toolchain/bin/gcc"
            })
        );
        assert!(matches!(
            row(&result, name, "direct").kind(),
            AnalysisValueKind::Boolean(false)
        ));
        environment(&result, name, action);
    }
    assert!(matches!(
        field(&result, "configuration").kind(),
        AnalysisValueKind::CcFeatureConfiguration(_)
    ));
    assert!(result.actions().is_empty());
    for (target, phrase) in [
        ("bad_rlib", "Invalid `crate_type` for linking action: rlib"),
        ("unknown", "Unknown `crate_type`: unknown"),
    ] {
        let error = request(&dice, &root, target, config(&[]))
            .await
            .err()
            .expect("invalid crate target must fail");
        assert!(error.contains(phrase), "{error}");
        assert!(error.contains("rustc.bzl"), "{error}");
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn authentic_rust_preferred_linker_filters_cc_flags_and_preserves_environment() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let flags = [
        "-Lnative",
        "--sysroot",
        "/two-part",
        "--dynamic-linker",
        "/loader",
        "-Wl,-rpath,/wrapped",
        "-mmacos-version-min=13.1",
        "--ordinary",
        "--rpath=/plain",
    ];
    let options: Vec<_> = flags
        .iter()
        .map(|value| (NativeCommandOption::Linkopt, *value))
        .collect();
    for (target, direct) in [("direct", true), ("indirect", false)] {
        let result = request(&dice, &root, target, config(&options))
            .await
            .unwrap();
        let AnalysisValueKind::Artifact(linker) = field(&result, "linker").kind() else {
            panic!("fallback linker must be a real File")
        };
        for &(name, action) in CASES {
            let mut filtered = if action == ARCHIVE {
                vec![]
            } else {
                // Upstream keeps the bare -rpath from legacy -Xlinker pairs,
                // but drops its separate operand. Do not repair that quirk here.
                let mut values = vec![
                    "-rpath",
                    "-rpath",
                    "-Lnative",
                    "--sysroot",
                    "/two-part",
                    "--dynamic-linker",
                    "/loader",
                ];
                if direct {
                    values.push("-rpath,/wrapped");
                }
                values.extend([
                    "-macos_version_min",
                    "13.1",
                    "--rpath=/plain",
                    "--sysroot=/sysroot",
                ]);
                values
            };
            filtered.extend(["-LIBPATH:one", "-LIBPATH:two"]);
            assert_eq!(argv(&result, name), filtered, "{target}/{name}");
            assert_eq!(
                row(&result, name, "tool").as_str().unwrap(),
                linker.path().as_ref()
            );
            assert!(
                matches!(row(&result, name, "direct").kind(), AnalysisValueKind::Boolean(value) if value == direct)
            );
            environment(&result, name, action);
        }
        assert!(result.actions().is_empty());
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn authentic_rust_linker_command_inputs_restore_in_same_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let a = config(&[(NativeCommandOption::Linkopt, "--A")]);
    let first = request(&dice, &root, "subject", a.clone()).await.unwrap();
    let changed = request(
        &dice,
        &root,
        "subject",
        config(&[
            (NativeCommandOption::Linkopt, "--B"),
            (NativeCommandOption::ExperimentalOmitfp, "true"),
        ]),
    )
    .await
    .unwrap();
    assert_ne!(first, changed);
    assert_eq!(argv(&first, "bin"), expected(EXE, &["--A"]));
    assert_eq!(
        argv(&changed, "bin"),
        expected(EXE, &["--B", "-Wl,--eh-frame-hdr"])
    );
    let host = config(&[
        (NativeCommandOption::Linkopt, "--A"),
        (NativeCommandOption::HostLinkopt, "--unused-host"),
    ]);
    assert_ne!(
        a, host,
        "unused host flags remain structural configuration input"
    );
    let unused = request(&dice, &root, "subject", host).await.unwrap();
    assert_eq!(argv(&first, "bin"), argv(&unused, "bin"));
    assert_ne!(
        first, unused,
        "configured identity must retain unused host flags"
    );
    let restored = request(&dice, &root, "subject", a).await.unwrap();
    assert_eq!(first, restored);
    fs::remove_dir_all(root).unwrap();
}
