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
    assert_eq!(default.root.local_path_overrides[0].path, "../root_dep");
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
