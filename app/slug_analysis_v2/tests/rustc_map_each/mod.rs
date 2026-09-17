use super::*;

include!("sources.rs");

const REPOSITORIES: &[(&str, &str)] = &[
    ("rules_rust+", "rules_rust"),
    ("rules_cc+", "rules_cc"),
    ("bazel_skylib+", "bazel_skylib"),
    ("cc_compatibility_proxy+", "cc_compatibility_proxy"),
];

const BUILTIN_MODULES: &[(&str, &str)] = &[
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("platforms", "1.0.0"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
];

fn workspace() -> PathBuf {
    let workspace = scratch();
    let mut root_module = "module(name = 'root')\n".to_owned();
    for (_, repository) in REPOSITORIES {
        root_module.push_str(&format!(
            "bazel_dep(name = '{repository}', version = '1.0')\nlocal_path_override(module_name = '{repository}', path = '{repository}')\n"
        ));
        fs::create_dir_all(workspace.join(repository)).unwrap();
        let mut module = format!("module(name = '{repository}', version = '1.0')\n");
        let dependencies: &[&str] = match *repository {
            "rules_rust" => &["rules_cc", "bazel_skylib"],
            "rules_cc" => &["cc_compatibility_proxy", "bazel_skylib", "platforms"],
            "cc_compatibility_proxy" => &["rules_cc"],
            _ => &[],
        };
        for dependency in dependencies {
            module.push_str(&format!(
                "bazel_dep(name = '{dependency}', version = '1.0')\n"
            ));
        }
        fs::write(workspace.join(repository).join("MODULE.bazel"), module).unwrap();
        fs::write(workspace.join(repository).join("REPO.bazel"), "").unwrap();
        fs::write(workspace.join(repository).join(".bazelignore"), "").unwrap();
    }
    // Keep the real built-in bazel_tools mapping. Its unrelated module
    // dependencies need only test module declarations, never rule bodies.
    for (name, version) in BUILTIN_MODULES {
        fs::create_dir_all(workspace.join(name)).unwrap();
        fs::write(
            workspace.join(name).join("MODULE.bazel"),
            format!("module(name = '{name}', version = '{version}')\n"),
        )
        .unwrap();
        root_module.push_str(&format!(
            "local_path_override(module_name = '{name}', path = '{name}')\n"
        ));
        if matches!(
            *name,
            "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
        ) {
            root_module.push_str(&format!(
                "bazel_dep(name = '{name}', version = '{version}')\n"
            ));
        }
    }
    fs::write(workspace.join("MODULE.bazel"), root_module).unwrap();
    for (path, source) in RUSTC_MAP_EACH_SOURCES {
        let path = workspace.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path.parent().unwrap().join("BUILD.bazel"), "").unwrap();
        fs::write(path, source).unwrap();
    }
    fs::write(workspace.join("lib.rs"), "pub fn probe() {}\n").unwrap();
    fs::write(
        workspace.join("rules_rust/proof.bzl"),
        include_str!("subject.bzl"),
    )
    .unwrap();
    fs::write(
        workspace.join("rules_rust/BUILD.bazel"),
        "load(':proof.bzl', 'subject')\nsubject(name = 'subject', src = '@@//:lib.rs')\n",
    )
    .unwrap();
    fs::write(workspace.join("BUILD.bazel"), "exports_files(['lib.rs'])\n").unwrap();
    workspace
}

async fn request(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let mut updater = dice.updater_with_data(UserComputationData {
        cycle_detector: Some(analysis_cycle_detector()),
        ..Default::default()
    });
    let epoch = root_epoch(workspace);
    updater
        .changed_to(slug_workspace_v2::path_observation_shards(&epoch))
        .unwrap();
    let builtin_repositories = BUILTIN_MODULES
        .iter()
        .map(|(name, _)| (format!("{name}+"), *name))
        .collect::<Vec<_>>();
    let mut repositories = REPOSITORIES.to_vec();
    repositories.extend(
        builtin_repositories
            .iter()
            .map(|(canonical, name)| (canonical.as_str(), *name)),
    );
    inject_root_target_inputs_with_policy(&mut updater, workspace, epoch, &repositories, false);
    slug_bzlmod_v2::inject_registry_request_inputs(
        &mut updater,
        workspace,
        slug_bzlmod_v2::RegistryUrls::new([format!("file://{}/registry", workspace.display())]),
        slug_bzlmod_v2::RegistryRequestGeneration(0),
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let key = match prepare_configured_node_analysis(
        &mut transaction,
        NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
        CanonicalLabel::parse("@@rules_rust+//:subject").unwrap(),
        typed_action_test_configuration(),
    )
    .await
    {
        AnalysisPreparationOutcome::Complete(Ok(key)) => key,
        other => return Err(format!("callback configuration preparation: {other:?}")),
    };
    match transaction
        .compute(&key)
        .await
        .map_err(|error| error.to_string())?
    {
        AnalysisPreparationOutcome::Complete(value) => value
            .as_ref()
            .as_ref()
            .cloned()
            .map_err(ToString::to_string),
        other => Err(format!("configured callback proof needs input: {other:?}")),
    }
}

#[tokio::test]
async fn pinned_rustc_arguments_publish_and_restore_after_source_edit() {
    let workspace = workspace();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let first = request(&dice, &workspace).await.unwrap();
    assert_eq!(first.actions().len(), 1);
    let spawn = first.actions()[0].spawn_spec().unwrap();
    assert_eq!(
        spawn.environment().fixed().get("CARGO_MANIFEST_DIR"),
        Some("${pwd}/external/rules_rust+")
    );
    let argv = spawn.render_argv();
    assert!(
        argv.windows(3)
            .any(|args| args == ["--", "rustc", "lib.rs"]),
        "{argv:?}"
    );
    let RetainedCommandLineSegment::ArgsSnapshot(rustc) = &spawn.command_line().segments()[2]
    else {
        panic!("rustc flags must retain their own Args snapshot");
    };
    assert_eq!(rustc.recipe().render()[0], "lib.rs");
    assert!(
        rustc
            .recipe()
            .render_write_content()
            .starts_with("lib.rs\n")
    );
    assert_eq!(rustc.param_file().unwrap().flag_format(), "@%s");
    assert!(rustc.param_file().unwrap().use_always());

    let source_path = workspace.join("rules_rust/rust/private/rustc.bzl");
    let source = fs::read_to_string(&source_path).unwrap();
    // Preserve label, callsite and callback name; only loaded source bytes change.
    fs::write(&source_path, format!("{source}\n")).unwrap();
    let error = request(&dice, &workspace).await.unwrap_err();
    assert!(
        error.contains("Args.add_all callback forms are not supported"),
        "{error}"
    );
    assert!(error.contains("rustc.bzl"), "{error}");

    fs::write(&source_path, source).unwrap();
    let restored = request(&dice, &workspace).await.unwrap();
    assert!(
        first == restored,
        "source A/B/A must restore the configured result"
    );

    let proof_path = workspace.join("rules_rust/proof.bzl");
    let proof = fs::read_to_string(&proof_path).unwrap();
    for (from, to, expected) in [
        (
            "root_path = \"unused-for-regular-file.rs\"",
            "root_path = False",
            "requires a string root path",
        ),
        (
            "transitive_crates = depset()",
            "transitive_crates = depset([ctx.attr.src])",
            "Args.add_all callback forms are not supported",
        ),
    ] {
        assert_eq!(proof.matches(from).count(), 1);
        fs::write(&proof_path, proof.replacen(from, to, 1)).unwrap();
        let error = request(&dice, &workspace).await.unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
    fs::write(&proof_path, proof).unwrap();
    assert!(first == request(&dice, &workspace).await.unwrap());
    fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn pinned_rustc_file_dirnames_preserve_order_and_generated_root() {
    let workspace = workspace();
    let files = [
        "lib.rs",
        "std/a/libstd.rlib",
        "std/a/liballoc.rlib",
        "std/b/libcore.rlib",
        "sysroot/anchor",
    ];
    for name in files {
        let path = workspace.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "fixture\n").unwrap();
    }
    fs::write(
        workspace.join("BUILD.bazel"),
        format!("exports_files({files:?})\n"),
    )
    .unwrap();
    let build = "load(':proof.bzl', 'subject')\nsubject(name = 'subject', src = '@@//:lib.rs', stdlib = ['@@//:std/a/libstd.rlib', '@@//:std/a/liballoc.rlib', '@@//:std/b/libcore.rlib'], sysroot = '@@//:sysroot/anchor', generated = False)\n";
    let build_path = workspace.join("rules_rust/BUILD.bazel");
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut initial = None;
    for generated in [false, true, false] {
        fs::write(
            &build_path,
            build.replace(
                "generated = False",
                if generated {
                    "generated = True"
                } else {
                    "generated = False"
                },
            ),
        )
        .unwrap();
        let result = request(&dice, &workspace).await.unwrap();
        assert_eq!(result.actions().len(), if generated { 2 } else { 1 });
        let spawn = result
            .actions()
            .iter()
            .find_map(|action| action.spawn_spec())
            .unwrap();
        let argv = spawn.render_argv();
        let root = if generated {
            "generated/input.rs"
        } else {
            "lib.rs"
        };
        assert!(
            argv.windows(3).any(|args| args == ["--", "rustc", root]),
            "{argv:?}"
        );
        assert_eq!(argv.iter().filter(|arg| *arg == "--out-dir=out").count(), 1);
        assert_eq!(
            argv.iter()
                .filter(|arg| *arg == "--sysroot=sysroot")
                .count(),
            1
        );
        let search = argv
            .windows(2)
            .filter(|args| args[0] == "-L")
            .map(|args| args[1].as_str())
            .collect::<Vec<_>>();
        assert_eq!(search, ["std/a", "std/b"]);
        if generated {
            assert!(
                argv.windows(2)
                    .any(|args| args == ["--subst", "cargo_manifest_dir=out"]),
                "{argv:?}"
            );
            assert_eq!(
                spawn.environment().fixed().get("CARGO_MANIFEST_DIR"),
                Some("${pwd}/${cargo_manifest_dir}")
            );
        }
        let RetainedCommandLineSegment::ArgsSnapshot(rustc) = &spawn.command_line().segments()[2]
        else {
            panic!("missing rustc Args")
        };
        let bytes = rustc.recipe().render_write_content();
        assert!(bytes.starts_with(&format!("{root}\n")));
        assert!(bytes.contains("\n--out-dir=out\n"));
        assert!(bytes.contains("\n-L\nstd/a\n-L\nstd/b\n"));
        assert!(bytes.contains("\n--sysroot=sysroot\n"));
        if !generated {
            if let Some(first) = &initial {
                assert_eq!(first, &result);
            } else {
                initial = Some(result);
            }
        }
    }
    let proof_path = workspace.join("rules_rust/proof.bzl");
    let proof = fs::read_to_string(&proof_path).unwrap();
    for (from, to, expected) in [
        (
            "rust_std = stdlib",
            "rust_std = depset(['not-a-file'])",
            "action inputs require a depset of File",
        ),
        (
            "sysroot_anchor = ctx.attr.sysroot",
            "sysroot_anchor = 'not-a-file'",
            "Args file dirname mapping requires Files",
        ),
    ] {
        assert_eq!(proof.matches(from).count(), 1);
        fs::write(&proof_path, proof.replace(from, to)).unwrap();
        let error = request(&dice, &workspace).await.unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
    fs::remove_dir_all(workspace).unwrap();
}
