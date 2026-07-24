use std::path::PathBuf;
use std::sync::Arc;

use dice::DetectCycles;
use dice::Dice;
use dice::UserComputationData;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::ModuleFileEvaluationKey;
use slug_bzlmod_v2::RootModuleCommandPolicy;
use slug_bzlmod_v2::RootModuleCommandPolicyKey;
use slug_bzlmod_v2::RootModuleEnvironmentPolicy;
use slug_bzlmod_v2::RootModuleEnvironmentPolicyKey;
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_bzlmod_v2::RootModuleLockfileMode;
use slug_bzlmod_v2::RootModuleLockfileModeKey;
use slug_bzlmod_v2::VisibleLockfileRead;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_identity_v2::ApparentRepoName;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::sorted_map::SortedMap;

#[derive(Clone)]
struct RequestInputs {
    command: Option<BzlmodCommandPolicyKey>,
    environment: Option<BzlmodEnvironmentPolicyKey>,
    lockfile_mode: Option<LockfileMode>,
}

impl RequestInputs {
    fn defaults() -> Self {
        Self {
            command: Some(BzlmodCommandPolicyKey::from_flags(None, false).unwrap()),
            environment: Some(
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            ),
            lockfile_mode: Some(LockfileMode::Update),
        }
    }
}

fn workspace() -> PathBuf {
    PathBuf::from("/root-module-dice-test")
}

fn snapshot(
    entries: impl IntoIterator<Item = (&'static str, WorkspaceFileValue)>,
) -> Arc<WorkspaceSnapshot> {
    let workspace = workspace();
    Arc::new(WorkspaceSnapshot {
        files: Arc::new(
            entries
                .into_iter()
                .map(|(path, value)| (workspace.join(path), value))
                .collect::<SortedMap<_, _>>(),
        ),
    })
}

async fn graph_value(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
) -> Arc<Result<slug_bzlmod_v2::RootModuleGraph, compact_str::CompactString>> {
    graph_and_module_value(dice, files, RequestInputs::defaults())
        .await
        .0
}

async fn graph_and_module_value(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
    inputs: RequestInputs,
) -> (
    Arc<Result<slug_bzlmod_v2::RootModuleGraph, compact_str::CompactString>>,
    Arc<Result<slug_bzlmod_v2::ModuleFileEvaluation, compact_str::CompactString>>,
) {
    let workspace = workspace();
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            files,
        )])
        .unwrap();
    if let Some(command) = inputs.command {
        updater
            .changed_to(vec![(
                RootModuleCommandPolicyKey {
                    workspace: workspace.clone(),
                },
                RootModuleCommandPolicy::from(command),
            )])
            .unwrap();
    }
    if let Some(environment) = inputs.environment {
        updater
            .changed_to(vec![(
                RootModuleEnvironmentPolicyKey {
                    workspace: workspace.clone(),
                },
                RootModuleEnvironmentPolicy::from(environment),
            )])
            .unwrap();
    }
    if let Some(lockfile_mode) = inputs.lockfile_mode {
        updater
            .changed_to(vec![(
                RootModuleLockfileModeKey {
                    workspace: workspace.clone(),
                },
                RootModuleLockfileMode::from(lockfile_mode),
            )])
            .unwrap();
    }
    let mut transaction = updater.commit().await;
    let graph = transaction
        .compute(&RootModuleGraphKey {
            workspace: workspace.clone(),
        })
        .await
        .unwrap();
    let module = transaction
        .compute(&ModuleFileEvaluationKey {
            workspace: workspace.clone(),
            path: workspace.join("MODULE.bazel"),
        })
        .await
        .unwrap();
    (graph, module)
}

async fn graph(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
) -> Result<slug_bzlmod_v2::RootModuleGraph, String> {
    graph_value(dice, files)
        .await
        .as_ref()
        .clone()
        .map_err(|error| error.to_string())
}

#[tokio::test]
async fn root_graph_requires_explicit_injected_request_values() {
    let files = || {
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new("module(name = 'root')".to_owned())),
        )])
    };
    let cases = [
        (
            RequestInputs {
                command: None,
                ..RequestInputs::defaults()
            },
            "missing injected root module command policy",
        ),
        (
            RequestInputs {
                environment: None,
                ..RequestInputs::defaults()
            },
            "missing injected root module environment policy",
        ),
        (
            RequestInputs {
                lockfile_mode: None,
                ..RequestInputs::defaults()
            },
            "missing injected root module lockfile mode",
        ),
    ];
    for (inputs, expected) in cases {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let error = graph_and_module_value(&dice, files(), inputs)
            .await
            .0
            .as_ref()
            .as_ref()
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[tokio::test]
async fn request_input_helper_populates_the_complete_fail_closed_triplet() {
    let workspace = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            snapshot([(
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("module(name = 'root')".to_owned())),
            )]),
        )])
        .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(Some("all"), true).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("dep@1.0")).unwrap(),
        LockfileMode::Off,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let graph = transaction
        .compute(&RootModuleGraphKey { workspace })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    assert!(graph.command_policy.ignore_dev_dependency());
}

#[tokio::test]
async fn root_graph_allows_an_omitted_module_declaration() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let graph = graph(
        &dice,
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "bazel_dep(name = 'dep', version = '1.0')".to_owned(),
            )),
        )]),
    )
    .await
    .unwrap();
    assert!(graph.root.header.is_none());
    assert_eq!(graph.root.dependencies[0].name, "dep");
}

#[tokio::test]
async fn root_graph_breadth_first_includes_and_preserves_file_states() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let root = WorkspaceFileValue::Present(Arc::new("module(name = 'root')\ninclude('//:one.MODULE.bazel')\ninclude('//:two.MODULE.bazel')\nbazel_dep(name = 'dep', version = '1.0')\nlocal_path_override(module_name = 'dep', path = '../dep')\n".to_owned()));
    let one =
        WorkspaceFileValue::Present(Arc::new("include('//:nested.MODULE.bazel')\n".to_owned()));
    let two = WorkspaceFileValue::Present(Arc::new(
        "bazel_dep(name = 'two', version = '2.0')\n".to_owned(),
    ));
    let nested = WorkspaceFileValue::Present(Arc::new(
        "bazel_dep(name = 'nested', version = '3.0')".to_owned(),
    ));
    let first = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            ("one.MODULE.bazel", one.clone()),
            ("two.MODULE.bazel", two.clone()),
            ("nested.MODULE.bazel", nested.clone()),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(first.root.header.as_ref().unwrap().name, "root");
    assert_eq!(
        first
            .includes
            .iter()
            .map(|file| file.path.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "one.MODULE.bazel",
            "two.MODULE.bazel",
            "nested.MODULE.bazel"
        ]
    );
    assert_eq!(first.root.dependencies[0].name, "dep");

    let unchanged = graph_value(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            ("one.MODULE.bazel", one.clone()),
            ("two.MODULE.bazel", two.clone()),
            ("nested.MODULE.bazel", nested.clone()),
        ]),
    )
    .await;
    let first_value = graph_value(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            ("one.MODULE.bazel", one.clone()),
            ("two.MODULE.bazel", two.clone()),
            ("nested.MODULE.bazel", nested.clone()),
        ]),
    )
    .await;
    assert!(Arc::ptr_eq(&unchanged, &first_value));

    let root_absent = graph(
        &dice,
        snapshot([("MODULE.bazel", WorkspaceFileValue::Absent)]),
    )
    .await
    .unwrap_err();
    assert!(
        root_absent.contains("workspace file is absent"),
        "{root_absent}"
    );
    let root_read_error = graph(
        &dice,
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::ReadError(Arc::new("root denied".to_owned())),
        )]),
    )
    .await
    .unwrap_err();
    assert_eq!(root_read_error, "root denied");

    let absent = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            ("one.MODULE.bazel", WorkspaceFileValue::Absent),
            ("two.MODULE.bazel", two.clone()),
        ]),
    )
    .await
    .unwrap_err();
    assert!(absent.contains("workspace file is absent"), "{absent}");
    let read_error = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            (
                "one.MODULE.bazel",
                WorkspaceFileValue::ReadError(Arc::new("denied".to_owned())),
            ),
            ("two.MODULE.bazel", two.clone()),
        ]),
    )
    .await
    .unwrap_err();
    assert_eq!(read_error, "denied");

    let edited = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            (
                "one.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "bazel_dep(name = 'changed', version = '4.0')".to_owned(),
                )),
            ),
            ("two.MODULE.bazel", two.clone()),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(edited.includes.len(), 2);
    let recreated = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root),
            ("one.MODULE.bazel", one),
            ("two.MODULE.bazel", two),
            ("nested.MODULE.bazel", nested),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(recreated.includes.len(), 3);
}

#[tokio::test]
async fn root_graph_preserves_bazel_global_shapes_and_repository_mapping() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let files = snapshot([
        (
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "module(name = 'root', version = '1.2.3', compatibility_level = 7, repo_name = 'self_repo', bazel_compatibility = ['>=9.0.0', '-9.1.0'])\n\
                 include('//deps:parts/nested.MODULE.bazel')\n\
                 bazel_dep(name = 'root_dep', version = '1.0', max_compatibility_level = 12)\n\
                 bazel_dep(name = 'aliased_dep', version = '2.0', repo_name = 'alias')\n\
                 bazel_dep(name = 'nodep_dep', version = '3.0', repo_name = None)\n\
                 bazel_dep(name = 'dev_dep', version = '4.0', dev_dependency = True)\n\
                 local_path_override(module_name = 'root_dep', path = '../root_dep')\n"
                    .to_owned(),
            )),
        ),
        (
            "deps/parts/nested.MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "bazel_dep(name = 'included_dep', version = '5.0')\n".to_owned(),
            )),
        ),
    ]);

    let default = graph(&dice, files.clone()).await.unwrap();
    assert_eq!(default.root.header.as_ref().unwrap().name, "root");
    let Some(slug_bzlmod_v2::RootModuleOverride::NonRegistry(local)) =
        default.overrides.get("root_dep")
    else {
        panic!("root_dep local_path_override was not captured");
    };
    assert_eq!(
        local.rule_id.bzl_file.to_string(),
        "@@bazel_tools//tools/build_defs/repo:local.bzl"
    );
    assert_eq!(local.rule_id.rule_name, "local_repository");
    assert!(matches!(
        local.attributes.get("path"),
        Some(slug_bzlmod_v2::OverrideAttributeValue::String(path))
            if path == "../root_dep"
    ));
    assert!(default.root.dependencies[2].nodep);
    for (apparent, canonical) in [
        ("root_dep", "root_dep+"),
        ("alias", "aliased_dep+"),
        ("dev_dep", "dev_dep+"),
        ("included_dep", "included_dep+"),
        ("nodep_dep", "nodep_dep"),
    ] {
        assert_eq!(
            default
                .repository_mapping
                .resolve(&ApparentRepoName::new(apparent).unwrap())
                .as_str(),
            canonical
        );
    }

    let ignored = graph_and_module_value(
        &dice,
        files,
        RequestInputs {
            command: Some(BzlmodCommandPolicyKey::from_flags(None, true).unwrap()),
            ..RequestInputs::defaults()
        },
    )
    .await
    .0
    .as_ref()
    .clone()
    .unwrap();
    assert_eq!(
        ignored
            .repository_mapping
            .resolve(&ApparentRepoName::new("dev_dep").unwrap())
            .as_str(),
        "dev_dep"
    );
}

#[tokio::test]
async fn root_override_owner_captures_all_forms_and_defaults() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let graph = graph(
        &dice,
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "module(name = 'root')\n\
                 local_path_override(module_name = 'local', path = 'third_party/local')\n\
                 single_version_override(module_name = 'single', patches = ['//:one.patch', ':short.patch', 'bare.patch', '@root//:own.patch', '@@visible//:two.patch'], patch_cmds = ['first', 'second'], patch_strip = -1)\n\
                 multiple_version_override(module_name = 'multiple', versions = ['1.0.0', '2.0.0'])\n\
                 archive_override(module_name = 'archive', urls = ['file:///archive'], patches = [':archive.patch'], nested = {'flag': True, 'number': -2147483648, 'items': ('value', None)})\n\
                 git_override(module_name = 'git', remote = 'file:///repo', commit = 'abc', patches = ('git.patch',), options = {'enabled': False})\n"
                    .to_owned(),
            )),
        )]),
    )
    .await
    .unwrap();

    assert_eq!(graph.overrides.iter().count(), 5);
    let Some(slug_bzlmod_v2::RootModuleOverride::RegistrySingle(single)) =
        graph.overrides.get("single")
    else {
        panic!("single override was not captured");
    };
    assert_eq!(single.version, "");
    assert_eq!(single.registry, "");
    assert_eq!(single.patch_strip, -1);
    assert_eq!(single.patch_cmds.as_ref(), ["first", "second"]);
    assert_eq!(single.patches[0].to_string(), "@@//:one.patch");
    assert_eq!(single.patches[1].to_string(), "@@//:short.patch");
    assert_eq!(single.patches[2].to_string(), "@@//:bare.patch");
    assert_eq!(single.patches[3].to_string(), "@@//:own.patch");
    assert_eq!(single.patches[4].to_string(), "@@visible//:two.patch");

    let Some(slug_bzlmod_v2::RootModuleOverride::RegistryMultiple(multiple)) =
        graph.overrides.get("multiple")
    else {
        panic!("multiple override was not captured");
    };
    assert_eq!(multiple.versions.as_ref(), ["1.0.0", "2.0.0"]);
    assert_eq!(multiple.registry, "");

    let Some(slug_bzlmod_v2::RootModuleOverride::NonRegistry(archive)) =
        graph.overrides.get("archive")
    else {
        panic!("archive override was not captured");
    };
    assert_eq!(
        archive.rule_id.bzl_file.to_string(),
        "@@bazel_tools//tools/build_defs/repo:http.bzl"
    );
    assert_eq!(archive.rule_id.rule_name, "http_archive");
    assert!(matches!(
        archive.attributes.get("nested"),
        Some(slug_bzlmod_v2::OverrideAttributeValue::Map(_))
    ));
    assert!(matches!(
        archive.attributes.get("patches"),
        Some(slug_bzlmod_v2::OverrideAttributeValue::Iterable(patches))
            if matches!(
                patches.as_ref(),
                [slug_bzlmod_v2::OverrideAttributeValue::String(patch)]
                    if patch == ":archive.patch"
            )
    ));
    let Some(slug_bzlmod_v2::RootModuleOverride::NonRegistry(git)) = graph.overrides.get("git")
    else {
        panic!("git override was not captured");
    };
    assert_eq!(
        git.rule_id.bzl_file.to_string(),
        "@@bazel_tools//tools/build_defs/repo:git.bzl"
    );
    assert_eq!(git.rule_id.rule_name, "git_repository");
}

#[tokio::test]
async fn root_override_owner_rejects_duplicates_and_invalid_boundaries() {
    let cases = [
        (
            "single_version_override(module_name = 'dup')\nsingle_version_override(module_name = 'dup')",
            "multiple overrides for module dup",
        ),
        (
            "multiple_version_override(module_name = 'versions', versions = ['1.0.0'])",
            "at least two versions",
        ),
        (
            "bazel_dep(name = 'visible', version = '1.0.0')\nsingle_version_override(module_name = 'label', patches = ['@visible//:patch'])",
            "not visible",
        ),
        (
            "archive_override(module_name = 'label', patches = ['@invisible//:patch'])",
            "not visible",
        ),
        (
            "single_version_override(module_name = 'version', version = 'not valid')",
            "Invalid version",
        ),
        (
            "archive_override(module_name = 'integer', value = 2147483648)",
            "unsupported repository override attribute value",
        ),
        (
            "cycle = []\ncycle.append(cycle)\narchive_override(module_name = 'cycle', value = cycle)",
            "must not contain cyclic values",
        ),
    ];
    for (directives, expected) in cases {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let error = graph(
            &dice,
            snapshot([(
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(format!(
                    "module(name = 'root')\n{directives}\n"
                ))),
            )]),
        )
        .await
        .unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[tokio::test]
async fn root_override_owner_merges_includes_and_replays_a_b_a() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let files = |root: &str| {
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(root.to_owned())),
            ),
            (
                "included.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "single_version_override(module_name = 'included', patches = ['@root//:included.patch'])\n"
                        .to_owned(),
                )),
            ),
        ])
    };
    let a_source = "module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'route', registry = 'file:///a')\n";
    let b_source = "module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'route', version = '2.0.0', registry = 'file:///b')\n";
    let a = graph(&dice, files(a_source)).await.unwrap();
    let b = graph(&dice, files(b_source)).await.unwrap();
    let a_again = graph(&dice, files(a_source)).await.unwrap();
    assert_eq!(a.overrides.iter().count(), 2);
    let Some(slug_bzlmod_v2::RootModuleOverride::RegistrySingle(included)) =
        a.overrides.get("included")
    else {
        panic!("included override was not captured");
    };
    assert_eq!(included.patches[0].to_string(), "@@//:included.patch");
    assert_ne!(a.overrides, b.overrides);
    assert_eq!(a.overrides, a_again.overrides);

    let ordered = graph(
        &dice,
        files("module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'alpha')\nsingle_version_override(module_name = 'omega')\n"),
    )
    .await
    .unwrap();
    let reordered = graph(
        &dice,
        files("module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'omega')\nsingle_version_override(module_name = 'alpha')\n"),
    )
    .await
    .unwrap();
    assert_eq!(ordered.overrides, reordered.overrides);

    let duplicate = graph(
        &dice,
        files("module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'included')\n"),
    )
    .await
    .unwrap_err();
    assert!(duplicate.contains("multiple overrides for module included"));
}

#[tokio::test]
async fn normalized_request_inputs_reuse_module_evaluation_across_a_b_a() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let files = snapshot([(
        "MODULE.bazel",
        WorkspaceFileValue::Present(Arc::new(
            "module(name = 'root')\nbazel_dep(name = 'dev_dep', dev_dependency = True)\n"
                .to_owned(),
        )),
    )]);
    let inputs_a = RequestInputs::defaults();
    let inputs_b = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(None, true).unwrap()),
        environment: Some(
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
        ),
        lockfile_mode: Some(LockfileMode::Update),
    };

    let (graph_a, module_a) = graph_and_module_value(&dice, files.clone(), inputs_a.clone()).await;
    let (graph_b, module_b) = graph_and_module_value(&dice, files.clone(), inputs_b).await;
    let (graph_a_again, module_a_again) = graph_and_module_value(&dice, files, inputs_a).await;

    let graph_a = graph_a.as_ref().as_ref().unwrap();
    let graph_b = graph_b.as_ref().as_ref().unwrap();
    let graph_a_again = graph_a_again.as_ref().as_ref().unwrap();
    assert_ne!(graph_a.command_policy, graph_b.command_policy);
    assert_ne!(graph_a.environment_policy, graph_b.environment_policy);
    assert_eq!(graph_a.command_policy, graph_a_again.command_policy);
    assert_eq!(graph_a.environment_policy, graph_a_again.environment_policy);
    assert!(Arc::ptr_eq(&module_a, &module_b));
    assert!(Arc::ptr_eq(&module_a, &module_a_again));
}

#[tokio::test]
async fn visible_lockfile_transitions_are_semantic_and_recover_on_retained_dice() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let module = WorkspaceFileValue::Present(Arc::new("module(name = 'root')\n".to_owned()));
    let files = |lockfile: Option<WorkspaceFileValue>| {
        let mut entries = vec![("MODULE.bazel", module.clone())];
        if let Some(lockfile) = lockfile {
            entries.push(("MODULE.bazel.lock", lockfile));
        }
        snapshot(entries)
    };

    let (absent, module_absent) =
        graph_and_module_value(&dice, files(None), RequestInputs::defaults()).await;
    let absent = absent.as_ref().as_ref().unwrap().clone();
    assert_eq!(
        absent.visible_lockfile,
        VisibleLockfileRead::Parsed(slug_bzlmod_v2::empty_bazel_lockfile().into())
    );

    let (formatted, module_formatted) = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\n  \"facts\": {},\n  \"lockFileVersion\": 28,\n  \"moduleExtensions\": {},\n  \"registryFileHashes\": {},\n  \"selectedYankedVersions\": {}\n}\n"
                .to_owned(),
        )))),
        RequestInputs::defaults(),
    )
    .await;
    assert_eq!(&absent, formatted.as_ref().as_ref().unwrap());
    assert!(Arc::ptr_eq(&module_absent, &module_formatted));

    let malformed = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\"lockFileVersion\":28, nope".to_owned(),
        )))),
        RequestInputs::defaults(),
    )
    .await
    .0;
    let error = malformed.as_ref().as_ref().unwrap_err().to_string();
    assert!(
        error.contains("Failed to read and parse the MODULE.bazel.lock file"),
        "{error}"
    );

    let restored = graph(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\"lockFileVersion\":28}\n".to_owned(),
        )))),
    )
    .await
    .unwrap();
    assert_eq!(restored, absent);

    let (deleted, module_deleted) =
        graph_and_module_value(&dice, files(None), RequestInputs::defaults()).await;
    assert_eq!(deleted.as_ref().as_ref().unwrap(), &absent);
    assert!(Arc::ptr_eq(&module_absent, &module_deleted));

    let (recreated, module_recreated) = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\"lockFileVersion\":28}\n".to_owned(),
        )))),
        RequestInputs::defaults(),
    )
    .await;
    assert_eq!(recreated.as_ref().as_ref().unwrap(), &absent);
    assert!(Arc::ptr_eq(&module_absent, &module_recreated));

    for mode in [
        LockfileMode::Update,
        LockfileMode::Refresh,
        LockfileMode::Error,
    ] {
        let read_error = graph_and_module_value(
            &dice,
            files(Some(WorkspaceFileValue::ReadError(Arc::new(
                "permission denied".to_owned(),
            )))),
            RequestInputs {
                lockfile_mode: Some(mode),
                ..RequestInputs::defaults()
            },
        )
        .await
        .0;
        let error = read_error.as_ref().as_ref().unwrap_err().to_string();
        assert!(error.contains("permission denied"), "{error}");
        assert!(error.contains("Try deleting it"), "{error}");
    }

    let stale_error = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\"lockFileVersion\":27, nope".to_owned(),
        )))),
        RequestInputs {
            lockfile_mode: Some(LockfileMode::Error),
            ..RequestInputs::defaults()
        },
    )
    .await
    .0;
    let error = stale_error.as_ref().as_ref().unwrap_err().to_string();
    assert_eq!(
        error,
        "The version of MODULE.bazel.lock is not supported by this version of Bazel. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
    );

    let ignored = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::ReadError(Arc::new(
            "permission denied".to_owned(),
        )))),
        RequestInputs {
            lockfile_mode: Some(LockfileMode::Off),
            ..RequestInputs::defaults()
        },
    )
    .await
    .0;
    assert_eq!(
        ignored.as_ref().as_ref().unwrap().visible_lockfile,
        VisibleLockfileRead::Ignored
    );
}

#[tokio::test]
async fn module_and_include_errors_precede_visible_lockfile_errors() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let cases = [
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "bazel_dep(name = 'dep')\nmodule(name = 'root')\n".to_owned(),
                )),
            ),
            (
                "MODULE.bazel.lock",
                WorkspaceFileValue::Present(Arc::new("{\"lockFileVersion\":28, nope".to_owned())),
            ),
        ]),
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "module(name = 'root')\ninclude('//pkg:child.MODULE.bazel')\n".to_owned(),
                )),
            ),
            (
                "pkg/child.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("module(name = 'included')\n".to_owned())),
            ),
            (
                "MODULE.bazel.lock",
                WorkspaceFileValue::Present(Arc::new("{\"lockFileVersion\":28, nope".to_owned())),
            ),
        ]),
    ];
    for files in cases {
        let error = graph_and_module_value(
            &dice,
            files,
            RequestInputs {
                lockfile_mode: Some(LockfileMode::Error),
                ..RequestInputs::defaults()
            },
        )
        .await
        .0;
        let error = error.as_ref().as_ref().unwrap_err().to_string();
        assert!(error.contains("module()"), "{error}");
        assert!(!error.contains("MODULE.bazel.lock"), "{error}");
    }
}

#[tokio::test]
async fn root_graph_rejects_invalid_global_calls_and_include_labels() {
    let invalid_sources = [
        "bazel_dep(name = 'dep')\nmodule(name = 'root')\n",
        "module(name = 'Bad')\n",
        "bazel_dep(name = '')\n",
        "module(name = 'root', version = '1..0')\n",
        "module(name = 'root', repo_name = 'bad+')\n",
        "bazel_dep(version = '1.0')\n",
        "include('@repo//pkg:file.MODULE.bazel')\n",
        "include(':relative.MODULE.bazel')\n",
        r#"include('//pkg:bad\name.MODULE.bazel')"#,
        "include('//pkg:../bad.MODULE.bazel')\n",
        "include('//pkg:.MODULE.bazel')\n",
        "include('//pkg:bad.txt')\n",
        "include('//pkg::bad.MODULE.bazel')\n",
        "include('//...:ok.MODULE.bazel')\n",
    ];
    for source in invalid_sources {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let error = graph(
            &dice,
            snapshot([(
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(source.to_owned())),
            )]),
        )
        .await
        .unwrap_err();
        assert!(!error.is_empty(), "source unexpectedly passed: {source}");
    }

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let included_module = graph(
        &dice,
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "include('//:part.MODULE.bazel')\n".to_owned(),
                )),
            ),
            (
                "part.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("module(name = 'part')\n".to_owned())),
            ),
        ]),
    )
    .await
    .unwrap_err();
    assert!(
        included_module.contains("module() is called"),
        "{included_module}"
    );
}
