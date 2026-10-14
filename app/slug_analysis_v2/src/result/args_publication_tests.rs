//! Joint provider/action publication compares aliases without invoking callbacks.
use slug_build_api_v2::*;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;

use super::*;
use crate::key::ConfigurationKey;

#[derive(Debug)]
struct NeverRender;
impl RuntimeMapper for NeverRender {
    fn map_batch(
        &self,
        _: &[AnalysisValue],
        _: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<CompactString>, ArgsExpansionError> {
        panic!("publication equality must never expand a callback")
    }
}

fn key(name: &str) -> ConfiguredTargetKey {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(ActionEnvironmentHost::without_environment(
        ActionEnvironmentHostOs::Linux,
    ));
    ConfiguredTargetKey::new(
        CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
        ConfigurationKey::from_slug(SlugConfiguration::default_target(&host).unwrap()),
    )
}

fn depset() -> AnalysisDepset {
    AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("retained")],
        Vec::new(),
    )
    .unwrap()
}

fn recipe(captured: AnalysisDepset) -> RetainedArgsRecipe {
    let mut graph = CallableGraphBuilder::new();
    let capture = graph.reserve().unwrap();
    graph
        .set_node(
            capture,
            CallableNode::Native(AnalysisValue::depset(captured)),
        )
        .unwrap();
    let root = graph.reserve().unwrap();
    graph
        .set_node(
            root,
            CallableNode::Intrinsic(CallableIntrinsic {
                api: NativeCallableApiId::new("test.result_mapper", 1).unwrap(),
                parameters: Arc::from([]),
                return_annotation: None,
                receiver: Some(capture),
            }),
        )
        .unwrap();
    let mapper =
        RetainedArgsMapper::new(graph.finish().unwrap(), root, Arc::new(NeverRender)).unwrap();
    RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            RetainedVectorSource::Mapped {
                inputs: RetainedArgsInputs::Sequence(vec![AnalysisValue::string("input")].into()),
                mapper,
                registration_location: None,
            },
            RetainedVectorOptions {
                arg_name: None,
                format_each: None,
                before_each: None,
                join_with: None,
                format_joined: None,
                omit_if_empty: true,
                uniquify: false,
                expand_directories: false,
                terminate_with: None,
            },
        ))],
        RetainedParamFileFormat::Multiline,
    )
}

fn result(split_provider: bool, split_actions: bool) -> ConfiguredNodeResult {
    let owner = key("subject");
    let shared = depset();
    let provider = ProviderOccurrence::new(
        ProviderIdentity::user(ProviderId::new("//:unrelated.bzl", "Info").unwrap()),
        [("captured", AnalysisValue::depset(shared.clone()))],
    );
    let providers =
        ProviderCollection::from_values(vec![ProviderValue::Occurrence(provider)], false).unwrap();
    let mut result = ConfiguredNodeResult::new_rule(
        owner.clone(),
        providers,
        None,
        RunfilesPackageDepset::new(DepsetOrder::Default, Vec::new(), Vec::new()).unwrap(),
    );
    let context =
        Arc::new(ConfiguredActionOwnerContext::unresolved_default(owner.clone()).unwrap());
    let first = if split_provider { depset() } else { shared };
    let second = if split_actions {
        depset()
    } else {
        first.clone()
    };
    let root =
        DerivedArtifactRoot::bin(owner.configuration().slug_configuration().unwrap().clone())
            .unwrap();
    result.actions = vec![first, second]
        .into_iter()
        .enumerate()
        .map(|(index, captured)| ConfiguredAction {
            spec: ActionSpec::args_write(ArgsWriteSpec::new(
                ActionOutput::new(
                    root.clone(),
                    format!("{index}.params"),
                    ActionOutputKind::File,
                ),
                recipe(captured),
                false,
            )),
            context: context.clone(),
        })
        .collect::<Vec<_>>()
        .into();
    result
}

#[test]
fn result_publication_preserves_cross_provider_and_multi_action_callback_aliases() {
    let original = result(false, false);
    let restored = result(false, false);
    let provider_split = result(true, false);
    let action_split = result(false, true);
    // Separate field equality would accept all these pairs. The shared native
    // state must detect cross-boundary alias splits without executing the mapper.
    assert_eq!(original.providers, provider_split.providers);
    assert_eq!(original.actions, provider_split.actions);
    assert_eq!(original.actions, action_split.actions);
    assert_eq!(original, restored);
    assert_ne!(original, provider_split);
    assert_ne!(original, action_split);
    assert_eq!(original, result(false, false));
}

fn changed(
    original: &ConfiguredNodeResult,
    description: &str,
    change: impl FnOnce(&mut ConfiguredNodeResult),
) {
    let mut candidate = original.clone();
    change(&mut candidate);
    assert_ne!(
        original, &candidate,
        "{description} must remain in configured-result equality"
    );
}

#[test]
fn result_joint_publication_keeps_non_args_fields_action_context_and_count() {
    let original = result(false, false);
    changed(&original, "requested key", |value| {
        value.key = key("other").into()
    });
    changed(&original, "actual target", |value| {
        value.actual_target = key("other").into()
    });
    changed(&original, "kind", |value| {
        value.kind = ConfiguredNodeKind::Alias
    });
    changed(&original, "declared outputs", |value| {
        value.declared_outputs = Arc::from([CompactString::new("extra")])
    });
    changed(&original, "edges", |value| {
        value.edges = Arc::from([ConfiguredEdge::new(
            key("other").into(),
            ConfiguredEdgeKind::HostPlatform,
        )])
    });
    changed(&original, "diagnostics", |value| {
        value.diagnostics = Arc::from([AnalysisDiagnostic::new(
            DiagnosticSeverity::Warning,
            "changed",
        )])
    });
    changed(&original, "rule capability", |value| {
        value.rule_capability = Some(RuleCapability {
            rule_class: "unrelated".into(),
            executable: false,
            test_kind: None,
        });
    });
    changed(&original, "execution groups", |value| {
        let group = crate::execution_groups::ConfiguredExecGroupRow::new(
            crate::exec_group::ConfiguredExecGroup::Default,
            Arc::from([]),
            Arc::from([]),
            value.actions[0].context.clone(),
        )
        .unwrap();
        value.exec_groups = Some(
            ConfiguredExecGroupCollection::new(
                vec![group],
                Arc::from([]),
                ToolchainTopology::new(Vec::new(), None).unwrap(),
                false,
            )
            .unwrap(),
        );
    });
    changed(&original, "platform fact", |value| {
        value.platform_semantic_fact = Some(PlatformSemanticFact {
            exec_properties: Arc::from([]),
            missing_toolchain_error: Some(Arc::from("changed")),
        })
    });
    changed(&original, "constraint default", |value| {
        value.constraint_setting_default =
            Some(Arc::new(CanonicalLabel::parse("@@//:default").unwrap()))
    });
    changed(&original, "runfiles package closure", |value| {
        value.runfiles_packages = RunfilesPackageDepset::new(
            DepsetOrder::Default,
            vec![Arc::new(RunfilesPackageMetadata::new(
                key("metadata").label().package().clone(),
                Arc::new(RunfilesRepositoryMapping::empty()),
            ))],
            Vec::new(),
        )
        .unwrap();
    });
    changed(&original, "action count", |value| {
        value.actions = Arc::from([value.actions[0].clone()])
    });
    changed(&original, "action context", |value| {
        let mut actions = value.actions.to_vec();
        actions[0].context =
            Arc::new(ConfiguredActionOwnerContext::unresolved_default(key("other")).unwrap());
        value.actions = actions.into();
    });
    assert_eq!(original, original.clone());
}
