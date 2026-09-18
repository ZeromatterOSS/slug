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
    let expanded = spawn.expand_forced_param_files().unwrap();
    assert_eq!(expanded.param_files().len(), 1);
    let file = &expanded.param_files()[0];
    assert_eq!(file.path(), "out/probe.rlib-0.params");
    assert_eq!(
        file.bytes(),
        rustc.recipe().render_write_content().as_bytes()
    );
    assert!(file.bytes().starts_with(b"lib.rs\n"));
    let mut expected = spawn.invocation().render_prefix();
    for (index, segment) in spawn.command_line().segments().iter().enumerate() {
        if index == 2 {
            expected.push("@out/probe.rlib-0.params".to_owned());
        } else {
            expected.extend(
                slug_build_api_v2::RetainedCommandLine::new(vec![segment.clone()]).render(),
            );
        }
    }
    assert_eq!(expanded.argv(), expected);

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
            "transitive_crates = transitive_crates",
            "transitive_crates = depset([ctx.attr.src])",
            "requires pinned CrateInfo",
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

#[tokio::test]
async fn pinned_rustc_dependencies_select_aliases_metadata_and_restore() {
    let workspace = workspace();
    let files = [
        "deps/a/liba.rlib",
        "deps/a/liba.rmeta",
        "deps/b/libb.rlib",
        "deps/b/libb.rmeta",
        "deps/b/libc.rlib",
    ];
    for name in files {
        let path = workspace.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "dependency fixture\n").unwrap();
    }
    fs::write(
        workspace.join("BUILD.bazel"),
        format!("exports_files(['lib.rs'] + {files:?})\n"),
    )
    .unwrap();
    let labels = files.map(|name| format!("@@//:{name}"));
    let build_path = workspace.join("rules_rust/BUILD.bazel");
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut initial = None;
    for (objects, direct, alias) in [
        (false, false, "renamed"),
        (true, false, "renamed"),
        (false, true, "renamed"),
        (false, false, "edited"),
        (false, false, "renamed"),
    ] {
        let build = format!(
            "load(':proof.bzl', 'subject')\nsubject(name = 'subject', src = '@@//:lib.rs', dependency_files = {labels:?}, force_objects = {}, force_direct = {}, dependency_alias = '{alias}')\n",
            if objects { "True" } else { "False" },
            if direct { "True" } else { "False" }
        );
        fs::write(&build_path, build).unwrap();
        let result = request(&dice, &workspace).await.unwrap();
        assert_eq!(result.actions().len(), 1);
        let spawn = result.actions()[0].spawn_spec().unwrap();
        let argv = spawn.render_argv();
        let selected = if objects {
            "deps/a/liba.rlib"
        } else {
            "deps/a/liba.rmeta"
        };
        let mut expected = vec![
            format!("--extern={alias}={selected}"),
            "--extern=b=deps/b/libb.rlib".to_owned(),
        ];
        if direct {
            expected.extend([
                "--extern=a=deps/a/liba.rmeta".to_owned(),
                "--extern=c=deps/b/libc.rlib".to_owned(),
            ]);
        }
        assert_eq!(
            argv.iter()
                .filter(|arg| arg.starts_with("--extern="))
                .cloned()
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            argv.iter()
                .filter(|arg| arg.starts_with("-Ldependency="))
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["-Ldependency=deps/a", "-Ldependency=deps/b"]
        );
        let RetainedCommandLineSegment::ArgsSnapshot(rustc) = &spawn.command_line().segments()[2]
        else {
            panic!("missing rustc Args")
        };
        let text = rustc.recipe().render_write_content();
        for flag in expected {
            assert!(text.contains(&format!("\n{flag}\n")), "{text}");
        }
        assert!(text.contains("\n-Ldependency=deps/a\n-Ldependency=deps/b\n"));
        if !objects && !direct && alias == "renamed" {
            if let Some(first) = &initial {
                assert_eq!(first, &result);
            } else {
                initial = Some(result);
            }
        } else {
            assert_ne!(initial.as_ref().unwrap(), &result);
        }
    }
    let proof_path = workspace.join("rules_rust/proof.bzl");
    let proof = fs::read_to_string(&proof_path).unwrap();
    fs::write(
        &proof_path,
        proof.replace(
            "metadata_supports_pipelining = True",
            "metadata_supports_pipelining = 'invalid'",
        ),
    )
    .unwrap();
    let error = request(&dice, &workspace).await.unwrap_err();
    assert!(
        error.contains("metadata_supports_pipelining must be a bool"),
        "{error}"
    );
    fs::write(&proof_path, &proof).unwrap();
    assert_eq!(initial.unwrap(), request(&dice, &workspace).await.unwrap());
    fs::write(
        &proof_path,
        proof.replace(
            "a = CrateInfo(name =",
            "a = CrateInfo(aliases = _no_coverage, name =",
        ),
    )
    .unwrap();
    let error = request(&dice, &workspace).await.unwrap_err();
    assert!(
        error.contains("unsupported analysis value of type `function`"),
        "{error}"
    );
    fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn pinned_rustc_native_link_flags_authenticate_imports_and_restore() {
    let workspace = workspace();
    let files = [
        "native/static/libplain.a",
        "native/pic/libplain.pic.a",
        "native/static/libalways.a",
        "native/dyn/libshared.so.1.2",
        "native/alias/librenamed.a",
        "native/lib/rustlib/libstd-xyz.a",
        "native/iface/libapi.ifso",
        "tools/ld",
    ];
    for name in files {
        let path = workspace.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "native link fixture\n").unwrap();
    }
    fs::write(
        workspace.join("BUILD.bazel"),
        format!("exports_files(['lib.rs'] + {files:?})\n"),
    )
    .unwrap();
    let labels = files.map(|name| format!("@@//:{name}"));
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut first = None;
    for (direct, include, flag) in [
        (false, true, "-z,now"),
        (true, true, "-z,now"),
        (true, false, "-z,now"),
        (false, true, "-changed"),
        (false, true, "-z,now"),
    ] {
        fs::write(workspace.join("rules_rust/BUILD.bazel"), format!(
            "load(':proof.bzl', 'subject')\nsubject(name = 'subject', src = '@@//:lib.rs', native_files = {labels:?}, direct_linker = {}, include_native_flags = {}, native_user_flag = '{flag}')\n",
            if direct {"True"} else {"False"}, if include {"True"} else {"False"},
        )).unwrap();
        let result = request(&dice, &workspace).await.unwrap();
        let spawn = result.actions()[0].spawn_spec().unwrap();
        let argv = spawn.render_argv();
        assert!(argv.contains(&"--codegen=linker=tools/ld".to_owned()));
        let native = argv
            .iter()
            .filter(|arg| {
                arg.starts_with("-Lnative=")
                    || arg.starts_with("-lstatic=")
                    || arg.starts_with("-ldylib=")
                    || arg.starts_with("-Clink-arg=")
                    || arg.starts_with("--codegen=link-arg=")
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut expected = [
            "-Lnative=native/pic",
            "-Lnative=native/static",
            "-Lnative=native/dyn",
            "-Lnative=native/iface",
            "-Lnative=native/lib/rustlib",
            "-Lnative=native/alias",
        ]
        .map(str::to_owned)
        .to_vec();
        if include {
            expected.extend(["-lstatic=renamed", "-Clink-arg=-lrenamed"].map(str::to_owned));
        }
        let prefix = if direct { "" } else { "-Wl," };
        expected.extend([
            format!("-Clink-arg={prefix}--whole-archive"),
            "-Clink-arg=native/static/libalways.a".to_owned(),
            format!("-Clink-arg={prefix}--no-whole-archive"),
        ]);
        if include {
            expected.extend(
                [
                    "-ldylib=shared",
                    "-ldylib=api",
                    "-lstatic=std-xyz",
                    "-lstatic=renamed",
                    "-Clink-arg=-lrenamed",
                ]
                .map(str::to_owned),
            );
        }
        expected.extend([
            format!("--codegen=link-arg={flag}"),
            "--codegen=link-arg=-pthread".to_owned(),
        ]);
        if include {
            expected.extend(["-lstatic=renamed", "-Clink-arg=-lrenamed"].map(str::to_owned));
        }
        assert_eq!(native, expected);
        let expanded = spawn.expand_forced_param_files().unwrap();
        assert_eq!(expanded.param_files().len(), 1);
        let bytes = std::str::from_utf8(expanded.param_files()[0].bytes()).unwrap();
        assert!(bytes.contains(&(expected.join("\n") + "\n")), "{bytes}");
        if let Some(initial) = &first {
            assert_eq!(initial == &result, !direct && include && flag == "-z,now");
        } else {
            first = Some(result);
        }
    }
    let utils_path = workspace.join("rules_rust/rust/private/utils.bzl");
    let utils = fs::read_to_string(&utils_path).unwrap();
    fs::write(&utils_path, format!("{utils}\n")).unwrap();
    let error = request(&dice, &workspace).await.unwrap_err();
    assert!(
        error.contains("requires pinned utils.bzl source provenance"),
        "{error}"
    );
    fs::write(&utils_path, utils).unwrap();
    assert_eq!(
        first.as_ref().unwrap(),
        &request(&dice, &workspace).await.unwrap()
    );

    let proof_path = workspace.join("rules_rust/proof.bzl");
    let proof = fs::read_to_string(&proof_path).unwrap();
    let from = "first = struct(libraries = libraries, user_link_flags = (ctx.attr.native_user_flag, \"-pthread\"))";
    assert_eq!(proof.matches(from).count(), 1);
    fs::write(&proof_path,proof.replace(from,"first = struct(libraries = libraries, user_link_flags = (ctx.attr.native_user_flag, \"-pthread\"), unused_callable = _no_coverage)")).unwrap();
    let error = request(&dice, &workspace).await.unwrap_err();
    assert!(error.contains("function"), "{error}");
    fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn pinned_cc_providers_feed_rustc_and_restore() {
    let workspace = workspace();
    let files = [
        "native/libplain.a",
        "pic/libplain.pic.a",
        "native/libalways.a",
        "unused.so",
        "alias/librenamed.a",
        "unused.a",
        "unused.ifso",
        "tools/ld",
    ];
    for name in files.into_iter().chain(["extra-a", "extra-b"]) {
        let path = workspace.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "cc provider fixture\n").unwrap();
    }
    fs::write(
        workspace.join("BUILD.bazel"),
        format!("exports_files(['lib.rs', 'extra-a', 'extra-b'] + {files:?})\n"),
    )
    .unwrap();
    let labels = files.map(|name| format!("@@//:{name}"));
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut first = None;
    let mut first_argv = None;
    for extra in ["extra-a", "extra-b", "extra-a"] {
        fs::write(workspace.join("rules_rust/BUILD.bazel"), format!(
            "load(':proof.bzl', 'subject')\nsubject(name = 'subject', src = '@@//:lib.rs', native_files = {labels:?}, real_cc = True, native_unused_input = '@@//:{extra}')\n"
        )).unwrap();
        let result = request(&dice, &workspace).await.unwrap();
        let spawn = result.actions()[0].spawn_spec().unwrap();
        let argv = spawn.render_argv();
        let native = argv
            .iter()
            .filter(|arg| {
                arg.starts_with("-Lnative=")
                    || arg.starts_with("-lstatic=")
                    || arg.starts_with("-Clink-arg=")
                    || arg.starts_with("--codegen=link-arg=")
            })
            .cloned()
            .collect::<Vec<_>>();
        let expected = [
            "-Lnative=pic",
            "-Lnative=native",
            "-Lnative=alias",
            "-lstatic=renamed",
            "-Clink-arg=-lrenamed",
            "-Clink-arg=-Wl,--whole-archive",
            "-Clink-arg=native/libalways.a",
            "-Clink-arg=-Wl,--no-whole-archive",
            "--codegen=link-arg=-z,now",
            "--codegen=link-arg=-pthread",
            "-lstatic=renamed",
            "-Clink-arg=-lrenamed",
        ]
        .map(str::to_owned);
        assert_eq!(native, expected);
        let expanded = spawn.expand_forced_param_files().unwrap();
        assert!(
            String::from_utf8_lossy(expanded.param_files()[0].bytes())
                .contains(&format!("{}\n", expected.join("\n")))
        );
        if let Some(previous) = &first {
            assert_eq!(&argv, first_argv.as_ref().unwrap());
            if extra == "extra-b" {
                assert_ne!(previous, &result);
            } else {
                assert_eq!(previous, &result);
            }
        } else {
            first_argv = Some(argv);
            first = Some(result);
        }
    }
    // Reload the defining Cc module to create a fresh HeaderInfo occurrence.
    // Equal configured publication must survive the new token's alias partition.
    let cc_path = workspace.join("rules_cc/cc/private/cc_info.bzl");
    let cc_source = fs::read_to_string(&cc_path).unwrap();
    fs::write(&cc_path, format!("{cc_source}\n")).unwrap();
    assert_eq!(
        first.as_ref().unwrap(),
        &request(&dice, &workspace).await.unwrap()
    );
    fs::write(&cc_path, cc_source).unwrap();
    assert_eq!(
        first.as_ref().unwrap(),
        &request(&dice, &workspace).await.unwrap()
    );
    // Public constructor validation still runs in unchanged upstream Starlark.
    let proof_path = workspace.join("rules_rust/proof.bzl");
    let proof = fs::read_to_string(&proof_path).unwrap();
    fs::write(
        &proof_path,
        proof.replace(
            "static_library = files[0], pic_static_library = files[1]",
            "static_library = files[3], pic_static_library = files[1]",
        ),
    )
    .unwrap();
    let error = request(&dice, &workspace).await.unwrap_err();
    assert!(error.contains("allowed extensions"), "{error}");
    fs::write(&proof_path, proof).unwrap();
    assert_eq!(first.unwrap(), request(&dice, &workspace).await.unwrap());
    fs::remove_dir_all(workspace).unwrap();
}
