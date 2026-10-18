use crate::runtime::dice::build_info_template_inputs::BuildInfoTemplateInputs;
use crate::runtime::dice::build_info_template_inputs::BuildInfoTemplatePreparationKey;
use crate::runtime::dice::build_info_template_inputs::checked_template_label;

const TEMPLATE_CASES: [(usize, &str, &[u8], &str); 2] = [
    (
        0,
        "volatile_file.h.template",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/build_info/templates/volatile_file.h.template"
        )),
        "83f47681c317f7b1249b6dcc9fa23a089233cf2f3a3801fedaca82869def8807",
    ),
    (
        1,
        "non_volatile_file.h.template",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/build_info/templates/non_volatile_file.h.template"
        )),
        "4d014ba8408a9a1e5cc9aca57183dee8bda0561ee92d4732bf174d0392218269",
    ),
];

fn template_build(target: &str, changed: bool) -> BuildCommandRootKey {
    let mut build = build_info_key(target, false);
    if changed {
        let overlay: CommandConfigurationOverlay = [
            slug_configuration_v2::NativeCommandOption::Copt,
            slug_configuration_v2::NativeCommandOption::HostCopt,
        ]
        .into_iter()
        .map(|option| {
            slug_configuration_v2::CommandConfigurationOccurrence::native(
                option,
                Some("-DBUILD_INFO_B"),
                false,
            )
        })
        .collect::<Vec<_>>()
        .into();
        let base = build.base_configuration.slug_configuration().unwrap();
        build.base_configuration = ConfigurationKey::from_slug(
            base.with_command_configuration(base.starlark_options().clone(), &overlay)
                .unwrap(),
        );
    }
    build
}

fn template_epoch() -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch {
        entries: build_info_epoch()
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.as_ref().clone()))
            .collect(),
    };
    epoch.missing("/workspace/abseil-cpp/REPO.bazel");
    epoch.missing("/workspace/abseil-cpp/.bazelignore");
    epoch.package("abseil-cpp", "load('@bazel_tools//tools/build_defs/build_info:bazel_cc_build_info.bzl','bazel_cc_build_info')\nbazel_cc_build_info(name='info',visibility=['//visibility:public'])\n", 1);
    let wrapper = build_info_epoch()
        .observations()
        .get(&BuildRootEpoch::demand(
            "/workspace/wrapper.bzl",
            PathObservationOperation::FileBytes,
        ))
        .unwrap()
        .as_ref()
        .clone();
    let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) = wrapper else {
        unreachable!()
    };
    let mut wrapper = String::from_utf8(bytes.to_vec()).unwrap();
    wrapper.push_str("target_wrapper=rule(implementation=_impl,attrs={'dep':attr.label()})\n");
    epoch.file("/workspace/wrapper.bzl", &wrapper, 1);
    epoch.package("", "load('@bazel_tools//tools/build_defs/build_info:bazel_cc_build_info.bzl','bazel_cc_build_info')\nload(':wrapper.bzl','wrapper','target_wrapper')\nplatform(name='platform')\nbazel_cc_build_info(name='info')\nwrapper(name='wrapper',dep=':info')\ntarget_wrapper(name='external_target',dep='@abseil-cpp//:info')\nwrapper(name='external_exec',dep='@abseil-cpp//:info')\n", 1);
    epoch.build()
}

fn ready(
    outcome: PreparationOutcome<Result<Arc<BuildInfoTemplateInputs>, Arc<str>>>,
) -> Arc<BuildInfoTemplateInputs> {
    let PreparationOutcome::Complete(result) = outcome else {
        panic!("template fixture retained Need: {outcome:?}")
    };
    result.unwrap()
}

async fn selected(
    tx: &mut dice::DiceTransaction,
    build: BuildCommandRootKey,
    action: usize,
) -> BuildInfoTemplatePreparationKey {
    let outcome = tx.compute(&build).await.unwrap();
    let owner = build_info_owner(build_info_evaluation(&outcome))
        .configured_target_key()
        .unwrap()
        .clone();
    BuildInfoTemplatePreparationKey::new(build, owner, action)
}

#[tokio::test]
async fn installed_templates_retain_main_external_target_exec_catalog_owners_and_configuration_aba()
{
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = build_info_transaction_with_epoch(&dice, template_epoch()).await;
    let mut held = Vec::new();
    for (target, external, exec) in [
        ("//:info", false, false),
        ("//:wrapper", false, true),
        ("//:external_target", true, false),
        ("//:external_exec", true, true),
    ] {
        for (ordinal, name, bytes, hash) in TEMPLATE_CASES {
            let key = selected(&mut tx, template_build(target, false), ordinal).await;
            let first = ready(tx.compute(&key).await.unwrap());
            let repeat = ready(tx.compute(&key).await.unwrap());
            assert!(Arc::ptr_eq(&first, &repeat));
            let action = first.action();
            assert_eq!(
                action.context().owner().label().package().repo().is_root(),
                !external
            );
            assert_eq!(
                action.context().owner().configuration().kind()
                    == slug_analysis_v2::ConfigurationKind::Exec,
                exec
            );
            assert!(
                first
                    .evaluation()
                    .action_closure
                    .owners()
                    .iter()
                    .flat_map(|owner| owner.actions())
                    .any(|retained| std::ptr::eq(retained, action))
            );
            let path = format!("tools/build_defs/build_info/templates/{name}");
            let content = first.source().result().as_ref().unwrap();
            let source = content
                .catalog_owner()
                .expect("actual builtin catalog owner");
            let value = source.as_ref().as_ref().unwrap();
            assert_eq!(content.bytes(), bytes);
            assert_eq!(value.path(), path);
            assert_eq!(value.bytes(), bytes);
            assert_eq!(hex::encode(value.sha256()), hash);
            assert!(!value.executable());
            let original = tx
                .compute(&slug_bzlmod_v2::BuiltinBazelToolsSourceFileKey::new(
                    slug_bzlmod_v2::BuiltinBazelToolsSnapshot::Bazel9_2,
                    path,
                ))
                .await
                .unwrap();
            assert!(Arc::ptr_eq(source, &original));
            for (demand, result) in first.source().observations().observations() {
                assert!(Arc::ptr_eq(
                    result,
                    first.observations().get(demand).unwrap()
                ));
            }
            for (demand, result) in first.observations().observations() {
                assert!(Arc::ptr_eq(
                    result,
                    first.certificate().observations().get(demand).unwrap()
                ));
            }
            let b = selected(&mut tx, template_build(target, true), ordinal).await;
            let changed = ready(tx.compute(&b).await.unwrap());
            assert_ne!(first, changed);
            assert_ne!(
                first.action().context().owner(),
                changed.action().context().owner()
            );
            assert!(Arc::ptr_eq(
                source,
                changed
                    .source()
                    .result()
                    .as_ref()
                    .unwrap()
                    .catalog_owner()
                    .unwrap()
            ));
            let restored = ready(tx.compute(&key).await.unwrap());
            assert!(Arc::ptr_eq(&first, &restored));
            held.push(first);
        }
    }
    let weak = Arc::downgrade(&held[0]);
    let catalog_weak = Arc::downgrade(
        held[0]
            .source()
            .result()
            .as_ref()
            .unwrap()
            .catalog_owner()
            .unwrap(),
    );
    drop(tx);
    dice.wait_for_idle().await;
    drop(dice);
    assert!(weak.upgrade().is_some());
    assert!(catalog_weak.upgrade().is_some());
    for (index, input) in held.iter().enumerate() {
        assert_eq!(
            input.source().result().as_ref().unwrap().bytes(),
            TEMPLATE_CASES[index % 2].2
        );
    }
    drop(held);
    // The independent DICE processor thread releases the version graph after
    // the last engine sender closes; cancellation idle does not join that thread.
    let released = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if weak.upgrade().is_none() && catalog_weak.upgrade().is_none() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await;
    assert!(
        released.is_ok(),
        "DICE owners still retained: metadata={}, catalog={}",
        weak.strong_count(),
        catalog_weak.strong_count()
    );
    assert!(weak.upgrade().is_none());
    assert!(catalog_weak.upgrade().is_none());
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct CatalogProbe(CanonicalLabel);
impl fmt::Display for CatalogProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "catalog-probe:{}", self.0)
    }
}
#[async_trait]
impl Key for CatalogProbe {
    type Value = PreparationOutcome<
        Result<Arc<slug_analysis_v2::ObservedCatalogSourceFile>, ObservedPathFrontierError>,
    >;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty()).unwrap();
        slug_analysis_v2::resolve_catalog_source_input_observed(
            ctx,
            &NormalizedAbsolutePath::new("/workspace").unwrap(),
            &self.0,
        )
        .await
        .map(|result| result.map(Arc::new))
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        matches!(value, PreparationOutcome::Complete(Ok(source)) if source.result().is_ok())
    }
}

#[tokio::test]
async fn catalog_template_preparation_preserves_need_and_rejects_root_local_missing_directory() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let build = build_info_key("//:info", false);
    let owner = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:info").unwrap(),
        build.base_configuration.clone(),
    );
    let key = BuildInfoTemplatePreparationKey::new(build, owner, 0);
    let mut tx = build_info_transaction_with_epoch(&dice, PathObservationEpoch::empty()).await;
    let need = tx.compute(&key).await.unwrap();
    assert!(matches!(need, PreparationOutcome::Need(_)));
    assert!(!<BuildInfoTemplatePreparationKey as Key>::validity(&need));
    drop(tx);
    let mut tx = build_info_transaction_with_epoch(&dice, template_epoch()).await;
    assert!(<BuildInfoTemplatePreparationKey as Key>::validity(
        &tx.compute(&key).await.unwrap()
    ));
    for (label, message) in [
        ("@@//:BUILD.bazel", "Builtin repository route"),
        ("@@abseil-cpp+//:BUILD.bazel", "Builtin repository route"),
        (
            "@@bazel_tools//tools/build_defs/build_info/templates:missing",
            "outside the Bazel 9.2 catalog",
        ),
        (
            "@@bazel_tools//tools/build_defs/build_info:templates",
            "wrong kind Directory",
        ),
    ] {
        let outcome = tx
            .compute(&CatalogProbe(CanonicalLabel::parse(label).unwrap()))
            .await
            .unwrap();
        let PreparationOutcome::Complete(result) = outcome else {
            panic!("catalog negative retained Need: {outcome:?}")
        };
        let source = result.unwrap();
        assert!(
            source
                .result()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains(message),
            "{label}: {:?}",
            source.result()
        );
        if !label.starts_with("@@//") {
            assert!(!source.observations().observations().is_empty());
        }
    }
}

#[tokio::test]
async fn template_preparation_rejects_absent_owner_ordinal_family_and_uninstalled_metadata() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = build_info_transaction(&dice).await;
    let build = build_info_key("//:info", false);
    let outcome = tx.compute(&build).await.unwrap();
    let evaluation = build_info_evaluation(&outcome);
    let owner = build_info_owner(evaluation);
    let owner_key = owner.configured_target_key().unwrap();
    for (selected_owner, ordinal, expected) in [
        (
            ConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//:absent").unwrap(),
                owner_key.configuration().clone(),
            ),
            0,
            "owner is absent",
        ),
        (owner_key.clone(), usize::MAX, "ordinal is absent"),
        (owner_key.clone(), 2, "BuildInfo"),
    ] {
        let error = tx
            .compute(&BuildInfoTemplatePreparationKey::new(
                build.clone(),
                selected_owner,
                ordinal,
            ))
            .await
            .unwrap();
        assert!(!<BuildInfoTemplatePreparationKey as Key>::validity(&error));
        let PreparationOutcome::Complete(Err(error)) = error else {
            panic!("expected guard error")
        };
        assert!(error.contains(expected), "{error}");
    }
    let spec = owner.actions()[1].build_info_transform_spec().unwrap();
    let make = |template| {
        ActionSpec::build_info_transform(BuildInfoTransformSpec::new(
            slug_build_api_v2::BuildInfoTransformHeader::new(
                spec.kind(),
                spec.status().clone(),
                template,
                spec.output().clone(),
            )
            .unwrap(),
            spec.callback().clone(),
        ))
    };
    let derived = AnalysisArtifact::Derived {
        owner: owner_key.artifact_owner(),
        output: ActionOutput::new(
            spec.output().root().clone(),
            "template",
            ActionOutputKind::File,
        ),
    };
    for template in [
        AnalysisArtifact::Source(CanonicalLabel::parse("@@//:custom").unwrap()),
        owner.actions()[0]
            .build_info_transform_spec()
            .unwrap()
            .template()
            .clone(),
    ] {
        let expected = template.clone();
        let valid = build_info_node(owner, vec![make(template)], true);
        let label =
            checked_template_label(&valid.actions()[0], evaluation.workspace(), owner_key).unwrap();
        assert_eq!(
            &AnalysisArtifact::Source(label.clone()),
            &expected,
            "general Source metadata has no installed-role selector"
        );
    }
    for (template, expected) in [
        (derived, "verified producer output"),
        (
            AnalysisArtifact::WorkspaceStatus(spec.status().clone()),
            "ordinary File template",
        ),
    ] {
        let malformed = build_info_node(owner, vec![make(template)], true);
        let error =
            checked_template_label(&malformed.actions()[0], evaluation.workspace(), owner_key)
                .unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
    let wrong_owner = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:other").unwrap(),
        owner_key.configuration().clone(),
    );
    assert!(
        checked_template_label(&owner.actions()[1], evaluation.workspace(), &wrong_owner)
            .unwrap_err()
            .contains("differs from selected owner")
    );
    let unresolved = build_info_node(owner, vec![make(spec.template().clone())], false);
    assert!(
        checked_template_label(&unresolved.actions()[0], evaluation.workspace(), owner_key)
            .unwrap_err()
            .contains("selected execution platform")
    );
}

fn native_request(runtime: &WorkspaceRuntime) -> NativeDemandRequestInputBundle {
    let mut request = NativeDemandRequestInputBundle::normalized_initial();
    request.registry_urls = RegistryUrls::new([format!(
        "file://{}/empty-registry",
        runtime.workspace.display()
    )]);
    request
}

#[derive(Clone)]
struct MutatingTemplateRoot {
    key: BuildInfoTemplatePreparationKey,
    mutation: Arc<Mutex<Option<(PathBuf, Vec<u8>)>>>,
    stale: Arc<Mutex<Option<Arc<BuildInfoTemplateInputs>>>>,
}
#[async_trait]
impl NativeCommandRoot for MutatingTemplateRoot {
    type Terminal = Arc<BuildInfoTemplateInputs>;
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(terminal.observations())
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        Some(terminal.certificate())
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        let outcome = NativeCommandRoot::compute(&self.key, tx).await?;
        if let PreparationOutcome::Complete(terminal) = &outcome {
            if let Some((path, bytes)) = self.mutation.lock().unwrap().take() {
                *self.stale.lock().unwrap() = Some(terminal.dupe());
                std::fs::write(path, bytes).unwrap();
            }
        }
        Ok(outcome)
    }
}

#[test]
fn native_template_preparation_certifies_current_frontier_ignores_shadow_and_refuses_revoked_route()
{
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp925/template-fixtures");
    std::fs::create_dir_all(&parent).unwrap();
    let workspace = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
    write_template_workspace(workspace.path());
    let shadow = workspace.path().join(
        "external/bazel_tools/tools/build_defs/build_info/templates/non_volatile_file.h.template",
    );
    std::fs::create_dir_all(shadow.parent().unwrap()).unwrap();
    std::fs::write(&shadow, "host shadow must remain untouched").unwrap();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let bundle = native_request(&runtime);
    let targets = [TargetPattern::parse("//:info").unwrap()];
    let accepted = runtime
        .build_command_with_repository_environment(
            &targets,
            bundle.command_policy,
            bundle.environment_policy,
            bundle.lockfile_mode,
            &[format!(
                "file://{}/empty-registry",
                workspace.path().display()
            )],
            bundle.repository_environment,
            CommandConfigurationOverlay::default(),
        )
        .unwrap();
    let owner = build_info_owner(accepted.terminal_for_test().as_ref().as_ref().unwrap())
        .configured_target_key()
        .unwrap()
        .clone();
    let bundle = native_request(&runtime);
    let (build, _) = runtime
        .prepare_build_request(
            &targets,
            bundle.command_policy,
            bundle.environment_policy,
            bundle.lockfile_mode,
            &[format!(
                "file://{}/empty-registry",
                workspace.path().display()
            )],
            bundle.repository_environment,
            CommandConfigurationOverlay::default(),
        )
        .unwrap();
    let key = BuildInfoTemplatePreparationKey::new(build, owner, 1);
    let first = runtime
        .drive_command(native_request(&runtime), key.clone())
        .unwrap()
        .accepted;
    let held = first.terminal_for_test().dupe();
    assert_eq!(
        held.source()
            .result()
            .as_ref()
            .unwrap()
            .catalog_owner()
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .path(),
        "tools/build_defs/build_info/templates/non_volatile_file.h.template"
    );
    let snapshot = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    for (demand, result) in held.observations().observations() {
        assert!(Arc::ptr_eq(
            result,
            snapshot.path_observations.get(demand).unwrap()
        ));
        assert!(Arc::ptr_eq(
            result,
            held.certificate().observations().get(demand).unwrap()
        ));
    }
    let build_path = workspace.path().join("BUILD.bazel");
    let mut changed = std::fs::read(&build_path).unwrap();
    changed.extend_from_slice(b"# observed source B\n");
    let stale = Arc::new(Mutex::new(None));
    let driven = runtime
        .drive_command(
            native_request(&runtime),
            MutatingTemplateRoot {
                key: key.clone(),
                mutation: Arc::new(Mutex::new(Some((build_path, changed)))),
                stale: stale.dupe(),
            },
        )
        .unwrap();
    let current = driven.accepted.terminal_for_test();
    assert!(!Arc::ptr_eq(
        current,
        stale.lock().unwrap().as_ref().unwrap()
    ));
    assert_ne!(held.observations(), current.observations());
    assert!(current.observations().observations().iter().any(|(demand, result)| {
        demand.path().as_path() == workspace.path().join("BUILD.bazel") && matches!(result.as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if bytes.ends_with(b"# observed source B\n"))
    }));
    assert!(Arc::ptr_eq(
        held.source()
            .result()
            .as_ref()
            .unwrap()
            .catalog_owner()
            .unwrap(),
        current
            .source()
            .result()
            .as_ref()
            .unwrap()
            .catalog_owner()
            .unwrap()
    ));
    assert_eq!(
        std::fs::read_to_string(&shadow).unwrap(),
        "host shadow must remain untouched"
    );
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    let module_path = workspace.path().join("MODULE.bazel");
    let mut module = std::fs::read_to_string(&module_path).unwrap();
    module.push_str(
        "local_path_override(module_name='bazel_tools',path='.slug_test_builtin/bazel_tools')\n",
    );
    std::fs::write(module_path, module).unwrap();
    assert!(
        runtime
            .drive_command(native_request(&runtime), key)
            .is_err()
    );
    let after = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    assert_eq!(before.path_observations, after.path_observations);
    assert_eq!(before.repository_results, after.repository_results);
    assert_eq!(before.selected, after.selected);
    assert_eq!(
        before.repository_environment_frontier,
        after.repository_environment_frontier
    );
    assert_eq!(before.inputs, after.inputs);
    assert_eq!(before.download_results, after.download_results);
    drop(after);
    drop(before);
    drop(snapshot);
    drop(stale);
    drop(driven);
    drop(first);
    drop(accepted);
    drop(targets);
    drop(runtime);
    assert_eq!(
        held.source().result().as_ref().unwrap().bytes(),
        TEMPLATE_CASES[1].2
    );
}

fn write_template_workspace(root: &Path) {
    for (demand, result) in template_epoch().observations() {
        let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) = result.as_ref()
        else {
            continue;
        };
        let path = root.join(demand.path().as_path().strip_prefix("/workspace").unwrap());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    std::fs::create_dir_all(root.join("platforms/host")).unwrap();
    std::fs::write(
        root.join("platforms/host/BUILD.bazel"),
        "exports_files(['constraints.bzl'])\nplatform(name='host')\n",
    )
    .unwrap();
    std::fs::write(
        root.join("platforms/host/constraints.bzl"),
        "HOST_CONSTRAINTS = []\n",
    )
    .unwrap();
}

mod workspace_status_inputs {
    use super::*;
    include!("workspace_status_inputs_tests.rs");
}
